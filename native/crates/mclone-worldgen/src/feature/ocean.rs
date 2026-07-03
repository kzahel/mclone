use crate::block::{
    KELP, KELP_PLANT, MAGMA_BLOCK, RawBlockId, SEAGRASS, TALL_SEAGRASS_LOWER, TALL_SEAGRASS_UPPER,
    is_water, material_blocks_motion,
};
use crate::placement::{BlockPos, HeightmapType};
use crate::prng::RandomSource;

use super::{FeatureWorld, SeagrassConfiguration};

pub(super) fn place_seagrass<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: SeagrassConfiguration,
) -> bool {
    let x = origin.x + random.next_int_bound(8) - random.next_int_bound(8);
    let z = origin.z + random.next_int_bound(8) - random.next_int_bound(8);
    let Some(y) = world.height_at(HeightmapType::OceanFloor, x, z) else {
        return false;
    };
    let pos = BlockPos::new(x, y, z);
    if !is_water_at(world, pos) || !can_survive_water_plant_at(world, pos) {
        return false;
    }

    if random.next_float() < config.tall_probability {
        let upper = BlockPos::new(pos.x, pos.y + 1, pos.z);
        if is_water_at(world, upper) {
            let lower_placed = world.set_block_world(pos, TALL_SEAGRASS_LOWER);
            let upper_placed = world.set_block_world(upper, TALL_SEAGRASS_UPPER);
            return lower_placed || upper_placed;
        }
        return false;
    }

    world.set_block_world(pos, SEAGRASS)
}

pub(super) fn place_kelp<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
) -> bool {
    let Some(y) = world.height_at(HeightmapType::OceanFloor, origin.x, origin.z) else {
        return false;
    };
    let mut pos = BlockPos::new(origin.x, y, origin.z);
    if !is_water_at(world, pos) {
        return false;
    }

    let height = 1 + random.next_int_bound(10);
    let mut placed_heads = 0;
    for y_offset in 0..=height {
        let above = BlockPos::new(pos.x, pos.y + 1, pos.z);
        if is_water_at(world, pos) && is_water_at(world, above) && can_survive_kelp_at(world, pos) {
            if y_offset == height {
                if world.set_block_world(pos, KELP) {
                    placed_heads += 1;
                }
            } else {
                world.set_block_world(pos, KELP_PLANT);
            }
        } else if y_offset > 0 {
            let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
            let below_below = BlockPos::new(pos.x, pos.y - 2, pos.z);
            if can_survive_kelp_at(world, below)
                && !world
                    .block_at_world(below_below)
                    .is_some_and(|block| block == KELP)
                && world.set_block_world(below, KELP)
            {
                placed_heads += 1;
            }
            break;
        }

        pos = above;
    }

    placed_heads > 0
}

fn is_water_at<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    world.block_at_world(pos).is_some_and(is_water)
}

fn can_survive_water_plant_at<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    world
        .block_at_world(below)
        .is_some_and(|block| block != MAGMA_BLOCK && material_blocks_motion(block))
}

fn can_survive_kelp_at<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    world.block_at_world(below).is_some_and(can_attach_kelp_to)
}

fn can_attach_kelp_to(block: RawBlockId) -> bool {
    block != MAGMA_BLOCK && (material_blocks_motion(block) || matches!(block, KELP | KELP_PLANT))
}
