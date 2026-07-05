use serde::{Deserialize, Serialize};

use crate::{FrameSummaryReport, QueuePanelReport, StageSpan, WorstFrameDetail};

pub const FRAME_PIPELINE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePipelineReport {
    pub schema_version: u32,
    pub frame_summary: FrameSummaryReport,
    pub stage_spans: Vec<StageSpan>,
    pub queue_panel: QueuePanelReport,
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
        }
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
    }
}
