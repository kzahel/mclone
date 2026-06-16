use crate::block::{
    CAVE_AIR, COARSE_DIRT, DIRT, GRASS_BLOCK, LAVA, MYCELIUM, PODZOL, RawBlockId, STONE, WATER,
    is_air_like, material_blocks_motion,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{FeatureWorld, LakeConfiguration};

pub(super) fn place_lake<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: LakeConfiguration,
) -> bool {
    let mut base = origin;
    while base.y > world.min_y() + 5 && world.block_at_world(base).is_some_and(is_air_like) {
        base.y -= 1;
    }

    if base.y <= world.min_y() + 4 {
        return false;
    }
    base.y -= 4;

    let mut carved = [false; 16 * 16 * 8];
    let ellipsoid_count = random.next_int_bound(4) + 4;
    for _ in 0..ellipsoid_count {
        let radius_x = random.next_double() * 6.0 + 3.0;
        let radius_y = random.next_double() * 4.0 + 2.0;
        let radius_z = random.next_double() * 6.0 + 3.0;
        let center_x = random.next_double() * (16.0 - radius_x - 2.0) + 1.0 + radius_x / 2.0;
        let center_y = random.next_double() * (8.0 - radius_y - 4.0) + 2.0 + radius_y / 2.0;
        let center_z = random.next_double() * (16.0 - radius_z - 2.0) + 1.0 + radius_z / 2.0;

        for x in 1..15 {
            for z in 1..15 {
                for y in 1..7 {
                    let normalized_x = (x as f64 - center_x) / (radius_x / 2.0);
                    let normalized_y = (y as f64 - center_y) / (radius_y / 2.0);
                    let normalized_z = (z as f64 - center_z) / (radius_z / 2.0);
                    if normalized_x * normalized_x
                        + normalized_y * normalized_y
                        + normalized_z * normalized_z
                        < 1.0
                    {
                        carved[lake_index(x, z, y)] = true;
                    }
                }
            }
        }
    }

    for x in 0..16 {
        for z in 0..16 {
            for y in 0..8 {
                if !lake_shell_cell(&carved, x, z, y) {
                    continue;
                }

                let pos = BlockPos::new(base.x + x as i32, base.y + y as i32, base.z + z as i32);
                let Some(block_id) = world.block_at_world(pos) else {
                    return false;
                };
                if y >= 4 && is_lake_liquid(block_id) {
                    return false;
                }
                if y < 4 && !lake_material_is_solid(block_id) && block_id != config.state {
                    return false;
                }
            }
        }
    }

    for x in 0..16 {
        for z in 0..16 {
            for y in 0..8 {
                if carved[lake_index(x, z, y)] {
                    let pos =
                        BlockPos::new(base.x + x as i32, base.y + y as i32, base.z + z as i32);
                    let replacement = if y >= 4 { CAVE_AIR } else { config.state };
                    world.set_block_world(pos, replacement);
                }
            }
        }
    }

    for x in 0..16 {
        for z in 0..16 {
            for y in 4..8 {
                if carved[lake_index(x, z, y)] {
                    let below =
                        BlockPos::new(base.x + x as i32, base.y + y as i32 - 1, base.z + z as i32);
                    let sky_pos =
                        BlockPos::new(base.x + x as i32, base.y + y as i32, base.z + z as i32);
                    if world.block_at_world(below).is_some_and(is_lake_dirt)
                        && has_lake_sky_light(world, sky_pos)
                    {
                        let replacement = if world.block_at_world(below) == Some(MYCELIUM) {
                            MYCELIUM
                        } else {
                            GRASS_BLOCK
                        };
                        world.set_block_world(below, replacement);
                    }
                }
            }
        }
    }

    if config.state == LAVA {
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..8 {
                    if lake_shell_cell(&carved, x, z, y) && (y < 4 || random.next_int_bound(2) != 0)
                    {
                        let pos =
                            BlockPos::new(base.x + x as i32, base.y + y as i32, base.z + z as i32);
                        if world
                            .block_at_world(pos)
                            .is_some_and(lake_material_is_solid)
                        {
                            world.set_block_world(pos, STONE);
                        }
                    }
                }
            }
        }
    }

    true
}

fn lake_index(x: usize, z: usize, y: usize) -> usize {
    (x * 16 + z) * 8 + y
}

fn lake_shell_cell(carved: &[bool; 16 * 16 * 8], x: usize, z: usize, y: usize) -> bool {
    !carved[lake_index(x, z, y)]
        && ((x < 15 && carved[lake_index(x + 1, z, y)])
            || (x > 0 && carved[lake_index(x - 1, z, y)])
            || (z < 15 && carved[lake_index(x, z + 1, y)])
            || (z > 0 && carved[lake_index(x, z - 1, y)])
            || (y < 7 && carved[lake_index(x, z, y + 1)])
            || (y > 0 && carved[lake_index(x, z, y - 1)]))
}

fn is_lake_liquid(block_id: RawBlockId) -> bool {
    matches!(block_id, WATER | LAVA)
}

fn lake_material_is_solid(block_id: RawBlockId) -> bool {
    material_blocks_motion(block_id)
}

fn is_lake_dirt(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        DIRT | GRASS_BLOCK | PODZOL | COARSE_DIRT | MYCELIUM
    )
}

fn has_lake_sky_light<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    // Java asks the worldgen light engine for LightLayer.SKY during FEATURES, before the
    // post-feature light status has populated final skylight data.
    world.block_at_world(pos).is_some()
}
