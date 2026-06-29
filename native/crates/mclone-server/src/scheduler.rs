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
    PackedLightSection, block_to_section_coord, chunk_section_index, local_block_coord,
    local_section_block_coord,
};
use mclone_protocol::{ChunkView, SectionBlockUpdate};
use mclone_worldgen::block::{
    OBSIDIAN, RawBlockId, STONE, block_light_emission, block_light_opacity,
    generated_block_state_id, is_water, material_blocks_motion,
};
use mclone_worldgen::feature::{FEATURES_CHUNK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF};
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
};
use crate::persistence::{ChunkSnapshotStore, ChunkStoreResult, NullChunkSnapshotStore};
use crate::player_chunk_tracking::chunk_positions_for_view;
use crate::timing::{
    ChunkSchedulerTickReport, ChunkSchedulerTickTiming, simulation_timing_elapsed_us,
    simulation_timing_start,
};
use crate::worldgen_mailbox::{PendingWorldgenPublication, WorldgenMailbox};
use crate::{
    CHUNK_LEVEL_FULL, ChunkJobId, ChunkJobState, ChunkResidency, ChunkStatusStep, ChunkTicketKey,
    ChunkTicketType, FORCED_TICKET_LEVEL, FluidKind, FullChunkStatus, LightStatusMailboxKind,
    MAX_CHUNK_DISTANCE, UNLOADED_CHUNK_LEVEL, WorkerFrameMetrics, WorldBlockPos,
    WorldgenMailboxKind, full_chunk_status_for_ticket_level,
};

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
    pub total_light_status_sky_source_enqueue_us: u128,
    pub total_light_status_block_source_enqueue_us: u128,
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
    pub total_light_status_collect_sections_us: u128,
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

    pub(crate) fn tick(
        &mut self,
        game_time: u64,
        entity_ticking_chunks: &[ChunkPos],
        scheduler: &mut ChunkScheduler,
    ) -> (FluidTickPhaseReport, Vec<ChunkSchedulerEvent>) {
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
            snapshot_events += mutation.snapshot_events;
            set_block_us += mutation.set_block_us;
            for scheduled in mutation.scheduled_ticks {
                self.schedule_tick(scheduled.pos, scheduled.fluid, scheduled.delay, game_time);
            }
            events.extend(mutation.events);
        }
        let event_count = events.len();

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
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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

#[derive(Debug)]
pub struct ChunkScheduler {
    seed: i64,
    lighting_enabled: bool,
    holders: BTreeMap<ChunkPos, ChunkHolder>,
    pending_unloads: BTreeSet<ChunkPos>,
    pub(crate) distance_manager: ChunkDistanceManager,
    jobs: BTreeMap<ChunkJobId, ChunkStatusJob>,
    job_timings: BTreeMap<ChunkJobId, OverworldFeatureBatchTiming>,
    worldgen_mailbox: WorldgenMailbox,
    pending_worldgen_publications: VecDeque<PendingWorldgenPublication>,
    light_mailbox: LightStatusMailbox,
    pending_light_status_batches: BTreeMap<ChunkJobId, Vec<PendingLightStatus>>,
    pending_light_publications: VecDeque<CompletedLightStatus>,
    completed_light_statuses: usize,
    completed_light_batches: usize,
    total_light_status_compute_us: u128,
    max_light_status_compute_us: u128,
    light_status_timing: LevelLightComputationTiming,
    pending_block_deltas: BTreeMap<(ChunkPos, i32), BTreeMap<usize, BlockStateId>>,
    pending_runtime_light_snapshots: VecDeque<ChunkSnapshot>,
    dirty_chunks: BTreeSet<ChunkPos>,
    next_job_id: u64,
    next_revision: u64,
    store: Box<dyn ChunkSnapshotStore>,
}

impl ChunkScheduler {
    pub fn new(seed: i64) -> Self {
        Self::with_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn with_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self {
            seed,
            lighting_enabled: true,
            holders: BTreeMap::new(),
            pending_unloads: BTreeSet::new(),
            distance_manager: ChunkDistanceManager::new(),
            jobs: BTreeMap::new(),
            job_timings: BTreeMap::new(),
            worldgen_mailbox: WorldgenMailbox::new(),
            pending_worldgen_publications: VecDeque::new(),
            light_mailbox: LightStatusMailbox::new(),
            pending_light_status_batches: BTreeMap::new(),
            pending_light_publications: VecDeque::new(),
            completed_light_statuses: 0,
            completed_light_batches: 0,
            total_light_status_compute_us: 0,
            max_light_status_compute_us: 0,
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
            holders: BTreeMap::new(),
            pending_unloads: BTreeSet::new(),
            distance_manager: ChunkDistanceManager::new(),
            jobs: BTreeMap::new(),
            job_timings: BTreeMap::new(),
            worldgen_mailbox: WorldgenMailbox::with_wasm_job_worker(config.clone()),
            pending_worldgen_publications: VecDeque::new(),
            light_mailbox: LightStatusMailbox::with_wasm_job_worker(config),
            pending_light_status_batches: BTreeMap::new(),
            pending_light_publications: VecDeque::new(),
            completed_light_statuses: 0,
            completed_light_batches: 0,
            total_light_status_compute_us: 0,
            max_light_status_compute_us: 0,
            light_status_timing: LevelLightComputationTiming::default(),
            pending_block_deltas: BTreeMap::new(),
            pending_runtime_light_snapshots: VecDeque::new(),
            dirty_chunks: BTreeSet::new(),
            next_job_id: 1,
            next_revision: 1,
            store,
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
        let mut events = self.publish_completed_worldgen_jobs()?;
        events.extend(self.publish_pending_light_statuses()?);
        Ok(events)
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
        let total_start = simulation_timing_start();
        let purge_start = simulation_timing_start();
        self.distance_manager.purge_stale_tickets();
        let purge_stale_tickets_us = simulation_timing_elapsed_us(purge_start);

        let reconcile_start = simulation_timing_start();
        let mut events = self.reconcile_ticketed_holders()?;
        let reconcile_holders_us = simulation_timing_elapsed_us(reconcile_start);

        let publish_start = simulation_timing_start();
        events.extend(self.poll()?);
        let publish_completed_us = simulation_timing_elapsed_us(publish_start);

        let pending_unload_start = simulation_timing_start();
        let pending_unloads_processed =
            self.process_pending_unloads(DEFAULT_PENDING_UNLOAD_BUDGET)?;
        let pending_unload_us = simulation_timing_elapsed_us(pending_unload_start);
        Ok(ChunkSchedulerTickReport {
            ticket_tick: self.ticket_tick(),
            block_ticking_chunks: self.block_ticking_chunks(),
            entity_ticking_chunks: self.entity_ticking_chunks(),
            pending_unloads_processed,
            events,
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
        let mut playable_chunk_ready = false;
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
                if playable {
                    playable_chunk_ready = target_ready;
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
            + self.pending_light_publications.len()
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

    pub fn metrics(&self) -> ChunkSchedulerMetrics {
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
            total_light_status_sky_source_enqueue_us: self
                .light_status_timing
                .sky_source_enqueue_us,
            total_light_status_block_source_enqueue_us: self
                .light_status_timing
                .block_source_enqueue_us,
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
            total_light_status_collect_sections_us: self.light_status_timing.collect_sections_us,
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
        let dirty_chunks = self.dirty_chunks.iter().copied().collect::<Vec<_>>();
        let mut saved = 0;
        for pos in dirty_chunks {
            let Some(holder) = self.holders.get(&pos).cloned() else {
                self.dirty_chunks.remove(&pos);
                continue;
            };
            if holder.dirty {
                self.save_holder(&holder)?;
                if let Some(current) = self.holders.get_mut(&pos) {
                    current.mark_saved();
                }
                saved += 1;
            }
            self.dirty_chunks.remove(&pos);
        }
        Ok(saved)
    }

    pub fn process_pending_unloads(&mut self, max_chunks: usize) -> ChunkStoreResult<usize> {
        if max_chunks == 0 || self.pending_unloads.is_empty() {
            return Ok(0);
        }

        let active_levels = self.distance_manager.active_levels();
        let candidates = self
            .pending_unloads
            .iter()
            .copied()
            .take(max_chunks)
            .collect::<Vec<_>>();
        let mut processed = 0;

        for pos in candidates {
            if active_levels.contains_key(&pos) {
                self.pending_unloads.remove(&pos);
                continue;
            }

            let Some(holder) = self.holders.get(&pos).cloned() else {
                self.pending_unloads.remove(&pos);
                self.dirty_chunks.remove(&pos);
                continue;
            };

            if holder.dirty {
                self.save_holder(&holder)?;
                self.dirty_chunks.remove(&pos);
            }

            self.holders.remove(&pos);
            self.pending_unloads.remove(&pos);
            processed += 1;
        }

        Ok(processed)
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
            &client_visible_set,
            self.runtime_chunk_target_status(),
            &priority_centers,
        )?);
        Ok(events)
    }

    fn enqueue_runtime_chunks(
        &mut self,
        desired_chunks: Vec<ChunkPos>,
        client_visible_set: &BTreeSet<ChunkPos>,
        target_status: ChunkStatus,
        priority_centers: &[ChunkPos],
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let mut events = Vec::new();
        let mut to_generate = Vec::new();

        for pos in desired_chunks {
            self.holders
                .entry(pos)
                .or_insert_with(|| ChunkHolder::new(pos))
                .target_status = Some(target_status);

            for status in status_path_to(target_status) {
                if self
                    .holders
                    .get(&pos)
                    .expect("holder must exist before scheduling")
                    .status_slot(status)
                    .is_some()
                {
                    continue;
                }

                {
                    let holder = self
                        .holders
                        .get_mut(&pos)
                        .expect("holder must exist before scheduling");
                    holder.mark_scheduled(status);
                }
                events.push(ChunkSchedulerEvent::StatusChanged {
                    pos,
                    status,
                    step: ChunkStatusStep::Scheduled,
                });

                if status == ChunkStatus::Features {
                    if let Some(snapshot) = self.load_stored_snapshot(pos, target_status)? {
                        let snapshot = if self.lighting_enabled {
                            hydrate_loaded_light_snapshot(snapshot)?
                        } else {
                            snapshot
                        };
                        let revision = snapshot.revision;
                        {
                            let holder = self
                                .holders
                                .get_mut(&pos)
                                .expect("holder must exist before marking features ready");
                            holder.mark_ready(ChunkStatus::Features, Some(revision));
                        }
                        events.push(ChunkSchedulerEvent::StatusChanged {
                            pos,
                            status,
                            step: ChunkStatusStep::Ready,
                        });
                        if target_status >= ChunkStatus::Light
                            && self
                                .holders
                                .get(&pos)
                                .expect("holder must exist before scheduling light")
                                .status_slot(ChunkStatus::Light)
                                .is_none()
                        {
                            self.holders
                                .get_mut(&pos)
                                .expect("holder must exist before marking light scheduled")
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
                        if client_visible_set.contains(&pos)
                            && self.snapshot_is_client_ready(&snapshot)
                        {
                            self.holders
                                .get_mut(&pos)
                                .expect("holder must exist before marking visible")
                                .set_client_visible(true);
                            events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
                        }
                    } else {
                        to_generate.push(pos);
                    }
                } else if status < ChunkStatus::Features {
                    let holder = self
                        .holders
                        .get_mut(&pos)
                        .expect("holder must exist before marking ready");
                    holder.mark_ready(status, None);
                    events.push(ChunkSchedulerEvent::StatusChanged {
                        pos,
                        status,
                        step: ChunkStatusStep::Ready,
                    });
                }
            }
        }

        if !to_generate.is_empty() {
            let (job_id, seeded_dependencies) =
                self.create_feature_job(&to_generate, priority_centers);
            for pos in &to_generate {
                self.holders
                    .get_mut(pos)
                    .expect("holder must exist before assigning job")
                    .assign_status_job(ChunkStatus::Features, job_id);
            }

            self.mark_job_state(job_id, ChunkJobState::Running);
            self.worldgen_mailbox.enqueue_features(
                job_id,
                self.seed,
                &to_generate,
                seeded_dependencies,
            );
        }

        Ok(events)
    }

    fn publish_completed_worldgen_jobs(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.pending_worldgen_publications.extend(
            self.worldgen_mailbox
                .drain_completed()
                .into_iter()
                .map(PendingWorldgenPublication::new),
        );
        let mut events = Vec::new();
        let mut remaining_budget = DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET;

        while remaining_budget > 0 {
            let Some(publication) = self.pending_worldgen_publications.pop_front() else {
                break;
            };
            let (mut published, pending) =
                self.publish_completed_feature_job(publication, &mut remaining_budget)?;
            events.append(&mut published);
            if let Some(pending) = pending {
                self.pending_worldgen_publications.push_front(pending);
                break;
            }
        }

        Ok(events)
    }

    fn publish_completed_feature_job(
        &mut self,
        mut publication: PendingWorldgenPublication,
        remaining_budget: &mut usize,
    ) -> ChunkStoreResult<(Vec<ChunkSchedulerEvent>, Option<PendingWorldgenPublication>)> {
        let Some(job) = self.jobs.get(&publication.completed.job_id).cloned() else {
            return Ok((Vec::new(), None));
        };

        let mut events = Vec::new();
        let generated = &publication.completed.generated_chunks;
        let retained_dependencies = &publication.completed.retained_dependencies;

        while publication.next_target_index < job.target_chunks.len() && *remaining_budget > 0 {
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
                continue;
            }

            let revision = ChunkRevision(self.next_revision);
            self.next_revision += 1;

            let chunk = generated.get(&pos).unwrap_or_else(|| {
                panic!(
                    "feature batch did not return scheduler target chunk ({}, {})",
                    pos.x, pos.z
                )
            });
            events.extend(fluid_tick_events_from_generated_chunk(&chunk));
            let snapshot = chunk.to_chunk_snapshot(revision, ChunkStatus::Features);
            self.mark_snapshot_ready(pos, snapshot.clone(), ChunkResidency::Generated, true);
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
                let ready_light_batch = {
                    let statuses = self
                        .pending_light_status_batches
                        .entry(publication.completed.job_id)
                        .or_default();
                    statuses.push(PendingLightStatus::from_feature_publication(
                        pos,
                        snapshot,
                        chunk,
                        generated.iter(),
                        retained_dependencies.iter(),
                    ));
                    (statuses.len() >= CENTER_PRIORITY_LIGHT_STATUS_BATCH_SIZE)
                        .then(|| std::mem::take(statuses))
                };
                if let Some(statuses) = ready_light_batch {
                    self.light_mailbox
                        .enqueue_batch(PendingLightStatusBatch::new(statuses));
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
        self.mark_job_complete(completed.job_id, completed.cache_report, completed.timing);
        if !pending_light_statuses.is_empty() {
            self.light_mailbox
                .enqueue_batch(PendingLightStatusBatch::new(pending_light_statuses));
        }
        Ok((events, None))
    }

    fn publish_pending_light_statuses(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.pending_light_publications
            .extend(self.light_mailbox.drain_completed());
        let mut events = Vec::new();
        let mut remaining_budget = DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET;
        while remaining_budget > 0 {
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
                continue;
            }

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

            self.mark_snapshot_ready(pos, snapshot.clone(), ChunkResidency::Generated, true);
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
                events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
            }
        }

        Ok(events)
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

    fn snapshot_is_client_ready(&self, snapshot: &ChunkSnapshot) -> bool {
        snapshot_is_client_ready_for_lighting_mode(snapshot, self.lighting_enabled)
    }

    fn load_stored_snapshot(
        &mut self,
        pos: ChunkPos,
        target_status: ChunkStatus,
    ) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        let Some(snapshot) = self.store.load_chunk(pos)? else {
            return Ok(None);
        };
        if snapshot.status < target_status {
            return Ok(None);
        }
        self.next_revision = self
            .next_revision
            .max(snapshot.revision.0.saturating_add(1));
        Ok(Some(snapshot))
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

    fn save_holder(&mut self, holder: &ChunkHolder) -> ChunkStoreResult<()> {
        if let Some(snapshot) = holder.snapshot() {
            self.store.save_chunk(snapshot)?;
        }
        Ok(())
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

fn fluid_tick_events_from_generated_chunk(chunk: &GeneratedChunk) -> Vec<ChunkSchedulerEvent> {
    chunk
        .liquid_ticks()
        .iter()
        .filter_map(fluid_tick_event_from_generated_tick)
        .collect()
}

fn fluid_tick_event_from_generated_tick(tick: &ScheduledTick) -> Option<ChunkSchedulerEvent> {
    Some(ChunkSchedulerEvent::FluidTickScheduled {
        pos: WorldBlockPos::new(tick.x, tick.y, tick.z),
        fluid: FluidKind::from_target(&tick.target)?,
        delay: tick.delay,
    })
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
        for dz in -FEATURES_CHUNK_DEPENDENCY_RADIUS..=FEATURES_CHUNK_DEPENDENCY_RADIUS {
            for dx in -FEATURES_CHUNK_DEPENDENCY_RADIUS..=FEATURES_CHUNK_DEPENDENCY_RADIUS {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn square(center: ChunkPos, radius: i32) -> BTreeSet<ChunkPos> {
        (-radius..=radius)
            .flat_map(|z| {
                (-radius..=radius).map(move |x| ChunkPos::new(center.x + x, center.z + z))
            })
            .collect()
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
const CENTER_PRIORITY_LIGHT_STATUS_BATCH_SIZE: usize = 9;
const MAX_SCHEDULED_FLUID_TICKS_PER_TICK: usize = 65_536;

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
