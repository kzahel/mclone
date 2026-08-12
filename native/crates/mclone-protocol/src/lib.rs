#![forbid(unsafe_code)]

mod ecology;
mod ephemeral;
mod player_lifecycle;
mod realm_dimension;
mod statistics;

use std::error::Error;
use std::fmt;

use mclone_core::{
    AnimationClipId, AnimationPhaseSource, AnimationState, AxisTopology, BlockHitResult, BlockPos,
    BlockStateId, CHUNK_WIDTH, ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus, Direction,
    HorizontalTopology, LIGHT_DATA_LAYER_BYTE_COUNT, MAX_ANIMATION_CLIP_ID_BYTES,
    PackedChunkSection, PackedLightSection, SECTION_HEIGHT, Vec3d,
};

pub use ecology::{
    MALLARD_FIELD_GUIDE_OBSERVATION_COUNT, MallardCallCue, MallardFieldGuideProgress,
    MallardLifeStage, MallardNestSnapshotData, MallardNestUpdateData, MallardObservationKind,
    MallardSnapshotData, MallardTrackCue, MallardUpdateData,
};
pub use ephemeral::{
    ClientEphemeralMessage, EffectiveEphemeralTransport, MAX_EPHEMERAL_MESSAGE_BYTES,
    PlayerBodyPoseSample, RemotePlayerBodyPoseSample, ServerEphemeralMessage,
    decode_client_ephemeral_message, decode_server_ephemeral_message,
    encode_client_ephemeral_message, encode_server_ephemeral_message, sequence_is_newer,
    validate_body_pose_sample,
};
pub use player_lifecycle::{
    DEFAULT_PLAYER_MAX_HEALTH, PLAYER_STANDING_HEIGHT, PLAYER_STANDING_WIDTH, PlayerDamageCause,
    PlayerLifeState, PlayerLifeStateError, PlayerVitals, PlayerVitalsError,
};
pub use realm_dimension::{
    DimensionChunkPos, DimensionKey, DimensionKeyError, MAX_DIMENSION_KEY_BYTES,
    OVERWORLD_DIMENSION_KEY, RealmId, RealmIdError,
};
pub use statistics::{
    CUSTOM_STATISTIC_TYPE_KEY, DEATHS_STATISTIC_VALUE_KEY, JUMP_STATISTIC_VALUE_KEY,
    MAX_PLAYER_STATISTIC_ENTRIES, MAX_PLAYER_STATISTIC_VALUE, MAX_STATISTIC_RESOURCE_KEY_BYTES,
    MCLONE_CUSTOM_STATISTIC_TYPE_KEY, PlayerStatistics,
    SUCCESSFUL_BLOCK_PLACEMENT_STATISTIC_VALUE_KEY, StatisticKey, StatisticKeyError,
};

pub const PROTOCOL_VERSION: u32 = 36;
pub const HOTBAR_SLOT_COUNT: u8 = 9;
pub const HOTBAR_SLOT_COUNT_USIZE: usize = HOTBAR_SLOT_COUNT as usize;
pub const MAX_PLAYER_DISPLAY_NAME_BYTES: usize = 16;
pub const MAX_DISCONNECT_DETAIL_BYTES: usize = 512;
pub const DEFAULT_DEBUG_HOTBAR: [Option<DebugHotbarItem>; HOTBAR_SLOT_COUNT_USIZE] = [
    Some(DebugHotbarItem::Block(BlockStateId(1))),
    Some(DebugHotbarItem::Block(BlockStateId(5))),
    Some(DebugHotbarItem::Block(BlockStateId(4))),
    Some(DebugHotbarItem::Block(BlockStateId(6))),
    Some(DebugHotbarItem::Block(BlockStateId(41))),
    Some(DebugHotbarItem::Block(BlockStateId(42))),
    Some(DebugHotbarItem::Block(BlockStateId(8))),
    Some(DebugHotbarItem::SpawnActor(DebugActorKind::Chicken)),
    Some(DebugHotbarItem::SpawnActor(DebugActorKind::Mannequin)),
];

const CLIENT_COMMAND_SET_CHUNK_VIEW: u8 = 1;
const CLIENT_COMMAND_PLAYER_ACTION: u8 = 2;
const CLIENT_COMMAND_USE_ITEM_ON: u8 = 3;
const CLIENT_COMMAND_MOVE_PLAYER: u8 = 4;
const CLIENT_COMMAND_SET_CARRIED_ITEM: u8 = 5;
const CLIENT_COMMAND_ACCEPT_TELEPORT: u8 = 6;
const CLIENT_COMMAND_SET_DEBUG_HOTBAR_SLOT: u8 = 7;
const CLIENT_COMMAND_SHOOT_DEBUG_PHYSICS_CUBE: u8 = 8;
const CLIENT_COMMAND_SET_PLAYER_APPEARANCE: u8 = 9;
const CLIENT_COMMAND_KEEP_ALIVE: u8 = 10;
const CLIENT_COMMAND_DISCONNECT: u8 = 11;
const CLIENT_COMMAND_RESPAWN: u8 = 12;
const CLIENT_COMMAND_EPHEMERAL_FALLBACK: u8 = 13;
const SERVER_UPDATE_CHUNK_SNAPSHOT: u8 = 1;
const SERVER_UPDATE_CHUNK_UNLOAD: u8 = 2;
const SERVER_UPDATE_SECTION_BLOCK_UPDATES: u8 = 3;
const SERVER_UPDATE_TIME: u8 = 4;
const SERVER_UPDATE_PLAYER_POSITION: u8 = 5;
const SERVER_UPDATE_REMOTE_PLAYER_ADD: u8 = 6;
const SERVER_UPDATE_REMOTE_PLAYER_UPDATE: u8 = 7;
const SERVER_UPDATE_REMOTE_PLAYER_REMOVE: u8 = 8;
const SERVER_UPDATE_ENTITY_SNAPSHOT: u8 = 9;
const SERVER_UPDATE_ENTITY_UPDATE: u8 = 10;
const SERVER_UPDATE_ENTITY_REMOVE: u8 = 11;
const SERVER_UPDATE_WORLD_INFO: u8 = 12;
const SERVER_UPDATE_PLAYER_EXPERIENCE: u8 = 13;
const SERVER_UPDATE_SESSION_CONFIGURATION: u8 = 14;
const SERVER_UPDATE_SESSION_READY: u8 = 15;
const SERVER_UPDATE_KEEP_ALIVE: u8 = 16;
const SERVER_UPDATE_DISCONNECT: u8 = 17;
const SERVER_UPDATE_DIMENSION_CHANGE: u8 = 18;
const SERVER_UPDATE_PLAYER_STATISTICS: u8 = 19;
const SERVER_UPDATE_PLAYER_LIFE: u8 = 20;
const SERVER_UPDATE_EPHEMERAL_FALLBACK: u8 = 21;
const SERVER_UPDATE_PLAYER_INVENTORY: u8 = 22;
const SERVER_UPDATE_MALLARD_FIELD_GUIDE: u8 = 23;
const SERVER_UPDATE_MALLARD_CALL: u8 = 24;
const SERVER_UPDATE_MALLARD_TRACK: u8 = 25;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SessionCapabilities(u64);

impl SessionCapabilities {
    pub const NONE: Self = Self(0);
    pub const DEBUG_ACTIONS: Self = Self(1 << 0);
    pub const EPHEMERAL_BODY_POSE: Self = Self(1 << 1);
    pub const KNOWN: Self = Self(Self::DEBUG_ACTIONS.0 | Self::EPHEMERAL_BODY_POSE.0);
    pub const DEVELOPMENT_DEFAULT: Self = Self::KNOWN;

    pub const fn from_bits_retain(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, capability: Self) -> bool {
        self.0 & capability.0 == capability.0
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn known(self) -> Self {
        self.intersection(Self::KNOWN)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionConfiguration {
    pub gameplay_rate_hz: u32,
    pub publication_rate_hz: u32,
    pub body_pose_report_rate_hz: u32,
    pub remote_pose_replication_rate_hz: u32,
    pub ephemeral_transport: EffectiveEphemeralTransport,
    pub max_render_distance: u32,
    pub max_chunk_tracking_radius: u32,
    pub capabilities: SessionCapabilities,
}

impl SessionConfiguration {
    pub const fn fixed_vanilla(
        max_render_distance: u32,
        max_chunk_tracking_radius: u32,
        capabilities: SessionCapabilities,
    ) -> Self {
        Self {
            gameplay_rate_hz: 20,
            publication_rate_hz: 20,
            body_pose_report_rate_hz: 20,
            remote_pose_replication_rate_hz: 20,
            ephemeral_transport: EffectiveEphemeralTransport::ReliableFallback,
            max_render_distance,
            max_chunk_tracking_radius,
            capabilities,
        }
    }

    pub const fn with_pose_profile(
        mut self,
        body_pose_report_rate_hz: u32,
        remote_pose_replication_rate_hz: u32,
        ephemeral_transport: EffectiveEphemeralTransport,
    ) -> Self {
        self.body_pose_report_rate_hz = body_pose_report_rate_hz;
        self.remote_pose_replication_rate_hz = remote_pose_replication_rate_hz;
        self.ephemeral_transport = ephemeral_transport;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDisconnectReason {
    Quit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisconnectReasonCode {
    Timeout,
    DuplicateProfile,
    ProtocolViolation,
    ServerShutdown,
    Kicked,
    InternalError,
    EndOfStream,
    TransportError,
    ClientQuit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisconnectReason {
    pub code: DisconnectReasonCode,
    pub detail: String,
}

impl DisconnectReason {
    pub fn new(code: DisconnectReasonCode, detail: impl Into<String>) -> Self {
        let mut detail = detail.into();
        if detail.len() > MAX_DISCONNECT_DETAIL_BYTES {
            let mut boundary = MAX_DISCONNECT_DETAIL_BYTES;
            while boundary > 0 && !detail.is_char_boundary(boundary) {
                boundary -= 1;
            }
            detail.truncate(boundary);
        }
        Self { code, detail }
    }

    pub fn timeout(detail: impl Into<String>) -> Self {
        Self::new(DisconnectReasonCode::Timeout, detail)
    }

    pub fn end_of_stream(detail: impl Into<String>) -> Self {
        Self::new(DisconnectReasonCode::EndOfStream, detail)
    }

    pub fn transport_error(detail: impl Into<String>) -> Self {
        Self::new(DisconnectReasonCode::TransportError, detail)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlayerProfileId(pub [u8; 16]);

impl PlayerProfileId {
    pub const TEST_DEFAULT: Self = Self([1; 16]);

    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }

    pub fn is_nil(self) -> bool {
        self.0 == [0; 16]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientIdentity {
    pub profile_id: PlayerProfileId,
    pub display_name: String,
}

impl ClientIdentity {
    pub fn new(
        profile_id: PlayerProfileId,
        display_name: impl Into<String>,
    ) -> ProtocolCodecResult<Self> {
        let identity = Self {
            profile_id,
            display_name: display_name.into(),
        };
        validate_client_identity(&identity)?;
        Ok(identity)
    }

    pub fn test_default() -> Self {
        Self {
            profile_id: PlayerProfileId::TEST_DEFAULT,
            display_name: "Player".to_owned(),
        }
    }
}

pub fn validate_client_identity(identity: &ClientIdentity) -> ProtocolCodecResult<()> {
    if identity.profile_id.is_nil() {
        return Err(ProtocolCodecError::InvalidData(
            "player profile UUID must not be nil",
        ));
    }
    if identity.display_name.is_empty() {
        return Err(ProtocolCodecError::InvalidData(
            "player display name must not be empty",
        ));
    }
    if identity.display_name.len() > MAX_PLAYER_DISPLAY_NAME_BYTES {
        return Err(ProtocolCodecError::InvalidData(
            "player display name exceeds the protocol maximum",
        ));
    }
    if identity.display_name.chars().any(char::is_control) {
        return Err(ProtocolCodecError::InvalidData(
            "player display name contains control characters",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkView {
    pub center: ChunkPos,
    pub render_distance: u32,
    pub chunk_tracking_radius: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientCommand {
    SetChunkView(ChunkView),
    MovePlayer(SequencedMovePlayerCommand),
    AcceptTeleport(AcceptTeleportCommand),
    SetCarriedItem(SetCarriedItemCommand),
    SetDebugHotbarSlot(SetDebugHotbarSlotCommand),
    SetPlayerAppearance(SetPlayerAppearanceCommand),
    PlayerAction(PlayerActionCommand),
    UseItemOn(UseItemOnCommand),
    ShootDebugPhysicsCube,
    KeepAlive { id: u64 },
    Respawn,
    EphemeralFallback(ClientEphemeralMessage),
    Disconnect(ClientDisconnectReason),
}

impl ClientCommand {
    /// Fixture/internal compatibility helper. Production movement should use
    /// [`Self::sequenced_move_player`] with a nonzero sequence.
    pub const fn move_player(movement: MovePlayerCommand) -> Self {
        Self::MovePlayer(SequencedMovePlayerCommand::fixture(movement))
    }

    pub const fn sequenced_move_player(sequence: u32, movement: MovePlayerCommand) -> Self {
        Self::MovePlayer(SequencedMovePlayerCommand::new(sequence, movement))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlayerModelKind {
    #[default]
    Player,
    UprightBear,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerAppearance {
    pub model: PlayerModelKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetPlayerAppearanceCommand {
    pub appearance: PlayerAppearance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MovePlayerCommand {
    Pos {
        position: Vec3d,
        on_ground: bool,
    },
    PosRot {
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        on_ground: bool,
    },
    Rot {
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        on_ground: bool,
    },
    StatusOnly {
        on_ground: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SequencedMovePlayerCommand {
    pub sequence: u32,
    pub movement: MovePlayerCommand,
}

impl SequencedMovePlayerCommand {
    pub const fn new(sequence: u32, movement: MovePlayerCommand) -> Self {
        Self { sequence, movement }
    }

    pub const fn fixture(movement: MovePlayerCommand) -> Self {
        Self::new(0, movement)
    }
}

impl MovePlayerCommand {
    pub fn position_or(self, fallback: Vec3d) -> Vec3d {
        match self {
            Self::Pos { position, .. } | Self::PosRot { position, .. } => position,
            Self::Rot { .. } | Self::StatusOnly { .. } => fallback,
        }
    }

    pub fn y_rot_degrees_or(self, fallback: f32) -> f32 {
        match self {
            Self::PosRot { y_rot_degrees, .. } | Self::Rot { y_rot_degrees, .. } => y_rot_degrees,
            Self::Pos { .. } | Self::StatusOnly { .. } => fallback,
        }
    }

    pub fn x_rot_degrees_or(self, fallback: f32) -> f32 {
        match self {
            Self::PosRot { x_rot_degrees, .. } | Self::Rot { x_rot_degrees, .. } => x_rot_degrees,
            Self::Pos { .. } | Self::StatusOnly { .. } => fallback,
        }
    }

    pub const fn on_ground(self) -> bool {
        match self {
            Self::Pos { on_ground, .. }
            | Self::PosRot { on_ground, .. }
            | Self::Rot { on_ground, .. }
            | Self::StatusOnly { on_ground } => on_ground,
        }
    }

    pub const fn has_position(self) -> bool {
        matches!(self, Self::Pos { .. } | Self::PosRot { .. })
    }

    pub const fn has_rotation(self) -> bool {
        matches!(self, Self::PosRot { .. } | Self::Rot { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptTeleportCommand {
    pub id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetCarriedItemCommand {
    pub slot: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetDebugHotbarSlotCommand {
    pub slot: u8,
    pub item: Option<DebugHotbarItem>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugHotbarItem {
    Block(BlockStateId),
    SpawnActor(DebugActorKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugActorKind {
    Chicken,
    Mannequin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerActionKind {
    StartDestroyBlock,
    StopDestroyBlock,
    AbortDestroyBlock,
    DebugInstantBreak,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerActionCommand {
    pub pos: BlockPos,
    pub direction: Direction,
    pub kind: PlayerActionKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseItemOnCommand {
    pub hand: InteractionHand,
    pub hit: BlockHitResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionHand {
    MainHand,
    OffHand,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServerUpdate {
    SessionConfiguration(SessionConfiguration),
    SessionReady,
    WorldInfo {
        dimension: DimensionKey,
        biome_zoom_seed: i64,
        topology: HorizontalTopology,
    },
    /// Vanilla-shaped respawn/dimension boundary. The client must replace its
    /// dimension-local replica before applying following world updates while
    /// retaining realm-scoped player state when requested.
    DimensionChange {
        dimension: DimensionKey,
        biome_zoom_seed: i64,
        topology: HorizontalTopology,
        keep_player_state: bool,
    },
    ChunkSnapshot(ChunkSnapshot),
    ChunkUnload {
        pos: ChunkPos,
    },
    SectionBlockUpdates {
        pos: ChunkPos,
        section_y: i32,
        updates: Vec<SectionBlockUpdate>,
    },
    /// Authoritative vanilla world clocks. The client advances both clocks
    /// locally between samples, except that `day_time` holds while the daylight
    /// cycle is not running.
    TimeUpdate {
        game_time: u64,
        day_time: u64,
        daylight_cycle_running: bool,
    },
    PlayerPosition(PlayerPositionUpdate),
    RemotePlayerAdd(RemotePlayerUpdate),
    RemotePlayerUpdate(RemotePlayerUpdate),
    RemotePlayerRemove {
        id: RemotePlayerId,
    },
    EntitySnapshot(EntitySnapshot),
    EntityUpdate(EntityUpdate),
    EntityRemove {
        id: EntityId,
    },
    /// Owner-only authoritative total experience points. This slice carries
    /// only the total; vanilla level/progress semantics remain future work.
    PlayerExperience {
        total_experience: u64,
    },
    /// Owner-only authoritative statistics snapshot. Keys retain vanilla's
    /// statistic-type/value shape while allowing mclone-namespaced entries.
    PlayerStatistics {
        statistics: PlayerStatistics,
    },
    PlayerInventory {
        hotbar: [Option<ItemStackSnapshot>; HOTBAR_SLOT_COUNT_USIZE],
    },
    MallardFieldGuide(MallardFieldGuideProgress),
    MallardCall(MallardCallCue),
    MallardTrack(MallardTrackCue),
    /// Owner-only atomic health/death snapshot.
    PlayerLife(PlayerLifeState),
    EphemeralFallback(ServerEphemeralMessage),
    KeepAlive {
        id: u64,
    },
    Disconnect(DisconnectReason),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RemotePlayerId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemotePlayerUpdate {
    pub id: RemotePlayerId,
    pub appearance: PlayerAppearance,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(pub u64);

/// Stable UUID-equivalent identity for one entity across save/load.
///
/// [`EntityId`] remains the session-local runtime/network handle. This value is
/// immutable spawn data and follows Minecraft Java's persisted entity UUID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityPersistentId {
    pub most: u64,
    pub least: u64,
}

impl EntityPersistentId {
    pub const fn new(most: u64, least: u64) -> Self {
        Self { most, least }
    }
}

impl fmt::Display for EntityPersistentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = (u128::from(self.most) << 64) | u128::from(self.least);
        let group1 = (value >> 96) as u32;
        let group2 = ((value >> 80) & 0xffff) as u16;
        let group3 = ((value >> 64) & 0xffff) as u16;
        let group4 = ((value >> 48) & 0xffff) as u16;
        let group5 = value & 0xffff_ffff_ffff;
        write!(
            formatter,
            "{group1:08x}-{group2:04x}-{group3:04x}-{group4:04x}-{group5:012x}"
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityKind {
    Cow,
    Chicken,
    Mallard,
    MallardNest,
    Mannequin,
    DebugCube,
    Item,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Egg,
    MallardEgg,
    MallardFeather,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemStackSnapshot {
    pub kind: ItemKind,
    pub count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityRotation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl EntityRotation {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub persistent_id: EntityPersistentId,
    pub kind: EntityKind,
    pub item_stack: Option<ItemStackSnapshot>,
    pub mallard: Option<MallardSnapshotData>,
    pub mallard_nest: Option<MallardNestSnapshotData>,
    pub animation: Option<AnimationState>,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub width: f32,
    pub height: f32,
    pub tick_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityUpdate {
    pub id: EntityId,
    pub item_stack: Option<ItemStackSnapshot>,
    pub mallard: Option<MallardUpdateData>,
    pub mallard_nest: Option<MallardNestUpdateData>,
    pub animation: Option<AnimationState>,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub tick_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerPositionUpdate {
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub relative: PlayerPositionRelativeFlags,
    pub last_applied_move_sequence: u32,
    pub teleport_id: u32,
    pub dismount_vehicle: bool,
    pub reset_continuity: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerPositionRelativeFlags {
    pub x: bool,
    pub y: bool,
    pub z: bool,
    pub y_rot: bool,
    pub x_rot: bool,
}

impl PlayerPositionRelativeFlags {
    const X_MASK: u8 = 1 << 0;
    const Y_MASK: u8 = 1 << 1;
    const Z_MASK: u8 = 1 << 2;
    const Y_ROT_MASK: u8 = 1 << 3;
    const X_ROT_MASK: u8 = 1 << 4;
    const VALID_MASK: u8 =
        Self::X_MASK | Self::Y_MASK | Self::Z_MASK | Self::Y_ROT_MASK | Self::X_ROT_MASK;

    pub const ABSOLUTE: Self = Self {
        x: false,
        y: false,
        z: false,
        y_rot: false,
        x_rot: false,
    };

    pub const fn bits(self) -> u8 {
        (if self.x { Self::X_MASK } else { 0 })
            | (if self.y { Self::Y_MASK } else { 0 })
            | (if self.z { Self::Z_MASK } else { 0 })
            | (if self.y_rot { Self::Y_ROT_MASK } else { 0 })
            | (if self.x_rot { Self::X_ROT_MASK } else { 0 })
    }

    pub fn from_bits(bits: u8) -> Option<Self> {
        if bits & !Self::VALID_MASK != 0 {
            return None;
        }
        Some(Self {
            x: bits & Self::X_MASK != 0,
            y: bits & Self::Y_MASK != 0,
            z: bits & Self::Z_MASK != 0,
            y_rot: bits & Self::Y_ROT_MASK != 0,
            x_rot: bits & Self::X_ROT_MASK != 0,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionBlockUpdate {
    pub local_x: u8,
    pub local_y: u8,
    pub local_z: u8,
    pub block_state: BlockStateId,
}

pub type ProtocolCodecResult<T> = Result<T, ProtocolCodecError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolCodecError {
    UnexpectedEof { needed: usize, remaining: usize },
    TrailingBytes { remaining: usize },
    UnknownClientCommandTag(u8),
    UnknownServerUpdateTag(u8),
    UnknownChunkStatus(u8),
    UnknownDirection(u8),
    UnknownEntityKind(u8),
    UnknownItemKind(u8),
    UnknownInteractionHand(u8),
    UnknownPlayerModelKind(u8),
    UnknownPlayerActionKind(u8),
    LengthOverflow { field: &'static str, len: usize },
    InvalidData(&'static str),
}

impl fmt::Display for ProtocolCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { needed, remaining } => write!(
                f,
                "protocol payload ended early: needed {needed} bytes, had {remaining}"
            ),
            Self::TrailingBytes { remaining } => {
                write!(f, "protocol payload had {remaining} trailing bytes")
            }
            Self::UnknownClientCommandTag(tag) => {
                write!(f, "unknown client command tag {tag}")
            }
            Self::UnknownServerUpdateTag(tag) => {
                write!(f, "unknown server update tag {tag}")
            }
            Self::UnknownChunkStatus(status) => {
                write!(f, "unknown chunk status tag {status}")
            }
            Self::UnknownDirection(direction) => {
                write!(f, "unknown direction tag {direction}")
            }
            Self::UnknownEntityKind(kind) => {
                write!(f, "unknown entity kind tag {kind}")
            }
            Self::UnknownItemKind(kind) => {
                write!(f, "unknown item kind tag {kind}")
            }
            Self::UnknownInteractionHand(hand) => {
                write!(f, "unknown interaction hand tag {hand}")
            }
            Self::UnknownPlayerModelKind(kind) => {
                write!(f, "unknown player model kind tag {kind}")
            }
            Self::UnknownPlayerActionKind(kind) => {
                write!(f, "unknown player action kind tag {kind}")
            }
            Self::LengthOverflow { field, len } => {
                write!(f, "{field} length {len} does not fit in u32")
            }
            Self::InvalidData(message) => write!(f, "invalid protocol payload: {message}"),
        }
    }
}

impl Error for ProtocolCodecError {}

pub fn encode_client_command(command: &ClientCommand) -> ProtocolCodecResult<Vec<u8>> {
    let mut writer = ByteWriter::new();
    match command {
        ClientCommand::SetChunkView(view) => {
            writer.write_u8(CLIENT_COMMAND_SET_CHUNK_VIEW);
            writer.write_chunk_pos(view.center);
            writer.write_u32(view.render_distance);
            writer.write_u32(view.chunk_tracking_radius);
        }
        ClientCommand::MovePlayer(command) => {
            writer.write_u8(CLIENT_COMMAND_MOVE_PLAYER);
            writer.write_move_player(command);
        }
        ClientCommand::AcceptTeleport(command) => {
            writer.write_u8(CLIENT_COMMAND_ACCEPT_TELEPORT);
            writer.write_accept_teleport(command);
        }
        ClientCommand::SetCarriedItem(command) => {
            writer.write_u8(CLIENT_COMMAND_SET_CARRIED_ITEM);
            writer.write_set_carried_item(command);
        }
        ClientCommand::SetDebugHotbarSlot(command) => {
            writer.write_u8(CLIENT_COMMAND_SET_DEBUG_HOTBAR_SLOT);
            writer.write_set_debug_hotbar_slot(command);
        }
        ClientCommand::SetPlayerAppearance(command) => {
            writer.write_u8(CLIENT_COMMAND_SET_PLAYER_APPEARANCE);
            writer.write_set_player_appearance(command);
        }
        ClientCommand::PlayerAction(command) => {
            writer.write_u8(CLIENT_COMMAND_PLAYER_ACTION);
            writer.write_player_action(command);
        }
        ClientCommand::UseItemOn(command) => {
            writer.write_u8(CLIENT_COMMAND_USE_ITEM_ON);
            writer.write_use_item_on(command);
        }
        ClientCommand::ShootDebugPhysicsCube => {
            writer.write_u8(CLIENT_COMMAND_SHOOT_DEBUG_PHYSICS_CUBE);
        }
        ClientCommand::KeepAlive { id } => {
            writer.write_u8(CLIENT_COMMAND_KEEP_ALIVE);
            writer.write_u64(*id);
        }
        ClientCommand::Respawn => writer.write_u8(CLIENT_COMMAND_RESPAWN),
        ClientCommand::EphemeralFallback(message) => {
            writer.write_u8(CLIENT_COMMAND_EPHEMERAL_FALLBACK);
            writer.write_raw(&encode_client_ephemeral_message(*message)?);
        }
        ClientCommand::Disconnect(reason) => {
            writer.write_u8(CLIENT_COMMAND_DISCONNECT);
            writer.write_client_disconnect_reason(*reason);
        }
    }
    Ok(writer.into_inner())
}

pub fn decode_client_command(bytes: &[u8]) -> ProtocolCodecResult<ClientCommand> {
    let mut reader = ByteReader::new(bytes);
    let tag = reader.read_u8()?;
    let command = match tag {
        CLIENT_COMMAND_SET_CHUNK_VIEW => {
            let center = reader.read_chunk_pos()?;
            let render_distance = reader.read_u32()?;
            let chunk_tracking_radius = reader.read_u32()?;
            ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance,
                chunk_tracking_radius,
            })
        }
        CLIENT_COMMAND_MOVE_PLAYER => ClientCommand::MovePlayer(reader.read_move_player()?),
        CLIENT_COMMAND_ACCEPT_TELEPORT => {
            ClientCommand::AcceptTeleport(reader.read_accept_teleport()?)
        }
        CLIENT_COMMAND_SET_CARRIED_ITEM => {
            ClientCommand::SetCarriedItem(reader.read_set_carried_item()?)
        }
        CLIENT_COMMAND_SET_DEBUG_HOTBAR_SLOT => {
            ClientCommand::SetDebugHotbarSlot(reader.read_set_debug_hotbar_slot()?)
        }
        CLIENT_COMMAND_SET_PLAYER_APPEARANCE => {
            ClientCommand::SetPlayerAppearance(reader.read_set_player_appearance()?)
        }
        CLIENT_COMMAND_PLAYER_ACTION => ClientCommand::PlayerAction(reader.read_player_action()?),
        CLIENT_COMMAND_USE_ITEM_ON => ClientCommand::UseItemOn(reader.read_use_item_on()?),
        CLIENT_COMMAND_SHOOT_DEBUG_PHYSICS_CUBE => ClientCommand::ShootDebugPhysicsCube,
        CLIENT_COMMAND_KEEP_ALIVE => ClientCommand::KeepAlive {
            id: reader.read_u64()?,
        },
        CLIENT_COMMAND_RESPAWN => ClientCommand::Respawn,
        CLIENT_COMMAND_EPHEMERAL_FALLBACK => ClientCommand::EphemeralFallback(
            decode_client_ephemeral_message(reader.read_remaining())?,
        ),
        CLIENT_COMMAND_DISCONNECT => {
            ClientCommand::Disconnect(reader.read_client_disconnect_reason()?)
        }
        _ => return Err(ProtocolCodecError::UnknownClientCommandTag(tag)),
    };
    reader.finish()?;
    Ok(command)
}

pub fn encode_server_update(update: &ServerUpdate) -> ProtocolCodecResult<Vec<u8>> {
    let mut writer = ByteWriter::new();
    match update {
        ServerUpdate::SessionConfiguration(configuration) => {
            validate_session_configuration(configuration)?;
            writer.write_u8(SERVER_UPDATE_SESSION_CONFIGURATION);
            writer.write_session_configuration(*configuration);
        }
        ServerUpdate::SessionReady => {
            writer.write_u8(SERVER_UPDATE_SESSION_READY);
        }
        ServerUpdate::WorldInfo {
            dimension,
            biome_zoom_seed,
            topology,
        } => {
            validate_horizontal_topology(*topology)?;
            writer.write_u8(SERVER_UPDATE_WORLD_INFO);
            writer.write_string("dimension key", dimension.as_str())?;
            writer.write_i64(*biome_zoom_seed);
            writer.write_horizontal_topology(*topology);
        }
        ServerUpdate::DimensionChange {
            dimension,
            biome_zoom_seed,
            topology,
            keep_player_state,
        } => {
            validate_horizontal_topology(*topology)?;
            writer.write_u8(SERVER_UPDATE_DIMENSION_CHANGE);
            writer.write_string("dimension key", dimension.as_str())?;
            writer.write_i64(*biome_zoom_seed);
            writer.write_horizontal_topology(*topology);
            writer.write_bool(*keep_player_state);
        }
        ServerUpdate::ChunkSnapshot(snapshot) => {
            writer.write_u8(SERVER_UPDATE_CHUNK_SNAPSHOT);
            writer.write_snapshot(snapshot)?;
        }
        ServerUpdate::ChunkUnload { pos } => {
            writer.write_u8(SERVER_UPDATE_CHUNK_UNLOAD);
            writer.write_chunk_pos(*pos);
        }
        ServerUpdate::SectionBlockUpdates {
            pos,
            section_y,
            updates,
        } => {
            if updates.is_empty() {
                return Err(ProtocolCodecError::InvalidData(
                    "section block update list is empty",
                ));
            }
            writer.write_u8(SERVER_UPDATE_SECTION_BLOCK_UPDATES);
            writer.write_chunk_pos(*pos);
            writer.write_i32(*section_y);
            writer.write_len("section block updates", updates.len())?;
            for update in updates {
                validate_section_block_update(update)?;
                writer.write_section_block_update(update);
            }
        }
        ServerUpdate::TimeUpdate {
            game_time,
            day_time,
            daylight_cycle_running,
        } => {
            writer.write_u8(SERVER_UPDATE_TIME);
            writer.write_u64(*game_time);
            writer.write_u64(*day_time);
            writer.write_bool(*daylight_cycle_running);
        }
        ServerUpdate::PlayerPosition(update) => {
            validate_player_position_update(update)?;
            writer.write_u8(SERVER_UPDATE_PLAYER_POSITION);
            writer.write_player_position_update(update);
        }
        ServerUpdate::RemotePlayerAdd(update) => {
            validate_remote_player_update(update)?;
            writer.write_u8(SERVER_UPDATE_REMOTE_PLAYER_ADD);
            writer.write_remote_player_update(update);
        }
        ServerUpdate::RemotePlayerUpdate(update) => {
            validate_remote_player_update(update)?;
            writer.write_u8(SERVER_UPDATE_REMOTE_PLAYER_UPDATE);
            writer.write_remote_player_update(update);
        }
        ServerUpdate::RemotePlayerRemove { id } => {
            writer.write_u8(SERVER_UPDATE_REMOTE_PLAYER_REMOVE);
            writer.write_remote_player_id(*id);
        }
        ServerUpdate::EntitySnapshot(snapshot) => {
            validate_entity_snapshot(snapshot)?;
            writer.write_u8(SERVER_UPDATE_ENTITY_SNAPSHOT);
            writer.write_entity_snapshot(snapshot);
        }
        ServerUpdate::EntityUpdate(update) => {
            validate_entity_update(update)?;
            writer.write_u8(SERVER_UPDATE_ENTITY_UPDATE);
            writer.write_entity_update(update);
        }
        ServerUpdate::EntityRemove { id } => {
            writer.write_u8(SERVER_UPDATE_ENTITY_REMOVE);
            writer.write_entity_id(*id);
        }
        ServerUpdate::PlayerExperience { total_experience } => {
            writer.write_u8(SERVER_UPDATE_PLAYER_EXPERIENCE);
            writer.write_u64(*total_experience);
        }
        ServerUpdate::PlayerStatistics { statistics } => {
            writer.write_u8(SERVER_UPDATE_PLAYER_STATISTICS);
            if statistics.len() > MAX_PLAYER_STATISTIC_ENTRIES {
                return Err(ProtocolCodecError::InvalidData(
                    "too many player statistic entries",
                ));
            }
            writer.write_len("player statistics", statistics.len())?;
            for (key, value) in statistics.iter() {
                writer.write_string("statistic type", key.statistic_type())?;
                writer.write_string("statistic value", key.value())?;
                writer.write_u32(*value);
            }
        }
        ServerUpdate::PlayerInventory { hotbar } => {
            writer.write_u8(SERVER_UPDATE_PLAYER_INVENTORY);
            for stack in hotbar {
                writer.write_optional_item_stack_snapshot(*stack);
            }
        }
        ServerUpdate::MallardFieldGuide(progress) => {
            writer.write_u8(SERVER_UPDATE_MALLARD_FIELD_GUIDE);
            writer.write_u32(progress.bits());
        }
        ServerUpdate::MallardCall(cue) => {
            if !cue.position.is_finite()
                || !cue.audible_radius.is_finite()
                || cue.audible_radius <= 0.0
            {
                return Err(ProtocolCodecError::InvalidData("invalid mallard call cue"));
            }
            writer.write_u8(SERVER_UPDATE_MALLARD_CALL);
            writer.write_entity_id(cue.source);
            writer.write_vec3d(cue.position);
            writer.write_u64(cue.sequence);
            writer.write_f32(cue.audible_radius);
        }
        ServerUpdate::MallardTrack(cue) => {
            if !cue.position.is_finite() || !cue.y_rot_degrees.is_finite() {
                return Err(ProtocolCodecError::InvalidData("invalid mallard track cue"));
            }
            writer.write_u8(SERVER_UPDATE_MALLARD_TRACK);
            writer.write_u64(cue.source.most);
            writer.write_u64(cue.source.least);
            writer.write_vec3d(cue.position);
            writer.write_f32(cue.y_rot_degrees);
            writer.write_u64(cue.sequence);
        }
        ServerUpdate::PlayerLife(state) => {
            validate_player_life_state(*state)?;
            writer.write_u8(SERVER_UPDATE_PLAYER_LIFE);
            writer.write_u32(state.epoch());
            writer.write_f32(state.vitals().health());
            writer.write_f32(state.vitals().max_health());
            writer.write_u8(match state.death_cause() {
                None => 0,
                Some(PlayerDamageCause::Lava) => 1,
            });
        }
        ServerUpdate::EphemeralFallback(message) => {
            writer.write_u8(SERVER_UPDATE_EPHEMERAL_FALLBACK);
            writer.write_raw(&encode_server_ephemeral_message(*message)?);
        }
        ServerUpdate::KeepAlive { id } => {
            writer.write_u8(SERVER_UPDATE_KEEP_ALIVE);
            writer.write_u64(*id);
        }
        ServerUpdate::Disconnect(reason) => {
            validate_disconnect_reason(reason)?;
            writer.write_u8(SERVER_UPDATE_DISCONNECT);
            writer.write_disconnect_reason(reason)?;
        }
    }
    Ok(writer.into_inner())
}

pub fn decode_server_update(bytes: &[u8]) -> ProtocolCodecResult<ServerUpdate> {
    let mut reader = ByteReader::new(bytes);
    let tag = reader.read_u8()?;
    let update = match tag {
        SERVER_UPDATE_SESSION_CONFIGURATION => {
            ServerUpdate::SessionConfiguration(reader.read_session_configuration()?)
        }
        SERVER_UPDATE_SESSION_READY => ServerUpdate::SessionReady,
        SERVER_UPDATE_WORLD_INFO => {
            let dimension =
                DimensionKey::parse(reader.read_string("dimension key", MAX_DIMENSION_KEY_BYTES)?)
                    .map_err(|_| ProtocolCodecError::InvalidData("invalid dimension key"))?;
            ServerUpdate::WorldInfo {
                dimension,
                biome_zoom_seed: reader.read_i64()?,
                topology: reader.read_horizontal_topology()?,
            }
        }
        SERVER_UPDATE_DIMENSION_CHANGE => {
            let dimension =
                DimensionKey::parse(reader.read_string("dimension key", MAX_DIMENSION_KEY_BYTES)?)
                    .map_err(|_| ProtocolCodecError::InvalidData("invalid dimension key"))?;
            ServerUpdate::DimensionChange {
                dimension,
                biome_zoom_seed: reader.read_i64()?,
                topology: reader.read_horizontal_topology()?,
                keep_player_state: reader.read_bool()?,
            }
        }
        SERVER_UPDATE_CHUNK_SNAPSHOT => ServerUpdate::ChunkSnapshot(reader.read_snapshot()?),
        SERVER_UPDATE_CHUNK_UNLOAD => ServerUpdate::ChunkUnload {
            pos: reader.read_chunk_pos()?,
        },
        SERVER_UPDATE_SECTION_BLOCK_UPDATES => {
            let pos = reader.read_chunk_pos()?;
            let section_y = reader.read_i32()?;
            let updates = (0..reader.read_len()?)
                .map(|_| reader.read_section_block_update())
                .collect::<ProtocolCodecResult<Vec<_>>>()?;
            if updates.is_empty() {
                return Err(ProtocolCodecError::InvalidData(
                    "section block update list is empty",
                ));
            }
            ServerUpdate::SectionBlockUpdates {
                pos,
                section_y,
                updates,
            }
        }
        SERVER_UPDATE_TIME => ServerUpdate::TimeUpdate {
            game_time: reader.read_u64()?,
            day_time: reader.read_u64()?,
            daylight_cycle_running: reader.read_bool()?,
        },
        SERVER_UPDATE_PLAYER_POSITION => {
            ServerUpdate::PlayerPosition(reader.read_player_position_update()?)
        }
        SERVER_UPDATE_REMOTE_PLAYER_ADD => {
            ServerUpdate::RemotePlayerAdd(reader.read_remote_player_update()?)
        }
        SERVER_UPDATE_REMOTE_PLAYER_UPDATE => {
            ServerUpdate::RemotePlayerUpdate(reader.read_remote_player_update()?)
        }
        SERVER_UPDATE_REMOTE_PLAYER_REMOVE => ServerUpdate::RemotePlayerRemove {
            id: reader.read_remote_player_id()?,
        },
        SERVER_UPDATE_ENTITY_SNAPSHOT => {
            ServerUpdate::EntitySnapshot(reader.read_entity_snapshot()?)
        }
        SERVER_UPDATE_ENTITY_UPDATE => ServerUpdate::EntityUpdate(reader.read_entity_update()?),
        SERVER_UPDATE_ENTITY_REMOVE => ServerUpdate::EntityRemove {
            id: reader.read_entity_id()?,
        },
        SERVER_UPDATE_PLAYER_EXPERIENCE => ServerUpdate::PlayerExperience {
            total_experience: reader.read_u64()?,
        },
        SERVER_UPDATE_PLAYER_STATISTICS => {
            let count = reader.read_len()?;
            if count > MAX_PLAYER_STATISTIC_ENTRIES {
                return Err(ProtocolCodecError::InvalidData(
                    "too many player statistic entries",
                ));
            }
            let mut statistics = PlayerStatistics::default();
            for _ in 0..count {
                let statistic_type =
                    reader.read_string("statistic type", MAX_STATISTIC_RESOURCE_KEY_BYTES)?;
                let value =
                    reader.read_string("statistic value", MAX_STATISTIC_RESOURCE_KEY_BYTES)?;
                let key = StatisticKey::new(statistic_type, value)
                    .map_err(|_| ProtocolCodecError::InvalidData("invalid statistic key"))?;
                statistics.set(key, reader.read_u32()?);
            }
            ServerUpdate::PlayerStatistics { statistics }
        }
        SERVER_UPDATE_PLAYER_INVENTORY => {
            let mut hotbar = [None; HOTBAR_SLOT_COUNT_USIZE];
            for stack in &mut hotbar {
                *stack = reader.read_optional_item_stack_snapshot()?;
            }
            ServerUpdate::PlayerInventory { hotbar }
        }
        SERVER_UPDATE_MALLARD_FIELD_GUIDE => ServerUpdate::MallardFieldGuide(
            MallardFieldGuideProgress::from_bits_retain(reader.read_u32()?),
        ),
        SERVER_UPDATE_MALLARD_CALL => ServerUpdate::MallardCall(MallardCallCue {
            source: reader.read_entity_id()?,
            position: reader.read_vec3d()?,
            sequence: reader.read_u64()?,
            audible_radius: reader.read_f32()?,
        }),
        SERVER_UPDATE_MALLARD_TRACK => ServerUpdate::MallardTrack(MallardTrackCue {
            source: EntityPersistentId::new(reader.read_u64()?, reader.read_u64()?),
            position: reader.read_vec3d()?,
            y_rot_degrees: reader.read_f32()?,
            sequence: reader.read_u64()?,
        }),
        SERVER_UPDATE_PLAYER_LIFE => {
            let epoch = reader.read_u32()?;
            let vitals = PlayerVitals::new(reader.read_f32()?, reader.read_f32()?)
                .map_err(|_| ProtocolCodecError::InvalidData("invalid player vitals"))?;
            let death_cause = match reader.read_u8()? {
                0 => None,
                1 => Some(PlayerDamageCause::Lava),
                _ => {
                    return Err(ProtocolCodecError::InvalidData(
                        "unknown player damage cause",
                    ));
                }
            };
            ServerUpdate::PlayerLife(
                PlayerLifeState::new(epoch, vitals, death_cause).map_err(|_| {
                    ProtocolCodecError::InvalidData("inconsistent player life state")
                })?,
            )
        }
        SERVER_UPDATE_EPHEMERAL_FALLBACK => ServerUpdate::EphemeralFallback(
            decode_server_ephemeral_message(reader.read_remaining())?,
        ),
        SERVER_UPDATE_KEEP_ALIVE => ServerUpdate::KeepAlive {
            id: reader.read_u64()?,
        },
        SERVER_UPDATE_DISCONNECT => ServerUpdate::Disconnect(reader.read_disconnect_reason()?),
        _ => return Err(ProtocolCodecError::UnknownServerUpdateTag(tag)),
    };
    reader.finish()?;
    Ok(update)
}

fn validate_section_block_update(update: &SectionBlockUpdate) -> ProtocolCodecResult<()> {
    if update.local_x as i32 >= CHUNK_WIDTH {
        return Err(ProtocolCodecError::InvalidData("section update local_x"));
    }
    if update.local_y as i32 >= SECTION_HEIGHT {
        return Err(ProtocolCodecError::InvalidData("section update local_y"));
    }
    if update.local_z as i32 >= CHUNK_WIDTH {
        return Err(ProtocolCodecError::InvalidData("section update local_z"));
    }
    Ok(())
}

fn validate_session_configuration(configuration: &SessionConfiguration) -> ProtocolCodecResult<()> {
    if configuration.gameplay_rate_hz == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "session gameplay rate must be nonzero",
        ));
    }
    if configuration.publication_rate_hz == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "session publication rate must be nonzero",
        ));
    }
    if configuration.body_pose_report_rate_hz == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "session body pose report rate must be nonzero",
        ));
    }
    if configuration.remote_pose_replication_rate_hz == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "session remote pose replication rate must be nonzero",
        ));
    }
    if configuration.max_render_distance > configuration.max_chunk_tracking_radius {
        return Err(ProtocolCodecError::InvalidData(
            "session render distance exceeds tracking radius",
        ));
    }
    Ok(())
}

fn validate_horizontal_topology(topology: HorizontalTopology) -> ProtocolCodecResult<()> {
    topology
        .validate()
        .map_err(|_| ProtocolCodecError::InvalidData("invalid dimension topology"))
}

fn validate_disconnect_reason(reason: &DisconnectReason) -> ProtocolCodecResult<()> {
    if reason.detail.len() > MAX_DISCONNECT_DETAIL_BYTES {
        return Err(ProtocolCodecError::InvalidData(
            "disconnect detail exceeds the protocol maximum",
        ));
    }
    Ok(())
}

fn validate_player_position_update(update: &PlayerPositionUpdate) -> ProtocolCodecResult<()> {
    if !update.position.is_finite()
        || !update.y_rot_degrees.is_finite()
        || !update.x_rot_degrees.is_finite()
    {
        return Err(ProtocolCodecError::InvalidData(
            "player position update contains non-finite value",
        ));
    }
    Ok(())
}

fn validate_remote_player_update(update: &RemotePlayerUpdate) -> ProtocolCodecResult<()> {
    if !update.position.is_finite()
        || !update.y_rot_degrees.is_finite()
        || !update.x_rot_degrees.is_finite()
    {
        return Err(ProtocolCodecError::InvalidData(
            "remote player update contains non-finite value",
        ));
    }
    Ok(())
}

fn validate_player_life_state(state: PlayerLifeState) -> ProtocolCodecResult<()> {
    PlayerVitals::new(state.vitals().health(), state.vitals().max_health())
        .map_err(|_| ProtocolCodecError::InvalidData("invalid player vitals"))?;
    PlayerLifeState::new(state.epoch(), state.vitals(), state.death_cause())
        .map_err(|_| ProtocolCodecError::InvalidData("inconsistent player life state"))?;
    Ok(())
}

fn validate_entity_snapshot(snapshot: &EntitySnapshot) -> ProtocolCodecResult<()> {
    validate_entity_update(&EntityUpdate {
        id: snapshot.id,
        item_stack: snapshot.item_stack,
        mallard: snapshot.mallard.map(|data| MallardUpdateData {
            life_stage: data.life_stage,
            in_water: data.in_water,
        }),
        mallard_nest: snapshot.mallard_nest.map(|data| MallardNestUpdateData {
            incubation_progress: data.incubation_progress,
            incubation_required: data.incubation_required,
            attended: data.attended,
        }),
        animation: snapshot.animation,
        position: snapshot.position,
        y_rot_degrees: snapshot.y_rot_degrees,
        x_rot_degrees: snapshot.x_rot_degrees,
        rotation: snapshot.rotation,
        on_ground: snapshot.on_ground,
        tick_count: snapshot.tick_count,
    })?;
    if !snapshot.width.is_finite()
        || !snapshot.height.is_finite()
        || snapshot.width <= 0.0
        || snapshot.height <= 0.0
    {
        return Err(ProtocolCodecError::InvalidData(
            "entity snapshot contains invalid dimensions",
        ));
    }
    match (snapshot.kind, snapshot.item_stack) {
        (EntityKind::Item, Some(stack)) => validate_item_stack_snapshot(stack)?,
        (EntityKind::Item, None) => {
            return Err(ProtocolCodecError::InvalidData(
                "item entity snapshot missing item stack",
            ));
        }
        (_, Some(_)) => {
            return Err(ProtocolCodecError::InvalidData(
                "non-item entity snapshot contains item stack",
            ));
        }
        (_, None) => {}
    }
    if (snapshot.kind == EntityKind::Mallard) != snapshot.mallard.is_some() {
        return Err(ProtocolCodecError::InvalidData(
            "mallard entity snapshot has inconsistent species data",
        ));
    }
    if (snapshot.kind == EntityKind::MallardNest) != snapshot.mallard_nest.is_some() {
        return Err(ProtocolCodecError::InvalidData(
            "mallard nest snapshot has inconsistent nest data",
        ));
    }
    Ok(())
}

fn validate_item_stack_snapshot(stack: ItemStackSnapshot) -> ProtocolCodecResult<()> {
    if stack.count == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "item stack snapshot has zero count",
        ));
    }
    Ok(())
}

fn validate_entity_update(update: &EntityUpdate) -> ProtocolCodecResult<()> {
    if let Some(stack) = update.item_stack {
        validate_item_stack_snapshot(stack)?;
    }
    if !update.position.is_finite()
        || !update.y_rot_degrees.is_finite()
        || !update.x_rot_degrees.is_finite()
    {
        return Err(ProtocolCodecError::InvalidData(
            "entity update contains non-finite value",
        ));
    }
    validate_optional_entity_rotation(update.rotation)?;
    if let Some(nest) = update.mallard_nest
        && (nest.incubation_required == 0 || nest.incubation_progress > nest.incubation_required)
    {
        return Err(ProtocolCodecError::InvalidData(
            "mallard nest update has invalid incubation progress",
        ));
    }
    if let Some(animation) = update.animation {
        AnimationClipId::parse(animation.clip.as_str())
            .map_err(|_| ProtocolCodecError::InvalidData("entity animation has invalid clip id"))?;
        if animation.phase_source == AnimationPhaseSource::Distance && animation.start_tick != 0 {
            return Err(ProtocolCodecError::InvalidData(
                "distance animation must not carry an elapsed start tick",
            ));
        }
    }
    Ok(())
}

fn validate_optional_entity_rotation(rotation: Option<EntityRotation>) -> ProtocolCodecResult<()> {
    let Some(rotation) = rotation else {
        return Ok(());
    };
    if !rotation.x.is_finite()
        || !rotation.y.is_finite()
        || !rotation.z.is_finite()
        || !rotation.w.is_finite()
    {
        return Err(ProtocolCodecError::InvalidData(
            "entity rotation contains non-finite value",
        ));
    }
    let len_sqr = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    if len_sqr <= f32::EPSILON {
        return Err(ProtocolCodecError::InvalidData(
            "entity rotation has zero length",
        ));
    }
    Ok(())
}

struct ByteWriter {
    bytes: Vec<u8>,
}

impl ByteWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_len(&mut self, field: &'static str, len: usize) -> ProtocolCodecResult<()> {
        let len =
            u32::try_from(len).map_err(|_| ProtocolCodecError::LengthOverflow { field, len })?;
        self.write_u32(len);
        Ok(())
    }

    fn write_string(&mut self, field: &'static str, value: &str) -> ProtocolCodecResult<()> {
        self.write_len(field, value.len())?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn write_chunk_pos(&mut self, pos: ChunkPos) {
        self.write_i32(pos.x);
        self.write_i32(pos.z);
    }

    fn write_horizontal_topology(&mut self, topology: HorizontalTopology) {
        self.write_axis_topology(topology.x);
        self.write_axis_topology(topology.z);
    }

    fn write_axis_topology(&mut self, axis: AxisTopology) {
        match axis {
            AxisTopology::Unbounded => self.write_u8(0),
            AxisTopology::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => {
                self.write_u8(1);
                self.write_i32(minimum_chunk);
                self.write_i32(maximum_chunk_exclusive);
            }
            AxisTopology::Periodic {
                minimum_chunk,
                period_chunks,
            } => {
                self.write_u8(2);
                self.write_i32(minimum_chunk);
                self.write_u32(period_chunks);
            }
        }
    }

    fn write_block_pos(&mut self, pos: BlockPos) {
        self.write_i32(pos.x);
        self.write_i32(pos.y);
        self.write_i32(pos.z);
    }

    fn write_direction(&mut self, direction: Direction) {
        self.write_u8(direction.index());
    }

    fn write_entity_kind(&mut self, kind: EntityKind) {
        self.write_u8(match kind {
            EntityKind::Cow => 0,
            EntityKind::Chicken => 1,
            EntityKind::DebugCube => 2,
            EntityKind::Item => 3,
            EntityKind::Mannequin => 4,
            EntityKind::Mallard => 5,
            EntityKind::MallardNest => 6,
        });
    }

    fn write_item_kind(&mut self, kind: ItemKind) {
        self.write_u8(match kind {
            ItemKind::Egg => 0,
            ItemKind::MallardEgg => 1,
            ItemKind::MallardFeather => 2,
        });
    }

    fn write_vec3d(&mut self, value: Vec3d) {
        self.write_f64(value.x);
        self.write_f64(value.y);
        self.write_f64(value.z);
    }

    fn write_block_hit_result(&mut self, hit: BlockHitResult) {
        self.write_bool(hit.miss);
        self.write_vec3d(hit.location);
        self.write_direction(hit.direction);
        self.write_block_pos(hit.block_pos);
        self.write_bool(hit.inside);
    }

    fn write_move_player(&mut self, command: &SequencedMovePlayerCommand) {
        self.write_u32(command.sequence);
        match command.movement {
            MovePlayerCommand::Pos {
                position,
                on_ground,
            } => {
                self.write_u8(0);
                self.write_vec3d(position);
                self.write_bool(on_ground);
            }
            MovePlayerCommand::PosRot {
                position,
                y_rot_degrees,
                x_rot_degrees,
                on_ground,
            } => {
                self.write_u8(1);
                self.write_vec3d(position);
                self.write_f32(y_rot_degrees);
                self.write_f32(x_rot_degrees);
                self.write_bool(on_ground);
            }
            MovePlayerCommand::Rot {
                y_rot_degrees,
                x_rot_degrees,
                on_ground,
            } => {
                self.write_u8(2);
                self.write_f32(y_rot_degrees);
                self.write_f32(x_rot_degrees);
                self.write_bool(on_ground);
            }
            MovePlayerCommand::StatusOnly { on_ground } => {
                self.write_u8(3);
                self.write_bool(on_ground);
            }
        }
    }

    fn write_client_disconnect_reason(&mut self, reason: ClientDisconnectReason) {
        self.write_u8(match reason {
            ClientDisconnectReason::Quit => 0,
        });
    }

    fn write_session_configuration(&mut self, configuration: SessionConfiguration) {
        self.write_u32(configuration.gameplay_rate_hz);
        self.write_u32(configuration.publication_rate_hz);
        self.write_u32(configuration.body_pose_report_rate_hz);
        self.write_u32(configuration.remote_pose_replication_rate_hz);
        self.write_u8(configuration.ephemeral_transport.tag());
        self.write_u32(configuration.max_render_distance);
        self.write_u32(configuration.max_chunk_tracking_radius);
        self.write_u64(configuration.capabilities.bits());
    }

    fn write_disconnect_reason(&mut self, reason: &DisconnectReason) -> ProtocolCodecResult<()> {
        self.write_u8(match reason.code {
            DisconnectReasonCode::Timeout => 0,
            DisconnectReasonCode::DuplicateProfile => 1,
            DisconnectReasonCode::ProtocolViolation => 2,
            DisconnectReasonCode::ServerShutdown => 3,
            DisconnectReasonCode::Kicked => 4,
            DisconnectReasonCode::InternalError => 5,
            DisconnectReasonCode::EndOfStream => 6,
            DisconnectReasonCode::TransportError => 7,
            DisconnectReasonCode::ClientQuit => 8,
        });
        self.write_string("disconnect detail", &reason.detail)
    }

    fn write_accept_teleport(&mut self, command: &AcceptTeleportCommand) {
        self.write_u32(command.id);
    }

    fn write_set_carried_item(&mut self, command: &SetCarriedItemCommand) {
        self.write_u8(command.slot);
    }

    fn write_set_debug_hotbar_slot(&mut self, command: &SetDebugHotbarSlotCommand) {
        self.write_u8(command.slot);
        match command.item {
            None => self.write_u8(0),
            Some(DebugHotbarItem::Block(block_state)) => {
                self.write_u8(1);
                self.write_u32(block_state.0);
            }
            Some(DebugHotbarItem::SpawnActor(kind)) => {
                self.write_u8(2);
                self.write_u8(match kind {
                    DebugActorKind::Chicken => 0,
                    DebugActorKind::Mannequin => 1,
                });
            }
        }
    }

    fn write_player_model_kind(&mut self, kind: PlayerModelKind) {
        self.write_u8(match kind {
            PlayerModelKind::Player => 0,
            PlayerModelKind::UprightBear => 1,
        });
    }

    fn write_player_appearance(&mut self, appearance: PlayerAppearance) {
        self.write_player_model_kind(appearance.model);
    }

    fn write_set_player_appearance(&mut self, command: &SetPlayerAppearanceCommand) {
        self.write_player_appearance(command.appearance);
    }

    fn write_player_action(&mut self, command: &PlayerActionCommand) {
        self.write_block_pos(command.pos);
        self.write_direction(command.direction);
        self.write_u8(match command.kind {
            PlayerActionKind::StartDestroyBlock => 0,
            PlayerActionKind::StopDestroyBlock => 1,
            PlayerActionKind::AbortDestroyBlock => 2,
            PlayerActionKind::DebugInstantBreak => 3,
        });
    }

    fn write_use_item_on(&mut self, command: &UseItemOnCommand) {
        self.write_interaction_hand(command.hand);
        self.write_block_hit_result(command.hit);
    }

    fn write_interaction_hand(&mut self, hand: InteractionHand) {
        self.write_u8(match hand {
            InteractionHand::MainHand => 0,
            InteractionHand::OffHand => 1,
        });
    }

    fn write_status(&mut self, status: ChunkStatus) {
        self.write_u8(match status {
            ChunkStatus::Terrain => 0,
            ChunkStatus::Surface => 1,
            ChunkStatus::Features => 2,
            ChunkStatus::Light => 3,
            ChunkStatus::Full => 4,
            ChunkStatus::StructureStarts => 5,
            ChunkStatus::StructureReferences => 6,
        });
    }

    fn write_snapshot(&mut self, snapshot: &ChunkSnapshot) -> ProtocolCodecResult<()> {
        self.write_chunk_pos(snapshot.pos);
        self.write_status(snapshot.status);
        self.write_u64(snapshot.revision.0);
        self.write_i32(snapshot.min_y);
        self.write_i32(snapshot.height);
        self.write_len("chunk biomes", snapshot.biomes.len())?;
        for biome in &snapshot.biomes {
            self.write_i32(*biome);
        }
        self.write_len("chunk sections", snapshot.sections.len())?;
        for section in &snapshot.sections {
            self.write_section(section)?;
        }
        self.write_bool(snapshot.light_correct);
        self.write_len("light sections", snapshot.light_sections.len())?;
        for section in &snapshot.light_sections {
            self.write_light_section(section)?;
        }
        Ok(())
    }

    fn write_section(&mut self, section: &PackedChunkSection) -> ProtocolCodecResult<()> {
        self.write_i32(section.section_y);
        self.write_len("section palette", section.palette_state_ids.len())?;
        for state_id in &section.palette_state_ids {
            self.write_u32(state_id.0);
        }
        self.write_u8(section.bits_per_block);
        self.write_len("section packed indices", section.packed_block_indices.len())?;
        for packed in &section.packed_block_indices {
            self.write_u64(*packed);
        }
        Ok(())
    }

    fn write_light_section(&mut self, section: &PackedLightSection) -> ProtocolCodecResult<()> {
        self.write_i32(section.section_y);
        self.write_optional_light_layer("sky light layer", &section.sky)?;
        self.write_optional_light_layer("block light layer", &section.block)?;
        Ok(())
    }

    fn write_section_block_update(&mut self, update: &SectionBlockUpdate) {
        self.write_u8(update.local_x);
        self.write_u8(update.local_y);
        self.write_u8(update.local_z);
        self.write_u32(update.block_state.0);
    }

    fn write_player_position_update(&mut self, update: &PlayerPositionUpdate) {
        self.write_vec3d(update.position);
        self.write_f32(update.y_rot_degrees);
        self.write_f32(update.x_rot_degrees);
        self.write_u8(update.relative.bits());
        self.write_u32(update.last_applied_move_sequence);
        self.write_u32(update.teleport_id);
        self.write_bool(update.dismount_vehicle);
        self.write_bool(update.reset_continuity);
    }

    fn write_remote_player_id(&mut self, id: RemotePlayerId) {
        self.write_u64(id.0);
    }

    fn write_remote_player_update(&mut self, update: &RemotePlayerUpdate) {
        self.write_remote_player_id(update.id);
        self.write_player_appearance(update.appearance);
        self.write_vec3d(update.position);
        self.write_f32(update.y_rot_degrees);
        self.write_f32(update.x_rot_degrees);
        self.write_bool(update.on_ground);
    }

    fn write_entity_id(&mut self, id: EntityId) {
        self.write_u64(id.0);
    }

    fn write_entity_persistent_id(&mut self, id: EntityPersistentId) {
        self.write_u64(id.most);
        self.write_u64(id.least);
    }

    fn write_entity_snapshot(&mut self, snapshot: &EntitySnapshot) {
        self.write_entity_id(snapshot.id);
        self.write_entity_persistent_id(snapshot.persistent_id);
        self.write_entity_kind(snapshot.kind);
        self.write_optional_item_stack_snapshot(snapshot.item_stack);
        self.write_optional_mallard_snapshot_data(snapshot.mallard);
        self.write_optional_mallard_nest_snapshot_data(snapshot.mallard_nest);
        self.write_optional_animation_state(snapshot.animation)
            .expect("validated entity animation");
        self.write_vec3d(snapshot.position);
        self.write_f32(snapshot.y_rot_degrees);
        self.write_f32(snapshot.x_rot_degrees);
        self.write_optional_entity_rotation(snapshot.rotation);
        self.write_bool(snapshot.on_ground);
        self.write_f32(snapshot.width);
        self.write_f32(snapshot.height);
        self.write_u64(snapshot.tick_count);
    }

    fn write_optional_item_stack_snapshot(&mut self, stack: Option<ItemStackSnapshot>) {
        let Some(stack) = stack else {
            self.write_bool(false);
            return;
        };
        self.write_bool(true);
        self.write_item_kind(stack.kind);
        self.write_u8(stack.count);
    }

    fn write_entity_update(&mut self, update: &EntityUpdate) {
        self.write_entity_id(update.id);
        self.write_optional_item_stack_snapshot(update.item_stack);
        self.write_optional_mallard_update_data(update.mallard);
        self.write_optional_mallard_nest_update_data(update.mallard_nest);
        self.write_optional_animation_state(update.animation)
            .expect("validated entity animation");
        self.write_vec3d(update.position);
        self.write_f32(update.y_rot_degrees);
        self.write_f32(update.x_rot_degrees);
        self.write_optional_entity_rotation(update.rotation);
        self.write_bool(update.on_ground);
        self.write_u64(update.tick_count);
    }

    fn write_mallard_life_stage(&mut self, stage: MallardLifeStage) {
        self.write_u8(match stage {
            MallardLifeStage::Duckling => 0,
            MallardLifeStage::Adult => 1,
        });
    }

    fn write_optional_mallard_snapshot_data(&mut self, data: Option<MallardSnapshotData>) {
        self.write_bool(data.is_some());
        if let Some(data) = data {
            self.write_mallard_life_stage(data.life_stage);
            self.write_bool(data.in_water);
        }
    }

    fn write_optional_mallard_update_data(&mut self, data: Option<MallardUpdateData>) {
        self.write_bool(data.is_some());
        if let Some(data) = data {
            self.write_mallard_life_stage(data.life_stage);
            self.write_bool(data.in_water);
        }
    }

    fn write_optional_mallard_nest_snapshot_data(&mut self, data: Option<MallardNestSnapshotData>) {
        self.write_bool(data.is_some());
        if let Some(data) = data {
            self.write_u32(data.incubation_progress);
            self.write_u32(data.incubation_required);
            self.write_bool(data.attended);
        }
    }

    fn write_optional_animation_state(
        &mut self,
        state: Option<AnimationState>,
    ) -> ProtocolCodecResult<()> {
        self.write_bool(state.is_some());
        if let Some(state) = state {
            self.write_string("animation clip id", state.clip.as_str())?;
            self.write_u8(match state.phase_source {
                AnimationPhaseSource::Distance => 0,
                AnimationPhaseSource::Elapsed => 1,
            });
            self.write_u32(state.epoch);
            self.write_u64(state.start_tick);
        }
        Ok(())
    }

    fn write_optional_mallard_nest_update_data(&mut self, data: Option<MallardNestUpdateData>) {
        self.write_bool(data.is_some());
        if let Some(data) = data {
            self.write_u32(data.incubation_progress);
            self.write_u32(data.incubation_required);
            self.write_bool(data.attended);
        }
    }

    fn write_optional_entity_rotation(&mut self, rotation: Option<EntityRotation>) {
        let Some(rotation) = rotation else {
            self.write_bool(false);
            return;
        };
        self.write_bool(true);
        self.write_f32(rotation.x);
        self.write_f32(rotation.y);
        self.write_f32(rotation.z);
        self.write_f32(rotation.w);
    }

    fn write_optional_light_layer(
        &mut self,
        field: &'static str,
        layer: &Option<Vec<u8>>,
    ) -> ProtocolCodecResult<()> {
        match layer {
            Some(bytes) => {
                if bytes.len() != LIGHT_DATA_LAYER_BYTE_COUNT {
                    return Err(ProtocolCodecError::InvalidData(field));
                }
                self.write_bool(true);
                self.bytes.extend_from_slice(bytes);
            }
            None => self.write_bool(false),
        }
        Ok(())
    }
}

struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> ProtocolCodecResult<()> {
        let remaining = self.bytes.len() - self.offset;
        if remaining == 0 {
            Ok(())
        } else {
            Err(ProtocolCodecError::TrailingBytes { remaining })
        }
    }

    fn read_exact<const N: usize>(&mut self) -> ProtocolCodecResult<[u8; N]> {
        let remaining = self.bytes.len() - self.offset;
        if remaining < N {
            return Err(ProtocolCodecError::UnexpectedEof {
                needed: N,
                remaining,
            });
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.offset..self.offset + N]);
        self.offset += N;
        Ok(out)
    }

    fn read_u8(&mut self) -> ProtocolCodecResult<u8> {
        Ok(self.read_exact::<1>()?[0])
    }

    fn read_remaining(&mut self) -> &'a [u8] {
        let remaining = &self.bytes[self.offset..];
        self.offset = self.bytes.len();
        remaining
    }

    fn read_bool(&mut self) -> ProtocolCodecResult<bool> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ProtocolCodecError::InvalidData(
                "boolean value must be 0 or 1",
            )),
        }
    }

    fn read_i32(&mut self) -> ProtocolCodecResult<i32> {
        Ok(i32::from_le_bytes(self.read_exact::<4>()?))
    }

    fn read_i64(&mut self) -> ProtocolCodecResult<i64> {
        Ok(i64::from_le_bytes(self.read_exact::<8>()?))
    }

    fn read_u32(&mut self) -> ProtocolCodecResult<u32> {
        Ok(u32::from_le_bytes(self.read_exact::<4>()?))
    }

    fn read_u64(&mut self) -> ProtocolCodecResult<u64> {
        Ok(u64::from_le_bytes(self.read_exact::<8>()?))
    }

    fn read_f64(&mut self) -> ProtocolCodecResult<f64> {
        Ok(f64::from_le_bytes(self.read_exact::<8>()?))
    }

    fn read_f32(&mut self) -> ProtocolCodecResult<f32> {
        Ok(f32::from_le_bytes(self.read_exact::<4>()?))
    }

    fn read_len(&mut self) -> ProtocolCodecResult<usize> {
        Ok(self.read_u32()? as usize)
    }

    fn read_string(
        &mut self,
        field: &'static str,
        max_bytes: usize,
    ) -> ProtocolCodecResult<String> {
        let len = self.read_len()?;
        if len > max_bytes {
            return Err(ProtocolCodecError::InvalidData(match field {
                "disconnect detail" => "disconnect detail exceeds the protocol maximum",
                _ => "protocol string exceeds its maximum",
            }));
        }
        let remaining = self.bytes.len() - self.offset;
        if remaining < len {
            return Err(ProtocolCodecError::UnexpectedEof {
                needed: len,
                remaining,
            });
        }
        let bytes = &self.bytes[self.offset..self.offset + len];
        self.offset += len;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| ProtocolCodecError::InvalidData("protocol string is not UTF-8"))
    }

    fn read_horizontal_topology(&mut self) -> ProtocolCodecResult<HorizontalTopology> {
        let topology =
            HorizontalTopology::new(self.read_axis_topology()?, self.read_axis_topology()?);
        validate_horizontal_topology(topology)?;
        Ok(topology)
    }

    fn read_axis_topology(&mut self) -> ProtocolCodecResult<AxisTopology> {
        match self.read_u8()? {
            0 => Ok(AxisTopology::Unbounded),
            1 => Ok(AxisTopology::finite(self.read_i32()?, self.read_i32()?)),
            2 => Ok(AxisTopology::periodic(self.read_i32()?, self.read_u32()?)),
            _ => Err(ProtocolCodecError::InvalidData(
                "unknown dimension axis topology tag",
            )),
        }
    }

    fn read_chunk_pos(&mut self) -> ProtocolCodecResult<ChunkPos> {
        Ok(ChunkPos::new(self.read_i32()?, self.read_i32()?))
    }

    fn read_block_pos(&mut self) -> ProtocolCodecResult<BlockPos> {
        Ok(BlockPos::new(
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
        ))
    }

    fn read_direction(&mut self) -> ProtocolCodecResult<Direction> {
        let direction = self.read_u8()?;
        Direction::from_index(direction).ok_or(ProtocolCodecError::UnknownDirection(direction))
    }

    fn read_entity_kind(&mut self) -> ProtocolCodecResult<EntityKind> {
        let kind = self.read_u8()?;
        match kind {
            0 => Ok(EntityKind::Cow),
            1 => Ok(EntityKind::Chicken),
            2 => Ok(EntityKind::DebugCube),
            3 => Ok(EntityKind::Item),
            4 => Ok(EntityKind::Mannequin),
            5 => Ok(EntityKind::Mallard),
            6 => Ok(EntityKind::MallardNest),
            kind => Err(ProtocolCodecError::UnknownEntityKind(kind)),
        }
    }

    fn read_item_kind(&mut self) -> ProtocolCodecResult<ItemKind> {
        let kind = self.read_u8()?;
        match kind {
            0 => Ok(ItemKind::Egg),
            1 => Ok(ItemKind::MallardEgg),
            2 => Ok(ItemKind::MallardFeather),
            kind => Err(ProtocolCodecError::UnknownItemKind(kind)),
        }
    }

    fn read_vec3d(&mut self) -> ProtocolCodecResult<Vec3d> {
        let value = Vec3d::new(self.read_f64()?, self.read_f64()?, self.read_f64()?);
        if !value.is_finite() {
            return Err(ProtocolCodecError::InvalidData(
                "vec3 contains non-finite value",
            ));
        }
        Ok(value)
    }

    fn read_block_hit_result(&mut self) -> ProtocolCodecResult<BlockHitResult> {
        let miss = self.read_bool()?;
        let location = self.read_vec3d()?;
        let direction = self.read_direction()?;
        let block_pos = self.read_block_pos()?;
        let inside = self.read_bool()?;
        Ok(if miss {
            BlockHitResult::miss(location, direction, block_pos)
        } else {
            BlockHitResult::new(location, direction, block_pos, inside)
        })
    }

    fn read_move_player(&mut self) -> ProtocolCodecResult<SequencedMovePlayerCommand> {
        let sequence = self.read_u32()?;
        let movement = match self.read_u8()? {
            0 => MovePlayerCommand::Pos {
                position: self.read_vec3d()?,
                on_ground: self.read_bool()?,
            },
            1 => {
                let position = self.read_vec3d()?;
                let (y_rot_degrees, x_rot_degrees) = self.read_move_player_rotation()?;
                MovePlayerCommand::PosRot {
                    position,
                    y_rot_degrees,
                    x_rot_degrees,
                    on_ground: self.read_bool()?,
                }
            }
            2 => {
                let (y_rot_degrees, x_rot_degrees) = self.read_move_player_rotation()?;
                MovePlayerCommand::Rot {
                    y_rot_degrees,
                    x_rot_degrees,
                    on_ground: self.read_bool()?,
                }
            }
            3 => MovePlayerCommand::StatusOnly {
                on_ground: self.read_bool()?,
            },
            _ => {
                return Err(ProtocolCodecError::InvalidData(
                    "unknown move player packet variant",
                ));
            }
        };
        Ok(SequencedMovePlayerCommand::new(sequence, movement))
    }

    fn read_client_disconnect_reason(&mut self) -> ProtocolCodecResult<ClientDisconnectReason> {
        match self.read_u8()? {
            0 => Ok(ClientDisconnectReason::Quit),
            _ => Err(ProtocolCodecError::InvalidData(
                "unknown client disconnect reason",
            )),
        }
    }

    fn read_session_configuration(&mut self) -> ProtocolCodecResult<SessionConfiguration> {
        let configuration = SessionConfiguration {
            gameplay_rate_hz: self.read_u32()?,
            publication_rate_hz: self.read_u32()?,
            body_pose_report_rate_hz: self.read_u32()?,
            remote_pose_replication_rate_hz: self.read_u32()?,
            ephemeral_transport: EffectiveEphemeralTransport::from_tag(self.read_u8()?)?,
            max_render_distance: self.read_u32()?,
            max_chunk_tracking_radius: self.read_u32()?,
            capabilities: SessionCapabilities::from_bits_retain(self.read_u64()?).known(),
        };
        validate_session_configuration(&configuration)?;
        Ok(configuration)
    }

    fn read_disconnect_reason(&mut self) -> ProtocolCodecResult<DisconnectReason> {
        let code = match self.read_u8()? {
            0 => DisconnectReasonCode::Timeout,
            1 => DisconnectReasonCode::DuplicateProfile,
            2 => DisconnectReasonCode::ProtocolViolation,
            3 => DisconnectReasonCode::ServerShutdown,
            4 => DisconnectReasonCode::Kicked,
            5 => DisconnectReasonCode::InternalError,
            6 => DisconnectReasonCode::EndOfStream,
            7 => DisconnectReasonCode::TransportError,
            8 => DisconnectReasonCode::ClientQuit,
            _ => {
                return Err(ProtocolCodecError::InvalidData(
                    "unknown server disconnect reason",
                ));
            }
        };
        Ok(DisconnectReason::new(
            code,
            self.read_string("disconnect detail", MAX_DISCONNECT_DETAIL_BYTES)?,
        ))
    }

    fn read_move_player_rotation(&mut self) -> ProtocolCodecResult<(f32, f32)> {
        let y_rot_degrees = self.read_f32()?;
        let x_rot_degrees = self.read_f32()?;
        if !y_rot_degrees.is_finite() || !x_rot_degrees.is_finite() {
            return Err(ProtocolCodecError::InvalidData(
                "move player rotation contains non-finite value",
            ));
        }
        Ok((y_rot_degrees, x_rot_degrees))
    }

    fn read_accept_teleport(&mut self) -> ProtocolCodecResult<AcceptTeleportCommand> {
        Ok(AcceptTeleportCommand {
            id: self.read_u32()?,
        })
    }

    fn read_set_carried_item(&mut self) -> ProtocolCodecResult<SetCarriedItemCommand> {
        Ok(SetCarriedItemCommand {
            slot: self.read_u8()?,
        })
    }

    fn read_set_debug_hotbar_slot(&mut self) -> ProtocolCodecResult<SetDebugHotbarSlotCommand> {
        let slot = self.read_u8()?;
        let item = match self.read_u8()? {
            0 => None,
            1 => Some(DebugHotbarItem::Block(BlockStateId(self.read_u32()?))),
            2 => Some(DebugHotbarItem::SpawnActor(match self.read_u8()? {
                0 => DebugActorKind::Chicken,
                1 => DebugActorKind::Mannequin,
                _ => return Err(ProtocolCodecError::InvalidData("unknown debug actor kind")),
            })),
            _ => {
                return Err(ProtocolCodecError::InvalidData(
                    "unknown debug hotbar item kind",
                ));
            }
        };
        Ok(SetDebugHotbarSlotCommand { slot, item })
    }

    fn read_player_model_kind(&mut self) -> ProtocolCodecResult<PlayerModelKind> {
        match self.read_u8()? {
            0 => Ok(PlayerModelKind::Player),
            1 => Ok(PlayerModelKind::UprightBear),
            kind => Err(ProtocolCodecError::UnknownPlayerModelKind(kind)),
        }
    }

    fn read_player_appearance(&mut self) -> ProtocolCodecResult<PlayerAppearance> {
        Ok(PlayerAppearance {
            model: self.read_player_model_kind()?,
        })
    }

    fn read_set_player_appearance(&mut self) -> ProtocolCodecResult<SetPlayerAppearanceCommand> {
        Ok(SetPlayerAppearanceCommand {
            appearance: self.read_player_appearance()?,
        })
    }

    fn read_player_action(&mut self) -> ProtocolCodecResult<PlayerActionCommand> {
        let pos = self.read_block_pos()?;
        let direction = self.read_direction()?;
        let kind = match self.read_u8()? {
            0 => PlayerActionKind::StartDestroyBlock,
            1 => PlayerActionKind::StopDestroyBlock,
            2 => PlayerActionKind::AbortDestroyBlock,
            3 => PlayerActionKind::DebugInstantBreak,
            kind => return Err(ProtocolCodecError::UnknownPlayerActionKind(kind)),
        };
        Ok(PlayerActionCommand {
            pos,
            direction,
            kind,
        })
    }

    fn read_use_item_on(&mut self) -> ProtocolCodecResult<UseItemOnCommand> {
        let hand = self.read_interaction_hand()?;
        let hit = self.read_block_hit_result()?;
        Ok(UseItemOnCommand { hand, hit })
    }

    fn read_interaction_hand(&mut self) -> ProtocolCodecResult<InteractionHand> {
        let hand = self.read_u8()?;
        match hand {
            0 => Ok(InteractionHand::MainHand),
            1 => Ok(InteractionHand::OffHand),
            hand => Err(ProtocolCodecError::UnknownInteractionHand(hand)),
        }
    }

    fn read_status(&mut self) -> ProtocolCodecResult<ChunkStatus> {
        match self.read_u8()? {
            0 => Ok(ChunkStatus::Terrain),
            1 => Ok(ChunkStatus::Surface),
            2 => Ok(ChunkStatus::Features),
            3 => Ok(ChunkStatus::Light),
            4 => Ok(ChunkStatus::Full),
            5 => Ok(ChunkStatus::StructureStarts),
            6 => Ok(ChunkStatus::StructureReferences),
            status => Err(ProtocolCodecError::UnknownChunkStatus(status)),
        }
    }

    fn read_snapshot(&mut self) -> ProtocolCodecResult<ChunkSnapshot> {
        let pos = self.read_chunk_pos()?;
        let status = self.read_status()?;
        let revision = ChunkRevision(self.read_u64()?);
        let min_y = self.read_i32()?;
        let height = self.read_i32()?;
        let biomes = (0..self.read_len()?)
            .map(|_| self.read_i32())
            .collect::<ProtocolCodecResult<Vec<_>>>()?;
        let sections = (0..self.read_len()?)
            .map(|_| self.read_section())
            .collect::<ProtocolCodecResult<Vec<_>>>()?;
        let light_correct = self.read_bool()?;
        let light_sections = (0..self.read_len()?)
            .map(|_| self.read_light_section())
            .collect::<ProtocolCodecResult<Vec<_>>>()?;

        Ok(ChunkSnapshot {
            pos,
            status,
            revision,
            min_y,
            height,
            biomes,
            sections,
            light_correct,
            light_sections,
        })
    }

    fn read_section(&mut self) -> ProtocolCodecResult<PackedChunkSection> {
        let section_y = self.read_i32()?;
        let palette_state_ids = (0..self.read_len()?)
            .map(|_| self.read_u32().map(BlockStateId))
            .collect::<ProtocolCodecResult<Vec<_>>>()?;
        if palette_state_ids.is_empty() {
            return Err(ProtocolCodecError::InvalidData(
                "packed chunk section palette is empty",
            ));
        }
        let bits_per_block = self.read_u8()?;
        let packed_block_indices = (0..self.read_len()?)
            .map(|_| self.read_u64())
            .collect::<ProtocolCodecResult<Vec<_>>>()?;

        Ok(PackedChunkSection {
            section_y,
            palette_state_ids,
            bits_per_block,
            packed_block_indices,
        })
    }

    fn read_light_section(&mut self) -> ProtocolCodecResult<PackedLightSection> {
        let section_y = self.read_i32()?;
        let sky = self.read_optional_light_layer()?;
        let block = self.read_optional_light_layer()?;
        Ok(PackedLightSection::new(section_y, sky, block))
    }

    fn read_section_block_update(&mut self) -> ProtocolCodecResult<SectionBlockUpdate> {
        let update = SectionBlockUpdate {
            local_x: self.read_u8()?,
            local_y: self.read_u8()?,
            local_z: self.read_u8()?,
            block_state: BlockStateId(self.read_u32()?),
        };
        validate_section_block_update(&update)?;
        Ok(update)
    }

    fn read_player_position_update(&mut self) -> ProtocolCodecResult<PlayerPositionUpdate> {
        let position = self.read_vec3d()?;
        let y_rot_degrees = self.read_f32()?;
        let x_rot_degrees = self.read_f32()?;
        if !y_rot_degrees.is_finite() || !x_rot_degrees.is_finite() {
            return Err(ProtocolCodecError::InvalidData(
                "player position update rotation contains non-finite value",
            ));
        }
        let relative = PlayerPositionRelativeFlags::from_bits(self.read_u8()?).ok_or(
            ProtocolCodecError::InvalidData("unknown player position relative flag"),
        )?;
        Ok(PlayerPositionUpdate {
            position,
            y_rot_degrees,
            x_rot_degrees,
            relative,
            last_applied_move_sequence: self.read_u32()?,
            teleport_id: self.read_u32()?,
            dismount_vehicle: self.read_bool()?,
            reset_continuity: self.read_bool()?,
        })
    }

    fn read_remote_player_id(&mut self) -> ProtocolCodecResult<RemotePlayerId> {
        Ok(RemotePlayerId(self.read_u64()?))
    }

    fn read_remote_player_update(&mut self) -> ProtocolCodecResult<RemotePlayerUpdate> {
        let id = self.read_remote_player_id()?;
        let appearance = self.read_player_appearance()?;
        let position = self.read_vec3d()?;
        let y_rot_degrees = self.read_f32()?;
        let x_rot_degrees = self.read_f32()?;
        if !y_rot_degrees.is_finite() || !x_rot_degrees.is_finite() {
            return Err(ProtocolCodecError::InvalidData(
                "remote player update rotation contains non-finite value",
            ));
        }
        Ok(RemotePlayerUpdate {
            id,
            appearance,
            position,
            y_rot_degrees,
            x_rot_degrees,
            on_ground: self.read_bool()?,
        })
    }

    fn read_entity_id(&mut self) -> ProtocolCodecResult<EntityId> {
        Ok(EntityId(self.read_u64()?))
    }

    fn read_entity_persistent_id(&mut self) -> ProtocolCodecResult<EntityPersistentId> {
        Ok(EntityPersistentId::new(self.read_u64()?, self.read_u64()?))
    }

    fn read_entity_snapshot(&mut self) -> ProtocolCodecResult<EntitySnapshot> {
        let snapshot = EntitySnapshot {
            id: self.read_entity_id()?,
            persistent_id: self.read_entity_persistent_id()?,
            kind: self.read_entity_kind()?,
            item_stack: self.read_optional_item_stack_snapshot()?,
            mallard: self.read_optional_mallard_snapshot_data()?,
            mallard_nest: self.read_optional_mallard_nest_snapshot_data()?,
            animation: self.read_optional_animation_state()?,
            position: self.read_vec3d()?,
            y_rot_degrees: self.read_f32()?,
            x_rot_degrees: self.read_f32()?,
            rotation: self.read_optional_entity_rotation()?,
            on_ground: self.read_bool()?,
            width: self.read_f32()?,
            height: self.read_f32()?,
            tick_count: self.read_u64()?,
        };
        validate_entity_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn read_optional_item_stack_snapshot(
        &mut self,
    ) -> ProtocolCodecResult<Option<ItemStackSnapshot>> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let stack = ItemStackSnapshot {
            kind: self.read_item_kind()?,
            count: self.read_u8()?,
        };
        validate_item_stack_snapshot(stack)?;
        Ok(Some(stack))
    }

    fn read_mallard_life_stage(&mut self) -> ProtocolCodecResult<MallardLifeStage> {
        match self.read_u8()? {
            0 => Ok(MallardLifeStage::Duckling),
            1 => Ok(MallardLifeStage::Adult),
            _ => Err(ProtocolCodecError::InvalidData(
                "unknown mallard life stage",
            )),
        }
    }

    fn read_optional_mallard_snapshot_data(
        &mut self,
    ) -> ProtocolCodecResult<Option<MallardSnapshotData>> {
        self.read_bool()?
            .then(|| {
                Ok(MallardSnapshotData {
                    life_stage: self.read_mallard_life_stage()?,
                    in_water: self.read_bool()?,
                })
            })
            .transpose()
    }

    fn read_optional_mallard_update_data(
        &mut self,
    ) -> ProtocolCodecResult<Option<MallardUpdateData>> {
        self.read_bool()?
            .then(|| {
                Ok(MallardUpdateData {
                    life_stage: self.read_mallard_life_stage()?,
                    in_water: self.read_bool()?,
                })
            })
            .transpose()
    }

    fn read_optional_mallard_nest_snapshot_data(
        &mut self,
    ) -> ProtocolCodecResult<Option<MallardNestSnapshotData>> {
        self.read_bool()?
            .then(|| {
                Ok(MallardNestSnapshotData {
                    incubation_progress: self.read_u32()?,
                    incubation_required: self.read_u32()?,
                    attended: self.read_bool()?,
                })
            })
            .transpose()
    }

    fn read_optional_mallard_nest_update_data(
        &mut self,
    ) -> ProtocolCodecResult<Option<MallardNestUpdateData>> {
        self.read_bool()?
            .then(|| {
                Ok(MallardNestUpdateData {
                    incubation_progress: self.read_u32()?,
                    incubation_required: self.read_u32()?,
                    attended: self.read_bool()?,
                })
            })
            .transpose()
    }

    fn read_optional_animation_state(&mut self) -> ProtocolCodecResult<Option<AnimationState>> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let clip = self.read_string("animation clip id", MAX_ANIMATION_CLIP_ID_BYTES)?;
        let clip = AnimationClipId::parse(&clip)
            .map_err(|_| ProtocolCodecError::InvalidData("entity animation has invalid clip id"))?;
        let phase_source = match self.read_u8()? {
            0 => AnimationPhaseSource::Distance,
            1 => AnimationPhaseSource::Elapsed,
            _ => {
                return Err(ProtocolCodecError::InvalidData(
                    "entity animation has unknown phase source",
                ));
            }
        };
        let state = AnimationState {
            clip,
            phase_source,
            epoch: self.read_u32()?,
            start_tick: self.read_u64()?,
        };
        if phase_source == AnimationPhaseSource::Distance && state.start_tick != 0 {
            return Err(ProtocolCodecError::InvalidData(
                "distance animation must not carry an elapsed start tick",
            ));
        }
        Ok(Some(state))
    }

    fn read_entity_update(&mut self) -> ProtocolCodecResult<EntityUpdate> {
        let update = EntityUpdate {
            id: self.read_entity_id()?,
            item_stack: self.read_optional_item_stack_snapshot()?,
            mallard: self.read_optional_mallard_update_data()?,
            mallard_nest: self.read_optional_mallard_nest_update_data()?,
            animation: self.read_optional_animation_state()?,
            position: self.read_vec3d()?,
            y_rot_degrees: self.read_f32()?,
            x_rot_degrees: self.read_f32()?,
            rotation: self.read_optional_entity_rotation()?,
            on_ground: self.read_bool()?,
            tick_count: self.read_u64()?,
        };
        validate_entity_update(&update)?;
        Ok(update)
    }

    fn read_optional_entity_rotation(&mut self) -> ProtocolCodecResult<Option<EntityRotation>> {
        if !self.read_bool()? {
            return Ok(None);
        }
        Ok(Some(EntityRotation {
            x: self.read_f32()?,
            y: self.read_f32()?,
            z: self.read_f32()?,
            w: self.read_f32()?,
        }))
    }

    fn read_optional_light_layer(&mut self) -> ProtocolCodecResult<Option<Vec<u8>>> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let remaining = self.bytes.len() - self.offset;
        if remaining < LIGHT_DATA_LAYER_BYTE_COUNT {
            return Err(ProtocolCodecError::UnexpectedEof {
                needed: LIGHT_DATA_LAYER_BYTE_COUNT,
                remaining,
            });
        }
        let end = self.offset + LIGHT_DATA_LAYER_BYTE_COUNT;
        let bytes = self.bytes[self.offset..end].to_vec();
        self.offset = end;
        Ok(Some(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus,
        LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection, chunk_section_index,
    };

    #[test]
    fn chunk_view_is_data_only() {
        let view = ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 8,
            chunk_tracking_radius: 9,
        };
        assert_eq!(view.render_distance, 8);
        assert_eq!(view.chunk_tracking_radius, 9);
    }

    #[test]
    fn default_debug_hotbar_exposes_actor_tools_in_final_slots() {
        assert_eq!(
            DEFAULT_DEBUG_HOTBAR[7],
            Some(DebugHotbarItem::SpawnActor(DebugActorKind::Chicken))
        );
        assert_eq!(
            DEFAULT_DEBUG_HOTBAR[8],
            Some(DebugHotbarItem::SpawnActor(DebugActorKind::Mannequin))
        );
    }

    #[test]
    fn distinguishes_view_commands_from_server_updates() {
        let view = ChunkView {
            center: ChunkPos::new(1, -2),
            render_distance: 3,
            chunk_tracking_radius: 4,
        };
        let command = ClientCommand::SetChunkView(view.clone());
        let unload = ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(4, 5),
        };

        assert_eq!(command, ClientCommand::SetChunkView(view));
        assert_eq!(
            unload,
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(4, 5)
            }
        );
    }

    #[test]
    fn server_update_can_publish_chunk_snapshot() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );

        assert_eq!(
            ServerUpdate::ChunkSnapshot(snapshot.clone()),
            ServerUpdate::ChunkSnapshot(snapshot)
        );
    }

    #[test]
    fn client_command_codec_round_trips_chunk_view() {
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(-12, 34),
            render_distance: 3,
            chunk_tracking_radius: 4,
        });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn client_command_codec_round_trips_move_player() {
        for command in [
            ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(-1.25, 63.0, 12.5),
                on_ground: true,
            }),
            ClientCommand::sequenced_move_player(
                7,
                MovePlayerCommand::PosRot {
                    position: Vec3d::new(-1.25, 63.0, 12.5),
                    y_rot_degrees: -181.5,
                    x_rot_degrees: 45.25,
                    on_ground: true,
                },
            ),
            ClientCommand::move_player(MovePlayerCommand::Rot {
                y_rot_degrees: -181.5,
                x_rot_degrees: 45.25,
                on_ground: false,
            }),
            ClientCommand::move_player(MovePlayerCommand::StatusOnly { on_ground: true }),
        ] {
            let bytes = encode_client_command(&command).unwrap();

            assert_eq!(decode_client_command(&bytes).unwrap(), command);
        }
    }

    #[test]
    fn reliable_fallback_round_trips_ephemeral_pose_messages() {
        let pose = PlayerBodyPoseSample::new(
            3,
            u32::MAX,
            17_500,
            Vec3d::new(4.5, 70.0, -8.25),
            125.0,
            -20.0,
            true,
        );
        let client = ClientCommand::EphemeralFallback(ClientEphemeralMessage::BodyPose(pose));
        let server = ServerUpdate::EphemeralFallback(ServerEphemeralMessage::RemoteBodyPose(
            RemotePlayerBodyPoseSample {
                id: RemotePlayerId(42),
                pose,
            },
        ));

        let client_bytes = encode_client_command(&client).unwrap();
        let server_bytes = encode_server_update(&server).unwrap();

        assert_eq!(decode_client_command(&client_bytes).unwrap(), client);
        assert_eq!(decode_server_update(&server_bytes).unwrap(), server);
    }

    #[test]
    fn client_control_commands_round_trip() {
        for command in [
            ClientCommand::KeepAlive { id: 0x1234_5678 },
            ClientCommand::Respawn,
            ClientCommand::Disconnect(ClientDisconnectReason::Quit),
        ] {
            let bytes = encode_client_command(&command).unwrap();
            assert_eq!(decode_client_command(&bytes).unwrap(), command);
        }
    }

    #[test]
    fn client_command_codec_round_trips_accept_teleport() {
        let command = ClientCommand::AcceptTeleport(AcceptTeleportCommand { id: 37 });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn client_command_codec_round_trips_set_carried_item() {
        let command = ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 7 });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn client_command_codec_round_trips_set_debug_hotbar_slot() {
        let command = ClientCommand::SetDebugHotbarSlot(SetDebugHotbarSlotCommand {
            slot: 3,
            item: Some(DebugHotbarItem::Block(BlockStateId(91))),
        });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);

        let clear_command = ClientCommand::SetDebugHotbarSlot(SetDebugHotbarSlotCommand {
            slot: 8,
            item: None,
        });
        let clear_bytes = encode_client_command(&clear_command).unwrap();

        assert_eq!(decode_client_command(&clear_bytes).unwrap(), clear_command);

        let actor_command = ClientCommand::SetDebugHotbarSlot(SetDebugHotbarSlotCommand {
            slot: 7,
            item: Some(DebugHotbarItem::SpawnActor(DebugActorKind::Chicken)),
        });
        let actor_bytes = encode_client_command(&actor_command).unwrap();

        assert_eq!(decode_client_command(&actor_bytes).unwrap(), actor_command);
    }

    #[test]
    fn client_command_codec_round_trips_set_player_appearance() {
        let command = ClientCommand::SetPlayerAppearance(SetPlayerAppearanceCommand {
            appearance: PlayerAppearance {
                model: PlayerModelKind::UprightBear,
            },
        });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn client_command_codec_round_trips_player_action() {
        let command = ClientCommand::PlayerAction(PlayerActionCommand {
            pos: BlockPos::new(-1, 64, 12),
            direction: Direction::North,
            kind: PlayerActionKind::DebugInstantBreak,
        });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn client_command_codec_round_trips_use_item_on() {
        let command = ClientCommand::UseItemOn(UseItemOnCommand {
            hand: InteractionHand::MainHand,
            hit: BlockHitResult::new(
                Vec3d::new(1.25, 64.0, -3.5),
                Direction::Up,
                BlockPos::new(1, 63, -4),
                false,
            ),
        });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn client_command_codec_round_trips_shoot_debug_physics_cube() {
        let command = ClientCommand::ShootDebugPhysicsCube;

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);
    }

    #[test]
    fn server_update_codec_round_trips_chunk_snapshot() {
        let mut blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        blocks[chunk_section_index(1, 2, 3)] = BlockStateId(9);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(2, -5),
            ChunkStatus::Surface,
            ChunkRevision(42),
            -64,
            16,
            &blocks,
        )
        .with_biomes(vec![6; mclone_core::expected_chunk_biome_count(16)])
        .with_light_sections(
            false,
            vec![
                PackedLightSection::new(-4, Some(vec![0xFF; LIGHT_DATA_LAYER_BYTE_COUNT]), None),
                PackedLightSection::new(-3, None, Some(vec![0x11; LIGHT_DATA_LAYER_BYTE_COUNT])),
                PackedLightSection::new(
                    -2,
                    Some(vec![0x22; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    Some(vec![0x33; LIGHT_DATA_LAYER_BYTE_COUNT]),
                ),
            ],
        );
        let update = ServerUpdate::ChunkSnapshot(snapshot);

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_unload() {
        let update = ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(7, -8),
        };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_player_position() {
        let update = ServerUpdate::PlayerPosition(PlayerPositionUpdate {
            position: Vec3d::new(1.25, 63.0, -4.5),
            y_rot_degrees: 181.0,
            x_rot_degrees: -45.0,
            relative: PlayerPositionRelativeFlags {
                x: true,
                y: false,
                z: true,
                y_rot: true,
                x_rot: false,
            },
            last_applied_move_sequence: 41,
            teleport_id: 42,
            dismount_vehicle: true,
            reset_continuity: true,
        });

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_player_statistics() {
        let mut statistics = PlayerStatistics::default();
        statistics.set(StatisticKey::jump(), 42);
        statistics.set(StatisticKey::successful_block_placement(), 17);
        let update = ServerUpdate::PlayerStatistics { statistics };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_atomic_player_life() {
        let update = ServerUpdate::PlayerLife(
            PlayerLifeState::new(
                9,
                PlayerVitals::new(0.0, 20.0).unwrap(),
                Some(PlayerDamageCause::Lava),
            )
            .unwrap(),
        );

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_player_experience() {
        let update = ServerUpdate::PlayerExperience {
            total_experience: 1_234,
        };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_session_updates_round_trip() {
        let capabilities = SessionCapabilities::DEVELOPMENT_DEFAULT;
        for update in [
            ServerUpdate::SessionConfiguration(SessionConfiguration::fixed_vanilla(
                10,
                11,
                capabilities,
            )),
            ServerUpdate::SessionReady,
            ServerUpdate::KeepAlive { id: 9_876 },
            ServerUpdate::Disconnect(DisconnectReason::new(
                DisconnectReasonCode::DuplicateProfile,
                "profile is already connected",
            )),
        ] {
            let bytes = encode_server_update(&update).unwrap();
            assert_eq!(decode_server_update(&bytes).unwrap(), update);
        }
    }

    #[test]
    fn session_capability_intersection_ignores_unknown_bits() {
        let client = SessionCapabilities::from_bits_retain(
            SessionCapabilities::DEBUG_ACTIONS.bits() | (1 << 63),
        );
        assert_eq!(
            client
                .intersection(SessionCapabilities::DEVELOPMENT_DEFAULT)
                .known(),
            SessionCapabilities::DEBUG_ACTIONS
        );
    }

    #[test]
    fn disconnect_detail_is_bounded_on_construction() {
        let reason = DisconnectReason::transport_error("é".repeat(300));
        assert!(reason.detail.len() <= MAX_DISCONNECT_DETAIL_BYTES);
        assert!(reason.detail.is_char_boundary(reason.detail.len()));
        let bytes = encode_server_update(&ServerUpdate::Disconnect(reason.clone())).unwrap();
        assert_eq!(
            decode_server_update(&bytes).unwrap(),
            ServerUpdate::Disconnect(reason)
        );
    }

    #[test]
    fn invalid_session_configuration_is_rejected() {
        let update = ServerUpdate::SessionConfiguration(SessionConfiguration {
            gameplay_rate_hz: 0,
            publication_rate_hz: 20,
            body_pose_report_rate_hz: 20,
            remote_pose_replication_rate_hz: 20,
            ephemeral_transport: EffectiveEphemeralTransport::ReliableFallback,
            max_render_distance: 10,
            max_chunk_tracking_radius: 11,
            capabilities: SessionCapabilities::NONE,
        });
        assert_eq!(
            encode_server_update(&update),
            Err(ProtocolCodecError::InvalidData(
                "session gameplay rate must be nonzero"
            ))
        );
    }

    #[test]
    fn server_update_codec_round_trips_remote_player_updates() {
        let update = RemotePlayerUpdate {
            id: RemotePlayerId(42),
            appearance: PlayerAppearance {
                model: PlayerModelKind::UprightBear,
            },
            position: Vec3d::new(12.5, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            on_ground: true,
        };

        for server_update in [
            ServerUpdate::RemotePlayerAdd(update),
            ServerUpdate::RemotePlayerUpdate(update),
            ServerUpdate::RemotePlayerRemove { id: update.id },
        ] {
            let bytes = encode_server_update(&server_update).unwrap();

            assert_eq!(decode_server_update(&bytes).unwrap(), server_update);
        }
    }

    #[test]
    fn server_update_codec_round_trips_entity_updates() {
        let snapshot = EntitySnapshot {
            id: EntityId(7),
            persistent_id: EntityPersistentId::new(0x1234, 0x5678),
            kind: EntityKind::Mallard,
            item_stack: None,
            mallard: Some(MallardSnapshotData {
                life_stage: MallardLifeStage::Adult,
                in_water: true,
            }),
            mallard_nest: None,
            animation: Some(AnimationState::elapsed(
                AnimationClipId::from_static("alert"),
                3,
                10,
            )),
            position: Vec3d::new(12.5, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: Some(EntityRotation {
                x: 0.0,
                y: 0.707_106_77,
                z: 0.0,
                w: 0.707_106_77,
            }),
            on_ground: true,
            width: 1.0,
            height: 1.0,
            tick_count: 12,
        };
        let update = EntityUpdate {
            id: snapshot.id,
            item_stack: None,
            mallard: Some(MallardUpdateData {
                life_stage: MallardLifeStage::Adult,
                in_water: false,
            }),
            mallard_nest: None,
            animation: Some(AnimationState::distance(
                AnimationClipId::from_static("waddle"),
                4,
            )),
            position: Vec3d::new(13.5, 70.0, -3.25),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            rotation: Some(EntityRotation {
                x: 0.0,
                y: 0.0,
                z: 0.382_683_43,
                w: 0.923_879_5,
            }),
            on_ground: true,
            tick_count: 13,
        };

        for server_update in [
            ServerUpdate::EntitySnapshot(snapshot),
            ServerUpdate::EntityUpdate(update),
            ServerUpdate::EntityRemove { id: snapshot.id },
        ] {
            let bytes = encode_server_update(&server_update).unwrap();

            assert_eq!(decode_server_update(&bytes).unwrap(), server_update);
        }
    }

    #[test]
    fn entity_persistent_id_formats_as_canonical_uuid_text() {
        let id = EntityPersistentId::new(0x0011_2233_4455_6677, 0x8899_aabb_ccdd_eeff);

        assert_eq!(id.to_string(), "00112233-4455-6677-8899-aabbccddeeff");
    }

    #[test]
    fn server_update_codec_round_trips_item_entity_snapshot() {
        let snapshot = EntitySnapshot {
            id: EntityId(8),
            persistent_id: EntityPersistentId::new(0x1234, 0x5679),
            kind: EntityKind::Item,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::MallardEgg,
                count: 1,
            }),
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: Vec3d::new(12.5, 64.0, -3.25),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: false,
            width: 0.25,
            height: 0.25,
            tick_count: 0,
        };
        let update = ServerUpdate::EntitySnapshot(snapshot);
        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);

        let update = ServerUpdate::EntityUpdate(EntityUpdate {
            id: snapshot.id,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::MallardEgg,
                count: 2,
            }),
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: snapshot.position,
            y_rot_degrees: snapshot.y_rot_degrees,
            x_rot_degrees: snapshot.x_rot_degrees,
            rotation: snapshot.rotation,
            on_ground: true,
            tick_count: 1,
        });
        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn remote_player_update_codec_rejects_non_finite_values() {
        let update = ServerUpdate::RemotePlayerUpdate(RemotePlayerUpdate {
            id: RemotePlayerId(42),
            appearance: PlayerAppearance::default(),
            position: Vec3d::new(f64::NAN, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            on_ground: true,
        });

        assert_eq!(
            encode_server_update(&update),
            Err(ProtocolCodecError::InvalidData(
                "remote player update contains non-finite value"
            ))
        );
    }

    #[test]
    fn entity_update_codec_rejects_invalid_values() {
        let non_finite = ServerUpdate::EntityUpdate(EntityUpdate {
            id: EntityId(42),
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: Vec3d::new(f64::NAN, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            tick_count: 1,
        });
        assert_eq!(
            encode_server_update(&non_finite),
            Err(ProtocolCodecError::InvalidData(
                "entity update contains non-finite value"
            ))
        );

        let invalid_dimensions = ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(42),
            persistent_id: EntityPersistentId::new(0x1234, 42),
            kind: EntityKind::Chicken,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            width: 0.0,
            height: 0.7,
            tick_count: 1,
        });
        assert_eq!(
            encode_server_update(&invalid_dimensions),
            Err(ProtocolCodecError::InvalidData(
                "entity snapshot contains invalid dimensions"
            ))
        );

        let missing_item_stack = ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(42),
            persistent_id: EntityPersistentId::new(0x1234, 42),
            kind: EntityKind::Item,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            width: 0.25,
            height: 0.25,
            tick_count: 1,
        });
        assert_eq!(
            encode_server_update(&missing_item_stack),
            Err(ProtocolCodecError::InvalidData(
                "item entity snapshot missing item stack"
            ))
        );

        let misplaced_item_stack = ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(42),
            persistent_id: EntityPersistentId::new(0x1234, 42),
            kind: EntityKind::Chicken,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            }),
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            width: 0.4,
            height: 0.7,
            tick_count: 1,
        });
        assert_eq!(
            encode_server_update(&misplaced_item_stack),
            Err(ProtocolCodecError::InvalidData(
                "non-item entity snapshot contains item stack"
            ))
        );

        let invalid_rotation = ServerUpdate::EntityUpdate(EntityUpdate {
            id: EntityId(42),
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            animation: None,
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: Some(EntityRotation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            }),
            on_ground: true,
            tick_count: 1,
        });
        assert_eq!(
            encode_server_update(&invalid_rotation),
            Err(ProtocolCodecError::InvalidData(
                "entity rotation has zero length"
            ))
        );
    }

    #[test]
    fn player_position_relative_flags_match_java_bit_layout() {
        let flags = PlayerPositionRelativeFlags {
            x: true,
            y: true,
            z: false,
            y_rot: true,
            x_rot: true,
        };

        assert_eq!(flags.bits(), 0b0001_1011);
        assert_eq!(
            PlayerPositionRelativeFlags::from_bits(flags.bits()),
            Some(flags)
        );
        assert_eq!(PlayerPositionRelativeFlags::from_bits(0b0010_0000), None);
    }

    #[test]
    fn server_update_codec_round_trips_time() {
        let update = ServerUpdate::TimeUpdate {
            game_time: 12_345,
            day_time: 1_000,
            daylight_cycle_running: false,
        };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_world_info() {
        let update = ServerUpdate::WorldInfo {
            dimension: DimensionKey::parse("mclone:moon").unwrap(),
            biome_zoom_seed: -1_234_567_890,
            topology: HorizontalTopology::UNBOUNDED,
        };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_dimension_change() {
        let update = ServerUpdate::DimensionChange {
            dimension: DimensionKey::parse("mclone:moon").unwrap(),
            biome_zoom_seed: -1_234_567_890,
            topology: HorizontalTopology::cylinder_x(0, 32),
            keep_player_state: true,
        };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn server_update_codec_round_trips_section_block_updates() {
        let update = ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(-2, 5),
            section_y: -3,
            updates: vec![
                SectionBlockUpdate {
                    local_x: 1,
                    local_y: 2,
                    local_z: 3,
                    block_state: BlockStateId(17),
                },
                SectionBlockUpdate {
                    local_x: 15,
                    local_y: 15,
                    local_z: 15,
                    block_state: BlockStateId(0),
                },
            ],
        };

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
    }

    #[test]
    fn section_block_update_codec_rejects_invalid_payloads() {
        let empty = ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 0,
            updates: Vec::new(),
        };
        assert_eq!(
            encode_server_update(&empty),
            Err(ProtocolCodecError::InvalidData(
                "section block update list is empty"
            ))
        );

        let invalid = ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 0,
            updates: vec![SectionBlockUpdate {
                local_x: 16,
                local_y: 0,
                local_z: 0,
                block_state: BlockStateId(1),
            }],
        };
        assert_eq!(
            encode_server_update(&invalid),
            Err(ProtocolCodecError::InvalidData("section update local_x"))
        );
    }

    #[test]
    fn codec_rejects_trailing_bytes() {
        let mut bytes = encode_client_command(&ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 2,
            chunk_tracking_radius: 3,
        }))
        .unwrap();
        bytes.push(99);

        assert_eq!(
            decode_client_command(&bytes),
            Err(ProtocolCodecError::TrailingBytes { remaining: 1 })
        );
    }

    #[test]
    fn codec_rejects_unknown_message_tags() {
        assert_eq!(
            decode_client_command(&[99]),
            Err(ProtocolCodecError::UnknownClientCommandTag(99))
        );
        assert_eq!(
            decode_server_update(&[88]),
            Err(ProtocolCodecError::UnknownServerUpdateTag(88))
        );
    }
}
