use glam::Vec3;
#[cfg(test)]
use mclone_core::ChunkPos;
use mclone_ui::{
    Color, Font, GameOptionsParent, GameScreen, GameUi, GameUiRenderState, GuiDrawList, GuiScale,
    Rect,
};

use crate::app::RenderStreamStats;
use crate::cli::HeadlessScreenshotUi;
use crate::frame_pacing::{FramePacingDebugStats, FramePacingMode, FrameTimingStats};
use crate::scene_runtime::WindowRuntimeStats;
use crate::{DEFAULT_RENDER_DISTANCE, MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DebugPaneStats {
    pub(crate) position: Vec3,
    pub(crate) speed: f32,
    pub(crate) movement_mode: &'static str,
    pub(crate) on_ground: bool,
    pub(crate) runtime: WindowRuntimeStats,
    pub(crate) render: RenderStreamStats,
    pub(crate) frame: FrameTimingStats,
    pub(crate) pacing: FramePacingDebugStats,
    pub(crate) section_occlusion: bool,
    pub(crate) force_fullbright: bool,
}

impl DebugPaneStats {
    pub(crate) fn lines(self) -> Vec<String> {
        let occlusion = if self.section_occlusion { "ON" } else { "OFF" };
        let lighting = if self.force_fullbright {
            "FULL"
        } else {
            "LIGHT"
        };
        let budget = self
            .pacing
            .target_frame_ms
            .map(|ms| format!("{ms:.1}MS"))
            .unwrap_or_else(|| "UNCAPPED".to_owned());
        let refresh = self
            .pacing
            .monitor_refresh_hz
            .map(|hz| format!("{hz:.1}HZ"))
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let runner = self
            .runtime
            .server_runner_kind
            .map(|kind| kind.label().to_ascii_uppercase())
            .unwrap_or_else(|| "REMOTE".to_owned());
        let pacing_target = match self.pacing.mode {
            FramePacingMode::Capped => format!("{}FPS", self.pacing.fps_cap),
            FramePacingMode::Vsync | FramePacingMode::Uncapped => refresh,
        };
        vec![
            "DEBUG".to_string(),
            format!(
                "POS {:.1} {:.1} {:.1}",
                self.position.x, self.position.y, self.position.z
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                self.runtime.interest_center.x, self.runtime.interest_center.z, self.speed
            ),
            format!(
                "MODE {} GROUND {}",
                self.movement_mode,
                if self.on_ground { "Y" } else { "N" }
            ),
            format!(
                "VIEW R{} T{}",
                self.runtime.render_distance, self.runtime.chunk_tracking_radius
            ),
            format!(
                "RUN {} CQ{} UQ{}",
                runner,
                self.runtime.server_command_queue_depth,
                self.runtime.server_update_queue_depth
            ),
            format!("OCC {}  {}", occlusion, lighting),
            format!(
                "TICK {} SIM {}",
                self.runtime.last_tick, self.runtime.last_simulation_tick
            ),
            format!(
                "CHUNKS L{} V{} P{}",
                self.runtime.loaded_chunks,
                self.runtime.client_visible_chunks,
                self.runtime.pending_jobs
            ),
            format!("STREAM PUB{}", self.runtime.pending_publications),
            format!(
                "TRACK P{} V{} A{} Q{}",
                self.runtime.tracked_players,
                self.runtime.player_visible_chunks,
                self.runtime.aggregate_player_ticket_chunks,
                self.runtime.player_outbound_queue_depth
            ),
            format!("MESH Q{}", self.runtime.pending_render_chunks),
            format!(
                "TICKING B{}:{} E{}:{}",
                self.runtime.block_ticking_chunks,
                self.runtime.last_simulation_block_tick_chunks,
                self.runtime.entity_ticking_chunks,
                self.runtime.last_simulation_entity_tick_chunks
            ),
            format!(
                "FLUID {}/{}/{}/{}",
                self.runtime.last_simulation_fluid_ticks_executed,
                self.runtime.last_simulation_deferred_fluid_ticks,
                self.runtime.last_simulation_fluid_mutated_blocks,
                self.runtime.scheduled_fluid_ticks
            ),
            format!(
                "DRAW S {}/{} F {}/{}",
                self.render.drawn_section_count,
                self.render.section_count,
                self.render.drawn_face_count,
                self.render.face_count
            ),
            format!(
                "ACTOR R {}/{} I{}",
                self.render.drawn_actor_count,
                self.render.actor_count,
                self.render.drawn_actor_index_count
            ),
            format!(
                "MESH R{} U{} D{} SQ{} CQ{} X{} F {:.1}MS",
                self.render.last_rebuilt_section_count,
                self.render.last_uploaded_section_count,
                self.render.last_deferred_section_count,
                self.render.last_submitted_compile_section_count,
                self.render.last_completed_compile_section_count,
                self.render.last_stale_compile_section_count,
                self.render.last_frame_ms
            ),
            format!("BUDGET {} FRAME {:.1}MS", budget, self.frame.last_frame_ms),
            format!(
                "OVER {}/{}/{} WORST {:.1}",
                self.frame.over_budget_count,
                self.frame.over_2x_budget_count,
                self.frame.over_4x_budget_count,
                self.frame.worst_frame_ms
            ),
            format!(
                "STAGE POLL {:.1} MESH {:.1} UP {:.1}",
                self.frame.last_runtime_poll_ms,
                self.frame.last_remesh_ms,
                self.frame.last_upload_ms
            ),
            format!(
                "GPU ACQ {:.1} ENC {:.1} SUB {:.1} PRS {:.1}",
                self.frame.last_surface_acquire_ms,
                self.frame.last_surface_encode_ms,
                self.frame.last_surface_submit_ms,
                self.frame.last_surface_present_ms
            ),
            format!(
                "PACE {} {} {}",
                self.pacing.mode.label(),
                pacing_target,
                self.pacing.active_present_mode_label.to_ascii_uppercase()
            ),
        ]
    }
}

impl HeadlessScreenshotUi {
    pub(crate) fn game_screen(self) -> Option<GameScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(GameScreen::Title),
            Self::Pause => Some(GameScreen::Pause),
            Self::OptionsTitle => Some(GameScreen::Options {
                parent: GameOptionsParent::Title,
            }),
            Self::OptionsPause => Some(GameScreen::Options {
                parent: GameOptionsParent::Pause,
            }),
        }
    }
}

pub(crate) fn render_debug_pane(scale: GuiScale, draw: &mut GuiDrawList, stats: &DebugPaneStats) {
    let font = Font::default();
    let line_height = font.line_height();
    let lines = stats.lines();
    let panel_width = 236.0_f32.min(scale.width - 8.0).max(120.0);
    let panel_height = 8.0 + line_height * lines.len() as f32;
    let panel = Rect::new(
        4.0,
        4.0,
        panel_width,
        panel_height.min((scale.height - 8.0).max(0.0)),
    );
    draw.fill(panel, Color::rgba(6, 9, 10, 185));
    draw.outline(panel, Color::rgba(110, 140, 136, 230));
    draw.push_clip(panel.inset(4.0));
    let text = Color::rgba(220, 238, 220, 255);
    let muted = Color::rgba(165, 186, 176, 255);
    let mut y = panel.y + 5.0;
    for (index, line) in lines.iter().enumerate() {
        font.draw_shadow(
            draw,
            line,
            panel.x + 6.0,
            y,
            if index == 0 { text } else { muted },
        );
        y += line_height;
    }
    draw.pop_clip();
}

pub(crate) fn render_static_title_ui(width: u32, height: u32) -> GuiDrawList {
    let scale = GuiScale::from_pixels(width, height);
    let mut ui = GameUi::new();
    ui.set_scale(scale);
    ui.render_draw_list(GameUiRenderState {
        render_distance: DEFAULT_RENDER_DISTANCE,
        min_render_distance: MIN_RENDER_DISTANCE,
        max_render_distance: MAX_RENDER_DISTANCE,
        ..GameUiRenderState::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_pane_formats_and_draws_runtime_stats() {
        let stats = DebugPaneStats {
            position: Vec3::new(1.25, 64.0, -2.5),
            speed: 32.0,
            movement_mode: "WALK",
            on_ground: true,
            runtime: WindowRuntimeStats {
                server_runner_kind: Some(mclone_server::ServerRunnerKind::NativeThread),
                server_command_queue_depth: 1,
                server_update_queue_depth: 2,
                interest_center: ChunkPos::new(3, -4),
                render_distance: 2,
                chunk_tracking_radius: 3,
                loaded_chunks: 9,
                pending_jobs: 1,
                pending_publications: 2,
                pending_render_chunks: 3,
                pending_render_compile_jobs: 1,
                inflight_render_sections: 4,
                client_visible_chunks: 8,
                active_ticket_chunks: 9,
                tracked_players: 1,
                player_visible_chunks: 8,
                aggregate_player_ticket_chunks: 8,
                player_outbound_queue_depth: 0,
                max_player_visible_chunks: 8,
                max_player_outbound_queue_depth: 0,
                pending_unload_chunks: 0,
                block_ticking_chunks: 4,
                entity_ticking_chunks: 2,
                last_tick: 12,
                last_simulation_tick: 11,
                last_tick_unloads_processed: 0,
                last_simulation_block_tick_chunks: 4,
                last_simulation_entity_tick_chunks: 2,
                last_simulation_scheduler_tick_ms: 0.1,
                last_simulation_block_tick_ms: 0.2,
                last_simulation_fluid_tick_ms: 0.3,
                last_simulation_entity_tick_ms: 0.4,
                last_simulation_fluid_ticks_executed: 5,
                last_simulation_deferred_fluid_ticks: 6,
                last_simulation_fluid_mutated_blocks: 7,
                scheduled_fluid_ticks: 8,
            },
            render: RenderStreamStats {
                section_count: 16,
                drawn_section_count: 10,
                face_count: 200,
                drawn_face_count: 120,
                last_rebuilt_section_count: 2,
                last_uploaded_section_count: 2,
                last_frame_ms: 16.7,
                actor_count: 2,
                drawn_actor_count: 1,
                drawn_actor_index_count: 180,
                ..RenderStreamStats::default()
            },
            frame: FrameTimingStats {
                frame_count: 12,
                over_budget_count: 3,
                over_2x_budget_count: 1,
                over_4x_budget_count: 0,
                last_frame_ms: 16.7,
                worst_frame_ms: 33.4,
                budget_ms: Some(8.3),
                last_runtime_poll_ms: 5.0,
                last_remesh_ms: 2.0,
                last_upload_ms: 1.0,
                last_surface_acquire_ms: 0.2,
                last_surface_encode_ms: 1.4,
                last_surface_submit_ms: 0.1,
                last_surface_present_ms: 0.0,
                ..FrameTimingStats::default()
            },
            pacing: FramePacingDebugStats {
                mode: FramePacingMode::Vsync,
                fps_cap: 120,
                monitor_refresh_hz: Some(120.0),
                target_frame_ms: Some(8.3),
                active_present_mode_label: "fifo",
            },
            section_occlusion: true,
            force_fullbright: false,
        };

        let lines = stats.lines();
        assert_eq!(lines[0], "DEBUG");
        assert_eq!(lines[1], "POS 1.2 64.0 -2.5");
        assert_eq!(lines[2], "CHUNK 3 -4 SPEED 32.0");
        assert_eq!(lines[3], "MODE WALK GROUND Y");
        assert_eq!(lines[4], "VIEW R2 T3");
        assert_eq!(lines[5], "RUN NATIVE-THREAD CQ1 UQ2");
        assert_eq!(lines[6], "OCC ON  LIGHT");
        assert!(lines.iter().any(|line| line == "TRACK P1 V8 A8 Q0"));
        assert!(lines.iter().any(|line| line == "ACTOR R 1/2 I180"));
        assert!(lines.iter().any(|line| line == "BUDGET 8.3MS FRAME 16.7MS"));
        assert!(lines.iter().any(|line| line == "OVER 3/1/0 WORST 33.4"));

        let mut draw = GuiDrawList::new();
        render_debug_pane(GuiScale::from_pixels(960, 540), &mut draw, &stats);
        assert!(!draw.commands().is_empty());
    }
}
