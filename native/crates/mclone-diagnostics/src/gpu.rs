use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuPassId {
    Sky,
    Terrain,
    TerrainOpaque,
    TerrainTranslucent,
    Actor,
    Ui,
    PerEyeLeft,
    PerEyeRight,
    Multiview,
    Calibration,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTimestampPassReport {
    pub pass: GpuPassId,
    pub elapsed_ms: f64,
    pub begin_tick: u64,
    pub end_tick: u64,
    pub valid: bool,
}

impl GpuTimestampPassReport {
    pub fn new(pass: GpuPassId, elapsed_ms: f64, begin_tick: u64, end_tick: u64) -> Self {
        Self {
            pass,
            elapsed_ms: sanitize_ms(elapsed_ms),
            begin_tick,
            end_tick,
            valid: end_tick != 0 && end_tick >= begin_tick,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTimestampPanelReport {
    pub schema_version: u32,
    pub supported: bool,
    pub timestamp_period_ns: Option<f64>,
    pub frames_submitted: u64,
    pub frames_resolved: u64,
    pub pending_frames: u64,
    pub dropped_frames: u64,
    pub latest_passes: Vec<GpuTimestampPassReport>,
}

impl GpuTimestampPanelReport {
    pub fn unsupported() -> Self {
        Self {
            schema_version: crate::FRAME_PIPELINE_SCHEMA_VERSION,
            supported: false,
            timestamp_period_ns: None,
            frames_submitted: 0,
            frames_resolved: 0,
            pending_frames: 0,
            dropped_frames: 0,
            latest_passes: Vec::new(),
        }
    }

    pub fn supported(timestamp_period_ns: f64) -> Self {
        Self {
            supported: true,
            timestamp_period_ns: Some(sanitize_ms(timestamp_period_ns)),
            ..Self::unsupported()
        }
    }

    pub fn with_counters(
        mut self,
        frames_submitted: u64,
        frames_resolved: u64,
        pending_frames: u64,
        dropped_frames: u64,
    ) -> Self {
        self.frames_submitted = frames_submitted;
        self.frames_resolved = frames_resolved;
        self.pending_frames = pending_frames;
        self.dropped_frames = dropped_frames;
        self
    }

    pub fn with_latest_passes(mut self, latest_passes: Vec<GpuTimestampPassReport>) -> Self {
        self.latest_passes = latest_passes;
        self
    }
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_timestamp_panel_defaults_to_unsupported() {
        let report = GpuTimestampPanelReport::unsupported();
        assert!(!report.supported);
        assert_eq!(report.schema_version, crate::FRAME_PIPELINE_SCHEMA_VERSION);
        assert!(report.latest_passes.is_empty());
    }

    #[test]
    fn gpu_timestamp_pass_report_marks_unwritten_end_tick_invalid() {
        let report = GpuTimestampPassReport::new(GpuPassId::Calibration, 1.0, 42, 0);
        assert!(!report.valid);
        assert_eq!(report.elapsed_ms, 1.0);
    }
}
