//! Shared presentation for [`FramePipelineReport`] diagnostics.
//!
//! Platform adapters choose where lines are written and which marker prefix is
//! appropriate. Field order, labels, optional-value spelling, and JSON embedding
//! stay shared so perf scripts and human-readable logs cannot drift by surface.

use mclone_diagnostics::{
    BudgetDecisionFamily, BudgetDecisionReason, CriticalPathLabel, FrameHostKind,
    FramePipelineReport, PeerThreadId, QueueId, StageId, WorkWindow,
};

/// Formats the line-oriented frame-pipeline report used by native perf logs.
///
/// Passing `MCLONE_ANDROID_XR_PERF` preserves the established Quest markers.
pub fn frame_pipeline_report_lines(
    marker_prefix: &str,
    report: &FramePipelineReport,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(
        1 + report.queue_panel.queues.len()
            + report.peer_thread_panel.peers.len()
            + report.stage_spans.len()
            + report.budget_decision_panel.decisions.len(),
    );
    lines.push(format!(
        "{marker_prefix}_FRAME_PIPELINE schema_version={} frames={} queues={} peers={} stages={} budget_decisions={}",
        report.schema_version,
        report.frame_summary.frames,
        report.queue_panel.queues.len(),
        report.peer_thread_panel.peers.len(),
        report.stage_spans.len(),
        report.budget_decision_panel.decisions.len()
    ));
    for queue in &report.queue_panel.queues {
        lines.push(format!(
            "{marker_prefix}_QUEUE queue={} enqueued={} dequeued={} depth={} oldest_age_ms={} max_oldest_age_ms={:.3} conservation_violations={} availability={}",
            queue_id_label(&queue.queue),
            queue.enqueued_total,
            queue.dequeued_total,
            queue.depth,
            format_optional_f64(queue.oldest_age_ms),
            queue.max_oldest_age_ms,
            queue.conservation_violations,
            queue.availability.label()
        ));
    }
    for peer in &report.peer_thread_panel.peers {
        lines.push(format!(
            "{marker_prefix}_PEER lane={} active={} pending_jobs={} request_frames={} response_frames={} request_bytes={} response_bytes={} max_pending_frames={} busy_ms={} idle_ms={} last_request_ms={} total_request_ms={:.3} max_request_ms={:.3} conservation_violations={} availability={}",
            peer_thread_id_label(&peer.lane),
            peer.active,
            peer.pending_jobs,
            peer.request_frames,
            peer.response_frames,
            peer.request_bytes,
            peer.response_bytes,
            peer.max_pending_frames,
            format_optional_f64(peer.busy_ms),
            format_optional_f64(peer.idle_ms),
            format_optional_f64(peer.last_request_ms),
            peer.total_request_ms,
            peer.max_request_ms,
            peer.conservation_violations,
            peer.availability.label()
        ));
    }
    for span in &report.stage_spans {
        lines.push(format!(
            "{marker_prefix}_STAGE stage={} label={} elapsed_ms={:.3} thread_cpu_ms={}",
            stage_id_label(span.stage),
            critical_path_label(span.label),
            span.elapsed_ms,
            format_optional_f64(span.thread_cpu_ms)
        ));
    }
    for decision in &report.budget_decision_panel.decisions {
        lines.push(format!(
            "{marker_prefix}_BUDGET_DECISION family={} host={} window={} stage={} reason={} grant_ms={:.3} min_units={} max_units={} max_pending_units={} target_period_ms={:.3} queue_depth={} oldest_queue_age_ms={} per_unit_cost_ms={} previous_max_units={} clean_windows={}",
            budget_decision_family_label(decision.family),
            frame_host_kind_label(decision.address.host_kind),
            work_window_label(decision.address.work_window),
            stage_id_label(decision.address.stage),
            budget_decision_reason_label(decision.trace.reason),
            decision.grant.elapsed_ms,
            decision.grant.min_units,
            decision.grant.max_units,
            format_optional_u32(decision.grant.max_pending_units),
            decision.trace.input_snapshot.target_period_ms,
            decision.trace.input_snapshot.queue_depth,
            format_optional_f64(decision.trace.input_snapshot.oldest_queue_age_ms),
            format_optional_f64(decision.trace.input_snapshot.per_unit_cost_ms),
            decision.trace.previous_max_units,
            decision.trace.consecutive_clean_windows
        ));
    }
    lines
}

/// Formats a report as a pretty-printed JSON field inside an existing object.
pub fn frame_pipeline_report_json_field_lines(
    field_name: &str,
    report: &FramePipelineReport,
    indent: &str,
    trailing_comma: bool,
) -> Result<Vec<String>, serde_json::Error> {
    let json = serde_json::to_string_pretty(report)?;
    let json_lines = json.lines().collect::<Vec<_>>();
    let suffix = if trailing_comma { "," } else { "" };
    Ok(json_lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                format!("{indent}\"{field_name}\": {line}")
            } else if index + 1 == json_lines.len() {
                format!("{indent}{line}{suffix}")
            } else {
                format!("{indent}{line}")
            }
        })
        .collect())
}

pub const fn critical_path_label(label: CriticalPathLabel) -> &'static str {
    match label {
        CriticalPathLabel::CurrentFrameCritical => "current-frame-critical",
        CriticalPathLabel::NextFrameSlack => "next-frame-slack",
        CriticalPathLabel::ParallelCpuPeer => "parallel-cpu-peer",
        CriticalPathLabel::RemoteHostWork => "remote-host-work",
        CriticalPathLabel::QueuedBackpressured => "queued-backpressured",
    }
}

pub const fn budget_decision_family_label(family: BudgetDecisionFamily) -> &'static str {
    match family {
        BudgetDecisionFamily::FeaturePublication => "feature-publication",
        BudgetDecisionFamily::LightPublication => "light-publication",
        BudgetDecisionFamily::FeatureJobAdmission => "feature-job-admission",
        BudgetDecisionFamily::RenderAdmission => "render-admission",
        BudgetDecisionFamily::CompletedResultAcceptance => "completed-result-acceptance",
        BudgetDecisionFamily::SectionUpload => "section-upload",
        BudgetDecisionFamily::RenderCompileWorkers => "render-compile-workers",
    }
}

pub const fn budget_decision_reason_label(reason: BudgetDecisionReason) -> &'static str {
    match reason {
        BudgetDecisionReason::ColdStartFloor => "cold-start-floor",
        BudgetDecisionReason::MissingInputFloor => "missing-input-floor",
        BudgetDecisionReason::TargetPeriodChangedFloor => "target-period-changed-floor",
        BudgetDecisionReason::CutFrameMiss => "cut-frame-miss",
        BudgetDecisionReason::CutNegativeHeadroom => "cut-negative-headroom",
        BudgetDecisionReason::CutQueueAge => "cut-queue-age",
        BudgetDecisionReason::RaiseSustainedHeadroom => "raise-sustained-headroom",
        BudgetDecisionReason::HoldHysteresis => "hold-hysteresis",
        BudgetDecisionReason::HoldAtCap => "hold-at-cap",
    }
}

pub const fn frame_host_kind_label(host: FrameHostKind) -> &'static str {
    match host {
        FrameHostKind::DesktopFlatWinit => "desktop-flat-winit",
        FrameHostKind::FlatAndroidWinit => "flat-android-winit",
        FrameHostKind::WebRafWorkers => "web-raf-workers",
        FrameHostKind::AndroidXrOpenXr => "android-xr-open-xr",
        FrameHostKind::DesktopXrOpenXr => "desktop-xr-open-xr",
        FrameHostKind::HeadlessOffscreenPerf => "headless-offscreen-perf",
        FrameHostKind::IntegratedServerRunner => "integrated-server-runner",
        FrameHostKind::DedicatedServerCommandLoop => "dedicated-server-command-loop",
    }
}

pub const fn work_window_label(window: WorkWindow) -> &'static str {
    match window {
        WorkWindow::BeforeRender => "before-render",
        WorkWindow::PostSubmitOverlapSlack => "post-submit-overlap-slack",
        WorkWindow::GameplayTick => "gameplay-tick",
        WorkWindow::TickSlack => "tick-slack",
        WorkWindow::WorkerPoll => "worker-poll",
        WorkWindow::OffscreenStep => "offscreen-step",
        WorkWindow::CommandTick => "command-tick",
    }
}

pub const fn stage_id_label(stage: StageId) -> &'static str {
    match stage {
        StageId::InputPoseEvents => "input-pose-events",
        StageId::HostSessionCommands => "host-session-commands",
        StageId::TerrainGeneration => "terrain-generation",
        StageId::LightComputeStatus => "light-compute-status",
        StageId::SchedulerPublication => "scheduler-publication",
        StageId::TransportDecode => "transport-decode",
        StageId::ClientUpdateApply => "client-update-apply",
        StageId::RenderSectionAdmission => "render-section-admission",
        StageId::RenderAdmissionDirtyReadyScan => "render-admission-dirty-ready-scan",
        StageId::RenderAdmissionRequestBuild => "render-admission-request-build",
        StageId::RenderAdmissionWorkerSubmit => "render-admission-worker-submit",
        StageId::RenderAdmissionPreparedRecordMaintenance => {
            "render-admission-prepared-record-maintenance"
        }
        StageId::CpuMeshCompile => "cpu-mesh-compile",
        StageId::CompletedResultAcceptance => "completed-result-acceptance",
        StageId::GpuUpload => "gpu-upload",
        StageId::UploadApply => "upload-apply",
        StageId::PreparedDrawRecords => "prepared-draw-records",
        StageId::DrawEncode => "draw-encode",
        StageId::GpuExecutionPresentationWait => "gpu-execution-presentation-wait",
        StageId::UiDebug => "ui-debug",
    }
}

pub fn queue_id_label(queue: &QueueId) -> &str {
    match queue {
        QueueId::InboundUpdates => "inbound-updates",
        QueueId::CompletedRenderResults => "completed-render-results",
        QueueId::UploadWork => "upload-work",
        QueueId::HostPublication => "host-publication",
        QueueId::HostPublicationRunner => "host-publication-runner",
        QueueId::HostPublicationWorldgen => "host-publication-worldgen",
        QueueId::HostPublicationLight => "host-publication-light",
        QueueId::RenderCompileJobs => "render-compile-jobs",
        QueueId::Custom(label) => label.as_str(),
    }
}

pub fn peer_thread_id_label(peer: &PeerThreadId) -> &str {
    match peer {
        PeerThreadId::ServerRunner => "server-runner",
        PeerThreadId::Worldgen => "worldgen",
        PeerThreadId::LightStatus => "light-status",
        PeerThreadId::RenderCompileWorkers => "render-compile-workers",
        PeerThreadId::Custom(label) => label.as_str(),
    }
}

fn format_optional_f64(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "none".to_owned())
}

fn format_optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_diagnostics::{
        FrameAccountingConfig, FrameAccumulator, FrameObservation, QueuePanelReport, StageSpan,
    };

    #[test]
    fn quest_marker_and_optional_value_spelling_are_stable() {
        let mut frames = FrameAccumulator::new(FrameAccountingConfig::from_target_period_ms(16.0));
        frames.record_frame(
            FrameObservation::new(1, 4.0).with_stage_span(StageSpan::new(StageId::DrawEncode, 2.5)),
        );
        let report =
            FramePipelineReport::new(frames.summary_report(), QueuePanelReport::new(vec![]));
        let lines = frame_pipeline_report_lines("MCLONE_ANDROID_XR_PERF", &report);
        assert!(lines[0].starts_with("MCLONE_ANDROID_XR_PERF_FRAME_PIPELINE "));
        assert_eq!(
            lines[1],
            "MCLONE_ANDROID_XR_PERF_STAGE stage=draw-encode label=current-frame-critical elapsed_ms=2.500 thread_cpu_ms=none"
        );
    }

    #[test]
    fn json_field_formatter_preserves_object_embedding_shape() {
        let frames = FrameAccumulator::new(FrameAccountingConfig::without_budget());
        let report =
            FramePipelineReport::new(frames.summary_report(), QueuePanelReport::new(vec![]));
        let lines = frame_pipeline_report_json_field_lines(
            "frame_pipeline_accounting",
            &report,
            "  ",
            true,
        )
        .expect("format JSON field");
        assert_eq!(
            lines.first().map(String::as_str),
            Some("  \"frame_pipeline_accounting\": {")
        );
        assert_eq!(lines.last().map(String::as_str), Some("  },"));
    }
}
