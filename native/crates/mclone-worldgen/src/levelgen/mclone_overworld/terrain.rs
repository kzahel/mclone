use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use super::biomes::mclone_overworld_biome_id_for_sample;
use super::fields::{
    MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS, McloneOverworldLandformSample, McloneOverworldSampler,
    McloneOverworldTerrainSample,
};
use super::surface::write_surface_column;
use crate::levelgen::chunk::sample_column_biome_payload;
use crate::levelgen::profile::{FLAT_GRASS_HEIGHT, FLAT_GRASS_MIN_Y};
use crate::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};

pub fn generate_mclone_overworld_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let samples = ChunkLandformSamples::new(seed, min_x, min_z);
    let buffer = generate_mclone_overworld_surface_buffer_from_samples(chunk_x, chunk_z, &samples);
    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        mclone_overworld_chunk_biomes_from_samples(min_x, min_z, &samples),
    )
}

pub(super) fn generate_mclone_overworld_surface_buffer(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> MutableChunkBlockBuffer {
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let samples = ChunkLandformSamples::new(seed, min_x, min_z);
    generate_mclone_overworld_surface_buffer_from_samples(chunk_x, chunk_z, &samples)
}

fn generate_mclone_overworld_surface_buffer_from_samples(
    chunk_x: i32,
    chunk_z: i32,
    samples: &ChunkLandformSamples,
) -> MutableChunkBlockBuffer {
    let mut buffer =
        MutableChunkBlockBuffer::new(chunk_x, chunk_z, FLAT_GRASS_MIN_Y, FLAT_GRASS_HEIGHT);

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            write_surface_column(
                &mut buffer,
                local_x,
                local_z,
                samples.landform(local_x, local_z),
            );
        }
    }
    buffer.prime_worldgen_heightmaps();

    buffer
}

pub(super) fn mclone_overworld_chunk_biomes(seed: i64, min_x: i32, min_z: i32) -> Vec<i32> {
    let samples = ChunkLandformSamples::new(seed, min_x, min_z);
    mclone_overworld_chunk_biomes_from_samples(min_x, min_z, &samples)
}

fn mclone_overworld_chunk_biomes_from_samples(
    min_x: i32,
    min_z: i32,
    samples: &ChunkLandformSamples,
) -> Vec<i32> {
    sample_column_biome_payload(min_x, min_z, FLAT_GRASS_HEIGHT, |world_x, world_z| {
        mclone_overworld_biome_id_for_sample(samples.landform(world_x - min_x, world_z - min_z))
    })
}

const CHUNK_LANDFORM_SAMPLE_WIDTH: i32 = CHUNK_WIDTH + MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS * 2;

#[derive(Clone, Debug)]
struct ChunkLandformSamples {
    terrain: Vec<McloneOverworldTerrainSample>,
}

impl ChunkLandformSamples {
    fn new(seed: i64, min_x: i32, min_z: i32) -> Self {
        let sampler = McloneOverworldSampler::new(seed);
        let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
        let width = usize::try_from(CHUNK_LANDFORM_SAMPLE_WIDTH)
            .expect("Mclone chunk landform sample width must fit usize");
        let mut terrain = Vec::with_capacity(width * width);
        for offset_z in -radius..CHUNK_WIDTH + radius {
            for offset_x in -radius..CHUNK_WIDTH + radius {
                terrain.push(sampler.sample(min_x + offset_x, min_z + offset_z));
            }
        }
        Self { terrain }
    }

    fn landform(&self, local_x: i32, local_z: i32) -> McloneOverworldLandformSample {
        let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
        McloneOverworldLandformSample::from_cardinal_samples(
            self.terrain(local_x, local_z),
            self.terrain(local_x - radius, local_z),
            self.terrain(local_x + radius, local_z),
            self.terrain(local_x, local_z - radius),
            self.terrain(local_x, local_z + radius),
        )
    }

    fn terrain(&self, local_x: i32, local_z: i32) -> McloneOverworldTerrainSample {
        let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
        let sample_x = usize::try_from(local_x + radius)
            .expect("Mclone chunk landform x must lie inside its halo");
        let sample_z = usize::try_from(local_z + radius)
            .expect("Mclone chunk landform z must lie inside its halo");
        let width = usize::try_from(CHUNK_LANDFORM_SAMPLE_WIDTH)
            .expect("Mclone chunk landform sample width must fit usize");
        self.terrain[sample_z * width + sample_x]
    }
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
                    let sample = sampler.sample_landform(min_x + local_x, min_z + local_z);
                    let expected_top = match mclone_overworld_surface_recipe(sample) {
                        McloneOverworldSurfaceRecipe::OceanFloor => GRAVEL,
                        McloneOverworldSurfaceRecipe::Beach => SAND,
                        McloneOverworldSurfaceRecipe::GrassSoil => GRASS_BLOCK,
                        McloneOverworldSurfaceRecipe::ExposedStone => STONE,
                    };
                    assert_eq!(
                        chunk
                            .block_at_y(local_x, sample.terrain.surface_y, local_z)
                            .0,
                        expected_top
                    );
                    let above = chunk.block_at_y(local_x, sample.terrain.surface_y + 1, local_z);
                    assert_eq!(
                        above.0,
                        if sample.terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL {
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
                (2_437_008_730_215_191_963, 540_454_697_130_909_605),
                (3_841_247_183_671_404_361, 3_995_179_115_581_767_979),
                (14_356_273_015_370_666_679, 14_722_381_067_837_031_305),
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
                    let landform = sampler.sample_landform(x, z);
                    let biome_id = mclone_overworld_biome_id_for_sample(landform);
                    let biome_index = match biome_id {
                        OCEAN_BIOME_ID => 0,
                        BEACH_BIOME_ID => 1,
                        PLAINS_BIOME_ID => 2,
                        MCLONE_OVERWORLD_FOREST_BIOME_ID => 3,
                        _ => panic!("unexpected Mclone biome ID {biome_id}"),
                    };
                    let surface_index = match mclone_overworld_surface_recipe(landform) {
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
            assert!(
                biome_counts.into_iter().all(|count| count > 0),
                "seed {seed} biome counts: {biome_counts:?}"
            );
            assert!(
                surface_counts[..3].iter().all(|count| *count > 0),
                "seed {seed} surface counts: {surface_counts:?}"
            );
            (biome_counts, surface_counts, hash)
        });
        assert!(receipts.iter().any(|receipt| receipt.1[3] > 0));

        assert_eq!(
            receipts,
            [
                (
                    [21_961, 8_436, 17_789, 17_350],
                    [13_072, 17_325, 35_069, 70],
                    5_971_664_242_438_023_034,
                ),
                (
                    [17_223, 7_066, 13_701, 27_546],
                    [8_521, 15_768, 41_176, 71],
                    7_806_727_491_514_814_574,
                ),
                (
                    [33_641, 11_524, 9_892, 10_479],
                    [21_336, 23_829, 20_371, 0],
                    5_151_670_375_816_009_055,
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
