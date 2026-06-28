use crate::block::{RED_SAND, RED_SANDSTONE, RawBlockId, SAND, SANDSTONE, is_air_like, is_water};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{DiskConfiguration, FeatureWorld};

pub(super) fn place_disk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: DiskConfiguration,
) -> bool {
    if !world.block_at_world(origin).is_some_and(is_water) {
        return false;
    }

    let radius = config.radius.sample(random);
    let max_y = origin.y + config.half_height;
    let min_y_exclusive = origin.y - config.half_height - 1;
    let falling_support = falling_support_block(config.state);
    let mut placed = false;

    for x in origin.x - radius..=origin.x + radius {
        for z in origin.z - radius..=origin.z + radius {
            let dx = x - origin.x;
            let dz = z - origin.z;
            if dx * dx + dz * dz > radius * radius {
                continue;
            }

            let mut replaced_above = false;
            for y in (min_y_exclusive..=max_y).rev() {
                let pos = BlockPos::new(x, y, z);
                let Some(current) = world.block_at_world(pos) else {
                    replaced_above = false;
                    continue;
                };

                let mut replaced_current = false;
                if y > min_y_exclusive && config.targets.contains(&current) {
                    world.set_block_world(pos, config.state);
                    placed = true;
                    replaced_current = true;
                }

                if let Some(support) = falling_support {
                    if replaced_above && is_air_like(current) {
                        world.set_block_world(BlockPos::new(x, y + 1, z), support);
                    }
                }

                replaced_above = replaced_current;
            }
        }
    }

    placed
}

fn falling_support_block(block_id: RawBlockId) -> Option<RawBlockId> {
    match block_id {
        SAND => Some(SANDSTONE),
        RED_SAND => Some(RED_SANDSTONE),
        _ => None,
    }
}
