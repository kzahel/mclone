use glam::Vec3;
use mclone_ui::{
    FlatDebugActorCounts, FlatDebugChunkCounts, FlatDebugDrawCounts, FlatDebugOverlay,
    FlatDebugRenderOptions, FlatDebugRunner, FlatDebugView, FlatHudDebugOverlay,
};

use crate::SingleViewRuntimeStats;
use crate::frame_pacing::{FramePacingDebugStats, FramePacingMode, FrameTimingStats};
use crate::frame_render::RenderStreamStats;

#[derive(Clone, Debug, PartialEq)]
pub struct DebugPaneStats {
    pub position: Vec3,
    pub speed: f32,
    pub movement_mode: String,
    pub on_ground: bool,
    pub seed: i64,
    pub generation_profile: &'static str,
    pub runtime: SingleViewRuntimeStats,
    pub render: RenderStreamStats,
    pub frame: FrameTimingStats,
    pub pacing: FramePacingDebugStats,
    pub section_occlusion: bool,
    pub force_fullbright: bool,
    pub color_profile: &'static str,
    pub render_scale: f32,
}

impl DebugPaneStats {
    pub fn overlay(&self) -> FlatDebugOverlay {
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
            self.movement_mode.clone(),
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
            format!("GEN {}", self.generation_profile.to_ascii_uppercase()),
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
                self.frame.double_budget_count,
                self.frame.quad_budget_count,
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

    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec!["DEBUG".to_string()];
        lines.extend(self.overlay().lines());
        lines
    }

    pub fn hud_debug_overlay(&self) -> FlatHudDebugOverlay {
        FlatHudDebugOverlay::new(self.overlay().to_debug_overlay())
    }
}
