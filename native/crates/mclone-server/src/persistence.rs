use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::io;
use std::rc::Rc;

use std::io::{Read, Write};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(not(target_arch = "wasm32"))]
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};

use mclone_core::{
    AxisTopology, BlockPos, ChunkPos, ChunkRevision, ChunkSnapshot, HorizontalTopology, Vec3d,
};
pub use mclone_protocol::EntityPersistentId;
use mclone_protocol::{
    DEFAULT_PLAYER_MAX_HEALTH, DimensionChunkPos, DimensionKey, EntityRotation, ItemStackSnapshot,
    MAX_PLAYER_STATISTIC_ENTRIES, MAX_STATISTIC_RESOURCE_KEY_BYTES, PlayerDamageCause,
    PlayerStatistics, PlayerVitals, RealmId, StatisticKey,
};

use crate::{WorldBehaviorProfile, WorldGenerationProfile};

mod record_executor;
pub use record_executor::{
    MemoryRecordExecutor, MemoryRecordExecutorFault, NullRecordExecutor,
    PersistenceExecutorFailureLatch, PersistenceRecordAddress, PersistenceRecordBatch,
    PersistenceRecordExecutor, PersistenceRecordKeyPart, PersistenceRecordMutation,
    PersistenceRecordNamespace, PersistenceRecordPayload, PersistenceRecordRequest,
    PersistenceRecordRequestId, PersistenceRecordResponse, RecordExecutorWorldStore,
    chunk_record_address, dimension_record_address, player_record_address,
    record_read_for_world_store_request, world_metadata_record_address,
    world_store_completion_from_record_read,
};

use mclone_core::{
    BlockStateId, ChunkStatus, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection, PackedLightSection,
};

const SNAPSHOT_MAGIC: &[u8; 12] = b"MCLONESNAP\0\0";
const SNAPSHOT_FORMAT_VERSION: u32 = 5;
const ENTITY_CHUNK_MAGIC: &[u8; 12] = b"MCLONEENT\0\0\0";
const PLAYER_RECORD_MAGIC: &[u8; 12] = b"MCLONEPLYR\0\0";
const WORLD_METADATA_MAGIC: &[u8; 12] = b"MCLONEWRLD\0\0";
const DIMENSION_RECORD_MAGIC: &[u8; 12] = b"MCLONEDIM\0\0\0";
#[cfg(not(target_arch = "wasm32"))]
const SQLITE_WORLD_SCHEMA_VERSION: i64 = 2;
#[cfg(not(target_arch = "wasm32"))]
pub const SQLITE_WORLD_DATABASE_FILE: &str = "world.sqlite3";
#[cfg(not(target_arch = "wasm32"))]
pub const WORLD_WRITER_LOCK_FILE: &str = "world.writer.lock";
#[cfg(not(target_arch = "wasm32"))]
const WORLD_ADMISSION_LOCK_FILE: &str = ".mclone-world-admission.lock";

pub const CHUNK_LIGHT_ALGORITHM_VERSION: u32 = 1;
pub const ENTITY_CHUNK_RECORD_VERSION: u32 = 2;
const LEGACY_PLAYER_RECORD_VERSION: u32 = 1;
const STATISTICS_PLAYER_RECORD_VERSION: u32 = 2;
pub const PLAYER_RECORD_VERSION: u32 = 3;
pub const DIMENSION_RECORD_VERSION: u32 = 2;
pub const WORLD_METADATA_VERSION: u32 = 2;
pub const WORLD_METADATA_TARGET_MINECRAFT_VERSION: &str = "1.17.1";

pub type PersistenceRequestId = u64;

pub type ChunkStoreResult<T> = Result<T, ChunkStoreError>;

/// Stable failure categories shared by persistence coordinators and physical
/// executors. The diagnostic message remains backend-specific; engine policy
/// branches on this category instead of parsing that message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistenceErrorKind {
    Io,
    InvalidData,
    Corrupt,
    Incompatible,
    Quota,
    Unavailable,
    LeaseConflict,
    Closed,
    Cancelled,
    Backend,
}

#[derive(Debug)]
pub enum ChunkStoreError {
    Io(io::Error),
    InvalidData(String),
    Closed(String),
    Classified {
        kind: PersistenceErrorKind,
        message: String,
    },
}

impl ChunkStoreError {
    pub const fn kind(&self) -> PersistenceErrorKind {
        match self {
            Self::Io(_) => PersistenceErrorKind::Io,
            Self::InvalidData(_) => PersistenceErrorKind::InvalidData,
            Self::Closed(_) => PersistenceErrorKind::Closed,
            Self::Classified { kind, .. } => *kind,
        }
    }

    pub fn classified(kind: PersistenceErrorKind, message: impl Into<String>) -> Self {
        Self::Classified {
            kind,
            message: message.into(),
        }
    }
}

impl PersistenceErrorKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::InvalidData => "invalid-data",
            Self::Corrupt => "corrupt",
            Self::Incompatible => "incompatible",
            Self::Quota => "quota",
            Self::Unavailable => "unavailable",
            Self::LeaseConflict => "lease-conflict",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
            Self::Backend => "backend",
        }
    }

    pub fn parse_label(label: &str) -> Option<Self> {
        match label {
            "io" => Some(Self::Io),
            "invalid-data" => Some(Self::InvalidData),
            "corrupt" => Some(Self::Corrupt),
            "incompatible" => Some(Self::Incompatible),
            "quota" => Some(Self::Quota),
            "unavailable" => Some(Self::Unavailable),
            "lease-conflict" => Some(Self::LeaseConflict),
            "closed" => Some(Self::Closed),
            "cancelled" => Some(Self::Cancelled),
            "backend" => Some(Self::Backend),
            _ => None,
        }
    }
}

impl fmt::Display for ChunkStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidData(message) => f.write_str(message),
            Self::Closed(message) => f.write_str(message),
            Self::Classified { message, .. } => f.write_str(message),
        }
    }
}

impl std::error::Error for ChunkStoreError {}

impl From<io::Error> for ChunkStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sqlite_error(error: rusqlite::Error) -> ChunkStoreError {
    ChunkStoreError::classified(
        PersistenceErrorKind::Backend,
        format!("sqlite world store error: {error}"),
    )
}

fn duplicate_store_error(error: &ChunkStoreError) -> ChunkStoreError {
    ChunkStoreError::classified(error.kind(), error.to_string())
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
    pub scheduled_block_ticks: Vec<ScheduledTickRecord>,
    pub scheduled_fluid_ticks: Vec<ScheduledTickRecord>,
}

impl ChunkRecord {
    pub fn from_snapshot(snapshot: ChunkSnapshot) -> Self {
        let light_algorithm_version = snapshot
            .light_correct
            .then_some(CHUNK_LIGHT_ALGORITHM_VERSION);
        Self {
            snapshot,
            light_algorithm_version,
            scheduled_block_ticks: Vec::new(),
            scheduled_fluid_ticks: Vec::new(),
        }
    }

    pub fn legacy_snapshot(snapshot: ChunkSnapshot) -> Self {
        Self {
            snapshot,
            light_algorithm_version: None,
            scheduled_block_ticks: Vec::new(),
            scheduled_fluid_ticks: Vec::new(),
        }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.snapshot.pos
    }

    pub const fn revision(&self) -> ChunkRevision {
        self.snapshot.revision
    }

    pub fn with_scheduled_block_ticks(mut self, ticks: Vec<ScheduledTickRecord>) -> Self {
        self.scheduled_block_ticks = ticks;
        self
    }

    pub fn with_scheduled_fluid_ticks(mut self, ticks: Vec<ScheduledTickRecord>) -> Self {
        self.scheduled_fluid_ticks = ticks;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledTickRecord {
    pub pos: BlockPos,
    pub target: String,
    pub delay: i32,
}

impl ScheduledTickRecord {
    pub fn new(pos: BlockPos, target: impl Into<String>, delay: i32) -> Self {
        Self {
            pos,
            target: target.into(),
            delay,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityChunkRecord {
    pub pos: ChunkPos,
    pub revision: u64,
    pub codec_version: u32,
    pub entities: Vec<EntitySaveRecord>,
}

impl EntityChunkRecord {
    pub fn new(pos: ChunkPos, revision: u64, entities: Vec<EntitySaveRecord>) -> Self {
        Self {
            pos,
            revision,
            codec_version: ENTITY_CHUNK_RECORD_VERSION,
            entities,
        }
    }

    pub const fn empty(pos: ChunkPos, revision: u64) -> Self {
        Self {
            pos,
            revision,
            codec_version: ENTITY_CHUNK_RECORD_VERSION,
            entities: Vec::new(),
        }
    }
}

pub fn encode_chunk_record(record: &ChunkRecord) -> ChunkStoreResult<Vec<u8>> {
    let mut bytes = Vec::new();
    write_chunk_record(&mut bytes, record)?;
    Ok(bytes)
}

pub fn decode_chunk_record(bytes: &[u8]) -> ChunkStoreResult<ChunkRecord> {
    let mut reader = bytes;
    read_chunk_record(&mut reader)
}

pub fn encode_entity_chunk_record(record: &EntityChunkRecord) -> ChunkStoreResult<Vec<u8>> {
    let mut bytes = Vec::new();
    write_entity_chunk_record(&mut bytes, record)?;
    Ok(bytes)
}

pub fn decode_entity_chunk_record(bytes: &[u8]) -> ChunkStoreResult<EntityChunkRecord> {
    let mut reader = bytes;
    read_entity_chunk_record(&mut reader)
}

pub fn encode_player_record(record: &PlayerRecord) -> ChunkStoreResult<Vec<u8>> {
    let mut bytes = Vec::new();
    write_player_record(&mut bytes, record)?;
    Ok(bytes)
}

pub fn decode_player_record(bytes: &[u8]) -> ChunkStoreResult<PlayerRecord> {
    let mut reader = bytes;
    let record = read_player_record(&mut reader)?;
    if !reader.is_empty() {
        return Err(ChunkStoreError::InvalidData(format!(
            "player record had {} trailing bytes",
            reader.len()
        )));
    }
    Ok(record)
}

pub fn encode_world_metadata(record: &WorldMetadata) -> ChunkStoreResult<Vec<u8>> {
    let mut bytes = Vec::new();
    write_world_metadata(&mut bytes, record)?;
    Ok(bytes)
}

pub fn decode_world_metadata(bytes: &[u8]) -> ChunkStoreResult<WorldMetadata> {
    let mut reader = bytes;
    let record = read_world_metadata(&mut reader)?;
    if !reader.is_empty() {
        return Err(ChunkStoreError::InvalidData(format!(
            "world metadata had {} trailing bytes",
            reader.len()
        )));
    }
    Ok(record)
}

pub fn encode_dimension_record(record: &DimensionRecord) -> ChunkStoreResult<Vec<u8>> {
    let mut bytes = Vec::new();
    write_dimension_record(&mut bytes, record)?;
    Ok(bytes)
}

pub fn decode_dimension_record(bytes: &[u8]) -> ChunkStoreResult<DimensionRecord> {
    let mut reader = bytes;
    let record = read_dimension_record(&mut reader)?;
    if !reader.is_empty() {
        return Err(ChunkStoreError::InvalidData(format!(
            "dimension record had {} trailing bytes",
            reader.len()
        )));
    }
    Ok(record)
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySaveRecord {
    pub persistent_id: EntityPersistentId,
    pub kind: String,
    pub position: Vec3d,
    pub delta_movement: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub payload: EntitySavePayload,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntitySavePayload {
    Cow,
    Mannequin,
    Chicken {
        egg_time: i32,
    },
    Item {
        stack: ItemStackSaveRecord,
        age: u64,
        pickup_delay: i32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemStackSaveRecord {
    pub kind: String,
    pub count: u8,
}

impl ItemStackSaveRecord {
    pub fn new(kind: impl Into<String>, count: u8) -> Self {
        Self {
            kind: kind.into(),
            count,
        }
    }
}

impl From<ItemStackSnapshot> for ItemStackSaveRecord {
    fn from(stack: ItemStackSnapshot) -> Self {
        let kind = match stack.kind {
            mclone_protocol::ItemKind::Egg => "minecraft:egg",
        };
        Self::new(kind, stack.count)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PlayerRecordKey {
    Uuid(String),
}

impl PlayerRecordKey {
    pub fn from_profile_id(profile_id: mclone_protocol::PlayerProfileId) -> Self {
        let bytes = profile_id.bytes();
        Self::Uuid(format!(
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

    pub fn as_str(&self) -> &str {
        match self {
            Self::Uuid(value) => value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRecord {
    pub player: PlayerRecordKey,
    pub codec_version: u32,
    pub revision: u64,
    pub last_known_name: String,
    pub dimension: DimensionKey,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub on_ground: bool,
    pub selected_hotbar_slot: u8,
    pub total_experience: u64,
    pub statistics: PlayerStatistics,
    pub health: f32,
    pub pending_death_cause: Option<PlayerDamageCause>,
}

impl PlayerRecord {
    pub fn new(
        player: PlayerRecordKey,
        revision: u64,
        last_known_name: impl Into<String>,
        position: Vec3d,
    ) -> Self {
        Self {
            player,
            codec_version: PLAYER_RECORD_VERSION,
            revision,
            last_known_name: last_known_name.into(),
            dimension: DimensionKey::overworld(),
            position,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: false,
            selected_hotbar_slot: 0,
            total_experience: 0,
            statistics: PlayerStatistics::default(),
            health: DEFAULT_PLAYER_MAX_HEALTH,
            pending_death_cause: None,
        }
    }
}

/// Persistent generation and environment facts for one dimension instance.
///
/// The first record vocabulary intentionally describes the current Overworld
/// exactly. Slice 3 adds its durable codec and registry migration; Slice 4
/// moves the corresponding live scheduler/state into `DimensionRuntime`.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionDefinition {
    pub seed: i64,
    pub generation_profile: WorldGenerationProfile,
    pub topology: HorizontalTopology,
    pub min_y: i32,
    pub height: i32,
    pub coordinate_scale: f64,
    pub has_sky_light: bool,
    pub has_ceiling: bool,
    pub ultrawarm: bool,
}

impl DimensionDefinition {
    pub const fn overworld(seed: i64, generation_profile: WorldGenerationProfile) -> Self {
        Self {
            seed,
            generation_profile,
            topology: HorizontalTopology::UNBOUNDED,
            min_y: 0,
            height: 256,
            coordinate_scale: 1.0,
            has_sky_light: true,
            has_ceiling: false,
            ultrawarm: false,
        }
    }
}

impl Default for DimensionDefinition {
    fn default() -> Self {
        Self::overworld(0, WorldGenerationProfile::default())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DimensionRecord {
    pub key: DimensionKey,
    pub codec_version: u32,
    pub revision: u64,
    pub definition: DimensionDefinition,
}

impl DimensionRecord {
    pub fn overworld(seed: i64, generation_profile: WorldGenerationProfile) -> Self {
        Self {
            key: DimensionKey::overworld(),
            codec_version: DIMENSION_RECORD_VERSION,
            revision: 1,
            definition: DimensionDefinition::overworld(seed, generation_profile),
        }
    }
}

impl Default for DimensionRecord {
    fn default() -> Self {
        Self::overworld(0, WorldGenerationProfile::default())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldMetadata {
    pub codec_version: u32,
    pub realm_id: RealmId,
    pub revision: u64,
    pub target_minecraft_version: String,
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub world_behavior_profile: WorldBehaviorProfile,
    pub created_unix_millis: u64,
    pub last_played_unix_millis: u64,
    pub game_time: u64,
    pub day_time: u64,
    pub do_daylight_cycle: bool,
}

impl WorldMetadata {
    pub fn new(
        seed: i64,
        world_generation_profile: WorldGenerationProfile,
        world_behavior_profile: WorldBehaviorProfile,
        now_unix_millis: u64,
    ) -> Self {
        Self::new_in_realm(
            RealmId::LEGACY_SINGLE_REALM,
            seed,
            world_generation_profile,
            world_behavior_profile,
            now_unix_millis,
        )
    }

    pub fn new_in_realm(
        realm_id: RealmId,
        seed: i64,
        world_generation_profile: WorldGenerationProfile,
        world_behavior_profile: WorldBehaviorProfile,
        now_unix_millis: u64,
    ) -> Self {
        Self {
            codec_version: WORLD_METADATA_VERSION,
            realm_id,
            revision: 1,
            target_minecraft_version: WORLD_METADATA_TARGET_MINECRAFT_VERSION.to_owned(),
            seed,
            world_generation_profile,
            world_behavior_profile,
            created_unix_millis: now_unix_millis,
            last_played_unix_millis: now_unix_millis,
            game_time: 0,
            day_time: 0,
            do_daylight_cycle: true,
        }
    }

    pub fn legacy_mclone(
        seed: i64,
        world_generation_profile: WorldGenerationProfile,
        world_behavior_profile: WorldBehaviorProfile,
        now_unix_millis: u64,
        legacy_day_time: u64,
    ) -> Self {
        let mut record = Self::new(
            seed,
            world_generation_profile,
            world_behavior_profile,
            now_unix_millis,
        );
        record.day_time = legacy_day_time;
        record
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorldMetadataLoad {
    pub record: Option<WorldMetadata>,
    pub legacy_records_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum WorldRecordKey {
    WorldMetadata,
    Dimension(DimensionKey),
    Chunk(DimensionChunkPos),
    EntityChunk(DimensionChunkPos),
    Player(PlayerRecordKey),
    SavedData(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldStoreRequest {
    LoadWorldMetadata {
        request_id: PersistenceRequestId,
    },
    SaveWorldMetadata {
        request_id: PersistenceRequestId,
        record: WorldMetadata,
    },
    LoadDimension {
        request_id: PersistenceRequestId,
        key: DimensionKey,
    },
    SaveDimension {
        request_id: PersistenceRequestId,
        record: DimensionRecord,
    },
    LoadChunk {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
    },
    SaveChunk {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        record: ChunkRecord,
        durability: SaveDurability,
    },
    LoadEntityChunk {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
    },
    SaveEntityChunk {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        record: EntityChunkRecord,
        durability: SaveDurability,
    },
    LoadPlayer {
        request_id: PersistenceRequestId,
        player: PlayerRecordKey,
    },
    SavePlayer {
        request_id: PersistenceRequestId,
        record: PlayerRecord,
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

impl WorldStoreRequest {
    pub const fn request_id(&self) -> PersistenceRequestId {
        match self {
            Self::LoadWorldMetadata { request_id }
            | Self::SaveWorldMetadata { request_id, .. }
            | Self::LoadDimension { request_id, .. }
            | Self::SaveDimension { request_id, .. }
            | Self::LoadChunk { request_id, .. }
            | Self::SaveChunk { request_id, .. }
            | Self::LoadEntityChunk { request_id, .. }
            | Self::SaveEntityChunk { request_id, .. }
            | Self::LoadPlayer { request_id, .. }
            | Self::SavePlayer { request_id, .. }
            | Self::LoadSavedData { request_id, .. }
            | Self::Flush { request_id }
            | Self::Close { request_id } => *request_id,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn into_closed_completion(self) -> WorldStoreCompletion {
        match self {
            Self::LoadWorldMetadata { request_id } => WorldStoreCompletion::WorldMetadataLoaded {
                request_id,
                result: Err(closed_error()),
            },
            Self::SaveWorldMetadata { request_id, record } => {
                WorldStoreCompletion::WorldMetadataSaved {
                    request_id,
                    revision: record.revision,
                    result: Err(closed_error()),
                }
            }
            Self::LoadDimension { request_id, key } => WorldStoreCompletion::DimensionLoaded {
                request_id,
                key,
                result: Err(closed_error()),
            },
            Self::SaveDimension { request_id, record } => WorldStoreCompletion::DimensionSaved {
                request_id,
                key: record.key,
                result: Err(closed_error()),
            },
            Self::LoadChunk {
                request_id,
                dimension,
                pos,
            } => WorldStoreCompletion::ChunkLoaded {
                request_id,
                dimension,
                pos,
                result: Err(closed_error()),
            },
            Self::SaveChunk {
                request_id,
                dimension,
                record,
                ..
            } => WorldStoreCompletion::ChunkSaved {
                request_id,
                dimension,
                pos: record.pos(),
                result: Err(closed_error()),
            },
            Self::LoadEntityChunk {
                request_id,
                dimension,
                pos,
            } => WorldStoreCompletion::EntityChunkLoaded {
                request_id,
                dimension,
                pos,
                result: Err(closed_error()),
            },
            Self::SaveEntityChunk {
                request_id,
                dimension,
                record,
                ..
            } => WorldStoreCompletion::EntityChunkSaved {
                request_id,
                dimension,
                pos: record.pos,
                result: Err(closed_error()),
            },
            Self::LoadPlayer { request_id, player } => WorldStoreCompletion::PlayerLoaded {
                request_id,
                player,
                result: Err(closed_error()),
            },
            Self::SavePlayer { request_id, record } => WorldStoreCompletion::PlayerSaved {
                request_id,
                player: record.player,
                result: Err(closed_error()),
            },
            Self::LoadSavedData { request_id, key } => WorldStoreCompletion::RequestFailed {
                request_id,
                key: WorldRecordKey::SavedData(key),
                result: Err(closed_error()),
            },
            Self::Flush { request_id } => WorldStoreCompletion::FlushComplete {
                request_id,
                result: Err(closed_error()),
            },
            Self::Close { request_id } => WorldStoreCompletion::CloseComplete {
                request_id,
                result: Err(closed_error()),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreWriteOutcome {
    Written,
    Superseded,
    SkippedOnClose,
}

#[derive(Debug)]
pub enum WorldStoreCompletion {
    WorldMetadataLoaded {
        request_id: PersistenceRequestId,
        result: ChunkStoreResult<WorldMetadataLoad>,
    },
    WorldMetadataSaved {
        request_id: PersistenceRequestId,
        revision: u64,
        result: ChunkStoreResult<StoreWriteOutcome>,
    },
    DimensionLoaded {
        request_id: PersistenceRequestId,
        key: DimensionKey,
        result: ChunkStoreResult<Option<DimensionRecord>>,
    },
    DimensionSaved {
        request_id: PersistenceRequestId,
        key: DimensionKey,
        result: ChunkStoreResult<StoreWriteOutcome>,
    },
    ChunkLoaded {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<ChunkRecord>>,
    },
    ChunkSaved {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
        result: ChunkStoreResult<StoreWriteOutcome>,
    },
    EntityChunkLoaded {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<EntityChunkRecord>>,
    },
    EntityChunkSaved {
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
        result: ChunkStoreResult<StoreWriteOutcome>,
    },
    PlayerLoaded {
        request_id: PersistenceRequestId,
        player: PlayerRecordKey,
        result: ChunkStoreResult<Option<PlayerRecord>>,
    },
    PlayerSaved {
        request_id: PersistenceRequestId,
        player: PlayerRecordKey,
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
            Self::WorldMetadataLoaded { request_id, .. }
            | Self::WorldMetadataSaved { request_id, .. }
            | Self::DimensionLoaded { request_id, .. }
            | Self::DimensionSaved { request_id, .. }
            | Self::ChunkLoaded { request_id, .. }
            | Self::ChunkSaved { request_id, .. }
            | Self::EntityChunkLoaded { request_id, .. }
            | Self::EntityChunkSaved { request_id, .. }
            | Self::PlayerLoaded { request_id, .. }
            | Self::PlayerSaved { request_id, .. }
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
    fn supports_entity_chunks(&self) -> bool {
        false
    }

    fn load_world_metadata(&mut self) -> ChunkStoreResult<WorldMetadataLoad> {
        Ok(WorldMetadataLoad::default())
    }

    fn save_world_metadata(&mut self, _record: &WorldMetadata) -> ChunkStoreResult<()> {
        Err(ChunkStoreError::InvalidData(
            "world metadata storage is not supported".to_owned(),
        ))
    }

    fn load_dimension(&mut self, _key: &DimensionKey) -> ChunkStoreResult<Option<DimensionRecord>> {
        Ok(None)
    }

    fn save_dimension(&mut self, record: &DimensionRecord) -> ChunkStoreResult<()> {
        Err(ChunkStoreError::InvalidData(format!(
            "dimension storage is not supported for {}",
            record.key
        )))
    }

    fn load_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>>;
    fn save_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &ChunkRecord,
    ) -> ChunkStoreResult<()>;

    fn load_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        Err(ChunkStoreError::InvalidData(format!(
            "entity chunk storage is not supported for {dimension} ({}, {})",
            pos.x, pos.z,
        )))
    }

    fn save_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &EntityChunkRecord,
    ) -> ChunkStoreResult<()> {
        Err(ChunkStoreError::InvalidData(format!(
            "entity chunk storage is not supported for {dimension} ({}, {})",
            record.pos.x, record.pos.z,
        )))
    }

    fn load_player(&mut self, _player: &PlayerRecordKey) -> ChunkStoreResult<Option<PlayerRecord>> {
        Ok(None)
    }

    fn save_player(&mut self, record: &PlayerRecord) -> ChunkStoreResult<()> {
        Err(ChunkStoreError::InvalidData(format!(
            "player storage is not supported for {}",
            record.player.as_str()
        )))
    }

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
    fn save_world_metadata(&mut self, _record: &WorldMetadata) -> ChunkStoreResult<()> {
        Ok(())
    }

    fn save_dimension(&mut self, _record: &DimensionRecord) -> ChunkStoreResult<()> {
        Ok(())
    }

    fn load_chunk(
        &mut self,
        _dimension: &DimensionKey,
        _pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
        Ok(None)
    }

    fn save_chunk(
        &mut self,
        _dimension: &DimensionKey,
        _record: &ChunkRecord,
    ) -> ChunkStoreResult<()> {
        Ok(())
    }

    fn load_entity_chunk(
        &mut self,
        _dimension: &DimensionKey,
        _pos: ChunkPos,
    ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        Ok(None)
    }

    fn save_entity_chunk(
        &mut self,
        _dimension: &DimensionKey,
        _record: &EntityChunkRecord,
    ) -> ChunkStoreResult<()> {
        Ok(())
    }

    fn save_player(&mut self, _record: &PlayerRecord) -> ChunkStoreResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MemoryWorldStore {
    world_metadata: Option<WorldMetadata>,
    dimensions: BTreeMap<DimensionKey, DimensionRecord>,
    chunks: BTreeMap<DimensionChunkPos, ChunkRecord>,
    entity_chunks: BTreeMap<DimensionChunkPos, EntityChunkRecord>,
    players: BTreeMap<PlayerRecordKey, PlayerRecord>,
}

impl MemoryWorldStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&ChunkRecord> {
        self.chunk_in_dimension(&DimensionKey::overworld(), pos)
    }

    pub fn chunk_in_dimension(
        &self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> Option<&ChunkRecord> {
        self.chunks
            .get(&DimensionChunkPos::new(dimension.clone(), pos))
    }

    pub fn entity_chunk(&self, pos: ChunkPos) -> Option<&EntityChunkRecord> {
        self.entity_chunk_in_dimension(&DimensionKey::overworld(), pos)
    }

    pub fn entity_chunk_in_dimension(
        &self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> Option<&EntityChunkRecord> {
        self.entity_chunks
            .get(&DimensionChunkPos::new(dimension.clone(), pos))
    }

    pub fn player(&self, key: &PlayerRecordKey) -> Option<&PlayerRecord> {
        self.players.get(key)
    }

    pub fn world_metadata(&self) -> Option<&WorldMetadata> {
        self.world_metadata.as_ref()
    }

    pub fn dimension(&self, key: &DimensionKey) -> Option<&DimensionRecord> {
        self.dimensions.get(key)
    }
}

impl WorldStore for MemoryWorldStore {
    fn load_world_metadata(&mut self) -> ChunkStoreResult<WorldMetadataLoad> {
        Ok(WorldMetadataLoad {
            record: self.world_metadata.clone(),
            legacy_records_present: !self.chunks.is_empty()
                || !self.entity_chunks.is_empty()
                || !self.players.is_empty()
                || !self.dimensions.is_empty(),
        })
    }

    fn save_world_metadata(&mut self, record: &WorldMetadata) -> ChunkStoreResult<()> {
        if self
            .world_metadata
            .as_ref()
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.world_metadata = Some(record.clone());
        Ok(())
    }

    fn load_dimension(&mut self, key: &DimensionKey) -> ChunkStoreResult<Option<DimensionRecord>> {
        Ok(self.dimensions.get(key).cloned())
    }

    fn save_dimension(&mut self, record: &DimensionRecord) -> ChunkStoreResult<()> {
        if self
            .dimensions
            .get(&record.key)
            .is_some_and(|stored| stored.revision > record.revision)
        {
            return Ok(());
        }
        self.dimensions.insert(record.key.clone(), record.clone());
        Ok(())
    }

    fn supports_entity_chunks(&self) -> bool {
        true
    }

    fn load_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
        Ok(self
            .chunks
            .get(&DimensionChunkPos::new(dimension.clone(), pos))
            .cloned())
    }

    fn save_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &ChunkRecord,
    ) -> ChunkStoreResult<()> {
        self.chunks.insert(
            DimensionChunkPos::new(dimension.clone(), record.pos()),
            record.clone(),
        );
        Ok(())
    }

    fn load_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        Ok(self
            .entity_chunks
            .get(&DimensionChunkPos::new(dimension.clone(), pos))
            .cloned())
    }

    fn save_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &EntityChunkRecord,
    ) -> ChunkStoreResult<()> {
        self.entity_chunks.insert(
            DimensionChunkPos::new(dimension.clone(), record.pos),
            record.clone(),
        );
        Ok(())
    }

    fn load_player(&mut self, player: &PlayerRecordKey) -> ChunkStoreResult<Option<PlayerRecord>> {
        Ok(self.players.get(player).cloned())
    }

    fn save_player(&mut self, record: &PlayerRecord) -> ChunkStoreResult<()> {
        self.players.insert(record.player.clone(), record.clone());
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
    fn load_chunk(
        &mut self,
        _dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
        self.store
            .load_chunk(pos)
            .map(|snapshot| snapshot.map(ChunkRecord::legacy_snapshot))
    }

    fn save_chunk(
        &mut self,
        _dimension: &DimensionKey,
        record: &ChunkRecord,
    ) -> ChunkStoreResult<()> {
        self.store.save_chunk(&record.snapshot)
    }
}

#[derive(Clone, Debug)]
struct PendingChunkWrite {
    request_id: PersistenceRequestId,
    dimension: DimensionKey,
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

#[derive(Clone, Debug)]
struct PendingEntityChunkWrite {
    request_id: PersistenceRequestId,
    dimension: DimensionKey,
    record: EntityChunkRecord,
    durability: SaveDurability,
}

impl PendingEntityChunkWrite {
    fn should_replace(&self, existing: &Self) -> bool {
        if self.record.revision != existing.record.revision {
            return self.record.revision > existing.record.revision;
        }
        match (self.durability, existing.durability) {
            (SaveDurability::Durable, SaveDurability::Cache) => true,
            (SaveDurability::Cache, SaveDurability::Durable) => false,
            _ => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingWriteKey {
    Chunk(DimensionChunkPos),
    EntityChunk(DimensionChunkPos),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExternalLoadKey {
    Chunk(DimensionChunkPos),
    EntityChunk(DimensionChunkPos),
    Player(PlayerRecordKey),
}

#[derive(Debug)]
pub struct PersistenceActor {
    store: Box<dyn WorldStore>,
    pending_chunk_writes: BTreeMap<DimensionChunkPos, PendingChunkWrite>,
    pending_entity_chunk_writes: BTreeMap<DimensionChunkPos, PendingEntityChunkWrite>,
    completions: VecDeque<WorldStoreCompletion>,
    closed: bool,
}

impl PersistenceActor {
    pub fn new(store: Box<dyn WorldStore>) -> Self {
        Self {
            store,
            pending_chunk_writes: BTreeMap::new(),
            pending_entity_chunk_writes: BTreeMap::new(),
            completions: VecDeque::new(),
            closed: false,
        }
    }

    pub fn entity_chunks_supported(&self) -> bool {
        self.store.supports_entity_chunks()
    }

    pub fn load_world_metadata(&mut self, request_id: PersistenceRequestId) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store.load_world_metadata()
        };
        self.completions
            .push_back(WorldStoreCompletion::WorldMetadataLoaded { request_id, result });
    }

    pub fn save_world_metadata(&mut self, request_id: PersistenceRequestId, record: WorldMetadata) {
        let revision = record.revision;
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store
                .save_world_metadata(&record)
                .map(|_| StoreWriteOutcome::Written)
        };
        self.completions
            .push_back(WorldStoreCompletion::WorldMetadataSaved {
                request_id,
                revision,
                result,
            });
    }

    pub fn load_dimension(&mut self, request_id: PersistenceRequestId, key: DimensionKey) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store.load_dimension(&key)
        };
        self.completions
            .push_back(WorldStoreCompletion::DimensionLoaded {
                request_id,
                key,
                result,
            });
    }

    pub fn save_dimension(&mut self, request_id: PersistenceRequestId, record: DimensionRecord) {
        let key = record.key.clone();
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store
                .save_dimension(&record)
                .map(|_| StoreWriteOutcome::Written)
        };
        self.completions
            .push_back(WorldStoreCompletion::DimensionSaved {
                request_id,
                key,
                result,
            });
    }

    pub fn load_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
    ) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::ChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let address = DimensionChunkPos::new(dimension.clone(), pos);
        let result = if let Some(pending) = self.pending_chunk_writes.get(&address) {
            Ok(Some(pending.record.clone()))
        } else {
            self.store.load_chunk(&dimension, pos)
        };
        self.completions
            .push_back(WorldStoreCompletion::ChunkLoaded {
                request_id,
                dimension,
                pos,
                result,
            });
    }

    pub fn save_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        record: ChunkRecord,
        durability: SaveDurability,
    ) {
        let pos = record.pos();
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::ChunkSaved {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let incoming = PendingChunkWrite {
            request_id,
            dimension: dimension.clone(),
            record,
            durability,
        };
        let address = DimensionChunkPos::new(dimension.clone(), pos);
        if let Some(existing) = self.pending_chunk_writes.get_mut(&address) {
            if incoming.should_replace(existing) {
                let superseded_id = existing.request_id;
                *existing = incoming;
                self.completions
                    .push_back(WorldStoreCompletion::ChunkSaved {
                        request_id: superseded_id,
                        dimension: dimension.clone(),
                        pos,
                        result: Ok(StoreWriteOutcome::Superseded),
                    });
            } else {
                self.completions
                    .push_back(WorldStoreCompletion::ChunkSaved {
                        request_id,
                        dimension,
                        pos,
                        result: Ok(StoreWriteOutcome::Superseded),
                    });
            }
            return;
        }

        self.pending_chunk_writes.insert(address, incoming);
    }

    pub fn load_entity_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
    ) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let address = DimensionChunkPos::new(dimension.clone(), pos);
        let result = if let Some(pending) = self.pending_entity_chunk_writes.get(&address) {
            Ok(Some(pending.record.clone()))
        } else {
            self.store.load_entity_chunk(&dimension, pos)
        };
        self.completions
            .push_back(WorldStoreCompletion::EntityChunkLoaded {
                request_id,
                dimension,
                pos,
                result,
            });
    }

    pub fn save_entity_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        record: EntityChunkRecord,
        durability: SaveDurability,
    ) {
        let pos = record.pos;
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkSaved {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let incoming = PendingEntityChunkWrite {
            request_id,
            dimension: dimension.clone(),
            record,
            durability,
        };
        let address = DimensionChunkPos::new(dimension.clone(), pos);
        if let Some(existing) = self.pending_entity_chunk_writes.get_mut(&address) {
            if incoming.should_replace(existing) {
                let superseded_id = existing.request_id;
                *existing = incoming;
                self.completions
                    .push_back(WorldStoreCompletion::EntityChunkSaved {
                        request_id: superseded_id,
                        dimension: dimension.clone(),
                        pos,
                        result: Ok(StoreWriteOutcome::Superseded),
                    });
            } else {
                self.completions
                    .push_back(WorldStoreCompletion::EntityChunkSaved {
                        request_id,
                        dimension,
                        pos,
                        result: Ok(StoreWriteOutcome::Superseded),
                    });
            }
            return;
        }

        self.pending_entity_chunk_writes.insert(address, incoming);
    }

    pub fn load_player(&mut self, request_id: PersistenceRequestId, player: PlayerRecordKey) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store.load_player(&player)
        };
        self.completions
            .push_back(WorldStoreCompletion::PlayerLoaded {
                request_id,
                player,
                result,
            });
    }

    pub fn save_player(&mut self, request_id: PersistenceRequestId, record: PlayerRecord) {
        let player = record.player.clone();
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store
                .save_player(&record)
                .map(|_| StoreWriteOutcome::Written)
        };
        self.completions
            .push_back(WorldStoreCompletion::PlayerSaved {
                request_id,
                player,
                result,
            });
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
            self.process_matching_pending_chunk_writes(|pending| pending.durability.is_durable());
        if result.is_ok() {
            result = self.process_matching_pending_entity_chunk_writes(|pending| {
                pending.durability.is_durable()
            });
        }
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
            self.process_matching_pending_chunk_writes(|pending| pending.durability.is_durable());
        if result.is_ok() {
            result = self.process_matching_pending_entity_chunk_writes(|pending| {
                pending.durability.is_durable()
            });
        }
        for address in self
            .pending_chunk_writes
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(pending) = self.pending_chunk_writes.remove(&address) {
                self.completions
                    .push_back(WorldStoreCompletion::ChunkSaved {
                        request_id: pending.request_id,
                        dimension: pending.dimension,
                        pos: address.pos,
                        result: Ok(StoreWriteOutcome::SkippedOnClose),
                    });
            }
        }
        for address in self
            .pending_entity_chunk_writes
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(pending) = self.pending_entity_chunk_writes.remove(&address) {
                self.completions
                    .push_back(WorldStoreCompletion::EntityChunkSaved {
                        request_id: pending.request_id,
                        dimension: pending.dimension,
                        pos: address.pos,
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
        let Some(key) = self.next_pending_write_key(true) else {
            return false;
        };
        match key {
            PendingWriteKey::Chunk(address) => {
                let _ = self.process_pending_chunk_write_at(address);
            }
            PendingWriteKey::EntityChunk(address) => {
                let _ = self.process_pending_entity_chunk_write_at(address);
            }
        }
        true
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn has_pending_background_write(&self) -> bool {
        self.next_pending_write_key(true).is_some()
    }

    pub fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        self.completions.drain(..).collect()
    }

    fn process_matching_pending_chunk_writes(
        &mut self,
        mut predicate: impl FnMut(&PendingChunkWrite) -> bool,
    ) -> ChunkStoreResult<()> {
        let addresses = self
            .pending_chunk_writes
            .iter()
            .filter_map(|(address, pending)| predicate(pending).then_some(address.clone()))
            .collect::<Vec<_>>();
        let mut result = Ok(());
        for address in addresses {
            if self.pending_chunk_writes.contains_key(&address) {
                let write_result = self.process_pending_chunk_write_at(address);
                if result.is_ok() {
                    result = write_result;
                }
            }
        }
        result
    }

    fn process_matching_pending_entity_chunk_writes(
        &mut self,
        mut predicate: impl FnMut(&PendingEntityChunkWrite) -> bool,
    ) -> ChunkStoreResult<()> {
        let addresses = self
            .pending_entity_chunk_writes
            .iter()
            .filter_map(|(address, pending)| predicate(pending).then_some(address.clone()))
            .collect::<Vec<_>>();
        let mut result = Ok(());
        for address in addresses {
            if self.pending_entity_chunk_writes.contains_key(&address) {
                let write_result = self.process_pending_entity_chunk_write_at(address);
                if result.is_ok() {
                    result = write_result;
                }
            }
        }
        result
    }

    fn next_pending_write_key(&self, include_cache: bool) -> Option<PendingWriteKey> {
        if let Some(address) = self
            .pending_chunk_writes
            .iter()
            .find_map(|(address, pending)| {
                pending.durability.is_durable().then_some(address.clone())
            })
        {
            return Some(PendingWriteKey::Chunk(address));
        }
        if let Some(address) =
            self.pending_entity_chunk_writes
                .iter()
                .find_map(|(address, pending)| {
                    pending.durability.is_durable().then_some(address.clone())
                })
        {
            return Some(PendingWriteKey::EntityChunk(address));
        }
        include_cache.then_some(())?;
        self.pending_chunk_writes
            .keys()
            .next()
            .cloned()
            .map(PendingWriteKey::Chunk)
            .or_else(|| {
                self.pending_entity_chunk_writes
                    .keys()
                    .next()
                    .cloned()
                    .map(PendingWriteKey::EntityChunk)
            })
    }

    fn process_pending_chunk_write_at(
        &mut self,
        address: DimensionChunkPos,
    ) -> ChunkStoreResult<()> {
        let Some(pending) = self.pending_chunk_writes.remove(&address) else {
            return Ok(());
        };
        let result = self.store.save_chunk(&pending.dimension, &pending.record);
        let barrier_result = result.as_ref().map(|_| ()).map_err(duplicate_store_error);
        self.completions
            .push_back(WorldStoreCompletion::ChunkSaved {
                request_id: pending.request_id,
                dimension: pending.dimension,
                pos: address.pos,
                result: result.map(|_| StoreWriteOutcome::Written),
            });
        barrier_result
    }

    fn process_pending_entity_chunk_write_at(
        &mut self,
        address: DimensionChunkPos,
    ) -> ChunkStoreResult<()> {
        let Some(pending) = self.pending_entity_chunk_writes.remove(&address) else {
            return Ok(());
        };
        let result = self
            .store
            .save_entity_chunk(&pending.dimension, &pending.record);
        let barrier_result = result.as_ref().map(|_| ()).map_err(duplicate_store_error);
        self.completions
            .push_back(WorldStoreCompletion::EntityChunkSaved {
                request_id: pending.request_id,
                dimension: pending.dimension,
                pos: address.pos,
                result: result.map(|_| StoreWriteOutcome::Written),
            });
        barrier_result
    }
}

#[derive(Debug)]
struct ExternalLoadPersistenceActor {
    store: Box<dyn WorldStore>,
    cached_chunk_records: BTreeMap<DimensionChunkPos, PendingChunkWrite>,
    cached_entity_chunk_records: BTreeMap<DimensionChunkPos, PendingEntityChunkWrite>,
    pending_external_loads: BTreeMap<PersistenceRequestId, ExternalLoadKey>,
    external_requests: VecDeque<WorldStoreRequest>,
    completions: VecDeque<WorldStoreCompletion>,
    entity_chunks_supported: bool,
    closed: bool,
}

impl ExternalLoadPersistenceActor {
    fn new(store: Box<dyn WorldStore>) -> Self {
        let entity_chunks_supported = store.supports_entity_chunks();
        Self {
            store,
            cached_chunk_records: BTreeMap::new(),
            cached_entity_chunk_records: BTreeMap::new(),
            pending_external_loads: BTreeMap::new(),
            external_requests: VecDeque::new(),
            completions: VecDeque::new(),
            entity_chunks_supported,
            closed: false,
        }
    }

    const fn entity_chunks_supported(&self) -> bool {
        self.entity_chunks_supported
    }

    fn send_request(&mut self, request: WorldStoreRequest) {
        match request {
            WorldStoreRequest::LoadWorldMetadata { request_id } => {
                self.load_world_metadata(request_id);
            }
            WorldStoreRequest::SaveWorldMetadata { request_id, record } => {
                self.save_world_metadata(request_id, record);
            }
            WorldStoreRequest::LoadDimension { request_id, key } => {
                self.load_dimension(request_id, key);
            }
            WorldStoreRequest::SaveDimension { request_id, record } => {
                self.save_dimension(request_id, record);
            }
            WorldStoreRequest::LoadChunk {
                request_id,
                dimension,
                pos,
            } => {
                self.load_chunk(request_id, dimension, pos);
            }
            WorldStoreRequest::SaveChunk {
                request_id,
                dimension,
                record,
                durability,
            } => {
                self.save_chunk(request_id, dimension, record, durability);
            }
            WorldStoreRequest::LoadEntityChunk {
                request_id,
                dimension,
                pos,
            } => {
                self.load_entity_chunk(request_id, dimension, pos);
            }
            WorldStoreRequest::SaveEntityChunk {
                request_id,
                dimension,
                record,
                durability,
            } => {
                self.save_entity_chunk(request_id, dimension, record, durability);
            }
            WorldStoreRequest::LoadPlayer { request_id, player } => {
                self.load_player(request_id, player);
            }
            WorldStoreRequest::SavePlayer { request_id, record } => {
                self.save_player(request_id, record);
            }
            WorldStoreRequest::LoadSavedData { request_id, key } => {
                self.fail_reserved_request(request_id, WorldRecordKey::SavedData(key));
            }
            WorldStoreRequest::Flush { request_id } => {
                self.flush(request_id);
            }
            WorldStoreRequest::Close { request_id } => {
                self.close(request_id);
            }
        }
    }

    fn load_world_metadata(&mut self, request_id: PersistenceRequestId) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store.load_world_metadata()
        };
        self.completions
            .push_back(WorldStoreCompletion::WorldMetadataLoaded { request_id, result });
    }

    fn save_world_metadata(&mut self, request_id: PersistenceRequestId, record: WorldMetadata) {
        let revision = record.revision;
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store
                .save_world_metadata(&record)
                .map(|_| StoreWriteOutcome::Written)
        };
        self.completions
            .push_back(WorldStoreCompletion::WorldMetadataSaved {
                request_id,
                revision,
                result,
            });
    }

    fn load_dimension(&mut self, request_id: PersistenceRequestId, key: DimensionKey) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store.load_dimension(&key)
        };
        self.completions
            .push_back(WorldStoreCompletion::DimensionLoaded {
                request_id,
                key,
                result,
            });
    }

    fn save_dimension(&mut self, request_id: PersistenceRequestId, record: DimensionRecord) {
        let key = record.key.clone();
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store
                .save_dimension(&record)
                .map(|_| StoreWriteOutcome::Written)
        };
        self.completions
            .push_back(WorldStoreCompletion::DimensionSaved {
                request_id,
                key,
                result,
            });
    }

    fn load_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
    ) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::ChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let address = DimensionChunkPos::new(dimension.clone(), pos);
        if let Some(cached) = self.cached_chunk_records.get(&address) {
            self.completions
                .push_back(WorldStoreCompletion::ChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Ok(Some(cached.record.clone())),
                });
            return;
        }

        match self.store.load_chunk(&dimension, pos) {
            Ok(Some(record)) => {
                self.cached_chunk_records.insert(
                    address,
                    PendingChunkWrite {
                        request_id: 0,
                        dimension: dimension.clone(),
                        record: record.clone(),
                        durability: SaveDurability::Durable,
                    },
                );
                self.completions
                    .push_back(WorldStoreCompletion::ChunkLoaded {
                        request_id,
                        dimension,
                        pos,
                        result: Ok(Some(record)),
                    });
            }
            Ok(None) => {
                self.pending_external_loads.insert(
                    request_id,
                    ExternalLoadKey::Chunk(DimensionChunkPos::new(dimension.clone(), pos)),
                );
                self.external_requests
                    .push_back(WorldStoreRequest::LoadChunk {
                        request_id,
                        dimension,
                        pos,
                    });
            }
            Err(error) => {
                self.completions
                    .push_back(WorldStoreCompletion::ChunkLoaded {
                        request_id,
                        dimension,
                        pos,
                        result: Err(error),
                    });
            }
        }
    }

    fn save_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        record: ChunkRecord,
        durability: SaveDurability,
    ) {
        let pos = record.pos();
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::ChunkSaved {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let incoming = PendingChunkWrite {
            request_id,
            dimension: dimension.clone(),
            record,
            durability,
        };
        let address = DimensionChunkPos::new(dimension.clone(), pos);
        if self
            .cached_chunk_records
            .get(&address)
            .is_some_and(|existing| !incoming.should_replace(existing))
        {
            self.completions
                .push_back(WorldStoreCompletion::ChunkSaved {
                    request_id,
                    dimension,
                    pos,
                    result: Ok(StoreWriteOutcome::Superseded),
                });
            return;
        }

        let result = self.store.save_chunk(&dimension, &incoming.record);
        if result.is_ok() {
            self.cached_chunk_records.insert(address, incoming);
        }
        self.completions
            .push_back(WorldStoreCompletion::ChunkSaved {
                request_id,
                dimension,
                pos,
                result: result.map(|_| StoreWriteOutcome::Written),
            });
    }

    fn load_entity_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
    ) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        if !self.entity_chunks_supported {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Err(ChunkStoreError::InvalidData(format!(
                        "entity chunk storage is not supported for ({}, {})",
                        pos.x, pos.z
                    ))),
                });
            return;
        }

        let address = DimensionChunkPos::new(dimension.clone(), pos);
        if let Some(cached) = self.cached_entity_chunk_records.get(&address) {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkLoaded {
                    request_id,
                    dimension,
                    pos,
                    result: Ok(Some(cached.record.clone())),
                });
            return;
        }

        match self.store.load_entity_chunk(&dimension, pos) {
            Ok(Some(record)) => {
                self.cached_entity_chunk_records.insert(
                    address,
                    PendingEntityChunkWrite {
                        request_id: 0,
                        dimension: dimension.clone(),
                        record: record.clone(),
                        durability: SaveDurability::Durable,
                    },
                );
                self.completions
                    .push_back(WorldStoreCompletion::EntityChunkLoaded {
                        request_id,
                        dimension,
                        pos,
                        result: Ok(Some(record)),
                    });
            }
            Ok(None) => {
                self.pending_external_loads.insert(
                    request_id,
                    ExternalLoadKey::EntityChunk(DimensionChunkPos::new(dimension.clone(), pos)),
                );
                self.external_requests
                    .push_back(WorldStoreRequest::LoadEntityChunk {
                        request_id,
                        dimension,
                        pos,
                    });
            }
            Err(error) => {
                self.completions
                    .push_back(WorldStoreCompletion::EntityChunkLoaded {
                        request_id,
                        dimension,
                        pos,
                        result: Err(error),
                    });
            }
        }
    }

    fn save_entity_chunk(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        record: EntityChunkRecord,
        durability: SaveDurability,
    ) {
        let pos = record.pos;
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkSaved {
                    request_id,
                    dimension,
                    pos,
                    result: Err(closed_error()),
                });
            return;
        }

        let incoming = PendingEntityChunkWrite {
            request_id,
            dimension: dimension.clone(),
            record,
            durability,
        };
        let address = DimensionChunkPos::new(dimension.clone(), pos);
        if self
            .cached_entity_chunk_records
            .get(&address)
            .is_some_and(|existing| !incoming.should_replace(existing))
        {
            self.completions
                .push_back(WorldStoreCompletion::EntityChunkSaved {
                    request_id,
                    dimension,
                    pos,
                    result: Ok(StoreWriteOutcome::Superseded),
                });
            return;
        }

        let result = self.store.save_entity_chunk(&dimension, &incoming.record);
        if result.is_ok() {
            self.cached_entity_chunk_records.insert(address, incoming);
        }
        self.completions
            .push_back(WorldStoreCompletion::EntityChunkSaved {
                request_id,
                dimension,
                pos,
                result: result.map(|_| StoreWriteOutcome::Written),
            });
    }

    fn load_player(&mut self, request_id: PersistenceRequestId, player: PlayerRecordKey) {
        if self.closed {
            self.completions
                .push_back(WorldStoreCompletion::PlayerLoaded {
                    request_id,
                    player,
                    result: Err(closed_error()),
                });
            return;
        }
        match self.store.load_player(&player) {
            Ok(Some(record)) => self
                .completions
                .push_back(WorldStoreCompletion::PlayerLoaded {
                    request_id,
                    player,
                    result: Ok(Some(record)),
                }),
            Ok(None) => {
                self.pending_external_loads
                    .insert(request_id, ExternalLoadKey::Player(player.clone()));
                self.external_requests
                    .push_back(WorldStoreRequest::LoadPlayer { request_id, player });
            }
            Err(error) => self
                .completions
                .push_back(WorldStoreCompletion::PlayerLoaded {
                    request_id,
                    player,
                    result: Err(error),
                }),
        }
    }

    fn save_player(&mut self, request_id: PersistenceRequestId, record: PlayerRecord) {
        let player = record.player.clone();
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store
                .save_player(&record)
                .map(|_| StoreWriteOutcome::Written)
        };
        self.completions
            .push_back(WorldStoreCompletion::PlayerSaved {
                request_id,
                player,
                result,
            });
    }

    fn fail_reserved_request(&mut self, request_id: PersistenceRequestId, key: WorldRecordKey) {
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

    fn flush(&mut self, request_id: PersistenceRequestId) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.store.flush()
        };
        self.completions
            .push_back(WorldStoreCompletion::FlushComplete { request_id, result });
    }

    fn close(&mut self, request_id: PersistenceRequestId) {
        let result = if self.closed {
            Err(closed_error())
        } else {
            self.closed = true;
            self.store.close()
        };
        self.completions
            .push_back(WorldStoreCompletion::CloseComplete { request_id, result });
    }

    fn process_one_background_write(&mut self) -> bool {
        false
    }

    fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        self.completions.drain(..).collect()
    }

    fn drain_external_requests(&mut self) -> Vec<WorldStoreRequest> {
        self.external_requests.drain(..).collect()
    }

    fn pending_external_request_count(&self) -> usize {
        self.pending_external_loads.len()
    }

    fn complete_external_request(
        &mut self,
        completion: WorldStoreCompletion,
    ) -> ChunkStoreResult<()> {
        match completion {
            WorldStoreCompletion::WorldMetadataLoaded { .. }
            | WorldStoreCompletion::WorldMetadataSaved { .. } => Err(ChunkStoreError::InvalidData(
                "world metadata is preloaded by the external persistence adapter".to_owned(),
            )),
            WorldStoreCompletion::ChunkLoaded {
                request_id,
                dimension,
                pos,
                result,
            } => self.complete_external_chunk_load(request_id, dimension, pos, result),
            WorldStoreCompletion::EntityChunkLoaded {
                request_id,
                dimension,
                pos,
                result,
            } => self.complete_external_entity_chunk_load(request_id, dimension, pos, result),
            WorldStoreCompletion::PlayerLoaded {
                request_id,
                player,
                result,
            } => self.complete_external_player_load(request_id, player, result),
            other => Err(ChunkStoreError::InvalidData(format!(
                "external persistence backend only accepts load completions, got {other:?}"
            ))),
        }
    }

    fn complete_external_chunk_load(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<ChunkRecord>>,
    ) -> ChunkStoreResult<()> {
        match self.pending_external_loads.remove(&request_id) {
            Some(ExternalLoadKey::Chunk(expected))
                if expected.dimension == dimension && expected.pos == pos => {}
            Some(other) => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external chunk load request {request_id} completed for ({}, {}) but was pending for {other:?}",
                    pos.x, pos.z
                )));
            }
            None => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external chunk load request {request_id} was not pending"
                )));
            }
        }

        let result = result.and_then(|record| {
            if let Some(record) = &record {
                if record.pos() != pos {
                    return Err(ChunkStoreError::InvalidData(format!(
                        "external chunk load request {request_id} returned ({}, {}) for requested ({}, {})",
                        record.pos().x,
                        record.pos().z,
                        pos.x,
                        pos.z
                    )));
                }
                self.cached_chunk_records.insert(
                    DimensionChunkPos::new(dimension.clone(), pos),
                    PendingChunkWrite {
                        request_id: 0,
                        dimension: dimension.clone(),
                        record: record.clone(),
                        durability: SaveDurability::Durable,
                    },
                );
            }
            Ok(record)
        });
        self.completions
            .push_back(WorldStoreCompletion::ChunkLoaded {
                request_id,
                dimension,
                pos,
                result,
            });
        Ok(())
    }

    fn complete_external_entity_chunk_load(
        &mut self,
        request_id: PersistenceRequestId,
        dimension: DimensionKey,
        pos: ChunkPos,
        result: ChunkStoreResult<Option<EntityChunkRecord>>,
    ) -> ChunkStoreResult<()> {
        match self.pending_external_loads.remove(&request_id) {
            Some(ExternalLoadKey::EntityChunk(expected))
                if expected.dimension == dimension && expected.pos == pos => {}
            Some(other) => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external entity chunk load request {request_id} completed for ({}, {}) but was pending for {other:?}",
                    pos.x, pos.z
                )));
            }
            None => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external entity chunk load request {request_id} was not pending"
                )));
            }
        }

        let result = result.and_then(|record| {
            if let Some(record) = &record {
                if record.pos != pos {
                    return Err(ChunkStoreError::InvalidData(format!(
                        "external entity chunk load request {request_id} returned ({}, {}) for requested ({}, {})",
                        record.pos.x, record.pos.z, pos.x, pos.z
                    )));
                }
                self.cached_entity_chunk_records.insert(
                    DimensionChunkPos::new(dimension.clone(), pos),
                    PendingEntityChunkWrite {
                        request_id: 0,
                        dimension: dimension.clone(),
                        record: record.clone(),
                        durability: SaveDurability::Durable,
                    },
                );
            }
            Ok(record)
        });
        self.completions
            .push_back(WorldStoreCompletion::EntityChunkLoaded {
                request_id,
                dimension,
                pos,
                result,
            });
        Ok(())
    }

    fn complete_external_player_load(
        &mut self,
        request_id: PersistenceRequestId,
        player: PlayerRecordKey,
        result: ChunkStoreResult<Option<PlayerRecord>>,
    ) -> ChunkStoreResult<()> {
        match self.pending_external_loads.remove(&request_id) {
            Some(ExternalLoadKey::Player(expected)) if expected == player => {}
            Some(other) => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external player load request {request_id} completed for {player:?} but was pending for {other:?}"
                )));
            }
            None => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external player load request {request_id} was not pending"
                )));
            }
        }
        let result = result.and_then(|record| {
            if record
                .as_ref()
                .is_some_and(|record| record.player != player)
            {
                return Err(ChunkStoreError::InvalidData(format!(
                    "external player load request {request_id} returned a mismatched key"
                )));
            }
            Ok(record)
        });
        self.completions
            .push_back(WorldStoreCompletion::PlayerLoaded {
                request_id,
                player,
                result,
            });
        Ok(())
    }
}

fn handle_world_store_request(actor: &mut PersistenceActor, request: WorldStoreRequest) {
    match request {
        WorldStoreRequest::LoadWorldMetadata { request_id } => {
            actor.load_world_metadata(request_id);
        }
        WorldStoreRequest::SaveWorldMetadata { request_id, record } => {
            actor.save_world_metadata(request_id, record);
        }
        WorldStoreRequest::LoadDimension { request_id, key } => {
            actor.load_dimension(request_id, key);
        }
        WorldStoreRequest::SaveDimension { request_id, record } => {
            actor.save_dimension(request_id, record);
        }
        WorldStoreRequest::LoadChunk {
            request_id,
            dimension,
            pos,
        } => {
            actor.load_chunk(request_id, dimension, pos);
        }
        WorldStoreRequest::SaveChunk {
            request_id,
            dimension,
            record,
            durability,
        } => {
            actor.save_chunk(request_id, dimension, record, durability);
        }
        WorldStoreRequest::LoadEntityChunk {
            request_id,
            dimension,
            pos,
        } => {
            actor.load_entity_chunk(request_id, dimension, pos);
        }
        WorldStoreRequest::SaveEntityChunk {
            request_id,
            dimension,
            record,
            durability,
        } => {
            actor.save_entity_chunk(request_id, dimension, record, durability);
        }
        WorldStoreRequest::LoadPlayer { request_id, player } => {
            actor.load_player(request_id, player);
        }
        WorldStoreRequest::SavePlayer { request_id, record } => {
            actor.save_player(request_id, record);
        }
        WorldStoreRequest::LoadSavedData { request_id, key } => {
            actor.fail_reserved_request(request_id, WorldRecordKey::SavedData(key));
        }
        WorldStoreRequest::Flush { request_id } => {
            actor.flush(request_id);
        }
        WorldStoreRequest::Close { request_id } => {
            actor.close(request_id);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct ThreadedPersistenceActor {
    sender: Option<mpsc::Sender<WorldStoreRequest>>,
    completion_receiver: mpsc::Receiver<WorldStoreCompletion>,
    completion_buffer: VecDeque<WorldStoreCompletion>,
    worker: Option<JoinHandle<()>>,
    entity_chunks_supported: bool,
    pending_requests: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Debug for ThreadedPersistenceActor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadedPersistenceActor")
            .field("entity_chunks_supported", &self.entity_chunks_supported)
            .field("pending_requests", &self.pending_requests)
            .field("completion_buffer_len", &self.completion_buffer.len())
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ThreadedPersistenceActor {
    fn new(store: Box<dyn WorldStore + Send>) -> ChunkStoreResult<Self> {
        let entity_chunks_supported = store.supports_entity_chunks();
        let (sender, receiver) = mpsc::channel::<WorldStoreRequest>();
        let (completion_sender, completion_receiver) = mpsc::channel::<WorldStoreCompletion>();
        let worker = thread::Builder::new()
            .name("mclone-persistence".to_owned())
            .spawn(move || {
                let store: Box<dyn WorldStore> = store;
                let mut actor = PersistenceActor::new(store);
                threaded_persistence_actor_loop(receiver, completion_sender, &mut actor);
            })?;

        Ok(Self {
            sender: Some(sender),
            completion_receiver,
            completion_buffer: VecDeque::new(),
            worker: Some(worker),
            entity_chunks_supported,
            pending_requests: 0,
        })
    }

    const fn entity_chunks_supported(&self) -> bool {
        self.entity_chunks_supported
    }

    fn send_request(&mut self, request: WorldStoreRequest) {
        let Some(sender) = &self.sender else {
            self.completion_buffer
                .push_back(request.into_closed_completion());
            return;
        };
        match sender.send(request) {
            Ok(()) => {
                self.pending_requests = self.pending_requests.saturating_add(1);
            }
            Err(error) => {
                self.completion_buffer
                    .push_back(error.0.into_closed_completion());
            }
        }
    }

    fn close(&mut self, request_id: PersistenceRequestId) {
        self.send_request(WorldStoreRequest::Close { request_id });
        self.sender = None;
    }

    fn process_one_background_write(&mut self) -> bool {
        if self.drain_available_completions() {
            return true;
        }
        if self.pending_requests == 0 {
            return false;
        }
        match self
            .completion_receiver
            .recv_timeout(Duration::from_millis(1))
        {
            Ok(completion) => {
                self.push_worker_completion(completion);
                true
            }
            Err(mpsc::RecvTimeoutError::Timeout) => true,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.pending_requests = 0;
                false
            }
        }
    }

    fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        self.drain_available_completions();
        self.completion_buffer.drain(..).collect()
    }

    fn drain_available_completions(&mut self) -> bool {
        let mut received = false;
        loop {
            match self.completion_receiver.try_recv() {
                Ok(completion) => {
                    self.push_worker_completion(completion);
                    received = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending_requests = 0;
                    break;
                }
            }
        }
        received
    }

    fn push_worker_completion(&mut self, completion: WorldStoreCompletion) {
        self.pending_requests = self.pending_requests.saturating_sub(1);
        self.completion_buffer.push_back(completion);
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for ThreadedPersistenceActor {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(WorldStoreRequest::Close { request_id: 0 });
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn threaded_persistence_actor_loop(
    receiver: mpsc::Receiver<WorldStoreRequest>,
    completion_sender: mpsc::Sender<WorldStoreCompletion>,
    actor: &mut PersistenceActor,
) {
    loop {
        if actor.has_pending_background_write() {
            match receiver.recv_timeout(Duration::from_millis(1)) {
                Ok(request) => {
                    let should_close = matches!(request, WorldStoreRequest::Close { .. });
                    handle_world_store_request(actor, request);
                    if !forward_actor_completions(actor, &completion_sender) || should_close {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if actor.process_one_background_write()
                        && !forward_actor_completions(actor, &completion_sender)
                    {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    actor.close(0);
                    let _ = forward_actor_completions(actor, &completion_sender);
                    break;
                }
            }
        } else {
            match receiver.recv() {
                Ok(request) => {
                    let should_close = matches!(request, WorldStoreRequest::Close { .. });
                    handle_world_store_request(actor, request);
                    if !forward_actor_completions(actor, &completion_sender) || should_close {
                        break;
                    }
                }
                Err(_) => {
                    actor.close(0);
                    let _ = forward_actor_completions(actor, &completion_sender);
                    break;
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn forward_actor_completions(
    actor: &mut PersistenceActor,
    completion_sender: &mpsc::Sender<WorldStoreCompletion>,
) -> bool {
    for completion in actor.drain_completions() {
        if completion_sender.send(completion).is_err() {
            return false;
        }
    }
    true
}

#[derive(Debug)]
enum PersistenceBackend {
    Inline(PersistenceActor),
    ExternalLoads(ExternalLoadPersistenceActor),
    #[cfg(not(target_arch = "wasm32"))]
    Threaded(ThreadedPersistenceActor),
}

impl PersistenceBackend {
    fn entity_chunks_supported(&self) -> bool {
        match self {
            Self::Inline(actor) => actor.entity_chunks_supported(),
            Self::ExternalLoads(actor) => actor.entity_chunks_supported(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(actor) => actor.entity_chunks_supported(),
        }
    }

    fn send_request(&mut self, request: WorldStoreRequest) {
        match self {
            Self::Inline(actor) => handle_world_store_request(actor, request),
            Self::ExternalLoads(actor) => actor.send_request(request),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(actor) => actor.send_request(request),
        }
    }

    fn close(&mut self, request_id: PersistenceRequestId) {
        match self {
            Self::Inline(actor) => actor.close(request_id),
            Self::ExternalLoads(actor) => actor.close(request_id),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(actor) => actor.close(request_id),
        }
    }

    fn process_one_background_write(&mut self) -> bool {
        match self {
            Self::Inline(actor) => actor.process_one_background_write(),
            Self::ExternalLoads(actor) => actor.process_one_background_write(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(actor) => actor.process_one_background_write(),
        }
    }

    fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        match self {
            Self::Inline(actor) => actor.drain_completions(),
            Self::ExternalLoads(actor) => actor.drain_completions(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(actor) => actor.drain_completions(),
        }
    }

    fn drain_external_requests(&mut self) -> Vec<WorldStoreRequest> {
        match self {
            Self::ExternalLoads(actor) => actor.drain_external_requests(),
            Self::Inline(_) => Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(_) => Vec::new(),
        }
    }

    fn complete_external_request(
        &mut self,
        completion: WorldStoreCompletion,
    ) -> ChunkStoreResult<()> {
        match self {
            Self::ExternalLoads(actor) => actor.complete_external_request(completion),
            Self::Inline(_) => Err(ChunkStoreError::InvalidData(
                "inline persistence backend does not accept external completions".to_owned(),
            )),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(_) => Err(ChunkStoreError::InvalidData(
                "threaded persistence backend does not accept external completions".to_owned(),
            )),
        }
    }

    fn pending_external_request_count(&self) -> usize {
        match self {
            Self::ExternalLoads(actor) => actor.pending_external_request_count(),
            Self::Inline(_) => 0,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(_) => 0,
        }
    }
}

#[derive(Debug)]
struct SharedPersistenceBackend {
    backend: PersistenceBackend,
    next_request_id: PersistenceRequestId,
    completions: VecDeque<WorldStoreCompletion>,
}

#[derive(Debug)]
pub struct PersistenceMailbox {
    dimension: DimensionKey,
    shared: Rc<RefCell<SharedPersistenceBackend>>,
    owned_requests: BTreeSet<PersistenceRequestId>,
}

impl PersistenceMailbox {
    pub fn new(store: Box<dyn WorldStore>) -> Self {
        Self::in_dimension(DimensionKey::overworld(), store)
    }

    pub fn in_dimension(dimension: DimensionKey, store: Box<dyn WorldStore>) -> Self {
        Self {
            dimension,
            shared: Rc::new(RefCell::new(SharedPersistenceBackend {
                backend: PersistenceBackend::Inline(PersistenceActor::new(store)),
                next_request_id: 1,
                completions: VecDeque::new(),
            })),
            owned_requests: BTreeSet::new(),
        }
    }

    pub fn external_loads(store: Box<dyn WorldStore>) -> Self {
        Self::external_loads_in_dimension(DimensionKey::overworld(), store)
    }

    pub fn external_loads_in_dimension(
        dimension: DimensionKey,
        store: Box<dyn WorldStore>,
    ) -> Self {
        Self {
            dimension,
            shared: Rc::new(RefCell::new(SharedPersistenceBackend {
                backend: PersistenceBackend::ExternalLoads(ExternalLoadPersistenceActor::new(
                    store,
                )),
                next_request_id: 1,
                completions: VecDeque::new(),
            })),
            owned_requests: BTreeSet::new(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn threaded(store: Box<dyn WorldStore + Send>) -> ChunkStoreResult<Self> {
        Self::threaded_in_dimension(DimensionKey::overworld(), store)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn threaded_in_dimension(
        dimension: DimensionKey,
        store: Box<dyn WorldStore + Send>,
    ) -> ChunkStoreResult<Self> {
        Ok(Self {
            dimension,
            shared: Rc::new(RefCell::new(SharedPersistenceBackend {
                backend: PersistenceBackend::Threaded(ThreadedPersistenceActor::new(store)?),
                next_request_id: 1,
                completions: VecDeque::new(),
            })),
            owned_requests: BTreeSet::new(),
        })
    }

    pub fn transient() -> Self {
        Self::new(Box::new(RecordExecutorWorldStore::null()))
    }

    pub fn load_world_metadata(&mut self) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::LoadWorldMetadata { request_id })
    }

    pub fn save_world_metadata(&mut self, record: WorldMetadata) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::SaveWorldMetadata { request_id, record })
    }

    pub fn load_dimension(&mut self, key: DimensionKey) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::LoadDimension { request_id, key })
    }

    pub fn save_dimension(&mut self, record: DimensionRecord) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::SaveDimension { request_id, record })
    }

    pub fn memory() -> Self {
        Self::new(Box::new(RecordExecutorWorldStore::memory()))
    }

    pub fn entity_chunks_supported(&self) -> bool {
        self.shared.borrow().backend.entity_chunks_supported()
    }

    pub fn dimension(&self) -> &DimensionKey {
        &self.dimension
    }

    /// Creates another dimension-scoped view over the same realm store actor.
    /// Request ids remain globally unique and completions are routed back only
    /// to the scheduler that issued them.
    pub fn scoped_to_dimension(&self, dimension: DimensionKey) -> Self {
        Self {
            dimension,
            shared: Rc::clone(&self.shared),
            owned_requests: BTreeSet::new(),
        }
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) -> PersistenceRequestId {
        let dimension = self.dimension.clone();
        self.send_request(|request_id| WorldStoreRequest::LoadChunk {
            request_id,
            dimension,
            pos,
        })
    }

    pub fn save_chunk(
        &mut self,
        record: ChunkRecord,
        durability: SaveDurability,
    ) -> PersistenceRequestId {
        let dimension = self.dimension.clone();
        self.send_request(|request_id| WorldStoreRequest::SaveChunk {
            request_id,
            dimension,
            record,
            durability,
        })
    }

    pub fn load_entity_chunk(&mut self, pos: ChunkPos) -> PersistenceRequestId {
        let dimension = self.dimension.clone();
        self.send_request(|request_id| WorldStoreRequest::LoadEntityChunk {
            request_id,
            dimension,
            pos,
        })
    }

    pub fn save_entity_chunk(
        &mut self,
        record: EntityChunkRecord,
        durability: SaveDurability,
    ) -> PersistenceRequestId {
        let dimension = self.dimension.clone();
        self.send_request(|request_id| WorldStoreRequest::SaveEntityChunk {
            request_id,
            dimension,
            record,
            durability,
        })
    }

    pub fn load_player(&mut self, player: PlayerRecordKey) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::LoadPlayer { request_id, player })
    }

    pub fn save_player(&mut self, record: PlayerRecord) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::SavePlayer { request_id, record })
    }

    pub fn load_saved_data(&mut self, key: String) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::LoadSavedData { request_id, key })
    }

    pub fn flush(&mut self) -> PersistenceRequestId {
        self.send_request(|request_id| WorldStoreRequest::Flush { request_id })
    }

    pub fn close(&mut self) -> PersistenceRequestId {
        let request_id = self.next_shared_request_id();
        self.shared.borrow_mut().backend.close(request_id);
        self.owned_requests.insert(request_id);
        request_id
    }

    pub fn process_one_background_write(&mut self) -> bool {
        self.shared
            .borrow_mut()
            .backend
            .process_one_background_write()
    }

    pub fn process_all_background_writes(&mut self) {
        while self.process_one_background_write() {}
    }

    pub fn drain_completions(&mut self) -> Vec<WorldStoreCompletion> {
        self.collect_shared_completions();
        let mut shared = self.shared.borrow_mut();
        let mut retained = VecDeque::new();
        let mut owned = Vec::new();
        while let Some(completion) = shared.completions.pop_front() {
            if self.owned_requests.remove(&completion.request_id()) {
                owned.push(completion);
            } else {
                retained.push_back(completion);
            }
        }
        shared.completions = retained;
        owned
    }

    pub fn take_completion(
        &mut self,
        request_id: PersistenceRequestId,
    ) -> Option<WorldStoreCompletion> {
        if !self.owned_requests.contains(&request_id) {
            return None;
        }
        self.collect_shared_completions();
        let mut shared = self.shared.borrow_mut();
        let index = shared
            .completions
            .iter()
            .position(|completion| completion.request_id() == request_id)?;
        self.owned_requests.remove(&request_id);
        shared.completions.remove(index)
    }

    pub fn drain_external_requests(&mut self) -> Vec<WorldStoreRequest> {
        self.shared.borrow_mut().backend.drain_external_requests()
    }

    pub fn complete_external_request(
        &mut self,
        completion: WorldStoreCompletion,
    ) -> ChunkStoreResult<()> {
        self.shared
            .borrow_mut()
            .backend
            .complete_external_request(completion)
    }

    pub fn pending_external_request_count(&self) -> usize {
        self.shared
            .borrow()
            .backend
            .pending_external_request_count()
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

    pub fn load_world_metadata_blocking(&mut self) -> ChunkStoreResult<WorldMetadataLoad> {
        let request_id = self.load_world_metadata();
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::WorldMetadataLoaded { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "load_world_metadata completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn save_world_metadata_blocking(
        &mut self,
        record: WorldMetadata,
    ) -> ChunkStoreResult<StoreWriteOutcome> {
        let request_id = self.save_world_metadata(record);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::WorldMetadataSaved { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "save_world_metadata completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn load_dimension_blocking(
        &mut self,
        key: DimensionKey,
    ) -> ChunkStoreResult<Option<DimensionRecord>> {
        let request_id = self.load_dimension(key);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::DimensionLoaded { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "load_dimension completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn save_dimension_blocking(
        &mut self,
        record: DimensionRecord,
    ) -> ChunkStoreResult<StoreWriteOutcome> {
        let request_id = self.save_dimension(record);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::DimensionSaved { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "save_dimension completed with unexpected persistence completion {completion:?}"
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

    pub fn load_entity_chunk_blocking(
        &mut self,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        let request_id = self.load_entity_chunk(pos);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::EntityChunkLoaded { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "load_entity_chunk completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn save_entity_chunk_blocking(
        &mut self,
        record: EntityChunkRecord,
        durability: SaveDurability,
    ) -> ChunkStoreResult<StoreWriteOutcome> {
        let request_id = self.save_entity_chunk(record, durability);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::EntityChunkSaved { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "save_entity_chunk completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn load_player_blocking(
        &mut self,
        player: PlayerRecordKey,
    ) -> ChunkStoreResult<Option<PlayerRecord>> {
        let request_id = self.load_player(player);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::PlayerLoaded { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "load_player completed with unexpected persistence completion {completion:?}"
            ))),
        }
    }

    pub fn save_player_blocking(
        &mut self,
        record: PlayerRecord,
    ) -> ChunkStoreResult<StoreWriteOutcome> {
        let request_id = self.save_player(record);
        match self.take_or_run_until_completion(request_id)? {
            WorldStoreCompletion::PlayerSaved { result, .. } => result,
            completion => Err(ChunkStoreError::InvalidData(format!(
                "save_player completed with unexpected persistence completion {completion:?}"
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

    fn next_shared_request_id(&self) -> PersistenceRequestId {
        let mut shared = self.shared.borrow_mut();
        let request_id = shared.next_request_id;
        shared.next_request_id = shared.next_request_id.saturating_add(1);
        request_id
    }

    fn send_request(
        &mut self,
        request: impl FnOnce(PersistenceRequestId) -> WorldStoreRequest,
    ) -> PersistenceRequestId {
        let request_id = self.next_shared_request_id();
        self.shared
            .borrow_mut()
            .backend
            .send_request(request(request_id));
        self.owned_requests.insert(request_id);
        request_id
    }

    fn collect_shared_completions(&mut self) {
        let mut shared = self.shared.borrow_mut();
        let completions = shared.backend.drain_completions();
        shared.completions.extend(completions);
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
    fn load_chunk(
        &mut self,
        _dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
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

    fn save_chunk(
        &mut self,
        _dimension: &DimensionKey,
        record: &ChunkRecord,
    ) -> ChunkStoreResult<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.chunk_path(record.pos());
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        write_chunk_record(&mut writer, record)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct SqliteWorldStore {
    path: PathBuf,
    inner: RecordExecutorWorldStore<SqliteRecordExecutor>,
}

#[cfg(not(target_arch = "wasm32"))]
impl SqliteWorldStore {
    pub fn new(path: impl Into<PathBuf>) -> ChunkStoreResult<Self> {
        let path = path.into();
        let world_dir = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(world_dir)?;
        let _admission = NativeFileLease::acquire_blocking(&world_admission_lock_path(world_dir))?;
        let writer_lease =
            NativeFileLease::acquire_writer(&world_dir.join(WORLD_WRITER_LOCK_FILE))?;
        let executor = SqliteRecordExecutor::open_writer(path.clone(), writer_lease)?;
        Ok(Self {
            path,
            inner: RecordExecutorWorldStore::new(executor, true),
        })
    }

    pub fn open_world_dir(world_dir: impl AsRef<Path>) -> ChunkStoreResult<Self> {
        Self::new(Self::database_path_for_world_dir(world_dir))
    }

    /// Open a non-authoritative SQLite reader without acquiring the world
    /// writer lease. Mutation methods fail with `Unavailable`.
    pub fn open_world_dir_read_only(world_dir: impl AsRef<Path>) -> ChunkStoreResult<Self> {
        let path = Self::database_path_for_world_dir(world_dir);
        let executor = SqliteRecordExecutor::open_read_only(path.clone())?;
        Ok(Self {
            path,
            inner: RecordExecutorWorldStore::new(executor, true),
        })
    }

    /// Remove one world while excluding both live writers and concurrent
    /// open/create transitions. The short parent admission lock closes the
    /// Windows gap between releasing the in-directory handle and deletion.
    pub fn remove_world_dir_exclusive(world_dir: impl AsRef<Path>) -> ChunkStoreResult<()> {
        let world_dir = world_dir.as_ref();
        let _admission = NativeFileLease::acquire_blocking(&world_admission_lock_path(world_dir))?;
        let writer_lease =
            NativeFileLease::acquire_writer(&world_dir.join(WORLD_WRITER_LOCK_FILE))?;
        writer_lease.release()?;
        fs::remove_dir_all(world_dir)?;
        Ok(())
    }

    pub fn database_path_for_world_dir(world_dir: impl AsRef<Path>) -> PathBuf {
        world_dir.as_ref().join(SQLITE_WORLD_DATABASE_FILE)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_read_only(&self) -> bool {
        self.inner.executor().read_only
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldStore for SqliteWorldStore {
    fn supports_entity_chunks(&self) -> bool {
        true
    }

    fn load_world_metadata(&mut self) -> ChunkStoreResult<WorldMetadataLoad> {
        self.inner.load_world_metadata()
    }

    fn save_world_metadata(&mut self, record: &WorldMetadata) -> ChunkStoreResult<()> {
        self.inner.save_world_metadata(record)
    }

    fn load_dimension(&mut self, key: &DimensionKey) -> ChunkStoreResult<Option<DimensionRecord>> {
        self.inner.load_dimension(key)
    }

    fn save_dimension(&mut self, record: &DimensionRecord) -> ChunkStoreResult<()> {
        self.inner.save_dimension(record)
    }

    fn load_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<ChunkRecord>> {
        self.inner.load_chunk(dimension, pos)
    }

    fn save_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &ChunkRecord,
    ) -> ChunkStoreResult<()> {
        self.inner.save_chunk(dimension, record)
    }

    fn load_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        pos: ChunkPos,
    ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        self.inner.load_entity_chunk(dimension, pos)
    }

    fn save_entity_chunk(
        &mut self,
        dimension: &DimensionKey,
        record: &EntityChunkRecord,
    ) -> ChunkStoreResult<()> {
        self.inner.save_entity_chunk(dimension, record)
    }

    fn load_player(&mut self, player: &PlayerRecordKey) -> ChunkStoreResult<Option<PlayerRecord>> {
        self.inner.load_player(player)
    }

    fn save_player(&mut self, record: &PlayerRecord) -> ChunkStoreResult<()> {
        self.inner.save_player(record)
    }

    fn flush(&mut self) -> ChunkStoreResult<()> {
        self.inner.flush()
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        self.inner.close()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct SqliteRecordExecutor {
    connection: Option<Connection>,
    writer_lease: Option<NativeFileLease>,
    read_only: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl SqliteRecordExecutor {
    fn open_writer(path: PathBuf, writer_lease: NativeFileLease) -> ChunkStoreResult<Self> {
        let connection = Connection::open(&path).map_err(sqlite_error)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA synchronous = FULL;",
            )
            .map_err(sqlite_error)?;
        let _ = connection.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        });
        initialize_sqlite_world_schema(&connection)?;
        Ok(Self {
            connection: Some(connection),
            writer_lease: Some(writer_lease),
            read_only: false,
        })
    }

    fn open_read_only(path: PathBuf) -> ChunkStoreResult<Self> {
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sqlite_error)?;
        validate_sqlite_world_schema_read_only(&connection)?;
        Ok(Self {
            connection: Some(connection),
            writer_lease: None,
            read_only: true,
        })
    }

    fn connection(&self) -> ChunkStoreResult<&Connection> {
        self.connection
            .as_ref()
            .ok_or_else(|| ChunkStoreError::Closed("sqlite world store is closed".to_owned()))
    }

    fn writer_connection(&mut self) -> ChunkStoreResult<&mut Connection> {
        if self.read_only {
            return Err(ChunkStoreError::classified(
                PersistenceErrorKind::Unavailable,
                "read-only sqlite world store cannot mutate records",
            ));
        }
        self.connection
            .as_mut()
            .ok_or_else(|| ChunkStoreError::Closed("sqlite world store is closed".to_owned()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PersistenceRecordExecutor for SqliteRecordExecutor {
    fn read(
        &mut self,
        address: &PersistenceRecordAddress,
    ) -> ChunkStoreResult<Option<PersistenceRecordPayload>> {
        let connection = self.connection()?;
        let stored = match address.namespace {
            PersistenceRecordNamespace::WorldMetadata => {
                require_empty_record_key(address)?;
                connection
                    .query_row(
                        "SELECT codec_version, revision, record_blob
                         FROM world_metadata WHERE singleton_id = 1",
                        [],
                        sqlite_payload_row,
                    )
                    .optional()
                    .map_err(sqlite_error)?
            }
            PersistenceRecordNamespace::Dimension => {
                let key = require_text_record_key(address)?;
                connection
                    .query_row(
                        "SELECT codec_version, revision, record_blob
                         FROM dimension_records WHERE dimension_key = ?1",
                        params![key],
                        sqlite_payload_row,
                    )
                    .optional()
                    .map_err(sqlite_error)?
            }
            PersistenceRecordNamespace::Chunk | PersistenceRecordNamespace::EntityChunk => {
                let (dimension, x, z) = require_chunk_record_key(address)?;
                let statement = match address.namespace {
                    PersistenceRecordNamespace::Chunk => {
                        "SELECT codec_version, revision, record_blob FROM chunk_records
                         WHERE dimension_key = ?1 AND x = ?2 AND z = ?3"
                    }
                    PersistenceRecordNamespace::EntityChunk => {
                        "SELECT codec_version, revision, record_blob FROM entity_chunk_records
                         WHERE dimension_key = ?1 AND x = ?2 AND z = ?3"
                    }
                    _ => unreachable!(),
                };
                connection
                    .query_row(statement, params![dimension, x, z], sqlite_payload_row)
                    .optional()
                    .map_err(sqlite_error)?
            }
            PersistenceRecordNamespace::Player | PersistenceRecordNamespace::SavedData => {
                let key = require_text_record_key(address)?;
                let statement = match address.namespace {
                    PersistenceRecordNamespace::Player => {
                        "SELECT codec_version, revision, record_blob
                         FROM player_records WHERE player_key = ?1"
                    }
                    PersistenceRecordNamespace::SavedData => {
                        "SELECT codec_version, revision, record_blob
                         FROM saved_data_records WHERE data_key = ?1"
                    }
                    _ => unreachable!(),
                };
                connection
                    .query_row(statement, params![key], sqlite_payload_row)
                    .optional()
                    .map_err(sqlite_error)?
            }
        };
        stored.map(sqlite_payload).transpose()
    }

    fn probe_any(&mut self, namespaces: &[PersistenceRecordNamespace]) -> ChunkStoreResult<bool> {
        let connection = self.connection()?;
        for namespace in namespaces {
            let statement = match namespace {
                PersistenceRecordNamespace::WorldMetadata => {
                    "SELECT EXISTS(SELECT 1 FROM world_metadata LIMIT 1)"
                }
                PersistenceRecordNamespace::Dimension => {
                    "SELECT EXISTS(SELECT 1 FROM dimension_records LIMIT 1)"
                }
                PersistenceRecordNamespace::Chunk => {
                    "SELECT EXISTS(SELECT 1 FROM chunk_records LIMIT 1)"
                }
                PersistenceRecordNamespace::EntityChunk => {
                    "SELECT EXISTS(SELECT 1 FROM entity_chunk_records LIMIT 1)"
                }
                PersistenceRecordNamespace::Player => {
                    "SELECT EXISTS(SELECT 1 FROM player_records LIMIT 1)"
                }
                PersistenceRecordNamespace::SavedData => {
                    "SELECT EXISTS(SELECT 1 FROM saved_data_records LIMIT 1)"
                }
            };
            if connection
                .query_row(statement, [], |row| row.get::<_, bool>(0))
                .map_err(sqlite_error)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn commit(&mut self, batch: &PersistenceRecordBatch) -> ChunkStoreResult<()> {
        let transaction = self
            .writer_connection()?
            .transaction()
            .map_err(sqlite_error)?;
        for mutation in &batch.mutations {
            apply_sqlite_mutation(&transaction, mutation)?;
        }
        transaction.commit().map_err(sqlite_error)
    }

    fn flush(&mut self) -> ChunkStoreResult<()> {
        if self.read_only {
            self.connection()?;
            return Ok(());
        }
        let _busy: i64 = self
            .connection()?
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| row.get(0))
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        self.flush()?;
        let connection = self
            .connection
            .take()
            .ok_or_else(|| ChunkStoreError::Closed("sqlite world store is closed".to_owned()))?;
        connection
            .close()
            .map_err(|(_, error)| sqlite_error(error))?;
        if let Some(lease) = self.writer_lease.take() {
            lease.release()?;
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for SqliteRecordExecutor {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = connection.close();
        }
        if let Some(lease) = self.writer_lease.take() {
            let _ = lease.release();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sqlite_payload_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, Vec<u8>)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

#[cfg(not(target_arch = "wasm32"))]
fn sqlite_payload(
    (codec_version, revision, bytes): (i64, String, Vec<u8>),
) -> ChunkStoreResult<PersistenceRecordPayload> {
    let codec_version = u32::try_from(codec_version).map_err(|_| {
        ChunkStoreError::classified(
            PersistenceErrorKind::Corrupt,
            format!("sqlite record codec version {codec_version} does not fit in u32"),
        )
    })?;
    let revision = revision.parse::<u64>().map_err(|error| {
        ChunkStoreError::classified(
            PersistenceErrorKind::Corrupt,
            format!("sqlite record revision {revision:?} is invalid: {error}"),
        )
    })?;
    Ok(PersistenceRecordPayload::new(
        codec_version,
        revision,
        bytes,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_sqlite_mutation(
    transaction: &Transaction<'_>,
    mutation: &PersistenceRecordMutation,
) -> ChunkStoreResult<()> {
    match mutation {
        PersistenceRecordMutation::Put { address, payload } => {
            put_sqlite_record(transaction, address, payload)
        }
        PersistenceRecordMutation::Delete { address } => delete_sqlite_record(transaction, address),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn put_sqlite_record(
    transaction: &Transaction<'_>,
    address: &PersistenceRecordAddress,
    payload: &PersistenceRecordPayload,
) -> ChunkStoreResult<()> {
    let revision = payload.revision.to_string();
    match address.namespace {
        PersistenceRecordNamespace::WorldMetadata => {
            require_empty_record_key(address)?;
            transaction.execute(
                "INSERT INTO world_metadata
                    (singleton_id, codec_version, revision, record_blob)
                 VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(singleton_id) DO UPDATE SET
                    codec_version = excluded.codec_version,
                    revision = excluded.revision,
                    record_blob = excluded.record_blob",
                params![payload.codec_version, revision, payload.bytes],
            )
        }
        PersistenceRecordNamespace::Dimension => {
            let key = require_text_record_key(address)?;
            transaction.execute(
                "INSERT INTO dimension_records
                    (dimension_key, codec_version, revision, record_blob)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(dimension_key) DO UPDATE SET
                    codec_version = excluded.codec_version,
                    revision = excluded.revision,
                    record_blob = excluded.record_blob",
                params![key, payload.codec_version, revision, payload.bytes],
            )
        }
        PersistenceRecordNamespace::Chunk | PersistenceRecordNamespace::EntityChunk => {
            let (dimension, x, z) = require_chunk_record_key(address)?;
            let statement = match address.namespace {
                PersistenceRecordNamespace::Chunk => {
                    "INSERT INTO chunk_records
                        (dimension_key, x, z, codec_version, revision, record_blob)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(dimension_key, x, z) DO UPDATE SET
                        codec_version = excluded.codec_version,
                        revision = excluded.revision,
                        record_blob = excluded.record_blob"
                }
                PersistenceRecordNamespace::EntityChunk => {
                    "INSERT INTO entity_chunk_records
                        (dimension_key, x, z, codec_version, revision, record_blob)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(dimension_key, x, z) DO UPDATE SET
                        codec_version = excluded.codec_version,
                        revision = excluded.revision,
                        record_blob = excluded.record_blob"
                }
                _ => unreachable!(),
            };
            transaction.execute(
                statement,
                params![
                    dimension,
                    x,
                    z,
                    payload.codec_version,
                    revision,
                    payload.bytes
                ],
            )
        }
        PersistenceRecordNamespace::Player | PersistenceRecordNamespace::SavedData => {
            let key = require_text_record_key(address)?;
            let (table, column) = match address.namespace {
                PersistenceRecordNamespace::Player => ("player_records", "player_key"),
                PersistenceRecordNamespace::SavedData => ("saved_data_records", "data_key"),
                _ => unreachable!(),
            };
            let statement = format!(
                "INSERT INTO {table} ({column}, codec_version, revision, record_blob)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT({column}) DO UPDATE SET
                    codec_version = excluded.codec_version,
                    revision = excluded.revision,
                    record_blob = excluded.record_blob"
            );
            transaction.execute(
                &statement,
                params![key, payload.codec_version, revision, payload.bytes],
            )
        }
    }
    .map_err(sqlite_error)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn delete_sqlite_record(
    transaction: &Transaction<'_>,
    address: &PersistenceRecordAddress,
) -> ChunkStoreResult<()> {
    match address.namespace {
        PersistenceRecordNamespace::WorldMetadata => {
            require_empty_record_key(address)?;
            transaction.execute("DELETE FROM world_metadata WHERE singleton_id = 1", [])
        }
        PersistenceRecordNamespace::Dimension => transaction.execute(
            "DELETE FROM dimension_records WHERE dimension_key = ?1",
            params![require_text_record_key(address)?],
        ),
        PersistenceRecordNamespace::Chunk | PersistenceRecordNamespace::EntityChunk => {
            let (dimension, x, z) = require_chunk_record_key(address)?;
            let statement = match address.namespace {
                PersistenceRecordNamespace::Chunk => {
                    "DELETE FROM chunk_records WHERE dimension_key = ?1 AND x = ?2 AND z = ?3"
                }
                PersistenceRecordNamespace::EntityChunk => {
                    "DELETE FROM entity_chunk_records
                     WHERE dimension_key = ?1 AND x = ?2 AND z = ?3"
                }
                _ => unreachable!(),
            };
            transaction.execute(statement, params![dimension, x, z])
        }
        PersistenceRecordNamespace::Player | PersistenceRecordNamespace::SavedData => {
            let key = require_text_record_key(address)?;
            let statement = match address.namespace {
                PersistenceRecordNamespace::Player => {
                    "DELETE FROM player_records WHERE player_key = ?1"
                }
                PersistenceRecordNamespace::SavedData => {
                    "DELETE FROM saved_data_records WHERE data_key = ?1"
                }
                _ => unreachable!(),
            };
            transaction.execute(statement, params![key])
        }
    }
    .map_err(sqlite_error)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn require_empty_record_key(address: &PersistenceRecordAddress) -> ChunkStoreResult<()> {
    if address.key.is_empty() {
        Ok(())
    } else {
        Err(invalid_record_key(address))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn require_text_record_key(address: &PersistenceRecordAddress) -> ChunkStoreResult<&str> {
    match address.key.as_slice() {
        [PersistenceRecordKeyPart::Text(key)] => Ok(key),
        _ => Err(invalid_record_key(address)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn require_chunk_record_key(
    address: &PersistenceRecordAddress,
) -> ChunkStoreResult<(&str, i32, i32)> {
    match address.key.as_slice() {
        [
            PersistenceRecordKeyPart::Text(dimension),
            PersistenceRecordKeyPart::I32(x),
            PersistenceRecordKeyPart::I32(z),
        ] => Ok((dimension, *x, *z)),
        _ => Err(invalid_record_key(address)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn invalid_record_key(address: &PersistenceRecordAddress) -> ChunkStoreError {
    ChunkStoreError::InvalidData(format!(
        "invalid key {:?} for persistence namespace {:?}",
        address.key, address.namespace
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct NativeFileLease {
    file: Option<File>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeFileLease {
    fn acquire_blocking(path: &Path) -> ChunkStoreResult<Self> {
        let file = open_lock_file(path)?;
        native_file_lock(&file)?;
        Ok(Self { file: Some(file) })
    }

    fn acquire_writer(path: &Path) -> ChunkStoreResult<Self> {
        let mut file = open_lock_file(path)?;
        if let Err(error) = native_file_try_lock(&file) {
            return Err(match error {
                std::fs::TryLockError::WouldBlock => ChunkStoreError::classified(
                    PersistenceErrorKind::LeaseConflict,
                    format!(
                        "world already has an active writer lease at `{}`",
                        path.display()
                    ),
                ),
                std::fs::TryLockError::Error(error) => error.into(),
            });
        }
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        let acquired_unix_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        writeln!(
            file,
            "pid={} acquired_unix_millis={} backend=sqlite",
            std::process::id(),
            acquired_unix_millis
        )?;
        file.sync_data()?;
        Ok(Self { file: Some(file) })
    }

    fn release(mut self) -> ChunkStoreResult<()> {
        if let Some(file) = self.file.take() {
            native_file_unlock(&file)?;
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for NativeFileLease {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = native_file_unlock(&file);
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn native_file_lock(file: &File) -> io::Result<()> {
    file.lock()
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn native_file_try_lock(file: &File) -> Result<(), std::fs::TryLockError> {
    file.try_lock()
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn native_file_unlock(file: &File) -> io::Result<()> {
    file.unlock()
}

// Rust 1.92's `std::fs::File` lock implementation returns `Unsupported` on
// Android even though bionic exposes the same `flock(2)` API used by std on
// Linux. Keep the lifetime-owned lock contract through fs4's safe rustix
// adapter instead of weakening Android admission.
#[cfg(target_os = "android")]
fn native_file_lock(file: &File) -> io::Result<()> {
    fs4::FileExt::lock(file)
}

#[cfg(target_os = "android")]
fn native_file_try_lock(file: &File) -> Result<(), std::fs::TryLockError> {
    match fs4::FileExt::try_lock(file) {
        Ok(()) => Ok(()),
        Err(fs4::TryLockError::WouldBlock) => Err(std::fs::TryLockError::WouldBlock),
        Err(fs4::TryLockError::Error(error)) => Err(std::fs::TryLockError::Error(error)),
    }
}

#[cfg(target_os = "android")]
fn native_file_unlock(file: &File) -> io::Result<()> {
    fs4::FileExt::unlock(file)
}

#[cfg(not(target_arch = "wasm32"))]
fn open_lock_file(path: &Path) -> ChunkStoreResult<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)?)
}

#[cfg(not(target_arch = "wasm32"))]
fn world_admission_lock_path(world_dir: &Path) -> PathBuf {
    world_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(WORLD_ADMISSION_LOCK_FILE)
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_sqlite_world_schema_read_only(connection: &Connection) -> ChunkStoreResult<()> {
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite_error)?;
    if user_version != SQLITE_WORLD_SCHEMA_VERSION {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!(
                "read-only sqlite world schema version {user_version}; expected {SQLITE_WORLD_SCHEMA_VERSION}"
            ),
        ));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn initialize_sqlite_world_schema(connection: &Connection) -> ChunkStoreResult<()> {
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite_error)?;
    if !(0..=SQLITE_WORLD_SCHEMA_VERSION).contains(&user_version) {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!(
                "unsupported sqlite world schema version {user_version}; expected {SQLITE_WORLD_SCHEMA_VERSION}"
            ),
        ));
    }

    if user_version == 1 {
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 ALTER TABLE chunk_records RENAME TO legacy_chunk_records;
                 ALTER TABLE entity_chunk_records RENAME TO legacy_entity_chunk_records;
                 CREATE TABLE chunk_records (
                    dimension_key TEXT NOT NULL,
                    x INTEGER NOT NULL,
                    z INTEGER NOT NULL,
                    codec_version INTEGER NOT NULL,
                    revision TEXT NOT NULL,
                    record_blob BLOB NOT NULL,
                    PRIMARY KEY (dimension_key, x, z)
                 );
                 CREATE TABLE entity_chunk_records (
                    dimension_key TEXT NOT NULL,
                    x INTEGER NOT NULL,
                    z INTEGER NOT NULL,
                    codec_version INTEGER NOT NULL,
                    revision TEXT NOT NULL,
                    record_blob BLOB NOT NULL,
                    PRIMARY KEY (dimension_key, x, z)
                 );
                 INSERT INTO chunk_records
                    (dimension_key, x, z, codec_version, revision, record_blob)
                    SELECT 'minecraft:overworld', x, z, codec_version, revision,
                           record_blob
                    FROM legacy_chunk_records;
                 INSERT INTO entity_chunk_records
                    (dimension_key, x, z, codec_version, revision, record_blob)
                    SELECT 'minecraft:overworld', x, z, codec_version, revision,
                           record_blob
                    FROM legacy_entity_chunk_records;
                 DROP TABLE legacy_chunk_records;
                 DROP TABLE legacy_entity_chunk_records;
                 CREATE TABLE IF NOT EXISTS dimension_records (
                    dimension_key TEXT PRIMARY KEY,
                    codec_version INTEGER NOT NULL,
                    revision TEXT NOT NULL,
                    record_blob BLOB NOT NULL
                 );
                 INSERT INTO metadata (key, value) VALUES ('schema_version', '2')
                    ON CONFLICT(key) DO UPDATE SET value = excluded.value;
                 PRAGMA user_version = 2;
                 COMMIT;",
            )
            .map_err(sqlite_error)?;
        return Ok(());
    }

    if user_version == SQLITE_WORLD_SCHEMA_VERSION {
        return Ok(());
    }

    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS world_metadata (
                singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
                codec_version INTEGER NOT NULL,
                revision TEXT NOT NULL,
                record_blob BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS chunk_records (
                dimension_key TEXT NOT NULL,
                x INTEGER NOT NULL,
                z INTEGER NOT NULL,
                codec_version INTEGER NOT NULL,
                revision TEXT NOT NULL,
                record_blob BLOB NOT NULL,
                PRIMARY KEY (dimension_key, x, z)
             );
             CREATE TABLE IF NOT EXISTS entity_chunk_records (
                dimension_key TEXT NOT NULL,
                x INTEGER NOT NULL,
                z INTEGER NOT NULL,
                codec_version INTEGER NOT NULL,
                revision TEXT NOT NULL,
                record_blob BLOB NOT NULL,
                PRIMARY KEY (dimension_key, x, z)
             );
             CREATE TABLE IF NOT EXISTS dimension_records (
                dimension_key TEXT PRIMARY KEY,
                codec_version INTEGER NOT NULL,
                revision TEXT NOT NULL,
                record_blob BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS player_records (
                player_key TEXT PRIMARY KEY,
                codec_version INTEGER NOT NULL,
                revision TEXT NOT NULL,
                record_blob BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS saved_data_records (
                data_key TEXT PRIMARY KEY,
                codec_version INTEGER NOT NULL,
                revision TEXT NOT NULL,
                record_blob BLOB NOT NULL
             );
             INSERT OR IGNORE INTO metadata (key, value)
                VALUES ('schema_version', '2');
             PRAGMA user_version = 2;
             COMMIT;",
        )
        .map_err(sqlite_error)?;
    Ok(())
}

#[cfg(test)]
fn write_snapshot(writer: &mut impl Write, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
    write_chunk_record(writer, &ChunkRecord::from_snapshot(snapshot.clone()))
}

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
    write_scheduled_ticks(
        writer,
        &record.scheduled_block_ticks,
        "scheduled block tick count",
    )?;
    write_scheduled_ticks(
        writer,
        &record.scheduled_fluid_ticks,
        "scheduled fluid tick count",
    )?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
fn read_snapshot(reader: &mut impl Read) -> ChunkStoreResult<ChunkSnapshot> {
    read_chunk_record(reader).map(|record| record.snapshot)
}

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
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!("unsupported chunk snapshot format version {version}"),
        ));
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
    let (scheduled_block_ticks, scheduled_fluid_ticks) = if version >= 5 {
        (read_scheduled_ticks(reader)?, read_scheduled_ticks(reader)?)
    } else {
        (Vec::new(), Vec::new())
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
        scheduled_block_ticks,
        scheduled_fluid_ticks,
    })
}

fn write_entity_chunk_record(
    writer: &mut impl Write,
    record: &EntityChunkRecord,
) -> ChunkStoreResult<()> {
    writer.write_all(ENTITY_CHUNK_MAGIC)?;
    write_u32(writer, ENTITY_CHUNK_RECORD_VERSION)?;
    write_i32(writer, record.pos.x)?;
    write_i32(writer, record.pos.z)?;
    write_u64(writer, record.revision)?;
    write_u32(writer, record.codec_version)?;
    write_len(writer, record.entities.len(), "entity count")?;
    for entity in &record.entities {
        write_entity_save_record(writer, entity)?;
    }
    writer.flush()?;
    Ok(())
}

fn read_entity_chunk_record(reader: &mut impl Read) -> ChunkStoreResult<EntityChunkRecord> {
    let mut magic = [0_u8; ENTITY_CHUNK_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != ENTITY_CHUNK_MAGIC {
        return Err(ChunkStoreError::InvalidData(
            "entity chunk record had invalid magic".to_owned(),
        ));
    }
    let version = read_u32(reader)?;
    if version != ENTITY_CHUNK_RECORD_VERSION {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!("unsupported entity chunk record version {version}"),
        ));
    }
    let pos = ChunkPos::new(read_i32(reader)?, read_i32(reader)?);
    let revision = read_u64(reader)?;
    let codec_version = read_u32(reader)?;
    if codec_version != ENTITY_CHUNK_RECORD_VERSION {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!("unsupported entity chunk codec version {codec_version}"),
        ));
    }
    let entity_count = read_len(reader)?;
    let mut entities = Vec::with_capacity(entity_count);
    for _ in 0..entity_count {
        entities.push(read_entity_save_record(reader)?);
    }
    Ok(EntityChunkRecord {
        pos,
        revision,
        codec_version,
        entities,
    })
}

fn write_player_record(writer: &mut impl Write, record: &PlayerRecord) -> ChunkStoreResult<()> {
    if record.codec_version != PLAYER_RECORD_VERSION {
        return Err(ChunkStoreError::InvalidData(format!(
            "unsupported player record codec version {}",
            record.codec_version
        )));
    }
    if !record.position.is_finite()
        || !record.y_rot_degrees.is_finite()
        || !record.x_rot_degrees.is_finite()
    {
        return Err(ChunkStoreError::InvalidData(
            "player record pose must be finite".to_owned(),
        ));
    }
    validate_player_lifecycle_record(record.health, record.pending_death_cause)?;
    writer.write_all(PLAYER_RECORD_MAGIC)?;
    write_u32(writer, PLAYER_RECORD_VERSION)?;
    write_string(writer, record.player.as_str(), "player key")?;
    write_u64(writer, record.revision)?;
    write_string(writer, &record.last_known_name, "player name")?;
    write_string(writer, record.dimension.as_str(), "player dimension")?;
    write_vec3d(writer, record.position)?;
    write_f32(writer, record.y_rot_degrees)?;
    write_f32(writer, record.x_rot_degrees)?;
    write_bool(writer, record.on_ground)?;
    write_u8(writer, record.selected_hotbar_slot)?;
    write_u64(writer, record.total_experience)?;
    write_player_statistics(writer, &record.statistics)?;
    write_f32(writer, record.health)?;
    write_player_damage_cause(writer, record.pending_death_cause)?;
    writer.flush()?;
    Ok(())
}

fn write_world_metadata(writer: &mut impl Write, record: &WorldMetadata) -> ChunkStoreResult<()> {
    if record.codec_version != WORLD_METADATA_VERSION {
        return Err(ChunkStoreError::InvalidData(format!(
            "unsupported world metadata codec version {}",
            record.codec_version
        )));
    }
    if record.target_minecraft_version != WORLD_METADATA_TARGET_MINECRAFT_VERSION {
        return Err(ChunkStoreError::InvalidData(format!(
            "unsupported world metadata target {}; expected {}",
            record.target_minecraft_version, WORLD_METADATA_TARGET_MINECRAFT_VERSION
        )));
    }
    writer.write_all(WORLD_METADATA_MAGIC)?;
    write_u32(writer, WORLD_METADATA_VERSION)?;
    writer.write_all(&record.realm_id.bytes())?;
    write_u64(writer, record.revision)?;
    write_string(
        writer,
        &record.target_minecraft_version,
        "target Minecraft version",
    )?;
    write_i64(writer, record.seed)?;
    write_world_generation_profile(writer, record.world_generation_profile)?;
    write_world_behavior_profile(writer, record.world_behavior_profile)?;
    write_u64(writer, record.created_unix_millis)?;
    write_u64(writer, record.last_played_unix_millis)?;
    write_u64(writer, record.game_time)?;
    write_u64(writer, record.day_time)?;
    write_bool(writer, record.do_daylight_cycle)?;
    writer.flush()?;
    Ok(())
}

fn write_dimension_record(
    writer: &mut impl Write,
    record: &DimensionRecord,
) -> ChunkStoreResult<()> {
    if record.codec_version != DIMENSION_RECORD_VERSION {
        return Err(ChunkStoreError::InvalidData(format!(
            "unsupported dimension record codec version {}",
            record.codec_version
        )));
    }
    let definition = &record.definition;
    if !definition.coordinate_scale.is_finite() || definition.coordinate_scale <= 0.0 {
        return Err(ChunkStoreError::InvalidData(
            "dimension coordinate scale must be finite and positive".to_owned(),
        ));
    }
    if definition.height <= 0 {
        return Err(ChunkStoreError::InvalidData(
            "dimension height must be positive".to_owned(),
        ));
    }
    definition.topology.validate().map_err(|error| {
        ChunkStoreError::InvalidData(format!("invalid dimension topology: {error}"))
    })?;
    writer.write_all(DIMENSION_RECORD_MAGIC)?;
    write_u32(writer, DIMENSION_RECORD_VERSION)?;
    write_string(writer, record.key.as_str(), "dimension key")?;
    write_u64(writer, record.revision)?;
    write_i64(writer, definition.seed)?;
    write_world_generation_profile(writer, definition.generation_profile)?;
    write_horizontal_topology(writer, definition.topology)?;
    write_i32(writer, definition.min_y)?;
    write_i32(writer, definition.height)?;
    write_f64(writer, definition.coordinate_scale)?;
    write_bool(writer, definition.has_sky_light)?;
    write_bool(writer, definition.has_ceiling)?;
    write_bool(writer, definition.ultrawarm)?;
    writer.flush()?;
    Ok(())
}

fn read_dimension_record(reader: &mut impl Read) -> ChunkStoreResult<DimensionRecord> {
    let mut magic = [0_u8; DIMENSION_RECORD_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != DIMENSION_RECORD_MAGIC {
        return Err(ChunkStoreError::InvalidData(
            "dimension record had invalid magic".to_owned(),
        ));
    }
    let codec_version = read_u32(reader)?;
    if !(1..=DIMENSION_RECORD_VERSION).contains(&codec_version) {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!("unsupported dimension record codec version {codec_version}"),
        ));
    }
    let key_text = read_string(reader)?;
    let key = DimensionKey::parse(&key_text).map_err(|error| {
        ChunkStoreError::InvalidData(format!(
            "dimension record had invalid key {key_text:?}: {error}"
        ))
    })?;
    let revision = read_u64(reader)?;
    let seed = read_i64(reader)?;
    let generation_profile = read_world_generation_profile(reader)?;
    let topology = if codec_version >= 2 {
        read_horizontal_topology(reader)?
    } else {
        HorizontalTopology::UNBOUNDED
    };
    let definition = DimensionDefinition {
        seed,
        generation_profile,
        topology,
        min_y: read_i32(reader)?,
        height: read_i32(reader)?,
        coordinate_scale: read_f64(reader)?,
        has_sky_light: read_bool(reader)?,
        has_ceiling: read_bool(reader)?,
        ultrawarm: read_bool(reader)?,
    };
    if !definition.coordinate_scale.is_finite() || definition.coordinate_scale <= 0.0 {
        return Err(ChunkStoreError::InvalidData(
            "dimension coordinate scale must be finite and positive".to_owned(),
        ));
    }
    if definition.height <= 0 {
        return Err(ChunkStoreError::InvalidData(
            "dimension height must be positive".to_owned(),
        ));
    }
    definition.topology.validate().map_err(|error| {
        ChunkStoreError::InvalidData(format!("invalid dimension topology: {error}"))
    })?;
    Ok(DimensionRecord {
        key,
        codec_version: DIMENSION_RECORD_VERSION,
        revision,
        definition,
    })
}

fn write_horizontal_topology(
    writer: &mut impl Write,
    topology: HorizontalTopology,
) -> ChunkStoreResult<()> {
    topology.validate().map_err(|error| {
        ChunkStoreError::InvalidData(format!("invalid dimension topology: {error}"))
    })?;
    write_axis_topology(writer, topology.x)?;
    write_axis_topology(writer, topology.z)
}

fn read_horizontal_topology(reader: &mut impl Read) -> ChunkStoreResult<HorizontalTopology> {
    let topology =
        HorizontalTopology::new(read_axis_topology(reader)?, read_axis_topology(reader)?);
    topology.validate().map_err(|error| {
        ChunkStoreError::InvalidData(format!("invalid dimension topology: {error}"))
    })?;
    Ok(topology)
}

fn write_axis_topology(writer: &mut impl Write, axis: AxisTopology) -> ChunkStoreResult<()> {
    match axis {
        AxisTopology::Unbounded => write_u8(writer, 0),
        AxisTopology::Finite {
            minimum_chunk,
            maximum_chunk_exclusive,
        } => {
            write_u8(writer, 1)?;
            write_i32(writer, minimum_chunk)?;
            write_i32(writer, maximum_chunk_exclusive)
        }
        AxisTopology::Periodic {
            minimum_chunk,
            period_chunks,
        } => {
            write_u8(writer, 2)?;
            write_i32(writer, minimum_chunk)?;
            write_u32(writer, period_chunks)
        }
    }
}

fn read_axis_topology(reader: &mut impl Read) -> ChunkStoreResult<AxisTopology> {
    match read_u8(reader)? {
        0 => Ok(AxisTopology::Unbounded),
        1 => Ok(AxisTopology::finite(read_i32(reader)?, read_i32(reader)?)),
        2 => Ok(AxisTopology::periodic(read_i32(reader)?, read_u32(reader)?)),
        tag => Err(ChunkStoreError::InvalidData(format!(
            "unknown dimension axis topology tag {tag}"
        ))),
    }
}

fn read_world_metadata(reader: &mut impl Read) -> ChunkStoreResult<WorldMetadata> {
    let mut magic = [0_u8; WORLD_METADATA_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != WORLD_METADATA_MAGIC {
        return Err(ChunkStoreError::InvalidData(
            "world metadata had invalid magic".to_owned(),
        ));
    }
    let codec_version = read_u32(reader)?;
    if !(1..=WORLD_METADATA_VERSION).contains(&codec_version) {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!("unsupported world metadata codec version {codec_version}"),
        ));
    }
    let realm_id = if codec_version >= 2 {
        let mut bytes = [0_u8; 16];
        reader.read_exact(&mut bytes)?;
        RealmId::new(bytes).map_err(|error| {
            ChunkStoreError::InvalidData(format!("world metadata had invalid realm id: {error}"))
        })?
    } else {
        RealmId::LEGACY_SINGLE_REALM
    };
    let record = WorldMetadata {
        codec_version,
        realm_id,
        revision: read_u64(reader)?,
        target_minecraft_version: read_string(reader)?,
        seed: read_i64(reader)?,
        world_generation_profile: read_world_generation_profile(reader)?,
        world_behavior_profile: read_world_behavior_profile(reader)?,
        created_unix_millis: read_u64(reader)?,
        last_played_unix_millis: read_u64(reader)?,
        game_time: read_u64(reader)?,
        day_time: read_u64(reader)?,
        do_daylight_cycle: read_bool(reader)?,
    };
    if record.target_minecraft_version != WORLD_METADATA_TARGET_MINECRAFT_VERSION {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!(
                "unsupported world metadata target {}; expected {}",
                record.target_minecraft_version, WORLD_METADATA_TARGET_MINECRAFT_VERSION
            ),
        ));
    }
    Ok(record)
}

fn write_world_generation_profile(
    writer: &mut impl Write,
    profile: WorldGenerationProfile,
) -> ChunkStoreResult<()> {
    write_u8(writer, profile.codec_tag())
}

fn read_world_generation_profile(
    reader: &mut impl Read,
) -> ChunkStoreResult<WorldGenerationProfile> {
    let tag = read_u8(reader)?;
    WorldGenerationProfile::from_codec_tag(tag).ok_or_else(|| {
        ChunkStoreError::InvalidData(format!("unknown world generation profile tag {tag}"))
    })
}

fn write_world_behavior_profile(
    writer: &mut impl Write,
    profile: WorldBehaviorProfile,
) -> ChunkStoreResult<()> {
    write_u8(
        writer,
        match profile {
            WorldBehaviorProfile::Mutable => 0,
            WorldBehaviorProfile::ProtectedLobby => 1,
        },
    )
}

fn read_world_behavior_profile(reader: &mut impl Read) -> ChunkStoreResult<WorldBehaviorProfile> {
    match read_u8(reader)? {
        0 => Ok(WorldBehaviorProfile::Mutable),
        1 => Ok(WorldBehaviorProfile::ProtectedLobby),
        tag => Err(ChunkStoreError::InvalidData(format!(
            "unknown world behavior profile tag {tag}"
        ))),
    }
}

fn read_player_record(reader: &mut impl Read) -> ChunkStoreResult<PlayerRecord> {
    let mut magic = [0_u8; PLAYER_RECORD_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != PLAYER_RECORD_MAGIC {
        return Err(ChunkStoreError::InvalidData(
            "player record had invalid magic".to_owned(),
        ));
    }
    let codec_version = read_u32(reader)?;
    if !(LEGACY_PLAYER_RECORD_VERSION..=PLAYER_RECORD_VERSION).contains(&codec_version) {
        return Err(ChunkStoreError::classified(
            PersistenceErrorKind::Incompatible,
            format!("unsupported player record codec version {codec_version}"),
        ));
    }
    let player = PlayerRecordKey::Uuid(read_string(reader)?);
    let record = PlayerRecord {
        player,
        codec_version: PLAYER_RECORD_VERSION,
        revision: read_u64(reader)?,
        last_known_name: read_string(reader)?,
        dimension: DimensionKey::parse(read_string(reader)?).map_err(|error| {
            ChunkStoreError::InvalidData(format!("invalid player dimension: {error}"))
        })?,
        position: read_vec3d(reader)?,
        y_rot_degrees: read_f32(reader)?,
        x_rot_degrees: read_f32(reader)?,
        on_ground: read_bool(reader)?,
        selected_hotbar_slot: read_u8(reader)?,
        total_experience: read_u64(reader)?,
        statistics: if codec_version >= STATISTICS_PLAYER_RECORD_VERSION {
            read_player_statistics(reader)?
        } else {
            PlayerStatistics::default()
        },
        health: if codec_version >= PLAYER_RECORD_VERSION {
            read_f32(reader)?
        } else {
            DEFAULT_PLAYER_MAX_HEALTH
        },
        pending_death_cause: if codec_version >= PLAYER_RECORD_VERSION {
            read_player_damage_cause(reader)?
        } else {
            None
        },
    };
    if !record.position.is_finite()
        || !record.y_rot_degrees.is_finite()
        || !record.x_rot_degrees.is_finite()
    {
        return Err(ChunkStoreError::InvalidData(
            "player record pose must be finite".to_owned(),
        ));
    }
    validate_player_lifecycle_record(record.health, record.pending_death_cause)?;
    Ok(record)
}

fn validate_player_lifecycle_record(
    health: f32,
    pending_death_cause: Option<PlayerDamageCause>,
) -> ChunkStoreResult<()> {
    let vitals = PlayerVitals::new(health, DEFAULT_PLAYER_MAX_HEALTH)
        .map_err(|error| ChunkStoreError::InvalidData(error.to_string()))?;
    if vitals.is_dead() != pending_death_cause.is_some() {
        return Err(ChunkStoreError::InvalidData(
            "player death cause must be present exactly when health is zero".to_owned(),
        ));
    }
    Ok(())
}

fn write_player_damage_cause(
    writer: &mut impl Write,
    cause: Option<PlayerDamageCause>,
) -> ChunkStoreResult<()> {
    write_u8(
        writer,
        match cause {
            None => 0,
            Some(PlayerDamageCause::Lava) => 1,
        },
    )
}

fn read_player_damage_cause(reader: &mut impl Read) -> ChunkStoreResult<Option<PlayerDamageCause>> {
    match read_u8(reader)? {
        0 => Ok(None),
        1 => Ok(Some(PlayerDamageCause::Lava)),
        tag => Err(ChunkStoreError::InvalidData(format!(
            "unknown player damage cause tag {tag}"
        ))),
    }
}

fn write_player_statistics(
    writer: &mut impl Write,
    statistics: &PlayerStatistics,
) -> ChunkStoreResult<()> {
    if statistics.len() > MAX_PLAYER_STATISTIC_ENTRIES {
        return Err(ChunkStoreError::InvalidData(format!(
            "player statistics contain {} entries; maximum is {}",
            statistics.len(),
            MAX_PLAYER_STATISTIC_ENTRIES
        )));
    }
    write_len(writer, statistics.len(), "player statistic entries")?;
    for (key, value) in statistics.iter() {
        write_string(writer, key.statistic_type(), "statistic type")?;
        write_string(writer, key.value(), "statistic value")?;
        write_u32(writer, *value)?;
    }
    Ok(())
}

fn read_player_statistics(reader: &mut impl Read) -> ChunkStoreResult<PlayerStatistics> {
    let count = read_len(reader)?;
    if count > MAX_PLAYER_STATISTIC_ENTRIES {
        return Err(ChunkStoreError::InvalidData(format!(
            "player statistics contain {count} entries; maximum is {MAX_PLAYER_STATISTIC_ENTRIES}"
        )));
    }
    let mut statistics = PlayerStatistics::default();
    for _ in 0..count {
        let statistic_type =
            read_bounded_string(reader, MAX_STATISTIC_RESOURCE_KEY_BYTES, "statistic type")?;
        let value =
            read_bounded_string(reader, MAX_STATISTIC_RESOURCE_KEY_BYTES, "statistic value")?;
        let key = StatisticKey::new(statistic_type, value).map_err(|error| {
            ChunkStoreError::InvalidData(format!("invalid player statistic key: {error}"))
        })?;
        statistics.set(key, read_u32(reader)?);
    }
    Ok(statistics)
}

fn write_entity_save_record(
    writer: &mut impl Write,
    record: &EntitySaveRecord,
) -> ChunkStoreResult<()> {
    write_u64(writer, record.persistent_id.most)?;
    write_u64(writer, record.persistent_id.least)?;
    write_string(writer, &record.kind, "entity kind")?;
    write_vec3d(writer, record.position)?;
    write_vec3d(writer, record.delta_movement)?;
    write_f32(writer, record.y_rot_degrees)?;
    write_f32(writer, record.x_rot_degrees)?;
    write_optional_rotation(writer, record.rotation)?;
    write_bool(writer, record.on_ground)?;
    write_entity_save_payload(writer, &record.payload)
}

fn read_entity_save_record(reader: &mut impl Read) -> ChunkStoreResult<EntitySaveRecord> {
    let persistent_id = EntityPersistentId::new(read_u64(reader)?, read_u64(reader)?);
    let kind = read_string(reader)?;
    let position = read_vec3d(reader)?;
    let delta_movement = read_vec3d(reader)?;
    let y_rot_degrees = read_f32(reader)?;
    let x_rot_degrees = read_f32(reader)?;
    let rotation = read_optional_rotation(reader)?;
    let on_ground = read_bool(reader)?;
    let payload = read_entity_save_payload(reader)?;
    Ok(EntitySaveRecord {
        persistent_id,
        kind,
        position,
        delta_movement,
        y_rot_degrees,
        x_rot_degrees,
        rotation,
        on_ground,
        payload,
    })
}

fn write_entity_save_payload(
    writer: &mut impl Write,
    payload: &EntitySavePayload,
) -> ChunkStoreResult<()> {
    match payload {
        EntitySavePayload::Cow => write_u8(writer, 0),
        EntitySavePayload::Mannequin => write_u8(writer, 3),
        EntitySavePayload::Chicken { egg_time } => {
            write_u8(writer, 1)?;
            write_i32(writer, *egg_time)
        }
        EntitySavePayload::Item {
            stack,
            age,
            pickup_delay,
        } => {
            write_u8(writer, 2)?;
            write_item_stack_save_record(writer, stack)?;
            write_u64(writer, *age)?;
            write_i32(writer, *pickup_delay)
        }
    }
}

fn read_entity_save_payload(reader: &mut impl Read) -> ChunkStoreResult<EntitySavePayload> {
    match read_u8(reader)? {
        0 => Ok(EntitySavePayload::Cow),
        1 => Ok(EntitySavePayload::Chicken {
            egg_time: read_i32(reader)?,
        }),
        2 => Ok(EntitySavePayload::Item {
            stack: read_item_stack_save_record(reader)?,
            age: read_u64(reader)?,
            pickup_delay: read_i32(reader)?,
        }),
        3 => Ok(EntitySavePayload::Mannequin),
        value => Err(ChunkStoreError::InvalidData(format!(
            "unknown entity save payload kind {value}"
        ))),
    }
}

fn write_item_stack_save_record(
    writer: &mut impl Write,
    stack: &ItemStackSaveRecord,
) -> ChunkStoreResult<()> {
    write_string(writer, &stack.kind, "item stack kind")?;
    write_u8(writer, stack.count)
}

fn read_item_stack_save_record(reader: &mut impl Read) -> ChunkStoreResult<ItemStackSaveRecord> {
    Ok(ItemStackSaveRecord::new(
        read_string(reader)?,
        read_u8(reader)?,
    ))
}

fn write_vec3d(writer: &mut impl Write, value: Vec3d) -> ChunkStoreResult<()> {
    write_f64(writer, value.x)?;
    write_f64(writer, value.y)?;
    write_f64(writer, value.z)
}

fn read_vec3d(reader: &mut impl Read) -> ChunkStoreResult<Vec3d> {
    Ok(Vec3d::new(
        read_f64(reader)?,
        read_f64(reader)?,
        read_f64(reader)?,
    ))
}

fn write_optional_rotation(
    writer: &mut impl Write,
    rotation: Option<EntityRotation>,
) -> ChunkStoreResult<()> {
    match rotation {
        Some(rotation) => {
            write_bool(writer, true)?;
            write_f32(writer, rotation.x)?;
            write_f32(writer, rotation.y)?;
            write_f32(writer, rotation.z)?;
            write_f32(writer, rotation.w)
        }
        None => write_bool(writer, false),
    }
}

fn read_optional_rotation(reader: &mut impl Read) -> ChunkStoreResult<Option<EntityRotation>> {
    if !read_bool(reader)? {
        return Ok(None);
    }
    Ok(Some(EntityRotation {
        x: read_f32(reader)?,
        y: read_f32(reader)?,
        z: read_f32(reader)?,
        w: read_f32(reader)?,
    }))
}

fn write_scheduled_ticks(
    writer: &mut impl Write,
    ticks: &[ScheduledTickRecord],
    name: &str,
) -> ChunkStoreResult<()> {
    write_len(writer, ticks.len(), name)?;
    for tick in ticks {
        write_block_pos(writer, tick.pos)?;
        write_string(writer, &tick.target, "scheduled tick target")?;
        write_i32(writer, tick.delay)?;
    }
    Ok(())
}

fn read_scheduled_ticks(reader: &mut impl Read) -> ChunkStoreResult<Vec<ScheduledTickRecord>> {
    let tick_count = read_len(reader)?;
    let mut ticks = Vec::with_capacity(tick_count);
    for _ in 0..tick_count {
        ticks.push(ScheduledTickRecord {
            pos: read_block_pos(reader)?,
            target: read_string(reader)?,
            delay: read_i32(reader)?,
        });
    }
    Ok(ticks)
}

fn write_block_pos(writer: &mut impl Write, pos: BlockPos) -> ChunkStoreResult<()> {
    write_i32(writer, pos.x)?;
    write_i32(writer, pos.y)?;
    write_i32(writer, pos.z)?;
    Ok(())
}

fn read_block_pos(reader: &mut impl Read) -> ChunkStoreResult<BlockPos> {
    Ok(BlockPos::new(
        read_i32(reader)?,
        read_i32(reader)?,
        read_i32(reader)?,
    ))
}

fn write_string(writer: &mut impl Write, value: &str, name: &str) -> ChunkStoreResult<()> {
    write_len(writer, value.len(), name)?;
    writer.write_all(value.as_bytes())?;
    Ok(())
}

fn read_string(reader: &mut impl Read) -> ChunkStoreResult<String> {
    let len = read_len(reader)?;
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| {
        ChunkStoreError::InvalidData(format!("scheduled tick target was not UTF-8: {error}"))
    })
}

fn read_bounded_string(
    reader: &mut impl Read,
    max_len: usize,
    name: &str,
) -> ChunkStoreResult<String> {
    let len = read_len(reader)?;
    if len > max_len {
        return Err(ChunkStoreError::InvalidData(format!(
            "{name} is {len} bytes; maximum is {max_len}"
        )));
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes)
        .map_err(|error| ChunkStoreError::InvalidData(format!("{name} was not UTF-8: {error}")))
}

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

fn read_optional_u32(reader: &mut impl Read) -> ChunkStoreResult<Option<u32>> {
    if read_bool(reader)? {
        Ok(Some(read_u32(reader)?))
    } else {
        Ok(None)
    }
}

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

fn write_light_section(
    writer: &mut impl Write,
    section: &PackedLightSection,
) -> ChunkStoreResult<()> {
    write_i32(writer, section.section_y)?;
    write_optional_light_layer(writer, &section.sky, "sky light layer")?;
    write_optional_light_layer(writer, &section.block, "block light layer")?;
    Ok(())
}

fn read_light_section(reader: &mut impl Read) -> ChunkStoreResult<PackedLightSection> {
    let section_y = read_i32(reader)?;
    let sky = read_optional_light_layer(reader)?;
    let block = read_optional_light_layer(reader)?;
    Ok(PackedLightSection::new(section_y, sky, block))
}

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

fn read_optional_light_layer(reader: &mut impl Read) -> ChunkStoreResult<Option<Vec<u8>>> {
    if !read_bool(reader)? {
        return Ok(None);
    }
    let mut bytes = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

fn write_len(writer: &mut impl Write, value: usize, name: &str) -> ChunkStoreResult<()> {
    let value = u32::try_from(value)
        .map_err(|_| ChunkStoreError::InvalidData(format!("{name} {value} does not fit in u32")))?;
    write_u32(writer, value)
}

fn read_len(reader: &mut impl Read) -> ChunkStoreResult<usize> {
    usize::try_from(read_u32(reader)?).map_err(|_| {
        ChunkStoreError::InvalidData("chunk snapshot length does not fit in usize".to_owned())
    })
}

fn status_to_u8(status: ChunkStatus) -> u8 {
    match status {
        ChunkStatus::Terrain => 0,
        ChunkStatus::Surface => 1,
        ChunkStatus::Features => 2,
        ChunkStatus::Light => 3,
        ChunkStatus::Full => 4,
    }
}

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

fn write_u8(writer: &mut impl Write, value: u8) -> ChunkStoreResult<()> {
    writer.write_all(&[value])?;
    Ok(())
}

fn read_u8(reader: &mut impl Read) -> ChunkStoreResult<u8> {
    let mut bytes = [0_u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn write_bool(writer: &mut impl Write, value: bool) -> ChunkStoreResult<()> {
    write_u8(writer, u8::from(value))
}

fn read_bool(reader: &mut impl Read) -> ChunkStoreResult<bool> {
    match read_u8(reader)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(ChunkStoreError::InvalidData(format!(
            "boolean value must be 0 or 1, got {value}"
        ))),
    }
}

fn write_u32(writer: &mut impl Write, value: u32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_u32(reader: &mut impl Read) -> ChunkStoreResult<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_i32(writer: &mut impl Write, value: i32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_i32(reader: &mut impl Read) -> ChunkStoreResult<i32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(i32::from_le_bytes(bytes))
}

fn write_u64(writer: &mut impl Write, value: u64) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_u64(reader: &mut impl Read) -> ChunkStoreResult<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_i64(writer: &mut impl Write, value: i64) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_i64(reader: &mut impl Read) -> ChunkStoreResult<i64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(i64::from_le_bytes(bytes))
}

fn write_f32(writer: &mut impl Write, value: f32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_f32(reader: &mut impl Read) -> ChunkStoreResult<f32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(f32::from_le_bytes(bytes))
}

fn write_f64(writer: &mut impl Write, value: f64) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_f64(reader: &mut impl Read) -> ChunkStoreResult<f64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(f64::from_le_bytes(bytes))
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
            scheduled_block_ticks: vec![ScheduledTickRecord::new(
                BlockPos::new(1, 64, 2),
                "minecraft:sand",
                2,
            )],
            scheduled_fluid_ticks: vec![ScheduledTickRecord::new(
                BlockPos::new(3, 63, 4),
                "minecraft:water",
                5,
            )],
        };
        let mut bytes = Vec::new();

        write_chunk_record(&mut bytes, &record).unwrap();
        let decoded = read_chunk_record(&mut bytes.as_slice()).unwrap();

        assert_eq!(decoded, record);
    }

    #[test]
    fn binary_entity_chunk_record_format_roundtrips_entities() {
        let record = EntityChunkRecord::new(
            ChunkPos::new(-7, 9),
            42,
            vec![
                test_entity_save_record(),
                EntitySaveRecord {
                    persistent_id: EntityPersistentId::new(0xABCD, 0x1234),
                    kind: "minecraft:chicken".to_owned(),
                    position: Vec3d::new(-1.0, 70.5, 3.25),
                    delta_movement: Vec3d::new(0.1, -0.2, 0.3),
                    y_rot_degrees: 45.0,
                    x_rot_degrees: -12.0,
                    rotation: Some(EntityRotation {
                        x: 0.0,
                        y: 0.5,
                        z: 0.0,
                        w: 0.8660254,
                    }),
                    on_ground: false,
                    payload: EntitySavePayload::Chicken { egg_time: 1234 },
                },
                EntitySaveRecord {
                    persistent_id: EntityPersistentId::new(0xABCD, 0x5678),
                    kind: "mclone:mannequin".to_owned(),
                    position: Vec3d::new(4.5, 64.0, 4.5),
                    delta_movement: Vec3d::ZERO,
                    y_rot_degrees: 90.0,
                    x_rot_degrees: 0.0,
                    rotation: None,
                    on_ground: true,
                    payload: EntitySavePayload::Mannequin,
                },
            ],
        );
        let mut bytes = Vec::new();

        write_entity_chunk_record(&mut bytes, &record).unwrap();
        let decoded = read_entity_chunk_record(&mut bytes.as_slice()).unwrap();

        assert_eq!(decoded, record);
    }

    #[test]
    fn binary_player_record_format_roundtrips_identity_pose_and_progress() {
        let record = test_player_record(43);

        let bytes = encode_player_record(&record).unwrap();
        let decoded = decode_player_record(&bytes).unwrap();

        assert_eq!(decoded, record);
    }

    #[test]
    fn dead_player_record_roundtrips_typed_pending_cause() {
        let mut record = test_player_record(44);
        record.health = 0.0;
        record.pending_death_cause = Some(PlayerDamageCause::Lava);

        let bytes = encode_player_record(&record).unwrap();
        let decoded = decode_player_record(&bytes).unwrap();

        assert_eq!(decoded, record);
    }

    #[test]
    fn legacy_player_record_migrates_with_empty_statistics() {
        let record = test_player_record(43);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(PLAYER_RECORD_MAGIC);
        write_u32(&mut bytes, LEGACY_PLAYER_RECORD_VERSION).unwrap();
        write_string(&mut bytes, record.player.as_str(), "player key").unwrap();
        write_u64(&mut bytes, record.revision).unwrap();
        write_string(&mut bytes, &record.last_known_name, "player name").unwrap();
        write_string(&mut bytes, record.dimension.as_str(), "player dimension").unwrap();
        write_vec3d(&mut bytes, record.position).unwrap();
        write_f32(&mut bytes, record.y_rot_degrees).unwrap();
        write_f32(&mut bytes, record.x_rot_degrees).unwrap();
        write_bool(&mut bytes, record.on_ground).unwrap();
        write_u8(&mut bytes, record.selected_hotbar_slot).unwrap();
        write_u64(&mut bytes, record.total_experience).unwrap();

        let decoded = decode_player_record(&bytes).unwrap();

        assert_eq!(decoded.codec_version, PLAYER_RECORD_VERSION);
        assert!(decoded.statistics.is_empty());
        assert_eq!(decoded.health, DEFAULT_PLAYER_MAX_HEALTH);
        assert_eq!(decoded.pending_death_cause, None);
        assert_eq!(decoded.player, record.player);
        assert_eq!(decoded.position, record.position);
        assert_eq!(decoded.total_experience, record.total_experience);
    }

    #[test]
    fn statistics_player_record_migrates_with_full_health() {
        let record = test_player_record(45);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(PLAYER_RECORD_MAGIC);
        write_u32(&mut bytes, STATISTICS_PLAYER_RECORD_VERSION).unwrap();
        write_string(&mut bytes, record.player.as_str(), "player key").unwrap();
        write_u64(&mut bytes, record.revision).unwrap();
        write_string(&mut bytes, &record.last_known_name, "player name").unwrap();
        write_string(&mut bytes, record.dimension.as_str(), "player dimension").unwrap();
        write_vec3d(&mut bytes, record.position).unwrap();
        write_f32(&mut bytes, record.y_rot_degrees).unwrap();
        write_f32(&mut bytes, record.x_rot_degrees).unwrap();
        write_bool(&mut bytes, record.on_ground).unwrap();
        write_u8(&mut bytes, record.selected_hotbar_slot).unwrap();
        write_u64(&mut bytes, record.total_experience).unwrap();
        write_player_statistics(&mut bytes, &record.statistics).unwrap();

        let decoded = decode_player_record(&bytes).unwrap();

        assert_eq!(decoded.codec_version, PLAYER_RECORD_VERSION);
        assert_eq!(decoded.statistics, record.statistics);
        assert_eq!(decoded.health, DEFAULT_PLAYER_MAX_HEALTH);
        assert_eq!(decoded.pending_death_cause, None);
    }

    #[test]
    fn player_record_codec_rejects_invalid_lifecycle_combinations() {
        let mut record = test_player_record(46);
        record.health = f32::NAN;
        assert!(encode_player_record(&record).is_err());

        record.health = 0.0;
        assert!(encode_player_record(&record).is_err());

        record.health = DEFAULT_PLAYER_MAX_HEALTH;
        record.pending_death_cause = Some(PlayerDamageCause::Lava);
        assert!(encode_player_record(&record).is_err());

        record.health = 0.0;
        let mut bytes = encode_player_record(&record).unwrap();
        *bytes.last_mut().unwrap() = 99;
        assert!(decode_player_record(&bytes).is_err());
    }

    #[test]
    fn binary_world_metadata_roundtrips_all_authoritative_facts() {
        let record = test_world_metadata(44);

        let bytes = encode_world_metadata(&record).unwrap();
        let decoded = decode_world_metadata(&bytes).unwrap();

        assert_eq!(decoded, record);
    }

    #[test]
    fn binary_world_generation_profile_discriminants_are_stable() {
        let mut overworld = Vec::new();
        write_world_generation_profile(&mut overworld, WorldGenerationProfile::Overworld).unwrap();
        assert_eq!(overworld, [0]);
        assert_eq!(
            read_world_generation_profile(&mut overworld.as_slice()).unwrap(),
            WorldGenerationProfile::Overworld
        );

        let mut authored_only = Vec::new();
        write_world_generation_profile(&mut authored_only, WorldGenerationProfile::authored_only())
            .unwrap();
        assert_eq!(authored_only, [1]);
        assert_eq!(
            read_world_generation_profile(&mut authored_only.as_slice()).unwrap(),
            WorldGenerationProfile::authored_only()
        );

        let mut flat_grass = Vec::new();
        write_world_generation_profile(&mut flat_grass, WorldGenerationProfile::FlatGrassV1)
            .unwrap();
        assert_eq!(flat_grass, [2]);
        assert_eq!(
            read_world_generation_profile(&mut flat_grass.as_slice()).unwrap(),
            WorldGenerationProfile::FlatGrassV1
        );

        let mut small_island = Vec::new();
        write_world_generation_profile(&mut small_island, WorldGenerationProfile::SmallIslandV1)
            .unwrap();
        assert_eq!(small_island, [3]);
        assert_eq!(
            read_world_generation_profile(&mut small_island.as_slice()).unwrap(),
            WorldGenerationProfile::SmallIslandV1
        );

        let mut mclone_overworld = Vec::new();
        write_world_generation_profile(
            &mut mclone_overworld,
            WorldGenerationProfile::McloneOverworldV1,
        )
        .unwrap();
        assert_eq!(mclone_overworld, [4]);
        assert_eq!(
            read_world_generation_profile(&mut mclone_overworld.as_slice()).unwrap(),
            WorldGenerationProfile::McloneOverworldV1
        );

        for (profile, tag) in [
            (WorldGenerationProfile::alpha_v1(false), 5),
            (WorldGenerationProfile::alpha_v1(true), 6),
            (WorldGenerationProfile::BetaV1, 7),
        ] {
            let mut encoded = Vec::new();
            write_world_generation_profile(&mut encoded, profile).unwrap();
            assert_eq!(encoded, [tag]);
            assert_eq!(
                read_world_generation_profile(&mut encoded.as_slice()).unwrap(),
                profile
            );
        }
        assert!(
            read_world_generation_profile(&mut [8].as_slice())
                .unwrap_err()
                .to_string()
                .contains("unknown world generation profile tag 8")
        );
    }

    #[test]
    fn binary_dimension_record_roundtrips_validated_definition() {
        let mut record = DimensionRecord::overworld(44, WorldGenerationProfile::Overworld);
        record.key = DimensionKey::parse("mclone:moon").unwrap();
        record.definition.coordinate_scale = 0.125;
        record.definition.has_sky_light = false;
        record.definition.topology = HorizontalTopology::cylinder_x(0, 32);

        let bytes = encode_dimension_record(&record).unwrap();

        assert_eq!(decode_dimension_record(&bytes).unwrap(), record);
    }

    #[test]
    fn binary_dimension_record_v1_defaults_to_unbounded_topology() {
        let mut legacy = Vec::new();
        legacy.extend_from_slice(DIMENSION_RECORD_MAGIC);
        write_u32(&mut legacy, 1).unwrap();
        write_string(
            &mut legacy,
            DimensionKey::overworld().as_str(),
            "dimension key",
        )
        .unwrap();
        write_u64(&mut legacy, 7).unwrap();
        write_i64(&mut legacy, 44).unwrap();
        write_world_generation_profile(&mut legacy, WorldGenerationProfile::FlatGrassV1).unwrap();
        write_i32(&mut legacy, 0).unwrap();
        write_i32(&mut legacy, 256).unwrap();
        write_f64(&mut legacy, 1.0).unwrap();
        write_bool(&mut legacy, true).unwrap();
        write_bool(&mut legacy, false).unwrap();
        write_bool(&mut legacy, false).unwrap();

        let decoded = decode_dimension_record(&legacy).unwrap();

        assert_eq!(decoded.codec_version, DIMENSION_RECORD_VERSION);
        assert_eq!(decoded.definition.topology, HorizontalTopology::UNBOUNDED);
    }

    #[test]
    fn binary_dimension_record_rejects_invalid_topology() {
        let mut record = DimensionRecord::overworld(44, WorldGenerationProfile::FlatGrassV1);
        record.definition.topology = HorizontalTopology::cylinder_x(0, 0);

        assert!(
            encode_dimension_record(&record)
                .unwrap_err()
                .to_string()
                .contains("zero period")
        );
    }

    #[test]
    fn binary_world_metadata_v1_decodes_with_legacy_realm_for_migration() {
        let current = encode_world_metadata(&test_world_metadata(45)).unwrap();
        let header_len = WORLD_METADATA_MAGIC.len() + std::mem::size_of::<u32>();
        let mut legacy = current[..header_len].to_vec();
        legacy[WORLD_METADATA_MAGIC.len()..header_len].copy_from_slice(&1_u32.to_le_bytes());
        legacy.extend_from_slice(&current[header_len + 16..]);

        let decoded = decode_world_metadata(&legacy).unwrap();

        assert_eq!(decoded.codec_version, 1);
        assert_eq!(decoded.realm_id, RealmId::LEGACY_SINGLE_REALM);
        assert_eq!(decoded.revision, 45);
    }

    #[test]
    fn binary_world_metadata_rejects_unknown_versions_and_trailing_bytes() {
        let mut unknown_version = encode_world_metadata(&test_world_metadata(1)).unwrap();
        unknown_version[WORLD_METADATA_MAGIC.len()] = 3;
        let error = decode_world_metadata(&unknown_version).unwrap_err();
        assert_eq!(error.kind(), PersistenceErrorKind::Incompatible);
        assert!(
            error
                .to_string()
                .contains("unsupported world metadata codec version 3")
        );

        let mut trailing = encode_world_metadata(&test_world_metadata(1)).unwrap();
        trailing.push(0xFF);
        assert!(
            decode_world_metadata(&trailing)
                .unwrap_err()
                .to_string()
                .contains("trailing bytes")
        );
    }

    #[test]
    fn actor_world_metadata_save_is_immediately_visible() {
        let mut mailbox = PersistenceMailbox::memory();
        let record = test_world_metadata(7);

        assert_eq!(
            mailbox
                .save_world_metadata_blocking(record.clone())
                .unwrap(),
            StoreWriteOutcome::Written
        );
        assert_eq!(
            mailbox.load_world_metadata_blocking().unwrap(),
            WorldMetadataLoad {
                record: Some(record),
                legacy_records_present: false,
            }
        );
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
    fn actor_load_sees_pending_same_entity_chunk_write() {
        let mut mailbox = PersistenceMailbox::memory();
        let pos = ChunkPos::new(2, 4);
        let record = test_entity_chunk_record(pos, 11);

        mailbox.save_entity_chunk(record.clone(), SaveDurability::Durable);
        let load_id = mailbox.load_entity_chunk(pos);

        assert_eq!(
            take_loaded_entity_chunk(&mut mailbox, load_id),
            Some(record)
        );
    }

    #[test]
    fn external_load_backend_emits_chunk_load_request_and_caches_completion() {
        let mut mailbox = PersistenceMailbox::external_loads(Box::<MemoryWorldStore>::default());
        let pos = ChunkPos::new(3, -5);
        let record = test_record(pos, 13);

        let load_id = mailbox.load_chunk(pos);
        assert!(mailbox.take_completion(load_id).is_none());
        assert_eq!(mailbox.pending_external_request_count(), 1);
        let requests = mailbox.drain_external_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(mailbox.pending_external_request_count(), 1);
        assert!(matches!(
            requests.first(),
            Some(WorldStoreRequest::LoadChunk {
                request_id,
                pos: request_pos,
                ..
            }) if *request_id == load_id && *request_pos == pos
        ));

        mailbox
            .complete_external_request(WorldStoreCompletion::ChunkLoaded {
                request_id: load_id,
                dimension: DimensionKey::overworld(),
                pos,
                result: Ok(Some(record.clone())),
            })
            .unwrap();
        assert_eq!(
            take_loaded_chunk(&mut mailbox, load_id),
            Some(record.clone())
        );

        let cached_load_id = mailbox.load_chunk(pos);
        assert_eq!(
            take_loaded_chunk(&mut mailbox, cached_load_id),
            Some(record)
        );
        assert!(mailbox.drain_external_requests().is_empty());
    }

    #[test]
    fn scoped_mailbox_automatically_qualifies_dimension_local_requests() {
        let moon = DimensionKey::parse("mclone:moon").unwrap();
        let pos = ChunkPos::new(0, 0);
        let mut mailbox = PersistenceMailbox::external_loads_in_dimension(
            moon.clone(),
            Box::<MemoryWorldStore>::default(),
        );

        let request_id = mailbox.load_chunk(pos);
        let requests = mailbox.drain_external_requests();

        assert_eq!(mailbox.dimension(), &moon);
        assert!(matches!(
            requests.as_slice(),
            [WorldStoreRequest::LoadChunk {
                request_id: actual_id,
                dimension,
                pos: actual_pos,
            }] if *actual_id == request_id && dimension == &moon && *actual_pos == pos
        ));
    }

    #[test]
    fn scoped_mailboxes_share_one_actor_without_crossing_completions() {
        let moon = DimensionKey::parse("mclone:moon").unwrap();
        let pos = ChunkPos::new(0, 0);
        let mut overworld = PersistenceMailbox::memory();
        let mut moon_mailbox = overworld.scoped_to_dimension(moon.clone());
        let overworld_id = overworld.save_chunk(test_record(pos, 1), SaveDurability::Durable);
        let moon_id = moon_mailbox.save_chunk(test_record(pos, 2), SaveDurability::Durable);

        overworld.process_all_background_writes();
        let moon_completions = moon_mailbox.drain_completions();
        let overworld_completions = overworld.drain_completions();

        assert!(matches!(
            moon_completions.as_slice(),
            [WorldStoreCompletion::ChunkSaved {
                request_id,
                dimension,
                ..
            }] if *request_id == moon_id && dimension == &moon
        ));
        assert!(matches!(
            overworld_completions.as_slice(),
            [WorldStoreCompletion::ChunkSaved {
                request_id,
                dimension,
                ..
            }] if *request_id == overworld_id && dimension == &DimensionKey::overworld()
        ));
        assert_eq!(
            overworld
                .load_chunk_blocking(pos)
                .unwrap()
                .unwrap()
                .revision(),
            ChunkRevision(1)
        );
        assert_eq!(
            moon_mailbox
                .load_chunk_blocking(pos)
                .unwrap()
                .unwrap()
                .revision(),
            ChunkRevision(2)
        );
    }

    #[test]
    fn external_load_backend_emits_entity_chunk_load_request_and_caches_completion() {
        let mut mailbox = PersistenceMailbox::external_loads(Box::<MemoryWorldStore>::default());
        let pos = ChunkPos::new(-3, 5);
        let record = test_entity_chunk_record(pos, 17);

        let load_id = mailbox.load_entity_chunk(pos);
        let requests = mailbox.drain_external_requests();
        assert_eq!(requests.len(), 1);
        assert!(matches!(
            requests.first(),
            Some(WorldStoreRequest::LoadEntityChunk {
                request_id,
                pos: request_pos,
                ..
            }) if *request_id == load_id && *request_pos == pos
        ));

        mailbox
            .complete_external_request(WorldStoreCompletion::EntityChunkLoaded {
                request_id: load_id,
                dimension: DimensionKey::overworld(),
                pos,
                result: Ok(Some(record.clone())),
            })
            .unwrap();
        assert_eq!(
            take_loaded_entity_chunk(&mut mailbox, load_id),
            Some(record.clone())
        );

        let cached_load_id = mailbox.load_entity_chunk(pos);
        assert_eq!(
            take_loaded_entity_chunk(&mut mailbox, cached_load_id),
            Some(record)
        );
        assert!(mailbox.drain_external_requests().is_empty());
    }

    #[test]
    fn external_load_backend_preserves_save_revision_precedence() {
        let mut mailbox = PersistenceMailbox::external_loads(Box::<MemoryWorldStore>::default());
        let pos = ChunkPos::new(4, -6);
        let durable = test_record(pos, 10);
        let durable_id = mailbox.save_chunk(durable.clone(), SaveDurability::Durable);
        let stale_id = mailbox.save_chunk(test_record(pos, 9), SaveDurability::Cache);

        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        assert_eq!(
            take_saved_chunk(&mut mailbox, stale_id),
            StoreWriteOutcome::Superseded
        );
        let load_id = mailbox.load_chunk(pos);
        assert_eq!(take_loaded_chunk(&mut mailbox, load_id), Some(durable));
        assert!(mailbox.drain_external_requests().is_empty());
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
    fn actor_newer_entity_revision_supersedes_stale_queued_write() {
        let mut mailbox = PersistenceMailbox::memory();
        let pos = ChunkPos::new(4, 6);
        let stale_id =
            mailbox.save_entity_chunk(test_entity_chunk_record(pos, 1), SaveDurability::Durable);
        let newer = test_entity_chunk_record(pos, 2);
        let newer_id = mailbox.save_entity_chunk(newer.clone(), SaveDurability::Cache);

        assert_eq!(
            take_saved_entity_chunk(&mut mailbox, stale_id),
            StoreWriteOutcome::Superseded
        );
        assert!(mailbox.process_one_background_write());
        assert_eq!(
            take_saved_entity_chunk(&mut mailbox, newer_id),
            StoreWriteOutcome::Written
        );
        let load_id = mailbox.load_entity_chunk(pos);
        assert_eq!(take_loaded_entity_chunk(&mut mailbox, load_id), Some(newer));
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
        let durable_entity_pos = ChunkPos::new(12, 1);
        let cache_entity_pos = ChunkPos::new(13, 1);
        let durable_id = mailbox.save_chunk(test_record(durable_pos, 1), SaveDurability::Durable);
        let cache_id = mailbox.save_chunk(test_record(cache_pos, 1), SaveDurability::Cache);
        let durable_entity_id = mailbox.save_entity_chunk(
            test_entity_chunk_record(durable_entity_pos, 1),
            SaveDurability::Durable,
        );
        let cache_entity_id = mailbox.save_entity_chunk(
            test_entity_chunk_record(cache_entity_pos, 1),
            SaveDurability::Cache,
        );
        let flush_id = mailbox.flush();

        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        assert_eq!(
            take_saved_entity_chunk(&mut mailbox, durable_entity_id),
            StoreWriteOutcome::Written
        );
        assert!(mailbox.take_completion(cache_id).is_none());
        assert!(mailbox.take_completion(cache_entity_id).is_none());
        take_flush_complete(&mut mailbox, flush_id);
        let durable_load_id = mailbox.load_chunk(durable_pos);
        assert_eq!(
            take_loaded_chunk(&mut mailbox, durable_load_id)
                .unwrap()
                .revision(),
            ChunkRevision(1)
        );
        let durable_entity_load_id = mailbox.load_entity_chunk(durable_entity_pos);
        assert_eq!(
            take_loaded_entity_chunk(&mut mailbox, durable_entity_load_id)
                .unwrap()
                .revision,
            1
        );
        let cache_load_id = mailbox.load_chunk(cache_pos);
        assert_eq!(
            take_loaded_chunk(&mut mailbox, cache_load_id)
                .unwrap()
                .revision(),
            ChunkRevision(1),
            "loads still see the pending cache write before it reaches the backend"
        );
        let cache_entity_load_id = mailbox.load_entity_chunk(cache_entity_pos);
        assert_eq!(
            take_loaded_entity_chunk(&mut mailbox, cache_entity_load_id)
                .unwrap()
                .revision,
            1,
            "entity loads still see the pending cache write before it reaches the backend"
        );
    }

    #[test]
    fn actor_close_drains_durable_writes_and_skips_cache_writes() {
        let mut mailbox = PersistenceMailbox::memory();
        let durable_pos = ChunkPos::new(14, 0);
        let cache_pos = ChunkPos::new(15, 0);
        let durable_entity_pos = ChunkPos::new(14, 1);
        let cache_entity_pos = ChunkPos::new(15, 1);
        let durable_id = mailbox.save_chunk(test_record(durable_pos, 3), SaveDurability::Durable);
        let cache_id = mailbox.save_chunk(test_record(cache_pos, 3), SaveDurability::Cache);
        let durable_entity_id = mailbox.save_entity_chunk(
            test_entity_chunk_record(durable_entity_pos, 3),
            SaveDurability::Durable,
        );
        let cache_entity_id = mailbox.save_entity_chunk(
            test_entity_chunk_record(cache_entity_pos, 3),
            SaveDurability::Cache,
        );
        let close_id = mailbox.close();

        assert_eq!(
            take_saved_chunk(&mut mailbox, durable_id),
            StoreWriteOutcome::Written
        );
        assert_eq!(
            take_saved_entity_chunk(&mut mailbox, durable_entity_id),
            StoreWriteOutcome::Written
        );
        assert_eq!(
            take_saved_chunk(&mut mailbox, cache_id),
            StoreWriteOutcome::SkippedOnClose
        );
        assert_eq!(
            take_saved_entity_chunk(&mut mailbox, cache_entity_id),
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
    fn threaded_mailbox_load_sees_prior_pending_write() {
        let mut mailbox = PersistenceMailbox::threaded(Box::<MemoryWorldStore>::default()).unwrap();
        let pos = ChunkPos::new(18, 0);
        let record = test_record(pos, 4);
        let save_id = mailbox.save_chunk(record.clone(), SaveDurability::Durable);
        let load_id = mailbox.load_chunk(pos);

        assert_eq!(take_loaded_chunk_wait(&mut mailbox, load_id), Some(record));
        assert_eq!(
            take_saved_chunk_wait(&mut mailbox, save_id),
            StoreWriteOutcome::Written
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn threaded_mailbox_does_not_block_host_on_slow_store_write() {
        let (save_started_sender, save_started_receiver) = mpsc::channel();
        let (save_release_sender, save_release_receiver) = mpsc::channel();
        let store = ReleasableWorldStore::new(save_started_sender, save_release_receiver);
        let mut mailbox = PersistenceMailbox::threaded(Box::new(store)).unwrap();
        let save_id = mailbox.save_chunk(
            test_record(ChunkPos::new(19, 0), 5),
            SaveDurability::Durable,
        );

        save_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("background store write should start");
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            save_release_sender
                .send(())
                .expect("test should release the store write");
        });
        let started = std::time::Instant::now();

        assert!(mailbox.process_one_background_write());
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "host-side persistence polling should not wait for the store write to finish"
        );

        release.join().unwrap();
        assert_eq!(
            take_saved_chunk_wait(&mut mailbox, save_id),
            StoreWriteOutcome::Written
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn threaded_mailbox_close_drains_durable_writes() {
        let mut mailbox = PersistenceMailbox::threaded(Box::<MemoryWorldStore>::default()).unwrap();
        let pos = ChunkPos::new(20, 0);
        let save_id = mailbox.save_chunk(test_record(pos, 6), SaveDurability::Durable);
        let close_id = mailbox.close();

        assert_eq!(
            take_saved_chunk_wait(&mut mailbox, save_id),
            StoreWriteOutcome::Written
        );
        take_close_complete_wait(&mut mailbox, close_id);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_writer_lease_is_world_scoped_and_close_releases_it() {
        let parent = unique_temp_dir("sqlite_writer_lease_is_world_scoped");
        let first_world = parent.join("first");
        let second_world = parent.join("second");
        let mut first = SqliteWorldStore::open_world_dir(&first_world).unwrap();

        let conflict = SqliteWorldStore::open_world_dir(&first_world).unwrap_err();
        assert_eq!(conflict.kind(), PersistenceErrorKind::LeaseConflict);

        let mut second = SqliteWorldStore::open_world_dir(&second_world).unwrap();
        second.close().unwrap();

        let mut reader = SqliteWorldStore::open_world_dir_read_only(&first_world).unwrap();
        assert!(reader.is_read_only());
        assert_eq!(reader.load_world_metadata().unwrap().record, None);
        let save_error = reader
            .save_world_metadata(&test_world_metadata(1))
            .unwrap_err();
        assert_eq!(save_error.kind(), PersistenceErrorKind::Unavailable);
        reader.close().unwrap();

        first.close().unwrap();
        fs::write(
            first_world.join(WORLD_WRITER_LOCK_FILE),
            b"stale diagnostics",
        )
        .unwrap();
        let mut reopened = SqliteWorldStore::open_world_dir(&first_world).unwrap();
        reopened.close().unwrap();

        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_exclusive_delete_rejects_live_writer_and_removes_after_close() {
        let parent = unique_temp_dir("sqlite_exclusive_delete");
        let world = parent.join("world");
        let mut store = SqliteWorldStore::open_world_dir(&world).unwrap();

        let conflict = SqliteWorldStore::remove_world_dir_exclusive(&world).unwrap_err();
        assert_eq!(conflict.kind(), PersistenceErrorKind::LeaseConflict);
        assert!(world.is_dir());

        store.close().unwrap();
        SqliteWorldStore::remove_world_dir_exclusive(&world).unwrap();
        assert!(!world.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_writer_lease_releases_after_subprocess_exit() {
        const CHILD_WORLD_ENV: &str = "MCLONE_SQLITE_LEASE_CRASH_WORLD";
        if let Some(world) = std::env::var_os(CHILD_WORLD_ENV) {
            let _store = SqliteWorldStore::open_world_dir(PathBuf::from(world)).unwrap();
            std::process::exit(73);
        }

        let parent = unique_temp_dir("sqlite_writer_lease_subprocess_exit");
        let world = parent.join("world");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("persistence::tests::sqlite_writer_lease_releases_after_subprocess_exit")
            .arg("--nocapture")
            .env(CHILD_WORLD_ENV, &world)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73));

        let mut reopened = SqliteWorldStore::open_world_dir(&world).unwrap();
        reopened.close().unwrap();
        fs::remove_dir_all(parent).unwrap();
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_world_store_roundtrips_world_records_across_reopen() {
        let root = unique_temp_dir("sqlite_world_store_roundtrips_records");
        let path = root.join("world.sqlite3");
        let chunk_pos = ChunkPos::new(21, -3);
        let entity_pos = ChunkPos::new(21, -2);
        let chunk = test_record(chunk_pos, 14);
        let entity_chunk = test_entity_chunk_record(entity_pos, 15);
        let player = test_player_record(16);
        let metadata = test_world_metadata(17);
        let overworld = DimensionKey::overworld();

        {
            let mut store = SqliteWorldStore::new(&path).unwrap();
            assert!(store.supports_entity_chunks());
            store.save_chunk(&overworld, &chunk).unwrap();
            store.save_entity_chunk(&overworld, &entity_chunk).unwrap();
            store.save_player(&player).unwrap();
            assert_eq!(
                store.load_world_metadata().unwrap(),
                WorldMetadataLoad {
                    record: None,
                    legacy_records_present: true,
                }
            );
            store.save_world_metadata(&metadata).unwrap();
            store.flush().unwrap();
            store.close().unwrap();
        }

        {
            let mut reopened = SqliteWorldStore::new(&path).unwrap();
            assert_eq!(reopened.path(), path.as_path());
            assert_eq!(
                reopened.load_chunk(&overworld, chunk_pos).unwrap(),
                Some(chunk)
            );
            assert_eq!(
                reopened.load_entity_chunk(&overworld, entity_pos).unwrap(),
                Some(entity_chunk)
            );
            assert_eq!(reopened.load_player(&player.player).unwrap(), Some(player));
            assert_eq!(
                reopened.load_world_metadata().unwrap(),
                WorldMetadataLoad {
                    record: Some(metadata),
                    legacy_records_present: false,
                }
            );
            assert_eq!(
                reopened
                    .load_chunk(&overworld, ChunkPos::new(99, 99))
                    .unwrap(),
                None
            );
            assert_eq!(
                reopened
                    .load_entity_chunk(&overworld, ChunkPos::new(99, 99))
                    .unwrap(),
                None
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_dimension_keys_isolate_identical_chunk_coordinates() {
        let root = unique_temp_dir("sqlite-dimension-collision");
        let path = root.join("world.sqlite3");
        let overworld = DimensionKey::overworld();
        let moon = DimensionKey::parse("mclone:moon").unwrap();
        let pos = ChunkPos::new(0, 0);
        let overworld_chunk = test_record(pos, 31);
        let moon_chunk = test_record(pos, 32);
        let overworld_entities = test_entity_chunk_record(pos, 41);
        let moon_entities = test_entity_chunk_record(pos, 42);

        {
            let mut store = SqliteWorldStore::new(&path).unwrap();
            store.save_chunk(&overworld, &overworld_chunk).unwrap();
            store.save_chunk(&moon, &moon_chunk).unwrap();
            store
                .save_entity_chunk(&overworld, &overworld_entities)
                .unwrap();
            store.save_entity_chunk(&moon, &moon_entities).unwrap();
        }

        let mut reopened = SqliteWorldStore::new(&path).unwrap();
        assert_eq!(
            reopened.load_chunk(&overworld, pos).unwrap(),
            Some(overworld_chunk)
        );
        assert_eq!(reopened.load_chunk(&moon, pos).unwrap(), Some(moon_chunk));
        assert_eq!(
            reopened.load_entity_chunk(&overworld, pos).unwrap(),
            Some(overworld_entities)
        );
        assert_eq!(
            reopened.load_entity_chunk(&moon, pos).unwrap(),
            Some(moon_entities)
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sqlite_v1_fixture_migrates_unqualified_records_to_overworld() {
        let root = unique_temp_dir("sqlite-v1-realm-dimension-fixture");
        let path = root.join("world.sqlite3");
        let chunk = test_record(ChunkPos::new(-31, 47), 21);
        let entity_chunk = test_entity_chunk_record(ChunkPos::new(-31, 48), 22);
        let player = test_player_record(23);
        let metadata = test_world_metadata(24);
        let overworld = DimensionKey::overworld();

        {
            fs::create_dir_all(&root).unwrap();
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE metadata (
                        key TEXT PRIMARY KEY,
                        value TEXT NOT NULL
                     );
                     CREATE TABLE world_metadata (
                        singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
                        codec_version INTEGER NOT NULL,
                        revision TEXT NOT NULL,
                        record_blob BLOB NOT NULL
                     );
                     CREATE TABLE chunk_records (
                        x INTEGER NOT NULL,
                        z INTEGER NOT NULL,
                        codec_version INTEGER NOT NULL,
                        revision TEXT NOT NULL,
                        record_blob BLOB NOT NULL,
                        PRIMARY KEY (x, z)
                     );
                     CREATE TABLE entity_chunk_records (
                        x INTEGER NOT NULL,
                        z INTEGER NOT NULL,
                        codec_version INTEGER NOT NULL,
                        revision TEXT NOT NULL,
                        record_blob BLOB NOT NULL,
                        PRIMARY KEY (x, z)
                     );
                     CREATE TABLE player_records (
                        player_key TEXT PRIMARY KEY,
                        codec_version INTEGER NOT NULL,
                        revision TEXT NOT NULL,
                        record_blob BLOB NOT NULL
                     );
                     CREATE TABLE saved_data_records (
                        data_key TEXT PRIMARY KEY,
                        codec_version INTEGER NOT NULL,
                        revision TEXT NOT NULL,
                        record_blob BLOB NOT NULL
                     );
                     INSERT INTO metadata (key, value) VALUES ('schema_version', '1');
                     PRAGMA user_version = 1;",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO world_metadata
                        (singleton_id, codec_version, revision, record_blob)
                     VALUES (1, ?1, ?2, ?3)",
                    params![
                        metadata.codec_version,
                        metadata.revision.to_string(),
                        encode_world_metadata(&metadata).unwrap()
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO chunk_records
                        (x, z, codec_version, revision, record_blob)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        chunk.pos().x,
                        chunk.pos().z,
                        SNAPSHOT_FORMAT_VERSION,
                        chunk.revision().0.to_string(),
                        encode_chunk_record(&chunk).unwrap()
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO entity_chunk_records
                        (x, z, codec_version, revision, record_blob)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        entity_chunk.pos.x,
                        entity_chunk.pos.z,
                        entity_chunk.codec_version,
                        entity_chunk.revision.to_string(),
                        encode_entity_chunk_record(&entity_chunk).unwrap()
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO player_records
                        (player_key, codec_version, revision, record_blob)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        player.player.as_str(),
                        player.codec_version,
                        player.revision.to_string(),
                        encode_player_record(&player).unwrap()
                    ],
                )
                .unwrap();
        }

        {
            let mut reopened = SqliteWorldStore::new(&path).unwrap();
            assert_eq!(
                reopened.load_chunk(&overworld, chunk.pos()).unwrap(),
                Some(chunk)
            );
            assert_eq!(
                reopened
                    .load_entity_chunk(&overworld, entity_chunk.pos)
                    .unwrap(),
                Some(entity_chunk)
            );
            assert_eq!(reopened.load_player(&player.player).unwrap(), Some(player));
            assert_eq!(
                reopened.load_world_metadata().unwrap().record,
                Some(metadata)
            );

            let connection = reopened.inner.executor().connection().unwrap();
            let mut statement = connection
                .prepare("PRAGMA table_info(chunk_records)")
                .unwrap();
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(
                columns,
                [
                    "dimension_key",
                    "x",
                    "z",
                    "codec_version",
                    "revision",
                    "record_blob"
                ]
            );
            let schema_version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(schema_version, SQLITE_WORLD_SCHEMA_VERSION);
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn threaded_sqlite_world_store_persists_durable_records_across_reopen() {
        let root = unique_temp_dir("threaded_sqlite_world_store_persists_records");
        let path = root.join("world.sqlite3");
        let chunk_pos = ChunkPos::new(22, 3);
        let entity_pos = ChunkPos::new(22, 4);
        let chunk = test_record(chunk_pos, 16);
        let entity_chunk = test_entity_chunk_record(entity_pos, 17);
        let player = test_player_record(18);
        let metadata = test_world_metadata(19);
        let overworld = DimensionKey::overworld();

        {
            let store = SqliteWorldStore::new(&path).unwrap();
            let mut mailbox = PersistenceMailbox::threaded(Box::new(store)).unwrap();
            let chunk_save_id = mailbox.save_chunk(chunk.clone(), SaveDurability::Durable);
            let entity_save_id =
                mailbox.save_entity_chunk(entity_chunk.clone(), SaveDurability::Durable);
            let player_save_id = mailbox.save_player(player.clone());
            let metadata_save_id = mailbox.save_world_metadata(metadata.clone());
            let close_id = mailbox.close();

            assert_eq!(
                take_saved_chunk_wait(&mut mailbox, chunk_save_id),
                StoreWriteOutcome::Written
            );
            assert_eq!(
                take_saved_entity_chunk_wait(&mut mailbox, entity_save_id),
                StoreWriteOutcome::Written
            );
            assert_eq!(
                take_saved_player_wait(&mut mailbox, player_save_id),
                StoreWriteOutcome::Written
            );
            assert_eq!(
                take_saved_world_metadata_wait(&mut mailbox, metadata_save_id),
                StoreWriteOutcome::Written
            );
            take_close_complete_wait(&mut mailbox, close_id);
        }

        {
            let mut reopened = SqliteWorldStore::new(&path).unwrap();
            assert_eq!(
                reopened.load_chunk(&overworld, chunk_pos).unwrap(),
                Some(chunk)
            );
            assert_eq!(
                reopened.load_entity_chunk(&overworld, entity_pos).unwrap(),
                Some(entity_chunk)
            );
            assert_eq!(reopened.load_player(&player.player).unwrap(), Some(player));
            assert_eq!(
                reopened.load_world_metadata().unwrap().record,
                Some(metadata)
            );
        }

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

    fn test_entity_chunk_record(pos: ChunkPos, revision: u64) -> EntityChunkRecord {
        EntityChunkRecord::new(pos, revision, vec![test_entity_save_record()])
    }

    fn test_entity_save_record() -> EntitySaveRecord {
        EntitySaveRecord {
            persistent_id: EntityPersistentId::new(0xCAFE, 0xF00D),
            kind: "minecraft:item".to_owned(),
            position: Vec3d::new(4.5, 64.0, 8.5),
            delta_movement: Vec3d::ZERO,
            y_rot_degrees: 10.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            payload: EntitySavePayload::Item {
                stack: ItemStackSaveRecord::new("minecraft:egg", 3),
                age: 12,
                pickup_delay: 7,
            },
        }
    }

    fn test_player_record(revision: u64) -> PlayerRecord {
        PlayerRecord {
            player: PlayerRecordKey::Uuid("00112233-4455-6677-8899-aabbccddeeff".to_owned()),
            codec_version: PLAYER_RECORD_VERSION,
            revision,
            last_known_name: "Builder".to_owned(),
            dimension: DimensionKey::overworld(),
            position: Vec3d::new(12.25, 78.5, -44.75),
            y_rot_degrees: 123.0,
            x_rot_degrees: -12.5,
            on_ground: false,
            selected_hotbar_slot: 4,
            total_experience: 987,
            statistics: {
                let mut statistics = PlayerStatistics::default();
                statistics.set(StatisticKey::jump(), 123);
                statistics.set(StatisticKey::successful_block_placement(), 45);
                statistics
            },
            health: 13.5,
            pending_death_cause: None,
        }
    }

    fn test_world_metadata(revision: u64) -> WorldMetadata {
        WorldMetadata {
            codec_version: WORLD_METADATA_VERSION,
            realm_id: RealmId::new([0x5a; 16]).unwrap(),
            revision,
            target_minecraft_version: WORLD_METADATA_TARGET_MINECRAFT_VERSION.to_owned(),
            seed: -9_223_372_036_854_775,
            world_generation_profile: WorldGenerationProfile::authored_only(),
            world_behavior_profile: WorldBehaviorProfile::ProtectedLobby,
            created_unix_millis: 1_784_203_200_000,
            last_played_unix_millis: 1_784_203_260_000,
            game_time: 98_765,
            day_time: 54_321,
            do_daylight_cycle: false,
        }
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

    fn take_loaded_entity_chunk(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> Option<EntityChunkRecord> {
        match mailbox
            .take_completion(request_id)
            .expect("missing entity chunk load completion")
        {
            WorldStoreCompletion::EntityChunkLoaded { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    fn take_saved_chunk(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        take_save_result(mailbox, request_id).unwrap()
    }

    fn take_saved_entity_chunk(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        match mailbox
            .take_completion(request_id)
            .expect("missing entity chunk save completion")
        {
            WorldStoreCompletion::EntityChunkSaved { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
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
    fn take_loaded_chunk_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> Option<ChunkRecord> {
        match take_completion_wait(mailbox, request_id) {
            WorldStoreCompletion::ChunkLoaded { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn take_saved_chunk_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        match take_completion_wait(mailbox, request_id) {
            WorldStoreCompletion::ChunkSaved { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn take_saved_entity_chunk_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        match take_completion_wait(mailbox, request_id) {
            WorldStoreCompletion::EntityChunkSaved { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn take_saved_player_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        match take_completion_wait(mailbox, request_id) {
            WorldStoreCompletion::PlayerSaved { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn take_saved_world_metadata_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> StoreWriteOutcome {
        match take_completion_wait(mailbox, request_id) {
            WorldStoreCompletion::WorldMetadataSaved { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn take_close_complete_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) {
        match take_completion_wait(mailbox, request_id) {
            WorldStoreCompletion::CloseComplete { result, .. } => result.unwrap(),
            completion => panic!("unexpected completion: {completion:?}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn take_completion_wait(
        mailbox: &mut PersistenceMailbox,
        request_id: PersistenceRequestId,
    ) -> WorldStoreCompletion {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(completion) = mailbox.take_completion(request_id) {
                return completion;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for persistence request {request_id}"
            );
            let _ = mailbox.process_one_background_write();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    struct ReleasableWorldStore {
        inner: MemoryWorldStore,
        save_started_sender: mpsc::Sender<()>,
        save_release_receiver: mpsc::Receiver<()>,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl ReleasableWorldStore {
        fn new(
            save_started_sender: mpsc::Sender<()>,
            save_release_receiver: mpsc::Receiver<()>,
        ) -> Self {
            Self {
                inner: MemoryWorldStore::new(),
                save_started_sender,
                save_release_receiver,
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl fmt::Debug for ReleasableWorldStore {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ReleasableWorldStore")
                .finish_non_exhaustive()
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl WorldStore for ReleasableWorldStore {
        fn supports_entity_chunks(&self) -> bool {
            self.inner.supports_entity_chunks()
        }

        fn load_chunk(
            &mut self,
            dimension: &DimensionKey,
            pos: ChunkPos,
        ) -> ChunkStoreResult<Option<ChunkRecord>> {
            self.inner.load_chunk(dimension, pos)
        }

        fn save_chunk(
            &mut self,
            dimension: &DimensionKey,
            record: &ChunkRecord,
        ) -> ChunkStoreResult<()> {
            let _ = self.save_started_sender.send(());
            self.save_release_receiver
                .recv_timeout(Duration::from_secs(1))
                .map_err(|error| {
                    ChunkStoreError::InvalidData(format!(
                        "test store write was not released: {error}"
                    ))
                })?;
            self.inner.save_chunk(dimension, record)
        }

        fn load_entity_chunk(
            &mut self,
            dimension: &DimensionKey,
            pos: ChunkPos,
        ) -> ChunkStoreResult<Option<EntityChunkRecord>> {
            self.inner.load_entity_chunk(dimension, pos)
        }

        fn save_entity_chunk(
            &mut self,
            dimension: &DimensionKey,
            record: &EntityChunkRecord,
        ) -> ChunkStoreResult<()> {
            self.inner.save_entity_chunk(dimension, record)
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
