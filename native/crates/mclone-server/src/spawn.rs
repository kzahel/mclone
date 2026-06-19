use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_worldgen::block::{
    BIRCH_LEAVES, BIRCH_LOG, OAK_LEAVES, OAK_LOG, RawBlockId, SPRUCE_LEAVES, SPRUCE_LOG, has_fluid,
    material_blocks_motion,
};

use crate::game_mode::JAVA_OVERWORLD_MAX_BUILD_HEIGHT;

const JAVA_OVERWORLD_MIN_BUILD_HEIGHT: i32 = 0;
const JAVA_DEFAULT_SPAWN_RADIUS: i32 = 10;

pub(crate) fn find_safe_surface_spawn(
    center: ChunkPos,
    mut block_at: impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<Vec3d> {
    let center_x = center.middle_block_x();
    let center_z = center.middle_block_z();
    for (x, z) in spawn_columns(center_x, center_z, JAVA_DEFAULT_SPAWN_RADIUS) {
        if let Some(feet_y) = safe_feet_y_at_column(x, z, &mut block_at) {
            return Some(Vec3d::new(
                x as f64 + 0.5,
                f64::from(feet_y),
                z as f64 + 0.5,
            ));
        }
    }
    None
}

fn safe_feet_y_at_column(
    x: i32,
    z: i32,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<i32> {
    for feet_y in (JAVA_OVERWORLD_MIN_BUILD_HEIGHT + 1..JAVA_OVERWORLD_MAX_BUILD_HEIGHT - 1).rev() {
        let floor = block_at(BlockPos::new(x, feet_y - 1, z))?;
        let feet = block_at(BlockPos::new(x, feet_y, z))?;
        let head = block_at(BlockPos::new(x, feet_y + 1, z))?;
        if is_spawn_floor(floor) && is_spawn_space(feet) && is_spawn_space(head) {
            return Some(feet_y);
        }
    }
    None
}

fn spawn_columns(center_x: i32, center_z: i32, radius: i32) -> impl Iterator<Item = (i32, i32)> {
    (0..=radius).flat_map(move |ring| {
        (-ring..=ring).flat_map(move |dz| {
            (-ring..=ring).filter_map(move |dx| {
                (dx.abs().max(dz.abs()) == ring).then_some((center_x + dx, center_z + dz))
            })
        })
    })
}

fn is_spawn_floor(block: RawBlockId) -> bool {
    material_blocks_motion(block) && !is_tree_block(block)
}

fn is_spawn_space(block: RawBlockId) -> bool {
    !material_blocks_motion(block) && !has_fluid(block)
}

fn is_tree_block(block: RawBlockId) -> bool {
    matches!(
        block,
        OAK_LOG | OAK_LEAVES | BIRCH_LOG | BIRCH_LEAVES | SPRUCE_LOG | SPRUCE_LEAVES
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mclone_worldgen::block::{AIR, GRASS_BLOCK, OAK_LEAVES, STONE, WATER};

    use super::*;

    #[test]
    fn finds_center_surface_spawn_with_two_clear_blocks() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(8, 63, 8), GRASS_BLOCK);

        let spawn = find_safe_surface_spawn(ChunkPos::new(0, 0), |pos| {
            Some(*blocks.get(&pos).unwrap_or(&AIR))
        })
        .expect("spawn");

        assert_eq!(spawn, Vec3d::new(8.5, 64.0, 8.5));
    }

    #[test]
    fn skips_fluid_and_tree_floor_columns() {
        let mut blocks = BTreeMap::new();
        blocks.insert(BlockPos::new(8, 63, 8), STONE);
        blocks.insert(BlockPos::new(8, 64, 8), WATER);
        blocks.insert(BlockPos::new(7, 63, 7), OAK_LEAVES);
        blocks.insert(BlockPos::new(7, 63, 8), GRASS_BLOCK);

        let spawn = find_safe_surface_spawn(ChunkPos::new(0, 0), |pos| {
            Some(*blocks.get(&pos).unwrap_or(&AIR))
        })
        .expect("spawn");

        assert_eq!(spawn, Vec3d::new(7.5, 64.0, 8.5));
    }

    #[test]
    fn unloaded_columns_do_not_produce_spawn() {
        assert_eq!(
            find_safe_surface_spawn(ChunkPos::new(0, 0), |_pos| None),
            None
        );
    }
}
