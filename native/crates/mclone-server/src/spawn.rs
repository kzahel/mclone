use mclone_core::{BlockPos, ChunkPos, HorizontalTopology, Vec3d};
use mclone_worldgen::biome::{BiomeDefinition, OverworldBiomeSource, get_layered_biome_by_id};
use mclone_worldgen::block::{
    GRASS_BLOCK, PODZOL, RawBlockId, has_fluid, is_air_like, material_blocks_motion,
};
use mclone_worldgen::continental_ecoregion::ContinentalEcoregionDescriptor;
use mclone_worldgen::continental_surface::ContinentalSurfacePlan;
use mclone_worldgen::levelgen::{
    McloneOverworldSamplingTopology, TopologyProbeSource, beta_biome_id,
    mclone_overworld_biome_id_with_topology, mclone_overworld_spawn_chunk_with_topology,
};
use mclone_worldgen::mclone_overworld_v3::{McloneOverworldV3TerrainPlan, V3TerrainWindowRequest};
use mclone_worldgen::surface::overworld_surface_top_material;

use crate::WorldGenerationProfile;
use crate::game_mode::JAVA_OVERWORLD_MAX_BUILD_HEIGHT;

const JAVA_OVERWORLD_MIN_BUILD_HEIGHT: i32 = 0;
const JAVA_INITIAL_SPAWN_MAX_CHUNK_STEPS: usize = 1024;
const JAVA_INITIAL_SPAWN_CHUNK_RADIUS: i32 = 16;

pub fn initial_spawn_center_for_seed(seed: i64) -> ChunkPos {
    OverworldBiomeSource::new(seed, false, false)
        .find_player_spawn_friendly_chunk()
        .unwrap_or(ChunkPos::new(0, 0))
}

pub fn initial_spawn_center_for_profile(seed: i64, profile: WorldGenerationProfile) -> ChunkPos {
    initial_spawn_center_for_descriptor(seed, profile, HorizontalTopology::UNBOUNDED)
}

pub fn initial_spawn_center_for_descriptor(
    seed: i64,
    profile: WorldGenerationProfile,
    topology: HorizontalTopology,
) -> ChunkPos {
    match profile {
        WorldGenerationProfile::Overworld => initial_spawn_center_for_seed(seed),
        WorldGenerationProfile::McloneOverworldV1 => mclone_overworld_spawn_chunk_with_topology(
            seed,
            McloneOverworldSamplingTopology::from_horizontal_topology(topology)
                .expect("Mclone spawn topology must pass profile admission"),
        ),
        WorldGenerationProfile::McloneOverworldV2 => continental_overworld_spawn_chunk(seed),
        WorldGenerationProfile::McloneOverworldV3 => mclone_overworld_v3_spawn_chunk(seed),
        WorldGenerationProfile::TopologyProbeV1 => TopologyProbeSource::new(seed, topology)
            .expect("topology probe spawn topology must pass profile admission")
            .origin_chunk(),
        WorldGenerationProfile::FlatGrassV1
        | WorldGenerationProfile::SmallIslandV1
        | WorldGenerationProfile::AlphaV1 { .. }
        | WorldGenerationProfile::BetaV1
        | WorldGenerationProfile::AuthoredOnly { .. } => ChunkPos::new(0, 0),
    }
}

/// Resolve the same safe surface used by ordinary player admission from an
/// already-published client/observer snapshot. Preview hosts use this to frame
/// an observer around its eventual join point without manufacturing a player
/// or publishing player-position authority before activation.
pub fn find_safe_surface_spawn_for_loaded_profile(
    seed: i64,
    profile: WorldGenerationProfile,
    center: ChunkPos,
    block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
    chunk_ready: impl FnMut(ChunkPos) -> bool,
) -> Option<Vec3d> {
    find_safe_surface_spawn_for_loaded_descriptor(
        seed,
        profile,
        HorizontalTopology::UNBOUNDED,
        center,
        block_at,
        chunk_ready,
    )
}

pub fn find_safe_surface_spawn_for_loaded_descriptor(
    seed: i64,
    profile: WorldGenerationProfile,
    topology: HorizontalTopology,
    center: ChunkPos,
    block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
    chunk_ready: impl FnMut(ChunkPos) -> bool,
) -> Option<Vec3d> {
    let column_order = if matches!(
        profile,
        WorldGenerationProfile::FlatGrassV1
            | WorldGenerationProfile::SmallIslandV1
            | WorldGenerationProfile::McloneOverworldV1
            | WorldGenerationProfile::McloneOverworldV2
            | WorldGenerationProfile::McloneOverworldV3
            | WorldGenerationProfile::TopologyProbeV1
            | WorldGenerationProfile::AlphaV1 { .. }
            | WorldGenerationProfile::BetaV1
            | WorldGenerationProfile::AuthoredOnly { .. }
    ) {
        SpawnColumnOrder::CenterFirst
    } else {
        SpawnColumnOrder::Scan
    };
    let mclone_sampling_topology = matches!(profile, WorldGenerationProfile::McloneOverworldV1)
        .then(|| {
            McloneOverworldSamplingTopology::from_horizontal_topology(topology)
                .expect("Mclone spawn topology must pass profile admission")
        });
    if profile == WorldGenerationProfile::McloneOverworldV2 {
        let continental_surface =
            ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(seed))
                .expect("V2 spawn uses a valid unbounded continental surface");
        return find_safe_surface_spawn_with_top_material(
            center,
            block_at,
            |x, z| {
                continental_surface
                    .query_point(x, z)
                    .sample
                    .substrate
                    .block_id()
            },
            chunk_ready,
            SpawnColumnOrder::CenterFirst,
            is_valid_continental_spawn_surface_block,
        );
    }
    if profile == WorldGenerationProfile::McloneOverworldV3 {
        let terrain = McloneOverworldV3TerrainPlan::new(seed);
        return find_safe_surface_spawn_with_top_material(
            center,
            block_at,
            |x, z| terrain.query_point(x, z).sample.visible_material(),
            chunk_ready,
            SpawnColumnOrder::CenterFirst,
            is_valid_continental_spawn_surface_block,
        );
    }
    let biome_source = OverworldBiomeSource::new(seed, false, false);
    find_safe_surface_spawn_with_column_order(
        center,
        block_at,
        |x, z| match profile {
            WorldGenerationProfile::FlatGrassV1
            | WorldGenerationProfile::SmallIslandV1
            | WorldGenerationProfile::TopologyProbeV1
            | WorldGenerationProfile::AlphaV1 { .. } => get_layered_biome_by_id(1),
            WorldGenerationProfile::McloneOverworldV1 => {
                get_layered_biome_by_id(mclone_overworld_biome_id_with_topology(
                    seed,
                    mclone_sampling_topology.expect("Mclone topology initialized above"),
                    x,
                    z,
                ))
            }
            WorldGenerationProfile::McloneOverworldV2 => {
                unreachable!("V2 spawn uses its exact substrate above")
            }
            WorldGenerationProfile::McloneOverworldV3 => {
                unreachable!("V3 spawn uses its exact substrate above")
            }
            WorldGenerationProfile::BetaV1 => get_layered_biome_by_id(beta_biome_id(seed, x, z)),
            WorldGenerationProfile::Overworld | WorldGenerationProfile::AuthoredOnly { .. } => {
                biome_source.get_block_position_biome_definition(seed, x, z)
            }
        },
        chunk_ready,
        column_order,
    )
}

fn continental_overworld_spawn_chunk(seed: i64) -> ChunkPos {
    const SAMPLE_STRIDE_CHUNKS: i32 = 32;
    const MAX_SAMPLE_RADIUS: i32 = 128;

    let surface = ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(seed))
        .expect("V2 spawn uses a valid unbounded continental surface");
    let accept = |sample_x: i32, sample_z: i32| {
        let chunk = ChunkPos::new(
            sample_x.saturating_mul(SAMPLE_STRIDE_CHUNKS),
            sample_z.saturating_mul(SAMPLE_STRIDE_CHUNKS),
        );
        let sample = surface
            .query_point(chunk.min_block_x() + 8, chunk.min_block_z() + 8)
            .sample;
        (!sample.is_water() && sample.solid_surface_y >= 63.0).then_some(chunk)
    };

    if let Some(chunk) = accept(0, 0) {
        return chunk;
    }
    for radius in 1..=MAX_SAMPLE_RADIUS {
        for x in -radius..=radius {
            if let Some(chunk) = accept(x, -radius) {
                return chunk;
            }
            if let Some(chunk) = accept(x, radius) {
                return chunk;
            }
        }
        for z in (-radius + 1)..radius {
            if let Some(chunk) = accept(-radius, z) {
                return chunk;
            }
            if let Some(chunk) = accept(radius, z) {
                return chunk;
            }
        }
    }
    ChunkPos::new(0, 0)
}

/// Select a deterministic lived-scale V3 start: locally walkable and open,
/// while keeping a strong landform or meaningful elevation change within a
/// short journey. The bounded 16-kiloblock review window is sampled once and
/// does not generate chunks or populate any dependency cache.
fn mclone_overworld_v3_spawn_chunk(seed: i64) -> ChunkPos {
    const RADIUS_BLOCKS: i32 = 8_192;
    const STEP_BLOCKS: i32 = 256;
    const WIDTH: usize = (RADIUS_BLOCKS as usize * 2 / STEP_BLOCKS as usize) + 1;
    const LANDMARK_OFFSETS: [usize; 2] = [1, 2];

    let terrain = McloneOverworldV3TerrainPlan::new(seed);
    let window = terrain
        .query_lod_window(V3TerrainWindowRequest::new(
            -RADIUS_BLOCKS,
            -RADIUS_BLOCKS,
            WIDTH as u32,
            WIDTH as u32,
            STEP_BLOCKS as u32,
        ))
        .expect("the fixed V3 spawn review window is valid");
    let at = |sample_x: usize, sample_z: usize| window.samples[sample_z * WIDTH + sample_x];
    let mut best: Option<(f32, i64, i32, i32)> = None;

    for sample_z in 8..(WIDTH - 8) {
        for sample_x in 8..(WIDTH - 8) {
            let sample = at(sample_x, sample_z);
            if sample.is_water()
                || sample.land_weight < 0.62
                || !(66.0..=100.0).contains(&sample.solid_surface_y)
                || sample.openness < 0.40
                || sample.range_strength > 0.28
                || sample.plateau > 0.35
                || sample.high_axis > 0.36
                || sample.escarpment > 0.35
            {
                continue;
            }

            let local_max_delta = [(-8, 0), (8, 0), (0, -8), (0, 8)]
                .into_iter()
                .map(|(dx, dz)| {
                    (terrain
                        .query_point(sample.world_x + dx, sample.world_z + dz)
                        .sample
                        .solid_surface_y
                        - sample.solid_surface_y)
                        .abs()
                })
                .fold(0.0_f32, f32::max);
            if local_max_delta > 3.5 {
                continue;
            }

            let mut nearby_uphill_relief = 0.0_f32;
            let mut nearby_landmark = 0.0_f32;
            for offset in LANDMARK_OFFSETS {
                for (offset_x, offset_z) in [
                    (-(offset as isize), 0),
                    (offset as isize, 0),
                    (0, -(offset as isize)),
                    (0, offset as isize),
                    (-(offset as isize), -(offset as isize)),
                    (offset as isize, -(offset as isize)),
                    (-(offset as isize), offset as isize),
                    (offset as isize, offset as isize),
                ] {
                    let neighbor = at(
                        sample_x.saturating_add_signed(offset_x),
                        sample_z.saturating_add_signed(offset_z),
                    );
                    nearby_uphill_relief =
                        nearby_uphill_relief.max(neighbor.solid_surface_y - sample.solid_surface_y);
                    nearby_landmark = nearby_landmark.max(
                        neighbor
                            .range_strength
                            .max(neighbor.high_axis)
                            .max(neighbor.plateau * 0.82)
                            .max(neighbor.escarpment * 0.75),
                    );
                }
            }
            if nearby_uphill_relief < 120.0 || nearby_landmark < 0.42 {
                continue;
            }

            let distance_squared =
                i64::from(sample.world_x).pow(2) + i64::from(sample.world_z).pow(2);
            let distance_penalty = (distance_squared as f32).sqrt() / RADIUS_BLOCKS as f32;
            let score = sample.openness * 3.2 + sample.clearing * 1.5
                - sample.forest_opportunity * 0.8
                - (sample.solid_surface_y - 76.0).abs() * 0.08
                - local_max_delta * 0.12
                + (nearby_uphill_relief / 64.0).min(3.0)
                + nearby_landmark * 3.0
                - distance_penalty * 0.9;
            let candidate = (score, distance_squared, sample.world_x, sample.world_z);
            if best.is_none_or(|current| {
                candidate.0.total_cmp(&current.0).is_gt()
                    || (candidate.0 == current.0
                        && (candidate.1, candidate.3, candidate.2)
                            < (current.1, current.3, current.2))
            }) {
                best = Some(candidate);
            }
        }
    }

    let (_, _, world_x, world_z) = best.unwrap_or_else(|| {
        let fallback = window
            .samples
            .iter()
            .copied()
            .filter(|sample| !sample.is_water() && sample.solid_surface_y >= 64.0)
            .min_by_key(|sample| {
                i64::from(sample.world_x).pow(2) + i64::from(sample.world_z).pow(2)
            })
            .unwrap_or_else(|| terrain.query_point(0, 0).sample);
        (0.0, 0, fallback.world_x, fallback.world_z)
    });
    ChunkPos::new(world_x.div_euclid(16), world_z.div_euclid(16))
}

#[cfg(test)]
pub(crate) fn find_safe_surface_spawn(
    center: ChunkPos,
    mut block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
    mut biome_at: impl FnMut(i32, i32) -> BiomeDefinition,
    mut chunk_ready: impl FnMut(ChunkPos) -> bool,
) -> Option<Vec3d> {
    find_safe_surface_spawn_with_column_order(
        center,
        &mut block_at,
        &mut biome_at,
        &mut chunk_ready,
        SpawnColumnOrder::Scan,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnColumnOrder {
    Scan,
    CenterFirst,
}

pub(crate) fn find_safe_surface_spawn_with_column_order(
    center: ChunkPos,
    block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
    mut biome_at: impl FnMut(i32, i32) -> BiomeDefinition,
    chunk_ready: impl FnMut(ChunkPos) -> bool,
    column_order: SpawnColumnOrder,
) -> Option<Vec3d> {
    find_safe_surface_spawn_with_top_material(
        center,
        block_at,
        |x, z| overworld_surface_top_material(biome_at(x, z)),
        chunk_ready,
        column_order,
        is_valid_spawn_surface_block,
    )
}

fn find_safe_surface_spawn_with_top_material(
    center: ChunkPos,
    mut block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
    mut top_material_at: impl FnMut(i32, i32) -> RawBlockId,
    mut chunk_ready: impl FnMut(ChunkPos) -> bool,
    column_order: SpawnColumnOrder,
    valid_surface: fn(RawBlockId) -> bool,
) -> Option<Vec3d> {
    for chunk in initial_spawn_chunks(center) {
        if !chunk_ready(chunk) {
            continue;
        }

        for (x, z) in ordered_chunk_columns(chunk, column_order) {
            let top_material = top_material_at(x, z);
            if let Some(feet_y) =
                safe_feet_y_at_column(x, z, top_material, valid_surface, &mut block_at)
            {
                return Some(Vec3d::new(
                    x as f64 + 0.5,
                    f64::from(feet_y),
                    z as f64 + 0.5,
                ));
            }
        }
    }
    None
}

fn safe_feet_y_at_column(
    x: i32,
    z: i32,
    top_material: RawBlockId,
    valid_surface: fn(RawBlockId) -> bool,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<i32> {
    if !valid_surface(top_material) {
        return None;
    }

    let heights = column_heightmap_floors(x, z, block_at)?;
    if heights.world_surface <= heights.motion_blocking
        && heights.world_surface > heights.ocean_floor
    {
        return None;
    }

    if heights.motion_blocking + 1 >= JAVA_OVERWORLD_MAX_BUILD_HEIGHT {
        return None;
    }

    for y in (JAVA_OVERWORLD_MIN_BUILD_HEIGHT..=heights.motion_blocking + 1).rev() {
        let block = block_at(BlockPos::new(x, y, z))?;
        if has_fluid(block) {
            break;
        }

        if block == top_material {
            let feet_y = y + 1;
            return has_spawn_clearance(x, feet_y, z, block_at).then_some(feet_y);
        }
    }

    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpawnColumnHeights {
    motion_blocking: i32,
    world_surface: i32,
    ocean_floor: i32,
}

fn column_heightmap_floors(
    x: i32,
    z: i32,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<SpawnColumnHeights> {
    let mut motion_blocking = None;
    let mut world_surface = None;
    let mut ocean_floor = None;

    for y in (JAVA_OVERWORLD_MIN_BUILD_HEIGHT..JAVA_OVERWORLD_MAX_BUILD_HEIGHT).rev() {
        let block = block_at(BlockPos::new(x, y, z))?;
        if motion_blocking.is_none() && is_motion_blocking_heightmap_block(block) {
            motion_blocking = Some(y);
        }
        if world_surface.is_none() && !is_air_like(block) {
            world_surface = Some(y);
        }
        if ocean_floor.is_none() && material_blocks_motion(block) {
            ocean_floor = Some(y);
        }

        if motion_blocking.is_some() && world_surface.is_some() && ocean_floor.is_some() {
            break;
        }
    }

    Some(SpawnColumnHeights {
        motion_blocking: motion_blocking?,
        world_surface: world_surface?,
        ocean_floor: ocean_floor?,
    })
}

fn is_motion_blocking_heightmap_block(block: RawBlockId) -> bool {
    material_blocks_motion(block) || has_fluid(block)
}

fn chunk_columns(chunk: ChunkPos) -> impl Iterator<Item = (i32, i32)> {
    let min_x = chunk.min_block_x();
    let min_z = chunk.min_block_z();
    (min_x..min_x + 16).flat_map(move |x| (min_z..min_z + 16).map(move |z| (x, z)))
}

fn ordered_chunk_columns(chunk: ChunkPos, order: SpawnColumnOrder) -> Vec<(i32, i32)> {
    let mut columns = chunk_columns(chunk).collect::<Vec<_>>();
    if order == SpawnColumnOrder::CenterFirst {
        let min_x = chunk.min_block_x();
        let min_z = chunk.min_block_z();
        columns.sort_by_key(|&(x, z)| {
            let doubled_x_from_center = 2 * (x - min_x) - 15;
            let doubled_z_from_center = 2 * (z - min_z) - 15;
            (
                doubled_x_from_center * doubled_x_from_center
                    + doubled_z_from_center * doubled_z_from_center,
                x,
                z,
            )
        });
    }
    columns
}

fn initial_spawn_chunks(center: ChunkPos) -> impl Iterator<Item = ChunkPos> {
    let mut chunks = Vec::new();
    let mut x = 0;
    let mut z = 0;
    let mut step_x = 0;
    let mut step_z = -1;

    for _ in 0..JAVA_INITIAL_SPAWN_MAX_CHUNK_STEPS {
        if x > -JAVA_INITIAL_SPAWN_CHUNK_RADIUS
            && x <= JAVA_INITIAL_SPAWN_CHUNK_RADIUS
            && z > -JAVA_INITIAL_SPAWN_CHUNK_RADIUS
            && z <= JAVA_INITIAL_SPAWN_CHUNK_RADIUS
        {
            chunks.push(ChunkPos::new(center.x + x, center.z + z));
        }

        if x == z || (x < 0 && x == -z) || (x > 0 && x == 1 - z) {
            let old_step_x = step_x;
            step_x = -step_z;
            step_z = old_step_x;
        }
        x += step_x;
        z += step_z;
    }

    chunks.into_iter()
}

fn is_spawn_space(block: RawBlockId) -> bool {
    !material_blocks_motion(block) && !has_fluid(block)
}

fn is_valid_spawn_surface_block(block: RawBlockId) -> bool {
    matches!(block, GRASS_BLOCK | PODZOL)
}

fn is_valid_continental_spawn_surface_block(block: RawBlockId) -> bool {
    material_blocks_motion(block) && !has_fluid(block)
}

fn has_spawn_clearance(
    x: i32,
    feet_y: i32,
    z: i32,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> bool {
    if feet_y >= JAVA_OVERWORLD_MAX_BUILD_HEIGHT - 1 {
        return false;
    }

    let Some(feet) = block_at(BlockPos::new(x, feet_y, z)) else {
        return false;
    };
    let Some(head) = block_at(BlockPos::new(x, feet_y + 1, z)) else {
        return false;
    };
    is_spawn_space(feet) && is_spawn_space(head)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mclone_worldgen::biome::get_layered_biome_by_id;
    use mclone_worldgen::block::{AIR, GRASS_BLOCK, OAK_LEAVES, STONE, WATER};
    use mclone_worldgen::levelgen::{generate_alpha_chunk, generate_beta_chunk};
    use mclone_worldgen::levelgen::{
        generate_continental_candidate_chunk, generate_mclone_overworld_chunk,
        generate_mclone_overworld_chunk_with_topology, generate_mclone_overworld_v3_chunk,
    };

    use super::*;

    fn plains_biome(_x: i32, _z: i32) -> BiomeDefinition {
        get_layered_biome_by_id(1)
    }

    #[test]
    fn finds_center_surface_spawn_with_two_clear_blocks() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(0, 63, 0), GRASS_BLOCK);

        let spawn = find_safe_surface_spawn(
            ChunkPos::new(0, 0),
            |pos| Some(*blocks.get(&pos).unwrap_or(&AIR)),
            plains_biome,
            |_| true,
        )
        .expect("spawn");

        assert_eq!(spawn, Vec3d::new(0.5, 64.0, 0.5));
    }

    #[test]
    fn authored_center_first_policy_prefers_supported_island_interior() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(1, 64, 8), GRASS_BLOCK);
        blocks.insert(BlockPos::new(7, 64, 7), GRASS_BLOCK);

        let scan = find_safe_surface_spawn(
            ChunkPos::new(0, 0),
            |pos| Some(*blocks.get(&pos).unwrap_or(&AIR)),
            plains_biome,
            |_| true,
        )
        .expect("scan-order spawn");
        let center_first = find_safe_surface_spawn_with_column_order(
            ChunkPos::new(0, 0),
            |pos| Some(*blocks.get(&pos).unwrap_or(&AIR)),
            plains_biome,
            |_| true,
            SpawnColumnOrder::CenterFirst,
        )
        .expect("center-first spawn");

        assert_eq!(scan, Vec3d::new(1.5, 65.0, 8.5));
        assert_eq!(center_first, Vec3d::new(7.5, 65.0, 7.5));
    }

    #[test]
    fn mclone_overworld_profile_selects_a_loaded_dry_spawn() {
        for seed in [12_345, -98_765, 8_675_309] {
            let profile = WorldGenerationProfile::McloneOverworldV1;
            let center = initial_spawn_center_for_profile(seed, profile);
            let chunk = generate_mclone_overworld_chunk(seed, center.x, center.z);
            let spawn = find_safe_surface_spawn_for_loaded_profile(
                seed,
                profile,
                center,
                |pos| {
                    (pos.chunk_pos() == center).then(|| {
                        chunk
                            .block_at_y(
                                pos.x - center.min_block_x(),
                                pos.y,
                                pos.z - center.min_block_z(),
                            )
                            .0
                    })
                },
                |pos| pos == center,
            )
            .unwrap_or_else(|| panic!("seed {seed} had no safe spawn in {center:?}"));
            let floor = BlockPos::new(
                spawn.x.floor() as i32,
                spawn.y.floor() as i32 - 1,
                spawn.z.floor() as i32,
            );
            assert_eq!(floor.chunk_pos(), center);
            assert_eq!(
                chunk
                    .block_at_y(
                        floor.x - center.min_block_x(),
                        floor.y,
                        floor.z - center.min_block_z()
                    )
                    .0,
                GRASS_BLOCK
            );
        }
    }

    #[test]
    fn mclone_overworld_v2_selects_a_stable_loaded_dry_spawn() {
        for seed in [12_345, -98_765, 8_675_309] {
            let profile = WorldGenerationProfile::McloneOverworldV2;
            let center = initial_spawn_center_for_profile(seed, profile);
            assert_eq!(center, initial_spawn_center_for_profile(seed, profile));
            let chunk = generate_continental_candidate_chunk(seed, center.x, center.z);
            let spawn = find_safe_surface_spawn_for_loaded_profile(
                seed,
                profile,
                center,
                |pos| {
                    (pos.chunk_pos() == center).then(|| {
                        chunk
                            .block_at_y(
                                pos.x - center.min_block_x(),
                                pos.y,
                                pos.z - center.min_block_z(),
                            )
                            .0
                    })
                },
                |pos| pos == center,
            )
            .unwrap_or_else(|| panic!("seed {seed} had no safe V2 spawn in {center:?}"));
            let floor = BlockPos::new(
                spawn.x.floor() as i32,
                spawn.y.floor() as i32 - 1,
                spawn.z.floor() as i32,
            );
            assert_eq!(floor.chunk_pos(), center);
            assert!(matches!(
                chunk
                    .block_at_y(
                        floor.x - center.min_block_x(),
                        floor.y,
                        floor.z - center.min_block_z()
                    )
                    .0,
                GRASS_BLOCK | PODZOL
            ));
        }
    }

    #[test]
    fn mclone_overworld_v3_selects_a_stable_loaded_quality_spawn() {
        for (seed, expected_center) in [
            (12_345, ChunkPos::new(-96, -128)),
            (-98_765, ChunkPos::new(128, 32)),
            (8_675_309, ChunkPos::new(240, 112)),
        ] {
            let profile = WorldGenerationProfile::McloneOverworldV3;
            let center = initial_spawn_center_for_profile(seed, profile);
            assert_eq!(center, expected_center);
            assert_eq!(center, initial_spawn_center_for_profile(seed, profile));
            assert!(center.x.abs() <= 512 && center.z.abs() <= 512, "{center:?}");

            let terrain = McloneOverworldV3TerrainPlan::new(seed);
            let selected = terrain
                .query_point(center.min_block_x(), center.min_block_z())
                .sample;
            assert!(!selected.is_water(), "seed {seed} selected water");
            assert!(selected.land_weight >= 0.62, "seed {seed}: {selected:?}");
            assert!(selected.openness >= 0.40, "seed {seed}: {selected:?}");

            let chunk = generate_mclone_overworld_v3_chunk(seed, center.x, center.z);
            let spawn = find_safe_surface_spawn_for_loaded_profile(
                seed,
                profile,
                center,
                |pos| {
                    (pos.chunk_pos() == center).then(|| {
                        chunk
                            .block_at_y(
                                pos.x - center.min_block_x(),
                                pos.y,
                                pos.z - center.min_block_z(),
                            )
                            .0
                    })
                },
                |pos| pos == center,
            )
            .unwrap_or_else(|| panic!("seed {seed} had no safe V3 spawn in {center:?}"));
            assert_eq!(
                BlockPos::new(
                    spawn.x.floor() as i32,
                    spawn.y.floor() as i32 - 1,
                    spawn.z.floor() as i32,
                )
                .chunk_pos(),
                center
            );
        }
    }

    #[test]
    fn reproduced_web_menu_seed_selects_the_expected_dry_spawn() {
        let seed = 553_534_047_293_117_028;
        let profile = WorldGenerationProfile::McloneOverworldV1;
        let center = initial_spawn_center_for_profile(seed, profile);
        assert_eq!(center, ChunkPos::new(-48, 20));

        let chunk = generate_mclone_overworld_chunk(seed, center.x, center.z);
        let spawn = find_safe_surface_spawn_for_loaded_profile(
            seed,
            profile,
            center,
            |pos| {
                (pos.chunk_pos() == center).then(|| {
                    chunk
                        .block_at_y(
                            pos.x - center.min_block_x(),
                            pos.y,
                            pos.z - center.min_block_z(),
                        )
                        .0
                })
            },
            |pos| pos == center,
        )
        .expect("reproduced menu seed has a safe spawn");
        let feet = BlockPos::new(
            spawn.x.floor() as i32,
            spawn.y.floor() as i32,
            spawn.z.floor() as i32,
        );
        let local_x = feet.x - center.min_block_x();
        let local_z = feet.z - center.min_block_z();

        assert_eq!(feet.y, 73);
        assert_eq!(
            chunk.block_at_y(local_x, feet.y - 1, local_z).0,
            GRASS_BLOCK
        );
        assert_eq!(chunk.block_at_y(local_x, feet.y, local_z).0, AIR);
        assert_eq!(chunk.block_at_y(local_x, feet.y + 1, local_z).0, AIR);
    }

    #[test]
    fn periodic_mclone_spawn_uses_the_periodic_surface_contract() {
        let seed = -98_765;
        let profile = WorldGenerationProfile::McloneOverworldV1;
        let topology = HorizontalTopology::cylinder_x(0, 384);
        let sampling_topology =
            McloneOverworldSamplingTopology::from_horizontal_topology(topology).unwrap();
        let center = initial_spawn_center_for_descriptor(seed, profile, topology);
        assert_eq!(topology.canonicalize_chunk(center), Some(center));

        let chunk = generate_mclone_overworld_chunk_with_topology(
            seed,
            sampling_topology,
            center.x,
            center.z,
        );
        let spawn = find_safe_surface_spawn_for_loaded_descriptor(
            seed,
            profile,
            topology,
            center,
            |pos| {
                (pos.chunk_pos() == center).then(|| {
                    chunk
                        .block_at_y(
                            pos.x - center.min_block_x(),
                            pos.y,
                            pos.z - center.min_block_z(),
                        )
                        .0
                })
            },
            |pos| pos == center,
        )
        .expect("periodic Mclone spawn chunk should contain a dry spawn");

        let floor = BlockPos::new(
            spawn.x.floor() as i32,
            spawn.y.floor() as i32 - 1,
            spawn.z.floor() as i32,
        );
        assert_eq!(floor.chunk_pos(), center);
        assert_eq!(
            chunk
                .block_at_y(
                    floor.x - center.min_block_x(),
                    floor.y,
                    floor.z - center.min_block_z(),
                )
                .0,
            GRASS_BLOCK
        );
    }

    #[test]
    fn alpha_profiles_select_a_safe_loaded_origin_spawn() {
        let seed = 12_345;
        let center = ChunkPos::new(0, 0);
        for profile in [
            WorldGenerationProfile::alpha_v1(false),
            WorldGenerationProfile::alpha_v1(true),
        ] {
            let chunk =
                generate_alpha_chunk(seed, center.x, center.z, profile.alpha_winter().unwrap());
            let spawn = find_safe_surface_spawn_for_loaded_profile(
                seed,
                profile,
                center,
                |pos| {
                    (pos.chunk_pos() == center).then(|| {
                        chunk
                            .block_at_y(
                                pos.x - center.min_block_x(),
                                pos.y,
                                pos.z - center.min_block_z(),
                            )
                            .0
                    })
                },
                |pos| pos == center,
            )
            .unwrap_or_else(|| panic!("{profile:?} had no safe spawn in {center:?}"));
            assert_eq!(
                BlockPos::new(
                    spawn.x.floor() as i32,
                    spawn.y.floor() as i32,
                    spawn.z.floor() as i32,
                )
                .chunk_pos(),
                center
            );
        }
    }

    #[test]
    fn beta_profile_selects_a_safe_loaded_origin_spawn() {
        let seed = 12_345;
        let profile = WorldGenerationProfile::BetaV1;
        let center = initial_spawn_center_for_profile(seed, profile);
        let chunk = generate_beta_chunk(seed, center.x, center.z);
        let spawn = find_safe_surface_spawn_for_loaded_profile(
            seed,
            profile,
            center,
            |pos| {
                (pos.chunk_pos() == center).then(|| {
                    chunk
                        .block_at_y(
                            pos.x - center.min_block_x(),
                            pos.y,
                            pos.z - center.min_block_z(),
                        )
                        .0
                })
            },
            |pos| pos == center,
        )
        .expect("Beta origin should expose a safe loaded spawn");
        assert_eq!(
            BlockPos::new(
                spawn.x.floor() as i32,
                spawn.y.floor() as i32,
                spawn.z.floor() as i32,
            )
            .chunk_pos(),
            center
        );
    }

    #[test]
    fn skips_fluid_and_tree_floor_columns() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(0, 63, 0), STONE);
        blocks.insert(BlockPos::new(0, 64, 0), WATER);
        blocks.insert(BlockPos::new(0, 63, 1), OAK_LEAVES);
        blocks.insert(BlockPos::new(0, 63, 2), GRASS_BLOCK);

        let spawn = find_safe_surface_spawn(
            ChunkPos::new(0, 0),
            |pos| Some(*blocks.get(&pos).unwrap_or(&AIR)),
            plains_biome,
            |_| true,
        )
        .expect("spawn");

        assert_eq!(spawn, Vec3d::new(0.5, 64.0, 2.5));
    }

    #[test]
    fn surface_spawn_does_not_descend_into_caves_under_rejected_surface() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(0, 80, 0), OAK_LEAVES);
        blocks.insert(BlockPos::new(0, 12, 0), STONE);
        blocks.insert(BlockPos::new(0, 63, 1), GRASS_BLOCK);

        let spawn = find_safe_surface_spawn(
            ChunkPos::new(0, 0),
            |pos| Some(*blocks.get(&pos).unwrap_or(&AIR)),
            plains_biome,
            |_| true,
        )
        .expect("spawn");

        assert_eq!(spawn, Vec3d::new(0.5, 64.0, 1.5));
    }

    #[test]
    fn waits_for_spawn_chunk_to_be_ready() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(0, 63, 0), GRASS_BLOCK);

        assert_eq!(
            find_safe_surface_spawn(
                ChunkPos::new(0, 0),
                |pos| Some(*blocks.get(&pos).unwrap_or(&AIR)),
                plains_biome,
                |_| false,
            ),
            None
        );
    }

    #[test]
    fn unloaded_columns_do_not_produce_spawn() {
        assert_eq!(
            find_safe_surface_spawn(ChunkPos::new(0, 0), |_pos| None, plains_biome, |_| true),
            None
        );
    }
}
