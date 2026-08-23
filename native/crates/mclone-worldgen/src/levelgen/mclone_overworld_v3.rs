use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use crate::{
    block::{
        BEDROCK, COARSE_DIRT, DIRT, GRASS_BLOCK, GRAVEL, RawBlockId, SAND, SANDSTONE, SNOW_BLOCK,
        STONE, WATER,
    },
    mclone_overworld_v3::{
        McloneOverworldV3TerrainPlan, V3SurfaceSubstrate, V3TerrainSample, V3TerrainWindowRequest,
        V3TerrainWork, V3WaterKind,
    },
};

use super::{
    GeneratedChunk, MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_RIVER_BIOME_ID,
    MCLONE_OVERWORLD_SAVANNA_BIOME_ID, MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID,
    MutableChunkBlockBuffer,
    chunk::sample_column_biome_payload,
    profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID},
};

pub const MCLONE_OVERWORLD_V3_EXACT_REVISION: u16 = 1;
const V3_MIN_Y: i32 = 0;
const V3_HEIGHT: i32 = 256;
const V3_MAX_SURFACE_Y: i32 = V3_HEIGHT - 2;

/// Exact column lowering for the experimental Mclone Overworld V3 source.
///
/// The source plan is retained for the compiler session. Each chunk requests
/// one contiguous 16x16 window, then lowers that same sample buffer into
/// blocks and the canonical biome payload without reconstructing the terrain
/// source or evaluating a dependency halo.
#[derive(Clone, Debug)]
pub struct McloneOverworldV3ExactGenerator {
    seed: i64,
    terrain: McloneOverworldV3TerrainPlan,
}

impl McloneOverworldV3ExactGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            terrain: McloneOverworldV3TerrainPlan::new(seed),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn terrain(&self) -> &McloneOverworldV3TerrainPlan {
        &self.terrain
    }

    pub fn generate_surface_chunk(&self, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
        self.generate_surface_chunk_with_work(chunk_x, chunk_z).0
    }

    pub fn generate_surface_chunk_with_work(
        &self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> (GeneratedChunk, V3TerrainWork) {
        let min_x = chunk_min_block_coord(chunk_x);
        let min_z = chunk_min_block_coord(chunk_z);
        let mut window = self
            .terrain
            .query_window(V3TerrainWindowRequest::new(
                min_x,
                min_z,
                CHUNK_WIDTH as u32,
                CHUNK_WIDTH as u32,
                1,
            ))
            .expect("one exact V3 chunk is a valid terrain window");
        window.work.exact_chunks = 1;

        let mut buffer = MutableChunkBlockBuffer::new(chunk_x, chunk_z, V3_MIN_Y, V3_HEIGHT);
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                write_v3_column(
                    &mut buffer,
                    local_x,
                    local_z,
                    window.samples[sample_index(local_x, local_z)],
                );
            }
        }
        buffer.prime_worldgen_heightmaps();

        let biomes = sample_column_biome_payload(min_x, min_z, V3_HEIGHT, |world_x, world_z| {
            let local_x = (world_x - min_x).clamp(0, CHUNK_WIDTH - 1);
            let local_z = (world_z - min_z).clamp(0, CHUNK_WIDTH - 1);
            mclone_overworld_v3_biome_id(window.samples[sample_index(local_x, local_z)])
        });
        (
            GeneratedChunk::from_mutable_buffer_with_biomes(buffer, biomes),
            window.work,
        )
    }
}

pub fn generate_mclone_overworld_v3_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    McloneOverworldV3ExactGenerator::new(seed).generate_surface_chunk(chunk_x, chunk_z)
}

/// V3 currently has no cross-chunk decoration stage. Keeping the full-chunk
/// entry point separate gives later ecology a stable promotion boundary while
/// making target-only generation explicit in the first experimental profile.
pub fn generate_mclone_overworld_v3_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    generate_mclone_overworld_v3_surface_chunk(seed, chunk_x, chunk_z)
}

/// Final biome identifier shared by exact chunks and procedural LOD.
pub fn mclone_overworld_v3_biome_id(sample: V3TerrainSample) -> i32 {
    match sample.water_kind {
        V3WaterKind::Ocean => OCEAN_BIOME_ID,
        V3WaterKind::BasinLake => MCLONE_OVERWORLD_RIVER_BIOME_ID,
        V3WaterKind::None if sample.substrate == V3SurfaceSubstrate::Sand => BEACH_BIOME_ID,
        V3WaterKind::None if sample.substrate == V3SurfaceSubstrate::Snow => {
            MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID
        }
        V3WaterKind::None if sample.moisture < 0.30 => MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
        V3WaterKind::None if sample.forest_opportunity >= 0.36 => MCLONE_OVERWORLD_FOREST_BIOME_ID,
        V3WaterKind::None => PLAINS_BIOME_ID,
    }
}

fn write_v3_column(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    sample: V3TerrainSample,
) {
    let surface_y = quantized_mclone_overworld_v3_surface_y(sample);
    buffer.set_block_at_y(local_x, V3_MIN_Y, local_z, BEDROCK);
    for y in (V3_MIN_Y + 1)..=surface_y {
        let depth = surface_y - y;
        buffer.set_block_at_y(
            local_x,
            y,
            local_z,
            mclone_overworld_v3_stratum(sample.substrate, depth),
        );
    }

    if let Some(level) = sample
        .water_level_y
        .filter(|level| *level > sample.solid_surface_y)
    {
        let water_y = (level.floor() as i32).clamp(surface_y, V3_HEIGHT - 1);
        for y in (surface_y + 1)..=water_y {
            buffer.set_block_at_y(local_x, y, local_z, WATER);
        }
    }
}

pub(crate) fn quantized_mclone_overworld_v3_surface_y(sample: V3TerrainSample) -> i32 {
    (sample.solid_surface_y.floor() as i32).clamp(V3_MIN_Y + 1, V3_MAX_SURFACE_Y)
}

pub(crate) fn mclone_overworld_v3_stratum(substrate: V3SurfaceSubstrate, depth: i32) -> RawBlockId {
    match substrate {
        V3SurfaceSubstrate::Grass if depth == 0 => GRASS_BLOCK,
        V3SurfaceSubstrate::Grass if depth <= 3 => DIRT,
        V3SurfaceSubstrate::CoarseSoil if depth == 0 => COARSE_DIRT,
        V3SurfaceSubstrate::CoarseSoil if depth <= 3 => DIRT,
        V3SurfaceSubstrate::Sand if depth <= 4 => SAND,
        V3SurfaceSubstrate::Sand if depth <= 7 => SANDSTONE,
        V3SurfaceSubstrate::Gravel if depth <= 2 => GRAVEL,
        V3SurfaceSubstrate::Snow if depth == 0 => SNOW_BLOCK,
        V3SurfaceSubstrate::Stone if depth == 0 => STONE,
        _ => STONE,
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
    fn exact_columns_and_biomes_match_the_shared_source() {
        let generator = McloneOverworldV3ExactGenerator::new(SEED);
        let review = generator.terrain().select_mountain_review_site();
        let review_chunk = (
            review.center_x.div_euclid(CHUNK_WIDTH),
            review.center_z.div_euclid(CHUNK_WIDTH),
        );
        for (chunk_x, chunk_z) in [(0, 0), (-17, 31), review_chunk] {
            let chunk = generator.generate_surface_chunk(chunk_x, chunk_z);
            let min_x = chunk_min_block_coord(chunk_x);
            let min_z = chunk_min_block_coord(chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let sample = generator
                        .terrain()
                        .query_point(min_x + local_x, min_z + local_z)
                        .sample;
                    let surface_y = quantized_mclone_overworld_v3_surface_y(sample);
                    assert_eq!(
                        chunk.block_at_y(local_x, surface_y, local_z).raw(),
                        mclone_overworld_v3_stratum(sample.substrate, 0)
                    );
                    let expected_water_y = sample
                        .water_level_y
                        .filter(|level| *level > sample.solid_surface_y)
                        .map(|level| (level.floor() as i32).clamp(surface_y, V3_HEIGHT - 1));
                    if let Some(water_y) = expected_water_y {
                        assert_eq!(chunk.block_at_y(local_x, water_y, local_z).raw(), WATER);
                    } else if surface_y + 1 < V3_HEIGHT {
                        assert_eq!(chunk.block_at_y(local_x, surface_y + 1, local_z).raw(), AIR);
                    }
                }
            }

            for quart_z in 0..(CHUNK_WIDTH / 4) {
                for quart_x in 0..(CHUNK_WIDTH / 4) {
                    let local_x = quart_x * 4 + 2;
                    let local_z = quart_z * 4 + 2;
                    let sample = generator
                        .terrain()
                        .query_point(min_x + local_x, min_z + local_z)
                        .sample;
                    let payload_index = (quart_z * (CHUNK_WIDTH / 4) + quart_x) as usize;
                    assert_eq!(
                        chunk.biomes()[payload_index],
                        mclone_overworld_v3_biome_id(sample)
                    );
                }
            }
        }
    }

    #[test]
    fn exact_chunk_uses_one_bounded_source_window() {
        let generator = McloneOverworldV3ExactGenerator::new(SEED);
        let (_chunk, work) = generator.generate_surface_chunk_with_work(-3_296, 4_266);
        assert_eq!(work.requested_samples, 256);
        assert_eq!(work.landform_owner_evaluations, 2_304);
        assert_eq!(work.field_evaluations, 2_048);
        assert_eq!(work.exact_chunks, 1);
    }

    #[test]
    fn exact_generation_is_request_order_independent() {
        let generator = McloneOverworldV3ExactGenerator::new(SEED);
        let positions = [(0, 0), (-3_296, 4_266), (511, -1_024)];
        let forward =
            positions.map(|(chunk_x, chunk_z)| generator.generate_surface_chunk(chunk_x, chunk_z));
        let reverse = positions
            .into_iter()
            .rev()
            .map(|(chunk_x, chunk_z)| generator.generate_surface_chunk(chunk_x, chunk_z))
            .collect::<Vec<_>>();
        assert_eq!(forward[0], reverse[2]);
        assert_eq!(forward[1], reverse[1]);
        assert_eq!(forward[2], reverse[0]);
    }

    #[test]
    fn adjacent_chunks_lower_direct_boundary_samples() {
        let generator = McloneOverworldV3ExactGenerator::new(SEED);
        let west = generator.generate_surface_chunk(-1, -3);
        let east = generator.generate_surface_chunk(0, -3);
        for local_z in 0..CHUNK_WIDTH {
            let world_z = chunk_min_block_coord(-3) + local_z;
            let west_sample = generator.terrain().query_point(-1, world_z).sample;
            let east_sample = generator.terrain().query_point(0, world_z).sample;
            let west_y = quantized_mclone_overworld_v3_surface_y(west_sample);
            let east_y = quantized_mclone_overworld_v3_surface_y(east_sample);
            assert_eq!(
                west.block_at_y(CHUNK_WIDTH - 1, west_y, local_z).raw(),
                mclone_overworld_v3_stratum(west_sample.substrate, 0)
            );
            assert_eq!(
                east.block_at_y(0, east_y, local_z).raw(),
                mclone_overworld_v3_stratum(east_sample.substrate, 0)
            );
            assert!((west_sample.solid_surface_y - east_sample.solid_surface_y).abs() < 12.0);
        }
    }
}
