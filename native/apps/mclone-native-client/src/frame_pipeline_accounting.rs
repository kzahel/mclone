use std::sync::Arc;

use mclone_app_runtime::frame_render::RenderStreamStats;
use mclone_app_runtime::{RenderSectionSyncTiming, SingleViewRuntimeStats};
use mclone_diagnostics::{
    FrameAccountingConfig, FrameAccumulator, FrameObservation, FramePipelineReport,
    QueueAgeTracker, QueueId, QueuePanelReport, StageId, StageSpan,
};

#[derive(Clone, Debug)]
pub(crate) struct DesktopFramePipelineAccounting {
    target_period_ms: Option<f64>,
    accumulator: FrameAccumulator,
    pending: Option<FrameObservation>,
    queue_trackers: DesktopFrameQueueTrackers,
    now_ms: f64,
    latest_report: Option<Arc<FramePipelineReport>>,
    revision: u64,
}

impl Default for DesktopFramePipelineAccounting {
    fn default() -> Self {
        Self {
            target_period_ms: None,
            accumulator: FrameAccumulator::new(frame_accounting_config(None)),
            pending: None,
            queue_trackers: DesktopFrameQueueTrackers::new(),
            now_ms: 0.0,
            latest_report: None,
            revision: 0,
        }
    }
}

impl DesktopFramePipelineAccounting {
    pub(crate) fn begin_frame(&mut self, frame_wall_ms: f64, target_period_ms: Option<f64>) {
        self.refresh_target(target_period_ms);
        self.now_ms += sanitize_ms(frame_wall_ms);
        let frame_index = self.accumulator.len() as u64 + 1;
        self.pending = Some(
            FrameObservation::new(frame_index, frame_wall_ms)
                .with_app_work_ms(sanitize_ms(frame_wall_ms)),
        );
    }

    pub(crate) fn record_runtime_poll(&mut self, elapsed_ms: f64) {
        self.push_stage(StageSpan::new(StageId::HostSessionCommands, elapsed_ms));
    }

    pub(crate) fn record_render_section_sync(
        &mut self,
        timing: RenderSectionSyncTiming,
        total_sync_ms: f64,
    ) {
        for span in render_section_sync_stage_spans(timing, total_sync_ms) {
            self.push_stage(span);
        }
    }

    pub(crate) fn record_upload_apply(&mut self, upload_ms: f64) {
        self.push_stage(StageSpan::new(StageId::UploadApply, upload_ms));
    }

    pub(crate) fn finish_frame(
        &mut self,
        render_ms: f64,
        surface_acquire_ms: f64,
        surface_submit_ms: f64,
        surface_present_ms: f64,
        runtime_stats: Option<SingleViewRuntimeStats>,
        render_stats: RenderStreamStats,
    ) {
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

        let queue_panel =
            self.queue_trackers
                .report(runtime_stats.as_ref(), render_stats, self.now_ms);
        let Some(observation) = self.pending.take() else {
            return;
        };
        self.accumulator.record_frame(observation);
        let report = FramePipelineReport::new(self.accumulator.summary_report(), queue_panel);
        self.revision = self.revision.saturating_add(1);
        self.latest_report = Some(Arc::new(report));
    }

    pub(crate) fn latest_report(&self) -> Option<(Arc<FramePipelineReport>, u64)> {
        self.latest_report
            .as_ref()
            .map(|report| (report.clone(), self.revision))
    }

    fn refresh_target(&mut self, target_period_ms: Option<f64>) {
        let target_period_ms = target_period_ms.and_then(finite_positive);
        if same_target(self.target_period_ms, target_period_ms) {
            return;
        }
        self.target_period_ms = target_period_ms;
        self.accumulator = FrameAccumulator::new(frame_accounting_config(target_period_ms));
        self.pending = None;
        self.latest_report = None;
    }

    fn push_stage(&mut self, span: StageSpan) {
        if let Some(observation) = self.pending.as_mut()
            && span.elapsed_ms > 0.0
        {
            observation.stage_spans.push(span);
        }
    }
}

#[derive(Clone, Debug)]
struct DesktopFrameQueueTrackers {
    inbound_updates: QueueAgeTracker,
    host_publication: QueueAgeTracker,
    render_compile_jobs: QueueAgeTracker,
    completed_results: QueueAgeTracker,
    upload_work: QueueAgeTracker,
}

impl DesktopFrameQueueTrackers {
    fn new() -> Self {
        Self {
            inbound_updates: QueueAgeTracker::new(QueueId::InboundUpdates),
            host_publication: QueueAgeTracker::new(QueueId::HostPublication),
            render_compile_jobs: QueueAgeTracker::new(QueueId::RenderCompileJobs),
            completed_results: QueueAgeTracker::new(QueueId::CompletedRenderResults),
            upload_work: QueueAgeTracker::new(QueueId::UploadWork),
        }
    }

    fn report(
        &mut self,
        runtime: Option<&SingleViewRuntimeStats>,
        render: RenderStreamStats,
        now_ms: f64,
    ) -> QueuePanelReport {
        self.inbound_updates.reconcile_depth(
            usize_to_u64(runtime.map_or(0, |stats| stats.server_update_queue_depth)),
            now_ms,
        );
        self.host_publication.reconcile_depth(
            usize_to_u64(runtime.map_or(0, |stats| stats.pending_publications)),
            now_ms,
        );
        self.render_compile_jobs.reconcile_depth(
            usize_to_u64(runtime.map_or(render.last_pending_compile_jobs, |stats| {
                stats.pending_render_compile_jobs
            })),
            now_ms,
        );
        self.completed_results.reconcile_depth(
            usize_to_u64(render.last_completed_compile_section_count),
            now_ms,
        );
        self.upload_work
            .reconcile_depth(usize_to_u64(render.last_deferred_section_count), now_ms);

        QueuePanelReport::new(vec![
            self.host_publication.report(now_ms),
            self.inbound_updates.report(now_ms),
            self.render_compile_jobs.report(now_ms),
            self.completed_results.report(now_ms),
            self.upload_work.report(now_ms),
        ])
    }
}

fn render_section_sync_stage_spans(
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
    vec![
        StageSpan::new(
            StageId::CompletedResultAcceptance,
            timing.completed_result_accept_ms,
        ),
        StageSpan::new(StageId::RenderAdmissionDirtyReadyScan, dirty_ready_scan_ms),
        StageSpan::new(StageId::RenderAdmissionRequestBuild, request_build_ms),
        StageSpan::new(StageId::RenderAdmissionWorkerSubmit, worker_submit_ms),
        StageSpan::new(
            StageId::RenderAdmissionPreparedRecordMaintenance,
            prepared_record_ms,
        ),
        StageSpan::new(StageId::RenderSectionAdmission, unattributed_ms),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_accounting_builds_latest_report_with_queues() {
        let mut accounting = DesktopFramePipelineAccounting::default();
        accounting.begin_frame(16.0, Some(16.0));
        accounting.record_runtime_poll(1.0);
        accounting.record_upload_apply(2.0);
        accounting.finish_frame(
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
        );

        let (report, revision) = accounting.latest_report().expect("report");
        assert_eq!(revision, 1);
        assert_eq!(report.frame_summary.latest_frame_wall_ms, 16.0);
        assert_eq!(report.frame_summary.latest_app_work_ms, 16.0);
        assert_eq!(report.stage_spans.len(), 4);
        assert_eq!(report.queue_panel.queues.len(), 5);
        assert!(
            report
                .queue_panel
                .queues
                .iter()
                .any(|queue| queue.queue == QueueId::UploadWork && queue.depth == 3)
        );
    }
}
