use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use super::biomes::mclone_overworld_biome_id_for_sample;
use super::fields::McloneOverworldSampler;
use super::surface::write_surface_column;
use crate::levelgen::chunk::sample_column_biome_payload;
use crate::levelgen::profile::{FLAT_GRASS_HEIGHT, FLAT_GRASS_MIN_Y};
use crate::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};

pub fn generate_mclone_overworld_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    let buffer = generate_mclone_overworld_surface_buffer(seed, chunk_x, chunk_z);
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        mclone_overworld_chunk_biomes(seed, min_x, min_z),
    )
}

pub(super) fn generate_mclone_overworld_surface_buffer(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> MutableChunkBlockBuffer {
    let mut buffer =
        MutableChunkBlockBuffer::new(chunk_x, chunk_z, FLAT_GRASS_MIN_Y, FLAT_GRASS_HEIGHT);
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let sampler = McloneOverworldSampler::new(seed);

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let sample = sampler.sample(min_x + local_x, min_z + local_z);
            write_surface_column(&mut buffer, local_x, local_z, sample);
        }
    }
    buffer.prime_worldgen_heightmaps();

    buffer
}

pub(super) fn mclone_overworld_chunk_biomes(seed: i64, min_x: i32, min_z: i32) -> Vec<i32> {
    let sampler = McloneOverworldSampler::new(seed);
    sample_column_biome_payload(min_x, min_z, FLAT_GRASS_HEIGHT, |world_x, world_z| {
        mclone_overworld_biome_id_for_sample(sampler.sample(world_x, world_z))
    })
}

#[cfg(test)]
mod tests {
    use crate::block::{AIR, GRASS_BLOCK, GRAVEL, SAND, STONE, WATER};

    use super::*;
    use crate::levelgen::mclone_overworld::biomes::{
        MCLONE_OVERWORLD_FOREST_BIOME_ID, mclone_overworld_biome_id_for_sample,
    };
    use crate::levelgen::mclone_overworld::fields::MCLONE_OVERWORLD_SEA_LEVEL;
    use crate::levelgen::mclone_overworld::surface::{
        McloneOverworldSurfaceRecipe, mclone_overworld_surface_recipe,
    };
    use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID};

    #[test]
    fn chunk_columns_follow_the_production_sampler_and_material_rules() {
        for (seed, chunk_x, chunk_z) in [(12_345, 0, 0), (-98_765, -17, 11)] {
            let chunk = generate_mclone_overworld_surface_chunk(seed, chunk_x, chunk_z);
            let sampler = McloneOverworldSampler::new(seed);
            let min_x = chunk_min_block_coord(chunk_x);
            let min_z = chunk_min_block_coord(chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let sample = sampler.sample(min_x + local_x, min_z + local_z);
                    let expected_top = match mclone_overworld_surface_recipe(sample) {
                        McloneOverworldSurfaceRecipe::OceanFloor => GRAVEL,
                        McloneOverworldSurfaceRecipe::Beach => SAND,
                        McloneOverworldSurfaceRecipe::GrassSoil => GRASS_BLOCK,
                        McloneOverworldSurfaceRecipe::ExposedStone => STONE,
                    };
                    assert_eq!(
                        chunk.block_at_y(local_x, sample.surface_y, local_z).0,
                        expected_top
                    );
                    let above = chunk.block_at_y(local_x, sample.surface_y + 1, local_z);
                    assert_eq!(
                        above.0,
                        if sample.surface_y < MCLONE_OVERWORLD_SEA_LEVEL {
                            WATER
                        } else {
                            AIR
                        }
                    );
                }
            }
            assert!(chunk.block_ticks().is_empty());
            assert!(chunk.liquid_ticks().is_empty());
        }
    }

    #[test]
    fn adjacent_chunks_sample_one_continuous_absolute_field() {
        let seed = 8_675_309;
        for (left_x, z) in [(-2, -3), (-1, 0), (0, 2), (47, -61)] {
            let left = generate_mclone_overworld_surface_chunk(seed, left_x, z);
            let right = generate_mclone_overworld_surface_chunk(seed, left_x + 1, z);
            for local_z in 0..CHUNK_WIDTH {
                let world_z = chunk_min_block_coord(z) + local_z;
                for (chunk, local_x, world_x) in [
                    (&left, CHUNK_WIDTH - 1, chunk_min_block_coord(left_x) + 15),
                    (&right, 0, chunk_min_block_coord(left_x + 1)),
                ] {
                    let surface_y = McloneOverworldSampler::new(seed)
                        .sample(world_x, world_z)
                        .surface_y;
                    assert_ne!(chunk.block_at_y(local_x, surface_y, local_z).0, AIR);
                    assert_eq!(
                        chunk.block_at_y(local_x, surface_y + 1, local_z).0,
                        if surface_y < MCLONE_OVERWORLD_SEA_LEVEL {
                            WATER
                        } else {
                            AIR
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn selected_regions_contain_land_water_and_pin_seed_output() {
        let fingerprints = [12_345, -98_765, 8_675_309].map(|seed| {
            let sampler = McloneOverworldSampler::new(seed);
            let mut hash = 0xcbf2_9ce4_8422_2325_u64;
            let mut foundation_hash = 0xcbf2_9ce4_8422_2325_u64;
            let mut land = 0;
            let mut water = 0;
            for z in (-2_048..2_048).step_by(16) {
                for x in (-2_048..2_048).step_by(16) {
                    let sample = sampler.sample(x, z);
                    if sample.surface_y > MCLONE_OVERWORLD_SEA_LEVEL {
                        land += 1;
                    } else {
                        water += 1;
                    }
                    for byte in sample
                        .continentalness
                        .to_bits()
                        .to_le_bytes()
                        .into_iter()
                        .chain(sample.relief.to_bits().to_le_bytes())
                        .chain(sample.ruggedness.to_bits().to_le_bytes())
                        .chain(sample.ridges.to_bits().to_le_bytes())
                        .chain(sample.surface_y.to_le_bytes())
                    {
                        hash ^= u64::from(byte);
                        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                    }
                    for byte in sample
                        .continentalness
                        .to_bits()
                        .to_le_bytes()
                        .into_iter()
                        .chain(sample.relief.to_bits().to_le_bytes())
                    {
                        foundation_hash ^= u64::from(byte);
                        foundation_hash = foundation_hash.wrapping_mul(0x0000_0100_0000_01b3);
                    }
                }
            }
            assert!(land > 0, "seed {seed} had no land");
            assert!(water > 0, "seed {seed} had no water");
            (hash, foundation_hash)
        });
        assert_eq!(
            fingerprints,
            [
                (5_180_832_238_175_102_046, 540_454_697_130_909_605),
                (3_045_799_481_234_307_995, 3_995_179_115_581_767_979),
                (991_073_067_360_857_874, 14_722_381_067_837_031_305),
            ]
        );
    }

    #[test]
    fn selected_regions_exercise_and_pin_biome_and_surface_language() {
        let receipts = [12_345, -98_765, 8_675_309].map(|seed| {
            let sampler = McloneOverworldSampler::new(seed);
            let mut biome_counts = [0_u32; 4];
            let mut surface_counts = [0_u32; 4];
            let mut hash = 0xcbf2_9ce4_8422_2325_u64;
            for z in (-2_048..2_048).step_by(16) {
                for x in (-2_048..2_048).step_by(16) {
                    let sample = sampler.sample(x, z);
                    let biome_id = mclone_overworld_biome_id_for_sample(sample);
                    let biome_index = match biome_id {
                        OCEAN_BIOME_ID => 0,
                        BEACH_BIOME_ID => 1,
                        PLAINS_BIOME_ID => 2,
                        MCLONE_OVERWORLD_FOREST_BIOME_ID => 3,
                        _ => panic!("unexpected Mclone biome ID {biome_id}"),
                    };
                    let surface_index = match mclone_overworld_surface_recipe(sample) {
                        McloneOverworldSurfaceRecipe::OceanFloor => 0,
                        McloneOverworldSurfaceRecipe::Beach => 1,
                        McloneOverworldSurfaceRecipe::GrassSoil => 2,
                        McloneOverworldSurfaceRecipe::ExposedStone => 3,
                    };
                    biome_counts[biome_index] += 1;
                    surface_counts[surface_index] += 1;
                    for byte in [biome_index as u8, surface_index as u8] {
                        hash ^= u64::from(byte);
                        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                    }
                }
            }
            assert!(biome_counts.into_iter().all(|count| count > 0));
            assert!(surface_counts.into_iter().all(|count| count > 0));
            (biome_counts, surface_counts, hash)
        });

        assert_eq!(
            receipts,
            [
                (
                    [21_961, 8_401, 15_387, 19_787],
                    [13_072, 17_290, 34_968, 206],
                    16_281_986_092_053_109_921,
                ),
                (
                    [17_223, 7_088, 8_156, 33_069],
                    [8_521, 15_790, 40_320, 905],
                    10_942_147_653_060_848_687,
                ),
                (
                    [33_641, 11_634, 9_858, 10_403],
                    [21_336, 23_939, 20_150, 111],
                    14_541_626_491_678_176_828,
                ),
            ]
        );
    }

    #[test]
    fn selected_chunks_pin_surface_blocks_and_biome_payloads() {
        let fingerprints = [12_345, -98_765, 8_675_309].map(|seed| {
            let mut hash = 0xcbf2_9ce4_8422_2325_u64;
            for (chunk_x, chunk_z) in [(0, 0), (-17, 11), (31, -1)] {
                let chunk = generate_mclone_overworld_surface_chunk(seed, chunk_x, chunk_z);
                for byte in chunk
                    .blocks()
                    .iter()
                    .copied()
                    .chain(chunk.biomes().iter().flat_map(|id| id.to_le_bytes()))
                {
                    hash ^= u64::from(byte);
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
            hash
        });

        assert_eq!(
            fingerprints,
            [
                9_298_043_774_959_183_043,
                11_087_554_102_491_393_574,
                9_064_488_643_018_196_967,
            ]
        );
    }
}
