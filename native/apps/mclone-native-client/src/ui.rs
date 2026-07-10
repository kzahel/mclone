#[cfg(test)]
use glam::Vec3;
pub(crate) use mclone_app_runtime::debug_overlay::DebugPaneStats;
#[cfg(test)]
use mclone_app_runtime::frame_pacing::{FramePacingDebugStats, FramePacingMode, FrameTimingStats};
#[cfg(test)]
use mclone_app_runtime::frame_render::RenderStreamStats;
#[cfg(test)]
use mclone_core::ChunkPos;
use mclone_ui::{GameHelpParent, GameOptionsCategory, GameOptionsParent, GameScreen};

use crate::cli::HeadlessScreenshotUi;
#[cfg(test)]
use crate::scene_runtime::WindowRuntimeStats;

impl HeadlessScreenshotUi {
    pub(crate) fn game_screen(self) -> Option<GameScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(GameScreen::Title),
            Self::WorldList => Some(GameScreen::WorldList),
            Self::WorldCreate => Some(GameScreen::WorldCreate),
            Self::WorldDeleteConfirm => Some(GameScreen::WorldDeleteConfirm {
                id: mclone_ui::WorldCatalogUiWorldId(0),
            }),
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
            Self::OptionsGraphicsPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Graphics,
            }),
            Self::OptionsMovementPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Movement,
            }),
            Self::OptionsDisplayPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Display,
            }),
            Self::OptionsDebugPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Debug,
            }),
            Self::ServerSettingsPause => Some(GameScreen::ServerSettings {
                parent: GameOptionsParent::Pause,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_pane_formats_and_draws_runtime_stats() {
        let stats = DebugPaneStats {
            position: Vec3::new(1.25, 64.0, -2.5),
            speed: 32.0,
            movement_mode: "WALK/NORMAL".to_owned(),
            on_ground: true,
            seed: 12345,
            runtime: WindowRuntimeStats {
                host_mode: mclone_app_runtime::host_mode::SingleViewHostMode::LocalIntegrated,
                server_runner_kind: Some(mclone_server::ServerRunnerKind::NativeThread),
                server_command_queue_depth: 1,
                server_update_queue_depth: 2,
                server_update_queue_bytes: 2048,
                interest_center: ChunkPos::new(3, -4),
                render_distance: 2,
                chunk_tracking_radius: 3,
                loaded_chunks: 9,
                pending_jobs: 1,
                pending_publications: 2,
                scheduler_pending_worldgen_publication_chunks: 1,
                scheduler_pending_light_publications: 1,
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
                double_budget_count: 1,
                quad_budget_count: 0,
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
        assert_eq!(lines[4], "MODE WALK/NORMAL GROUND Y");
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

        let hud_debug = stats.hud_debug_overlay();
        assert!(hud_debug.visible());
        assert_eq!(hud_debug.overlay.title, "DEBUG");
    }
}
