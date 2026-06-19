#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

use mclone_core::{
    BlockHitResult, BlockPos, BlockStateId, CHUNK_WIDTH, ChunkPos, ChunkRevision, ChunkSnapshot,
    ChunkStatus, Direction, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection, PackedLightSection,
    SECTION_HEIGHT, Vec3d,
};

pub const PROTOCOL_VERSION: u32 = 5;

const CLIENT_COMMAND_SET_CHUNK_VIEW: u8 = 1;
const CLIENT_COMMAND_PLAYER_ACTION: u8 = 2;
const CLIENT_COMMAND_USE_ITEM_ON: u8 = 3;
const SERVER_UPDATE_CHUNK_SNAPSHOT: u8 = 1;
const SERVER_UPDATE_CHUNK_UNLOAD: u8 = 2;
const SERVER_UPDATE_SECTION_BLOCK_UPDATES: u8 = 3;
const SERVER_UPDATE_TIME: u8 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkView {
    pub center: ChunkPos,
    pub render_distance: u32,
    pub chunk_tracking_radius: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientCommand {
    SetChunkView(ChunkView),
    PlayerAction(PlayerActionCommand),
    UseItemOn(UseItemOnCommand),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerActionKind {
    StartDestroyBlock,
    StopDestroyBlock,
    AbortDestroyBlock,
    DebugInstantBreak,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerActionCommand {
    pub actor_feet_position: Vec3d,
    pub pos: BlockPos,
    pub direction: Direction,
    pub kind: PlayerActionKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseItemOnCommand {
    pub actor_feet_position: Vec3d,
    pub hit: BlockHitResult,
    pub action: UseItemOnKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseItemOnKind {
    DebugPlaceBlock { block_state: BlockStateId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    UnknownPlayerActionKind(u8),
    UnknownUseItemOnKind(u8),
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
            Self::UnknownPlayerActionKind(kind) => {
                write!(f, "unknown player action kind tag {kind}")
            }
            Self::UnknownUseItemOnKind(kind) => {
                write!(f, "unknown use-item-on kind tag {kind}")
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
        ClientCommand::PlayerAction(command) => {
            writer.write_u8(CLIENT_COMMAND_PLAYER_ACTION);
            writer.write_player_action(command);
        }
        ClientCommand::UseItemOn(command) => {
            writer.write_u8(CLIENT_COMMAND_USE_ITEM_ON);
            writer.write_use_item_on(command);
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
        CLIENT_COMMAND_PLAYER_ACTION => ClientCommand::PlayerAction(reader.read_player_action()?),
        CLIENT_COMMAND_USE_ITEM_ON => ClientCommand::UseItemOn(reader.read_use_item_on()?),
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

    fn write_player_action(&mut self, command: &PlayerActionCommand) {
        self.write_vec3d(command.actor_feet_position);
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
        self.write_vec3d(command.actor_feet_position);
        self.write_block_hit_result(command.hit);
        match command.action {
            UseItemOnKind::DebugPlaceBlock { block_state } => {
                self.write_u8(0);
                self.write_u32(block_state.0);
            }
        }
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

    fn read_player_action(&mut self) -> ProtocolCodecResult<PlayerActionCommand> {
        let actor_feet_position = self.read_vec3d()?;
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
            actor_feet_position,
            pos,
            direction,
            kind,
        })
    }

    fn read_use_item_on(&mut self) -> ProtocolCodecResult<UseItemOnCommand> {
        let actor_feet_position = self.read_vec3d()?;
        let hit = self.read_block_hit_result()?;
        let action = match self.read_u8()? {
            0 => UseItemOnKind::DebugPlaceBlock {
                block_state: BlockStateId(self.read_u32()?),
            },
            kind => return Err(ProtocolCodecError::UnknownUseItemOnKind(kind)),
        };
        Ok(UseItemOnCommand {
            actor_feet_position,
            hit,
            action,
        })
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
    fn client_command_codec_round_trips_player_action() {
        let command = ClientCommand::PlayerAction(PlayerActionCommand {
            actor_feet_position: Vec3d::new(-1.0, 62.0, 12.0),
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
            actor_feet_position: Vec3d::new(1.25, 62.0, -3.5),
            hit: BlockHitResult::new(
                Vec3d::new(1.25, 64.0, -3.5),
                Direction::Up,
                BlockPos::new(1, 63, -4),
                false,
            ),
            action: UseItemOnKind::DebugPlaceBlock {
                block_state: BlockStateId(42),
            },
        });

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
