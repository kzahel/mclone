#![forbid(unsafe_code)]

mod bit_storage;
mod chunk;

pub use bit_storage::{BitStorage, local_palette_bits_for, palette_bits_for};
pub use chunk::{
    AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkRevision,
    ChunkSnapshot, ChunkStatus, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection,
    PackedLightSection, SECTION_HEIGHT, chunk_section_index,
};

pub const TARGET_MINECRAFT_VERSION: &str = "1.17.1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_target_minecraft_version() {
        assert_eq!(TARGET_MINECRAFT_VERSION, "1.17.1");
    }
}
