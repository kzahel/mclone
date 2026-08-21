use std::collections::{BTreeMap, VecDeque};

use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord};

use crate::{
    block::{
        BEDROCK, DIRT, GRASS_BLOCK, GRAVEL, ORANGE_TERRACOTTA, RED_SAND, RED_SANDSTONE,
        RED_TERRACOTTA, RawBlockId, SAND, SANDSTONE, STONE, TERRACOTTA, WATER,
    },
    continental_ecoregion::ContinentalEcoregionDescriptor,
    continental_surface::{
        ContinentalSurfacePlan, ContinentalSurfaceSample, ContinentalSurfaceSubstrate,
        continental_surface_biome_id,
    },
    feature::FeatureRegion,
    terrain_preview::continental_candidate_tree_records_intersecting,
};

use super::{
    GeneratedChunk, MutableChunkBlockBuffer, chunk::sample_column_biome_payload,
    mclone_overworld::realize_mclone_tree_occurrences,
};

pub const CONTINENTAL_CANDIDATE_EXACT_REVISION: u16 = 6;
const CANDIDATE_MIN_Y: i32 = 0;
const CANDIDATE_HEIGHT: i32 = 256;
const CANDIDATE_MAX_SURFACE_Y: i32 = CANDIDATE_HEIGHT - 2;
const CANDIDATE_SURFACE_CACHE_MAX_CHUNKS: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContinentalCandidateFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

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
                continental_surface_biome_id(samples[sample_index(local_x, local_z)])
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

/// Bounded immutable-surface cache for deterministic candidate tree
/// realization. Feature writes always start from cloned surface chunks, so
/// request order cannot feed already-decorated blocks back into generation.
#[derive(Debug)]
pub struct ContinentalCandidateFeatureDependencyCache {
    generator: ContinentalCandidateExactGenerator,
    surfaces: BTreeMap<ChunkPos, GeneratedChunk>,
    lru: VecDeque<ChunkPos>,
}

impl ContinentalCandidateFeatureDependencyCache {
    pub fn new(seed: i64) -> Self {
        Self {
            generator: ContinentalCandidateExactGenerator::new(seed),
            surfaces: BTreeMap::new(),
            lru: VecDeque::new(),
        }
    }

    pub fn generator(&self) -> &ContinentalCandidateExactGenerator {
        &self.generator
    }

    pub fn retained_chunk_count(&self) -> usize {
        self.surfaces.len()
    }

    pub fn clear(&mut self) {
        self.surfaces.clear();
        self.lru.clear();
    }

    pub fn generate_features_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> (
        GeneratedChunk,
        ContinentalCandidateFeatureDependencyCacheReport,
    ) {
        let target = ChunkPos::new(chunk_x, chunk_z);
        let mut report = ContinentalCandidateFeatureDependencyCacheReport::default();
        let mut buffers = Vec::with_capacity(9);
        let mut target_biomes = Vec::new();
        for offset_z in -1..=1 {
            for offset_x in -1..=1 {
                let position = ChunkPos::new(chunk_x + offset_x, chunk_z + offset_z);
                report.requested_dependency_chunks += 1;
                let chunk = if let Some(chunk) = self.surfaces.get(&position).cloned() {
                    report.cache_hits += 1;
                    self.touch(position);
                    chunk
                } else {
                    report.generated_dependency_chunks += 1;
                    let chunk = self
                        .generator
                        .generate_surface_chunk(position.x, position.z);
                    self.surfaces.insert(position, chunk.clone());
                    self.touch(position);
                    chunk
                };
                if position == target {
                    target_biomes = chunk.biomes().to_vec();
                }
                buffers.push(MutableChunkBlockBuffer::from_raw_parts_with_metadata(
                    chunk.chunk_x,
                    chunk.chunk_z,
                    chunk.min_y,
                    chunk.height,
                    chunk.blocks().to_vec(),
                    BTreeMap::new(),
                    Vec::new(),
                    Vec::new(),
                    true,
                ));
            }
        }
        self.evict();
        report.retained_dependency_chunks = self.surfaces.len();

        let min_x = chunk_min_block_coord(chunk_x);
        let min_z = chunk_min_block_coord(chunk_z);
        let bounds = super::McloneVegetationBounds::new(
            min_x,
            min_z,
            min_x + CHUNK_WIDTH - 1,
            min_z + CHUNK_WIDTH - 1,
        )
        .expect("one exact chunk has representable vegetation bounds");
        let occurrences =
            continental_candidate_tree_records_intersecting(self.generator.seed(), bounds)
                .expect("one exact chunk has a bounded candidate tree query");
        let mut region = FeatureRegion::with_radii(chunk_x, chunk_z, 1, 1, buffers);
        realize_mclone_tree_occurrences(&mut region, &occurrences);
        let buffer = region
            .remove_chunk(chunk_x, chunk_z)
            .expect("the candidate feature region retains its target chunk");
        (
            GeneratedChunk::from_mutable_buffer_with_biomes(buffer, target_biomes),
            report,
        )
    }

    fn touch(&mut self, position: ChunkPos) {
        self.lru.retain(|candidate| *candidate != position);
        self.lru.push_back(position);
    }

    fn evict(&mut self) {
        while self.surfaces.len() > CANDIDATE_SURFACE_CACHE_MAX_CHUNKS {
            let Some(position) = self.lru.pop_front() else {
                break;
            };
            self.surfaces.remove(&position);
        }
    }
}

pub fn generate_continental_candidate_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    ContinentalCandidateFeatureDependencyCache::new(seed)
        .generate_features_chunk(chunk_x, chunk_z)
        .0
}

fn write_candidate_column(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    sample: ContinentalSurfaceSample,
) {
    let surface_y = quantized_continental_candidate_surface_y(sample);
    buffer.set_block_at_y(local_x, CANDIDATE_MIN_Y, local_z, BEDROCK);
    for y in (CANDIDATE_MIN_Y + 1)..=surface_y {
        let depth = surface_y - y;
        buffer.set_block_at_y(
            local_x,
            y,
            local_z,
            continental_candidate_stratum(sample.substrate, depth),
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

pub(crate) fn quantized_continental_candidate_surface_y(sample: ContinentalSurfaceSample) -> i32 {
    (sample.solid_surface_y.floor() as i32).clamp(CANDIDATE_MIN_Y + 1, CANDIDATE_MAX_SURFACE_Y)
}

pub(crate) fn continental_candidate_stratum(
    substrate: ContinentalSurfaceSubstrate,
    depth: i32,
) -> RawBlockId {
    match substrate {
        ContinentalSurfaceSubstrate::Grass if depth == 0 => GRASS_BLOCK,
        ContinentalSurfaceSubstrate::Grass if depth <= 3 => DIRT,
        ContinentalSurfaceSubstrate::CoarseSoil if depth == 0 => substrate.block_id(),
        ContinentalSurfaceSubstrate::CoarseSoil if depth <= 3 => DIRT,
        ContinentalSurfaceSubstrate::Sand if depth <= 4 => SAND,
        ContinentalSurfaceSubstrate::Sand if depth <= 7 => SANDSTONE,
        ContinentalSurfaceSubstrate::RedSand if depth <= 4 => RED_SAND,
        ContinentalSurfaceSubstrate::RedSand if depth <= 8 => RED_SANDSTONE,
        ContinentalSurfaceSubstrate::Gravel if depth <= 2 => GRAVEL,
        ContinentalSurfaceSubstrate::Stone if depth == 0 => STONE,
        ContinentalSurfaceSubstrate::Snow if depth == 0 => substrate.block_id(),
        ContinentalSurfaceSubstrate::Terracotta if depth <= 2 => TERRACOTTA,
        ContinentalSurfaceSubstrate::Terracotta if depth <= 7 => ORANGE_TERRACOTTA,
        ContinentalSurfaceSubstrate::OrangeTerracotta if depth <= 3 => ORANGE_TERRACOTTA,
        ContinentalSurfaceSubstrate::OrangeTerracotta if depth <= 8 => TERRACOTTA,
        ContinentalSurfaceSubstrate::RedTerracotta if depth <= 2 => RED_TERRACOTTA,
        ContinentalSurfaceSubstrate::RedTerracotta if depth <= 5 => ORANGE_TERRACOTTA,
        ContinentalSurfaceSubstrate::RedTerracotta if depth <= 10 => TERRACOTTA,
        _ => STONE,
    }
}

fn sample_index(local_x: i32, local_z: i32) -> usize {
    (local_z * CHUNK_WIDTH + local_x) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{ACACIA_LOG, AIR, OAK_LOG, SPRUCE_LOG};
    use crate::continental_surface::{
        ContinentalRegionalArchetype, ContinentalSurfaceWaterKind, MesaLandformKind,
    };

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
                    let surface_y = quantized_continental_candidate_surface_y(sample);
                    assert_eq!(
                        chunk.block_at_y(local_x, surface_y, local_z).raw(),
                        continental_candidate_stratum(sample.substrate, 0)
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
            let west_y = quantized_continental_candidate_surface_y(west_sample);
            let east_y = quantized_continental_candidate_surface_y(east_sample);
            assert_eq!(
                west.block_at_y(CHUNK_WIDTH - 1, west_y, local_z).raw(),
                continental_candidate_stratum(west_sample.substrate, 0)
            );
            assert_eq!(
                east.block_at_y(0, east_y, local_z).raw(),
                continental_candidate_stratum(east_sample.substrate, 0)
            );
            assert!((west_sample.solid_surface_y - east_sample.solid_surface_y).abs() < 8.0);
        }
    }

    #[test]
    fn mesa_desert_exact_column_realizes_layered_shared_strata() {
        let generator = ContinentalCandidateExactGenerator::new(SEED);
        let world_x = 11_264;
        let world_z = 36_352;
        let sample = generator.surface().query_point(world_x, world_z).sample;
        assert_eq!(
            sample.regional_archetype,
            ContinentalRegionalArchetype::MesaDesert
        );
        assert!(matches!(
            sample.mesa_landform,
            MesaLandformKind::CaprockTable | MesaLandformKind::Butte
        ));
        assert_eq!(sample.substrate, ContinentalSurfaceSubstrate::RedTerracotta);

        let chunk_x = world_x.div_euclid(CHUNK_WIDTH);
        let chunk_z = world_z.div_euclid(CHUNK_WIDTH);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH);
        let local_z = world_z.rem_euclid(CHUNK_WIDTH);
        let surface_y = quantized_continental_candidate_surface_y(sample);
        let chunk = generator.generate_surface_chunk(chunk_x, chunk_z);
        for depth in 0..=10 {
            assert_eq!(
                chunk.block_at_y(local_x, surface_y - depth, local_z).raw(),
                continental_candidate_stratum(sample.substrate, depth)
            );
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
        let catchment = crate::continental_hydrography::ContinentalHydrographyPlan::new(
            crate::continental_ecoregion::ContinentalEcoregionDescriptor::plane(SEED),
        )
        .catchment_for_owner(0, -2);
        let (world_x, world_z) = catchment.local_to_world(catchment.lake_center);
        let world_x = world_x.round() as i32;
        let world_z = world_z.round() as i32;
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
                    .block_at_y(
                        local_x,
                        quantized_continental_candidate_surface_y(neighbor),
                        local_z,
                    )
                    .raw(),
                GRAVEL
            );
        }
    }

    #[test]
    fn final_chunks_realize_the_same_stable_candidate_tree_records() {
        let bounds =
            super::super::McloneVegetationBounds::new(-13_856, 9_184, -13_793, 9_247).unwrap();
        let occurrences = continental_candidate_tree_records_intersecting(SEED, bounds).unwrap();
        let base = occurrences
            .first()
            .expect("the established wooded review region has candidate trees")
            .working_base()
            .unwrap();
        let chunk_x = base.x.div_euclid(CHUNK_WIDTH);
        let chunk_z = base.z.div_euclid(CHUNK_WIDTH);
        let surface = generate_continental_candidate_surface_chunk(SEED, chunk_x, chunk_z);
        let mut cache = ContinentalCandidateFeatureDependencyCache::new(SEED);
        let (first, first_report) = cache.generate_features_chunk(chunk_x, chunk_z);
        let (repeated, repeated_report) = cache.generate_features_chunk(chunk_x, chunk_z);

        let log_count = first.block_count(OAK_LOG)
            + first.block_count(SPRUCE_LOG)
            + first.block_count(ACACIA_LOG);
        assert!(log_count > 0);
        assert_ne!(first.blocks(), surface.blocks());
        assert_eq!(first, repeated);
        assert_eq!(first_report.requested_dependency_chunks, 9);
        assert_eq!(first_report.generated_dependency_chunks, 9);
        assert_eq!(repeated_report.cache_hits, 9);
        assert_eq!(repeated_report.generated_dependency_chunks, 0);
    }
}
