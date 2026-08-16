//! Shared frame-pipeline accountant.
//!
//! Turns neutral per-frame observations, queue depths, and peer-thread snapshots
//! into a [`FramePipelineReport`] for overlays and perf output. Flat clients keep
//! their incremental convenience API, while XR and offscreen/perf consumers feed
//! the same accountant through [`FramePipelineAccountant::record_prebuilt`].

use std::sync::Arc;

use mclone_diagnostics::{
    BudgetDecisionPanelReport, DiagnosticLaneAvailability, FrameAccountingConfig, FrameAccumulator,
    FrameObservation, FramePipelineReport, PeerThreadActivityReport, PeerThreadId,
    PeerThreadPanelReport, QueueAgeReport, QueueAgeTracker, QueueId, QueuePanelReport, StageId,
    StageSpan,
};
use mclone_server::WorkerFrameMetrics;

use crate::frame_render::RenderStreamStats;
use crate::{RenderSectionSyncTiming, SingleViewRuntimeStats};

pub const LIVE_FRAME_HISTORY_CAPACITY: usize = 512;
pub const LIVE_RICH_REPORT_INTERVAL_FRAMES: u64 = 30;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FramePipelineQueueDepths {
    pub observed: bool,
    pub server_owned_lanes_remote: bool,
    pub inbound_updates: usize,
    pub host_publication_runner: usize,
    pub host_publication_worldgen: usize,
    pub host_publication_light: usize,
    pub render_compile_jobs: usize,
    pub completed_results: usize,
    pub completed_results_processed: usize,
    pub upload_work: usize,
    pub upload_work_processed: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePipelinePeerThreadInput {
    pub server_owned_lanes_remote: bool,
    pub server_pending_jobs: usize,
    pub server_busy_ms: f64,
    pub worldgen_metrics: WorkerFrameMetrics,
    pub worldgen_pending_jobs: usize,
    pub light_metrics: WorkerFrameMetrics,
    pub light_pending_jobs: usize,
    pub render_compile_pending_jobs: usize,
    pub render_compile_submitted: usize,
    pub render_compile_completed: usize,
    pub render_compile_busy_ms: f64,
    pub render_compile_max_task_ms: f64,
    pub render_compile_idle_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePipelinePeerThreadAccumulator {
    aggregate: FramePipelinePeerThreadInput,
}

impl FramePipelinePeerThreadAccumulator {
    pub fn observe(&mut self, input: FramePipelinePeerThreadInput) {
        self.aggregate.server_owned_lanes_remote = input.server_owned_lanes_remote;
        self.aggregate.server_pending_jobs = self
            .aggregate
            .server_pending_jobs
            .max(input.server_pending_jobs);
        self.aggregate.server_busy_ms += sanitize_ms(input.server_busy_ms);
        self.aggregate.worldgen_metrics = input.worldgen_metrics;
        self.aggregate.worldgen_pending_jobs = self
            .aggregate
            .worldgen_pending_jobs
            .max(input.worldgen_pending_jobs);
        self.aggregate.light_metrics = input.light_metrics;
        self.aggregate.light_pending_jobs = self
            .aggregate
            .light_pending_jobs
            .max(input.light_pending_jobs);
        self.aggregate.render_compile_pending_jobs = self
            .aggregate
            .render_compile_pending_jobs
            .max(input.render_compile_pending_jobs);
        self.aggregate.render_compile_submitted = self
            .aggregate
            .render_compile_submitted
            .saturating_add(input.render_compile_submitted);
        self.aggregate.render_compile_completed = self
            .aggregate
            .render_compile_completed
            .saturating_add(input.render_compile_completed);
        self.aggregate.render_compile_busy_ms = self
            .aggregate
            .render_compile_busy_ms
            .max(sanitize_ms(input.render_compile_busy_ms));
        self.aggregate.render_compile_max_task_ms = self
            .aggregate
            .render_compile_max_task_ms
            .max(sanitize_ms(input.render_compile_max_task_ms));
        self.aggregate.render_compile_idle_ms = input.render_compile_idle_ms;
    }

    pub fn report(self) -> FramePipelinePeerThreadInput {
        self.aggregate
    }

    pub fn with_render_compile_idle_ms(mut self, idle_ms: Option<f64>) -> Self {
        self.aggregate.render_compile_idle_ms = idle_ms;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FramePipelineReportExtras {
    pub peer_threads: Option<FramePipelinePeerThreadInput>,
    pub budget_decision_panel: BudgetDecisionPanelReport,
}

impl FramePipelineReportExtras {
    pub fn with_peer_threads(mut self, peer_threads: FramePipelinePeerThreadInput) -> Self {
        self.peer_threads = Some(peer_threads);
        self
    }

    pub fn with_budget_decision_panel(
        mut self,
        budget_decision_panel: BudgetDecisionPanelReport,
    ) -> Self {
        self.budget_decision_panel = budget_decision_panel;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePipelineBudgetSignal {
    pub frame_index: u64,
    pub frame_wall_ms: f64,
    pub app_work_ms: f64,
    pub headroom_ms: Option<f64>,
    pub render_queue_depth: u64,
    pub render_queue_oldest_age_ms: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct FramePipelineAccountingUpdate {
    pub budget_signal: FramePipelineBudgetSignal,
    pub published_report: Option<(Arc<FramePipelineReport>, u64)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePipelineStageTiming {
    pub host_session_commands_ms: f64,
    pub completed_result_acceptance_ms: f64,
    pub render_admission_dirty_ready_scan_ms: f64,
    pub render_admission_request_build_ms: f64,
    pub render_admission_worker_submit_ms: f64,
    pub render_admission_prepared_record_maintenance_ms: f64,
    pub render_section_admission_ms: f64,
    pub upload_apply_ms: f64,
    pub prepared_draw_records_ms: f64,
    pub draw_encode_ms: f64,
}

#[derive(Clone, Debug)]
pub struct FramePipelineAccountant {
    config: FrameAccountingConfig,
    mode: FramePipelineAccountingMode,
    accumulator: FrameAccumulator,
    pending: Option<FrameObservation>,
    pending_clock_advance_ms: f64,
    queue_trackers: FramePipelineQueueTrackers,
    now_ms: f64,
    latest_report: Option<Arc<FramePipelineReport>>,
    latest_budget_signal: Option<FramePipelineBudgetSignal>,
    revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FramePipelineAccountingMode {
    Exact {
        publish_on_record: bool,
    },
    Live {
        history_capacity: usize,
        report_interval_frames: u64,
    },
}

impl Default for FramePipelineAccountant {
    fn default() -> Self {
        Self::new_live(frame_accounting_config(None))
    }
}

impl FramePipelineAccountant {
    /// Exact accumulator that publishes a rich report after every observation.
    ///
    /// This is intended for tests and non-frame-critical reconstruction.
    /// Frame-critical finite benchmarks should use [`Self::new_exact_on_demand`].
    pub fn new(config: FrameAccountingConfig) -> Self {
        Self::with_mode(
            config,
            FramePipelineAccountingMode::Exact {
                publish_on_record: true,
            },
        )
    }

    /// Exact accumulator that defers rich report construction until explicitly
    /// requested.
    ///
    /// This is the finite benchmark mode for frame-critical capture loops:
    /// recording remains incremental while percentile sorting and report
    /// allocation happen once, after the measured window.
    pub fn new_exact_on_demand(config: FrameAccountingConfig) -> Self {
        Self::with_mode(
            config,
            FramePipelineAccountingMode::Exact {
                publish_on_record: false,
            },
        )
    }

    /// Bounded always-on accumulator for interactive clients.
    pub fn new_live(config: FrameAccountingConfig) -> Self {
        Self::with_mode(
            config,
            FramePipelineAccountingMode::Live {
                history_capacity: LIVE_FRAME_HISTORY_CAPACITY,
                report_interval_frames: LIVE_RICH_REPORT_INTERVAL_FRAMES,
            },
        )
    }

    fn with_mode(config: FrameAccountingConfig, mode: FramePipelineAccountingMode) -> Self {
        Self {
            config,
            mode,
            accumulator: accumulator_for_mode(config, mode),
            pending: None,
            pending_clock_advance_ms: 0.0,
            queue_trackers: FramePipelineQueueTrackers::new(),
            now_ms: 0.0,
            latest_report: None,
            latest_budget_signal: None,
            revision: 0,
        }
    }

    pub fn set_config(&mut self, config: FrameAccountingConfig) {
        if self.config == config {
            return;
        }
        self.config = config;
        self.accumulator = accumulator_for_mode(config, self.mode);
        self.pending = None;
        self.pending_clock_advance_ms = 0.0;
        self.queue_trackers = FramePipelineQueueTrackers::new();
        self.now_ms = 0.0;
        self.latest_report = None;
        self.latest_budget_signal = None;
    }

    /// Opens a new frame. `frame_period_ms` is the real inter-frame interval and
    /// only advances the queue-age clock; the observation's wall is not the period
    /// but the frame's own active span, stamped in `finish_frame` so that it covers
    /// exactly the stage spans recorded for this frame (see conservation invariant).
    pub fn begin_frame(&mut self, frame_period_ms: f64, target_period_ms: Option<f64>) {
        self.refresh_target(target_period_ms);
        self.pending_clock_advance_ms = sanitize_ms(frame_period_ms);
        let frame_index = self.accumulator.frames_recorded().saturating_add(1);
        self.pending = Some(FrameObservation::new(frame_index, 0.0));
    }

    pub fn record_runtime_poll(&mut self, elapsed_ms: f64) {
        self.push_stage(StageSpan::new(StageId::HostSessionCommands, elapsed_ms));
    }

    pub fn record_render_section_sync(
        &mut self,
        timing: RenderSectionSyncTiming,
        total_sync_ms: f64,
    ) {
        for span in render_section_sync_stage_spans(timing, total_sync_ms) {
            self.push_stage(span);
        }
    }

    pub fn record_upload_apply(&mut self, upload_ms: f64) {
        self.push_stage(StageSpan::new(StageId::UploadApply, upload_ms));
    }

    /// Closes the current frame. `frame_active_ms` is the frame's own active wall
    /// span (frame start to post-present), measured from the same clock as the
    /// stage spans, so it is guaranteed to cover their sum and satisfy the
    /// accumulator's conservation invariant regardless of pacing sleep or hitches.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_frame(
        &mut self,
        frame_active_ms: f64,
        render_ms: f64,
        surface_acquire_ms: f64,
        surface_submit_ms: f64,
        surface_present_ms: f64,
        runtime_stats: Option<SingleViewRuntimeStats>,
        render_stats: RenderStreamStats,
        budget_decision_panel: BudgetDecisionPanelReport,
    ) -> Option<FramePipelineAccountingUpdate> {
        let present_wait_ms = sanitize_ms(surface_acquire_ms)
            + sanitize_ms(surface_submit_ms)
            + sanitize_ms(surface_present_ms);
        self.push_stage(StageSpan::new(
            StageId::DrawEncode,
            (sanitize_ms(render_ms) - present_wait_ms).max(0.0),
        ));
        self.push_stage(StageSpan::new(
            StageId::GpuExecutionPresentationWait,
            present_wait_ms,
        ));

        let Some(mut observation) = self.pending.take() else {
            return None;
        };
        let frame_active_ms = sanitize_ms(frame_active_ms);
        observation.frame_wall_ms = frame_active_ms;
        observation.app_work_ms = Some(frame_active_ms);
        let queues = frame_pipeline_queue_depths(runtime_stats.as_ref(), render_stats);
        let clock_advance_ms = self.pending_clock_advance_ms;
        self.pending_clock_advance_ms = 0.0;
        Some(self.record_prebuilt_with_clock_advance(
            observation,
            queues,
            FramePipelineReportExtras::default().with_budget_decision_panel(budget_decision_panel),
            clock_advance_ms,
        ))
    }

    /// Records a complete observation from XR, offscreen, or another driver
    /// that already owns its timing brackets.
    pub fn record_prebuilt(
        &mut self,
        observation: FrameObservation,
        queues: FramePipelineQueueDepths,
        extras: FramePipelineReportExtras,
    ) -> FramePipelineAccountingUpdate {
        let clock_advance_ms = observation.frame_wall_ms;
        self.record_prebuilt_with_clock_advance(observation, queues, extras, clock_advance_ms)
    }

    /// Records a complete observation whose queue clock is an absolute sample
    /// timestamp, as used by historical/offscreen report reconstruction.
    pub fn record_prebuilt_at(
        &mut self,
        observation: FrameObservation,
        queues: FramePipelineQueueDepths,
        extras: FramePipelineReportExtras,
        observed_at_ms: f64,
    ) -> FramePipelineAccountingUpdate {
        self.now_ms = sanitize_ms(observed_at_ms).max(self.now_ms);
        self.record_prebuilt_at_current_clock(observation, queues, extras)
    }

    pub fn latest_report(&self) -> Option<(Arc<FramePipelineReport>, u64)> {
        self.latest_report
            .as_ref()
            .map(|report| (report.clone(), self.revision))
    }

    /// Builds and publishes a rich report from all observations retained by an
    /// exact on-demand accountant.
    ///
    /// Callers should invoke this outside the measured frame loop. Live and
    /// publish-on-record accountants may also use it for an explicit refresh.
    pub fn publish_report_now(
        &mut self,
        extras: FramePipelineReportExtras,
    ) -> Option<(Arc<FramePipelineReport>, u64)> {
        if self.accumulator.is_empty() {
            return None;
        }
        let report = self.build_report(extras);
        Some((report, self.revision))
    }

    pub fn latest_budget_signal(&self) -> Option<FramePipelineBudgetSignal> {
        self.latest_budget_signal
    }

    /// Largest current depth among the tracked pipeline queues.
    ///
    /// This scalar query avoids constructing a rich queue/report panel in
    /// frame-critical benchmark loops.
    pub fn current_max_queue_depth(&self) -> u64 {
        self.queue_trackers.current_max_depth()
    }

    pub fn retained_frame_count(&self) -> usize {
        self.accumulator.retained_len()
    }

    pub fn next_frame_index(&self) -> u64 {
        self.accumulator.frames_recorded().saturating_add(1)
    }

    fn refresh_target(&mut self, target_period_ms: Option<f64>) {
        let target_period_ms = target_period_ms.and_then(finite_positive);
        if same_target(self.config.target_period_ms, target_period_ms) {
            return;
        }
        let config = frame_accounting_config(target_period_ms);
        self.config = config;
        self.accumulator = accumulator_for_mode(config, self.mode);
        self.pending = None;
        self.pending_clock_advance_ms = 0.0;
        self.latest_report = None;
        self.latest_budget_signal = None;
    }

    fn push_stage(&mut self, span: StageSpan) {
        if let Some(observation) = self.pending.as_mut()
            && span.elapsed_ms > 0.0
        {
            observation.stage_spans.push(span);
        }
    }

    fn record_prebuilt_with_clock_advance(
        &mut self,
        observation: FrameObservation,
        queues: FramePipelineQueueDepths,
        extras: FramePipelineReportExtras,
        clock_advance_ms: f64,
    ) -> FramePipelineAccountingUpdate {
        self.now_ms += sanitize_ms(clock_advance_ms);
        self.record_prebuilt_at_current_clock(observation, queues, extras)
    }

    fn record_prebuilt_at_current_clock(
        &mut self,
        observation: FrameObservation,
        queues: FramePipelineQueueDepths,
        extras: FramePipelineReportExtras,
    ) -> FramePipelineAccountingUpdate {
        let frame_index = observation.frame_index;
        let frame_wall_ms = observation.frame_wall_ms;
        let app_work_ms = observation.computed_app_work_ms();
        let headroom_ms = observation.headroom_ms(self.config.target_period_ms);
        self.accumulator.record_frame(observation);
        self.queue_trackers.reconcile(queues, self.now_ms);
        let (render_queue_depth, render_queue_oldest_age_ms) =
            self.queue_trackers.render_queue_pressure(self.now_ms);
        let budget_signal = FramePipelineBudgetSignal {
            frame_index,
            frame_wall_ms,
            app_work_ms,
            headroom_ms,
            render_queue_depth,
            render_queue_oldest_age_ms,
        };
        self.latest_budget_signal = Some(budget_signal);
        let should_publish = match self.mode {
            FramePipelineAccountingMode::Exact { publish_on_record } => publish_on_record,
            FramePipelineAccountingMode::Live {
                report_interval_frames,
                ..
            } => {
                self.latest_report.is_none()
                    || self
                        .accumulator
                        .frames_recorded()
                        .is_multiple_of(report_interval_frames)
            }
        };
        if !should_publish {
            return FramePipelineAccountingUpdate {
                budget_signal,
                published_report: None,
            };
        }
        let report = self.build_report(extras);
        FramePipelineAccountingUpdate {
            budget_signal,
            published_report: Some((report, self.revision)),
        }
    }

    fn build_report(&mut self, extras: FramePipelineReportExtras) -> Arc<FramePipelineReport> {
        let queue_panel = self.queue_trackers.report(self.now_ms);
        let peer_thread_panel = extras.peer_threads.map_or_else(
            PeerThreadPanelReport::empty,
            frame_pipeline_peer_thread_panel,
        );
        let report = FramePipelineReport::new(self.accumulator.summary_report(), queue_panel)
            .with_peer_thread_panel(peer_thread_panel)
            .with_budget_decision_panel(extras.budget_decision_panel);
        self.revision = self.revision.saturating_add(1);
        let report = Arc::new(report);
        self.latest_report = Some(report.clone());
        report
    }
}

fn accumulator_for_mode(
    config: FrameAccountingConfig,
    mode: FramePipelineAccountingMode,
) -> FrameAccumulator {
    match mode {
        FramePipelineAccountingMode::Exact { .. } => FrameAccumulator::new(config),
        FramePipelineAccountingMode::Live {
            history_capacity, ..
        } => FrameAccumulator::new_rolling(config, history_capacity),
    }
}

#[derive(Clone, Debug)]
struct FramePipelineQueueTrackers {
    server_owned_lanes_remote: bool,
    inbound_updates: QueueAgeTracker,
    host_publication: QueueAgeTracker,
    host_publication_runner: QueueAgeTracker,
    host_publication_worldgen: QueueAgeTracker,
    host_publication_light: QueueAgeTracker,
    render_compile_jobs: QueueAgeTracker,
    completed_results: QueueAgeTracker,
    upload_work: QueueAgeTracker,
}

impl FramePipelineQueueTrackers {
    fn new() -> Self {
        Self {
            server_owned_lanes_remote: false,
            inbound_updates: QueueAgeTracker::new(QueueId::InboundUpdates),
            host_publication: QueueAgeTracker::new(QueueId::HostPublication),
            host_publication_runner: QueueAgeTracker::new(QueueId::HostPublicationRunner),
            host_publication_worldgen: QueueAgeTracker::new(QueueId::HostPublicationWorldgen),
            host_publication_light: QueueAgeTracker::new(QueueId::HostPublicationLight),
            render_compile_jobs: QueueAgeTracker::new(QueueId::RenderCompileJobs),
            completed_results: QueueAgeTracker::new(QueueId::CompletedRenderResults),
            upload_work: QueueAgeTracker::new(QueueId::UploadWork),
        }
    }

    fn reconcile(&mut self, queues: FramePipelineQueueDepths, now_ms: f64) {
        let host_publication_remote = queues.server_owned_lanes_remote;
        self.server_owned_lanes_remote = host_publication_remote;
        if queues.observed {
            self.inbound_updates
                .reconcile_depth(usize_to_u64(queues.inbound_updates), now_ms);
            if host_publication_remote {
                self.host_publication = QueueAgeTracker::new(QueueId::HostPublication);
                self.host_publication_runner = QueueAgeTracker::new(QueueId::HostPublicationRunner);
                self.host_publication_worldgen =
                    QueueAgeTracker::new(QueueId::HostPublicationWorldgen);
                self.host_publication_light = QueueAgeTracker::new(QueueId::HostPublicationLight);
            } else {
                let runner_depth = queues.host_publication_runner;
                let worldgen_depth = queues.host_publication_worldgen;
                let light_depth = queues.host_publication_light;
                let publication_depth = runner_depth
                    .saturating_add(worldgen_depth)
                    .saturating_add(light_depth);
                self.host_publication
                    .reconcile_depth(usize_to_u64(publication_depth), now_ms);
                self.host_publication_runner
                    .reconcile_depth(usize_to_u64(runner_depth), now_ms);
                self.host_publication_worldgen
                    .reconcile_depth(usize_to_u64(worldgen_depth), now_ms);
                self.host_publication_light
                    .reconcile_depth(usize_to_u64(light_depth), now_ms);
            }
            self.render_compile_jobs
                .reconcile_depth(usize_to_u64(queues.render_compile_jobs), now_ms);
            let completed_results_processed = usize_to_u64(queues.completed_results_processed);
            if completed_results_processed > 0 {
                self.completed_results
                    .enqueue(completed_results_processed, now_ms);
                self.completed_results
                    .dequeue(completed_results_processed, now_ms);
            }
            self.completed_results
                .reconcile_depth(usize_to_u64(queues.completed_results), now_ms);
            let upload_work_processed = usize_to_u64(queues.upload_work_processed);
            if upload_work_processed > 0 {
                self.upload_work.enqueue(upload_work_processed, now_ms);
                self.upload_work.dequeue(upload_work_processed, now_ms);
            }
            self.upload_work
                .reconcile_depth(usize_to_u64(queues.upload_work), now_ms);
        }
    }

    fn report(&mut self, now_ms: f64) -> QueuePanelReport {
        let host_publication_remote = self.server_owned_lanes_remote;
        QueuePanelReport::new(vec![
            self.inbound_updates.report(now_ms),
            self.completed_results.report(now_ms),
            self.upload_work.report(now_ms),
            if host_publication_remote {
                remote_host_queue_report(QueueId::HostPublication)
            } else {
                self.host_publication.report(now_ms)
            },
            if host_publication_remote {
                remote_host_queue_report(QueueId::HostPublicationRunner)
            } else {
                self.host_publication_runner.report(now_ms)
            },
            if host_publication_remote {
                remote_host_queue_report(QueueId::HostPublicationWorldgen)
            } else {
                self.host_publication_worldgen.report(now_ms)
            },
            if host_publication_remote {
                remote_host_queue_report(QueueId::HostPublicationLight)
            } else {
                self.host_publication_light.report(now_ms)
            },
            self.render_compile_jobs.report(now_ms),
        ])
    }

    fn current_max_depth(&self) -> u64 {
        let host_publication_depth = if self.server_owned_lanes_remote {
            0
        } else {
            self.host_publication_runner
                .depth()
                .saturating_add(self.host_publication_worldgen.depth())
                .saturating_add(self.host_publication_light.depth())
        };
        [
            self.inbound_updates.depth(),
            host_publication_depth,
            self.render_compile_jobs.depth(),
            self.completed_results.depth(),
            self.upload_work.depth(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }

    fn render_queue_pressure(&self, now_ms: f64) -> (u64, Option<f64>) {
        let depth = self
            .render_compile_jobs
            .depth()
            .saturating_add(self.completed_results.depth())
            .saturating_add(self.upload_work.depth());
        let oldest_age_ms = [
            self.render_compile_jobs.oldest_age_ms(now_ms),
            self.completed_results.oldest_age_ms(now_ms),
            self.upload_work.oldest_age_ms(now_ms),
        ]
        .into_iter()
        .flatten()
        .max_by(f64::total_cmp);
        (depth, oldest_age_ms)
    }
}

pub fn frame_pipeline_queue_depths(
    runtime: Option<&SingleViewRuntimeStats>,
    render: RenderStreamStats,
) -> FramePipelineQueueDepths {
    FramePipelineQueueDepths {
        observed: true,
        server_owned_lanes_remote: runtime
            .is_some_and(|stats| stats.host_mode.server_owned_lanes_are_remote()),
        inbound_updates: runtime.map_or(0, |stats| stats.server_update_queue_depth),
        host_publication_runner: runtime.map_or(0, |stats| stats.pending_publications),
        host_publication_worldgen: runtime.map_or(0, |stats| {
            stats.scheduler_pending_worldgen_publication_chunks
        }),
        host_publication_light: runtime
            .map_or(0, |stats| stats.scheduler_pending_light_publications),
        render_compile_jobs: runtime.map_or(render.last_pending_compile_jobs, |stats| {
            stats.pending_render_compile_jobs
        }),
        // Preserve the flat accountant's sampled-depth semantics. XR and perf
        // inputs use the processed/depth pair below to retain their event ages.
        completed_results: render.last_completed_compile_section_count,
        completed_results_processed: 0,
        upload_work: render.last_deferred_section_count,
        upload_work_processed: 0,
    }
}

fn remote_host_queue_report(queue: QueueId) -> QueueAgeReport {
    QueueAgeReport::snapshot(queue, 0, 0, 0, None, 0.0, 0)
        .with_availability(DiagnosticLaneAvailability::RemoteHost)
}

pub fn render_section_sync_stage_spans(
    timing: RenderSectionSyncTiming,
    total_sync_ms: f64,
) -> Vec<StageSpan> {
    let dirty_ready_scan_ms = timing.dirty_seed_ms + timing.prepare_ms;
    let request_build_ms = timing.submit_snapshot_ms + timing.submit_request_build_ms;
    let worker_submit_ms = timing.submit_compiler_ms;
    let prepared_record_ms = timing.submit_mark_inflight_ms
        + timing.submit_apply_ready_plan_ms
        + timing.submit_ready_update_ms;
    let attributed_ms = timing.completed_result_accept_ms
        + dirty_ready_scan_ms
        + request_build_ms
        + worker_submit_ms
        + prepared_record_ms;
    let unattributed_ms = (total_sync_ms - attributed_ms).max(0.0);
    frame_pipeline_stage_spans(FramePipelineStageTiming {
        completed_result_acceptance_ms: timing.completed_result_accept_ms,
        render_admission_dirty_ready_scan_ms: dirty_ready_scan_ms,
        render_admission_request_build_ms: request_build_ms,
        render_admission_worker_submit_ms: worker_submit_ms,
        render_admission_prepared_record_maintenance_ms: prepared_record_ms,
        render_section_admission_ms: unattributed_ms,
        ..FramePipelineStageTiming::default()
    })
    .into_iter()
    .filter(|span| {
        matches!(
            span.stage,
            StageId::CompletedResultAcceptance
                | StageId::RenderAdmissionDirtyReadyScan
                | StageId::RenderAdmissionRequestBuild
                | StageId::RenderAdmissionWorkerSubmit
                | StageId::RenderAdmissionPreparedRecordMaintenance
                | StageId::RenderSectionAdmission
        )
    })
    .collect()
}

pub fn frame_pipeline_stage_spans(timing: FramePipelineStageTiming) -> Vec<StageSpan> {
    [
        (
            StageId::HostSessionCommands,
            timing.host_session_commands_ms,
        ),
        (
            StageId::CompletedResultAcceptance,
            timing.completed_result_acceptance_ms,
        ),
        (
            StageId::RenderAdmissionDirtyReadyScan,
            timing.render_admission_dirty_ready_scan_ms,
        ),
        (
            StageId::RenderAdmissionRequestBuild,
            timing.render_admission_request_build_ms,
        ),
        (
            StageId::RenderAdmissionWorkerSubmit,
            timing.render_admission_worker_submit_ms,
        ),
        (
            StageId::RenderAdmissionPreparedRecordMaintenance,
            timing.render_admission_prepared_record_maintenance_ms,
        ),
        (
            StageId::RenderSectionAdmission,
            timing.render_section_admission_ms,
        ),
        (StageId::UploadApply, timing.upload_apply_ms),
        (
            StageId::PreparedDrawRecords,
            timing.prepared_draw_records_ms,
        ),
        (StageId::DrawEncode, timing.draw_encode_ms),
    ]
    .into_iter()
    .map(|(stage, elapsed_ms)| StageSpan::new(stage, elapsed_ms))
    .collect()
}

pub fn frame_pipeline_peer_thread_panel(
    input: FramePipelinePeerThreadInput,
) -> PeerThreadPanelReport {
    PeerThreadPanelReport::new(vec![
        server_runner_peer_report(
            input.server_owned_lanes_remote,
            input.server_pending_jobs,
            input.server_busy_ms,
        ),
        worker_metrics_peer_report(
            PeerThreadId::Worldgen,
            input.server_owned_lanes_remote,
            input.worldgen_metrics,
            input.worldgen_pending_jobs,
        ),
        worker_metrics_peer_report(
            PeerThreadId::LightStatus,
            input.server_owned_lanes_remote,
            input.light_metrics,
            input.light_pending_jobs,
        ),
        PeerThreadActivityReport::new(PeerThreadId::RenderCompileWorkers)
            .with_pending_jobs(usize_to_u64(input.render_compile_pending_jobs))
            .with_frames(
                usize_to_u64(input.render_compile_submitted),
                usize_to_u64(input.render_compile_completed),
            )
            .with_request_timing_ms(
                None,
                input.render_compile_busy_ms,
                input.render_compile_max_task_ms,
            )
            .with_busy_idle_ms(
                Some(input.render_compile_busy_ms),
                input.render_compile_idle_ms,
            ),
    ])
}

fn server_runner_peer_report(
    server_lanes_remote: bool,
    pending_jobs: usize,
    busy_ms: f64,
) -> PeerThreadActivityReport {
    let report = PeerThreadActivityReport::new(PeerThreadId::ServerRunner)
        .with_pending_jobs(usize_to_u64(pending_jobs))
        .with_busy_idle_ms(Some(busy_ms), None);
    if server_lanes_remote {
        report.with_availability(DiagnosticLaneAvailability::RemoteHost)
    } else {
        report
    }
}

fn worker_metrics_peer_report(
    lane: PeerThreadId,
    server_lanes_remote: bool,
    metrics: WorkerFrameMetrics,
    pending_jobs: usize,
) -> PeerThreadActivityReport {
    let report = PeerThreadActivityReport::new(lane)
        .with_pending_jobs(usize_to_u64(pending_jobs))
        .with_frames(
            usize_to_u64(metrics.request_frames),
            usize_to_u64(metrics.inbound_frames),
        )
        .with_bytes(
            usize_to_u64(metrics.request_bytes),
            usize_to_u64(metrics.inbound_bytes),
        )
        .with_max_pending_frames(usize_to_u64(metrics.max_pending_frames))
        .with_request_timing_ms(
            (metrics.last_request_us > 0).then_some(micros_to_ms(metrics.last_request_us)),
            micros_to_ms(metrics.total_request_us),
            micros_to_ms(metrics.max_request_us),
        )
        .with_busy_idle_ms(Some(micros_to_ms(metrics.total_request_us)), None);
    if server_lanes_remote {
        report.with_availability(DiagnosticLaneAvailability::RemoteHost)
    } else {
        report
    }
}

fn frame_accounting_config(target_period_ms: Option<f64>) -> FrameAccountingConfig {
    target_period_ms.map_or_else(
        FrameAccountingConfig::without_budget,
        FrameAccountingConfig::from_target_period_ms,
    )
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

fn finite_positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn same_target(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => (left - right).abs() <= 0.001,
        (None, None) => true,
        _ => false,
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn micros_to_ms(value: u128) -> f64 {
    value as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accountant_builds_latest_report_with_queues() {
        let mut accounting = FramePipelineAccountant::default();
        accounting.begin_frame(16.0, Some(16.0));
        accounting.record_runtime_poll(1.0);
        accounting.record_upload_apply(2.0);
        accounting.finish_frame(
            16.0,
            4.0,
            0.25,
            0.25,
            0.5,
            None,
            RenderStreamStats {
                last_deferred_section_count: 3,
                last_completed_compile_section_count: 1,
                ..RenderStreamStats::default()
            },
            BudgetDecisionPanelReport::empty(),
        );

        let (report, revision) = accounting.latest_report().expect("report");
        assert_eq!(revision, 1);
        assert_eq!(report.frame_summary.latest_frame_wall_ms, 16.0);
        assert_eq!(report.frame_summary.latest_app_work_ms, 16.0);
        assert_eq!(report.stage_spans.len(), 4);
        assert_eq!(report.queue_panel.queues.len(), 8);
        assert!(
            report
                .queue_panel
                .queues
                .iter()
                .any(|queue| queue.queue == QueueId::UploadWork && queue.depth == 3)
        );
    }

    #[test]
    fn accountant_marks_remote_host_publication_queue() {
        let mut accounting = FramePipelineAccountant::default();
        accounting.begin_frame(16.0, Some(16.0));
        accounting.finish_frame(
            16.0,
            4.0,
            0.25,
            0.25,
            0.5,
            Some(runtime_stats(
                crate::host_mode::SingleViewHostMode::RemoteDedicated,
            )),
            RenderStreamStats::default(),
            BudgetDecisionPanelReport::empty(),
        );

        let (report, _) = accounting.latest_report().expect("report");
        let host_publication = report
            .queue_panel
            .queues
            .iter()
            .find(|queue| queue.queue == QueueId::HostPublication)
            .expect("host publication queue");
        assert_eq!(
            host_publication.availability,
            DiagnosticLaneAvailability::RemoteHost
        );
        assert_eq!(host_publication.depth, 0);

        let inbound_updates = report
            .queue_panel
            .queues
            .iter()
            .find(|queue| queue.queue == QueueId::InboundUpdates)
            .expect("inbound update queue");
        assert_eq!(
            inbound_updates.availability,
            DiagnosticLaneAvailability::Local
        );
        assert_eq!(inbound_updates.depth, 2);
    }

    #[test]
    fn peer_thread_accumulator_preserves_window_max_and_sum_semantics() {
        let mut peers = FramePipelinePeerThreadAccumulator::default();
        peers.observe(FramePipelinePeerThreadInput {
            server_pending_jobs: 2,
            server_busy_ms: 1.5,
            render_compile_pending_jobs: 3,
            render_compile_submitted: 2,
            render_compile_completed: 1,
            render_compile_busy_ms: 4.0,
            ..FramePipelinePeerThreadInput::default()
        });
        peers.observe(FramePipelinePeerThreadInput {
            server_pending_jobs: 1,
            server_busy_ms: 2.5,
            render_compile_pending_jobs: 5,
            render_compile_submitted: 4,
            render_compile_completed: 3,
            render_compile_busy_ms: 6.0,
            ..FramePipelinePeerThreadInput::default()
        });

        let report = peers.report();
        assert_eq!(report.server_pending_jobs, 2);
        assert_eq!(report.server_busy_ms, 4.0);
        assert_eq!(report.render_compile_pending_jobs, 5);
        assert_eq!(report.render_compile_submitted, 6);
        assert_eq!(report.render_compile_completed, 4);
        assert_eq!(report.render_compile_busy_ms, 6.0);
    }

    #[test]
    fn sync_stage_adapter_keeps_the_six_stage_report_shape() {
        let spans = render_section_sync_stage_spans(RenderSectionSyncTiming::default(), 0.0);
        assert_eq!(spans.len(), 6);
        assert_eq!(spans[0].stage, StageId::CompletedResultAcceptance);
        assert_eq!(spans[5].stage, StageId::RenderSectionAdmission);
    }

    #[test]
    fn live_accountant_bounds_history_and_decimates_rich_reports() {
        let mut accounting =
            FramePipelineAccountant::new_live(FrameAccountingConfig::from_target_period_ms(10.0));
        let frame_count = LIVE_FRAME_HISTORY_CAPACITY as u64 + 88;
        let mut published_frames = Vec::new();
        for frame_index in 1..=frame_count {
            let update = accounting.record_prebuilt(
                FrameObservation::new(frame_index, frame_index as f64),
                FramePipelineQueueDepths::default(),
                FramePipelineReportExtras::default(),
            );
            assert_eq!(update.budget_signal.frame_index, frame_index);
            if update.published_report.is_some() {
                published_frames.push(frame_index);
            }
        }

        assert_eq!(published_frames.first(), Some(&1));
        assert!(
            published_frames
                .iter()
                .skip(1)
                .all(|frame| frame.is_multiple_of(LIVE_RICH_REPORT_INTERVAL_FRAMES))
        );
        assert_eq!(
            accounting.retained_frame_count(),
            LIVE_FRAME_HISTORY_CAPACITY
        );
        assert_eq!(accounting.next_frame_index(), frame_count + 1);
        assert_eq!(
            accounting
                .latest_budget_signal()
                .expect("budget signal")
                .frame_index,
            frame_count
        );
        let (report, revision) = accounting.latest_report().expect("rich report");
        let expected_last_publish = frame_count - frame_count % LIVE_RICH_REPORT_INTERVAL_FRAMES;
        assert_eq!(report.frame_summary.frames, expected_last_publish);
        assert_eq!(
            revision,
            1 + expected_last_publish / LIVE_RICH_REPORT_INTERVAL_FRAMES
        );
        assert_eq!(
            report.frame_summary.percentile_window_frames,
            LIVE_FRAME_HISTORY_CAPACITY as u64
        );
        assert_eq!(
            report.frame_summary.percentile_window_capacity,
            Some(LIVE_FRAME_HISTORY_CAPACITY as u64)
        );
    }

    #[test]
    fn exact_on_demand_accountant_defers_rich_report_construction() {
        let mut accounting = FramePipelineAccountant::new_exact_on_demand(
            FrameAccountingConfig::from_target_period_ms(10.0),
        );
        for frame_index in 1..=3 {
            let update = accounting.record_prebuilt(
                FrameObservation::new(frame_index, frame_index as f64),
                FramePipelineQueueDepths::default(),
                FramePipelineReportExtras::default(),
            );
            assert_eq!(update.budget_signal.frame_index, frame_index);
            assert!(update.published_report.is_none());
        }

        assert!(accounting.latest_report().is_none());
        assert_eq!(accounting.retained_frame_count(), 3);
        let (report, revision) = accounting
            .publish_report_now(FramePipelineReportExtras::default())
            .expect("on-demand report");
        assert_eq!(revision, 1);
        assert_eq!(report.frame_summary.frames, 3);
        assert_eq!(report.frame_summary.percentile_window_frames, 3);
        assert_eq!(report.frame_summary.percentile_window_capacity, None);
        assert_eq!(
            accounting.latest_report().expect("published report").1,
            revision
        );
    }

    #[test]
    fn current_max_queue_depth_does_not_require_report_publication() {
        let mut accounting = FramePipelineAccountant::new_exact_on_demand(
            FrameAccountingConfig::from_target_period_ms(10.0),
        );
        accounting.record_prebuilt(
            FrameObservation::new(1, 2.0),
            FramePipelineQueueDepths {
                observed: true,
                host_publication_runner: 3,
                host_publication_worldgen: 4,
                render_compile_jobs: 5,
                ..FramePipelineQueueDepths::default()
            },
            FramePipelineReportExtras::default(),
        );

        assert_eq!(accounting.current_max_queue_depth(), 7);
        assert!(accounting.latest_report().is_none());
    }

    #[test]
    fn live_accountant_stays_bounded_for_one_hour_at_sixty_hz() {
        const ONE_HOUR_AT_SIXTY_HZ: u64 = 60 * 60 * 60;
        let mut accounting = FramePipelineAccountant::new_live(
            FrameAccountingConfig::from_target_period_ms(1000.0 / 60.0),
        );

        for frame_index in 1..=ONE_HOUR_AT_SIXTY_HZ {
            let update = accounting.record_prebuilt(
                FrameObservation::new(frame_index, 4.0),
                FramePipelineQueueDepths::default(),
                FramePipelineReportExtras::default(),
            );
            assert_eq!(update.budget_signal.frame_index, frame_index);
        }

        assert_eq!(
            accounting.retained_frame_count(),
            LIVE_FRAME_HISTORY_CAPACITY
        );
        assert_eq!(accounting.next_frame_index(), ONE_HOUR_AT_SIXTY_HZ + 1);
        let (report, revision) = accounting.latest_report().expect("rich report");
        assert_eq!(report.frame_summary.frames, ONE_HOUR_AT_SIXTY_HZ);
        assert_eq!(
            report.frame_summary.percentile_window_frames,
            LIVE_FRAME_HISTORY_CAPACITY as u64
        );
        assert_eq!(
            revision,
            1 + ONE_HOUR_AT_SIXTY_HZ / LIVE_RICH_REPORT_INTERVAL_FRAMES
        );
    }

    fn runtime_stats(host_mode: crate::host_mode::SingleViewHostMode) -> SingleViewRuntimeStats {
        SingleViewRuntimeStats {
            host_mode,
            server_runner_kind: None,
            server_command_queue_depth: 0,
            server_update_queue_depth: 2,
            server_update_queue_bytes: 0,
            interest_center: mclone_core::ChunkPos::new(0, 0),
            render_distance: 2,
            chunk_tracking_radius: 3,
            loaded_chunks: 0,
            command_count: 0,
            update_count: 0,
            snapshot_update_count: 0,
            section_block_update_count: 0,
            unload_update_count: 0,
            pending_jobs: 0,
            pending_publications: 7,
            pending_persistence_loads: 0,
            pending_persistence_saves: 0,
            scheduler_pending_worldgen_publication_chunks: 3,
            scheduler_pending_light_publications: 4,
            pending_render_chunks: 0,
            pending_render_compile_jobs: 0,
            inflight_render_sections: 0,
            client_visible_chunks: 0,
            active_ticket_chunks: 0,
            loading_progress: None,
            tracked_players: 0,
            player_visible_chunks: 0,
            player_published_chunks: 0,
            player_published_visible_chunks: 0,
            player_missing_published_chunks: 0,
            player_published_outside_visible_chunks: 0,
            player_queued_snapshot_updates: 0,
            player_queued_unload_updates: 0,
            player_drained_snapshot_updates: 0,
            player_drained_unload_updates: 0,
            runner_emitted_snapshot_updates: 0,
            runner_emitted_unload_updates: 0,
            aggregate_player_ticket_chunks: 0,
            player_outbound_queue_depth: 0,
            max_player_visible_chunks: 0,
            max_player_outbound_queue_depth: 0,
            pending_unload_chunks: 0,
            block_ticking_chunks: 0,
            entity_ticking_chunks: 0,
            last_tick: 0,
            last_simulation_tick: 0,
            last_tick_unloads_processed: 0,
            last_simulation_block_tick_chunks: 0,
            last_simulation_entity_tick_chunks: 0,
            last_simulation_scheduler_tick_ms: 0.0,
            last_simulation_block_tick_ms: 0.0,
            last_simulation_fluid_tick_ms: 0.0,
            last_simulation_entity_tick_ms: 0.0,
            last_simulation_fluid_ticks_executed: 0,
            last_simulation_deferred_fluid_ticks: 0,
            last_simulation_fluid_mutated_blocks: 0,
            scheduled_fluid_ticks: 0,
        }
    }
}
