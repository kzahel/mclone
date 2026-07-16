use mclone_core::{CHUNK_WIDTH, expected_chunk_biome_count};

use crate::block::{BEDROCK, DIRT, GRASS_BLOCK};

use super::{GeneratedChunk, MutableChunkBlockBuffer};

pub const FLAT_GRASS_MIN_Y: i32 = 0;
pub const FLAT_GRASS_HEIGHT: i32 = 256;
pub const FLAT_GRASS_SURFACE_Y: i32 = 3;
pub const PLAINS_BIOME_ID: i32 = 1;

/// Generate the immutable `flat-grass-v1` column stack.
///
/// This profile is intentionally seed-independent and target-only: every
/// world-coordinate chunk has the same layers and needs no neighboring terrain
/// to produce its canonical block or biome payload.
pub fn generate_flat_grass_chunk(chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let mut buffer =
        MutableChunkBlockBuffer::new(chunk_x, chunk_z, FLAT_GRASS_MIN_Y, FLAT_GRASS_HEIGHT);
    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            buffer.set_block_at_y(local_x, 0, local_z, BEDROCK);
            buffer.set_block_at_y(local_x, 1, local_z, DIRT);
            buffer.set_block_at_y(local_x, 2, local_z, DIRT);
            buffer.set_block_at_y(local_x, FLAT_GRASS_SURFACE_Y, local_z, GRASS_BLOCK);
        }
    }

    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        vec![PLAINS_BIOME_ID; expected_chunk_biome_count(FLAT_GRASS_HEIGHT)],
    )
}

#[cfg(test)]
mod tests {
    use crate::block::{AIR, BEDROCK, DIRT, GRASS_BLOCK};

    use super::*;

    #[test]
    fn flat_grass_v1_has_exact_layers_biomes_and_no_ticks() {
        let chunk = generate_flat_grass_chunk(0, 0);

        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                assert_eq!(chunk.block_at_y(local_x, 0, local_z).0, BEDROCK);
                assert_eq!(chunk.block_at_y(local_x, 1, local_z).0, DIRT);
                assert_eq!(chunk.block_at_y(local_x, 2, local_z).0, DIRT);
                assert_eq!(
                    chunk.block_at_y(local_x, FLAT_GRASS_SURFACE_Y, local_z).0,
                    GRASS_BLOCK
                );
                assert_eq!(chunk.block_at_y(local_x, 4, local_z).0, AIR);
                assert_eq!(chunk.block_at_y(local_x, 255, local_z).0, AIR);
            }
        }
        assert_eq!(chunk.block_count(BEDROCK), 256);
        assert_eq!(chunk.block_count(DIRT), 512);
        assert_eq!(chunk.block_count(GRASS_BLOCK), 256);
        assert_eq!(chunk.non_air_block_count(), 1_024);
        assert_eq!(
            chunk.biomes(),
            vec![PLAINS_BIOME_ID; expected_chunk_biome_count(FLAT_GRASS_HEIGHT)]
        );
        assert!(chunk.block_ticks().is_empty());
        assert!(chunk.liquid_ticks().is_empty());
    }

    #[test]
    fn flat_grass_v1_is_coordinate_and_seed_independent_by_construction() {
        let origin = generate_flat_grass_chunk(0, 0);
        for pos in [(-1, -1), (1, 2), (30_000_000 / 16, -30_000_000 / 16)] {
            let chunk = generate_flat_grass_chunk(pos.0, pos.1);
            assert_eq!(chunk.blocks(), origin.blocks());
            assert_eq!(chunk.biomes(), origin.biomes());
        }
    }
}
