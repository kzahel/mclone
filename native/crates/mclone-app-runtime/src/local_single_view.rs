use std::collections::{BTreeSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::ClientRuntime;
use mclone_core::{BlockStateId, ChunkPos, ChunkSnapshot};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
use mclone_protocol::{ClientCommand, ServerUpdate, encode_server_update};
use mclone_render::far_lod::FarTerrainLodMesh;
use mclone_render_session::{RenderSectionCacheUpdate, RenderSectionCompileQueueHealth};
use mclone_server::{
    IntegratedServerRunner, NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    NativeIntegratedServerWorldStorage, ServerRunnerDiagnostics, ServerUpdateEnvelope,
    SimulationCadenceConfig, host_tick_interval_for_rate_hz, initial_spawn_center_for_seed,
};
use mclone_ui::LoadingProgressOverlay;

use crate::client_connection::{
    ClientConnection, ClientConnectionDrainResult, ClientConnectionQueueMetrics,
    ConnectionUpdateDrainMode, QueuedServerUpdate, pump_client_connection_updates_report,
};
use crate::far_lod::{FarTerrainLodCache, FarTerrainLodConfig};
use crate::host_mode::{
    RemoteCommandUpdateBatch, RemoteDedicatedServerSession, SingleViewHostMode,
    SingleViewHostOptions, deferred_command_exchange, dispatch_remote_dedicated_command,
    prepare_remote_dedicated_resync_command, reconnect_remote_dedicated_session_and_resync,
};
use crate::render_assets::{
    DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS, DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
    NativeRenderSectionCompileDispatcher, TexturedMeshAssets, load_textured_mesh_assets,
};
use crate::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, RemoteSessionEndpoint,
    SessionFailure, SessionStartRequest, SessionStartResult, SessionStatus, StartedGameSession,
};
use crate::{
    DEFAULT_CLIENT_DEFERRED_CHUNK_DROP_ITEM_BUDGET, DEFAULT_RENDER_CHUNK_MESH_BUDGET,
    GameplayCommandTiming, GameplayCommandUpdatePolicy, RuntimePollDiagnostics, RuntimePollTiming,
    RuntimeUpdateApplyReport, RuntimeUpdatePumpBudget, SingleViewRuntime, SingleViewRuntimeStats,
    TargetRenderWorkStats, TimedRenderSectionCacheUpdate,
    chunk_tracking_radius_for_render_distance, elapsed_ms,
    loading_progress_overlay_from_diagnostics, view_readiness_overlay_from_diagnostics,
};

const RUNTIME_DIAGNOSTICS_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_LOCAL_SINGLE_VIEW_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSingleViewSceneOptions {
    pub seed: i64,
    pub center: ChunkPos,
    pub render_distance: u32,
    pub cadence: SimulationCadenceConfig,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub freeze_scheduled_fluid_ticks: bool,
    pub debug_passive_showcase: bool,
    pub lighting_enabled: bool,
    pub adaptive_chunk_publication_budget: bool,
    pub world_storage: NativeIntegratedServerWorldStorage,
    pub render_compile_worker_count: usize,
    pub render_compile_max_pending_jobs: Option<usize>,
    pub render_compile_worker_timing_enabled: bool,
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
            freeze_scheduled_fluid_ticks: false,
            debug_passive_showcase: true,
            lighting_enabled: true,
            adaptive_chunk_publication_budget: false,
            world_storage: NativeIntegratedServerWorldStorage::Transient,
            render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            render_compile_max_pending_jobs: Some(DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS),
            render_compile_worker_timing_enabled: true,
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

    pub const fn with_freeze_scheduled_fluid_ticks(mut self, freeze: bool) -> Self {
        self.freeze_scheduled_fluid_ticks = freeze;
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

    pub const fn with_adaptive_chunk_publication_budget(mut self, enabled: bool) -> Self {
        self.adaptive_chunk_publication_budget = enabled;
        self
    }

    pub fn with_world_storage(mut self, world_storage: NativeIntegratedServerWorldStorage) -> Self {
        self.world_storage = world_storage;
        self
    }

    pub fn with_persistent_world_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.world_storage = NativeIntegratedServerWorldStorage::Persistent { dir: dir.into() };
        self
    }

    pub const fn with_render_compile_worker_count(
        mut self,
        render_compile_worker_count: usize,
    ) -> Self {
        self.render_compile_worker_count = render_compile_worker_count;
        self
    }

    pub const fn with_render_compile_max_pending_jobs(
        mut self,
        render_compile_max_pending_jobs: Option<usize>,
    ) -> Self {
        self.render_compile_max_pending_jobs = render_compile_max_pending_jobs;
        self
    }

    pub const fn with_render_compile_worker_timing_enabled(mut self, enabled: bool) -> Self {
        self.render_compile_worker_timing_enabled = enabled;
        self
    }

    pub fn chunk_tracking_radius(&self) -> u32 {
        chunk_tracking_radius_for_render_distance(self.render_distance)
    }
}

#[derive(Debug)]
pub struct LocalSingleViewSceneRuntime {
    core: SingleViewRuntime,
    connection: LocalIntegratedConnection,
    mesh_assets: TexturedMeshAssets,
    render_compile_dispatcher: NativeRenderSectionCompileDispatcher,
    far_lod_cache: FarTerrainLodCache,
    deferred_chunk_drop_worker: DeferredChunkDropWorker,
    simulation_cadence: SimulationCadenceConfig,
    last_runner_diagnostics: Option<ServerRunnerDiagnostics>,
    last_runner_diagnostics_poll_at: Option<Instant>,
}

#[derive(Debug)]
struct LocalIntegratedConnection {
    runner: NativeIntegratedServerRunner,
}

impl LocalIntegratedConnection {
    fn new(runner: NativeIntegratedServerRunner) -> Self {
        Self { runner }
    }

    fn set_simulation_cadence(
        &mut self,
        cadence: SimulationCadenceConfig,
    ) -> mclone_server::ServerRunnerResult<()> {
        self.runner.set_simulation_cadence(cadence)
    }

    fn poll_diagnostics(&self) -> mclone_server::ServerRunnerResult<ServerRunnerDiagnostics> {
        self.runner.poll_diagnostics()
    }

    fn refresh_fast_diagnostics(&self, diagnostics: &mut ServerRunnerDiagnostics) {
        self.runner.refresh_fast_diagnostics(diagnostics);
    }
}

impl ClientConnection for LocalIntegratedConnection {
    fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
        self.runner
            .send_command(command)
            .context("failed to send local single-view integrated server command")
    }

    fn drain_next_update(
        &mut self,
        _mode: ConnectionUpdateDrainMode,
    ) -> Result<ClientConnectionDrainResult> {
        let Some(envelope) = self
            .runner
            .try_recv_update()
            .context("failed to receive local single-view integrated server update")?
        else {
            return Ok(ClientConnectionDrainResult::default());
        };
        Ok(ClientConnectionDrainResult::with_update(
            queued_update_from_runner_envelope(envelope),
            0,
            0,
        ))
    }

    fn pending_update_metrics(&mut self) -> Result<ClientConnectionQueueMetrics> {
        let diagnostics = self
            .runner
            .poll_diagnostics()
            .context("failed to poll local single-view integrated server diagnostics")?;
        Ok(ClientConnectionQueueMetrics::new(
            diagnostics.update_queue_depth,
            diagnostics.update_queue_bytes,
        ))
    }
}

fn queued_update_from_runner_envelope(envelope: ServerUpdateEnvelope) -> QueuedServerUpdate {
    QueuedServerUpdate::single(
        envelope.update,
        envelope.encoded_len,
        envelope.queued_age,
        true,
    )
}

struct DeferredChunkDropWorker {
    sender: Option<mpsc::Sender<(ChunkSnapshot, usize)>>,
    pending_items: Arc<AtomicUsize>,
    join_handle: Option<JoinHandle<()>>,
}

impl DeferredChunkDropWorker {
    fn new() -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<(ChunkSnapshot, usize)>();
        let pending_items = Arc::new(AtomicUsize::new(0));
        let worker_pending_items = Arc::clone(&pending_items);
        let join_handle = thread::Builder::new()
            .name("mclone-chunk-drop".to_owned())
            .spawn(move || {
                while let Ok((snapshot, item_count)) = receiver.recv() {
                    drop(snapshot);
                    worker_pending_items.fetch_sub(item_count, Ordering::AcqRel);
                }
            })
            .context("failed to spawn deferred chunk drop worker")?;
        Ok(Self {
            sender: Some(sender),
            pending_items,
            join_handle: Some(join_handle),
        })
    }

    fn enqueue(&self, snapshot: ChunkSnapshot, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.pending_items.fetch_add(item_count, Ordering::AcqRel);
        let Some(sender) = &self.sender else {
            drop(snapshot);
            self.pending_items.fetch_sub(item_count, Ordering::AcqRel);
            return;
        };
        if let Err(error) = sender.send((snapshot, item_count)) {
            let (snapshot, item_count) = error.0;
            drop(snapshot);
            self.pending_items.fetch_sub(item_count, Ordering::AcqRel);
        }
    }

    fn pending_item_count(&self) -> usize {
        self.pending_items.load(Ordering::Acquire)
    }
}

impl Drop for DeferredChunkDropWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

impl std::fmt::Debug for DeferredChunkDropWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredChunkDropWorker")
            .field("pending_items", &self.pending_item_count())
            .finish_non_exhaustive()
    }
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
        let mut changed = self
            .runtime
            .poll_with_update_budget(RuntimeUpdatePumpBudget::default_frame())?;
        let poll_ms = elapsed_ms(poll_start.elapsed());
        self.poll_count += 1;
        self.poll_ms += poll_ms;
        let diagnostics = self.runtime.server_runner_diagnostics()?;
        self.runtime.last_runner_diagnostics = Some(diagnostics);
        self.runtime.last_runner_diagnostics_poll_at = Some(Instant::now());

        let section_update = self.runtime.sync_render_sections(camera_position)?;
        self.runtime
            .release_render_compile_jobs(section_update.accepted_compile_result_count);
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
        let render_compile_dispatcher =
            NativeRenderSectionCompileDispatcher::with_worker_count_and_max_pending_jobs_and_timing(
                mesh_assets.catalog.clone(),
                options.render_compile_worker_count,
                options
                    .render_compile_max_pending_jobs
                    .unwrap_or(options.render_compile_worker_count),
                options.render_compile_worker_timing_enabled,
            )?;
        let server_runner = NativeIntegratedServerRunner::new(native_runner_config(&options))
            .context("failed to start local single-view integrated server runner")?;
        let mut scene = Self {
            core: SingleViewRuntime::local_integrated_with_seed(
                options.seed,
                options.center,
                options.render_distance,
                options.chunk_tracking_radius(),
            ),
            connection: LocalIntegratedConnection::new(server_runner),
            mesh_assets,
            render_compile_dispatcher,
            far_lod_cache: FarTerrainLodCache::new(),
            deferred_chunk_drop_worker: DeferredChunkDropWorker::new()?,
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

    fn handoff_deferred_client_chunk_drops(&mut self) -> usize {
        let mut handed_off_items = 0;
        loop {
            if handed_off_items >= DEFAULT_CLIENT_DEFERRED_CHUNK_DROP_ITEM_BUDGET {
                break;
            }
            let Some((snapshot, item_count)) = self.core.take_deferred_client_chunk_drop_snapshot()
            else {
                break;
            };
            handed_off_items += item_count;
            self.deferred_chunk_drop_worker
                .enqueue(snapshot, item_count);
        }
        handed_off_items
    }

    fn deferred_client_chunk_drop_backlog_items(&self) -> usize {
        self.core.deferred_client_chunk_drop_item_count()
            + self.deferred_chunk_drop_worker.pending_item_count()
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

    pub fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        self.render_compile_dispatcher.release_completed_jobs(count)
    }

    pub fn render_compile_queue_health(&self) -> RenderSectionCompileQueueHealth {
        self.render_compile_dispatcher.queue_health()
    }

    pub const fn simulation_cadence(&self) -> SimulationCadenceConfig {
        self.simulation_cadence
    }

    pub fn set_simulation_cadence(&mut self, cadence: SimulationCadenceConfig) -> Result<bool> {
        if cadence == self.simulation_cadence {
            return Ok(false);
        }
        self.connection
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
        self.connection
            .send_command_only(command)
            .context("failed to send local single-view chunk view command")?;
        Ok(true)
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        self.send_gameplay_command_timed(command)
            .map(|(changed, _)| changed)
    }

    pub fn send_gameplay_command_with_update_policy(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<bool> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
            .map(|(changed, _)| changed)
    }

    pub fn send_gameplay_command_timed(
        &mut self,
        command: ClientCommand,
    ) -> Result<(bool, GameplayCommandTiming)> {
        self.send_gameplay_command_with_update_policy_timed(
            command,
            GameplayCommandUpdatePolicy::DrainImmediately,
        )
    }

    pub fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        let total_start = Instant::now();
        let send_start = Instant::now();
        self.connection
            .send_command_only(command)
            .context("failed to send local single-view gameplay command")?;
        let send_ms = elapsed_ms(send_start.elapsed());
        let (drain_updates_ms, apply_report) = match policy {
            GameplayCommandUpdatePolicy::DrainImmediately => {
                let pump_report = pump_client_connection_updates_report(
                    &mut self.core,
                    &mut self.connection,
                    RuntimeUpdatePumpBudget::unlimited(),
                    ConnectionUpdateDrainMode::ReadyOnly,
                )
                .context("failed to drain local single-view integrated server updates")?;
                let drain_updates_ms = pump_report.drain_updates_ms;
                (drain_updates_ms, pump_report.apply_report)
            }
            GameplayCommandUpdatePolicy::SendOnly => (0.0, RuntimeUpdateApplyReport::default()),
        };
        Ok((
            true,
            GameplayCommandTiming {
                total_ms: elapsed_ms(total_start.elapsed()),
                send_ms,
                drain_updates_ms,
                apply_updates_ms: apply_report.total_ms,
                apply_dirty_mark_ms: apply_report.dirty_mark_ms,
                apply_client_updates_ms: apply_report.client_apply_updates_ms,
                updates: apply_report.updates,
                snapshot_updates: apply_report.snapshot_updates,
                section_block_updates: apply_report.section_block_updates,
                unload_updates: apply_report.unload_updates,
            },
        ))
    }

    pub fn poll(&mut self) -> Result<bool> {
        self.poll_with_update_budget(RuntimeUpdatePumpBudget::default())
    }

    pub fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool> {
        let poll_start = Instant::now();
        let pump_report = pump_client_connection_updates_report(
            &mut self.core,
            &mut self.connection,
            budget,
            ConnectionUpdateDrainMode::ReadyOnly,
        )?;
        let changed = pump_report.apply_report.changed;
        let deferred_drop_start = Instant::now();
        let client_deferred_chunk_drop_items = self.handoff_deferred_client_chunk_drops();
        let client_deferred_chunk_drop_ms = elapsed_ms(deferred_drop_start.elapsed());
        let client_deferred_chunk_drop_backlog_items =
            self.deferred_client_chunk_drop_backlog_items();
        let (
            runner_diagnostics,
            poll_diagnostics_ms,
            diagnostics_refreshed,
            diagnostics_cache_age_ms,
        ) = self.poll_runner_diagnostics_for_frame()?;
        self.core.finish_poll_diagnostics(
            RuntimePollTiming {
                total_ms: elapsed_ms(poll_start.elapsed()),
                drain_updates_ms: pump_report.drain_updates_ms,
                producer_read_ms: pump_report.producer_read_ms,
                producer_decode_ms: pump_report.producer_decode_ms,
                producer_response_sequence: pump_report.producer_response_sequence,
                client_deferred_chunk_drop_ms,
                client_deferred_chunk_drop_items,
                client_deferred_chunk_drop_backlog_items,
                update_pump_stalled: pump_report.stalled,
                update_pump_stall_count: pump_report.stall_count,
                server_update_queue_depth: pump_report.remaining_queue_depth,
                server_update_queue_bytes: pump_report.remaining_queue_bytes,
                server_update_applied_bytes: pump_report.update_bytes,
                server_update_oldest_applied_age_ms: pump_report.oldest_applied_update_age_ms,
                poll_diagnostics_ms,
                diagnostics_refreshed,
                diagnostics_cache_age_ms,
            },
            pump_report.apply_report,
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
                .connection
                .poll_diagnostics()
                .context("failed to poll local single-view integrated server diagnostics")?;
            self.simulation_cadence = runner_diagnostics.simulation_cadence;
            poll_diagnostics_ms = elapsed_ms(diagnostics_start.elapsed());
            self.last_runner_diagnostics = Some(runner_diagnostics);
            self.last_runner_diagnostics_poll_at = Some(Instant::now());
            diagnostics_refreshed = true;
        } else if let Some(runner_diagnostics) = self.last_runner_diagnostics.as_mut() {
            self.connection.refresh_fast_diagnostics(runner_diagnostics);
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
        self.poll_until_idle_with_timeout(DEFAULT_LOCAL_SINGLE_VIEW_IDLE_TIMEOUT)
    }

    pub fn poll_until_idle_with_timeout(&mut self, timeout: Duration) -> Result<(usize, f64)> {
        let deadline = Instant::now() + timeout;
        let mut polls = 0_usize;
        let mut poll_ms = 0.0_f64;
        loop {
            let poll_start = Instant::now();
            self.poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())?;
            poll_ms += elapsed_ms(poll_start.elapsed());
            polls += 1;
            let diagnostics = self.server_runner_diagnostics()?;
            if runner_idle(&diagnostics) && self.deferred_client_chunk_drop_backlog_items() == 0 {
                return Ok((polls, poll_ms));
            }
            if Instant::now() >= deadline {
                bail!(
                    "timed out waiting for local single-view worldgen jobs after {:.3}s",
                    timeout.as_secs_f64()
                );
            }
            if diagnostics.update_queue_depth == 0
                && self.deferred_client_chunk_drop_backlog_items() == 0
            {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                render_compile_dispatcher,
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
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                render_compile_dispatcher,
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
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots_timed(
                render_compile_dispatcher,
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
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots(
                render_compile_dispatcher,
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
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots(
                render_compile_dispatcher,
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
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots_timed(
                render_compile_dispatcher,
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    pub fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        let render_compile_dispatcher = &mut self.render_compile_dispatcher;
        self.core
            .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_targeted_snapshots_timed(
                render_compile_dispatcher,
                camera_position,
                deadline,
                max_compile_requests,
                completed_result_accept_budget,
            )
    }

    pub fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core.sync_all_render_sections_targeted_snapshots(
            &mut self.render_compile_dispatcher,
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

    pub fn target_render_work_stats(&self, camera_position: Vec3) -> TargetRenderWorkStats {
        self.core.target_render_work_stats(camera_position)
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
        self.connection
            .poll_diagnostics()
            .context("failed to poll local single-view integrated server diagnostics")
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

    pub fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        match self {
            Self::Local(scene) => scene.release_render_compile_jobs(count),
            Self::RemoteDedicated(scene) => scene.release_render_compile_jobs(count),
        }
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

    pub fn set_interest_center_with_update_policy_timed(
        &mut self,
        center: ChunkPos,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        self.set_chunk_view_with_update_policy_timed(
            center,
            self.render_distance(),
            self.chunk_tracking_radius(),
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

    pub fn set_chunk_view_with_update_policy_timed(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        match self {
            Self::Local(scene) => {
                let Some(command) = scene.core_mut().set_chunk_view_command(
                    center,
                    render_distance,
                    chunk_tracking_radius,
                ) else {
                    return Ok((false, GameplayCommandTiming::default()));
                };
                scene.send_gameplay_command_with_update_policy_timed(command, policy)
            }
            Self::RemoteDedicated(scene) => {
                let Some(command) = scene.core_mut().set_chunk_view_command(
                    center,
                    render_distance,
                    chunk_tracking_radius,
                ) else {
                    return Ok((false, GameplayCommandTiming::default()));
                };
                scene.send_gameplay_command_with_update_policy_timed(command, policy)
            }
        }
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        match self {
            Self::Local(scene) => scene.send_gameplay_command(command),
            Self::RemoteDedicated(scene) => scene.send_gameplay_command(command),
        }
    }

    pub fn send_gameplay_command_with_update_policy(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<bool> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
            .map(|(changed, _)| changed)
    }

    pub fn send_gameplay_command_timed(
        &mut self,
        command: ClientCommand,
    ) -> Result<(bool, GameplayCommandTiming)> {
        self.send_gameplay_command_with_update_policy_timed(
            command,
            GameplayCommandUpdatePolicy::DrainImmediately,
        )
    }

    pub fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        match self {
            Self::Local(scene) => {
                scene.send_gameplay_command_with_update_policy_timed(command, policy)
            }
            Self::RemoteDedicated(scene) => {
                scene.send_gameplay_command_with_update_policy_timed(command, policy)
            }
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

    pub fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool> {
        match self {
            Self::Local(scene) => scene.poll_with_update_budget(budget),
            Self::RemoteDedicated(scene) => scene.poll_with_update_budget(budget),
        }
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        match self {
            Self::Local(scene) => scene.poll_until_idle(),
            Self::RemoteDedicated(scene) => scene.poll_until_idle(),
        }
    }

    pub fn poll_until_idle_with_timeout(&mut self, timeout: Duration) -> Result<(usize, f64)> {
        match self {
            Self::Local(scene) => scene.poll_until_idle_with_timeout(timeout),
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

    pub fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        match self {
            Self::Local(scene) => scene
                .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
                    camera_position,
                    deadline,
                    max_compile_requests,
                    completed_result_accept_budget,
                ),
            Self::RemoteDedicated(scene) => scene
                .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
                    camera_position,
                    deadline,
                    max_compile_requests,
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

    pub fn target_render_work_stats(&self, camera_position: Vec3) -> TargetRenderWorkStats {
        self.core().target_render_work_stats(camera_position)
    }

    pub fn render_compile_pending_job_count(&self) -> usize {
        match self {
            Self::Local(scene) => scene.render_compile_dispatcher.pending_job_count(),
            Self::RemoteDedicated(scene) => scene.render_compile_dispatcher.pending_job_count(),
        }
    }

    pub fn pending_completed_compile_result_count(&self) -> usize {
        self.core().pending_completed_compile_result_count()
    }

    pub fn render_compile_max_pending_job_count(&self) -> usize {
        match self {
            Self::Local(scene) => scene.render_compile_dispatcher.max_pending_job_count(),
            Self::RemoteDedicated(scene) => scene.render_compile_dispatcher.max_pending_job_count(),
        }
    }

    pub fn render_compile_available_pending_job_slots(&self) -> usize {
        match self {
            Self::Local(scene) => scene
                .render_compile_dispatcher
                .available_pending_job_slots(),
            Self::RemoteDedicated(scene) => scene
                .render_compile_dispatcher
                .available_pending_job_slots(),
        }
    }

    pub fn render_compile_queue_health(&self) -> RenderSectionCompileQueueHealth {
        match self {
            Self::Local(scene) => scene.render_compile_queue_health(),
            Self::RemoteDedicated(scene) => scene.render_compile_queue_health(),
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
            self.host_mode(),
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
        let request = SessionStartRequest::new_seed_local_world(options.seed);
        Self::start_with(request, || NativeSingleViewSceneRuntime::local(options))
    }

    pub fn local_with_mesh_assets(
        options: LocalSingleViewSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let request = SessionStartRequest::new_seed_local_world(options.seed);
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
        Ok(Self::from_active_runtime_with_descriptor(
            descriptor, runtime,
        ))
    }

    pub fn from_active_runtime_with_descriptor(
        descriptor: ActiveSessionDescriptor,
        runtime: NativeSingleViewSceneRuntime<S>,
    ) -> Self {
        let mut session = GameSessionCoordinator::new();
        let result: SessionStartResult<()> = Ok(StartedGameSession::new(descriptor, ()));
        session.apply_start_result(&result);
        Self { session, runtime }
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
    connection: RemoteDedicatedConnection<S>,
    mesh_assets: TexturedMeshAssets,
    render_compile_dispatcher: NativeRenderSectionCompileDispatcher,
    far_lod_cache: FarTerrainLodCache,
}

#[derive(Debug)]
struct RemoteDedicatedConnection<S> {
    session: S,
    // Current native/WebSocket wire protocols still pair one response batch
    // with each sent command. This counter tracks those outstanding batches
    // before their decoded updates are queued for the shared ClientConnection
    // pump; it is not runtime-thread socket ownership.
    pending_response_batches: usize,
    queued_updates: VecDeque<QueuedServerUpdate>,
    queued_update_bytes: usize,
}

impl<S> RemoteDedicatedConnection<S>
where
    S: RemoteDedicatedServerSession,
{
    fn new(session: S) -> Self {
        Self {
            session,
            pending_response_batches: 0,
            queued_updates: VecDeque::new(),
            queued_update_bytes: 0,
        }
    }

    fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    fn clear_pending_updates(&mut self) {
        self.pending_response_batches = 0;
        self.queued_updates.clear();
        self.queued_update_bytes = 0;
    }

    fn push_response_batch(&mut self, batch: RemoteCommandUpdateBatch) -> Result<()> {
        let transport_drained_after_batch = self.pending_response_batches == 0;
        let update_count = batch.updates.len();
        let batch_response_sequence = batch.response_sequence;
        let batch_producer_read_ms = batch.producer_read_ms;
        let batch_producer_decode_ms = batch.producer_decode_ms;
        let batch_has_producer_timing =
            batch_producer_read_ms > 0.0 || batch_producer_decode_ms > 0.0;
        if update_count == 0 {
            self.queued_updates.push_back(
                QueuedServerUpdate::empty(transport_drained_after_batch).with_remote_metadata(
                    batch_response_sequence,
                    batch_producer_read_ms,
                    batch_producer_decode_ms,
                ),
            );
            return Ok(());
        }

        for (index, update) in batch.updates.into_iter().enumerate() {
            let (producer_read_ms, producer_decode_ms) = if batch_has_producer_timing {
                if index == 0 {
                    (batch_producer_read_ms, batch_producer_decode_ms)
                } else {
                    (0.0, 0.0)
                }
            } else {
                (update.producer_read_ms, update.producer_decode_ms)
            };
            let encoded_len = match update.encoded_len {
                Some(encoded_len) => encoded_len,
                None => encode_server_update(&update.update)
                    .context("failed to measure remote dedicated server update bytes")?
                    .len(),
            };
            self.queued_update_bytes = self.queued_update_bytes.saturating_add(encoded_len);
            self.queued_updates.push_back(
                QueuedServerUpdate::single(
                    update.update,
                    encoded_len,
                    update.queued_age,
                    transport_drained_after_batch && index + 1 == update_count,
                )
                .with_remote_metadata(
                    update.response_sequence.or(batch_response_sequence),
                    producer_read_ms,
                    producer_decode_ms,
                ),
            );
        }
        Ok(())
    }
}

impl<S> ClientConnection for RemoteDedicatedConnection<S>
where
    S: RemoteDedicatedServerSession,
{
    fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
        self.session.send_command_only(command)?;
        self.pending_response_batches = self.pending_response_batches.saturating_add(1);
        Ok(())
    }

    fn drain_next_update(
        &mut self,
        mode: ConnectionUpdateDrainMode,
    ) -> Result<ClientConnectionDrainResult> {
        if let Some(update) = self.queued_updates.pop_front() {
            self.queued_update_bytes = self
                .queued_update_bytes
                .saturating_sub(update.encoded_len());
            return Ok(ClientConnectionDrainResult::with_update(
                update,
                self.pending_response_batches + self.queued_updates.len(),
                self.queued_update_bytes,
            ));
        }

        if self.pending_response_batches == 0 {
            return Ok(ClientConnectionDrainResult::default());
        }

        let batch = match mode {
            ConnectionUpdateDrainMode::Blocking => Some(
                self.session
                    .drain_command_update_batch()
                    .context("failed to drain remote dedicated server updates")?,
            ),
            ConnectionUpdateDrainMode::ReadyOnly => {
                self.session
                    .try_drain_command_update_batch()
                    .context("failed to poll remote dedicated server updates")?
            }
        };
        let Some(batch) = batch else {
            return Ok(ClientConnectionDrainResult::pending(
                self.pending_response_batches,
                self.queued_update_bytes,
            ));
        };

        self.pending_response_batches = self.pending_response_batches.saturating_sub(1);
        self.push_response_batch(batch)?;
        if let Some(update) = self.queued_updates.pop_front() {
            self.queued_update_bytes = self
                .queued_update_bytes
                .saturating_sub(update.encoded_len());
            return Ok(ClientConnectionDrainResult::with_update(
                update,
                self.pending_response_batches + self.queued_updates.len(),
                self.queued_update_bytes,
            ));
        }

        Ok(ClientConnectionDrainResult::pending(
            self.pending_response_batches,
            self.queued_update_bytes,
        ))
    }

    fn pending_update_metrics(&mut self) -> Result<ClientConnectionQueueMetrics> {
        Ok(ClientConnectionQueueMetrics::new(
            self.pending_response_batches + self.queued_updates.len(),
            self.queued_update_bytes,
        ))
    }
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
        session: S,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let mut connection = RemoteDedicatedConnection::new(session);
        let render_compile_dispatcher =
            NativeRenderSectionCompileDispatcher::with_worker_count_and_max_pending_jobs_and_timing(
                mesh_assets.catalog.clone(),
                options.render_compile_worker_count,
                options
                    .render_compile_max_pending_jobs
                    .unwrap_or(options.render_compile_worker_count),
                options.render_compile_worker_timing_enabled,
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
            dispatch_remote_dedicated_command(&mut core, connection.session_mut(), command)
                .context("failed to initialize remote dedicated single-view runtime")?;
        }
        Ok(Self {
            core,
            connection,
            mesh_assets,
            render_compile_dispatcher,
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

    pub fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        self.render_compile_dispatcher.release_completed_jobs(count)
    }

    pub fn render_compile_queue_health(&self) -> RenderSectionCompileQueueHealth {
        self.render_compile_dispatcher.queue_health()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.core.client().loaded_chunk_count()
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        self.send_gameplay_command_timed(command)
            .map(|(changed, _)| changed)
    }

    pub fn send_gameplay_command_with_update_policy(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<bool> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
            .map(|(changed, _)| changed)
    }

    pub fn send_gameplay_command_timed(
        &mut self,
        command: ClientCommand,
    ) -> Result<(bool, GameplayCommandTiming)> {
        self.send_gameplay_command_with_update_policy_timed(
            command,
            GameplayCommandUpdatePolicy::DrainImmediately,
        )
    }

    pub fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        let total_start = Instant::now();
        let mut drain_updates_ms = 0.0;
        let mut apply_report = RuntimeUpdateApplyReport::default();
        let mut changed = true;

        if matches!(policy, GameplayCommandUpdatePolicy::DrainImmediately)
            && self.connection.pending_update_metrics()?.update_depth() > 0
        {
            let pump_report = pump_client_connection_updates_report(
                &mut self.core,
                &mut self.connection,
                RuntimeUpdatePumpBudget::unlimited(),
                ConnectionUpdateDrainMode::Blocking,
            )?;
            drain_updates_ms += pump_report.drain_updates_ms;
            changed |= pump_report.apply_report.changed;
            apply_report.accumulate(pump_report.apply_report);
        }

        let send_start = Instant::now();
        if let Err(error) = self.connection.send_command_only(command) {
            let resync_command = prepare_remote_dedicated_resync_command(&mut self.core, error)?;
            self.connection.clear_pending_updates();
            let changed = reconnect_remote_dedicated_session_and_resync(
                &mut self.core,
                self.connection.session_mut(),
                resync_command,
            )?;
            let total_ms = elapsed_ms(total_start.elapsed());
            return Ok((
                changed,
                GameplayCommandTiming {
                    total_ms,
                    send_ms: elapsed_ms(send_start.elapsed()),
                    ..GameplayCommandTiming::default()
                },
            ));
        }
        let send_ms = elapsed_ms(send_start.elapsed());

        match policy {
            GameplayCommandUpdatePolicy::DrainImmediately => {
                let pump_report = match pump_client_connection_updates_report(
                    &mut self.core,
                    &mut self.connection,
                    RuntimeUpdatePumpBudget::unlimited(),
                    ConnectionUpdateDrainMode::Blocking,
                ) {
                    Ok(pump_report) => pump_report,
                    Err(error) => {
                        let resync_command =
                            prepare_remote_dedicated_resync_command(&mut self.core, error)?;
                        self.connection.clear_pending_updates();
                        let changed = reconnect_remote_dedicated_session_and_resync(
                            &mut self.core,
                            self.connection.session_mut(),
                            resync_command,
                        )?;
                        let total_ms = elapsed_ms(total_start.elapsed());
                        return Ok((
                            changed,
                            GameplayCommandTiming {
                                total_ms,
                                send_ms,
                                drain_updates_ms,
                                ..GameplayCommandTiming::default()
                            },
                        ));
                    }
                };
                drain_updates_ms += pump_report.drain_updates_ms;
                changed |= pump_report.apply_report.changed;
                apply_report.accumulate(pump_report.apply_report);
            }
            GameplayCommandUpdatePolicy::SendOnly => {
                self.core.apply_exchange(deferred_command_exchange());
            }
        }

        Ok((
            changed,
            GameplayCommandTiming {
                total_ms: elapsed_ms(total_start.elapsed()),
                send_ms,
                drain_updates_ms,
                apply_updates_ms: apply_report.total_ms,
                apply_dirty_mark_ms: apply_report.dirty_mark_ms,
                apply_client_updates_ms: apply_report.client_apply_updates_ms,
                updates: apply_report.updates,
                snapshot_updates: apply_report.snapshot_updates,
                section_block_updates: apply_report.section_block_updates,
                unload_updates: apply_report.unload_updates,
            },
        ))
    }

    pub fn poll(&mut self) -> Result<bool> {
        self.poll_with_update_budget(RuntimeUpdatePumpBudget::default())
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        let poll_start = Instant::now();
        let mut polls = 0;
        while self.connection.pending_update_metrics()?.update_depth() > 0 {
            self.poll_with_update_budget_and_drain_mode(
                RuntimeUpdatePumpBudget::unlimited(),
                ConnectionUpdateDrainMode::Blocking,
            )?;
            polls += 1;
        }
        Ok((polls, elapsed_ms(poll_start.elapsed())))
    }

    pub fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool> {
        self.poll_with_update_budget_and_drain_mode(budget, ConnectionUpdateDrainMode::ReadyOnly)
    }

    fn poll_with_update_budget_and_drain_mode(
        &mut self,
        budget: RuntimeUpdatePumpBudget,
        drain_mode: ConnectionUpdateDrainMode,
    ) -> Result<bool> {
        let poll_start = Instant::now();
        let pump_report = pump_client_connection_updates_report(
            &mut self.core,
            &mut self.connection,
            budget,
            drain_mode,
        )?;
        let changed = pump_report.apply_report.changed;
        self.core.finish_poll_diagnostics(
            RuntimePollTiming {
                total_ms: elapsed_ms(poll_start.elapsed()),
                drain_updates_ms: pump_report.drain_updates_ms,
                producer_read_ms: pump_report.producer_read_ms,
                producer_decode_ms: pump_report.producer_decode_ms,
                producer_response_sequence: pump_report.producer_response_sequence,
                update_pump_stalled: pump_report.stalled,
                update_pump_stall_count: pump_report.stall_count,
                server_update_queue_depth: pump_report.remaining_queue_depth,
                server_update_queue_bytes: pump_report.remaining_queue_bytes,
                server_update_applied_bytes: pump_report.update_bytes,
                server_update_oldest_applied_age_ms: pump_report.oldest_applied_update_age_ms,
                ..RuntimePollTiming::default()
            },
            pump_report.apply_report,
            None,
        );
        Ok(changed)
    }

    pub fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                &mut self.render_compile_dispatcher,
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
                &mut self.render_compile_dispatcher,
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
                &mut self.render_compile_dispatcher,
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
                &mut self.render_compile_dispatcher,
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
                &mut self.render_compile_dispatcher,
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
                &mut self.render_compile_dispatcher,
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
            &mut self.render_compile_dispatcher,
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

    pub fn target_render_work_stats(&self, camera_position: Vec3) -> TargetRenderWorkStats {
        self.core.target_render_work_stats(camera_position)
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

impl<S> RemoteDedicatedSingleViewSceneRuntime<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        self.core
            .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_targeted_snapshots_timed(
                &mut self.render_compile_dispatcher,
                camera_position,
                deadline,
                max_compile_requests,
                completed_result_accept_budget,
            )
    }
}

pub fn build_local_single_view_client_runtime(
    options: LocalSingleViewSceneOptions,
) -> Result<ClientRuntime> {
    let mut runtime = SingleViewRuntime::local_integrated_with_seed(
        options.seed,
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
        .with_scheduled_fluid_ticks_frozen(options.freeze_scheduled_fluid_ticks)
        .with_local_integrated_chunk_tracking()
        .with_adaptive_chunk_publication_budget(options.adaptive_chunk_publication_budget)
        .with_world_storage(options.world_storage.clone())
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
    use std::collections::VecDeque;

    use super::*;
    use crate::render_assets::extracted_asset_root;

    #[derive(Debug)]
    enum NoRemoteSession {}

    impl RemoteDedicatedServerSession for NoRemoteSession {
        fn send_command_only(&mut self, _command: ClientCommand) -> Result<()> {
            match *self {}
        }

        fn drain_command_updates(&mut self) -> Result<Vec<ServerUpdate>> {
            match *self {}
        }

        fn reconnect(&mut self) -> Result<()> {
            match *self {}
        }
    }

    #[derive(Debug)]
    struct PollableRemoteSession {
        blocking_drains: VecDeque<Result<Vec<ServerUpdate>>>,
        ready_drains: VecDeque<Option<Result<Vec<ServerUpdate>>>>,
        send_count: usize,
        blocking_drain_count: usize,
        try_drain_count: usize,
        reconnect_count: usize,
    }

    impl PollableRemoteSession {
        fn new(
            blocking_drains: Vec<Result<Vec<ServerUpdate>>>,
            ready_drains: Vec<Option<Result<Vec<ServerUpdate>>>>,
        ) -> Self {
            Self {
                blocking_drains: blocking_drains.into(),
                ready_drains: ready_drains.into(),
                send_count: 0,
                blocking_drain_count: 0,
                try_drain_count: 0,
                reconnect_count: 0,
            }
        }
    }

    impl RemoteDedicatedServerSession for PollableRemoteSession {
        fn send_command_only(&mut self, _command: ClientCommand) -> Result<()> {
            self.send_count += 1;
            Ok(())
        }

        fn drain_command_updates(&mut self) -> Result<Vec<ServerUpdate>> {
            self.blocking_drain_count += 1;
            self.blocking_drains
                .pop_front()
                .unwrap_or_else(|| anyhow::bail!("scripted blocking drain missing response"))
        }

        fn try_drain_command_updates(&mut self) -> Result<Option<Vec<ServerUpdate>>> {
            self.try_drain_count += 1;
            match self.ready_drains.pop_front().unwrap_or(None) {
                Some(result) => result.map(Some),
                None => Ok(None),
            }
        }

        fn reconnect(&mut self) -> Result<()> {
            self.reconnect_count += 1;
            Ok(())
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
    fn local_send_only_command_defers_update_application_until_poll() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalSingleViewSceneRuntime::new(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();
        assert!(
            runtime
                .client()
                .chunk_snapshot(ChunkPos::new(0, 0))
                .is_some()
        );

        let next_center = ChunkPos::new(1, 0);
        let command = ClientCommand::SetChunkView(crate::chunk_view(
            next_center,
            0,
            chunk_tracking_radius_for_render_distance(0),
        ));
        let (_changed, timing) = runtime
            .send_gameplay_command_with_update_policy_timed(
                command,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .unwrap();
        assert_eq!(timing.drain_updates_ms, 0.0);
        assert_eq!(timing.updates, 0);
        assert!(runtime.client().chunk_snapshot(next_center).is_none());

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let diagnostics = runtime.server_runner_diagnostics().unwrap();
            if diagnostics.update_queue_depth > 0 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for deferred server update"
            );
            std::thread::sleep(Duration::from_millis(1));
        }

        assert!(runtime.poll().unwrap());
        assert!(runtime.client().chunk_snapshot(next_center).is_some());
    }

    #[test]
    fn local_set_chunk_view_defers_update_application_until_poll() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalSingleViewSceneRuntime::new(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();

        let next_center = ChunkPos::new(1, 0);
        assert!(
            runtime
                .set_chunk_view(next_center, 0, chunk_tracking_radius_for_render_distance(0))
                .unwrap()
        );
        assert!(runtime.client().chunk_snapshot(next_center).is_none());

        wait_for_update_queue_depth(&runtime, 1);
        assert!(
            runtime
                .poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())
                .unwrap()
        );
        assert!(runtime.client().chunk_snapshot(next_center).is_some());
    }

    #[test]
    fn local_update_pump_applies_one_update_when_budget_is_exhausted() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalSingleViewSceneRuntime::new(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();
        let old_center = ChunkPos::new(0, 0);
        let next_center = ChunkPos::new(1, 0);

        assert!(
            runtime
                .set_chunk_view(next_center, 0, chunk_tracking_radius_for_render_distance(0))
                .unwrap()
        );
        wait_for_update_queue_depth(&runtime, 2);

        assert!(
            runtime
                .poll_with_update_budget(RuntimeUpdatePumpBudget::MaxElapsed(Duration::ZERO))
                .unwrap()
        );
        let diagnostics = runtime.core().last_poll_diagnostics();
        assert_eq!(diagnostics.updates, 1);
        assert!(diagnostics.update_pump_stalled);
        assert_eq!(diagnostics.update_pump_stall_count, 1);
        assert!(diagnostics.server_update_queue_depth > 0);
        assert!(diagnostics.server_update_queue_bytes > 0);

        runtime.poll_until_idle().unwrap();
        assert!(runtime.client().chunk_snapshot(next_center).is_some());
        assert!(runtime.client().chunk_snapshot(old_center).is_none());
        assert_eq!(
            runtime
                .server_runner_diagnostics()
                .unwrap()
                .update_queue_depth,
            0
        );
    }

    #[test]
    fn remote_poll_skips_pending_response_until_ready() {
        if !extracted_asset_root().exists() {
            return;
        }

        let center = ChunkPos::new(0, 0);
        let mesh_assets = load_textured_mesh_assets().unwrap();
        let session = PollableRemoteSession::new(
            vec![Ok(Vec::new())],
            vec![
                None,
                Some(Ok(vec![ServerUpdate::TimeUpdate { day_time: 6000 }])),
            ],
        );
        let mut runtime = RemoteDedicatedSingleViewSceneRuntime::with_mesh_assets(
            SingleViewHostOptions::new(center, 0),
            session,
            mesh_assets,
        )
        .unwrap();

        assert_eq!(runtime.connection.pending_response_batches, 0);
        assert_eq!(runtime.connection.session.blocking_drain_count, 1);
        assert_eq!(runtime.core().day_time(), 0);

        let command = ClientCommand::SetChunkView(crate::chunk_view(
            ChunkPos::new(1, 0),
            0,
            chunk_tracking_radius_for_render_distance(0),
        ));
        let (_changed, timing) = runtime
            .send_gameplay_command_with_update_policy_timed(
                command,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .unwrap();

        assert_eq!(timing.drain_updates_ms, 0.0);
        assert_eq!(timing.updates, 0);
        assert_eq!(runtime.connection.pending_response_batches, 1);

        assert!(!runtime.poll().unwrap());
        let diagnostics = runtime.core().last_poll_diagnostics();
        assert_eq!(diagnostics.server_update_queue_depth, 1);
        assert_eq!(diagnostics.updates, 0);
        assert!(!diagnostics.update_pump_stalled);
        assert_eq!(runtime.connection.pending_response_batches, 1);
        assert_eq!(runtime.connection.session.try_drain_count, 1);
        assert_eq!(runtime.core().day_time(), 0);

        let _changed = runtime.poll().unwrap();
        let diagnostics = runtime.core().last_poll_diagnostics();
        assert_eq!(diagnostics.server_update_queue_depth, 0);
        assert_eq!(diagnostics.updates, 1);
        assert_eq!(runtime.connection.pending_response_batches, 0);
        assert_eq!(runtime.connection.session.try_drain_count, 2);
        assert_eq!(runtime.core().day_time(), 6000);
    }

    #[test]
    fn local_chunk_view_churn_attributes_unload_apply() {
        if !extracted_asset_root().exists() {
            return;
        }

        let render_distance = 1;
        let tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
        let mut runtime = LocalSingleViewSceneRuntime::new(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), render_distance)
                .with_lighting_enabled(false),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();
        let old_positions = runtime
            .client()
            .loaded_chunk_positions()
            .collect::<Vec<_>>();
        assert_eq!(
            old_positions.len(),
            9,
            "render-distance-1 initial view should load a 3x3 chunk window"
        );

        let next_center = ChunkPos::new(16, 0);
        assert!(
            runtime
                .set_chunk_view(next_center, render_distance, tracking_radius)
                .unwrap()
        );

        let deadline = Instant::now() + Duration::from_secs(30);
        let mut polls = 0_usize;
        let mut changed_polls = 0_usize;
        let mut snapshot_updates = 0_usize;
        let mut section_block_updates = 0_usize;
        let mut unload_updates = 0_usize;
        let mut other_updates = 0_usize;
        let mut mixed_updates = 0_usize;
        let mut unload_apply_ms = 0.0_f64;
        let mut unload_dirty_mark_ms = 0.0_f64;
        let mut unload_client_apply_ms = 0.0_f64;
        let mut max_unload_apply_ms = 0.0_f64;
        let mut max_unload_client_apply_ms = 0.0_f64;
        let mut deferred_chunk_drop_items = 0_usize;
        let mut deferred_chunk_drop_ms = 0.0_f64;
        let mut max_deferred_chunk_drop_ms = 0.0_f64;
        let mut max_deferred_chunk_drop_backlog_items = 0_usize;
        let mut max_queue_depth_before_poll = 0_usize;

        loop {
            let runner_diagnostics_before = runtime.server_runner_diagnostics().unwrap();
            max_queue_depth_before_poll =
                max_queue_depth_before_poll.max(runner_diagnostics_before.update_queue_depth);
            if polls > 0
                && runner_idle(&runner_diagnostics_before)
                && runtime.deferred_client_chunk_drop_backlog_items() == 0
            {
                break;
            }

            let changed = runtime
                .poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())
                .unwrap();
            polls += 1;
            if changed {
                changed_polls += 1;
            }
            let diagnostics = runtime.core().last_poll_diagnostics();
            snapshot_updates += diagnostics.snapshot_updates;
            section_block_updates += diagnostics.section_block_updates;
            unload_updates += diagnostics.unload_updates;
            other_updates += diagnostics.other_updates;
            mixed_updates += diagnostics.mixed_updates;
            unload_apply_ms += diagnostics.unload_update_apply_ms;
            unload_dirty_mark_ms += diagnostics.unload_update_dirty_mark_ms;
            unload_client_apply_ms += diagnostics.unload_update_client_apply_ms;
            max_unload_apply_ms = max_unload_apply_ms.max(diagnostics.unload_update_apply_ms);
            max_unload_client_apply_ms =
                max_unload_client_apply_ms.max(diagnostics.unload_update_client_apply_ms);
            deferred_chunk_drop_items += diagnostics.client_deferred_chunk_drop_items;
            deferred_chunk_drop_ms += diagnostics.client_deferred_chunk_drop_ms;
            max_deferred_chunk_drop_ms =
                max_deferred_chunk_drop_ms.max(diagnostics.client_deferred_chunk_drop_ms);
            max_deferred_chunk_drop_backlog_items = max_deferred_chunk_drop_backlog_items
                .max(diagnostics.client_deferred_chunk_drop_backlog_items);

            let runner_diagnostics = runtime.server_runner_diagnostics().unwrap();
            assert!(
                Instant::now() < deadline,
                "timed out waiting for churned chunk view to settle; runner={runner_diagnostics:?}"
            );
            if runner_diagnostics.update_queue_depth == 0
                && runtime.deferred_client_chunk_drop_backlog_items() == 0
            {
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        assert!(changed_polls > 0, "view churn should apply visible updates");
        assert!(
            snapshot_updates >= old_positions.len(),
            "far view jump should queue at least one new snapshot per old visible chunk"
        );
        assert!(
            unload_updates >= old_positions.len(),
            "far view jump should queue at least one unload per old visible chunk"
        );
        assert_eq!(mixed_updates, 0);
        assert_eq!(runtime.loaded_chunk_count(), old_positions.len());
        assert!(runtime.client().chunk_snapshot(next_center).is_some());
        assert!(
            old_positions
                .iter()
                .all(|pos| runtime.client().chunk_snapshot(*pos).is_none()),
            "far view jump should fully unload the old chunk window"
        );
        assert!(
            unload_apply_ms > 0.0,
            "unload updates should populate the unload apply timing category"
        );
        assert!(
            unload_client_apply_ms > 0.0,
            "unload updates should populate client-replica apply timing"
        );
        assert!(
            deferred_chunk_drop_items >= old_positions.len(),
            "old chunk snapshot payload items should be deferred and drained after unload apply"
        );
        assert_eq!(runtime.deferred_client_chunk_drop_backlog_items(), 0);
        eprintln!(
            "local_chunk_view_churn_attributes_unload_apply polls={polls} changed_polls={changed_polls} snapshots={snapshot_updates} section_updates={section_block_updates} unloads={unload_updates} other={other_updates} unload_apply_ms={unload_apply_ms:.3} unload_dirty_ms={unload_dirty_mark_ms:.3} unload_client_ms={unload_client_apply_ms:.3} max_unload_apply_ms={max_unload_apply_ms:.3} max_unload_client_ms={max_unload_client_apply_ms:.3} deferred_chunk_drop_items={deferred_chunk_drop_items} deferred_chunk_drop_ms={deferred_chunk_drop_ms:.3} max_deferred_chunk_drop_ms={max_deferred_chunk_drop_ms:.3} max_deferred_chunk_drop_backlog_items={max_deferred_chunk_drop_backlog_items} max_queue_depth_before_poll={max_queue_depth_before_poll}"
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

    fn wait_for_update_queue_depth(runtime: &LocalSingleViewSceneRuntime, min_depth: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let diagnostics = runtime.server_runner_diagnostics().unwrap();
            if diagnostics.update_queue_depth >= min_depth {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for update queue depth {min_depth}; diagnostics={diagnostics:?}"
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
    fn high_render_distance_startup_pump_reaches_playable_before_full_view_settles() {
        if !extracted_asset_root().exists() {
            return;
        }

        let render_distance = 30;
        let tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
        let full_view_chunk_count = {
            let side = tracking_radius as usize * 2 + 1;
            side * side
        };
        let playable_gate_chunk_count = 9;
        let mesh_assets = load_textured_mesh_assets().unwrap();
        let mut pump = LocalSingleViewStartupPump::with_mesh_assets(
            LocalSingleViewSceneOptions::new(12345, ChunkPos::new(0, 0), render_distance)
                .with_lighting_enabled(false),
            mesh_assets,
        )
        .unwrap();
        let camera_position = Vec3::new(8.0, 80.0, 8.0);
        let deadline = Instant::now() + Duration::from_secs(30);

        loop {
            let step = pump.step(camera_position).unwrap();
            if step.playable_ready {
                let startup_progress = step.progress.as_ref().unwrap();
                assert_eq!(startup_progress.display_radius, tracking_radius);
                assert_eq!(
                    startup_progress.target_chunk_count,
                    playable_gate_chunk_count
                );
                assert_eq!(
                    startup_progress.target_ready_chunks,
                    playable_gate_chunk_count
                );
                assert_eq!(startup_progress.percent(), 100);
                assert!(startup_progress.playable_ready);

                let view_progress = pump
                    .runtime
                    .last_runner_diagnostics
                    .as_ref()
                    .and_then(view_readiness_overlay_from_diagnostics)
                    .unwrap();
                assert_eq!(view_progress.display_radius, tracking_radius);
                assert_eq!(view_progress.target_chunk_count, full_view_chunk_count);
                assert!(
                    view_progress.target_ready_chunks < view_progress.target_chunk_count,
                    "RD30 startup should not wait for the full view to settle; view_progress={view_progress:?}"
                );
                assert!(view_progress.playable_ready);

                assert!(step.cached_section_count > 0);
                let runtime = pump.into_runtime();
                let loaded_chunk_count = runtime.loaded_chunk_count();
                assert!(
                    loaded_chunk_count < full_view_chunk_count,
                    "RD30 startup should enter before loading all tracked chunks; loaded_chunk_count={loaded_chunk_count} full_view_chunk_count={full_view_chunk_count}"
                );
                assert!(
                    runtime
                        .client()
                        .chunk_snapshot(ChunkPos::new(0, 0))
                        .is_some()
                );
                return;
            }
            assert!(
                Instant::now() < deadline,
                "RD30 startup pump did not reach playable before deadline; last_step={step:?}"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
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
                session: ActiveSessionDescriptor::new_seed_local_world(12345)
            }
        );
        assert_eq!(
            runtime.active_session(),
            Some(&ActiveSessionDescriptor::new_seed_local_world(12345))
        );
        assert_eq!(runtime.session_status(), None);
        assert_eq!(runtime.loaded_chunk_count(), 1);
    }

    #[test]
    fn native_single_view_session_runtime_local_interest_change_defers_updates_until_poll() {
        if !extracted_asset_root().exists() {
            return;
        }

        let initial_center = ChunkPos::new(0, 0);
        let next_center = ChunkPos::new(1, 0);
        let mut runtime = NativeSingleViewSessionRuntime::<NoRemoteSession>::local(
            LocalSingleViewSceneOptions::new(12345, initial_center, 0).with_lighting_enabled(false),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();
        assert!(runtime.client().chunk_snapshot(initial_center).is_some());
        assert!(runtime.client().chunk_snapshot(next_center).is_none());

        assert!(runtime.set_interest_center(next_center).unwrap());
        assert_eq!(runtime.stats().interest_center, next_center);
        assert!(runtime.client().chunk_snapshot(initial_center).is_some());
        assert!(runtime.client().chunk_snapshot(next_center).is_none());

        runtime.poll_until_idle().unwrap();
        assert!(runtime.client().chunk_snapshot(initial_center).is_none());
        assert!(runtime.client().chunk_snapshot(next_center).is_some());
    }
}
