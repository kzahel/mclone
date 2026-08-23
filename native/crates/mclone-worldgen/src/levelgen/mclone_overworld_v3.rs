use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use crate::{
    block::{
        BEDROCK, COARSE_DIRT, DIRT, GRASS_BLOCK, GRAVEL, RawBlockId, SAND, SANDSTONE, SNOW_BLOCK,
        STONE, WATER,
    },
    feature::FeatureRegion,
    mclone_overworld_v3::{
        McloneOverworldV3TerrainPlan, V3SurfaceSubstrate, V3TerrainSample, V3TerrainWindowRequest,
        V3TerrainWork, V3WaterKind,
    },
};

use super::{
    GeneratedChunk, MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_RIVER_BIOME_ID,
    MCLONE_OVERWORLD_SAVANNA_BIOME_ID, MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID, McloneTreeFamily,
    McloneTreeId, McloneTreeOccurrence, McloneTreeRecord, McloneVegetationBounds,
    MutableChunkBlockBuffer,
    chunk::sample_column_biome_payload,
    mclone_overworld::{realize_mclone_tree_occurrences, resolve_silhouette, tree_bounds},
    profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID},
};

pub const MCLONE_OVERWORLD_V3_EXACT_REVISION: u16 = 2;
pub const MCLONE_OVERWORLD_V3_VEGETATION_REVISION: u16 = 1;
pub const MCLONE_OVERWORLD_V3_VEGETATION_SOURCE_REVISION: &str =
    "mclone-overworld-v3-vegetation-v1";
const V3_MIN_Y: i32 = 0;
const V3_HEIGHT: i32 = 256;
const V3_MAX_SURFACE_Y: i32 = V3_HEIGHT - 2;
const V3_VEGETATION_CELL_BLOCKS: i32 = 12;
const V3_TREE_MAX_HORIZONTAL_RADIUS: i32 = 3;
const V3_VEGETATION_HASH_DOMAIN: u64 = 0x7633_5f76_6567_3031;

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
        let (buffer, biomes, work) = self.generate_surface_buffer_with_work(chunk_x, chunk_z);
        (
            GeneratedChunk::from_mutable_buffer_with_biomes(buffer, biomes),
            work,
        )
    }

    pub fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
        let (buffer, biomes, _work) = self.generate_surface_buffer_with_work(chunk_x, chunk_z);
        let min_x = chunk_min_block_coord(chunk_x);
        let min_z = chunk_min_block_coord(chunk_z);
        let bounds = McloneVegetationBounds::new(
            min_x,
            min_z,
            min_x + CHUNK_WIDTH - 1,
            min_z + CHUNK_WIDTH - 1,
        )
        .expect("one exact V3 chunk has representable vegetation bounds");
        let occurrences = mclone_overworld_v3_tree_records_intersecting(self.seed, bounds)
            .expect("one exact V3 chunk has a bounded vegetation query");
        let mut region = FeatureRegion::with_radii(chunk_x, chunk_z, 0, 0, [buffer]);
        realize_mclone_tree_occurrences(&mut region, &occurrences);
        let buffer = region
            .remove_chunk(chunk_x, chunk_z)
            .expect("the target-only V3 feature region retains its target");
        GeneratedChunk::from_mutable_buffer_with_biomes(buffer, biomes)
    }

    fn generate_surface_buffer_with_work(
        &self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> (MutableChunkBlockBuffer, Vec<i32>, V3TerrainWork) {
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
        (buffer, biomes, window.work)
    }
}

pub fn generate_mclone_overworld_v3_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    McloneOverworldV3ExactGenerator::new(seed).generate_surface_chunk(chunk_x, chunk_z)
}

pub fn generate_mclone_overworld_v3_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    McloneOverworldV3ExactGenerator::new(seed).generate_chunk(chunk_x, chunk_z)
}

/// Return the ordinary V3 tree records whose complete bounds intersect a
/// horizontal query. One deterministic global lattice consumes the accepted
/// terrain and forest-opportunity facts, so exact chunks and procedural LOD
/// discover the same trees without feature dependencies or tile ownership.
pub fn mclone_overworld_v3_tree_records_intersecting(
    seed: i64,
    bounds: McloneVegetationBounds,
) -> Result<Vec<McloneTreeOccurrence>, String> {
    let expanded_min_x = bounds
        .min_x
        .checked_sub(V3_TREE_MAX_HORIZONTAL_RADIUS)
        .ok_or("V3 vegetation minimum X overflow")?;
    let expanded_min_z = bounds
        .min_z
        .checked_sub(V3_TREE_MAX_HORIZONTAL_RADIUS)
        .ok_or("V3 vegetation minimum Z overflow")?;
    let expanded_max_x = bounds
        .max_x
        .checked_add(V3_TREE_MAX_HORIZONTAL_RADIUS)
        .ok_or("V3 vegetation maximum X overflow")?;
    let expanded_max_z = bounds
        .max_z
        .checked_add(V3_TREE_MAX_HORIZONTAL_RADIUS)
        .ok_or("V3 vegetation maximum Z overflow")?;
    let min_cell_x = expanded_min_x.div_euclid(V3_VEGETATION_CELL_BLOCKS);
    let min_cell_z = expanded_min_z.div_euclid(V3_VEGETATION_CELL_BLOCKS);
    let max_cell_x = expanded_max_x.div_euclid(V3_VEGETATION_CELL_BLOCKS);
    let max_cell_z = expanded_max_z.div_euclid(V3_VEGETATION_CELL_BLOCKS);
    let cell_count = i64::from(max_cell_x) - i64::from(min_cell_x) + 1;
    let cell_rows = i64::from(max_cell_z) - i64::from(min_cell_z) + 1;
    if cell_count.saturating_mul(cell_rows) > 65_536 {
        return Err("V3 vegetation query exceeds 65536 lattice cells".to_owned());
    }

    let terrain = McloneOverworldV3TerrainPlan::new(seed);
    let mut occurrences = Vec::new();
    for cell_z in min_cell_z..=max_cell_z {
        for cell_x in min_cell_x..=max_cell_x {
            let position_hash = v3_vegetation_hash(seed, cell_x, cell_z, 0);
            let inset = 2;
            let jitter_span = V3_VEGETATION_CELL_BLOCKS - inset * 2;
            let world_x = cell_x
                .checked_mul(V3_VEGETATION_CELL_BLOCKS)
                .and_then(|value| {
                    value.checked_add(inset + (position_hash as i32).rem_euclid(jitter_span))
                })
                .ok_or("V3 vegetation X coordinate overflow")?;
            let world_z = cell_z
                .checked_mul(V3_VEGETATION_CELL_BLOCKS)
                .and_then(|value| {
                    value
                        .checked_add(inset + ((position_hash >> 32) as i32).rem_euclid(jitter_span))
                })
                .ok_or("V3 vegetation Z coordinate overflow")?;
            let sample = terrain.query_point(world_x, world_z).sample;
            if sample.is_water()
                || !matches!(
                    sample.substrate,
                    V3SurfaceSubstrate::Grass | V3SurfaceSubstrate::CoarseSoil
                )
            {
                continue;
            }
            let density = v3_vegetation_smoothstep(0.14, 0.68, sample.forest_opportunity) * 0.90;
            let admission =
                (v3_vegetation_hash(seed, cell_x, cell_z, 1) as u32) as f64 / f64::from(u32::MAX);
            if admission >= f64::from(density) {
                continue;
            }

            let family = if sample.solid_surface_y >= 118.0 || sample.moisture >= 0.64 {
                McloneTreeFamily::CoolWetConifer
            } else {
                McloneTreeFamily::TemperateBroadleaf
            };
            let silhouette_hash = v3_vegetation_hash(seed, cell_x, cell_z, 2);
            let (archetype, trunk_height, crown_radius, crown_depth) =
                resolve_silhouette(family, silhouette_hash);
            let base_y = (sample.solid_surface_y.floor() as i32)
                .checked_add(1)
                .ok_or("V3 vegetation Y coordinate overflow")?;
            let base = crate::placement::BlockPos::new(world_x, base_y, world_z);
            let tree_bounds = tree_bounds(base, family, trunk_height, crown_radius, crown_depth)
                .map_err(|error| error.to_string())?;
            if tree_bounds.max_y >= V3_HEIGHT || !tree_bounds.intersects_horizontal(bounds) {
                continue;
            }
            let record = McloneTreeRecord {
                id: McloneTreeId {
                    planning_cell_x: cell_x,
                    planning_cell_z: cell_z,
                    candidate_slot: 0,
                    vegetation_revision: MCLONE_OVERWORLD_V3_VEGETATION_REVISION,
                },
                canonical_base: base,
                family,
                archetype,
                trunk_height,
                crown_radius,
                crown_depth,
                orientation: (v3_vegetation_hash(seed, cell_x, cell_z, 3) & 3) as u8,
                landmark_rank: (v3_vegetation_hash(seed, cell_x, cell_z, 4) >> 62) as u8,
                variant_seed: v3_vegetation_hash(seed, cell_x, cell_z, 5),
                bounds: tree_bounds,
            };
            occurrences.push(McloneTreeOccurrence {
                record,
                x_lift: 0,
                working_bounds: tree_bounds,
            });
        }
    }
    occurrences.sort_unstable();
    Ok(occurrences)
}

fn v3_vegetation_smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn v3_vegetation_hash(seed: i64, cell_x: i32, cell_z: i32, lane: u64) -> u64 {
    let mut value =
        (seed as u64) ^ V3_VEGETATION_HASH_DOMAIN ^ lane.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= (cell_x as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = v3_splitmix64(value);
    value ^= (cell_z as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    v3_splitmix64(value)
}

fn v3_splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
    use crate::block::{AIR, OAK_LOG, SPRUCE_LOG};

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

    #[test]
    fn tree_records_partition_stably_and_follow_forest_opportunity() {
        let whole_bounds = McloneVegetationBounds::new(-512, -512, 511, 511).unwrap();
        let whole = mclone_overworld_v3_tree_records_intersecting(SEED, whole_bounds).unwrap();
        assert!(whole.len() > 10, "tree records={}", whole.len());

        let mut partitioned = [
            McloneVegetationBounds::new(-512, -512, -1, -1).unwrap(),
            McloneVegetationBounds::new(0, -512, 511, -1).unwrap(),
            McloneVegetationBounds::new(-512, 0, -1, 511).unwrap(),
            McloneVegetationBounds::new(0, 0, 511, 511).unwrap(),
        ]
        .into_iter()
        .flat_map(|bounds| mclone_overworld_v3_tree_records_intersecting(SEED, bounds).unwrap())
        .collect::<Vec<_>>();
        partitioned.sort_unstable();
        partitioned.dedup();
        assert_eq!(partitioned, whole);

        let terrain = McloneOverworldV3TerrainPlan::new(SEED);
        for occurrence in &whole {
            let base = occurrence.working_base().unwrap();
            let sample = terrain.query_point(base.x, base.z).sample;
            assert!(!sample.is_water());
            assert!(sample.forest_opportunity >= 0.14, "sample={sample:?}");
            assert!(matches!(
                sample.substrate,
                V3SurfaceSubstrate::Grass | V3SurfaceSubstrate::CoarseSoil
            ));
        }
    }

    #[test]
    fn final_chunks_realize_the_same_v3_tree_records() {
        let bounds = McloneVegetationBounds::new(-512, -512, 511, 511).unwrap();
        let occurrence = mclone_overworld_v3_tree_records_intersecting(SEED, bounds)
            .unwrap()
            .into_iter()
            .next()
            .expect("the V3 vegetation review region contains a tree");
        let base = occurrence.working_base().unwrap();
        let chunk_x = base.x.div_euclid(CHUNK_WIDTH);
        let chunk_z = base.z.div_euclid(CHUNK_WIDTH);
        let local_x = base.x.rem_euclid(CHUNK_WIDTH);
        let local_z = base.z.rem_euclid(CHUNK_WIDTH);
        let generator = McloneOverworldV3ExactGenerator::new(SEED);
        let surface = generator.generate_surface_chunk(chunk_x, chunk_z);
        let final_chunk = generator.generate_chunk(chunk_x, chunk_z);

        assert_eq!(surface.block_at_y(local_x, base.y, local_z).raw(), AIR);
        assert!(matches!(
            final_chunk.block_at_y(local_x, base.y, local_z).raw(),
            OAK_LOG | SPRUCE_LOG
        ));
        assert_eq!(final_chunk, generator.generate_chunk(chunk_x, chunk_z));
    }
}
