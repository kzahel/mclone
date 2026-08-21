use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use crate::{
    block::{BEDROCK, DIRT, GRASS_BLOCK, GRAVEL, RawBlockId, SAND, SANDSTONE, STONE, WATER},
    continental_ecoregion::ContinentalEcoregionDescriptor,
    continental_surface::{
        ContinentalSurfacePlan, ContinentalSurfaceSample, ContinentalSurfaceSubstrate,
        ContinentalSurfaceWaterKind,
    },
};

use super::{
    BEACH_BIOME_ID, GeneratedChunk, MCLONE_OVERWORLD_FOREST_BIOME_ID,
    MCLONE_OVERWORLD_RIVER_BIOME_ID, MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
    MCLONE_OVERWORLD_TAIGA_BIOME_ID, MutableChunkBlockBuffer, OCEAN_BIOME_ID, PLAINS_BIOME_ID,
    chunk::sample_column_biome_payload,
};

pub const CONTINENTAL_CANDIDATE_EXACT_REVISION: u16 = 1;
const CANDIDATE_MIN_Y: i32 = 0;
const CANDIDATE_HEIGHT: i32 = 256;
const CANDIDATE_MAX_SURFACE_Y: i32 = CANDIDATE_HEIGHT - 2;

/// Detached exact lowering for the continental/ecoregional review source.
///
/// The plan is retained for the compiler session so chunk generation does not
/// reconstruct the multi-scale source for every requested chunk. This is not a
/// persisted world-generation profile; promotion remains a separate decision.
#[derive(Clone, Debug)]
pub struct ContinentalCandidateExactGenerator {
    seed: i64,
    surface: ContinentalSurfacePlan,
}

impl ContinentalCandidateExactGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            surface: ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(seed))
                .expect("the unbounded continental candidate descriptor is valid"),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn surface(&self) -> &ContinentalSurfacePlan {
        &self.surface
    }

    pub fn generate_surface_chunk(&self, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
        let min_x = chunk_min_block_coord(chunk_x);
        let min_z = chunk_min_block_coord(chunk_z);
        let mut buffer =
            MutableChunkBlockBuffer::new(chunk_x, chunk_z, CANDIDATE_MIN_Y, CANDIDATE_HEIGHT);
        let mut samples = Vec::with_capacity((CHUNK_WIDTH * CHUNK_WIDTH) as usize);

        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let sample = self
                    .surface
                    .query_point(min_x + local_x, min_z + local_z)
                    .sample;
                write_candidate_column(&mut buffer, local_x, local_z, sample);
                samples.push(sample);
            }
        }
        buffer.prime_worldgen_heightmaps();

        let biomes =
            sample_column_biome_payload(min_x, min_z, CANDIDATE_HEIGHT, |world_x, world_z| {
                let local_x = (world_x - min_x).clamp(0, CHUNK_WIDTH - 1);
                let local_z = (world_z - min_z).clamp(0, CHUNK_WIDTH - 1);
                candidate_biome_id(samples[sample_index(local_x, local_z)])
            });
        GeneratedChunk::from_mutable_buffer_with_biomes(buffer, biomes)
    }
}

pub fn generate_continental_candidate_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    ContinentalCandidateExactGenerator::new(seed).generate_surface_chunk(chunk_x, chunk_z)
}

fn write_candidate_column(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    sample: ContinentalSurfaceSample,
) {
    let surface_y = quantized_surface_y(sample);
    buffer.set_block_at_y(local_x, CANDIDATE_MIN_Y, local_z, BEDROCK);
    for y in (CANDIDATE_MIN_Y + 1)..=surface_y {
        let depth = surface_y - y;
        buffer.set_block_at_y(
            local_x,
            y,
            local_z,
            candidate_stratum(sample.substrate, depth),
        );
    }

    if let Some(level) = sample
        .water_level_y
        .filter(|level| *level > sample.solid_surface_y)
    {
        let water_y = (level.floor() as i32).clamp(surface_y, CANDIDATE_HEIGHT - 1);
        for y in (surface_y + 1)..=water_y {
            buffer.set_block_at_y(local_x, y, local_z, WATER);
        }
    }
}

fn quantized_surface_y(sample: ContinentalSurfaceSample) -> i32 {
    (sample.solid_surface_y.floor() as i32).clamp(CANDIDATE_MIN_Y + 1, CANDIDATE_MAX_SURFACE_Y)
}

fn candidate_stratum(substrate: ContinentalSurfaceSubstrate, depth: i32) -> RawBlockId {
    match substrate {
        ContinentalSurfaceSubstrate::Grass if depth == 0 => GRASS_BLOCK,
        ContinentalSurfaceSubstrate::Grass if depth <= 3 => DIRT,
        ContinentalSurfaceSubstrate::CoarseSoil if depth == 0 => substrate.block_id(),
        ContinentalSurfaceSubstrate::CoarseSoil if depth <= 3 => DIRT,
        ContinentalSurfaceSubstrate::Sand if depth <= 4 => SAND,
        ContinentalSurfaceSubstrate::Sand if depth <= 7 => SANDSTONE,
        ContinentalSurfaceSubstrate::Gravel if depth <= 2 => GRAVEL,
        ContinentalSurfaceSubstrate::Stone if depth == 0 => STONE,
        _ => STONE,
    }
}

fn candidate_biome_id(sample: ContinentalSurfaceSample) -> i32 {
    match sample.water_kind {
        ContinentalSurfaceWaterKind::Ocean => OCEAN_BIOME_ID,
        ContinentalSurfaceWaterKind::Lake
        | ContinentalSurfaceWaterKind::River
        | ContinentalSurfaceWaterKind::WetlandPool => MCLONE_OVERWORLD_RIVER_BIOME_ID,
        ContinentalSurfaceWaterKind::None
            if sample.substrate == ContinentalSurfaceSubstrate::Sand =>
        {
            BEACH_BIOME_ID
        }
        ContinentalSurfaceWaterKind::None if sample.aridity >= 0.62 => {
            MCLONE_OVERWORLD_SAVANNA_BIOME_ID
        }
        ContinentalSurfaceWaterKind::None
            if sample.temperature <= 0.38 && sample.forest_core >= 0.28 =>
        {
            MCLONE_OVERWORLD_TAIGA_BIOME_ID
        }
        ContinentalSurfaceWaterKind::None if sample.forest_core.max(sample.forest_edge) >= 0.28 => {
            MCLONE_OVERWORLD_FOREST_BIOME_ID
        }
        ContinentalSurfaceWaterKind::None => PLAINS_BIOME_ID,
    }
}

fn sample_index(local_x: i32, local_z: i32) -> usize {
    (local_z * CHUNK_WIDTH + local_x) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::AIR;

    const SEED: i64 = 12_345;

    #[test]
    fn exact_columns_match_the_shared_surface_source() {
        let generator = ContinentalCandidateExactGenerator::new(SEED);
        for (chunk_x, chunk_z) in [(0, 0), (-17, 31), (2_048, 96)] {
            let chunk = generator.generate_surface_chunk(chunk_x, chunk_z);
            let min_x = chunk_min_block_coord(chunk_x);
            let min_z = chunk_min_block_coord(chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let sample = generator
                        .surface()
                        .query_point(min_x + local_x, min_z + local_z)
                        .sample;
                    let surface_y = quantized_surface_y(sample);
                    assert_eq!(
                        chunk.block_at_y(local_x, surface_y, local_z).raw(),
                        candidate_stratum(sample.substrate, 0)
                    );
                    let expected_water_y = sample
                        .water_level_y
                        .filter(|level| *level > sample.solid_surface_y)
                        .map(|level| (level.floor() as i32).clamp(surface_y, CANDIDATE_HEIGHT - 1));
                    if let Some(water_y) = expected_water_y {
                        assert_eq!(chunk.block_at_y(local_x, water_y, local_z).raw(), WATER);
                    } else if surface_y + 1 < CANDIDATE_HEIGHT {
                        assert_eq!(chunk.block_at_y(local_x, surface_y + 1, local_z).raw(), AIR);
                    }
                }
            }
        }
    }

    #[test]
    fn adjacent_chunks_share_the_same_boundary_samples() {
        let generator = ContinentalCandidateExactGenerator::new(SEED);
        let west = generator.generate_surface_chunk(-1, -3);
        let east = generator.generate_surface_chunk(0, -3);
        for local_z in 0..CHUNK_WIDTH {
            let west_sample = generator
                .surface()
                .query_point(-1, chunk_min_block_coord(-3) + local_z)
                .sample;
            let east_sample = generator
                .surface()
                .query_point(0, chunk_min_block_coord(-3) + local_z)
                .sample;
            let west_y = quantized_surface_y(west_sample);
            let east_y = quantized_surface_y(east_sample);
            assert_eq!(
                west.block_at_y(CHUNK_WIDTH - 1, west_y, local_z).raw(),
                candidate_stratum(west_sample.substrate, 0)
            );
            assert_eq!(
                east.block_at_y(0, east_y, local_z).raw(),
                candidate_stratum(east_sample.substrate, 0)
            );
            assert!((west_sample.solid_surface_y - east_sample.solid_surface_y).abs() < 8.0);
        }
    }

    #[test]
    fn repeated_and_reordered_generation_is_identical() {
        let generator = ContinentalCandidateExactGenerator::new(SEED);
        let coordinates = [(-1_024, -97), (2_048, 96), (-17, 31)];
        let forward = coordinates.map(|(x, z)| generator.generate_surface_chunk(x, z));
        let reverse = coordinates
            .into_iter()
            .rev()
            .map(|(x, z)| generator.generate_surface_chunk(x, z))
            .collect::<Vec<_>>();
        for (index, expected) in forward.iter().enumerate() {
            assert_eq!(expected, &reverse[reverse.len() - 1 - index]);
        }
    }

    #[test]
    fn owned_lake_water_is_flat_and_preserves_its_gravel_bed() {
        let generator = ContinentalCandidateExactGenerator::new(SEED);
        let world_x = 7_168;
        let world_z = -21_504;
        let sample = generator.surface().query_point(world_x, world_z).sample;
        assert_eq!(sample.water_kind, ContinentalSurfaceWaterKind::Lake);
        assert_eq!(sample.substrate, ContinentalSurfaceSubstrate::Gravel);
        let water_y = sample.water_level_y.unwrap().floor() as i32;
        for (offset_x, offset_z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let neighbor = generator
                .surface()
                .query_point(world_x + offset_x, world_z + offset_z)
                .sample;
            assert_eq!(neighbor.water_kind, ContinentalSurfaceWaterKind::Lake);
            assert_eq!(neighbor.water_level_y.unwrap().floor() as i32, water_y);
            let chunk_x = (world_x + offset_x).div_euclid(CHUNK_WIDTH);
            let chunk_z = (world_z + offset_z).div_euclid(CHUNK_WIDTH);
            let chunk = generator.generate_surface_chunk(chunk_x, chunk_z);
            let local_x = (world_x + offset_x).rem_euclid(CHUNK_WIDTH);
            let local_z = (world_z + offset_z).rem_euclid(CHUNK_WIDTH);
            assert_eq!(chunk.block_at_y(local_x, water_y, local_z).raw(), WATER);
            assert_eq!(
                chunk
                    .block_at_y(local_x, quantized_surface_y(neighbor), local_z)
                    .raw(),
                GRAVEL
            );
        }
    }
}
