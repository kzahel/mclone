use mclone_diagnostics::OverBudgetTiers;

pub const DEFAULT_FPS_CAP: u32 = 120;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FramePacingMode {
    #[default]
    Vsync,
    Capped,
    Uncapped,
}

impl FramePacingMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vsync => "VSync",
            Self::Capped => "Max FPS",
            Self::Uncapped => "Uncapped",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Vsync => Self::Capped,
            Self::Capped => Self::Uncapped,
            Self::Uncapped => Self::Vsync,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramePacingUiState {
    pub mode: FramePacingMode,
    pub fps_cap: u32,
}

impl Default for FramePacingUiState {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePacingDebugStats {
    pub mode: FramePacingMode,
    pub fps_cap: u32,
    pub monitor_refresh_hz: Option<f32>,
    pub target_frame_ms: Option<f64>,
    pub active_present_mode_label: &'static str,
}

impl Default for FramePacingDebugStats {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
            monitor_refresh_hz: None,
            target_frame_ms: None,
            active_present_mode_label: "fifo",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTimingStats {
    pub frame_count: u64,
    pub over_budget_count: u64,
    pub double_budget_count: u64,
    pub quad_budget_count: u64,
    pub last_frame_ms: f64,
    pub worst_frame_ms: f64,
    pub budget_ms: Option<f64>,
    pub last_runtime_poll_ms: f64,
    pub last_remesh_ms: f64,
    pub last_upload_ms: f64,
    pub last_render_ms: f64,
    pub last_surface_acquire_ms: f64,
    pub last_surface_encode_ms: f64,
    pub last_surface_submit_ms: f64,
    pub last_surface_present_ms: f64,
}

impl FrameTimingStats {
    pub fn begin_frame(&mut self, frame_ms: f64, budget_ms: Option<f64>) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.last_frame_ms = frame_ms;
        self.worst_frame_ms = self.worst_frame_ms.max(frame_ms);
        self.budget_ms = budget_ms;
        self.last_runtime_poll_ms = 0.0;
        self.last_remesh_ms = 0.0;
        self.last_upload_ms = 0.0;
        self.last_render_ms = 0.0;
        self.last_surface_acquire_ms = 0.0;
        self.last_surface_encode_ms = 0.0;
        self.last_surface_submit_ms = 0.0;
        self.last_surface_present_ms = 0.0;

        let mut tiers = OverBudgetTiers::default();
        tiers.observe(frame_ms, budget_ms);
        self.over_budget_count = self
            .over_budget_count
            .saturating_add(tiers.single_period_frames());
        self.double_budget_count = self
            .double_budget_count
            .saturating_add(tiers.double_period_frames());
        self.quad_budget_count = self
            .quad_budget_count
            .saturating_add(tiers.quad_period_frames());
    }

    pub fn record_runtime_poll(&mut self, ms: f64) {
        self.last_runtime_poll_ms = ms;
    }

    pub fn record_remesh_upload(&mut self, remesh_ms: f64, upload_ms: f64) {
        self.last_remesh_ms = remesh_ms;
        self.last_upload_ms = upload_ms;
    }

    pub fn record_surface_frame(
        &mut self,
        render_ms: f64,
        acquire_ms: f64,
        encode_ms: f64,
        submit_ms: f64,
        present_ms: f64,
    ) {
        self.last_render_ms = render_ms;
        self.last_surface_acquire_ms = acquire_ms;
        self.last_surface_encode_ms = encode_ms;
        self.last_surface_submit_ms = submit_ms;
        self.last_surface_present_ms = present_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_timing_counts_over_budget_tiers() {
        let mut stats = FrameTimingStats::default();

        stats.begin_frame(8.0, Some(10.0));
        stats.begin_frame(15.0, Some(10.0));
        stats.begin_frame(25.0, Some(10.0));
        stats.begin_frame(45.0, Some(10.0));

        assert_eq!(stats.over_budget_count, 3);
        assert_eq!(stats.double_budget_count, 2);
        assert_eq!(stats.quad_budget_count, 1);
        assert_eq!(stats.worst_frame_ms, 45.0);
    }
}
