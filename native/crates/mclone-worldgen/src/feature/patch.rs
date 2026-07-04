use crate::block::{
    CACTUS, DANDELION, DEAD_BUSH, DIRT, FERN, GLOW_LICHEN, GRASS, GRASS_BLOCK, ICE,
    LARGE_FERN_LOWER, LARGE_FERN_UPPER, LILY_PAD, MYCELIUM, PODZOL, POPPY, RED_SAND, RawBlockId,
    SAND, SUGAR_CANE, SWEET_BERRY_BUSH, TERRACOTTA, is_air_like, is_lava, is_water,
    material_blocks_motion,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{FeatureWorld, RandomPatchConfiguration, SimpleBlockConfiguration, project_to_surface};

pub(super) fn place_simple_block<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    origin: BlockPos,
    config: SimpleBlockConfiguration,
) -> bool {
    let below = BlockPos::new(origin.x, origin.y - 1, origin.z);
    let above = BlockPos::new(origin.x, origin.y + 1, origin.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    let Some(block_above) = world.block_at_world(above) else {
        return false;
    };

    if !matches_allowed(config.place_on, block_below)
        || !matches_allowed(config.place_in, current)
        || !matches_allowed(config.place_under, block_above)
        || !can_survive_simple_plant(config.to_place, current, block_below)
    {
        return false;
    }

    world.set_block_world(origin, config.to_place)
}

pub(super) fn place_random_patch<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: RandomPatchConfiguration,
) -> bool {
    let state = select_patch_state(random, config);
    let projected = if config.project {
        project_to_surface(world, origin).unwrap_or(origin)
    } else {
        origin
    };
    let mut placed = 0;

    for _ in 0..config.tries {
        let pos = BlockPos::new(
            projected.x + random.next_int_bound(config.xspread + 1)
                - random.next_int_bound(config.xspread + 1),
            projected.y + random.next_int_bound(config.yspread + 1)
                - random.next_int_bound(config.yspread + 1),
            projected.z + random.next_int_bound(config.zspread + 1)
                - random.next_int_bound(config.zspread + 1),
        );
        let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
        let Some(current) = world.block_at_world(pos) else {
            continue;
        };
        let Some(block_below) = world.block_at_world(below) else {
            continue;
        };
        let can_replace =
            is_air_like(current) || (config.can_replace && is_replaceable_plant(current));
        if can_replace
            && matches_allowed(config.place_on, block_below)
            && (!config.need_water || has_horizontal_water_adjacent_to(world, below))
            && can_survive_patch_plant(world, state, pos, current, block_below)
        {
            let did_place = if let Some(height_provider) = config.column_height {
                place_column(world, pos, state, height_provider.sample(random))
            } else if config.double_plant {
                let lower = world.set_block_world(pos, LARGE_FERN_LOWER);
                let upper =
                    world.set_block_world(BlockPos::new(pos.x, pos.y + 1, pos.z), LARGE_FERN_UPPER);
                lower && upper
            } else {
                world.set_block_world(pos, state)
            };
            if did_place {
                placed += 1;
            }
        }
    }

    placed > 0
}

fn place_column<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    state: RawBlockId,
    height: i32,
) -> bool {
    let mut placed = false;
    for y_offset in 0..height.max(0) {
        placed |= world.set_block_world(
            BlockPos::new(origin.x, origin.y + y_offset, origin.z),
            state,
        );
    }
    placed
}

pub(super) fn place_flower<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: RandomPatchConfiguration,
) -> bool {
    let state = select_patch_state(random, config);
    let mut placed = 0;

    for _ in 0..config.tries {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(config.xspread)
                - random.next_int_bound(config.xspread),
            origin.y + random.next_int_bound(config.yspread)
                - random.next_int_bound(config.yspread),
            origin.z + random.next_int_bound(config.zspread)
                - random.next_int_bound(config.zspread),
        );
        let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
        let Some(current) = world.block_at_world(pos) else {
            continue;
        };
        let Some(block_below) = world.block_at_world(below) else {
            continue;
        };
        if is_air_like(current)
            && can_survive_simple_plant(state, current, block_below)
            && world.set_block_world(pos, state)
        {
            placed += 1;
        }
    }

    placed > 0
}

fn select_patch_state(
    random: &mut impl RandomSource,
    config: RandomPatchConfiguration,
) -> RawBlockId {
    if config.weighted_states.is_empty() {
        return config.state;
    }

    let total_weight = config
        .weighted_states
        .iter()
        .map(|entry| entry.weight)
        .sum::<i32>();
    let mut selected_weight = random.next_int_bound(total_weight);
    for entry in config.weighted_states {
        selected_weight -= entry.weight;
        if selected_weight < 0 {
            return entry.state;
        }
    }

    config.state
}

fn matches_allowed(allowed: &[RawBlockId], block_id: RawBlockId) -> bool {
    allowed.is_empty() || allowed.contains(&block_id)
}

fn can_survive_simple_plant(
    block_id: RawBlockId,
    current: RawBlockId,
    block_below: RawBlockId,
) -> bool {
    is_air_like(current)
        && match block_id {
            GRASS | FERN | DANDELION | POPPY => {
                matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
            }
            LARGE_FERN_LOWER | LARGE_FERN_UPPER => {
                matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
            }
            SWEET_BERRY_BUSH => matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM),
            DEAD_BUSH => matches!(
                block_below,
                SAND | RED_SAND | TERRACOTTA | DIRT | GRASS_BLOCK | PODZOL
            ),
            _ => false,
        }
}

fn can_survive_patch_plant<W: FeatureWorld>(
    world: &mut W,
    block_id: RawBlockId,
    pos: BlockPos,
    current: RawBlockId,
    block_below: RawBlockId,
) -> bool {
    if !is_air_like(current) {
        return false;
    }

    match block_id {
        CACTUS => {
            matches!(block_below, CACTUS | SAND | RED_SAND)
                && !block_above_is_liquid(world, pos)
                && !has_horizontal_motion_blocker_or_lava(world, pos)
        }
        SUGAR_CANE => {
            block_below == SUGAR_CANE
                || (matches!(block_below, GRASS_BLOCK | DIRT | SAND | RED_SAND)
                    && has_horizontal_water_adjacent_to(
                        world,
                        BlockPos::new(pos.x, pos.y - 1, pos.z),
                    ))
        }
        LILY_PAD => is_water(block_below) || block_below == ICE,
        _ => can_survive_simple_plant(block_id, current, block_below),
    }
}

fn block_above_is_liquid<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    world
        .block_at_world(BlockPos::new(pos.x, pos.y + 1, pos.z))
        .is_some_and(|block_id| is_water(block_id) || is_lava(block_id))
}

fn has_horizontal_motion_blocker_or_lava<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    horizontal_neighbor_blocks(world, pos)
        .into_iter()
        .any(|block_id| {
            block_id.is_some_and(|block_id| material_blocks_motion(block_id) || is_lava(block_id))
        })
}

fn has_horizontal_water_adjacent_to<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    horizontal_neighbor_blocks(world, pos)
        .into_iter()
        .any(|block_id| block_id.is_some_and(is_water))
}

fn horizontal_neighbor_blocks<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
) -> [Option<RawBlockId>; 4] {
    [
        world.block_at_world(BlockPos::new(pos.x - 1, pos.y, pos.z)),
        world.block_at_world(BlockPos::new(pos.x + 1, pos.y, pos.z)),
        world.block_at_world(BlockPos::new(pos.x, pos.y, pos.z - 1)),
        world.block_at_world(BlockPos::new(pos.x, pos.y, pos.z + 1)),
    ]
}

fn is_replaceable_plant(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
    )
}
