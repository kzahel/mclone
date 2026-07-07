use std::sync::Arc;

use mclone_diagnostics::{
    BudgetDecisionPanelReport, DiagnosticLaneAvailability, FrameAccountingConfig, FrameAccumulator,
    FrameObservation, FramePipelineReport, PeerThreadActivityReport, PeerThreadId,
    PeerThreadPanelReport, PercentileMethod, QueueAgeReport, QueueAgeTracker, QueueId,
    QueuePanelReport, StageId, StageSpan,
};

use crate::{
    XrTerrainFrameSummary, XrTerrainFrameTiming, XrTerrainHostMode, XrTerrainUploadSummary,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct XrFramePipelineHostTiming {
    pub frame_wall_ms: f64,
    pub wait_frame_ms: f64,
    pub controller_poll_ms: f64,
    pub rendered: bool,
    pub thread_cpu_ms: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct XrFramePipelineReporter {
    target_hz: Option<f64>,
    accumulator: FrameAccumulator,
    queue_trackers: XrFramePipelineQueueTrackers,
    now_ms: f64,
    latest_report: Option<Arc<FramePipelineReport>>,
    revision: u64,
}

impl XrFramePipelineReporter {
    pub fn new(target_hz: Option<f64>) -> Self {
        let target_hz = target_hz.and_then(finite_positive);
        Self {
            target_hz,
            accumulator: FrameAccumulator::new(frame_accounting_config(target_hz)),
            queue_trackers: XrFramePipelineQueueTrackers::new(),
            now_ms: 0.0,
            latest_report: None,
            revision: 0,
        }
    }

    pub fn set_target_hz(&mut self, target_hz: Option<f64>) {
        let target_hz = target_hz.and_then(finite_positive);
        if same_target_hz(self.target_hz, target_hz) {
            return;
        }
        self.target_hz = target_hz;
        self.accumulator = FrameAccumulator::new(frame_accounting_config(target_hz));
        self.queue_trackers = XrFramePipelineQueueTrackers::new();
        self.now_ms = 0.0;
        self.latest_report = None;
    }

    pub fn record_frame(
        &mut self,
        host: XrFramePipelineHostTiming,
        summary: Option<XrTerrainFrameSummary>,
    ) -> (Arc<FramePipelineReport>, u64) {
        self.record_frame_with_budget_decision_panel(
            host,
            summary,
            BudgetDecisionPanelReport::empty(),
        )
    }

    pub fn record_frame_with_budget_decision_panel(
        &mut self,
        host: XrFramePipelineHostTiming,
        summary: Option<XrTerrainFrameSummary>,
        budget_decision_panel: BudgetDecisionPanelReport,
    ) -> (Arc<FramePipelineReport>, u64) {
        self.now_ms += sanitize_ms(host.frame_wall_ms);
        let frame_index = self.accumulator.len() as u64 + 1;
        let app_work_ms =
            (sanitize_ms(host.frame_wall_ms) - sanitize_ms(host.wait_frame_ms)).max(0.0);
        let mut observation = FrameObservation::new(frame_index, host.frame_wall_ms)
            .with_wait_ms(host.wait_frame_ms)
            .with_app_work_ms(app_work_ms)
            .with_thread_cpu_ms(host.thread_cpu_ms)
            .with_rendered(host.rendered)
            .with_stage_span(StageSpan::new(
                StageId::GpuExecutionPresentationWait,
                host.wait_frame_ms,
            ))
            .with_stage_span(StageSpan::new(
                StageId::InputPoseEvents,
                host.controller_poll_ms,
            ));
        if let Some(summary) = summary {
            for span in xr_frame_pipeline_stage_spans(summary.timing) {
                observation = observation.with_stage_span(span);
            }
        }
        self.accumulator.record_frame(observation);

        let upload = summary.map(|summary| summary.upload);
        let queue_panel = self.queue_trackers.report(upload, self.now_ms);
        let peer_thread_panel = xr_frame_pipeline_peer_thread_panel(upload);
        let report = FramePipelineReport::new(self.accumulator.summary_report(), queue_panel)
            .with_peer_thread_panel(peer_thread_panel)
            .with_budget_decision_panel(budget_decision_panel);
        self.revision = self.revision.saturating_add(1);
        let report = Arc::new(report);
        self.latest_report = Some(report.clone());
        (report, self.revision)
    }

    pub fn latest_report(&self) -> Option<(Arc<FramePipelineReport>, u64)> {
        self.latest_report
            .as_ref()
            .map(|report| (report.clone(), self.revision))
    }
}

pub fn xr_frame_pipeline_stage_spans(timing: XrTerrainFrameTiming) -> Vec<StageSpan> {
    let dirty_ready_scan_ms = timing.runtime_dirty_seed_ms + timing.runtime_prepare_ms;
    let request_build_ms =
        timing.runtime_submit_snapshot_ms + timing.runtime_submit_request_build_ms;
    let worker_submit_ms = timing.runtime_submit_compiler_ms;
    let prepared_record_ms = timing.runtime_submit_mark_inflight_ms
        + timing.runtime_submit_apply_ready_plan_ms
        + timing.runtime_submit_ready_update_ms;
    let attributed_sync_ms = timing.runtime_result_accept_ms
        + dirty_ready_scan_ms
        + request_build_ms
        + worker_submit_ms
        + prepared_record_ms;
    let sync_unattributed_ms = timing
        .runtime_sync_unattributed_ms
        .max((timing.runtime_sync_ms - attributed_sync_ms).max(0.0));
    let multiview_draw_ms = timing.multiview_sky_ms
        + timing.multiview_terrain_ms
        + timing.multiview_actor_ms
        + timing.multiview_screen_effect_ms
        + timing.multiview_world_overlays_ms
        + timing.multiview_submit_ms;
    let draw_encode_ms = timing.render_views_ms.max(multiview_draw_ms);
    vec![
        StageSpan::new(StageId::HostSessionCommands, timing.runtime_poll_ms),
        StageSpan::new(
            StageId::CompletedResultAcceptance,
            timing.runtime_result_accept_ms,
        ),
        StageSpan::new(StageId::RenderAdmissionDirtyReadyScan, dirty_ready_scan_ms),
        StageSpan::new(StageId::RenderAdmissionRequestBuild, request_build_ms),
        StageSpan::new(StageId::RenderAdmissionWorkerSubmit, worker_submit_ms),
        StageSpan::new(
            StageId::RenderAdmissionPreparedRecordMaintenance,
            prepared_record_ms,
        ),
        StageSpan::new(StageId::RenderSectionAdmission, sync_unattributed_ms),
        StageSpan::new(StageId::UploadApply, timing.runtime_gpu_upload_ms),
        StageSpan::new(StageId::PreparedDrawRecords, timing.shared_records_ms),
        StageSpan::new(StageId::DrawEncode, draw_encode_ms),
    ]
}

#[derive(Clone, Debug)]
struct XrFramePipelineQueueTrackers {
    inbound_updates: QueueAgeTracker,
    completed_results: QueueAgeTracker,
    upload_work: QueueAgeTracker,
    host_publication: QueueAgeTracker,
    host_publication_runner: QueueAgeTracker,
    host_publication_worldgen: QueueAgeTracker,
    host_publication_light: QueueAgeTracker,
    render_compile_jobs: QueueAgeTracker,
}

impl XrFramePipelineQueueTrackers {
    fn new() -> Self {
        Self {
            inbound_updates: QueueAgeTracker::new(QueueId::InboundUpdates),
            completed_results: QueueAgeTracker::new(QueueId::CompletedRenderResults),
            upload_work: QueueAgeTracker::new(QueueId::UploadWork),
            host_publication: QueueAgeTracker::new(QueueId::HostPublication),
            host_publication_runner: QueueAgeTracker::new(QueueId::HostPublicationRunner),
            host_publication_worldgen: QueueAgeTracker::new(QueueId::HostPublicationWorldgen),
            host_publication_light: QueueAgeTracker::new(QueueId::HostPublicationLight),
            render_compile_jobs: QueueAgeTracker::new(QueueId::RenderCompileJobs),
        }
    }

    fn report(&mut self, upload: Option<XrTerrainUploadSummary>, now_ms: f64) -> QueuePanelReport {
        if let Some(upload) = upload {
            self.inbound_updates
                .reconcile_depth(usize_to_u64(upload.server_update_queue_depth), now_ms);

            let completed = usize_to_u64(upload.completed_compile_section_count);
            if completed > 0 {
                self.completed_results.enqueue(completed, now_ms);
                self.completed_results.dequeue(completed, now_ms);
            }
            self.completed_results.reconcile_depth(
                usize_to_u64(upload.queued_completed_compile_result_count),
                now_ms,
            );

            self.upload_work.reconcile_depth(
                usize_to_u64(upload.queued_upload_lifecycle_item_count),
                now_ms,
            );

            if upload.host_mode.server_owned_lanes_are_remote() {
                self.host_publication = QueueAgeTracker::new(QueueId::HostPublication);
                self.host_publication_runner = QueueAgeTracker::new(QueueId::HostPublicationRunner);
                self.host_publication_worldgen =
                    QueueAgeTracker::new(QueueId::HostPublicationWorldgen);
                self.host_publication_light = QueueAgeTracker::new(QueueId::HostPublicationLight);
            } else {
                let runner_depth = upload.server_pending_publications;
                let worldgen_depth = upload.poll_scheduler_pending_worldgen_publication_chunks;
                let light_depth = upload.poll_scheduler_pending_light_publications;
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
                .reconcile_depth(usize_to_u64(upload.pending_compile_jobs_after), now_ms);
        }

        let host_mode = upload.map_or(XrTerrainHostMode::LocalIntegrated, |upload| {
            upload.host_mode
        });
        let host_publication = if host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublication)
        } else {
            self.host_publication.report(now_ms)
        };
        let host_publication_runner = if host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublicationRunner)
        } else {
            self.host_publication_runner.report(now_ms)
        };
        let host_publication_worldgen = if host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublicationWorldgen)
        } else {
            self.host_publication_worldgen.report(now_ms)
        };
        let host_publication_light = if host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublicationLight)
        } else {
            self.host_publication_light.report(now_ms)
        };

        QueuePanelReport::new(vec![
            self.inbound_updates.report(now_ms),
            self.completed_results.report(now_ms),
            self.upload_work.report(now_ms),
            host_publication,
            host_publication_runner,
            host_publication_worldgen,
            host_publication_light,
            self.render_compile_jobs.report(now_ms),
        ])
    }
}

fn xr_frame_pipeline_peer_thread_panel(
    upload: Option<XrTerrainUploadSummary>,
) -> PeerThreadPanelReport {
    let Some(upload) = upload else {
        return PeerThreadPanelReport::empty();
    };
    let max_worldgen_pending_jobs = upload.poll_scheduler_worldgen_mailbox_pending_jobs;
    let max_light_pending_jobs = upload.poll_scheduler_light_mailbox_pending_statuses;
    let max_render_compile_pending_jobs = upload.pending_compile_jobs_after;
    let server_lanes_remote = upload.host_mode.server_owned_lanes_are_remote();

    PeerThreadPanelReport::new(vec![
        server_runner_peer_report(
            server_lanes_remote,
            upload.server_pending_jobs,
            upload.poll_server_reported_total_ms,
        ),
        worker_metrics_peer_report(
            PeerThreadId::Worldgen,
            server_lanes_remote,
            usize_to_u64(upload.worldgen_job_frame_metrics.request_frames),
            usize_to_u64(upload.worldgen_job_frame_metrics.response_frames),
            usize_to_u64(upload.worldgen_job_frame_metrics.request_bytes),
            usize_to_u64(upload.worldgen_job_frame_metrics.response_bytes),
            usize_to_u64(upload.worldgen_job_frame_metrics.max_pending_frames),
            upload.worldgen_job_frame_metrics.last_request_us,
            upload.worldgen_job_frame_metrics.total_request_us,
            upload.worldgen_job_frame_metrics.max_request_us,
            max_worldgen_pending_jobs,
        ),
        worker_metrics_peer_report(
            PeerThreadId::LightStatus,
            server_lanes_remote,
            usize_to_u64(upload.light_status_job_frame_metrics.request_frames),
            usize_to_u64(upload.light_status_job_frame_metrics.response_frames),
            usize_to_u64(upload.light_status_job_frame_metrics.request_bytes),
            usize_to_u64(upload.light_status_job_frame_metrics.response_bytes),
            usize_to_u64(upload.light_status_job_frame_metrics.max_pending_frames),
            upload.light_status_job_frame_metrics.last_request_us,
            upload.light_status_job_frame_metrics.total_request_us,
            upload.light_status_job_frame_metrics.max_request_us,
            max_light_pending_jobs,
        ),
        PeerThreadActivityReport::new(PeerThreadId::RenderCompileWorkers)
            .with_pending_jobs(usize_to_u64(max_render_compile_pending_jobs))
            .with_frames(
                usize_to_u64(upload.submitted_compile_section_count),
                usize_to_u64(upload.completed_compile_section_count),
            )
            .with_request_timing_ms(
                None,
                upload.dispatcher_total_compile_worker_busy_ms,
                upload.dispatcher_max_compile_worker_task_ms,
            )
            .with_busy_idle_ms(Some(upload.dispatcher_total_compile_worker_busy_ms), None),
    ])
}

fn remote_host_queue_report(queue: QueueId) -> QueueAgeReport {
    QueueAgeReport::snapshot(queue, 0, 0, 0, None, 0.0, 0)
        .with_availability(DiagnosticLaneAvailability::RemoteHost)
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
    request_frames: u64,
    response_frames: u64,
    request_bytes: u64,
    response_bytes: u64,
    max_pending_frames: u64,
    last_request_us: u128,
    total_request_us: u128,
    max_request_us: u128,
    pending_jobs: usize,
) -> PeerThreadActivityReport {
    let report = PeerThreadActivityReport::new(lane)
        .with_pending_jobs(usize_to_u64(pending_jobs))
        .with_frames(request_frames, response_frames)
        .with_bytes(request_bytes, response_bytes)
        .with_max_pending_frames(max_pending_frames)
        .with_request_timing_ms(
            (last_request_us > 0).then_some(micros_to_ms(last_request_us)),
            micros_to_ms(total_request_us),
            micros_to_ms(max_request_us),
        )
        .with_busy_idle_ms(Some(micros_to_ms(total_request_us)), None);
    if server_lanes_remote {
        report.with_availability(DiagnosticLaneAvailability::RemoteHost)
    } else {
        report
    }
}

fn frame_accounting_config(target_hz: Option<f64>) -> FrameAccountingConfig {
    target_hz
        .map_or_else(
            FrameAccountingConfig::without_budget,
            FrameAccountingConfig::from_target_hz,
        )
        .with_percentile_method(PercentileMethod::NearestRank)
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

fn finite_positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn same_target_hz(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => (left - right).abs() <= f64::EPSILON,
        (None, None) => true,
        _ => false,
    }
}

fn usize_to_u64(value: usize) -> u64 {
    value.try_into().unwrap_or(u64::MAX)
}

fn micros_to_ms(value: u128) -> f64 {
    (value as f64) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporter_records_visible_frame_report() {
        let mut reporter = XrFramePipelineReporter::new(Some(90.0));
        let (report, revision) = reporter.record_frame(
            XrFramePipelineHostTiming {
                frame_wall_ms: 11.0,
                wait_frame_ms: 2.0,
                controller_poll_ms: 0.25,
                rendered: true,
                thread_cpu_ms: Some(8.0),
            },
            None,
        );
        assert_eq!(revision, 1);
        assert_eq!(report.frame_summary.frames, 1);
        assert_eq!(report.queue_panel.queues.len(), 8);
        assert!(report.stage_spans.iter().any(|span| {
            span.stage == StageId::GpuExecutionPresentationWait && span.elapsed_ms == 2.0
        }));
    }

    #[test]
    fn reporter_marks_remote_host_lanes_as_unavailable_locally() {
        let mut reporter = XrFramePipelineReporter::new(Some(72.0));
        let (report, _revision) = reporter.record_frame(
            XrFramePipelineHostTiming {
                frame_wall_ms: 13.0,
                wait_frame_ms: 4.0,
                controller_poll_ms: 0.25,
                rendered: true,
                thread_cpu_ms: Some(7.0),
            },
            Some(XrTerrainFrameSummary {
                rendered_frames: 1,
                section_count: 1,
                drawn_section_count: 1,
                index_count: 6,
                drawn_index_count: 6,
                gui_command_count: 0,
                ui_panel: Default::default(),
                ui_draw_cache: Default::default(),
                ui_active: false,
                local_startup_active: false,
                actor_count: 0,
                drawn_actor_count: 0,
                head_comfort: Default::default(),
                timing: XrTerrainFrameTiming::default(),
                upload: XrTerrainUploadSummary {
                    host_mode: XrTerrainHostMode::RemoteDedicated,
                    server_update_queue_depth: 3,
                    pending_compile_jobs_after: 2,
                    ..XrTerrainUploadSummary::default()
                },
            }),
        );

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
        for queue_id in [
            QueueId::HostPublicationRunner,
            QueueId::HostPublicationWorldgen,
            QueueId::HostPublicationLight,
        ] {
            let queue = report
                .queue_panel
                .queues
                .iter()
                .find(|queue| queue.queue == queue_id)
                .expect("host publication component queue");
            assert_eq!(queue.availability, DiagnosticLaneAvailability::RemoteHost);
            assert_eq!(queue.depth, 0);
        }
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
        assert_eq!(inbound_updates.depth, 3);

        for lane in [
            PeerThreadId::ServerRunner,
            PeerThreadId::Worldgen,
            PeerThreadId::LightStatus,
        ] {
            let peer = report
                .peer_thread_panel
                .peers
                .iter()
                .find(|peer| peer.lane == lane)
                .expect("server-owned peer lane");
            assert_eq!(peer.availability, DiagnosticLaneAvailability::RemoteHost);
            assert!(!peer.active);
        }
        let render_workers = report
            .peer_thread_panel
            .peers
            .iter()
            .find(|peer| peer.lane == PeerThreadId::RenderCompileWorkers)
            .expect("render workers lane");
        assert_eq!(
            render_workers.availability,
            DiagnosticLaneAvailability::Local
        );
        assert!(render_workers.active);
    }

    #[test]
    fn stage_spans_include_xr_scene_timing() {
        let timing = XrTerrainFrameTiming {
            runtime_poll_ms: 0.5,
            runtime_sync_ms: 4.0,
            runtime_result_accept_ms: 0.25,
            runtime_dirty_seed_ms: 0.5,
            runtime_prepare_ms: 0.5,
            runtime_submit_snapshot_ms: 0.25,
            runtime_submit_request_build_ms: 0.25,
            runtime_submit_compiler_ms: 0.5,
            runtime_submit_mark_inflight_ms: 0.25,
            runtime_submit_apply_ready_plan_ms: 0.25,
            runtime_submit_ready_update_ms: 0.25,
            runtime_gpu_upload_ms: 1.0,
            shared_records_ms: 0.75,
            render_views_ms: 3.0,
            ..Default::default()
        };
        let spans = xr_frame_pipeline_stage_spans(timing);
        assert!(spans.iter().any(|span| {
            span.stage == StageId::RenderSectionAdmission && span.elapsed_ms == 1.0
        }));
        assert!(
            spans
                .iter()
                .any(|span| span.stage == StageId::DrawEncode && span.elapsed_ms == 3.0)
        );
    }
}
