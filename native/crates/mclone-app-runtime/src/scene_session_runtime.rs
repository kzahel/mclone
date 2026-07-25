//! Target-neutral scene/session shell.
//!
//! The shell owns session identity and exposes the runtime policy surface used
//! by `mclone-scene`. Concrete server connections, compilers, drop drains, and
//! other platform services are assembled outside it and hidden behind one
//! object-safe boundary.

use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

use anyhow::Result;
use glam::Vec3;
use mclone_client::{ClientRuntime, ClientSessionPhase};
use mclone_core::{BlockStateId, ChunkPos};
use mclone_mesh::{
    RenderSectionKey, TexturedRenderSectionBuildReport, TexturedRenderSectionMesh,
    TexturedRenderSectionMetadata,
};
use mclone_protocol::{ClientCommand, ClientEphemeralMessage, PlayerPositionUpdate};
use mclone_render_session::{RenderSectionCacheUpdate, RenderSectionCompileQueueHealth};
use mclone_server::SimulationCadenceConfig;
use mclone_ui::LoadingProgressOverlay;

use crate::host_mode::SingleViewHostMode;
use crate::monotonic::MonotonicDeadline;
use crate::render_asset_data::TexturedMeshAssets;
use crate::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, SessionStatus,
};
use crate::{
    GameplayCommandSubmission, GameplayCommandTiming, GameplayCommandUpdatePolicy,
    RuntimePollDiagnostics, RuntimeUpdatePumpBudget, SingleViewRuntime, SingleViewRuntimeStats,
    TargetRenderWorkStats, TimedRenderSectionCacheUpdate,
    chunk_tracking_radius_for_render_distance,
};

pub const DEFAULT_STARTUP_READINESS_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StartupReadinessPolicy {
    #[default]
    Playable,
    Idle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeRenderPriority {
    #[default]
    Active,
    Standby,
}

impl RuntimeRenderPriority {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Standby => "standby",
        }
    }
}

/// Platform-neutral evidence required before a client may enter gameplay.
///
/// Native startup pumps and asynchronously driven browser startup both use
/// this contract. Host readiness proves that the authoritative spawn/view is
/// present; drawable coverage prevents admitting into a blank frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StartupAdmissionEvidence {
    pub host_ready: bool,
    pub drawable_section_count: usize,
}

impl StartupAdmissionEvidence {
    pub const fn ready(self) -> bool {
        self.host_ready && self.drawable_section_count > 0
    }
}

pub trait SceneRuntimeService {
    fn host_mode(&self) -> SingleViewHostMode;
    fn host_label(&self) -> &'static str;
    fn core(&self) -> &SingleViewRuntime;
    fn core_mut(&mut self) -> &mut SingleViewRuntime;
    fn mesh_assets(&self) -> &TexturedMeshAssets;
    fn replace_asset_epoch(
        &mut self,
        epoch: u64,
        mesh_assets: TexturedMeshAssets,
        sections: TexturedRenderSectionBuildReport,
    ) -> Result<()>;
    fn release_render_compile_jobs(&mut self, count: usize) -> usize;
    fn simulation_cadence(&self) -> Option<SimulationCadenceConfig>;
    fn set_simulation_cadence(&mut self, cadence: SimulationCadenceConfig) -> Result<bool>;
    fn set_render_priority(&mut self, _priority: RuntimeRenderPriority) {}
    fn loaded_chunk_count(&self) -> usize;
    fn set_chunk_view_with_update_policy_timed(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)>;
    fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission>;
    fn send_ephemeral_with_update_policy_timed(
        &mut self,
        message: ClientEphemeralMessage,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission>;
    fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool>;
    fn poll_until_idle_with_timeout(&mut self, timeout: Duration) -> Result<(usize, f64)>;
    fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate>;
    fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: MonotonicDeadline,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate>;
    fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: MonotonicDeadline,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate>;
    fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate>;
    fn resident_section_metadata(&self) -> Vec<TexturedRenderSectionMetadata>;
    fn cached_section_count(&self) -> usize;
    fn mark_all_render_sections_dirty_for_resource_rebuild(&mut self) -> usize;
    fn recompile_all_render_section_meshes_for_resource_rebuild(
        &mut self,
        camera_position: Vec3,
    ) -> Result<Vec<TexturedRenderSectionMesh>>;
    fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey>;
    fn sky_clear_color(&self) -> wgpu::Color;
    fn render_compile_pending_job_count(&self) -> usize;
    fn render_compile_max_pending_job_count(&self) -> usize;
    fn render_compile_available_pending_job_slots(&self) -> usize;
    fn render_compile_queue_health(&self) -> RenderSectionCompileQueueHealth;
    fn view_readiness_overlay(&self) -> Option<LoadingProgressOverlay>;
    fn flush_persistence(&mut self) -> Result<usize>;
    fn promote_observer_to_player(&mut self) -> Result<()> {
        anyhow::bail!("scene runtime does not own a promotable local observer")
    }
    fn demote_player_to_observer(&mut self) -> Result<()> {
        anyhow::bail!("scene runtime does not own a demotable local player")
    }
    fn debug_break_observed_block(&mut self, _pos: mclone_core::BlockPos) -> Result<bool> {
        anyhow::bail!("scene runtime does not own a mutable local observer")
    }
    fn refresh_startup_diagnostics(&mut self) -> Result<()>;
    fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay>;
    fn startup_host_ready(&self, policy: StartupReadinessPolicy, camera_position: Vec3) -> bool;
    fn stats(&self) -> SingleViewRuntimeStats;
}

pub struct SceneSessionRuntime {
    session: GameSessionCoordinator<()>,
    service: Box<dyn SceneRuntimeService>,
}

impl std::fmt::Debug for SceneSessionRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SceneSessionRuntime")
            .field("session", &self.session)
            .field("host_mode", &self.service.host_mode())
            .finish_non_exhaustive()
    }
}

impl SceneSessionRuntime {
    pub fn from_active_service(
        descriptor: ActiveSessionDescriptor,
        service: Box<dyn SceneRuntimeService>,
    ) -> Self {
        let mut session = GameSessionCoordinator::new();
        session.complete_start(descriptor);
        Self { session, service }
    }

    pub fn session(&self) -> &GameSessionCoordinator<()> {
        &self.session
    }

    pub fn session_state(&self) -> &GameSessionState {
        self.session.state()
    }

    pub fn active_session(&self) -> Option<&ActiveSessionDescriptor> {
        match self.session.state() {
            GameSessionState::Active { session } => Some(session),
            GameSessionState::NoSession
            | GameSessionState::Starting { .. }
            | GameSessionState::Failed { .. } => None,
        }
    }

    pub fn session_status(&self) -> Option<SessionStatus> {
        client_session_status(self.client(), self.session.status())
    }

    pub fn client(&self) -> &ClientRuntime {
        self.service.core().client()
    }

    pub fn startup_host_ready(
        &self,
        policy: StartupReadinessPolicy,
        camera_position: Vec3,
    ) -> bool {
        self.service.startup_host_ready(policy, camera_position)
    }

    pub fn render_distance(&self) -> u32 {
        self.service.core().render_distance()
    }

    pub fn chunk_tracking_radius(&self) -> u32 {
        self.service.core().chunk_tracking_radius()
    }

    pub fn interest_center(&self) -> ChunkPos {
        self.service.core().interest_center()
    }

    pub fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        self.set_chunk_view(center, self.render_distance(), self.chunk_tracking_radius())
    }

    pub fn set_interest_center_with_update_policy_timed(
        &mut self,
        center: ChunkPos,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        let render_distance = self.render_distance();
        let chunk_tracking_radius = self.chunk_tracking_radius();
        self.set_chunk_view_with_update_policy_timed(
            center,
            render_distance,
            chunk_tracking_radius,
            policy,
        )
    }

    pub fn set_render_distance(&mut self, render_distance: u32) -> Result<bool> {
        self.set_chunk_view(
            self.interest_center(),
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
    }

    pub fn set_grass_patches_enabled(&mut self, enabled: bool) -> usize {
        self.service.core_mut().set_grass_patches_enabled(enabled)
    }

    pub fn grass_patches_enabled(&self) -> bool {
        self.service.core().grass_patches_enabled()
    }

    pub fn set_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<bool> {
        self.set_chunk_view_with_update_policy_timed(
            center,
            render_distance,
            chunk_tracking_radius,
            GameplayCommandUpdatePolicy::SendOnly,
        )
        .map(|(changed, _)| changed)
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<()> {
        self.send_gameplay_command_with_update_policy_timed(
            command,
            GameplayCommandUpdatePolicy::SendOnly,
        )
        .map(|_| ())
    }

    pub fn send_gameplay_command_timed(
        &mut self,
        command: ClientCommand,
    ) -> Result<GameplayCommandSubmission> {
        self.send_gameplay_command_with_update_policy_timed(
            command,
            GameplayCommandUpdatePolicy::DrainImmediately,
        )
    }

    pub fn send_ephemeral_with_update_policy_timed(
        &mut self,
        message: ClientEphemeralMessage,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        self.service
            .send_ephemeral_with_update_policy_timed(message, policy)
    }

    pub fn drain_player_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.core_mut().drain_player_position_updates()
    }

    pub fn promote_observer_to_player(&mut self) -> Result<()> {
        self.service.promote_observer_to_player()
    }

    pub fn demote_player_to_observer(&mut self) -> Result<()> {
        self.service.demote_player_to_observer()
    }

    pub fn debug_break_observed_block(&mut self, pos: mclone_core::BlockPos) -> Result<bool> {
        self.service.debug_break_observed_block(pos)
    }

    pub fn poll(&mut self) -> Result<bool> {
        self.poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_with_completed_result_acceptance_timed(camera_position, None)
            .map(|timed| timed.cache_update)
    }

    pub fn pending_completed_compile_result_count(&self) -> usize {
        self.core().pending_completed_compile_result_count()
    }

    pub fn pending_render_chunk_count(&self) -> usize {
        self.core().pending_render_chunk_count()
    }

    pub fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.core().last_poll_diagnostics()
    }

    pub fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.core()
            .has_pending_render_work(self.render_compile_pending_job_count(), camera_position)
    }

    pub fn target_render_work_stats(&self, camera_position: Vec3) -> TargetRenderWorkStats {
        self.core().target_render_work_stats(camera_position)
    }

    pub fn sky_clear_color(&self) -> wgpu::Color {
        self.service.sky_clear_color()
    }

    pub fn time_of_day(&self) -> f32 {
        self.core().time_of_day()
    }

    pub fn sun_angle(&self) -> f32 {
        self.core().sun_angle()
    }

    pub fn force_day_time(&mut self, day_time: u64) {
        self.core_mut().force_day_time(day_time);
    }

    pub fn camera_inside_water(&self, position: Vec3) -> bool {
        self.core().camera_inside_water(position)
    }

    pub fn highest_non_air_block_y_at_world(&self, world_x: i32, world_z: i32) -> Option<i32> {
        self.core()
            .highest_non_air_block_y_at_world(world_x, world_z)
    }

    pub fn block_state_at_position(&self, position: Vec3) -> Option<BlockStateId> {
        self.core().block_state_at_position(position)
    }
}

fn client_session_status(
    client: &ClientRuntime,
    fallback: Option<SessionStatus>,
) -> Option<SessionStatus> {
    match client.session_phase() {
        ClientSessionPhase::Connecting => Some(SessionStatus {
            message: "Connecting...".to_owned(),
            ok: true,
        }),
        ClientSessionPhase::Configuring => Some(SessionStatus {
            message: "Configuring session...".to_owned(),
            ok: true,
        }),
        ClientSessionPhase::Playing => fallback,
        ClientSessionPhase::Disconnected => {
            let reason = client.disconnect_reason();
            Some(SessionStatus {
                message: reason.map_or_else(
                    || "Disconnected".to_owned(),
                    |reason| {
                        if reason.detail.is_empty() {
                            format!("Disconnected: {:?}", reason.code)
                        } else {
                            format!("Disconnected: {}", reason.detail)
                        }
                    },
                ),
                ok: false,
            })
        }
    }
}

impl Deref for SceneSessionRuntime {
    type Target = dyn SceneRuntimeService;

    fn deref(&self) -> &Self::Target {
        self.service.as_ref()
    }
}

impl DerefMut for SceneSessionRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.service.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::{StartupAdmissionEvidence, client_session_status};
    use mclone_client::{ClientHost, ClientRuntime};
    use mclone_protocol::{
        DisconnectReason, DisconnectReasonCode, ServerUpdate, SessionCapabilities,
        SessionConfiguration,
    };

    #[test]
    fn session_status_projects_protocol_phase_and_disconnect_reason() {
        let mut client = ClientRuntime::new(ClientHost::RemoteDedicated);
        assert_eq!(
            client_session_status(&client, None).map(|status| status.message),
            Some("Connecting...".to_owned())
        );

        client.apply_update(ServerUpdate::SessionConfiguration(
            SessionConfiguration::fixed_vanilla(11, 11, SessionCapabilities::NONE),
        ));
        assert_eq!(
            client_session_status(&client, None).map(|status| status.message),
            Some("Configuring session...".to_owned())
        );

        client.apply_update(ServerUpdate::SessionReady);
        assert_eq!(client_session_status(&client, None), None);

        client.apply_update(ServerUpdate::Disconnect(DisconnectReason::new(
            DisconnectReasonCode::Kicked,
            "operator kick",
        )));
        assert_eq!(
            client_session_status(&client, None),
            Some(crate::session::SessionStatus {
                message: "Disconnected: operator kick".to_owned(),
                ok: false,
            })
        );
    }

    #[test]
    fn startup_admission_requires_authority_and_drawable_coverage() {
        let ready = StartupAdmissionEvidence {
            host_ready: true,
            drawable_section_count: 1,
        };
        assert!(ready.ready());
        assert!(
            !StartupAdmissionEvidence {
                host_ready: false,
                ..ready
            }
            .ready()
        );
        assert!(
            !StartupAdmissionEvidence {
                drawable_section_count: 0,
                ..ready
            }
            .ready()
        );
    }
}
