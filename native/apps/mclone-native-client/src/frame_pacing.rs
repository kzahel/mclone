use std::time::{Duration, Instant};

use mclone_render::native::{
    NativeSurfaceContext, SurfacePresentModePreference, surface_present_mode_label,
};
use winit::window::Window;

pub(crate) const DEFAULT_FPS_CAP: u32 = 120;
const FPS_CAPS: [u32; 6] = [60, 90, 120, 144, 165, 240];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FramePacingMode {
    #[default]
    Vsync,
    Capped,
    Uncapped,
}

impl FramePacingMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Vsync => "VSync",
            Self::Capped => "Max FPS",
            Self::Uncapped => "Uncapped",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Vsync => Self::Capped,
            Self::Capped => Self::Uncapped,
            Self::Uncapped => Self::Vsync,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FramePacing {
    pub(crate) mode: FramePacingMode,
    pub(crate) fps_cap: u32,
    pub(crate) monitor_name: Option<String>,
    pub(crate) monitor_refresh_hz: Option<f32>,
    pub(crate) active_present_mode_label: &'static str,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
            monitor_name: None,
            monitor_refresh_hz: None,
            active_present_mode_label: "fifo",
        }
    }
}

impl FramePacing {
    pub(crate) fn present_mode_preference(&self) -> SurfacePresentModePreference {
        match self.mode {
            FramePacingMode::Vsync => SurfacePresentModePreference::Vsync,
            FramePacingMode::Capped | FramePacingMode::Uncapped => {
                SurfacePresentModePreference::NoVsync
            }
        }
    }

    pub(crate) fn target_frame_duration(&self) -> Option<Duration> {
        if self.mode != FramePacingMode::Capped {
            return None;
        }
        Some(Duration::from_secs_f64(1.0 / self.fps_cap.max(1) as f64))
    }

    pub(crate) fn target_frame_ms(&self) -> Option<f64> {
        match self.mode {
            FramePacingMode::Vsync => self
                .monitor_refresh_hz
                .filter(|refresh_hz| *refresh_hz > 1.0)
                .map(|refresh_hz| 1000.0 / refresh_hz as f64),
            FramePacingMode::Capped => Some(1000.0 / self.fps_cap.max(1) as f64),
            FramePacingMode::Uncapped => None,
        }
    }

    pub(crate) fn update_monitor(&mut self, window: &Window) {
        if let Some(monitor) = window.current_monitor() {
            self.monitor_name = monitor.name();
            self.monitor_refresh_hz = monitor
                .refresh_rate_millihertz()
                .map(|millihertz| millihertz as f32 / 1000.0);
        } else {
            self.monitor_name = None;
            self.monitor_refresh_hz = None;
        }
    }

    pub(crate) fn apply_to_surface(&mut self, surface: &mut NativeSurfaceContext) {
        let active = surface.set_present_mode_preference(self.present_mode_preference());
        self.active_present_mode_label = surface_present_mode_label(active);
    }

    pub(crate) fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
    }

    pub(crate) fn cycle_fps_cap(&mut self) {
        let current = FPS_CAPS
            .iter()
            .position(|cap| *cap == self.fps_cap)
            .unwrap_or_else(|| {
                FPS_CAPS
                    .iter()
                    .position(|cap| *cap >= self.fps_cap)
                    .unwrap_or(FPS_CAPS.len() - 1)
            });
        self.fps_cap = FPS_CAPS[(current + 1) % FPS_CAPS.len()];
    }

    pub(crate) fn ui_state(&self) -> FramePacingUiState {
        FramePacingUiState {
            mode: self.mode,
            fps_cap: self.fps_cap,
        }
    }

    pub(crate) fn debug_stats(&self) -> FramePacingDebugStats {
        FramePacingDebugStats {
            mode: self.mode,
            fps_cap: self.fps_cap,
            monitor_refresh_hz: self.monitor_refresh_hz,
            target_frame_ms: self.target_frame_ms(),
            active_present_mode_label: self.active_present_mode_label,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FramePacingUiState {
    pub(crate) mode: FramePacingMode,
    pub(crate) fps_cap: u32,
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
pub(crate) struct FramePacingDebugStats {
    pub(crate) mode: FramePacingMode,
    pub(crate) fps_cap: u32,
    pub(crate) monitor_refresh_hz: Option<f32>,
    pub(crate) target_frame_ms: Option<f64>,
    pub(crate) active_present_mode_label: &'static str,
}

impl Default for FramePacingDebugStats {
    fn default() -> Self {
        FramePacing::default().debug_stats()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FrameTimingStats {
    pub(crate) frame_count: u64,
    pub(crate) over_budget_count: u64,
    pub(crate) over_2x_budget_count: u64,
    pub(crate) over_4x_budget_count: u64,
    pub(crate) last_frame_ms: f64,
    pub(crate) worst_frame_ms: f64,
    pub(crate) budget_ms: Option<f64>,
    pub(crate) last_runtime_poll_ms: f64,
    pub(crate) last_remesh_ms: f64,
    pub(crate) last_upload_ms: f64,
    pub(crate) last_render_ms: f64,
    pub(crate) last_surface_acquire_ms: f64,
    pub(crate) last_surface_encode_ms: f64,
    pub(crate) last_surface_submit_ms: f64,
    pub(crate) last_surface_present_ms: f64,
}

impl FrameTimingStats {
    pub(crate) fn begin_frame(&mut self, frame_ms: f64, budget_ms: Option<f64>) {
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

        if let Some(budget_ms) = budget_ms.filter(|budget_ms| *budget_ms > 0.0) {
            if frame_ms > budget_ms {
                self.over_budget_count = self.over_budget_count.saturating_add(1);
            }
            if frame_ms > budget_ms * 2.0 {
                self.over_2x_budget_count = self.over_2x_budget_count.saturating_add(1);
            }
            if frame_ms > budget_ms * 4.0 {
                self.over_4x_budget_count = self.over_4x_budget_count.saturating_add(1);
            }
        }
    }

    pub(crate) fn record_runtime_poll(&mut self, ms: f64) {
        self.last_runtime_poll_ms = ms;
    }

    pub(crate) fn record_remesh_upload(&mut self, remesh_ms: f64, upload_ms: f64) {
        self.last_remesh_ms = remesh_ms;
        self.last_upload_ms = upload_ms;
    }

    pub(crate) fn record_surface_frame(
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

pub(crate) fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RedrawSchedule {
    RequestNow,
    WaitUntil(Instant),
}

pub(crate) fn redraw_schedule(
    mode: FramePacingMode,
    now: Instant,
    next_redraw_at: Option<Instant>,
) -> RedrawSchedule {
    if mode == FramePacingMode::Capped
        && let Some(deadline) = next_redraw_at
        && now < deadline
    {
        return RedrawSchedule::WaitUntil(deadline);
    }
    RedrawSchedule::RequestNow
}

pub(crate) fn next_capped_redraw_deadline(
    frame_start: Instant,
    finish: Instant,
    previous_deadline: Option<Instant>,
    frame_duration: Duration,
) -> Instant {
    let mut next = previous_deadline.unwrap_or(frame_start) + frame_duration;
    while next <= finish {
        next += frame_duration;
    }
    next
}

pub(crate) fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_redraw_deadline_skips_missed_frames() {
        let frame_duration = Duration::from_millis(8);
        let start = Instant::now();
        let previous_deadline = start + frame_duration;
        let finish = start + Duration::from_millis(27);

        let next =
            next_capped_redraw_deadline(start, finish, Some(previous_deadline), frame_duration);

        assert!(next > finish);
        assert_eq!(next.duration_since(start), frame_duration * 4);
    }

    #[test]
    fn capped_redraw_schedule_waits_until_due() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(16);

        assert_eq!(
            redraw_schedule(FramePacingMode::Capped, now, Some(deadline)),
            RedrawSchedule::WaitUntil(deadline)
        );
        assert_eq!(
            redraw_schedule(FramePacingMode::Capped, deadline, Some(deadline)),
            RedrawSchedule::RequestNow
        );
        assert_eq!(
            redraw_schedule(FramePacingMode::Capped, now, None),
            RedrawSchedule::RequestNow
        );
        assert_eq!(
            redraw_schedule(FramePacingMode::Vsync, now, Some(deadline)),
            RedrawSchedule::RequestNow
        );
    }

    #[test]
    fn frame_timing_counts_budget_overruns() {
        let mut stats = FrameTimingStats::default();

        stats.begin_frame(9.0, Some(8.0));
        stats.begin_frame(17.0, Some(8.0));
        stats.begin_frame(33.0, Some(8.0));

        assert_eq!(stats.frame_count, 3);
        assert_eq!(stats.over_budget_count, 3);
        assert_eq!(stats.over_2x_budget_count, 2);
        assert_eq!(stats.over_4x_budget_count, 1);
        assert_eq!(stats.worst_frame_ms, 33.0);
    }

    #[test]
    fn frame_pacing_cycles_mode_and_fps_cap() {
        let mut pacing = FramePacing::default();

        pacing.cycle_mode();
        assert_eq!(pacing.mode, FramePacingMode::Capped);
        pacing.cycle_fps_cap();
        assert_eq!(pacing.fps_cap, 144);
    }
}
