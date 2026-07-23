//! Native scene-service construction and concrete adapters.
//!
//! Shared session identity and the host-facing API live in
//! `scene_session_runtime`. This module owns native runners, compile workers,
//! filesystem storage, the bounded drop thread, and the type-erased adapter
//! that installs them behind that shell.

use std::collections::{BTreeSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::ClientRuntime;
use mclone_core::HorizontalTopology;
use mclone_core::{BlockStateId, ChunkPos, ChunkSnapshot, LodTileKey};
use mclone_mesh::{
    RenderSectionKey, TexturedRenderSectionBuildReport, TexturedRenderSectionMesh,
    TexturedRenderSectionMetadata,
};
use mclone_protocol::{ClientCommand, ClientEphemeralMessage, ServerUpdate, encode_server_update};
use mclone_render::far_lod::FarTerrainLodFrameUpdate;
use mclone_render_session::{RenderSectionCacheUpdate, RenderSectionCompileQueueHealth};
use mclone_server::{
    DEFAULT_LIGHT_STATUS_BATCH_SIZE, IntegratedServerRunner, NativeIntegratedServerRunner,
    NativeIntegratedServerRunnerConfig, NativeIntegratedServerWorldStorage,
    ServerRunnerDiagnostics, SimulationCadenceConfig, WorldBehaviorProfile, WorldGenerationProfile,
    host_tick_interval_for_rate_hz, initial_spawn_center_for_descriptor,
};
use mclone_ui::LoadingProgressOverlay;

use crate::catalog_executor::WorldCatalogOperationService;
use crate::client_connection::{
    ClientConnection, ClientConnectionDrainResult, ClientConnectionQueueMetrics,
    IntegratedRunnerConnection, QueuedServerUpdate, pump_client_connection_updates_report,
};
use crate::deferred_drop::{
    DEFAULT_DEFERRED_DROP_MAX_ITEMS, DeferredDropBacklog, DeferredDropService,
};
use crate::far_lod::{
    FarTerrainLodCache, FarTerrainLodCompiler, FarTerrainLodConfig, FarTerrainLodCoverage,
    FarTerrainLodProducerStats, STARTUP_LOD_PREWARM_CHUNK_BUILD_BUDGET, StartupLodPrewarmConfig,
};
use crate::host_mode::{
    RemoteDedicatedServerSession, RemoteServerUpdateBatch, RemoteUpdateQueueMetrics,
    SingleViewHostMode, SingleViewHostOptions, deferred_command_exchange,
    prepare_remote_dedicated_resync_command, reconnect_remote_dedicated_session_and_resync,
};
use crate::lod_coverage::{LodCoverageCoordinator, LodReplacementCounters, LodTileAvailability};
use crate::monotonic::{MonotonicDeadline, system_monotonic_clock};
use crate::render_asset_data::TexturedMeshAssets;
use crate::render_assets::{
    DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS, DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
    NativeRenderSectionCompileDispatcher, load_textured_mesh_assets,
};
use crate::scene_session_runtime::{
    DEFAULT_STARTUP_READINESS_TIMEOUT, FarLodRuntimeSettleSnapshot, SceneRuntimeService,
    SceneSessionRuntime, StartupReadinessPolicy,
};
use crate::session::{ActiveSessionDescriptor, RemoteSessionEndpoint, SessionStartRequest};
use crate::startup_render_seed::StartupRenderSectionSeed;
use crate::world_catalog::NativeWorldCatalog;
use crate::{
    DEFAULT_CLIENT_DEFERRED_CHUNK_DROP_ITEM_BUDGET, DEFAULT_RENDER_CHUNK_MESH_BUDGET,
    GameplayCommandSubmission, GameplayCommandTiming, GameplayCommandUpdatePolicy,
    RuntimePollDiagnostics, RuntimePollTiming, RuntimeUpdateApplyReport, RuntimeUpdatePumpBudget,
    SingleViewRuntime, SingleViewRuntimeStats, TargetRenderWorkStats,
    TimedRenderSectionCacheUpdate, chunk_tracking_radius_for_render_distance, elapsed_ms,
    loading_progress_overlay_from_diagnostics, view_readiness_overlay_from_diagnostics,
};

const RUNTIME_DIAGNOSTICS_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_LOCAL_INTEGRATED_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// Bound on startup camera/interest reconciliation re-pump passes
/// (docs/tactical/167 Slice 4). One correction/view-pose settles in a single
/// pass; the bound only guards a pathological correction cascade.
const MAX_STARTUP_RECONCILE_PASSES: usize = 4;

pub fn native_world_catalog_operations(root: PathBuf) -> WorldCatalogOperationService {
    WorldCatalogOperationService::immediate(Box::new(NativeWorldCatalog::new(root)))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalIntegratedSceneOptions {
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub world_topology: HorizontalTopology,
    pub world_behavior_profile: WorldBehaviorProfile,
    pub center: ChunkPos,
    pub render_distance: u32,
    pub cadence: SimulationCadenceConfig,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub freeze_scheduled_fluid_ticks: bool,
    pub debug_passive_showcase: bool,
    pub debug_auxiliary_player_script: bool,
    pub lighting_enabled: bool,
    pub light_status_batch_size: usize,
    pub adaptive_chunk_publication_budget: bool,
    pub world_storage: NativeIntegratedServerWorldStorage,
    pub render_compile_worker_count: usize,
    pub render_compile_max_pending_jobs: Option<usize>,
    pub render_compile_worker_timing_enabled: bool,
    pub startup_lod_prewarm: StartupLodPrewarmConfig,
    pub local_player_identity: Option<mclone_protocol::ClientIdentity>,
    pub observer_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegratedWorldSessionStorage {
    persistent_world_dir: Option<PathBuf>,
    adaptive_chunk_publication_budget: bool,
}

impl Default for IntegratedWorldSessionStorage {
    fn default() -> Self {
        Self::transient()
    }
}

impl IntegratedWorldSessionStorage {
    pub const fn transient() -> Self {
        Self {
            persistent_world_dir: None,
            adaptive_chunk_publication_budget: false,
        }
    }

    pub fn from_world_dir(world_dir: Option<&Path>) -> Self {
        let mut storage = Self::transient();
        if let Some(world_dir) = world_dir {
            storage.persistent_world_dir = Some(world_dir.to_path_buf());
        }
        storage
    }

    pub fn with_persistent_world_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.persistent_world_dir = Some(dir.into());
        self
    }

    pub const fn with_adaptive_chunk_publication_budget(mut self, enabled: bool) -> Self {
        self.adaptive_chunk_publication_budget = enabled;
        self
    }

    pub fn persistent_world_dir(&self) -> Option<&Path> {
        self.persistent_world_dir.as_deref()
    }

    pub const fn adaptive_chunk_publication_budget(&self) -> bool {
        self.adaptive_chunk_publication_budget
    }
}

impl LocalIntegratedSceneOptions {
    pub const fn new(seed: i64, center: ChunkPos, render_distance: u32) -> Self {
        Self {
            seed,
            world_generation_profile: WorldGenerationProfile::Overworld,
            world_topology: HorizontalTopology::UNBOUNDED,
            world_behavior_profile: WorldBehaviorProfile::Mutable,
            center,
            render_distance,
            cadence: SimulationCadenceConfig::new(20, 20, 60),
            day_time_override: None,
            freeze_time: false,
            freeze_scheduled_fluid_ticks: false,
            debug_passive_showcase: true,
            debug_auxiliary_player_script: false,
            lighting_enabled: true,
            light_status_batch_size: DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            adaptive_chunk_publication_budget: false,
            world_storage: NativeIntegratedServerWorldStorage::Transient,
            render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            render_compile_max_pending_jobs: Some(DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS),
            render_compile_worker_timing_enabled: true,
            startup_lod_prewarm: StartupLodPrewarmConfig::disabled(),
            local_player_identity: None,
            observer_only: false,
        }
    }

    pub const fn with_cadence(mut self, cadence: SimulationCadenceConfig) -> Self {
        self.cadence = cadence;
        self
    }

    pub const fn with_world_generation_profile(mut self, profile: WorldGenerationProfile) -> Self {
        self.world_generation_profile = profile;
        self
    }

    pub const fn with_world_topology(mut self, topology: HorizontalTopology) -> Self {
        self.world_topology = topology;
        self
    }

    pub const fn with_world_behavior_profile(mut self, profile: WorldBehaviorProfile) -> Self {
        self.world_behavior_profile = profile;
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

    pub const fn with_debug_auxiliary_player_script(mut self, enabled: bool) -> Self {
        self.debug_auxiliary_player_script = enabled;
        self
    }

    pub fn with_local_player_identity(mut self, identity: mclone_protocol::ClientIdentity) -> Self {
        self.local_player_identity = Some(identity);
        self
    }

    pub const fn with_observer_only(mut self, observer_only: bool) -> Self {
        self.observer_only = observer_only;
        self
    }

    pub fn with_initial_spawn_center(mut self) -> Self {
        self.center = initial_spawn_center_for_descriptor(
            self.seed,
            self.world_generation_profile,
            self.world_topology,
        );
        self
    }

    pub const fn with_lighting_enabled(mut self, lighting_enabled: bool) -> Self {
        self.lighting_enabled = lighting_enabled;
        self
    }

    pub const fn with_light_status_batch_size(mut self, light_status_batch_size: usize) -> Self {
        self.light_status_batch_size = if light_status_batch_size == 0 {
            1
        } else {
            light_status_batch_size
        };
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

    pub fn with_integrated_world_session_storage(
        mut self,
        storage: IntegratedWorldSessionStorage,
    ) -> Self {
        self.adaptive_chunk_publication_budget = storage.adaptive_chunk_publication_budget;
        self.world_storage = storage
            .persistent_world_dir
            .map_or(NativeIntegratedServerWorldStorage::Transient, |dir| {
                NativeIntegratedServerWorldStorage::Persistent { dir }
            });
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

    pub const fn with_startup_lod_prewarm(mut self, prewarm: StartupLodPrewarmConfig) -> Self {
        self.startup_lod_prewarm = prewarm;
        self
    }

    pub fn chunk_tracking_radius(&self) -> u32 {
        chunk_tracking_radius_for_render_distance(self.render_distance)
    }
}

/// Local-integrated scene host, generic over the integrated-server runner
/// (docs/tactical/168 Slice 2). `R` defaults to [`NativeIntegratedServerRunner`]
/// so every existing native caller writes `LocalIntegratedSceneRuntime`
/// unchanged; the parameter only removes the hard-coded-runner seam that would
/// otherwise permanently fork web out of the shared local host mode.
#[derive(Debug)]
pub struct LocalIntegratedSceneRuntime<R = NativeIntegratedServerRunner> {
    core: SingleViewRuntime,
    connection: IntegratedRunnerConnection<R>,
    mesh_assets: TexturedMeshAssets,
    render_compile_dispatcher: NativeRenderSectionCompileDispatcher,
    far_lod_cache: FarTerrainLodCache,
    lod_coverage: LodCoverageCoordinator,
    deferred_chunk_drops: Box<dyn DeferredDropService>,
    simulation_cadence: SimulationCadenceConfig,
    last_runner_diagnostics: Option<ServerRunnerDiagnostics>,
    last_runner_diagnostics_poll_at: Option<Instant>,
}

struct NativeDeferredDropService {
    sender: Option<mpsc::SyncSender<(ChunkSnapshot, usize)>>,
    pending_items: Arc<AtomicUsize>,
    inline_fallback_items: usize,
    max_items: usize,
    join_handle: Option<JoinHandle<()>>,
}

impl NativeDeferredDropService {
    fn new() -> Result<Self> {
        // Capacity is bounded both by messages and item accounting. Slice 0
        // observed <=295 pending items; 64 messages and 4096 items leave ample
        // native headroom without allowing unbounded frame-to-worker growth.
        let (sender, receiver) = mpsc::sync_channel::<(ChunkSnapshot, usize)>(64);
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
            inline_fallback_items: 0,
            max_items: DEFAULT_DEFERRED_DROP_MAX_ITEMS,
            join_handle: Some(join_handle),
        })
    }
}

impl DeferredDropService for NativeDeferredDropService {
    fn enqueue(&mut self, snapshot: ChunkSnapshot, item_count: usize) -> bool {
        if item_count == 0 {
            return true;
        }
        let reserved = self
            .pending_items
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                pending
                    .checked_add(item_count)
                    .filter(|next| *next <= self.max_items)
            })
            .is_ok();
        if !reserved {
            self.inline_fallback_items = self.inline_fallback_items.saturating_add(item_count);
            drop(snapshot);
            return false;
        }
        let Some(sender) = &self.sender else {
            drop(snapshot);
            self.pending_items.fetch_sub(item_count, Ordering::AcqRel);
            self.inline_fallback_items = self.inline_fallback_items.saturating_add(item_count);
            return false;
        };
        match sender.try_send((snapshot, item_count)) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full((snapshot, item_count)))
            | Err(mpsc::TrySendError::Disconnected((snapshot, item_count))) => {
                drop(snapshot);
                self.pending_items.fetch_sub(item_count, Ordering::AcqRel);
                self.inline_fallback_items = self.inline_fallback_items.saturating_add(item_count);
                false
            }
        }
    }

    fn backlog(&self) -> DeferredDropBacklog {
        DeferredDropBacklog {
            pending_items: self.pending_items.load(Ordering::Acquire),
            max_items: self.max_items,
            inline_fallback_items: self.inline_fallback_items,
        }
    }

    fn drain(&mut self, _item_budget: usize) -> usize {
        0
    }
}

impl Drop for NativeDeferredDropService {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

impl std::fmt::Debug for NativeDeferredDropService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeDeferredDropService")
            .field("backlog", &self.backlog())
            .finish_non_exhaustive()
    }
}

/// Startup readiness gate shared by every native startup lane (docs/tactical/167).
///
/// `Playable` is the default for desktop, Android, and XR: startup differences
/// must come from host-mode evidence (`LocalIntegrated` vs `RemoteDedicated`),
/// not from per-platform threshold forks. `Idle` is a named diagnostics /
/// screenshot / test option that additionally waits for the active view to have
/// no pending render work; it must never be the hidden reason a platform startup
/// behaves differently.
/// Uninhabited [`RemoteDedicatedServerSession`] used to parameterize the shared
/// startup pump for local-integrated startup, where no remote session exists.
///
/// It lets [`NativeSessionStartupPump`] own a `NativeSessionServices<S>` for both
/// host modes while the local convenience constructors and the
/// [`LocalIntegratedStartupPump`] wrapper stay session-type-free.
#[derive(Debug)]
pub enum LocalOnlySession {}

impl RemoteDedicatedServerSession for LocalOnlySession {
    fn send_command_only(&mut self, _command: ClientCommand) -> Result<()> {
        match *self {}
    }

    fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>> {
        match *self {}
    }

    fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics {
        match *self {}
    }

    fn reconnect(&mut self) -> Result<()> {
        match *self {}
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalIntegratedStartupStep {
    pub poll_count: usize,
    pub poll_ms: f64,
    pub changed: bool,
    /// Combined gate the driver waits on: spawn authority plus prewarm settled.
    pub playable_ready: bool,
    /// Spawn-authority readiness: underfoot/near real chunks are honest. This is
    /// the only gate that gameplay honesty depends on; prewarm never sets it.
    pub spawn_authority_ready: bool,
    /// Whether startup LOD prewarm is active this session.
    pub lod_prewarm_enabled: bool,
    /// Prewarm reached its desired (tile-capped) visual coverage.
    pub lod_prewarm_complete: bool,
    /// Prewarm hit its time cap before completing coverage.
    pub lod_prewarm_timeout: bool,
    /// Wall-clock spent on prewarm so far (`lod_prewarm_ms`).
    pub lod_prewarm_ms: f64,
    /// Retained prewarm tiles drawable so far (`startup_lod_tiles_ready`).
    pub startup_lod_tiles_ready: usize,
    /// Desired prewarm tiles, tile-capped (`startup_lod_tiles_target`).
    pub startup_lod_tiles_target: usize,
    pub cached_section_count: usize,
    pub rebuilt_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub completed_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    /// Sections accumulated in the startup render seed so far (docs/tactical/167).
    pub render_seed_section_count: usize,
    /// Startup render-seed sections carrying drawable geometry (docs/tactical/167).
    pub render_seed_drawable_section_count: usize,
    pub progress: Option<LoadingProgressOverlay>,
}

/// Startup visual-coverage prewarm tracker (tactical 162 Slice 1).
///
/// Runs alongside spawn-authority loading, building cheap retained far-LOD
/// coverage around the spawn center so the first playable frame has less blank
/// space. It is bounded by a hard time cap and tile cap, and never contributes
/// to the spawn-authority gate. When it times out, startup degrades to current
/// behavior (playable as soon as spawn authority is ready).
#[derive(Clone, Copy, Debug)]
struct StartupLodPrewarm {
    config: StartupLodPrewarmConfig,
    far_lod: FarTerrainLodConfig,
    seed: i64,
    generation_profile: WorldGenerationProfile,
    center: ChunkPos,
    started_at: Option<Instant>,
    elapsed_ms: f64,
    tiles_ready: usize,
    tiles_target: usize,
    complete: bool,
    timed_out: bool,
}

impl StartupLodPrewarm {
    fn new(
        config: StartupLodPrewarmConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
    ) -> Self {
        Self {
            config,
            far_lod: config.far_lod_config(),
            seed,
            generation_profile,
            center,
            started_at: None,
            elapsed_ms: 0.0,
            tiles_ready: 0,
            tiles_target: 0,
            complete: false,
            timed_out: false,
        }
    }

    const fn enabled(&self) -> bool {
        self.config.enabled
    }

    /// Whether the prewarm no longer blocks the playable transition: disabled,
    /// coverage complete, or the time cap has been hit.
    const fn settled(&self) -> bool {
        !self.config.enabled || self.complete || self.timed_out
    }

    fn advance(&mut self, runtime: &mut LocalIntegratedSceneRuntime, camera_position: Vec3) {
        if self.settled() {
            return;
        }
        let started = *self.started_at.get_or_insert_with(Instant::now);
        let budget = STARTUP_LOD_PREWARM_CHUNK_BUILD_BUDGET;
        let coverage = runtime.prewarm_far_lod(
            self.far_lod,
            self.seed,
            self.generation_profile,
            self.center,
            camera_position,
            budget,
        );
        self.record(coverage, started.elapsed());
    }

    fn record(&mut self, coverage: FarTerrainLodCoverage, elapsed: Duration) {
        let effective_target = coverage.target_tiles.min(self.config.tile_cap);
        self.tiles_target = effective_target;
        self.tiles_ready = coverage.ready_tiles.min(effective_target);
        self.elapsed_ms = elapsed_ms(elapsed);
        self.complete = self.tiles_ready >= effective_target;
        self.timed_out = !self.complete && elapsed >= self.config.time_cap;
    }
}

/// One shared startup step over a [`NativeSessionServices`] (docs/tactical/167).
///
/// Host-neutral: `local_progress` and the `lod_prewarm_*` / `spawn_authority_*`
/// fields carry local-integrated diagnostics and stay `None`/zeroed for remote
/// dedicated startup. `startup_ready` is the one gate every lane waits on under
/// the shared [`StartupReadinessPolicy`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeSessionStartupStep {
    pub host_mode: SingleViewHostMode,
    pub poll_count: usize,
    pub poll_ms: f64,
    pub changed: bool,
    /// Combined gate the driver waits on for the active [`StartupReadinessPolicy`]:
    /// host-mode evidence plus a drawable render seed plus prewarm settled.
    pub startup_ready: bool,
    /// Host-mode readiness evidence alone (local spawn authority / remote drained
    /// active view), before the render-seed and prewarm gates are applied.
    pub host_ready: bool,
    /// Whether startup LOD prewarm is active this session (local only).
    pub lod_prewarm_enabled: bool,
    /// Prewarm reached its desired (tile-capped) visual coverage.
    pub lod_prewarm_complete: bool,
    /// Prewarm hit its time cap before completing coverage.
    pub lod_prewarm_timeout: bool,
    /// Wall-clock spent on prewarm so far.
    pub lod_prewarm_ms: f64,
    /// Retained prewarm tiles drawable so far.
    pub startup_lod_tiles_ready: usize,
    /// Desired prewarm tiles, tile-capped.
    pub startup_lod_tiles_target: usize,
    pub cached_section_count: usize,
    pub rebuilt_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub accepted_compile_result_count: usize,
    pub completed_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    /// Sections accumulated in the startup render seed so far (docs/tactical/167).
    pub render_seed_section_count: usize,
    /// Startup render-seed sections carrying drawable geometry (docs/tactical/167).
    pub render_seed_drawable_section_count: usize,
    /// Local-integrated loading-progress overlay; `None` for remote dedicated.
    pub local_progress: Option<LoadingProgressOverlay>,
}

impl NativeSessionStartupStep {
    /// Project the host-neutral step onto the local-integrated startup step shape
    /// consumed by the desktop/XR/perf local pump wrapper. Keeps those callers on
    /// a single startup implementation during the tactical 167 migration.
    fn into_local_integrated(self) -> LocalIntegratedStartupStep {
        LocalIntegratedStartupStep {
            poll_count: self.poll_count,
            poll_ms: self.poll_ms,
            changed: self.changed,
            playable_ready: self.startup_ready,
            spawn_authority_ready: self.host_ready,
            lod_prewarm_enabled: self.lod_prewarm_enabled,
            lod_prewarm_complete: self.lod_prewarm_complete,
            lod_prewarm_timeout: self.lod_prewarm_timeout,
            lod_prewarm_ms: self.lod_prewarm_ms,
            startup_lod_tiles_ready: self.startup_lod_tiles_ready,
            startup_lod_tiles_target: self.startup_lod_tiles_target,
            cached_section_count: self.cached_section_count,
            rebuilt_section_count: self.rebuilt_section_count,
            submitted_compile_section_count: self.submitted_compile_section_count,
            completed_compile_section_count: self.completed_compile_section_count,
            pending_compile_jobs: self.pending_compile_jobs,
            render_seed_section_count: self.render_seed_section_count,
            render_seed_drawable_section_count: self.render_seed_drawable_section_count,
            progress: self.local_progress,
        }
    }
}

/// Host-neutral native world startup pump (docs/tactical/167).
///
/// Owns a [`NativeSessionServices`] plus the transient render-section seed, the
/// startup LOD prewarm policy (local-integrated only), the poll/sync/compile-job
/// release loop, and the shared readiness gate. Both local integrated and remote
/// dedicated startup reach ready through this one contract: remote never falls
/// back to `poll_until_idle` + `sync_all_render_sections`.
#[derive(Debug)]
pub struct NativeSessionStartupPump<S> {
    runtime: NativeSessionServices<S>,
    // Local-integrated startup LOD prewarm; remote pumps carry a disabled prewarm
    // that is always settled so the shared gate collapses to host evidence + seed.
    prewarm: StartupLodPrewarm,
    readiness: StartupReadinessPolicy,
    // docs/tactical/167: accumulate the transient CPU section meshes the pump
    // compiles so the platform adapter can seed draw resources at completion
    // without a redundant mark-all-dirty recompile (docs/tactical/163).
    render_seed: StartupRenderSectionSeed,
    poll_count: usize,
    poll_ms: f64,
    last_step: NativeSessionStartupStep,
}

/// Consumed startup result: the ready runtime plus the transient render seed for
/// a one-shot platform draw-resource upload, and the final startup step.
#[derive(Debug)]
pub struct NativeSessionStartupCompletion<S> {
    pub runtime: NativeSessionServices<S>,
    pub startup_sections: Vec<TexturedRenderSectionMesh>,
    pub final_step: NativeSessionStartupStep,
}

impl<S> NativeSessionStartupPump<S>
where
    S: RemoteDedicatedServerSession,
{
    /// Construct a local-integrated startup pump. Generic over `S` so desktop
    /// (`RemoteServerSession`), XR, and Android can share one pump type across
    /// both host modes; local-only callers infer `S = LocalOnlySession`
    /// (docs/tactical/167 Slice 3).
    pub fn local(options: LocalIntegratedSceneOptions) -> Result<Self> {
        let mesh_assets = load_textured_mesh_assets()?;
        Self::local_with_mesh_assets(options, mesh_assets)
    }

    pub fn local_with_mesh_assets(
        options: LocalIntegratedSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let prewarm = StartupLodPrewarm::new(
            options.startup_lod_prewarm,
            options.seed,
            options.world_generation_profile,
            options.center,
        );
        let runtime = NativeSessionServices::local_with_mesh_assets(options, mesh_assets)?;
        Ok(Self::from_session_runtime(
            runtime,
            StartupReadinessPolicy::Playable,
            prewarm,
        ))
    }

    pub fn remote_dedicated(
        endpoint: RemoteSessionEndpoint,
        options: SingleViewHostOptions,
        session: S,
    ) -> Result<Self> {
        let mesh_assets = load_textured_mesh_assets()?;
        Self::remote_dedicated_with_mesh_assets(endpoint, options, session, mesh_assets)
    }

    pub fn remote_dedicated_with_mesh_assets(
        endpoint: RemoteSessionEndpoint,
        options: SingleViewHostOptions,
        session: S,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let center = options.center;
        let runtime = NativeSessionServices::remote_dedicated_with_mesh_assets(
            endpoint,
            options,
            session,
            mesh_assets,
        )?;
        // Remote startup carries a disabled prewarm: startup LOD prewarm is a
        // local-integrated policy only (docs/tactical/167).
        let prewarm = StartupLodPrewarm::new(
            StartupLodPrewarmConfig::disabled(),
            0,
            WorldGenerationProfile::Overworld,
            center,
        );
        Ok(Self::from_session_runtime(
            runtime,
            StartupReadinessPolicy::Playable,
            prewarm,
        ))
    }

    /// Wrap an already-constructed session runtime. Prewarm should be disabled for
    /// remote runtimes; for local runtimes pass a prewarm built from the local
    /// options so startup coverage matches the direct constructors.
    fn from_session_runtime(
        runtime: NativeSessionServices<S>,
        readiness: StartupReadinessPolicy,
        prewarm: StartupLodPrewarm,
    ) -> Self {
        let host_mode = runtime.host_mode();
        Self {
            runtime,
            prewarm,
            readiness,
            render_seed: StartupRenderSectionSeed::new(),
            poll_count: 0,
            poll_ms: 0.0,
            last_step: NativeSessionStartupStep {
                host_mode,
                ..NativeSessionStartupStep::default()
            },
        }
    }

    /// Wrap an already-constructed session runtime for a synchronous startup
    /// drive (docs/tactical/167 Slice 3: XR remote/replacement, Android remote,
    /// and any lane that builds the runtime up front). Startup LOD prewarm is
    /// disabled — it is a local-first-construction policy owned by the `local*`
    /// constructors, so wrapping an existing runtime preserves the pre-tactical
    /// `poll_until_idle` behavior (no prewarm) for these lanes while still routing
    /// them through the shared readiness gate and render seed.
    pub fn from_runtime(runtime: NativeSessionServices<S>) -> Self {
        let center = runtime.interest_center();
        let prewarm = StartupLodPrewarm::new(
            StartupLodPrewarmConfig::disabled(),
            0,
            WorldGenerationProfile::Overworld,
            center,
        );
        Self::from_session_runtime(runtime, StartupReadinessPolicy::Playable, prewarm)
    }

    /// Select the startup readiness policy. `Playable` is the default for every
    /// lane; `Idle` is a named diagnostics/screenshot/test option.
    pub fn with_readiness(mut self, readiness: StartupReadinessPolicy) -> Self {
        self.readiness = readiness;
        self
    }

    pub fn step(&mut self, camera_position: Vec3) -> Result<NativeSessionStartupStep> {
        let host_mode = self.runtime.host_mode();
        let poll_start = Instant::now();
        let mut changed = self
            .runtime
            .poll_with_update_budget(RuntimeUpdatePumpBudget::default_frame())?;
        let poll_ms = elapsed_ms(poll_start.elapsed());
        self.poll_count += 1;
        self.poll_ms += poll_ms;

        // Local startup readiness / loading-progress overlay reads server-runner
        // diagnostics; remote host modes have no runner and this is a no-op.
        self.runtime.refresh_startup_runner_diagnostics()?;

        let section_update = self.runtime.sync_render_sections(camera_position)?;
        self.runtime
            .release_render_compile_jobs(section_update.accepted_compile_result_count);
        // docs/tactical/167: fold the transient update into the startup seed before
        // it is dropped, so completion can hand a full draw batch to the platform
        // adapter without recompiling from the metadata-only resident cache. The
        // compile-job release above stays in the pump: accepted jobs are not held
        // by the seed after this step.
        self.render_seed.observe(&section_update);
        let render_changed = section_update.rebuilt_section_count() > 0
            || section_update.removed_section_count() > 0
            || section_update.submitted_compile_section_count > 0
            || section_update.completed_compile_section_count > 0;
        changed |= render_changed;

        // Startup LOD prewarm is a local-integrated policy; remote runtimes have a
        // disabled prewarm and are skipped here.
        if let Some(local) = self.runtime.as_local_mut() {
            self.prewarm.advance(local, camera_position);
        }

        let local_progress = self.runtime.startup_progress_overlay();
        let host_ready = self
            .runtime
            .startup_host_ready(self.readiness, camera_position);
        let render_seed_drawable = self.render_seed.drawable_section_count();
        // Every lane uses the same gate: host-mode evidence, a drawable render
        // seed, and prewarm settled (always true when prewarm is disabled).
        let startup_ready = crate::StartupAdmissionEvidence {
            host_ready,
            drawable_section_count: render_seed_drawable,
            presentation_settled: self.prewarm.settled(),
        }
        .ready();

        let step = NativeSessionStartupStep {
            host_mode,
            poll_count: self.poll_count,
            poll_ms: self.poll_ms,
            changed,
            startup_ready,
            host_ready,
            lod_prewarm_enabled: self.prewarm.enabled(),
            lod_prewarm_complete: self.prewarm.complete,
            lod_prewarm_timeout: self.prewarm.timed_out,
            lod_prewarm_ms: self.prewarm.elapsed_ms,
            startup_lod_tiles_ready: self.prewarm.tiles_ready,
            startup_lod_tiles_target: self.prewarm.tiles_target,
            cached_section_count: self.runtime.cached_section_count(),
            rebuilt_section_count: section_update.rebuilt_section_count(),
            submitted_compile_section_count: section_update.submitted_compile_section_count,
            accepted_compile_result_count: section_update.accepted_compile_result_count,
            completed_compile_section_count: section_update.completed_compile_section_count,
            pending_compile_jobs: section_update.pending_compile_jobs,
            render_seed_section_count: self.render_seed.section_count(),
            render_seed_drawable_section_count: render_seed_drawable,
            local_progress,
        };
        self.last_step = step.clone();
        Ok(step)
    }

    pub fn progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.runtime.startup_progress_overlay()
    }

    pub const fn readiness_policy(&self) -> StartupReadinessPolicy {
        self.readiness
    }

    pub const fn poll_count(&self) -> usize {
        self.poll_count
    }

    pub const fn poll_ms(&self) -> f64 {
        self.poll_ms
    }

    pub fn runtime(&self) -> &NativeSessionServices<S> {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut NativeSessionServices<S> {
        &mut self.runtime
    }

    /// Startup render-seed sections accumulated so far, including empty sections.
    pub fn render_seed_section_count(&self) -> usize {
        self.render_seed.section_count()
    }

    /// Startup render-seed sections carrying drawable geometry.
    pub fn render_seed_drawable_section_count(&self) -> usize {
        self.render_seed.drawable_section_count()
    }

    pub fn into_runtime(self) -> NativeSessionServices<S> {
        self.runtime
    }

    /// Drive the pump synchronously at a fixed startup camera until startup
    /// readiness (under the active [`StartupReadinessPolicy`]) or the timeout,
    /// then consume it into the completion (docs/tactical/167 Slice 3). This is
    /// the shared blocking startup drive for the native lanes that initialize the
    /// world up front (XR remote/replacement, Android, desktop remote/idle) so
    /// none of them fall back to `poll_until_idle` + `sync_all_render_sections`.
    /// The seed reflects this fixed startup camera; interest drift from a later
    /// pose correction is reconciled by normal streaming (shared camera/interest
    /// reconciliation is tactical 167 Slice 4).
    pub fn drive_to_ready_with_timeout(
        mut self,
        camera_position: Vec3,
        timeout: Duration,
    ) -> Result<NativeSessionStartupCompletion<S>> {
        let deadline = system_monotonic_clock().deadline_after(timeout);
        self.drive_until_ready(camera_position, &deadline, None, timeout)?;
        Ok(self.complete())
    }

    /// Drive the pump synchronously to readiness with the default startup timeout.
    pub fn drive_to_ready(
        self,
        camera_position: Vec3,
    ) -> Result<NativeSessionStartupCompletion<S>> {
        self.drive_to_ready_with_timeout(camera_position, DEFAULT_STARTUP_READINESS_TIMEOUT)
    }

    /// Drive to readiness, then reconcile the final startup camera/interest before
    /// completing (docs/tactical/167 Slice 4). `reconcile` applies the platform's
    /// physical startup pose — flat spectator placement, an XR startup view pose,
    /// or an accepted server position correction — to the runtime and returns the
    /// resulting camera eye position. If reconciliation moves chunk interest, the
    /// pump keeps driving at the new camera until the render seed covers it, so the
    /// seed and readiness decision reflect the final startup camera rather than the
    /// pre-reconciliation one. A large programmatic startup teleport (a far XR view
    /// pose or a far server spawn correction) therefore cannot ship a seed built
    /// only around the pre-teleport camera. Physical pose inputs stay platform-local
    /// inside `reconcile`; this owns only the "re-pump at the new camera" invariant.
    pub fn drive_to_ready_reconciled(
        mut self,
        initial_camera: Vec3,
        timeout: Duration,
        mut reconcile: impl FnMut(&mut NativeSceneServices<S>) -> Result<Vec3>,
    ) -> Result<NativeSessionStartupCompletion<S>> {
        let deadline = system_monotonic_clock().deadline_after(timeout);
        self.drive_until_ready(initial_camera, &deadline, None, timeout)?;
        for _ in 0..MAX_STARTUP_RECONCILE_PASSES {
            let interest_before = self.runtime.interest_center();
            let camera = reconcile(&mut self.runtime)?;
            let interest_after = self.runtime.interest_center();
            if interest_after == interest_before {
                break;
            }
            // Interest moved during reconciliation: keep pumping at the new camera
            // until the render seed covers it (docs/tactical/167 Slice 4). Another
            // pass may surface a further correction, so loop until interest settles.
            self.drive_until_ready(camera, &deadline, Some(interest_after), timeout)?;
        }
        Ok(self.complete())
    }

    /// Step the pump at `camera_position` until startup readiness (and, when
    /// `require_drawable_near` is set, render-seed coverage near that chunk) or the
    /// deadline. A reconciled camera may legitimately look at a view with no
    /// drawable sections (over void), so a coverage-only timeout completes with the
    /// best-effort seed rather than failing; a base-readiness timeout still errors.
    fn drive_until_ready(
        &mut self,
        camera_position: Vec3,
        deadline: &MonotonicDeadline,
        require_drawable_near: Option<ChunkPos>,
        timeout: Duration,
    ) -> Result<()> {
        loop {
            let step = self.step(camera_position)?;
            let covered = match require_drawable_near {
                None => true,
                Some(center) => {
                    self.render_seed
                        .drawable_section_count_near(center, self.runtime.render_distance() as i32)
                        > 0
                }
            };
            if step.startup_ready && covered {
                return Ok(());
            }
            if deadline.is_reached() {
                if require_drawable_near.is_some() && step.startup_ready {
                    log::warn!(
                        "startup reconciliation timed out after {:.3}s waiting for {} render-seed coverage near the final camera; completing with best-effort seed (render_seed_drawable_sections={})",
                        timeout.as_secs_f64(),
                        self.runtime.host_label(),
                        step.render_seed_drawable_section_count,
                    );
                    return Ok(());
                }
                bail!(
                    "timed out after {:.3}s waiting for {} startup readiness: loaded_chunks={} render_seed_drawable_sections={} pending_compile_jobs={}",
                    timeout.as_secs_f64(),
                    self.runtime.host_label(),
                    self.runtime.loaded_chunk_count(),
                    step.render_seed_drawable_section_count,
                    step.pending_compile_jobs,
                );
            }
            if !step.changed {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// Consume the pump, returning the ready runtime, the transient startup render
    /// seed batch for a one-shot draw-resource upload, and the final startup step
    /// (docs/tactical/167). Use this instead of
    /// `recompile_all_render_section_meshes_for_resource_rebuild(...)`: it hands
    /// over meshes the pump already compiled rather than recompiling from
    /// metadata-only resident state.
    pub fn complete(mut self) -> NativeSessionStartupCompletion<S> {
        let startup_sections = self.render_seed.drain_sections();
        NativeSessionStartupCompletion {
            runtime: self.runtime,
            startup_sections,
            final_step: self.last_step,
        }
    }
}

/// Thin local-integrated wrapper over the shared [`NativeSessionStartupPump`]
/// (docs/tactical/167 Slice 2). It keeps the desktop/XR/perf local startup API
/// stable while sharing one startup implementation; it holds no separate startup
/// behavior. Local platform consumers migrate to the shared pump in Slice 3.
#[derive(Debug)]
pub struct LocalIntegratedStartupPump {
    inner: NativeSessionStartupPump<LocalOnlySession>,
}

impl LocalIntegratedStartupPump {
    pub fn with_mesh_assets(
        options: LocalIntegratedSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        Ok(Self {
            inner: NativeSessionStartupPump::local_with_mesh_assets(options, mesh_assets)?,
        })
    }

    pub fn step(&mut self, camera_position: Vec3) -> Result<LocalIntegratedStartupStep> {
        Ok(self.inner.step(camera_position)?.into_local_integrated())
    }

    pub fn progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.inner.progress_overlay()
    }

    pub const fn poll_count(&self) -> usize {
        self.inner.poll_count()
    }

    pub const fn poll_ms(&self) -> f64 {
        self.inner.poll_ms()
    }

    pub fn runtime(&self) -> &LocalIntegratedSceneRuntime {
        self.inner
            .runtime()
            .as_local()
            .expect("local startup pump always owns a local integrated runtime")
    }

    /// Mutable shared-service view used by scene-owned detached startup to
    /// acknowledge the authoritative initial pose without exposing the native
    /// runner or duplicating startup policy.
    pub fn runtime_services_mut(&mut self) -> &mut NativeSessionServices<LocalOnlySession> {
        self.inner.runtime_mut()
    }

    /// Startup render-seed sections accumulated so far, including empty sections.
    pub fn render_seed_section_count(&self) -> usize {
        self.inner.render_seed_section_count()
    }

    /// Startup render-seed sections carrying drawable geometry.
    pub fn render_seed_drawable_section_count(&self) -> usize {
        self.inner.render_seed_drawable_section_count()
    }

    /// CPU mesh payload retained by the one-shot startup seed.
    pub fn render_seed_estimated_owned_bytes(&self) -> usize {
        self.inner.render_seed.estimated_owned_bytes()
    }

    pub fn into_runtime(self) -> LocalIntegratedSceneRuntime {
        expect_local_scene(self.inner.into_runtime().into_runtime())
    }

    /// Consume the pump, returning the runtime plus the transient startup render
    /// seed batch for a one-shot draw-resource upload (docs/tactical/167).
    pub fn into_runtime_with_startup_sections(
        self,
    ) -> (LocalIntegratedSceneRuntime, Vec<TexturedRenderSectionMesh>) {
        let completion = self.inner.complete();
        (
            expect_local_scene(completion.runtime.into_runtime()),
            completion.startup_sections,
        )
    }

    /// Reconcile the final startup camera/interest, then consume the pump into the
    /// runtime plus render seed (docs/tactical/167 Slice 4). Used by local lanes
    /// (XR local) that reach playable at the spawn camera and then apply a startup
    /// view pose: if the pose moves chunk interest, the pump re-pumps at the new
    /// camera so the seed covers it. See
    /// [`NativeSessionStartupPump::drive_to_ready_reconciled`].
    pub fn drive_to_ready_reconciled(
        self,
        initial_camera: Vec3,
        timeout: Duration,
        reconcile: impl FnMut(&mut NativeSceneServices<LocalOnlySession>) -> Result<Vec3>,
    ) -> Result<(LocalIntegratedSceneRuntime, Vec<TexturedRenderSectionMesh>)> {
        let completion =
            self.inner
                .drive_to_ready_reconciled(initial_camera, timeout, reconcile)?;
        Ok((
            expect_local_scene(completion.runtime.into_runtime()),
            completion.startup_sections,
        ))
    }
}

fn expect_local_scene<S>(runtime: NativeSceneServices<S>) -> LocalIntegratedSceneRuntime {
    match runtime {
        NativeSceneServices::Local(scene) => scene,
        NativeSceneServices::RemoteDedicated(_) => {
            unreachable!("local startup pump always owns a local integrated runtime")
        }
    }
}

impl LocalIntegratedSceneRuntime<NativeIntegratedServerRunner> {
    pub fn new(options: LocalIntegratedSceneOptions) -> Result<Self> {
        let mesh_assets = load_textured_mesh_assets()?;
        Self::with_mesh_assets(options, mesh_assets)
    }

    /// Build a local-integrated scene backed by the native threaded server
    /// runner. Runner-agnostic construction lives in
    /// [`Self::with_mesh_assets_and_runner`]; this only owns spawning the native
    /// runner from the options (docs/tactical/168 Slice 2).
    pub fn with_mesh_assets(
        options: LocalIntegratedSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let server_runner = NativeIntegratedServerRunner::new(native_runner_config(&options))
            .context("failed to start local integrated server runner")?;
        Self::with_mesh_assets_and_runner(options, mesh_assets, server_runner)
    }
}

impl<R: IntegratedServerRunner> LocalIntegratedSceneRuntime<R> {
    /// Runner-generic local-integrated scene construction (docs/tactical/168
    /// Slice 2). The caller supplies an already-started [`IntegratedServerRunner`]
    /// so the local host mode no longer hard-codes the native runner. The scene
    /// wiring (render-compile dispatcher, far-LOD, deferred drop worker, initial
    /// chunk view) is identical across runners.
    pub fn with_mesh_assets_and_runner(
        options: LocalIntegratedSceneOptions,
        mesh_assets: TexturedMeshAssets,
        server_runner: R,
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
        let core = SingleViewRuntime::local_integrated_with_seed(
            options.seed,
            options.center,
            options.render_distance,
            options.chunk_tracking_radius(),
        );
        let far_lod_cache = FarTerrainLodCache::with_clock(core.monotonic_clock().clone());
        let mut scene = Self {
            core,
            connection: IntegratedRunnerConnection::new(server_runner),
            mesh_assets,
            render_compile_dispatcher,
            far_lod_cache,
            lod_coverage: LodCoverageCoordinator::new(),
            deferred_chunk_drops: Box::new(NativeDeferredDropService::new()?),
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
            self.deferred_chunk_drops.enqueue(snapshot, item_count);
        }
        handed_off_items
    }

    fn deferred_client_chunk_drop_backlog_items(&self) -> usize {
        self.core.deferred_client_chunk_drop_item_count()
            + self.deferred_chunk_drops.backlog().pending_items
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.core.client()
    }

    pub fn promote_observer_to_player(&mut self) -> Result<()> {
        self.connection
            .promote_observer_to_player()
            .context("failed to promote local observer to player")
    }

    pub fn demote_player_to_observer(&mut self) -> Result<()> {
        self.connection
            .demote_player_to_observer()
            .context("failed to demote local player to observer")
    }

    pub fn debug_break_observed_block(&mut self, pos: mclone_core::BlockPos) -> Result<bool> {
        self.connection
            .debug_break_observed_block(pos)
            .context("failed to mutate observed world from debug host")
    }

    pub const fn mesh_assets(&self) -> &TexturedMeshAssets {
        &self.mesh_assets
    }

    pub fn replace_asset_epoch(
        &mut self,
        epoch: u64,
        mesh_assets: TexturedMeshAssets,
        sections: TexturedRenderSectionBuildReport,
    ) -> Result<()> {
        let health = self.render_compile_dispatcher.queue_health();
        let replacement =
            NativeRenderSectionCompileDispatcher::with_worker_count_and_max_pending_jobs_and_timing(
                mesh_assets.catalog.clone(),
                health.compile_worker_count.max(1),
                health.max_pending_jobs.max(1),
                self.render_compile_dispatcher.worker_timing_enabled(),
            )?;
        let _ = self.core.replace_asset_epoch_sections(epoch, sections);
        self.render_compile_dispatcher = replacement;
        self.mesh_assets = mesh_assets;
        self.clear_far_lod();
        Ok(())
    }

    pub fn clear_far_lod(&mut self) {
        let abandoned = self.far_lod_cache.clear();
        self.render_compile_dispatcher
            .release_completed_far_lod_jobs(abandoned);
        self.lod_coverage.clear();
    }

    /// Cumulative LOD coverage replacement/pop counters (tactical 162 Slice 2).
    pub fn lod_coverage_counters(&self) -> LodReplacementCounters {
        self.lod_coverage.counters()
    }

    pub fn prepare_far_lod_frame(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        camera_position: Vec3,
        build_budget: usize,
        upload_budget: usize,
    ) -> Result<Option<&FarTerrainLodFrameUpdate>> {
        if !config.enabled {
            self.clear_far_lod();
            return Ok(None);
        }
        let normal_terrain_chunks = traversal_ready_chunks(
            &self
                .core
                .traversal_ready_render_section_keys(camera_position),
        );
        let materials = self.mesh_assets.far_lod_materials.clone();
        self.far_lod_cache.advance_for_camera_with_profile(
            config,
            seed,
            generation_profile,
            center,
            self.core.render_distance(),
            materials.as_ref(),
            &mut self.render_compile_dispatcher,
            build_budget,
        )?;
        self.far_lod_cache
            .drain_render_uploads(upload_budget, &mut self.render_compile_dispatcher);
        let visible = self.resolve_lod_coverage(&normal_terrain_chunks);
        let frame = self.far_lod_cache.prepare_render_update(&visible);
        assert_no_real_far_lod_overlap(&normal_terrain_chunks, &frame.visible_tiles);
        Ok(Some(frame))
    }

    /// Drive the shared LOD coverage coordinator with this frame's drawable
    /// normal chunks, loaded chunks, and synthetic tile availability. This makes
    /// per-tile precedence (normal drawable > reduced real > synthetic > nothing)
    /// explicit and records replacement/pop diagnostics; it does not rebuild the
    /// mesh (a derived product of the synthetic cache).
    fn resolve_lod_coverage(&mut self, normal_drawable: &BTreeSet<ChunkPos>) -> BTreeSet<ChunkPos> {
        let normal_loaded: BTreeSet<ChunkPos> =
            self.core.client().loaded_chunk_positions().collect();
        let synthetic: Vec<LodTileAvailability> = self
            .far_lod_cache
            .drawable_lod_tiles()
            .map(LodTileAvailability::synthetic)
            .collect();
        self.lod_coverage
            .resolve(normal_drawable, &normal_loaded, synthetic)
            .visible_lod_tiles
    }

    /// Build cheap retained far-LOD coverage during startup with an explicit
    /// build budget, sharing the same retained patches the live far-LOD path
    /// reuses. Presentation only: this never satisfies spawn authority.
    pub fn prewarm_far_lod(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        _camera_position: Vec3,
        chunk_budget: usize,
    ) -> FarTerrainLodCoverage {
        self.far_lod_cache.prewarm_with_profile(
            config,
            seed,
            generation_profile,
            center,
            self.core.render_distance(),
            self.mesh_assets.far_lod_materials.as_ref(),
            chunk_budget,
            &mut self.render_compile_dispatcher,
        )
    }

    pub fn far_lod_stats(&self) -> FarTerrainLodProducerStats {
        self.far_lod_cache.stats()
    }

    pub fn far_lod_settle_snapshot(&self, camera_position: Vec3) -> FarLodRuntimeSettleSnapshot {
        far_lod_runtime_settle_snapshot(
            &self.core,
            &self.far_lod_cache,
            &self.lod_coverage,
            camera_position,
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
            .context("failed to set local integrated simulation cadence")?;
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
            .context("failed to send local integrated chunk view command")?;
        Ok(true)
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<()> {
        self.send_gameplay_command_timed(command).map(|_| ())
    }

    pub fn send_gameplay_command_with_update_policy(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<()> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
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

    pub fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        let total_start = Instant::now();
        let send_start = Instant::now();
        self.connection
            .send_command_only(command)
            .context("failed to send local integrated gameplay command")?;
        let send_ms = elapsed_ms(send_start.elapsed());
        let (drain_updates_ms, apply_report) = match policy {
            GameplayCommandUpdatePolicy::DrainImmediately => {
                let pump_report = pump_client_connection_updates_report(
                    &mut self.core,
                    &mut self.connection,
                    RuntimeUpdatePumpBudget::unlimited(),
                )
                .context("failed to drain local integrated server updates")?;
                let drain_updates_ms = pump_report.drain_updates_ms;
                (drain_updates_ms, pump_report.apply_report)
            }
            GameplayCommandUpdatePolicy::SendOnly => (0.0, RuntimeUpdateApplyReport::default()),
        };
        Ok(GameplayCommandSubmission {
            timing: GameplayCommandTiming {
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
        })
    }

    pub fn send_ephemeral_with_update_policy_timed(
        &mut self,
        message: ClientEphemeralMessage,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        let total_start = Instant::now();
        let send_start = Instant::now();
        self.connection
            .send_ephemeral(message)
            .context("failed to send local integrated ephemeral message")?;
        let send_ms = elapsed_ms(send_start.elapsed());
        let (drain_updates_ms, apply_report) = match policy {
            GameplayCommandUpdatePolicy::DrainImmediately => {
                let pump_report = pump_client_connection_updates_report(
                    &mut self.core,
                    &mut self.connection,
                    RuntimeUpdatePumpBudget::unlimited(),
                )
                .context("failed to drain local integrated server updates")?;
                (pump_report.drain_updates_ms, pump_report.apply_report)
            }
            GameplayCommandUpdatePolicy::SendOnly => (0.0, RuntimeUpdateApplyReport::default()),
        };
        Ok(GameplayCommandSubmission {
            timing: GameplayCommandTiming {
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
        })
    }

    pub fn poll(&mut self) -> Result<bool> {
        self.poll_with_update_budget(RuntimeUpdatePumpBudget::default())
    }

    pub fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool> {
        let poll_start = Instant::now();
        let pump_report =
            pump_client_connection_updates_report(&mut self.core, &mut self.connection, budget)?;
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
                producer_inbound_frame_sequence: pump_report.producer_inbound_frame_sequence,
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
                .context("failed to poll local integrated server diagnostics")?;
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
        self.poll_until_idle_with_timeout(DEFAULT_LOCAL_INTEGRATED_IDLE_TIMEOUT)
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
                    "timed out waiting for local integrated worldgen jobs after {:.3}s",
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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

    pub fn resident_section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.core.resident_section_metadata()
    }

    pub fn cached_section_count(&self) -> usize {
        self.core.cached_section_count()
    }

    pub fn mark_all_render_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        self.core
            .mark_all_render_sections_dirty_for_resource_rebuild()
    }

    /// Recompile every resident render section and return the transient full
    /// mesh batch for a one-shot GPU (re)upload, without the resident cache ever
    /// retaining CPU meshes (docs/tactical/163).
    ///
    /// docs/tactical/167: this is a renderer/surface **resource-rebuild** path,
    /// not a startup seeding path — the name says so. Startup callers must take
    /// their draw seed from the startup pump's render seed
    /// ([`NativeSessionStartupCompletion::startup_sections`]); the last startup
    /// callers were migrated off this in Slice 3. Do not add startup uses.
    pub fn recompile_all_render_section_meshes_for_resource_rebuild(
        &mut self,
        camera_position: Vec3,
    ) -> Result<Vec<TexturedRenderSectionMesh>> {
        self.mark_all_render_sections_dirty_for_resource_rebuild();
        Ok(self
            .sync_all_render_sections(camera_position)?
            .rebuilt_sections)
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
            .context("failed to poll local integrated server diagnostics")
    }

    /// Refresh the cached server-runner diagnostics that back local startup
    /// readiness and the loading-progress overlay (docs/tactical/167 shared
    /// startup pump). Remote host modes have no runner and skip this.
    pub(crate) fn refresh_startup_runner_diagnostics(&mut self) -> Result<()> {
        let diagnostics = self.server_runner_diagnostics()?;
        self.last_runner_diagnostics = Some(diagnostics);
        self.last_runner_diagnostics_poll_at = Some(Instant::now());
        Ok(())
    }

    /// Synchronously flush the integrated server's dirty chunks to disk. Backs
    /// the shared lifecycle save point so world edits survive an OS kill without
    /// tearing down the session (tactical 168 Slice 0).
    fn flush_persistence(&mut self) -> Result<usize> {
        self.connection
            .flush_persistence()
            .context("failed to flush local integrated world persistence")
    }

    /// Honest local spawn-authority gate: the playable chunk is server-ready and
    /// its client snapshot is present. Render-seed drawability and startup LOD
    /// prewarm are layered on by the shared startup pump, never here.
    fn startup_spawn_authority_ready(&self) -> bool {
        let Some(progress) = self
            .last_runner_diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.view_readiness_snapshot.as_ref())
            .map(|snapshot| snapshot.stats)
        else {
            return false;
        };
        progress.playable_chunk_ready
            && self
                .client()
                .chunk_snapshot(progress.playable_chunk)
                .is_some()
    }

    fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.last_runner_diagnostics
            .as_ref()
            .and_then(|diagnostics| {
                loading_progress_overlay_from_diagnostics(diagnostics)
                    .or_else(|| view_readiness_overlay_from_diagnostics(diagnostics))
            })
    }
}

#[derive(Debug)]
pub enum NativeSceneServices<S> {
    Local(LocalIntegratedSceneRuntime),
    RemoteDedicated(RemoteDedicatedSceneRuntime<S>),
}

#[derive(Debug)]
pub struct NativeSessionServices<S> {
    descriptor: ActiveSessionDescriptor,
    runtime: NativeSceneServices<S>,
}

impl<S> NativeSceneServices<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn local(options: LocalIntegratedSceneOptions) -> Result<Self> {
        Ok(Self::Local(LocalIntegratedSceneRuntime::new(options)?))
    }

    pub fn local_with_mesh_assets(
        options: LocalIntegratedSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        Ok(Self::Local(LocalIntegratedSceneRuntime::with_mesh_assets(
            options,
            mesh_assets,
        )?))
    }

    pub fn remote_dedicated(options: SingleViewHostOptions, session: S) -> Result<Self> {
        Ok(Self::RemoteDedicated(RemoteDedicatedSceneRuntime::new(
            options, session,
        )?))
    }

    pub fn remote_dedicated_with_mesh_assets(
        options: SingleViewHostOptions,
        session: S,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        Ok(Self::RemoteDedicated(
            RemoteDedicatedSceneRuntime::with_mesh_assets(options, session, mesh_assets)?,
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

    pub fn as_local(&self) -> Option<&LocalIntegratedSceneRuntime> {
        match self {
            Self::Local(scene) => Some(scene),
            Self::RemoteDedicated(_) => None,
        }
    }

    fn as_local_mut(&mut self) -> Option<&mut LocalIntegratedSceneRuntime> {
        match self {
            Self::Local(scene) => Some(scene),
            Self::RemoteDedicated(_) => None,
        }
    }

    /// Refresh the local server-runner diagnostics that back startup readiness /
    /// the loading-progress overlay. No-op for remote host modes, which have no
    /// integrated runner (docs/tactical/167 shared startup pump).
    fn refresh_startup_runner_diagnostics(&mut self) -> Result<()> {
        match self {
            Self::Local(scene) => scene.refresh_startup_runner_diagnostics(),
            Self::RemoteDedicated(_) => Ok(()),
        }
    }

    /// Local loading-progress overlay for the startup step; remote returns `None`
    /// because there is no integrated loading-progress source.
    pub fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        match self {
            Self::Local(scene) => scene.startup_progress_overlay(),
            Self::RemoteDedicated(_) => None,
        }
    }

    /// Flush world edits to persistent storage without tearing down the session.
    /// No-op for remote host modes: the dedicated server owns its own persistence
    /// and this client holds no authoritative world state (tactical 168 Slice 0).
    pub fn flush_persistence(&mut self) -> Result<usize> {
        match self {
            Self::Local(scene) => scene.flush_persistence(),
            Self::RemoteDedicated(_) => Ok(0),
        }
    }

    /// Host-mode startup readiness evidence, excluding the render seed and startup
    /// LOD prewarm which the shared startup pump owns (docs/tactical/167). Every
    /// lane uses the same [`StartupReadinessPolicy`]; the only divergence is the
    /// host-mode evidence (local spawn authority vs. remote drained active view).
    pub(crate) fn startup_host_ready(
        &self,
        policy: StartupReadinessPolicy,
        camera_position: Vec3,
    ) -> bool {
        let host_evidence = match self {
            Self::Local(scene) => scene.startup_spawn_authority_ready(),
            Self::RemoteDedicated(scene) => scene.startup_host_ready(),
        };
        match policy {
            StartupReadinessPolicy::Playable => host_evidence,
            // Idle additionally waits for the active view to have no pending
            // render work at the final startup camera (screenshots/diagnostics).
            StartupReadinessPolicy::Idle => {
                host_evidence && !self.has_pending_render_work(camera_position)
            }
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

    pub fn replace_asset_epoch(
        &mut self,
        epoch: u64,
        mesh_assets: TexturedMeshAssets,
        sections: TexturedRenderSectionBuildReport,
    ) -> Result<()> {
        match self {
            Self::Local(scene) => scene.replace_asset_epoch(epoch, mesh_assets, sections),
            Self::RemoteDedicated(scene) => scene.replace_asset_epoch(epoch, mesh_assets, sections),
        }
    }

    pub fn clear_far_lod(&mut self) {
        match self {
            Self::Local(scene) => scene.clear_far_lod(),
            Self::RemoteDedicated(scene) => scene.clear_far_lod(),
        }
    }

    pub fn prepare_far_lod_frame(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        camera_position: Vec3,
        build_budget: usize,
        upload_budget: usize,
    ) -> Result<Option<&FarTerrainLodFrameUpdate>> {
        match self {
            Self::Local(scene) => scene.prepare_far_lod_frame(
                config,
                seed,
                generation_profile,
                center,
                camera_position,
                build_budget,
                upload_budget,
            ),
            Self::RemoteDedicated(scene) => scene.prepare_far_lod_frame(
                config,
                seed,
                generation_profile,
                center,
                camera_position,
                build_budget,
                upload_budget,
            ),
        }
    }

    pub fn far_lod_stats(&self) -> FarTerrainLodProducerStats {
        match self {
            Self::Local(scene) => scene.far_lod_stats(),
            Self::RemoteDedicated(scene) => scene.far_lod_stats(),
        }
    }

    pub fn far_lod_settle_snapshot(&self, camera_position: Vec3) -> FarLodRuntimeSettleSnapshot {
        match self {
            Self::Local(scene) => scene.far_lod_settle_snapshot(camera_position),
            Self::RemoteDedicated(scene) => scene.far_lod_settle_snapshot(camera_position),
        }
    }

    /// Cumulative LOD coverage replacement/pop counters (tactical 162 Slice 2).
    pub fn lod_coverage_counters(&self) -> LodReplacementCounters {
        match self {
            Self::Local(scene) => scene.lod_coverage_counters(),
            Self::RemoteDedicated(scene) => scene.lod_coverage_counters(),
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
                scene
                    .send_gameplay_command_with_update_policy_timed(command, policy)
                    .map(|submission| (true, submission.timing))
            }
            Self::RemoteDedicated(scene) => {
                let Some(command) = scene.core_mut().set_chunk_view_command(
                    center,
                    render_distance,
                    chunk_tracking_radius,
                ) else {
                    return Ok((false, GameplayCommandTiming::default()));
                };
                scene
                    .send_gameplay_command_with_update_policy_timed(command, policy)
                    .map(|submission| (true, submission.timing))
            }
        }
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<()> {
        match self {
            Self::Local(scene) => scene.send_gameplay_command(command),
            Self::RemoteDedicated(scene) => scene.send_gameplay_command(command),
        }
    }

    pub fn send_gameplay_command_with_update_policy(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<()> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
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

    pub fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        match self {
            Self::Local(scene) => {
                scene.send_gameplay_command_with_update_policy_timed(command, policy)
            }
            Self::RemoteDedicated(scene) => {
                scene.send_gameplay_command_with_update_policy_timed(command, policy)
            }
        }
    }

    pub fn send_ephemeral_with_update_policy_timed(
        &mut self,
        message: ClientEphemeralMessage,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        match self {
            Self::Local(scene) => scene.send_ephemeral_with_update_policy_timed(message, policy),
            Self::RemoteDedicated(scene) => {
                scene.send_ephemeral_with_update_policy_timed(message, policy)
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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

    pub fn resident_section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        match self {
            Self::Local(scene) => scene.resident_section_metadata(),
            Self::RemoteDedicated(scene) => scene.resident_section_metadata(),
        }
    }

    pub fn cached_section_count(&self) -> usize {
        match self {
            Self::Local(scene) => scene.cached_section_count(),
            Self::RemoteDedicated(scene) => scene.cached_section_count(),
        }
    }

    pub fn mark_all_render_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        match self {
            Self::Local(scene) => scene.mark_all_render_sections_dirty_for_resource_rebuild(),
            Self::RemoteDedicated(scene) => {
                scene.mark_all_render_sections_dirty_for_resource_rebuild()
            }
        }
    }

    /// See
    /// [`LocalIntegratedSceneRuntime::recompile_all_render_section_meshes_for_resource_rebuild`].
    /// docs/tactical/167: resource-rebuild only — not for startup draw seeding.
    pub fn recompile_all_render_section_meshes_for_resource_rebuild(
        &mut self,
        camera_position: Vec3,
    ) -> Result<Vec<TexturedRenderSectionMesh>> {
        match self {
            Self::Local(scene) => {
                scene.recompile_all_render_section_meshes_for_resource_rebuild(camera_position)
            }
            Self::RemoteDedicated(scene) => {
                scene.recompile_all_render_section_meshes_for_resource_rebuild(camera_position)
            }
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

impl<S> NativeSessionServices<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn local(options: LocalIntegratedSceneOptions) -> Result<Self> {
        let request = SessionStartRequest::new_seed_local_world(options.seed);
        Self::start_with(request, || NativeSceneServices::local(options))
    }

    pub fn local_with_mesh_assets(
        options: LocalIntegratedSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<Self> {
        let request = SessionStartRequest::new_seed_local_world(options.seed);
        Self::start_with(request, || {
            NativeSceneServices::local_with_mesh_assets(options, mesh_assets)
        })
    }

    pub fn remote_dedicated(
        endpoint: RemoteSessionEndpoint,
        options: SingleViewHostOptions,
        session: S,
    ) -> Result<Self> {
        let request = SessionStartRequest::JoinRemote { endpoint };
        Self::start_with(request, || {
            NativeSceneServices::remote_dedicated(options, session)
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
            NativeSceneServices::remote_dedicated_with_mesh_assets(options, session, mesh_assets)
        })
    }

    pub fn from_active_runtime(
        request: SessionStartRequest,
        runtime: NativeSceneServices<S>,
    ) -> Result<Self> {
        let descriptor = request
            .active_descriptor()
            .context("native game session request did not describe an active session")?;
        Ok(Self::from_active_runtime_with_descriptor(
            descriptor, runtime,
        ))
    }

    pub fn from_active_runtime_with_descriptor(
        descriptor: ActiveSessionDescriptor,
        runtime: NativeSceneServices<S>,
    ) -> Self {
        Self {
            descriptor,
            runtime,
        }
    }

    pub fn active_session(&self) -> Option<&ActiveSessionDescriptor> {
        Some(&self.descriptor)
    }

    pub fn into_runtime(self) -> NativeSceneServices<S> {
        self.runtime
    }

    fn start_with(
        request: SessionStartRequest,
        start: impl FnOnce() -> Result<NativeSceneServices<S>>,
    ) -> Result<Self> {
        let descriptor = request
            .active_descriptor()
            .context("native game session request did not describe an active session")?;
        match start() {
            Ok(runtime) => Ok(Self {
                descriptor,
                runtime,
            }),
            Err(error) => {
                log::error!("failed to start native game session {request:?}: {error:#}");
                Err(error)
            }
        }
    }
}

impl<S> Deref for NativeSessionServices<S> {
    type Target = NativeSceneServices<S>;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl<S> DerefMut for NativeSessionServices<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

#[derive(Debug)]
pub struct RemoteDedicatedSceneRuntime<S> {
    core: SingleViewRuntime,
    connection: RemoteDedicatedConnection<S>,
    mesh_assets: TexturedMeshAssets,
    render_compile_dispatcher: NativeRenderSectionCompileDispatcher,
    far_lod_cache: FarTerrainLodCache,
    lod_coverage: LodCoverageCoordinator,
}

#[derive(Debug)]
struct RemoteDedicatedConnection<S> {
    session: S,
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
            queued_updates: VecDeque::new(),
            queued_update_bytes: 0,
        }
    }

    fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    fn clear_pending_updates(&mut self) {
        self.queued_updates.clear();
        self.queued_update_bytes = 0;
    }

    fn queue_metrics(&self) -> ClientConnectionQueueMetrics {
        let producer = self.session.pending_update_metrics();
        ClientConnectionQueueMetrics::new(
            self.queued_updates
                .len()
                .saturating_add(producer.update_depth),
            self.queued_update_bytes
                .saturating_add(producer.update_bytes),
        )
    }

    fn push_update_batch(
        &mut self,
        batch: RemoteServerUpdateBatch,
        transport_drained_after_batch: bool,
    ) -> Result<()> {
        let update_count = batch.updates.len();
        let batch_inbound_frame_sequence = batch.inbound_frame_sequence;
        let batch_producer_read_ms = batch.producer_read_ms;
        let batch_producer_decode_ms = batch.producer_decode_ms;
        let batch_has_producer_timing =
            batch_producer_read_ms > 0.0 || batch_producer_decode_ms > 0.0;
        if update_count == 0 {
            self.queued_updates.push_back(
                QueuedServerUpdate::empty(transport_drained_after_batch).with_remote_metadata(
                    batch_inbound_frame_sequence,
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
                    update
                        .inbound_frame_sequence
                        .or(batch_inbound_frame_sequence),
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
        self.session.send_command_only(command)
    }

    fn try_drain_next_update(&mut self) -> Result<ClientConnectionDrainResult> {
        if let Some(update) = self.queued_updates.pop_front() {
            self.queued_update_bytes = self
                .queued_update_bytes
                .saturating_sub(update.encoded_len());
            let metrics = self.queue_metrics();
            return Ok(ClientConnectionDrainResult::with_update(
                update,
                metrics.update_depth(),
                metrics.update_bytes(),
            ));
        }

        let batch = self
            .session
            .try_drain_update_batch()
            .context("failed to poll remote dedicated server updates")?;
        let Some(batch) = batch else {
            let metrics = self.queue_metrics();
            return Ok(ClientConnectionDrainResult::pending(
                metrics.update_depth(),
                metrics.update_bytes(),
            ));
        };

        let producer = self.session.pending_update_metrics();
        let transport_drained_after_batch = producer.frame_depth == 0;
        self.push_update_batch(batch, transport_drained_after_batch)?;
        if let Some(update) = self.queued_updates.pop_front() {
            self.queued_update_bytes = self
                .queued_update_bytes
                .saturating_sub(update.encoded_len());
            let metrics = self.queue_metrics();
            return Ok(ClientConnectionDrainResult::with_update(
                update,
                metrics.update_depth(),
                metrics.update_bytes(),
            ));
        }

        let metrics = self.queue_metrics();
        Ok(ClientConnectionDrainResult::pending(
            metrics.update_depth(),
            metrics.update_bytes(),
        ))
    }

    fn pending_update_metrics(&mut self) -> Result<ClientConnectionQueueMetrics> {
        Ok(self.queue_metrics())
    }
}

impl<S> RemoteDedicatedSceneRuntime<S>
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
            connection
                .send_command_only(command)
                .context("failed to initialize remote dedicated scene runtime")?;
            core.apply_exchange(deferred_command_exchange());
        }
        let far_lod_cache = FarTerrainLodCache::with_clock(core.monotonic_clock().clone());
        Ok(Self {
            core,
            connection,
            mesh_assets,
            render_compile_dispatcher,
            far_lod_cache,
            lod_coverage: LodCoverageCoordinator::new(),
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

    pub fn replace_asset_epoch(
        &mut self,
        epoch: u64,
        mesh_assets: TexturedMeshAssets,
        sections: TexturedRenderSectionBuildReport,
    ) -> Result<()> {
        let health = self.render_compile_dispatcher.queue_health();
        let replacement =
            NativeRenderSectionCompileDispatcher::with_worker_count_and_max_pending_jobs_and_timing(
                mesh_assets.catalog.clone(),
                health.compile_worker_count.max(1),
                health.max_pending_jobs.max(1),
                self.render_compile_dispatcher.worker_timing_enabled(),
            )?;
        let _ = self.core.replace_asset_epoch_sections(epoch, sections);
        self.render_compile_dispatcher = replacement;
        self.mesh_assets = mesh_assets;
        self.clear_far_lod();
        Ok(())
    }

    pub fn clear_far_lod(&mut self) {
        let abandoned = self.far_lod_cache.clear();
        self.render_compile_dispatcher
            .release_completed_far_lod_jobs(abandoned);
        self.lod_coverage.clear();
    }

    /// Cumulative LOD coverage replacement/pop counters (tactical 162 Slice 2).
    pub fn lod_coverage_counters(&self) -> LodReplacementCounters {
        self.lod_coverage.counters()
    }

    pub fn prepare_far_lod_frame(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        camera_position: Vec3,
        build_budget: usize,
        upload_budget: usize,
    ) -> Result<Option<&FarTerrainLodFrameUpdate>> {
        if !config.enabled {
            self.clear_far_lod();
            return Ok(None);
        }
        let normal_terrain_chunks = traversal_ready_chunks(
            &self
                .core
                .traversal_ready_render_section_keys(camera_position),
        );
        let materials = self.mesh_assets.far_lod_materials.clone();
        self.far_lod_cache.advance_for_camera_with_profile(
            config,
            seed,
            generation_profile,
            center,
            self.core.render_distance(),
            materials.as_ref(),
            &mut self.render_compile_dispatcher,
            build_budget,
        )?;
        self.far_lod_cache
            .drain_render_uploads(upload_budget, &mut self.render_compile_dispatcher);
        let normal_loaded: BTreeSet<ChunkPos> =
            self.core.client().loaded_chunk_positions().collect();
        let synthetic: Vec<LodTileAvailability> = self
            .far_lod_cache
            .drawable_lod_tiles()
            .map(LodTileAvailability::synthetic)
            .collect();
        let visible = self
            .lod_coverage
            .resolve(&normal_terrain_chunks, &normal_loaded, synthetic)
            .visible_lod_tiles;
        let frame = self.far_lod_cache.prepare_render_update(&visible);
        assert_no_real_far_lod_overlap(&normal_terrain_chunks, &frame.visible_tiles);
        Ok(Some(frame))
    }

    pub fn far_lod_stats(&self) -> FarTerrainLodProducerStats {
        self.far_lod_cache.stats()
    }

    pub fn far_lod_settle_snapshot(&self, camera_position: Vec3) -> FarLodRuntimeSettleSnapshot {
        far_lod_runtime_settle_snapshot(
            &self.core,
            &self.far_lod_cache,
            &self.lod_coverage,
            camera_position,
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

    /// A push stream has no durable "idle" state. Remote startup becomes ready
    /// when the active view has produced client-resident terrain; later
    /// publications continue through the normal frame budget.
    fn startup_host_ready(&self) -> bool {
        self.loaded_chunk_count() > 0
    }

    pub fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<()> {
        self.send_gameplay_command_timed(command).map(|_| ())
    }

    pub fn send_gameplay_command_with_update_policy(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<()> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
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

    pub fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        let total_start = Instant::now();
        let mut drain_updates_ms = 0.0;
        let mut apply_report = RuntimeUpdateApplyReport::default();
        if matches!(policy, GameplayCommandUpdatePolicy::DrainImmediately)
            && self.connection.pending_update_metrics()?.update_depth() > 0
        {
            let pump_report = pump_client_connection_updates_report(
                &mut self.core,
                &mut self.connection,
                RuntimeUpdatePumpBudget::unlimited(),
            )?;
            drain_updates_ms += pump_report.drain_updates_ms;
            apply_report.accumulate(pump_report.apply_report);
        }

        let send_start = Instant::now();
        self.connection
            .send_command_only(command)
            .context("failed to enqueue remote dedicated gameplay command")?;
        let send_ms = elapsed_ms(send_start.elapsed());
        self.core.apply_exchange(deferred_command_exchange());

        match policy {
            GameplayCommandUpdatePolicy::DrainImmediately => {
                let pump_report = pump_client_connection_updates_report(
                    &mut self.core,
                    &mut self.connection,
                    RuntimeUpdatePumpBudget::unlimited(),
                )?;
                drain_updates_ms += pump_report.drain_updates_ms;
                apply_report.accumulate(pump_report.apply_report);
            }
            GameplayCommandUpdatePolicy::SendOnly => {}
        }

        Ok(GameplayCommandSubmission {
            timing: GameplayCommandTiming {
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
        })
    }

    pub fn send_ephemeral_with_update_policy_timed(
        &mut self,
        message: ClientEphemeralMessage,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        let total_start = Instant::now();
        let mut drain_updates_ms = 0.0;
        let mut apply_report = RuntimeUpdateApplyReport::default();
        if matches!(policy, GameplayCommandUpdatePolicy::DrainImmediately)
            && self.connection.pending_update_metrics()?.update_depth() > 0
        {
            let pump_report = pump_client_connection_updates_report(
                &mut self.core,
                &mut self.connection,
                RuntimeUpdatePumpBudget::unlimited(),
            )?;
            drain_updates_ms += pump_report.drain_updates_ms;
            apply_report.accumulate(pump_report.apply_report);
        }

        let send_start = Instant::now();
        self.connection
            .send_ephemeral(message)
            .context("failed to enqueue remote dedicated ephemeral message")?;
        let send_ms = elapsed_ms(send_start.elapsed());
        self.core.apply_exchange(deferred_command_exchange());

        if matches!(policy, GameplayCommandUpdatePolicy::DrainImmediately) {
            let pump_report = pump_client_connection_updates_report(
                &mut self.core,
                &mut self.connection,
                RuntimeUpdatePumpBudget::unlimited(),
            )?;
            drain_updates_ms += pump_report.drain_updates_ms;
            apply_report.accumulate(pump_report.apply_report);
        }

        Ok(GameplayCommandSubmission {
            timing: GameplayCommandTiming {
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
        })
    }

    /// Reconnect and request the current view from a clean replica/queue state.
    ///
    /// Connection establishment may block and therefore belongs to the session
    /// lifecycle executor, never a drawable/frame callback.
    pub fn reconnect_and_resync(&mut self) -> Result<()> {
        let command = prepare_remote_dedicated_resync_command(
            &mut self.core,
            anyhow::anyhow!("explicit remote session reconnect"),
        )?;
        self.connection.clear_pending_updates();
        reconnect_remote_dedicated_session_and_resync(
            &mut self.core,
            self.connection.session_mut(),
            command,
        )?;
        Ok(())
    }

    pub fn poll(&mut self) -> Result<bool> {
        self.poll_with_update_budget(RuntimeUpdatePumpBudget::default())
    }

    pub fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        let poll_start = Instant::now();
        self.poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())?;
        Ok((1, elapsed_ms(poll_start.elapsed())))
    }

    pub fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool> {
        let poll_start = Instant::now();
        let pump_report =
            pump_client_connection_updates_report(&mut self.core, &mut self.connection, budget)?;
        let changed = pump_report.apply_report.changed;
        self.core.finish_poll_diagnostics(
            RuntimePollTiming {
                total_ms: elapsed_ms(poll_start.elapsed()),
                drain_updates_ms: pump_report.drain_updates_ms,
                producer_read_ms: pump_report.producer_read_ms,
                producer_decode_ms: pump_report.producer_decode_ms,
                producer_inbound_frame_sequence: pump_report.producer_inbound_frame_sequence,
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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
        deadline: MonotonicDeadline,
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

    pub fn resident_section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.core.resident_section_metadata()
    }

    pub fn cached_section_count(&self) -> usize {
        self.core.cached_section_count()
    }

    pub fn mark_all_render_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        self.core
            .mark_all_render_sections_dirty_for_resource_rebuild()
    }

    /// Recompile every resident render section and return the transient full
    /// mesh batch for a one-shot GPU (re)upload, without the resident cache ever
    /// retaining CPU meshes (docs/tactical/163).
    ///
    /// docs/tactical/167: this is a renderer/surface **resource-rebuild** path,
    /// not a startup seeding path — the name says so. Startup callers must take
    /// their draw seed from the startup pump's render seed
    /// ([`NativeSessionStartupCompletion::startup_sections`]); the last startup
    /// callers were migrated off this in Slice 3. Do not add startup uses.
    pub fn recompile_all_render_section_meshes_for_resource_rebuild(
        &mut self,
        camera_position: Vec3,
    ) -> Result<Vec<TexturedRenderSectionMesh>> {
        self.mark_all_render_sections_dirty_for_resource_rebuild();
        Ok(self
            .sync_all_render_sections(camera_position)?
            .rebuilt_sections)
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

impl<S> RemoteDedicatedSceneRuntime<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: MonotonicDeadline,
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

pub fn build_local_integrated_client_runtime(
    options: LocalIntegratedSceneOptions,
) -> Result<ClientRuntime> {
    let mut runtime = SingleViewRuntime::local_integrated_with_seed(
        options.seed,
        options.center,
        options.render_distance,
        options.chunk_tracking_radius(),
    );
    let mut runner = NativeIntegratedServerRunner::new(native_runner_config(&options))
        .context("failed to start local integrated server runner")?;
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
            .context("failed to send local integrated chunk view command")?;
        runtime.apply_server_updates(drain_integrated_server_runner_until_idle(&mut runner)?);
    }
    runner
        .join_shutdown()
        .context("failed to stop local integrated server runner")?;
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
                .context("failed to drain local integrated server updates")?,
        );
        let diagnostics = runner
            .poll_diagnostics()
            .context("failed to poll local integrated server diagnostics")?;
        if runner_idle(&diagnostics) {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for local integrated server jobs");
        }
        if diagnostics.update_queue_depth == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn native_runner_config(
    options: &LocalIntegratedSceneOptions,
) -> NativeIntegratedServerRunnerConfig {
    let mut config = NativeIntegratedServerRunnerConfig::new(options.seed)
        .with_world_generation_profile(options.world_generation_profile)
        .with_world_topology(options.world_topology)
        .with_world_behavior_profile(options.world_behavior_profile)
        .with_lighting_enabled(options.lighting_enabled)
        .with_light_status_batch_size(options.light_status_batch_size)
        .with_debug_passive_showcase(options.debug_passive_showcase)
        .with_debug_auxiliary_player_script(options.debug_auxiliary_player_script)
        .with_day_time(options.day_time_override)
        .with_day_time_frozen(options.freeze_time)
        .with_scheduled_fluid_ticks_frozen(options.freeze_scheduled_fluid_ticks)
        .with_local_integrated_chunk_tracking()
        .with_adaptive_chunk_publication_budget(options.adaptive_chunk_publication_budget)
        .with_world_storage(options.world_storage.clone())
        .with_observer_only(options.observer_only)
        .with_cadence_derived_tick_interval(options.cadence);
    if let Some(identity) = options.local_player_identity.clone() {
        config = config.with_local_player_identity(identity);
    }
    config
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

fn assert_no_real_far_lod_overlap(
    traversal_ready_real_chunks: &BTreeSet<ChunkPos>,
    visible_far_lod_tiles: &BTreeSet<LodTileKey>,
) {
    if let Some(tile) = visible_far_lod_tiles
        .iter()
        .find(|tile| traversal_ready_real_chunks.contains(&tile.chunk))
    {
        panic!(
            "far LOD invariant violated: chunk ({}, {}) is visible as far LOD while real terrain is traversal-ready; real and LOD terrain must never co-render",
            tile.chunk.x, tile.chunk.z
        );
    }
}

fn far_lod_runtime_settle_snapshot(
    core: &SingleViewRuntime,
    cache: &FarTerrainLodCache,
    coverage: &LodCoverageCoordinator,
    camera_position: Vec3,
) -> FarLodRuntimeSettleSnapshot {
    FarLodRuntimeSettleSnapshot {
        producer: cache.settle_snapshot(),
        loaded_chunks: core.client().loaded_chunk_positions().collect(),
        traversal_ready_sections: core.traversal_ready_render_section_keys(camera_position),
        suppressed_chunks: coverage.normal_coverage().drawable_chunks().collect(),
    }
}

impl<S> SceneRuntimeService for NativeSceneServices<S>
where
    S: RemoteDedicatedServerSession + 'static,
{
    fn host_mode(&self) -> SingleViewHostMode {
        NativeSceneServices::host_mode(self)
    }

    fn host_label(&self) -> &'static str {
        NativeSceneServices::host_label(self)
    }

    fn core(&self) -> &SingleViewRuntime {
        NativeSceneServices::core(self)
    }

    fn core_mut(&mut self) -> &mut SingleViewRuntime {
        NativeSceneServices::core_mut(self)
    }

    fn mesh_assets(&self) -> &TexturedMeshAssets {
        NativeSceneServices::mesh_assets(self)
    }

    fn replace_asset_epoch(
        &mut self,
        epoch: u64,
        mesh_assets: TexturedMeshAssets,
        sections: TexturedRenderSectionBuildReport,
    ) -> Result<()> {
        NativeSceneServices::replace_asset_epoch(self, epoch, mesh_assets, sections)
    }

    fn clear_far_lod(&mut self) {
        NativeSceneServices::clear_far_lod(self);
    }

    fn prepare_far_lod_frame(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        camera_position: Vec3,
        build_budget: usize,
        upload_budget: usize,
    ) -> Result<Option<&FarTerrainLodFrameUpdate>> {
        NativeSceneServices::prepare_far_lod_frame(
            self,
            config,
            seed,
            generation_profile,
            center,
            camera_position,
            build_budget,
            upload_budget,
        )
    }

    fn far_lod_stats(&self) -> FarTerrainLodProducerStats {
        NativeSceneServices::far_lod_stats(self)
    }

    fn far_lod_settle_snapshot(&self, camera_position: Vec3) -> FarLodRuntimeSettleSnapshot {
        NativeSceneServices::far_lod_settle_snapshot(self, camera_position)
    }

    fn lod_coverage_counters(&self) -> LodReplacementCounters {
        NativeSceneServices::lod_coverage_counters(self)
    }

    fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        NativeSceneServices::release_render_compile_jobs(self, count)
    }

    fn simulation_cadence(&self) -> Option<SimulationCadenceConfig> {
        NativeSceneServices::simulation_cadence(self)
    }

    fn set_simulation_cadence(&mut self, cadence: SimulationCadenceConfig) -> Result<bool> {
        NativeSceneServices::set_simulation_cadence(self, cadence)
    }

    fn loaded_chunk_count(&self) -> usize {
        NativeSceneServices::loaded_chunk_count(self)
    }

    fn set_chunk_view_with_update_policy_timed(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        NativeSceneServices::set_chunk_view_with_update_policy_timed(
            self,
            center,
            render_distance,
            chunk_tracking_radius,
            policy,
        )
    }

    fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        NativeSceneServices::send_gameplay_command_with_update_policy_timed(self, command, policy)
    }

    fn send_ephemeral_with_update_policy_timed(
        &mut self,
        message: ClientEphemeralMessage,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        NativeSceneServices::send_ephemeral_with_update_policy_timed(self, message, policy)
    }

    fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> Result<bool> {
        NativeSceneServices::poll_with_update_budget(self, budget)
    }

    fn poll_until_idle_with_timeout(&mut self, timeout: Duration) -> Result<(usize, f64)> {
        NativeSceneServices::poll_until_idle_with_timeout(self, timeout)
    }

    fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        NativeSceneServices::sync_render_sections_with_completed_result_acceptance_timed(
            self,
            camera_position,
            completed_result_accept_budget,
        )
    }

    fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: MonotonicDeadline,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        NativeSceneServices::sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
            self,
            camera_position,
            deadline,
            completed_result_accept_budget,
        )
    }

    fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: Vec3,
        deadline: MonotonicDeadline,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate> {
        NativeSceneServices::sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
            self,
            camera_position,
            deadline,
            max_compile_requests,
            completed_result_accept_budget,
        )
    }

    fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        NativeSceneServices::sync_all_render_sections(self, camera_position)
    }

    fn resident_section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        NativeSceneServices::resident_section_metadata(self)
    }

    fn cached_section_count(&self) -> usize {
        NativeSceneServices::cached_section_count(self)
    }

    fn mark_all_render_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        NativeSceneServices::mark_all_render_sections_dirty_for_resource_rebuild(self)
    }

    fn recompile_all_render_section_meshes_for_resource_rebuild(
        &mut self,
        camera_position: Vec3,
    ) -> Result<Vec<TexturedRenderSectionMesh>> {
        NativeSceneServices::recompile_all_render_section_meshes_for_resource_rebuild(
            self,
            camera_position,
        )
    }

    fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        NativeSceneServices::traversal_ready_render_section_keys(self, camera_position)
    }

    fn sky_clear_color(&self) -> wgpu::Color {
        NativeSceneServices::sky_clear_color(self)
    }

    fn render_compile_pending_job_count(&self) -> usize {
        NativeSceneServices::render_compile_pending_job_count(self)
    }

    fn render_compile_max_pending_job_count(&self) -> usize {
        NativeSceneServices::render_compile_max_pending_job_count(self)
    }

    fn render_compile_available_pending_job_slots(&self) -> usize {
        NativeSceneServices::render_compile_available_pending_job_slots(self)
    }

    fn render_compile_queue_health(&self) -> RenderSectionCompileQueueHealth {
        NativeSceneServices::render_compile_queue_health(self)
    }

    fn view_readiness_overlay(&self) -> Option<LoadingProgressOverlay> {
        NativeSceneServices::view_readiness_overlay(self)
    }

    fn flush_persistence(&mut self) -> Result<usize> {
        NativeSceneServices::flush_persistence(self)
    }

    fn promote_observer_to_player(&mut self) -> Result<()> {
        match self {
            NativeSceneServices::Local(runtime) => runtime.promote_observer_to_player(),
            NativeSceneServices::RemoteDedicated(_) => {
                anyhow::bail!("remote dedicated runtime cannot promote a local observer")
            }
        }
    }

    fn demote_player_to_observer(&mut self) -> Result<()> {
        match self {
            NativeSceneServices::Local(runtime) => runtime.demote_player_to_observer(),
            NativeSceneServices::RemoteDedicated(_) => {
                anyhow::bail!("remote dedicated runtime cannot demote a local player")
            }
        }
    }

    fn debug_break_observed_block(&mut self, pos: mclone_core::BlockPos) -> Result<bool> {
        match self {
            NativeSceneServices::Local(runtime) => runtime.debug_break_observed_block(pos),
            NativeSceneServices::RemoteDedicated(_) => {
                anyhow::bail!("remote dedicated runtime has no local observer debug control")
            }
        }
    }

    fn refresh_startup_diagnostics(&mut self) -> Result<()> {
        NativeSceneServices::refresh_startup_runner_diagnostics(self)
    }

    fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        NativeSceneServices::startup_progress_overlay(self)
    }

    fn startup_host_ready(&self, policy: StartupReadinessPolicy, camera_position: Vec3) -> bool {
        NativeSceneServices::startup_host_ready(self, policy, camera_position)
    }

    fn stats(&self) -> SingleViewRuntimeStats {
        NativeSceneServices::stats(self)
    }
}

impl<S> From<NativeSessionServices<S>> for SceneSessionRuntime
where
    S: RemoteDedicatedServerSession + 'static,
{
    fn from(runtime: NativeSessionServices<S>) -> Self {
        let descriptor = runtime
            .active_session()
            .cloned()
            .expect("native runtime conversion requires an active session");
        SceneSessionRuntime::from_active_service(descriptor, Box::new(runtime.into_runtime()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::render_assets::extracted_asset_root;

    static AUTHORED_STARTUP_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn time_update(day_time: u64) -> ServerUpdate {
        ServerUpdate::TimeUpdate {
            game_time: day_time.saturating_add(100),
            day_time,
            daylight_cycle_running: true,
        }
    }

    #[derive(Debug)]
    enum NoRemoteSession {}

    impl RemoteDedicatedServerSession for NoRemoteSession {
        fn send_command_only(&mut self, _command: ClientCommand) -> Result<()> {
            match *self {}
        }

        fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>> {
            match *self {}
        }

        fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics {
            match *self {}
        }

        fn reconnect(&mut self) -> Result<()> {
            match *self {}
        }
    }

    #[test]
    fn real_and_far_lod_visibility_may_be_disjoint() {
        let real = BTreeSet::from([ChunkPos::new(0, 0)]);
        let lod = BTreeSet::from([LodTileKey::new(ChunkPos::new(1, 0), 1)]);

        assert_no_real_far_lod_overlap(&real, &lod);
    }

    #[test]
    #[should_panic(expected = "real and LOD terrain must never co-render")]
    fn real_and_far_lod_visibility_overlap_panics() {
        let overlap = ChunkPos::new(3, -2);
        let real = BTreeSet::from([overlap]);
        let lod = BTreeSet::from([LodTileKey::new(overlap, 3)]);

        assert_no_real_far_lod_overlap(&real, &lod);
    }

    #[test]
    fn integrated_world_session_storage_defaults_to_transient_storage() {
        let options = LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 2)
            .with_persistent_world_dir("/tmp/old-world")
            .with_adaptive_chunk_publication_budget(true)
            .with_integrated_world_session_storage(IntegratedWorldSessionStorage::transient());

        assert_eq!(
            options.world_storage,
            NativeIntegratedServerWorldStorage::Transient
        );
        assert!(!options.adaptive_chunk_publication_budget);
    }

    #[test]
    fn integrated_world_session_storage_applies_persistent_dir_and_adaptive_budget() {
        let world_dir = PathBuf::from("/tmp/mclone-worlds/world");
        let storage = IntegratedWorldSessionStorage::from_world_dir(Some(world_dir.as_path()))
            .with_adaptive_chunk_publication_budget(true);
        assert_eq!(storage.persistent_world_dir(), Some(world_dir.as_path()));
        assert!(storage.adaptive_chunk_publication_budget());

        let options = LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 2)
            .with_integrated_world_session_storage(storage);

        assert_eq!(
            options.world_storage,
            NativeIntegratedServerWorldStorage::Persistent { dir: world_dir }
        );
        assert!(options.adaptive_chunk_publication_budget);
    }

    #[derive(Debug)]
    struct PollableRemoteSession {
        ready_drains: VecDeque<Option<Result<Vec<ServerUpdate>>>>,
        send_count: usize,
        try_drain_count: usize,
        reconnect_count: usize,
    }

    impl PollableRemoteSession {
        fn new(ready_drains: Vec<Option<Result<Vec<ServerUpdate>>>>) -> Self {
            Self {
                ready_drains: ready_drains.into(),
                send_count: 0,
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

        fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>> {
            self.try_drain_count += 1;
            match self.ready_drains.pop_front().unwrap_or(None) {
                Some(result) => result.map(RemoteServerUpdateBatch::from_updates).map(Some),
                None => Ok(None),
            }
        }

        fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics {
            RemoteUpdateQueueMetrics::default()
        }

        fn reconnect(&mut self) -> Result<()> {
            self.reconnect_count += 1;
            Ok(())
        }
    }

    #[test]
    fn local_integrated_scene_options_apply_java_tracking_radius() {
        let options = LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 2);

        assert_eq!(options.chunk_tracking_radius(), 3);
    }

    #[test]
    fn local_integrated_scene_options_can_use_java_initial_spawn_center() {
        let options = LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 2)
            .with_initial_spawn_center();

        assert_eq!(
            options.center,
            initial_spawn_center_for_descriptor(
                12345,
                WorldGenerationProfile::Overworld,
                HorizontalTopology::UNBOUNDED,
            )
        );
    }

    #[test]
    fn local_integrated_scene_options_use_selected_profile_spawn_center() {
        let options = LocalIntegratedSceneOptions::new(12345, ChunkPos::new(19, -20), 2)
            .with_world_generation_profile(WorldGenerationProfile::SmallIslandV1)
            .with_initial_spawn_center();

        assert_eq!(options.center, ChunkPos::new(0, 0));
    }

    #[test]
    fn local_integrated_scene_options_apply_topology_to_mclone_spawn() {
        let topology = HorizontalTopology::cylinder_x(0, 384);
        let options = LocalIntegratedSceneOptions::new(-98_765, ChunkPos::new(19, -20), 2)
            .with_world_generation_profile(WorldGenerationProfile::McloneOverworldV1)
            .with_world_topology(topology)
            .with_initial_spawn_center();

        assert_eq!(
            options.center,
            initial_spawn_center_for_descriptor(
                -98_765,
                WorldGenerationProfile::McloneOverworldV1,
                topology,
            )
        );
        assert_eq!(
            topology.canonicalize_chunk(options.center),
            Some(options.center)
        );
    }

    #[test]
    fn native_runner_config_derives_tick_interval_from_cadence_host_rate() {
        let cadence = SimulationCadenceConfig::new(60, 20, 60);
        let options =
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 2).with_cadence(cadence);

        let config = native_runner_config(&options);

        assert_eq!(config.cadence, cadence);
        assert_eq!(config.tick_interval, Duration::from_nanos(16_666_667));
    }

    #[test]
    fn native_runner_config_forwards_light_status_batch_size() {
        let options = LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 2)
            .with_light_status_batch_size(5);

        let config = native_runner_config(&options);

        assert_eq!(config.light_status_batch_size, 5);
    }

    #[test]
    fn build_local_integrated_client_runtime_loads_center_chunk_without_assets() {
        let client = build_local_integrated_client_runtime(LocalIntegratedSceneOptions::new(
            12345,
            ChunkPos::new(0, 0),
            0,
        ))
        .unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(client.chunk_snapshot(ChunkPos::new(0, 0)).is_some());
    }

    #[test]
    fn local_integrated_runtime_loads_center_chunk() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalIntegratedSceneRuntime::new(LocalIntegratedSceneOptions::new(
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

        let mut runtime = LocalIntegratedSceneRuntime::new(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
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
        let submission = runtime
            .send_gameplay_command_with_update_policy_timed(
                command,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .unwrap();
        let timing = submission.timing;
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

        let mut runtime = LocalIntegratedSceneRuntime::new(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
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

        let mut runtime = LocalIntegratedSceneRuntime::new(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
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
    fn remote_poll_checks_stream_without_command_response_state() {
        if !extracted_asset_root().exists() {
            return;
        }

        let center = ChunkPos::new(0, 0);
        let mesh_assets = load_textured_mesh_assets().unwrap();
        let session = PollableRemoteSession::new(vec![None, Some(Ok(vec![time_update(6000)]))]);
        let mut runtime = RemoteDedicatedSceneRuntime::with_mesh_assets(
            SingleViewHostOptions::new(center, 0),
            session,
            mesh_assets,
        )
        .unwrap();

        assert_eq!(runtime.connection.session.send_count, 1);
        assert_eq!(runtime.connection.session.try_drain_count, 0);
        assert_eq!(runtime.core().day_time(), 0);

        let command = ClientCommand::SetChunkView(crate::chunk_view(
            ChunkPos::new(1, 0),
            0,
            chunk_tracking_radius_for_render_distance(0),
        ));
        let submission = runtime
            .send_gameplay_command_with_update_policy_timed(
                command,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .unwrap();
        let timing = submission.timing;

        assert_eq!(timing.drain_updates_ms, 0.0);
        assert_eq!(timing.updates, 0);
        assert_eq!(runtime.connection.session.send_count, 2);

        assert!(!runtime.poll().unwrap());
        let diagnostics = runtime.core().last_poll_diagnostics();
        assert_eq!(diagnostics.server_update_queue_depth, 0);
        assert_eq!(diagnostics.updates, 0);
        assert!(!diagnostics.update_pump_stalled);
        assert_eq!(runtime.connection.session.try_drain_count, 1);
        assert_eq!(runtime.core().day_time(), 0);

        let _changed = runtime.poll().unwrap();
        let diagnostics = runtime.core().last_poll_diagnostics();
        assert_eq!(diagnostics.server_update_queue_depth, 0);
        assert_eq!(diagnostics.updates, 1);
        assert_eq!(runtime.connection.session.try_drain_count, 3);
        assert_eq!(runtime.core().day_time(), 6000);
    }

    #[test]
    fn explicit_remote_reconnect_clears_queued_stream_state_before_resync() {
        if !extracted_asset_root().exists() {
            return;
        }

        let center = ChunkPos::new(0, 0);
        let mesh_assets = load_textured_mesh_assets().unwrap();
        let session = PollableRemoteSession::new(Vec::new());
        let mut runtime = RemoteDedicatedSceneRuntime::with_mesh_assets(
            SingleViewHostOptions::new(center, 0),
            session,
            mesh_assets,
        )
        .unwrap();
        runtime
            .connection
            .push_update_batch(
                RemoteServerUpdateBatch::from_updates(vec![time_update(1), time_update(2)]),
                false,
            )
            .unwrap();
        assert_eq!(runtime.connection.queued_updates.len(), 2);

        runtime.reconnect_and_resync().unwrap();

        assert!(runtime.connection.queued_updates.is_empty());
        assert_eq!(runtime.connection.queued_update_bytes, 0);
        assert_eq!(runtime.connection.session.reconnect_count, 1);
        assert_eq!(runtime.connection.session.send_count, 2);
        assert_eq!(runtime.core().command_count(), 2);
    }

    #[test]
    fn local_chunk_view_churn_attributes_unload_apply() {
        if !extracted_asset_root().exists() {
            return;
        }

        let render_distance = 1;
        let tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
        let mut runtime = LocalIntegratedSceneRuntime::new(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), render_distance)
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
    fn local_integrated_runtime_applies_simulation_cadence_control() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = LocalIntegratedSceneRuntime::new(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
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

    fn wait_for_update_queue_depth(runtime: &LocalIntegratedSceneRuntime, min_depth: usize) {
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
        let mut pump = LocalIntegratedStartupPump::with_mesh_assets(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
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
                // docs/tactical/167: playable readiness must imply a non-empty
                // startup render seed with at least one drawable section, so the
                // platform adapter can seed draw resources without recompiling.
                assert!(step.render_seed_section_count > 0);
                assert!(step.render_seed_drawable_section_count > 0);
                let (runtime, startup_sections) = pump.into_runtime_with_startup_sections();
                assert!(!startup_sections.is_empty());
                assert!(
                    startup_sections.iter().any(|section| !section.is_empty()),
                    "startup seed must carry at least one drawable section"
                );
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
    fn startup_pump_prewarms_far_lod_before_playable() {
        if !extracted_asset_root().exists() {
            return;
        }

        let prewarm = StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::enabled(), true)
            .with_extra_chunks(3);
        let mesh_assets = load_textured_mesh_assets().unwrap();
        let mut pump = LocalIntegratedStartupPump::with_mesh_assets(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false)
                .with_startup_lod_prewarm(prewarm),
            mesh_assets,
        )
        .unwrap();
        let camera_position = Vec3::new(8.0, 80.0, 8.0);

        for _ in 0..2_000 {
            let step = pump.step(camera_position).unwrap();
            assert!(step.lod_prewarm_enabled);
            // Spawn authority never depends on prewarm; playable requires both
            // spawn authority and the prewarm settling (complete or timed out).
            if step.playable_ready {
                assert!(step.spawn_authority_ready);
                assert!(step.lod_prewarm_complete || step.lod_prewarm_timeout);
                assert!(step.startup_lod_tiles_target > 0);
                assert!(step.startup_lod_tiles_ready <= step.startup_lod_tiles_target);
                if step.lod_prewarm_complete {
                    assert_eq!(step.startup_lod_tiles_ready, step.startup_lod_tiles_target);
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        panic!("startup pump did not reach playable center with prewarm enabled");
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
        let mut pump = LocalIntegratedStartupPump::with_mesh_assets(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), render_distance)
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
                    .runtime()
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

    fn stone_test_snapshot(pos: ChunkPos) -> ChunkSnapshot {
        use mclone_core::CHUNK_SECTION_VOLUME;
        // A fully-solid single section: its outer faces have no loaded neighbor,
        // so the compiled render section carries drawable geometry.
        ChunkSnapshot::from_block_state_ids(
            pos,
            mclone_core::ChunkStatus::Full,
            mclone_core::ChunkRevision(1),
            0,
            16,
            &vec![BlockStateId(1); CHUNK_SECTION_VOLUME],
        )
    }

    #[test]
    fn startup_readiness_policy_defaults_to_playable() {
        assert_eq!(
            StartupReadinessPolicy::default(),
            StartupReadinessPolicy::Playable
        );
    }

    #[test]
    fn native_startup_pump_local_reaches_playable_through_shared_seed() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mesh_assets = load_textured_mesh_assets().unwrap();
        let mut pump = NativeSessionStartupPump::<LocalOnlySession>::local_with_mesh_assets(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0)
                .with_lighting_enabled(false),
            mesh_assets,
        )
        .unwrap();
        let camera_position = Vec3::new(8.0, 80.0, 8.0);

        for _ in 0..2_000 {
            let step = pump.step(camera_position).unwrap();
            if step.startup_ready {
                assert_eq!(step.host_mode, SingleViewHostMode::LocalIntegrated);
                assert!(step.host_ready);
                assert!(step.local_progress.as_ref().unwrap().playable_ready);
                // Playable readiness implies a non-empty startup render seed with
                // at least one drawable section (docs/tactical/167).
                assert!(step.render_seed_drawable_section_count > 0);
                assert!(step.cached_section_count > 0);

                let completion = pump.complete();
                assert!(completion.final_step.startup_ready);
                assert!(!completion.startup_sections.is_empty());
                assert!(
                    completion
                        .startup_sections
                        .iter()
                        .any(|section| !section.is_empty()),
                    "startup seed must carry at least one drawable section"
                );
                // Accepted compile jobs are released by the pump each step, not
                // held by the seed: capacity is fully available after completion.
                assert_eq!(
                    completion
                        .runtime
                        .render_compile_available_pending_job_slots(),
                    completion.runtime.render_compile_max_pending_job_count(),
                );
                assert_eq!(completion.runtime.loaded_chunk_count(), 1);
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        panic!("shared local startup pump did not reach playable");
    }

    #[test]
    fn authored_sqlite_startup_reaches_playable_through_native_runner() {
        if !extracted_asset_root().exists() {
            return;
        }

        let sequence = AUTHORED_STARTUP_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mclone-authored-native-startup-{}-{sequence}",
            std::process::id()
        ));
        let manifest = mclone_server::write_authored_world_fixture_dir(
            &root,
            mclone_server::AuthoredWorldFixtureKind::Table,
        )
        .unwrap();
        let mesh_assets = load_textured_mesh_assets().unwrap();
        let mut pump = NativeSessionStartupPump::<LocalOnlySession>::local_with_mesh_assets(
            LocalIntegratedSceneOptions::new(manifest.seed, ChunkPos::new(0, 0), 2)
                .with_world_generation_profile(manifest.world_generation_profile)
                .with_persistent_world_dir(&root)
                .with_debug_passive_showcase(false)
                .with_lighting_enabled(false),
            mesh_assets,
        )
        .unwrap();
        let camera_position = Vec3::new(8.0, 104.0, 8.0);

        let mut last_step = None;
        for _ in 0..2_000 {
            let step = pump.step(camera_position).unwrap();
            if step.startup_ready {
                assert!(step.host_ready);
                assert!(step.render_seed_drawable_section_count > 0);
                assert!(pump.runtime().loaded_chunk_count() >= 1);
                let completion = pump.complete();
                drop(completion);
                std::fs::remove_dir_all(root).unwrap();
                return;
            }
            last_step = Some(step);
            std::thread::sleep(Duration::from_millis(1));
        }

        let loaded_chunk_count = pump.runtime().loaded_chunk_count();
        drop(pump);
        let _ = std::fs::remove_dir_all(root);
        panic!(
            "authored native startup did not become playable: loaded_chunks={loaded_chunk_count} {last_step:?}"
        );
    }

    // docs/tactical/167 Slice 4 tripwire: a startup pose correction that teleports
    // the camera beyond render distance moves chunk interest, and the reconciled
    // drive re-pumps at the new camera so the completion seed has drawable sections
    // for the *final* camera — not just the pre-teleport startup camera. This is the
    // full clear-ready/re-pump invariant: a far XR startup view pose (validated
    // on-device at ~6 chunks) or a far server spawn correction leaves nothing drawn
    // at the ready frame without it.
    #[test]
    fn reconciled_startup_repumps_seed_to_the_final_teleported_camera() {
        use crate::camera_reconcile::{
            EngineCameraCommitContext, EngineCameraCommitTiming,
            apply_pending_engine_camera_position_updates, commit_engine_camera_player_pose,
        };
        use mclone_render_session::EngineCameraController;

        if !extracted_asset_root().exists() {
            return;
        }

        let mesh_assets = load_textured_mesh_assets().unwrap();
        let render_distance = 1;
        let pump = NativeSessionStartupPump::<LocalOnlySession>::local_with_mesh_assets(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), render_distance)
                .with_lighting_enabled(false),
            mesh_assets,
        )
        .unwrap();

        // Startup camera at spawn chunk (0, 0); reconcile teleports to (3, 0), three
        // chunks away — beyond render distance, so the spawn seed cannot cover it.
        let startup_camera = Vec3::new(8.0, 80.0, 8.0);
        let final_chunk = ChunkPos::new(3, 0);
        let mut applied = false;
        let completion = pump
            .drive_to_ready_reconciled(startup_camera, Duration::from_secs(60), |runtime| {
                let context = EngineCameraCommitContext::send_only("test");
                // Match the shared XR startup order: accept the server's initial
                // spawn correction before applying the physical startup pose.
                let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
                apply_pending_engine_camera_position_updates(runtime, &mut camera, context)?;
                let mut camera = EngineCameraController::spawn_for_chunk(final_chunk);
                let mut timing = EngineCameraCommitTiming::default();
                commit_engine_camera_player_pose(
                    runtime,
                    &mut camera,
                    context,
                    &system_monotonic_clock(),
                    Some(&mut timing),
                )?;
                assert!(timing.server_command_ms.is_finite());
                assert!(timing.position_updates_ms.is_finite());
                assert!(timing.interest_ms.is_finite());
                applied = true;
                let eye = camera.snapshot().eye;
                Ok(Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32))
            })
            .unwrap();

        assert!(applied, "reconcile closure must run after base readiness");
        assert_eq!(
            completion.runtime.interest_center(),
            final_chunk,
            "reconciliation must leave interest at the final camera chunk"
        );
        // The completion seed must carry drawable sections near the final camera,
        // proving the pump re-pumped at (3, 0) rather than shipping the (0, 0) seed.
        let near_final = completion
            .startup_sections
            .iter()
            .filter(|section| !section.is_empty())
            .filter(|section| {
                (section.key.chunk_x - final_chunk.x).abs() <= render_distance as i32
                    && (section.key.chunk_z - final_chunk.z).abs() <= render_distance as i32
            })
            .count();
        assert!(
            near_final > 0,
            "reconciled completion seed must cover the final teleported camera"
        );
    }

    // Tactical 168 Slice 7e tripwire: the old untimed shared sync used a
    // short-circuiting `pose_sent || apply_pending_corrections` expression,
    // while XR's timed fork always did both. The unified path must retain XR's
    // immediate correction behavior even when the camera also emits a pose.
    #[test]
    fn camera_commit_applies_pending_correction_after_pose_send() {
        use crate::camera_reconcile::{
            EngineCameraCommitContext, commit_engine_camera_player_pose,
        };
        use mclone_render_session::EngineCameraController;

        if !extracted_asset_root().exists() {
            return;
        }

        let mesh_assets = load_textured_mesh_assets().unwrap();
        let render_distance = 1;
        let pump = NativeSessionStartupPump::<LocalOnlySession>::local_with_mesh_assets(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), render_distance)
                .with_lighting_enabled(false),
            mesh_assets,
        )
        .unwrap();

        let startup_camera = Vec3::new(8.0, 80.0, 8.0);
        let attempted_chunk = ChunkPos::new(3, 0);
        let corrected_chunk = ChunkPos::new(0, 0);
        let completion = pump
            .drive_to_ready_reconciled(startup_camera, Duration::from_secs(60), |runtime| {
                let mut camera = EngineCameraController::spawn_for_chunk(attempted_chunk);
                commit_engine_camera_player_pose(
                    runtime,
                    &mut camera,
                    EngineCameraCommitContext::send_only("test"),
                    &system_monotonic_clock(),
                    None,
                )?;
                assert_eq!(camera.snapshot().chunk_pos, corrected_chunk);
                let eye = camera.snapshot().eye;
                Ok(Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32))
            })
            .unwrap();

        assert_eq!(completion.runtime.interest_center(), corrected_chunk);
    }

    #[test]
    fn native_startup_pump_remote_reaches_ready_without_poll_until_idle() {
        if !extracted_asset_root().exists() {
            return;
        }

        let center = ChunkPos::new(0, 0);
        let mesh_assets = load_textured_mesh_assets().unwrap();
        // The initial SetChunkView publication is received through the same
        // ready-only frame pump used after startup.
        let session = PollableRemoteSession::new(vec![Some(Ok(vec![
            ServerUpdate::WorldInfo {
                dimension: mclone_protocol::DimensionKey::overworld(),
                biome_zoom_seed: 1124,
                topology: mclone_core::HorizontalTopology::UNBOUNDED,
            },
            ServerUpdate::ChunkSnapshot(stone_test_snapshot(center)),
        ]))]);
        let mut pump = NativeSessionStartupPump::remote_dedicated_with_mesh_assets(
            RemoteSessionEndpoint::new("127.0.0.1:25565"),
            SingleViewHostOptions::new(center, 0),
            session,
            mesh_assets,
        )
        .unwrap();
        let camera_position = Vec3::new(8.0, 80.0, 8.0);

        for _ in 0..2_000 {
            let step = pump.step(camera_position).unwrap();
            if step.startup_ready {
                assert_eq!(step.host_mode, SingleViewHostMode::RemoteDedicated);
                assert!(step.host_ready);
                // Remote startup carries no local loading-progress overlay.
                assert!(step.local_progress.is_none());
                assert!(!step.lod_prewarm_enabled);
                assert!(step.render_seed_drawable_section_count > 0);

                let completion = pump.complete();
                assert!(
                    completion
                        .startup_sections
                        .iter()
                        .any(|section| !section.is_empty()),
                    "remote startup seed must carry at least one drawable section"
                );
                assert!(completion.runtime.loaded_chunk_count() > 0);
                match &*completion.runtime {
                    NativeSceneServices::RemoteDedicated(scene) => {
                        assert!(scene.connection.session.try_drain_count >= 2);
                    }
                    NativeSceneServices::Local(_) => panic!("expected a remote runtime"),
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        panic!("shared remote startup pump did not reach ready");
    }

    #[test]
    fn native_service_assembly_records_local_session() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = NativeSessionServices::<NoRemoteSession>::local(
            LocalIntegratedSceneOptions::new(12345, ChunkPos::new(0, 0), 0),
        )
        .unwrap();
        runtime.poll_until_idle().unwrap();

        assert_eq!(
            runtime.active_session(),
            Some(&ActiveSessionDescriptor::new_seed_local_world(12345))
        );
        assert_eq!(runtime.loaded_chunk_count(), 1);
    }

    #[test]
    fn native_service_assembly_local_interest_change_defers_updates_until_poll() {
        if !extracted_asset_root().exists() {
            return;
        }

        let initial_center = ChunkPos::new(0, 0);
        let next_center = ChunkPos::new(1, 0);
        let mut runtime = NativeSessionServices::<NoRemoteSession>::local(
            LocalIntegratedSceneOptions::new(12345, initial_center, 0).with_lighting_enabled(false),
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
