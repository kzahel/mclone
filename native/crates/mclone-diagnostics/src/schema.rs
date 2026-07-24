use serde::{Deserialize, Serialize};

use crate::{
    BudgetDecisionPanelReport, FrameSummaryReport, GpuTimestampPanelReport, PeerThreadPanelReport,
    QueuePanelReport, StageSpan, WorstFrameDetail,
};

pub const FRAME_PIPELINE_SCHEMA_VERSION: u32 = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePipelineReport {
    pub schema_version: u32,
    pub frame_summary: FrameSummaryReport,
    pub stage_spans: Vec<StageSpan>,
    pub queue_panel: QueuePanelReport,
    pub peer_thread_panel: PeerThreadPanelReport,
    pub gpu_timestamp_panel: GpuTimestampPanelReport,
    #[serde(default)]
    pub budget_decision_panel: BudgetDecisionPanelReport,
    pub worst_frames: Vec<WorstFrameDetail>,
}

impl FramePipelineReport {
    pub fn new(frame_summary: FrameSummaryReport, queue_panel: QueuePanelReport) -> Self {
        Self {
            schema_version: FRAME_PIPELINE_SCHEMA_VERSION,
            stage_spans: frame_summary.latest_stage_spans.clone(),
            worst_frames: frame_summary.worst_frames.clone(),
            frame_summary,
            queue_panel,
            peer_thread_panel: PeerThreadPanelReport::empty(),
            gpu_timestamp_panel: GpuTimestampPanelReport::unsupported(),
            budget_decision_panel: BudgetDecisionPanelReport::empty(),
        }
    }

    pub fn with_peer_thread_panel(mut self, peer_thread_panel: PeerThreadPanelReport) -> Self {
        self.peer_thread_panel = peer_thread_panel;
        self
    }

    pub fn with_gpu_timestamp_panel(
        mut self,
        gpu_timestamp_panel: GpuTimestampPanelReport,
    ) -> Self {
        self.gpu_timestamp_panel = gpu_timestamp_panel;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameAccountingConfig, FrameAccumulator, FrameObservation};

    #[test]
    fn top_level_report_serializes_schema_version() {
        let mut frames =
            FrameAccumulator::new(FrameAccountingConfig::from_target_period_ms(16.667));
        frames.record_frame(FrameObservation::new(1, 4.0));
        let frame_summary = frames.summary_report();
        let queue_panel = QueuePanelReport::new(Vec::new());
        let report = FramePipelineReport::new(frame_summary, queue_panel);
        let json = serde_json::to_value(&report).expect("serialize frame pipeline report");
        assert_eq!(json["schemaVersion"], FRAME_PIPELINE_SCHEMA_VERSION);
        assert_eq!(
            json["frameSummary"]["schemaVersion"],
            FRAME_PIPELINE_SCHEMA_VERSION
        );
        assert_eq!(
            json["queuePanel"]["schemaVersion"],
            FRAME_PIPELINE_SCHEMA_VERSION
        );
        assert_eq!(
            json["peerThreadPanel"]["schemaVersion"],
            FRAME_PIPELINE_SCHEMA_VERSION
        );
        assert_eq!(
            json["gpuTimestampPanel"]["schemaVersion"],
            FRAME_PIPELINE_SCHEMA_VERSION
        );
        assert_eq!(
            json["budgetDecisionPanel"]["schemaVersion"],
            FRAME_PIPELINE_SCHEMA_VERSION
        );
        assert_eq!(json["frameSummary"]["percentileWindowFrames"], 1);
        assert_eq!(
            json["frameSummary"]["percentileWindowCapacity"],
            serde_json::Value::Null
        );
    }
}
