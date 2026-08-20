#![forbid(unsafe_code)]

mod animation;
mod bit_storage;
mod chunk;
mod pos;
mod terrain_lod;
pub mod time;
mod topology;

pub use animation::{
    AnimationClipId, AnimationClipIdError, AnimationPhaseSource, AnimationState,
    MAX_ANIMATION_CLIP_ID_BYTES,
};
pub use bit_storage::{BitStorage, local_palette_bits_for, palette_bits_for};
pub use chunk::{
    AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkRevision,
    ChunkSnapshot, ChunkStatus, DEFAULT_BIOME_ID, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection,
    PackedLightSection, SECTION_HEIGHT, block_to_chunk_coord, block_to_section_coord,
    chunk_block_coord, chunk_block_index, chunk_middle_block_coord, chunk_min_block_coord,
    chunk_section_index, expected_chunk_biome_count, local_block_coord, local_section_block_coord,
    validate_chunk_biomes,
};
pub use pos::{
    Aabb, BlockHitResult, BlockPos, Direction, HitResultType, Vec3d, block_pos_to_chunk_coord,
};
use sha2::{Digest, Sha256};
pub use terrain_lod::TerrainLodPreset;
pub use topology::{AxisTopology, ChunkLift, HorizontalTopology, LiftedChunkPos, TopologyError};

pub const TARGET_MINECRAFT_VERSION: &str = "1.17.1";

pub fn obfuscate_biome_zoom_seed(seed: i64) -> i64 {
    let digest = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("sha256 digest has at least eight bytes"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_target_minecraft_version() {
        assert_eq!(TARGET_MINECRAFT_VERSION, "1.17.1");
    }

    #[test]
    fn obfuscates_biome_zoom_seed_like_biome_manager() {
        assert_eq!(obfuscate_biome_zoom_seed(1124), 2_991_024_998_819_753_860);
    }
}
