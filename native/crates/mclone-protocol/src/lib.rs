#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

use mclone_core::{
    BlockStateId, ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus, LIGHT_DATA_LAYER_BYTE_COUNT,
    PackedChunkSection, PackedLightSection,
};

pub const PROTOCOL_VERSION: u32 = 1;

const CLIENT_COMMAND_SET_CHUNK_INTEREST: u8 = 1;
const SERVER_UPDATE_CHUNK_SNAPSHOT: u8 = 1;
const SERVER_UPDATE_CHUNK_UNLOAD: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkInterest {
    pub center: ChunkPos,
    pub radius_chunks: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientCommand {
    SetChunkInterest(ChunkInterest),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerUpdate {
    ChunkSnapshot(ChunkSnapshot),
    ChunkUnload { pos: ChunkPos },
}

pub type ProtocolCodecResult<T> = Result<T, ProtocolCodecError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolCodecError {
    UnexpectedEof { needed: usize, remaining: usize },
    TrailingBytes { remaining: usize },
    UnknownClientCommandTag(u8),
    UnknownServerUpdateTag(u8),
    UnknownChunkStatus(u8),
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
        ClientCommand::SetChunkInterest(interest) => {
            writer.write_u8(CLIENT_COMMAND_SET_CHUNK_INTEREST);
            writer.write_chunk_pos(interest.center);
            writer.write_u32(interest.radius_chunks);
        }
    }
    Ok(writer.into_inner())
}

pub fn decode_client_command(bytes: &[u8]) -> ProtocolCodecResult<ClientCommand> {
    let mut reader = ByteReader::new(bytes);
    let tag = reader.read_u8()?;
    let command = match tag {
        CLIENT_COMMAND_SET_CHUNK_INTEREST => {
            let center = reader.read_chunk_pos()?;
            let radius_chunks = reader.read_u32()?;
            ClientCommand::SetChunkInterest(ChunkInterest {
                center,
                radius_chunks,
            })
        }
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
        _ => return Err(ProtocolCodecError::UnknownServerUpdateTag(tag)),
    };
    reader.finish()?;
    Ok(update)
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

    fn read_len(&mut self) -> ProtocolCodecResult<usize> {
        Ok(self.read_u32()? as usize)
    }

    fn read_chunk_pos(&mut self) -> ProtocolCodecResult<ChunkPos> {
        Ok(ChunkPos::new(self.read_i32()?, self.read_i32()?))
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
    fn chunk_interest_is_data_only() {
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 8,
        };
        assert_eq!(interest.radius_chunks, 8);
    }

    #[test]
    fn distinguishes_interest_commands_from_server_updates() {
        let interest = ChunkInterest {
            center: ChunkPos::new(1, -2),
            radius_chunks: 3,
        };
        let command = ClientCommand::SetChunkInterest(interest.clone());
        let unload = ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(4, 5),
        };

        assert_eq!(command, ClientCommand::SetChunkInterest(interest));
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
    fn client_command_codec_round_trips_chunk_interest() {
        let command = ClientCommand::SetChunkInterest(ChunkInterest {
            center: ChunkPos::new(-12, 34),
            radius_chunks: 3,
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
    fn codec_rejects_trailing_bytes() {
        let mut bytes = encode_client_command(&ClientCommand::SetChunkInterest(ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
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
