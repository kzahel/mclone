use crate::block::{
    BRAIN_CORAL_BLOCK, BUBBLE_CORAL_BLOCK, FIRE_CORAL_BLOCK, HORN_CORAL_BLOCK, KELP, KELP_PLANT,
    MAGMA_BLOCK, RawBlockId, SEA_PICKLE_1, SEA_PICKLE_2, SEA_PICKLE_3, SEA_PICKLE_4, SEAGRASS,
    TALL_SEAGRASS_LOWER, TALL_SEAGRASS_UPPER, TUBE_CORAL_BLOCK, is_coral_block, is_water,
    material_blocks_motion,
};
use crate::placement::{BlockPos, CountConfiguration, HeightmapType};
use crate::prng::RandomSource;

use super::{CoralShape, Direction, FeatureWorld, SeagrassConfiguration};

const CORAL_BLOCKS: [RawBlockId; 5] = [
    TUBE_CORAL_BLOCK,
    BRAIN_CORAL_BLOCK,
    BUBBLE_CORAL_BLOCK,
    FIRE_CORAL_BLOCK,
    HORN_CORAL_BLOCK,
];

const HORIZONTAL_DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

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

pub(super) fn place_sea_pickle<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: CountConfiguration,
) -> bool {
    let mut placed = 0;
    let count = config.count().sample(random);
    for _ in 0..count {
        let x = origin.x + random.next_int_bound(8) - random.next_int_bound(8);
        let z = origin.z + random.next_int_bound(8) - random.next_int_bound(8);
        let Some(y) = world.height_at(HeightmapType::OceanFloor, x, z) else {
            continue;
        };
        let pos = BlockPos::new(x, y, z);
        let state = random_sea_pickle(random);
        if is_water_at(world, pos)
            && can_survive_sea_pickle_at(world, pos)
            && world.set_block_world(pos, state)
        {
            placed += 1;
        }
    }

    placed > 0
}

pub(super) fn place_coral<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    shape: CoralShape,
) -> bool {
    let state = random_coral_block(random);
    match shape {
        CoralShape::Tree => place_coral_tree(world, random, origin, state),
        CoralShape::Claw => place_coral_claw(world, random, origin, state),
        CoralShape::Mushroom => place_coral_mushroom(world, random, origin, state),
    }
}

fn place_coral_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    state: RawBlockId,
) -> bool {
    let mut pos = origin;
    let height = random.next_int_bound(3) + 1;
    for _ in 0..height {
        if !place_coral_block(world, random, pos, state) {
            return true;
        }
        pos = offset_pos(pos, Direction::Up);
    }

    let branch_origin = pos;
    let branch_count = random.next_int_bound(3) + 2;
    let mut directions = HORIZONTAL_DIRECTIONS;
    shuffle(&mut directions, random);
    for direction in directions.iter().take(branch_count as usize).copied() {
        pos = offset_pos(branch_origin, direction);
        let branch_length = random.next_int_bound(5) + 2;
        let mut since_horizontal = 0;
        for step in 0..branch_length {
            if !place_coral_block(world, random, pos, state) {
                break;
            }
            since_horizontal += 1;
            pos = offset_pos(pos, Direction::Up);
            if step == 0 || (since_horizontal >= 2 && random.next_float() < 0.25) {
                pos = offset_pos(pos, direction);
                since_horizontal = 0;
            }
        }
    }

    true
}

fn place_coral_claw<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    state: RawBlockId,
) -> bool {
    if !place_coral_block(world, random, origin, state) {
        return false;
    }

    let main_direction = random_horizontal_direction(random);
    let branch_count = random.next_int_bound(2) + 2;
    let mut directions = [
        main_direction,
        clockwise(main_direction),
        counter_clockwise(main_direction),
    ];
    shuffle(&mut directions, random);

    for direction in directions.iter().take(branch_count as usize).copied() {
        let mut pos = offset_pos(origin, direction);
        let first_run = random.next_int_bound(2) + 1;
        let (extension_direction, extension_length) = if direction == main_direction {
            (main_direction, random.next_int_bound(3) + 2)
        } else {
            pos = offset_pos(pos, Direction::Up);
            let extension_direction = if random.next_int_bound(2) == 0 {
                direction
            } else {
                Direction::Up
            };
            (extension_direction, random.next_int_bound(3) + 3)
        };

        for _ in 0..first_run {
            if !place_coral_block(world, random, pos, state) {
                break;
            }
            pos = offset_pos(pos, extension_direction);
        }

        pos = offset_pos(pos, opposite(extension_direction));
        pos = offset_pos(pos, Direction::Up);
        for _ in 0..extension_length {
            pos = offset_pos(pos, main_direction);
            if !place_coral_block(world, random, pos, state) {
                break;
            }
            if random.next_float() < 0.25 {
                pos = offset_pos(pos, Direction::Up);
            }
        }
    }

    true
}

fn place_coral_mushroom<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    state: RawBlockId,
) -> bool {
    let x_size = random.next_int_bound(3) + 3;
    let y_size = random.next_int_bound(3) + 3;
    let z_size = random.next_int_bound(3) + 3;
    let y_offset = random.next_int_bound(3) + 1;

    for x in 0..=y_size {
        for y in 0..=x_size {
            for z in 0..=z_size {
                let pos = BlockPos::new(origin.x + x, origin.y + y - y_offset, origin.z + z);
                let not_x_y_corner = (x != 0 && x != y_size) || (y != 0 && y != x_size);
                let not_z_y_corner = (z != 0 && z != z_size) || (y != 0 && y != x_size);
                let not_x_z_corner = (x != 0 && x != y_size) || (z != 0 && z != z_size);
                let on_shell =
                    x == 0 || x == y_size || y == 0 || y == x_size || z == 0 || z == z_size;
                if not_x_y_corner
                    && not_z_y_corner
                    && not_x_z_corner
                    && on_shell
                    && random.next_float() >= 0.1
                {
                    place_coral_block(world, random, pos, state);
                }
            }
        }
    }

    true
}

fn place_coral_block<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    pos: BlockPos,
    state: RawBlockId,
) -> bool {
    let above = offset_pos(pos, Direction::Up);
    let can_replace = world
        .block_at_world(pos)
        .is_some_and(|block| is_water(block) || is_coral_block(block));
    if !can_replace || !is_water_at(world, above) {
        return false;
    }

    if !world.set_block_world(pos, state) {
        return false;
    }

    if random.next_float() < 0.25 {
        let _skipped_coral_plant = random_coral_block(random);
    } else if random.next_float() < 0.05 {
        let state = random_sea_pickle(random);
        world.set_block_world(above, state);
    }

    for direction in HORIZONTAL_DIRECTIONS {
        if random.next_float() < 0.2 {
            let side = offset_pos(pos, direction);
            if is_water_at(world, side) {
                let _skipped_wall_coral = random_coral_block(random);
            }
        }
    }

    true
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

fn can_survive_sea_pickle_at<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    world
        .block_at_world(below)
        .is_some_and(|block| material_blocks_motion(block) || is_coral_block(block))
}

fn random_coral_block(random: &mut impl RandomSource) -> RawBlockId {
    CORAL_BLOCKS[random.next_int_bound(CORAL_BLOCKS.len() as i32) as usize]
}

fn random_sea_pickle(random: &mut impl RandomSource) -> RawBlockId {
    match random.next_int_bound(4) + 1 {
        1 => SEA_PICKLE_1,
        2 => SEA_PICKLE_2,
        3 => SEA_PICKLE_3,
        _ => SEA_PICKLE_4,
    }
}

fn random_horizontal_direction(random: &mut impl RandomSource) -> Direction {
    HORIZONTAL_DIRECTIONS[random.next_int_bound(HORIZONTAL_DIRECTIONS.len() as i32) as usize]
}

fn shuffle<T>(values: &mut [T], random: &mut impl RandomSource) {
    for i in (1..values.len()).rev() {
        let j = random.next_int_bound((i + 1) as i32) as usize;
        values.swap(i, j);
    }
}

fn offset_pos(pos: BlockPos, direction: Direction) -> BlockPos {
    let (dx, dy, dz) = match direction {
        Direction::Down => (0, -1, 0),
        Direction::Up => (0, 1, 0),
        Direction::North => (0, 0, -1),
        Direction::South => (0, 0, 1),
        Direction::West => (-1, 0, 0),
        Direction::East => (1, 0, 0),
    };
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}

fn opposite(direction: Direction) -> Direction {
    match direction {
        Direction::Down => Direction::Up,
        Direction::Up => Direction::Down,
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::West => Direction::East,
        Direction::East => Direction::West,
    }
}

fn clockwise(direction: Direction) -> Direction {
    match direction {
        Direction::North => Direction::East,
        Direction::East => Direction::South,
        Direction::South => Direction::West,
        Direction::West => Direction::North,
        Direction::Up | Direction::Down => direction,
    }
}

fn counter_clockwise(direction: Direction) -> Direction {
    match direction {
        Direction::North => Direction::West,
        Direction::West => Direction::South,
        Direction::South => Direction::East,
        Direction::East => Direction::North,
        Direction::Up | Direction::Down => direction,
    }
}
