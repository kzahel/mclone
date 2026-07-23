use std::collections::BTreeMap;

use mclone_core::{
    CHUNK_WIDTH, ChunkPos, block_to_chunk_coord, chunk_min_block_coord, local_block_coord,
};

use super::biomes::mclone_overworld_biome_id_for_sample;
use super::fields::{
    MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS, McloneOverworldLandformSample, McloneOverworldSampler,
    McloneOverworldSamplingTopology, McloneOverworldTerrainSample,
};
use super::streams::{McloneOverworldStreamPlan, McloneOverworldStreamPlanCache};
use super::surface::write_surface_column;
use crate::levelgen::chunk::sample_column_biome_payload;
use crate::levelgen::profile::{FLAT_GRASS_HEIGHT, FLAT_GRASS_MIN_Y};
use crate::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneOverworldHydraulicClosureReport {
    pub target_chunks: usize,
    pub source_water_blocks: usize,
    pub flowing_water_blocks: usize,
    pub source_boundary_blocks: usize,
    pub horizontally_open_source_faces: usize,
    pub first_open_source: Option<[i32; 3]>,
    pub unsupported_source_blocks: usize,
    pub unsupported_flowing_blocks: usize,
    pub sloped_surface_edges: usize,
    pub intentional_drop_edges: usize,
    pub scheduled_liquid_ticks: usize,
}

impl McloneOverworldHydraulicClosureReport {
    pub const fn is_closed(self) -> bool {
        self.horizontally_open_source_faces == 0
            && self.unsupported_source_blocks == 0
            && self.unsupported_flowing_blocks == 0
            && self.sloped_surface_edges == 0
            && self.scheduled_liquid_ticks == 0
    }
}

pub fn generate_mclone_overworld_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    generate_mclone_overworld_surface_chunk_with_topology(
        seed,
        McloneOverworldSamplingTopology::Unbounded,
        chunk_x,
        chunk_z,
    )
}

pub fn generate_mclone_overworld_surface_chunk_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    let mut stream_cache = McloneOverworldStreamPlanCache::new(seed, topology);
    generate_mclone_overworld_surface_chunk_with_stream_cache(
        seed,
        topology,
        chunk_x,
        chunk_z,
        &mut stream_cache,
    )
}

fn generate_mclone_overworld_surface_chunk_with_stream_cache(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    chunk_x: i32,
    chunk_z: i32,
    stream_cache: &mut McloneOverworldStreamPlanCache,
) -> GeneratedChunk {
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let samples =
        ChunkLandformSamples::new_with_stream_cache(seed, topology, min_x, min_z, stream_cache);
    let buffer = generate_mclone_overworld_surface_buffer_from_samples(chunk_x, chunk_z, &samples);
    GeneratedChunk::from_mutable_buffer_with_biomes(
        buffer,
        mclone_overworld_chunk_biomes_from_samples(min_x, min_z, &samples),
    )
}

/// Inspect generated water without running fluid simulation.
///
/// Major rivers and oceans use level-zero Y63 source water. Bounded valley
/// streams add several flat source reaches and authored transition stencils.
/// Source bodies are closed when every horizontal boundary meets water or a
/// motion-blocking cell and every source has solid or water support below it.
/// A falling column is supported by water above it. A one-chunk halo makes the
/// proof independent of the requested target boundary.
pub fn analyze_mclone_overworld_hydraulic_closure(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    center: ChunkPos,
    radius_chunks: u32,
) -> McloneOverworldHydraulicClosureReport {
    let radius = i32::try_from(radius_chunks).expect("hydraulic review radius must fit i32");
    let halo_radius = radius
        .checked_add(1)
        .expect("hydraulic review halo radius overflow");
    let mut chunks = BTreeMap::new();
    let mut stream_cache = McloneOverworldStreamPlanCache::new(seed, topology);
    for chunk_z in center.z - halo_radius..=center.z + halo_radius {
        for chunk_x in center.x - halo_radius..=center.x + halo_radius {
            let pos = ChunkPos::new(chunk_x, chunk_z);
            chunks.insert(
                pos,
                generate_mclone_overworld_surface_chunk_with_stream_cache(
                    seed,
                    topology,
                    chunk_x,
                    chunk_z,
                    &mut stream_cache,
                ),
            );
        }
    }

    let mut report = McloneOverworldHydraulicClosureReport::default();
    for chunk_z in center.z - radius..=center.z + radius {
        for chunk_x in center.x - radius..=center.x + radius {
            let pos = ChunkPos::new(chunk_x, chunk_z);
            let chunk = chunks
                .get(&pos)
                .expect("hydraulic target chunk must be generated");
            report.target_chunks += 1;
            report.scheduled_liquid_ticks += chunk.liquid_ticks().len();
            let min_x = chunk_min_block_coord(chunk_x);
            let min_z = chunk_min_block_coord(chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let world_x = min_x + local_x;
                    let world_z = min_z + local_z;
                    let mut top_water_y = None;
                    let mut column_has_flowing = false;
                    for y in chunk.min_y..chunk.min_y + chunk.height {
                        let block = chunk.block_at_y(local_x, y, local_z).0;
                        if !crate::block::is_water(block) {
                            continue;
                        }
                        top_water_y = Some(y);
                        if block != crate::block::WATER {
                            report.flowing_water_blocks += 1;
                            column_has_flowing = true;
                            if block == crate::block::WATER_LEVEL_8 {
                                let above = hydraulic_block_at(&chunks, world_x, y + 1, world_z);
                                if !above.is_some_and(crate::block::is_water) {
                                    report.unsupported_flowing_blocks += 1;
                                }
                            }
                            continue;
                        }
                        report.source_water_blocks += 1;
                        let below = hydraulic_block_at(&chunks, world_x, y - 1, world_z);
                        if !below.is_some_and(crate::block::is_water)
                            && !below.is_some_and(crate::block::material_blocks_motion)
                        {
                            report.unsupported_source_blocks += 1;
                        }

                        let mut boundary = false;
                        for (offset_x, offset_z) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            let neighbor = hydraulic_block_at(
                                &chunks,
                                world_x + offset_x,
                                y,
                                world_z + offset_z,
                            );
                            if neighbor.is_some_and(crate::block::is_water) {
                                continue;
                            }
                            boundary = true;
                            if !neighbor.is_some_and(crate::block::material_blocks_motion) {
                                report.horizontally_open_source_faces += 1;
                                report
                                    .first_open_source
                                    .get_or_insert([world_x, y, world_z]);
                            }
                        }
                        if boundary {
                            report.source_boundary_blocks += 1;
                        }
                    }

                    for (offset_x, offset_z) in [(1, 0), (0, 1)] {
                        let Some(neighbor_top) =
                            hydraulic_top_water_y(&chunks, world_x + offset_x, world_z + offset_z)
                        else {
                            continue;
                        };
                        if top_water_y.is_some_and(|top| top != neighbor_top) {
                            if column_has_flowing
                                || hydraulic_column_has_flowing(
                                    &chunks,
                                    world_x + offset_x,
                                    world_z + offset_z,
                                )
                            {
                                report.intentional_drop_edges += 1;
                            } else {
                                report.sloped_surface_edges += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    report
}

fn hydraulic_block_at(
    chunks: &BTreeMap<ChunkPos, GeneratedChunk>,
    world_x: i32,
    y: i32,
    world_z: i32,
) -> Option<u8> {
    let pos = ChunkPos::new(block_to_chunk_coord(world_x), block_to_chunk_coord(world_z));
    let chunk = chunks.get(&pos)?;
    (y >= chunk.min_y && y < chunk.min_y + chunk.height).then(|| {
        chunk
            .block_at_y(local_block_coord(world_x), y, local_block_coord(world_z))
            .0
    })
}

fn hydraulic_top_water_y(
    chunks: &BTreeMap<ChunkPos, GeneratedChunk>,
    world_x: i32,
    world_z: i32,
) -> Option<i32> {
    let pos = ChunkPos::new(block_to_chunk_coord(world_x), block_to_chunk_coord(world_z));
    let chunk = chunks.get(&pos)?;
    (chunk.min_y..chunk.min_y + chunk.height).rev().find(|y| {
        let block = chunk
            .block_at_y(local_block_coord(world_x), *y, local_block_coord(world_z))
            .0;
        crate::block::is_water(block)
    })
}

fn hydraulic_column_has_flowing(
    chunks: &BTreeMap<ChunkPos, GeneratedChunk>,
    world_x: i32,
    world_z: i32,
) -> bool {
    let pos = ChunkPos::new(block_to_chunk_coord(world_x), block_to_chunk_coord(world_z));
    let Some(chunk) = chunks.get(&pos) else {
        return false;
    };
    (chunk.min_y..chunk.min_y + chunk.height).any(|y| {
        let block = chunk
            .block_at_y(local_block_coord(world_x), y, local_block_coord(world_z))
            .0;
        crate::block::is_water(block) && block != crate::block::WATER
    })
}

pub(super) fn generate_mclone_overworld_surface_buffer_with_stream_cache(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    chunk_x: i32,
    chunk_z: i32,
    stream_cache: &mut McloneOverworldStreamPlanCache,
) -> MutableChunkBlockBuffer {
    let min_x = chunk_min_block_coord(chunk_x);
    let min_z = chunk_min_block_coord(chunk_z);
    let samples =
        ChunkLandformSamples::new_with_stream_cache(seed, topology, min_x, min_z, stream_cache);
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
            let fall_top_flow_level =
                baked_fall_top_flow_level(samples, local_x, local_z).unwrap_or(1);
            let landform = samples.landform(local_x, local_z);
            write_surface_column(&mut buffer, local_x, local_z, landform, fall_top_flow_level);
        }
    }
    buffer.prime_worldgen_heightmaps();

    buffer
}

fn baked_fall_top_flow_level(
    samples: &ChunkLandformSamples,
    local_x: i32,
    local_z: i32,
) -> Option<u8> {
    let watercourse = samples.terrain(local_x, local_z).watercourse;
    watercourse
        .is_fall_column()
        .then(|| (-watercourse.drop_distance).ceil().clamp(0.0, 7.0) as u8)
}

pub(super) fn mclone_overworld_chunk_biomes_with_stream_cache(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    min_x: i32,
    min_z: i32,
    stream_cache: &mut McloneOverworldStreamPlanCache,
) -> Vec<i32> {
    let samples =
        ChunkLandformSamples::new_with_stream_cache(seed, topology, min_x, min_z, stream_cache);
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
    fn new_with_stream_cache(
        seed: i64,
        topology: McloneOverworldSamplingTopology,
        min_x: i32,
        min_z: i32,
        stream_cache: &mut McloneOverworldStreamPlanCache,
    ) -> Self {
        assert!(
            stream_cache.matches(seed, topology),
            "Mclone stream cache seed/topology must match terrain generation"
        );
        let sampler = McloneOverworldSampler::new_with_topology(seed, topology);
        let center = ChunkPos::new(block_to_chunk_coord(min_x), block_to_chunk_coord(min_z));
        let stream_plans = stream_cache
            .plans_intersecting_chunks(
                ChunkPos::new(center.x - 1, center.z - 1),
                ChunkPos::new(center.x + 1, center.z + 1),
            )
            .expect("Mclone stream placement constants must remain valid");
        let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
        let width = usize::try_from(CHUNK_LANDFORM_SAMPLE_WIDTH)
            .expect("Mclone chunk landform sample width must fit usize");
        let mut terrain = Vec::with_capacity(width * width);
        for offset_z in -radius..CHUNK_WIDTH + radius {
            for offset_x in -radius..CHUNK_WIDTH + radius {
                let world_x = min_x + offset_x;
                let world_z = min_z + offset_z;
                let mut sample = sampler.sample(world_x, world_z);
                apply_stream_plans(&mut sample, world_x, world_z, &stream_plans);
                terrain.push(sample);
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

fn apply_stream_plans(
    sample: &mut McloneOverworldTerrainSample,
    world_x: i32,
    world_z: i32,
    plans: &[McloneOverworldStreamPlan],
) {
    let intent = plans
        .iter()
        .filter(|plan| {
            world_x >= plan.structure.bounds.min_x
                && world_x <= plan.structure.bounds.max_x
                && world_z >= plan.structure.bounds.min_z
                && world_z <= plan.structure.bounds.max_z
        })
        .filter_map(|plan| plan.terrain_intent(world_x, world_z, sample.base_surface_y))
        .max_by(|left, right| left.influence.total_cmp(&right.influence));
    let Some(intent) = intent else {
        return;
    };

    sample.surface_y = sample.surface_y.min(intent.target_surface_y);
    sample.watercourse.distance = sample.watercourse.distance.min(intent.distance);
    sample.watercourse.bank_influence =
        sample.watercourse.bank_influence.max(intent.bank_influence);
    sample.watercourse.planned_stream_influence = sample
        .watercourse
        .planned_stream_influence
        .max(intent.influence);
    if intent.channel_influence == 0.0 {
        return;
    }

    sample.watercourse.channel_influence = sample
        .watercourse
        .channel_influence
        .max(intent.channel_influence);
    sample.watercourse.stream_headwater_influence = if intent.is_headwater {
        intent.channel_influence
    } else {
        0.0
    };
    sample.watercourse.half_width = intent.half_width;
    sample.watercourse.water_surface_y = intent.water_y;
    sample.watercourse.bed_y = intent.bed_y;
    sample.watercourse.tangent_x = intent.tangent_x;
    sample.watercourse.tangent_z = intent.tangent_z;
    sample.watercourse.flow_x = intent.tangent_x;
    sample.watercourse.flow_z = intent.tangent_z;
    sample.watercourse.grade = if intent.drop_height > 0 {
        f64::from(intent.drop_height) / intent.drop_distance.abs().max(1.0)
    } else {
        0.0
    };
    sample.watercourse.drop_distance = intent.drop_distance;
    sample.watercourse.drop_height = intent.drop_height;
    sample.watercourse.drop_upper_y = intent.drop_upper_y;
    sample.watercourse.drop_lower_y = intent.drop_lower_y;
}

#[cfg(test)]
mod tests {
    use crate::block::{AIR, CLAY, GRASS_BLOCK, GRAVEL, SAND, STONE, WATER};

    use super::*;
    use crate::levelgen::mclone_overworld::biomes::{
        MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_RIVER_BIOME_ID,
        mclone_overworld_biome_id_for_sample,
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
                        McloneOverworldSurfaceRecipe::RiverBed => GRAVEL,
                        McloneOverworldSurfaceRecipe::WetlandBed => CLAY,
                        McloneOverworldSurfaceRecipe::RiverBank
                            if sample.terrain.base_surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 5 =>
                        {
                            SAND
                        }
                        McloneOverworldSurfaceRecipe::RiverBank => GRASS_BLOCK,
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
                        if sample.terrain.watercourse.is_water()
                            || sample.terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL
                        {
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
    fn flat_reaches_are_hydraulically_closed_across_regions_and_periodic_seam() {
        for (seed, topology, center) in [
            (
                -98_765,
                McloneOverworldSamplingTopology::Unbounded,
                ChunkPos::new(-25, 72),
            ),
            (
                -98_765,
                McloneOverworldSamplingTopology::Unbounded,
                ChunkPos::new(183, -177),
            ),
            (
                12_345,
                McloneOverworldSamplingTopology::Unbounded,
                ChunkPos::new(141, 100),
            ),
            (
                -98_765,
                McloneOverworldSamplingTopology::PeriodicX,
                ChunkPos::new(0, -190),
            ),
            (
                -98_765,
                McloneOverworldSamplingTopology::PeriodicX,
                ChunkPos::new(-75, -95),
            ),
        ] {
            let report = analyze_mclone_overworld_hydraulic_closure(seed, topology, center, 1);
            assert!(report.source_water_blocks > 0, "{seed} {center:?}");
            assert!(report.source_boundary_blocks > 0, "{seed} {center:?}");
            assert!(report.is_closed(), "{seed} {center:?}: {report:?}");
        }
    }

    #[test]
    fn planned_stream_is_closed_across_its_complete_source_to_sink_bounds() {
        let report = analyze_mclone_overworld_hydraulic_closure(
            -98_765,
            McloneOverworldSamplingTopology::Unbounded,
            ChunkPos::new(149, -124),
            4,
        );
        assert!(report.source_water_blocks > 1_000, "{report:?}");
        assert!(report.flowing_water_blocks > 0, "{report:?}");
        assert!(report.intentional_drop_edges >= 3, "{report:?}");
        assert!(report.is_closed(), "{report:?}");
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
                        .chain(sample.mountain_detail.to_bits().to_le_bytes())
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
                (16_814_275_564_477_349_497, 540_454_697_130_909_605),
                (17_836_579_662_090_859_138, 3_995_179_115_581_767_979),
                (9_399_436_600_503_439_313, 14_722_381_067_837_031_305),
            ]
        );
    }

    #[test]
    fn selected_regions_exercise_and_pin_biome_and_surface_language() {
        let receipts = [12_345, -98_765, 8_675_309].map(|seed| {
            let sampler = McloneOverworldSampler::new(seed);
            let mut biome_counts = [0_u32; 5];
            let mut surface_counts = [0_u32; 7];
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
                        MCLONE_OVERWORLD_RIVER_BIOME_ID => 4,
                        _ => panic!("unexpected Mclone biome ID {biome_id}"),
                    };
                    let surface_index = match mclone_overworld_surface_recipe(landform) {
                        McloneOverworldSurfaceRecipe::OceanFloor => 0,
                        McloneOverworldSurfaceRecipe::Beach => 1,
                        McloneOverworldSurfaceRecipe::RiverBed => 2,
                        McloneOverworldSurfaceRecipe::WetlandBed => 3,
                        McloneOverworldSurfaceRecipe::RiverBank => 4,
                        McloneOverworldSurfaceRecipe::GrassSoil => 5,
                        McloneOverworldSurfaceRecipe::ExposedStone => 6,
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
                    [21_961, 9_556, 18_465, 14_117, 1_437],
                    [16_943, 12_607, 1_395, 42, 2_560, 31_884, 105],
                    3_897_804_533_720_171_775,
                ),
                (
                    [17_223, 8_076, 17_269, 21_253, 1_715],
                    [12_358, 11_054, 1_604, 111, 2_850, 37_271, 288],
                    110_489_083_812_910_482,
                ),
                (
                    [33_641, 11_579, 10_825, 8_519, 972],
                    [26_125, 17_799, 900, 72, 1_887, 18_745, 8],
                    16_139_248_388_008_531_613,
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
                5_584_272_403_799_234_324,
                53_515_349_257_108_940,
                171_330_102_405_640_746,
            ]
        );
    }
}
