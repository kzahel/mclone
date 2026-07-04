use std::sync::OnceLock;

use crate::block::{
    ALLIUM, AZURE_BLUET, BLUE_ORCHID, BROWN_MUSHROOM, CACTUS, CORNFLOWER, DANDELION, DEAD_BUSH,
    DIRT, FERN, GLOW_LICHEN, GRASS, GRASS_BLOCK, ICE, LARGE_FERN_LOWER, LARGE_FERN_UPPER,
    LILAC_LOWER, LILAC_UPPER, LILY_OF_THE_VALLEY, LILY_PAD, MYCELIUM, ORANGE_TULIP, OXEYE_DAISY,
    PEONY_LOWER, PEONY_UPPER, PINK_TULIP, PODZOL, POPPY, PUMPKIN, RED_MUSHROOM, RED_SAND,
    RED_TULIP, ROSE_BUSH_LOWER, ROSE_BUSH_UPPER, RawBlockId, SAND, SUGAR_CANE, SUNFLOWER_LOWER,
    SUNFLOWER_UPPER, SWEET_BERRY_BUSH, TERRACOTTA, WHITE_TULIP, block_light_emission,
    block_light_opacity, is_air_like, is_lava, is_water, material_blocks_motion,
};
use crate::noise::PerlinSimplexNoise;
use crate::placement::BlockPos;
use crate::prng::{RandomSource, WorldgenRandom};

use super::{
    FeatureWorld, RandomPatchConfiguration, RandomPatchStateProvider, SimpleBlockConfiguration,
    project_to_surface,
};

const FOREST_FLOWERS: [RawBlockId; 11] = [
    DANDELION,
    POPPY,
    ALLIUM,
    AZURE_BLUET,
    RED_TULIP,
    ORANGE_TULIP,
    WHITE_TULIP,
    PINK_TULIP,
    OXEYE_DAISY,
    CORNFLOWER,
    LILY_OF_THE_VALLEY,
];

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
    let state = select_patch_state(random, config, origin);
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
                place_double_plant(world, pos, state)
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

fn place_double_plant<W: FeatureWorld>(world: &mut W, origin: BlockPos, state: RawBlockId) -> bool {
    let Some((lower_state, upper_state)) = double_plant_halves(state) else {
        return false;
    };
    let lower = world.set_block_world(origin, lower_state);
    let upper = world.set_block_world(BlockPos::new(origin.x, origin.y + 1, origin.z), upper_state);
    lower && upper
}

pub(super) fn place_flower<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: RandomPatchConfiguration,
) -> bool {
    let state = select_patch_state(random, config, origin);
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
    origin: BlockPos,
) -> RawBlockId {
    match config.state_provider {
        RandomPatchStateProvider::Simple => config.state,
        RandomPatchStateProvider::Weighted => select_weighted_patch_state(random, config),
        RandomPatchStateProvider::ForestFlower => forest_flower_state(origin),
    }
}

fn select_weighted_patch_state(
    random: &mut impl RandomSource,
    config: RandomPatchConfiguration,
) -> RawBlockId {
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

fn forest_flower_state(origin: BlockPos) -> RawBlockId {
    let noise = biome_info_noise().get_value(origin.x as f64 / 48.0, origin.z as f64 / 48.0, false);
    let value = ((1.0 + noise) / 2.0).clamp(0.0, 0.9999);
    FOREST_FLOWERS[(value * FOREST_FLOWERS.len() as f64) as usize]
}

fn biome_info_noise() -> &'static PerlinSimplexNoise {
    static NOISE: OnceLock<PerlinSimplexNoise> = OnceLock::new();
    NOISE.get_or_init(|| {
        let mut random = WorldgenRandom::new(2345);
        PerlinSimplexNoise::from_octaves(&mut random, &[0])
    })
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
            GRASS | FERN => matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM),
            flower if is_small_flower(flower) => {
                matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
            }
            block if double_plant_halves(block).is_some() => {
                matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
            }
            mushroom if is_small_mushroom(mushroom) => false,
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
        PUMPKIN => block_below == GRASS_BLOCK,
        mushroom if is_small_mushroom(mushroom) => {
            can_survive_small_mushroom(world, pos, block_below)
        }
        _ => can_survive_simple_plant(block_id, current, block_below),
    }
}

fn can_survive_small_mushroom<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    block_below: RawBlockId,
) -> bool {
    is_mushroom_grow_block(block_below)
        || (generated_raw_brightness_at(world, pos).is_some_and(|brightness| brightness < 13)
            && material_blocks_motion(block_below))
}

fn generated_raw_brightness_at<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> Option<i32> {
    let current = world.block_at_world(pos)?;
    Some((block_light_emission(current) as i32).max(generated_sky_light_at(world, pos)?))
}

fn generated_sky_light_at<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> Option<i32> {
    let min_y = world.min_y();
    let max_y = min_y + world.height();
    if !(min_y..max_y).contains(&pos.y) {
        return None;
    }

    if world
        .world_surface_height_at(pos.x, pos.z)
        .is_some_and(|surface_y| pos.y >= surface_y)
    {
        return Some(15);
    }

    let mut light = 15;
    for y in pos.y + 1..max_y {
        let opacity =
            block_light_opacity(world.block_at_world(BlockPos::new(pos.x, y, pos.z))?) as i32;
        light -= opacity;
        if light <= 0 {
            return Some(0);
        }
    }
    Some(light)
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
    matches!(block_id, GRASS | FERN | DEAD_BUSH | GLOW_LICHEN)
        || is_small_flower(block_id)
        || is_small_mushroom(block_id)
        || double_plant_halves(block_id).is_some()
}

fn is_small_flower(block_id: RawBlockId) -> bool {
    FOREST_FLOWERS.contains(&block_id) || block_id == BLUE_ORCHID
}

fn is_small_mushroom(block_id: RawBlockId) -> bool {
    matches!(block_id, BROWN_MUSHROOM | RED_MUSHROOM)
}

fn is_mushroom_grow_block(block_id: RawBlockId) -> bool {
    matches!(block_id, MYCELIUM | PODZOL)
}

fn double_plant_halves(block_id: RawBlockId) -> Option<(RawBlockId, RawBlockId)> {
    match block_id {
        LARGE_FERN_LOWER | LARGE_FERN_UPPER => Some((LARGE_FERN_LOWER, LARGE_FERN_UPPER)),
        LILAC_LOWER | LILAC_UPPER => Some((LILAC_LOWER, LILAC_UPPER)),
        ROSE_BUSH_LOWER | ROSE_BUSH_UPPER => Some((ROSE_BUSH_LOWER, ROSE_BUSH_UPPER)),
        PEONY_LOWER | PEONY_UPPER => Some((PEONY_LOWER, PEONY_UPPER)),
        SUNFLOWER_LOWER | SUNFLOWER_UPPER => Some((SUNFLOWER_LOWER, SUNFLOWER_UPPER)),
        _ => None,
    }
}
