use std::collections::BTreeSet;
use std::time::Duration;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::far_lod::FarTerrainLodConfig;
use mclone_app_runtime::host_mode::{SingleViewHostOptions, build_remote_dedicated_client_runtime};
use mclone_app_runtime::local_single_view::{
    LocalSingleViewSceneOptions, LocalSingleViewSceneRuntime, LocalSingleViewStartupPump,
    LocalSingleViewStartupStep, NativeSingleViewSceneRuntime,
    build_local_single_view_client_runtime,
};
use mclone_app_runtime::{
    GameplayCommandUpdatePolicy, RuntimePollDiagnostics, RuntimeUpdatePumpBudget,
    SingleViewRuntimeStats, TargetRenderWorkStats, TimedRenderSectionCacheUpdate,
};
#[cfg(test)]
use mclone_app_runtime::{camera_position_inside_water_block, snapshot_block_state_at_world};
pub(crate) use mclone_app_runtime::{chunk_tracking_radius_for_render_distance, square_count};
#[cfg(test)]
use mclone_client::ClientHost;
use mclone_client::ClientRuntime;
#[cfg(test)]
use mclone_core::AIR_BLOCK_STATE_ID;
use mclone_core::ChunkPos;
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
#[cfg(test)]
use mclone_protocol::ServerUpdate;
use mclone_protocol::{ClientCommand, PlayerPositionUpdate};
use mclone_render::far_lod::FarTerrainLodMesh;
use mclone_server::SimulationCadenceConfig;
#[cfg(test)]
use mclone_server::{IntegratedServer, ServerRunnerKind};
use mclone_ui::LoadingProgressOverlay;

use crate::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use crate::cli::SceneOptions;
use crate::remote_session::RemoteServerSession;
use crate::render_cache::{SceneTexturedSections, TexturedMeshAssets, load_textured_mesh_assets};
use mclone_render_session::{
    EngineCameraController, RenderSectionCacheUpdate, build_client_textured_sections,
};
#[cfg(test)]
use mclone_render_session::{RenderSectionSession, render_section_neighbor_readiness};

pub(crate) type WindowRuntimeStats = SingleViewRuntimeStats;
pub(crate) type NativeWindowSceneRuntime = NativeSingleViewSceneRuntime<RemoteServerSession>;

#[derive(Clone, Debug)]
pub(crate) struct WindowSceneAssets {
    pub(crate) mesh_assets: TexturedMeshAssets,
    pub(crate) actor_textures: ActorTextureAssets,
}

impl WindowSceneAssets {
    pub(crate) fn load() -> Result<Self> {
        Ok(Self {
            mesh_assets: load_textured_mesh_assets()?,
            actor_textures: load_actor_texture_assets()?,
        })
    }
}

fn scene_render_distance(scene: &SceneOptions) -> Result<u32> {
    u32::try_from(scene.render_distance).context("render distance must be non-negative")
}

pub(crate) fn build_scene_textured_sections(scene: &SceneOptions) -> Result<SceneTexturedSections> {
    let client = build_scene_client_runtime(scene)?;
    let mesh_assets = load_textured_mesh_assets()?;
    let build = build_client_textured_sections(&client, &mesh_assets.catalog)?;
    if build.sections.is_empty() {
        bail!(
            "generated chunk area seed={} center=({}, {}) render_distance={} produced no textured render sections",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.render_distance
        );
    }
    Ok(SceneTexturedSections {
        sections: build.sections,
        visibility_graph_stats: build.visibility_graph,
        atlas: mesh_assets.atlas,
    })
}

fn build_scene_client_runtime(scene: &SceneOptions) -> Result<ClientRuntime> {
    let render_distance = scene_render_distance(scene)?;
    let center = ChunkPos::new(scene.chunk_x, scene.chunk_z);
    let Some(remote_addr) = &scene.remote_addr else {
        return build_local_single_view_client_runtime(local_single_view_options(scene)?);
    };

    let mut session = RemoteServerSession::connect(remote_addr.as_str())?;
    build_remote_dedicated_client_runtime(
        SingleViewHostOptions::new(center, render_distance)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs),
        &mut session,
    )
}

pub(crate) fn local_single_view_options(
    scene: &SceneOptions,
) -> Result<LocalSingleViewSceneOptions> {
    let mut options = LocalSingleViewSceneOptions::new(
        scene.seed,
        ChunkPos::new(scene.chunk_x, scene.chunk_z),
        scene_render_distance(scene)?,
    )
    .with_day_time(scene.day_time_override)
    .with_freeze_time(scene.freeze_time)
    .with_cadence(scene.simulation_cadence)
    .with_debug_passive_showcase(scene.debug_passive_showcase)
    .with_lighting_enabled(scene.lighting_enabled)
    .with_adaptive_chunk_publication_budget(scene.adaptive_chunk_publication_budget)
    .with_render_compile_worker_count(scene.render_compile_worker_count)
    .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs);
    if let Some(world_dir) = &scene.world_dir {
        options = options.with_persistent_world_dir(world_dir.clone());
    }
    Ok(options)
}

#[cfg_attr(not(feature = "xr"), allow(dead_code))]
pub(crate) fn native_window_scene_runtime(
    scene: &SceneOptions,
) -> Result<NativeWindowSceneRuntime> {
    native_window_scene_runtime_with_mesh_assets(scene, load_textured_mesh_assets()?)
}

pub(crate) fn native_window_scene_runtime_with_mesh_assets(
    scene: &SceneOptions,
    mesh_assets: TexturedMeshAssets,
) -> Result<NativeWindowSceneRuntime> {
    let render_distance = scene_render_distance(scene)?;
    let center = ChunkPos::new(scene.chunk_x, scene.chunk_z);
    let Some(remote_addr) = &scene.remote_addr else {
        return NativeWindowSceneRuntime::local_with_mesh_assets(
            local_single_view_options(scene)?,
            mesh_assets,
        );
    };

    let session = RemoteServerSession::connect(remote_addr.as_str())?;
    NativeWindowSceneRuntime::remote_dedicated_with_mesh_assets(
        SingleViewHostOptions::new(center, render_distance)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs),
        session,
        mesh_assets,
    )
    .with_context(|| {
        format!("failed to initialize desktop remote dedicated runtime from {remote_addr}")
    })
}

#[derive(Debug)]
pub(crate) struct WindowSceneRuntime {
    scene: NativeWindowSceneRuntime,
    pub(crate) actor_textures: ActorTextureAssets,
}

#[derive(Debug)]
pub(crate) struct WindowSceneStartupPump {
    pump: LocalSingleViewStartupPump,
    actor_textures: ActorTextureAssets,
}

impl WindowSceneStartupPump {
    pub(crate) fn new_local(scene: &SceneOptions, assets: &WindowSceneAssets) -> Result<Self> {
        Self::with_local_options(
            local_single_view_options(scene)?.with_initial_spawn_center(),
            assets,
        )
    }

    pub(crate) fn with_local_options(
        options: LocalSingleViewSceneOptions,
        assets: &WindowSceneAssets,
    ) -> Result<Self> {
        Ok(Self {
            pump: LocalSingleViewStartupPump::with_mesh_assets(options, assets.mesh_assets.clone())
                .context("failed to create local world startup pump")?,
            actor_textures: assets.actor_textures.clone(),
        })
    }

    pub(crate) fn step(&mut self, camera_position: Vec3) -> Result<LocalSingleViewStartupStep> {
        self.pump.step(camera_position)
    }

    pub(crate) fn progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.pump.progress_overlay()
    }

    pub(crate) fn into_runtime(self) -> WindowSceneRuntime {
        WindowSceneRuntime::from_local_runtime(self.pump.into_runtime(), self.actor_textures)
    }
}

impl WindowSceneRuntime {
    pub(crate) fn new(scene: &SceneOptions) -> Result<Self> {
        Self::with_assets(scene, &WindowSceneAssets::load()?)
    }

    pub(crate) fn with_assets(scene: &SceneOptions, assets: &WindowSceneAssets) -> Result<Self> {
        Ok(Self {
            scene: native_window_scene_runtime_with_mesh_assets(scene, assets.mesh_assets.clone())?,
            actor_textures: assets.actor_textures.clone(),
        })
    }

    fn from_local_runtime(
        runtime: LocalSingleViewSceneRuntime,
        actor_textures: ActorTextureAssets,
    ) -> Self {
        Self {
            scene: NativeWindowSceneRuntime::Local(runtime),
            actor_textures,
        }
    }

    pub(crate) fn client(&self) -> &ClientRuntime {
        self.scene.client()
    }

    pub(crate) fn mesh_assets(&self) -> &TexturedMeshAssets {
        self.scene.mesh_assets()
    }

    pub(crate) fn clear_far_lod(&mut self) {
        self.scene.clear_far_lod();
    }

    pub(crate) fn prepare_far_lod_mesh(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        camera_position: Vec3,
    ) -> Option<&FarTerrainLodMesh> {
        self.scene
            .prepare_far_lod_mesh(config, seed, center, camera_position)
    }

    #[cfg(test)]
    fn render_session(&self) -> &RenderSectionSession {
        self.scene.core().render_session()
    }

    #[cfg(test)]
    fn render_session_mut(&mut self) -> &mut RenderSectionSession {
        self.scene.core_mut().render_session_mut()
    }

    pub(crate) fn render_distance(&self) -> u32 {
        self.scene.render_distance()
    }

    pub(crate) fn render_compile_pending_job_count(&self) -> usize {
        self.scene.render_compile_pending_job_count()
    }

    pub(crate) fn render_compile_max_pending_job_count(&self) -> usize {
        self.scene.render_compile_max_pending_job_count()
    }

    pub(crate) fn render_compile_available_pending_job_slots(&self) -> usize {
        self.scene.render_compile_available_pending_job_slots()
    }

    pub(crate) fn simulation_cadence(&self) -> Option<SimulationCadenceConfig> {
        self.scene.simulation_cadence()
    }

    pub(crate) fn set_simulation_cadence(
        &mut self,
        cadence: SimulationCadenceConfig,
    ) -> Result<bool> {
        self.scene.set_simulation_cadence(cadence)
    }

    pub(crate) fn chunk_tracking_radius(&self) -> u32 {
        self.scene.chunk_tracking_radius()
    }

    pub(crate) fn interest_center(&self) -> ChunkPos {
        self.scene.interest_center()
    }

    pub(crate) fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        self.scene.set_interest_center(center)
    }

    pub(crate) fn set_render_distance(&mut self, render_distance: u32) -> Result<bool> {
        self.scene.set_render_distance(render_distance)
    }

    pub(crate) fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        self.scene.send_gameplay_command(command)
    }

    pub(crate) fn drain_player_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.scene.drain_player_position_updates()
    }

    pub(crate) fn commit_engine_camera_player_pose(
        &mut self,
        camera: &mut EngineCameraController,
    ) -> Result<bool> {
        let server_changed = self.sync_engine_camera_player_pose(camera)?;
        let interest_changed = self.update_interest_from_engine_camera(camera)?;
        Ok(server_changed || interest_changed)
    }

    pub(crate) fn sync_engine_camera_player_pose(
        &mut self,
        camera: &mut EngineCameraController,
    ) -> Result<bool> {
        let changed = if let Some(report) = camera.next_pose_sync_command() {
            self.scene
                .send_gameplay_command_with_update_policy(
                    report.command,
                    GameplayCommandUpdatePolicy::SendOnly,
                )
                .context("failed to sync player pose to server")?
        } else {
            false
        };
        Ok(changed || self.apply_pending_engine_camera_position_updates(camera)?)
    }

    pub(crate) fn apply_pending_engine_camera_position_updates(
        &mut self,
        camera: &mut EngineCameraController,
    ) -> Result<bool> {
        let mut changed = false;
        for update in self.drain_player_position_updates() {
            let accepted = camera.accept_position_update(update);
            self.send_gameplay_command(accepted.accept_command)
                .context("failed to acknowledge player position correction")?;
            let resync = camera.corrected_pose_sync_command();
            self.send_gameplay_command(resync.command)
                .context("failed to sync corrected player pose to server")?;
            log::warn!(
                "accepted server player position correction id={} feet=({:.2}, {:.2}, {:.2})",
                accepted.update.teleport_id,
                accepted.feet_position.x,
                accepted.feet_position.y,
                accepted.feet_position.z
            );
            changed = true;
        }
        if changed {
            changed |= self.update_interest_from_engine_camera(camera)?;
        }
        Ok(changed)
    }

    pub(crate) fn update_interest_from_engine_camera(
        &mut self,
        camera: &EngineCameraController,
    ) -> Result<bool> {
        let snapshot = camera.snapshot();
        let center = snapshot.chunk_pos;
        if self.set_interest_center(center)? {
            log::info!(
                "chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
                center.x,
                center.z,
                snapshot.eye.x,
                snapshot.eye.y,
                snapshot.eye.z
            );
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn poll(&mut self) -> Result<bool> {
        self.scene.poll()
    }

    pub(crate) fn poll_with_update_budget(
        &mut self,
        budget: RuntimeUpdatePumpBudget,
    ) -> Result<bool> {
        self.scene.poll_with_update_budget(budget)
    }

    #[cfg(test)]
    fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> bool {
        self.scene.core_mut().apply_server_updates(updates)
    }

    #[cfg(test)]
    pub(crate) fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.scene.sync_render_sections(camera_position)
    }

    pub(crate) fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        self.scene
            .sync_render_sections_with_completed_result_acceptance_timed(
                camera_position,
                completed_result_accept_budget,
            )
    }

    pub(crate) fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        self.scene
            .sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    pub(crate) fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        self.scene
            .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
                camera_position,
                deadline,
                max_compile_requests,
                completed_result_accept_budget,
            )
    }

    pub(crate) fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.scene.sync_all_render_sections(camera_position)
    }

    pub(crate) fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        self.scene.release_render_compile_jobs(count)
    }

    pub(crate) fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.scene.cached_sections()
    }

    /// Sky clear color for the current day-time, driving the day/night gradient.
    pub(crate) fn sky_clear_color(&self) -> wgpu::Color {
        self.scene.sky_clear_color()
    }

    /// Celestial phase in `[0, 1)` for the current day-time (noon = 0).
    pub(crate) fn time_of_day(&self) -> f32 {
        self.scene.time_of_day()
    }

    /// Celestial rig rotation in radians for the current day-time.
    pub(crate) fn sun_angle(&self) -> f32 {
        self.scene.sun_angle()
    }

    /// Force the client day/night clock to a specific `dayTime`. Debug-only hook
    /// for captures; the next server tick will overwrite it with authoritative
    /// time.
    pub(crate) fn force_day_time(&mut self, day_time: u64) {
        self.scene.force_day_time(day_time);
    }

    pub(crate) fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.scene
            .traversal_ready_render_section_keys(camera_position)
    }

    pub(crate) fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.scene.has_pending_render_work(camera_position)
    }

    pub(crate) fn target_render_work_stats(&self, camera_position: Vec3) -> TargetRenderWorkStats {
        self.scene.target_render_work_stats(camera_position)
    }

    #[cfg(test)]
    pub(crate) fn pending_render_chunk_count(&self) -> usize {
        self.scene.pending_render_chunk_count()
    }

    pub(crate) fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.scene.last_poll_diagnostics()
    }

    pub(crate) fn view_readiness_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.scene.view_readiness_overlay()
    }

    pub(crate) fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(state_id) = self.block_state_at_position(position) else {
            return false;
        };
        self.mesh_assets().catalog.occludes(state_id)
    }

    pub(crate) fn camera_inside_water(&self, position: Vec3) -> bool {
        self.scene.camera_inside_water(position)
    }

    pub(crate) fn highest_non_air_block_y_at_world(
        &self,
        world_x: i32,
        world_z: i32,
    ) -> Option<i32> {
        self.scene
            .highest_non_air_block_y_at_world(world_x, world_z)
    }

    fn block_state_at_position(&self, position: Vec3) -> Option<mclone_core::BlockStateId> {
        self.scene.block_state_at_position(position)
    }

    pub(crate) fn stats(&self) -> WindowRuntimeStats {
        self.scene.stats()
    }
}

#[cfg(test)]
fn poll_integrated_server_until_idle(server: &mut IntegratedServer) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            server
                .try_poll()
                .context("failed to poll integrated server worldgen jobs")?,
        );
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for integrated server worldgen jobs");
        }
        if server.pending_publication_count() == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub(crate) fn poll_window_runtime_until_idle(
    runtime: &mut WindowSceneRuntime,
) -> Result<(usize, f64)> {
    runtime.scene.poll_until_idle()
}

pub(crate) fn poll_window_runtime_until_idle_with_timeout(
    runtime: &mut WindowSceneRuntime,
    timeout: Duration,
) -> Result<(usize, f64)> {
    runtime.scene.poll_until_idle_with_timeout(timeout)
}

impl SceneOptions {
    #[cfg(test)]
    fn chunk_positions(&self) -> impl Iterator<Item = (i32, i32)> {
        let min_x = self.chunk_x - self.render_distance;
        let max_x = self.chunk_x + self.render_distance;
        let min_z = self.chunk_z - self.render_distance;
        let max_z = self.chunk_z + self.render_distance;
        (min_x..=max_x)
            .flat_map(move |chunk_x| (min_z..=max_z).map(move |chunk_z| (chunk_x, chunk_z)))
    }
}

#[cfg(test)]
mod tests;
