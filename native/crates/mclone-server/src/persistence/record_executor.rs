use std::collections::BTreeMap;
use std::fmt;

use mclone_core::ChunkPos;
use mclone_protocol::DimensionKey;

use super::{
    ChunkRecord, ChunkStoreError, ChunkStoreResult, DimensionRecord, EntityChunkRecord,
    PersistenceErrorKind, PlayerRecord, PlayerRecordKey, SNAPSHOT_FORMAT_VERSION, SavedDataRecord,
    WorldMetadata, WorldMetadataLoad, WorldStore, WorldStoreCompletion, WorldStoreRequest,
    decode_chunk_record, decode_dimension_record, decode_entity_chunk_record, decode_player_record,
    decode_world_metadata, encode_chunk_record, encode_dimension_record,
    encode_entity_chunk_record, encode_player_record, encode_world_metadata,
};

pub type PersistenceRecordRequestId = u64;

/// Stable physical namespaces. These numbers are part of the executor ABI;
/// platform adapters may map them to tables or object stores, but do not need
/// to understand the record family represented by each number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PersistenceRecordNamespace {
    WorldMetadata = 0,
    Dimension = 1,
    Chunk = 2,
    EntityChunk = 3,
    Player = 4,
    SavedData = 5,
}

impl PersistenceRecordNamespace {
    pub const ALL: [Self; 6] = [
        Self::WorldMetadata,
        Self::Dimension,
        Self::Chunk,
        Self::EntityChunk,
        Self::Player,
        Self::SavedData,
    ];

    pub const fn stable_id(self) -> u8 {
        self as u8
    }

    pub const fn from_stable_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::WorldMetadata),
            1 => Some(Self::Dimension),
            2 => Some(Self::Chunk),
            3 => Some(Self::EntityChunk),
            4 => Some(Self::Player),
            5 => Some(Self::SavedData),
            _ => None,
        }
    }
}

/// One component of a backend-neutral record key. Browser adapters prepend
/// their opened-world id; native backends bind these parts to table keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PersistenceRecordKeyPart {
    Text(String),
    I32(i32),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PersistenceRecordAddress {
    pub namespace: PersistenceRecordNamespace,
    pub key: Vec<PersistenceRecordKeyPart>,
}

impl PersistenceRecordAddress {
    pub fn new(
        namespace: PersistenceRecordNamespace,
        key: impl Into<Vec<PersistenceRecordKeyPart>>,
    ) -> Self {
        Self {
            namespace,
            key: key.into(),
        }
    }
}

/// Opaque encoded bytes plus write hints needed by physical backends such as
/// the existing SQLite schema. Record interpretation remains above this seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistenceRecordPayload {
    pub codec_version: u32,
    pub revision: u64,
    pub bytes: Vec<u8>,
}

impl PersistenceRecordPayload {
    pub fn new(codec_version: u32, revision: u64, bytes: Vec<u8>) -> Self {
        Self {
            codec_version,
            revision,
            bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistenceRecordMutation {
    Put {
        address: PersistenceRecordAddress,
        payload: PersistenceRecordPayload,
    },
    Delete {
        address: PersistenceRecordAddress,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PersistenceRecordBatch {
    pub mutations: Vec<PersistenceRecordMutation>,
}

impl PersistenceRecordBatch {
    pub fn new(mutations: Vec<PersistenceRecordMutation>) -> Self {
        Self { mutations }
    }

    pub fn put(address: PersistenceRecordAddress, payload: PersistenceRecordPayload) -> Self {
        Self::new(vec![PersistenceRecordMutation::Put { address, payload }])
    }

    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }
}

/// Owned request/response vocabulary used by asynchronous executors. No
/// request borrows Wasm memory across a browser promise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistenceRecordRequest {
    Read {
        request_id: PersistenceRecordRequestId,
        address: PersistenceRecordAddress,
    },
    ProbeAny {
        request_id: PersistenceRecordRequestId,
        namespaces: Vec<PersistenceRecordNamespace>,
    },
    Commit {
        request_id: PersistenceRecordRequestId,
        batch: PersistenceRecordBatch,
    },
    Flush {
        request_id: PersistenceRecordRequestId,
    },
    Close {
        request_id: PersistenceRecordRequestId,
    },
}

impl PersistenceRecordRequest {
    pub const fn request_id(&self) -> PersistenceRecordRequestId {
        match self {
            Self::Read { request_id, .. }
            | Self::ProbeAny { request_id, .. }
            | Self::Commit { request_id, .. }
            | Self::Flush { request_id }
            | Self::Close { request_id } => *request_id,
        }
    }
}

#[derive(Debug)]
pub enum PersistenceRecordResponse {
    Read {
        request_id: PersistenceRecordRequestId,
        address: PersistenceRecordAddress,
        result: ChunkStoreResult<Option<PersistenceRecordPayload>>,
    },
    ProbeAny {
        request_id: PersistenceRecordRequestId,
        result: ChunkStoreResult<bool>,
    },
    Commit {
        request_id: PersistenceRecordRequestId,
        result: ChunkStoreResult<()>,
    },
    Flush {
        request_id: PersistenceRecordRequestId,
        result: ChunkStoreResult<()>,
    },
    Close {
        request_id: PersistenceRecordRequestId,
        result: ChunkStoreResult<()>,
    },
}

impl PersistenceRecordResponse {
    pub const fn request_id(&self) -> PersistenceRecordRequestId {
        match self {
            Self::Read { request_id, .. }
            | Self::ProbeAny { request_id, .. }
            | Self::Commit { request_id, .. }
            | Self::Flush { request_id, .. }
            | Self::Close { request_id, .. } => *request_id,
        }
    }
}

/// Synchronous control executor. Native storage threads call this directly;
/// browser integration transports the owned request/response vocabulary above
/// through a completion port rather than implementing this trait with waits.
pub trait PersistenceRecordExecutor: fmt::Debug {
    fn read(
        &mut self,
        address: &PersistenceRecordAddress,
    ) -> ChunkStoreResult<Option<PersistenceRecordPayload>>;

    fn probe_any(&mut self, namespaces: &[PersistenceRecordNamespace]) -> ChunkStoreResult<bool>;

    /// Apply every mutation atomically or apply none of them.
    fn commit(&mut self, batch: &PersistenceRecordBatch) -> ChunkStoreResult<()>;

    fn flush(&mut self) -> ChunkStoreResult<()> {
        Ok(())
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        Ok(())
    }
}

/// Fatal-write latch for executors which cannot safely resume after a durable
/// operation fails. The first failure remains authoritative, so later work can
/// never make an unhealthy save path appear healthy again.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PersistenceExecutorFailureLatch {
    failure: Option<(PersistenceErrorKind, String)>,
}

impl PersistenceExecutorFailureLatch {
    pub fn check_healthy(&self) -> ChunkStoreResult<()> {
        match &self.failure {
            Some((kind, message)) => Err(ChunkStoreError::classified(*kind, message.clone())),
            None => Ok(()),
        }
    }

    pub fn poison(&mut self, error: &ChunkStoreError) -> ChunkStoreError {
        if self.failure.is_none() {
            self.failure = Some((error.kind(), error.to_string()));
        }
        self.check_healthy()
            .expect_err("persistence failure latch was just poisoned")
    }

    pub const fn is_healthy(&self) -> bool {
        self.failure.is_none()
    }
}

/// Synchronous compatibility adapter used by native storage actors and by
/// executor conformance tests. It is also the codec/key authority reused by
/// the asynchronous browser coordinator.
#[derive(Debug)]
pub struct RecordExecutorWorldStore<E> {
    executor: E,
    entity_chunks_supported: bool,
}

impl<E> RecordExecutorWorldStore<E> {
    pub fn new(executor: E, entity_chunks_supported: bool) -> Self {
        Self {
            executor,
            entity_chunks_supported,
        }
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }

    pub fn executor_mut(&mut self) -> &mut E {
        &mut self.executor
    }

    pub fn into_executor(self) -> E {
        self.executor
    }
}

impl RecordExecutorWorldStore<MemoryRecordExecutor> {
    pub fn memory() -> Self {
        Self::new(MemoryRecordExecutor::new(), true)
    }
}

impl RecordExecutorWorldStore<NullRecordExecutor> {
    pub fn null() -> Self {
        Self::new(NullRecordExecutor::default(), false)
    }
}

impl<E: PersistenceRecordExecutor> WorldStore for RecordExecutorWorldStore<E> {
    fn supports_entity_chunks(&self) -> bool {
        self.entity_chunks_supported
    }

    fn load_world_metadata(&mut self) -> ChunkStoreResult<WorldMetadataLoad> {
        let record = self
            .executor
            .read(&world_metadata_record_address())?
            .map(|payload| decode_world_metadata_payload(&payload))
            .transpose()?;
        let legacy_records_present = if record.is_some() {
            false
        } else {
            self.executor.probe_any(&[
                PersistenceRecordNamespace::Dimension,
                PersistenceRecordNamespace::Chunk,
                PersistenceRecordNamespace::EntityChunk,
                PersistenceRecordNamespace::Player,
            ])?
        };
        Ok(WorldMetadataLoad {
            record,
            legacy_records_present,
        })
    }

    fn save_world_metadata(&mut self, record: &WorldMetadata) -> ChunkStoreResult<()> {
        if self
            .load_world_metadata()?
            .record
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.executor.commit(&PersistenceRecordBatch::put(
            world_metadata_record_address(),
            PersistenceRecordPayload::new(
                record.codec_version,
                record.revision,
                encode_world_metadata(record)?,
            ),
        ))
    }

    fn load_dimension(&mut self, key: &DimensionKey) -> ChunkStoreResult<Option<DimensionRecord>> {
        let address = dimension_record_address(key);
        let Some(payload) = self.executor.read(&address)? else {
            return Ok(None);
        };
        let record = decode_dimension_payload(&payload)?;
        if record.key != *key {
            return Err(corrupt_record_error(
                &address,
                format!("record contained dimension key {}", record.key),
            ));
        }
        Ok(Some(record))
    }

    fn save_dimension(&mut self, record: &DimensionRecord) -> ChunkStoreResult<()> {
        if self
            .load_dimension(&record.key)?
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.executor.commit(&PersistenceRecordBatch::put(
            dimension_record_address(&record.key),
            PersistenceRecordPayload::new(
                record.codec_version,
                record.revision,
                encode_dimension_record(record)?,
            ),
        ))
    }

    fn load_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
        let address = chunk_record_address(PersistenceRecordNamespace::Chunk, dimension, pos);
        let Some(payload) = self.executor.read(&address)? else {
            return Ok(None);
        };
        let record = decode_chunk_payload(&payload)?;
        if record.pos() != pos {
            return Err(corrupt_record_error(
                &address,
                format!(
                    "record contained chunk position ({}, {})",
                    record.pos().x,
                    record.pos().z
                ),
            ));
        }
        Ok(Some(record))
    }

    fn save_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &ChunkRecord,
    ) -> ChunkStoreResult<()> {
        if self
            .load_chunk(dimension, record.pos())?
            .is_some_and(|stored| stored.revision() > record.revision())
        {
            return Ok(());
        }
        self.executor.commit(&PersistenceRecordBatch::put(
            chunk_record_address(PersistenceRecordNamespace::Chunk, dimension, record.pos()),
            PersistenceRecordPayload::new(
                SNAPSHOT_FORMAT_VERSION,
                record.revision().0,
                encode_chunk_record(record)?,
            ),
        ))
    }

    fn load_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        let address = chunk_record_address(PersistenceRecordNamespace::EntityChunk, dimension, pos);
        let Some(payload) = self.executor.read(&address)? else {
            return Ok(None);
        };
        let record = decode_entity_chunk_payload(&payload)?;
        if record.pos != pos {
            return Err(corrupt_record_error(
                &address,
                format!(
                    "record contained entity-chunk position ({}, {})",
                    record.pos.x, record.pos.z
                ),
            ));
        }
        Ok(Some(record))
    }

    fn save_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &EntityChunkRecord,
    ) -> ChunkStoreResult<()> {
        if self
            .load_entity_chunk(dimension, record.pos)?
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.executor.commit(&PersistenceRecordBatch::put(
            chunk_record_address(
                PersistenceRecordNamespace::EntityChunk,
                dimension,
                record.pos,
            ),
            PersistenceRecordPayload::new(
                record.codec_version,
                record.revision,
                encode_entity_chunk_record(record)?,
            ),
        ))
    }

    fn load_player(&mut self, player: &PlayerRecordKey) -> ChunkStoreResult<Option<PlayerRecord>> {
        let address = player_record_address(player);
        let Some(payload) = self.executor.read(&address)? else {
            return Ok(None);
        };
        let record = decode_player_payload(&payload)?;
        if record.player != *player {
            return Err(corrupt_record_error(
                &address,
                "record contained a different player key",
            ));
        }
        Ok(Some(record))
    }

    fn save_player(&mut self, record: &PlayerRecord) -> ChunkStoreResult<()> {
        if self
            .load_player(&record.player)?
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.executor.commit(&PersistenceRecordBatch::put(
            player_record_address(&record.player),
            PersistenceRecordPayload::new(
                record.codec_version,
                record.revision,
                encode_player_record(record)?,
            ),
        ))
    }

    fn load_saved_data(&mut self, key: &str) -> ChunkStoreResult<Option<SavedDataRecord>> {
        let address = saved_data_record_address(key);
        self.executor
            .read(&address)?
            .map(|payload| {
                SavedDataRecord::new(key, payload.codec_version, payload.revision, payload.bytes)
            })
            .transpose()
    }

    fn save_saved_data(&mut self, record: &SavedDataRecord) -> ChunkStoreResult<()> {
        if self
            .load_saved_data(&record.key)?
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.executor.commit(&PersistenceRecordBatch::put(
            saved_data_record_address(&record.key),
            PersistenceRecordPayload::new(
                record.codec_version,
                record.revision,
                record.bytes.clone(),
            ),
        ))
    }

    fn flush(&mut self) -> ChunkStoreResult<()> {
        self.executor.flush()
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        self.executor.close()
    }
}

pub fn world_metadata_record_address() -> PersistenceRecordAddress {
    PersistenceRecordAddress::new(PersistenceRecordNamespace::WorldMetadata, Vec::new())
}

pub fn dimension_record_address(key: &DimensionKey) -> PersistenceRecordAddress {
    PersistenceRecordAddress::new(
        PersistenceRecordNamespace::Dimension,
        vec![PersistenceRecordKeyPart::Text(key.as_str().to_owned())],
    )
}

pub fn chunk_record_address(
    namespace: PersistenceRecordNamespace,
    dimension: &DimensionKey,
    pos: ChunkPos,
) -> PersistenceRecordAddress {
    debug_assert!(matches!(
        namespace,
        PersistenceRecordNamespace::Chunk | PersistenceRecordNamespace::EntityChunk
    ));
    PersistenceRecordAddress::new(
        namespace,
        vec![
            PersistenceRecordKeyPart::Text(dimension.as_str().to_owned()),
            PersistenceRecordKeyPart::I32(pos.x),
            PersistenceRecordKeyPart::I32(pos.z),
        ],
    )
}

pub fn player_record_address(player: &PlayerRecordKey) -> PersistenceRecordAddress {
    PersistenceRecordAddress::new(
        PersistenceRecordNamespace::Player,
        vec![PersistenceRecordKeyPart::Text(player.as_str().to_owned())],
    )
}

pub fn saved_data_record_address(key: &str) -> PersistenceRecordAddress {
    PersistenceRecordAddress::new(
        PersistenceRecordNamespace::SavedData,
        vec![PersistenceRecordKeyPart::Text(key.to_owned())],
    )
}

/// Translate an engine load into the generic record read served by an
/// asynchronous platform executor. Save policy and decoding remain in Rust;
/// browser adapters only see this physical address.
pub fn record_read_for_world_store_request(
    request: &WorldStoreRequest,
) -> ChunkStoreResult<PersistenceRecordRequest> {
    let (request_id, address) = match request {
        WorldStoreRequest::LoadChunk {
            request_id,
            dimension,
            pos,
        } => (
            *request_id,
            chunk_record_address(PersistenceRecordNamespace::Chunk, dimension, *pos),
        ),
        WorldStoreRequest::LoadEntityChunk {
            request_id,
            dimension,
            pos,
        } => (
            *request_id,
            chunk_record_address(PersistenceRecordNamespace::EntityChunk, dimension, *pos),
        ),
        WorldStoreRequest::LoadPlayer { request_id, player } => {
            (*request_id, player_record_address(player))
        }
        WorldStoreRequest::LoadSavedData { request_id, key } => {
            (*request_id, saved_data_record_address(key))
        }
        other => {
            return Err(ChunkStoreError::InvalidData(format!(
                "world-store request is not an externally serviced record read: {other:?}"
            )));
        }
    };
    Ok(PersistenceRecordRequest::Read {
        request_id,
        address,
    })
}

/// Convert one generic record response back into the typed engine completion
/// for the original load. This is the inverse of
/// [`record_read_for_world_store_request`] and is deliberately shared so a
/// platform adapter never decodes an engine record family.
pub fn world_store_completion_from_record_read(
    request: WorldStoreRequest,
    response: PersistenceRecordResponse,
) -> ChunkStoreResult<WorldStoreCompletion> {
    let PersistenceRecordResponse::Read {
        request_id,
        address,
        result,
    } = response
    else {
        return Err(ChunkStoreError::InvalidData(
            "external world-store load completed with a non-read record response".to_owned(),
        ));
    };
    if request.request_id() != request_id {
        return Err(ChunkStoreError::InvalidData(format!(
            "record response id {request_id} did not match world-store request {}",
            request.request_id()
        )));
    }
    let expected = record_read_for_world_store_request(&request)?;
    let PersistenceRecordRequest::Read {
        address: expected_address,
        ..
    } = expected
    else {
        unreachable!("record-read translation always returns Read")
    };
    if address != expected_address {
        return Err(ChunkStoreError::InvalidData(format!(
            "record response address {address:?} did not match {expected_address:?}"
        )));
    }

    match request {
        WorldStoreRequest::LoadChunk { dimension, pos, .. } => {
            let decoded = result.and_then(|payload| {
                payload
                    .map(|payload| decode_chunk_payload(&payload))
                    .transpose()
            });
            Ok(WorldStoreCompletion::ChunkLoaded {
                request_id,
                dimension,
                pos,
                result: decoded,
            })
        }
        WorldStoreRequest::LoadEntityChunk { dimension, pos, .. } => {
            let decoded = result.and_then(|payload| {
                payload
                    .map(|payload| decode_entity_chunk_payload(&payload))
                    .transpose()
            });
            Ok(WorldStoreCompletion::EntityChunkLoaded {
                request_id,
                dimension,
                pos,
                result: decoded,
            })
        }
        WorldStoreRequest::LoadPlayer { player, .. } => {
            let decoded = result.and_then(|payload| {
                payload
                    .map(|payload| decode_player_payload(&payload))
                    .transpose()
            });
            Ok(WorldStoreCompletion::PlayerLoaded {
                request_id,
                player,
                result: decoded,
            })
        }
        WorldStoreRequest::LoadSavedData { key, .. } => {
            let decoded = result.and_then(|payload| {
                payload
                    .map(|payload| {
                        SavedDataRecord::new(
                            key.clone(),
                            payload.codec_version,
                            payload.revision,
                            payload.bytes,
                        )
                    })
                    .transpose()
            });
            Ok(WorldStoreCompletion::SavedDataLoaded {
                request_id,
                key,
                result: decoded,
            })
        }
        _ => unreachable!("record-read translation rejected non-load requests"),
    }
}

fn decode_world_metadata_payload(
    payload: &PersistenceRecordPayload,
) -> ChunkStoreResult<WorldMetadata> {
    decode_world_metadata(&payload.bytes).map_err(classify_decode_error)
}

fn decode_dimension_payload(
    payload: &PersistenceRecordPayload,
) -> ChunkStoreResult<DimensionRecord> {
    decode_dimension_record(&payload.bytes).map_err(classify_decode_error)
}

fn decode_chunk_payload(payload: &PersistenceRecordPayload) -> ChunkStoreResult<ChunkRecord> {
    decode_chunk_record(&payload.bytes).map_err(classify_decode_error)
}

fn decode_entity_chunk_payload(
    payload: &PersistenceRecordPayload,
) -> ChunkStoreResult<EntityChunkRecord> {
    decode_entity_chunk_record(&payload.bytes).map_err(classify_decode_error)
}

fn decode_player_payload(payload: &PersistenceRecordPayload) -> ChunkStoreResult<PlayerRecord> {
    decode_player_record(&payload.bytes).map_err(classify_decode_error)
}

fn classify_decode_error(error: ChunkStoreError) -> ChunkStoreError {
    let kind = match error.kind() {
        PersistenceErrorKind::Incompatible => PersistenceErrorKind::Incompatible,
        _ => PersistenceErrorKind::Corrupt,
    };
    ChunkStoreError::classified(kind, error.to_string())
}

fn corrupt_record_error(
    address: &PersistenceRecordAddress,
    detail: impl fmt::Display,
) -> ChunkStoreError {
    ChunkStoreError::classified(
        PersistenceErrorKind::Corrupt,
        format!("corrupt persistence record {address:?}: {detail}"),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRecordExecutorFault {
    NextRead,
    NextProbe,
    NextCommit,
    NextFlush,
    NextClose,
}

#[derive(Debug, Default)]
pub struct MemoryRecordExecutor {
    records: BTreeMap<PersistenceRecordAddress, PersistenceRecordPayload>,
    next_fault: Option<MemoryRecordExecutorFault>,
    closed: bool,
}

impl MemoryRecordExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_next(&mut self, fault: MemoryRecordExecutorFault) {
        self.next_fault = Some(fault);
    }

    pub fn reopen(mut self) -> Self {
        self.closed = false;
        self
    }

    pub fn record(&self, address: &PersistenceRecordAddress) -> Option<&PersistenceRecordPayload> {
        self.records.get(address)
    }

    fn check_open(&self) -> ChunkStoreResult<()> {
        if self.closed {
            Err(ChunkStoreError::Closed(
                "persistence record executor is closed".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    fn take_fault(&mut self, expected: MemoryRecordExecutorFault) -> ChunkStoreResult<()> {
        if self.next_fault == Some(expected) {
            self.next_fault = None;
            Err(ChunkStoreError::classified(
                PersistenceErrorKind::Backend,
                format!("injected memory record executor {expected:?} failure"),
            ))
        } else {
            Ok(())
        }
    }
}

impl PersistenceRecordExecutor for MemoryRecordExecutor {
    fn read(
        &mut self,
        address: &PersistenceRecordAddress,
    ) -> ChunkStoreResult<Option<PersistenceRecordPayload>> {
        self.check_open()?;
        self.take_fault(MemoryRecordExecutorFault::NextRead)?;
        Ok(self.records.get(address).cloned())
    }

    fn probe_any(&mut self, namespaces: &[PersistenceRecordNamespace]) -> ChunkStoreResult<bool> {
        self.check_open()?;
        self.take_fault(MemoryRecordExecutorFault::NextProbe)?;
        Ok(self
            .records
            .keys()
            .any(|address| namespaces.contains(&address.namespace)))
    }

    fn commit(&mut self, batch: &PersistenceRecordBatch) -> ChunkStoreResult<()> {
        self.check_open()?;
        self.take_fault(MemoryRecordExecutorFault::NextCommit)?;
        let mut next = self.records.clone();
        for mutation in &batch.mutations {
            match mutation {
                PersistenceRecordMutation::Put { address, payload } => {
                    next.insert(address.clone(), payload.clone());
                }
                PersistenceRecordMutation::Delete { address } => {
                    next.remove(address);
                }
            }
        }
        self.records = next;
        Ok(())
    }

    fn flush(&mut self) -> ChunkStoreResult<()> {
        self.check_open()?;
        self.take_fault(MemoryRecordExecutorFault::NextFlush)
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        self.check_open()?;
        self.take_fault(MemoryRecordExecutorFault::NextClose)?;
        self.closed = true;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NullRecordExecutor {
    closed: bool,
}

impl PersistenceRecordExecutor for NullRecordExecutor {
    fn read(
        &mut self,
        _address: &PersistenceRecordAddress,
    ) -> ChunkStoreResult<Option<PersistenceRecordPayload>> {
        self.check_open()?;
        Ok(None)
    }

    fn probe_any(&mut self, _namespaces: &[PersistenceRecordNamespace]) -> ChunkStoreResult<bool> {
        self.check_open()?;
        Ok(false)
    }

    fn commit(&mut self, _batch: &PersistenceRecordBatch) -> ChunkStoreResult<()> {
        self.check_open()
    }

    fn flush(&mut self) -> ChunkStoreResult<()> {
        self.check_open()
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        self.check_open()?;
        self.closed = true;
        Ok(())
    }
}

impl NullRecordExecutor {
    fn check_open(&self) -> ChunkStoreResult<()> {
        if self.closed {
            Err(ChunkStoreError::Closed(
                "persistence record executor is closed".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        PLAYER_RECORD_VERSION, WORLD_METADATA_TARGET_MINECRAFT_VERSION, WORLD_METADATA_VERSION,
    };
    use super::*;
    use mclone_core::{
        BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkSnapshot, ChunkStatus, Vec3d,
    };
    use mclone_protocol::{PlayerStatistics, RealmId};

    use crate::{WorldBehaviorProfile, WorldGenerationProfile};

    fn address(namespace: PersistenceRecordNamespace, name: &str) -> PersistenceRecordAddress {
        PersistenceRecordAddress::new(
            namespace,
            vec![PersistenceRecordKeyPart::Text(name.to_owned())],
        )
    }

    #[test]
    fn namespace_ids_are_stable_and_total() {
        for (expected, namespace) in PersistenceRecordNamespace::ALL.into_iter().enumerate() {
            assert_eq!(namespace.stable_id(), expected as u8);
            assert_eq!(
                PersistenceRecordNamespace::from_stable_id(expected as u8),
                Some(namespace)
            );
        }
        assert_eq!(PersistenceRecordNamespace::from_stable_id(6), None);
    }

    #[test]
    fn owned_async_vocabulary_preserves_request_correlation() {
        let request = PersistenceRecordRequest::Read {
            request_id: 41,
            address: address(PersistenceRecordNamespace::Chunk, "record"),
        };
        let response = PersistenceRecordResponse::Commit {
            request_id: 42,
            result: Ok(()),
        };

        assert_eq!(request.request_id(), 41);
        assert_eq!(response.request_id(), 42);
    }

    #[test]
    fn memory_executor_conforms_to_atomic_record_contract() {
        let first = address(PersistenceRecordNamespace::Chunk, "first");
        let second = address(PersistenceRecordNamespace::Player, "second");
        let first_payload = PersistenceRecordPayload::new(2, 7, vec![1, 2, 3]);
        let second_payload = PersistenceRecordPayload::new(3, 8, vec![4, 5]);
        let mut executor = MemoryRecordExecutor::new();

        assert_eq!(executor.read(&first).unwrap(), None);
        executor
            .commit(&PersistenceRecordBatch::put(
                first.clone(),
                first_payload.clone(),
            ))
            .unwrap();
        assert_eq!(executor.read(&first).unwrap(), Some(first_payload.clone()));
        assert!(
            executor
                .probe_any(&[PersistenceRecordNamespace::Chunk])
                .unwrap()
        );

        executor.fail_next(MemoryRecordExecutorFault::NextCommit);
        let failed = executor.commit(&PersistenceRecordBatch::new(vec![
            PersistenceRecordMutation::Delete {
                address: first.clone(),
            },
            PersistenceRecordMutation::Put {
                address: second.clone(),
                payload: second_payload,
            },
        ]));
        assert_eq!(failed.unwrap_err().kind(), PersistenceErrorKind::Backend);
        assert_eq!(executor.read(&first).unwrap(), Some(first_payload));
        assert_eq!(executor.read(&second).unwrap(), None);

        executor.flush().unwrap();
        executor.close().unwrap();
        assert_eq!(
            executor.read(&first).unwrap_err().kind(),
            PersistenceErrorKind::Closed
        );
    }

    #[test]
    fn null_executor_is_noop_but_obeys_lifecycle() {
        let key = address(PersistenceRecordNamespace::WorldMetadata, "singleton");
        let mut executor = NullRecordExecutor::default();

        executor
            .commit(&PersistenceRecordBatch::put(
                key.clone(),
                PersistenceRecordPayload::new(1, 1, vec![1]),
            ))
            .unwrap();
        assert_eq!(executor.read(&key).unwrap(), None);
        executor.flush().unwrap();
        executor.close().unwrap();
        assert_eq!(
            executor.flush().unwrap_err().kind(),
            PersistenceErrorKind::Closed
        );
    }

    #[test]
    fn typed_failure_categories_do_not_require_message_parsing() {
        for kind in [
            PersistenceErrorKind::Corrupt,
            PersistenceErrorKind::Incompatible,
            PersistenceErrorKind::Quota,
            PersistenceErrorKind::Unavailable,
            PersistenceErrorKind::LeaseConflict,
            PersistenceErrorKind::Cancelled,
            PersistenceErrorKind::Backend,
        ] {
            let error = ChunkStoreError::classified(kind, "backend detail");
            assert_eq!(error.kind(), kind);
            assert_eq!(error.to_string(), "backend detail");
        }
    }

    #[test]
    fn world_store_adapter_roundtrips_every_live_record_family_across_reopen() {
        let overworld = DimensionKey::overworld();
        let metadata = world_metadata(7);
        let dimension = DimensionRecord::overworld(44, WorldGenerationProfile::FlatGrassV1);
        let chunk = chunk_record(ChunkPos::new(-2, 5), 8);
        let entities = EntityChunkRecord::empty(ChunkPos::new(-2, 5), 9);
        let player = player_record(10);
        let saved_data = SavedDataRecord::new("mclone:test-data", 3, 11, vec![1, 4, 9]).unwrap();
        let mut store = RecordExecutorWorldStore::memory();

        assert_eq!(
            store.load_world_metadata().unwrap(),
            WorldMetadataLoad::default()
        );
        assert_eq!(store.load_dimension(&overworld).unwrap(), None);
        assert_eq!(store.load_chunk(&overworld, chunk.pos()).unwrap(), None);
        assert_eq!(
            store.load_entity_chunk(&overworld, entities.pos).unwrap(),
            None
        );
        assert_eq!(store.load_player(&player.player).unwrap(), None);
        assert_eq!(store.load_saved_data(&saved_data.key).unwrap(), None);

        store.save_world_metadata(&metadata).unwrap();
        store.save_dimension(&dimension).unwrap();
        store.save_chunk(&overworld, &chunk).unwrap();
        store.save_entity_chunk(&overworld, &entities).unwrap();
        store.save_player(&player).unwrap();
        store.save_saved_data(&saved_data).unwrap();
        store.flush().unwrap();
        store.close().unwrap();

        let mut reopened = RecordExecutorWorldStore::new(store.into_executor().reopen(), true);
        assert_eq!(
            reopened.load_world_metadata().unwrap().record,
            Some(metadata)
        );
        assert_eq!(
            reopened.load_dimension(&overworld).unwrap(),
            Some(dimension)
        );
        assert_eq!(
            reopened.load_chunk(&overworld, chunk.pos()).unwrap(),
            Some(chunk)
        );
        assert_eq!(
            reopened
                .load_entity_chunk(&overworld, entities.pos)
                .unwrap(),
            Some(entities)
        );
        assert_eq!(reopened.load_player(&player.player).unwrap(), Some(player));
        assert_eq!(
            reopened.load_saved_data(&saved_data.key).unwrap(),
            Some(saved_data)
        );
    }

    #[test]
    fn world_store_adapter_rejects_stale_persisted_revisions() {
        let overworld = DimensionKey::overworld();
        let pos = ChunkPos::new(3, 4);
        let mut store = RecordExecutorWorldStore::memory();

        store
            .save_chunk(&overworld, &chunk_record(pos, 12))
            .unwrap();
        store
            .save_chunk(&overworld, &chunk_record(pos, 11))
            .unwrap();
        store
            .save_saved_data(&SavedDataRecord::new("mclone:test", 1, 12, vec![12]).unwrap())
            .unwrap();
        store
            .save_saved_data(&SavedDataRecord::new("mclone:test", 1, 11, vec![11]).unwrap())
            .unwrap();

        assert_eq!(
            store
                .load_chunk(&overworld, pos)
                .unwrap()
                .unwrap()
                .revision(),
            ChunkRevision(12)
        );
        assert_eq!(
            store.load_saved_data("mclone:test").unwrap().unwrap().bytes,
            vec![12]
        );
    }

    #[test]
    fn world_store_adapter_classifies_corrupt_physical_bytes() {
        let overworld = DimensionKey::overworld();
        let pos = ChunkPos::new(6, 7);
        let address = chunk_record_address(PersistenceRecordNamespace::Chunk, &overworld, pos);
        let mut store = RecordExecutorWorldStore::memory();
        store
            .executor_mut()
            .commit(&PersistenceRecordBatch::put(
                address,
                PersistenceRecordPayload::new(SNAPSHOT_FORMAT_VERSION, 1, vec![0, 1, 2]),
            ))
            .unwrap();

        assert_eq!(
            store.load_chunk(&overworld, pos).unwrap_err().kind(),
            PersistenceErrorKind::Corrupt
        );
    }

    #[test]
    fn external_record_read_roundtrips_without_platform_domain_projection() {
        let dimension = DimensionKey::overworld();
        let pos = ChunkPos::new(-7, 9);
        let request = WorldStoreRequest::LoadChunk {
            request_id: 41,
            dimension: dimension.clone(),
            pos,
        };
        let record_request = record_read_for_world_store_request(&request).unwrap();
        let PersistenceRecordRequest::Read {
            request_id,
            address,
        } = record_request
        else {
            panic!("chunk load did not become a generic read");
        };
        assert_eq!(request_id, 41);
        assert_eq!(address.namespace, PersistenceRecordNamespace::Chunk);
        assert_eq!(
            address.key,
            vec![
                PersistenceRecordKeyPart::Text(dimension.as_str().to_owned()),
                PersistenceRecordKeyPart::I32(-7),
                PersistenceRecordKeyPart::I32(9),
            ]
        );

        let record = chunk_record(pos, 12);
        let completion = world_store_completion_from_record_read(
            request,
            PersistenceRecordResponse::Read {
                request_id,
                address,
                result: Ok(Some(PersistenceRecordPayload::new(
                    SNAPSHOT_FORMAT_VERSION,
                    12,
                    encode_chunk_record(&record).unwrap(),
                ))),
            },
        )
        .unwrap();
        match completion {
            WorldStoreCompletion::ChunkLoaded { result, .. } => {
                assert_eq!(result.unwrap(), Some(record));
            }
            other => panic!("unexpected completion {other:?}"),
        }

        let saved_request = WorldStoreRequest::LoadSavedData {
            request_id: 42,
            key: "mclone:test-data".to_owned(),
        };
        let saved_address = saved_data_record_address("mclone:test-data");
        assert_eq!(
            record_read_for_world_store_request(&saved_request).unwrap(),
            PersistenceRecordRequest::Read {
                request_id: 42,
                address: saved_address.clone(),
            }
        );
        let completion = world_store_completion_from_record_read(
            saved_request,
            PersistenceRecordResponse::Read {
                request_id: 42,
                address: saved_address,
                result: Ok(Some(PersistenceRecordPayload::new(7, 8, vec![2, 3]))),
            },
        )
        .unwrap();
        assert!(matches!(
            completion,
            WorldStoreCompletion::SavedDataLoaded {
                result: Ok(Some(SavedDataRecord {
                    codec_version: 7,
                    revision: 8,
                    bytes,
                    ..
                })),
                ..
            } if bytes == vec![2, 3]
        ));
    }

    #[test]
    fn fatal_executor_failure_latch_keeps_the_first_typed_error() {
        let mut health = PersistenceExecutorFailureLatch::default();
        assert!(health.is_healthy());
        health.poison(&ChunkStoreError::classified(
            PersistenceErrorKind::Quota,
            "browser quota exhausted",
        ));
        health.poison(&ChunkStoreError::classified(
            PersistenceErrorKind::Backend,
            "later generic backend failure",
        ));

        assert!(!health.is_healthy());
        let error = health.check_healthy().unwrap_err();
        assert_eq!(error.kind(), PersistenceErrorKind::Quota);
        assert!(error.to_string().contains("browser quota exhausted"));
    }

    fn chunk_record(pos: ChunkPos, revision: u64) -> ChunkRecord {
        ChunkRecord::from_snapshot(ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Features,
            ChunkRevision(revision),
            0,
            16,
            &vec![BlockStateId(1); CHUNK_SECTION_VOLUME],
        ))
    }

    fn player_record(revision: u64) -> PlayerRecord {
        PlayerRecord {
            player: PlayerRecordKey::Uuid("00112233-4455-6677-8899-aabbccddeeff".to_owned()),
            codec_version: PLAYER_RECORD_VERSION,
            revision,
            last_known_name: "Builder".to_owned(),
            dimension: DimensionKey::overworld(),
            position: Vec3d::new(1.5, 64.0, 2.5),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
            selected_hotbar_slot: 0,
            inventory: [None; 36],
            total_experience: 0,
            statistics: PlayerStatistics::default(),
            mallard_field_guide: mclone_protocol::MallardFieldGuideProgress::default(),
            health: 20.0,
            pending_death_cause: None,
        }
    }

    fn world_metadata(revision: u64) -> WorldMetadata {
        WorldMetadata {
            codec_version: WORLD_METADATA_VERSION,
            realm_id: RealmId::new([0x5a; 16]).unwrap(),
            revision,
            target_minecraft_version: WORLD_METADATA_TARGET_MINECRAFT_VERSION.to_owned(),
            seed: 44,
            world_generation_profile: WorldGenerationProfile::FlatGrassV1,
            starter_content: crate::StarterContentDescriptor::IntroHomesteadV1,
            realized_starter_plan: Some(crate::RealizedStarterPlanIdentity::new(1, 2, [0x44; 32])),
            world_behavior_profile: WorldBehaviorProfile::Mutable,
            created_unix_millis: 1,
            last_played_unix_millis: 2,
            game_time: 3,
            day_time: 4,
            do_daylight_cycle: true,
        }
    }
}
