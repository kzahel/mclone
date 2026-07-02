#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

use mclone_core::{
    BlockHitResult, BlockPos, BlockStateId, CHUNK_WIDTH, ChunkPos, ChunkRevision, ChunkSnapshot,
    ChunkStatus, Direction, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection, PackedLightSection,
    SECTION_HEIGHT, Vec3d,
};

pub const PROTOCOL_VERSION: u32 = 18;
pub const HOTBAR_SLOT_COUNT: u8 = 9;
pub const HOTBAR_SLOT_COUNT_USIZE: usize = HOTBAR_SLOT_COUNT as usize;
pub const DEFAULT_DEBUG_HOTBAR: [Option<BlockStateId>; HOTBAR_SLOT_COUNT_USIZE] = [
    Some(BlockStateId(1)),
    Some(BlockStateId(5)),
    Some(BlockStateId(4)),
    Some(BlockStateId(6)),
    Some(BlockStateId(41)),
    Some(BlockStateId(42)),
    Some(BlockStateId(8)),
    Some(BlockStateId(91)),
    Some(BlockStateId(100)),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkView {
    pub center: ChunkPos,
    pub render_distance: u32,
    pub chunk_tracking_radius: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientCommand {
    SetChunkView(ChunkView),
    MovePlayer(MovePlayerCommand),
    AcceptTeleport(AcceptTeleportCommand),
    SetCarriedItem(SetCarriedItemCommand),
    SetDebugHotbarSlot(SetDebugHotbarSlotCommand),
    SetPlayerAppearance(SetPlayerAppearanceCommand),
    PlayerAction(PlayerActionCommand),
    UseItemOn(UseItemOnCommand),
    ShootDebugPhysicsCube,
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
    pub block_state: Option<BlockStateId>,
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
    ChunkSnapshot(ChunkSnapshot),
    ChunkUnload {
        pos: ChunkPos,
    },
    SectionBlockUpdates {
        pos: ChunkPos,
        section_y: i32,
        updates: Vec<SectionBlockUpdate>,
    },
    /// Authoritative world day-time (in ticks) for the day/night cycle. Mirrors
    /// the day-time half of Java's `ClientboundSetTimePacket`.
    TimeUpdate {
        day_time: u64,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityKind {
    Cow,
    Chicken,
    DebugCube,
    Item,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Egg,
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
    pub kind: EntityKind,
    pub item_stack: Option<ItemStackSnapshot>,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub width: f32,
    pub height: f32,
    pub age_ticks: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityUpdate {
    pub id: EntityId,
    pub item_stack: Option<ItemStackSnapshot>,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub age_ticks: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerPositionUpdate {
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub relative: PlayerPositionRelativeFlags,
    pub teleport_id: u32,
    pub dismount_vehicle: bool,
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
        _ => return Err(ProtocolCodecError::UnknownClientCommandTag(tag)),
    };
    reader.finish()?;
    Ok(command)
}

pub fn encode_server_update(update: &ServerUpdate) -> ProtocolCodecResult<Vec<u8>> {
    let mut writer = ByteWriter::new();
    match update {
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
        ServerUpdate::TimeUpdate { day_time } => {
            writer.write_u8(SERVER_UPDATE_TIME);
            writer.write_u64(*day_time);
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
    }
    Ok(writer.into_inner())
}

pub fn decode_server_update(bytes: &[u8]) -> ProtocolCodecResult<ServerUpdate> {
    let mut reader = ByteReader::new(bytes);
    let tag = reader.read_u8()?;
    let update = match tag {
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
            day_time: reader.read_u64()?,
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

fn validate_entity_snapshot(snapshot: &EntitySnapshot) -> ProtocolCodecResult<()> {
    validate_entity_update(&EntityUpdate {
        id: snapshot.id,
        item_stack: snapshot.item_stack,
        position: snapshot.position,
        y_rot_degrees: snapshot.y_rot_degrees,
        x_rot_degrees: snapshot.x_rot_degrees,
        rotation: snapshot.rotation,
        on_ground: snapshot.on_ground,
        age_ticks: snapshot.age_ticks,
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

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_i32(&mut self, value: i32) {
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

    fn write_chunk_pos(&mut self, pos: ChunkPos) {
        self.write_i32(pos.x);
        self.write_i32(pos.z);
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
        });
    }

    fn write_item_kind(&mut self, kind: ItemKind) {
        self.write_u8(match kind {
            ItemKind::Egg => 0,
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

    fn write_move_player(&mut self, command: &MovePlayerCommand) {
        match *command {
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

    fn write_accept_teleport(&mut self, command: &AcceptTeleportCommand) {
        self.write_u32(command.id);
    }

    fn write_set_carried_item(&mut self, command: &SetCarriedItemCommand) {
        self.write_u8(command.slot);
    }

    fn write_set_debug_hotbar_slot(&mut self, command: &SetDebugHotbarSlotCommand) {
        self.write_u8(command.slot);
        self.write_bool(command.block_state.is_some());
        if let Some(block_state) = command.block_state {
            self.write_u32(block_state.0);
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
        self.write_u32(update.teleport_id);
        self.write_bool(update.dismount_vehicle);
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

    fn write_entity_snapshot(&mut self, snapshot: &EntitySnapshot) {
        self.write_entity_id(snapshot.id);
        self.write_entity_kind(snapshot.kind);
        self.write_optional_item_stack_snapshot(snapshot.item_stack);
        self.write_vec3d(snapshot.position);
        self.write_f32(snapshot.y_rot_degrees);
        self.write_f32(snapshot.x_rot_degrees);
        self.write_optional_entity_rotation(snapshot.rotation);
        self.write_bool(snapshot.on_ground);
        self.write_f32(snapshot.width);
        self.write_f32(snapshot.height);
        self.write_u64(snapshot.age_ticks);
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
        self.write_vec3d(update.position);
        self.write_f32(update.y_rot_degrees);
        self.write_f32(update.x_rot_degrees);
        self.write_optional_entity_rotation(update.rotation);
        self.write_bool(update.on_ground);
        self.write_u64(update.age_ticks);
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
            kind => Err(ProtocolCodecError::UnknownEntityKind(kind)),
        }
    }

    fn read_item_kind(&mut self) -> ProtocolCodecResult<ItemKind> {
        let kind = self.read_u8()?;
        match kind {
            0 => Ok(ItemKind::Egg),
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

    fn read_move_player(&mut self) -> ProtocolCodecResult<MovePlayerCommand> {
        match self.read_u8()? {
            0 => Ok(MovePlayerCommand::Pos {
                position: self.read_vec3d()?,
                on_ground: self.read_bool()?,
            }),
            1 => {
                let position = self.read_vec3d()?;
                let (y_rot_degrees, x_rot_degrees) = self.read_move_player_rotation()?;
                Ok(MovePlayerCommand::PosRot {
                    position,
                    y_rot_degrees,
                    x_rot_degrees,
                    on_ground: self.read_bool()?,
                })
            }
            2 => {
                let (y_rot_degrees, x_rot_degrees) = self.read_move_player_rotation()?;
                Ok(MovePlayerCommand::Rot {
                    y_rot_degrees,
                    x_rot_degrees,
                    on_ground: self.read_bool()?,
                })
            }
            3 => Ok(MovePlayerCommand::StatusOnly {
                on_ground: self.read_bool()?,
            }),
            _ => Err(ProtocolCodecError::InvalidData(
                "unknown move player packet variant",
            )),
        }
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
        let has_block_state = self.read_bool()?;
        let block_state = has_block_state
            .then(|| self.read_u32().map(BlockStateId))
            .transpose()?;
        Ok(SetDebugHotbarSlotCommand { slot, block_state })
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
            teleport_id: self.read_u32()?,
            dismount_vehicle: self.read_bool()?,
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

    fn read_entity_snapshot(&mut self) -> ProtocolCodecResult<EntitySnapshot> {
        let snapshot = EntitySnapshot {
            id: self.read_entity_id()?,
            kind: self.read_entity_kind()?,
            item_stack: self.read_optional_item_stack_snapshot()?,
            position: self.read_vec3d()?,
            y_rot_degrees: self.read_f32()?,
            x_rot_degrees: self.read_f32()?,
            rotation: self.read_optional_entity_rotation()?,
            on_ground: self.read_bool()?,
            width: self.read_f32()?,
            height: self.read_f32()?,
            age_ticks: self.read_u64()?,
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

    fn read_entity_update(&mut self) -> ProtocolCodecResult<EntityUpdate> {
        let update = EntityUpdate {
            id: self.read_entity_id()?,
            item_stack: self.read_optional_item_stack_snapshot()?,
            position: self.read_vec3d()?,
            y_rot_degrees: self.read_f32()?,
            x_rot_degrees: self.read_f32()?,
            rotation: self.read_optional_entity_rotation()?,
            on_ground: self.read_bool()?,
            age_ticks: self.read_u64()?,
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
    fn default_debug_hotbar_exposes_torch_in_final_slot() {
        assert_eq!(DEFAULT_DEBUG_HOTBAR[8], Some(BlockStateId(100)));
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
            ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(-1.25, 63.0, 12.5),
                on_ground: true,
            }),
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(-1.25, 63.0, 12.5),
                y_rot_degrees: -181.5,
                x_rot_degrees: 45.25,
                on_ground: true,
            }),
            ClientCommand::MovePlayer(MovePlayerCommand::Rot {
                y_rot_degrees: -181.5,
                x_rot_degrees: 45.25,
                on_ground: false,
            }),
            ClientCommand::MovePlayer(MovePlayerCommand::StatusOnly { on_ground: true }),
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
            block_state: Some(BlockStateId(91)),
        });

        let bytes = encode_client_command(&command).unwrap();

        assert_eq!(decode_client_command(&bytes).unwrap(), command);

        let clear_command = ClientCommand::SetDebugHotbarSlot(SetDebugHotbarSlotCommand {
            slot: 8,
            block_state: None,
        });
        let clear_bytes = encode_client_command(&clear_command).unwrap();

        assert_eq!(decode_client_command(&clear_bytes).unwrap(), clear_command);
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
            teleport_id: 42,
            dismount_vehicle: true,
        });

        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);
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
            kind: EntityKind::DebugCube,
            item_stack: None,
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
            age_ticks: 12,
        };
        let update = EntityUpdate {
            id: snapshot.id,
            item_stack: None,
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
            age_ticks: 13,
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
    fn server_update_codec_round_trips_item_entity_snapshot() {
        let snapshot = EntitySnapshot {
            id: EntityId(8),
            kind: EntityKind::Item,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            }),
            position: Vec3d::new(12.5, 64.0, -3.25),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: false,
            width: 0.25,
            height: 0.25,
            age_ticks: 0,
        };
        let update = ServerUpdate::EntitySnapshot(snapshot);
        let bytes = encode_server_update(&update).unwrap();

        assert_eq!(decode_server_update(&bytes).unwrap(), update);

        let update = ServerUpdate::EntityUpdate(EntityUpdate {
            id: snapshot.id,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2,
            }),
            position: snapshot.position,
            y_rot_degrees: snapshot.y_rot_degrees,
            x_rot_degrees: snapshot.x_rot_degrees,
            rotation: snapshot.rotation,
            on_ground: true,
            age_ticks: 1,
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
            position: Vec3d::new(f64::NAN, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            age_ticks: 1,
        });
        assert_eq!(
            encode_server_update(&non_finite),
            Err(ProtocolCodecError::InvalidData(
                "entity update contains non-finite value"
            ))
        );

        let invalid_dimensions = ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(42),
            kind: EntityKind::Chicken,
            item_stack: None,
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            width: 0.0,
            height: 0.7,
            age_ticks: 1,
        });
        assert_eq!(
            encode_server_update(&invalid_dimensions),
            Err(ProtocolCodecError::InvalidData(
                "entity snapshot contains invalid dimensions"
            ))
        );

        let missing_item_stack = ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(42),
            kind: EntityKind::Item,
            item_stack: None,
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            width: 0.25,
            height: 0.25,
            age_ticks: 1,
        });
        assert_eq!(
            encode_server_update(&missing_item_stack),
            Err(ProtocolCodecError::InvalidData(
                "item entity snapshot missing item stack"
            ))
        );

        let misplaced_item_stack = ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(42),
            kind: EntityKind::Chicken,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            }),
            position: Vec3d::new(1.0, 70.0, -3.25),
            y_rot_degrees: 90.0,
            x_rot_degrees: -15.0,
            rotation: None,
            on_ground: true,
            width: 0.4,
            height: 0.7,
            age_ticks: 1,
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
            age_ticks: 1,
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
        let update = ServerUpdate::TimeUpdate { day_time: 1_000 };

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
