use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::ClientRuntime;
use mclone_core::{BlockStateId, ChunkPos};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
use mclone_protocol::{ClientCommand, ServerUpdate};
use mclone_render::far_lod::FarTerrainLodMesh;
use mclone_render_session::RenderSectionCacheUpdate;
use mclone_server::{
    IntegratedServerRunner, NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    ServerRunnerDiagnostics, SimulationCadenceConfig, host_tick_interval_for_rate_hz,
    initial_spawn_center_for_seed,
};
use mclone_ui::LoadingProgressOverlay;

use crate::far_lod::{FarTerrainLodCache, FarTerrainLodConfig};
use crate::host_mode::{
    RemoteDedicatedServerSession, SingleViewHostMode, SingleViewHostOptions,
    dispatch_remote_dedicated_command,
};
use crate::render_assets::{
    DEFAULT_RENDER_SECTION_COMPILE_WORKERS, RenderSectionCompileWorker, TexturedMeshAssets,
    load_textured_mesh_assets,
};
use crate::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, RemoteSessionEndpoint,
    SessionFailure, SessionStartRequest, SessionStartResult, SessionStatus, StartedGameSession,
};
use crate::{
    DEFAULT_RENDER_CHUNK_MESH_BUDGET, RuntimePollDiagnostics, RuntimePollTiming,
    RuntimeUpdateApplyReport, SingleViewRuntime, SingleViewRuntimeStats,
    TimedRenderSectionCacheUpdate, chunk_tracking_radius_for_render_distance, elapsed_ms,
    loading_progress_overlay_from_diagnostics, view_readiness_overlay_from_diagnostics,
};

const RUNTIME_DIAGNOSTICS_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSingleViewSceneOptions {
    pub seed: i64,
    pub center: ChunkPos,
    pub render_distance: u32,
    pub cadence: SimulationCadenceConfig,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub debug_passive_showcase: bool,
    pub lighting_enabled: bool,
    pub render_compile_worker_count: usize,
}

impl LocalSingleViewSceneOptions {
    pub const fn new(seed: i64, center: ChunkPos, render_distance: u32) -> Self {
        Self {
            seed,
            center,
            render_distance,
            cadence: SimulationCadenceConfig::new(20, 20, 60),
            day_time_override: None,
            freeze_time: false,
            debug_passive_showcase: true,
            lighting_enabled: true,
            render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
        }
    }

    pub const fn with_cadence(mut self, cadence: SimulationCadenceConfig) -> Self {
        self.cadence = cadence;
        self
    }

    pub const fn with_day_time(mut self, day_time: Option<u64>) -> Self {
        self.day_time_override = day_time;
        self
    }

    pub const fn with_freeze_time(mut self, freeze_time: bool) -> Self {
        self.freeze_time = freeze_time;
        self
    }

    pub const fn with_debug_passive_showcase(mut self, enabled: bool) -> Self {
        self.debug_passive_showcase = enabled;
        self
    }

    pub fn with_initial_spawn_center(mut self) -> Self {
        self.center = initial_spawn_center_for_seed(self.seed);
        self
    }

    pub const fn with_lighting_enabled(mut self, lighting_enabled: bool) -> Self {
        self.lighting_enabled = lighting_enabled;
        self
    }

    pub const fn with_render_compile_worker_count(
        mut self,
        render_compile_worker_count: usize,
    ) -> Self {
        self.render_compile_worker_count = render_compile_worker_count;
        self
    }

    pub fn chunk_tracking_radius(&self) -> u32 {
        chunk_tracking_radius_for_render_distance(self.render_distance)
    }
}

#[derive(Debug)]
pub struct LocalSingleViewSceneRuntime {
    core: SingleViewRuntime,
    server_runner: NativeIntegratedServerRunner,
    mesh_assets: TexturedMeshAssets,
    render_compile_worker: RenderSectionCompileWorker,
    far_lod_cache: FarTerrainLodCache,
    simulation_cadence: SimulationCadenceConfig,
    last_runner_diagnostics: Option<ServerRunnerDiagnostics>,
    last_runner_diagnostics_poll_at: Option<Instant>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalSingleViewStartupStep {
    pub poll_count: usize,
    pub poll_ms: f64,
    pub changed: bool,
    pub playable_ready: bool,
    pub cached_section_count: usize,
    pub rebuilt_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub completed_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    pub progress: Option<LoadingProgressOverlay>,
}

#[derive(Debug)]
pub struct LocalSingleViewStartupPump {
    runtime: LocalSingleViewSceneRuntime,
    poll_count: usize,
    poll_ms: f64,
}

impl LocalSingleViewStartupPump {
    pub fn with_mesh_assets(
        options: LocalSingleViewSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        Ok(Self {
            runtime: LocalSingleViewSceneRuntime::with_mesh_assets(options, mesh_assets)?,
            poll_count: 0,
            poll_ms: 0.0,
        })
    }

    pub fn step(&mut self, camera_position: Vec3) -> Result<LocalSingleViewStartupStep> {
        let poll_start = Instant::now();
        let mut changed = self.runtime.poll()?;
        let poll_ms = elapsed_ms(poll_start.elapsed());
        self.poll_count += 1;
        self.poll_ms += poll_ms;
        let diagnostics = self.runtime.server_runner_diagnostics()?;
        self.runtime.last_runner_diagnostics = Some(diagnostics);
        self.runtime.last_runner_diagnostics_poll_at = Some(Instant::now());

        let section_update = self.runtime.sync_render_sections(camera_position)?;
        let render_changed = section_update.rebuilt_section_count() > 0
            || section_update.removed_section_count() > 0
            || section_update.submitted_compile_section_count > 0
            || section_update.completed_compile_section_count > 0;
        changed |= render_changed;
        let progress = self.progress_overlay();
        let playable_ready = self.playable_ready();
        Ok(LocalSingleViewStartupStep {
            poll_count: self.poll_count,
            poll_ms: self.poll_ms,
            changed,
            playable_ready,
            cached_section_count: self.runtime.cached_sections().len(),
            rebuilt_section_count: section_update.rebuilt_section_count(),
            submitted_compile_section_count: section_update.submitted_compile_section_count,
            completed_compile_section_count: section_update.completed_compile_section_count,
            pending_compile_jobs: section_update.pending_compile_jobs,
            progress,
        })
    }

    pub fn progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.runtime
            .last_runner_diagnostics
            .as_ref()
            .and_then(loading_progress_overlay_from_diagnostics)
    }

    pub fn playable_ready(&self) -> bool {
        let Some(progress) = self
            .runtime
            .last_runner_diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.loading_progress)
        else {
            return false;
        };
        progress.playable_chunk_ready
            && self
                .runtime
                .client()
                .chunk_snapshot(progress.playable_chunk)
                .is_some()
            && !self.runtime.cached_sections().is_empty()
    }

    pub const fn poll_count(&self) -> usize {
        self.poll_count
    }

    pub const fn poll_ms(&self) -> f64 {
        self.poll_ms
    }

    pub fn runtime(&self) -> &LocalSingleViewSceneRuntime {
        &self.runtime
    }

    pub fn into_runtime(self) -> LocalSingleViewSceneRuntime {
        self.runtime
    }
}

impl LocalSingleViewSceneRuntime {
    pub fn new(options: LocalSingleViewSceneOptions) -> Result<Self> {
        let mesh_assets = load_textured_mesh_assets()?;
        Self::with_mesh_assets(options, mesh_assets)
    }

    pub fn with_mesh_assets(
        options: LocalSingleViewSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let render_compile_worker = RenderSectionCompileWorker::with_worker_count(
            mesh_assets.catalog.clone(),
            options.render_compile_worker_count,
        )?;
        let server_runner = NativeIntegratedServerRunner::new(native_runner_config(&options))
            .context("failed to start local single-view integrated server runner")?;
        let mut scene = Self {
            core: SingleViewRuntime::local_integrated(
                options.center,
                options.render_distance,
                options.chunk_tracking_radius(),
            ),
            server_runner,
            mesh_assets,
            render_compile_worker,
            far_lod_cache: FarTerrainLodCache::new(),
            simulation_cadence: options.cadence,
            last_runner_diagnostics: None,
            last_runner_diagnostics_poll_at: None,
        };
        if let Some(day_time) = options.day_time_override {
            scene.core.force_day_time(day_time);
        }
        scene.set_chunk_view(
            options.center,
            options.render_distance,
            options.chunk_tracking_radius(),
        )?;
        Ok(scene)
    }

    pub const fn core(&self) -> &SingleViewRuntime {
        &self.core
    }

    pub const fn core_mut(&mut self) -> &mut SingleViewRuntime {
        &mut self.core
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.core.client()
    }

    pub const fn mesh_assets(&self) -> &TexturedMeshAssets {
        &self.mesh_assets
    }

    pub fn clear_far_lod(&mut self) {
        self.far_lod_cache.clear();
    }

    pub fn prepare_far_lod_mesh(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        camera_position: Vec3,
    ) -> Option<&FarTerrainLodMesh> {
        let normal_terrain_chunks = traversal_ready_chunks(
            &self
                .core
                .traversal_ready_render_section_keys(camera_position),
        );
        self.far_lod_cache.mesh_for_camera(
            config,
            seed,
            center,
            self.core.render_distance(),
            Some(&normal_terrain_chunks),
            self.mesh_assets.far_lod_materials.as_ref(),
        )
    }

    pub fn render_distance(&self) -> u32 {
        self.core.render_distance()
    }

    pub const fn simulation_cadence(&self) -> SimulationCadenceConfig {
        self.simulation_cadence
    }

    pub fn set_simulation_cadence(&mut self, cadence: SimulationCadenceConfig) -> Result<bool> {
        if cadence == self.simulation_cadence {
            return Ok(false);
        }
        self.server_runner
            .set_simulation_cadence(cadence)
            .context("failed to set local single-view simulation cadence")?;
        self.simulation_cadence = cadence;
        if let Some(diagnostics) = self.last_runner_diagnostics.as_mut() {
            diagnostics.simulation_cadence = cadence;
            diagnostics.host_tick_interval = host_tick_interval_for_rate_hz(cadence.host_rate_hz);
        }
        Ok(true)
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.core.client().loaded_chunk_count()
    }

    pub fn set_chunk_view(
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
        self.server_runner
            .send_command(command)
            .context("failed to send local single-view chunk view command")?;
        let _ = self.drain_runner_updates_report()?;
        Ok(true)
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        self.server_runner
            .send_command(command)
            .context("failed to send local single-view gameplay command")?;
        let _ = self.drain_runner_updates_report()?;
        Ok(true)
    }

    pub fn poll(&mut self) -> Result<bool> {
        let poll_start = Instant::now();
        let drain_start = Instant::now();
        let updates = self
            .server_runner
            .drain_updates()
            .context("failed to drain local single-view integrated server updates")?;
        let drain_updates_ms = elapsed_ms(drain_start.elapsed());
        let apply_report = if updates.is_empty() {
            RuntimeUpdateApplyReport::default()
        } else {
            self.core.apply_server_updates_report(updates)
        };
        let changed = apply_report.changed;
        let (
            runner_diagnostics,
            poll_diagnostics_ms,
            diagnostics_refreshed,
            diagnostics_cache_age_ms,
        ) = self.poll_runner_diagnostics_for_frame()?;
        self.core.finish_poll_diagnostics(
            RuntimePollTiming {
                total_ms: elapsed_ms(poll_start.elapsed()),
                drain_updates_ms,
                poll_diagnostics_ms,
                diagnostics_refreshed,
                diagnostics_cache_age_ms,
            },
            apply_report,
            runner_diagnostics.as_ref(),
        );
        Ok(changed)
    }

    fn poll_runner_diagnostics_for_frame(
        &mut self,
    ) -> Result<(Option<ServerRunnerDiagnostics>, f64, bool, f64)> {
        let should_poll = self
            .last_runner_diagnostics_poll_at
            .is_none_or(|last_poll| last_poll.elapsed() >= RUNTIME_DIAGNOSTICS_POLL_INTERVAL);
        let mut poll_diagnostics_ms = 0.0;
        let mut diagnostics_refreshed = false;
        if should_poll {
            let diagnostics_start = Instant::now();
            let runner_diagnostics = self
                .server_runner
                .poll_diagnostics()
                .context("failed to poll local single-view integrated server diagnostics")?;
            self.simulation_cadence = runner_diagnostics.simulation_cadence;
            poll_diagnostics_ms = elapsed_ms(diagnostics_start.elapsed());
            self.last_runner_diagnostics = Some(runner_diagnostics);
            self.last_runner_diagnostics_poll_at = Some(Instant::now());
            diagnostics_refreshed = true;
        } else if let Some(runner_diagnostics) = self.last_runner_diagnostics.as_mut() {
            self.server_runner
                .refresh_fast_diagnostics(runner_diagnostics);
        }

        let diagnostics_cache_age_ms = self
            .last_runner_diagnostics_poll_at
            .map_or(0.0, |last_poll| elapsed_ms(last_poll.elapsed()));
        Ok((
            self.last_runner_diagnostics.clone(),
            poll_diagnostics_ms,
            diagnostics_refreshed,
            diagnostics_cache_age_ms,
        ))
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut polls = 0_usize;
        let mut poll_ms = 0.0_f64;
        loop {
            let poll_start = Instant::now();
            self.poll()?;
            poll_ms += elapsed_ms(poll_start.elapsed());
            polls += 1;
            let diagnostics = self.server_runner_diagnostics()?;
            if runner_idle(&diagnostics) {
                return Ok((polls, poll_ms));
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for local single-view worldgen jobs");
            }
            if diagnostics.update_queue_depth == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                render_compile_worker,
                camera_position,
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                None,
            )
    }

    pub fn sync_render_sections_with_completed_result_acceptance(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                render_compile_worker,
                camera_position,
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots_timed(
                render_compile_worker,
                camera_position,
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_until_deadline(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
    ) -> Result<RenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots(
                render_compile_worker,
                camera_position,
                deadline,
                None,
            )
    }

    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots(
                render_compile_worker,
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        let render_compile_worker = &mut self.render_compile_worker;
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots_timed(
                render_compile_worker,
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    pub fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core.sync_all_render_sections_targeted_snapshots(
            &mut self.render_compile_worker,
            camera_position,
        )
    }

    pub fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.core.cached_sections()
    }

    pub fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.core
            .traversal_ready_render_section_keys(camera_position)
    }

    pub fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.core.time_of_day())
    }

    pub fn time_of_day(&self) -> f32 {
        self.core.time_of_day()
    }

    pub fn sun_angle(&self) -> f32 {
        self.core.sun_angle()
    }

    fn server_runner_diagnostics(&self) -> Result<ServerRunnerDiagnostics> {
        self.server_runner
            .poll_diagnostics()
            .context("failed to poll local single-view integrated server diagnostics")
    }

    fn drain_runner_updates_report(&mut self) -> Result<RuntimeUpdateApplyReport> {
        let updates = self
            .server_runner
            .drain_updates()
            .context("failed to drain local single-view integrated server updates")?;
        if updates.is_empty() {
            return Ok(RuntimeUpdateApplyReport::default());
        }
        Ok(self.core.apply_server_updates_report(updates))
    }
}

#[derive(Debug)]
pub enum NativeSingleViewSceneRuntime<S> {
    Local(LocalSingleViewSceneRuntime),
    RemoteDedicated(RemoteDedicatedSingleViewSceneRuntime<S>),
}

#[derive(Debug)]
pub struct NativeSingleViewSessionRuntime<S> {
    session: GameSessionCoordinator<()>,
    runtime: NativeSingleViewSceneRuntime<S>,
}

impl<S> NativeSingleViewSceneRuntime<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn local(options: LocalSingleViewSceneOptions) -> Result<Self> {
        Ok(Self::Local(LocalSingleViewSceneRuntime::new(options)?))
    }

    pub fn local_with_mesh_assets(
        options: LocalSingleViewSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        Ok(Self::Local(LocalSingleViewSceneRuntime::with_mesh_assets(
            options,
            mesh_assets,
        )?))
    }

    pub fn remote_dedicated(options: SingleViewHostOptions, session: S) -> Result<Self> {
        Ok(Self::RemoteDedicated(
            RemoteDedicatedSingleViewSceneRuntime::new(options, session)?,
        ))
    }

    pub fn remote_dedicated_with_mesh_assets(
        options: SingleViewHostOptions,
        session: S,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        Ok(Self::RemoteDedicated(
            RemoteDedicatedSingleViewSceneRuntime::with_mesh_assets(options, session, mesh_assets)?,
        ))
    }

    pub const fn host_mode(&self) -> SingleViewHostMode {
        match self {
            Self::Local(_) => SingleViewHostMode::LocalIntegrated,
            Self::RemoteDedicated(_) => SingleViewHostMode::RemoteDedicated,
        }
    }

    pub fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene
                .sync_render_sections_with_completed_result_acceptance_timed(
                    camera_position,
                    completed_result_accept_budget,
                ),
            Self::RemoteDedicated(scene) => scene
                .sync_render_sections_with_completed_result_acceptance_timed(
                    camera_position,
                    completed_result_accept_budget,
                ),
        }
    }

    pub const fn host_label(&self) -> &'static str {
        match self.host_mode() {
            SingleViewHostMode::LocalIntegrated => "local integrated",
            SingleViewHostMode::RemoteDedicated => "remote dedicated",
        }
    }

    pub fn core(&self) -> &SingleViewRuntime {
        match self {
            Self::Local(scene) => scene.core(),
            Self::RemoteDedicated(scene) => scene.core(),
        }
    }

    pub fn core_mut(&mut self) -> &mut SingleViewRuntime {
        match self {
            Self::Local(scene) => scene.core_mut(),
            Self::RemoteDedicated(scene) => scene.core_mut(),
        }
    }

    pub fn client(&self) -> &ClientRuntime {
        match self {
            Self::Local(scene) => scene.client(),
            Self::RemoteDedicated(scene) => scene.client(),
        }
    }

    pub fn mesh_assets(&self) -> &TexturedMeshAssets {
        match self {
            Self::Local(scene) => scene.mesh_assets(),
            Self::RemoteDedicated(scene) => scene.mesh_assets(),
        }
    }

    pub fn clear_far_lod(&mut self) {
        match self {
            Self::Local(scene) => scene.clear_far_lod(),
            Self::RemoteDedicated(scene) => scene.clear_far_lod(),
        }
    }

    pub fn prepare_far_lod_mesh(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        camera_position: Vec3,
    ) -> Option<&FarTerrainLodMesh> {
        match self {
            Self::Local(scene) => scene.prepare_far_lod_mesh(config, seed, center, camera_position),
            Self::RemoteDedicated(scene) => {
                scene.prepare_far_lod_mesh(config, seed, center, camera_position)
            }
        }
    }

    pub fn render_distance(&self) -> u32 {
        self.core().render_distance()
    }

    pub const fn simulation_cadence(&self) -> Option<SimulationCadenceConfig> {
        match self {
            Self::Local(scene) => Some(scene.simulation_cadence()),
            Self::RemoteDedicated(_) => None,
        }
    }

    pub fn set_simulation_cadence(&mut self, cadence: SimulationCadenceConfig) -> Result<bool> {
        match self {
            Self::Local(scene) => scene.set_simulation_cadence(cadence),
            Self::RemoteDedicated(_) => Ok(false),
        }
    }

    pub fn chunk_tracking_radius(&self) -> u32 {
        self.core().chunk_tracking_radius()
    }

    pub fn interest_center(&self) -> ChunkPos {
        self.core().interest_center()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        match self {
            Self::Local(scene) => scene.loaded_chunk_count(),
            Self::RemoteDedicated(scene) => scene.loaded_chunk_count(),
        }
    }

    pub fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        self.set_chunk_view(center, self.render_distance(), self.chunk_tracking_radius())
    }

    pub fn set_render_distance(&mut self, render_distance: u32) -> Result<bool> {
        self.set_chunk_view(
            self.interest_center(),
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
    }

    pub fn set_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<bool> {
        let Some(command) =
            self.core_mut()
                .set_chunk_view_command(center, render_distance, chunk_tracking_radius)
        else {
            return Ok(false);
        };
        self.send_gameplay_command(command)
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        match self {
            Self::Local(scene) => scene.send_gameplay_command(command),
            Self::RemoteDedicated(scene) => scene.send_gameplay_command(command),
        }
    }

    pub fn drain_player_position_updates(&mut self) -> Vec<mclone_protocol::PlayerPositionUpdate> {
        self.core_mut().drain_player_position_updates()
    }

    pub fn poll(&mut self) -> Result<bool> {
        match self {
            Self::Local(scene) => scene.poll(),
            Self::RemoteDedicated(scene) => scene.poll(),
        }
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        match self {
            Self::Local(scene) => scene.poll_until_idle(),
            Self::RemoteDedicated(scene) => scene.poll_until_idle(),
        }
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene.sync_render_sections(camera_position),
            Self::RemoteDedicated(scene) => scene.sync_render_sections(camera_position),
        }
    }

    pub fn sync_render_sections_with_completed_result_acceptance(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene.sync_render_sections_with_completed_result_acceptance(
                camera_position,
                completed_result_accept_budget,
            ),
            Self::RemoteDedicated(scene) => scene
                .sync_render_sections_with_completed_result_acceptance(
                    camera_position,
                    completed_result_accept_budget,
                ),
        }
    }

    pub fn sync_render_sections_until_deadline(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
    ) -> Result<RenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => {
                scene.sync_render_sections_until_deadline(camera_position, deadline)
            }
            Self::RemoteDedicated(scene) => {
                scene.sync_render_sections_until_deadline(camera_position, deadline)
            }
        }
    }

    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene
                .sync_render_sections_until_deadline_with_completed_result_acceptance(
                    camera_position,
                    deadline,
                    completed_result_accept_budget,
                ),
            Self::RemoteDedicated(scene) => scene
                .sync_render_sections_until_deadline_with_completed_result_acceptance(
                    camera_position,
                    deadline,
                    completed_result_accept_budget,
                ),
        }
    }

    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene
                .sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
                    camera_position,
                    deadline,
                    completed_result_accept_budget,
                ),
            Self::RemoteDedicated(scene) => scene
                .sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
                    camera_position,
                    deadline,
                    completed_result_accept_budget,
                ),
        }
    }

    pub fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene.sync_all_render_sections(camera_position),
            Self::RemoteDedicated(scene) => scene.sync_all_render_sections(camera_position),
        }
    }

    pub fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        match self {
            Self::Local(scene) => scene.cached_sections(),
            Self::RemoteDedicated(scene) => scene.cached_sections(),
        }
    }

    pub fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        match self {
            Self::Local(scene) => scene.traversal_ready_render_section_keys(camera_position),
            Self::RemoteDedicated(scene) => {
                scene.traversal_ready_render_section_keys(camera_position)
            }
        }
    }

    pub fn sky_clear_color(&self) -> wgpu::Color {
        match self {
            Self::Local(scene) => scene.sky_clear_color(),
            Self::RemoteDedicated(scene) => scene.sky_clear_color(),
        }
    }

    pub fn time_of_day(&self) -> f32 {
        match self {
            Self::Local(scene) => scene.time_of_day(),
            Self::RemoteDedicated(scene) => scene.time_of_day(),
        }
    }

    pub fn sun_angle(&self) -> f32 {
        match self {
            Self::Local(scene) => scene.sun_angle(),
            Self::RemoteDedicated(scene) => scene.sun_angle(),
        }
    }

    pub fn force_day_time(&mut self, day_time: u64) {
        self.core_mut().force_day_time(day_time);
    }

    pub fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.core()
            .has_pending_render_work(self.render_compile_pending_job_count(), camera_position)
    }

    pub fn render_compile_pending_job_count(&self) -> usize {
        match self {
            Self::Local(scene) => scene.render_compile_worker.pending_job_count(),
            Self::RemoteDedicated(scene) => scene.render_compile_worker.pending_job_count(),
        }
    }

    pub fn pending_completed_compile_result_count(&self) -> usize {
        self.core().pending_completed_compile_result_count()
    }

    pub fn render_compile_max_pending_job_count(&self) -> usize {
        match self {
            Self::Local(scene) => scene.render_compile_worker.max_pending_job_count(),
            Self::RemoteDedicated(scene) => scene.render_compile_worker.max_pending_job_count(),
        }
    }

    pub fn render_compile_available_pending_job_slots(&self) -> usize {
        match self {
            Self::Local(scene) => scene.render_compile_worker.available_pending_job_slots(),
            Self::RemoteDedicated(scene) => {
                scene.render_compile_worker.available_pending_job_slots()
            }
        }
    }

    pub fn pending_render_chunk_count(&self) -> usize {
        self.core().pending_render_chunk_count()
    }

    pub fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.core().last_poll_diagnostics()
    }

    pub fn view_readiness_overlay(&self) -> Option<LoadingProgressOverlay> {
        match self {
            Self::Local(scene) => scene
                .last_runner_diagnostics
                .as_ref()
                .and_then(view_readiness_overlay_from_diagnostics),
            Self::RemoteDedicated(_) => None,
        }
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

    pub fn stats(&self) -> SingleViewRuntimeStats {
        let runner_diagnostics = match self {
            Self::Local(scene) => scene.server_runner_diagnostics().ok(),
            Self::RemoteDedicated(_) => None,
        };
        self.core().stats(
            runner_diagnostics.as_ref(),
            self.render_compile_pending_job_count(),
        )
    }
}

impl<S> NativeSingleViewSessionRuntime<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn local(options: LocalSingleViewSceneOptions) -> Result<Self> {
        let request = SessionStartRequest::NewLocalWorld { seed: options.seed };
        Self::start_with(request, || NativeSingleViewSceneRuntime::local(options))
    }

    pub fn local_with_mesh_assets(
        options: LocalSingleViewSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let request = SessionStartRequest::NewLocalWorld { seed: options.seed };
        Self::start_with(request, || {
            NativeSingleViewSceneRuntime::local_with_mesh_assets(options, mesh_assets)
        })
    }

    pub fn remote_dedicated(
        endpoint: RemoteSessionEndpoint,
        options: SingleViewHostOptions,
        session: S,
    ) -> Result<Self> {
        let request = SessionStartRequest::JoinRemote { endpoint };
        Self::start_with(request, || {
            NativeSingleViewSceneRuntime::remote_dedicated(options, session)
        })
    }

    pub fn remote_dedicated_with_mesh_assets(
        endpoint: RemoteSessionEndpoint,
        options: SingleViewHostOptions,
        session: S,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let request = SessionStartRequest::JoinRemote { endpoint };
        Self::start_with(request, || {
            NativeSingleViewSceneRuntime::remote_dedicated_with_mesh_assets(
                options,
                session,
                mesh_assets,
            )
        })
    }

    pub fn from_active_runtime(
        request: SessionStartRequest,
        runtime: NativeSingleViewSceneRuntime<S>,
    ) -> Result<Self> {
        let descriptor = request
            .active_descriptor()
            .context("native single-view session request did not describe an active session")?;
        let mut session = GameSessionCoordinator::new();
        let result: SessionStartResult<()> = Ok(StartedGameSession::new(descriptor, ()));
        session.apply_start_result(&result);
        Ok(Self { session, runtime })
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
        self.session.status()
    }

    pub fn into_runtime(self) -> NativeSingleViewSceneRuntime<S> {
        self.runtime
    }

    fn start_with(
        request: SessionStartRequest,
        start: impl FnOnce() -> Result<NativeSingleViewSceneRuntime<S>>,
    ) -> Result<Self> {
        let mut session = GameSessionCoordinator::new();
        session.begin_start(request.clone());
        let descriptor = request
            .active_descriptor()
            .context("native single-view session request did not describe an active session")?;
        match start() {
            Ok(runtime) => {
                let result: SessionStartResult<()> = Ok(StartedGameSession::new(descriptor, ()));
                session.apply_start_result(&result);
                Ok(Self { session, runtime })
            }
            Err(error) => {
                log::error!("failed to start native single-view session {request:?}: {error:#}");
                session.fail_start(SessionFailure::new(request.default_failure_message()));
                Err(error)
            }
        }
    }
}

impl<S> Deref for NativeSingleViewSessionRuntime<S> {
    type Target = NativeSingleViewSceneRuntime<S>;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl<S> DerefMut for NativeSingleViewSessionRuntime<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

#[derive(Debug)]
pub struct RemoteDedicatedSingleViewSceneRuntime<S> {
    core: SingleViewRuntime,
    session: S,
    mesh_assets: TexturedMeshAssets,
    render_compile_worker: RenderSectionCompileWorker,
    far_lod_cache: FarTerrainLodCache,
}

impl<S> RemoteDedicatedSingleViewSceneRuntime<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn new(options: SingleViewHostOptions, session: S) -> Result<Self> {
        let mesh_assets = load_textured_mesh_assets()?;
        Self::with_mesh_assets(options, session, mesh_assets)
    }

    pub fn with_mesh_assets(
        options: SingleViewHostOptions,
        mut session: S,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let render_compile_worker = RenderSectionCompileWorker::with_worker_count(
            mesh_assets.catalog.clone(),
            options.render_compile_worker_count,
        )?;
        let mut core = SingleViewRuntime::remote_dedicated(
            options.center,
            options.render_distance,
            options.chunk_tracking_radius,
        );
        if let Some(command) = core.set_chunk_view_command(
            options.center,
            options.render_distance,
            options.chunk_tracking_radius,
        ) {
            dispatch_remote_dedicated_command(&mut core, &mut session, command)
                .context("failed to initialize remote dedicated single-view runtime")?;
        }
        Ok(Self {
            core,
            session,
            mesh_assets,
            render_compile_worker,
            far_lod_cache: FarTerrainLodCache::new(),
        })
    }

    pub const fn core(&self) -> &SingleViewRuntime {
        &self.core
    }

    pub const fn core_mut(&mut self) -> &mut SingleViewRuntime {
        &mut self.core
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.core.client()
    }

    pub const fn mesh_assets(&self) -> &TexturedMeshAssets {
        &self.mesh_assets
    }

    pub fn clear_far_lod(&mut self) {
        self.far_lod_cache.clear();
    }

    pub fn prepare_far_lod_mesh(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        camera_position: Vec3,
    ) -> Option<&FarTerrainLodMesh> {
        let normal_terrain_chunks = traversal_ready_chunks(
            &self
                .core
                .traversal_ready_render_section_keys(camera_position),
        );
        self.far_lod_cache.mesh_for_camera(
            config,
            seed,
            center,
            self.core.render_distance(),
            Some(&normal_terrain_chunks),
            self.mesh_assets.far_lod_materials.as_ref(),
        )
    }

    pub fn render_distance(&self) -> u32 {
        self.core.render_distance()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.core.client().loaded_chunk_count()
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        dispatch_remote_dedicated_command(&mut self.core, &mut self.session, command)
    }

    pub fn poll(&mut self) -> Result<bool> {
        Ok(false)
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        Ok((0, 0.0))
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                &mut self.render_compile_worker,
                camera_position,
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                None,
            )
    }

    pub fn sync_render_sections_with_completed_result_acceptance(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                &mut self.render_compile_worker,
                camera_position,
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots_timed(
                &mut self.render_compile_worker,
                camera_position,
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_until_deadline(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots(
                &mut self.render_compile_worker,
                camera_position,
                deadline,
                None,
            )
    }

    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots(
                &mut self.render_compile_worker,
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots_timed(
                &mut self.render_compile_worker,
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    pub fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core.sync_all_render_sections_targeted_snapshots(
            &mut self.render_compile_worker,
            camera_position,
        )
    }

    pub fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.core.cached_sections()
    }

    pub fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.core
            .traversal_ready_render_section_keys(camera_position)
    }

    pub fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.core.time_of_day())
    }

    pub fn time_of_day(&self) -> f32 {
        self.core.time_of_day()
    }

    pub fn sun_angle(&self) -> f32 {
        self.core.sun_angle()
    }
}

pub fn build_local_single_view_client_runtime(
    options: LocalSingleViewSceneOptions,
) -> Result<ClientRuntime> {
    let mut runtime = SingleViewRuntime::local_integrated(
        options.center,
        options.render_distance,
        options.chunk_tracking_radius(),
    );
    let mut runner = NativeIntegratedServerRunner::new(native_runner_config(&options))
        .context("failed to start local single-view integrated server runner")?;
    if let Some(day_time) = options.day_time_override {
        runtime.force_day_time(day_time);
    }
    if let Some(command) = runtime.set_chunk_view_command(
        options.center,
        options.render_distance,
        options.chunk_tracking_radius(),
    ) {
        runner
            .send_command(command)
            .context("failed to send local single-view chunk view command")?;
        runtime.apply_server_updates(drain_integrated_server_runner_until_idle(&mut runner)?);
    }
    runner
        .join_shutdown()
        .context("failed to stop local single-view integrated server runner")?;
    Ok(runtime.client().clone())
}

pub fn drain_integrated_server_runner_until_idle(
    runner: &mut NativeIntegratedServerRunner,
) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            runner
                .drain_updates()
                .context("failed to drain local single-view integrated server updates")?,
        );
        let diagnostics = runner
            .poll_diagnostics()
            .context("failed to poll local single-view integrated server diagnostics")?;
        if runner_idle(&diagnostics) {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for local single-view integrated server jobs");
        }
        if diagnostics.update_queue_depth == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn native_runner_config(
    options: &LocalSingleViewSceneOptions,
) -> NativeIntegratedServerRunnerConfig {
    NativeIntegratedServerRunnerConfig::new(options.seed)
        .with_lighting_enabled(options.lighting_enabled)
        .with_debug_passive_showcase(options.debug_passive_showcase)
        .with_day_time(options.day_time_override)
        .with_day_time_frozen(options.freeze_time)
        .with_cadence_derived_tick_interval(options.cadence)
}

fn runner_idle(diagnostics: &ServerRunnerDiagnostics) -> bool {
    diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && !diagnostics.awaiting_tick
        && diagnostics.pending_jobs == 0
        && diagnostics.pending_publications == 0
}

fn traversal_ready_chunks(sections: &BTreeSet<RenderSectionKey>) -> BTreeSet<ChunkPos> {
    sections
        .iter()
        .map(|key| ChunkPos::new(key.chunk_x, key.chunk_z))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_assets::extracted_asset_root;

    #[derive(Debug)]
    enum NoRemoteSession {}

    impl RemoteDedicatedServerSession for NoRemoteSession {
        fn send_command(&mut self, _command: ClientCommand) -> Result<Vec<ServerUpdate>> {
            match *self {}
        }

        fn reconnect(&mut self) -> Result<()> {
            match *self {}
        }
    }

    #[test]
    fn local_single_view_options_apply_java_tracking_radius() {
        let options = LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 2);

        assert_eq!(options.chunk_tracking_radius(), 3);
    }

    #[test]
    fn local_single_view_options_can_use_java_initial_spawn_center() {
        let options = LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 2)
            .with_initial_spawn_center();

        assert_eq!(options.center, initial_spawn_center_for_seed(12345));
    }

    #[test]
    fn native_runner_config_derives_tick_interval_from_cadence_host_rate() {
        let cadence = SimulationCadenceConfig::new(60, 20, 60);
        let options =
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 2).with_cadence(cadence);

        let config = native_runner_config(&options);

        assert_eq!(config.cadence, cadence);
        assert_eq!(config.tick_interval, Duration::from_nanos(16_666_667));
    }

    #[test]
    fn build_local_single_view_client_runtime_loads_center_chunk_without_assets() {
        let client = build_local_single_view_client_runtime(LocalSingleViewSceneOptions::new(
            12345,
            ChunkPos::new(0, 0),
            0,
        ))
        .unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(client.chunk_snapshot(ChunkPos::new(0, 0)).is_some());
    }

    #[test]
    fn local_single_view_runtime_loads_center_chunk() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalSingleViewSceneRuntime::new(LocalSingleViewSceneOptions::new(
            12345,
            ChunkPos::new(0, 0),
            0,
        ))
        .unwrap();
        runtime.poll_until_idle().unwrap();

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert!(
            runtime
                .client()
                .chunk_snapshot(ChunkPos::new(0, 0))
                .is_some()
        );
    }

    #[test]
    fn local_single_view_runtime_applies_simulation_cadence_control() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalSingleViewSceneRuntime::new(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false),
        )
        .unwrap();
        let cadence = SimulationCadenceConfig::new(60, 20, 120);

        assert_eq!(
            runtime.simulation_cadence(),
            SimulationCadenceConfig::default()
        );
        assert!(runtime.set_simulation_cadence(cadence).unwrap());
        assert_eq!(runtime.simulation_cadence(), cadence);
        assert!(!runtime.set_simulation_cadence(cadence).unwrap());

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            runtime.poll().unwrap();
            if runtime
                .last_runner_diagnostics
                .as_ref()
                .is_some_and(|diagnostics| {
                    diagnostics.command_queue_depth == 0
                        && diagnostics.simulation_cadence == cadence
                })
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for runtime cadence diagnostics"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn local_startup_pump_reaches_playable_center() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mesh_assets = load_textured_mesh_assets().unwrap();
        let mut pump = LocalSingleViewStartupPump::with_mesh_assets(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false),
            mesh_assets,
        )
        .unwrap();
        let camera_position = Vec3::new(8.0, 80.0, 8.0);

        for _ in 0..2_000 {
            let step = pump.step(camera_position).unwrap();
            if step.playable_ready {
                assert!(step.cached_section_count > 0);
                assert!(step.progress.unwrap().playable_ready);
                let runtime = pump.into_runtime();
                assert_eq!(runtime.loaded_chunk_count(), 1);
                assert!(
                    runtime
                        .client()
                        .chunk_snapshot(ChunkPos::new(0, 0))
                        .is_some()
                );
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        panic!("startup pump did not reach playable center");
    }

    #[test]
    fn native_single_view_session_runtime_records_local_session() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = NativeSingleViewSessionRuntime::<NoRemoteSession>::local(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 0),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();

        assert_eq!(
            runtime.session_state(),
            &GameSessionState::Active {
                session: ActiveSessionDescriptor::LocalWorld { seed: 12345 }
            }
        );
        assert_eq!(
            runtime.active_session(),
            Some(&ActiveSessionDescriptor::LocalWorld { seed: 12345 })
        );
        assert_eq!(runtime.session_status(), None);
        assert_eq!(runtime.loaded_chunk_count(), 1);
    }
}
