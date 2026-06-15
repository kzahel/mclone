#![forbid(unsafe_code)]

mod persistence;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[cfg(target_arch = "wasm32")]
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::{sync::mpsc, thread};

use mclone_core::{ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus};
use mclone_protocol::{ChunkInterest, ClientCommand, ServerUpdate};
use mclone_worldgen::feature::{FEATURES_CHUNK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF};
use mclone_worldgen::levelgen::{
    GeneratedChunk, OverworldFeatureDependencyCache, OverworldFeatureDependencyCacheReport,
};
pub use persistence::{
    ChunkSnapshotStore, ChunkStoreError, ChunkStoreResult, NullChunkSnapshotStore,
};

#[cfg(not(target_arch = "wasm32"))]
pub use persistence::FilesystemChunkSnapshotStore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerMode {
    Integrated,
    Dedicated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkStatusStep {
    Scheduled,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkResidency {
    NotResident,
    Generated,
    LoadedFromStore,
    Saved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FullChunkStatus {
    Inaccessible,
    Border,
    Ticking,
    EntityTicking,
}

impl FullChunkStatus {
    pub fn is_or_after(self, status: Self) -> bool {
        self >= status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ChunkTicketType {
    Start,
    Dragon,
    Player,
    Forced,
    Light,
    Portal,
    PostTeleport,
    Unknown,
}

impl ChunkTicketType {
    const fn timeout_ticks(self) -> Option<u64> {
        match self {
            Self::Portal => Some(300),
            Self::PostTeleport => Some(5),
            Self::Unknown => Some(1),
            Self::Start | Self::Dragon | Self::Player | Self::Forced | Self::Light => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ChunkTicketKey {
    Chunk(ChunkPos),
    Named(String),
}

#[derive(Clone, Debug)]
pub struct ChunkTicket {
    pub ticket_type: ChunkTicketType,
    pub level: i32,
    pub key: ChunkTicketKey,
    pub created_tick: u64,
}

impl ChunkTicket {
    pub fn new(ticket_type: ChunkTicketType, level: i32, key: ChunkTicketKey) -> Self {
        Self {
            ticket_type,
            level,
            key,
            created_tick: 0,
        }
    }

    fn timed_out(&self, current_tick: u64) -> bool {
        self.ticket_type
            .timeout_ticks()
            .is_some_and(|timeout| current_tick.saturating_sub(self.created_tick) > timeout)
    }
}

impl PartialEq for ChunkTicket {
    fn eq(&self, other: &Self) -> bool {
        self.ticket_type == other.ticket_type && self.level == other.level && self.key == other.key
    }
}

impl Eq for ChunkTicket {}

impl PartialOrd for ChunkTicket {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkTicket {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.level
            .cmp(&other.level)
            .then_with(|| self.ticket_type.cmp(&other.ticket_type))
            .then_with(|| self.key.cmp(&other.key))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldgenMailboxKind {
    Inline,
    NativeThread,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ChunkJobId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkJobState {
    Queued,
    Running,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkStatusJob {
    pub id: ChunkJobId,
    pub status: ChunkStatus,
    pub state: ChunkJobState,
    pub target_chunks: Vec<ChunkPos>,
    pub feature_centers: Vec<ChunkPos>,
    pub dependency_chunks: Vec<ChunkPos>,
    pub dependency_cache_hits: usize,
    pub dependency_cache_misses: usize,
    pub retained_dependency_chunks: usize,
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

    fn set_player_interest(&mut self, interest: ChunkInterest) {
        let new_positions = interest_positions(&interest)
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

    fn ticketed_chunks(&self) -> Vec<ChunkPos> {
        self.tickets
            .iter()
            .filter_map(|(pos, tickets)| {
                tickets
                    .first()
                    .is_some_and(|ticket| ticket.level <= MAX_CHUNK_DISTANCE)
                    .then_some(*pos)
            })
            .collect()
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
}

#[derive(Debug)]
pub struct ChunkScheduler {
    seed: i64,
    holders: BTreeMap<ChunkPos, ChunkHolder>,
    distance_manager: ChunkDistanceManager,
    jobs: BTreeMap<ChunkJobId, ChunkStatusJob>,
    worldgen_mailbox: WorldgenMailbox,
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
            distance_manager: ChunkDistanceManager::new(),
            jobs: BTreeMap::new(),
            worldgen_mailbox: WorldgenMailbox::new(),
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
        interest: ChunkInterest,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.distance_manager.set_player_interest(interest);
        self.reconcile_ticketed_holders()
    }

    pub fn poll(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.publish_completed_worldgen_jobs()
    }

    pub fn tick(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        self.distance_manager.purge_stale_tickets();
        let mut events = self.reconcile_ticketed_holders()?;
        events.extend(self.poll()?);
        Ok(events)
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

    pub fn loaded_chunk_count(&self) -> usize {
        self.holders
            .values()
            .filter(|holder| holder.published_snapshot.is_some())
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

    pub fn ticket_count_at(&self, pos: ChunkPos) -> usize {
        self.distance_manager.ticket_count_at(pos)
    }

    pub fn ticket_level_at(&self, pos: ChunkPos) -> i32 {
        self.distance_manager.ticket_level_at(pos)
    }

    pub fn job(&self, id: ChunkJobId) -> Option<&ChunkStatusJob> {
        self.jobs.get(&id)
    }

    pub fn jobs(&self) -> impl Iterator<Item = &ChunkStatusJob> {
        self.jobs.values()
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

    pub fn worldgen_mailbox_kind(&self) -> WorldgenMailboxKind {
        self.worldgen_mailbox.kind()
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

    fn reconcile_ticketed_holders(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let ticketed_chunks = self.distance_manager.ticketed_chunks();
        let desired_set = ticketed_chunks.iter().copied().collect::<BTreeSet<_>>();
        let mut events = Vec::new();

        for pos in self
            .holders
            .keys()
            .copied()
            .filter(|pos| !desired_set.contains(pos))
            .collect::<Vec<_>>()
        {
            let mut holder = self.holders.remove(&pos).expect("holder key disappeared");
            holder.set_ticket_level(UNLOADED_CHUNK_LEVEL);
            if holder.dirty {
                self.save_holder(&holder)?;
                self.dirty_chunks.remove(&pos);
            }
            if holder.published_snapshot.is_some() {
                events.push(ChunkSchedulerEvent::Unloaded { pos });
            }
        }

        let mut feature_targets = BTreeSet::new();
        for pos in ticketed_chunks {
            let ticket_level = self.distance_manager.ticket_level_at(pos);
            let holder = self
                .holders
                .entry(pos)
                .or_insert_with(|| ChunkHolder::new(pos));
            holder.set_ticket_level(ticket_level);
            if ticket_level_status_target(ticket_level)
                .is_some_and(|status| status >= ChunkStatus::Features)
            {
                feature_targets.insert(pos);
            }
        }

        events
            .extend(self.enqueue_feature_chunks(sorted_chunk_positions_z_major(feature_targets))?);
        Ok(events)
    }

    fn enqueue_feature_chunks(
        &mut self,
        desired_chunks: Vec<ChunkPos>,
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
                        events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
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
            let job_id = self.create_feature_job(&to_generate);
            for pos in &to_generate {
                self.holders
                    .get_mut(pos)
                    .expect("holder must exist before assigning job")
                    .assign_status_job(ChunkStatus::Features, job_id);
            }

            self.mark_job_state(job_id, ChunkJobState::Running);
            self.worldgen_mailbox
                .enqueue_features(job_id, self.seed, &to_generate);
        }

        Ok(events)
    }

    fn publish_completed_worldgen_jobs(&mut self) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let completed_jobs = self.worldgen_mailbox.drain_completed();
        let mut events = Vec::new();

        for completed in completed_jobs {
            events.extend(self.publish_completed_feature_job(completed)?);
        }

        Ok(events)
    }

    fn publish_completed_feature_job(
        &mut self,
        completed: WorldgenCompletedJob,
    ) -> ChunkStoreResult<Vec<ChunkSchedulerEvent>> {
        let Some(job) = self.jobs.get(&completed.job_id).cloned() else {
            return Ok(Vec::new());
        };

        let mut events = Vec::new();
        let mut generated = completed.generated_chunks;

        for pos in job.target_chunks {
            let should_publish = self
                .holders
                .get(&pos)
                .and_then(|holder| holder.status_slot(ChunkStatus::Features))
                .is_some_and(|slot| {
                    slot.job_id == Some(completed.job_id) && slot.step == ChunkStatusStep::Scheduled
                });
            if !should_publish {
                continue;
            }

            let revision = ChunkRevision(self.next_revision);
            self.next_revision += 1;

            let chunk = generated.remove(&pos).unwrap_or_else(|| {
                panic!(
                    "feature batch did not return scheduler target chunk ({}, {})",
                    pos.x, pos.z
                )
            });
            let snapshot = chunk.to_chunk_snapshot(revision, ChunkStatus::Features);
            self.mark_snapshot_ready(pos, snapshot.clone(), ChunkResidency::Generated, true);
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status: ChunkStatus::Features,
                step: ChunkStatusStep::Ready,
            });
            events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
        }

        self.mark_job_complete(completed.job_id, completed.cache_report);
        Ok(events)
    }

    fn create_feature_job(&mut self, targets: &[ChunkPos]) -> ChunkJobId {
        let id = ChunkJobId(self.next_job_id);
        self.next_job_id += 1;
        let (target_chunks, feature_centers, dependency_chunks) = feature_job_positions(targets);
        self.jobs.insert(
            id,
            ChunkStatusJob {
                id,
                status: ChunkStatus::Features,
                state: ChunkJobState::Queued,
                target_chunks,
                feature_centers,
                dependency_chunks,
                dependency_cache_hits: 0,
                dependency_cache_misses: 0,
                retained_dependency_chunks: 0,
            },
        );
        id
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
    ) {
        let job = self
            .jobs
            .get_mut(&id)
            .expect("job must exist before state transition");
        job.dependency_cache_hits = cache_report.cache_hits;
        job.dependency_cache_misses = cache_report.generated_dependency_chunks;
        job.retained_dependency_chunks = cache_report.retained_dependency_chunks;
        job.state = ChunkJobState::Complete;
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

    fn save_holder(&mut self, holder: &ChunkHolder) -> ChunkStoreResult<()> {
        if let Some(snapshot) = holder.snapshot() {
            self.store.save_chunk(snapshot)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct WorldgenCompletedJob {
    job_id: ChunkJobId,
    generated_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    cache_report: OverworldFeatureDependencyCacheReport,
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

    fn enqueue_features(&mut self, job_id: ChunkJobId, seed: i64, targets: &[ChunkPos]) {
        self.backend.enqueue_features(job_id, seed, targets);
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        self.backend.drain_completed()
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
    dependency_cache: OverworldFeatureDependencyCache,
}

#[cfg(target_arch = "wasm32")]
impl WorldgenMailboxBackend {
    fn new() -> Self {
        Self {
            completed: VecDeque::new(),
            dependency_cache: OverworldFeatureDependencyCache::new(),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::Inline
    }

    fn enqueue_features(&mut self, job_id: ChunkJobId, seed: i64, targets: &[ChunkPos]) {
        let result = self
            .dependency_cache
            .generate_features_chunks(seed, targets.iter().copied());
        self.completed.push_back(WorldgenCompletedJob {
            job_id,
            generated_chunks: result.chunks,
            cache_report: result.cache_report,
        });
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        self.completed.drain(..).collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct WorldgenMailboxBackend {
    sender: mpsc::Sender<WorldgenRequest>,
    completion_receiver: mpsc::Receiver<WorldgenCompletedJob>,
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
                let mut dependency_cache = OverworldFeatureDependencyCache::new();
                while let Ok(request) = receiver.recv() {
                    match request {
                        WorldgenRequest::GenerateFeatures {
                            job_id,
                            seed,
                            targets,
                        } => {
                            let result = dependency_cache
                                .generate_features_chunks(seed, targets.iter().copied());
                            if completion_sender
                                .send(WorldgenCompletedJob {
                                    job_id,
                                    generated_chunks: result.chunks,
                                    cache_report: result.cache_report,
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
            worker: Some(worker),
        }
    }

    fn kind(&self) -> WorldgenMailboxKind {
        WorldgenMailboxKind::NativeThread
    }

    fn enqueue_features(&mut self, job_id: ChunkJobId, seed: i64, targets: &[ChunkPos]) {
        self.sender
            .send(WorldgenRequest::GenerateFeatures {
                job_id,
                seed,
                targets: targets.to_vec(),
            })
            .expect("native worldgen worker stopped before receiving job");
    }

    fn drain_completed(&mut self) -> Vec<WorldgenCompletedJob> {
        let mut completed = Vec::new();
        while let Ok(job) = self.completion_receiver.try_recv() {
            completed.push(job);
        }
        completed
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
    },
    Shutdown,
}

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    scheduler: ChunkScheduler,
}

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self::with_chunk_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self {
            seed,
            scheduler: ChunkScheduler::with_store(seed, store),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
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
            ClientCommand::SetChunkInterest(interest) => self.set_chunk_interest(interest),
        }
    }

    pub fn poll(&mut self) -> Vec<ServerUpdate> {
        self.try_poll().expect("integrated server poll failed")
    }

    pub fn try_poll(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        Ok(self
            .scheduler
            .poll()?
            .into_iter()
            .filter_map(server_update_from_scheduler_event)
            .collect())
    }

    pub fn tick(&mut self) -> Vec<ServerUpdate> {
        self.try_tick().expect("integrated server tick failed")
    }

    pub fn try_tick(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        Ok(self
            .scheduler
            .tick()?
            .into_iter()
            .filter_map(server_update_from_scheduler_event)
            .collect())
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.scheduler.loaded_chunk_count()
    }

    pub fn pending_job_count(&self) -> usize {
        self.scheduler.pending_job_count()
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

    fn set_chunk_interest(
        &mut self,
        interest: ChunkInterest,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let mut events = self.scheduler.apply_interest(interest)?;
        events.extend(self.scheduler.poll()?);
        Ok(events
            .into_iter()
            .filter_map(server_update_from_scheduler_event)
            .collect())
    }
}

fn server_update_from_scheduler_event(event: ChunkSchedulerEvent) -> Option<ServerUpdate> {
    match event {
        ChunkSchedulerEvent::SnapshotReady(snapshot) => Some(ServerUpdate::ChunkSnapshot(snapshot)),
        ChunkSchedulerEvent::Unloaded { pos } => Some(ServerUpdate::ChunkUnload { pos }),
        ChunkSchedulerEvent::StatusChanged { .. } => None,
    }
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

fn interest_positions(interest: &ChunkInterest) -> Vec<ChunkPos> {
    let radius = i32::try_from(interest.radius_chunks).expect("chunk interest radius exceeds i32");
    let min_x = interest.center.x - radius;
    let max_x = interest.center.x + radius;
    let min_z = interest.center.z - radius;
    let max_z = interest.center.z + radius;
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

pub const CHUNK_LEVEL_FULL: i32 = 33;
pub const PLAYER_TICKET_LEVEL: i32 = CHUNK_LEVEL_FULL - ENTITY_TICKING_RANGE;
pub const FORCED_TICKET_LEVEL: i32 = 31;
pub const MAX_CHUNK_DISTANCE: i32 = CHUNK_LEVEL_FULL + VANILLA_STATUS_AROUND_FULL_DISTANCE;
pub const UNLOADED_CHUNK_LEVEL: i32 = MAX_CHUNK_DISTANCE + 1;

const ENTITY_TICKING_RANGE: i32 = 2;
const VANILLA_STATUS_AROUND_FULL_DISTANCE: i32 = 11;

fn full_chunk_status_for_ticket_level(ticket_level: i32) -> FullChunkStatus {
    match (CHUNK_LEVEL_FULL - ticket_level + 1).clamp(0, 3) {
        0 => FullChunkStatus::Inaccessible,
        1 => FullChunkStatus::Border,
        2 => FullChunkStatus::Ticking,
        3 => FullChunkStatus::EntityTicking,
        _ => unreachable!("clamped full chunk status index must be in 0..=3"),
    }
}

fn ticket_level_status_target(ticket_level: i32) -> Option<ChunkStatus> {
    if ticket_level > MAX_CHUNK_DISTANCE {
        None
    } else if ticket_level <= CHUNK_LEVEL_FULL + 1 {
        Some(ChunkStatus::Features)
    } else {
        Some(ChunkStatus::Terrain)
    }
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

    fn apply_interest_and_poll(
        scheduler: &mut ChunkScheduler,
        interest: ChunkInterest,
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
            wait_for_worker_tick();
        }
        panic!("timed out waiting for scheduler worldgen jobs");
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
            wait_for_worker_tick();
        }
        panic!("timed out waiting for integrated server worldgen jobs");
    }

    fn wait_for_worker_tick() {
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(std::time::Duration::from_millis(1));
        #[cfg(target_arch = "wasm32")]
        std::hint::spin_loop();
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
            ClientCommand::SetChunkInterest(ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 1,
            }),
        );

        assert_eq!(updates.len(), 9);
        assert_eq!(server.loaded_chunk_count(), 9);
        assert_eq!(server.scheduler().holder_count(), 9);
        assert_eq!(server.scheduler().job_count(), 1);
        let job = server.scheduler().jobs().next().unwrap();
        assert_eq!(job.id, ChunkJobId(1));
        assert_eq!(job.status, ChunkStatus::Features);
        assert_eq!(job.state, ChunkJobState::Complete);
        assert_eq!(job.target_chunks.len(), 9);
        assert_eq!(job.feature_centers.len(), 5 * 5);
        assert_eq!(job.dependency_chunks.len(), 21 * 21);
        assert_eq!(job.dependency_cache_hits, 0);
        assert_eq!(job.dependency_cache_misses, 21 * 21);
        assert_eq!(job.retained_dependency_chunks, 21 * 21);
        assert!(job.dependency_chunks.contains(&ChunkPos::new(-10, -10)));
        assert!(job.dependency_chunks.contains(&ChunkPos::new(10, 10)));
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
    fn duplicate_interest_does_not_regenerate_loaded_chunks() {
        let mut server = IntegratedServer::new(12_345);
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        };

        assert_eq!(
            handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkInterest(interest.clone())
            )
            .len(),
            1
        );
        assert_eq!(server.scheduler().job_count(), 1);
        assert_eq!(
            handle_command_and_poll(&mut server, ClientCommand::SetChunkInterest(interest)).len(),
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
                ClientCommand::SetChunkInterest(ChunkInterest {
                    center: ChunkPos::new(0, 0),
                    radius_chunks: 0,
                }),
            )
            .len(),
            1
        );
        assert_eq!(
            server.scheduler().job(ChunkJobId(1)).map(|job| (
                job.dependency_cache_hits,
                job.dependency_cache_misses,
                job.retained_dependency_chunks
            )),
            Some((0, 19 * 19, 19 * 19))
        );

        assert_eq!(
            handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkInterest(ChunkInterest {
                    center: ChunkPos::new(1, 0),
                    radius_chunks: 0,
                }),
            )
            .len(),
            2
        );
        assert_eq!(
            server.scheduler().job(ChunkJobId(2)).map(|job| (
                job.dependency_cache_hits,
                job.dependency_cache_misses,
                job.retained_dependency_chunks
            )),
            Some((18 * 19, 19, 19 * 19))
        );
    }

    #[test]
    fn chunk_scheduler_apply_interest_enqueues_features_before_poll() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .apply_interest(ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 0,
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
        assert_eq!(
            events
                .iter()
                .filter_map(|event| {
                    if let ChunkSchedulerEvent::StatusChanged { status, step, .. } = event {
                        Some((*status, *step))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
            vec![
                (ChunkStatus::Terrain, ChunkStatusStep::Scheduled),
                (ChunkStatus::Terrain, ChunkStatusStep::Ready),
                (ChunkStatus::Surface, ChunkStatusStep::Scheduled),
                (ChunkStatus::Surface, ChunkStatusStep::Ready),
                (ChunkStatus::Features, ChunkStatusStep::Scheduled),
            ]
        );

        let ready_events = poll_scheduler_until_idle(&mut scheduler);
        assert_eq!(scheduler.pending_job_count(), 0);
        assert_eq!(scheduler.loaded_chunk_count(), 1);
        assert!(matches!(
            ready_events.as_slice(),
            [
                ChunkSchedulerEvent::StatusChanged {
                    status: ChunkStatus::Features,
                    step: ChunkStatusStep::Ready,
                    ..
                },
                ChunkSchedulerEvent::SnapshotReady(_)
            ]
        ));
    }

    #[test]
    fn chunk_interest_updates_player_tickets_and_holder_levels() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler
            .apply_interest(ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 0,
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
        assert_eq!(events.len(), 5);

        let moved_events = scheduler
            .apply_interest(ChunkInterest {
                center: ChunkPos::new(1, 0),
                radius_chunks: 0,
            })
            .unwrap();

        assert_eq!(scheduler.ticketed_chunk_count(), 1);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(1, 0)), 1);
        assert_eq!(
            scheduler.ticket_level_at(ChunkPos::new(0, 0)),
            UNLOADED_CHUNK_LEVEL
        );
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());
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
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        };

        let first_events = scheduler.apply_interest(interest.clone()).unwrap();
        assert_eq!(first_events.len(), 5);
        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.job_count(), 1);

        let second_events = scheduler.apply_interest(interest).unwrap();
        assert_eq!(second_events, Vec::new());
        assert_eq!(scheduler.pending_job_count(), 1);
        assert_eq!(scheduler.job_count(), 1);

        assert_eq!(poll_scheduler_until_idle(&mut scheduler).len(), 2);
        assert_eq!(scheduler.loaded_chunk_count(), 1);
        assert_eq!(scheduler.job_count(), 1);
    }

    #[test]
    fn forced_ticket_keeps_chunk_resident_after_interest_moves() {
        let mut scheduler = ChunkScheduler::new(12_345);

        apply_interest_and_poll(
            &mut scheduler,
            ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 0,
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
            ChunkInterest {
                center: ChunkPos::new(1, 0),
                radius_chunks: 0,
            },
        );

        assert!(events
            .iter()
            .all(|event| !matches!(event, ChunkSchedulerEvent::Unloaded { pos } if *pos == ChunkPos::new(0, 0))));
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
        assert!(scheduler.holder(ChunkPos::new(1, 0)).is_some());
        assert_eq!(scheduler.loaded_chunk_count(), 2);

        let events = scheduler
            .set_chunk_forced(ChunkPos::new(0, 0), false)
            .unwrap();

        assert_eq!(
            events,
            vec![ChunkSchedulerEvent::Unloaded {
                pos: ChunkPos::new(0, 0)
            }]
        );
        assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());
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
        assert_eq!(poll_scheduler_until_idle(&mut scheduler).len(), 2);
        assert_eq!(scheduler.loaded_chunk_count(), 1);

        assert!(scheduler.tick().unwrap().is_empty());
        assert_eq!(scheduler.ticket_tick(), 1);
        assert_eq!(scheduler.ticketed_chunk_count(), 1);

        let events = scheduler.tick().unwrap();

        assert_eq!(scheduler.ticket_tick(), 2);
        assert_eq!(scheduler.ticketed_chunk_count(), 0);
        assert_eq!(
            events,
            vec![ChunkSchedulerEvent::Unloaded {
                pos: ChunkPos::new(0, 0)
            }]
        );
        assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());
    }

    #[test]
    fn chunk_scheduler_records_holder_status_slots_in_order() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = apply_interest_and_poll(
            &mut scheduler,
            ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 0,
            },
        );

        assert_eq!(
            events
                .iter()
                .filter_map(|event| {
                    if let ChunkSchedulerEvent::StatusChanged { status, step, .. } = event {
                        Some((*status, *step))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
            vec![
                (ChunkStatus::Terrain, ChunkStatusStep::Scheduled),
                (ChunkStatus::Terrain, ChunkStatusStep::Ready),
                (ChunkStatus::Surface, ChunkStatusStep::Scheduled),
                (ChunkStatus::Surface, ChunkStatusStep::Ready),
                (ChunkStatus::Features, ChunkStatusStep::Scheduled),
                (ChunkStatus::Features, ChunkStatusStep::Ready),
            ]
        );
        assert!(matches!(
            events.last(),
            Some(ChunkSchedulerEvent::SnapshotReady(snapshot))
                if snapshot.pos == ChunkPos::new(0, 0)
                    && snapshot.status == ChunkStatus::Features
        ));

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
                revision: Some(ChunkRevision(1)),
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
                target_chunks: vec![ChunkPos::new(0, 0)],
                feature_centers: (-1..=1)
                    .flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)))
                    .collect(),
                dependency_chunks: (-9..=9)
                    .flat_map(|z| (-9..=9).map(move |x| ChunkPos::new(x, z)))
                    .collect(),
                dependency_cache_hits: 0,
                dependency_cache_misses: 19 * 19,
                retained_dependency_chunks: 19 * 19,
            })
        );
    }

    #[test]
    fn chunk_scheduler_coalesces_duplicate_status_requests() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        };

        assert_eq!(
            apply_interest_and_poll(&mut scheduler, interest.clone()).len(),
            7
        );
        assert_eq!(
            apply_interest_and_poll(&mut scheduler, interest),
            Vec::new()
        );
        assert_eq!(scheduler.loaded_chunk_count(), 1);
    }

    #[test]
    fn generated_chunks_are_dirty_until_saved() {
        let mut scheduler = ChunkScheduler::new(12_345);
        apply_interest_and_poll(
            &mut scheduler,
            ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 0,
            },
        );

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::Generated);
        assert!(holder.is_dirty());
        assert_eq!(scheduler.dirty_chunk_count(), 1);

        assert_eq!(scheduler.save_dirty_chunks().unwrap(), 1);

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.residency(), ChunkResidency::Saved);
        assert!(!holder.is_dirty());
        assert_eq!(scheduler.dirty_chunk_count(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn integrated_server_saves_and_reloads_resident_chunk() {
        let root = unique_temp_dir("integrated_server_saves_and_reloads_resident_chunk");
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        };
        let first_snapshot = {
            let mut server = IntegratedServer::with_chunk_store(
                12_345,
                Box::new(FilesystemChunkSnapshotStore::new(&root)),
            );
            let updates = try_handle_command_and_poll(
                &mut server,
                ClientCommand::SetChunkInterest(interest.clone()),
            )
            .unwrap();
            let ServerUpdate::ChunkSnapshot(snapshot) = updates.first().unwrap() else {
                panic!("first update was not a chunk snapshot");
            };
            assert_eq!(server.scheduler().job_count(), 1);
            assert_eq!(server.scheduler().dirty_chunk_count(), 1);
            assert_eq!(
                server
                    .scheduler()
                    .holder(ChunkPos::new(0, 0))
                    .unwrap()
                    .residency(),
                ChunkResidency::Generated
            );

            assert_eq!(server.save_dirty_chunks().unwrap(), 1);
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
            try_handle_command_and_poll(&mut reloaded, ClientCommand::SetChunkInterest(interest))
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
            ClientCommand::SetChunkInterest(ChunkInterest {
                center: ChunkPos::new(0, 0),
                radius_chunks: 0,
            }),
        );

        let updates = handle_command_and_poll(
            &mut server,
            ClientCommand::SetChunkInterest(ChunkInterest {
                center: ChunkPos::new(1, 0),
                radius_chunks: 0,
            }),
        );

        assert_eq!(
            updates,
            vec![
                ServerUpdate::ChunkUnload {
                    pos: ChunkPos::new(0, 0)
                },
                ServerUpdate::ChunkSnapshot(
                    mclone_worldgen::levelgen::generate_overworld_features_chunk(12_345, 1, 0)
                        .to_chunk_snapshot(ChunkRevision(2), ChunkStatus::Features)
                )
            ]
        );
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
