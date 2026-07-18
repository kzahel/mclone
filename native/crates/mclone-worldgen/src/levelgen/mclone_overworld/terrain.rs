use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord, expected_chunk_biome_count};

use crate::block::{BEDROCK, DIRT, GRASS_BLOCK, SAND, STONE, WATER};

use super::fields::{MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler};
use crate::levelgen::profile::{
    BEACH_BIOME_ID, FLAT_GRASS_HEIGHT, FLAT_GRASS_MIN_Y, OCEAN_BIOME_ID, PLAINS_BIOME_ID,
};
use crate::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};

pub fn generate_mclone_overworld_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let mut buffer =
        MutableChunkBlockBuffer::new(chunk_x, chunk_z, FLAT_GRASS_MIN_Y, FLAT_GRASS_HEIGHT);
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let sampler = McloneOverworldSampler::new(seed);

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let sample = sampler.sample(min_x + local_x, min_z + local_z);
            write_column(&mut buffer, local_x, local_z, sample.surface_y);
        }
    }
    buffer.prime_worldgen_heightmaps();

    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        mclone_overworld_chunk_biomes(sampler, min_x, min_z),
    )
}

pub fn mclone_overworld_biome_id(seed: i64, world_x: i32, world_z: i32) -> i32 {
    biome_id_for_surface(
        McloneOverworldSampler::new(seed)
            .sample(world_x, world_z)
            .surface_y,
    )
}

fn write_column(buffer: &mut MutableChunkBlockBuffer, local_x: i32, local_z: i32, surface_y: i32) {
    buffer.set_block_at_y(local_x, 0, local_z, BEDROCK);
    if surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        let sand_min_y = (surface_y - 3).max(1);
        for y in 1..sand_min_y {
            buffer.set_block_at_y(local_x, y, local_z, STONE);
        }
        for y in sand_min_y..=surface_y {
            buffer.set_block_at_y(local_x, y, local_z, SAND);
        }
    } else {
        for y in 1..surface_y - 2 {
            buffer.set_block_at_y(local_x, y, local_z, STONE);
        }
        buffer.set_block_at_y(local_x, surface_y - 2, local_z, DIRT);
        buffer.set_block_at_y(local_x, surface_y - 1, local_z, DIRT);
        buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
    }
    for y in surface_y + 1..=MCLONE_OVERWORLD_SEA_LEVEL {
        buffer.set_block_at_y(local_x, y, local_z, WATER);
    }
}

fn biome_id_for_surface(surface_y: i32) -> i32 {
    if surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
        OCEAN_BIOME_ID
    } else if surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        BEACH_BIOME_ID
    } else {
        PLAINS_BIOME_ID
    }
}

fn mclone_overworld_chunk_biomes(
    sampler: McloneOverworldSampler,
    min_x: i32,
    min_z: i32,
) -> Vec<i32> {
    let quart_height = FLAT_GRASS_HEIGHT / 4;
    let mut biomes = Vec::with_capacity(expected_chunk_biome_count(FLAT_GRASS_HEIGHT));
    for _quart_y in 0..quart_height {
        for quart_z in 0..4 {
            for quart_x in 0..4 {
                let surface_y = sampler
                    .sample(min_x + quart_x * 4 + 2, min_z + quart_z * 4 + 2)
                    .surface_y;
                biomes.push(biome_id_for_surface(surface_y));
            }
        }
    }
    biomes
}

#[cfg(test)]
mod tests {
    use crate::block::AIR;

    use super::*;

    #[test]
    fn chunk_columns_follow_the_production_sampler_and_material_rules() {
        for (seed, chunk_x, chunk_z) in [(12_345, 0, 0), (-98_765, -17, 11)] {
            let chunk = generate_mclone_overworld_chunk(seed, chunk_x, chunk_z);
            let sampler = McloneOverworldSampler::new(seed);
            let min_x = chunk_min_block_coord(chunk_x);
            let min_z = chunk_min_block_coord(chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let sample = sampler.sample(min_x + local_x, min_z + local_z);
                    let expected_top = if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
                        SAND
                    } else {
                        GRASS_BLOCK
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
            let left = generate_mclone_overworld_chunk(seed, left_x, z);
            let right = generate_mclone_overworld_chunk(seed, left_x + 1, z);
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
                        .chain(sample.surface_y.to_le_bytes())
                    {
                        hash ^= u64::from(byte);
                        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                    }
                }
            }
            assert!(land > 0, "seed {seed} had no land");
            assert!(water > 0, "seed {seed} had no water");
            hash
        });
        assert_eq!(
            fingerprints,
            [
                3_503_757_461_131_335_250,
                13_898_607_341_532_105_566,
                17_278_483_164_450_982_141,
            ]
        );
    }
}
