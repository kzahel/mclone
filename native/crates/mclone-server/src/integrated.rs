//! Integrated single-process server facade.
//!
//! Move-only home for `IntegratedServer`, the in-process wrapper that drives a
//! `ChunkScheduler` plus the fluid tick list and turns scheduler events into
//! protocol `ServerUpdate`s. Roughly mirrors Java's integrated-server glue over
//! `ServerChunkCache`; the heavy chunk-management logic lives in the scheduler.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::time::Duration;

use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos, ChunkSnapshot, Vec3d,
    obfuscate_biome_zoom_seed,
};
#[cfg(feature = "physics-rapier")]
use mclone_protocol::EntityRotation;
use mclone_protocol::{
    ChunkView, ClientCommand, InteractionHand, MovePlayerCommand, PlayerActionCommand,
    PlayerActionKind, ServerUpdate, SetCarriedItemCommand, SetDebugHotbarSlotCommand,
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
use crate::persistence::{EntityChunkRecord, EntityPersistentId};
#[cfg(feature = "physics-rapier")]
use crate::physics_runtime::ServerPhysicsRuntime;
use crate::placement::DebugBlockItem;
use crate::player::{MovePlayerApplyResult, ServerPlayerState};
use crate::player_chunk_tracking::{PlayerChunkTracking, PlayerChunkTrackingPolicy};
use crate::players::{ServerPlayerId, ServerPlayerList};
use crate::remote_players::{RemotePlayerState, RemotePlayerTracking, RoutedRemotePlayerUpdate};
use crate::spawn::find_safe_surface_spawn;
use crate::timing::{simulation_timing_elapsed_us, simulation_timing_start};
use crate::{
    ChunkLoadingProgress, ChunkLoadingProgressSnapshot, ChunkLoadingProgressStats,
    ChunkPublicationBudgetConfig, ChunkRecord, ChunkScheduler, ChunkSchedulerEvent,
    ChunkSnapshotStore, ChunkStoreError, ChunkStoreResult, FluidKind, FluidTickList,
    NullChunkSnapshotStore, PlayerChunkTrackingDiagnostics, ServerPhysicsStepReport,
    ServerPhysicsStepTiming, ServerPhysicsTickDiagnostics, ServerSimulationTickReport,
    ServerSimulationTickTiming, ServerTickReport, ServerTickTiming, WorldBlockPos, WorldStore,
};

#[cfg(feature = "physics-rapier")]
const DEBUG_PHYSICS_CUBE_EYE_HEIGHT: f64 = 1.5;
#[cfg(feature = "physics-rapier")]
const DEBUG_PHYSICS_CUBE_SPAWN_DISTANCE: f64 = 1.25;
#[cfg(feature = "physics-rapier")]
const DEBUG_PHYSICS_CUBE_LAUNCH_SPEED: f64 = 14.0;
#[cfg(feature = "physics-rapier")]
const DEBUG_PHYSICS_CUBE_HALF_EXTENT: f64 = 0.5;
const DEFAULT_PHYSICS_STEPS_PER_GAMEPLAY_TICK: u32 = 3;
const DEFAULT_PHYSICS_STEP_DT_SECONDS: f64 = 1.0 / 60.0;
const NATURAL_SPAWN_TICK_SEED_MULTIPLIER: i64 = 6_364_136_223_846_793_005;

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    biome_source: ServerBiomeSource,
    scheduler: ChunkScheduler,
    block_ticks: BlockTickList,
    pub(crate) liquid_ticks: FluidTickList,
    simulation_tick: u64,
    day_time: u64,
    day_time_frozen: bool,
    debug_passive_showcase_enabled: bool,
    volatile_natural_spawning_enabled: bool,
    initial_spawn_center: Option<ChunkPos>,
    player: ServerPlayerState,
    inventory: ServerInventory,
    dedicated_players: ServerPlayerList,
    chunk_tracking: PlayerChunkTracking,
    remote_players: RemotePlayerTracking,
    entities: ServerEntityStore,
    entity_tracking: EntityTracking,
    dirty_entity_chunks: BTreeSet<ChunkPos>,
    #[cfg(feature = "physics-rapier")]
    physics: ServerPhysicsRuntime,
    #[cfg(feature = "physics-rapier")]
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

/// Vanilla overworld spawns at morning (`dayTime` 1000), not midnight.
pub const INITIAL_DAY_TIME: u64 = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandTarget {
    Local,
    Dedicated(ServerPlayerId),
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

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self::with_chunk_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn local_integrated(seed: i64) -> Self {
        Self::with_player_chunk_tracking_policy(seed, PlayerChunkTrackingPolicy::java_max())
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self::with_scheduler(seed, ChunkScheduler::with_store(seed, store))
    }

    pub fn with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::with_scheduler(seed, ChunkScheduler::with_world_store(seed, store))
    }

    pub fn local_integrated_with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
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
            seed,
            ChunkScheduler::with_external_load_world_store_and_wasm_job_workers(
                seed, store, config,
            ),
            PlayerChunkTrackingPolicy::java_max(),
        )
    }

    fn with_scheduler(seed: i64, scheduler: ChunkScheduler) -> Self {
        Self::with_scheduler_and_player_chunk_tracking_policy(
            seed,
            scheduler,
            PlayerChunkTrackingPolicy::default(),
        )
    }

    fn with_scheduler_and_player_chunk_tracking_policy(
        seed: i64,
        scheduler: ChunkScheduler,
        policy: PlayerChunkTrackingPolicy,
    ) -> Self {
        let mut chunk_tracking = PlayerChunkTracking::new(policy);
        chunk_tracking.add_player(ServerPlayerId::LOCAL);
        let loading_progress = ChunkLoadingProgress::new(runtime_chunk_target_status(&scheduler));
        Self {
            seed,
            biome_source: ServerBiomeSource::new(seed),
            scheduler,
            block_ticks: BlockTickList::new(),
            liquid_ticks: FluidTickList::new(),
            simulation_tick: 0,
            day_time: INITIAL_DAY_TIME,
            day_time_frozen: false,
            debug_passive_showcase_enabled: true,
            volatile_natural_spawning_enabled: true,
            initial_spawn_center: None,
            player: ServerPlayerState::default(),
            inventory: ServerInventory::default(),
            dedicated_players: ServerPlayerList::default(),
            chunk_tracking,
            remote_players: RemotePlayerTracking::default(),
            entities: ServerEntityStore::default(),
            entity_tracking: EntityTracking::default(),
            dirty_entity_chunks: BTreeSet::new(),
            #[cfg(feature = "physics-rapier")]
            physics: ServerPhysicsRuntime::new(),
            #[cfg(feature = "physics-rapier")]
            debug_physics_player_target: None,
            loading_progress,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn simulation_tick(&self) -> u64 {
        self.simulation_tick
    }

    /// Authoritative world day-time in ticks, driving the day/night cycle.
    pub const fn day_time(&self) -> u64 {
        self.day_time
    }

    /// Set the authoritative day-time. Debug hook for forcing a starting time.
    pub fn set_day_time(&mut self, day_time: u64) {
        self.day_time = day_time;
    }

    /// Freeze or resume the day/night clock. While frozen, simulation ticks leave
    /// `day_time` unchanged (debug hook for inspecting a fixed time of day).
    pub fn set_day_time_frozen(&mut self, frozen: bool) {
        self.day_time_frozen = frozen;
    }

    pub fn set_debug_passive_showcase_enabled(&mut self, enabled: bool) {
        self.debug_passive_showcase_enabled = enabled;
    }

    pub fn set_volatile_natural_spawning_enabled(&mut self, enabled: bool) {
        self.volatile_natural_spawning_enabled = enabled;
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

    pub fn view_readiness_snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        let view = self.chunk_tracking.accepted_view(ServerPlayerId::LOCAL)?;
        Some(self.scheduler.view_readiness_snapshot(view))
    }

    pub fn physics_diagnostics(&self) -> ServerPhysicsTickDiagnostics {
        #[cfg(feature = "physics-rapier")]
        {
            return self.physics.diagnostics();
        }
        #[cfg(not(feature = "physics-rapier"))]
        {
            ServerPhysicsTickDiagnostics::default()
        }
    }

    #[cfg(feature = "physics-rapier")]
    pub fn spawn_debug_physics_cube(&mut self, position: Vec3d, velocity: Vec3d) -> bool {
        self.debug_physics_player_target = None;
        self.physics
            .spawn_debug_cube(&self.scheduler, position, velocity, None)
    }

    #[cfg(feature = "physics-rapier")]
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

    pub fn add_dedicated_player(&mut self) -> ServerPlayerId {
        let player_id = self.dedicated_players.add();
        let world_info = self.world_info_update();
        self.chunk_tracking.add_player(player_id);
        self.chunk_tracking
            .queue_update_for_player(player_id, world_info);
        self.remote_players.add_player(player_id);
        player_id
    }

    pub fn remove_dedicated_player(&mut self, player_id: ServerPlayerId) -> bool {
        if self.dedicated_players.remove(player_id).is_none() {
            return false;
        }
        let routes = self.remote_players.remove_player(player_id);
        self.route_remote_player_updates(routes);
        self.entity_tracking.remove_observer(player_id);
        self.remove_player_chunk_tracking(player_id);
        true
    }

    pub fn dedicated_player_count(&self) -> usize {
        self.dedicated_players.len()
    }

    pub fn dedicated_player_position(&self, player_id: ServerPlayerId) -> Option<Vec3d> {
        self.dedicated_players.position(player_id)
    }

    fn mob_player_targets(&self) -> Vec<MobPlayerTarget> {
        let mut targets = Vec::with_capacity(self.dedicated_players.len() + 1);
        targets.push(MobPlayerTarget::from_position(self.player.position()));
        targets.extend(
            self.dedicated_players
                .iter()
                .map(|(_, entry)| MobPlayerTarget::from_position(entry.state.position())),
        );
        targets
    }

    fn natural_spawn_player_positions(&self) -> Vec<Vec3d> {
        let mut positions = Vec::with_capacity(self.dedicated_players.len() + 1);
        if self.player.has_accepted_position() {
            positions.push(self.player.position());
        }
        positions.extend(
            self.dedicated_players
                .iter()
                .filter(|(_, entry)| entry.state.has_accepted_position())
                .map(|(_, entry)| entry.state.position()),
        );
        positions
    }

    fn item_pickup_targets(&self) -> Vec<ItemPickupTarget> {
        let mut targets = Vec::with_capacity(self.dedicated_players.len() + 1);
        if self.player.has_accepted_position() {
            targets.push(ItemPickupTarget {
                player_id: ServerPlayerId::LOCAL,
                position: self.player.position(),
            });
        }
        targets.extend(
            self.dedicated_players
                .iter()
                .filter(|(_, entry)| entry.state.has_accepted_position())
                .map(|(player_id, entry)| ItemPickupTarget {
                    player_id,
                    position: entry.state.position(),
                }),
        );
        targets
    }

    pub fn handle_command(&mut self, command: ClientCommand) -> Vec<ServerUpdate> {
        self.try_handle_command(command)
            .expect("integrated server command failed")
    }

    pub fn try_handle_command(
        &mut self,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_handle_command_for_target(CommandTarget::Local, command)
    }

    pub fn try_handle_command_for_player(
        &mut self,
        player_id: ServerPlayerId,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_handle_command_for_target(CommandTarget::Dedicated(player_id), command)
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
        }
    }

    pub fn poll(&mut self) -> Vec<ServerUpdate> {
        self.try_poll().expect("integrated server poll failed")
    }

    pub fn try_poll(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_poll_for_target(CommandTarget::Local)
    }

    pub fn try_poll_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_poll_for_target(CommandTarget::Dedicated(player_id))
    }

    fn try_poll_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        let events = self.scheduler.poll()?;
        self.apply_scheduler_events_for_target(target, events)
    }

    pub fn tick(&mut self) -> Vec<ServerUpdate> {
        self.try_tick().expect("integrated server tick failed")
    }

    pub fn try_tick(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        Ok(self.try_simulation_tick_report()?.updates)
    }

    pub fn tick_report(&mut self) -> ServerTickReport {
        self.try_tick_report()
            .expect("integrated server tick report failed")
    }

    pub fn try_tick_report(&mut self) -> ChunkStoreResult<ServerTickReport> {
        self.try_tick_report_for_target(CommandTarget::Local)
    }

    fn try_tick_report_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<ServerTickReport> {
        self.ensure_target_exists(target)?;
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
        let updates = self.apply_scheduler_events_for_target(target, report.events)?;
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
            updates,
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

    pub fn simulation_tick_report(&mut self) -> ServerSimulationTickReport {
        self.try_simulation_tick_report()
            .expect("integrated server simulation tick report failed")
    }

    pub fn try_simulation_tick_report(&mut self) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_for_target(CommandTarget::Local)
    }

    pub fn try_simulation_tick_report_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_for_target(CommandTarget::Dedicated(player_id))
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
        self.try_simulation_tick_report_for_target_with_physics_steps(
            CommandTarget::Local,
            physics_steps,
            physics_step_dt_seconds,
        )
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
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let tick_report = self.try_tick_report_for_target(target)?;
        let scheduler_tick_us = simulation_timing_elapsed_us(scheduler_start);
        let tick_timing = tick_report.timing;

        let simulation_tick = self.simulation_tick.saturating_add(1);
        self.simulation_tick = simulation_tick;
        self.mark_player_tick_boundaries();

        // Advance the day/night clock one tick (Java `ServerLevel.tickTime` with
        // `doDaylightCycle` on). Coupled to the simulation tick cadence, which is
        // itself frame-driven in the current runtime; revisit if/when ticks are
        // fixed-step. Skipped while frozen (debug `--freeze-time`).
        if !self.day_time_frozen {
            self.day_time = self.day_time.wrapping_add(1);
        }

        let block_tick_start = simulation_timing_start();
        let block_tick_chunks = run_noop_simulation_phase(&tick_report.block_ticking_chunks);
        let _block_tick_report =
            self.tick_scheduled_blocks(simulation_tick, &tick_report.block_ticking_chunks);
        let block_tick_us = simulation_timing_elapsed_us(block_tick_start);

        let fluid_tick_start = simulation_timing_start();
        let (fluid_report, mut fluid_events, fluid_mutated_positions) = self.liquid_ticks.tick(
            simulation_tick,
            &tick_report.entity_ticking_chunks,
            &mut self.scheduler,
        );
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
        let local_inventory = &mut self.inventory;
        let dedicated_players = &mut self.dedicated_players;
        entity_updates.extend(self.entities.collect_item_entities(
            &item_pickup_targets,
            |player_id, stack| {
                if player_id == ServerPlayerId::LOCAL {
                    return local_inventory.add_item_stack(stack).remaining;
                }
                dedicated_players
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
            #[cfg(feature = "physics-rapier")]
            self.sync_debug_physics_player_collider();
            let physics = self.step_physics_steps(physics_steps, physics_step_dt_seconds);
            #[cfg(feature = "physics-rapier")]
            if let Some(entity) = self.sync_debug_physics_cube_entity(simulation_tick, physics) {
                entity_updates.push(entity);
            }
            physics
        };
        let physics_tick_us = simulation_timing_elapsed_us(physics_tick_start);
        let entity_chunks_after_tick = self.entities.persistent_entity_chunk_positions();
        self.mark_entity_chunk_index_changes(entity_chunks_before_tick, entity_chunks_after_tick);
        self.mark_entity_updates_dirty(&entity_updates);

        let mut updates = tick_report.updates;
        updates.push(ServerUpdate::TimeUpdate {
            day_time: self.day_time,
        });
        let fluid_event_apply_start = simulation_timing_start();
        self.route_scheduler_events(fluid_events)?;
        self.reconcile_entity_subjects(entity_updates, true);
        updates.extend(self.drain_chunk_updates_for_target(target)?);
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
            updates,
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
    pub(crate) fn try_physics_step_report(
        &mut self,
        physics_steps: u32,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        self.try_physics_step_report_with_step_dt(physics_steps, DEFAULT_PHYSICS_STEP_DT_SECONDS)
    }

    pub(crate) fn try_physics_step_report_with_step_dt(
        &mut self,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        self.try_physics_step_report_for_target(
            CommandTarget::Local,
            physics_steps,
            physics_step_dt_seconds,
        )
    }

    fn try_physics_step_report_for_target(
        &mut self,
        target: CommandTarget,
        physics_steps: u32,
        physics_step_dt_seconds: f64,
    ) -> ChunkStoreResult<ServerPhysicsStepReport> {
        self.ensure_target_exists(target)?;
        let total_start = simulation_timing_start();

        let physics_tick_start = simulation_timing_start();
        #[cfg(feature = "physics-rapier")]
        if physics_steps > 0 {
            self.sync_debug_physics_player_collider();
        }
        let physics = self.step_physics_steps(physics_steps, physics_step_dt_seconds);
        let physics_tick_us = simulation_timing_elapsed_us(physics_tick_start);

        let physics_event_apply_start = simulation_timing_start();
        #[cfg(feature = "physics-rapier")]
        if physics_steps > 0 {
            if let Some(entity) = self.sync_debug_physics_cube_entity(self.simulation_tick, physics)
            {
                self.reconcile_entity_subjects(std::iter::once(entity), true);
            }
        }
        let updates = self.drain_chunk_updates_for_target(target)?;
        let physics_event_apply_us = simulation_timing_elapsed_us(physics_event_apply_start);
        let chunk_tracking = self.chunk_tracking_diagnostics();

        Ok(ServerPhysicsStepReport {
            simulation_tick: self.simulation_tick,
            physics_steps,
            physics,
            chunk_tracking,
            updates,
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
        #[cfg(feature = "physics-rapier")]
        {
            self.physics
                .step_steps(physics_steps, physics_step_dt_seconds)
        }
        #[cfg(not(feature = "physics-rapier"))]
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
        let queued = self.save_dirty_chunks()?;
        self.scheduler.close_persistence()?;
        Ok(queued)
    }

    fn set_chunk_view_for_target(
        &mut self,
        target: CommandTarget,
        view: ChunkView,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        if target == CommandTarget::Local {
            self.loading_progress.set_view(&view);
        }
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
        command: MovePlayerCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let simulation_tick = self.simulation_tick;
        let player = self.player_mut_for_target(target)?;
        let result = player.apply_move_player(command);
        let updates = match result {
            MovePlayerApplyResult::Accepted | MovePlayerApplyResult::RejectedInvalid => Vec::new(),
            MovePlayerApplyResult::AwaitingTeleport => player
                .resend_pending_correction_update(simulation_tick)
                .map(ServerUpdate::PlayerPosition)
                .into_iter()
                .collect(),
        };
        if result == MovePlayerApplyResult::Accepted {
            self.reconcile_remote_player_subject(target.player_id(), true);
        }
        Ok(updates)
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
        let Some(player_id) = target.dedicated_player_id() else {
            return Ok(Vec::new());
        };
        let Some(player) = self.dedicated_players.get_mut(player_id) else {
            return Err(unknown_player_error(player_id));
        };
        if player.appearance != command.appearance {
            player.appearance = command.appearance;
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
        if command.kind == PlayerActionKind::DebugInstantBreak {
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
        if let Some((target, block_state)) =
            self.held_item_place_target_for_target(target, command)?
        {
            self.set_block_debug(target, block_state);
        }
        self.drain_pending_block_delta_updates_for_target(target)
    }

    fn handle_shoot_debug_physics_cube_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        #[cfg(not(feature = "physics-rapier"))]
        {
            self.ensure_target_exists(target)?;
            Ok(Vec::new())
        }
        #[cfg(feature = "physics-rapier")]
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
        let Some(observer) = target.dedicated_player_id() else {
            return;
        };
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
        if !self.dedicated_players.contains(subject) {
            return;
        }
        let Some(state) = self.remote_player_state(subject) else {
            return;
        };
        let observers = self
            .dedicated_players
            .iter()
            .map(|(player_id, _)| player_id)
            .collect::<Vec<_>>();
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
        self.dedicated_players
            .iter()
            .map(|(player_id, _)| {
                self.remote_player_state(player_id)
                    .expect("iterated dedicated player must have state")
            })
            .collect()
    }

    fn remote_player_state(&self, player_id: ServerPlayerId) -> Option<RemotePlayerState> {
        let player = self.dedicated_players.get(player_id)?;
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

    #[cfg(feature = "physics-rapier")]
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

    #[cfg(feature = "physics-rapier")]
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

    #[cfg(feature = "physics-rapier")]
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
        std::iter::once(ServerPlayerId::LOCAL)
            .chain(
                self.dedicated_players
                    .iter()
                    .map(|(player_id, _)| player_id),
            )
            .collect()
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
        let Some(position) = find_safe_surface_spawn(
            center,
            |pos| self.scheduler.block_at_world(pos),
            |x, z| self.biome_source.block_position_biome_definition(x, z),
            |chunk| self.scheduler.client_visible_snapshot(chunk).is_some(),
        ) else {
            return Ok(None);
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
        Ok(Some(
            self.player_mut_for_target(target)?.initial_position_update(
                position,
                0.0,
                0.0,
                simulation_tick,
            ),
        ))
    }

    fn set_initial_spawn_center_for_target(
        &mut self,
        target: CommandTarget,
        center: ChunkPos,
    ) -> ChunkStoreResult<()> {
        match target {
            CommandTarget::Local => {
                if self.initial_spawn_center.is_none() && self.player.needs_initial_position_sync()
                {
                    self.initial_spawn_center = Some(center);
                }
                Ok(())
            }
            CommandTarget::Dedicated(player_id) => {
                let player = self
                    .dedicated_players
                    .get_mut(player_id)
                    .ok_or_else(|| unknown_player_error(player_id))?;
                if player.initial_spawn_center.is_none()
                    && player.state.needs_initial_position_sync()
                {
                    player.initial_spawn_center = Some(center);
                }
                Ok(())
            }
        }
    }

    fn initial_spawn_center_for_target(
        &self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<ChunkPos>> {
        match target {
            CommandTarget::Local => Ok(self.initial_spawn_center),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get(player_id)
                .map(|player| player.initial_spawn_center)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn player_for_target(&self, target: CommandTarget) -> ChunkStoreResult<&ServerPlayerState> {
        match target {
            CommandTarget::Local => Ok(&self.player),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get(player_id)
                .map(|player| &player.state)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn player_mut_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<&mut ServerPlayerState> {
        match target {
            CommandTarget::Local => Ok(&mut self.player),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get_mut(player_id)
                .map(|player| &mut player.state)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn inventory_for_target(&self, target: CommandTarget) -> ChunkStoreResult<&ServerInventory> {
        match target {
            CommandTarget::Local => Ok(&self.inventory),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get(player_id)
                .map(|player| &player.inventory)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn inventory_mut_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<&mut ServerInventory> {
        match target {
            CommandTarget::Local => Ok(&mut self.inventory),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get_mut(player_id)
                .map(|player| &mut player.inventory)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn ensure_target_exists(&self, target: CommandTarget) -> ChunkStoreResult<()> {
        match target {
            CommandTarget::Local => Ok(()),
            CommandTarget::Dedicated(player_id) if self.dedicated_players.contains(player_id) => {
                Ok(())
            }
            CommandTarget::Dedicated(player_id) => Err(unknown_player_error(player_id)),
        }
    }

    fn mark_player_tick_boundaries(&mut self) {
        self.player.mark_tick_boundary();
        for player in self.dedicated_players.values_mut() {
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
            .expect("failed to reconcile chunk tracking after dedicated player disconnect");
        self.route_scheduler_events(events)
            .expect("failed to route scheduler events after dedicated player disconnect");
    }
}

impl CommandTarget {
    const fn player_id(self) -> ServerPlayerId {
        match self {
            CommandTarget::Local => ServerPlayerId::LOCAL,
            CommandTarget::Dedicated(player_id) => player_id,
        }
    }

    const fn dedicated_player_id(self) -> Option<ServerPlayerId> {
        match self {
            CommandTarget::Local => None,
            CommandTarget::Dedicated(player_id) => Some(player_id),
        }
    }
}

fn unknown_player_error(player_id: ServerPlayerId) -> ChunkStoreError {
    ChunkStoreError::InvalidData(format!("unknown server player {player_id}"))
}

fn raw_block_id_from_block_state(block_state: BlockStateId) -> Option<RawBlockId> {
    RawBlockId::try_from(block_state.0).ok()
}

#[cfg(feature = "physics-rapier")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct DebugPhysicsCubeLaunch {
    position: Vec3d,
    velocity: Vec3d,
}

#[cfg(feature = "physics-rapier")]
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

#[cfg(feature = "physics-rapier")]
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

#[cfg(feature = "physics-rapier")]
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

#[cfg(feature = "physics-rapier")]
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

#[cfg(feature = "physics-rapier")]
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

fn run_noop_simulation_phase(chunks: &[ChunkPos]) -> usize {
    chunks.iter().fold(0, |count, _pos| count + 1)
}

#[cfg(test)]
mod tests;
