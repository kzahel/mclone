//! Chunk scheduler: ticket-driven chunk status management, worldgen publication,
//! and fluid ticking.
//!
//! Move-only home for `ChunkScheduler` and its directly-coupled machinery,
//! roughly mirroring Java's `server/level/ChunkMap` + `ServerChunkCache`: per-tick
//! ticket reconciliation, status scheduling against per-chunk `ChunkHolder`s,
//! worldgen job dispatch through the mailbox, snapshot publication, and the fluid
//! tick list + per-tick fluid mutation. Plain data lives in `crate::types`; holder
//! state, ticket distance, fluid rules, provisional lighting, timing, and the
//! worldgen mailbox live in their own sibling modules.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;

use mclone_core::{
    BlockStateId, CHUNK_SECTION_VOLUME, ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus,
    LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection, block_to_section_coord, chunk_section_index,
    local_block_coord, local_section_block_coord,
};
use mclone_frame_budget::{
    BudgetController, BudgetControllerConfig, BudgetControllerInput, BudgetDecisionAddress,
    BudgetDecisionFamily, BudgetDecisionReport, BudgetTelemetryWindow, DEFAULT_COST_EWMA_ALPHA,
    EwmaCostEstimator, FamilyBudgetConfig, FrameHostKind, StageId, WorkWindow,
};
use mclone_protocol::{ChunkView, SectionBlockUpdate};
use mclone_worldgen::block::{
    OBSIDIAN, RawBlockId, STONE, block_light_emission, block_light_opacity,
    generated_block_state_id, is_water, material_blocks_motion,
};
use mclone_worldgen::feature::{FEATURES_BLOCK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF};
use mclone_worldgen::levelgen::{
    GeneratedChunk, MutableChunkBlockBuffer, OverworldFeatureBatchTiming,
    OverworldFeatureDependencyCacheReport, ScheduledTick,
};

#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
use crate::distance_manager::ChunkDistanceManager;
use crate::fluid::{
    ALL_FLUID_DIRECTIONS, FluidDirection, HORIZONTAL_FLUID_DIRECTIONS,
    LAVA_SOURCE_CONTACT_DIRECTIONS, NativeFluidState, fluid_cache_key, fluid_can_convert_to_source,
    fluid_drop_off, fluid_slope_find_distance, legacy_block_for_fluid_state, offset_pos,
    target_fluid_can_be_replaced_with,
};
use crate::holder::ChunkHolder;
use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_mailbox::{CompletedLightStatus, LightStatusMailbox};
use crate::light_status::{
    PendingLightStatus, PendingLightStatusBatch, hydrate_loaded_light_snapshot,
};
use crate::light_world::RetainedInitialLightState;
use crate::lighting_seed::provisional_sky_light_includes_chunk;
use crate::loading_progress::{
    ChunkLoadingProgressCell, ChunkLoadingProgressSnapshot, ChunkLoadingProgressStats,
    PLAYABLE_GATE_RADIUS, playable_gate_ready_chunk_count,
};
use crate::persistence::{
    ChunkRecord, ChunkSnapshotStore, ChunkSnapshotWorldStore, ChunkStoreError, ChunkStoreResult,
    EntityChunkRecord, PersistenceMailbox, PersistenceRequestId, SaveDurability,
    ScheduledTickRecord, StoreWriteOutcome, WorldStore, WorldStoreCompletion, WorldStoreRequest,
};
use crate::player_chunk_tracking::chunk_positions_for_view;
use crate::timing::{
    ChunkSchedulerPublicationDiagnostics, ChunkSchedulerTickReport, ChunkSchedulerTickTiming,
    simulation_timing_elapsed_us, simulation_timing_start,
};
use crate::worldgen_mailbox::{PendingWorldgenPublication, WorldgenMailbox};
use crate::{
    CHUNK_LEVEL_FULL, ChunkJobId, ChunkJobState, ChunkResidency, ChunkStatusStep, ChunkTicketKey,
    ChunkTicketType, DEFAULT_GAMEPLAY_RATE_HZ, FORCED_TICKET_LEVEL, FluidKind, FullChunkStatus,
    LightStatusMailboxKind, LightStatusMailboxMetrics, MAX_CHUNK_DISTANCE, UNLOADED_CHUNK_LEVEL,
    WorkerFrameMetrics, WorldBlockPos, WorldgenMailboxKind, full_chunk_status_for_ticket_level,
};

#[cfg(not(target_arch = "wasm32"))]
type SimulationTimingStart = Option<std::time::Instant>;
#[cfg(target_arch = "wasm32")]
type SimulationTimingStart = Option<()>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkStatusJob {
    pub id: ChunkJobId,
    pub status: ChunkStatus,
    pub state: ChunkJobState,
    pub target_chunks: Vec<ChunkPos>,
    pub feature_centers: Vec<ChunkPos>,
    pub dependency_chunks: Vec<ChunkPos>,
    pub seeded_dependency_chunks: usize,
    pub dependency_cache_hits: usize,
    pub dependency_cache_misses: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChunkSchedulerMetrics {
    pub direct_ticket_chunks: usize,
    pub active_ticket_chunks: usize,
    pub holder_chunks: usize,
    pub pending_unload_chunks: usize,
    pub inaccessible_status_chunks: usize,
    pub border_status_chunks: usize,
    pub ticking_status_chunks: usize,
    pub entity_ticking_status_chunks: usize,
    pub block_ticking_chunks: usize,
    pub client_visible_chunks: usize,
    pub loaded_snapshot_chunks: usize,
    pub dependency_holder_chunks: usize,
    pub ready_dependency_chunks: usize,
    pub dirty_chunks: usize,
    pub pending_jobs: usize,
    pub completed_jobs: usize,
    pub total_seeded_dependency_chunks: usize,
    pub total_dependency_cache_hits: usize,
    pub total_dependency_cache_misses: usize,
    pub total_retained_dependency_chunks: usize,
    pub max_feature_job_target_chunks: usize,
    pub max_feature_job_feature_centers: usize,
    pub max_feature_job_dependency_chunks: usize,
    pub latest_feature_job_id: Option<ChunkJobId>,
    pub latest_feature_job_target_chunks: usize,
    pub latest_feature_job_feature_centers: usize,
    pub latest_feature_job_dependency_chunks: usize,
    pub latest_feature_job_first_target: Option<ChunkPos>,
    pub completed_light_statuses: usize,
    pub completed_light_batches: usize,
    pub total_light_status_compute_us: u128,
    pub max_light_status_compute_us: u128,
    pub total_light_status_world_init_us: u128,
    pub total_light_status_active_sections_us: u128,
    pub total_light_status_sky_source_scan_us: u128,
    pub total_light_status_block_source_scan_us: u128,
    pub total_light_status_engine_init_us: u128,
    pub total_light_status_section_setup_us: u128,
    pub total_light_status_section_status_update_us: u128,
    pub total_light_status_sky_column_enable_us: u128,
    pub total_light_status_sky_source_enqueue_us: u128,
    pub total_light_status_block_source_enqueue_us: u128,
    pub total_light_status_input_chunks: usize,
    pub total_light_status_inserted_chunks: usize,
    pub total_light_status_replaced_chunks: usize,
    pub total_light_status_unchanged_chunks: usize,
    pub total_light_status_changed_block_raw_checks: usize,
    pub total_light_status_changed_block_light_property_changes: usize,
    pub total_light_status_changed_block_opacity_changes: usize,
    pub total_light_status_changed_block_emission_changes: usize,
    pub total_light_status_changed_block_raw_only_changes: usize,
    pub total_light_status_changed_block_check_us: u128,
    pub total_light_status_run_updates_us: u128,
    pub total_light_status_run_update_iterations: usize,
    pub total_light_status_block_run_update_calls: usize,
    pub total_light_status_sky_run_update_calls: usize,
    pub total_light_status_block_run_update_processed_nodes: usize,
    pub total_light_status_sky_run_update_processed_nodes: usize,
    pub max_light_status_block_run_update_queue_before: usize,
    pub max_light_status_sky_run_update_queue_before: usize,
    pub final_light_status_block_run_update_queue_after: usize,
    pub final_light_status_sky_run_update_queue_after: usize,
    pub total_light_status_block_run_updates_us: u128,
    pub total_light_status_sky_run_updates_us: u128,
    pub total_light_status_sky_source_update_count: usize,
    pub total_light_status_sky_source_updates_us: u128,
    pub total_light_status_block_run_update_graph_us: u128,
    pub total_light_status_sky_run_update_graph_us: u128,
    pub total_light_status_block_run_update_storage_swap_us: u128,
    pub total_light_status_sky_run_update_storage_swap_us: u128,
    pub total_light_status_block_run_update_affected_sections: usize,
    pub total_light_status_sky_run_update_affected_sections: usize,
    pub total_light_status_collect_sections_us: u128,
    pub total_light_status_publication_us: u128,
    pub max_light_status_publication_us: u128,
    pub total_light_status_publication_units: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FluidTickPhaseReport {
    pub scheduled_ticks: usize,
    pub due_ticks: usize,
    pub deferred_due_ticks: usize,
    pub executed_ticks: usize,
    pub mutated_blocks: usize,
    pub snapshot_events: usize,
    pub event_count: usize,
    pub due_scan_us: u128,
    pub remove_due_us: u128,
    pub tick_fluid_us: u128,
    pub set_block_us: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct FluidTickKey {
    pos: WorldBlockPos,
    fluid: FluidKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ScheduledFluidTick {
    trigger_tick: u64,
    sequence: u64,
    key: FluidTickKey,
}

#[derive(Debug, Default)]
pub(crate) struct FluidTickList {
    scheduled_keys: BTreeSet<FluidTickKey>,
    scheduled_ticks: BTreeSet<ScheduledFluidTick>,
    next_sequence: u64,
}

impl FluidTickList {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn schedule_tick(
        &mut self,
        pos: WorldBlockPos,
        fluid: FluidKind,
        delay: i32,
        game_time: u64,
    ) {
        let key = FluidTickKey { pos, fluid };
        if !self.scheduled_keys.insert(key) {
            return;
        }

        let entry = ScheduledFluidTick {
            trigger_tick: game_time.saturating_add(delay.max(0) as u64),
            sequence: self.next_sequence,
            key,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.scheduled_ticks.insert(entry);
    }

    pub(crate) fn size(&self) -> usize {
        self.scheduled_keys.len()
    }

    pub(crate) fn scheduled_chunk_tick_records(
        &self,
        chunk_pos: ChunkPos,
        game_time: u64,
    ) -> Vec<ScheduledTickRecord> {
        self.scheduled_ticks
            .iter()
            .filter(|entry| entry.key.pos.chunk_pos() == chunk_pos)
            .map(|entry| {
                ScheduledTickRecord::new(
                    entry.key.pos,
                    scheduled_fluid_tick_target(entry.key.fluid),
                    remaining_tick_delay(entry.trigger_tick, game_time),
                )
            })
            .collect()
    }

    pub(crate) fn remove_chunk_ticks(&mut self, chunk_pos: ChunkPos) -> usize {
        let entries = self
            .scheduled_ticks
            .iter()
            .filter(|entry| entry.key.pos.chunk_pos() == chunk_pos)
            .copied()
            .collect::<Vec<_>>();
        for entry in &entries {
            self.scheduled_ticks.remove(entry);
            self.scheduled_keys.remove(&entry.key);
        }
        entries.len()
    }

    #[cfg(test)]
    pub(crate) fn has_scheduled_tick(&self, pos: WorldBlockPos, fluid: FluidKind) -> bool {
        self.scheduled_keys.contains(&FluidTickKey { pos, fluid })
    }

    #[cfg(test)]
    pub(crate) fn scheduled_tick_entries(
        &self,
        game_time: u64,
    ) -> Vec<(WorldBlockPos, FluidKind, i32)> {
        let mut entries = self
            .scheduled_ticks
            .iter()
            .map(|entry| {
                (
                    entry.key.pos,
                    entry.key.fluid,
                    entry.trigger_tick.saturating_sub(game_time) as i32,
                )
            })
            .collect::<Vec<_>>();
        entries.sort();
        entries
    }

    pub(crate) fn frozen_report(
        &self,
    ) -> (
        FluidTickPhaseReport,
        Vec<ChunkSchedulerEvent>,
        Vec<WorldBlockPos>,
    ) {
        (
            FluidTickPhaseReport {
                scheduled_ticks: self.size(),
                ..FluidTickPhaseReport::default()
            },
            Vec::new(),
            Vec::new(),
        )
    }

    pub(crate) fn tick(
        &mut self,
        game_time: u64,
        entity_ticking_chunks: &[ChunkPos],
        scheduler: &mut ChunkScheduler,
    ) -> (
        FluidTickPhaseReport,
        Vec<ChunkSchedulerEvent>,
        Vec<WorldBlockPos>,
    ) {
        let due_scan_start = simulation_timing_start();
        let ticking_chunks = entity_ticking_chunks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut deferred_due_ticks: usize = 0;
        let mut due_ticks = Vec::new();
        for entry in self
            .scheduled_ticks
            .iter()
            .take_while(|entry| entry.trigger_tick <= game_time)
        {
            if ticking_chunks.contains(&entry.key.pos.chunk_pos()) {
                due_ticks.push(*entry);
                if due_ticks.len() == MAX_SCHEDULED_FLUID_TICKS_PER_TICK {
                    break;
                }
            } else {
                deferred_due_ticks += 1;
            }
        }
        let due_scan_us = simulation_timing_elapsed_us(due_scan_start);

        let mut events = Vec::new();
        let mut executed_ticks: usize = 0;
        let mut mutated_blocks: usize = 0;
        let mut mutated_positions = Vec::new();
        let mut snapshot_events: usize = 0;
        let mut tick_fluid_us = 0;
        let mut set_block_us = 0;
        let remove_due_start = simulation_timing_start();
        for entry in &due_ticks {
            self.scheduled_ticks.remove(entry);
            self.scheduled_keys.remove(&entry.key);
        }
        let remove_due_us = simulation_timing_elapsed_us(remove_due_start);

        for entry in due_ticks {
            executed_ticks += 1;

            let tick_fluid_start = simulation_timing_start();
            let mutation = scheduler.tick_fluid(entry.key.pos, entry.key.fluid);
            tick_fluid_us += simulation_timing_elapsed_us(tick_fluid_start);
            mutated_blocks += mutation.mutated_positions.len();
            mutated_positions.extend(mutation.mutated_positions.iter().copied());
            snapshot_events += mutation.snapshot_events;
            set_block_us += mutation.set_block_us;
            for scheduled in mutation.scheduled_ticks {
                self.schedule_tick(scheduled.pos, scheduled.fluid, scheduled.delay, game_time);
            }
            events.extend(mutation.events);
        }
        let event_count = events.len();
        mutated_positions.sort();
        mutated_positions.dedup();

        (
            FluidTickPhaseReport {
                scheduled_ticks: self.size(),
                due_ticks: executed_ticks.saturating_add(deferred_due_ticks),
                deferred_due_ticks,
                executed_ticks,
                mutated_blocks,
                snapshot_events,
                event_count,
                due_scan_us,
                remove_due_us,
                tick_fluid_us,
                set_block_us,
            },
            events,
            mutated_positions,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChunkSchedulerEvent {
    StatusChanged {
        pos: ChunkPos,
        status: ChunkStatus,
        step: ChunkStatusStep,
    },
    SnapshotReady(ChunkSnapshot),
    Unloaded {
        pos: ChunkPos,
    },
    HolderUnloaded {
        pos: ChunkPos,
    },
    EntityChunkLoaded {
        pos: ChunkPos,
        record: Option<EntityChunkRecord>,
    },
    SectionBlockUpdates {
        pos: ChunkPos,
        section_y: i32,
        updates: Vec<SectionBlockUpdate>,
    },
    FluidTickScheduled {
        pos: WorldBlockPos,
        fluid: FluidKind,
        delay: i32,
    },
    BlockTickScheduled {
        pos: WorldBlockPos,
        target: String,
        delay: i32,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FluidMutationReport {
    events: Vec<ChunkSchedulerEvent>,
    mutated_positions: Vec<WorldBlockPos>,
    scheduled_ticks: Vec<ScheduledFluidTickRequest>,
    snapshot_events: usize,
    set_block_us: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScheduledFluidTickRequest {
    pub(crate) pos: WorldBlockPos,
    pub(crate) fluid: FluidKind,
    pub(crate) delay: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingChunkLoad {
    pos: ChunkPos,
    target_status: ChunkStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingChunkSave {
    pos: ChunkPos,
    revision: ChunkRevision,
    durability: SaveDurability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingEntityChunkSave {
    pos: ChunkPos,
    revision: u64,
    durability: SaveDurability,
}

#[derive(Debug)]
pub struct ChunkScheduler {
    seed: i64,
    lighting_enabled: bool,
    light_status_batch_size: usize,
    holders: BTreeMap<ChunkPos, ChunkHolder>,
    pending_unloads: BTreeSet<ChunkPos>,
    pending_chunk_loads: BTreeMap<PersistenceRequestId, PendingChunkLoad>,
    pending_chunk_load_by_pos: BTreeMap<ChunkPos, PersistenceRequestId>,
    stored_chunk_misses: BTreeSet<ChunkPos>,
    pending_chunk_saves: BTreeMap<PersistenceRequestId, PendingChunkSave>,
    pending_chunk_save_by_pos: BTreeMap<ChunkPos, PersistenceRequestId>,
    pending_entity_chunk_loads: BTreeMap<PersistenceRequestId, ChunkPos>,
    pending_entity_chunk_load_by_pos: BTreeMap<ChunkPos, PersistenceRequestId>,
    loaded_entity_chunks: BTreeSet<ChunkPos>,
    pending_entity_chunk_saves: BTreeMap<PersistenceRequestId, PendingEntityChunkSave>,
    pending_entity_chunk_save_by_pos: BTreeMap<ChunkPos, PersistenceRequestId>,
    entity_unload_saves: BTreeSet<ChunkPos>,
    pub(crate) distance_manager: ChunkDistanceManager,
    jobs: BTreeMap<ChunkJobId, ChunkStatusJob>,
    job_timings: BTreeMap<ChunkJobId, OverworldFeatureBatchTiming>,
    worldgen_mailbox: WorldgenMailbox,
    pending_worldgen_publications: VecDeque<PendingWorldgenPublication>,
    publication_budget: ChunkPublicationBudgetState,
    light_mailbox: LightStatusMailbox,
    pending_light_status_batches: BTreeMap<ChunkJobId, Vec<PendingLightStatus>>,
    pending_light_publications: VecDeque<CompletedLightStatus>,
    completed_light_statuses: usize,
    completed_light_batches: usize,
    total_light_status_compute_us: u128,
    max_light_status_compute_us: u128,
    total_light_status_publication_us: u128,
    max_light_status_publication_us: u128,
    total_light_status_publication_units: usize,
    light_status_timing: LevelLightComputationTiming,
    pending_block_deltas: BTreeMap<(ChunkPos, i32), BTreeMap<usize, BlockStateId>>,
    pending_runtime_light_snapshots: VecDeque<ChunkSnapshot>,
    dirty_chunks: BTreeSet<ChunkPos>,
    next_job_id: u64,
    next_revision: u64,
    store: PersistenceMailbox,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkPublicationBudgetConfig {
    pub enabled: bool,
    pub gameplay_rate_hz: u32,
}

impl ChunkPublicationBudgetConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            gameplay_rate_hz: DEFAULT_GAMEPLAY_RATE_HZ,
        }
    }

    pub const fn adaptive_for_gameplay_rate_hz(gameplay_rate_hz: u32) -> Self {
        Self {
            enabled: true,
            gameplay_rate_hz,
        }
    }

    fn target_period_ms(self) -> Option<f64> {
        if !self.enabled || self.gameplay_rate_hz == 0 {
            return None;
        }
        Some(1_000.0 / f64::from(self.gameplay_rate_hz))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ChunkPublicationBudgetState {
    config: ChunkPublicationBudgetConfig,
    controller: BudgetController,
    estimator: EwmaCostEstimator,
    last_pending_worldgen_publication_chunk_limit: usize,
}

impl Default for ChunkPublicationBudgetState {
    fn default() -> Self {
        Self::new(ChunkPublicationBudgetConfig::disabled())
    }
}

impl ChunkPublicationBudgetState {
    fn new(config: ChunkPublicationBudgetConfig) -> Self {
        Self {
            config,
            controller: BudgetController::new(chunk_publication_budget_controller_config()),
            estimator: EwmaCostEstimator::new(DEFAULT_COST_EWMA_ALPHA),
            last_pending_worldgen_publication_chunk_limit:
                DEFAULT_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT,
        }
    }

    fn enabled(&self) -> bool {
        self.config.enabled
    }

    fn set_config(&mut self, config: ChunkPublicationBudgetConfig) {
        if self.config == config {
            return;
        }
        *self = Self::new(config);
    }

    fn set_gameplay_rate_hz(&mut self, gameplay_rate_hz: u32) {
        self.set_config(ChunkPublicationBudgetConfig {
            gameplay_rate_hz,
            ..self.config
        });
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PublicationGrant {
    min_units: usize,
    max_units: usize,
    elapsed_us: u128,
}

impl PublicationGrant {
    const fn fixed(units: usize) -> Self {
        Self {
            min_units: units,
            max_units: units,
            elapsed_us: 0,
        }
    }

    fn from_decision(
        decision: Option<&BudgetDecisionReport>,
        fallback_units: usize,
        estimated_unit_us: Option<u128>,
    ) -> Self {
        decision.map_or_else(
            || Self::fixed(fallback_units),
            |decision| {
                let min_units = decision.grant.min_units as usize;
                let decision_max_units = decision.grant.max_units as usize;
                let elapsed_us = elapsed_ms_to_us(decision.grant.elapsed_ms);
                Self {
                    min_units,
                    max_units: publication_max_units_from_elapsed_cost(
                        min_units,
                        decision_max_units,
                        elapsed_us,
                        estimated_unit_us,
                    ),
                    elapsed_us,
                }
            },
        )
    }

    fn allows_next_unit(self, start: SimulationTimingStart, spent_units: usize) -> bool {
        if spent_units < self.min_units {
            return true;
        }
        if spent_units >= self.max_units {
            return false;
        }
        self.elapsed_us == 0 || simulation_timing_elapsed_us(start) < self.elapsed_us
    }
}

fn publication_max_units_from_elapsed_cost(
    min_units: usize,
    cold_estimator_max_units: usize,
    elapsed_us: u128,
    estimated_unit_us: Option<u128>,
) -> usize {
    let fallback_units = cold_estimator_max_units.max(min_units);
    let Some(unit_us) = estimated_unit_us.filter(|unit_us| *unit_us > 0) else {
        return fallback_units;
    };
    if elapsed_us == 0 {
        return fallback_units;
    }
    let elapsed_units = elapsed_us / unit_us;
    let elapsed_units = elapsed_units.min(usize::MAX as u128) as usize;
    elapsed_units.max(min_units)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PublicationBudgetGrants {
    adaptive_enabled: bool,
    feature: PublicationGrant,
    light: PublicationGrant,
    pending_worldgen_publication_chunk_limit: usize,
    feature_estimated_unit_us: Option<u128>,
    light_estimated_unit_us: Option<u128>,
}

impl ChunkSchedulerPublicationDiagnostics {
    fn record_budget_grants(&mut self, grants: PublicationBudgetGrants) {
        self.adaptive_budget_enabled = grants.adaptive_enabled;
        self.feature_publish_budget_min_units = grants.feature.min_units;
        self.feature_publish_budget_max_units = grants.feature.max_units;
        self.feature_publish_budget_elapsed_us = grants.feature.elapsed_us;
        self.feature_publish_estimated_unit_us = grants.feature_estimated_unit_us;
        self.light_publish_budget_min_units = grants.light.min_units;
        self.light_publish_budget_max_units = grants.light.max_units;
        self.light_publish_budget_elapsed_us = grants.light.elapsed_us;
        self.light_publish_estimated_unit_us = grants.light_estimated_unit_us;
        self.pending_worldgen_publication_chunk_limit =
            grants.pending_worldgen_publication_chunk_limit;
    }
}

impl ChunkScheduler {
    pub fn new(seed: i64) -> Self {
        Self::with_persistence(seed, PersistenceMailbox::transient())
    }

    pub fn with_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self::with_world_store(seed, Box::new(ChunkSnapshotWorldStore::new(store)))
    }

    pub fn with_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::with_persistence(seed, PersistenceMailbox::new(store))
    }

    pub fn with_external_load_world_store(seed: i64, store: Box<dyn WorldStore>) -> Self {
        Self::with_persistence(seed, PersistenceMailbox::external_loads(store))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn with_world_store_and_wasm_job_workers(
        seed: i64,
        store: Box<dyn WorldStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        let mut scheduler = Self::with_world_store(seed, store);
        scheduler.worldgen_mailbox = WorldgenMailbox::with_wasm_job_worker(config.clone());
        scheduler.light_mailbox = LightStatusMailbox::with_wasm_job_worker(config);
        scheduler
    }

    #[cfg(target_arch = "wasm32")]
    pub fn with_external_load_world_store_and_wasm_job_workers(
        seed: i64,
        store: Box<dyn WorldStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        let mut scheduler = Self::with_external_load_world_store(seed, store);
        scheduler.worldgen_mailbox = WorldgenMailbox::with_wasm_job_worker(config.clone());
        scheduler.light_mailbox = LightStatusMailbox::with_wasm_job_worker(config);
        scheduler
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_with_threaded_world_store(
        seed: i64,
        store: Box<dyn WorldStore + Send>,
    ) -> ChunkStoreResult<Self> {
        Ok(Self::with_persistence(
            seed,
            PersistenceMailbox::threaded(store)?,
        ))
    }

    pub fn with_persistence(seed: i64, store: PersistenceMailbox) -> Self {
        Self {
            seed,
            lighting_enabled: true,
            light_status_batch_size: DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            holders: BTreeMap::new(),
            pending_unloads: BTreeSet::new(),
            pending_chunk_loads: BTreeMap::new(),
            pending_chunk_load_by_pos: BTreeMap::new(),
            stored_chunk_misses: BTreeSet::new(),
            pending_chunk_saves: BTreeMap::new(),
            pending_chunk_save_by_pos: BTreeMap::new(),
            pending_entity_chunk_loads: BTreeMap::new(),
            pending_entity_chunk_load_by_pos: BTreeMap::new(),
            loaded_entity_chunks: BTreeSet::new(),
            pending_entity_chunk_saves: BTreeMap::new(),
            pending_entity_chunk_save_by_pos: BTreeMap::new(),
            entity_unload_saves: BTreeSet::new(),
            distance_manager: ChunkDistanceManager::new(),
            jobs: BTreeMap::new(),
            job_timings: BTreeMap::new(),
            worldgen_mailbox: WorldgenMailbox::new(),
            pending_worldgen_publications: VecDeque::new(),
            publication_budget: ChunkPublicationBudgetState::default(),
            light_mailbox: LightStatusMailbox::new(),
            pending_light_status_batches: BTreeMap::new(),
            pending_light_publications: VecDeque::new(),
            completed_light_statuses: 0,
            completed_light_batches: 0,
            total_light_status_compute_us: 0,
            max_light_status_compute_us: 0,
            total_light_status_publication_us: 0,
            max_light_status_publication_us: 0,
            total_light_status_publication_units: 0,
            light_status_timing: LevelLightComputationTiming::default(),
            pending_block_deltas: BTreeMap::new(),
            pending_runtime_light_snapshots: VecDeque::new(),
            dirty_chunks: BTreeSet::new(),
            next_job_id: 1,
            next_revision: 1,
            store,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn with_wasm_job_workers(
        seed: i64,
        store: Box<dyn ChunkSnapshotStore>,
        config: WasmServerJobWorkerConfig,
    ) -> Self {
        Self {
            seed,
            lighting_enabled: true,
            light_status_batch_size: DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            holders: BTreeMap::new(),
            pending_unloads: BTreeSet::new(),
            pending_chunk_loads: BTreeMap::new(),
            pending_chunk_load_by_pos: BTreeMap::new(),
            stored_chunk_misses: BTreeSet::new(),
            pending_chunk_saves: BTreeMap::new(),
            pending_chunk_save_by_pos: BTreeMap::new(),
            pending_entity_chunk_loads: BTreeMap::new(),
            pending_entity_chunk_load_by_pos: BTreeMap::new(),
            loaded_entity_chunks: BTreeSet::new(),
            pending_entity_chunk_saves: BTreeMap::new(),
            pending_entity_chunk_save_by_pos: BTreeMap::new(),
            entity_unload_saves: BTreeSet::new(),
            distance_manager: ChunkDistanceManager::new(),
            jobs: BTreeMap::new(),
            job_timings: BTreeMap::new(),
            worldgen_mailbox: WorldgenMailbox::with_wasm_job_worker(config.clone()),
            pending_worldgen_publications: VecDeque::new(),
            publication_budget: ChunkPublicationBudgetState::default(),
            light_mailbox: LightStatusMailbox::with_wasm_job_worker(config),
            pending_light_status_batches: BTreeMap::new(),
            pending_light_publications: VecDeque::new(),
            completed_light_statuses: 0,
            completed_light_batches: 0,
            total_light_status_compute_us: 0,
            max_light_status_compute_us: 0,
            total_light_status_publication_us: 0,
            max_light_status_publication_us: 0,
            total_light_status_publication_units: 0,
            light_status_timing: LevelLightComputationTiming::default(),
            pending_block_deltas: BTreeMap::new(),
            pending_runtime_light_snapshots: VecDeque::new(),
            dirty_chunks: BTreeSet::new(),
            next_job_id: 1,
            next_revision: 1,
            store: PersistenceMailbox::new(Box::new(ChunkSnapshotWorldStore::new(store))),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn lighting_enabled(&self) -> bool {
        self.lighting_enabled
    }

    pub fn set_lighting_enabled(&mut self, enabled: bool) {
        self.lighting_enabled = enabled;
    }

    pub const fn light_status_batch_size(&self) -> usize {
        self.light_status_batch_size
    }

    pub fn set_light_status_batch_size(&mut self, batch_size: usize) {
        self.light_status_batch_size = batch_size.max(1);
    }

    pub const fn publication_budget_config(&self) -> ChunkPublicationBudgetConfig {
        self.publication_budget.config
    }

    pub fn set_publication_budget_config(&mut self, config: ChunkPublicationBudgetConfig) {
        self.publication_budget.set_config(config);
    }

    pub fn set_publication_budget_gameplay_rate_hz(&mut self, gameplay_rate_hz: u32) {
        self.publication_budget
            .set_gameplay_rate_hz(gameplay_rate_hz);
    }

    pub fn apply_interest(
        &mut self,
        view: ChunkView,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.apply_player_ticket_positions_with_priority(
            chunk_positions_for_view(&view),
            vec![view.center],
        )
    }

    pub(crate) fn apply_player_ticket_positions_with_priority(
        &mut self,
        positions: BTreeSet<ChunkPos>,
        priority_centers: Vec<ChunkPos>,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.distance_manager
            .set_aggregate_player_ticket_positions_with_priority(positions, priority_centers);
        self.reconcile_ticketed_holders()
    }

    pub fn poll(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        Ok(self.poll_with_publication_diagnostics()?.0)
    }

    fn poll_with_publication_diagnostics(
        &mut self,
    ) -> ChunkStoreResult<(
        Vec<ChunkSchedulerEvent>,
        ChunkSchedulerPublicationDiagnostics,
    )> {
        let mut publication = ChunkSchedulerPublicationDiagnostics::default();
        let grants = self.publication_budget_grants(&mut publication);
        self.store.process_one_background_write();
        let mut events = self.poll_persistence()?;
        events.extend(self.publish_completed_worldgen_jobs(&mut publication, grants.feature)?);
        events.extend(self.publish_pending_light_statuses(&mut publication, grants.light)?);
        self.store.process_one_background_write();
        events.extend(self.poll_persistence()?);
        publication.pending_worldgen_publication_jobs = self.pending_worldgen_publications.len();
        publication.pending_worldgen_publication_chunks =
            self.pending_worldgen_publication_target_count();
        publication.pending_light_publications = self.pending_light_publications.len();
        Ok((events, publication))
    }

    fn publication_budget_grants(
        &mut self,
        diagnostics: &mut ChunkSchedulerPublicationDiagnostics,
    ) -> PublicationBudgetGrants {
        if !self.publication_budget.enabled() {
            let grants = PublicationBudgetGrants {
                adaptive_enabled: false,
                feature: PublicationGrant::fixed(DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET),
                light: PublicationGrant::fixed(DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET),
                pending_worldgen_publication_chunk_limit: usize::MAX,
                feature_estimated_unit_us: None,
                light_estimated_unit_us: None,
            };
            diagnostics.record_budget_grants(grants);
            return grants;
        }

        let target_period_ms = self
            .publication_budget
            .config
            .target_period_ms()
            .unwrap_or(50.0);
        let queue_depth = (self.pending_worldgen_publication_target_count()
            + self.pending_light_publications.len()) as u64;
        let input = BudgetControllerInput::new(target_period_ms)
            .with_window(
                BudgetTelemetryWindow::new(
                    FrameHostKind::IntegratedServerRunner,
                    WorkWindow::GameplayTick,
                )
                .with_app_work_p95_ms(0.0)
                .with_headroom_p05_ms(target_period_ms)
                .with_queue_age_ms(queue_depth, 0.0),
            )
            .with_costs(self.publication_budget.estimator.estimates);
        let panel = self.publication_budget.controller.decide(&input);
        diagnostics.budget_decision_panel = panel.clone();
        let feature_decision =
            decision_for(&panel.decisions, BudgetDecisionFamily::FeaturePublication);
        let light_decision = decision_for(&panel.decisions, BudgetDecisionFamily::LightPublication);
        let admission_decision =
            decision_for(&panel.decisions, BudgetDecisionFamily::FeatureJobAdmission);
        let pending_worldgen_publication_chunk_limit = admission_decision
            .and_then(|decision| decision.grant.max_pending_units)
            .map(|units| units as usize)
            .unwrap_or(DEFAULT_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT)
            .max(1);
        self.publication_budget
            .last_pending_worldgen_publication_chunk_limit =
            pending_worldgen_publication_chunk_limit;
        let feature_estimated_unit_us = self
            .publication_budget
            .estimator
            .estimates
            .feature_publish_ms
            .map(elapsed_ms_to_us);
        let light_estimated_unit_us = self
            .publication_budget
            .estimator
            .estimates
            .light_publish_ms
            .map(elapsed_ms_to_us);
        let grants = PublicationBudgetGrants {
            adaptive_enabled: true,
            feature: PublicationGrant::from_decision(
                feature_decision,
                DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET,
                feature_estimated_unit_us,
            ),
            light: PublicationGrant::from_decision(
                light_decision,
                DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET,
                light_estimated_unit_us,
            ),
            pending_worldgen_publication_chunk_limit,
            feature_estimated_unit_us,
            light_estimated_unit_us,
        };
        diagnostics.record_budget_grants(grants);
        grants
    }

    pub fn wait_for_worldgen_completion(&mut self, timeout: Duration) -> bool {
        self.worldgen_mailbox.wait_for_completed(timeout)
    }

    pub fn wait_for_light_completion(&mut self, timeout: Duration) -> bool {
        self.light_mailbox.wait_for_completed(timeout)
    }

    pub fn tick(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        Ok(self.tick_report()?.events)
    }

    pub fn tick_report(&mut self) -> ChunkStoreResult<ChunkSchedulerTickReport> {
        self.tick_report_with_record_builder(|snapshot| {
            ChunkRecord::from_snapshot(snapshot.clone())
        })
    }

    pub(crate) fn tick_report_with_record_builder(
        &mut self,
        mut record_builder: impl FnMut(&ChunkSnapshot) -> ChunkRecord,
    ) -> ChunkStoreResult<ChunkSchedulerTickReport> {
        self.tick_report_with_record_builders(&mut record_builder, &mut |_, _| None)
    }

    pub(crate) fn tick_report_with_record_builders(
        &mut self,
        record_builder: &mut impl FnMut(&ChunkSnapshot) -> ChunkRecord,
        entity_record_builder: &mut impl FnMut(ChunkPos, u64) -> Option<EntityChunkRecord>,
    ) -> ChunkStoreResult<ChunkSchedulerTickReport> {
        let total_start = simulation_timing_start();
        let purge_start = simulation_timing_start();
        self.distance_manager.purge_stale_tickets();
        let purge_stale_tickets_us = simulation_timing_elapsed_us(purge_start);

        let reconcile_start = simulation_timing_start();
        let mut events = self.reconcile_ticketed_holders()?;
        let reconcile_holders_us = simulation_timing_elapsed_us(reconcile_start);

        let publish_start = simulation_timing_start();
        let (publish_events, publication) = self.poll_with_publication_diagnostics()?;
        events.extend(publish_events);
        let publish_completed_us = simulation_timing_elapsed_us(publish_start);

        let pending_unload_start = simulation_timing_start();
        let (pending_unloads_processed, unload_events) = self
            .process_pending_unloads_with_events_and_record_builder(
                DEFAULT_PENDING_UNLOAD_BUDGET,
                record_builder,
                entity_record_builder,
            )?;
        events.extend(unload_events);
        let pending_unload_us = simulation_timing_elapsed_us(pending_unload_start);
        Ok(ChunkSchedulerTickReport {
            ticket_tick: self.ticket_tick(),
            block_ticking_chunks: self.block_ticking_chunks(),
            entity_ticking_chunks: self.entity_ticking_chunks(),
            pending_unloads_processed,
            events,
            publication,
            timing: ChunkSchedulerTickTiming {
                total_us: simulation_timing_elapsed_us(total_start),
                purge_stale_tickets_us,
                reconcile_holders_us,
                publish_completed_us,
                pending_unload_us,
            },
        })
    }

    pub fn add_region_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        distance: i32,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.distance_manager.add_region_ticket(
            ticket_type,
            pos,
            distance,
            ChunkTicketKey::Chunk(pos),
        );
        self.reconcile_ticketed_holders()
    }

    pub fn remove_region_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        distance: i32,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.distance_manager.remove_region_ticket(
            ticket_type,
            pos,
            distance,
            ChunkTicketKey::Chunk(pos),
        );
        self.reconcile_ticketed_holders()
    }

    pub fn set_chunk_forced(
        &mut self,
        pos: ChunkPos,
        forced: bool,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        if forced {
            self.distance_manager.add_ticket(
                ChunkTicketType::Forced,
                pos,
                FORCED_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        } else {
            self.distance_manager.remove_ticket(
                ChunkTicketType::Forced,
                pos,
                FORCED_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        }
        self.reconcile_ticketed_holders()
    }

    pub fn holder(&self, pos: ChunkPos) -> Option<&ChunkHolder> {
        self.holders.get(&pos)
    }

    pub(crate) fn view_readiness_snapshot(&self, view: &ChunkView) -> ChunkLoadingProgressSnapshot {
        let target_status = self.runtime_chunk_target_status();
        let radius = i32::try_from(view.chunk_tracking_radius)
            .expect("chunk tracking radius must fit into i32");
        let target_chunk_count = {
            let side = radius as usize * 2 + 1;
            side * side
        };
        let mut target_ready_chunks = 0;
        let playable_gate_ready_chunks = playable_gate_ready_chunk_count(
            |pos| {
                self.holders
                    .get(&pos)
                    .and_then(ChunkHolder::highest_ready_status)
            },
            view.center,
            target_status,
        );
        let playable_gate_chunk_count = {
            let side = PLAYABLE_GATE_RADIUS as usize * 2 + 1;
            side * side
        };
        let playable_chunk_ready = playable_gate_ready_chunks == playable_gate_chunk_count;
        let mut cells = Vec::with_capacity(target_chunk_count);

        for relative_z in -radius..=radius {
            for relative_x in -radius..=radius {
                let pos = ChunkPos::new(view.center.x + relative_x, view.center.z + relative_z);
                let status = self
                    .holders
                    .get(&pos)
                    .and_then(ChunkHolder::highest_ready_status);
                let target_ready = status.is_some_and(|status| status >= target_status);
                let playable = pos == view.center;
                if target_ready {
                    target_ready_chunks += 1;
                }
                cells.push(ChunkLoadingProgressCell {
                    relative_x,
                    relative_z,
                    status,
                    target_ready,
                    playable,
                });
            }
        }

        ChunkLoadingProgressSnapshot {
            stats: ChunkLoadingProgressStats {
                center: view.center,
                target_radius: view.chunk_tracking_radius,
                target_status,
                target_chunk_count,
                target_ready_chunks,
                playable_chunk: view.center,
                playable_gate_radius: PLAYABLE_GATE_RADIUS,
                playable_gate_chunk_count,
                playable_gate_ready_chunks,
                playable_chunk_ready,
            },
            cells,
        }
    }

    pub(crate) fn client_visible_snapshot(&self, pos: ChunkPos) -> Option<ChunkSnapshot> {
        let holder = self.holders.get(&pos)?;
        if !holder.client_visible {
            return None;
        }
        let snapshot = holder.published_snapshot.as_ref()?;
        self.snapshot_is_client_ready(snapshot)
            .then(|| snapshot.clone())
    }

    pub fn holder_count(&self) -> usize {
        self.holders.len()
    }

    pub fn pending_unload_count(&self) -> usize {
        self.pending_unloads.len()
    }

    pub fn pending_persistence_load_count(&self) -> usize {
        self.pending_chunk_loads.len() + self.pending_entity_chunk_loads.len()
    }

    pub fn pending_persistence_save_count(&self) -> usize {
        self.pending_chunk_saves.len() + self.pending_entity_chunk_saves.len()
    }

    pub fn pending_external_persistence_request_count(&self) -> usize {
        self.store.pending_external_request_count()
    }

    pub fn drain_external_persistence_requests(&mut self) -> Vec<WorldStoreRequest> {
        self.store.drain_external_requests()
    }

    pub fn complete_external_persistence_request(
        &mut self,
        completion: WorldStoreCompletion,
    ) -> ChunkStoreResult<()> {
        self.store.complete_external_request(completion)
    }

    pub fn entity_chunks_supported(&self) -> bool {
        self.store.entity_chunks_supported()
    }

    pub fn is_pending_unload(&self, pos: ChunkPos) -> bool {
        self.pending_unloads.contains(&pos)
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.holders
            .values()
            .filter(|holder| holder.published_snapshot.is_some())
            .count()
    }

    pub fn client_visible_chunk_count(&self) -> usize {
        self.holders
            .values()
            .filter(|holder| holder.client_visible)
            .count()
    }

    pub fn full_status_chunk_count(&self, status: FullChunkStatus) -> usize {
        self.holders
            .values()
            .filter(|holder| holder.ticket_level <= MAX_CHUNK_DISTANCE)
            .filter(|holder| holder.full_status() == status)
            .count()
    }

    pub fn block_ticking_chunk_count(&self) -> usize {
        self.block_ticking_chunks().len()
    }

    pub fn entity_ticking_chunk_count(&self) -> usize {
        self.entity_ticking_chunks().len()
    }

    pub fn block_ticking_chunks(&self) -> Vec<ChunkPos> {
        self.full_status_chunks_at_or_after(FullChunkStatus::Ticking)
    }

    pub fn entity_ticking_chunks(&self) -> Vec<ChunkPos> {
        self.full_status_chunks_at_or_after(FullChunkStatus::EntityTicking)
    }

    pub fn dependency_holder_count(&self) -> usize {
        self.holders
            .values()
            .filter(|holder| {
                holder
                    .target_status
                    .is_some_and(|status| status <= ChunkStatus::Surface)
                    && holder.published_snapshot.is_none()
            })
            .count()
    }

    pub fn ready_dependency_chunk_count(&self) -> usize {
        self.holders
            .values()
            .filter(|holder| holder.dependency_buffer.is_some())
            .count()
    }

    pub fn dirty_chunk_count(&self) -> usize {
        self.dirty_chunks.len()
    }

    pub fn ticket_tick(&self) -> u64 {
        self.distance_manager.ticket_tick()
    }

    pub fn ticketed_chunk_count(&self) -> usize {
        self.distance_manager.ticketed_chunk_count()
    }

    pub fn active_ticketed_chunk_count(&self) -> usize {
        self.distance_manager.active_levels().len()
    }

    pub fn ticket_count_at(&self, pos: ChunkPos) -> usize {
        self.distance_manager.ticket_count_at(pos)
    }

    pub fn ticket_level_at(&self, pos: ChunkPos) -> i32 {
        self.distance_manager.ticket_level_at(pos)
    }

    pub fn active_ticket_level_at(&self, pos: ChunkPos) -> i32 {
        self.distance_manager.active_level_at(pos)
    }

    pub fn job(&self, id: ChunkJobId) -> Option<&ChunkStatusJob> {
        self.jobs.get(&id)
    }

    pub fn jobs(&self) -> impl Iterator<Item = &ChunkStatusJob> {
        self.jobs.values()
    }

    pub fn job_timing(&self, id: ChunkJobId) -> Option<OverworldFeatureBatchTiming> {
        self.job_timings.get(&id).copied()
    }

    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }

    pub fn pending_job_count(&self) -> usize {
        let pending_status_jobs = self
            .jobs
            .values()
            .filter(|job| matches!(job.state, ChunkJobState::Queued | ChunkJobState::Running))
            .count();
        pending_status_jobs
            + self
                .pending_light_status_batches
                .values()
                .map(Vec::len)
                .sum::<usize>()
            + self.light_mailbox.pending_count()
            + self.pending_light_publications.len()
    }

    pub fn pending_publication_count(&self) -> usize {
        self.pending_worldgen_publication_target_count() + self.pending_light_publications.len()
    }

    fn pending_worldgen_publication_target_count(&self) -> usize {
        self.pending_worldgen_publications
            .iter()
            .map(|publication| {
                self.jobs
                    .get(&publication.completed.job_id)
                    .map_or(0, |job| {
                        job.target_chunks
                            .len()
                            .saturating_sub(publication.next_target_index)
                    })
            })
            .sum::<usize>()
    }

    pub fn worldgen_mailbox_kind(&self) -> WorldgenMailboxKind {
        self.worldgen_mailbox.kind()
    }

    pub fn light_status_mailbox_kind(&self) -> LightStatusMailboxKind {
        self.light_mailbox.kind()
    }

    pub fn worldgen_mailbox_pending_count(&self) -> usize {
        self.worldgen_mailbox.pending_count()
    }

    pub fn light_status_mailbox_pending_count(&self) -> usize {
        self.light_mailbox.pending_count()
    }

    pub fn worldgen_mailbox_frame_metrics(&self) -> WorkerFrameMetrics {
        self.worldgen_mailbox.frame_metrics()
    }

    pub fn light_status_mailbox_frame_metrics(&self) -> WorkerFrameMetrics {
        self.light_mailbox.frame_metrics()
    }

    pub fn light_status_mailbox_metrics(&self) -> LightStatusMailboxMetrics {
        self.light_mailbox.mailbox_metrics()
    }

    pub fn metrics(&self) -> ChunkSchedulerMetrics {
        let latest_feature_job = self.jobs.values().max_by_key(|job| job.id);
        ChunkSchedulerMetrics {
            direct_ticket_chunks: self.ticketed_chunk_count(),
            active_ticket_chunks: self.active_ticketed_chunk_count(),
            holder_chunks: self.holder_count(),
            pending_unload_chunks: self.pending_unload_count(),
            inaccessible_status_chunks: self.full_status_chunk_count(FullChunkStatus::Inaccessible),
            border_status_chunks: self.full_status_chunk_count(FullChunkStatus::Border),
            ticking_status_chunks: self.full_status_chunk_count(FullChunkStatus::Ticking),
            entity_ticking_status_chunks: self
                .full_status_chunk_count(FullChunkStatus::EntityTicking),
            block_ticking_chunks: self.block_ticking_chunk_count(),
            client_visible_chunks: self.client_visible_chunk_count(),
            loaded_snapshot_chunks: self.loaded_chunk_count(),
            dependency_holder_chunks: self.dependency_holder_count(),
            ready_dependency_chunks: self.ready_dependency_chunk_count(),
            dirty_chunks: self.dirty_chunk_count(),
            pending_jobs: self.pending_job_count(),
            completed_jobs: self
                .jobs
                .values()
                .filter(|job| job.state == ChunkJobState::Complete)
                .count(),
            total_seeded_dependency_chunks: self
                .jobs
                .values()
                .map(|job| job.seeded_dependency_chunks)
                .sum(),
            total_dependency_cache_hits: self
                .jobs
                .values()
                .map(|job| job.dependency_cache_hits)
                .sum(),
            total_dependency_cache_misses: self
                .jobs
                .values()
                .map(|job| job.dependency_cache_misses)
                .sum(),
            total_retained_dependency_chunks: self
                .jobs
                .values()
                .map(|job| job.retained_dependency_chunks)
                .sum(),
            max_feature_job_target_chunks: self
                .jobs
                .values()
                .map(|job| job.target_chunks.len())
                .max()
                .unwrap_or(0),
            max_feature_job_feature_centers: self
                .jobs
                .values()
                .map(|job| job.feature_centers.len())
                .max()
                .unwrap_or(0),
            max_feature_job_dependency_chunks: self
                .jobs
                .values()
                .map(|job| job.dependency_chunks.len())
                .max()
                .unwrap_or(0),
            latest_feature_job_id: latest_feature_job.map(|job| job.id),
            latest_feature_job_target_chunks: latest_feature_job
                .map_or(0, |job| job.target_chunks.len()),
            latest_feature_job_feature_centers: latest_feature_job
                .map_or(0, |job| job.feature_centers.len()),
            latest_feature_job_dependency_chunks: latest_feature_job
                .map_or(0, |job| job.dependency_chunks.len()),
            latest_feature_job_first_target: latest_feature_job
                .and_then(|job| job.target_chunks.first().copied()),
            completed_light_statuses: self.completed_light_statuses,
            completed_light_batches: self.completed_light_batches,
            total_light_status_compute_us: self.total_light_status_compute_us,
            max_light_status_compute_us: self.max_light_status_compute_us,
            total_light_status_world_init_us: self.light_status_timing.world_init_us,
            total_light_status_active_sections_us: self.light_status_timing.active_sections_us,
            total_light_status_sky_source_scan_us: self.light_status_timing.sky_source_scan_us,
            total_light_status_block_source_scan_us: self.light_status_timing.block_source_scan_us,
            total_light_status_engine_init_us: self.light_status_timing.engine_init_us,
            total_light_status_section_setup_us: self.light_status_timing.section_setup_us,
            total_light_status_section_status_update_us: self
                .light_status_timing
                .section_status_update_us,
            total_light_status_sky_column_enable_us: self.light_status_timing.sky_column_enable_us,
            total_light_status_sky_source_enqueue_us: self
                .light_status_timing
                .sky_source_enqueue_us,
            total_light_status_block_source_enqueue_us: self
                .light_status_timing
                .block_source_enqueue_us,
            total_light_status_input_chunks: self.light_status_timing.light_status_input_chunks,
            total_light_status_inserted_chunks: self
                .light_status_timing
                .light_status_inserted_chunks,
            total_light_status_replaced_chunks: self
                .light_status_timing
                .light_status_replaced_chunks,
            total_light_status_unchanged_chunks: self
                .light_status_timing
                .light_status_unchanged_chunks,
            total_light_status_changed_block_raw_checks: self
                .light_status_timing
                .changed_block_raw_checks,
            total_light_status_changed_block_light_property_changes: self
                .light_status_timing
                .changed_block_light_property_changes,
            total_light_status_changed_block_opacity_changes: self
                .light_status_timing
                .changed_block_opacity_changes,
            total_light_status_changed_block_emission_changes: self
                .light_status_timing
                .changed_block_emission_changes,
            total_light_status_changed_block_raw_only_changes: self
                .light_status_timing
                .changed_block_raw_only_changes,
            total_light_status_changed_block_check_us: self
                .light_status_timing
                .changed_block_check_us,
            total_light_status_run_updates_us: self.light_status_timing.run_updates_us,
            total_light_status_run_update_iterations: self
                .light_status_timing
                .run_update_iterations,
            total_light_status_block_run_update_calls: self
                .light_status_timing
                .block_run_update_calls,
            total_light_status_sky_run_update_calls: self.light_status_timing.sky_run_update_calls,
            total_light_status_block_run_update_processed_nodes: self
                .light_status_timing
                .block_run_update_processed_nodes,
            total_light_status_sky_run_update_processed_nodes: self
                .light_status_timing
                .sky_run_update_processed_nodes,
            max_light_status_block_run_update_queue_before: self
                .light_status_timing
                .max_block_run_update_queue_before,
            max_light_status_sky_run_update_queue_before: self
                .light_status_timing
                .max_sky_run_update_queue_before,
            final_light_status_block_run_update_queue_after: self
                .light_status_timing
                .final_block_run_update_queue_after,
            final_light_status_sky_run_update_queue_after: self
                .light_status_timing
                .final_sky_run_update_queue_after,
            total_light_status_block_run_updates_us: self.light_status_timing.block_run_updates_us,
            total_light_status_sky_run_updates_us: self.light_status_timing.sky_run_updates_us,
            total_light_status_sky_source_update_count: self
                .light_status_timing
                .sky_source_update_count,
            total_light_status_sky_source_updates_us: self
                .light_status_timing
                .sky_source_updates_us,
            total_light_status_block_run_update_graph_us: self
                .light_status_timing
                .block_run_update_graph_us,
            total_light_status_sky_run_update_graph_us: self
                .light_status_timing
                .sky_run_update_graph_us,
            total_light_status_block_run_update_storage_swap_us: self
                .light_status_timing
                .block_run_update_storage_swap_us,
            total_light_status_sky_run_update_storage_swap_us: self
                .light_status_timing
                .sky_run_update_storage_swap_us,
            total_light_status_block_run_update_affected_sections: self
                .light_status_timing
                .block_run_update_affected_sections,
            total_light_status_sky_run_update_affected_sections: self
                .light_status_timing
                .sky_run_update_affected_sections,
            total_light_status_collect_sections_us: self.light_status_timing.collect_sections_us,
            total_light_status_publication_us: self.total_light_status_publication_us,
            max_light_status_publication_us: self.max_light_status_publication_us,
            total_light_status_publication_units: self.total_light_status_publication_units,
        }
    }

    pub(crate) fn block_at_world(&self, pos: WorldBlockPos) -> Option<RawBlockId> {
        let chunk_pos = pos.chunk_pos();
        let local_x = local_block_coord(pos.x);
        let local_z = local_block_coord(pos.z);
        self.holders
            .get(&chunk_pos)
            .and_then(|holder| holder.live_blocks.as_ref())
            .and_then(|buffer| {
                if pos.y < buffer.min_y || pos.y >= buffer.min_y + buffer.height {
                    None
                } else {
                    Some(buffer.get_block_at_y(local_x, pos.y, local_z))
                }
            })
    }

    pub(crate) fn raw_brightness_at_world(&self, pos: WorldBlockPos, sky_darken: u8) -> Option<u8> {
        let chunk_pos = pos.chunk_pos();
        let local_x = local_block_coord(pos.x);
        let local_z = local_block_coord(pos.z);
        self.holders
            .get(&chunk_pos)
            .and_then(|holder| holder.published_snapshot.as_ref())
            .and_then(|snapshot| {
                if snapshot.status < ChunkStatus::Light
                    || !snapshot.light_correct
                    || snapshot.light_sections.is_empty()
                    || pos.y < snapshot.min_y
                    || pos.y >= snapshot.min_y + snapshot.height
                {
                    return None;
                }

                let index = chunk_section_index(local_x, local_section_block_coord(pos.y), local_z);
                let section_y = block_to_section_coord(pos.y);
                let block = block_light_at_strict(&snapshot.light_sections, section_y, index);
                let sky = sky_light_at_strict(&snapshot.light_sections, section_y, index);
                Some(block.max(sky.saturating_sub(sky_darken)))
            })
    }

    #[cfg(feature = "physics")]
    pub fn physics_terrain_section(
        &self,
        pos: ChunkPos,
        section_y: i32,
    ) -> Option<mclone_physics::PhysicsTerrainSection> {
        self.holders
            .get(&pos)
            .and_then(|holder| holder.live_blocks.as_ref())
            .and_then(|live_blocks| {
                crate::physics_terrain::physics_terrain_section_from_live_blocks(
                    live_blocks,
                    section_y,
                )
            })
    }

    #[cfg(feature = "physics")]
    pub fn physics_terrain_section_at_block(
        &self,
        pos: WorldBlockPos,
    ) -> Option<mclone_physics::PhysicsTerrainSection> {
        self.physics_terrain_section(pos.chunk_pos(), block_to_section_coord(pos.y))
    }

    pub(crate) fn set_block_at_world(&mut self, pos: WorldBlockPos, block_id: RawBlockId) -> bool {
        let chunk_pos = pos.chunk_pos();
        let local_x = local_block_coord(pos.x);
        let local_z = local_block_coord(pos.z);
        let local_y = local_section_block_coord(pos.y);
        let section_y = block_to_section_coord(pos.y);
        let block_state = generated_block_state_id(block_id);
        let record_delta;
        {
            let Some(holder) = self.holders.get_mut(&chunk_pos) else {
                return false;
            };
            let Some(live_blocks) = holder.live_blocks.as_mut() else {
                return false;
            };
            if pos.y < live_blocks.min_y || pos.y >= live_blocks.min_y + live_blocks.height {
                return false;
            }
            if live_blocks.get_block_at_y(local_x, pos.y, local_z) == block_id {
                return false;
            }

            live_blocks.set_block_at_y(local_x, pos.y, local_z, block_id);
            let revision = ChunkRevision(self.next_revision);
            self.next_revision = self.next_revision.saturating_add(1);
            let published_status = if let Some(snapshot) = holder.published_snapshot.as_mut() {
                snapshot.revision = revision;
                snapshot.patch_section_block(section_y, local_x, local_y, local_z, block_state);
                Some(snapshot.status)
            } else {
                None
            };
            if let Some(status) = published_status {
                holder.mark_ready(status, Some(revision));
            }
            holder.dirty = true;
            record_delta = holder.client_visible;
        }
        self.dirty_chunks.insert(chunk_pos);
        if record_delta {
            self.record_section_block_delta(
                chunk_pos,
                section_y,
                local_x,
                local_y,
                local_z,
                block_state,
            );
        }

        true
    }

    pub(crate) fn refresh_runtime_lighting_after_block_change(
        &mut self,
        pos: WorldBlockPos,
        old_block: RawBlockId,
        new_block: RawBlockId,
    ) -> bool {
        if !self.lighting_enabled || !block_change_affects_light(old_block, new_block) {
            return false;
        }

        let chunk_pos = pos.chunk_pos();
        let Some(status) = self.runtime_light_status_for_chunk(chunk_pos) else {
            return false;
        };

        let mut light_state = RetainedInitialLightState::new();
        let mut completed = light_state.compute_batch(PendingLightStatusBatch::new(vec![status]));
        let Some((_status, light_sections, _timing)) = completed.pop() else {
            return false;
        };

        self.publish_runtime_light_sections(chunk_pos, light_sections)
    }

    fn runtime_light_status_for_chunk(&self, target_pos: ChunkPos) -> Option<PendingLightStatus> {
        let holder = self.holders.get(&target_pos)?;
        let snapshot = holder.published_snapshot.as_ref()?;
        if snapshot.status < ChunkStatus::Light || !snapshot.light_correct {
            return None;
        }
        let raw_blocks = holder.live_blocks.as_ref()?;
        if raw_blocks.min_y != snapshot.min_y || raw_blocks.height != snapshot.height {
            return None;
        }

        let neighbor_blocks = self
            .holders
            .iter()
            .filter_map(|(neighbor_pos, neighbor)| {
                if *neighbor_pos == target_pos
                    || !provisional_sky_light_includes_chunk(target_pos, *neighbor_pos)
                {
                    return None;
                }
                let blocks = neighbor
                    .live_blocks
                    .as_ref()
                    .or(neighbor.dependency_buffer.as_ref())?;
                if blocks.min_y != snapshot.min_y || blocks.height != snapshot.height {
                    return None;
                }
                Some((*neighbor_pos, blocks.blocks.clone()))
            })
            .collect();

        Some(PendingLightStatus::from_parts(
            target_pos,
            snapshot.clone(),
            raw_blocks.blocks.clone(),
            neighbor_blocks,
        ))
    }

    fn publish_runtime_light_sections(
        &mut self,
        pos: ChunkPos,
        light_sections: Vec<PackedLightSection>,
    ) -> bool {
        let revision = ChunkRevision(self.next_revision);
        self.next_revision = self.next_revision.saturating_add(1);

        let Some(holder) = self.holders.get_mut(&pos) else {
            return false;
        };
        let Some(snapshot) = holder.published_snapshot.as_mut() else {
            return false;
        };
        if snapshot.status < ChunkStatus::Light {
            return false;
        }

        let status = snapshot.status;
        let mut updated = snapshot.clone().with_light_sections(true, light_sections);
        updated.revision = revision;
        *snapshot = updated.clone();
        holder.mark_ready(status, Some(revision));
        holder.dirty = true;
        self.dirty_chunks.insert(pos);
        if holder.client_visible {
            self.pending_runtime_light_snapshots.push_back(updated);
        }
        true
    }

    pub(crate) fn fluid_tick_requests_after_block_change(
        &self,
        pos: WorldBlockPos,
        block_id: RawBlockId,
    ) -> Vec<ScheduledFluidTickRequest> {
        let mut requests = Vec::new();
        if let Some(fluid) = FluidKind::from_block_id(block_id) {
            record_scheduled_fluid_tick_request(&mut requests, pos, fluid);
        }

        for direction in ALL_FLUID_DIRECTIONS {
            let neighbor = offset_pos(pos, direction);
            let Some(neighbor_block) = self.block_at_world(neighbor) else {
                continue;
            };
            if let Some(fluid) = FluidKind::from_block_id(neighbor_block) {
                record_scheduled_fluid_tick_request(&mut requests, neighbor, fluid);
            }
        }
        requests
    }

    fn record_section_block_delta(
        &mut self,
        pos: ChunkPos,
        section_y: i32,
        local_x: i32,
        local_y: i32,
        local_z: i32,
        block_state: BlockStateId,
    ) {
        let local_index = chunk_section_index(local_x, local_y, local_z);
        self.pending_block_deltas
            .entry((pos, section_y))
            .or_default()
            .insert(local_index, block_state);
    }

    pub(crate) fn drain_pending_block_delta_events(&mut self) -> Vec<ChunkSchedulerEvent> {
        let mut events = self
            .pending_runtime_light_snapshots
            .drain(..)
            .map(ChunkSchedulerEvent::SnapshotReady)
            .collect::<Vec<_>>();
        let pending = std::mem::take(&mut self.pending_block_deltas);
        events.extend(
            pending
                .into_iter()
                .filter_map(|((pos, section_y), section_updates)| {
                    if !self.holders.get(&pos).is_some_and(|holder| {
                        holder.client_visible && holder.published_snapshot.is_some()
                    }) {
                        return None;
                    }
                    let updates = section_updates
                        .into_iter()
                        .map(|(local_index, block_state)| {
                            section_block_update_from_index(local_index, block_state)
                        })
                        .collect::<Vec<_>>();
                    (!updates.is_empty()).then_some(ChunkSchedulerEvent::SectionBlockUpdates {
                        pos,
                        section_y,
                        updates,
                    })
                }),
        );
        events
    }

    fn tick_fluid(&mut self, pos: WorldBlockPos, fluid: FluidKind) -> FluidMutationReport {
        let current_state = self.fluid_state_at_world(pos);
        if !current_state.is_same_fluid(fluid) {
            return FluidMutationReport::default();
        }

        let mut spread_state = current_state;
        let mut report = FluidMutationReport::default();
        if !current_state.source {
            let new_liquid = self.get_new_liquid(fluid, pos);
            if new_liquid.is_empty() {
                spread_state = NativeFluidState::EMPTY;
                self.set_block_during_fluid_tick(pos, mclone_worldgen::block::AIR, &mut report);
            } else if new_liquid != current_state {
                spread_state = new_liquid;
                if let Some(block_id) = legacy_block_for_fluid_state(new_liquid) {
                    self.set_block_during_fluid_tick(pos, block_id, &mut report);
                    record_scheduled_fluid_tick(&mut report, pos, fluid);
                }
            }
        }

        self.spread_fluid(pos, spread_state, &mut report);
        report
    }

    fn spread_fluid(
        &mut self,
        pos: WorldBlockPos,
        state: NativeFluidState,
        report: &mut FluidMutationReport,
    ) {
        if state.is_empty() {
            return;
        }
        let Some(fluid) = state.kind else {
            return;
        };
        let block_state = self.block_at_world(pos);
        let below = pos.below();
        let below_state = self.block_at_world(below);
        let new_below_liquid = self.get_new_liquid(fluid, below);
        if self.can_spread_to(
            fluid,
            pos,
            block_state,
            FluidDirection::Down,
            below,
            below_state,
            self.fluid_state_at_world(below),
            new_below_liquid.kind,
        ) {
            self.spread_to(
                below,
                below_state,
                FluidDirection::Down,
                new_below_liquid,
                report,
            );
            if self.source_neighbor_count(pos, fluid) >= 3 {
                self.spread_to_sides(pos, state, block_state, report);
            }
        } else if state.source
            || !self.is_water_hole(
                fluid,
                pos,
                block_state,
                below,
                below_state,
                new_below_liquid.kind,
            )
        {
            self.spread_to_sides(pos, state, block_state, report);
        }
    }

    fn spread_to_sides(
        &mut self,
        pos: WorldBlockPos,
        state: NativeFluidState,
        block_state: Option<RawBlockId>,
        report: &mut FluidMutationReport,
    ) {
        let Some(fluid) = state.kind else {
            return;
        };
        let amount = if state.falling {
            7
        } else {
            state.amount.saturating_sub(fluid_drop_off(fluid))
        };
        if amount == 0 {
            return;
        }

        for (direction, spread_state) in self.get_spread(fluid, pos, block_state) {
            let target = offset_pos(pos, direction);
            let target_state = self.block_at_world(target);
            if self.can_spread_to(
                fluid,
                pos,
                block_state,
                direction,
                target,
                target_state,
                self.fluid_state_at_world(target),
                spread_state.kind,
            ) {
                self.spread_to(target, target_state, direction, spread_state, report);
            }
        }
    }

    fn spread_to(
        &mut self,
        pos: WorldBlockPos,
        old_block: Option<RawBlockId>,
        direction: FluidDirection,
        fluid_state: NativeFluidState,
        report: &mut FluidMutationReport,
    ) {
        let block_id = if direction == FluidDirection::Down
            && fluid_state.kind == Some(FluidKind::Lava)
            && old_block.is_some_and(is_water)
        {
            STONE
        } else if fluid_state.is_empty() {
            mclone_worldgen::block::AIR
        } else if let Some(block_id) = legacy_block_for_fluid_state(fluid_state) {
            block_id
        } else {
            return;
        };
        self.set_block_during_fluid_tick(pos, block_id, report);
    }

    fn set_block_during_fluid_tick(
        &mut self,
        pos: WorldBlockPos,
        block_id: RawBlockId,
        report: &mut FluidMutationReport,
    ) {
        let before = self.block_at_world(pos);
        let set_block_start = simulation_timing_start();
        let changed_by_set = self.set_block_at_world(pos, block_id);
        report.set_block_us += simulation_timing_elapsed_us(set_block_start);
        let changed = before != self.block_at_world(pos);
        if changed_by_set && changed && self.block_at_world(pos) == Some(block_id) {
            report.mutated_positions.push(pos);
            self.schedule_fluid_ticks_after_block_change(pos, block_id, report);
            self.resolve_lava_contacts_after_block_change(pos, report);
        }
    }

    fn resolve_lava_contacts_after_block_change(
        &mut self,
        pos: WorldBlockPos,
        report: &mut FluidMutationReport,
    ) {
        let mut candidates = BTreeSet::new();
        if self.fluid_state_at_world(pos).kind == Some(FluidKind::Lava) {
            candidates.insert(pos);
        }
        if self.fluid_state_at_world(pos).kind == Some(FluidKind::Water) {
            candidates.insert(pos.below());
            for direction in HORIZONTAL_FLUID_DIRECTIONS {
                candidates.insert(offset_pos(pos, direction));
            }
        }

        for candidate in candidates {
            let set_block_start = simulation_timing_start();
            let changed = self.resolve_lava_source_contact_at(candidate);
            report.set_block_us += simulation_timing_elapsed_us(set_block_start);
            if changed {
                report.mutated_positions.push(candidate);
            }
        }
    }

    pub(crate) fn resolve_lava_source_contact_at(&mut self, pos: WorldBlockPos) -> bool {
        let state = self.fluid_state_at_world(pos);
        if state.kind != Some(FluidKind::Lava) || !state.source {
            return false;
        }
        if !LAVA_SOURCE_CONTACT_DIRECTIONS.iter().any(|direction| {
            self.fluid_state_at_world(offset_pos(pos, *direction)).kind == Some(FluidKind::Water)
        }) {
            return false;
        }
        self.set_block_at_world(pos, OBSIDIAN)
    }

    fn schedule_fluid_ticks_after_block_change(
        &self,
        pos: WorldBlockPos,
        block_id: RawBlockId,
        report: &mut FluidMutationReport,
    ) {
        for request in self.fluid_tick_requests_after_block_change(pos, block_id) {
            record_scheduled_fluid_tick(report, request.pos, request.fluid);
        }
    }

    fn fluid_state_at_world(&self, pos: WorldBlockPos) -> NativeFluidState {
        self.block_at_world(pos)
            .map(NativeFluidState::from_block_id)
            .unwrap_or(NativeFluidState::EMPTY)
    }

    fn get_new_liquid(&self, fluid: FluidKind, pos: WorldBlockPos) -> NativeFluidState {
        let mut amount = 0;
        let mut sources = 0;
        for direction in HORIZONTAL_FLUID_DIRECTIONS {
            let neighbor = offset_pos(pos, direction);
            let Some(neighbor_block) = self.block_at_world(neighbor) else {
                continue;
            };
            let neighbor_fluid = NativeFluidState::from_block_id(neighbor_block);
            if neighbor_fluid.is_same_fluid(fluid)
                && self.can_pass_through_wall(
                    direction,
                    pos,
                    self.block_at_world(pos),
                    neighbor,
                    Some(neighbor_block),
                )
            {
                if neighbor_fluid.source {
                    sources += 1;
                }
                amount = amount.max(neighbor_fluid.amount);
            }
        }

        if fluid_can_convert_to_source(fluid) && sources >= 2 {
            let below = pos.below();
            let below_block = self.block_at_world(below);
            let below_fluid = self.fluid_state_at_world(below);
            if below_block.is_some_and(|block| material_blocks_motion(block))
                || (below_fluid.is_same_fluid(fluid) && below_fluid.source)
            {
                return NativeFluidState::source(fluid);
            }
        }

        let above = offset_pos(pos, FluidDirection::Up);
        let above_fluid = self.fluid_state_at_world(above);
        if !above_fluid.is_empty()
            && above_fluid.is_same_fluid(fluid)
            && self.can_pass_through_wall(
                FluidDirection::Up,
                pos,
                self.block_at_world(pos),
                above,
                self.block_at_world(above),
            )
        {
            return NativeFluidState::flowing(fluid, 8, true);
        }

        let new_amount = amount.saturating_sub(fluid_drop_off(fluid));
        if new_amount == 0 {
            NativeFluidState::EMPTY
        } else {
            NativeFluidState::flowing(fluid, new_amount, false)
        }
    }

    fn get_spread(
        &self,
        fluid: FluidKind,
        pos: WorldBlockPos,
        state: Option<RawBlockId>,
    ) -> Vec<(FluidDirection, NativeFluidState)> {
        let mut best = 1000;
        let mut spread = Vec::new();
        let mut state_cache = BTreeMap::new();
        let mut hole_cache = BTreeMap::new();
        for direction in HORIZONTAL_FLUID_DIRECTIONS {
            let target = offset_pos(pos, direction);
            let key = fluid_cache_key(pos, target);
            let (target_state, target_fluid) =
                cached_block_and_fluid(self, key, target, &mut state_cache);
            let new_liquid = self.get_new_liquid(fluid, target);
            if self.can_pass_through(
                new_liquid.kind,
                pos,
                state,
                direction,
                target,
                target_state,
                target_fluid,
            ) {
                let water_hole = *hole_cache.entry(key).or_insert_with(|| {
                    let below = target.below();
                    self.is_water_hole(
                        fluid,
                        target,
                        target_state,
                        below,
                        self.block_at_world(below),
                        Some(fluid),
                    )
                });
                let distance = if water_hole {
                    0
                } else {
                    self.get_slope_distance(
                        fluid,
                        target,
                        1,
                        direction.opposite(),
                        target_state,
                        pos,
                        &mut state_cache,
                        &mut hole_cache,
                    )
                };
                if distance < best {
                    spread.clear();
                }
                if distance <= best {
                    spread.push((direction, new_liquid));
                    best = distance;
                }
            }
        }
        spread
    }

    fn get_slope_distance(
        &self,
        fluid: FluidKind,
        pos: WorldBlockPos,
        distance: i32,
        from_direction: FluidDirection,
        state: Option<RawBlockId>,
        origin: WorldBlockPos,
        state_cache: &mut BTreeMap<i32, (Option<RawBlockId>, NativeFluidState)>,
        hole_cache: &mut BTreeMap<i32, bool>,
    ) -> i32 {
        let mut best = 1000;
        for direction in HORIZONTAL_FLUID_DIRECTIONS {
            if direction == from_direction {
                continue;
            }
            let target = offset_pos(pos, direction);
            let key = fluid_cache_key(origin, target);
            let (target_state, target_fluid) =
                cached_block_and_fluid(self, key, target, state_cache);
            if self.can_pass_through(
                Some(fluid),
                pos,
                state,
                direction,
                target,
                target_state,
                target_fluid,
            ) {
                let water_hole = *hole_cache.entry(key).or_insert_with(|| {
                    let below = target.below();
                    self.is_water_hole(
                        fluid,
                        target,
                        target_state,
                        below,
                        self.block_at_world(below),
                        Some(fluid),
                    )
                });
                if water_hole {
                    return distance;
                }
                if distance < fluid_slope_find_distance(fluid) {
                    let recursive = self.get_slope_distance(
                        fluid,
                        target,
                        distance + 1,
                        direction.opposite(),
                        target_state,
                        origin,
                        state_cache,
                        hole_cache,
                    );
                    if recursive < best {
                        best = recursive;
                    }
                }
            }
        }
        best
    }

    fn is_water_hole(
        &self,
        fluid: FluidKind,
        pos: WorldBlockPos,
        state: Option<RawBlockId>,
        below: WorldBlockPos,
        below_state: Option<RawBlockId>,
        incoming_fluid: Option<FluidKind>,
    ) -> bool {
        if !self.can_pass_through_wall(FluidDirection::Down, pos, state, below, below_state) {
            return false;
        }
        let below_fluid = below_state
            .map(NativeFluidState::from_block_id)
            .unwrap_or(NativeFluidState::EMPTY);
        below_fluid.is_same_fluid(fluid) || self.can_hold_fluid(below, below_state, incoming_fluid)
    }

    fn can_pass_through(
        &self,
        incoming_fluid: Option<FluidKind>,
        pos: WorldBlockPos,
        state: Option<RawBlockId>,
        direction: FluidDirection,
        target: WorldBlockPos,
        target_state: Option<RawBlockId>,
        target_fluid: NativeFluidState,
    ) -> bool {
        !target_fluid.source
            && self.can_pass_through_wall(direction, pos, state, target, target_state)
            && self.can_hold_fluid(target, target_state, incoming_fluid)
    }

    fn can_spread_to(
        &self,
        current_fluid: FluidKind,
        pos: WorldBlockPos,
        state: Option<RawBlockId>,
        direction: FluidDirection,
        target: WorldBlockPos,
        target_state: Option<RawBlockId>,
        target_fluid: NativeFluidState,
        incoming_fluid: Option<FluidKind>,
    ) -> bool {
        target_fluid_can_be_replaced_with(target_fluid, current_fluid, direction)
            && self.can_pass_through_wall(direction, pos, state, target, target_state)
            && self.can_hold_fluid(target, target_state, incoming_fluid)
    }

    fn can_hold_fluid(
        &self,
        _pos: WorldBlockPos,
        state: Option<RawBlockId>,
        _fluid: Option<FluidKind>,
    ) -> bool {
        state.is_some_and(|block| !material_blocks_motion(block))
    }

    fn can_pass_through_wall(
        &self,
        _direction: FluidDirection,
        _from_pos: WorldBlockPos,
        _from_state: Option<RawBlockId>,
        _to_pos: WorldBlockPos,
        to_state: Option<RawBlockId>,
    ) -> bool {
        to_state.is_some()
    }

    fn source_neighbor_count(&self, pos: WorldBlockPos, fluid: FluidKind) -> usize {
        HORIZONTAL_FLUID_DIRECTIONS
            .iter()
            .filter(|direction| {
                let neighbor = offset_pos(pos, **direction);
                let state = self.fluid_state_at_world(neighbor);
                state.is_same_fluid(fluid) && state.source
            })
            .count()
    }

    pub fn save_dirty_chunks(&mut self) -> ChunkStoreResult<usize> {
        self.save_dirty_chunks_with_record_builder(|snapshot| {
            ChunkRecord::from_snapshot(snapshot.clone())
        })
    }

    pub fn close_persistence(&mut self) -> ChunkStoreResult<()> {
        let request_id = self.store.close();
        loop {
            let completions = self.store.drain_completions();
            let mut made_progress = !completions.is_empty();
            for completion in completions {
                match completion {
                    WorldStoreCompletion::CloseComplete {
                        request_id: completed_id,
                        result,
                    } if completed_id == request_id => return result,
                    other => {
                        self.handle_persistence_completion(other)?;
                    }
                }
            }
            if self.store.process_one_background_write() {
                made_progress = true;
            }
            if !made_progress {
                return Err(ChunkStoreError::InvalidData(format!(
                    "persistence close request {request_id} did not complete"
                )));
            }
        }
    }

    pub(crate) fn save_dirty_chunks_with_record_builder(
        &mut self,
        mut record_builder: impl FnMut(&ChunkSnapshot) -> ChunkRecord,
    ) -> ChunkStoreResult<usize> {
        self.save_dirty_chunks_with_record_builders(&mut record_builder, &mut |_, _| None)
    }

    pub(crate) fn save_dirty_chunks_with_record_builders(
        &mut self,
        record_builder: &mut impl FnMut(&ChunkSnapshot) -> ChunkRecord,
        entity_record_builder: &mut impl FnMut(ChunkPos, u64) -> Option<EntityChunkRecord>,
    ) -> ChunkStoreResult<usize> {
        let dirty_chunks = self.dirty_chunks.iter().copied().collect::<Vec<_>>();
        let mut queued = 0;
        for pos in dirty_chunks {
            let Some(holder) = self.holders.get(&pos).cloned() else {
                self.dirty_chunks.remove(&pos);
                continue;
            };
            if holder.dirty {
                if self.queue_holder_save_with_record_builder(
                    &holder,
                    SaveDurability::Durable,
                    record_builder,
                ) {
                    queued += 1;
                }
            } else {
                self.dirty_chunks.remove(&pos);
            }
        }
        if self.entity_chunks_supported() {
            for pos in self
                .loaded_entity_chunks
                .iter()
                .copied()
                .collect::<Vec<_>>()
            {
                if self.pending_entity_chunk_save_by_pos.contains_key(&pos) {
                    continue;
                }
                let revision = self.next_entity_revision();
                if let Some(record) = entity_record_builder(pos, revision)
                    && self.queue_entity_record_save(record, SaveDurability::Durable)
                {
                    queued += 1;
                }
            }
        }
        self.flush_durable_persistence()?;
        Ok(queued)
    }

    pub fn process_pending_unloads(&mut self, max_chunks: usize) -> ChunkStoreResult<usize> {
        Ok(self.process_pending_unloads_with_events(max_chunks)?.0)
    }

    fn process_pending_unloads_with_events(
        &mut self,
        max_chunks: usize,
    ) -> ChunkStoreResult<(usize, Vec<ChunkSchedulerEvent>)> {
        let mut record_builder =
            |snapshot: &ChunkSnapshot| ChunkRecord::from_snapshot(snapshot.clone());
        self.process_pending_unloads_with_events_and_record_builder(
            max_chunks,
            &mut record_builder,
            &mut |_, _| None,
        )
    }

    fn process_pending_unloads_with_events_and_record_builder(
        &mut self,
        max_chunks: usize,
        record_builder: &mut impl FnMut(&ChunkSnapshot) -> ChunkRecord,
        entity_record_builder: &mut impl FnMut(ChunkPos, u64) -> Option<EntityChunkRecord>,
    ) -> ChunkStoreResult<(usize, Vec<ChunkSchedulerEvent>)> {
        if max_chunks == 0 || self.pending_unloads.is_empty() {
            return Ok((0, Vec::new()));
        }

        let active_levels = self.distance_manager.active_levels();
        let candidates = self
            .pending_unloads
            .iter()
            .copied()
            .take(max_chunks)
            .collect::<Vec<_>>();
        let mut processed = 0;
        let mut events = Vec::new();

        for pos in candidates {
            if active_levels.contains_key(&pos) {
                self.pending_unloads.remove(&pos);
                self.entity_unload_saves.remove(&pos);
                continue;
            }

            let Some(holder) = self.holders.get(&pos).cloned() else {
                self.pending_unloads.remove(&pos);
                self.dirty_chunks.remove(&pos);
                self.entity_unload_saves.remove(&pos);
                continue;
            };

            if holder.dirty {
                self.queue_holder_save_with_record_builder(
                    &holder,
                    SaveDurability::Durable,
                    record_builder,
                );
                continue;
            }

            if self.pending_entity_chunk_save_by_pos.contains_key(&pos) {
                continue;
            }

            if self.entity_chunks_supported() && !self.entity_unload_saves.contains(&pos) {
                let revision = self.next_entity_revision();
                if let Some(record) = entity_record_builder(pos, revision)
                    && self.queue_entity_record_save(record, SaveDurability::Durable)
                {
                    self.entity_unload_saves.insert(pos);
                    continue;
                }
            }

            self.clear_pending_chunk_load(pos);
            self.clear_pending_entity_chunk_load(pos);
            self.stored_chunk_misses.remove(&pos);
            self.loaded_entity_chunks.remove(&pos);
            self.entity_unload_saves.remove(&pos);
            self.holders.remove(&pos);
            self.pending_unloads.remove(&pos);
            processed += 1;
            events.push(ChunkSchedulerEvent::HolderUnloaded { pos });
        }

        Ok((processed, events))
    }

    pub(crate) fn reconcile_ticketed_holders(
        &mut self,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let active_levels = self.distance_manager.active_levels();
        let desired_set = active_levels.keys().copied().collect::<BTreeSet<_>>();
        let client_visible_set = self.distance_manager.player_interest_positions();
        let priority_centers = self
            .distance_manager
            .player_interest_priority_centers()
            .to_vec();
        let lighting_enabled = self.lighting_enabled;
        let mut events = Vec::new();

        for pos in self
            .holders
            .keys()
            .copied()
            .filter(|pos| !desired_set.contains(pos))
            .collect::<Vec<_>>()
        {
            let holder = self.holders.get_mut(&pos).expect("holder key disappeared");
            holder.set_ticket_level(UNLOADED_CHUNK_LEVEL);
            if holder.client_visible {
                events.push(ChunkSchedulerEvent::Unloaded { pos });
                holder.set_client_visible(false);
            }
            self.pending_unloads.insert(pos);
        }

        let mut runtime_targets = BTreeSet::new();
        for (pos, ticket_level) in active_levels {
            self.pending_unloads.remove(&pos);
            let should_be_client_visible = client_visible_set.contains(&pos);
            let full_status = full_chunk_status_for_ticket_level(ticket_level);
            let has_direct_feature_ticket =
                self.distance_manager.ticket_level_at(pos) <= CHUNK_LEVEL_FULL + 1;
            let should_run_block_ticks = full_status.is_or_after(FullChunkStatus::Ticking);
            let target_status = if should_be_client_visible
                || has_direct_feature_ticket
                || should_run_block_ticks
            {
                self.runtime_chunk_target_status()
            } else {
                ticket_level_dependency_status_target(ticket_level)
            };
            let mut snapshot_to_publish = None;
            {
                let holder = self
                    .holders
                    .entry(pos)
                    .or_insert_with(|| ChunkHolder::new(pos));
                holder.set_ticket_level(ticket_level);
                holder.set_target_status(target_status);

                if !should_be_client_visible && holder.client_visible {
                    holder.set_client_visible(false);
                    events.push(ChunkSchedulerEvent::Unloaded { pos });
                } else if should_be_client_visible
                    && !holder.client_visible
                    && holder.published_snapshot.as_ref().is_some_and(|snapshot| {
                        snapshot_is_client_ready_for_lighting_mode(snapshot, lighting_enabled)
                    })
                {
                    holder.set_client_visible(true);
                    snapshot_to_publish = holder.published_snapshot.clone();
                }
            }

            if let Some(snapshot) = snapshot_to_publish {
                events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
            }

            if target_status >= ChunkStatus::Features {
                runtime_targets.insert(pos);
            } else {
                self.ensure_dependency_status_scheduled(pos, target_status);
            }
        }

        events.extend(self.enqueue_runtime_chunks(
            sorted_chunk_positions_by_priority(runtime_targets, &priority_centers),
            self.runtime_chunk_target_status(),
            &priority_centers,
        )?);
        Ok(events)
    }

    fn enqueue_runtime_chunks(
        &mut self,
        desired_chunks: Vec<ChunkPos>,
        target_status: ChunkStatus,
        priority_centers: &[ChunkPos],
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let mut events = Vec::new();
        let mut to_generate = Vec::new();
        let load_request_limit = self.chunk_load_request_limit(desired_chunks.len());
        let mut scheduled_chunk_loads = 0_usize;
        let mut scheduled_entity_loads = 0_usize;

        for pos in desired_chunks {
            self.holders
                .entry(pos)
                .or_insert_with(|| ChunkHolder::new(pos))
                .target_status = Some(target_status);
            if scheduled_entity_loads < load_request_limit && self.schedule_entity_chunk_load(pos) {
                scheduled_entity_loads += 1;
            }

            for status in status_path_to(target_status) {
                if let Some(slot) = self
                    .holders
                    .get(&pos)
                    .expect("holder must exist before scheduling")
                    .status_slot(status)
                {
                    if status == ChunkStatus::Features
                        && slot.step == ChunkStatusStep::Scheduled
                        && slot.job_id.is_none()
                    {
                        if self.stored_chunk_misses.contains(&pos) {
                            to_generate.push(pos);
                        } else if scheduled_chunk_loads < load_request_limit
                            && self.schedule_chunk_load(pos, target_status)
                        {
                            scheduled_chunk_loads += 1;
                        }
                    }
                    continue;
                }

                {
                    let holder = self
                        .holders
                        .get_mut(&pos)
                        .expect("holder must exist before scheduling");
                    holder.mark_scheduled(status);
                }
                if status < ChunkStatus::Features {
                    events.push(status_changed_event(
                        pos,
                        status,
                        ChunkStatusStep::Scheduled,
                    ));
                }

                if status == ChunkStatus::Features {
                    if self.stored_chunk_misses.contains(&pos) {
                        to_generate.push(pos);
                    } else if scheduled_chunk_loads < load_request_limit
                        && self.schedule_chunk_load(pos, target_status)
                    {
                        scheduled_chunk_loads += 1;
                    }
                } else if status < ChunkStatus::Features {
                    let holder = self
                        .holders
                        .get_mut(&pos)
                        .expect("holder must exist before marking ready");
                    holder.mark_ready(status, None);
                    events.push(status_changed_event(pos, status, ChunkStatusStep::Ready));
                }
            }
        }

        events.extend(self.enqueue_feature_job_for_missing_targets(to_generate, priority_centers));

        Ok(events)
    }

    fn chunk_load_request_limit(&self, candidate_count: usize) -> usize {
        if self.jobs.is_empty() && candidate_count > BACKGROUND_CHUNK_LOAD_REQUEST_LIMIT {
            STARTUP_CHUNK_LOAD_REQUEST_LIMIT
        } else {
            candidate_count.min(BACKGROUND_CHUNK_LOAD_REQUEST_LIMIT)
        }
    }

    fn enqueue_next_pending_feature_job(&mut self) -> Vec<ChunkSchedulerEvent> {
        let priority_centers = self
            .distance_manager
            .player_interest_priority_centers()
            .to_vec();
        let candidates = self
            .stored_chunk_misses
            .iter()
            .copied()
            .filter(|pos| {
                self.holders
                    .get(pos)
                    .and_then(|holder| holder.status_slot(ChunkStatus::Features))
                    .is_some_and(|slot| {
                        slot.step == ChunkStatusStep::Scheduled && slot.job_id.is_none()
                    })
            })
            .collect::<BTreeSet<_>>();
        let candidates = sorted_chunk_positions_by_priority(candidates, &priority_centers);
        self.enqueue_feature_job_for_missing_targets(candidates, &priority_centers)
    }

    fn enqueue_feature_job_for_missing_targets(
        &mut self,
        candidates_in_priority_order: Vec<ChunkPos>,
        priority_centers: &[ChunkPos],
    ) -> Vec<ChunkSchedulerEvent> {
        if candidates_in_priority_order.is_empty() || self.has_incomplete_feature_status_job() {
            return Vec::new();
        }

        let candidates = dedupe_chunk_positions_preserving_order(candidates_in_priority_order);
        let target_limit = self.feature_job_target_limit(candidates.len());
        let job_targets = candidates
            .into_iter()
            .take(target_limit)
            .collect::<Vec<_>>();
        if job_targets.is_empty() {
            return Vec::new();
        }
        if self.feature_publication_backlog_blocks_job(job_targets.len()) {
            return Vec::new();
        }

        let (job_id, seeded_dependencies) = self.create_feature_job(&job_targets, priority_centers);
        let mut events = Vec::with_capacity(job_targets.len());
        for pos in &job_targets {
            self.stored_chunk_misses.remove(pos);
            self.holders
                .get_mut(pos)
                .expect("holder must exist before assigning job")
                .assign_status_job(ChunkStatus::Features, job_id);
            events.push(status_changed_event(
                *pos,
                ChunkStatus::Features,
                ChunkStatusStep::Scheduled,
            ));
        }

        self.mark_job_state(job_id, ChunkJobState::Running);
        self.worldgen_mailbox.enqueue_features(
            job_id,
            self.seed,
            &job_targets,
            seeded_dependencies,
        );
        events
    }

    fn has_incomplete_feature_status_job(&self) -> bool {
        self.jobs
            .values()
            .any(|job| matches!(job.state, ChunkJobState::Queued | ChunkJobState::Running))
    }

    fn feature_job_target_limit(&self, candidate_count: usize) -> usize {
        if self.jobs.is_empty() && candidate_count > BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT {
            STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT
        } else {
            candidate_count.min(BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT)
        }
    }

    fn feature_publication_backlog_blocks_job(&self, target_count: usize) -> bool {
        if !self.publication_budget.enabled() {
            return false;
        }
        let pending_after_admission = self
            .pending_worldgen_publication_target_count()
            .saturating_add(target_count);
        pending_after_admission
            > self
                .publication_budget
                .last_pending_worldgen_publication_chunk_limit
    }

    fn publish_completed_worldgen_jobs(
        &mut self,
        diagnostics: &mut ChunkSchedulerPublicationDiagnostics,
        grant: PublicationGrant,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let completed = self.worldgen_mailbox.drain_completed();
        diagnostics.completed_feature_jobs_drained = diagnostics
            .completed_feature_jobs_drained
            .saturating_add(completed.len());
        if self.publication_budget.enabled() {
            for completed_job in &completed {
                self.mark_job_complete(
                    completed_job.job_id,
                    completed_job.cache_report,
                    completed_job.timing,
                );
                diagnostics.feature_jobs_pipeline_completed = diagnostics
                    .feature_jobs_pipeline_completed
                    .saturating_add(1);
            }
        }
        self.pending_worldgen_publications
            .extend(completed.into_iter().map(PendingWorldgenPublication::new));
        let mut events = Vec::new();
        if self.publication_budget.enabled() && !self.pending_worldgen_publications.is_empty() {
            events.extend(self.enqueue_next_pending_feature_job());
        }
        let publish_start = simulation_timing_start();
        let before_units = diagnostics
            .feature_chunks_published
            .saturating_add(diagnostics.feature_chunks_skipped);
        let mut remaining_budget = grant.max_units;

        while grant.allows_next_unit(
            publish_start,
            grant.max_units.saturating_sub(remaining_budget),
        ) {
            let Some(publication) = self.pending_worldgen_publications.pop_front() else {
                break;
            };
            let (mut published, pending) = self.publish_completed_feature_job(
                publication,
                &mut remaining_budget,
                grant,
                publish_start,
                diagnostics,
            )?;
            events.append(&mut published);
            if let Some(pending) = pending {
                self.pending_worldgen_publications.push_front(pending);
                break;
            }
        }

        let spent_units = diagnostics
            .feature_chunks_published
            .saturating_add(diagnostics.feature_chunks_skipped)
            .saturating_sub(before_units);
        let spent_us = simulation_timing_elapsed_us(publish_start);
        diagnostics.feature_publish_spent_units = spent_units;
        diagnostics.feature_publish_spent_us = spent_us;
        self.observe_publication_spend(
            BudgetDecisionFamily::FeaturePublication,
            spent_us,
            spent_units,
        );
        Ok(events)
    }

    fn publish_completed_feature_job(
        &mut self,
        mut publication: PendingWorldgenPublication,
        remaining_budget: &mut usize,
        grant: PublicationGrant,
        publish_start: SimulationTimingStart,
        diagnostics: &mut ChunkSchedulerPublicationDiagnostics,
    ) -> ChunkStoreResult<(Vec<ChunkSchedulerEvent>, Option<PendingWorldgenPublication>)> {
        let Some(job) = self.jobs.get(&publication.completed.job_id).cloned() else {
            return Ok((Vec::new(), None));
        };

        let mut events = Vec::new();
        let generated = &publication.completed.generated_chunks;
        let retained_dependencies = &publication.completed.retained_dependencies;

        while publication.next_target_index < job.target_chunks.len()
            && *remaining_budget > 0
            && grant.allows_next_unit(
                publish_start,
                grant.max_units.saturating_sub(*remaining_budget),
            )
        {
            let pos = job.target_chunks[publication.next_target_index];
            publication.next_target_index += 1;
            *remaining_budget -= 1;
            let should_publish = self
                .holders
                .get(&pos)
                .and_then(|holder| holder.status_slot(ChunkStatus::Features))
                .is_some_and(|slot| {
                    slot.job_id == Some(publication.completed.job_id)
                        && slot.step == ChunkStatusStep::Scheduled
                });
            if !should_publish {
                diagnostics.feature_chunks_skipped =
                    diagnostics.feature_chunks_skipped.saturating_add(1);
                continue;
            }
            diagnostics.feature_chunks_published =
                diagnostics.feature_chunks_published.saturating_add(1);

            let revision = ChunkRevision(self.next_revision);
            self.next_revision += 1;

            let chunk = generated.get(&pos).unwrap_or_else(|| {
                panic!(
                    "feature batch did not return scheduler target chunk ({}, {})",
                    pos.x, pos.z
                )
            });
            let scheduled_block_ticks = scheduled_block_tick_records_from_generated_chunk(&chunk);
            let scheduled_fluid_ticks = scheduled_fluid_tick_records_from_generated_chunk(&chunk);
            events.extend(block_tick_events_from_records(&scheduled_block_ticks)?);
            events.extend(fluid_tick_events_from_records(&scheduled_fluid_ticks)?);
            let snapshot = chunk.to_chunk_snapshot(revision, ChunkStatus::Features);
            self.mark_snapshot_ready(pos, snapshot.clone(), ChunkResidency::Generated, false);
            self.queue_record_save(
                ChunkRecord::from_snapshot(snapshot.clone())
                    .with_scheduled_block_ticks(scheduled_block_ticks.clone())
                    .with_scheduled_fluid_ticks(scheduled_fluid_ticks.clone()),
                SaveDurability::Cache,
            );
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status: ChunkStatus::Features,
                step: ChunkStatusStep::Ready,
            });

            let should_light = self.lighting_enabled
                && self
                    .holders
                    .get(&pos)
                    .and_then(|holder| holder.status_slot(ChunkStatus::Light))
                    .is_some_and(|slot| slot.step == ChunkStatusStep::Scheduled);
            if should_light {
                events.push(status_changed_event(
                    pos,
                    ChunkStatus::Light,
                    ChunkStatusStep::Scheduled,
                ));
                let ready_light_batch = {
                    let statuses = self
                        .pending_light_status_batches
                        .entry(publication.completed.job_id)
                        .or_default();
                    statuses.push(PendingLightStatus::from_feature_publication(
                        pos,
                        snapshot,
                        chunk,
                        scheduled_block_ticks,
                        scheduled_fluid_ticks,
                        generated.iter(),
                        retained_dependencies.iter(),
                    ));
                    (statuses.len() >= self.light_status_batch_size)
                        .then(|| std::mem::take(statuses))
                };
                if let Some(statuses) = ready_light_batch {
                    diagnostics.light_status_batches_enqueued =
                        diagnostics.light_status_batches_enqueued.saturating_add(1);
                    self.enqueue_light_status_batch(statuses);
                }
            } else if self
                .distance_manager
                .player_interest_positions()
                .contains(&pos)
                && self.snapshot_is_client_ready(&snapshot)
            {
                self.holders
                    .get_mut(&pos)
                    .expect("holder must exist before marking visible")
                    .set_client_visible(true);
                diagnostics.feature_snapshot_ready_events =
                    diagnostics.feature_snapshot_ready_events.saturating_add(1);
                events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
            }
        }

        if publication.next_target_index < job.target_chunks.len() {
            return Ok((events, Some(publication)));
        }

        let completed = publication.completed;
        let pending_light_statuses = self
            .pending_light_status_batches
            .remove(&completed.job_id)
            .unwrap_or_default();
        for dependency in completed.retained_dependencies.into_values() {
            self.mark_dependency_ready(dependency);
        }
        if !self.publication_budget.enabled() {
            self.mark_job_complete(completed.job_id, completed.cache_report, completed.timing);
        }
        diagnostics.feature_jobs_completed = diagnostics.feature_jobs_completed.saturating_add(1);
        if !pending_light_statuses.is_empty() {
            diagnostics.light_status_batches_enqueued =
                diagnostics.light_status_batches_enqueued.saturating_add(1);
            self.enqueue_light_status_batch(pending_light_statuses);
        }
        if !self.publication_budget.enabled() {
            events.extend(self.enqueue_next_pending_feature_job());
        }
        Ok((events, None))
    }

    fn enqueue_light_status_batch(&mut self, statuses: Vec<PendingLightStatus>) {
        let priority_centers = self
            .distance_manager
            .player_interest_priority_centers()
            .to_vec();
        let statuses = light_statuses_sorted_by_priority(statuses, &priority_centers);
        self.light_mailbox
            .enqueue_batch(PendingLightStatusBatch::new(statuses));
    }

    fn publish_pending_light_statuses(
        &mut self,
        diagnostics: &mut ChunkSchedulerPublicationDiagnostics,
        grant: PublicationGrant,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let completed = self.light_mailbox.drain_completed();
        diagnostics.completed_light_statuses_drained = diagnostics
            .completed_light_statuses_drained
            .saturating_add(completed.len());
        self.pending_light_publications.extend(completed);
        self.prioritize_pending_light_publications();
        let mut events = Vec::new();
        let publish_start = simulation_timing_start();
        let before_units = diagnostics
            .light_statuses_published
            .saturating_add(diagnostics.light_statuses_skipped);
        let mut remaining_budget = grant.max_units;
        while grant.allows_next_unit(
            publish_start,
            grant.max_units.saturating_sub(remaining_budget),
        ) {
            let Some(completed) = self.pending_light_publications.pop_front() else {
                break;
            };
            remaining_budget -= 1;

            let should_publish = self
                .holders
                .get(&completed.pos)
                .and_then(|holder| holder.status_slot(ChunkStatus::Light))
                .is_some_and(|slot| slot.step == ChunkStatusStep::Scheduled);
            if !should_publish {
                diagnostics.light_statuses_skipped =
                    diagnostics.light_statuses_skipped.saturating_add(1);
                continue;
            }
            diagnostics.light_statuses_published =
                diagnostics.light_statuses_published.saturating_add(1);

            let revision = ChunkRevision(self.next_revision);
            self.next_revision += 1;
            self.completed_light_statuses = self.completed_light_statuses.saturating_add(1);
            if completed.batch_compute_leader {
                self.completed_light_batches = self.completed_light_batches.saturating_add(1);
                self.total_light_status_compute_us = self
                    .total_light_status_compute_us
                    .saturating_add(completed.compute_us);
                self.max_light_status_compute_us =
                    self.max_light_status_compute_us.max(completed.compute_us);
            }
            self.light_status_timing.add_assign(completed.timing);
            let mut snapshot = completed.feature_snapshot;
            snapshot.status = ChunkStatus::Light;
            snapshot.revision = revision;
            let snapshot = snapshot.with_light_sections(true, completed.light_sections);
            let pos = snapshot.pos;

            self.mark_snapshot_ready(pos, snapshot.clone(), ChunkResidency::Generated, false);
            self.queue_record_save(
                ChunkRecord::from_snapshot(snapshot.clone())
                    .with_scheduled_block_ticks(completed.scheduled_block_ticks)
                    .with_scheduled_fluid_ticks(completed.scheduled_fluid_ticks),
                SaveDurability::Cache,
            );
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status: ChunkStatus::Light,
                step: ChunkStatusStep::Ready,
            });
            if self
                .distance_manager
                .player_interest_positions()
                .contains(&pos)
                && self.snapshot_is_client_ready(&snapshot)
            {
                self.holders
                    .get_mut(&pos)
                    .expect("holder must exist before marking visible")
                    .set_client_visible(true);
                diagnostics.light_snapshot_ready_events =
                    diagnostics.light_snapshot_ready_events.saturating_add(1);
                events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
            }
        }

        let spent_units = diagnostics
            .light_statuses_published
            .saturating_add(diagnostics.light_statuses_skipped)
            .saturating_sub(before_units);
        let spent_us = simulation_timing_elapsed_us(publish_start);
        diagnostics.light_publish_spent_units = spent_units;
        diagnostics.light_publish_spent_us = spent_us;
        if spent_units > 0 {
            self.total_light_status_publication_us = self
                .total_light_status_publication_us
                .saturating_add(spent_us);
            self.max_light_status_publication_us =
                self.max_light_status_publication_us.max(spent_us);
            self.total_light_status_publication_units = self
                .total_light_status_publication_units
                .saturating_add(spent_units);
        }
        self.observe_publication_spend(
            BudgetDecisionFamily::LightPublication,
            spent_us,
            spent_units,
        );
        Ok(events)
    }

    fn prioritize_pending_light_publications(&mut self) {
        if self.pending_light_publications.len() < 2 {
            return;
        }
        let priority_centers = self
            .distance_manager
            .player_interest_priority_centers()
            .to_vec();
        if priority_centers.is_empty() {
            return;
        }
        self.pending_light_publications
            .make_contiguous()
            .sort_by_key(|completed| chunk_priority_key(completed.pos, &priority_centers));
    }

    fn observe_publication_spend(
        &mut self,
        family: BudgetDecisionFamily,
        elapsed_us: u128,
        units: usize,
    ) {
        if !self.publication_budget.enabled() || units == 0 {
            return;
        }
        let Some(target_period_ms) = self.publication_budget.config.target_period_ms() else {
            return;
        };
        self.publication_budget.estimator.observe_for_target_period(
            target_period_ms,
            family,
            micros_to_ms(elapsed_us),
            units as u32,
        );
    }

    fn create_feature_job(
        &mut self,
        targets: &[ChunkPos],
        priority_centers: &[ChunkPos],
    ) -> (ChunkJobId, Vec<MutableChunkBlockBuffer>) {
        let id = ChunkJobId(self.next_job_id);
        self.next_job_id += 1;
        let (target_chunks, feature_centers, dependency_chunks) =
            feature_job_positions(targets, priority_centers);
        let seeded_dependencies = self.seeded_dependency_buffers(&dependency_chunks);
        for pos in &dependency_chunks {
            self.ensure_dependency_status_scheduled(*pos, ChunkStatus::Surface);
        }
        self.jobs.insert(
            id,
            ChunkStatusJob {
                id,
                status: ChunkStatus::Features,
                state: ChunkJobState::Queued,
                target_chunks,
                feature_centers,
                dependency_chunks,
                seeded_dependency_chunks: seeded_dependencies.len(),
                dependency_cache_hits: 0,
                dependency_cache_misses: 0,
                retained_dependency_chunks: 0,
            },
        );
        (id, seeded_dependencies)
    }

    fn mark_job_state(&mut self, id: ChunkJobId, state: ChunkJobState) {
        self.jobs
            .get_mut(&id)
            .expect("job must exist before state transition")
            .state = state;
    }

    fn mark_job_complete(
        &mut self,
        id: ChunkJobId,
        cache_report: OverworldFeatureDependencyCacheReport,
        timing: OverworldFeatureBatchTiming,
    ) {
        let job = self
            .jobs
            .get_mut(&id)
            .expect("job must exist before state transition");
        job.dependency_cache_hits = cache_report.cache_hits;
        job.dependency_cache_misses = cache_report.generated_dependency_chunks;
        job.retained_dependency_chunks = cache_report.retained_dependency_chunks;
        job.state = ChunkJobState::Complete;
        self.job_timings.insert(id, timing);
    }

    fn runtime_chunk_target_status(&self) -> ChunkStatus {
        if self.lighting_enabled {
            ChunkStatus::Light
        } else {
            ChunkStatus::Features
        }
    }

    fn next_entity_revision(&mut self) -> u64 {
        let revision = self.next_revision;
        self.next_revision = self.next_revision.saturating_add(1);
        revision
    }

    fn snapshot_is_client_ready(&self, snapshot: &ChunkSnapshot) -> bool {
        snapshot_is_client_ready_for_lighting_mode(snapshot, self.lighting_enabled)
    }

    fn mark_snapshot_ready(
        &mut self,
        pos: ChunkPos,
        snapshot: ChunkSnapshot,
        residency: ChunkResidency,
        dirty: bool,
    ) {
        let holder = self
            .holders
            .get_mut(&pos)
            .expect("holder must exist before publishing");
        holder.publish_snapshot(snapshot, residency, dirty);
        if dirty {
            self.dirty_chunks.insert(pos);
        } else {
            self.dirty_chunks.remove(&pos);
        }
    }

    fn schedule_chunk_load(&mut self, pos: ChunkPos, target_status: ChunkStatus) -> bool {
        if self.pending_chunk_load_by_pos.contains_key(&pos) {
            return false;
        }
        let request_id = self.store.load_chunk(pos);
        self.pending_chunk_loads
            .insert(request_id, PendingChunkLoad { pos, target_status });
        self.pending_chunk_load_by_pos.insert(pos, request_id);
        true
    }

    fn schedule_entity_chunk_load(&mut self, pos: ChunkPos) -> bool {
        if !self.entity_chunks_supported()
            || self.loaded_entity_chunks.contains(&pos)
            || self.pending_entity_chunk_load_by_pos.contains_key(&pos)
        {
            return false;
        }
        let request_id = self.store.load_entity_chunk(pos);
        self.pending_entity_chunk_loads.insert(request_id, pos);
        self.pending_entity_chunk_load_by_pos
            .insert(pos, request_id);
        true
    }

    fn clear_pending_chunk_load(&mut self, pos: ChunkPos) {
        if let Some(request_id) = self.pending_chunk_load_by_pos.remove(&pos) {
            self.pending_chunk_loads.remove(&request_id);
        }
    }

    fn clear_pending_entity_chunk_load(&mut self, pos: ChunkPos) {
        if let Some(request_id) = self.pending_entity_chunk_load_by_pos.remove(&pos) {
            self.pending_entity_chunk_loads.remove(&request_id);
        }
    }

    fn poll_persistence(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let mut events = Vec::new();
        let stored_chunk_miss_count = self.stored_chunk_misses.len();
        let pending_load_count = self.pending_persistence_load_count();
        for completion in self.store.drain_completions() {
            events.extend(self.handle_persistence_completion(completion)?);
        }
        if self.stored_chunk_misses.len() != stored_chunk_miss_count
            || self.pending_persistence_load_count() != pending_load_count
        {
            events.extend(self.reconcile_ticketed_holders()?);
        }
        Ok(events)
    }

    fn handle_persistence_completion(
        &mut self,
        completion: WorldStoreCompletion,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        match completion {
            WorldStoreCompletion::ChunkLoaded {
                request_id,
                pos,
                result,
            } => self.handle_chunk_load_completion(request_id, pos, result),
            WorldStoreCompletion::ChunkSaved {
                request_id,
                pos,
                result,
            } => {
                self.handle_chunk_save_completion(request_id, pos, result)?;
                Ok(Vec::new())
            }
            WorldStoreCompletion::EntityChunkLoaded {
                request_id,
                pos,
                result,
            } => self.handle_entity_chunk_load_completion(request_id, pos, result),
            WorldStoreCompletion::EntityChunkSaved {
                request_id,
                pos,
                result,
            } => {
                self.handle_entity_chunk_save_completion(request_id, pos, result)?;
                Ok(Vec::new())
            }
            WorldStoreCompletion::RequestFailed { result, .. }
            | WorldStoreCompletion::FlushComplete { result, .. }
            | WorldStoreCompletion::CloseComplete { result, .. } => {
                result?;
                Ok(Vec::new())
            }
        }
    }

    fn handle_chunk_load_completion(
        &mut self,
        request_id: PersistenceRequestId,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<ChunkRecord>>,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let Some(pending) = self.pending_chunk_loads.remove(&request_id) else {
            return Ok(Vec::new());
        };
        if self
            .pending_chunk_load_by_pos
            .get(&pos)
            .is_some_and(|pending_request_id| *pending_request_id == request_id)
        {
            self.pending_chunk_load_by_pos.remove(&pos);
        }
        if pending.pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "chunk load request {request_id} completed for ({}, {}) but was pending for ({}, {})",
                pos.x, pos.z, pending.pos.x, pending.pos.z
            )));
        }

        let record = result?;
        if !self.holder_is_active(pos) {
            return Ok(Vec::new());
        }

        let Some(record) = record else {
            return self.mark_stored_chunk_miss(pos, pending.target_status);
        };
        if record.snapshot.status < pending.target_status {
            return self.mark_stored_chunk_miss(pos, pending.target_status);
        }

        self.publish_loaded_record(pos, record, pending.target_status)
    }

    fn handle_entity_chunk_load_completion(
        &mut self,
        request_id: PersistenceRequestId,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<EntityChunkRecord>>,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let Some(pending_pos) = self.pending_entity_chunk_loads.remove(&request_id) else {
            return Ok(Vec::new());
        };
        if self
            .pending_entity_chunk_load_by_pos
            .get(&pos)
            .is_some_and(|pending_request_id| *pending_request_id == request_id)
        {
            self.pending_entity_chunk_load_by_pos.remove(&pos);
        }
        if pending_pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "entity chunk load request {request_id} completed for ({}, {}) but was pending for ({}, {})",
                pos.x, pos.z, pending_pos.x, pending_pos.z
            )));
        }

        let record = result?;
        if !self.holder_is_active(pos) {
            return Ok(Vec::new());
        }
        if let Some(record) = &record
            && record.pos != pos
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "entity chunk load request {request_id} returned ({}, {}) for requested ({}, {})",
                record.pos.x, record.pos.z, pos.x, pos.z
            )));
        }
        self.loaded_entity_chunks.insert(pos);
        Ok(vec![ChunkSchedulerEvent::EntityChunkLoaded { pos, record }])
    }

    fn holder_is_active(&self, pos: ChunkPos) -> bool {
        self.holders
            .get(&pos)
            .is_some_and(|holder| holder.ticket_level <= MAX_CHUNK_DISTANCE)
    }

    fn mark_stored_chunk_miss(
        &mut self,
        pos: ChunkPos,
        _target_status: ChunkStatus,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.stored_chunk_misses.insert(pos);
        Ok(Vec::new())
    }

    fn publish_loaded_record(
        &mut self,
        pos: ChunkPos,
        record: ChunkRecord,
        target_status: ChunkStatus,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let scheduled_block_ticks = record.scheduled_block_ticks.clone();
        let scheduled_fluid_ticks = record.scheduled_fluid_ticks.clone();
        let snapshot = record.snapshot;
        let snapshot = if self.lighting_enabled {
            hydrate_loaded_light_snapshot(snapshot)?
        } else {
            snapshot
        };
        let revision = snapshot.revision;
        self.next_revision = self.next_revision.max(revision.0.saturating_add(1));

        let mut events = Vec::new();
        let Some(holder) = self.holders.get(&pos) else {
            return Ok(events);
        };
        if !holder
            .status_slot(ChunkStatus::Features)
            .is_some_and(|slot| slot.step == ChunkStatusStep::Scheduled)
            && holder.highest_ready_status() < Some(ChunkStatus::Features)
        {
            return Ok(events);
        }

        {
            let holder = self
                .holders
                .get_mut(&pos)
                .expect("holder must exist before marking loaded features ready");
            holder.mark_ready(ChunkStatus::Features, Some(revision));
        }
        events.push(ChunkSchedulerEvent::StatusChanged {
            pos,
            status: ChunkStatus::Features,
            step: ChunkStatusStep::Ready,
        });

        if target_status >= ChunkStatus::Light
            && self
                .holders
                .get(&pos)
                .expect("holder must exist before scheduling loaded light")
                .status_slot(ChunkStatus::Light)
                .is_none()
        {
            self.holders
                .get_mut(&pos)
                .expect("holder must exist before marking loaded light scheduled")
                .mark_scheduled(ChunkStatus::Light);
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status: ChunkStatus::Light,
                step: ChunkStatusStep::Scheduled,
            });
        }

        self.mark_snapshot_ready(
            pos,
            snapshot.clone(),
            ChunkResidency::LoadedFromStore,
            false,
        );
        if snapshot.status >= ChunkStatus::Light {
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status: ChunkStatus::Light,
                step: ChunkStatusStep::Ready,
            });
        }
        events.extend(block_tick_events_from_records(&scheduled_block_ticks)?);
        events.extend(fluid_tick_events_from_records(&scheduled_fluid_ticks)?);

        if self
            .distance_manager
            .player_interest_positions()
            .contains(&pos)
            && self.snapshot_is_client_ready(&snapshot)
        {
            self.holders
                .get_mut(&pos)
                .expect("holder must exist before marking loaded snapshot visible")
                .set_client_visible(true);
            events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
        }
        Ok(events)
    }

    fn queue_holder_save_with_record_builder(
        &mut self,
        holder: &ChunkHolder,
        durability: SaveDurability,
        record_builder: &mut impl FnMut(&ChunkSnapshot) -> ChunkRecord,
    ) -> bool {
        let Some(snapshot) = holder.snapshot() else {
            return false;
        };
        self.queue_record_save(record_builder(snapshot), durability)
    }

    fn queue_record_save(&mut self, record: ChunkRecord, durability: SaveDurability) -> bool {
        let pos = record.pos();
        let revision = record.revision();
        if let Some(existing_id) = self.pending_chunk_save_by_pos.get(&pos).copied()
            && let Some(existing) = self.pending_chunk_saves.get(&existing_id)
            && !pending_save_should_replace(revision, durability, existing)
        {
            return false;
        }

        let request_id = self.store.save_chunk(record, durability);
        self.pending_chunk_saves.insert(
            request_id,
            PendingChunkSave {
                pos,
                revision,
                durability,
            },
        );
        self.pending_chunk_save_by_pos.insert(pos, request_id);
        true
    }

    fn queue_entity_record_save(
        &mut self,
        record: EntityChunkRecord,
        durability: SaveDurability,
    ) -> bool {
        if !self.entity_chunks_supported() {
            return false;
        }
        let pos = record.pos;
        let revision = record.revision;
        if let Some(existing_id) = self.pending_entity_chunk_save_by_pos.get(&pos).copied()
            && let Some(existing) = self.pending_entity_chunk_saves.get(&existing_id)
            && !pending_entity_save_should_replace(revision, durability, existing)
        {
            return false;
        }

        let request_id = self.store.save_entity_chunk(record, durability);
        self.pending_entity_chunk_saves.insert(
            request_id,
            PendingEntityChunkSave {
                pos,
                revision,
                durability,
            },
        );
        self.pending_entity_chunk_save_by_pos
            .insert(pos, request_id);
        true
    }

    fn handle_chunk_save_completion(
        &mut self,
        request_id: PersistenceRequestId,
        pos: ChunkPos,
        result: ChunkStoreResult<StoreWriteOutcome>,
    ) -> ChunkStoreResult<()> {
        let pending = self.pending_chunk_saves.remove(&request_id);
        if self
            .pending_chunk_save_by_pos
            .get(&pos)
            .is_some_and(|pending_request_id| *pending_request_id == request_id)
        {
            self.pending_chunk_save_by_pos.remove(&pos);
        }

        let outcome = result?;
        let Some(pending) = pending else {
            return Ok(());
        };
        if pending.pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "chunk save request {request_id} completed for ({}, {}) but was pending for ({}, {})",
                pos.x, pos.z, pending.pos.x, pending.pos.z
            )));
        }

        if outcome != StoreWriteOutcome::Written || pending.durability != SaveDurability::Durable {
            return Ok(());
        }

        if let Some(holder) = self.holders.get_mut(&pos) {
            let current_revision = holder.snapshot().map(|snapshot| snapshot.revision);
            if current_revision.is_none_or(|revision| revision <= pending.revision) {
                holder.mark_saved();
                self.dirty_chunks.remove(&pos);
            }
        } else {
            self.dirty_chunks.remove(&pos);
        }
        Ok(())
    }

    fn handle_entity_chunk_save_completion(
        &mut self,
        request_id: PersistenceRequestId,
        pos: ChunkPos,
        result: ChunkStoreResult<StoreWriteOutcome>,
    ) -> ChunkStoreResult<()> {
        let pending = self.pending_entity_chunk_saves.remove(&request_id);
        if self
            .pending_entity_chunk_save_by_pos
            .get(&pos)
            .is_some_and(|pending_request_id| *pending_request_id == request_id)
        {
            self.pending_entity_chunk_save_by_pos.remove(&pos);
        }

        let _outcome = result?;
        let Some(pending) = pending else {
            return Ok(());
        };
        if pending.pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "entity chunk save request {request_id} completed for ({}, {}) but was pending for ({}, {})",
                pos.x, pos.z, pending.pos.x, pending.pos.z
            )));
        }
        Ok(())
    }

    fn flush_durable_persistence(&mut self) -> ChunkStoreResult<()> {
        let request_id = self.store.flush();
        loop {
            let completions = self.store.drain_completions();
            let mut made_progress = !completions.is_empty();
            for completion in completions {
                match completion {
                    WorldStoreCompletion::FlushComplete {
                        request_id: completed_id,
                        result,
                    } if completed_id == request_id => return result,
                    other => {
                        self.handle_persistence_completion(other)?;
                    }
                }
            }
            if self.store.process_one_background_write() {
                made_progress = true;
            }
            if !made_progress {
                return Err(ChunkStoreError::InvalidData(format!(
                    "persistence flush request {request_id} did not complete"
                )));
            }
        }
    }

    fn ensure_dependency_status_scheduled(&mut self, pos: ChunkPos, target_status: ChunkStatus) {
        let holder = self
            .holders
            .entry(pos)
            .or_insert_with(|| ChunkHolder::new(pos));
        holder.set_target_status(target_status);

        for status in status_path_to(target_status) {
            if holder.status_slot(status).is_none() {
                holder.mark_scheduled(status);
            }
        }
    }

    fn mark_dependency_ready(&mut self, buffer: MutableChunkBlockBuffer) {
        let pos = ChunkPos::new(buffer.chunk_x, buffer.chunk_z);
        let holder = self
            .holders
            .entry(pos)
            .or_insert_with(|| ChunkHolder::new(pos));
        holder.set_target_status(ChunkStatus::Surface);
        holder.set_dependency_buffer(buffer);
        for status in status_path_to(ChunkStatus::Surface) {
            holder.mark_ready(status, None);
        }
    }

    fn seeded_dependency_buffers(
        &self,
        dependency_chunks: &[ChunkPos],
    ) -> Vec<MutableChunkBlockBuffer> {
        dependency_chunks
            .iter()
            .filter_map(|pos| {
                self.holders
                    .get(pos)
                    .and_then(ChunkHolder::dependency_buffer)
                    .cloned()
            })
            .collect()
    }

    fn full_status_chunks_at_or_after(&self, status: FullChunkStatus) -> Vec<ChunkPos> {
        let positions = self
            .holders
            .values()
            .filter(|holder| holder.ticket_level <= MAX_CHUNK_DISTANCE)
            .filter(|holder| holder.full_status().is_or_after(status))
            .map(ChunkHolder::pos)
            .collect::<BTreeSet<_>>();
        sorted_chunk_positions_z_major(positions)
    }
}

fn pending_save_should_replace(
    revision: ChunkRevision,
    durability: SaveDurability,
    existing: &PendingChunkSave,
) -> bool {
    if revision != existing.revision {
        return revision > existing.revision;
    }
    match (durability, existing.durability) {
        (SaveDurability::Durable, SaveDurability::Cache) => true,
        (SaveDurability::Cache, SaveDurability::Durable) => false,
        _ => true,
    }
}

fn pending_entity_save_should_replace(
    revision: u64,
    durability: SaveDurability,
    existing: &PendingEntityChunkSave,
) -> bool {
    if revision != existing.revision {
        return revision > existing.revision;
    }
    match (durability, existing.durability) {
        (SaveDurability::Durable, SaveDurability::Cache) => true,
        (SaveDurability::Cache, SaveDurability::Durable) => false,
        _ => true,
    }
}

fn scheduled_block_tick_records_from_generated_chunk(
    chunk: &GeneratedChunk,
) -> Vec<ScheduledTickRecord> {
    chunk
        .block_ticks()
        .iter()
        .map(scheduled_tick_record_from_generated_tick)
        .collect()
}

fn scheduled_tick_record_from_generated_tick(tick: &ScheduledTick) -> ScheduledTickRecord {
    ScheduledTickRecord::new(
        WorldBlockPos::new(tick.x, tick.y, tick.z),
        tick.target.clone(),
        tick.delay,
    )
}

fn block_tick_events_from_records(
    ticks: &[ScheduledTickRecord],
) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
    ticks
        .iter()
        .map(block_tick_event_from_record)
        .collect::<ChunkStoreResult<Vec<_>>>()
}

fn block_tick_event_from_record(
    tick: &ScheduledTickRecord,
) -> ChunkStoreResult<ChunkSchedulerEvent> {
    if tick.target.is_empty() {
        return Err(ChunkStoreError::InvalidData(
            "scheduled block tick target was empty".to_owned(),
        ));
    }
    Ok(ChunkSchedulerEvent::BlockTickScheduled {
        pos: tick.pos,
        target: tick.target.clone(),
        delay: tick.delay,
    })
}

fn scheduled_fluid_tick_records_from_generated_chunk(
    chunk: &GeneratedChunk,
) -> Vec<ScheduledTickRecord> {
    chunk
        .liquid_ticks()
        .iter()
        .filter_map(scheduled_fluid_tick_record_from_generated_tick)
        .collect()
}

fn scheduled_fluid_tick_record_from_generated_tick(
    tick: &ScheduledTick,
) -> Option<ScheduledTickRecord> {
    let fluid = FluidKind::from_target(&tick.target)?;
    Some(ScheduledTickRecord::new(
        WorldBlockPos::new(tick.x, tick.y, tick.z),
        scheduled_fluid_tick_target(fluid),
        tick.delay,
    ))
}

fn fluid_tick_events_from_records(
    ticks: &[ScheduledTickRecord],
) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
    ticks
        .iter()
        .map(fluid_tick_event_from_record)
        .collect::<ChunkStoreResult<Vec<_>>>()
}

fn fluid_tick_event_from_record(
    tick: &ScheduledTickRecord,
) -> ChunkStoreResult<ChunkSchedulerEvent> {
    let fluid = FluidKind::from_target(&tick.target).ok_or_else(|| {
        ChunkStoreError::InvalidData(format!(
            "scheduled fluid tick target '{}' is not a known fluid",
            tick.target
        ))
    })?;
    Ok(ChunkSchedulerEvent::FluidTickScheduled {
        pos: tick.pos,
        fluid,
        delay: tick.delay,
    })
}

fn status_changed_event(
    pos: ChunkPos,
    status: ChunkStatus,
    step: ChunkStatusStep,
) -> ChunkSchedulerEvent {
    ChunkSchedulerEvent::StatusChanged { pos, status, step }
}

pub(crate) const fn scheduled_fluid_tick_target(fluid: FluidKind) -> &'static str {
    match fluid {
        FluidKind::Water => "minecraft:water",
        FluidKind::Lava => "minecraft:lava",
    }
}

fn remaining_tick_delay(trigger_tick: u64, game_time: u64) -> i32 {
    i32::try_from(trigger_tick.saturating_sub(game_time)).unwrap_or(i32::MAX)
}

fn block_change_affects_light(old_block: RawBlockId, new_block: RawBlockId) -> bool {
    // Java Level.setBlock / ProtoChunk#setBlockState call checkBlock when light
    // opacity or emission changes; shape occlusion parity is still block-model work.
    block_light_opacity(old_block) != block_light_opacity(new_block)
        || block_light_emission(old_block) != block_light_emission(new_block)
}

fn feature_job_positions(
    targets: &[ChunkPos],
    priority_centers: &[ChunkPos],
) -> (Vec<ChunkPos>, Vec<ChunkPos>, Vec<ChunkPos>) {
    let target_chunks = targets.iter().copied().collect::<BTreeSet<_>>();
    let mut feature_centers = BTreeSet::new();
    let mut dependency_chunks = BTreeSet::new();

    for target in &target_chunks {
        for dz in -FEATURES_WRITE_RADIUS_CUTOFF..=FEATURES_WRITE_RADIUS_CUTOFF {
            for dx in -FEATURES_WRITE_RADIUS_CUTOFF..=FEATURES_WRITE_RADIUS_CUTOFF {
                feature_centers.insert(ChunkPos::new(target.x + dx, target.z + dz));
            }
        }
    }

    for center in &feature_centers {
        for dz in -FEATURES_BLOCK_DEPENDENCY_RADIUS..=FEATURES_BLOCK_DEPENDENCY_RADIUS {
            for dx in -FEATURES_BLOCK_DEPENDENCY_RADIUS..=FEATURES_BLOCK_DEPENDENCY_RADIUS {
                dependency_chunks.insert(ChunkPos::new(center.x + dx, center.z + dz));
            }
        }
    }

    (
        sorted_chunk_positions_by_priority(target_chunks, priority_centers),
        sorted_chunk_positions_by_priority(feature_centers, priority_centers),
        sorted_chunk_positions_by_priority(dependency_chunks, priority_centers),
    )
}

fn sorted_chunk_positions_by_priority(
    positions: BTreeSet<ChunkPos>,
    priority_centers: &[ChunkPos],
) -> Vec<ChunkPos> {
    if priority_centers.is_empty() {
        return sorted_chunk_positions_z_major(positions);
    }

    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by_key(|pos| chunk_priority_key(*pos, priority_centers));
    positions
}

fn light_statuses_sorted_by_priority(
    mut statuses: Vec<PendingLightStatus>,
    priority_centers: &[ChunkPos],
) -> Vec<PendingLightStatus> {
    if !priority_centers.is_empty() {
        statuses.sort_by_key(|status| chunk_priority_key(status.pos, priority_centers));
    }
    statuses
}

fn chunk_priority_key(pos: ChunkPos, priority_centers: &[ChunkPos]) -> (i64, i64, i32, i32) {
    let (chebyshev_distance, manhattan_distance) = priority_centers
        .iter()
        .map(|center| {
            let dx = (i64::from(pos.x) - i64::from(center.x)).abs();
            let dz = (i64::from(pos.z) - i64::from(center.z)).abs();
            (dx.max(dz), dx + dz)
        })
        .min()
        .expect("priority key requires at least one center");
    (chebyshev_distance, manhattan_distance, pos.z, pos.x)
}

fn sorted_chunk_positions_z_major(positions: BTreeSet<ChunkPos>) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by_key(|pos| (pos.z, pos.x));
    positions
}

fn dedupe_chunk_positions_preserving_order(chunks: Vec<ChunkPos>) -> Vec<ChunkPos> {
    let mut seen = BTreeSet::new();
    chunks.into_iter().filter(|pos| seen.insert(*pos)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light_layer_with_value(index: usize, value: u8) -> Vec<u8> {
        debug_assert!(value <= 15);
        let mut layer = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
        layer[index >> 1] |= value << (4 * (index & 1));
        layer
    }

    fn scheduler_with_snapshot(snapshot: ChunkSnapshot) -> ChunkScheduler {
        let pos = snapshot.pos;
        let mut holder = ChunkHolder::new(pos);
        holder.publish_snapshot(snapshot, ChunkResidency::Generated, false);

        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler.holders.insert(pos, holder);
        scheduler
    }

    fn test_feature_snapshot(pos: ChunkPos) -> ChunkSnapshot {
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Features,
            ChunkRevision(1),
            0,
            16,
            &vec![BlockStateId(0); CHUNK_SECTION_VOLUME],
        )
    }

    fn test_pending_light_status(pos: ChunkPos) -> PendingLightStatus {
        PendingLightStatus::from_parts(pos, test_feature_snapshot(pos), Vec::new(), Vec::new())
    }

    fn test_completed_light_status(pos: ChunkPos) -> CompletedLightStatus {
        CompletedLightStatus {
            pos,
            feature_snapshot: test_feature_snapshot(pos),
            scheduled_block_ticks: Vec::new(),
            scheduled_fluid_ticks: Vec::new(),
            light_sections: Vec::new(),
            batch_compute_leader: false,
            compute_us: 0,
            timing: LevelLightComputationTiming::default(),
        }
    }

    fn test_scheduled_light_holder(pos: ChunkPos) -> ChunkHolder {
        let mut holder = ChunkHolder::new(pos);
        holder.mark_scheduled(ChunkStatus::Light);
        holder
    }

    fn square(center: ChunkPos, radius: i32) -> BTreeSet<ChunkPos> {
        (-radius..=radius)
            .flat_map(|z| {
                (-radius..=radius).map(move |x| ChunkPos::new(center.x + x, center.z + z))
            })
            .collect()
    }

    #[test]
    fn publication_elapsed_cost_estimate_can_exceed_cold_unit_cap() {
        assert_eq!(
            publication_max_units_from_elapsed_cost(1, 4, 10_000, Some(250)),
            40
        );
    }

    #[test]
    fn publication_elapsed_cost_keeps_cold_unit_cap_without_estimate() {
        assert_eq!(
            publication_max_units_from_elapsed_cost(1, 4, 10_000, None),
            4
        );
        assert_eq!(
            publication_max_units_from_elapsed_cost(1, 4, 10_000, Some(0)),
            4
        );
    }

    #[test]
    fn publication_elapsed_cost_floor_honors_min_units() {
        assert_eq!(
            publication_max_units_from_elapsed_cost(1, 4, 1_000, Some(2_500)),
            1
        );
        assert_eq!(
            publication_max_units_from_elapsed_cost(3, 1, 1_000, Some(2_500)),
            3
        );
    }

    #[test]
    fn priority_sort_keeps_z_major_without_centers() {
        assert_eq!(
            sorted_chunk_positions_by_priority(square(ChunkPos::new(0, 0), 1), &[]),
            (-1..=1)
                .flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn priority_sort_orders_chunks_from_view_center_outward() {
        let sorted = sorted_chunk_positions_by_priority(
            square(ChunkPos::new(0, 0), 2),
            &[ChunkPos::new(0, 0)],
        );

        assert_eq!(sorted.first(), Some(&ChunkPos::new(0, 0)));
        let first_ring = sorted.iter().take(9).copied().collect::<BTreeSet<_>>();
        assert_eq!(first_ring, square(ChunkPos::new(0, 0), 1));
        assert!(
            sorted
                .iter()
                .position(|pos| *pos == ChunkPos::new(1, 1))
                .unwrap()
                < sorted
                    .iter()
                    .position(|pos| *pos == ChunkPos::new(2, 2))
                    .unwrap()
        );
    }

    #[test]
    fn feature_job_positions_keep_target_and_dependency_lists_center_first() {
        let targets = square(ChunkPos::new(5, -3), 1)
            .into_iter()
            .collect::<Vec<_>>();

        let (target_chunks, feature_centers, dependency_chunks) =
            feature_job_positions(&targets, &[ChunkPos::new(5, -3)]);

        assert_eq!(target_chunks.first(), Some(&ChunkPos::new(5, -3)));
        assert_eq!(feature_centers.first(), Some(&ChunkPos::new(5, -3)));
        assert_eq!(dependency_chunks.first(), Some(&ChunkPos::new(5, -3)));
    }

    #[test]
    fn light_status_batch_sort_orders_chunks_from_view_center_outward() {
        let statuses = vec![
            test_pending_light_status(ChunkPos::new(4, 0)),
            test_pending_light_status(ChunkPos::new(0, 0)),
            test_pending_light_status(ChunkPos::new(1, 1)),
        ];

        let sorted = light_statuses_sorted_by_priority(statuses, &[ChunkPos::new(0, 0)])
            .into_iter()
            .map(|status| status.pos)
            .collect::<Vec<_>>();

        assert_eq!(
            sorted,
            vec![
                ChunkPos::new(0, 0),
                ChunkPos::new(1, 1),
                ChunkPos::new(4, 0),
            ]
        );
    }

    #[test]
    fn light_publication_backlog_prioritizes_current_view_center() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let center = ChunkPos::new(0, 0);
        let far = ChunkPos::new(8, 0);
        scheduler
            .distance_manager
            .set_aggregate_player_ticket_positions_with_priority(BTreeSet::new(), vec![center]);
        scheduler
            .holders
            .insert(center, test_scheduled_light_holder(center));
        scheduler
            .holders
            .insert(far, test_scheduled_light_holder(far));
        scheduler
            .pending_light_publications
            .push_back(test_completed_light_status(far));
        scheduler
            .pending_light_publications
            .push_back(test_completed_light_status(center));

        let mut diagnostics = ChunkSchedulerPublicationDiagnostics::default();
        let events = scheduler
            .publish_pending_light_statuses(&mut diagnostics, PublicationGrant::fixed(2))
            .unwrap();
        let ready_positions = events
            .iter()
            .filter_map(|event| match event {
                ChunkSchedulerEvent::StatusChanged {
                    pos,
                    status: ChunkStatus::Light,
                    step: ChunkStatusStep::Ready,
                } => Some(*pos),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(ready_positions, vec![center, far]);
        assert_eq!(diagnostics.light_statuses_published, 2);
    }

    #[test]
    fn raw_high_render_distance_feature_job_shape_is_whole_view_sized() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let targets = square(ChunkPos::new(0, 0), 33)
            .into_iter()
            .collect::<Vec<_>>();

        let (job_id, _) = scheduler.create_feature_job(&targets, &[ChunkPos::new(0, 0)]);

        let metrics = scheduler.metrics();
        assert_eq!(metrics.pending_jobs, 1);
        assert_eq!(metrics.latest_feature_job_id, Some(job_id));
        assert_eq!(
            metrics.latest_feature_job_first_target,
            Some(ChunkPos::new(0, 0))
        );
        assert_eq!(metrics.latest_feature_job_target_chunks, 67 * 67);
        assert_eq!(metrics.latest_feature_job_feature_centers, 69 * 69);
        assert_eq!(metrics.latest_feature_job_dependency_chunks, 71 * 71);
        assert_eq!(
            metrics.max_feature_job_target_chunks,
            metrics.latest_feature_job_target_chunks
        );
        assert_eq!(
            metrics.max_feature_job_dependency_chunks,
            metrics.latest_feature_job_dependency_chunks
        );
    }

    #[test]
    fn high_render_distance_startup_feature_job_should_be_bounded_to_center_gate() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let center = ChunkPos::new(0, 0);
        let targets = sorted_chunk_positions_by_priority(square(center, 33), &[center]);

        scheduler
            .stored_chunk_misses
            .extend(targets.iter().copied());
        scheduler
            .enqueue_runtime_chunks(targets.clone(), ChunkStatus::Light, &[center])
            .unwrap();

        let metrics = scheduler.metrics();
        assert_eq!(metrics.pending_jobs, 1);
        assert_eq!(scheduler.job_count(), 1);
        assert_eq!(metrics.latest_feature_job_first_target, Some(center));
        assert_eq!(
            metrics.latest_feature_job_target_chunks,
            STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT
        );
        assert_eq!(metrics.latest_feature_job_feature_centers, 5 * 5);
        assert_eq!(metrics.latest_feature_job_dependency_chunks, 7 * 7);
        assert_eq!(
            scheduler.stored_chunk_misses.len(),
            (67 * 67) - STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT
        );

        scheduler
            .enqueue_runtime_chunks(targets, ChunkStatus::Light, &[center])
            .unwrap();
        assert_eq!(
            scheduler.job_count(),
            1,
            "a second feature job should not be queued before the first one completes"
        );
    }

    #[test]
    fn adaptive_publication_marks_pipeline_complete_and_admits_followup_job() {
        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler.set_lighting_enabled(false);
        scheduler.set_publication_budget_config(
            ChunkPublicationBudgetConfig::adaptive_for_gameplay_rate_hz(20),
        );
        let center = ChunkPos::new(0, 0);
        let targets = sorted_chunk_positions_by_priority(square(center, 33), &[center]);

        scheduler
            .stored_chunk_misses
            .extend(targets.iter().copied());
        scheduler
            .enqueue_runtime_chunks(targets, ChunkStatus::Features, &[center])
            .unwrap();
        let first_job_id = scheduler.metrics().latest_feature_job_id.unwrap();
        assert!(scheduler.wait_for_worldgen_completion(std::time::Duration::from_secs(30)));

        let report = scheduler.tick_report().unwrap();

        assert!(report.publication.adaptive_budget_enabled);
        assert_eq!(report.publication.completed_feature_jobs_drained, 1);
        assert_eq!(report.publication.feature_jobs_pipeline_completed, 1);
        assert_eq!(report.publication.feature_publish_budget_min_units, 1);
        assert_eq!(report.publication.feature_publish_budget_max_units, 1);
        assert_eq!(report.publication.feature_publish_spent_units, 1);
        assert_eq!(
            report.publication.pending_worldgen_publication_chunk_limit,
            DEFAULT_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT
        );
        assert_eq!(
            scheduler.job(first_job_id).map(|job| job.state),
            Some(ChunkJobState::Complete)
        );
        assert!(
            scheduler
                .jobs()
                .any(|job| job.id != first_job_id && job.state == ChunkJobState::Running),
            "adaptive publication should admit the next feature job while the first publication drains"
        );
        assert!(scheduler.pending_publication_count() > 0);
    }

    #[test]
    fn high_render_distance_initial_persistence_loads_are_center_bounded() {
        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler.set_lighting_enabled(false);
        let center = ChunkPos::new(0, 0);

        scheduler
            .apply_interest(ChunkView {
                center,
                render_distance: 33,
                chunk_tracking_radius: 33,
            })
            .unwrap();

        assert_eq!(
            scheduler.pending_persistence_load_count(),
            STARTUP_CHUNK_LOAD_REQUEST_LIMIT
        );
        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.metrics().latest_feature_job_id, None);

        scheduler.poll().unwrap();

        let metrics = scheduler.metrics();
        assert_eq!(metrics.pending_jobs, 1);
        assert_eq!(
            metrics.latest_feature_job_target_chunks,
            STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT
        );
        assert_eq!(metrics.latest_feature_job_first_target, Some(center));
        assert!(scheduler.pending_persistence_load_count() <= BACKGROUND_CHUNK_LOAD_REQUEST_LIMIT);
    }

    #[test]
    fn high_render_distance_followup_feature_job_should_use_background_limit() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let center = ChunkPos::new(0, 0);
        let targets = sorted_chunk_positions_by_priority(square(center, 33), &[center]);

        scheduler
            .stored_chunk_misses
            .extend(targets.iter().copied());
        scheduler
            .enqueue_runtime_chunks(targets, ChunkStatus::Light, &[center])
            .unwrap();
        let first_job_id = scheduler.metrics().latest_feature_job_id.unwrap();
        scheduler.mark_job_complete(
            first_job_id,
            OverworldFeatureDependencyCacheReport::default(),
            OverworldFeatureBatchTiming::default(),
        );

        scheduler.enqueue_next_pending_feature_job();

        let metrics = scheduler.metrics();
        assert_eq!(scheduler.job_count(), 2);
        assert_ne!(metrics.latest_feature_job_id, Some(first_job_id));
        assert_eq!(
            metrics.latest_feature_job_target_chunks,
            BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT
        );
        assert_eq!(
            scheduler.stored_chunk_misses.len(),
            (67 * 67)
                - STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT
                - BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT
        );
    }

    #[test]
    fn raw_brightness_at_world_uses_light_correct_snapshot_layers() {
        let pos = WorldBlockPos::new(2, 4, 3);
        let index = chunk_section_index(2, 4, 3);
        let blocks = vec![BlockStateId(0); CHUNK_SECTION_VOLUME];
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(1),
            0,
            16,
            &blocks,
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                0,
                Some(light_layer_with_value(index, 12)),
                Some(light_layer_with_value(index, 5)),
            )],
        );
        let scheduler = scheduler_with_snapshot(snapshot);

        assert_eq!(scheduler.raw_brightness_at_world(pos, 0), Some(12));
        assert_eq!(scheduler.raw_brightness_at_world(pos, 8), Some(5));
    }

    #[test]
    fn raw_brightness_at_world_has_no_fullbright_fallback_for_missing_light_payload() {
        let blocks = vec![BlockStateId(0); CHUNK_SECTION_VOLUME];
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(1),
            0,
            16,
            &blocks,
        )
        .with_light_sections(true, Vec::new());
        let scheduler = scheduler_with_snapshot(snapshot);

        assert_eq!(
            scheduler.raw_brightness_at_world(WorldBlockPos::new(2, 4, 3), 0),
            None
        );
    }

    #[test]
    fn raw_brightness_at_world_requires_light_status_and_light_correct_data() {
        let pos = WorldBlockPos::new(2, 4, 3);
        let index = chunk_section_index(2, 4, 3);
        let blocks = vec![BlockStateId(0); CHUNK_SECTION_VOLUME];
        let light_sections = vec![PackedLightSection::new(
            0,
            Some(light_layer_with_value(index, 15)),
            None,
        )];
        let features_snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkRevision(1),
            0,
            16,
            &blocks,
        )
        .with_light_sections(true, light_sections.clone());
        let untrusted_light_snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(2),
            0,
            16,
            &blocks,
        )
        .with_light_sections(false, light_sections.clone());
        let light_snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(3),
            0,
            16,
            &blocks,
        )
        .with_light_sections(true, light_sections);

        assert_eq!(
            scheduler_with_snapshot(features_snapshot).raw_brightness_at_world(pos, 0),
            None
        );
        assert_eq!(
            scheduler_with_snapshot(untrusted_light_snapshot).raw_brightness_at_world(pos, 0),
            None
        );
        let scheduler = scheduler_with_snapshot(light_snapshot);
        assert_eq!(scheduler.raw_brightness_at_world(pos, 0), Some(15));
        assert_eq!(
            scheduler.raw_brightness_at_world(WorldBlockPos::new(2, 16, 3), 0),
            None
        );
    }

    #[test]
    fn raw_brightness_at_world_uses_next_sky_section() {
        let bottom_pos = WorldBlockPos::new(2, 4, 3);
        let top_pos = WorldBlockPos::new(2, 20, 3);
        let index = chunk_section_index(2, 4, 3);
        let blocks = vec![BlockStateId(0); CHUNK_SECTION_VOLUME * 2];
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(1),
            0,
            32,
            &blocks,
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                1,
                Some(light_layer_with_value(index, 11)),
                None,
            )],
        );
        let scheduler = scheduler_with_snapshot(snapshot);

        assert_eq!(scheduler.raw_brightness_at_world(bottom_pos, 0), Some(11));
        assert_eq!(scheduler.raw_brightness_at_world(top_pos, 0), Some(11));
    }

    #[test]
    fn raw_brightness_at_world_uses_top_sky_fallback_without_fullbright_snapshot_fallback() {
        let bottom_pos = WorldBlockPos::new(2, 4, 3);
        let top_pos = WorldBlockPos::new(2, 20, 3);
        let index = chunk_section_index(2, 4, 3);
        let blocks = vec![BlockStateId(0); CHUNK_SECTION_VOLUME * 2];
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(1),
            0,
            32,
            &blocks,
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                0,
                Some(light_layer_with_value(index, 11)),
                None,
            )],
        );
        let scheduler = scheduler_with_snapshot(snapshot);

        assert_eq!(scheduler.raw_brightness_at_world(bottom_pos, 0), Some(11));
        assert_eq!(scheduler.raw_brightness_at_world(top_pos, 0), Some(15));
        assert_eq!(scheduler.raw_brightness_at_world(top_pos, 6), Some(9));
        assert_eq!(
            scheduler.raw_brightness_at_world(WorldBlockPos::new(2, -1, 3), 0),
            None
        );
        assert_eq!(
            scheduler.raw_brightness_at_world(WorldBlockPos::new(2, 32, 3), 0),
            None
        );
    }

    #[cfg(feature = "physics")]
    #[test]
    fn physics_terrain_section_reads_live_scheduler_blocks() {
        use mclone_worldgen::block::{DIRT, WATER};

        let chunk_pos = ChunkPos::new(2, -3);
        let mut live_blocks = MutableChunkBlockBuffer::new(chunk_pos.x, chunk_pos.z, -16, 64);
        live_blocks.set_block_at_y(0, 0, 0, STONE);
        live_blocks.set_block_at_y(1, 0, 0, WATER);
        live_blocks.set_block_at_y(2, 0, 0, DIRT);

        let mut holder = ChunkHolder::new(chunk_pos);
        holder.live_blocks = Some(live_blocks);

        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler.holders.insert(chunk_pos, holder);

        let section = scheduler
            .physics_terrain_section_at_block(WorldBlockPos::new(32, 0, -48))
            .expect("live section terrain");

        assert_eq!(section.origin, mclone_core::Vec3d::new(32.0, 0.0, -48.0));
        assert_eq!(section.solid_cell_count(), 2);
        assert!(section.is_solid(0, 0, 0));
        assert!(!section.is_solid(1, 0, 0));
        assert!(section.is_solid(2, 0, 0));
        assert!(scheduler.physics_terrain_section(chunk_pos, -2).is_none());
        assert!(
            scheduler
                .physics_terrain_section(ChunkPos::new(99, 99), 0)
                .is_none()
        );
    }
}

fn status_path_to(target_status: ChunkStatus) -> Vec<ChunkStatus> {
    ALL_STATUSES
        .iter()
        .copied()
        .take_while(|status| *status <= target_status)
        .collect()
}

pub(crate) const DEFAULT_PENDING_UNLOAD_BUDGET: usize = 200;
pub(crate) const DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET: usize = 1;
pub(crate) const DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET: usize = 1;
pub const DEFAULT_LIGHT_STATUS_BATCH_SIZE: usize = 9;
const STARTUP_CHUNK_LOAD_REQUEST_LIMIT: usize = 9;
const BACKGROUND_CHUNK_LOAD_REQUEST_LIMIT: usize = 128;
const STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT: usize = 9;
const BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT: usize = 128;
const DEFAULT_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT: usize =
    STARTUP_FEATURE_JOB_TARGET_CHUNK_LIMIT + BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT;
const MAX_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT: usize =
    BACKGROUND_FEATURE_JOB_TARGET_CHUNK_LIMIT * 2;
const MAX_SCHEDULED_FLUID_TICKS_PER_TICK: usize = 65_536;

fn chunk_publication_budget_controller_config() -> BudgetControllerConfig {
    let scheduler_publication = BudgetDecisionAddress::new(
        FrameHostKind::IntegratedServerRunner,
        WorkWindow::GameplayTick,
        StageId::SchedulerPublication,
    );
    BudgetControllerConfig {
        raise_after_clean_windows: 3,
        over_period_pct_cut_threshold: 0.0,
        queue_age_limit_ms: None,
        queue_depth_limit: None,
        families: vec![
            FamilyBudgetConfig::new(
                BudgetDecisionFamily::FeaturePublication,
                scheduler_publication,
                DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET as u32,
                4,
                0.02,
                0.20,
            ),
            FamilyBudgetConfig::new(
                BudgetDecisionFamily::LightPublication,
                scheduler_publication,
                DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET as u32,
                4,
                0.02,
                0.20,
            ),
            FamilyBudgetConfig::new(
                BudgetDecisionFamily::FeatureJobAdmission,
                BudgetDecisionAddress::new(
                    FrameHostKind::IntegratedServerRunner,
                    WorkWindow::GameplayTick,
                    StageId::TerrainGeneration,
                ),
                1,
                1,
                0.0,
                0.0,
            )
            .with_pending_units(
                DEFAULT_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT as u32,
                MAX_PENDING_WORLDGEN_PUBLICATION_CHUNK_LIMIT as u32,
            ),
        ],
    }
}

fn decision_for(
    decisions: &[BudgetDecisionReport],
    family: BudgetDecisionFamily,
) -> Option<&BudgetDecisionReport> {
    decisions.iter().find(|decision| decision.family == family)
}

fn elapsed_ms_to_us(ms: f64) -> u128 {
    if !ms.is_finite() || ms <= 0.0 {
        return 0;
    }
    (ms * 1_000.0).round() as u128
}

fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1_000.0
}

fn snapshot_is_client_ready_for_lighting_mode(
    snapshot: &ChunkSnapshot,
    lighting_enabled: bool,
) -> bool {
    if lighting_enabled {
        snapshot.status >= ChunkStatus::Light && snapshot.light_correct
    } else {
        snapshot.status >= ChunkStatus::Features
    }
}

fn block_light_at_strict(
    light_sections: &[PackedLightSection],
    section_y: i32,
    index: usize,
) -> u8 {
    light_sections
        .iter()
        .find(|section| section.section_y == section_y)
        .and_then(|section| section.block.as_deref())
        .map_or(0, |layer| light_data_layer_value(layer, index))
}

fn sky_light_at_strict(light_sections: &[PackedLightSection], section_y: i32, index: usize) -> u8 {
    let any_sky_layer = light_sections.iter().any(|section| section.sky.is_some());
    let mut next_sky_layer = None;
    for section in light_sections {
        if section.section_y < section_y {
            continue;
        }
        let Some(layer) = section.sky.as_deref() else {
            continue;
        };
        if section.section_y == section_y {
            return light_data_layer_value(layer, index);
        }
        if next_sky_layer.is_none_or(|(next_section_y, _)| section.section_y < next_section_y) {
            next_sky_layer = Some((section.section_y, layer));
        }
    }

    if any_sky_layer {
        next_sky_layer
            .map(|(_, layer)| light_data_layer_value(layer, index))
            .unwrap_or(15)
    } else {
        0
    }
}

fn light_data_layer_value(layer: &[u8], index: usize) -> u8 {
    debug_assert_eq!(layer.len(), LIGHT_DATA_LAYER_BYTE_COUNT);
    let byte = layer[index >> 1];
    let shift = 4 * (index & 1);
    (byte >> shift) & 15
}

fn section_block_update_from_index(
    local_index: usize,
    block_state: BlockStateId,
) -> SectionBlockUpdate {
    debug_assert!(local_index < CHUNK_SECTION_VOLUME);
    SectionBlockUpdate {
        local_x: (local_index & 0xF) as u8,
        local_y: ((local_index >> 8) & 0xF) as u8,
        local_z: ((local_index >> 4) & 0xF) as u8,
        block_state,
    }
}
fn cached_block_and_fluid(
    scheduler: &ChunkScheduler,
    key: i32,
    pos: WorldBlockPos,
    cache: &mut BTreeMap<i32, (Option<RawBlockId>, NativeFluidState)>,
) -> (Option<RawBlockId>, NativeFluidState) {
    *cache.entry(key).or_insert_with(|| {
        let block = scheduler.block_at_world(pos);
        let fluid = block
            .map(NativeFluidState::from_block_id)
            .unwrap_or(NativeFluidState::EMPTY);
        (block, fluid)
    })
}

fn record_scheduled_fluid_tick(
    report: &mut FluidMutationReport,
    pos: WorldBlockPos,
    fluid: FluidKind,
) {
    record_scheduled_fluid_tick_request(&mut report.scheduled_ticks, pos, fluid);
}

fn record_scheduled_fluid_tick_request(
    requests: &mut Vec<ScheduledFluidTickRequest>,
    pos: WorldBlockPos,
    fluid: FluidKind,
) {
    let request = ScheduledFluidTickRequest {
        pos,
        fluid,
        delay: fluid.tick_delay(),
    };
    if !requests.contains(&request) {
        requests.push(request);
    }
}
fn ticket_level_dependency_status_target(_ticket_level: i32) -> ChunkStatus {
    ChunkStatus::Surface
}

const ALL_STATUSES: [ChunkStatus; 5] = [
    ChunkStatus::Terrain,
    ChunkStatus::Surface,
    ChunkStatus::Features,
    ChunkStatus::Light,
    ChunkStatus::Full,
];
