use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_worldgen::biome::{BiomeDefinition, OverworldBiomeSource, get_layered_biome_by_id};
use mclone_worldgen::block::{
    GRASS_BLOCK, PODZOL, RawBlockId, has_fluid, is_air_like, material_blocks_motion,
};
use mclone_worldgen::levelgen::{
    beta_biome_id, mclone_overworld_biome_id, mclone_overworld_spawn_chunk,
};
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
    match profile {
        WorldGenerationProfile::Overworld => initial_spawn_center_for_seed(seed),
        WorldGenerationProfile::McloneOverworldV1 => mclone_overworld_spawn_chunk(seed),
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
    let column_order = if matches!(
        profile,
        WorldGenerationProfile::FlatGrassV1
            | WorldGenerationProfile::SmallIslandV1
            | WorldGenerationProfile::McloneOverworldV1
            | WorldGenerationProfile::AlphaV1 { .. }
            | WorldGenerationProfile::BetaV1
            | WorldGenerationProfile::AuthoredOnly { .. }
    ) {
        SpawnColumnOrder::CenterFirst
    } else {
        SpawnColumnOrder::Scan
    };
    let biome_source = OverworldBiomeSource::new(seed, false, false);
    find_safe_surface_spawn_with_column_order(
        center,
        block_at,
        |x, z| match profile {
            WorldGenerationProfile::FlatGrassV1
            | WorldGenerationProfile::SmallIslandV1
            | WorldGenerationProfile::AlphaV1 { .. } => get_layered_biome_by_id(1),
            WorldGenerationProfile::McloneOverworldV1 => {
                get_layered_biome_by_id(mclone_overworld_biome_id(seed, x, z))
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
    mut block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
    mut biome_at: impl FnMut(i32, i32) -> BiomeDefinition,
    mut chunk_ready: impl FnMut(ChunkPos) -> bool,
    column_order: SpawnColumnOrder,
) -> Option<Vec3d> {
    for chunk in initial_spawn_chunks(center) {
        if !chunk_ready(chunk) {
            continue;
        }

        for (x, z) in ordered_chunk_columns(chunk, column_order) {
            let biome = biome_at(x, z);
            if let Some(feet_y) = safe_feet_y_at_column(x, z, biome, &mut block_at) {
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
    biome: BiomeDefinition,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<i32> {
    let top_material = overworld_surface_top_material(biome);
    if !is_valid_spawn_surface_block(top_material) {
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
    use mclone_worldgen::levelgen::generate_mclone_overworld_chunk;
    use mclone_worldgen::levelgen::{generate_alpha_chunk, generate_beta_chunk};

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
