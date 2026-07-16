//! Shared authoritative realm server and local in-process session adapter.
//!
//! `RealmServer` owns gameplay, players, persistence, and publication without
//! knowing whether a session is local, TCP, or WebSocket. `LocalRealmSession`
//! is the in-memory adapter used by integrated hosts and tests.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::time::Duration;

use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos, ChunkSnapshot, Vec3d,
    block_to_chunk_coord, obfuscate_biome_zoom_seed,
};
#[cfg(feature = "physics-engine")]
use mclone_protocol::EntityRotation;
use mclone_protocol::{
    AcceptTeleportCommand, ChunkView, ClientCommand, ClientIdentity, DimensionKey, InteractionHand,
    MovePlayerCommand, PlayerActionCommand, PlayerActionKind, PlayerAppearance, PlayerModelKind,
    PlayerProfileId, RealmId, SequencedMovePlayerCommand, ServerUpdate, SessionCapabilities,
    SessionConfiguration, SetCarriedItemCommand, SetDebugHotbarSlotCommand,
    SetPlayerAppearanceCommand, UseItemOnCommand,
};
use mclone_worldgen::biome::OverworldBiomeSource;
use mclone_worldgen::block::{AIR, RawBlockId, block_name, generated_block_state_id};
use mclone_worldgen::prng::SimpleRandomSource;

use crate::NaturalSpawningDiagnostics;
#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
use crate::entity::spawning::dry_run::{
    NaturalSpawnDryRunDiagnostics, dry_run_creature_spawn_eligibility,
};
use crate::entity::spawning::live::{
    VOLATILE_CREATURE_SPAWN_MAX_SPAWNS_PER_TICK, VolatileCreatureSpawnDiagnostics,
    plan_volatile_creature_spawns,
};
use crate::entity::spawning::mob_category::MobCategory;
use crate::entity::spawning::natural::{
    NaturalSpawnChunkInputs, NaturalSpawnConfig, NaturalSpawnContext, plan_natural_spawns,
};
use crate::entity::spawning::spawn_state::SpawnState;
use crate::entity::{
    EntityTracking, ItemPickupTarget, MobPlayerTarget, RoutedEntityUpdate, ServerEntityState,
    ServerEntityStore,
};
use crate::falling_block::{
    BlockTickList, BlockTickPhaseReport, basic_falling_block_move,
    block_tick_requests_after_block_change,
};
use crate::game_mode::ServerInteractionContext;
use crate::inventory::ServerInventory;
use crate::persistence::{
    EntityChunkRecord, EntityPersistentId, PlayerRecord, PlayerRecordKey, StoreWriteOutcome,
    WorldMetadata,
};
#[cfg(feature = "physics-engine")]
use crate::physics_runtime::ServerPhysicsRuntime;
use crate::placement::DebugBlockItem;
use crate::player::{MovePlayerApplyResult, ServerPlayerState};
use crate::player_chunk_tracking::{PlayerChunkTracking, PlayerChunkTrackingPolicy};
use crate::players::{ServerPlayerId, ServerPlayerList};
use crate::remote_players::{RemotePlayerState, RemotePlayerTracking, RoutedRemotePlayerUpdate};
use crate::spawn::{SpawnColumnOrder, find_safe_surface_spawn_with_column_order};
use crate::timing::{simulation_timing_elapsed_us, simulation_timing_start};
use crate::{
    ChunkLoadingProgress, ChunkLoadingProgressSnapshot, ChunkLoadingProgressStats,
    ChunkPublicationBudgetConfig, ChunkRecord, ChunkScheduler, ChunkSchedulerEvent,
    ChunkSnapshotStore, ChunkStoreError, ChunkStoreResult, DimensionRegistry, FluidKind,
    FluidTickList, NullChunkSnapshotStore, PlayerChunkTrackingDiagnostics, ServerPhysicsStepReport,
    ServerPhysicsStepTiming, ServerPhysicsTickDiagnostics, ServerSimulationTickReport,
    ServerSimulationTickTiming, ServerTickReport, ServerTickTiming, WorldBehaviorProfile,
    WorldBlockPos, WorldGenerationProfile, WorldStore,
};

#[cfg(feature = "physics-engine")]
const DEBUG_PHYSICS_CUBE_EYE_HEIGHT: f64 = 1.5;
#[cfg(feature = "physics-engine")]
const DEBUG_PHYSICS_CUBE_SPAWN_DISTANCE: f64 = 1.25;
#[cfg(feature = "physics-engine")]
const DEBUG_PHYSICS_CUBE_LAUNCH_SPEED: f64 = 14.0;
#[cfg(feature = "physics-engine")]
const DEBUG_PHYSICS_CUBE_HALF_EXTENT: f64 = 0.5;
const DEFAULT_PHYSICS_STEPS_PER_GAMEPLAY_TICK: u32 = 3;
const DEFAULT_PHYSICS_STEP_DT_SECONDS: f64 = 1.0 / 60.0;
const NATURAL_SPAWN_TICK_SEED_MULTIPLIER: i64 = 6_364_136_223_846_793_005;

fn session_configuration(
    policy: PlayerChunkTrackingPolicy,
    capabilities: SessionCapabilities,
) -> SessionConfiguration {
    let (max_render_distance, max_chunk_tracking_radius) = policy.session_limits();
    SessionConfiguration::fixed_vanilla(
        max_render_distance,
        max_chunk_tracking_radius,
        capabilities,
    )
}

#[derive(Debug)]
pub struct RealmServer {
    realm_id: RealmId,
    dimensions: DimensionRegistry,
    seed: i64,
    biome_source: ServerBiomeSource,
    scheduler: ChunkScheduler,
    block_ticks: BlockTickList,
    pub(crate) liquid_ticks: FluidTickList,
    simulation_tick: u64,
    day_time: u64,
    do_daylight_cycle: bool,
    day_time_frozen: bool,
    day_time_debug_override: bool,
    world_metadata: Option<WorldMetadata>,
    world_metadata_dirty: bool,
    scheduled_fluid_ticks_frozen: bool,
    debug_passive_showcase_enabled: bool,
    volatile_natural_spawning_enabled: bool,
    world_behavior_profile: WorldBehaviorProfile,
    persistence_demo_jump_experience_enabled: bool,
    players: ServerPlayerList,
    debug_auxiliary_player_script: Option<DebugAuxiliaryPlayerScript>,
    chunk_tracking: PlayerChunkTracking,
    remote_players: RemotePlayerTracking,
    entities: ServerEntityStore,
    entity_tracking: EntityTracking,
    dirty_entity_chunks: BTreeSet<ChunkPos>,
    #[cfg(feature = "physics-engine")]
    physics: ServerPhysicsRuntime,
    #[cfg(feature = "physics-engine")]
    debug_physics_player_target: Option<CommandTarget>,
    loading_progress: ChunkLoadingProgress,
}

#[derive(Clone)]
struct ServerBiomeSource {
    seed: i64,
    source: OverworldBiomeSource,
}

impl ServerBiomeSource {
    fn new(seed: i64) -> Self {
        Self {
            seed,
            source: OverworldBiomeSource::new(seed, false, false),
        }
    }

    fn block_position_biome_definition(
        &self,
        block_x: i32,
        block_z: i32,
    ) -> mclone_worldgen::biome::BiomeDefinition {
        self.source
            .get_block_position_biome_definition(self.seed, block_x, block_z)
    }
}

impl std::fmt::Debug for ServerBiomeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerBiomeSource")
            .field("seed", &self.seed)
            .finish_non_exhaustive()
    }
}

/// Legacy mclone transient worlds start at the conventional `/time set day`
/// value. Typed persistent worlds initialize at vanilla's fresh-world value 0.
pub const INITIAL_DAY_TIME: u64 = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandTarget {
    Player(ServerPlayerId),
}

/// Shared Rust-authored validation actor. Platform adapters may only enable or
/// disable the script; all admission, appearance, and movement facts travel
/// through ordinary authoritative player commands.
#[derive(Clone, Copy, Debug)]
struct DebugAuxiliaryPlayerScript {
    player_id: ServerPlayerId,
    view_requested: bool,
    accepted_position: bool,
    current_position: Vec3d,
    move_sequence: u64,
}

impl DebugAuxiliaryPlayerScript {
    fn new(player_id: ServerPlayerId) -> Self {
        Self {
            player_id,
            view_requested: false,
            accepted_position: false,
            current_position: Vec3d::new(8.5, 66.0, 8.5),
            move_sequence: 0,
        }
    }
}

#[derive(Debug)]
struct NaturalSpawningEvaluation {
    player_positions: Vec<Vec3d>,
    chunk_inputs: NaturalSpawnChunkInputs,
    plan: crate::entity::spawning::natural::NaturalSpawnPlan,
    creature_state: crate::entity::spawning::spawn_state::CategorySpawnState,
    creature_cadence_ready: bool,
    creature_cap_has_room: bool,
    dry_run: NaturalSpawnDryRunDiagnostics,
}

#[derive(Debug)]
struct NaturalSpawningTickResult {
    diagnostics: NaturalSpawningDiagnostics,
    spawned_entities: Vec<ServerEntityState>,
}

impl RealmServer {
    pub fn new(seed: i64) -> Self {
        Self::with_chunk_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn new_in_realm(realm_id: RealmId, seed: i64) -> Self {
        Self::with_chunk_store_in_realm(realm_id, seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn local_integrated(seed: i64) -> Self {
        Self::with_player_chunk_tracking_policy(seed, PlayerChunkTrackingPolicy::java_max())
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self::with_chunk_store_in_realm(RealmId::LEGACY_SINGLE_REALM, seed, store)
    }

    pub fn with_chunk_store_in_realm(
        realm_id: RealmId,
        seed: i64,
        store: Box<dyn ChunkSnapshotStore>,
    ) -> Self {
        Self::with_scheduler_in_realm(realm_id, seed, ChunkScheduler::with_store(seed, store))
    }

    pub fn with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::with_world_store_in_realm(RealmId::LEGACY_SINGLE_REALM, seed, store)
    }

    pub fn with_world_store_in_realm(
        realm_id: RealmId,
        seed: i64,
        store: Box<dyn WorldStore>,
    ) -> Self {
        Self::with_scheduler_in_realm(
            realm_id,
            seed,
            ChunkScheduler::with_world_store(seed, store),
        )
    }

    pub fn local_integrated_with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::with_world_store(seed, store),
            PlayerChunkTrackingPolicy::java_max(),
        )
    }

    pub fn local_integrated_with_external_load_world_store(
        seed: i64,
        store: Box<dyn WorldStore>,
    ) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::with_external_load_world_store(seed, store),
            PlayerChunkTrackingPolicy::java_max(),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_with_threaded_world_store(
        seed: i64,
        store: Box<dyn WorldStore + Send>,
    ) -> ChunkStoreResult<Self> {
        Ok(Self::with_scheduler(
            seed,
            ChunkScheduler::try_with_threaded_world_store(seed, store)?,
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_with_threaded_sqlite_world_dir(
        seed: i64,
        world_dir: impl AsRef<Path>,
    ) -> ChunkStoreResult<Self> {
        let store = crate::persistence::SqliteWorldStore::open_world_dir(world_dir)?;
        Self::try_with_threaded_world_store(seed, Box::new(store))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn try_with_threaded_sqlite_world_dir_and_player_chunk_tracking_policy(
        seed: i64,
        world_dir: impl AsRef<Path>,
        policy: PlayerChunkTrackingPolicy,
    ) -> ChunkStoreResult<Self> {
        let store = crate::persistence::SqliteWorldStore::open_world_dir(world_dir)?;
        Ok(Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::try_with_threaded_world_store(seed, Box::new(store))?,
            policy,
        ))
    }

    pub(crate) fn with_player_chunk_tracking_policy(
        seed: i64,
        policy: PlayerChunkTrackingPolicy,
    ) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::new(seed),
            policy,
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub fn with_wasm_job_workers(seed: i64, config: WasmServerJobWorkerConfig) -> Self {
        Self::with_scheduler(
            seed,
            ChunkScheduler::with_wasm_job_workers(
                seed,
                Box::<NullChunkSnapshotStore>::default(),
                config,
            ),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub fn local_integrated_with_wasm_job_workers(
        seed: i64,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::with_wasm_job_workers(
                seed,
                Box::<NullChunkSnapshotStore>::default(),
                config,
            ),
            PlayerChunkTrackingPolicy::java_max(),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub fn local_integrated_with_world_store_and_wasm_job_workers(
        seed: i64,
        store: Box<dyn WorldStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::with_world_store_and_wasm_job_workers(seed, store, config),
            PlayerChunkTrackingPolicy::java_max(),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub fn local_integrated_with_external_load_world_store_and_wasm_job_workers(
        seed: i64,
        store: Box<dyn WorldStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            ChunkScheduler::with_external_load_world_store_and_wasm_job_workers(
                seed, store, config,
            ),
            PlayerChunkTrackingPolicy::java_max(),
        )
    }

    fn with_scheduler(seed: i64, scheduler: ChunkScheduler) -> Self {
        Self::with_scheduler_in_realm(RealmId::LEGACY_SINGLE_REALM, seed, scheduler)
    }

    fn with_scheduler_in_realm(realm_id: RealmId, seed: i64, scheduler: ChunkScheduler) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            realm_id,
            seed,
            scheduler,
            PlayerChunkTrackingPolicy::default(),
        )
    }

    fn with_scheduler_and_player_chunk_tracking_policy(
        realm_id: RealmId,
        seed: i64,
        scheduler: ChunkScheduler,
        policy: PlayerChunkTrackingPolicy,
    ) -> Self {
        let chunk_tracking = PlayerChunkTracking::new(policy);
        let loading_progress = ChunkLoadingProgress::new(runtime_chunk_target_status(&scheduler));
        Self {
            realm_id,
            dimensions: DimensionRegistry::single_overworld(
                seed,
                WorldGenerationProfile::default(),
            ),
            seed,
            biome_source: ServerBiomeSource::new(seed),
            scheduler,
            block_ticks: BlockTickList::new(),
            liquid_ticks: FluidTickList::new(),
            simulation_tick: 0,
            day_time: INITIAL_DAY_TIME,
            do_daylight_cycle: true,
            day_time_frozen: false,
            day_time_debug_override: false,
            world_metadata: None,
            world_metadata_dirty: false,
            scheduled_fluid_ticks_frozen: false,
            debug_passive_showcase_enabled: true,
            volatile_natural_spawning_enabled: true,
            world_behavior_profile: WorldBehaviorProfile::default(),
            persistence_demo_jump_experience_enabled: false,
            players: ServerPlayerList::default(),
            debug_auxiliary_player_script: None,
            chunk_tracking,
            remote_players: RemotePlayerTracking::default(),
            entities: ServerEntityStore::default(),
            entity_tracking: EntityTracking::default(),
            dirty_entity_chunks: BTreeSet::new(),
            #[cfg(feature = "physics-engine")]
            physics: ServerPhysicsRuntime::new(),
            #[cfg(feature = "physics-engine")]
            debug_physics_player_target: None,
            loading_progress,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn realm_id(&self) -> RealmId {
        self.realm_id
    }

    pub fn dimensions(&self) -> &DimensionRegistry {
        &self.dimensions
    }

    pub fn dimension_definition(&self, key: &DimensionKey) -> Option<&crate::DimensionDefinition> {
        self.dimensions.get(key)
    }

    pub const fn simulation_tick(&self) -> u64 {
        self.simulation_tick
    }

    /// Durable vanilla `gameTime` semantics. `simulation_tick()` remains as a
    /// compatibility name while callers migrate to the world-clock vocabulary.
    pub const fn game_time(&self) -> u64 {
        self.simulation_tick
    }

    /// Authoritative world day-time in ticks, driving the day/night cycle.
    pub const fn day_time(&self) -> u64 {
        self.day_time
    }

    /// Set the authoritative day-time. Debug hook for forcing a starting time.
    pub fn set_day_time(&mut self, day_time: u64) {
        self.day_time = day_time;
        self.day_time_debug_override = true;
    }

    /// Durable daylight gamerule. Debug `--freeze-time` is layered separately
    /// and never changes this saved value.
    pub fn set_do_daylight_cycle(&mut self, enabled: bool) {
        if self.do_daylight_cycle == enabled {
            return;
        }
        self.do_daylight_cycle = enabled;
        if self.world_metadata.is_some() {
            self.world_metadata_dirty = true;
        }
    }

    pub const fn do_daylight_cycle(&self) -> bool {
        self.do_daylight_cycle
    }

    pub const fn daylight_cycle_running(&self) -> bool {
        self.do_daylight_cycle && !self.day_time_frozen
    }

    /// Freeze or resume the day/night clock. While frozen, simulation ticks leave
    /// `day_time` unchanged (debug hook for inspecting a fixed time of day).
    pub fn set_day_time_frozen(&mut self, frozen: bool) {
        self.day_time_frozen = frozen;
    }

    pub fn initialize_world_metadata_blocking(&mut self) -> ChunkStoreResult<WorldMetadata> {
        self.initialize_world_metadata_at_unix_millis(current_unix_millis())
    }

    pub fn initialize_world_metadata_at_unix_millis(
        &mut self,
        now_unix_millis: u64,
    ) -> ChunkStoreResult<WorldMetadata> {
        if let Some(metadata) = &self.world_metadata {
            return Ok(metadata.clone());
        }
        let load = self.scheduler.load_world_metadata_blocking()?;
        let metadata_was_present = load.record.is_some();
        let requested_generation = self.scheduler.world_generation_profile();
        let mut metadata = match load.record {
            Some(metadata) => {
                validate_world_metadata(
                    &metadata,
                    self.seed,
                    requested_generation,
                    self.world_behavior_profile,
                )?;
                metadata
            }
            None if load.legacy_records_present => WorldMetadata::legacy_mclone(
                self.seed,
                requested_generation,
                self.world_behavior_profile,
                now_unix_millis,
                INITIAL_DAY_TIME,
            ),
            None => WorldMetadata::new(
                self.seed,
                requested_generation,
                self.world_behavior_profile,
                now_unix_millis,
            ),
        };
        if !metadata_was_present {
            match self
                .scheduler
                .save_world_metadata_blocking(metadata.clone())?
            {
                StoreWriteOutcome::Written | StoreWriteOutcome::Superseded => {}
                StoreWriteOutcome::SkippedOnClose => {
                    return Err(ChunkStoreError::Closed(
                        "world metadata initialization was skipped on close".to_owned(),
                    ));
                }
            }
        }
        self.simulation_tick = metadata.game_time;
        self.day_time = metadata.day_time;
        self.do_daylight_cycle = metadata.do_daylight_cycle;
        self.day_time_debug_override = false;
        self.world_metadata_dirty = false;
        metadata.last_played_unix_millis = metadata
            .last_played_unix_millis
            .max(metadata.created_unix_millis);
        self.world_metadata = Some(metadata.clone());
        Ok(metadata)
    }

    pub fn world_metadata(&self) -> Option<&WorldMetadata> {
        self.world_metadata.as_ref()
    }

    pub fn save_world_metadata_blocking(&mut self) -> ChunkStoreResult<usize> {
        self.save_world_metadata_at_unix_millis(current_unix_millis())
    }

    pub fn save_world_metadata_at_unix_millis(
        &mut self,
        now_unix_millis: u64,
    ) -> ChunkStoreResult<usize> {
        let Some(mut metadata) = self.world_metadata.clone() else {
            return Ok(0);
        };
        if !self.world_metadata_dirty {
            return Ok(0);
        }
        metadata.revision = metadata.revision.saturating_add(1);
        metadata.last_played_unix_millis = now_unix_millis.max(metadata.created_unix_millis);
        metadata.game_time = self.simulation_tick;
        if !self.day_time_debug_override {
            metadata.day_time = self.day_time;
        }
        metadata.do_daylight_cycle = self.do_daylight_cycle;
        match self
            .scheduler
            .save_world_metadata_blocking(metadata.clone())?
        {
            StoreWriteOutcome::Written | StoreWriteOutcome::Superseded => {}
            StoreWriteOutcome::SkippedOnClose => {
                return Err(ChunkStoreError::Closed(
                    "world metadata save was skipped on close".to_owned(),
                ));
            }
        }
        self.world_metadata = Some(metadata);
        self.world_metadata_dirty = false;
        Ok(1)
    }

    /// Freeze or resume scheduled fluid ticks. While frozen, due water/lava
    /// ticks remain queued but do not mutate blocks.
    pub fn set_scheduled_fluid_ticks_frozen(&mut self, frozen: bool) {
        self.scheduled_fluid_ticks_frozen = frozen;
    }

    pub fn set_debug_passive_showcase_enabled(&mut self, enabled: bool) {
        self.debug_passive_showcase_enabled = enabled;
    }

    pub fn set_debug_auxiliary_player_script_enabled(&mut self, enabled: bool) {
        match (enabled, self.debug_auxiliary_player_script.take()) {
            (true, Some(script)) => {
                self.debug_auxiliary_player_script = Some(script);
            }
            (true, None) => {
                let player_id = self.add_player();
                self.debug_auxiliary_player_script =
                    Some(DebugAuxiliaryPlayerScript::new(player_id));
            }
            (false, Some(script)) => {
                let _ = self.remove_player(script.player_id);
            }
            (false, None) => {}
        }
    }

    pub fn debug_auxiliary_player_id(&self) -> Option<ServerPlayerId> {
        self.debug_auxiliary_player_script
            .map(|script| script.player_id)
    }

    pub fn set_volatile_natural_spawning_enabled(&mut self, enabled: bool) {
        self.volatile_natural_spawning_enabled = enabled;
    }

    pub const fn world_behavior_profile(&self) -> WorldBehaviorProfile {
        self.world_behavior_profile
    }

    pub fn set_world_behavior_profile(&mut self, profile: WorldBehaviorProfile) {
        self.world_behavior_profile = profile;
    }

    /// Enables the deliberately non-vanilla persistence proof that awards one
    /// total experience point for an accepted upward jump transition.
    pub fn set_persistence_demo_jump_experience_enabled(&mut self, enabled: bool) {
        self.persistence_demo_jump_experience_enabled = enabled;
    }

    pub const fn persistence_demo_jump_experience_enabled(&self) -> bool {
        self.persistence_demo_jump_experience_enabled
    }

    pub fn schedule_fluid_tick(&mut self, pos: WorldBlockPos, fluid: FluidKind, delay: i32) {
        self.liquid_ticks
            .schedule_tick(pos, fluid, delay, self.simulation_tick);
    }

    pub fn scheduled_block_tick_count(&self) -> usize {
        self.block_ticks.size()
    }

    pub fn scheduled_fluid_tick_count(&self) -> usize {
        self.liquid_ticks.size()
    }

    pub fn loading_progress_stats(&self) -> Option<ChunkLoadingProgressStats> {
        self.loading_progress.stats()
    }

    pub fn loading_progress_snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        self.loading_progress.snapshot()
    }

    pub fn view_readiness_snapshot(
        &self,
        player_id: ServerPlayerId,
    ) -> Option<ChunkLoadingProgressSnapshot> {
        let view = self.chunk_tracking.accepted_view(player_id)?;
        Some(self.scheduler.view_readiness_snapshot(view))
    }

    pub fn physics_diagnostics(&self) -> ServerPhysicsTickDiagnostics {
        #[cfg(feature = "physics-engine")]
        {
            return self.physics.diagnostics();
        }
        #[cfg(not(feature = "physics-engine"))]
        {
            ServerPhysicsTickDiagnostics::default()
        }
    }

    #[cfg(feature = "physics-engine")]
    pub fn spawn_debug_physics_cube(&mut self, position: Vec3d, velocity: Vec3d) -> bool {
        self.debug_physics_player_target = None;
        self.physics
            .spawn_debug_cube(&self.scheduler, position, velocity, None)
    }

    #[cfg(feature = "physics-engine")]
    pub fn debug_physics_cube_pose(&self) -> Option<mclone_physics::PhysicsBodyPose> {
        self.physics.debug_cube_pose()
    }

    pub fn lighting_enabled(&self) -> bool {
        self.scheduler.lighting_enabled()
    }

    pub fn set_lighting_enabled(&mut self, enabled: bool) {
        self.scheduler.set_lighting_enabled(enabled);
        self.loading_progress
            .set_target_status(runtime_chunk_target_status(&self.scheduler));
    }

    pub fn light_status_batch_size(&self) -> usize {
        self.scheduler.light_status_batch_size()
    }

    pub fn set_light_status_batch_size(&mut self, batch_size: usize) {
        self.scheduler.set_light_status_batch_size(batch_size);
    }

    pub fn publication_budget_config(&self) -> ChunkPublicationBudgetConfig {
        self.scheduler.publication_budget_config()
    }

    pub fn set_publication_budget_config(&mut self, config: ChunkPublicationBudgetConfig) {
        self.scheduler.set_publication_budget_config(config);
    }

    pub fn set_publication_budget_gameplay_rate_hz(&mut self, gameplay_rate_hz: u32) {
        self.scheduler
            .set_publication_budget_gameplay_rate_hz(gameplay_rate_hz);
    }

    pub fn add_player(&mut self) -> ServerPlayerId {
        self.add_player_with_capabilities(SessionCapabilities::DEVELOPMENT_DEFAULT)
    }

    pub fn add_player_with_capabilities(
        &mut self,
        capabilities: SessionCapabilities,
    ) -> ServerPlayerId {
        let player_id = self.players.add();
        let configuration = session_configuration(self.chunk_tracking.policy(), capabilities);
        let world_info = self.world_info_update();
        let time_update = self.time_update();
        self.chunk_tracking.add_player(player_id);
        self.chunk_tracking
            .queue_update_for_player(player_id, ServerUpdate::SessionConfiguration(configuration));
        self.chunk_tracking
            .queue_update_for_player(player_id, ServerUpdate::SessionReady);
        self.chunk_tracking
            .queue_update_for_player(player_id, world_info);
        self.chunk_tracking
            .queue_update_for_player(player_id, time_update);
        self.remote_players.add_player(player_id);
        player_id
    }

    pub fn add_player_with_identity(
        &mut self,
        identity: ClientIdentity,
    ) -> ChunkStoreResult<ServerPlayerId> {
        self.add_player_with_identity_and_capabilities(
            identity,
            SessionCapabilities::DEVELOPMENT_DEFAULT,
        )
    }

    pub fn add_player_with_identity_and_capabilities(
        &mut self,
        identity: ClientIdentity,
        capabilities: SessionCapabilities,
    ) -> ChunkStoreResult<ServerPlayerId> {
        let key = player_record_key(identity.profile_id);
        let record = self.scheduler.load_player_record_blocking(key)?;
        let player_id = self.add_player_with_capabilities(capabilities);
        let player = self
            .players
            .get_mut(player_id)
            .expect("new realm player must exist");
        player.identity = Some(identity);
        if let Some(record) = record.filter(player_record_is_usable) {
            player.initial_spawn_center = Some(ChunkPos::new(
                block_to_chunk_coord(record.position.x.floor() as i32),
                block_to_chunk_coord(record.position.z.floor() as i32),
            ));
            player
                .inventory
                .restore_selected_hotbar_slot(record.selected_hotbar_slot);
            player.total_experience = record.total_experience;
            player.player_record_revision = record.revision;
            player.resume_record = Some(record);
        }
        let total_experience = player.total_experience;
        self.chunk_tracking.queue_update_for_player(
            player_id,
            ServerUpdate::PlayerExperience { total_experience },
        );
        Ok(player_id)
    }

    pub fn configure_player_identity(
        &mut self,
        player_id: ServerPlayerId,
        identity: ClientIdentity,
    ) -> ChunkStoreResult<()> {
        let Some(player) = self.players.get_mut(player_id) else {
            return Err(unknown_player_error(player_id));
        };
        let key = player_record_key(identity.profile_id);
        player.identity = Some(identity);
        self.scheduler.load_player_record(key);
        Ok(())
    }

    pub fn configure_player_identity_blocking(
        &mut self,
        player_id: ServerPlayerId,
        identity: ClientIdentity,
    ) -> ChunkStoreResult<()> {
        let key = player_record_key(identity.profile_id);
        let record = self.scheduler.load_player_record_blocking(key.clone())?;
        let Some(player) = self.players.get_mut(player_id) else {
            return Err(unknown_player_error(player_id));
        };
        player.identity = Some(identity);
        self.apply_loaded_player_record(&key, record)
    }

    pub fn save_player_record(&mut self, player_id: ServerPlayerId) -> ChunkStoreResult<bool> {
        let Some(player) = self.players.get_mut(player_id) else {
            return Ok(false);
        };
        let Some(record) = player_record_from_entry(player) else {
            return Ok(false);
        };
        self.scheduler.save_player_record(record);
        Ok(true)
    }

    pub fn save_all_player_records(&mut self) -> ChunkStoreResult<usize> {
        let records = self
            .players
            .values_mut()
            .filter_map(player_record_from_entry)
            .collect::<Vec<_>>();
        let count = records.len();
        for record in records {
            self.scheduler.save_player_record(record);
        }
        Ok(count)
    }

    pub fn remove_player(&mut self, player_id: ServerPlayerId) -> bool {
        if self.players.remove(player_id).is_none() {
            return false;
        }
        let routes = self.remote_players.remove_player(player_id);
        self.route_remote_player_updates(routes);
        self.entity_tracking.remove_observer(player_id);
        self.remove_player_chunk_tracking(player_id);
        true
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn player_position(&self, player_id: ServerPlayerId) -> Option<Vec3d> {
        self.players.position(player_id)
    }

    fn mob_player_targets(&self) -> Vec<MobPlayerTarget> {
        self.players
            .iter()
            .map(|(_, entry)| MobPlayerTarget::from_position(entry.state.position()))
            .collect()
    }

    fn natural_spawn_player_positions(&self) -> Vec<Vec3d> {
        self.players
            .iter()
            .filter(|(_, entry)| entry.state.has_accepted_position())
            .map(|(_, entry)| entry.state.position())
            .collect()
    }

    fn item_pickup_targets(&self) -> Vec<ItemPickupTarget> {
        self.players
            .iter()
            .filter(|(_, entry)| entry.state.has_accepted_position())
            .map(|(player_id, entry)| ItemPickupTarget {
                player_id,
                position: entry.state.position(),
            })
            .collect()
    }

    pub fn try_handle_command_for_player(
        &mut self,
        player_id: ServerPlayerId,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_handle_command_for_target(CommandTarget::Player(player_id), command)
    }

    /// Applies one dedicated-player command and appends all resulting updates
    /// to that player's ordered publication stream.
    ///
    /// Response-shaped host adapters keep using
    /// [`Self::try_handle_command_for_player`] until its transport cutover.
    /// Autonomous hosts use this method so command results and later tick
    /// publications share one ordered per-player queue.
    pub fn try_enqueue_command_for_player(
        &mut self,
        player_id: ServerPlayerId,
        command: ClientCommand,
    ) -> ChunkStoreResult<()> {
        let updates = self.try_handle_command_for_player(player_id, command)?;
        for update in updates {
            self.chunk_tracking
                .queue_update_for_player(player_id, update);
        }
        Ok(())
    }

    fn try_handle_command_for_target(
        &mut self,
        target: CommandTarget,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        match command {
            ClientCommand::SetChunkView(view) => self.set_chunk_view_for_target(target, view),
            ClientCommand::MovePlayer(command) => {
                self.handle_move_player_for_target(target, command)
            }
            ClientCommand::AcceptTeleport(command) => {
                self.handle_accept_teleport_for_target(target, command.id)
            }
            ClientCommand::SetCarriedItem(command) => {
                self.handle_set_carried_item_for_target(target, command)
            }
            ClientCommand::SetDebugHotbarSlot(command) => {
                self.handle_set_debug_hotbar_slot_for_target(target, command)
            }
            ClientCommand::SetPlayerAppearance(command) => {
                self.handle_set_player_appearance_for_target(target, command)
            }
            ClientCommand::PlayerAction(command) => {
                self.handle_player_action_for_target(target, command)
            }
            ClientCommand::UseItemOn(command) => {
                self.handle_use_item_on_for_target(target, command)
            }
            ClientCommand::ShootDebugPhysicsCube => {
                self.handle_shoot_debug_physics_cube_for_target(target)
            }
            ClientCommand::KeepAlive { .. } | ClientCommand::Disconnect(_) => Ok(Vec::new()),
        }
    }

    pub fn try_poll_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_poll_for_target(CommandTarget::Player(player_id))
    }

    fn try_poll_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        let events = self.scheduler.poll()?;
        self.apply_scheduler_events_for_target(target, events)
    }

    /// Drains already-routed updates for one player without polling workers or
    /// advancing global simulation.
    pub fn try_drain_updates_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.drain_chunk_updates_for_target(CommandTarget::Player(player_id))
    }

    fn try_tick_report_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<ServerTickReport> {
        self.ensure_target_exists(target)?;
        let mut report = self.try_tick_report_global()?;
        report.updates = self.drain_chunk_updates_for_target(target)?;
        Ok(report)
    }

    /// Advances scheduler-owned global work once and routes its publications
    /// to every eligible player without draining any player's stream.
    pub fn try_tick_report_global(&mut self) -> ChunkStoreResult<ServerTickReport> {
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let simulation_tick = self.simulation_tick;
        let block_ticks = &self.block_ticks;
        let liquid_ticks = &self.liquid_ticks;
        let entities = &self.entities;
        let dirty_entity_chunks = &self.dirty_entity_chunks;
        let mut chunk_record_builder = |snapshot: &ChunkSnapshot| {
            chunk_record_with_live_ticks(snapshot, block_ticks, liquid_ticks, simulation_tick)
        };
        let mut entity_record_builder = |pos: ChunkPos, revision: u64| {
            entity_chunk_record_for_persistence(entities, dirty_entity_chunks, pos, revision)
        };
        let report = self.scheduler.tick_report_with_record_builders(
            &mut chunk_record_builder,
            &mut entity_record_builder,
        )?;
        let scheduler_report_us = simulation_timing_elapsed_us(scheduler_start);
        let scheduler_event_count = report.events.len();
        let scheduler_apply_start = simulation_timing_start();
        self.route_scheduler_events(report.events)?;
        self.prepare_publications_for_all_player_observers()?;
        let scheduler_apply_events_us = simulation_timing_elapsed_us(scheduler_apply_start);
        let chunk_tracking = self.chunk_tracking_diagnostics();
        let scheduler_timing = report.timing;
        Ok(ServerTickReport {
            ticket_tick: report.ticket_tick,
            block_ticking_chunks: report.block_ticking_chunks,
            entity_ticking_chunks: report.entity_ticking_chunks,
            pending_unloads_processed: report.pending_unloads_processed,
            scheduler_event_count,
            scheduler_publication: report.publication,
            chunk_tracking,
            updates: Vec::new(),
            timing: ServerTickTiming {
                total_us: simulation_timing_elapsed_us(total_start),
                scheduler_report_us,
                scheduler_purge_stale_tickets_us: scheduler_timing.purge_stale_tickets_us,
                scheduler_reconcile_holders_us: scheduler_timing.reconcile_holders_us,
                scheduler_publish_completed_us: scheduler_timing.publish_completed_us,
                scheduler_pending_unload_us: scheduler_timing.pending_unload_us,
                scheduler_apply_events_us,
            },
        })
    }

    pub fn try_simulation_tick_report_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_for_target(CommandTarget::Player(player_id))
    }

    fn try_simulation_tick_report_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_for_target_with_physics_steps(
            target,
            DEFAULT_PHYSICS_STEPS_PER_GAMEPLAY_TICK,
            DEFAULT_PHYSICS_STEP_DT_SECONDS,
        )
    }

    fn try_simulation_tick_report_for_target_with_physics_steps(
        &mut self,
        target: CommandTarget,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.ensure_target_exists(target)?;
        let mut report = self.try_simulation_tick_report_global_with_physics_steps(
            physics_steps,
            physics_step_dt_seconds,
        )?;
        report.updates = self.drain_chunk_updates_for_target(target)?;
        Ok(report)
    }

    /// Advances global gameplay simulation once and routes publications to
    /// per-player queues without draining any player's ordered stream.
    pub fn try_simulation_tick_report_global(
        &mut self,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_global_with_physics_steps(
            DEFAULT_PHYSICS_STEPS_PER_GAMEPLAY_TICK,
            DEFAULT_PHYSICS_STEP_DT_SECONDS,
        )
    }

    pub(crate) fn try_simulation_tick_report_global_with_physics_steps(
        &mut self,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let tick_report = self.try_tick_report_global()?;
        let scheduler_tick_us = simulation_timing_elapsed_us(scheduler_start);
        let tick_timing = tick_report.timing;

        let simulation_tick = self.simulation_tick.saturating_add(1);
        self.simulation_tick = simulation_tick;
        self.mark_player_tick_boundaries();
        self.advance_debug_auxiliary_player_script()?;

        // Advance the day/night clock one tick (Java `ServerLevel.tickTime` with
        // `doDaylightCycle` on). Coupled to the simulation tick cadence, which is
        // itself frame-driven in the current runtime; revisit if/when ticks are
        // fixed-step. Skipped while frozen (debug `--freeze-time`).
        if self.daylight_cycle_running() {
            self.day_time = self.day_time.wrapping_add(1);
        }
        if self.world_metadata.is_some() {
            self.world_metadata_dirty = true;
        }

        let block_tick_start = simulation_timing_start();
        let block_tick_chunks = run_noop_simulation_phase(&tick_report.block_ticking_chunks);
        let _block_tick_report =
            self.tick_scheduled_blocks(simulation_tick, &tick_report.block_ticking_chunks);
        let block_tick_us = simulation_timing_elapsed_us(block_tick_start);

        let fluid_tick_start = simulation_timing_start();
        let (fluid_report, mut fluid_events, fluid_mutated_positions) =
            if self.scheduled_fluid_ticks_frozen {
                self.liquid_ticks.frozen_report()
            } else {
                self.liquid_ticks.tick(
                    simulation_tick,
                    &tick_report.entity_ticking_chunks,
                    &mut self.scheduler,
                )
            };
        let fluid_tick_us = simulation_timing_elapsed_us(fluid_tick_start);
        self.entities.on_blocks_changed(&fluid_mutated_positions);
        fluid_events.extend(self.scheduler.drain_pending_block_delta_events());
        let fluid_event_count = fluid_events.len();

        let entity_chunks_before_tick = self.entities.persistent_entity_chunk_positions();
        let entity_tick_start = simulation_timing_start();
        let entity_tick_chunks = run_noop_simulation_phase(&tick_report.entity_ticking_chunks);
        let natural_spawning_tick =
            self.tick_natural_spawning(simulation_tick, &tick_report.entity_ticking_chunks);
        let natural_spawning = natural_spawning_tick.diagnostics;
        let mob_player_targets = self.mob_player_targets();
        let scheduler = &self.scheduler;
        let mut entity_updates = natural_spawning_tick.spawned_entities;
        entity_updates.extend(self.entities.tick_stationary(
            &tick_report.entity_ticking_chunks,
            &mob_player_targets,
            |pos| {
                scheduler
                    .block_at_world(pos)
                    .map(|block| BlockStateId(u32::from(block)))
            },
        ));
        let item_pickup_targets = self.item_pickup_targets();
        let players = &mut self.players;
        entity_updates.extend(self.entities.collect_item_entities(
            &item_pickup_targets,
            |player_id, stack| {
                players
                    .get_mut(player_id)
                    .map(|player| player.inventory.add_item_stack(stack).remaining)
                    .unwrap_or(Some(stack))
            },
        ));
        let entity_tick_us = simulation_timing_elapsed_us(entity_tick_start);

        let physics_tick_start = simulation_timing_start();
        let physics = if physics_steps == 0 {
            self.physics_diagnostics()
        } else {
            #[cfg(feature = "physics-engine")]
            self.sync_debug_physics_player_collider();
            let physics = self.step_physics_steps(physics_steps, physics_step_dt_seconds);
            #[cfg(feature = "physics-engine")]
            if let Some(entity) = self.sync_debug_physics_cube_entity(simulation_tick, physics) {
                entity_updates.push(entity);
            }
            physics
        };
        let physics_tick_us = simulation_timing_elapsed_us(physics_tick_start);
        let entity_chunks_after_tick = self.entities.persistent_entity_chunk_positions();
        self.mark_entity_chunk_index_changes(entity_chunks_before_tick, entity_chunks_after_tick);
        self.mark_entity_updates_dirty(&entity_updates);

        if simulation_tick == 1 || simulation_tick.is_multiple_of(20) {
            self.queue_update_for_all_players(self.time_update());
        }
        let fluid_event_apply_start = simulation_timing_start();
        self.route_scheduler_events(fluid_events)?;
        self.reconcile_entity_subjects(entity_updates, true);
        let fluid_event_apply_us = simulation_timing_elapsed_us(fluid_event_apply_start);
        let chunk_tracking = self.chunk_tracking_diagnostics();

        Ok(ServerSimulationTickReport {
            simulation_tick,
            chunk_tick: tick_report.ticket_tick,
            block_tick_chunks,
            fluid_ticks_executed: fluid_report.executed_ticks,
            fluid_due_ticks: fluid_report.due_ticks,
            deferred_fluid_ticks: fluid_report.deferred_due_ticks,
            fluid_mutated_blocks: fluid_report.mutated_blocks,
            fluid_snapshot_events: fluid_report.snapshot_events,
            fluid_event_count,
            scheduled_fluid_ticks: fluid_report.scheduled_ticks,
            entity_tick_chunks,
            natural_spawning,
            physics,
            pending_unloads_processed: tick_report.pending_unloads_processed,
            scheduler_event_count: tick_report.scheduler_event_count,
            scheduler_publication: tick_report.scheduler_publication,
            chunk_tracking,
            updates: Vec::new(),
            timing: ServerSimulationTickTiming {
                total_us: simulation_timing_elapsed_us(total_start),
                scheduler_tick_us,
                scheduler_report_us: tick_timing.scheduler_report_us,
                scheduler_purge_stale_tickets_us: tick_timing.scheduler_purge_stale_tickets_us,
                scheduler_reconcile_holders_us: tick_timing.scheduler_reconcile_holders_us,
                scheduler_publish_completed_us: tick_timing.scheduler_publish_completed_us,
                scheduler_pending_unload_us: tick_timing.scheduler_pending_unload_us,
                scheduler_apply_events_us: tick_timing.scheduler_apply_events_us,
                block_tick_us,
                fluid_tick_us,
                fluid_event_apply_us,
                fluid_due_scan_us: fluid_report.due_scan_us,
                fluid_remove_due_us: fluid_report.remove_due_us,
                fluid_tick_fluid_us: fluid_report.tick_fluid_us,
                fluid_set_block_us: fluid_report.set_block_us,
                entity_tick_us,
                physics_tick_us,
            },
        })
    }

    #[allow(dead_code)]
    pub(crate) fn try_physics_step_report_for_player(
        &mut self,
        player_id: ServerPlayerId,
        physics_steps: u32,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        self.try_physics_step_report_for_player_with_step_dt(
            player_id,
            physics_steps,
            DEFAULT_PHYSICS_STEP_DT_SECONDS,
        )
    }

    pub(crate) fn try_physics_step_report_for_player_with_step_dt(
        &mut self,
        player_id: ServerPlayerId,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        let mut report = self
            .try_physics_step_report_global_with_step_dt(physics_steps, physics_step_dt_seconds)?;
        report.updates = self.drain_chunk_updates_for_target(CommandTarget::Player(player_id))?;
        Ok(report)
    }

    pub(crate) fn try_physics_step_report_global_with_step_dt(
        &mut self,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        let total_start = simulation_timing_start();

        let physics_tick_start = simulation_timing_start();
        #[cfg(feature = "physics-engine")]
        if physics_steps > 0 {
            self.sync_debug_physics_player_collider();
        }
        let physics = self.step_physics_steps(physics_steps, physics_step_dt_seconds);
        let physics_tick_us = simulation_timing_elapsed_us(physics_tick_start);

        let physics_event_apply_start = simulation_timing_start();
        #[cfg(feature = "physics-engine")]
        if physics_steps > 0 {
            if let Some(entity) = self.sync_debug_physics_cube_entity(self.simulation_tick, physics)
            {
                self.reconcile_entity_subjects(std::iter::once(entity), true);
            }
        }
        let physics_event_apply_us = simulation_timing_elapsed_us(physics_event_apply_start);
        let chunk_tracking = self.chunk_tracking_diagnostics();

        Ok(ServerPhysicsStepReport {
            simulation_tick: self.simulation_tick,
            physics_steps,
            physics,
            chunk_tracking,
            updates: Vec::new(),
            timing: ServerPhysicsStepTiming {
                total_us: simulation_timing_elapsed_us(total_start),
                physics_tick_us,
                physics_event_apply_us,
            },
        })
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.scheduler.loaded_chunk_count()
    }

    pub fn chunk_tracking_diagnostics(&self) -> PlayerChunkTrackingDiagnostics {
        self.chunk_tracking.diagnostics()
    }

    pub fn natural_spawning_diagnostics(
        &self,
        game_time: u64,
        entity_ticking_chunks: &[ChunkPos],
    ) -> NaturalSpawningDiagnostics {
        let evaluation = self.evaluate_natural_spawning(game_time, entity_ticking_chunks);
        self.natural_spawning_diagnostics_from_evaluation(
            &evaluation,
            VolatileCreatureSpawnDiagnostics::default(),
        )
    }

    fn tick_natural_spawning(
        &mut self,
        game_time: u64,
        entity_ticking_chunks: &[ChunkPos],
    ) -> NaturalSpawningTickResult {
        let evaluation = self.evaluate_natural_spawning(game_time, entity_ticking_chunks);
        let mut live = VolatileCreatureSpawnDiagnostics::default();
        let mut spawned_entities = Vec::new();

        if self.volatile_natural_spawning_enabled && !evaluation.plan.is_blocked() {
            let creature_plan = evaluation
                .plan
                .categories
                .iter()
                .copied()
                .find(|category| category.category == MobCategory::Creature);
            if let Some(creature_plan) = creature_plan.filter(|category| category.should_attempt) {
                let max_spawns = creature_plan
                    .cap
                    .saturating_sub(creature_plan.current_count)
                    .min(VOLATILE_CREATURE_SPAWN_MAX_SPAWNS_PER_TICK as u32)
                    as usize;
                let mut random =
                    SimpleRandomSource::new(natural_spawn_tick_seed(self.seed, game_time));
                let result = plan_volatile_creature_spawns(
                    &evaluation.chunk_inputs.eligible_entity_ticking_chunks,
                    &evaluation.player_positions,
                    max_spawns,
                    &mut random,
                    |pos| self.scheduler.block_at_world(pos),
                    |x, z| self.biome_source.block_position_biome_definition(x, z),
                    |pos| self.scheduler.raw_brightness_at_world(pos, 0),
                );
                live = result.diagnostics;
                spawned_entities.extend(result.requests.into_iter().map(|request| {
                    self.entities.spawn_volatile_passive_mob(
                        request.kind,
                        request.position,
                        request.y_rot_degrees,
                    )
                }));
                live.spawned = spawned_entities.len();
            }
        }

        NaturalSpawningTickResult {
            diagnostics: self.natural_spawning_diagnostics_from_evaluation(&evaluation, live),
            spawned_entities,
        }
    }

    fn evaluate_natural_spawning(
        &self,
        game_time: u64,
        entity_ticking_chunks: &[ChunkPos],
    ) -> NaturalSpawningEvaluation {
        let player_positions = self.natural_spawn_player_positions();
        let chunk_inputs = NaturalSpawnChunkInputs::from_players_and_entity_ticking_chunks(
            &player_positions,
            entity_ticking_chunks,
        );
        let category_counts = self.entities.natural_spawn_category_counts();
        let spawnable_chunk_count =
            u32::try_from(chunk_inputs.player_distance_chunk_count()).unwrap_or(u32::MAX);
        let dry_run = dry_run_creature_spawn_eligibility(
            &chunk_inputs.eligible_entity_ticking_chunks,
            |pos| self.scheduler.block_at_world(pos),
            |x, z| self.biome_source.block_position_biome_definition(x, z),
            |pos| self.scheduler.raw_brightness_at_world(pos, 0),
        );

        let mut context = NaturalSpawnContext::with_live_chunk_and_count_inputs(
            game_time,
            spawnable_chunk_count,
            category_counts,
        );
        context.biome_spawn_tables_ready = true;
        context.placement_predicates_ready = true;
        context.brightness_checks_ready = self.scheduler.lighting_enabled();
        context.collision_checks_ready = true;
        context.gamerules_ready = self.volatile_natural_spawning_enabled;
        let plan = plan_natural_spawns(self.natural_spawn_config(), context);
        let creature_state = SpawnState::new(spawnable_chunk_count, category_counts)
            .category_state(MobCategory::Creature);
        let creature_cadence_ready =
            MobCategory::Creature.should_run_natural_spawn_at_tick(game_time);
        let creature_cap_has_room = creature_state.has_room();

        NaturalSpawningEvaluation {
            player_positions,
            chunk_inputs,
            plan,
            creature_state,
            creature_cadence_ready,
            creature_cap_has_room,
            dry_run,
        }
    }

    fn natural_spawn_config(&self) -> NaturalSpawnConfig {
        if self.volatile_natural_spawning_enabled {
            NaturalSpawnConfig::enabled_volatile_passive_creatures()
        } else {
            NaturalSpawnConfig::default()
        }
    }

    fn natural_spawning_diagnostics_from_evaluation(
        &self,
        evaluation: &NaturalSpawningEvaluation,
        live: VolatileCreatureSpawnDiagnostics,
    ) -> NaturalSpawningDiagnostics {
        NaturalSpawningDiagnostics {
            live_attempts_enabled: self.volatile_natural_spawning_enabled,
            live_spawns_are_volatile: self.volatile_natural_spawning_enabled,
            ready_for_live_attempts: !evaluation.plan.is_blocked(),
            blocker_count: evaluation.plan.blocked_by.len(),
            player_distance_spawnable_chunks: evaluation.chunk_inputs.player_distance_chunk_count(),
            eligible_entity_ticking_spawn_chunks: evaluation
                .chunk_inputs
                .eligible_entity_ticking_chunk_count(),
            creature_count: evaluation.creature_state.current_count,
            creature_cap: evaluation.creature_state.cap,
            creature_cadence_ready: evaluation.creature_cadence_ready,
            creature_cap_has_room: evaluation.creature_cap_has_room,
            creature_should_attempt_if_enabled: evaluation.creature_cadence_ready
                && evaluation.creature_cap_has_room,
            live_chunks_checked: live.chunks_checked,
            live_chunk_budget_exhausted: live.chunk_budget_exhausted,
            live_attempts: live.attempts,
            live_spawned: live.spawned,
            live_spawn_budget_exhausted: live.spawn_budget_exhausted,
            live_blocked_by_biome: live.blocked_by_biome,
            live_blocked_missing_block_data: live.blocked_missing_block_data,
            live_blocked_player_distance: live.blocked_player_distance,
            live_blocked_world_predicate: live.blocked_world_predicate,
            live_blocked_unsupported: live.blocked_unsupported,
            dry_run_chunks_checked: evaluation.dry_run.chunks_checked,
            dry_run_chunk_budget_exhausted: evaluation.dry_run.chunk_budget_exhausted,
            dry_run_positions_checked: evaluation.dry_run.positions_checked,
            dry_run_biome_supported_positions: evaluation.dry_run.biome_supported_positions,
            dry_run_implemented_entries_checked: evaluation.dry_run.implemented_entries_checked,
            dry_run_valid_candidates: evaluation.dry_run.valid_candidates,
            dry_run_blocked_by_biome: evaluation.dry_run.blocked_by_biome,
            dry_run_blocked_missing_block_data: evaluation.dry_run.blocked_missing_block_data,
            dry_run_blocked_missing_brightness: evaluation.dry_run.blocked_missing_brightness,
            dry_run_blocked_invalid_floor: evaluation.dry_run.blocked_invalid_floor,
            dry_run_blocked_space: evaluation.dry_run.blocked_space,
            dry_run_blocked_collision: evaluation.dry_run.blocked_collision,
            dry_run_blocked_too_dark: evaluation.dry_run.blocked_too_dark,
            dry_run_blocked_unsupported: evaluation.dry_run.blocked_unsupported,
        }
    }

    pub fn pending_job_count(&self) -> usize {
        self.scheduler.pending_job_count()
    }

    pub fn pending_publication_count(&self) -> usize {
        self.scheduler.pending_publication_count()
    }

    pub fn wait_for_worldgen_completion(&mut self, timeout: Duration) -> bool {
        self.scheduler.wait_for_worldgen_completion(timeout)
    }

    pub fn wait_for_light_completion(&mut self, timeout: Duration) -> bool {
        self.scheduler.wait_for_light_completion(timeout)
    }

    pub fn set_world_generation_profile(
        &mut self,
        profile: WorldGenerationProfile,
    ) -> ChunkStoreResult<()> {
        self.scheduler.set_world_generation_profile(profile)?;
        self.dimensions.set_overworld_generation_profile(profile);
        Ok(())
    }

    pub fn scheduler(&self) -> &ChunkScheduler {
        &self.scheduler
    }

    pub fn scheduler_mut(&mut self) -> &mut ChunkScheduler {
        &mut self.scheduler
    }

    fn step_physics_steps(
        &mut self,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ServerPhysicsTickDiagnostics {
        #[cfg(feature = "physics-engine")]
        {
            self.physics
                .step_steps(physics_steps, physics_step_dt_seconds)
        }
        #[cfg(not(feature = "physics-engine"))]
        {
            let _ = physics_steps;
            let _ = physics_step_dt_seconds;
            ServerPhysicsTickDiagnostics::default()
        }
    }

    pub fn save_dirty_chunks(&mut self) -> ChunkStoreResult<usize> {
        let simulation_tick = self.simulation_tick;
        let block_ticks = &self.block_ticks;
        let liquid_ticks = &self.liquid_ticks;
        let entities = &self.entities;
        let dirty_entity_chunks = &self.dirty_entity_chunks;
        let mut chunk_record_builder = |snapshot: &ChunkSnapshot| {
            chunk_record_with_live_ticks(snapshot, block_ticks, liquid_ticks, simulation_tick)
        };
        let mut entity_record_builder = |pos: ChunkPos, revision: u64| {
            entity_chunk_record_for_persistence(entities, dirty_entity_chunks, pos, revision)
        };
        let queued = self.scheduler.save_dirty_chunks_with_record_builders(
            &mut chunk_record_builder,
            &mut entity_record_builder,
        )?;
        self.dirty_entity_chunks.clear();
        Ok(queued)
    }

    pub fn shutdown_persistence(&mut self) -> ChunkStoreResult<usize> {
        let queued = self
            .save_dirty_chunks()?
            .saturating_add(self.save_all_player_records()?)
            .saturating_add(self.save_world_metadata_blocking()?);
        self.scheduler.close_persistence()?;
        Ok(queued)
    }

    fn time_update(&self) -> ServerUpdate {
        ServerUpdate::TimeUpdate {
            game_time: self.game_time(),
            day_time: self.day_time,
            daylight_cycle_running: self.daylight_cycle_running(),
        }
    }

    fn set_chunk_view_for_target(
        &mut self,
        target: CommandTarget,
        mut view: ChunkView,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        if let Some(record) = self.resume_record_for_target(target)? {
            view.center = chunk_pos_for_player_position(record.position);
        }
        self.loading_progress.set_view(&view);
        self.set_initial_spawn_center_for_target(target, view.center)?;
        let player_id = target.player_id();
        let change = self.chunk_tracking.set_requested_view(player_id, view);
        for pos in &change.removed_chunks {
            self.chunk_tracking.queue_unload_for_player(player_id, *pos);
        }
        for pos in &change.added_chunks {
            if let Some(snapshot) = self.scheduler.client_visible_snapshot(*pos) {
                self.chunk_tracking
                    .queue_snapshot_for_player(player_id, snapshot);
            }
        }
        let events = if change.aggregate_changed || change.priority_centers_changed {
            self.scheduler.apply_player_ticket_positions_with_priority(
                self.chunk_tracking.aggregate_player_ticket_positions(),
                self.chunk_tracking
                    .aggregate_player_ticket_priority_centers(),
            )?
        } else {
            Vec::new()
        };
        self.apply_scheduler_events_for_target(target, events)
    }

    fn handle_move_player_for_target(
        &mut self,
        target: CommandTarget,
        command: SequencedMovePlayerCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let simulation_tick = self.simulation_tick;
        let jump_demo_enabled = self.persistence_demo_jump_experience_enabled
            && self
                .world_behavior_profile
                .allows_persistence_demo_jump_experience();
        let (result, recognized_jump, pending_correction) = {
            let player = self.player_mut_for_target(target)?;
            let before_position = player.position();
            let before_on_ground = player.on_ground();
            let before_position_accepted = player.has_accepted_position();
            let movement = command.movement;
            let result = player.apply_sequenced_move_player(command);
            let recognized_jump = jump_demo_enabled
                && result == MovePlayerApplyResult::Accepted
                && before_position_accepted
                && before_on_ground
                && movement.has_position()
                && !player.on_ground()
                && player.position().y > before_position.y;
            let pending_correction = (result == MovePlayerApplyResult::AwaitingTeleport)
                .then(|| player.resend_pending_correction_update(simulation_tick))
                .flatten();
            (result, recognized_jump, pending_correction)
        };
        let mut updates = pending_correction
            .map(ServerUpdate::PlayerPosition)
            .into_iter()
            .collect::<Vec<_>>();
        if recognized_jump {
            let total_experience = self.award_persistence_demo_jump_experience(target)?;
            updates.push(ServerUpdate::PlayerExperience { total_experience });
        }
        if result == MovePlayerApplyResult::Accepted {
            self.reconcile_remote_player_subject(target.player_id(), true);
        }
        Ok(updates)
    }

    fn award_persistence_demo_jump_experience(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<u64> {
        let player_id = target.player_id();
        let player = self
            .players
            .get_mut(player_id)
            .ok_or_else(|| unknown_player_error(player_id))?;
        player.total_experience = player.total_experience.saturating_add(1);
        Ok(player.total_experience)
    }

    fn advance_debug_auxiliary_player_script(&mut self) -> ChunkStoreResult<()> {
        let Some(mut script) = self.debug_auxiliary_player_script.take() else {
            return Ok(());
        };
        let result = (|| {
            let updates = if script.view_requested {
                self.try_drain_updates_for_player(script.player_id)?
            } else {
                script.view_requested = true;
                self.try_handle_command_for_player(
                    script.player_id,
                    ClientCommand::SetChunkView(ChunkView {
                        center: ChunkPos::new(0, 0),
                        render_distance: 0,
                        chunk_tracking_radius: 0,
                    }),
                )?
            };
            if let Some(position_update) = updates.iter().rev().find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(*update),
                _ => None,
            }) {
                script.current_position = position_update.position;
                self.try_handle_command_for_player(
                    script.player_id,
                    ClientCommand::SetPlayerAppearance(SetPlayerAppearanceCommand {
                        appearance: PlayerAppearance {
                            model: PlayerModelKind::UprightBear,
                        },
                    }),
                )?;
                self.try_handle_command_for_player(
                    script.player_id,
                    ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                        id: position_update.teleport_id,
                    }),
                )?;
                script.accepted_position = true;
            }
            if !script.accepted_position {
                return Ok(());
            }

            // Keep the shared validation actor near an admitted physical
            // player so accepted-entry-relative preview crops can exercise it
            // for arbitrary generated-world spawn coordinates. The fixed
            // anchor remains the pre-admission/test fallback.
            let anchor = self
                .players
                .iter()
                .filter(|(player_id, _)| *player_id != script.player_id)
                .find(|(_, player)| player.state.has_accepted_position())
                .map(|(_, player)| player.state.position())
                .unwrap_or(Vec3d::new(8.5, 66.0, 8.5));
            let to_anchor = anchor.subtract(script.current_position);
            let desired = if to_anchor.length_sqr() > 0.25 * 0.25 {
                let distance = to_anchor.length_sqr().sqrt();
                script
                    .current_position
                    .add(to_anchor.scale(1.0_f64.min(distance) / distance))
            } else {
                let phase = script.move_sequence % 80;
                let offset = if phase < 40 {
                    f64::from(phase as u32) * 0.025
                } else {
                    f64::from((80 - phase) as u32) * 0.025
                };
                Vec3d::new(anchor.x + offset, anchor.y, anchor.z)
            };
            let y_rot_degrees = if desired.x >= script.current_position.x {
                -90.0
            } else {
                90.0
            };
            self.try_handle_command_for_player(
                script.player_id,
                ClientCommand::move_player(MovePlayerCommand::PosRot {
                    position: desired,
                    y_rot_degrees,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                }),
            )?;
            script.current_position = self
                .player_position(script.player_id)
                .unwrap_or(script.current_position);
            script.move_sequence = script.move_sequence.saturating_add(1);
            Ok(())
        })();
        self.debug_auxiliary_player_script = Some(script);
        result
    }

    fn handle_accept_teleport_for_target(
        &mut self,
        target: CommandTarget,
        id: u32,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        if self.player_mut_for_target(target)?.accept_teleport(id) {
            self.reconcile_remote_player_subject(target.player_id(), true);
        }
        Ok(Vec::new())
    }

    fn handle_set_carried_item_for_target(
        &mut self,
        target: CommandTarget,
        command: SetCarriedItemCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.inventory_mut_for_target(target)?
            .apply_set_carried_item(command);
        Ok(Vec::new())
    }

    fn handle_set_player_appearance_for_target(
        &mut self,
        target: CommandTarget,
        command: SetPlayerAppearanceCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let player_id = target.player_id();
        let changed = {
            let Some(player) = self.players.get_mut(player_id) else {
                return Err(unknown_player_error(player_id));
            };
            let changed = player.appearance != command.appearance;
            player.appearance = command.appearance;
            changed
        };
        if changed {
            self.reconcile_remote_player_subject(player_id, true);
        }
        Ok(Vec::new())
    }

    fn handle_set_debug_hotbar_slot_for_target(
        &mut self,
        target: CommandTarget,
        command: SetDebugHotbarSlotCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.inventory_mut_for_target(target)?
            .apply_set_debug_hotbar_slot(command);
        Ok(Vec::new())
    }

    fn handle_player_action_for_target(
        &mut self,
        target: CommandTarget,
        command: PlayerActionCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        if command.kind == PlayerActionKind::DebugInstantBreak
            && self.world_behavior_profile.allows_player_break()
        {
            let player_position = self.player_for_target(target)?.position();
            let context = ServerInteractionContext::debug_creative(player_position);
            if context.may_break_block(command.pos) {
                self.set_block_debug(command.pos, AIR_BLOCK_STATE_ID);
            }
        }
        self.drain_pending_block_delta_updates_for_target(target)
    }

    fn handle_use_item_on_for_target(
        &mut self,
        target: CommandTarget,
        command: UseItemOnCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        if self.world_behavior_profile.allows_player_place() {
            if let Some((target, block_state)) =
                self.held_item_place_target_for_target(target, command)?
            {
                self.set_block_debug(target, block_state);
            }
        } else {
            self.ensure_target_exists(target)?;
        }
        self.drain_pending_block_delta_updates_for_target(target)
    }

    fn handle_shoot_debug_physics_cube_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        #[cfg(not(feature = "physics-engine"))]
        {
            self.ensure_target_exists(target)?;
            Ok(Vec::new())
        }
        #[cfg(feature = "physics-engine")]
        {
            let (position, y_rot_degrees, x_rot_degrees) = {
                let player = self.player_for_target(target)?;
                (
                    player.position(),
                    player.y_rot_degrees(),
                    player.x_rot_degrees(),
                )
            };
            let launch = debug_physics_cube_launch(position, y_rot_degrees, x_rot_degrees);
            if self.physics.spawn_debug_cube(
                &self.scheduler,
                launch.position,
                launch.velocity,
                Some(position),
            ) {
                self.debug_physics_player_target = Some(target);
                let physics = self.physics.diagnostics();
                if let Some(entity_spawn) =
                    self.spawn_debug_physics_cube_entity(self.simulation_tick, physics)
                {
                    let subjects = entity_spawn
                        .removed
                        .into_iter()
                        .chain(std::iter::once(entity_spawn.current));
                    self.reconcile_entity_subjects(subjects, true);
                    return self.drain_chunk_updates_for_target(target);
                }
            }
            Ok(Vec::new())
        }
    }

    fn held_item_place_target_for_target(
        &self,
        target: CommandTarget,
        command: UseItemOnCommand,
    ) -> ChunkStoreResult<Option<(BlockPos, BlockStateId)>> {
        if command.hand != InteractionHand::MainHand {
            return Ok(None);
        }
        let block_state = match self.inventory_for_target(target)?.selected_block_state() {
            Some(block_state) => block_state,
            None => return Ok(None),
        };
        let Some(block_id) = raw_block_id_from_block_state(block_state) else {
            return Ok(None);
        };
        let player_position = self.player_for_target(target)?.position();
        let context = ServerInteractionContext::debug_creative(player_position);
        if !context.may_use_item_on(command.hit) {
            return Ok(None);
        }
        let Some(block_item) = DebugBlockItem::new(block_id) else {
            return Ok(None);
        };
        let Some(clicked_block) = self.scheduler.block_at_world(command.hit.block_pos) else {
            return Ok(None);
        };
        let Some(placement) = block_item.use_on(command.hit, clicked_block, |pos| {
            self.scheduler.block_at_world(pos)
        }) else {
            return Ok(None);
        };
        Ok(context
            .may_place_at(placement.pos)
            .then_some((placement.pos, generated_block_state_id(placement.block))))
    }

    fn set_block_debug(&mut self, pos: BlockPos, block_state: BlockStateId) -> bool {
        let Some(block_id) = raw_block_id_from_block_state(block_state) else {
            return false;
        };
        self.set_block_from_simulation(pos, block_id)
    }

    fn set_block_from_simulation(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool {
        let before = self.scheduler.block_at_world(pos);
        let changed = self.scheduler.set_block_at_world(pos, block_id);
        if changed {
            self.entities.on_block_changed(pos);
            if let Some(before) = before {
                self.scheduler
                    .refresh_runtime_lighting_after_block_change(pos, before, block_id);
            }
            for request in self
                .scheduler
                .fluid_tick_requests_after_block_change(pos, block_id)
            {
                self.schedule_fluid_tick(request.pos, request.fluid, request.delay);
            }
            for request in block_tick_requests_after_block_change(pos, block_id) {
                let target = self
                    .scheduler
                    .block_at_world(request.pos)
                    .map(block_name)
                    .unwrap_or("minecraft:air");
                self.block_ticks.schedule_tick(
                    request.pos,
                    target,
                    request.delay,
                    self.simulation_tick,
                );
            }
        }
        changed
    }

    fn tick_scheduled_blocks(
        &mut self,
        game_time: u64,
        block_ticking_chunks: &[ChunkPos],
    ) -> BlockTickPhaseReport {
        let due = self.block_ticks.drain_due(game_time, block_ticking_chunks);
        let mut executed_ticks = 0;
        let mut mutated_blocks = 0;
        for pos in due.positions {
            executed_ticks += 1;
            let Some(plan) = basic_falling_block_move(&self.scheduler, pos) else {
                continue;
            };
            if !self.set_block_from_simulation(plan.from, AIR) {
                continue;
            }
            mutated_blocks += 1;
            if self.set_block_from_simulation(plan.to, plan.block) {
                mutated_blocks += 1;
            }
        }

        BlockTickPhaseReport {
            executed_ticks,
            due_ticks: executed_ticks.saturating_add(due.deferred_due_ticks),
            deferred_due_ticks: due.deferred_due_ticks,
            mutated_blocks,
            scheduled_ticks: self.block_ticks.size(),
        }
    }

    fn drain_pending_block_delta_updates_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let events = self.scheduler.drain_pending_block_delta_events();
        self.apply_scheduler_events_for_target(target, events)
    }

    fn apply_scheduler_events_for_target(
        &mut self,
        target: CommandTarget,
        events: Vec<ChunkSchedulerEvent>,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        self.route_scheduler_events(events)?;
        self.reconcile_remote_players_for_target_observer(target);
        let initial_spawn_update = self.initial_spawn_update_for_target(target)?;
        self.reconcile_entities_for_target_observer(target);
        let mut updates = self.drain_chunk_updates_for_target(target)?;
        if let Some(update) = initial_spawn_update {
            updates.push(ServerUpdate::PlayerPosition(update));
        }
        Ok(updates)
    }

    fn route_scheduler_events(&mut self, events: Vec<ChunkSchedulerEvent>) -> ChunkStoreResult<()> {
        for event in events {
            match event {
                ChunkSchedulerEvent::SnapshotReady(snapshot) => {
                    self.chunk_tracking
                        .queue_snapshot_for_tracking_players(snapshot);
                }
                ChunkSchedulerEvent::Unloaded { pos } => {
                    self.loading_progress.clear_chunk(pos);
                    self.chunk_tracking.queue_unload_for_tracking_players(pos);
                }
                ChunkSchedulerEvent::HolderUnloaded { pos } => {
                    self.block_ticks.remove_chunk_ticks(pos);
                    self.liquid_ticks.remove_chunk_ticks(pos);
                    let removed = if self.scheduler.entity_chunks_supported() {
                        self.entities.remove_entities_in_chunk(pos)
                    } else {
                        self.entities.discard_volatile_entities_in_chunk(pos)
                    };
                    self.dirty_entity_chunks.remove(&pos);
                    self.reconcile_entity_subjects(removed, true);
                }
                ChunkSchedulerEvent::EntityChunkLoaded { pos, record } => {
                    self.dirty_entity_chunks.remove(&pos);
                    if let Some(record) = record {
                        let loaded = self.entities.hydrate_entity_chunk_record(&record)?;
                        self.reconcile_entity_subjects(loaded, true);
                    }
                }
                ChunkSchedulerEvent::PlayerLoaded { player, record } => {
                    self.apply_loaded_player_record(&player, record)?;
                }
                ChunkSchedulerEvent::SectionBlockUpdates {
                    pos,
                    section_y,
                    updates,
                } => {
                    self.chunk_tracking
                        .queue_section_updates_for_tracking_players(pos, section_y, updates);
                }
                ChunkSchedulerEvent::FluidTickScheduled { pos, fluid, delay } => {
                    self.liquid_ticks
                        .schedule_tick(pos, fluid, delay, self.simulation_tick);
                }
                ChunkSchedulerEvent::BlockTickScheduled { pos, target, delay } => {
                    self.block_ticks
                        .schedule_tick(pos, target, delay, self.simulation_tick);
                }
                ChunkSchedulerEvent::StatusChanged { pos, status, step } => {
                    self.loading_progress
                        .record_status_change(pos, status, step);
                }
            }
        }
        Ok(())
    }

    fn route_remote_player_updates(&mut self, routes: Vec<RoutedRemotePlayerUpdate>) {
        for route in routes {
            self.chunk_tracking
                .queue_update_for_player(route.recipient, route.update);
        }
    }

    fn route_entity_updates(&mut self, routes: Vec<RoutedEntityUpdate>) {
        for route in routes {
            self.chunk_tracking
                .queue_update_for_player(route.recipient, route.update);
        }
    }

    fn reconcile_remote_players_for_target_observer(&mut self, target: CommandTarget) {
        let observer = target.player_id();
        if !self.players.contains(observer) {
            return;
        }
        let states = self.remote_player_states();
        let chunk_tracking = &self.chunk_tracking;
        let routes = self
            .remote_players
            .reconcile_observer(observer, &states, |player_id, pos| {
                chunk_tracking.player_tracks_chunk(player_id, pos)
            });
        self.route_remote_player_updates(routes);
    }

    fn reconcile_remote_player_subject(
        &mut self,
        subject: ServerPlayerId,
        emit_existing_updates: bool,
    ) {
        if !self.players.contains(subject) {
            return;
        }
        let Some(state) = self.remote_player_state(subject) else {
            return;
        };
        let observers = self.player_observers();
        let chunk_tracking = &self.chunk_tracking;
        let routes = self.remote_players.reconcile_subject(
            state,
            observers,
            |player_id, pos| chunk_tracking.player_tracks_chunk(player_id, pos),
            emit_existing_updates,
        );
        self.route_remote_player_updates(routes);
    }

    fn remote_player_states(&self) -> Vec<RemotePlayerState> {
        self.player_observers()
            .into_iter()
            .map(|player_id| {
                self.remote_player_state(player_id)
                    .expect("iterated player observer must have state")
            })
            .collect()
    }

    fn remote_player_state(&self, player_id: ServerPlayerId) -> Option<RemotePlayerState> {
        let player = self.players.get(player_id)?;
        Some(RemotePlayerState {
            player_id,
            appearance: player.appearance,
            position: player.state.position(),
            y_rot_degrees: player.state.y_rot_degrees(),
            x_rot_degrees: player.state.x_rot_degrees(),
            on_ground: player.state.on_ground(),
            publishable: player.state.has_accepted_position(),
        })
    }

    fn reconcile_entities_for_target_observer(&mut self, target: CommandTarget) {
        let observer = target.player_id();
        let states = self.entities.states();
        let chunk_tracking = &self.chunk_tracking;
        let routes =
            self.entity_tracking
                .reconcile_observer(observer, &states, |player_id, pos| {
                    chunk_tracking.player_tracks_chunk(player_id, pos)
                });
        self.route_entity_updates(routes);
    }

    fn reconcile_entity_subjects(
        &mut self,
        subjects: impl IntoIterator<Item = ServerEntityState>,
        emit_existing_updates: bool,
    ) {
        let observers = self.player_observers();
        let chunk_tracking = &self.chunk_tracking;
        let mut routes = Vec::new();
        for subject in subjects {
            routes.extend(self.entity_tracking.reconcile_subject(
                subject,
                observers.iter().copied(),
                |player_id, pos| chunk_tracking.player_tracks_chunk(player_id, pos),
                emit_existing_updates,
            ));
        }
        self.route_entity_updates(routes);
    }

    fn mark_entity_updates_dirty(&mut self, subjects: &[ServerEntityState]) {
        for subject in subjects {
            if self.entities.is_persistent_entity(subject.id) {
                self.dirty_entity_chunks.insert(subject.chunk_pos());
            }
        }
    }

    fn mark_entity_chunk_index_changes(
        &mut self,
        before: BTreeMap<EntityPersistentId, ChunkPos>,
        after: BTreeMap<EntityPersistentId, ChunkPos>,
    ) {
        for (persistent_id, old_pos) in &before {
            match after.get(persistent_id) {
                Some(new_pos) if new_pos == old_pos => {}
                Some(new_pos) => {
                    self.dirty_entity_chunks.insert(*old_pos);
                    self.dirty_entity_chunks.insert(*new_pos);
                }
                None => {
                    self.dirty_entity_chunks.insert(*old_pos);
                }
            }
        }
        for (persistent_id, new_pos) in after {
            if !before.contains_key(&persistent_id) {
                self.dirty_entity_chunks.insert(new_pos);
            }
        }
    }

    #[cfg(feature = "physics-engine")]
    fn spawn_debug_physics_cube_entity(
        &mut self,
        age_ticks: u64,
        physics: ServerPhysicsTickDiagnostics,
    ) -> Option<crate::entity::DebugPhysicsCubeEntitySpawn> {
        let pose = self.physics.debug_cube_pose()?;
        let (position, y_rot_degrees, x_rot_degrees, rotation) =
            debug_physics_cube_entity_pose(pose);
        debug_assert_eq!(physics.test_cube_position, Some(pose.position));
        Some(self.entities.spawn_debug_physics_cube(
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            age_ticks,
        ))
    }

    #[cfg(feature = "physics-engine")]
    fn sync_debug_physics_cube_entity(
        &mut self,
        age_ticks: u64,
        physics: ServerPhysicsTickDiagnostics,
    ) -> Option<ServerEntityState> {
        let pose = self.physics.debug_cube_pose()?;
        let (position, y_rot_degrees, x_rot_degrees, rotation) =
            debug_physics_cube_entity_pose(pose);
        debug_assert_eq!(physics.test_cube_position, Some(pose.position));
        Some(self.entities.upsert_debug_physics_cube(
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            age_ticks,
        ))
    }

    #[cfg(feature = "physics-engine")]
    fn sync_debug_physics_player_collider(&mut self) {
        let Some(target) = self.debug_physics_player_target else {
            return;
        };
        let Ok(player) = self.player_for_target(target) else {
            self.debug_physics_player_target = None;
            return;
        };
        self.physics.sync_player_collider(player.position());
    }

    fn player_observers(&self) -> Vec<ServerPlayerId> {
        self.players
            .iter()
            .map(|(player_id, _)| player_id)
            .collect()
    }

    fn player_targets(&self) -> Vec<CommandTarget> {
        self.players
            .iter()
            .map(|(player_id, _)| CommandTarget::Player(player_id))
            .collect()
    }

    fn prepare_publications_for_all_player_observers(&mut self) -> ChunkStoreResult<()> {
        for target in self.player_targets() {
            self.reconcile_remote_players_for_target_observer(target);
            let initial_spawn_update = self.initial_spawn_update_for_target(target)?;
            self.reconcile_entities_for_target_observer(target);
            if let Some(update) = initial_spawn_update {
                self.chunk_tracking.queue_update_for_player(
                    target.player_id(),
                    ServerUpdate::PlayerPosition(update),
                );
            }
        }
        Ok(())
    }

    fn queue_update_for_all_players(&mut self, update: ServerUpdate) {
        for player_id in self.player_observers() {
            self.chunk_tracking
                .queue_update_for_player(player_id, update.clone());
        }
    }

    fn drain_chunk_updates_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        Ok(self.chunk_tracking.drain_updates(target.player_id()))
    }

    fn world_info_update(&self) -> ServerUpdate {
        ServerUpdate::WorldInfo {
            biome_zoom_seed: obfuscate_biome_zoom_seed(self.seed),
        }
    }

    fn initial_spawn_update_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<mclone_protocol::PlayerPositionUpdate>> {
        let player = self.player_for_target(target)?;
        if !player.needs_initial_position_sync() {
            return Ok(None);
        }
        let Some(center) = self.initial_spawn_center_for_target(target)? else {
            return Ok(None);
        };
        let resume = self.resume_record_for_target(target)?.cloned();
        if let Some(record) = resume.as_ref()
            && !self
                .scheduler
                .client_visible_snapshot(chunk_pos_for_player_position(record.position))
                .is_some()
        {
            return Ok(None);
        }
        let column_order = if matches!(
            self.scheduler.world_generation_profile(),
            WorldGenerationProfile::AuthoredOnly { .. }
        ) {
            SpawnColumnOrder::CenterFirst
        } else {
            SpawnColumnOrder::Scan
        };
        let exact_resume = resume
            .as_ref()
            .filter(|record| self.player_pose_has_clearance(record.position));
        let position = if let Some(record) = exact_resume {
            record.position
        } else {
            let Some(position) = find_safe_surface_spawn_with_column_order(
                center,
                |pos| self.scheduler.block_at_world(pos),
                |x, z| self.biome_source.block_position_biome_definition(x, z),
                |chunk| self.scheduler.client_visible_snapshot(chunk).is_some(),
                column_order,
            ) else {
                return Ok(None);
            };
            position
        };
        let showcase_ids = self.entities.ensure_debug_passive_showcase_near_spawn(
            position,
            self.debug_passive_showcase_enabled,
        );
        let showcase_states = showcase_ids
            .into_iter()
            .filter_map(|id| self.entities.state(id))
            .collect::<Vec<_>>();
        self.mark_entity_updates_dirty(&showcase_states);
        let simulation_tick = self.simulation_tick;
        let update = if let Some(record) = exact_resume {
            self.player_mut_for_target(target)?
                .restored_position_update(
                    position,
                    record.y_rot_degrees,
                    record.x_rot_degrees,
                    record.on_ground,
                    simulation_tick,
                )
        } else {
            self.player_mut_for_target(target)?.initial_position_update(
                position,
                0.0,
                0.0,
                simulation_tick,
            )
        };
        self.take_resume_record_for_target(target)?;
        Ok(Some(update))
    }

    fn resume_record_for_target(
        &self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<&PlayerRecord>> {
        let player_id = target.player_id();
        self.players
            .get(player_id)
            .map(|player| player.resume_record.as_ref())
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn take_resume_record_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<PlayerRecord>> {
        let player_id = target.player_id();
        self.players
            .get_mut(player_id)
            .map(|player| player.resume_record.take())
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn player_pose_has_clearance(&self, position: Vec3d) -> bool {
        const PLAYER_RADIUS: f64 = 0.299;
        const PLAYER_HEIGHT: f64 = 1.799;
        let xs = [position.x - PLAYER_RADIUS, position.x + PLAYER_RADIUS];
        let ys = [position.y, position.y + PLAYER_HEIGHT];
        let zs = [position.z - PLAYER_RADIUS, position.z + PLAYER_RADIUS];
        xs.into_iter().all(|x| {
            ys.into_iter().all(|y| {
                zs.into_iter().all(|z| {
                    self.scheduler
                        .block_at_world(BlockPos::new(
                            x.floor() as i32,
                            y.floor() as i32,
                            z.floor() as i32,
                        ))
                        .is_some_and(|block| !mclone_worldgen::block::material_blocks_motion(block))
                })
            })
        })
    }

    fn apply_loaded_player_record(
        &mut self,
        key: &PlayerRecordKey,
        record: Option<PlayerRecord>,
    ) -> ChunkStoreResult<()> {
        let Some(record) = record.filter(player_record_is_usable) else {
            return Ok(());
        };
        let matched = self.players.iter().find_map(|(player_id, candidate)| {
            candidate
                .identity
                .as_ref()
                .is_some_and(|identity| player_record_key(identity.profile_id) == *key)
                .then_some(player_id)
        });
        if let Some(player_id) = matched {
            let player = self
                .players
                .get_mut(player_id)
                .expect("matched realm player must exist");
            player.initial_spawn_center = Some(chunk_pos_for_player_position(record.position));
            player
                .inventory
                .restore_selected_hotbar_slot(record.selected_hotbar_slot);
            player.total_experience = record.total_experience;
            player.player_record_revision = record.revision;
            player.resume_record = Some(record);
            let total_experience = player.total_experience;
            self.chunk_tracking.queue_update_for_player(
                player_id,
                ServerUpdate::PlayerExperience { total_experience },
            );
            let center = self
                .players
                .get(player_id)
                .and_then(|player| player.initial_spawn_center)
                .expect("loaded realm resume has a spawn center");
            self.retarget_player_view_for_resume(CommandTarget::Player(player_id), center)?;
        }
        Ok(())
    }

    fn retarget_player_view_for_resume(
        &mut self,
        target: CommandTarget,
        center: ChunkPos,
    ) -> ChunkStoreResult<()> {
        let Some(mut view) = self
            .chunk_tracking
            .accepted_view(target.player_id())
            .cloned()
        else {
            return Ok(());
        };
        view.center = center;
        let updates = self.set_chunk_view_for_target(target, view)?;
        for update in updates {
            self.chunk_tracking
                .queue_update_for_player(target.player_id(), update);
        }
        Ok(())
    }

    fn set_initial_spawn_center_for_target(
        &mut self,
        target: CommandTarget,
        center: ChunkPos,
    ) -> ChunkStoreResult<()> {
        let player_id = target.player_id();
        let player = self
            .players
            .get_mut(player_id)
            .ok_or_else(|| unknown_player_error(player_id))?;
        if player.initial_spawn_center.is_none() && player.state.needs_initial_position_sync() {
            player.initial_spawn_center = Some(center);
        }
        Ok(())
    }

    fn initial_spawn_center_for_target(
        &self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<ChunkPos>> {
        let player_id = target.player_id();
        self.players
            .get(player_id)
            .map(|player| player.initial_spawn_center)
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn player_for_target(&self, target: CommandTarget) -> ChunkStoreResult<&ServerPlayerState> {
        let player_id = target.player_id();
        self.players
            .get(player_id)
            .map(|player| &player.state)
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn player_mut_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<&mut ServerPlayerState> {
        let player_id = target.player_id();
        self.players
            .get_mut(player_id)
            .map(|player| &mut player.state)
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn inventory_for_target(&self, target: CommandTarget) -> ChunkStoreResult<&ServerInventory> {
        let player_id = target.player_id();
        self.players
            .get(player_id)
            .map(|player| &player.inventory)
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn inventory_mut_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<&mut ServerInventory> {
        let player_id = target.player_id();
        self.players
            .get_mut(player_id)
            .map(|player| &mut player.inventory)
            .ok_or_else(|| unknown_player_error(player_id))
    }

    fn ensure_target_exists(&self, target: CommandTarget) -> ChunkStoreResult<()> {
        let player_id = target.player_id();
        if self.players.contains(player_id) {
            Ok(())
        } else {
            Err(unknown_player_error(player_id))
        }
    }

    fn mark_player_tick_boundaries(&mut self) {
        for player in self.players.values_mut() {
            player.state.mark_tick_boundary();
        }
    }

    fn remove_player_chunk_tracking(&mut self, player_id: ServerPlayerId) {
        let change = self.chunk_tracking.remove_player(player_id);
        if !change.aggregate_changed {
            return;
        }
        let events = self
            .scheduler
            .apply_player_ticket_positions_with_priority(
                self.chunk_tracking.aggregate_player_ticket_positions(),
                self.chunk_tracking
                    .aggregate_player_ticket_priority_centers(),
            )
            .expect("failed to reconcile chunk tracking after player disconnect");
        self.route_scheduler_events(events)
            .expect("failed to route scheduler events after player disconnect");
    }
}

/// In-memory integrated-host adapter for one ordinary realm player.
///
/// The authoritative state remains entirely in [`RealmServer`]. This adapter
/// only remembers which ordinary `ServerPlayerId` belongs to the owning local
/// connection and supplies that id to the same command, publication, save, and
/// disconnect paths used by remote transports.
#[derive(Debug)]
pub struct LocalRealmSession {
    server: RealmServer,
    player_id: ServerPlayerId,
}

impl LocalRealmSession {
    pub fn from_server(mut server: RealmServer) -> Self {
        let player_id = server.add_player();
        Self { server, player_id }
    }

    pub fn new(seed: i64) -> Self {
        Self::from_server(RealmServer::new(seed))
    }

    pub fn new_in_realm(realm_id: RealmId, seed: i64) -> Self {
        Self::from_server(RealmServer::new_in_realm(realm_id, seed))
    }

    pub fn local_integrated(seed: i64) -> Self {
        Self::from_server(RealmServer::local_integrated(seed))
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self::from_server(RealmServer::with_chunk_store(seed, store))
    }

    pub fn with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::from_server(RealmServer::with_world_store(seed, store))
    }

    pub fn local_integrated_with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::from_server(RealmServer::local_integrated_with_world_store(seed, store))
    }

    pub fn local_integrated_with_external_load_world_store(
        seed: i64,
        store: Box<dyn WorldStore>,
    ) -> Self {
        Self::from_server(RealmServer::local_integrated_with_external_load_world_store(seed, store))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_with_threaded_world_store(
        seed: i64,
        store: Box<dyn WorldStore + Send>,
    ) -> ChunkStoreResult<Self> {
        Ok(Self::from_server(
            RealmServer::try_with_threaded_world_store(seed, store)?,
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_with_threaded_sqlite_world_dir(
        seed: i64,
        world_dir: impl AsRef<Path>,
    ) -> ChunkStoreResult<Self> {
        Ok(Self::from_server(
            RealmServer::try_with_threaded_sqlite_world_dir(seed, world_dir)?,
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn try_with_threaded_sqlite_world_dir_and_player_chunk_tracking_policy(
        seed: i64,
        world_dir: impl AsRef<Path>,
        policy: PlayerChunkTrackingPolicy,
    ) -> ChunkStoreResult<Self> {
        Ok(Self::from_server(
            RealmServer::try_with_threaded_sqlite_world_dir_and_player_chunk_tracking_policy(
                seed, world_dir, policy,
            )?,
        ))
    }

    pub(crate) fn with_player_chunk_tracking_policy(
        seed: i64,
        policy: PlayerChunkTrackingPolicy,
    ) -> Self {
        Self::from_server(RealmServer::with_player_chunk_tracking_policy(seed, policy))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn with_wasm_job_workers(seed: i64, config: WasmServerJobWorkerConfig) -> Self {
        Self::from_server(RealmServer::with_wasm_job_workers(seed, config))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn local_integrated_with_wasm_job_workers(
        seed: i64,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self::from_server(RealmServer::local_integrated_with_wasm_job_workers(
            seed, config,
        ))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn local_integrated_with_world_store_and_wasm_job_workers(
        seed: i64,
        store: Box<dyn WorldStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self::from_server(
            RealmServer::local_integrated_with_world_store_and_wasm_job_workers(
                seed, store, config,
            ),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub fn local_integrated_with_external_load_world_store_and_wasm_job_workers(
        seed: i64,
        store: Box<dyn WorldStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self::from_server(
            RealmServer::local_integrated_with_external_load_world_store_and_wasm_job_workers(
                seed, store, config,
            ),
        )
    }

    pub const fn player_id(&self) -> ServerPlayerId {
        self.player_id
    }

    pub fn server(&self) -> &RealmServer {
        &self.server
    }

    pub fn server_mut(&mut self) -> &mut RealmServer {
        &mut self.server
    }

    pub fn configure_local_player_identity(
        &mut self,
        identity: ClientIdentity,
    ) -> ChunkStoreResult<()> {
        self.server
            .configure_player_identity(self.player_id, identity)
    }

    pub fn configure_local_player_identity_blocking(
        &mut self,
        identity: ClientIdentity,
    ) -> ChunkStoreResult<()> {
        self.server
            .configure_player_identity_blocking(self.player_id, identity)
    }

    pub fn handle_command(&mut self, command: ClientCommand) -> Vec<ServerUpdate> {
        self.try_handle_command(command)
            .expect("local realm session command failed")
    }

    pub fn try_handle_command(
        &mut self,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.server
            .try_handle_command_for_player(self.player_id, command)
    }

    pub fn poll(&mut self) -> Vec<ServerUpdate> {
        self.try_poll().expect("local realm session poll failed")
    }

    pub fn try_poll(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.server.try_poll_for_player(self.player_id)
    }

    pub fn try_drain_updates(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.server.try_drain_updates_for_player(self.player_id)
    }

    pub fn tick(&mut self) -> Vec<ServerUpdate> {
        self.try_tick().expect("local realm session tick failed")
    }

    pub fn try_tick(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        Ok(self.try_simulation_tick_report()?.updates)
    }

    pub fn tick_report(&mut self) -> ServerTickReport {
        self.try_tick_report()
            .expect("local realm session tick report failed")
    }

    pub fn try_tick_report(&mut self) -> ChunkStoreResult<ServerTickReport> {
        self.server
            .try_tick_report_for_target(CommandTarget::Player(self.player_id))
    }

    pub fn simulation_tick_report(&mut self) -> ServerSimulationTickReport {
        self.try_simulation_tick_report()
            .expect("local realm session simulation tick failed")
    }

    pub fn try_simulation_tick_report(&mut self) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.server
            .try_simulation_tick_report_for_player(self.player_id)
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn try_simulation_tick_report_with_physics_steps(
        &mut self,
        physics_steps: u32,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_with_physics_steps_and_step_dt(
            physics_steps,
            DEFAULT_PHYSICS_STEP_DT_SECONDS,
        )
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn try_simulation_tick_report_with_physics_steps_and_step_dt(
        &mut self,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.server
            .try_simulation_tick_report_for_target_with_physics_steps(
                CommandTarget::Player(self.player_id),
                physics_steps,
                physics_step_dt_seconds,
            )
    }

    #[allow(dead_code)]
    pub(crate) fn try_physics_step_report(
        &mut self,
        physics_steps: u32,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        self.server
            .try_physics_step_report_for_player(self.player_id, physics_steps)
    }

    pub(crate) fn try_physics_step_report_with_step_dt(
        &mut self,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        self.server.try_physics_step_report_for_player_with_step_dt(
            self.player_id,
            physics_steps,
            physics_step_dt_seconds,
        )
    }

    pub fn view_readiness_snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        self.server.view_readiness_snapshot(self.player_id)
    }

    #[cfg(test)]
    pub(crate) fn player(&self) -> &ServerPlayerState {
        self.server
            .players
            .get(self.player_id)
            .map(|player| &player.state)
            .expect("local realm session player must exist")
    }

    #[cfg(test)]
    pub(crate) fn player_mut(&mut self) -> &mut ServerPlayerState {
        self.server
            .players
            .get_mut(self.player_id)
            .map(|player| &mut player.state)
            .expect("local realm session player must exist")
    }

    #[cfg(test)]
    pub(crate) fn inventory(&self) -> &ServerInventory {
        self.server
            .players
            .get(self.player_id)
            .map(|player| &player.inventory)
            .expect("local realm session player must exist")
    }

    #[cfg(test)]
    pub(crate) fn total_experience(&self) -> u64 {
        self.server
            .players
            .get(self.player_id)
            .map(|player| player.total_experience)
            .expect("local realm session player must exist")
    }

    #[cfg(test)]
    pub(crate) fn resume_record(&self) -> Option<&PlayerRecord> {
        self.server
            .players
            .get(self.player_id)
            .and_then(|player| player.resume_record.as_ref())
    }
}

impl std::ops::Deref for LocalRealmSession {
    type Target = RealmServer;

    fn deref(&self) -> &Self::Target {
        &self.server
    }
}

impl std::ops::DerefMut for LocalRealmSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.server
    }
}

impl CommandTarget {
    const fn player_id(self) -> ServerPlayerId {
        match self {
            Self::Player(player_id) => player_id,
        }
    }
}

fn unknown_player_error(player_id: ServerPlayerId) -> ChunkStoreError {
    ChunkStoreError::InvalidData(format!("unknown server player {player_id}"))
}

fn raw_block_id_from_block_state(block_state: BlockStateId) -> Option<RawBlockId> {
    RawBlockId::try_from(block_state.0).ok()
}

fn player_record_key(profile_id: PlayerProfileId) -> PlayerRecordKey {
    let bytes = profile_id.bytes();
    PlayerRecordKey::Uuid(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    ))
}

fn chunk_pos_for_player_position(position: Vec3d) -> ChunkPos {
    ChunkPos::new(
        block_to_chunk_coord(position.x.floor() as i32),
        block_to_chunk_coord(position.z.floor() as i32),
    )
}

fn player_record_is_usable(record: &PlayerRecord) -> bool {
    record.codec_version == crate::persistence::PLAYER_RECORD_VERSION
        && record.dimension == "minecraft:overworld"
        && record.position.is_finite()
        && record.y_rot_degrees.is_finite()
        && record.x_rot_degrees.is_finite()
}

fn player_record_from_entry(
    player: &mut crate::players::ServerPlayerEntry,
) -> Option<PlayerRecord> {
    let identity = player.identity.as_ref()?;
    let revision = player.player_record_revision.saturating_add(1);
    player.player_record_revision = revision;
    if !player.state.has_accepted_position() {
        let mut record = player.resume_record.clone()?;
        record.revision = revision;
        record.last_known_name = identity.display_name.clone();
        record.selected_hotbar_slot = player.inventory.selected_hotbar_slot();
        record.total_experience = player.total_experience;
        return Some(record);
    }
    Some(PlayerRecord {
        player: player_record_key(identity.profile_id),
        codec_version: crate::persistence::PLAYER_RECORD_VERSION,
        revision,
        last_known_name: identity.display_name.clone(),
        dimension: "minecraft:overworld".to_owned(),
        position: player.state.position(),
        y_rot_degrees: player.state.y_rot_degrees(),
        x_rot_degrees: player.state.x_rot_degrees(),
        on_ground: player.state.on_ground(),
        selected_hotbar_slot: player.inventory.selected_hotbar_slot(),
        total_experience: player.total_experience,
    })
}

#[cfg(feature = "physics-engine")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct DebugPhysicsCubeLaunch {
    position: Vec3d,
    velocity: Vec3d,
}

#[cfg(feature = "physics-engine")]
fn debug_physics_cube_launch(
    player_position: Vec3d,
    y_rot_degrees: f32,
    x_rot_degrees: f32,
) -> DebugPhysicsCubeLaunch {
    let direction = look_direction_from_rot(y_rot_degrees, x_rot_degrees);
    DebugPhysicsCubeLaunch {
        position: player_position
            .add(Vec3d::new(0.0, DEBUG_PHYSICS_CUBE_EYE_HEIGHT, 0.0))
            .add(direction.scale(DEBUG_PHYSICS_CUBE_SPAWN_DISTANCE)),
        velocity: direction.scale(DEBUG_PHYSICS_CUBE_LAUNCH_SPEED),
    }
}

#[cfg(feature = "physics-engine")]
fn debug_physics_cube_entity_pose(
    pose: mclone_physics::PhysicsBodyPose,
) -> (Vec3d, f32, f32, EntityRotation) {
    let position = pose
        .position
        .add(Vec3d::new(0.0, -DEBUG_PHYSICS_CUBE_HALF_EXTENT, 0.0));
    let forward = rotate_debug_physics_vector(pose.rotation, Vec3d::new(0.0, 0.0, 1.0));
    let horizontal_len = (forward.x * forward.x + forward.z * forward.z).sqrt();
    let y_rot_degrees = (-forward.x).atan2(forward.z).to_degrees() as f32;
    let x_rot_degrees = (-forward.y).atan2(horizontal_len).to_degrees() as f32;
    (
        position,
        y_rot_degrees,
        x_rot_degrees,
        debug_physics_entity_rotation(pose.rotation),
    )
}

#[cfg(feature = "physics-engine")]
fn debug_physics_entity_rotation(rotation: mclone_physics::PhysicsRotation) -> EntityRotation {
    let len_sqr = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    if !len_sqr.is_finite() || len_sqr <= f64::EPSILON {
        return EntityRotation::IDENTITY;
    }
    let inv_len = len_sqr.sqrt().recip();
    EntityRotation {
        x: (rotation.x * inv_len) as f32,
        y: (rotation.y * inv_len) as f32,
        z: (rotation.z * inv_len) as f32,
        w: (rotation.w * inv_len) as f32,
    }
}

#[cfg(feature = "physics-engine")]
fn rotate_debug_physics_vector(rotation: mclone_physics::PhysicsRotation, vector: Vec3d) -> Vec3d {
    let len_sqr = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    if !len_sqr.is_finite() || len_sqr <= f64::EPSILON {
        return vector;
    }
    let inv_len = len_sqr.sqrt().recip();
    let qx = rotation.x * inv_len;
    let qy = rotation.y * inv_len;
    let qz = rotation.z * inv_len;
    let qw = rotation.w * inv_len;
    let tx = 2.0 * (qy * vector.z - qz * vector.y);
    let ty = 2.0 * (qz * vector.x - qx * vector.z);
    let tz = 2.0 * (qx * vector.y - qy * vector.x);
    Vec3d::new(
        vector.x + qw * tx + (qy * tz - qz * ty),
        vector.y + qw * ty + (qz * tx - qx * tz),
        vector.z + qw * tz + (qx * ty - qy * tx),
    )
}

#[cfg(feature = "physics-engine")]
fn look_direction_from_rot(y_rot_degrees: f32, x_rot_degrees: f32) -> Vec3d {
    let yaw = f64::from(y_rot_degrees).to_radians();
    let pitch = f64::from(x_rot_degrees).to_radians();
    let pitch_cos = pitch.cos();
    Vec3d::new(-yaw.sin() * pitch_cos, -pitch.sin(), yaw.cos() * pitch_cos)
}

fn runtime_chunk_target_status(scheduler: &ChunkScheduler) -> mclone_core::ChunkStatus {
    if scheduler.lighting_enabled() {
        mclone_core::ChunkStatus::Light
    } else {
        mclone_core::ChunkStatus::Features
    }
}

fn chunk_record_with_live_ticks(
    snapshot: &ChunkSnapshot,
    block_ticks: &BlockTickList,
    liquid_ticks: &FluidTickList,
    simulation_tick: u64,
) -> ChunkRecord {
    ChunkRecord::from_snapshot(snapshot.clone())
        .with_scheduled_block_ticks(
            block_ticks.scheduled_chunk_tick_records(snapshot.pos, simulation_tick),
        )
        .with_scheduled_fluid_ticks(
            liquid_ticks.scheduled_chunk_tick_records(snapshot.pos, simulation_tick),
        )
}

fn entity_chunk_record_for_persistence(
    entities: &ServerEntityStore,
    dirty_entity_chunks: &BTreeSet<ChunkPos>,
    pos: ChunkPos,
    revision: u64,
) -> Option<EntityChunkRecord> {
    if dirty_entity_chunks.contains(&pos) || entities.has_persistent_entities_in_chunk(pos) {
        Some(entities.entity_chunk_record(pos, revision))
    } else {
        None
    }
}

fn natural_spawn_tick_seed(world_seed: i64, game_time: u64) -> i64 {
    world_seed
        .wrapping_mul(31)
        .wrapping_add((game_time as i64).wrapping_mul(NATURAL_SPAWN_TICK_SEED_MULTIPLIER))
        .wrapping_add(0x5DEECE66D)
}

fn current_unix_millis() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now().max(0.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0)
    }
}

fn validate_world_metadata(
    metadata: &WorldMetadata,
    requested_seed: i64,
    requested_generation: WorldGenerationProfile,
    requested_behavior: WorldBehaviorProfile,
) -> ChunkStoreResult<()> {
    if metadata.seed != requested_seed {
        return Err(ChunkStoreError::InvalidData(format!(
            "world seed mismatch: stored {}, requested {requested_seed}",
            metadata.seed
        )));
    }
    if metadata.world_generation_profile != requested_generation {
        return Err(ChunkStoreError::InvalidData(format!(
            "world generation profile mismatch: stored {}, requested {}",
            metadata.world_generation_profile.label(),
            requested_generation.label()
        )));
    }
    if metadata.world_behavior_profile != requested_behavior {
        return Err(ChunkStoreError::InvalidData(format!(
            "world behavior profile mismatch: stored {}, requested {}",
            metadata.world_behavior_profile.label(),
            requested_behavior.label()
        )));
    }
    Ok(())
}

fn run_noop_simulation_phase(chunks: &[ChunkPos]) -> usize {
    chunks.iter().fold(0, |count, _pos| count + 1)
}

#[cfg(test)]
mod tests;
