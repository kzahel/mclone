use crate::block::{
    AIR, ANDESITE, CAVE_AIR, DEEPSLATE, DIORITE, DIRT, DRIPSTONE_BLOCK, GRANITE, POINTED_DRIPSTONE,
    RawBlockId, STONE, TUFF, is_lava, is_water,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{
    Direction, DripstoneClusterConfiguration, FeatureWorld, SmallDripstoneConfiguration, offset_pos,
};

const ALL_DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
const HORIZONTAL_DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScannedColumn {
    floor: Option<i32>,
    ceiling: Option<i32>,
}

impl ScannedColumn {
    const fn with_floor(self, floor: Option<i32>) -> Self {
        Self {
            floor,
            ceiling: self.ceiling,
        }
    }

    const fn height(self) -> Option<i32> {
        match (self.floor, self.ceiling) {
            (Some(floor), Some(ceiling)) => Some(ceiling - floor - 1),
            _ => None,
        }
    }
}

pub(super) fn place_dripstone_cluster<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: DripstoneClusterConfiguration,
) -> bool {
    if !block_matches(world, origin, is_empty_or_water) {
        return false;
    }

    let height = config.height.sample(random);
    let wetness = config.wetness.sample(random);
    let density = config.density.sample(random);
    let radius_x = config.radius.sample(random);
    let radius_z = config.radius.sample(random);

    for dx in -radius_x..=radius_x {
        for dz in -radius_z..=radius_z {
            let chance = get_chance_of_stalagmite_or_stalactite(radius_x, radius_z, dx, dz, config);
            let column_origin = BlockPos::new(origin.x + dx, origin.y, origin.z + dz);
            place_cluster_column(
                world,
                random,
                column_origin,
                dx,
                dz,
                wetness,
                chance,
                height,
                density,
                config,
            );
        }
    }

    true
}

#[allow(clippy::too_many_arguments)]
fn place_cluster_column<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    dx: i32,
    dz: i32,
    wetness: f32,
    chance: f64,
    height: i32,
    density: f32,
    config: DripstoneClusterConfiguration,
) {
    let Some(column) = scan_column(world, origin, config.floor_to_ceiling_search_range) else {
        return;
    };

    if column.ceiling.is_none() && column.floor.is_none() {
        return;
    }

    let place_pool = random.next_float() < wetness;
    let column = if place_pool
        && column
            .floor
            .is_some_and(|floor| can_place_pool(world, at_y(origin, floor)))
    {
        let floor = column.floor.expect("floor checked by is_some_and");
        world.set_block_world(at_y(origin, floor), crate::block::WATER);
        column.with_floor(Some(floor - 1))
    } else {
        column
    };

    let floor = column.floor;
    let place_stalactite = random.next_double() < chance;
    let stalactite_height = if let Some(ceiling) = column.ceiling {
        if place_stalactite && !block_matches(world, at_y(origin, ceiling), is_lava) {
            let thickness = config.dripstone_block_layer_thickness.sample(random);
            replace_blocks_with_dripstone_blocks(
                world,
                at_y(origin, ceiling),
                thickness,
                Direction::Up,
            );
            let max_height = if let Some(floor) = floor {
                height.min(ceiling - floor)
            } else {
                height
            };
            get_dripstone_height(random, dx, dz, density, max_height, config)
        } else {
            0
        }
    } else {
        0
    };

    let place_stalagmite = random.next_double() < chance;
    let stalagmite_height = if let Some(floor_y) = floor {
        if place_stalagmite && !block_matches(world, at_y(origin, floor_y), is_lava) {
            let thickness = config.dripstone_block_layer_thickness.sample(random);
            replace_blocks_with_dripstone_blocks(
                world,
                at_y(origin, floor_y),
                thickness,
                Direction::Down,
            );
            (stalactite_height
                + random_between_inclusive(
                    random,
                    -config.max_stalagmite_stalactite_height_diff,
                    config.max_stalagmite_stalactite_height_diff,
                ))
            .max(0)
        } else {
            0
        }
    } else {
        0
    };

    let (stalactite_height, stalagmite_height) =
        maybe_split_overlapping_dripstones(random, column, stalactite_height, stalagmite_height);
    let merge = random.next_boolean()
        && stalactite_height > 0
        && stalagmite_height > 0
        && column
            .height()
            .is_some_and(|h| stalactite_height + stalagmite_height == h);

    if let Some(ceiling) = column.ceiling {
        grow_pointed_dripstone(
            world,
            at_y(origin, ceiling - 1),
            Direction::Down,
            stalactite_height,
            merge,
        );
    }

    if let Some(floor_y) = floor {
        grow_pointed_dripstone(
            world,
            at_y(origin, floor_y + 1),
            Direction::Up,
            stalagmite_height,
            merge,
        );
    }
}

fn maybe_split_overlapping_dripstones(
    random: &mut impl RandomSource,
    column: ScannedColumn,
    stalactite_height: i32,
    stalagmite_height: i32,
) -> (i32, i32) {
    let (Some(ceiling), Some(floor)) = (column.ceiling, column.floor) else {
        return (stalactite_height, stalagmite_height);
    };

    if ceiling - stalactite_height > floor + stalagmite_height {
        return (stalactite_height, stalagmite_height);
    }

    let min_tip_y = (ceiling - stalactite_height).max(floor + 1);
    let max_tip_y = (floor + stalagmite_height).min(ceiling - 1);
    let split_tip_y = random_between_inclusive(random, min_tip_y, max_tip_y + 1);
    let upper_tip_y = split_tip_y - 1;
    (ceiling - split_tip_y, upper_tip_y - floor)
}

pub(super) fn place_small_dripstone<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: SmallDripstoneConfiguration,
) -> bool {
    if !block_matches(world, origin, is_empty_or_water) {
        return false;
    }

    let placements = random_between_inclusive(random, 1, config.max_placements);
    let mut placed = false;
    for _ in 0..placements {
        let pos = random_offset(random, origin, config.max_offset_from_origin);
        if search_and_try_to_place_dripstone(world, random, pos, config) {
            placed = true;
        }
    }
    placed
}

fn search_and_try_to_place_dripstone<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: SmallDripstoneConfiguration,
) -> bool {
    let search_direction = random_direction(random);
    let primary_direction = if random.next_boolean() {
        Direction::Up
    } else {
        Direction::Down
    };
    let mut pos = origin;

    for _ in 0..config.empty_space_search_radius {
        if !block_matches(world, pos, is_empty_or_water) {
            return false;
        }

        if try_to_place_small_dripstone(world, random, pos, primary_direction, config) {
            return true;
        }

        if try_to_place_small_dripstone(world, random, pos, primary_direction.opposite(), config) {
            return true;
        }

        pos = offset_pos(pos, search_direction);
    }

    false
}

fn try_to_place_small_dripstone<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    pos: BlockPos,
    direction: Direction,
    config: SmallDripstoneConfiguration,
) -> bool {
    if !block_matches(world, pos, is_empty_or_water) {
        return false;
    }

    let base_pos = offset_pos(pos, direction.opposite());
    if !block_matches(world, base_pos, is_dripstone_base) {
        return false;
    }

    create_patch_of_dripstone_blocks(world, random, base_pos);
    let height = if random.next_float() < config.chance_of_taller_dripstone
        && block_matches(world, offset_pos(pos, direction), is_empty_or_water)
    {
        2
    } else {
        1
    };
    grow_pointed_dripstone(world, pos, direction, height, false);
    true
}

fn create_patch_of_dripstone_blocks<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
) {
    place_dripstone_block_if_possible(world, origin);

    for direction in HORIZONTAL_DIRECTIONS {
        if random.next_float() < 0.3 {
            continue;
        }

        let first = offset_pos(origin, direction);
        place_dripstone_block_if_possible(world, first);

        if !random.next_boolean() {
            let second = offset_pos(first, random_direction(random));
            place_dripstone_block_if_possible(world, second);

            if !random.next_boolean() {
                let third = offset_pos(second, random_direction(random));
                place_dripstone_block_if_possible(world, third);
            }
        }
    }
}

fn scan_column<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    search_range: i32,
) -> Option<ScannedColumn> {
    if !block_matches(world, origin, is_empty_or_water) {
        return None;
    }

    Some(ScannedColumn {
        floor: scan_direction(world, origin, search_range, Direction::Down),
        ceiling: scan_direction(world, origin, search_range, Direction::Up),
    })
}

fn scan_direction<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    search_range: i32,
    direction: Direction,
) -> Option<i32> {
    let mut pos = origin;
    for _ in 1..search_range {
        if !block_matches(world, pos, is_empty_or_water) {
            break;
        }
        pos = offset_pos(pos, direction);
    }

    if block_matches(world, pos, is_dripstone_base_or_lava) {
        Some(pos.y)
    } else {
        None
    }
}

fn get_dripstone_height(
    random: &mut impl RandomSource,
    dx: i32,
    dz: i32,
    density: f32,
    max_height: i32,
    config: DripstoneClusterConfiguration,
) -> i32 {
    if random.next_float() > density {
        return 0;
    }

    let distance = dx.abs() + dz.abs();
    let bias = clamped_map(
        distance as f64,
        0.0,
        config.max_distance_from_center_affecting_height_bias as f64,
        max_height as f64 / 2.0,
        0.0,
    ) as f32;
    random_between_biased(
        random,
        0.0,
        max_height as f32,
        bias,
        config.height_deviation as f32,
    ) as i32
}

fn can_place_pool<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    if block_matches(world, pos, |block| {
        is_water(block) || block == DRIPSTONE_BLOCK || block == POINTED_DRIPSTONE
    }) {
        return false;
    }

    for direction in HORIZONTAL_DIRECTIONS {
        if !block_matches(world, offset_pos(pos, direction), can_be_adjacent_to_water) {
            return false;
        }
    }

    block_matches(
        world,
        offset_pos(pos, Direction::Down),
        can_be_adjacent_to_water,
    )
}

fn can_be_adjacent_to_water(block: RawBlockId) -> bool {
    is_base_stone_overworld(block) || is_water(block)
}

fn replace_blocks_with_dripstone_blocks<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    thickness: i32,
    direction: Direction,
) {
    let mut pos = origin;
    for _ in 0..thickness {
        if !place_dripstone_block_if_possible(world, pos) {
            return;
        }
        pos = offset_pos(pos, direction);
    }
}

fn grow_pointed_dripstone<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    direction: Direction,
    height: i32,
    _merge: bool,
) {
    let mut pos = origin;
    for _ in 0..height {
        world.set_block_world(pos, POINTED_DRIPSTONE);
        pos = offset_pos(pos, direction);
    }
}

fn place_dripstone_block_if_possible<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    if !block_matches(world, pos, is_dripstone_replaceable) {
        return false;
    }
    world.set_block_world(pos, DRIPSTONE_BLOCK)
}

fn get_chance_of_stalagmite_or_stalactite(
    radius_x: i32,
    radius_z: i32,
    dx: i32,
    dz: i32,
    config: DripstoneClusterConfiguration,
) -> f64 {
    let edge_distance_x = radius_x - dx.abs();
    let edge_distance_z = radius_z - dz.abs();
    let edge_distance = edge_distance_x.min(edge_distance_z);
    clamped_map(
        edge_distance as f64,
        0.0,
        config.max_distance_from_edge_affecting_chance_of_dripstone_column as f64,
        config.chance_of_dripstone_column_at_max_distance_from_center as f64,
        1.0,
    )
}

fn random_offset(
    random: &mut impl RandomSource,
    origin: BlockPos,
    max_offset_from_origin: i32,
) -> BlockPos {
    BlockPos::new(
        origin.x
            + random_between_inclusive(random, -max_offset_from_origin, max_offset_from_origin),
        origin.y
            + random_between_inclusive(random, -max_offset_from_origin, max_offset_from_origin),
        origin.z
            + random_between_inclusive(random, -max_offset_from_origin, max_offset_from_origin),
    )
}

fn random_direction(random: &mut impl RandomSource) -> Direction {
    ALL_DIRECTIONS[random.next_int_bound(ALL_DIRECTIONS.len() as i32) as usize]
}

fn random_between_inclusive(random: &mut impl RandomSource, min: i32, max: i32) -> i32 {
    random.next_int_bound(max - min + 1) + min
}

fn random_between_biased(
    random: &mut impl RandomSource,
    min: f32,
    max: f32,
    mean: f32,
    deviation: f32,
) -> f32 {
    ((random.next_gaussian() as f32) * deviation + mean).clamp(min, max)
}

fn clamped_map(
    value: f64,
    input_min: f64,
    input_max: f64,
    output_min: f64,
    output_max: f64,
) -> f64 {
    clamped_lerp(
        output_min,
        output_max,
        (value - input_min) / (input_max - input_min),
    )
}

fn clamped_lerp(start: f64, end: f64, delta: f64) -> f64 {
    if delta < 0.0 {
        start
    } else if delta > 1.0 {
        end
    } else {
        start + delta * (end - start)
    }
}

fn at_y(pos: BlockPos, y: i32) -> BlockPos {
    BlockPos::new(pos.x, y, pos.z)
}

fn block_matches<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    predicate: impl FnOnce(RawBlockId) -> bool,
) -> bool {
    world.block_at_world(pos).is_some_and(predicate)
}

fn is_empty_or_water(block: RawBlockId) -> bool {
    matches!(block, AIR | CAVE_AIR) || is_water(block)
}

fn is_base_stone_overworld(block: RawBlockId) -> bool {
    matches!(
        block,
        STONE | GRANITE | DIORITE | ANDESITE | TUFF | DEEPSLATE
    )
}

fn is_dripstone_replaceable(block: RawBlockId) -> bool {
    is_base_stone_overworld(block) || block == DIRT
}

fn is_dripstone_base(block: RawBlockId) -> bool {
    block == DRIPSTONE_BLOCK || is_dripstone_replaceable(block)
}

fn is_dripstone_base_or_lava(block: RawBlockId) -> bool {
    is_dripstone_base(block) || is_lava(block)
}
