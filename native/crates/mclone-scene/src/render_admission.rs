use std::sync::Arc;
use std::time::Duration;

use mclone_app_runtime::{RenderSectionSyncTiming, TimedRenderSectionCacheUpdate};
use mclone_diagnostics::{
    BudgetDecisionFamily, BudgetDecisionPanelReport, BudgetHostMode, FrameHostKind,
    FramePipelineReport, QueueId, WorkWindow,
};
use mclone_frame_budget::{
    BudgetController, BudgetControllerInput, BudgetTelemetryWindow, DEFAULT_COST_EWMA_ALPHA,
    EwmaCostEstimator, decision_for_family, render_frame_budget_controller_config,
};

/// The effective client-side render-section admission grant for one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderAdmissionGrant {
    pub elapsed_budget: Duration,
    pub max_compile_requests: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LodFrameBudgetGrant {
    pub build_tiles: usize,
    pub upload_tiles: usize,
}

/// Host-neutral adaptive render-section admission policy.
///
/// Drivers provide only their target frame period. The policy consumes the
/// preceding frame's shared pipeline report, learns section-sync unit costs,
/// and produces the same grant shape for flat and XR scene topologies.
#[derive(Clone, Debug)]
pub struct RenderAdmissionPolicy {
    host_kind: FrameHostKind,
    admission_window: WorkWindow,
    controller: BudgetController,
    estimator: EwmaCostEstimator,
    latest_report: Option<Arc<FramePipelineReport>>,
    last_panel: BudgetDecisionPanelReport,
    lod_build_queue_depth: u64,
    lod_build_oldest_age_ms: Option<f64>,
    lod_upload_queue_depth: u64,
}

impl RenderAdmissionPolicy {
    pub fn new(host_kind: FrameHostKind, admission_window: WorkWindow) -> Self {
        Self {
            host_kind,
            admission_window,
            controller: BudgetController::new(render_frame_budget_controller_config(
                host_kind,
                admission_window,
            )),
            estimator: EwmaCostEstimator::new(DEFAULT_COST_EWMA_ALPHA),
            latest_report: None,
            last_panel: BudgetDecisionPanelReport::empty(),
            lod_build_queue_depth: 0,
            lod_build_oldest_age_ms: None,
            lod_upload_queue_depth: 0,
        }
    }

    pub fn set_host_kind(&mut self, host_kind: FrameHostKind) {
        if self.host_kind == host_kind {
            return;
        }
        self.host_kind = host_kind;
        self.reset();
    }

    pub fn set_frame_pipeline_report(&mut self, report: Arc<FramePipelineReport>) {
        self.latest_report = Some(report);
    }

    pub fn clear_frame_pipeline_report(&mut self) {
        self.latest_report = None;
    }

    pub fn set_lod_producer_active(&mut self, active: bool) {
        self.controller
            .set_producer_active(BudgetDecisionFamily::LodBuildAdmission, active);
        self.controller
            .set_producer_active(BudgetDecisionFamily::LodUpload, active);
    }

    pub fn set_lod_queue_telemetry(
        &mut self,
        build_depth: usize,
        build_oldest_age_ms: Option<f64>,
        upload_depth: usize,
    ) {
        self.lod_build_queue_depth = u64::try_from(build_depth).unwrap_or(u64::MAX);
        self.lod_build_oldest_age_ms = build_oldest_age_ms;
        self.lod_upload_queue_depth = u64::try_from(upload_depth).unwrap_or(u64::MAX);
    }

    pub fn decide(
        &mut self,
        target_period_ms: Option<f64>,
        host_mode: BudgetHostMode,
        max_compile_requests_override: Option<usize>,
    ) -> Option<RenderAdmissionGrant> {
        let Some(target_period_ms) = target_period_ms.and_then(finite_positive_ms) else {
            let mut input = BudgetControllerInput::new(0.0);
            input.host_mode = host_mode;
            input = input
                .with_family_queue(
                    BudgetDecisionFamily::LodBuildAdmission,
                    self.lod_build_queue_depth,
                    self.lod_build_oldest_age_ms,
                )
                .with_family_queue(
                    BudgetDecisionFamily::LodUpload,
                    self.lod_upload_queue_depth,
                    None,
                );
            self.last_panel = self.controller.decide(&input);
            return None;
        };
        let mut input = BudgetControllerInput::new(target_period_ms);
        input.host_mode = host_mode;
        input.costs = self.estimator.estimates;
        if let Some(report) = &self.latest_report {
            input = input.with_window(render_admission_budget_window(
                report,
                target_period_ms,
                self.host_kind,
                self.admission_window,
            ));
        }
        input = input
            .with_family_queue(
                BudgetDecisionFamily::LodBuildAdmission,
                self.lod_build_queue_depth,
                self.lod_build_oldest_age_ms,
            )
            .with_family_queue(
                BudgetDecisionFamily::LodUpload,
                self.lod_upload_queue_depth,
                None,
            );
        self.last_panel = self.controller.decide(&input);
        clamp_render_admission_grant(&mut self.last_panel, max_compile_requests_override);
        let decision =
            decision_for_family(&self.last_panel, BudgetDecisionFamily::RenderAdmission)?;
        Some(RenderAdmissionGrant {
            elapsed_budget: duration_from_ms(decision.grant.elapsed_ms),
            max_compile_requests: usize::try_from(decision.grant.max_units)
                .unwrap_or(usize::MAX)
                .max(1),
        })
    }

    pub fn observe_sync(
        &mut self,
        target_period_ms: Option<f64>,
        update: &TimedRenderSectionCacheUpdate,
    ) {
        let Some(target_period_ms) = target_period_ms.and_then(finite_positive_ms) else {
            return;
        };
        self.estimator.observe_for_target_period(
            target_period_ms,
            BudgetDecisionFamily::RenderAdmission,
            render_admission_observed_ms(update.timing),
            usize_to_u32(update.timing.submit_request_count),
        );
        self.estimator.observe_for_target_period(
            target_period_ms,
            BudgetDecisionFamily::CompletedResultAcceptance,
            update.timing.completed_result_accept_ms,
            usize_to_u32(update.cache_update.accepted_compile_result_count),
        );
    }

    pub fn panel(&self) -> &BudgetDecisionPanelReport {
        &self.last_panel
    }

    pub fn lod_grant(&self) -> LodFrameBudgetGrant {
        let units = |family| {
            decision_for_family(&self.last_panel, family)
                .map(|decision| usize::try_from(decision.grant.max_units).unwrap_or(usize::MAX))
                .unwrap_or_default()
        };
        let mut grant = LodFrameBudgetGrant {
            build_tiles: units(BudgetDecisionFamily::LodBuildAdmission),
            upload_tiles: units(BudgetDecisionFamily::LodUpload),
        };
        // Startup prewarm feeds the same producer/upload queues before a frame
        // accountant report exists. Give that cold backlog one bounded burst;
        // subsequent reported frames use the adaptive family decisions.
        if self.latest_report.is_none() {
            if grant.build_tiles > 0 && self.lod_build_queue_depth > 0 {
                grant.build_tiles = grant.build_tiles.max(4);
            }
            if grant.upload_tiles > 0 && self.lod_upload_queue_depth > 0 {
                grant.upload_tiles = grant.upload_tiles.max(16);
            }
        }
        grant
    }

    pub fn clear_panel(&mut self) {
        self.last_panel = BudgetDecisionPanelReport::empty();
    }

    pub fn reset(&mut self) {
        self.controller = BudgetController::new(render_frame_budget_controller_config(
            self.host_kind,
            self.admission_window,
        ));
        self.estimator = EwmaCostEstimator::new(DEFAULT_COST_EWMA_ALPHA);
        self.latest_report = None;
        self.last_panel = BudgetDecisionPanelReport::empty();
        self.lod_build_queue_depth = 0;
        self.lod_build_oldest_age_ms = None;
        self.lod_upload_queue_depth = 0;
    }
}

pub fn merge_budget_decision_panels(
    mut primary: BudgetDecisionPanelReport,
    secondary: &BudgetDecisionPanelReport,
) -> BudgetDecisionPanelReport {
    primary
        .decisions
        .extend(secondary.decisions.iter().cloned());
    BudgetDecisionPanelReport::new(primary.decisions)
}

fn render_admission_budget_window(
    report: &FramePipelineReport,
    target_period_ms: f64,
    host_kind: FrameHostKind,
    admission_window: WorkWindow,
) -> BudgetTelemetryWindow {
    let summary = &report.frame_summary;
    let (queue_depth, oldest_queue_age_ms) = render_queue_pressure(report);
    let mut window = BudgetTelemetryWindow::new(host_kind, admission_window)
        .with_app_work_p95_ms(summary.latest_app_work_ms);
    if let Some(headroom_ms) = summary.latest_headroom_ms {
        window = window.with_headroom_p05_ms(headroom_ms);
    }
    window.app_over_period_pct = if summary.latest_app_work_ms > target_period_ms {
        100.0
    } else {
        0.0
    };
    window.missed_frames = u64::from(summary.latest_frame_wall_ms > target_period_ms);
    window.over_2x_frames = u64::from(summary.latest_frame_wall_ms > target_period_ms * 2.0);
    window.queue_depth = queue_depth;
    window.oldest_queue_age_ms = oldest_queue_age_ms;
    window
}

fn render_queue_pressure(report: &FramePipelineReport) -> (u64, Option<f64>) {
    let mut depth = 0_u64;
    let mut oldest = None;
    for queue in &report.queue_panel.queues {
        if matches!(
            queue.queue,
            QueueId::RenderCompileJobs | QueueId::CompletedRenderResults | QueueId::UploadWork
        ) {
            depth = depth.saturating_add(queue.depth);
            oldest = max_optional_ms(oldest, queue.oldest_age_ms);
        }
    }
    (depth, oldest)
}

fn clamp_render_admission_grant(
    panel: &mut BudgetDecisionPanelReport,
    max_compile_requests_override: Option<usize>,
) {
    let Some(cap) = max_compile_requests_override.filter(|cap| *cap > 0) else {
        return;
    };
    let cap = u32::try_from(cap).unwrap_or(u32::MAX);
    if let Some(decision) = panel
        .decisions
        .iter_mut()
        .find(|decision| decision.family == BudgetDecisionFamily::RenderAdmission)
    {
        decision.grant.max_units = decision.grant.max_units.min(cap).max(1);
        decision.grant.min_units = decision.grant.min_units.min(decision.grant.max_units);
    }
}

fn render_admission_observed_ms(timing: RenderSectionSyncTiming) -> f64 {
    let dirty_ready_scan_ms = timing.dirty_seed_ms + timing.prepare_ms;
    let request_build_ms = timing.submit_snapshot_ms + timing.submit_request_build_ms;
    let worker_submit_ms = timing.submit_compiler_ms;
    let prepared_record_ms = timing.submit_mark_inflight_ms
        + timing.submit_apply_ready_plan_ms
        + timing.submit_ready_update_ms;
    dirty_ready_scan_ms + request_build_ms + worker_submit_ms + prepared_record_ms
}

fn duration_from_ms(ms: f64) -> Duration {
    Duration::from_secs_f64(finite_positive_ms(ms).unwrap_or(0.0) / 1_000.0)
}

fn finite_positive_ms(ms: f64) -> Option<f64> {
    (ms.is_finite() && ms > 0.0).then_some(ms)
}

fn max_optional_ms(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right.and_then(finite_positive_ms)) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn usize_to_u32(value: usize) -> u32 {
    value.try_into().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_app_runtime::RenderSectionSyncTiming;
    use mclone_diagnostics::{
        BudgetDecisionReason, FrameAccountingConfig, FrameAccumulator, FrameObservation,
        QueuePanelReport,
    };

    fn clean_report(target_period_ms: f64) -> Arc<FramePipelineReport> {
        let mut frames = FrameAccumulator::new(FrameAccountingConfig::from_target_period_ms(
            target_period_ms,
        ));
        frames.record_frame(
            FrameObservation::new(1, 2.0)
                .with_app_work_ms(2.0)
                .with_wait_ms(0.0),
        );
        Arc::new(FramePipelineReport::new(
            frames.summary_report(),
            QueuePanelReport::new(Vec::new()),
        ))
    }

    fn render_decision(
        policy: &RenderAdmissionPolicy,
    ) -> &mclone_diagnostics::BudgetDecisionReport {
        policy
            .panel()
            .decisions
            .iter()
            .find(|decision| decision.family == BudgetDecisionFamily::RenderAdmission)
            .expect("render admission decision")
    }

    #[test]
    fn missing_target_disables_real_admission_and_keeps_lod_floor_trace() {
        let mut policy =
            RenderAdmissionPolicy::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender);
        assert_eq!(
            policy.decide(None, BudgetHostMode::LocalIntegrated, None),
            None
        );
        assert_eq!(
            render_decision(&policy).trace.reason,
            mclone_diagnostics::BudgetDecisionReason::MissingInputFloor
        );
        assert_eq!(policy.lod_grant(), LodFrameBudgetGrant::default());

        policy.set_lod_producer_active(true);
        policy.set_lod_queue_telemetry(8, Some(25.0), 8);
        assert_eq!(
            policy.decide(None, BudgetHostMode::LocalIntegrated, None),
            None
        );
        assert_eq!(
            policy.lod_grant(),
            LodFrameBudgetGrant {
                build_tiles: 4,
                upload_tiles: 16,
            }
        );
    }

    #[test]
    fn clean_windows_raise_the_same_adaptive_grant_as_the_desktop_controller() {
        let mut policy =
            RenderAdmissionPolicy::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender);
        policy.set_frame_pipeline_report(clean_report(10.0));
        for _ in 0..3 {
            policy
                .decide(Some(10.0), BudgetHostMode::LocalIntegrated, None)
                .expect("adaptive grant");
        }
        let grant = policy
            .decide(Some(10.0), BudgetHostMode::LocalIntegrated, None)
            .expect("raised adaptive grant");
        assert_eq!(grant.max_compile_requests, 2);
        assert_eq!(
            render_decision(&policy).trace.reason,
            BudgetDecisionReason::RaiseSustainedHeadroom
        );
    }

    #[test]
    fn static_override_clamps_effective_and_reported_admission() {
        let mut policy =
            RenderAdmissionPolicy::new(FrameHostKind::AndroidXrOpenXr, WorkWindow::BeforeRender);
        policy.set_frame_pipeline_report(clean_report(10.0));
        for _ in 0..4 {
            policy.decide(Some(10.0), BudgetHostMode::LocalIntegrated, None);
        }
        let adaptive = policy
            .decide(Some(10.0), BudgetHostMode::LocalIntegrated, None)
            .expect("adaptive grant");
        assert!(adaptive.max_compile_requests > 1);

        let clamped = policy
            .decide(Some(10.0), BudgetHostMode::LocalIntegrated, Some(1))
            .expect("clamped grant");
        assert_eq!(clamped.max_compile_requests, 1);
        assert_eq!(render_decision(&policy).grant.max_units, 1);
        assert_eq!(
            render_decision(&policy).address.host_kind,
            FrameHostKind::AndroidXrOpenXr
        );
    }

    #[test]
    fn target_period_change_resets_the_adaptive_grant() {
        let mut policy =
            RenderAdmissionPolicy::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender);
        policy.set_frame_pipeline_report(clean_report(10.0));
        for _ in 0..4 {
            policy.decide(Some(10.0), BudgetHostMode::LocalIntegrated, None);
        }
        assert_eq!(
            policy
                .decide(Some(10.0), BudgetHostMode::LocalIntegrated, None)
                .expect("raised grant")
                .max_compile_requests,
            2
        );

        let changed = policy
            .decide(Some(1000.0 / 90.0), BudgetHostMode::LocalIntegrated, None)
            .expect("target-change grant");
        assert_eq!(changed.max_compile_requests, 1);
        assert_eq!(
            render_decision(&policy).trace.reason,
            BudgetDecisionReason::TargetPeriodChangedFloor
        );
    }

    #[test]
    fn timed_sync_observation_feeds_per_unit_costs() {
        let mut policy =
            RenderAdmissionPolicy::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender);
        policy.set_frame_pipeline_report(clean_report(10.0));
        let update = TimedRenderSectionCacheUpdate {
            timing: RenderSectionSyncTiming {
                dirty_seed_ms: 1.0,
                prepare_ms: 1.0,
                submit_request_count: 2,
                ..RenderSectionSyncTiming::default()
            },
            ..TimedRenderSectionCacheUpdate::default()
        };
        policy.observe_sync(Some(10.0), &update);
        policy.decide(Some(10.0), BudgetHostMode::LocalIntegrated, None);
        assert_eq!(
            render_decision(&policy)
                .trace
                .input_snapshot
                .per_unit_cost_ms,
            Some(1.0)
        );
    }

    #[test]
    fn panel_merge_preserves_scheduler_then_render_order() {
        let mut policy =
            RenderAdmissionPolicy::new(FrameHostKind::DesktopFlatWinit, WorkWindow::BeforeRender);
        policy.decide(Some(10.0), BudgetHostMode::LocalIntegrated, None);
        let merged =
            merge_budget_decision_panels(BudgetDecisionPanelReport::empty(), policy.panel());
        assert_eq!(merged.decisions, policy.panel().decisions);
    }
}
