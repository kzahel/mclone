use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::{
    RuntimeExchange, RuntimePollDiagnostics, RuntimeUpdateApplyReport, SingleViewRuntime,
    SingleViewRuntimeStats, elapsed_ms,
};
#[cfg(test)]
use mclone_app_runtime::{camera_position_inside_water_block, snapshot_block_state_at_world};
pub(crate) use mclone_app_runtime::{chunk_tracking_radius_for_render_distance, square_count};
use mclone_client::{ClientHost, ClientRuntime};
#[cfg(test)]
use mclone_core::AIR_BLOCK_STATE_ID;
use mclone_core::ChunkPos;
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
use mclone_protocol::{ClientCommand, PlayerPositionUpdate, ServerUpdate};
#[cfg(test)]
use mclone_server::{IntegratedServer, ServerRunnerKind};
use mclone_server::{
    IntegratedServerRunner, NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    ServerRunnerDiagnostics,
};

use crate::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use crate::cli::SceneOptions;
use crate::remote_session::RemoteServerSession;
use crate::render_cache::{
    RenderSectionCompileWorker, SceneTexturedSections, TexturedMeshAssets,
    load_textured_mesh_assets,
};
use mclone_render_session::{RenderSectionCacheUpdate, build_client_textured_sections};
#[cfg(test)]
use mclone_render_session::{RenderSectionSession, render_section_neighbor_readiness};

const DEFAULT_RENDER_CHUNK_MESH_BUDGET: usize = 1;
pub(crate) type WindowRuntimeStats = SingleViewRuntimeStats;

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
    let mut client = if scene.remote_addr.is_some() {
        ClientRuntime::new(ClientHost::RemoteDedicated)
    } else {
        ClientRuntime::local_integrated()
    };
    let render_distance = scene_render_distance(scene)?;

    let command = client.set_chunk_view(mclone_app_runtime::chunk_view(
        ChunkPos::new(scene.chunk_x, scene.chunk_z),
        render_distance,
        chunk_tracking_radius_for_render_distance(render_distance),
    ));

    if let Some(remote_addr) = &scene.remote_addr {
        let mut session = RemoteServerSession::connect(remote_addr.as_str())?;
        let updates = session.send_command(command)?;
        client.apply_updates(updates);
    } else {
        let mut runner = NativeIntegratedServerRunner::new(native_runner_config(scene))?;
        runner.send_command(command)?;
        client.apply_updates(drain_integrated_server_runner_until_idle(&mut runner)?);
        runner.join_shutdown()?;
    }
    Ok(client)
}

fn native_runner_config(scene: &SceneOptions) -> NativeIntegratedServerRunnerConfig {
    NativeIntegratedServerRunnerConfig::new(scene.seed)
        .with_lighting_enabled(scene.lighting_enabled)
        .with_day_time(scene.day_time_override)
        .with_day_time_frozen(scene.freeze_time)
}

#[derive(Debug)]
pub(crate) struct WindowSceneRuntime {
    core: SingleViewRuntime,
    server_runner: Option<NativeIntegratedServerRunner>,
    remote_session: Option<RemoteServerSession>,
    pub(crate) mesh_assets: TexturedMeshAssets,
    pub(crate) actor_textures: ActorTextureAssets,
    render_compile_worker: RenderSectionCompileWorker,
}

impl WindowSceneRuntime {
    pub(crate) fn new(scene: &SceneOptions) -> Result<Self> {
        let client = if scene.remote_addr.is_some() {
            ClientRuntime::new(ClientHost::RemoteDedicated)
        } else {
            ClientRuntime::local_integrated()
        };
        let render_distance = scene_render_distance(scene)?;
        let chunk_tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
        let mesh_assets = load_textured_mesh_assets()?;
        let actor_textures = load_actor_texture_assets()?;
        let render_compile_worker = RenderSectionCompileWorker::new(mesh_assets.catalog.clone())?;
        let remote_session = scene
            .remote_addr
            .as_deref()
            .map(RemoteServerSession::connect)
            .transpose()?;
        let server_runner = if remote_session.is_none() {
            Some(NativeIntegratedServerRunner::new(native_runner_config(
                scene,
            ))?)
        } else {
            None
        };
        let mut runtime = Self {
            core: SingleViewRuntime::new(
                client,
                ChunkPos::new(scene.chunk_x, scene.chunk_z),
                render_distance,
                chunk_tracking_radius,
            ),
            server_runner,
            remote_session,
            mesh_assets,
            actor_textures,
            render_compile_worker,
        };
        if let Some(day_time) = runtime
            .server_runner
            .as_ref()
            .map(|runner| {
                runner
                    .poll_diagnostics()
                    .map(|diagnostics| diagnostics.day_time)
            })
            .transpose()?
        {
            runtime.force_day_time(day_time);
        }
        runtime.set_chunk_view(
            runtime.interest_center(),
            runtime.render_distance(),
            runtime.chunk_tracking_radius(),
        )?;
        // Debug day/night controls are applied before the runner starts ticking;
        // seed the client so the first frame is correct before server updates.
        if let Some(day_time) = scene.day_time_override {
            runtime.force_day_time(day_time);
        }
        Ok(runtime)
    }

    pub(crate) fn client(&self) -> &ClientRuntime {
        self.core.client()
    }

    #[cfg(test)]
    fn render_session(&self) -> &RenderSectionSession {
        self.core.render_session()
    }

    #[cfg(test)]
    fn render_session_mut(&mut self) -> &mut RenderSectionSession {
        self.core.render_session_mut()
    }

    pub(crate) fn render_distance(&self) -> u32 {
        self.core.render_distance()
    }

    pub(crate) fn chunk_tracking_radius(&self) -> u32 {
        self.core.chunk_tracking_radius()
    }

    pub(crate) fn interest_center(&self) -> ChunkPos {
        self.core.interest_center()
    }

    pub(crate) fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        self.set_chunk_view(center, self.render_distance(), self.chunk_tracking_radius())
    }

    pub(crate) fn set_render_distance(&mut self, render_distance: u32) -> Result<bool> {
        self.set_chunk_view(
            self.interest_center(),
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
    }

    fn set_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<bool> {
        let Some(command) =
            self.core
                .set_chunk_view_command(center, render_distance, chunk_tracking_radius)
        else {
            return Ok(false);
        };
        self.dispatch_client_command(command)
    }

    pub(crate) fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        self.dispatch_client_command(command)
    }

    pub(crate) fn drain_player_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.core.drain_player_position_updates()
    }

    fn dispatch_client_command(&mut self, command: ClientCommand) -> Result<bool> {
        if self.remote_session.is_some() {
            return self.dispatch_remote_client_command(command);
        }

        let Some(runner) = &mut self.server_runner else {
            return Ok(false);
        };
        runner
            .send_command(command)
            .context("failed to send command to integrated server runner")?;
        let _ = self.drain_local_runner_updates_report()?;
        Ok(true)
    }

    fn dispatch_remote_client_command(&mut self, command: ClientCommand) -> Result<bool> {
        let remote_session = self
            .remote_session
            .as_mut()
            .expect("remote dispatch requires a remote session");
        match remote_session.send_command(command) {
            Ok(updates) => {
                let changed = !updates.is_empty();
                self.core.apply_exchange(RuntimeExchange::command(updates));
                Ok(changed)
            }
            Err(err) => self.reconnect_remote_session_and_resync(err),
        }
    }

    fn reconnect_remote_session_and_resync(&mut self, err: anyhow::Error) -> Result<bool> {
        let Some(view) = self.client().chunk_view().cloned() else {
            bail!("remote server command failed before a chunk view was established: {err:#}");
        };

        log::warn!("remote server command failed; reconnecting and resyncing chunk view: {err:#}");
        self.clear_remote_replica_for_resync();
        let remote_session = self
            .remote_session
            .as_mut()
            .expect("remote resync requires a remote session");
        remote_session.reconnect()?;
        let updates = remote_session
            .send_command(ClientCommand::SetChunkView(view))
            .context("failed to resync chunk view after reconnect")?;
        self.apply_server_updates(updates);
        Ok(true)
    }

    fn clear_remote_replica_for_resync(&mut self) {
        self.core.clear_client_replica_and_mark_render_dirty();
    }

    pub(crate) fn poll(&mut self) -> Result<bool> {
        let flush_start = Instant::now();
        let apply_report = self.drain_local_runner_updates_report()?;
        let changed = apply_report.changed;
        let runner_diagnostics = self
            .server_runner
            .as_ref()
            .map(|runner| runner.poll_diagnostics())
            .transpose()
            .context("failed to poll integrated server runner diagnostics")?;
        self.core.finish_poll_diagnostics(
            elapsed_ms(flush_start.elapsed()),
            apply_report,
            runner_diagnostics.as_ref(),
        );
        Ok(changed)
    }

    fn drain_local_runner_updates_report(&mut self) -> Result<RuntimeUpdateApplyReport> {
        let Some(runner) = &mut self.server_runner else {
            return Ok(RuntimeUpdateApplyReport::default());
        };
        let updates = runner
            .drain_updates()
            .context("failed to drain integrated server runner updates")?;
        if updates.is_empty() {
            return Ok(RuntimeUpdateApplyReport::default());
        }
        Ok(self.core.apply_server_updates_report(updates))
    }

    fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> bool {
        self.core.apply_server_updates(updates)
    }

    pub(crate) fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_with_budget(camera_position, DEFAULT_RENDER_CHUNK_MESH_BUDGET)
    }

    pub(crate) fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut combined = RenderSectionCacheUpdate::default();
        loop {
            let update = self.sync_render_sections_with_budget(camera_position, usize::MAX)?;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || update.submitted_compile_section_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            combined.merge(update);
            if self.render_compile_worker.pending_job_count() == 0
                && !self.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(combined);
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for render section compile queue");
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn sync_render_sections_with_budget(
        &mut self,
        camera_position: Vec3,
        chunk_budget: usize,
    ) -> Result<RenderSectionCacheUpdate> {
        // 067 Stage 3: desktop drives the shared streaming loop over its OS-thread
        // `mpsc` worker. Distance ordering, near-camera readiness, and immediate removal
        // are the shared engine/web policy; the only desktop specifics are the worker
        // (moved by reference) and the by-clone snapshot collector for the request.
        let render_compile_worker = &mut self.render_compile_worker;
        self.core.sync_render_sections_with_budget(
            render_compile_worker,
            camera_position,
            chunk_budget,
            // Desktop ignores the compiler argument: the owned snapshot `Vec` is moved over
            // `mpsc` to the worker thread, so cloning every loaded column is load-bearing here.
            |client, _compiler| client.chunk_snapshots().cloned().collect(),
        )
    }

    pub(crate) fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.core.cached_sections()
    }

    /// Sky clear color for the current day-time, driving the day/night gradient.
    pub(crate) fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.client().time_of_day())
    }

    /// Celestial phase in `[0, 1)` for the current day-time (noon = 0).
    pub(crate) fn time_of_day(&self) -> f32 {
        self.core.time_of_day()
    }

    /// Celestial rig rotation in radians for the current day-time.
    pub(crate) fn sun_angle(&self) -> f32 {
        self.core.sun_angle()
    }

    /// Force the client day/night clock to a specific `dayTime`. Debug-only hook
    /// for captures; the next server tick will overwrite it with authoritative
    /// time.
    pub(crate) fn force_day_time(&mut self, day_time: u64) {
        self.core.force_day_time(day_time);
    }

    pub(crate) fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.core
            .traversal_ready_render_section_keys(camera_position)
    }

    pub(crate) fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.core.has_pending_render_work(
            self.render_compile_worker.pending_job_count(),
            camera_position,
        )
    }

    #[cfg(test)]
    pub(crate) fn pending_render_chunk_count(&self) -> usize {
        self.core.pending_render_chunk_count()
    }

    fn has_ready_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.core.has_ready_pending_render_work(camera_position)
    }

    fn server_runner_diagnostics(&self) -> Option<ServerRunnerDiagnostics> {
        self.server_runner
            .as_ref()
            .and_then(|runner| runner.poll_diagnostics().ok())
    }

    fn local_server_idle(&self) -> bool {
        self.server_runner_diagnostics().is_none_or(|diagnostics| {
            diagnostics.command_queue_depth == 0
                && diagnostics.update_queue_depth == 0
                && !diagnostics.awaiting_tick
                && diagnostics.pending_jobs == 0
                && diagnostics.pending_publications == 0
        })
    }

    pub(crate) fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.core.last_poll_diagnostics()
    }

    pub(crate) fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(state_id) = self.block_state_at_position(position) else {
            return false;
        };
        self.mesh_assets.catalog.occludes(state_id)
    }

    pub(crate) fn camera_inside_water(&self, position: Vec3) -> bool {
        self.core.camera_inside_water(position)
    }

    pub(crate) fn highest_non_air_block_y_at_world(
        &self,
        world_x: i32,
        world_z: i32,
    ) -> Option<i32> {
        self.core.highest_non_air_block_y_at_world(world_x, world_z)
    }

    fn block_state_at_position(&self, position: Vec3) -> Option<mclone_core::BlockStateId> {
        self.core.block_state_at_position(position)
    }

    pub(crate) fn stats(&self) -> WindowRuntimeStats {
        let runner_diagnostics = self.server_runner_diagnostics();
        self.core.stats(
            runner_diagnostics.as_ref(),
            self.render_compile_worker.pending_job_count(),
        )
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

fn drain_integrated_server_runner_until_idle(
    runner: &mut NativeIntegratedServerRunner,
) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            runner
                .drain_updates()
                .context("failed to drain integrated server runner updates")?,
        );
        let diagnostics = runner
            .poll_diagnostics()
            .context("failed to poll integrated server runner diagnostics")?;
        if diagnostics.command_queue_depth == 0
            && diagnostics.update_queue_depth == 0
            && !diagnostics.awaiting_tick
            && diagnostics.pending_jobs == 0
            && diagnostics.pending_publications == 0
        {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for integrated server runner jobs");
        }
        if diagnostics.update_queue_depth == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub(crate) fn poll_window_runtime_until_idle(
    runtime: &mut WindowSceneRuntime,
) -> Result<(usize, f64)> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut polls = 0_usize;
    let mut poll_ms = 0.0_f64;
    loop {
        let poll_start = Instant::now();
        runtime.poll()?;
        poll_ms += elapsed_ms(poll_start.elapsed());
        polls += 1;
        if runtime.local_server_idle() {
            return Ok((polls, poll_ms));
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for window runtime worldgen jobs");
        }
        if runtime
            .server_runner_diagnostics()
            .is_none_or(|diagnostics| diagnostics.update_queue_depth == 0)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
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
mod tests {
    use super::*;
    use crate::DEFAULT_SEED;
    use crate::camera::SpectatorCamera;
    use crate::render_cache::extracted_asset_root;
    use mclone_core::{
        BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkSnapshot, ChunkStatus,
    };
    use mclone_protocol::{ChunkView, SectionBlockUpdate};
    use mclone_render::chunk::{
        ChunkCamera, TexturedSectionRenderOptions,
        textured_section_visibility_stats_with_options_and_ready_sections,
    };
    use mclone_render_session::{
        RenderSectionNeighborReadiness, render_dirty_section_keys_for_block_update,
        render_section_center, render_section_keys_for_snapshot, sort_chunk_positions_by_distance,
    };

    #[test]
    fn camera_water_detection_uses_fluid_height_boundary() {
        let water = mclone_client::block_facts::WATER_BLOCK_STATE_ID;
        let lava = mclone_client::block_facts::LAVA_BLOCK_STATE_ID;
        let air = AIR_BLOCK_STATE_ID;

        assert!(camera_position_inside_water_block(62.999, 62, water));
        assert!(!camera_position_inside_water_block(63.0, 62, water));
        assert!(!camera_position_inside_water_block(62.5, 62, lava));
        assert!(!camera_position_inside_water_block(62.5, 62, air));
        assert!(!camera_position_inside_water_block(f32::NAN, 62, water));
    }

    #[test]
    fn snapshot_block_state_lookup_reads_loaded_sections_and_omitted_air() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        block_state_ids[mclone_core::chunk_section_index(1, 15, 15)] = BlockStateId(42);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(1, -1),
            ChunkStatus::Full,
            ChunkRevision(1),
            -16,
            32,
            &block_state_ids,
        );

        assert_eq!(
            snapshot_block_state_at_world(&snapshot, 17, -1, -1),
            Some(BlockStateId(42))
        );
        assert_eq!(
            snapshot_block_state_at_world(&snapshot, 17, 0, -1),
            Some(AIR_BLOCK_STATE_ID)
        );
        assert_eq!(snapshot_block_state_at_world(&snapshot, 0, -1, -1), None);
        assert_eq!(snapshot_block_state_at_world(&snapshot, 17, 16, -1), None);
    }

    #[test]
    fn render_section_readiness_uses_near_exception_and_horizontal_neighbors() {
        let target = ChunkPos::new(2, -3);
        let key = RenderSectionKey::new(target.x, 0, target.z);
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(target)));

        assert_eq!(
            render_section_neighbor_readiness(&client, key, render_section_center(key)),
            RenderSectionNeighborReadiness::ReadyNearCamera
        );

        let high_camera = render_section_center(key) + Vec3::new(0.0, 256.0, 0.0);
        assert_eq!(
            render_section_neighbor_readiness(&client, key, high_camera),
            RenderSectionNeighborReadiness::ReadyNearCamera
        );

        let diagonal_neighbor_key = RenderSectionKey::new(target.x + 1, 0, target.z + 1);
        assert_eq!(
            render_section_neighbor_readiness(&client, diagonal_neighbor_key, high_camera),
            RenderSectionNeighborReadiness::ReadyNearCamera
        );

        let far_camera = render_section_center(key) + Vec3::new(128.0, 0.0, 0.0);
        assert_eq!(
            render_section_neighbor_readiness(&client, key, far_camera),
            RenderSectionNeighborReadiness::DeferredMissingNeighbors
        );

        for neighbor in [
            ChunkPos::new(target.x - 1, target.z),
            ChunkPos::new(target.x + 1, target.z),
            ChunkPos::new(target.x, target.z - 1),
            ChunkPos::new(target.x, target.z + 1),
        ] {
            client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(neighbor)));
        }

        assert_eq!(
            render_section_neighbor_readiness(&client, key, far_camera),
            RenderSectionNeighborReadiness::ReadyWithNeighbors
        );
    }

    #[test]
    fn render_distance_derives_java_shaped_tracking_radius() {
        assert_eq!(chunk_tracking_radius_for_render_distance(0), 0);
        assert_eq!(chunk_tracking_radius_for_render_distance(1), 1);
        assert_eq!(chunk_tracking_radius_for_render_distance(2), 3);
        assert_eq!(chunk_tracking_radius_for_render_distance(3), 3);
        assert_eq!(chunk_tracking_radius_for_render_distance(4), 4);
    }

    #[test]
    fn window_runtime_local_integrated_defaults_to_native_thread_runner() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let runtime = WindowSceneRuntime::new(&scene).unwrap();
        let stats = runtime.stats();

        assert_eq!(
            stats.server_runner_kind,
            Some(ServerRunnerKind::NativeThread)
        );
        assert_ne!(
            stats.server_runner_kind,
            Some(ServerRunnerKind::InlineFallback)
        );
    }

    #[test]
    fn render_section_keys_cover_snapshot_height() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(-1, 4),
            ChunkStatus::Full,
            ChunkRevision(1),
            -16,
            32,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2],
        );

        assert_eq!(
            render_section_keys_for_snapshot(&snapshot),
            vec![
                RenderSectionKey::new(-1, -1, 4),
                RenderSectionKey::new(-1, 0, 4)
            ]
        );
    }

    #[test]
    fn block_delta_dirty_sections_stay_local_for_interior_blocks() {
        let keys = render_dirty_section_keys_for_block_update(
            ChunkPos::new(2, -3),
            5,
            &SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: BlockStateId(42),
            },
        );

        assert_eq!(keys, BTreeSet::from([RenderSectionKey::new(2, 5, -3)]));
    }

    #[test]
    fn block_delta_dirty_sections_cross_section_boundaries() {
        let keys = render_dirty_section_keys_for_block_update(
            ChunkPos::new(0, 0),
            0,
            &SectionBlockUpdate {
                local_x: 0,
                local_y: 0,
                local_z: 15,
                block_state: BlockStateId(42),
            },
        );

        assert_eq!(
            keys,
            BTreeSet::from([
                RenderSectionKey::new(-1, -1, 0),
                RenderSectionKey::new(-1, -1, 1),
                RenderSectionKey::new(-1, 0, 0),
                RenderSectionKey::new(-1, 0, 1),
                RenderSectionKey::new(0, -1, 0),
                RenderSectionKey::new(0, -1, 1),
                RenderSectionKey::new(0, 0, 0),
                RenderSectionKey::new(0, 0, 1),
            ])
        );
    }

    #[test]
    fn window_runtime_streams_chunks_when_spectator_crosses_boundary() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let initial_center = ChunkPos::new(0, 0);
        let initial_stats = runtime.stats();
        assert_eq!(initial_stats.interest_center, initial_center);
        assert_eq!(
            initial_stats.server_runner_kind,
            Some(ServerRunnerKind::NativeThread)
        );
        assert_eq!(initial_stats.loaded_chunks, 1);
        assert_eq!(initial_stats.pending_jobs, 0);
        assert!(runtime.client().chunk_snapshot(initial_center).is_some());
        assert!(
            runtime
                .highest_non_air_block_y_at_world(8, 8)
                .is_some_and(|y| (-64..320).contains(&y))
        );

        let mut spectator = SpectatorCamera::spawn_for_scene(&scene);
        let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
        assert_eq!(initial_submit.rebuilt_section_count(), 0);
        assert!(initial_submit.submitted_compile_section_count > 0);
        assert!(runtime.stats().pending_render_compile_jobs > 0);

        let initial_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(initial_update.rebuilt_section_count() > 0);
        assert_eq!(initial_update.removed_section_count(), 0);
        let initial_sections = runtime.cached_sections();
        assert!(!initial_sections.is_empty());
        assert!(section_index_count(&initial_sections) > 0);
        assert!(
            initial_sections
                .iter()
                .all(|section| section.key.chunk_x == 0 && section.key.chunk_z == 0)
        );

        spectator.position.x = 16.25;
        let next_center = spectator.chunk_pos();
        assert_eq!(next_center, ChunkPos::new(1, 0));
        assert!(runtime.set_interest_center(next_center).unwrap());
        assert_eq!(runtime.stats().interest_center, next_center);

        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let moved_stats = runtime.stats();
        assert_eq!(moved_stats.interest_center, next_center);
        assert_eq!(moved_stats.loaded_chunks, 1);
        assert_eq!(moved_stats.pending_jobs, 0);
        assert!(runtime.client().chunk_snapshot(initial_center).is_none());
        assert!(runtime.client().chunk_snapshot(next_center).is_some());

        let moved_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(moved_update.rebuilt_section_count() > 0);
        assert!(moved_update.removed_section_count() > 0);
        let moved_sections = runtime.cached_sections();
        assert!(!moved_sections.is_empty());
        assert!(section_index_count(&moved_sections) > 0);
        assert!(
            moved_sections
                .iter()
                .all(|section| section.key.chunk_x == 1 && section.key.chunk_z == 0)
        );
    }

    #[test]
    fn window_runtime_defers_far_boundary_sections_without_neighbors() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let far_camera = spectator.position + Vec3::new(128.0, 0.0, 0.0);
        let update = runtime.sync_all_render_sections(far_camera).unwrap();

        assert_eq!(update.rebuilt_section_count(), 0);
        assert_eq!(update.near_exception_section_count, 0);
        assert!(update.deferred_section_count > 0);
        assert!(runtime.pending_render_chunk_count() > 0);
        assert!(!runtime.has_pending_render_work(far_camera));
        assert!(runtime.cached_sections().is_empty());

        let deferred_key = *runtime
            .render_session()
            .dirty()
            .dirty_sections
            .iter()
            .next()
            .expect("deferred dirty section should be retained");
        let ready_position = render_section_center(deferred_key);
        assert!(runtime.has_pending_render_work(ready_position));
        let ready_update = runtime.sync_all_render_sections(ready_position).unwrap();
        assert!(ready_update.rebuilt_section_count() > 0);
        assert!(
            !runtime
                .render_session()
                .dirty()
                .dirty_sections
                .contains(&deferred_key)
        );
    }

    #[test]
    fn window_runtime_marks_section_block_updates_without_chunk_dirtying() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = WindowSceneRuntime::new(&SceneOptions::default()).unwrap();
        runtime
            .render_session_mut()
            .dirty_mut()
            .dirty_chunks
            .clear();
        runtime
            .render_session_mut()
            .dirty_mut()
            .dirty_sections
            .clear();

        runtime.apply_server_updates(vec![ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 7,
            updates: vec![SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: AIR_BLOCK_STATE_ID,
            }],
        }]);

        assert!(runtime.render_session().dirty().dirty_chunks.is_empty());
        assert_eq!(
            runtime.render_session().dirty().dirty_sections,
            BTreeSet::from([RenderSectionKey::new(0, 7, 0)])
        );
    }

    #[test]
    fn render_compile_scheduling_orders_dirty_chunks_by_camera_distance() {
        let camera = Vec3::new(8.0, 88.0, 8.0);
        let sorted = sort_chunk_positions_by_distance(
            [
                ChunkPos::new(4, 0),
                ChunkPos::new(0, 0),
                ChunkPos::new(-2, 0),
            ],
            camera,
        );

        assert_eq!(
            sorted,
            vec![
                ChunkPos::new(0, 0),
                ChunkPos::new(-2, 0),
                ChunkPos::new(4, 0)
            ]
        );
    }

    #[test]
    fn render_compile_revisions_stale_only_changed_sections() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
        assert!(initial_submit.submitted_compile_section_count > 1);
        let changed_key = RenderSectionKey::new(0, 5, 0);
        assert!(
            runtime
                .render_session()
                .dirty()
                .inflight_sections
                .contains(&changed_key)
        );

        runtime.apply_server_updates(vec![ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 5,
            updates: vec![SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: AIR_BLOCK_STATE_ID,
            }],
        }]);

        let completed = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();

        assert_eq!(completed.stale_compile_section_count, 1);
        assert!(
            completed.completed_compile_section_count
                >= initial_submit.submitted_compile_section_count
        );
        assert!(!runtime.has_pending_render_work(spectator.position));
    }

    #[test]
    fn window_runtime_updates_render_distance_live() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.render_distance(), 0);
        assert_eq!(runtime.chunk_tracking_radius(), 0);
        assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

        assert!(runtime.set_render_distance(1).unwrap());
        assert_eq!(runtime.render_distance(), 1);
        assert_eq!(runtime.chunk_tracking_radius(), 1);
        assert_eq!(runtime.client().chunk_view().unwrap().render_distance, 1);
        assert_eq!(
            runtime.client().chunk_view().unwrap().chunk_tracking_radius,
            1
        );
        poll_window_runtime_until_idle(&mut runtime).unwrap();
        assert_eq!(runtime.stats().loaded_chunks, square_count(1).unwrap());

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let grown_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(grown_update.rebuilt_section_count() > 0);
        assert_eq!(grown_update.removed_section_count(), 0);
        assert!(runtime.cached_sections().iter().any(|section| {
            section.key.chunk_x != scene.chunk_x || section.key.chunk_z != scene.chunk_z
        }));

        assert!(runtime.set_render_distance(0).unwrap());
        poll_window_runtime_until_idle(&mut runtime).unwrap();
        assert_eq!(runtime.render_distance(), 0);
        assert_eq!(runtime.chunk_tracking_radius(), 0);
        assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

        let shrunk_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(shrunk_update.removed_section_count() > 0);
        assert!(runtime.cached_sections().iter().all(|section| {
            section.key.chunk_x == scene.chunk_x && section.key.chunk_z == scene.chunk_z
        }));
    }

    #[test]
    fn window_runtime_mesh_queue_processes_ready_work_by_chunk_budget() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 1,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.stats().loaded_chunks, 9);
        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let first_update = runtime.sync_render_sections(spectator.position).unwrap();
        assert_eq!(first_update.rebuilt_section_count(), 0);
        assert!(first_update.submitted_compile_section_count > 0);
        assert!(runtime.stats().pending_render_compile_jobs > 0);
        assert!(runtime.pending_render_chunk_count() > 0);

        let first_sections = runtime.cached_sections();
        assert!(first_sections.is_empty());

        let remaining_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(remaining_update.rebuilt_section_count() > 0);
        assert!(!runtime.has_pending_render_work(spectator.position));
        assert!(!runtime.cached_sections().is_empty());
    }

    #[test]
    fn render_distance_two_tracks_extra_ring_without_drawing_it() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 2,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.render_distance(), 2);
        assert_eq!(runtime.chunk_tracking_radius(), 3);
        assert_eq!(runtime.stats().loaded_chunks, square_count(3).unwrap());
        assert!(runtime.client().chunk_snapshots().any(|snapshot| {
            chunk_distance_from_scene_center(&scene, snapshot.pos.x, snapshot.pos.z) == 3
        }));

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        let ready_sections = runtime.traversal_ready_render_section_keys(spectator.position);
        assert!(!ready_sections.is_empty());
        assert!(ready_sections.iter().all(|key| {
            chunk_distance_from_scene_center(&scene, key.chunk_x, key.chunk_z) <= 2
        }));
    }

    #[test]
    fn high_altitude_radius_one_keeps_neighbor_chunks_draw_ready() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 1,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        let sections = runtime.cached_sections();
        assert!(sections.iter().any(|section| {
            section.key.chunk_x != scene.chunk_x || section.key.chunk_z != scene.chunk_z
        }));

        let high_position = spectator.position + Vec3::new(0.0, 256.0, 0.0);
        let ready_sections = runtime.traversal_ready_render_section_keys(high_position);
        assert!(
            ready_sections
                .iter()
                .any(|key| { key.chunk_x != scene.chunk_x || key.chunk_z != scene.chunk_z })
        );

        let center_drawable_sections = sections
            .iter()
            .filter(|section| {
                section.key.chunk_x == scene.chunk_x
                    && section.key.chunk_z == scene.chunk_z
                    && !section.is_empty()
            })
            .count();
        let camera = ChunkCamera {
            eye: high_position.to_array(),
            target: (high_position + Vec3::NEG_Y).to_array(),
            up: [0.0, 0.0, 1.0],
            fov_y_radians: 100.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        };
        let stats = textured_section_visibility_stats_with_options_and_ready_sections(
            &sections,
            camera.render_view(960, 640),
            TexturedSectionRenderOptions {
                section_occlusion_culling: false,
                ..TexturedSectionRenderOptions::default()
            },
            Some(&ready_sections),
        );

        assert_eq!(stats.readiness_culled_section_count, 0);
        assert!(stats.drawn_section_count > center_drawable_sections);
    }

    #[test]
    fn scene_chunk_positions_cover_square_radius() {
        let positions = SceneOptions {
            seed: 0,
            chunk_x: -2,
            chunk_z: 3,
            render_distance: 1,
            remote_addr: None,
            day_time_override: None,
            freeze_time: false,
            lighting_enabled: true,
        }
        .chunk_positions()
        .collect::<Vec<_>>();

        assert_eq!(positions.len(), 9);
        assert!(positions.contains(&(-3, 2)));
        assert!(positions.contains(&(-2, 3)));
        assert!(positions.contains(&(-1, 4)));
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_integrated_server() {
        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let client = build_scene_client_runtime(&scene).unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_remote_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut stream).unwrap();
            let command = mclone_net::read_client_command_frame(&mut stream).unwrap();
            let mut server = IntegratedServer::new(DEFAULT_SEED);
            let mut updates = server.try_handle_command(command).unwrap();
            updates.extend(poll_integrated_server_until_idle(&mut server).unwrap());
            mclone_net::write_server_update_batch(&mut stream, &updates).unwrap();
        });
        let scene = SceneOptions {
            render_distance: 0,
            remote_addr: Some(addr.to_string()),
            ..SceneOptions::default()
        };

        let client = build_scene_client_runtime(&scene).unwrap();
        server.join().unwrap();

        assert_eq!(client.host(), ClientHost::RemoteDedicated);
        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn window_runtime_reuses_remote_session_for_interest_updates() {
        if !extracted_asset_root().exists() {
            return;
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut stream,
                &[ServerUpdate::TimeUpdate { day_time: 1 }],
            )
            .unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut stream,
                &[ServerUpdate::TimeUpdate { day_time: 2 }],
            )
            .unwrap();
            assert!(
                mclone_net::try_read_client_command_frame(&mut stream)
                    .unwrap()
                    .is_none()
            );
        });

        {
            let scene = SceneOptions {
                render_distance: 0,
                remote_addr: Some(addr.to_string()),
                ..SceneOptions::default()
            };
            let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
            assert!(runtime.set_interest_center(ChunkPos::new(1, 0)).unwrap());
        }
        server.join().unwrap();
    }

    #[test]
    fn window_runtime_reconnects_remote_session_and_resyncs_chunk_cache() {
        if !extracted_asset_root().exists() {
            return;
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let initial_center = ChunkPos::new(0, 0);
        let moved_center = ChunkPos::new(1, 0);
        let server = std::thread::spawn(move || {
            let (mut first_stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut first_stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: initial_center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut first_stream,
                &[ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                    initial_center,
                ))],
            )
            .unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: moved_center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            drop(first_stream);

            let (mut second_stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut second_stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut second_stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: moved_center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut second_stream,
                &[ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                    moved_center,
                ))],
            )
            .unwrap();
        });

        {
            let scene = SceneOptions {
                render_distance: 0,
                remote_addr: Some(addr.to_string()),
                ..SceneOptions::default()
            };
            let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
            assert!(runtime.client().chunk_snapshot(initial_center).is_some());
            runtime
                .render_session_mut()
                .dirty_mut()
                .dirty_chunks
                .clear();
            runtime
                .render_session_mut()
                .dirty_mut()
                .dirty_sections
                .clear();

            assert!(runtime.set_interest_center(moved_center).unwrap());

            assert_eq!(runtime.client().loaded_chunk_count(), 1);
            assert!(runtime.client().chunk_snapshot(initial_center).is_none());
            assert!(runtime.client().chunk_snapshot(moved_center).is_some());
            assert!(
                runtime
                    .render_session()
                    .dirty()
                    .dirty_chunks
                    .contains(&initial_center)
            );
            assert!(
                runtime
                    .render_session()
                    .dirty()
                    .dirty_chunks
                    .contains(&moved_center)
            );
        }
        server.join().unwrap();
    }

    #[test]
    fn build_scene_textured_sections_uses_client_runtime_snapshots() {
        if !extracted_asset_root().exists() {
            return;
        }
        let scene_mesh = build_scene_textured_sections(&SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        })
        .unwrap();

        assert!(scene_mesh.section_count() > 0);
        assert!(scene_mesh.index_count() > 0);
        assert!(scene_mesh.atlas.width > 0);
        assert!(scene_mesh.atlas.height > 0);
    }

    fn chunk_distance_from_scene_center(scene: &SceneOptions, chunk_x: i32, chunk_z: i32) -> i32 {
        (chunk_x - scene.chunk_x)
            .abs()
            .max((chunk_z - scene.chunk_z).abs())
    }

    fn empty_test_snapshot(pos: ChunkPos) -> ChunkSnapshot {
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
    }

    fn poll_window_runtime_until_idle(runtime: &mut WindowSceneRuntime) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            runtime.poll()?;
            if runtime.local_server_idle() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for window runtime worldgen jobs");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn section_index_count(sections: &[TexturedRenderSectionMesh]) -> u32 {
        sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}
