use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::io;

#[cfg(any(test, not(target_arch = "wasm32")))]
use std::io::{Read, Write};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use mclone_core::{ChunkPos, ChunkRevision, ChunkSnapshot};

#[cfg(any(test, not(target_arch = "wasm32")))]
use mclone_core::{
    BlockStateId, ChunkStatus, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection, PackedLightSection,
};

#[cfg(any(test, not(target_arch = "wasm32")))]
const SNAPSHOT_MAGIC: &[u8; 12] = b"MCLONESNAP\0\0";
#[cfg(any(test, not(target_arch = "wasm32")))]
const SNAPSHOT_FORMAT_VERSION: u32 = 4;

pub const CHUNK_LIGHT_ALGORITHM_VERSION: u32 = 1;

pub type PersistenceRequestId = u64;

pub type ChunkStoreResult<T> = Result<T, ChunkStoreError>;

#[derive(Debug)]
pub enum ChunkStoreError {
    Io(io::Error),
    InvalidData(String),
    Closed(String),
}

impl fmt::Display for ChunkStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidData(message) => f.write_str(message),
            Self::Closed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ChunkStoreError {}

impl From<io::Error> for ChunkStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn duplicate_store_error(error: &ChunkStoreError) -> ChunkStoreError {
    ChunkStoreError::InvalidData(error.to_string())
}

fn closed_error() -> ChunkStoreError {
    ChunkStoreError::Closed("persistence actor is closed".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveDurability {
    Cache,
    Durable,
}

impl SaveDurability {
    pub const fn is_durable(self) -> bool {
        matches!(self, Self::Durable)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkRecord {
    pub snapshot: ChunkSnapshot,
    pub light_algorithm_version: Option<u32>,
}

impl ChunkRecord {
    pub fn from_snapshot(snapshot: ChunkSnapshot) -> Self {
        let light_algorithm_version = snapshot
            .light_correct
            .then_some(CHUNK_LIGHT_ALGORITHM_VERSION);
        Self {
            snapshot,
            light_algorithm_version,
        }
    }

    pub fn legacy_snapshot(snapshot: ChunkSnapshot) -> Self {
        Self {
            snapshot,
            light_algorithm_version: None,
        }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.snapshot.pos
    }

    pub const fn revision(&self) -> ChunkRevision {
        self.snapshot.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PlayerRecordKey {
    Uuid(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum WorldRecordKey {
    Chunk(ChunkPos),
    EntityChunk(ChunkPos),
    Player(PlayerRecordKey),
    SavedData(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldStoreRequest {
    LoadChunk {
        request_id: PersistenceRequestId,
        pos: ChunkPos,
    },
    SaveChunk {
        request_id: PersistenceRequestId,
        record: ChunkRecord,
        durability: SaveDurability,
    },
    LoadEntityChunk {
        request_id: PersistenceRequestId,
        pos: ChunkPos,
    },
    LoadPlayer {
        request_id: PersistenceRequestId,
        player: PlayerRecordKey,
    },
    LoadSavedData {
        request_id: PersistenceRequestId,
        key: String,
    },
    Flush {
        request_id: PersistenceRequestId,
    },
    Close {
        request_id: PersistenceRequestId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreWriteOutcome {
    Written,
    Superseded,
    SkippedOnClose,
}

#[derive(Debug)]
pub enum WorldStoreCompletion {
    ChunkLoaded {
        request_id: PersistenceRequestId,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<ChunkRecord>>,
    },
    ChunkSaved {
        request_id: PersistenceRequestId,
        pos: ChunkPos,
        result: ChunkStoreResult<StoreWriteOutcome>,
    },
    RequestFailed {
        request_id: PersistenceRequestId,
        key: WorldRecordKey,
        result: ChunkStoreResult<()>,
    },
    FlushComplete {
        request_id: PersistenceRequestId,
        result: ChunkStoreResult<()>,
    },
    CloseComplete {
        request_id: PersistenceRequestId,
        result: ChunkStoreResult<()>,
    },
}

impl WorldStoreCompletion {
    pub const fn request_id(&self) -> PersistenceRequestId {
        match self {
            Self::ChunkLoaded { request_id, .. }
            | Self::ChunkSaved { request_id, .. }
            | Self::RequestFailed { request_id, .. }
            | Self::FlushComplete { request_id, .. }
            | Self::CloseComplete { request_id, .. } => *request_id,
        }
    }
}

pub trait ChunkSnapshotStore: fmt::Debug {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>>;
    fn save_chunk(&mut self, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()>;
}

pub trait WorldStore: fmt::Debug {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>>;
    fn save_chunk(&mut self, record: &ChunkRecord) -> ChunkStoreResult<()>;

    fn flush(&mut self) -> ChunkStoreResult<()> {
        Ok(())
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NullChunkSnapshotStore;

impl ChunkSnapshotStore for NullChunkSnapshotStore {
    fn load_chunk(&mut self, _pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        Ok(None)
    }

    fn save_chunk(&mut self, _snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NullWorldStore;

impl WorldStore for NullWorldStore {
    fn load_chunk(&mut self, _pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        Ok(None)
    }

    fn save_chunk(&mut self, _record: &ChunkRecord) -> ChunkStoreResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MemoryWorldStore {
    chunks: BTreeMap<ChunkPos, ChunkRecord>,
}

impl MemoryWorldStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&ChunkRecord> {
        self.chunks.get(&pos)
    }
}

impl WorldStore for MemoryWorldStore {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        Ok(self.chunks.get(&pos).cloned())
    }

    fn save_chunk(&mut self, record: &ChunkRecord) -> ChunkStoreResult<()> {
        self.chunks.insert(record.pos(), record.clone());
        Ok(())
    }
}

#[derive(Debug)]
pub struct ChunkSnapshotWorldStore {
    store: Box<dyn ChunkSnapshotStore>,
}

impl ChunkSnapshotWorldStore {
    pub fn new(store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self { store }
    }
}

impl WorldStore for ChunkSnapshotWorldStore {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        self.store
            .load_chunk(pos)
            .map(|snapshot| snapshot.map(ChunkRecord::legacy_snapshot))
    }

    fn save_chunk(&mut self, record: &ChunkRecord) -> ChunkStoreResult<()> {
        self.store.save_chunk(&record.snapshot)
    }
}

#[derive(Clone, Debug)]
struct PendingChunkWrite {
    request_id: PersistenceRequestId,
    record: ChunkRecord,
    durability: SaveDurability,
}

impl PendingChunkWrite {
    fn should_replace(&self, existing: &Self) -> bool {
        let new_revision = self.record.revision();
        let old_revision = existing.record.revision();
        if new_revision != old_revision {
            return new_revision > old_revision;
        }
        match (self.durability, existing.durability) {
            (SaveDurability::Durable, SaveDurability::Cache) => true,
            (SaveDurability::Cache, SaveDurability::Durable) => false,
            _ => true,
        }
    }
}

#[derive(Debug)]
pub struct PersistenceActor {
    store: Box<dyn WorldStore>,
    pending_chunk_writes: BTreeMap<ChunkPos, PendingChunkWrite>,
    completions: VecDeque<WorldStoreCompletion>,
    closed: bool,
}

impl PersistenceActor {
    pub fn new(store: Box<dyn WorldStore>) -> Self {
        Self {
            store,
            pending_chunk_writes: BTreeMap::new(),
            completions: VecDeque::new(),
            closed: false,
        }
    }

    pub fn load_chunk(&mut self, request_id: PersistenceRequestId, pos: ChunkPos) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::ChunkLoaded {
                    request_id,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let result = if let Some(pending) = self.pending_chunk_writes.get(&pos) {
            Ok(Some(pending.record.clone()))
        } else {
            self.store.load_chunk(pos)
        };
        self.completions
            .push_back(WorldStoreCompletion::ChunkLoaded {
                request_id,
                pos,
                result,
            });
    }

    pub fn save_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        record: ChunkRecord,
        durability: SaveDurability,
    ) {
        let pos = record.pos();
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::ChunkSaved {
                    request_id,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let incoming = PendingChunkWrite {
            request_id,
            record,
            durability,
        };
        if let Some(existing) = self.pending_chunk_writes.get_mut(&pos) {
            if incoming.should_replace(existing) {
                let superseded_id = existing.request_id;
                *existing = incoming;
                self.completions
                    .push_back(WorldStoreCompletion::ChunkSaved {
                        request_id: superseded_id,
                        pos,
                        result: Ok(StoreWriteOutcome::Superseded),
                    });
            } else {
                self.completions
                    .push_back(WorldStoreCompletion::ChunkSaved {
                        request_id,
                        pos,
                        result: Ok(StoreWriteOutcome::Superseded),
                    });
            }
            return;
        }

        self.pending_chunk_writes.insert(pos, incoming);
    }

    pub fn fail_reserved_request(&mut self, request_id: PersistenceRequestId, key: WorldRecordKey) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            Err(ChunkStoreError::InvalidData(format!(
                "persistence record family {key:?} is reserved but not implemented"
            )))
        };
        self.completions
            .push_back(WorldStoreCompletion::RequestFailed {
                request_id,
                key,
                result,
            });
    }

    pub fn flush(&mut self, request_id: PersistenceRequestId) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::FlushComplete {
                    request_id,
                    result: Err(closed_error()),
                });
            return;
        }

        let mut result =
            self.process_matching_pending_writes(|pending| pending.durability.is_durable());
        if result.is_ok() {
            result = self.store.flush();
        }
        self.completions
            .push_back(WorldStoreCompletion::FlushComplete { request_id, result });
    }

    pub fn close(&mut self, request_id: PersistenceRequestId) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::CloseComplete {
                    request_id,
                    result: Err(closed_error()),
                });
            return;
        }

        let mut result =
            self.process_matching_pending_writes(|pending| pending.durability.is_durable());
        for pos in self
            .pending_chunk_writes
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some(pending) = self.pending_chunk_writes.remove(&pos) {
                self.completions
                    .push_back(WorldStoreCompletion::ChunkSaved {
                        request_id: pending.request_id,
                        pos,
                        result: Ok(StoreWriteOutcome::SkippedOnClose),
                    });
            }
        }
        if result.is_ok() {
            result = self.store.close();
        } else {
            let _ = self.store.close();
        }
        self.closed = true;
        self.completions
            .push_back(WorldStoreCompletion::CloseComplete { request_id, result });
    }

    pub fn process_one_background_write(&mut self) -> bool {
        let Some(pos) = self.next_pending_write_pos(true) else {
            return false;
        };
        let _ = self.process_pending_write_at(pos);
        true
    }

    pub fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        self.completions.drain(..).collect()
    }

    fn process_matching_pending_writes(
        &mut self,
        mut predicate: impl FnMut(&PendingChunkWrite) -> bool,
    ) -> ChunkStoreResult<()> {
        let positions = self
            .pending_chunk_writes
            .iter()
            .filter_map(|(pos, pending)| predicate(pending).then_some(*pos))
            .collect::<Vec<_>>();
        let mut result = Ok(());
        for pos in positions {
            if self.pending_chunk_writes.contains_key(&pos) {
                let write_result = self.process_pending_write_at(pos);
                if result.is_ok() {
                    result = write_result;
                }
            }
        }
        result
    }

    fn next_pending_write_pos(&self, include_cache: bool) -> Option<ChunkPos> {
        if let Some(pos) = self
            .pending_chunk_writes
            .iter()
            .find_map(|(pos, pending)| pending.durability.is_durable().then_some(*pos))
        {
            return Some(pos);
        }
        include_cache.then_some(())?;
        self.pending_chunk_writes.keys().next().copied()
    }

    fn process_pending_write_at(&mut self, pos: ChunkPos) -> ChunkStoreResult<()> {
        let Some(pending) = self.pending_chunk_writes.remove(&pos) else {
            return Ok(());
        };
        let result = self.store.save_chunk(&pending.record);
        let barrier_result = result.as_ref().map(|_| ()).map_err(duplicate_store_error);
        self.completions
            .push_back(WorldStoreCompletion::ChunkSaved {
                request_id: pending.request_id,
                pos,
                result: result.map(|_| StoreWriteOutcome::Written),
            });
        barrier_result
    }
}

#[derive(Debug)]
pub struct PersistenceMailbox {
    actor: PersistenceActor,
    next_request_id: PersistenceRequestId,
}

impl PersistenceMailbox {
    pub fn new(store: Box<dyn WorldStore>) -> Self {
        Self {
            actor: PersistenceActor::new(store),
            next_request_id: 1,
        }
    }

    pub fn transient() -> Self {
        Self::new(Box::<NullWorldStore>::default())
    }

    pub fn memory() -> Self {
        Self::new(Box::<MemoryWorldStore>::default())
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor.load_chunk(request_id, pos);
        request_id
    }

    pub fn save_chunk(
        &mut self,
        record: ChunkRecord,
        durability: SaveDurability,
    ) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor.save_chunk(request_id, record, durability);
        request_id
    }

    pub fn load_entity_chunk(&mut self, pos: ChunkPos) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor
            .fail_reserved_request(request_id, WorldRecordKey::EntityChunk(pos));
        request_id
    }

    pub fn load_player(&mut self, player: PlayerRecordKey) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor
            .fail_reserved_request(request_id, WorldRecordKey::Player(player));
        request_id
    }

    pub fn load_saved_data(&mut self, key: String) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor
            .fail_reserved_request(request_id, WorldRecordKey::SavedData(key));
        request_id
    }

    pub fn flush(&mut self) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor.flush(request_id);
        request_id
    }

    pub fn close(&mut self) -> PersistenceRequestId {
        let request_id = self.next_request_id();
        self.actor.close(request_id);
        request_id
    }

    pub fn process_one_background_write(&mut self) -> bool {
        self.actor.process_one_background_write()
    }

    pub fn process_all_background_writes(&mut self) {
        while self.process_one_background_write() {}
    }

    pub fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        self.actor.drain_completions()
    }

    pub fn take_completion(
        &mut self,
        request_id: PersistenceRequestId,
    ) -> Option<WorldStoreCompletion> {
        let index = self
            .actor
            .completions
            .iter()
            .position(|completion| completion.request_id() == request_id)?;
        self.actor.completions.remove(index)
    }

    pub fn load_chunk_blocking(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        let request_id = self.load_chunk(pos);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::ChunkLoaded { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "load_chunk completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn save_chunk_blocking(
        &mut self,
        record: ChunkRecord,
        durability: SaveDurability,
    ) -> ChunkStoreResult<StoreWriteOutcome> {
        let request_id = self.save_chunk(record, durability);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::ChunkSaved { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "save_chunk completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn flush_blocking(&mut self) -> ChunkStoreResult<()> {
        let request_id = self.flush();
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::FlushComplete { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "flush completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn close_blocking(&mut self) -> ChunkStoreResult<()> {
        let request_id = self.close();
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::CloseComplete { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "close completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    fn next_request_id(&mut self) -> PersistenceRequestId {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        request_id
    }

    fn take_or_run_until_completion(
        &mut self,
        request_id: PersistenceRequestId,
    ) -> ChunkStoreResult<WorldStoreCompletion> {
        loop {
            if let Some(completion) = self.take_completion(request_id) {
                return Ok(completion);
            }
            if !self.process_one_background_write() {
                return Err(ChunkStoreError::InvalidData(format!(
                    "persistence request {request_id} did not complete"
                )));
            }
        }
    }
}

#[derive(Debug)]
pub struct SynchronousPersistenceFacade {
    mailbox: PersistenceMailbox,
}

impl SynchronousPersistenceFacade {
    pub fn transient() -> Self {
        Self {
            mailbox: PersistenceMailbox::transient(),
        }
    }

    pub fn from_world_store(store: Box<dyn WorldStore>) -> Self {
        Self {
            mailbox: PersistenceMailbox::new(store),
        }
    }

    pub fn from_chunk_snapshot_store(store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self::from_world_store(Box::new(ChunkSnapshotWorldStore::new(store)))
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        self.mailbox
            .load_chunk_blocking(pos)
            .map(|record| record.map(|record| record.snapshot))
    }

    pub fn save_chunk(&mut self, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
        let outcome = self.mailbox.save_chunk_blocking(
            ChunkRecord::from_snapshot(snapshot.clone()),
            SaveDurability::Durable,
        )?;
        match outcome {
            StoreWriteOutcome::Written | StoreWriteOutcome::Superseded => Ok(()),
            StoreWriteOutcome::SkippedOnClose => Err(closed_error()),
        }
    }

    pub fn flush(&mut self) -> ChunkStoreResult<()> {
        self.mailbox.flush_blocking()
    }

    pub fn close(&mut self) -> ChunkStoreResult<()> {
        self.mailbox.close_blocking()
    }
}

impl Default for SynchronousPersistenceFacade {
    fn default() -> Self {
        Self::transient()
    }
}

impl ChunkSnapshotStore for SynchronousPersistenceFacade {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        SynchronousPersistenceFacade::load_chunk(self, pos)
    }

    fn save_chunk(&mut self, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
        SynchronousPersistenceFacade::save_chunk(self, snapshot)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FilesystemChunkSnapshotStore {
    root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FilesystemChunkSnapshotStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn chunk_path(&self, pos: ChunkPos) -> PathBuf {
        self.root.join(format!("c.{}.{}.mcsnap", pos.x, pos.z))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ChunkSnapshotStore for FilesystemChunkSnapshotStore {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        let path = self.chunk_path(pos);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut reader = BufReader::new(file);
        let snapshot = read_chunk_record(&mut reader)?.snapshot;
        if snapshot.pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "chunk snapshot file {} contained position {:?}, expected {:?}",
                path.display(),
                snapshot.pos,
                pos
            )));
        }
        Ok(Some(snapshot))
    }

    fn save_chunk(&mut self, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.chunk_path(snapshot.pos);
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        write_chunk_record(&mut writer, &ChunkRecord::from_snapshot(snapshot.clone()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldStore for FilesystemChunkSnapshotStore {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        let path = self.chunk_path(pos);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut reader = BufReader::new(file);
        let record = read_chunk_record(&mut reader)?;
        if record.snapshot.pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "chunk snapshot file {} contained position {:?}, expected {:?}",
                path.display(),
                record.snapshot.pos,
                pos
            )));
        }
        Ok(Some(record))
    }

    fn save_chunk(&mut self, record: &ChunkRecord) -> ChunkStoreResult<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.chunk_path(record.pos());
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        write_chunk_record(&mut writer, record)
    }
}

#[cfg(test)]
fn write_snapshot(writer: &mut impl Write, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
    write_chunk_record(writer, &ChunkRecord::from_snapshot(snapshot.clone()))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_chunk_record(writer: &mut impl Write, record: &ChunkRecord) -> ChunkStoreResult<()> {
    let snapshot = &record.snapshot;
    writer.write_all(SNAPSHOT_MAGIC)?;
    write_u32(writer, SNAPSHOT_FORMAT_VERSION)?;
    write_i32(writer, snapshot.pos.x)?;
    write_i32(writer, snapshot.pos.z)?;
    write_u8(writer, status_to_u8(snapshot.status))?;
    write_u64(writer, snapshot.revision.0)?;
    write_i32(writer, snapshot.min_y)?;
    write_i32(writer, snapshot.height)?;
    write_len(writer, snapshot.biomes.len(), "biome count")?;
    for biome in &snapshot.biomes {
        write_i32(writer, *biome)?;
    }
    write_len(writer, snapshot.sections.len(), "section count")?;
    for section in &snapshot.sections {
        write_section(writer, section)?;
    }
    write_bool(writer, snapshot.light_correct)?;
    write_len(writer, snapshot.light_sections.len(), "light section count")?;
    for section in &snapshot.light_sections {
        write_light_section(writer, section)?;
    }
    write_optional_u32(writer, record.light_algorithm_version)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
fn read_snapshot(reader: &mut impl Read) -> ChunkStoreResult<ChunkSnapshot> {
    read_chunk_record(reader).map(|record| record.snapshot)
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_chunk_record(reader: &mut impl Read) -> ChunkStoreResult<ChunkRecord> {
    let mut magic = [0_u8; SNAPSHOT_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != SNAPSHOT_MAGIC {
        return Err(ChunkStoreError::InvalidData(
            "chunk snapshot had invalid magic".to_owned(),
        ));
    }

    let version = read_u32(reader)?;
    if !(1..=SNAPSHOT_FORMAT_VERSION).contains(&version) {
        return Err(ChunkStoreError::InvalidData(format!(
            "unsupported chunk snapshot format version {version}"
        )));
    }

    let pos = ChunkPos::new(read_i32(reader)?, read_i32(reader)?);
    let status = status_from_u8(read_u8(reader)?)?;
    let revision = ChunkRevision(read_u64(reader)?);
    let min_y = read_i32(reader)?;
    let height = read_i32(reader)?;
    let biomes = if version >= 3 {
        let biome_count = read_len(reader)?;
        let mut biomes = Vec::with_capacity(biome_count);
        for _ in 0..biome_count {
            biomes.push(read_i32(reader)?);
        }
        biomes
    } else {
        Vec::new()
    };
    let section_count = read_len(reader)?;
    let mut sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        sections.push(read_section(reader)?);
    }
    let (light_correct, light_sections) = if version >= 2 {
        let light_correct = read_bool(reader)?;
        let light_section_count = read_len(reader)?;
        let mut light_sections = Vec::with_capacity(light_section_count);
        for _ in 0..light_section_count {
            light_sections.push(read_light_section(reader)?);
        }
        (light_correct, light_sections)
    } else {
        (false, Vec::new())
    };

    let light_algorithm_version = if version >= 4 {
        read_optional_u32(reader)?
    } else {
        None
    };

    Ok(ChunkRecord {
        snapshot: ChunkSnapshot {
            pos,
            status,
            revision,
            min_y,
            height,
            biomes,
            sections,
            light_correct,
            light_sections,
        },
        light_algorithm_version,
    })
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_optional_u32(writer: &mut impl Write, value: Option<u32>) -> ChunkStoreResult<()> {
    match value {
        Some(value) => {
            write_bool(writer, true)?;
            write_u32(writer, value)?;
        }
        None => write_bool(writer, false)?,
    }
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_optional_u32(reader: &mut impl Read) -> ChunkStoreResult<Option<u32>> {
    if read_bool(reader)? {
        Ok(Some(read_u32(reader)?))
    } else {
        Ok(None)
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_section(writer: &mut impl Write, section: &PackedChunkSection) -> ChunkStoreResult<()> {
    write_i32(writer, section.section_y)?;
    write_len(writer, section.palette_state_ids.len(), "palette length")?;
    for state_id in &section.palette_state_ids {
        write_u32(writer, state_id.0)?;
    }
    write_u8(writer, section.bits_per_block)?;
    write_len(
        writer,
        section.packed_block_indices.len(),
        "packed block index length",
    )?;
    for word in &section.packed_block_indices {
        write_u64(writer, *word)?;
    }
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_section(reader: &mut impl Read) -> ChunkStoreResult<PackedChunkSection> {
    let section_y = read_i32(reader)?;
    let palette_len = read_len(reader)?;
    let mut palette_state_ids = Vec::with_capacity(palette_len);
    for _ in 0..palette_len {
        palette_state_ids.push(BlockStateId(read_u32(reader)?));
    }
    let bits_per_block = read_u8(reader)?;
    let packed_len = read_len(reader)?;
    let mut packed_block_indices = Vec::with_capacity(packed_len);
    for _ in 0..packed_len {
        packed_block_indices.push(read_u64(reader)?);
    }
    Ok(PackedChunkSection {
        section_y,
        palette_state_ids,
        bits_per_block,
        packed_block_indices,
    })
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_light_section(
    writer: &mut impl Write,
    section: &PackedLightSection,
) -> ChunkStoreResult<()> {
    write_i32(writer, section.section_y)?;
    write_optional_light_layer(writer, &section.sky, "sky light layer")?;
    write_optional_light_layer(writer, &section.block, "block light layer")?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_light_section(reader: &mut impl Read) -> ChunkStoreResult<PackedLightSection> {
    let section_y = read_i32(reader)?;
    let sky = read_optional_light_layer(reader)?;
    let block = read_optional_light_layer(reader)?;
    Ok(PackedLightSection::new(section_y, sky, block))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_optional_light_layer(
    writer: &mut impl Write,
    layer: &Option<Vec<u8>>,
    name: &str,
) -> ChunkStoreResult<()> {
    match layer {
        Some(bytes) => {
            if bytes.len() != LIGHT_DATA_LAYER_BYTE_COUNT {
                return Err(ChunkStoreError::InvalidData(format!(
                    "{name} has {} bytes; expected {LIGHT_DATA_LAYER_BYTE_COUNT}",
                    bytes.len()
                )));
            }
            write_bool(writer, true)?;
            writer.write_all(bytes)?;
        }
        None => write_bool(writer, false)?,
    }
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_optional_light_layer(reader: &mut impl Read) -> ChunkStoreResult<Option<Vec<u8>>> {
    if !read_bool(reader)? {
        return Ok(None);
    }
    let mut bytes = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_len(writer: &mut impl Write, value: usize, name: &str) -> ChunkStoreResult<()> {
    let value = u32::try_from(value)
        .map_err(|_| ChunkStoreError::InvalidData(format!("{name} {value} does not fit in u32")))?;
    write_u32(writer, value)
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_len(reader: &mut impl Read) -> ChunkStoreResult<usize> {
    usize::try_from(read_u32(reader)?).map_err(|_| {
        ChunkStoreError::InvalidData("chunk snapshot length does not fit in usize".to_owned())
    })
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn status_to_u8(status: ChunkStatus) -> u8 {
    match status {
        ChunkStatus::Terrain => 0,
        ChunkStatus::Surface => 1,
        ChunkStatus::Features => 2,
        ChunkStatus::Light => 3,
        ChunkStatus::Full => 4,
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn status_from_u8(value: u8) -> ChunkStoreResult<ChunkStatus> {
    match value {
        0 => Ok(ChunkStatus::Terrain),
        1 => Ok(ChunkStatus::Surface),
        2 => Ok(ChunkStatus::Features),
        3 => Ok(ChunkStatus::Light),
        4 => Ok(ChunkStatus::Full),
        _ => Err(ChunkStoreError::InvalidData(format!(
            "unknown chunk status id {value}"
        ))),
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_u8(writer: &mut impl Write, value: u8) -> ChunkStoreResult<()> {
    writer.write_all(&[value])?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_u8(reader: &mut impl Read) -> ChunkStoreResult<u8> {
    let mut bytes = [0_u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_bool(writer: &mut impl Write, value: bool) -> ChunkStoreResult<()> {
    write_u8(writer, u8::from(value))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_bool(reader: &mut impl Read) -> ChunkStoreResult<bool> {
    match read_u8(reader)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(ChunkStoreError::InvalidData(format!(
            "boolean value must be 0 or 1, got {value}"
        ))),
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_u32(writer: &mut impl Write, value: u32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_u32(reader: &mut impl Read) -> ChunkStoreResult<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_i32(writer: &mut impl Write, value: i32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_i32(reader: &mut impl Read) -> ChunkStoreResult<i32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(i32::from_le_bytes(bytes))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_u64(writer: &mut impl Write, value: u64) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_u64(reader: &mut impl Read) -> ChunkStoreResult<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, LIGHT_DATA_LAYER_BYTE_COUNT};

    #[test]
    fn binary_snapshot_format_roundtrips_sections() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        block_state_ids[7] = BlockStateId(3);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(-2, 5),
            ChunkStatus::Surface,
            ChunkRevision(42),
            0,
            16,
            &block_state_ids,
        )
        .with_light_sections(
            false,
            vec![
                PackedLightSection::new(0, Some(vec![0xFF; LIGHT_DATA_LAYER_BYTE_COUNT]), None),
                PackedLightSection::new(
                    1,
                    Some(vec![0x22; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    Some(vec![0x33; LIGHT_DATA_LAYER_BYTE_COUNT]),
                ),
            ],
        );
        let mut bytes = Vec::new();

        write_snapshot(&mut bytes, &snapshot).unwrap();
        let decoded = read_snapshot(&mut bytes.as_slice()).unwrap();

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn binary_chunk_record_format_roundtrips_light_algorithm_version() {
        let record = ChunkRecord {
            snapshot: test_snapshot(ChunkPos::new(0, 0), 7).with_light_sections(
                true,
                vec![PackedLightSection::new(
                    0,
                    Some(vec![0x11; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    None,
                )],
            ),
            light_algorithm_version: Some(CHUNK_LIGHT_ALGORITHM_VERSION),
        };
        let mut bytes = Vec::new();

        write_chunk_record(&mut bytes, &record).unwrap();
        let decoded = read_chunk_record(&mut bytes.as_slice()).unwrap();

        assert_eq!(decoded, record);
    }

    #[test]
    fn actor_load_sees_pending_same_chunk_write() {
        let mut mailbox = PersistenceMailbox::memory();
        let pos = ChunkPos::new(2, 3);
        let record = test_record(pos, 11);

        mailbox.save_chunk(record.clone(), SaveDurability::Durable);
        let load_id = mailbox.load_chunk(pos);

        assert_eq!(take_loaded_chunk(&mut mailbox, load_id), Some(record));
    }

    #[test]
    fn actor_newer_revision_supersedes_stale_queued_write() {
        let mut mailbox = PersistenceMailbox::memory();
        let pos = ChunkPos::new(4, 5);
        let stale_id = mailbox.save_chunk(test_record(pos, 1), SaveDurability::Durable);
        let newer = test_record(pos, 2);
        let newer_id = mailbox.save_chunk(newer.clone(), SaveDurability::Cache);

        assert_eq!(
            take_saved_chunk(&mut mailbox, stale_id),
            StoreWriteOutcome::Superseded
        );
        assert!(mailbox.process_one_background_write());
        assert_eq!(
            take_saved_chunk(&mut mailbox, newer_id),
            StoreWriteOutcome::Written
        );
        let load_id = mailbox.load_chunk(pos);
        assert_eq!(take_loaded_chunk(&mut mailbox, load_id), Some(newer));
    }

    #[test]
    fn actor_stale_cache_write_cannot_overwrite_higher_revision_durable_write() {
        let mut mailbox = PersistenceMailbox::memory();
        let pos = ChunkPos::new(6, 7);
        let durable = test_record(pos, 10);
        let durable_id = mailbox.save_chunk(durable.clone(), SaveDurability::Durable);
        let stale_id = mailbox.save_chunk(test_record(pos, 9), SaveDurability::Cache);

        assert_eq!(
            take_saved_chunk(&mut mailbox, stale_id),
            StoreWriteOutcome::Superseded
        );
        assert!(mailbox.process_one_background_write());
        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        let load_id = mailbox.load_chunk(pos);
        assert_eq!(take_loaded_chunk(&mut mailbox, load_id), Some(durable));
    }

    #[test]
    fn actor_equal_revision_prefers_durable_over_cache() {
        let mut mailbox = PersistenceMailbox::memory();
        let pos = ChunkPos::new(8, 9);
        let cache_id = mailbox.save_chunk(test_record(pos, 12), SaveDurability::Cache);
        let durable = test_record(pos, 12);
        let durable_id = mailbox.save_chunk(durable.clone(), SaveDurability::Durable);

        assert_eq!(
            take_saved_chunk(&mut mailbox, cache_id),
            StoreWriteOutcome::Superseded
        );
        assert!(mailbox.process_one_background_write());
        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        let load_id = mailbox.load_chunk(pos);
        assert_eq!(take_loaded_chunk(&mut mailbox, load_id), Some(durable));
    }

    #[test]
    fn actor_writes_durable_lane_before_cache_lane() {
        let mut mailbox = PersistenceMailbox::memory();
        let cache_pos = ChunkPos::new(10, 0);
        let durable_pos = ChunkPos::new(11, 0);
        let cache_id = mailbox.save_chunk(test_record(cache_pos, 1), SaveDurability::Cache);
        let durable_id = mailbox.save_chunk(test_record(durable_pos, 1), SaveDurability::Durable);

        assert!(mailbox.process_one_background_write());
        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        assert!(mailbox.take_completion(cache_id).is_none());
        assert!(mailbox.process_one_background_write());
        assert_eq!(
            take_saved_chunk(&mut mailbox, cache_id),
            StoreWriteOutcome::Written
        );
    }

    #[test]
    fn actor_flush_waits_for_pending_durable_writes() {
        let mut mailbox = PersistenceMailbox::memory();
        let durable_pos = ChunkPos::new(12, 0);
        let cache_pos = ChunkPos::new(13, 0);
        let durable_id = mailbox.save_chunk(test_record(durable_pos, 1), SaveDurability::Durable);
        let cache_id = mailbox.save_chunk(test_record(cache_pos, 1), SaveDurability::Cache);
        let flush_id = mailbox.flush();

        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        assert!(mailbox.take_completion(cache_id).is_none());
        take_flush_complete(&mut mailbox, flush_id);
        let durable_load_id = mailbox.load_chunk(durable_pos);
        assert_eq!(
            take_loaded_chunk(&mut mailbox, durable_load_id)
                .unwrap()
                .revision(),
            ChunkRevision(1)
        );
        let cache_load_id = mailbox.load_chunk(cache_pos);
        assert_eq!(
            take_loaded_chunk(&mut mailbox, cache_load_id)
                .unwrap()
                .revision(),
            ChunkRevision(1),
            "loads still see the pending cache write before it reaches the backend"
        );
    }

    #[test]
    fn actor_close_drains_durable_writes_and_skips_cache_writes() {
        let mut mailbox = PersistenceMailbox::memory();
        let durable_pos = ChunkPos::new(14, 0);
        let cache_pos = ChunkPos::new(15, 0);
        let durable_id = mailbox.save_chunk(test_record(durable_pos, 3), SaveDurability::Durable);
        let cache_id = mailbox.save_chunk(test_record(cache_pos, 3), SaveDurability::Cache);
        let close_id = mailbox.close();

        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        assert_eq!(
            take_saved_chunk(&mut mailbox, cache_id),
            StoreWriteOutcome::SkippedOnClose
        );
        take_close_complete(&mut mailbox, close_id);
        let load_id = mailbox.load_chunk(durable_pos);
        assert!(matches!(
            take_completion_result(&mut mailbox, load_id),
            Err(ChunkStoreError::Closed(_))
        ));
    }

    #[test]
    fn actor_post_close_save_fails_explicitly() {
        let mut mailbox = PersistenceMailbox::memory();
        let close_id = mailbox.close();
        take_close_complete(&mut mailbox, close_id);

        let save_id = mailbox.save_chunk(
            test_record(ChunkPos::new(16, 0), 1),
            SaveDurability::Durable,
        );

        assert!(matches!(
            take_save_result(&mut mailbox, save_id),
            Err(ChunkStoreError::Closed(_))
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn filesystem_chunk_store_roundtrips_snapshot() {
        let root = unique_temp_dir("filesystem_chunk_store_roundtrips_snapshot");
        let mut store = FilesystemChunkSnapshotStore::new(&root);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(3, -4),
            ChunkStatus::Surface,
            ChunkRevision(9),
            0,
            16,
            &vec![BlockStateId(1); CHUNK_SECTION_VOLUME],
        );

        ChunkSnapshotStore::save_chunk(&mut store, &snapshot).unwrap();
        assert_eq!(
            ChunkSnapshotStore::load_chunk(&mut store, ChunkPos::new(3, -4)).unwrap(),
            Some(snapshot)
        );
        assert_eq!(
            ChunkSnapshotStore::load_chunk(&mut store, ChunkPos::new(0, 0)).unwrap(),
            None
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn test_snapshot(pos: ChunkPos, revision: u64) -> ChunkSnapshot {
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Features,
            ChunkRevision(revision),
            0,
            16,
            &vec![BlockStateId(1); CHUNK_SECTION_VOLUME],
        )
    }

    fn test_record(pos: ChunkPos, revision: u64) -> ChunkRecord {
        ChunkRecord::from_snapshot(test_snapshot(pos, revision))
    }

    fn take_loaded_chunk(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> Option<ChunkRecord> {
        match mailbox
            .take_completion(request_id)
            .expect("missing chunk load completion")
        {
            WorldStoreCompletion::ChunkLoaded { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    fn take_saved_chunk(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        take_save_result(mailbox, request_id).unwrap()
    }

    fn take_save_result(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> ChunkStoreResult<StoreWriteOutcome> {
        match mailbox
            .take_completion(request_id)
            .expect("missing chunk save completion")
        {
            WorldStoreCompletion::ChunkSaved { result, .. } => result,
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    fn take_completion_result(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
        match mailbox
            .take_completion(request_id)
            .expect("missing chunk load completion")
        {
            WorldStoreCompletion::ChunkLoaded { result, .. } => result,
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    fn take_flush_complete(mailbox: &mut PersistenceMailbox, request_id: PersistenceRequestId) {
        match mailbox
            .take_completion(request_id)
            .expect("missing flush completion")
        {
            WorldStoreCompletion::FlushComplete { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    fn take_close_complete(mailbox: &mut PersistenceMailbox, request_id: PersistenceRequestId) {
        match mailbox
            .take_completion(request_id)
            .expect("missing close completion")
        {
            WorldStoreCompletion::CloseComplete { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn unique_temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("mclone-{name}-{}-{nanos}", std::process::id()));
        path
    }
}
