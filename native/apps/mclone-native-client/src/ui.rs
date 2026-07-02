use glam::Vec3;
use mclone_app_runtime::frame_render::RenderStreamStats;
#[cfg(test)]
use mclone_core::ChunkPos;
use mclone_ui::{
    FlatDebugActorCounts, FlatDebugChunkCounts, FlatDebugDrawCounts, FlatDebugOverlay,
    FlatDebugRenderOptions, FlatDebugRunner, FlatDebugView, GameHelpParent, GameOptionsParent,
    GameScreen, GuiDrawList, GuiScale, render_debug_overlay,
};

use crate::cli::HeadlessScreenshotUi;
use crate::frame_pacing::{FramePacingDebugStats, FramePacingMode, FrameTimingStats};
use crate::scene_runtime::WindowRuntimeStats;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DebugPaneStats {
    pub(crate) position: Vec3,
    pub(crate) speed: f32,
    pub(crate) movement_mode: &'static str,
    pub(crate) on_ground: bool,
    pub(crate) seed: i64,
    pub(crate) runtime: WindowRuntimeStats,
    pub(crate) render: RenderStreamStats,
    pub(crate) frame: FrameTimingStats,
    pub(crate) pacing: FramePacingDebugStats,
    pub(crate) section_occlusion: bool,
    pub(crate) force_fullbright: bool,
    pub(crate) color_profile: &'static str,
    pub(crate) render_scale: f32,
}

impl DebugPaneStats {
    pub(crate) fn overlay(self) -> FlatDebugOverlay {
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

        let mut overlay = FlatDebugOverlay::new(
            [self.position.x, self.position.y, self.position.z],
            [
                self.runtime.interest_center.x,
                self.runtime.interest_center.z,
            ],
            self.speed,
            self.movement_mode,
            self.on_ground,
            FlatDebugView::with_tracking_radius(
                self.runtime.render_distance as i32,
                self.runtime.chunk_tracking_radius as i32,
            ),
        );
        overlay.seed = Some(self.seed);
        overlay.runner = Some(FlatDebugRunner::new(
            runner,
            self.runtime.server_command_queue_depth,
            self.runtime.server_update_queue_depth,
        ));
        overlay.chunks = Some(FlatDebugChunkCounts::loaded_visible_pending(
            self.runtime.loaded_chunks,
            self.runtime.client_visible_chunks,
            self.runtime.pending_jobs,
        ));
        overlay.draw = Some(FlatDebugDrawCounts {
            drawn_sections: self.render.drawn_section_count,
            section_count: self.render.section_count,
            drawn_faces: self.render.drawn_face_count,
            face_count: self.render.face_count,
        });
        overlay.actors = Some(FlatDebugActorCounts {
            drawn_actors: self.render.drawn_actor_count,
            actor_count: self.render.actor_count,
            drawn_actor_indices: self.render.drawn_actor_index_count,
        });
        overlay.render_options = Some(FlatDebugRenderOptions {
            section_occlusion_culling: self.section_occlusion,
            force_fullbright: self.force_fullbright,
            color_profile: self.color_profile,
        });
        overlay.extra_lines = vec![
            format!("RENDER SCALE {:.2}", self.render_scale),
            format!(
                "TICK {} SIM {}",
                self.runtime.last_tick, self.runtime.last_simulation_tick
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
        ];
        overlay
    }

    #[cfg(test)]
    pub(crate) fn lines(self) -> Vec<String> {
        let mut lines = vec!["DEBUG".to_string()];
        lines.extend(self.overlay().lines());
        lines
    }
}

impl HeadlessScreenshotUi {
    pub(crate) fn game_screen(self) -> Option<GameScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(GameScreen::Title),
            Self::NewWorld => Some(GameScreen::NewWorld),
            Self::JoinRemote => Some(GameScreen::JoinRemote),
            Self::Pause => Some(GameScreen::Pause),
            Self::Help => Some(GameScreen::Help {
                parent: GameHelpParent::Game,
            }),
            Self::BlockPalette => Some(GameScreen::BlockPalette),
            Self::OptionsTitle => Some(GameScreen::Options {
                parent: GameOptionsParent::Title,
            }),
            Self::OptionsPause => Some(GameScreen::Options {
                parent: GameOptionsParent::Pause,
            }),
            Self::ServerSettingsPause => Some(GameScreen::ServerSettings {
                parent: GameOptionsParent::Pause,
            }),
        }
    }
}

pub(crate) fn render_debug_pane(scale: GuiScale, draw: &mut GuiDrawList, stats: &DebugPaneStats) {
    let overlay = stats.to_owned().overlay().to_debug_overlay();
    render_debug_overlay(scale, draw, &overlay);
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
            seed: 12345,
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
                loading_progress: None,
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
            color_profile: "VANILLA",
            render_scale: 0.5,
        };

        let lines = stats.lines();
        assert_eq!(lines[0], "DEBUG");
        assert_eq!(lines[1], "POS 1.2 64.0 -2.5");
        assert_eq!(lines[2], "CHUNK 3 -4 SPEED 32.0");
        assert_eq!(lines[3], "SEED 12345");
        assert_eq!(lines[4], "MODE WALK GROUND Y");
        assert_eq!(lines[5], "VIEW R2 T3");
        assert_eq!(lines[6], "RUN NATIVE-THREAD CQ1 UQ2");
        assert_eq!(lines[7], "CHUNKS L9 V8 P1");
        assert!(lines.iter().any(|line| line == "TRACK P1 V8 A8 Q0"));
        assert!(lines.iter().any(|line| line == "ACTOR R 1/2 I180"));
        assert!(
            lines
                .iter()
                .any(|line| line == "OCC ON  LIGHT  COLOR VANILLA")
        );
        assert!(lines.iter().any(|line| line == "RENDER SCALE 0.50"));
        assert!(lines.iter().any(|line| line == "BUDGET 8.3MS FRAME 16.7MS"));
        assert!(lines.iter().any(|line| line == "OVER 3/1/0 WORST 33.4"));

        let mut draw = GuiDrawList::new();
        render_debug_pane(GuiScale::from_pixels(960, 540), &mut draw, &stats);
        assert!(!draw.commands().is_empty());
    }
}
