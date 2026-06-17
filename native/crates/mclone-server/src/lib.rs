#![forbid(unsafe_code)]

mod persistence;
mod timing;
mod types;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
use std::{sync::mpsc, thread};

use mclone_core::{
    BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkRevision, ChunkSnapshot,
    ChunkStatus, PackedLightSection, SECTION_HEIGHT, block_to_section_coord, chunk_block_index,
    chunk_section_index, local_block_coord, local_section_block_coord,
};
use mclone_light::{DataLayer, LightLayer};
use mclone_protocol::{ChunkView, ClientCommand, SectionBlockUpdate, ServerUpdate};
#[cfg(test)]
use mclone_worldgen::block::{LAVA, WATER};
use mclone_worldgen::block::{
    OBSIDIAN, RawBlockId, STONE, fluid_level, generated_block_state_id, is_lava, is_water,
    material_blocks_motion,
};
use mclone_worldgen::feature::{FEATURES_CHUNK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF};
use mclone_worldgen::levelgen::{
    GeneratedChunk, MutableChunkBlockBuffer, OverworldFeatureBatchTiming,
    OverworldFeatureDependencyCache, OverworldFeatureDependencyCacheReport, ScheduledTick,
};
pub use persistence::{
    ChunkSnapshotStore, ChunkStoreError, ChunkStoreResult, NullChunkSnapshotStore,
};
pub use timing::{
    ChunkSchedulerTickReport, ChunkSchedulerTickTiming, ServerSimulationTickReport,
    ServerSimulationTickTiming, ServerTickReport, ServerTickTiming,
};
use timing::{simulation_timing_elapsed_us, simulation_timing_start};
pub use types::{
    CHUNK_LEVEL_FULL, ChunkJobId, ChunkJobState, ChunkResidency, ChunkStatusStep, ChunkTicket,
    ChunkTicketKey, ChunkTicketType, FORCED_TICKET_LEVEL, FluidKind, FullChunkStatus,
    MAX_CHUNK_DISTANCE, PLAYER_TICKET_LEVEL, ServerMode, UNLOADED_CHUNK_LEVEL, WorldBlockPos,
    WorldgenMailboxKind,
};

#[cfg(not(target_arch = "wasm32"))]
pub use persistence::FilesystemChunkSnapshotStore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeFluidState {
    kind: Option<FluidKind>,
    amount: u8,
    falling: bool,
    source: bool,
}

impl NativeFluidState {
    const EMPTY: Self = Self {
        kind: None,
        amount: 0,
        falling: false,
        source: false,
    };

    const fn source(kind: FluidKind) -> Self {
        Self {
            kind: Some(kind),
            amount: 8,
            falling: false,
            source: true,
        }
    }

    const fn flowing(kind: FluidKind, amount: u8, falling: bool) -> Self {
        Self {
            kind: Some(kind),
            amount,
            falling,
            source: false,
        }
    }

    fn from_block_id(block_id: RawBlockId) -> Self {
        let Some(kind) = FluidKind::from_block_id(block_id) else {
            return Self::EMPTY;
        };
        match fluid_level(block_id).unwrap_or(0) {
            0 => Self::source(kind),
            8 => Self::flowing(kind, 8, true),
            level => Self::flowing(kind, 8_u8.saturating_sub(level), false),
        }
    }

    const fn is_empty(self) -> bool {
        self.kind.is_none()
    }

    const fn is_same_fluid(self, fluid: FluidKind) -> bool {
        matches!(self.kind, Some(kind) if kind as u8 == fluid as u8)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FluidDirection {
    Down,
    Up,
    North,
    East,
    South,
    West,
}

impl FluidDirection {
    const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::East => (1, 0, 0),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
        }
    }

    const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

const HORIZONTAL_FLUID_DIRECTIONS: [FluidDirection; 4] = [
    FluidDirection::North,
    FluidDirection::East,
    FluidDirection::South,
    FluidDirection::West,
];

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
struct FluidTickList {
    scheduled_keys: BTreeSet<FluidTickKey>,
    scheduled_ticks: BTreeSet<ScheduledFluidTick>,
    next_sequence: u64,
}

impl FluidTickList {
    fn new() -> Self {
        Self::default()
    }

    fn schedule_tick(&mut self, pos: WorldBlockPos, fluid: FluidKind, delay: i32, game_time: u64) {
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

    fn size(&self) -> usize {
        self.scheduled_keys.len()
    }

    #[cfg(test)]
    fn has_scheduled_tick(&self, pos: WorldBlockPos, fluid: FluidKind) -> bool {
        self.scheduled_keys.contains(&FluidTickKey { pos, fluid })
    }

    #[cfg(test)]
    fn scheduled_tick_entries(&self, game_time: u64) -> Vec<(WorldBlockPos, FluidKind, i32)> {
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

    fn tick(
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
struct ScheduledFluidTickRequest {
    pos: WorldBlockPos,
    fluid: FluidKind,
    delay: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkStatusSlot {
    pub status: ChunkStatus,
    pub step: ChunkStatusStep,
    pub revision: Option<ChunkRevision>,
    pub job_id: Option<ChunkJobId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkHolder {
    pos: ChunkPos,
    ticket_level: i32,
    target_status: Option<ChunkStatus>,
    status_slots: BTreeMap<ChunkStatus, ChunkStatusSlot>,
    published_snapshot: Option<ChunkSnapshot>,
    live_blocks: Option<MutableChunkBlockBuffer>,
    dependency_buffer: Option<MutableChunkBlockBuffer>,
    client_visible: bool,
    residency: ChunkResidency,
    dirty: bool,
}

impl ChunkHolder {
    fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            ticket_level: UNLOADED_CHUNK_LEVEL,
            target_status: None,
            status_slots: BTreeMap::new(),
            published_snapshot: None,
            live_blocks: None,
            dependency_buffer: None,
            client_visible: false,
            residency: ChunkResidency::NotResident,
            dirty: false,
        }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn target_status(&self) -> Option<ChunkStatus> {
        self.target_status
    }

    pub fn ticket_level(&self) -> i32 {
        self.ticket_level
    }

    pub fn full_status(&self) -> FullChunkStatus {
        full_chunk_status_for_ticket_level(self.ticket_level)
    }

    pub fn status_slot(&self, status: ChunkStatus) -> Option<&ChunkStatusSlot> {
        self.status_slots.get(&status)
    }

    pub fn ready_status_count(&self) -> usize {
        self.status_slots
            .values()
            .filter(|slot| slot.step == ChunkStatusStep::Ready)
            .count()
    }

    pub fn residency(&self) -> ChunkResidency {
        self.residency
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn snapshot(&self) -> Option<&ChunkSnapshot> {
        self.published_snapshot.as_ref()
    }

    pub fn has_dependency_buffer(&self) -> bool {
        self.dependency_buffer.is_some()
    }

    pub fn is_client_visible(&self) -> bool {
        self.client_visible
    }

    fn mark_scheduled(&mut self, status: ChunkStatus) {
        self.status_slots.insert(
            status,
            ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Scheduled,
                revision: None,
                job_id: None,
            },
        );
    }

    fn mark_ready(&mut self, status: ChunkStatus, revision: Option<ChunkRevision>) {
        let job_id = self.status_slots.get(&status).and_then(|slot| slot.job_id);
        self.status_slots.insert(
            status,
            ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Ready,
                revision,
                job_id,
            },
        );
    }

    fn assign_status_job(&mut self, status: ChunkStatus, job_id: ChunkJobId) {
        self.status_slots
            .entry(status)
            .or_insert(ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Scheduled,
                revision: None,
                job_id: None,
            })
            .job_id = Some(job_id);
    }

    fn publish_snapshot(
        &mut self,
        snapshot: ChunkSnapshot,
        residency: ChunkResidency,
        dirty: bool,
    ) {
        self.mark_ready(snapshot.status, Some(snapshot.revision));
        self.live_blocks = Some(mutable_buffer_from_snapshot(&snapshot));
        self.published_snapshot = Some(snapshot);
        self.residency = residency;
        self.dirty = dirty;
    }

    fn mark_saved(&mut self) {
        self.dirty = false;
        self.residency = ChunkResidency::Saved;
    }

    fn set_ticket_level(&mut self, ticket_level: i32) {
        self.ticket_level = ticket_level;
    }

    fn set_target_status(&mut self, target_status: ChunkStatus) {
        if self
            .target_status
            .is_none_or(|current| current < target_status)
        {
            self.target_status = Some(target_status);
        }
    }

    fn set_dependency_buffer(&mut self, buffer: MutableChunkBlockBuffer) {
        self.dependency_buffer = Some(buffer);
    }

    fn dependency_buffer(&self) -> Option<&MutableChunkBlockBuffer> {
        self.dependency_buffer.as_ref()
    }

    fn set_client_visible(&mut self, client_visible: bool) {
        self.client_visible = client_visible;
    }
}

#[derive(Debug)]
struct ChunkDistanceManager {
    tickets: BTreeMap<ChunkPos, BTreeSet<ChunkTicket>>,
    player_ticket_positions: BTreeSet<ChunkPos>,
    ticket_tick: u64,
}

impl ChunkDistanceManager {
    fn new() -> Self {
        Self {
            tickets: BTreeMap::new(),
            player_ticket_positions: BTreeSet::new(),
            ticket_tick: 0,
        }
    }

    fn ticket_tick(&self) -> u64 {
        self.ticket_tick
    }

    fn set_player_view(&mut self, view: ChunkView) {
        let new_positions = chunk_view_positions(&view)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let old_positions = std::mem::take(&mut self.player_ticket_positions);

        for pos in old_positions.difference(&new_positions).copied() {
            self.remove_ticket(
                ChunkTicketType::Player,
                pos,
                PLAYER_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        }

        for pos in new_positions.difference(&old_positions).copied() {
            self.add_ticket(
                ChunkTicketType::Player,
                pos,
                PLAYER_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        }

        self.player_ticket_positions = new_positions;
    }

    fn add_region_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        distance: i32,
        key: ChunkTicketKey,
    ) {
        self.add_ticket(ticket_type, pos, CHUNK_LEVEL_FULL - distance, key);
    }

    fn remove_region_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        distance: i32,
        key: ChunkTicketKey,
    ) {
        self.remove_ticket(ticket_type, pos, CHUNK_LEVEL_FULL - distance, key);
    }

    fn add_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        level: i32,
        key: ChunkTicketKey,
    ) {
        let mut ticket = ChunkTicket::new(ticket_type, level, key);
        ticket.created_tick = self.ticket_tick;
        let tickets = self.tickets.entry(pos).or_default();
        tickets.replace(ticket);
    }

    fn remove_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        level: i32,
        key: ChunkTicketKey,
    ) {
        let ticket = ChunkTicket::new(ticket_type, level, key);
        if let Some(tickets) = self.tickets.get_mut(&pos) {
            tickets.remove(&ticket);
            if tickets.is_empty() {
                self.tickets.remove(&pos);
            }
        }
    }

    fn purge_stale_tickets(&mut self) {
        self.ticket_tick += 1;
        let ticket_tick = self.ticket_tick;
        let mut empty_chunks = Vec::new();

        for (pos, tickets) in &mut self.tickets {
            tickets.retain(|ticket| !ticket.timed_out(ticket_tick));
            if tickets.is_empty() {
                empty_chunks.push(*pos);
            }
        }

        for pos in empty_chunks {
            self.tickets.remove(&pos);
        }
    }

    fn active_levels(&self) -> BTreeMap<ChunkPos, i32> {
        let mut levels: BTreeMap<ChunkPos, i32> = BTreeMap::new();

        for (source_pos, tickets) in &self.tickets {
            let Some(ticket) = tickets.first() else {
                continue;
            };
            if ticket.level > MAX_CHUNK_DISTANCE {
                continue;
            }

            let radius = MAX_CHUNK_DISTANCE - ticket.level;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let distance = dx.abs().max(dz.abs());
                    let level = ticket.level + distance;
                    if level > MAX_CHUNK_DISTANCE {
                        continue;
                    }

                    let pos = ChunkPos::new(source_pos.x + dx, source_pos.z + dz);
                    levels
                        .entry(pos)
                        .and_modify(|current| *current = (*current).min(level))
                        .or_insert(level);
                }
            }
        }

        levels
    }

    fn player_interest_positions(&self) -> BTreeSet<ChunkPos> {
        self.player_ticket_positions.clone()
    }

    fn ticketed_chunk_count(&self) -> usize {
        self.tickets
            .values()
            .filter(|tickets| {
                tickets
                    .first()
                    .is_some_and(|ticket| ticket.level <= MAX_CHUNK_DISTANCE)
            })
            .count()
    }

    fn ticket_count_at(&self, pos: ChunkPos) -> usize {
        self.tickets.get(&pos).map_or(0, BTreeSet::len)
    }

    fn ticket_level_at(&self, pos: ChunkPos) -> i32 {
        self.tickets
            .get(&pos)
            .and_then(|tickets| tickets.first())
            .map_or(UNLOADED_CHUNK_LEVEL, |ticket| ticket.level)
    }

    fn active_level_at(&self, pos: ChunkPos) -> i32 {
        self.active_levels()
            .get(&pos)
            .copied()
            .unwrap_or(UNLOADED_CHUNK_LEVEL)
    }
}

#[derive(Debug)]
pub struct ChunkScheduler {
    seed: i64,
    holders: BTreeMap<ChunkPos, ChunkHolder>,
    pending_unloads: BTreeSet<ChunkPos>,
    distance_manager: ChunkDistanceManager,
    jobs: BTreeMap<ChunkJobId, ChunkStatusJob>,
    job_timings: BTreeMap<ChunkJobId, OverworldFeatureBatchTiming>,
    worldgen_mailbox: WorldgenMailbox,
    pending_worldgen_publications: VecDeque<PendingWorldgenPublication>,
    pending_block_deltas: BTreeMap<(ChunkPos, i32), BTreeMap<usize, BlockStateId>>,
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
            holders: BTreeMap::new(),
            pending_unloads: BTreeSet::new(),
            distance_manager: ChunkDistanceManager::new(),
            jobs: BTreeMap::new(),
            job_timings: BTreeMap::new(),
            worldgen_mailbox: WorldgenMailbox::new(),
            pending_worldgen_publications: VecDeque::new(),
            pending_block_deltas: BTreeMap::new(),
            dirty_chunks: BTreeSet::new(),
            next_job_id: 1,
            next_revision: 1,
            store,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn apply_interest(
        &mut self,
        view: ChunkView,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.distance_manager.set_player_view(view);
        self.reconcile_ticketed_holders()
    }

    pub fn poll(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.publish_completed_worldgen_jobs()
    }

    pub fn wait_for_worldgen_completion(&mut self, timeout: Duration) -> bool {
        self.worldgen_mailbox.wait_for_completed(timeout)
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
        self.jobs
            .values()
            .filter(|job| matches!(job.state, ChunkJobState::Queued | ChunkJobState::Running))
            .count()
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
            .sum()
    }

    pub fn worldgen_mailbox_kind(&self) -> WorldgenMailboxKind {
        self.worldgen_mailbox.kind()
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
        }
    }

    fn block_at_world(&self, pos: WorldBlockPos) -> Option<RawBlockId> {
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

    fn set_block_at_world(&mut self, pos: WorldBlockPos, block_id: RawBlockId) -> bool {
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

    fn drain_pending_block_delta_events(&mut self) -> Vec<ChunkSchedulerEvent> {
        let pending = std::mem::take(&mut self.pending_block_deltas);
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
            })
            .collect()
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

    fn resolve_lava_source_contact_at(&mut self, pos: WorldBlockPos) -> bool {
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
        if let Some(fluid) = FluidKind::from_block_id(block_id) {
            record_scheduled_fluid_tick(report, pos, fluid);
        }

        for direction in ALL_FLUID_DIRECTIONS {
            let neighbor = offset_pos(pos, direction);
            let Some(neighbor_block) = self.block_at_world(neighbor) else {
                continue;
            };
            if let Some(fluid) = FluidKind::from_block_id(neighbor_block) {
                record_scheduled_fluid_tick(report, neighbor, fluid);
            }
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

    fn reconcile_ticketed_holders(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let active_levels = self.distance_manager.active_levels();
        let desired_set = active_levels.keys().copied().collect::<BTreeSet<_>>();
        let client_visible_set = self.distance_manager.player_interest_positions();
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

        let mut feature_targets = BTreeSet::new();
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
                ChunkStatus::Features
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
                    && holder.published_snapshot.is_some()
                {
                    holder.set_client_visible(true);
                    snapshot_to_publish = holder.published_snapshot.clone();
                }
            }

            if let Some(snapshot) = snapshot_to_publish {
                events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
            }

            if target_status >= ChunkStatus::Features {
                feature_targets.insert(pos);
            } else {
                self.ensure_dependency_status_scheduled(pos, target_status);
            }
        }

        events.extend(self.enqueue_feature_chunks(
            sorted_chunk_positions_z_major(feature_targets),
            &client_visible_set,
        )?);
        Ok(events)
    }

    fn enqueue_feature_chunks(
        &mut self,
        desired_chunks: Vec<ChunkPos>,
        client_visible_set: &BTreeSet<ChunkPos>,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let mut events = Vec::new();
        let mut to_generate = Vec::new();

        for pos in desired_chunks {
            self.holders
                .entry(pos)
                .or_insert_with(|| ChunkHolder::new(pos))
                .target_status = Some(ChunkStatus::Features);

            for status in status_path_to(ChunkStatus::Features) {
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
                    if let Some(snapshot) = self.load_stored_snapshot(pos, ChunkStatus::Features)? {
                        self.mark_snapshot_ready(
                            pos,
                            snapshot.clone(),
                            ChunkResidency::LoadedFromStore,
                            false,
                        );
                        events.push(ChunkSchedulerEvent::StatusChanged {
                            pos,
                            status,
                            step: ChunkStatusStep::Ready,
                        });
                        if client_visible_set.contains(&pos) {
                            self.holders
                                .get_mut(&pos)
                                .expect("holder must exist before marking visible")
                                .set_client_visible(true);
                            events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
                        }
                    } else {
                        to_generate.push(pos);
                    }
                } else {
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
            let (job_id, seeded_dependencies) = self.create_feature_job(&to_generate);
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
            let light_sections = publication
                .completed
                .light_sections
                .get(&pos)
                .cloned()
                .unwrap_or_else(|| {
                    let light_neighbors = retained_dependencies
                        .iter()
                        .filter_map(|(neighbor_pos, buffer)| {
                            provisional_sky_light_includes_chunk(pos, *neighbor_pos)
                                .then_some((*neighbor_pos, buffer.blocks.as_slice()))
                        })
                        .chain(generated.iter().filter_map(|(neighbor_pos, chunk)| {
                            provisional_sky_light_includes_chunk(pos, *neighbor_pos)
                                .then_some((*neighbor_pos, chunk.blocks()))
                        }));
                    provisional_light_sections_from_neighbors(
                        pos,
                        chunk.min_y,
                        chunk.height,
                        chunk.blocks(),
                        light_neighbors,
                    )
                });
            let snapshot = chunk
                .to_chunk_snapshot(revision, ChunkStatus::Features)
                .with_light_sections(false, light_sections);
            self.mark_snapshot_ready(pos, snapshot.clone(), ChunkResidency::Generated, true);
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status: ChunkStatus::Features,
                step: ChunkStatusStep::Ready,
            });
            if self
                .distance_manager
                .player_interest_positions()
                .contains(&pos)
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
        for dependency in completed.retained_dependencies.into_values() {
            self.mark_dependency_ready(dependency);
        }
        self.mark_job_complete(completed.job_id, completed.cache_report, completed.timing);
        Ok((events, None))
    }

    fn create_feature_job(
        &mut self,
        targets: &[ChunkPos],
    ) -> (ChunkJobId, Vec<MutableChunkBlockBuffer>) {
        let id = ChunkJobId(self.next_job_id);
        self.next_job_id += 1;
        let (target_chunks, feature_centers, dependency_chunks) = feature_job_positions(targets);
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

#[derive(Debug)]
struct WorldgenCompletedJob {
    job_id: ChunkJobId,
    generated_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    light_sections: BTreeMap<ChunkPos, Vec<PackedLightSection>>,
    cache_report: OverworldFeatureDependencyCacheReport,
    timing: OverworldFeatureBatchTiming,
}

#[derive(Debug)]
struct PendingWorldgenPublication {
    completed: WorldgenCompletedJob,
    next_target_index: usize,
}

impl PendingWorldgenPublication {
    fn new(completed: WorldgenCompletedJob) -> Self {
        Self {
            completed,
            next_target_index: 0,
        }
    }
}

fn precompute_completed_light_sections(
    generated_chunks: &BTreeMap<ChunkPos, GeneratedChunk>,
    retained_dependencies: &BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    targets: &[ChunkPos],
) -> BTreeMap<ChunkPos, Vec<PackedLightSection>> {
    targets
        .iter()
        .filter_map(|pos| {
            let chunk = generated_chunks.get(pos)?;
            let light_neighbors = retained_dependencies
                .iter()
                .filter_map(|(neighbor_pos, buffer)| {
                    provisional_sky_light_includes_chunk(*pos, *neighbor_pos)
                        .then_some((*neighbor_pos, buffer.blocks.as_slice()))
                })
                .chain(generated_chunks.iter().filter_map(|(neighbor_pos, chunk)| {
                    provisional_sky_light_includes_chunk(*pos, *neighbor_pos)
                        .then_some((*neighbor_pos, chunk.blocks()))
                }));
            Some((
                *pos,
                provisional_light_sections_from_neighbors(
                    *pos,
                    chunk.min_y,
                    chunk.height,
                    chunk.blocks(),
                    light_neighbors,
                ),
            ))
        })
        .collect()
}

struct WorldgenMailbox {
    backend: WorldgenMailboxBackend,
}

impl WorldgenMailbox {
    fn new() -> Self {
        Self {
            backend: WorldgenMailboxBackend::new(),
        }
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.backend
            .enqueue_features(job_id, seed, targets, dependencies);
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        self.backend.drain_completed()
    }

    fn wait_for_completed(&mut self, timeout: Duration) -> bool {
        self.backend.wait_for_completed(timeout)
    }

    fn kind(&self) -> WorldgenMailboxKind {
        self.backend.kind()
    }
}

impl fmt::Debug for WorldgenMailbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldgenMailbox")
            .field("backend", &self.backend)
            .finish()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct WorldgenMailboxBackend {
    completed: VecDeque<WorldgenCompletedJob>,
}

#[cfg(target_arch = "wasm32")]
impl WorldgenMailboxBackend {
    fn new() -> Self {
        Self {
            completed: VecDeque::new(),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::Inline
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        let mut dependency_cache = OverworldFeatureDependencyCache::new();
        let result = dependency_cache.generate_features_chunks_with_dependencies(
            seed,
            targets.iter().copied(),
            dependencies,
        );
        let light_sections = precompute_completed_light_sections(
            &result.chunks,
            &result.retained_dependencies,
            targets,
        );
        self.completed.push_back(WorldgenCompletedJob {
            job_id,
            generated_chunks: result.chunks,
            retained_dependencies: result.retained_dependencies,
            light_sections,
            cache_report: result.cache_report,
            timing: result.timing,
        });
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        self.completed.drain(..).collect()
    }

    fn wait_for_completed(&mut self, _timeout: Duration) -> bool {
        !self.completed.is_empty()
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct WorldgenMailboxBackend {
    sender: mpsc::Sender<WorldgenRequest>,
    completion_receiver: mpsc::Receiver<WorldgenCompletedJob>,
    completed: VecDeque<WorldgenCompletedJob>,
    worker: Option<thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Debug for WorldgenMailboxBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldgenMailboxBackend")
            .field("kind", &"native-thread")
            .field("worker_running", &self.worker.is_some())
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldgenMailboxBackend {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<WorldgenRequest>();
        let (completion_sender, completion_receiver) = mpsc::channel::<WorldgenCompletedJob>();
        let worker = thread::Builder::new()
            .name("mclone-worldgen".to_owned())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    match request {
                        WorldgenRequest::GenerateFeatures {
                            job_id,
                            seed,
                            targets,
                            dependencies,
                        } => {
                            let mut dependency_cache = OverworldFeatureDependencyCache::new();
                            let result = dependency_cache
                                .generate_features_chunks_with_dependencies(
                                    seed,
                                    targets.iter().copied(),
                                    dependencies,
                                );
                            let light_sections = precompute_completed_light_sections(
                                &result.chunks,
                                &result.retained_dependencies,
                                &targets,
                            );
                            if completion_sender
                                .send(WorldgenCompletedJob {
                                    job_id,
                                    generated_chunks: result.chunks,
                                    retained_dependencies: result.retained_dependencies,
                                    light_sections,
                                    cache_report: result.cache_report,
                                    timing: result.timing,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        WorldgenRequest::Shutdown => break,
                    }
                }
            })
            .expect("failed to spawn native worldgen worker");

        Self {
            sender,
            completion_receiver,
            completed: VecDeque::new(),
            worker: Some(worker),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::NativeThread
    }

    fn enqueue_features(
        &mut self,
        job_id: ChunkJobId,
        seed: i64,
        targets: &[ChunkPos],
        dependencies: Vec<MutableChunkBlockBuffer>,
    ) {
        self.sender
            .send(WorldgenRequest::GenerateFeatures {
                job_id,
                seed,
                targets: targets.to_vec(),
                dependencies,
            })
            .expect("native worldgen worker stopped before receiving job");
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        let mut completed = self.completed.drain(..).collect::<Vec<_>>();
        while let Ok(job) = self.completion_receiver.try_recv() {
            completed.push(job);
        }
        completed
    }

    fn wait_for_completed(&mut self, timeout: Duration) -> bool {
        if !self.completed.is_empty() {
            return true;
        }

        match self.completion_receiver.recv_timeout(timeout) {
            Ok(job) => {
                self.completed.push_back(job);
                true
            }
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for WorldgenMailboxBackend {
    fn drop(&mut self) {
        let _ = self.sender.send(WorldgenRequest::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum WorldgenRequest {
    GenerateFeatures {
        job_id: ChunkJobId,
        seed: i64,
        targets: Vec<ChunkPos>,
        dependencies: Vec<MutableChunkBlockBuffer>,
    },
    Shutdown,
}

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    scheduler: ChunkScheduler,
    liquid_ticks: FluidTickList,
    simulation_tick: u64,
}

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self::with_chunk_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self {
            seed,
            scheduler: ChunkScheduler::with_store(seed, store),
            liquid_ticks: FluidTickList::new(),
            simulation_tick: 0,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn simulation_tick(&self) -> u64 {
        self.simulation_tick
    }

    pub fn schedule_fluid_tick(&mut self, pos: WorldBlockPos, fluid: FluidKind, delay: i32) {
        self.liquid_ticks
            .schedule_tick(pos, fluid, delay, self.simulation_tick);
    }

    pub fn scheduled_fluid_tick_count(&self) -> usize {
        self.liquid_ticks.size()
    }

    pub fn handle_command(&mut self, command: ClientCommand) -> Vec<ServerUpdate> {
        self.try_handle_command(command)
            .expect("integrated server command failed")
    }

    pub fn try_handle_command(
        &mut self,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        match command {
            ClientCommand::SetChunkView(view) => self.set_chunk_view(view),
        }
    }

    pub fn poll(&mut self) -> Vec<ServerUpdate> {
        self.try_poll().expect("integrated server poll failed")
    }

    pub fn try_poll(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let events = self.scheduler.poll()?;
        Ok(self.apply_scheduler_events(events))
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
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let report = self.scheduler.tick_report()?;
        let scheduler_report_us = simulation_timing_elapsed_us(scheduler_start);
        let scheduler_event_count = report.events.len();
        let scheduler_apply_start = simulation_timing_start();
        let updates = self.apply_scheduler_events(report.events);
        let scheduler_apply_events_us = simulation_timing_elapsed_us(scheduler_apply_start);
        let scheduler_timing = report.timing;
        Ok(ServerTickReport {
            ticket_tick: report.ticket_tick,
            block_ticking_chunks: report.block_ticking_chunks,
            entity_ticking_chunks: report.entity_ticking_chunks,
            pending_unloads_processed: report.pending_unloads_processed,
            scheduler_event_count,
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
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let tick_report = self.try_tick_report()?;
        let scheduler_tick_us = simulation_timing_elapsed_us(scheduler_start);
        let tick_timing = tick_report.timing;

        let simulation_tick = self.simulation_tick.saturating_add(1);
        self.simulation_tick = simulation_tick;

        let block_tick_start = simulation_timing_start();
        let block_tick_chunks = run_noop_simulation_phase(&tick_report.block_ticking_chunks);
        let block_tick_us = simulation_timing_elapsed_us(block_tick_start);

        let fluid_tick_start = simulation_timing_start();
        let (fluid_report, mut fluid_events) = self.liquid_ticks.tick(
            simulation_tick,
            &tick_report.entity_ticking_chunks,
            &mut self.scheduler,
        );
        let fluid_tick_us = simulation_timing_elapsed_us(fluid_tick_start);
        fluid_events.extend(self.scheduler.drain_pending_block_delta_events());
        let fluid_event_count = fluid_events.len();

        let entity_tick_start = simulation_timing_start();
        let entity_tick_chunks = run_noop_simulation_phase(&tick_report.entity_ticking_chunks);
        let entity_tick_us = simulation_timing_elapsed_us(entity_tick_start);

        let mut updates = tick_report.updates;
        let fluid_event_apply_start = simulation_timing_start();
        updates.extend(
            fluid_events
                .into_iter()
                .filter_map(server_update_from_scheduler_event),
        );
        let fluid_event_apply_us = simulation_timing_elapsed_us(fluid_event_apply_start);

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
            pending_unloads_processed: tick_report.pending_unloads_processed,
            scheduler_event_count: tick_report.scheduler_event_count,
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
            },
        })
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.scheduler.loaded_chunk_count()
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

    pub fn scheduler(&self) -> &ChunkScheduler {
        &self.scheduler
    }

    pub fn scheduler_mut(&mut self) -> &mut ChunkScheduler {
        &mut self.scheduler
    }

    pub fn save_dirty_chunks(&mut self) -> ChunkStoreResult<usize> {
        self.scheduler.save_dirty_chunks()
    }

    fn set_chunk_view(&mut self, view: ChunkView) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let events = self.scheduler.apply_interest(view)?;
        Ok(self.apply_scheduler_events(events))
    }

    fn apply_scheduler_events(&mut self, events: Vec<ChunkSchedulerEvent>) -> Vec<ServerUpdate> {
        let mut updates = Vec::new();
        for event in events {
            match event {
                ChunkSchedulerEvent::FluidTickScheduled { pos, fluid, delay } => {
                    self.liquid_ticks
                        .schedule_tick(pos, fluid, delay, self.simulation_tick);
                }
                event => {
                    if let Some(update) = server_update_from_scheduler_event(event) {
                        updates.push(update);
                    }
                }
            }
        }
        updates
    }
}

fn server_update_from_scheduler_event(event: ChunkSchedulerEvent) -> Option<ServerUpdate> {
    match event {
        ChunkSchedulerEvent::SnapshotReady(snapshot) => Some(ServerUpdate::ChunkSnapshot(snapshot)),
        ChunkSchedulerEvent::Unloaded { pos } => Some(ServerUpdate::ChunkUnload { pos }),
        ChunkSchedulerEvent::SectionBlockUpdates {
            pos,
            section_y,
            updates,
        } => Some(ServerUpdate::SectionBlockUpdates {
            pos,
            section_y,
            updates,
        }),
        ChunkSchedulerEvent::StatusChanged { .. }
        | ChunkSchedulerEvent::FluidTickScheduled { .. } => None,
    }
}

fn run_noop_simulation_phase(chunks: &[ChunkPos]) -> usize {
    chunks.iter().fold(0, |count, _pos| count + 1)
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

fn feature_job_positions(targets: &[ChunkPos]) -> (Vec<ChunkPos>, Vec<ChunkPos>, Vec<ChunkPos>) {
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
        sorted_chunk_positions_z_major(target_chunks),
        sorted_chunk_positions_z_major(feature_centers),
        sorted_chunk_positions_z_major(dependency_chunks),
    )
}

fn sorted_chunk_positions_z_major(positions: BTreeSet<ChunkPos>) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by_key(|pos| (pos.z, pos.x));
    positions
}

fn chunk_view_positions(view: &ChunkView) -> Vec<ChunkPos> {
    let radius =
        i32::try_from(view.chunk_tracking_radius).expect("chunk tracking radius exceeds i32");
    let min_x = view.center.x - radius;
    let max_x = view.center.x + radius;
    let min_z = view.center.z - radius;
    let max_z = view.center.z + radius;
    (min_x..=max_x)
        .flat_map(|x| (min_z..=max_z).map(move |z| ChunkPos::new(x, z)))
        .collect()
}

fn status_path_to(target_status: ChunkStatus) -> Vec<ChunkStatus> {
    ALL_STATUSES
        .iter()
        .copied()
        .take_while(|status| *status <= target_status)
        .collect()
}

const DEFAULT_PENDING_UNLOAD_BUDGET: usize = 200;
const DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET: usize = 1;
const MAX_SCHEDULED_FLUID_TICKS_PER_TICK: usize = 65_536;
const ALL_FLUID_DIRECTIONS: [FluidDirection; 6] = [
    FluidDirection::Down,
    FluidDirection::Up,
    FluidDirection::North,
    FluidDirection::South,
    FluidDirection::West,
    FluidDirection::East,
];
const LAVA_SOURCE_CONTACT_DIRECTIONS: [FluidDirection; 5] = [
    FluidDirection::Up,
    FluidDirection::North,
    FluidDirection::South,
    FluidDirection::West,
    FluidDirection::East,
];

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

fn mutable_buffer_from_snapshot(snapshot: &ChunkSnapshot) -> MutableChunkBlockBuffer {
    let mut buffer = MutableChunkBlockBuffer::new(
        snapshot.pos.x,
        snapshot.pos.z,
        snapshot.min_y,
        snapshot.height,
    );
    for section in &snapshot.sections {
        let section_blocks = section.unpack_block_state_ids();
        debug_assert_eq!(section_blocks.len(), CHUNK_SECTION_VOLUME);
        let section_base_y = section.section_y * SECTION_HEIGHT;
        for local_y in 0..SECTION_HEIGHT {
            let y = section_base_y + local_y;
            if y < snapshot.min_y || y >= snapshot.min_y + snapshot.height {
                continue;
            }
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let index = chunk_section_index(local_x, local_y, local_z);
                    buffer.set_block_at_y(
                        local_x,
                        y,
                        local_z,
                        raw_block_id_from_state_id(section_blocks[index]),
                    );
                }
            }
        }
    }
    buffer
}

#[cfg(test)]
fn snapshot_with_provisional_lighting(
    snapshot: ChunkSnapshot,
    raw_blocks: &[RawBlockId],
) -> ChunkSnapshot {
    snapshot_with_provisional_lighting_from_neighbors(snapshot, raw_blocks, std::iter::empty())
}

#[cfg(test)]
fn snapshot_with_provisional_lighting_from_neighbors<'a>(
    snapshot: ChunkSnapshot,
    raw_blocks: &'a [RawBlockId],
    neighbor_blocks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> ChunkSnapshot {
    let light_sections = provisional_light_sections_from_neighbors(
        snapshot.pos,
        snapshot.min_y,
        snapshot.height,
        raw_blocks,
        neighbor_blocks,
    );
    snapshot.with_light_sections(false, light_sections)
}

fn provisional_light_sections_from_neighbors<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    raw_blocks: &'a [RawBlockId],
    neighbor_blocks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    assert_eq!(
        raw_blocks.len(),
        expected_len,
        "chunk {:?} lighting input had {} blocks instead of {expected_len}",
        target_pos,
        raw_blocks.len()
    );
    let mut chunks = neighbor_blocks.into_iter().collect::<Vec<_>>();
    chunks.push((target_pos, raw_blocks));
    let sky_sections =
        provisional_sky_light_sections_for_chunk(target_pos, min_y, height, chunks.iter().copied());
    let block_sections = provisional_block_light_sections_for_chunk(
        target_pos,
        min_y,
        height,
        chunks.iter().copied(),
    );
    merge_light_sections(sky_sections, block_sections)
}

#[cfg(test)]
fn provisional_sky_light_sections(
    min_y: i32,
    height: i32,
    raw_blocks: &[RawBlockId],
) -> Vec<PackedLightSection> {
    provisional_sky_light_sections_for_chunk(
        ChunkPos::new(0, 0),
        min_y,
        height,
        std::iter::once((ChunkPos::new(0, 0), raw_blocks)),
    )
}

fn provisional_sky_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let world = ProvisionalSkyLightWorld::new(target_pos, min_y, height, chunks);
    world.light_sections()
}

fn provisional_block_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let world = ProvisionalSkyLightWorld::new(target_pos, min_y, height, chunks);
    world.block_light_sections()
}

fn merge_light_sections(
    sky_sections: Vec<PackedLightSection>,
    block_sections: Vec<PackedLightSection>,
) -> Vec<PackedLightSection> {
    let mut sections = BTreeMap::<i32, (Option<Vec<u8>>, Option<Vec<u8>>)>::new();
    for section in sky_sections.into_iter().chain(block_sections) {
        let entry = sections.entry(section.section_y).or_default();
        if section.sky.is_some() {
            entry.0 = section.sky;
        }
        if section.block.is_some() {
            entry.1 = section.block;
        }
    }

    sections
        .into_iter()
        .map(|(section_y, (sky, block))| PackedLightSection::new(section_y, sky, block))
        .collect()
}

fn provisional_sky_light_includes_chunk(target_pos: ChunkPos, chunk_pos: ChunkPos) -> bool {
    (chunk_pos.x - target_pos.x).abs() <= 1 && (chunk_pos.z - target_pos.z).abs() <= 1
}

struct ProvisionalSkyLightWorld<'a> {
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: BTreeMap<ChunkPos, &'a [RawBlockId]>,
}

impl<'a> ProvisionalSkyLightWorld<'a> {
    fn new(
        target_pos: ChunkPos,
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
    ) -> Self {
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "provisional light height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        let chunks = chunks
            .into_iter()
            .map(|(pos, blocks)| {
                assert_eq!(
                    blocks.len(),
                    expected_len,
                    "chunk {pos:?} lighting input had {} blocks instead of {expected_len}",
                    blocks.len()
                );
                (pos, blocks)
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            chunks.contains_key(&target_pos),
            "provisional light target chunk {target_pos:?} was missing from input chunks"
        );

        Self {
            target_pos,
            min_y,
            height,
            chunks,
        }
    }

    fn light_sections(&self) -> Vec<PackedLightSection> {
        let mut sky_values = self
            .chunks
            .keys()
            .map(|pos| {
                (
                    *pos,
                    vec![0_u8; self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut queue = VecDeque::new();

        for (&chunk_pos, blocks) in &self.chunks {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let mut sky_open = true;
                    for local_y in (0..self.height).rev() {
                        let index = chunk_block_index(local_x, local_y, local_z);
                        let block_id = blocks[index];
                        if sky_open && sky_light_opacity(block_id) >= 15 {
                            sky_open = false;
                            continue;
                        }
                        if sky_open {
                            sky_values.get_mut(&chunk_pos).unwrap()[index] = 15;
                            queue.push_back((chunk_pos, local_x, local_y, local_z));
                        }
                    }
                }
            }
        }

        while let Some((chunk_pos, local_x, local_y, local_z)) = queue.pop_front() {
            let source_level = sky_values[&chunk_pos][chunk_block_index(local_x, local_y, local_z)];
            if source_level <= 1 {
                continue;
            }

            for [dx, dy, dz] in SKY_LIGHT_DIRECTIONS {
                let Some((next_pos, next_x, next_y, next_z)) =
                    self.offset_cell(chunk_pos, local_x, local_y, local_z, dx, dy, dz)
                else {
                    continue;
                };

                let next_index = chunk_block_index(next_x, next_y, next_z);
                let target_block = self.chunks[&next_pos][next_index];
                let Some(next_level) = propagated_sky_light_level(source_level, dy, target_block)
                else {
                    continue;
                };
                let next_values = sky_values.get_mut(&next_pos).unwrap();
                if next_level > next_values[next_index] {
                    next_values[next_index] = next_level;
                    queue.push_back((next_pos, next_x, next_y, next_z));
                }
            }
        }

        self.target_light_sections(&sky_values[&self.target_pos], LightLayer::Sky)
    }

    fn block_light_sections(&self) -> Vec<PackedLightSection> {
        let mut block_values = self
            .chunks
            .keys()
            .map(|pos| {
                (
                    *pos,
                    vec![0_u8; self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut queue = VecDeque::new();

        for (&chunk_pos, blocks) in &self.chunks {
            for local_y in 0..self.height {
                for local_z in 0..CHUNK_WIDTH {
                    for local_x in 0..CHUNK_WIDTH {
                        let index = chunk_block_index(local_x, local_y, local_z);
                        let emission = block_light_emission(blocks[index]);
                        if emission > 0 {
                            block_values.get_mut(&chunk_pos).unwrap()[index] = emission;
                            queue.push_back((chunk_pos, local_x, local_y, local_z));
                        }
                    }
                }
            }
        }

        while let Some((chunk_pos, local_x, local_y, local_z)) = queue.pop_front() {
            let source_level =
                block_values[&chunk_pos][chunk_block_index(local_x, local_y, local_z)];
            if source_level <= 1 {
                continue;
            }

            for [dx, dy, dz] in SKY_LIGHT_DIRECTIONS {
                let Some((next_pos, next_x, next_y, next_z)) =
                    self.offset_cell(chunk_pos, local_x, local_y, local_z, dx, dy, dz)
                else {
                    continue;
                };

                let next_index = chunk_block_index(next_x, next_y, next_z);
                let target_block = self.chunks[&next_pos][next_index];
                let Some(next_level) = propagated_block_light_level(source_level, target_block)
                else {
                    continue;
                };
                let next_values = block_values.get_mut(&next_pos).unwrap();
                if next_level > next_values[next_index] {
                    next_values[next_index] = next_level;
                    queue.push_back((next_pos, next_x, next_y, next_z));
                }
            }
        }

        self.target_light_sections(&block_values[&self.target_pos], LightLayer::Block)
    }

    fn offset_cell(
        &self,
        chunk_pos: ChunkPos,
        local_x: i32,
        local_y: i32,
        local_z: i32,
        dx: i32,
        dy: i32,
        dz: i32,
    ) -> Option<(ChunkPos, i32, i32, i32)> {
        let next_y = local_y + dy;
        if !(0..self.height).contains(&next_y) {
            return None;
        }

        let mut next_pos = chunk_pos;
        let mut next_x = local_x + dx;
        if next_x < 0 {
            next_pos.x -= 1;
            next_x += CHUNK_WIDTH;
        } else if next_x >= CHUNK_WIDTH {
            next_pos.x += 1;
            next_x -= CHUNK_WIDTH;
        }

        let mut next_z = local_z + dz;
        if next_z < 0 {
            next_pos.z -= 1;
            next_z += CHUNK_WIDTH;
        } else if next_z >= CHUNK_WIDTH {
            next_pos.z += 1;
            next_z -= CHUNK_WIDTH;
        }

        self.chunks
            .contains_key(&next_pos)
            .then_some((next_pos, next_x, next_y, next_z))
    }

    fn target_light_sections(
        &self,
        light_values: &[u8],
        layer: LightLayer,
    ) -> Vec<PackedLightSection> {
        debug_assert_eq!(
            light_values.len(),
            self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize
        );
        let section_count = self.height / SECTION_HEIGHT;
        let min_section_y = block_to_section_coord(self.min_y);

        let mut layers = (0..section_count)
            .map(|_| DataLayer::new())
            .collect::<Vec<_>>();
        for local_y in 0..self.height {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let level = light_values[chunk_block_index(local_x, local_y, local_z)];
                    if level == 0 {
                        continue;
                    }
                    let section_offset = block_to_section_coord(local_y);
                    let section_local_y = local_section_block_coord(local_y);
                    layers[section_offset as usize].set(local_x, section_local_y, local_z, level);
                }
            }
        }

        let mut sections = layers
            .into_iter()
            .enumerate()
            .filter_map(|(offset, data_layer)| {
                data_layer.into_bytes().map(|bytes| {
                    let (sky, block) = match layer {
                        LightLayer::Sky => (Some(bytes), None),
                        LightLayer::Block => (None, Some(bytes)),
                    };
                    PackedLightSection::new(min_section_y + offset as i32, sky, block)
                })
            })
            .collect::<Vec<_>>();
        if layer == LightLayer::Sky && sections.is_empty() && section_count > 0 {
            sections.push(PackedLightSection::new(
                min_section_y,
                Some(vec![0; mclone_light::DATA_LAYER_SIZE]),
                None,
            ));
        }
        sections
    }
}

const SKY_LIGHT_DIRECTIONS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

fn propagated_sky_light_level(
    source_level: u8,
    direction_y: i32,
    target_block: RawBlockId,
) -> Option<u8> {
    let opacity = sky_light_opacity(target_block);
    if source_level == 0 || opacity >= 15 {
        return None;
    }

    let attenuation = if direction_y == -1 && source_level == 15 && opacity == 0 {
        0
    } else {
        opacity.max(1)
    };
    let level = source_level.saturating_sub(attenuation);
    (level > 0).then_some(level)
}

fn propagated_block_light_level(source_level: u8, target_block: RawBlockId) -> Option<u8> {
    let opacity = sky_light_opacity(target_block);
    if source_level == 0 || opacity >= 15 {
        return None;
    }

    let level = source_level.saturating_sub(opacity.max(1));
    (level > 0).then_some(level)
}

fn block_light_emission(block_id: RawBlockId) -> u8 {
    if is_lava(block_id) { 15 } else { 0 }
}

fn sky_light_opacity(block_id: RawBlockId) -> u8 {
    if material_blocks_motion(block_id) {
        15
    } else {
        0
    }
}

fn raw_block_id_from_state_id(state_id: BlockStateId) -> RawBlockId {
    RawBlockId::try_from(state_id.0)
        .unwrap_or_else(|_| panic!("block state id {} does not fit native raw id", state_id.0))
}

fn offset_pos(pos: WorldBlockPos, direction: FluidDirection) -> WorldBlockPos {
    let (dx, dy, dz) = direction.offset();
    pos.offset(dx, dy, dz)
}

fn legacy_block_for_fluid_state(state: NativeFluidState) -> Option<RawBlockId> {
    let fluid = state.kind?;
    let legacy_level = if state.source {
        0
    } else {
        8_u8.saturating_sub(state.amount.min(8))
            .saturating_add(if state.falling { 8 } else { 0 })
            .min(8)
    };
    fluid.block_for_level(legacy_level)
}

fn fluid_can_convert_to_source(fluid: FluidKind) -> bool {
    matches!(fluid, FluidKind::Water)
}

fn fluid_drop_off(fluid: FluidKind) -> u8 {
    match fluid {
        FluidKind::Water => 1,
        FluidKind::Lava => 2,
    }
}

fn fluid_slope_find_distance(fluid: FluidKind) -> i32 {
    match fluid {
        FluidKind::Water => 4,
        FluidKind::Lava => 2,
    }
}

fn target_fluid_can_be_replaced_with(
    target: NativeFluidState,
    incoming: FluidKind,
    direction: FluidDirection,
) -> bool {
    match target.kind {
        None => true,
        Some(FluidKind::Water) => direction == FluidDirection::Down && incoming != FluidKind::Water,
        Some(FluidKind::Lava) => direction == FluidDirection::Down && incoming != FluidKind::Lava,
    }
}

fn fluid_cache_key(origin: WorldBlockPos, pos: WorldBlockPos) -> i32 {
    let x = pos.x - origin.x;
    let z = pos.z - origin.z;
    ((x + 128) & 0xff) << 8 | ((z + 128) & 0xff)
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
    let request = ScheduledFluidTickRequest {
        pos,
        fluid,
        delay: fluid.tick_delay(),
    };
    if !report.scheduled_ticks.contains(&request) {
        report.scheduled_ticks.push(request);
    }
}

fn full_chunk_status_for_ticket_level(ticket_level: i32) -> FullChunkStatus {
    match (CHUNK_LEVEL_FULL - ticket_level + 1).clamp(0, 3) {
        0 => FullChunkStatus::Inaccessible,
        1 => FullChunkStatus::Border,
        2 => FullChunkStatus::Ticking,
        3 => FullChunkStatus::EntityTicking,
        _ => unreachable!("clamped full chunk status index must be in 0..=3"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{
        AIR, LAVA_LEVEL_2, LAVA_LEVEL_8, OBSIDIAN, STONE, WATER_LEVEL_1, WATER_LEVEL_2,
        WATER_LEVEL_8, lava_block_for_level, water_block_for_level,
    };
    use serde_json::Value;

    fn apply_interest_and_poll(
        scheduler: &mut ChunkScheduler,
        interest: ChunkView,
    ) -> Vec<ChunkSchedulerEvent> {
        let mut events = scheduler.apply_interest(interest).unwrap();
        events.extend(poll_scheduler_until_idle(scheduler));
        events
    }

    fn poll_scheduler_until_idle(scheduler: &mut ChunkScheduler) -> Vec<ChunkSchedulerEvent> {
        let mut events = Vec::new();
        for _ in 0..60_000 {
            events.extend(scheduler.poll().unwrap());
            if scheduler.pending_job_count() == 0 {
                return events;
            }
            if scheduler.pending_publication_count() == 0 {
                wait_for_scheduler_completion(scheduler);
            }
        }
        panic!("timed out waiting for scheduler worldgen jobs");
    }

    #[test]
    fn provisional_sky_light_spreads_sideways_below_overhangs() {
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(0, 14, 0)] = STONE;

        let sections = provisional_sky_light_sections(0, SECTION_HEIGHT, &blocks);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].section_y, 0);
        assert!(sections[0].block.is_none());
        let sky =
            mclone_light::packed_light_section_layer(&sections[0], mclone_light::LightLayer::Sky)
                .unwrap()
                .unwrap();
        assert_eq!(sky.get(0, 15, 0), 15);
        assert_eq!(sky.get(0, 14, 0), 0);
        assert_eq!(sky.get(0, 13, 0), 14);
        assert_eq!(sky.get(1, 0, 0), 15);
    }

    #[test]
    fn provisional_sky_light_keeps_full_roof_dark_below() {
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                blocks[chunk_block_index(local_x, 14, local_z)] = STONE;
            }
        }

        let sections = provisional_sky_light_sections(0, SECTION_HEIGHT, &blocks);

        assert_eq!(sections.len(), 1);
        let sky =
            mclone_light::packed_light_section_layer(&sections[0], mclone_light::LightLayer::Sky)
                .unwrap()
                .unwrap();
        assert_eq!(sky.get(8, 15, 8), 15);
        assert_eq!(sky.get(8, 14, 8), 0);
        assert_eq!(sky.get(8, 13, 8), 0);
    }

    #[test]
    fn provisional_sky_light_marks_all_dark_chunks_as_lit_data() {
        let blocks = vec![STONE; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];

        let sections = provisional_sky_light_sections(0, SECTION_HEIGHT, &blocks);

        assert_eq!(sections.len(), 1);
        let sky =
            mclone_light::packed_light_section_layer(&sections[0], mclone_light::LightLayer::Sky)
                .unwrap()
                .unwrap();
        assert_eq!(sky.get(8, 8, 8), 0);
    }

    #[test]
    fn provisional_sky_light_matches_java_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        assert_eq!(fixture["module"], "synthetic-light");
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");
        assert_eq!(min_y, -SECTION_HEIGHT);
        assert_eq!(height, SECTION_HEIGHT * 3);

        for case_name in ["openColumn", "fullRoof", "singleOverhang", "stoneRoom"] {
            let case = synthetic_light_case(&fixture, case_name);
            let blocks = native_blocks_from_synthetic_light_case(case, min_y, height);
            let sections = provisional_sky_light_sections(min_y, height, &blocks);

            assert_synthetic_light_samples(case, ChunkPos::new(0, 0), &sections);
            let java_section = synthetic_light_section(case, 0, 0, 0);
            assert_eq!(
                sky_section_hex(&sections, 0),
                java_section["dataHex"]
                    .as_str()
                    .expect("synthetic light dataHex must be a string"),
                "sky DataLayer mismatch for synthetic light case `{case_name}`"
            );
        }
    }

    #[test]
    fn provisional_sky_light_matches_java_cross_chunk_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");
        let case = synthetic_light_case(&fixture, "fullRoofEastOpenNeighbor");
        let chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);
        let target_pos = ChunkPos::new(0, 0);
        let sections = provisional_sky_light_sections_for_chunk(
            target_pos,
            min_y,
            height,
            chunks.iter().map(|(pos, blocks)| (*pos, blocks.as_slice())),
        );

        assert_synthetic_light_samples(case, target_pos, &sections);
        assert_eq!(sample_sky_light(&sections, 15, 13, 8), 14);
        assert_eq!(sample_sky_light(&sections, 14, 13, 8), 13);

        let java_section = synthetic_light_section(case, 0, 0, 0);
        assert_eq!(
            sky_section_hex(&sections, 0),
            java_section["dataHex"]
                .as_str()
                .expect("synthetic light dataHex must be a string")
        );

        let local_only_sections =
            provisional_sky_light_sections(min_y, height, chunks.get(&target_pos).unwrap());
        assert_eq!(sample_sky_light(&local_only_sections, 15, 13, 8), 0);
    }

    #[test]
    fn provisional_block_light_matches_java_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");

        for case_name in ["lavaOpen", "lavaBlockedByStone"] {
            let case = synthetic_block_light_case(&fixture, case_name);
            let chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);
            let target_pos = ChunkPos::new(0, 0);
            let sections = provisional_block_light_sections_for_chunk(
                target_pos,
                min_y,
                height,
                chunks.iter().map(|(pos, blocks)| (*pos, blocks.as_slice())),
            );

            assert_synthetic_block_light_samples(case, target_pos, &sections);
            let java_section = synthetic_block_light_section(case, 0, 0, 0);
            assert_eq!(
                block_section_hex(&sections, 0),
                java_section["dataHex"]
                    .as_str()
                    .expect("synthetic block light dataHex must be a string"),
                "block DataLayer mismatch for synthetic light case `{case_name}`"
            );
        }
    }

    #[test]
    fn provisional_block_light_matches_java_cross_chunk_synthetic_fixture() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let min_y = fixture_i32(&fixture["level"], "minY");
        let height = fixture_i32(&fixture["level"], "height");
        let case = synthetic_block_light_case(&fixture, "lavaCrossChunk");
        let chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);

        for target_pos in [ChunkPos::new(0, 0), ChunkPos::new(1, 0)] {
            let sections = provisional_block_light_sections_for_chunk(
                target_pos,
                min_y,
                height,
                chunks.iter().map(|(pos, blocks)| (*pos, blocks.as_slice())),
            );

            assert_synthetic_block_light_samples(case, target_pos, &sections);
            let java_section = synthetic_block_light_section(case, target_pos.x, 0, target_pos.z);
            assert_eq!(
                block_section_hex(&sections, 0),
                java_section["dataHex"]
                    .as_str()
                    .expect("synthetic block light dataHex must be a string")
            );
        }
    }

    #[test]
    fn snapshot_with_provisional_lighting_packs_block_light_layer() {
        let min_y = -SECTION_HEIGHT;
        let height = SECTION_HEIGHT * 3;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1 - min_y, 1)] = LAVA;
        let block_state_ids = blocks
            .iter()
            .map(|block_id| generated_block_state_id(*block_id))
            .collect::<Vec<_>>();
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkRevision(1),
            min_y,
            height,
            &block_state_ids,
        );

        let snapshot = snapshot_with_provisional_lighting(snapshot, &blocks);

        assert_eq!(sample_block_light(&snapshot.light_sections, 1, 1, 1), 15);
        assert_eq!(sample_block_light(&snapshot.light_sections, 2, 1, 1), 14);
    }

    #[test]
    fn synthetic_light_fixture_records_cross_chunk_boundary_case() {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../test/fixtures/lighting/sky-synthetic.json"
        ))
        .unwrap();
        let cross_light_case = synthetic_light_case(&fixture, "fullRoofEastOpenNeighbor");
        assert_eq!(cross_light_case["skySections"].as_array().unwrap().len(), 6);
        assert_eq!(
            cross_light_case["samples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|sample| {
                    fixture_i32(sample, "x") == 15
                        && fixture_i32(sample, "y") == 13
                        && fixture_i32(sample, "z") == 8
                })
                .map(|sample| fixture_i32(sample, "sky")),
            Some(14)
        );

        let case = synthetic_light_case(&fixture, "chunkBoundaryOverhang");

        assert_eq!(case["skySections"].as_array().unwrap().len(), 6);
        assert_eq!(
            case["samples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|sample| {
                    fixture_i32(sample, "x") == 16
                        && fixture_i32(sample, "y") == 13
                        && fixture_i32(sample, "z") == 0
                })
                .map(|sample| fixture_i32(sample, "sky")),
            Some(15)
        );
    }

    fn synthetic_light_case<'a>(fixture: &'a Value, name: &str) -> &'a Value {
        fixture["cases"]
            .as_array()
            .expect("synthetic light fixture cases must be an array")
            .iter()
            .find(|case| case["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("synthetic light fixture missing case `{name}`"))
    }

    fn synthetic_block_light_case<'a>(fixture: &'a Value, name: &str) -> &'a Value {
        fixture["blockCases"]
            .as_array()
            .expect("synthetic light fixture blockCases must be an array")
            .iter()
            .find(|case| case["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("synthetic light fixture missing block case `{name}`"))
    }

    fn synthetic_light_section(
        case: &Value,
        section_x: i32,
        section_y: i32,
        section_z: i32,
    ) -> &Value {
        synthetic_light_section_from(case, "skySections", section_x, section_y, section_z)
    }

    fn synthetic_block_light_section(
        case: &Value,
        section_x: i32,
        section_y: i32,
        section_z: i32,
    ) -> &Value {
        synthetic_light_section_from(case, "blockSections", section_x, section_y, section_z)
    }

    fn synthetic_light_section_from<'a>(
        case: &'a Value,
        key: &str,
        section_x: i32,
        section_y: i32,
        section_z: i32,
    ) -> &'a Value {
        case[key]
            .as_array()
            .unwrap_or_else(|| panic!("synthetic light case {key} must be an array"))
            .iter()
            .find(|section| {
                fixture_i32(section, "sectionX") == section_x
                    && fixture_i32(section, "sectionY") == section_y
                    && fixture_i32(section, "sectionZ") == section_z
            })
            .unwrap_or_else(|| {
                panic!(
                    "synthetic light case missing section ({section_x}, {section_y}, {section_z})"
                )
            })
    }

    fn native_blocks_from_synthetic_light_case(
        case: &Value,
        min_y: i32,
        height: i32,
    ) -> Vec<RawBlockId> {
        let mut chunks = native_chunk_blocks_from_synthetic_light_case(case, min_y, height);
        chunks
            .remove(&ChunkPos::new(0, 0))
            .expect("synthetic light case must include chunk (0, 0)")
    }

    fn native_chunk_blocks_from_synthetic_light_case(
        case: &Value,
        min_y: i32,
        height: i32,
    ) -> BTreeMap<ChunkPos, Vec<RawBlockId>> {
        let mut chunks = BTreeMap::new();
        for chunk in case["chunks"]
            .as_array()
            .expect("synthetic light case chunks must be an array")
        {
            let coords = chunk
                .as_array()
                .expect("synthetic light chunk entry must be an array");
            assert_eq!(coords.len(), 2);
            let chunk_pos = ChunkPos::new(
                coords[0].as_i64().unwrap() as i32,
                coords[1].as_i64().unwrap() as i32,
            );
            chunks.insert(
                chunk_pos,
                vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize],
            );
        }

        for cell in case["opaque"]
            .as_array()
            .expect("synthetic light case opaque must be an array")
        {
            let coords = cell
                .as_array()
                .expect("synthetic light opaque cell must be an array");
            assert_eq!(coords.len(), 3);
            let x = coords[0].as_i64().unwrap() as i32;
            let y = coords[1].as_i64().unwrap() as i32;
            let z = coords[2].as_i64().unwrap() as i32;
            let pos = WorldBlockPos::new(x, y, z);
            let chunk_pos = pos.chunk_pos();
            let blocks = chunks
                .get_mut(&chunk_pos)
                .unwrap_or_else(|| panic!("opaque cell ({x}, {y}, {z}) had no loaded chunk"));
            assert!((min_y..min_y + height).contains(&y));
            blocks[chunk_block_index(local_block_coord(x), y - min_y, local_block_coord(z))] =
                STONE;
        }
        if let Some(lava_cells) = case.get("lava").and_then(Value::as_array) {
            for cell in lava_cells {
                let coords = cell
                    .as_array()
                    .expect("synthetic light lava cell must be an array");
                assert_eq!(coords.len(), 3);
                let x = coords[0].as_i64().unwrap() as i32;
                let y = coords[1].as_i64().unwrap() as i32;
                let z = coords[2].as_i64().unwrap() as i32;
                let pos = WorldBlockPos::new(x, y, z);
                let chunk_pos = pos.chunk_pos();
                let blocks = chunks
                    .get_mut(&chunk_pos)
                    .unwrap_or_else(|| panic!("lava cell ({x}, {y}, {z}) had no loaded chunk"));
                assert!((min_y..min_y + height).contains(&y));
                blocks[chunk_block_index(local_block_coord(x), y - min_y, local_block_coord(z))] =
                    LAVA;
            }
        }
        chunks
    }

    fn assert_synthetic_light_samples(
        case: &Value,
        target_pos: ChunkPos,
        sections: &[PackedLightSection],
    ) {
        let case_name = case["name"].as_str().unwrap();
        let mut checked = 0;
        for sample in case["samples"]
            .as_array()
            .expect("synthetic light case samples must be an array")
        {
            let x = fixture_i32(sample, "x");
            let y = fixture_i32(sample, "y");
            let z = fixture_i32(sample, "z");
            if WorldBlockPos::new(x, y, z).chunk_pos() != target_pos {
                continue;
            }
            let expected_sky = fixture_i32(sample, "sky") as u8;
            assert_eq!(
                sample_sky_light(sections, x, y, z),
                expected_sky,
                "sky sample mismatch for synthetic light case `{case_name}` at ({x}, {y}, {z})"
            );
            assert_eq!(
                fixture_i32(sample, "block"),
                0,
                "synthetic sky fixture should not emit block light"
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "synthetic light case `{case_name}` had no samples in target chunk {target_pos:?}"
        );
    }

    fn assert_synthetic_block_light_samples(
        case: &Value,
        target_pos: ChunkPos,
        sections: &[PackedLightSection],
    ) {
        let case_name = case["name"].as_str().unwrap();
        let mut checked = 0;
        for sample in case["samples"]
            .as_array()
            .expect("synthetic light case samples must be an array")
        {
            let x = fixture_i32(sample, "x");
            let y = fixture_i32(sample, "y");
            let z = fixture_i32(sample, "z");
            if WorldBlockPos::new(x, y, z).chunk_pos() != target_pos {
                continue;
            }
            let expected_block = fixture_i32(sample, "block") as u8;
            assert_eq!(
                sample_block_light(sections, x, y, z),
                expected_block,
                "block sample mismatch for synthetic light case `{case_name}` at ({x}, {y}, {z})"
            );
            assert_eq!(
                fixture_i32(sample, "sky"),
                0,
                "synthetic block fixture should not contain sky light"
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "synthetic light case `{case_name}` had no block samples in target chunk {target_pos:?}"
        );
    }

    fn sample_sky_light(sections: &[PackedLightSection], x: i32, y: i32, z: i32) -> u8 {
        sample_light(sections, LightLayer::Sky, x, y, z)
    }

    fn sample_block_light(sections: &[PackedLightSection], x: i32, y: i32, z: i32) -> u8 {
        sample_light(sections, LightLayer::Block, x, y, z)
    }

    fn sample_light(
        sections: &[PackedLightSection],
        layer: LightLayer,
        x: i32,
        y: i32,
        z: i32,
    ) -> u8 {
        let section_y = block_to_section_coord(y);
        let Some(section) = sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return 0;
        };
        let Some(data_layer) = mclone_light::packed_light_section_layer(section, layer).unwrap()
        else {
            return 0;
        };
        data_layer.get(
            local_block_coord(x),
            local_section_block_coord(y),
            local_block_coord(z),
        )
    }

    fn sky_section_hex(sections: &[PackedLightSection], section_y: i32) -> String {
        light_section_hex(sections, section_y, LightLayer::Sky)
    }

    fn block_section_hex(sections: &[PackedLightSection], section_y: i32) -> String {
        light_section_hex(sections, section_y, LightLayer::Block)
    }

    fn light_section_hex(
        sections: &[PackedLightSection],
        section_y: i32,
        layer: LightLayer,
    ) -> String {
        let section = sections
            .iter()
            .find(|section| section.section_y == section_y)
            .unwrap_or_else(|| panic!("missing light section {section_y}"));
        let data_layer = mclone_light::packed_light_section_layer(section, layer)
            .unwrap()
            .unwrap_or_else(mclone_light::DataLayer::new);
        data_layer_hex(&data_layer)
    }

    fn data_layer_hex(layer: &mclone_light::DataLayer) -> String {
        let bytes = layer
            .as_bytes()
            .map(|bytes| bytes.as_slice())
            .unwrap_or(&[0; mclone_light::DATA_LAYER_SIZE]);
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(&mut hex, "{byte:02x}").unwrap();
        }
        hex
    }

    fn handle_command_and_poll(
        server: &mut IntegratedServer,
        command: ClientCommand,
    ) -> Vec<ServerUpdate> {
        let mut updates = server.handle_command(command);
        updates.extend(poll_server_until_idle(server));
        updates
    }

    fn try_handle_command_and_poll(
        server: &mut IntegratedServer,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let mut updates = server.try_handle_command(command)?;
        updates.extend(try_poll_server_until_idle(server)?);
        Ok(updates)
    }

    fn poll_server_until_idle(server: &mut IntegratedServer) -> Vec<ServerUpdate> {
        try_poll_server_until_idle(server).unwrap()
    }

    fn try_poll_server_until_idle(
        server: &mut IntegratedServer,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let mut updates = Vec::new();
        for _ in 0..60_000 {
            updates.extend(server.try_poll()?);
            if server.pending_job_count() == 0 {
                return Ok(updates);
            }
            if server.pending_publication_count() == 0 {
                wait_for_server_completion(server);
            }
        }
        panic!("timed out waiting for integrated server worldgen jobs");
    }

    fn assert_liquid_oracle_fixture_matches(server: &IntegratedServer, fixture_json: &str) {
        let fixture = serde_json::from_str::<Value>(fixture_json).unwrap();
        let bounds = &fixture["bounds"];
        let min_x = fixture_i32(bounds, "minX");
        let min_y = fixture_i32(bounds, "minY");
        let min_z = fixture_i32(bounds, "minZ");
        let size_x = fixture_i32(bounds, "sizeX");
        let size_y = fixture_i32(bounds, "sizeY");
        let size_z = fixture_i32(bounds, "sizeZ");
        let palette = fixture["palette"]
            .as_array()
            .expect("liquid fixture palette must be an array")
            .iter()
            .map(fixture_palette_block_id)
            .collect::<Vec<_>>();
        let blocks = fixture["blocks"]
            .as_array()
            .expect("liquid fixture blocks must be an array");
        assert_eq!(
            blocks.len(),
            (size_x * size_y * size_z) as usize,
            "liquid fixture block volume should match bounds"
        );

        for (index, block) in blocks.iter().enumerate() {
            let index = index as i32;
            let x = index % size_x;
            let z = (index / size_x) % size_z;
            let y = index / (size_x * size_z);
            let palette_index = block
                .as_u64()
                .expect("liquid fixture block entry must be a palette index")
                as usize;
            let expected = palette[palette_index];
            let pos = WorldBlockPos::new(min_x + x, min_y + y, min_z + z);
            assert_eq!(
                server.scheduler().block_at_world(pos),
                Some(expected),
                "liquid fixture block mismatch at {pos:?}"
            );
        }

        let mut expected_ticks = fixture["liquidTicks"]
            .as_array()
            .expect("liquid fixture liquidTicks must be an array")
            .iter()
            .map(|tick| {
                (
                    WorldBlockPos::new(
                        fixture_i32(tick, "x"),
                        fixture_i32(tick, "y"),
                        fixture_i32(tick, "z"),
                    ),
                    FluidKind::from_target(
                        tick["target"]
                            .as_str()
                            .expect("liquid tick target must be a string"),
                    )
                    .expect("liquid tick target must be a known fluid"),
                    fixture_i32(tick, "delay"),
                )
            })
            .collect::<Vec<_>>();
        expected_ticks.sort();
        assert_eq!(
            server
                .liquid_ticks
                .scheduled_tick_entries(server.simulation_tick()),
            expected_ticks
        );
    }

    fn new_liquid_oracle_server() -> IntegratedServer {
        new_liquid_oracle_server_with_radius(0)
    }

    fn new_liquid_oracle_server_with_radius(radius_chunks: u32) -> IntegratedServer {
        let mut server = IntegratedServer::new(12_345);
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: radius_chunks,
                chunk_tracking_radius: radius_chunks,
            }),
        );
        server.liquid_ticks = FluidTickList::new();
        server
    }

    fn fill_blocks(
        server: &mut IntegratedServer,
        min: WorldBlockPos,
        max: WorldBlockPos,
        block: RawBlockId,
    ) {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    server
                        .scheduler_mut()
                        .set_block_at_world(WorldBlockPos::new(x, y, z), block);
                }
            }
        }
    }

    fn run_simulation_ticks(server: &mut IntegratedServer, ticks: usize) {
        for _ in 0..ticks {
            server.simulation_tick_report();
        }
    }

    fn setup_liquid_slope(
        server: &mut IntegratedServer,
        source_block: RawBlockId,
        fluid: FluidKind,
    ) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(12, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(12, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 1),
            WorldBlockPos::new(12, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 3),
            WorldBlockPos::new(12, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 2),
            WorldBlockPos::new(0, 81, 2),
            STONE,
        );

        let source = WorldBlockPos::new(1, 80, 2);
        server
            .scheduler_mut()
            .set_block_at_world(source, source_block);
        server.schedule_fluid_tick(source, fluid, fluid.tick_delay());
    }

    fn setup_water_slope(server: &mut IntegratedServer) {
        setup_liquid_slope(server, WATER, FluidKind::Water);
    }

    fn setup_lava_slope(server: &mut IntegratedServer) {
        setup_liquid_slope(server, LAVA, FluidKind::Lava);
    }

    fn setup_cross_chunk_water_slope(server: &mut IntegratedServer) {
        fill_blocks(
            server,
            WorldBlockPos::new(14, 78, 0),
            WorldBlockPos::new(18, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 79, 0),
            WorldBlockPos::new(18, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 80, 1),
            WorldBlockPos::new(18, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 80, 3),
            WorldBlockPos::new(18, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(14, 80, 2),
            WorldBlockPos::new(14, 81, 2),
            STONE,
        );

        let source = WorldBlockPos::new(15, 80, 2);
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.schedule_fluid_tick(source, FluidKind::Water, FluidKind::Water.tick_delay());
    }

    fn setup_liquid_fall(
        server: &mut IntegratedServer,
        source_block: RawBlockId,
        fluid: FluidKind,
    ) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(4, 88, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(4, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 0),
            WorldBlockPos::new(0, 88, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(4, 80, 0),
            WorldBlockPos::new(4, 88, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 0),
            WorldBlockPos::new(4, 88, 0),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 4),
            WorldBlockPos::new(4, 88, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(1, 80, 1),
            WorldBlockPos::new(3, 87, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(1, 80, 3),
            WorldBlockPos::new(3, 87, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(1, 80, 2),
            WorldBlockPos::new(1, 87, 2),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(3, 80, 2),
            WorldBlockPos::new(3, 87, 2),
            STONE,
        );

        let source = WorldBlockPos::new(2, 86, 2);
        server
            .scheduler_mut()
            .set_block_at_world(source, source_block);
        server.schedule_fluid_tick(source, fluid, fluid.tick_delay());
    }

    fn setup_water_fall(server: &mut IntegratedServer) {
        setup_liquid_fall(server, WATER, FluidKind::Water);
    }

    fn setup_lava_fall(server: &mut IntegratedServer) {
        setup_liquid_fall(server, LAVA, FluidKind::Lava);
    }

    fn setup_water_source_conversion(server: &mut IntegratedServer) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(4, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(4, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 1),
            WorldBlockPos::new(4, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 3),
            WorldBlockPos::new(4, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 2),
            WorldBlockPos::new(0, 81, 2),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(4, 80, 2),
            WorldBlockPos::new(4, 81, 2),
            STONE,
        );

        for source in [WorldBlockPos::new(1, 80, 2), WorldBlockPos::new(3, 80, 2)] {
            server.scheduler_mut().set_block_at_world(source, WATER);
            server.schedule_fluid_tick(source, FluidKind::Water, 5);
        }
    }

    fn setup_lava_source_water_contact(server: &mut IntegratedServer) {
        fill_blocks(
            server,
            WorldBlockPos::new(0, 78, 0),
            WorldBlockPos::new(4, 84, 4),
            AIR,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 79, 0),
            WorldBlockPos::new(4, 79, 4),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 1),
            WorldBlockPos::new(4, 81, 1),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 3),
            WorldBlockPos::new(4, 81, 3),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(0, 80, 2),
            WorldBlockPos::new(0, 81, 2),
            STONE,
        );
        fill_blocks(
            server,
            WorldBlockPos::new(4, 80, 2),
            WorldBlockPos::new(4, 81, 2),
            STONE,
        );

        let lava = WorldBlockPos::new(1, 80, 2);
        let water = WorldBlockPos::new(2, 80, 2);
        server.scheduler_mut().set_block_at_world(lava, LAVA);
        server.schedule_fluid_tick(lava, FluidKind::Lava, FluidKind::Lava.tick_delay());
        server.scheduler_mut().set_block_at_world(water, WATER);
        server.schedule_fluid_tick(water, FluidKind::Water, FluidKind::Water.tick_delay());
        assert!(
            server.scheduler_mut().resolve_lava_source_contact_at(lava),
            "water neighbor should convert lava source to obsidian"
        );
    }

    fn fixture_i32(value: &Value, key: &str) -> i32 {
        value[key]
            .as_i64()
            .unwrap_or_else(|| panic!("fixture field `{key}` must be an integer")) as i32
    }

    fn fixture_palette_block_id(entry: &Value) -> RawBlockId {
        let name = entry["name"]
            .as_str()
            .expect("fixture palette entry name must be a string");
        match name {
            "minecraft:air" => AIR,
            "minecraft:stone" => STONE,
            "minecraft:obsidian" => OBSIDIAN,
            "minecraft:water" => {
                let level = entry
                    .get("properties")
                    .and_then(|properties| properties.get("level"))
                    .and_then(Value::as_str)
                    .unwrap_or("0")
                    .parse::<u8>()
                    .expect("fixture water level must be a u8");
                water_block_for_level(level).expect("fixture water level must be supported")
            }
            "minecraft:lava" => {
                let level = entry
                    .get("properties")
                    .and_then(|properties| properties.get("level"))
                    .and_then(Value::as_str)
                    .unwrap_or("0")
                    .parse::<u8>()
                    .expect("fixture lava level must be a u8");
                lava_block_for_level(level).expect("fixture lava level must be supported")
            }
            _ => panic!("unsupported liquid fixture palette entry `{name}`"),
        }
    }

    fn wait_for_scheduler_completion(scheduler: &mut ChunkScheduler) {
        scheduler.wait_for_worldgen_completion(std::time::Duration::from_secs(30));
    }

    fn wait_for_server_completion(server: &mut IntegratedServer) {
        server.wait_for_worldgen_completion(std::time::Duration::from_secs(30));
    }

    fn active_ticket_square_count(ticket_level: i32) -> usize {
        let radius = usize::try_from(MAX_CHUNK_DISTANCE - ticket_level)
            .expect("ticket level must be within active distance");
        let side = radius * 2 + 1;
        side * side
    }

    fn square_side_for_radius(radius: u32) -> usize {
        usize::try_from(radius).expect("chunk radius must fit usize") * 2 + 1
    }

    fn player_status_counts(radius: u32) -> (usize, usize, usize, usize, usize) {
        let entity_ticking = square_side_for_radius(radius).pow(2);
        let block_ticking = square_side_for_radius(radius + 1).pow(2);
        let ticking = block_ticking - entity_ticking;
        let border_outer = square_side_for_radius(radius + 2).pow(2);
        let border = border_outer - block_ticking;
        let active = square_side_for_radius(
            radius + u32::try_from(MAX_CHUNK_DISTANCE - PLAYER_TICKET_LEVEL).unwrap(),
        )
        .pow(2);
        let inaccessible = active - border_outer;
        (inaccessible, border, ticking, entity_ticking, block_ticking)
    }

    fn chunk_square(center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
        (-radius..=radius)
            .flat_map(|z| {
                (-radius..=radius).map(move |x| ChunkPos::new(center.x + x, center.z + z))
            })
            .collect()
    }

    fn without_fluid_tick_events(events: &[ChunkSchedulerEvent]) -> Vec<ChunkSchedulerEvent> {
        events
            .iter()
            .filter(|event| !matches!(event, ChunkSchedulerEvent::FluidTickScheduled { .. }))
            .cloned()
            .collect()
    }

    fn status_event_count(
        events: &[ChunkSchedulerEvent],
        status: ChunkStatus,
        step: ChunkStatusStep,
    ) -> usize {
        events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    ChunkSchedulerEvent::StatusChanged {
                        status: event_status,
                        step: event_step,
                        ..
                    } if *event_status == status && *event_step == step
                )
            })
            .count()
    }

    fn snapshot_ready_count(events: &[ChunkSchedulerEvent]) -> usize {
        events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
            .count()
    }

    fn snapshot_update_for(updates: &[ServerUpdate], pos: ChunkPos) -> Option<&ChunkSnapshot> {
        updates.iter().rev().find_map(|update| match update {
            ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
            _ => None,
        })
    }

    fn apply_section_updates_to_snapshot(
        snapshot: &mut ChunkSnapshot,
        updates: &[ServerUpdate],
    ) -> usize {
        let mut applied = 0;
        for update in updates {
            let ServerUpdate::SectionBlockUpdates {
                pos,
                section_y,
                updates,
            } = update
            else {
                continue;
            };
            if *pos != snapshot.pos {
                continue;
            }
            for update in updates {
                if snapshot.patch_section_block(
                    *section_y,
                    update.local_x as i32,
                    update.local_y as i32,
                    update.local_z as i32,
                    update.block_state,
                ) {
                    applied += 1;
                }
            }
        }
        applied
    }

    fn snapshot_block_state(snapshot: &ChunkSnapshot, pos: WorldBlockPos) -> BlockStateId {
        assert_eq!(snapshot.pos, pos.chunk_pos());
        assert!(
            pos.y >= snapshot.min_y && pos.y < snapshot.min_y + snapshot.height,
            "world y {} is outside snapshot range {}..{}",
            pos.y,
            snapshot.min_y,
            snapshot.min_y + snapshot.height
        );

        let section_y = block_to_section_coord(pos.y);
        let section_local_y = local_section_block_coord(pos.y);
        let local_x = local_block_coord(pos.x);
        let local_z = local_block_coord(pos.z);
        let Some(section) = snapshot
            .sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return BlockStateId(0);
        };
        let blocks = section.unpack_block_state_ids();
        blocks[chunk_section_index(local_x, section_local_y, local_z)]
    }

    #[test]
    fn distinguishes_integrated_and_dedicated_modes() {
        assert_ne!(ServerMode::Integrated, ServerMode::Dedicated);
    }

    #[test]
    fn chunk_scheduler_uses_platform_worldgen_mailbox() {
        let scheduler = ChunkScheduler::new(12_345);

        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(
            scheduler.worldgen_mailbox_kind(),
            WorldgenMailboxKind::NativeThread
        );
        #[cfg(target_arch = "wasm32")]
        assert_eq!(
            scheduler.worldgen_mailbox_kind(),
            WorldgenMailboxKind::Inline
        );
    }

    #[test]
    fn integrated_server_publishes_interested_chunks() {
        let mut server = IntegratedServer::new(12_345);

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 1,
                chunk_tracking_radius: 1,
            }),
        );

        assert_eq!(updates.len(), 9);
        assert_eq!(server.loaded_chunk_count(), 25);
        assert_eq!(server.scheduler().client_visible_chunk_count(), 9);
        assert_eq!(server.scheduler().holder_count(), 29 * 29);
        assert_eq!(server.scheduler().active_ticketed_chunk_count(), 29 * 29);
        assert_eq!(
            (
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::Inaccessible),
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::Border),
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::Ticking),
                server
                    .scheduler()
                    .full_status_chunk_count(FullChunkStatus::EntityTicking),
                server.scheduler().block_ticking_chunk_count(),
                server.scheduler().entity_ticking_chunk_count(),
            ),
            (792, 24, 16, 9, 25, 9)
        );
        assert_eq!(server.scheduler().ready_dependency_chunk_count(), 23 * 23);
        assert_eq!(
            server.scheduler().metrics(),
            ChunkSchedulerMetrics {
                direct_ticket_chunks: 9,
                active_ticket_chunks: 29 * 29,
                holder_chunks: 29 * 29,
                pending_unload_chunks: 0,
                inaccessible_status_chunks: 792,
                border_status_chunks: 24,
                ticking_status_chunks: 16,
                entity_ticking_status_chunks: 9,
                block_ticking_chunks: 25,
                client_visible_chunks: 9,
                loaded_snapshot_chunks: 25,
                dependency_holder_chunks: 29 * 29 - 25,
                ready_dependency_chunks: 23 * 23,
                dirty_chunks: 25,
                pending_jobs: 0,
                completed_jobs: 1,
                total_seeded_dependency_chunks: 0,
                total_dependency_cache_hits: 0,
                total_dependency_cache_misses: 23 * 23,
                total_retained_dependency_chunks: 23 * 23,
            }
        );
        assert_eq!(server.scheduler().job_count(), 1);
        let job = server.scheduler().jobs().next().unwrap();
        assert_eq!(job.id, ChunkJobId(1));
        assert_eq!(job.status, ChunkStatus::Features);
        assert_eq!(job.state, ChunkJobState::Complete);
        assert_eq!(job.target_chunks.len(), 25);
        assert_eq!(job.feature_centers.len(), 7 * 7);
        assert_eq!(job.dependency_chunks.len(), 23 * 23);
        assert_eq!(job.seeded_dependency_chunks, 0);
        assert_eq!(job.dependency_cache_hits, 0);
        assert_eq!(job.dependency_cache_misses, 23 * 23);
        assert_eq!(job.retained_dependency_chunks, 23 * 23);
        assert!(job.dependency_chunks.contains(&ChunkPos::new(-11, -11)));
        assert!(job.dependency_chunks.contains(&ChunkPos::new(11, 11)));
        for target in &job.target_chunks {
            assert_eq!(
                server
                    .scheduler()
                    .holder(*target)
                    .unwrap()
                    .status_slot(ChunkStatus::Features)
                    .unwrap()
                    .job_id,
                Some(job.id)
            );
        }
        assert!(
            updates
                .iter()
                .all(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
        );
    }

    #[test]
    fn player_ticket_levels_define_runtime_status_lanes() {
        let mut scheduler = ChunkScheduler::new(12_345);

        scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(player_status_counts(0), (704, 16, 8, 1, 9));
        assert_eq!(
            (
                scheduler.full_status_chunk_count(FullChunkStatus::Inaccessible),
                scheduler.full_status_chunk_count(FullChunkStatus::Border),
                scheduler.full_status_chunk_count(FullChunkStatus::Ticking),
                scheduler.full_status_chunk_count(FullChunkStatus::EntityTicking),
                scheduler.block_ticking_chunk_count(),
                scheduler.entity_ticking_chunk_count(),
            ),
            (704, 16, 8, 1, 9, 1)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(0, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::EntityTicking)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(1, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::Ticking)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(2, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::Border)
        );
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(3, 0))
                .map(ChunkHolder::full_status),
            Some(FullChunkStatus::Inaccessible)
        );
        assert!(scheduler.holder(ChunkPos::new(14, 0)).is_none());
    }

    #[test]
    fn client_visibility_is_separate_from_ticking_status() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );
        assert!(
            scheduler
                .holder(ChunkPos::new(0, 0))
                .unwrap()
                .is_client_visible()
        );

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        let old_center = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert!(!old_center.is_client_visible());
        assert_eq!(old_center.full_status(), FullChunkStatus::Ticking);
        assert_eq!(scheduler.client_visible_chunk_count(), 1);
        assert_eq!(scheduler.entity_ticking_chunk_count(), 1);
        assert_eq!(scheduler.block_ticking_chunk_count(), 9);
    }

    #[test]
    fn scheduler_tick_report_lists_runtime_lanes_without_simulation() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        let report = scheduler.tick_report().unwrap();

        assert_eq!(report.ticket_tick, 1);
        assert_eq!(
            report.block_ticking_chunks,
            chunk_square(ChunkPos::new(0, 0), 1)
        );
        assert_eq!(report.entity_ticking_chunks, vec![ChunkPos::new(0, 0)]);
        assert_eq!(report.pending_unloads_processed, 0);
        assert!(report.events.is_empty());
    }

    #[test]
    fn integrated_server_tick_report_exposes_protocol_updates_and_lanes() {
        let mut server = IntegratedServer::new(12_345);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        let report = server.tick_report();

        assert_eq!(report.ticket_tick, 1);
        assert_eq!(
            report.block_ticking_chunks,
            chunk_square(ChunkPos::new(0, 0), 1)
        );
        assert_eq!(report.entity_ticking_chunks, vec![ChunkPos::new(0, 0)]);
        assert_eq!(report.pending_unloads_processed, 0);
        assert!(report.updates.is_empty());
    }

    #[test]
    fn integrated_server_simulation_tick_report_records_phases() {
        let mut server = IntegratedServer::new(12_345);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let before_metrics = server.scheduler().metrics();

        let report = server.simulation_tick_report();

        assert_eq!(report.simulation_tick, 1);
        assert_eq!(report.chunk_tick, 1);
        assert_eq!(report.block_tick_chunks, 9);
        assert!(report.fluid_ticks_executed > 0);
        assert_eq!(report.entity_tick_chunks, 1);
        assert_eq!(report.pending_unloads_processed, 0);
        assert_eq!(
            server.scheduler().metrics().block_ticking_chunks,
            before_metrics.block_ticking_chunks
        );
    }

    #[test]
    fn fluid_tick_list_dedupes_position_and_fluid() {
        let mut ticks = FluidTickList::new();
        let pos = WorldBlockPos::new(8, 120, 8);

        ticks.schedule_tick(pos, FluidKind::Water, 5, 10);
        ticks.schedule_tick(pos, FluidKind::Water, 1, 10);
        ticks.schedule_tick(pos, FluidKind::Lava, 1, 10);

        assert_eq!(ticks.size(), 2);
        assert!(ticks.has_scheduled_tick(pos, FluidKind::Water));
        assert!(ticks.has_scheduled_tick(pos, FluidKind::Lava));
    }

    #[test]
    fn fluid_kind_accepts_generated_and_persisted_tick_targets() {
        assert_eq!(
            FluidKind::from_target("minecraft:water"),
            Some(FluidKind::Water)
        );
        assert_eq!(
            FluidKind::from_target("minecraft:flowing_water"),
            Some(FluidKind::Water)
        );
        assert_eq!(
            FluidKind::from_target("minecraft:lava"),
            Some(FluidKind::Lava)
        );
        assert_eq!(
            FluidKind::from_target("minecraft:flowing_lava"),
            Some(FluidKind::Lava)
        );
        assert_eq!(FluidKind::from_target("minecraft:empty"), None);
    }

    #[test]
    fn generated_liquid_ticks_are_registered_when_watery_chunk_is_published() {
        let mut server = IntegratedServer::new(12_345);
        let center = ChunkPos::new(117, -128);

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        assert!(
            snapshot_update_for(&updates, center).is_some(),
            "watery oracle chunk should publish a visible snapshot"
        );
        let scheduled_before_tick = server.scheduled_fluid_tick_count();
        assert!(
            scheduled_before_tick >= 90,
            "seed 12345 chunk (117,-128) should carry at least the 90 liquid-carved oracle ticks"
        );

        let report = server.simulation_tick_report();

        assert!(
            report.fluid_ticks_executed > 0,
            "generated liquid ticks should execute once the chunk is entity-ticking"
        );
        assert!(
            report.fluid_ticks_executed < scheduled_before_tick,
            "only liquid ticks in entity-ticking chunks should execute immediately; ticking-lane neighbors remain pending"
        );
    }

    #[test]
    fn scheduled_water_tick_spreads_down_and_publishes_section_update() {
        let mut server = IntegratedServer::new(12_345);
        let initial_updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let mut client_snapshot = snapshot_update_for(&initial_updates, ChunkPos::new(0, 0))
            .expect("initial interest should publish visible chunk snapshot")
            .clone();
        let source = WorldBlockPos::new(8, 120, 8);
        let below = source.below();

        server.scheduler_mut().set_block_at_world(source, WATER);
        server.scheduler_mut().set_block_at_world(below, AIR);
        server.schedule_fluid_tick(source, FluidKind::Water, 0);

        let report = server.simulation_tick_report();

        assert!(report.fluid_ticks_executed >= 1);
        assert!(report.fluid_mutated_blocks >= 1);
        assert!(report.scheduled_fluid_ticks >= 1);
        assert_eq!(
            server.scheduler().block_at_world(below),
            Some(WATER_LEVEL_8)
        );
        assert_eq!(
            report.fluid_snapshot_events, 0,
            "fluid mutation should no longer publish per-block chunk snapshots"
        );
        assert!(
            snapshot_update_for(&report.updates, ChunkPos::new(0, 0)).is_none(),
            "fluid mutation should publish section deltas instead of chunk snapshots"
        );
        assert!(
            apply_section_updates_to_snapshot(&mut client_snapshot, &report.updates) >= 1,
            "fluid mutation should publish at least one section block update"
        );
        assert_eq!(
            snapshot_block_state(&client_snapshot, source),
            generated_block_state_id(WATER)
        );
        assert_eq!(
            snapshot_block_state(&client_snapshot, below),
            generated_block_state_id(WATER_LEVEL_8)
        );
    }

    #[test]
    fn scheduled_water_slope_matches_oracle_fixture_after_5_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_slope(&mut server);

        run_simulation_ticks(&mut server, 5);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER_LEVEL_1)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(3, 80, 2)),
            Some(AIR)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-slope-5-ticks.json"),
        );
    }

    #[test]
    fn scheduled_water_slope_matches_oracle_fixture_after_10_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_slope(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER_LEVEL_1)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(3, 80, 2)),
            Some(WATER_LEVEL_2)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(4, 80, 2)),
            Some(AIR)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-slope-10-ticks.json"),
        );
    }

    #[test]
    fn scheduled_water_cross_chunk_slope_matches_oracle_fixture_after_10_ticks() {
        let mut server = new_liquid_oracle_server_with_radius(1);
        setup_cross_chunk_water_slope(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(15, 80, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(16, 80, 2)),
            Some(WATER_LEVEL_1)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(17, 80, 2)),
            Some(WATER_LEVEL_2)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-cross-chunk-slope-10-ticks.json"),
        );
    }

    #[test]
    fn cross_chunk_water_tick_waits_until_neighbor_is_entity_ticking() {
        let mut server = new_liquid_oracle_server();
        setup_cross_chunk_water_slope(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(1, 0))
                .unwrap()
                .full_status(),
            FullChunkStatus::Ticking
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(16, 80, 2)),
            Some(WATER_LEVEL_1),
            "entity-ticking chunk (0,0) should mutate the loaded neighbor chunk"
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(17, 80, 2)),
            Some(AIR),
            "non-entity-ticking chunk (1,0) should not run its due follow-up tick yet"
        );
        let pending_ticks = server
            .liquid_ticks
            .scheduled_tick_entries(server.simulation_tick());
        assert!(
            pending_ticks.contains(&(WorldBlockPos::new(16, 80, 2), FluidKind::Water, 0)),
            "the cross-chunk follow-up tick should remain due and pending: {pending_ticks:?}"
        );

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        run_simulation_ticks(&mut server, 1);

        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(1, 0))
                .unwrap()
                .full_status(),
            FullChunkStatus::EntityTicking
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(17, 80, 2)),
            Some(WATER_LEVEL_2),
            "the due cross-chunk tick should run after the neighbor becomes entity-ticking"
        );
    }

    #[test]
    fn scheduled_water_fall_matches_oracle_fixture_after_10_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_fall(&mut server);

        run_simulation_ticks(&mut server, 10);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 86, 2)),
            Some(WATER)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 85, 2)),
            Some(WATER_LEVEL_8)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 84, 2)),
            Some(WATER_LEVEL_8)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-fall-10-ticks.json"),
        );
    }

    #[test]
    fn scheduled_water_source_conversion_matches_oracle_fixture_after_5_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_water_source_conversion(&mut server);

        run_simulation_ticks(&mut server, 5);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/water-source-conversion-5-ticks.json"),
        );
    }

    #[test]
    fn scheduled_lava_slope_matches_oracle_fixture_after_30_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_lava_slope(&mut server);

        run_simulation_ticks(&mut server, 30);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(LAVA)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(LAVA_LEVEL_2)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/lava-slope-30-ticks.json"),
        );
    }

    #[test]
    fn scheduled_lava_fall_matches_oracle_fixture_after_60_ticks() {
        let mut server = new_liquid_oracle_server();
        setup_lava_fall(&mut server);

        run_simulation_ticks(&mut server, 60);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 86, 2)),
            Some(LAVA)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 85, 2)),
            Some(LAVA_LEVEL_8)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 84, 2)),
            Some(LAVA_LEVEL_8)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/lava-fall-60-ticks.json"),
        );
    }

    #[test]
    fn lava_source_water_contact_matches_oracle_fixture_after_1_script_tick() {
        let mut server = new_liquid_oracle_server();
        setup_lava_source_water_contact(&mut server);

        run_simulation_ticks(&mut server, 2);

        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(1, 80, 2)),
            Some(OBSIDIAN)
        );
        assert_eq!(
            server
                .scheduler()
                .block_at_world(WorldBlockPos::new(2, 80, 2)),
            Some(WATER)
        );
        assert_liquid_oracle_fixture_matches(
            &server,
            include_str!("../../../../test/fixtures/liquid/lava-source-water-contact-1-ticks.json"),
        );
    }

    #[test]
    fn scheduled_fluid_tick_waits_until_chunk_is_entity_ticking() {
        let mut server = IntegratedServer::new(12_345);
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let source = WorldBlockPos::new(8, 120, 8);
        let below = source.below();
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.scheduler_mut().set_block_at_world(below, AIR);
        server.schedule_fluid_tick(source, FluidKind::Water, 0);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        let report = server.simulation_tick_report();

        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(0, 0))
                .unwrap()
                .full_status(),
            FullChunkStatus::Ticking
        );
        assert!(
            report.scheduled_fluid_ticks >= 1,
            "the manually scheduled tick in the old non-entity-ticking chunk should remain pending"
        );
        assert_ne!(server.scheduler().block_at_world(below), Some(WATER));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn scheduled_fluid_tick_survives_fully_unloaded_chunk_until_reload() {
        let root =
            unique_temp_dir("scheduled_fluid_tick_survives_fully_unloaded_chunk_until_reload");
        let original_interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };
        let source = WorldBlockPos::new(8, 120, 8);
        let below = source.below();
        let mut server = IntegratedServer::with_chunk_store(
            12_345,
            Box::new(FilesystemChunkSnapshotStore::new(&root)),
        );
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(original_interest.clone()),
        );
        server.liquid_ticks = FluidTickList::new();
        server.scheduler_mut().set_block_at_world(source, WATER);
        server.scheduler_mut().set_block_at_world(below, AIR);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(100, 100),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );
        server
            .scheduler_mut()
            .process_pending_unloads(usize::MAX)
            .unwrap();
        assert!(
            server.scheduler().holder(ChunkPos::new(0, 0)).is_none(),
            "the original chunk should be fully removed from scheduler holders"
        );

        server.liquid_ticks = FluidTickList::new();
        server.schedule_fluid_tick(source, FluidKind::Water, 0);
        let deferred = server.simulation_tick_report();

        assert_eq!(deferred.fluid_ticks_executed, 0);
        assert_eq!(deferred.deferred_fluid_ticks, 1);
        assert_eq!(deferred.scheduled_fluid_ticks, 1);
        assert!(
            server
                .liquid_ticks
                .has_scheduled_tick(source, FluidKind::Water),
            "a due tick outside any entity-ticking holder should stay pending"
        );

        let updates =
            handle_command_and_poll(&mut server, ClientCommand::SetChunkView(original_interest));
        assert!(
            snapshot_update_for(&updates, ChunkPos::new(0, 0)).is_some(),
            "the original chunk should reload from the snapshot store"
        );
        let executed = server.simulation_tick_report();

        assert_eq!(executed.deferred_fluid_ticks, 0);
        assert_eq!(executed.fluid_ticks_executed, 1);
        assert_eq!(
            server.scheduler().block_at_world(below),
            Some(WATER_LEVEL_8),
            "the overdue fluid tick should run after the chunk becomes entity-ticking again"
        );
        let pending_after_execution = server
            .liquid_ticks
            .scheduled_tick_entries(server.simulation_tick());
        assert!(
            !pending_after_execution.contains(&(source, FluidKind::Water, 0)),
            "the overdue zero-delay source tick should be consumed: {pending_after_execution:?}"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn integrated_server_tick_uses_simulation_layer() {
        let mut server = IntegratedServer::new(12_345);

        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        assert_eq!(server.simulation_tick(), 0);
        let _ = server.tick();
        assert_eq!(server.simulation_tick(), 1);
        let _ = server.tick();
        assert_eq!(server.simulation_tick(), 2);
    }

    #[test]
    fn scheduler_tick_report_counts_bounded_pending_unload_work() {
        let mut scheduler = ChunkScheduler::new(12_345);

        scheduler
            .add_region_ticket(ChunkTicketType::Unknown, ChunkPos::new(0, 0), 0)
            .unwrap();
        poll_scheduler_until_idle(&mut scheduler);

        let first_report = scheduler.tick_report().unwrap();
        assert_eq!(first_report.ticket_tick, 1);
        assert_eq!(first_report.pending_unloads_processed, 0);
        assert!(first_report.block_ticking_chunks.is_empty());
        assert!(first_report.entity_ticking_chunks.is_empty());
        assert!(first_report.events.is_empty());

        let second_report = scheduler.tick_report().unwrap();
        assert_eq!(second_report.ticket_tick, 2);
        assert_eq!(
            second_report.pending_unloads_processed,
            DEFAULT_PENDING_UNLOAD_BUDGET
        );
        assert!(second_report.block_ticking_chunks.is_empty());
        assert!(second_report.entity_ticking_chunks.is_empty());
        assert!(second_report.events.is_empty());
        assert_eq!(
            scheduler.pending_unload_count(),
            active_ticket_square_count(CHUNK_LEVEL_FULL) - DEFAULT_PENDING_UNLOAD_BUDGET
        );
    }

    #[test]
    fn duplicate_interest_does_not_regenerate_loaded_chunks() {
        let mut server = IntegratedServer::new(12_345);
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };

        assert_eq!(
            handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest.clone()))
                .len(),
            1
        );
        assert_eq!(server.scheduler().job_count(), 1);
        assert_eq!(
            handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest)).len(),
            0
        );
        assert_eq!(server.scheduler().job_count(), 1);
    }

    #[test]
    fn adjacent_interest_reuses_retained_dependency_chunks() {
        let mut server = IntegratedServer::new(12_345);

        assert_eq!(
            handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .len(),
            1
        );
        assert_eq!(
            server.scheduler().job(ChunkJobId(1)).map(|job| (
                job.seeded_dependency_chunks,
                job.dependency_cache_hits,
                job.dependency_cache_misses,
                job.retained_dependency_chunks
            )),
            Some((0, 0, 21 * 21, 21 * 21))
        );

        assert_eq!(
            handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .len(),
            2
        );
        assert_eq!(
            server.scheduler().job(ChunkJobId(2)).map(|job| (
                job.seeded_dependency_chunks,
                job.dependency_cache_hits,
                job.dependency_cache_misses,
                job.retained_dependency_chunks
            )),
            Some((18 * 21, 18 * 21, 21, 19 * 21))
        );
    }

    #[test]
    fn chunk_scheduler_apply_interest_enqueues_features_before_poll() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.loaded_chunk_count(), 0);
        assert_eq!(
            scheduler.job(ChunkJobId(1)).map(|job| job.state),
            Some(ChunkJobState::Running)
        );
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
        );
        assert_eq!(events.len(), 9 * 5);
        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
            9
        );

        let ready_events = poll_scheduler_until_idle(&mut scheduler);
        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert!(
            ready_events
                .iter()
                .any(|event| matches!(event, ChunkSchedulerEvent::FluidTickScheduled { .. }))
        );
        let ready_events_without_fluid = without_fluid_tick_events(&ready_events);
        assert_eq!(
            status_event_count(
                &ready_events_without_fluid,
                ChunkStatus::Features,
                ChunkStatusStep::Ready
            ),
            9
        );
        assert_eq!(snapshot_ready_count(&ready_events_without_fluid), 1);
    }

    #[test]
    fn chunk_scheduler_poll_slices_completed_publication() {
        let mut scheduler = ChunkScheduler::new(12_345);
        scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();
        wait_for_scheduler_completion(&mut scheduler);

        let first_events = scheduler.poll().unwrap();

        assert_eq!(
            scheduler.loaded_chunk_count(),
            DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET
        );
        assert!(scheduler.pending_publication_count() > 0);
        assert_eq!(scheduler.pending_job_count(), 1);
        assert!(
            without_fluid_tick_events(&first_events)
                .iter()
                .filter(|event| matches!(event, ChunkSchedulerEvent::StatusChanged { .. }))
                .count()
                <= DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET
        );

        poll_scheduler_until_idle(&mut scheduler);
        assert_eq!(scheduler.pending_publication_count(), 0);
        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 9);
    }

    #[test]
    fn chunk_interest_updates_player_tickets_and_holder_levels() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 1);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            PLAYER_TICKET_LEVEL
        );
        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.ticket_level(), PLAYER_TICKET_LEVEL);
        assert_eq!(holder.full_status(), FullChunkStatus::EntityTicking);
        assert!(holder.full_status().is_or_after(FullChunkStatus::Ticking));
        assert_eq!(events.len(), 9 * 5);

        let moved_events = scheduler
            .apply_interest(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
            .unwrap();

        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(1, 0)), 1);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            UNLOADED_CHUNK_LEVEL
        );
        assert_eq!(
            scheduler.active_ticket_level_at(ChunkPos::new(0, 0)),
            PLAYER_TICKET_LEVEL + 1
        );
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
        assert_eq!(
            scheduler
                .holder(ChunkPos::new(1, 0))
                .map(ChunkHolder::ticket_level),
            Some(PLAYER_TICKET_LEVEL)
        );
        assert!(
            moved_events
                .iter()
                .all(|event| !matches!(event, ChunkSchedulerEvent::Unloaded { .. }))
        );
    }

    #[test]
    fn duplicate_interest_while_job_running_does_not_enqueue_second_job() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };

        let first_events = scheduler.apply_interest(interest.clone()).unwrap();
        assert_eq!(first_events.len(), 9 * 5);
        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.job_count(), 1);

        let second_events = scheduler.apply_interest(interest).unwrap();
        assert_eq!(second_events, Vec::new());
        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.job_count(), 1);

        assert_eq!(
            without_fluid_tick_events(&poll_scheduler_until_idle(&mut scheduler)).len(),
            10
        );
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert_eq!(scheduler.job_count(), 1);
    }

    #[test]
    fn forced_ticket_keeps_chunk_resident_after_interest_moves() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );
        assert!(
            scheduler
                .set_chunk_forced(ChunkPos::new(0, 0), true)
                .unwrap()
                .is_empty()
        );
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 2);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            FORCED_TICKET_LEVEL
        );

        let events = apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        assert!(events.iter().any(
            |event| matches!(event, ChunkSchedulerEvent::Unloaded { pos } if *pos == ChunkPos::new(0, 0))
        ));
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
        assert!(scheduler.holder(ChunkPos::new(1, 0)).is_some());
        assert_eq!(scheduler.loaded_chunk_count(), 12);

        let events = scheduler
            .set_chunk_forced(ChunkPos::new(0, 0), false)
            .unwrap();

        assert!(events.is_empty());
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert_eq!(
            scheduler.active_ticket_level_at(ChunkPos::new(0, 0)),
            PLAYER_TICKET_LEVEL + 1
        );
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
    }

    #[test]
    fn stale_unknown_ticket_expires_on_scheduler_tick() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .add_region_ticket(ChunkTicketType::Unknown, ChunkPos::new(0, 0), 0)
            .unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_level_at(ChunkPos::new(0, 0)), 33);
        assert_eq!(
            without_fluid_tick_events(&poll_scheduler_until_idle(&mut scheduler)).len(),
            1
        );
        assert_eq!(scheduler.loaded_chunk_count(), 1);
        assert_eq!(scheduler.client_visible_chunk_count(), 0);

        assert!(scheduler.tick().unwrap().is_empty());
        assert_eq!(scheduler.ticket_tick(), 1);
        assert_eq!(scheduler.ticketed_chunk_count(), 1);

        let events = scheduler.tick().unwrap();

        assert_eq!(scheduler.ticket_tick(), 2);
        assert_eq!(scheduler.ticketed_chunk_count(), 0);
        assert!(events.is_empty());
        assert_eq!(
            scheduler.pending_unload_count(),
            active_ticket_square_count(CHUNK_LEVEL_FULL) - DEFAULT_PENDING_UNLOAD_BUDGET
        );

        scheduler.process_pending_unloads(usize::MAX).unwrap();
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());
    }

    #[test]
    fn expired_ticket_queues_pending_unload_until_processed() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let pos = ChunkPos::new(0, 0);

        scheduler
            .add_region_ticket(ChunkTicketType::Unknown, pos, 0)
            .unwrap();
        poll_scheduler_until_idle(&mut scheduler);
        let active_count = active_ticket_square_count(CHUNK_LEVEL_FULL);
        assert_eq!(scheduler.holder_count(), active_count);
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 1);

        scheduler.distance_manager.purge_stale_tickets();
        scheduler.distance_manager.purge_stale_tickets();
        assert!(scheduler.reconcile_ticketed_holders().unwrap().is_empty());

        assert_eq!(scheduler.ticketed_chunk_count(), 0);
        assert_eq!(scheduler.pending_unload_count(), active_count);
        assert_eq!(scheduler.holder_count(), active_count);
        assert!(scheduler.is_pending_unload(pos));
        assert_eq!(
            scheduler.holder(pos).map(ChunkHolder::ticket_level),
            Some(UNLOADED_CHUNK_LEVEL)
        );
        assert_eq!(scheduler.loaded_chunk_count(), 1);

        assert_eq!(scheduler.process_pending_unloads(10).unwrap(), 10);
        assert_eq!(scheduler.pending_unload_count(), active_count - 10);

        assert_eq!(
            scheduler.process_pending_unloads(usize::MAX).unwrap(),
            active_count - 10
        );
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert_eq!(scheduler.holder_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 0);
        assert_eq!(scheduler.dirty_chunk_count(), 0);
        assert!(scheduler.holder(pos).is_none());
    }

    #[test]
    fn pending_unload_holder_is_rescued_when_ticket_returns() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let pos = ChunkPos::new(0, 0);

        scheduler
            .add_region_ticket(ChunkTicketType::Unknown, pos, 0)
            .unwrap();
        poll_scheduler_until_idle(&mut scheduler);

        scheduler.distance_manager.purge_stale_tickets();
        scheduler.distance_manager.purge_stale_tickets();
        scheduler.reconcile_ticketed_holders().unwrap();
        assert!(scheduler.is_pending_unload(pos));
        assert_eq!(
            scheduler.holder(pos).map(ChunkHolder::ticket_level),
            Some(UNLOADED_CHUNK_LEVEL)
        );

        assert!(!scheduler.set_chunk_forced(pos, true).unwrap().is_empty());
        poll_scheduler_until_idle(&mut scheduler);

        assert!(!scheduler.is_pending_unload(pos));
        assert_eq!(scheduler.pending_unload_count(), 0);
        assert_eq!(
            scheduler.holder(pos).map(ChunkHolder::ticket_level),
            Some(FORCED_TICKET_LEVEL)
        );
        assert_eq!(scheduler.loaded_chunk_count(), 9);
        assert_eq!(scheduler.process_pending_unloads(usize::MAX).unwrap(), 0);
        assert!(scheduler.holder(pos).is_some());
    }

    #[test]
    fn chunk_scheduler_records_holder_status_slots_in_order() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
            9
        );
        assert_eq!(
            status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
            9
        );
        assert!(events.iter().any(|event| matches!(
            event,
            ChunkSchedulerEvent::SnapshotReady(snapshot)
                if snapshot.pos == ChunkPos::new(0, 0)
                    && snapshot.status == ChunkStatus::Features
        )));

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.target_status(), Some(ChunkStatus::Features));
        assert_eq!(holder.ready_status_count(), 3);
        assert_eq!(
            holder.status_slot(ChunkStatus::Terrain),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Terrain,
                step: ChunkStatusStep::Ready,
                revision: None,
                job_id: None,
            })
        );
        assert_eq!(
            holder.status_slot(ChunkStatus::Surface),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Surface,
                step: ChunkStatusStep::Ready,
                revision: None,
                job_id: None,
            })
        );
        assert_eq!(
            holder.status_slot(ChunkStatus::Features),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Features,
                step: ChunkStatusStep::Ready,
                revision: Some(ChunkRevision(5)),
                job_id: Some(ChunkJobId(1)),
            })
        );
        assert_eq!(scheduler.job_count(), 1);
        assert_eq!(
            scheduler.job(ChunkJobId(1)),
            Some(&ChunkStatusJob {
                id: ChunkJobId(1),
                status: ChunkStatus::Features,
                state: ChunkJobState::Complete,
                target_chunks: (-1..=1)
                    .flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)))
                    .collect(),
                feature_centers: (-2..=2)
                    .flat_map(|z| (-2..=2).map(move |x| ChunkPos::new(x, z)))
                    .collect(),
                dependency_chunks: (-10..=10)
                    .flat_map(|z| (-10..=10).map(move |x| ChunkPos::new(x, z)))
                    .collect(),
                seeded_dependency_chunks: 0,
                dependency_cache_hits: 0,
                dependency_cache_misses: 21 * 21,
                retained_dependency_chunks: 21 * 21,
            })
        );
    }

    #[test]
    fn chunk_scheduler_coalesces_duplicate_status_requests() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };

        assert_eq!(
            without_fluid_tick_events(&apply_interest_and_poll(&mut scheduler, interest.clone()))
                .len(),
            55
        );
        assert_eq!(
            apply_interest_and_poll(&mut scheduler, interest),
            Vec::new()
        );
        assert_eq!(scheduler.loaded_chunk_count(), 9);
    }

    #[test]
    fn generated_chunks_are_dirty_until_saved() {
        let mut scheduler = ChunkScheduler::new(12_345);
        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::Generated);
        assert!(holder.is_dirty());
        assert_eq!(scheduler.dirty_chunk_count(), 9);

        assert_eq!(scheduler.save_dirty_chunks().unwrap(), 9);

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::Saved);
        assert!(!holder.is_dirty());
        assert_eq!(scheduler.dirty_chunk_count(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn integrated_server_saves_and_reloads_resident_chunk() {
        let root = unique_temp_dir("integrated_server_saves_and_reloads_resident_chunk");
        let interest = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        };
        let first_snapshot = {
            let mut server = IntegratedServer::with_chunk_store(
                12_345,
                Box::new(FilesystemChunkSnapshotStore::new(&root)),
            );
            let updates = try_handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkView(interest.clone()),
            )
            .unwrap();
            let ServerUpdate::ChunkSnapshot(snapshot) = updates.first().unwrap() else {
                panic!("first update was not a chunk snapshot");
            };
            assert_eq!(server.scheduler().job_count(), 1);
            assert_eq!(server.scheduler().dirty_chunk_count(), 9);
            assert_eq!(
                server
                    .scheduler()
                    .holder(ChunkPos::new(0, 0))
                    .unwrap()
                    .residency(),
                ChunkResidency::Generated
            );

            assert_eq!(server.save_dirty_chunks().unwrap(), 9);
            assert_eq!(server.scheduler().dirty_chunk_count(), 0);
            assert_eq!(
                server
                    .scheduler()
                    .holder(ChunkPos::new(0, 0))
                    .unwrap()
                    .residency(),
                ChunkResidency::Saved
            );
            snapshot.clone()
        };

        let mut reloaded = IntegratedServer::with_chunk_store(
            12_345,
            Box::new(FilesystemChunkSnapshotStore::new(&root)),
        );
        let updates =
            try_handle_command_and_poll(&mut reloaded, ClientCommand::SetChunkView(interest))
                .unwrap();

        assert_eq!(updates, vec![ServerUpdate::ChunkSnapshot(first_snapshot)]);
        assert_eq!(reloaded.scheduler().job_count(), 0);
        let holder = reloaded.scheduler().holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::LoadedFromStore);
        assert!(!holder.is_dirty());
        assert_eq!(reloaded.scheduler().dirty_chunk_count(), 0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_interest_unloads_chunks_outside_view() {
        let mut server = IntegratedServer::new(12_345);
        handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        );

        assert_eq!(updates.len(), 2);
        assert!(
            updates
                .iter()
                .any(|update| matches!(update, ServerUpdate::ChunkUnload { pos } if *pos == ChunkPos::new(0, 0)))
        );
        assert!(updates.iter().any(|update| matches!(
            update,
            ServerUpdate::ChunkSnapshot(snapshot)
                if snapshot.pos == ChunkPos::new(1, 0)
                    && snapshot.status == ChunkStatus::Features
        )));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("mclone-{name}-{}-{nanos}", std::process::id()));
        path
    }
}
