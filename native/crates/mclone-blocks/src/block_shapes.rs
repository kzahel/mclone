use mclone_core::{Aabb, BlockHitResult, BlockPos, BlockStateId, Vec3d};

use crate::block_clip::clip_aabb;
use crate::block_facts::{is_fluid, terrain_id};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShapeUse {
    Outline,
    Collision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OffsetKind {
    None,
    Xz,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalShape {
    bounds: Aabb,
    offset: OffsetKind,
}

/// Returns the coarse bounds enclosing a block's complete collision shape.
///
/// Multi-box shapes such as stairs fill less space than this union. Movement,
/// point containment, and entity overlap must use [`block_collision_aabbs`].
pub fn block_collision_aabb(state: BlockStateId, pos: BlockPos) -> Option<Aabb> {
    let mut shapes = block_collision_aabbs(state, pos).into_iter();
    let first = shapes.next()?;
    Some(shapes.fold(first, |bounds, shape| {
        Aabb::new(
            bounds.min_x.min(shape.min_x),
            bounds.min_y.min(shape.min_y),
            bounds.min_z.min(shape.min_z),
            bounds.max_x.max(shape.max_x),
            bounds.max_y.max(shape.max_y),
            bounds.max_z.max(shape.max_z),
        )
    }))
}

/// Iterates the exact axis-aligned boxes in a block's collision shape without
/// allocating for the common one-box case.
pub fn block_collision_aabbs(state: BlockStateId, pos: BlockPos) -> impl Iterator<Item = Aabb> {
    shapes_for(state, ShapeUse::Collision)
        .into_iter()
        .flatten()
        .map(move |shape| shape.world_aabb(pos))
}

pub fn block_outline_aabbs(state: BlockStateId, pos: BlockPos) -> Vec<Aabb> {
    shapes_for(state, ShapeUse::Outline)
        .into_iter()
        .flatten()
        .map(|shape| shape.world_aabb(pos))
        .collect()
}

pub fn clip_block_outline(
    state: BlockStateId,
    from: Vec3d,
    to: Vec3d,
    pos: BlockPos,
) -> Option<BlockHitResult> {
    shapes_for(state, ShapeUse::Outline)
        .into_iter()
        .flatten()
        .filter_map(|shape| clip_aabb(from, to, pos, shape.world_aabb(pos)))
        .min_by(|left, right| {
            from.distance_to_sqr(left.location)
                .total_cmp(&from.distance_to_sqr(right.location))
        })
}

fn shapes_for(state: BlockStateId, use_case: ShapeUse) -> [Option<LocalShape>; 5] {
    if (terrain_id::OAK_FENCE_STATE_START..=terrain_id::OAK_FENCE_STATE_END).contains(&state.0) {
        return oak_fence_shapes(state.0 - terrain_id::OAK_FENCE_STATE_START, use_case);
    }
    if (terrain_id::OAK_FENCE_GATE_STATE_START..=terrain_id::OAK_FENCE_GATE_STATE_END)
        .contains(&state.0)
    {
        return oak_fence_gate_shapes(state.0 - terrain_id::OAK_FENCE_GATE_STATE_START, use_case);
    }
    match state.0 {
        terrain_id::SPRUCE_SLAB_BOTTOM => [Some(bottom_slab()), None, None, None, None],
        terrain_id::SPRUCE_SLAB_TOP => [Some(top_slab()), None, None, None, None],
        terrain_id::SPRUCE_STAIRS_NORTH => straight_bottom_stair(0.0, 0.0, 1.0, 0.5),
        terrain_id::SPRUCE_STAIRS_EAST => straight_bottom_stair(0.5, 0.0, 1.0, 1.0),
        terrain_id::SPRUCE_STAIRS_SOUTH => straight_bottom_stair(0.0, 0.5, 1.0, 1.0),
        terrain_id::SPRUCE_STAIRS_WEST => straight_bottom_stair(0.0, 0.0, 0.5, 1.0),
        _ => [shape_for(state, use_case), None, None, None, None],
    }
}

fn shape_for(state: BlockStateId, use_case: ShapeUse) -> Option<LocalShape> {
    match use_case {
        ShapeUse::Outline => outline_shape(state),
        ShapeUse::Collision => collision_shape(state),
    }
}

fn outline_shape(state: BlockStateId) -> Option<LocalShape> {
    match state.0 {
        terrain_id::AIR | terrain_id::CAVE_AIR => None,
        id if is_fluid(BlockStateId(id)) => None,
        terrain_id::SNOW => Some(local_box(0.0, 0.0, 0.0, 1.0, 2.0 / 16.0, 1.0)),
        terrain_id::GRASS | terrain_id::FERN | terrain_id::DEAD_BUSH => Some(local_box(
            2.0 / 16.0,
            0.0,
            2.0 / 16.0,
            14.0 / 16.0,
            13.0 / 16.0,
            14.0 / 16.0,
        )),
        id if is_small_flower(id) => Some(
            local_box(
                5.0 / 16.0,
                0.0,
                5.0 / 16.0,
                11.0 / 16.0,
                10.0 / 16.0,
                11.0 / 16.0,
            )
            .with_offset(OffsetKind::Xz),
        ),
        terrain_id::BROWN_MUSHROOM | terrain_id::RED_MUSHROOM => Some(mushroom_shape()),
        terrain_id::CACTUS => Some(cactus_outline_shape()),
        terrain_id::SUGAR_CANE => Some(sugar_cane_shape()),
        id if is_large_bamboo(id) => Some(bamboo_large_outline_shape()),
        id if is_bamboo(id) => Some(bamboo_outline_shape()),
        terrain_id::LILY_PAD => Some(lily_pad_shape()),
        terrain_id::SEAGRASS => Some(seagrass_shape()),
        terrain_id::TALL_SEAGRASS_LOWER | terrain_id::TALL_SEAGRASS_UPPER => {
            Some(tall_seagrass_shape())
        }
        terrain_id::KELP => Some(kelp_head_shape()),
        terrain_id::KELP_PLANT => Some(full_block()),
        id if is_vine(id) => None,
        id if is_cocoa(id) => Some(cocoa_outline_shape()),
        id if is_double_plant(id) => Some(full_block()),
        terrain_id::SEA_PICKLE_1 => Some(sea_pickle_shape(1)),
        terrain_id::SEA_PICKLE_2 => Some(sea_pickle_shape(2)),
        terrain_id::SEA_PICKLE_3 => Some(sea_pickle_shape(3)),
        terrain_id::SEA_PICKLE_4 => Some(sea_pickle_shape(4)),
        id if is_coral_plant(id) => Some(coral_plant_shape()),
        id if is_coral_fan(id) => Some(coral_fan_shape()),
        terrain_id::TUBE_CORAL_WALL_FAN_NORTH
        | terrain_id::BRAIN_CORAL_WALL_FAN_NORTH
        | terrain_id::BUBBLE_CORAL_WALL_FAN_NORTH
        | terrain_id::FIRE_CORAL_WALL_FAN_NORTH
        | terrain_id::HORN_CORAL_WALL_FAN_NORTH => Some(coral_wall_fan_north_shape()),
        terrain_id::TUBE_CORAL_WALL_FAN_EAST
        | terrain_id::BRAIN_CORAL_WALL_FAN_EAST
        | terrain_id::BUBBLE_CORAL_WALL_FAN_EAST
        | terrain_id::FIRE_CORAL_WALL_FAN_EAST
        | terrain_id::HORN_CORAL_WALL_FAN_EAST => Some(coral_wall_fan_east_shape()),
        terrain_id::TUBE_CORAL_WALL_FAN_SOUTH
        | terrain_id::BRAIN_CORAL_WALL_FAN_SOUTH
        | terrain_id::BUBBLE_CORAL_WALL_FAN_SOUTH
        | terrain_id::FIRE_CORAL_WALL_FAN_SOUTH
        | terrain_id::HORN_CORAL_WALL_FAN_SOUTH => Some(coral_wall_fan_south_shape()),
        terrain_id::TUBE_CORAL_WALL_FAN_WEST
        | terrain_id::BRAIN_CORAL_WALL_FAN_WEST
        | terrain_id::BUBBLE_CORAL_WALL_FAN_WEST
        | terrain_id::FIRE_CORAL_WALL_FAN_WEST
        | terrain_id::HORN_CORAL_WALL_FAN_WEST => Some(coral_wall_fan_west_shape()),
        terrain_id::POINTED_DRIPSTONE => Some(pointed_dripstone_shape()),
        terrain_id::TORCH => Some(torch_shape()),
        terrain_id::WALL_TORCH_NORTH => Some(wall_torch_north_shape()),
        terrain_id::WALL_TORCH_EAST => Some(wall_torch_east_shape()),
        terrain_id::WALL_TORCH_SOUTH => Some(wall_torch_south_shape()),
        terrain_id::WALL_TORCH_WEST => Some(wall_torch_west_shape()),
        terrain_id::GLOW_LICHEN => None,
        terrain_id::FARMLAND_MOISTURE_0..=terrain_id::FARMLAND_MOISTURE_7 => Some(farmland_shape()),
        terrain_id::WHEAT_AGE_0..=terrain_id::WHEAT_AGE_7 => {
            Some(wheat_outline_shape(state.0 - terrain_id::WHEAT_AGE_0))
        }
        terrain_id::CARROTS_AGE_0..=terrain_id::CARROTS_AGE_7 => {
            Some(wheat_outline_shape(state.0 - terrain_id::CARROTS_AGE_0))
        }
        _ => Some(full_block()),
    }
}

fn collision_shape(state: BlockStateId) -> Option<LocalShape> {
    match state.0 {
        terrain_id::AIR
        | terrain_id::CAVE_AIR
        | terrain_id::SNOW
        | terrain_id::GRASS
        | terrain_id::FERN
        | terrain_id::DANDELION
        | terrain_id::POPPY
        | terrain_id::ALLIUM
        | terrain_id::AZURE_BLUET
        | terrain_id::RED_TULIP
        | terrain_id::ORANGE_TULIP
        | terrain_id::WHITE_TULIP
        | terrain_id::PINK_TULIP
        | terrain_id::OXEYE_DAISY
        | terrain_id::CORNFLOWER
        | terrain_id::LILY_OF_THE_VALLEY
        | terrain_id::BLUE_ORCHID
        | terrain_id::BROWN_MUSHROOM
        | terrain_id::RED_MUSHROOM
        | terrain_id::DEAD_BUSH
        | terrain_id::LARGE_FERN_LOWER
        | terrain_id::LARGE_FERN_UPPER
        | terrain_id::TALL_GRASS_LOWER
        | terrain_id::TALL_GRASS_UPPER
        | terrain_id::LILAC_LOWER
        | terrain_id::LILAC_UPPER
        | terrain_id::ROSE_BUSH_LOWER
        | terrain_id::ROSE_BUSH_UPPER
        | terrain_id::PEONY_LOWER
        | terrain_id::PEONY_UPPER
        | terrain_id::SUNFLOWER_LOWER
        | terrain_id::SUNFLOWER_UPPER
        | terrain_id::GLOW_LICHEN
        | terrain_id::SUGAR_CANE
        | terrain_id::SEAGRASS
        | terrain_id::TALL_SEAGRASS_LOWER
        | terrain_id::TALL_SEAGRASS_UPPER
        | terrain_id::KELP
        | terrain_id::KELP_PLANT
        | terrain_id::SWEET_BERRY_BUSH
        | terrain_id::SEA_PICKLE_1
        | terrain_id::SEA_PICKLE_2
        | terrain_id::SEA_PICKLE_3
        | terrain_id::SEA_PICKLE_4
        | terrain_id::TUBE_CORAL
        | terrain_id::BRAIN_CORAL
        | terrain_id::BUBBLE_CORAL
        | terrain_id::FIRE_CORAL
        | terrain_id::HORN_CORAL
        | terrain_id::TUBE_CORAL_FAN
        | terrain_id::BRAIN_CORAL_FAN
        | terrain_id::BUBBLE_CORAL_FAN
        | terrain_id::FIRE_CORAL_FAN
        | terrain_id::HORN_CORAL_FAN
        | terrain_id::TUBE_CORAL_WALL_FAN_NORTH
        | terrain_id::TUBE_CORAL_WALL_FAN_EAST
        | terrain_id::TUBE_CORAL_WALL_FAN_SOUTH
        | terrain_id::TUBE_CORAL_WALL_FAN_WEST
        | terrain_id::BRAIN_CORAL_WALL_FAN_NORTH
        | terrain_id::BRAIN_CORAL_WALL_FAN_EAST
        | terrain_id::BRAIN_CORAL_WALL_FAN_SOUTH
        | terrain_id::BRAIN_CORAL_WALL_FAN_WEST
        | terrain_id::BUBBLE_CORAL_WALL_FAN_NORTH
        | terrain_id::BUBBLE_CORAL_WALL_FAN_EAST
        | terrain_id::BUBBLE_CORAL_WALL_FAN_SOUTH
        | terrain_id::BUBBLE_CORAL_WALL_FAN_WEST
        | terrain_id::FIRE_CORAL_WALL_FAN_NORTH
        | terrain_id::FIRE_CORAL_WALL_FAN_EAST
        | terrain_id::FIRE_CORAL_WALL_FAN_SOUTH
        | terrain_id::FIRE_CORAL_WALL_FAN_WEST
        | terrain_id::HORN_CORAL_WALL_FAN_NORTH
        | terrain_id::HORN_CORAL_WALL_FAN_EAST
        | terrain_id::HORN_CORAL_WALL_FAN_SOUTH
        | terrain_id::HORN_CORAL_WALL_FAN_WEST
        | terrain_id::TORCH
        | terrain_id::WALL_TORCH_NORTH
        | terrain_id::WALL_TORCH_EAST
        | terrain_id::WALL_TORCH_SOUTH
        | terrain_id::WALL_TORCH_WEST => None,
        terrain_id::WHEAT_AGE_0..=terrain_id::WHEAT_AGE_7
        | terrain_id::CARROTS_AGE_0..=terrain_id::CARROTS_AGE_7 => None,
        id if is_vine(id) || is_cocoa(id) => None,
        terrain_id::FARMLAND_MOISTURE_0..=terrain_id::FARMLAND_MOISTURE_7 => Some(farmland_shape()),
        terrain_id::CACTUS => Some(cactus_collision_shape()),
        id if is_bamboo(id) => Some(bamboo_collision_shape()),
        terrain_id::LILY_PAD => Some(lily_pad_shape()),
        terrain_id::POINTED_DRIPSTONE => Some(pointed_dripstone_shape()),
        id if is_fluid(BlockStateId(id)) => None,
        _ => Some(full_block()),
    }
}

fn local_box(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> LocalShape {
    LocalShape {
        bounds: Aabb::new(min_x, min_y, min_z, max_x, max_y, max_z),
        offset: OffsetKind::None,
    }
}

fn full_block() -> LocalShape {
    local_box(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
}

fn farmland_shape() -> LocalShape {
    local_box(0.0, 0.0, 0.0, 1.0, 15.0 / 16.0, 1.0)
}

fn wheat_outline_shape(age: u32) -> LocalShape {
    let height = (age.clamp(0, 7) + 1) as f64 * 2.0 / 16.0;
    local_box(0.0, 0.0, 0.0, 1.0, height, 1.0)
}

fn oak_fence_shapes(bits: u32, use_case: ShapeUse) -> [Option<LocalShape>; 5] {
    let height = match use_case {
        ShapeUse::Outline => 1.0,
        ShapeUse::Collision => 24.0 / 16.0,
    };
    let mut shapes = [None; 5];
    shapes[0] = Some(local_box(
        6.0 / 16.0,
        0.0,
        6.0 / 16.0,
        10.0 / 16.0,
        height,
        10.0 / 16.0,
    ));
    let mut next = 1;
    for (connected, shape) in [
        (
            bits & 1 != 0,
            local_box(6.0 / 16.0, 0.0, 0.0, 10.0 / 16.0, height, 10.0 / 16.0),
        ),
        (
            bits & 2 != 0,
            local_box(6.0 / 16.0, 0.0, 6.0 / 16.0, 1.0, height, 10.0 / 16.0),
        ),
        (
            bits & 4 != 0,
            local_box(6.0 / 16.0, 0.0, 6.0 / 16.0, 10.0 / 16.0, height, 1.0),
        ),
        (
            bits & 8 != 0,
            local_box(0.0, 0.0, 6.0 / 16.0, 10.0 / 16.0, height, 10.0 / 16.0),
        ),
    ] {
        if connected {
            shapes[next] = Some(shape);
            next += 1;
        }
    }
    shapes
}

fn oak_fence_gate_shapes(bits: u32, use_case: ShapeUse) -> [Option<LocalShape>; 5] {
    let facing = bits & 3;
    let open = bits & 4 != 0;
    let in_wall = bits & 16 != 0;
    if use_case == ShapeUse::Collision && open {
        return [None; 5];
    }
    let height = match use_case {
        ShapeUse::Outline if in_wall => 13.0 / 16.0,
        ShapeUse::Outline => 1.0,
        ShapeUse::Collision => 24.0 / 16.0,
    };
    let shape = if facing == 0 || facing == 2 {
        local_box(0.0, 0.0, 6.0 / 16.0, 1.0, height, 10.0 / 16.0)
    } else {
        local_box(6.0 / 16.0, 0.0, 0.0, 10.0 / 16.0, height, 1.0)
    };
    [Some(shape), None, None, None, None]
}

fn bottom_slab() -> LocalShape {
    local_box(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)
}

fn top_slab() -> LocalShape {
    local_box(0.0, 0.5, 0.0, 1.0, 1.0, 1.0)
}

fn straight_bottom_stair(
    upper_min_x: f64,
    upper_min_z: f64,
    upper_max_x: f64,
    upper_max_z: f64,
) -> [Option<LocalShape>; 5] {
    [
        Some(bottom_slab()),
        Some(local_box(
            upper_min_x,
            0.5,
            upper_min_z,
            upper_max_x,
            1.0,
            upper_max_z,
        )),
        None,
        None,
        None,
    ]
}

fn is_small_flower(id: u32) -> bool {
    matches!(
        id,
        terrain_id::DANDELION
            | terrain_id::POPPY
            | terrain_id::ALLIUM
            | terrain_id::AZURE_BLUET
            | terrain_id::RED_TULIP
            | terrain_id::ORANGE_TULIP
            | terrain_id::WHITE_TULIP
            | terrain_id::PINK_TULIP
            | terrain_id::OXEYE_DAISY
            | terrain_id::CORNFLOWER
            | terrain_id::LILY_OF_THE_VALLEY
            | terrain_id::BLUE_ORCHID
    )
}

fn is_double_plant(id: u32) -> bool {
    matches!(
        id,
        terrain_id::LARGE_FERN_LOWER
            | terrain_id::LARGE_FERN_UPPER
            | terrain_id::TALL_GRASS_LOWER
            | terrain_id::TALL_GRASS_UPPER
            | terrain_id::LILAC_LOWER
            | terrain_id::LILAC_UPPER
            | terrain_id::ROSE_BUSH_LOWER
            | terrain_id::ROSE_BUSH_UPPER
            | terrain_id::PEONY_LOWER
            | terrain_id::PEONY_UPPER
            | terrain_id::SUNFLOWER_LOWER
            | terrain_id::SUNFLOWER_UPPER
    )
}

fn is_vine(id: u32) -> bool {
    matches!(
        id,
        terrain_id::VINE_EAST
            | terrain_id::VINE_UP
            | terrain_id::VINE_NORTH
            | terrain_id::VINE_SOUTH
            | terrain_id::VINE_WEST
    )
}

fn is_cocoa(id: u32) -> bool {
    matches!(
        id,
        terrain_id::COCOA_AGE0_NORTH
            | terrain_id::COCOA_AGE0_EAST
            | terrain_id::COCOA_AGE0_SOUTH
            | terrain_id::COCOA_AGE0_WEST
            | terrain_id::COCOA_AGE1_NORTH
            | terrain_id::COCOA_AGE1_EAST
            | terrain_id::COCOA_AGE1_SOUTH
            | terrain_id::COCOA_AGE1_WEST
            | terrain_id::COCOA_AGE2_NORTH
            | terrain_id::COCOA_AGE2_EAST
            | terrain_id::COCOA_AGE2_SOUTH
            | terrain_id::COCOA_AGE2_WEST
    )
}

fn is_bamboo(id: u32) -> bool {
    matches!(
        id,
        terrain_id::BAMBOO
            | terrain_id::BAMBOO_TOP_SMALL
            | terrain_id::BAMBOO_TOP_LARGE
            | terrain_id::BAMBOO_FINAL_LARGE
    )
}

fn is_large_bamboo(id: u32) -> bool {
    matches!(
        id,
        terrain_id::BAMBOO_TOP_LARGE | terrain_id::BAMBOO_FINAL_LARGE
    )
}

fn is_coral_plant(id: u32) -> bool {
    matches!(
        id,
        terrain_id::TUBE_CORAL
            | terrain_id::BRAIN_CORAL
            | terrain_id::BUBBLE_CORAL
            | terrain_id::FIRE_CORAL
            | terrain_id::HORN_CORAL
    )
}

fn is_coral_fan(id: u32) -> bool {
    matches!(
        id,
        terrain_id::TUBE_CORAL_FAN
            | terrain_id::BRAIN_CORAL_FAN
            | terrain_id::BUBBLE_CORAL_FAN
            | terrain_id::FIRE_CORAL_FAN
            | terrain_id::HORN_CORAL_FAN
    )
}

fn pointed_dripstone_shape() -> LocalShape {
    local_box(5.0 / 16.0, 0.0, 5.0 / 16.0, 11.0 / 16.0, 1.0, 11.0 / 16.0)
}

fn cocoa_outline_shape() -> LocalShape {
    local_box(
        4.0 / 16.0,
        3.0 / 16.0,
        4.0 / 16.0,
        12.0 / 16.0,
        12.0 / 16.0,
        12.0 / 16.0,
    )
}

fn cactus_outline_shape() -> LocalShape {
    local_box(1.0 / 16.0, 0.0, 1.0 / 16.0, 15.0 / 16.0, 1.0, 15.0 / 16.0)
}

fn cactus_collision_shape() -> LocalShape {
    local_box(
        1.0 / 16.0,
        0.0,
        1.0 / 16.0,
        15.0 / 16.0,
        15.0 / 16.0,
        15.0 / 16.0,
    )
}

fn sugar_cane_shape() -> LocalShape {
    local_box(2.0 / 16.0, 0.0, 2.0 / 16.0, 14.0 / 16.0, 1.0, 14.0 / 16.0)
}

fn bamboo_outline_shape() -> LocalShape {
    local_box(5.0 / 16.0, 0.0, 5.0 / 16.0, 11.0 / 16.0, 1.0, 11.0 / 16.0)
        .with_offset(OffsetKind::Xz)
}

fn bamboo_large_outline_shape() -> LocalShape {
    local_box(3.0 / 16.0, 0.0, 3.0 / 16.0, 13.0 / 16.0, 1.0, 13.0 / 16.0)
        .with_offset(OffsetKind::Xz)
}

fn bamboo_collision_shape() -> LocalShape {
    local_box(6.5 / 16.0, 0.0, 6.5 / 16.0, 9.5 / 16.0, 1.0, 9.5 / 16.0).with_offset(OffsetKind::Xz)
}

fn mushroom_shape() -> LocalShape {
    local_box(
        5.0 / 16.0,
        0.0,
        5.0 / 16.0,
        11.0 / 16.0,
        6.0 / 16.0,
        11.0 / 16.0,
    )
}

fn lily_pad_shape() -> LocalShape {
    local_box(
        1.0 / 16.0,
        0.0,
        1.0 / 16.0,
        15.0 / 16.0,
        1.5 / 16.0,
        15.0 / 16.0,
    )
}

fn seagrass_shape() -> LocalShape {
    local_box(
        2.0 / 16.0,
        0.0,
        2.0 / 16.0,
        14.0 / 16.0,
        12.0 / 16.0,
        14.0 / 16.0,
    )
}

fn tall_seagrass_shape() -> LocalShape {
    local_box(2.0 / 16.0, 0.0, 2.0 / 16.0, 14.0 / 16.0, 1.0, 14.0 / 16.0)
}

fn kelp_head_shape() -> LocalShape {
    local_box(0.0, 0.0, 0.0, 1.0, 9.0 / 16.0, 1.0)
}

fn sea_pickle_shape(pickles: u32) -> LocalShape {
    match pickles {
        1 => local_box(
            6.0 / 16.0,
            0.0,
            6.0 / 16.0,
            10.0 / 16.0,
            6.0 / 16.0,
            10.0 / 16.0,
        ),
        2 => local_box(
            3.0 / 16.0,
            0.0,
            3.0 / 16.0,
            13.0 / 16.0,
            6.0 / 16.0,
            13.0 / 16.0,
        ),
        3 => local_box(
            2.0 / 16.0,
            0.0,
            2.0 / 16.0,
            14.0 / 16.0,
            6.0 / 16.0,
            14.0 / 16.0,
        ),
        _ => local_box(
            2.0 / 16.0,
            0.0,
            2.0 / 16.0,
            14.0 / 16.0,
            7.0 / 16.0,
            14.0 / 16.0,
        ),
    }
}

fn coral_plant_shape() -> LocalShape {
    local_box(
        2.0 / 16.0,
        0.0,
        2.0 / 16.0,
        14.0 / 16.0,
        15.0 / 16.0,
        14.0 / 16.0,
    )
}

fn coral_fan_shape() -> LocalShape {
    local_box(
        2.0 / 16.0,
        0.0,
        2.0 / 16.0,
        14.0 / 16.0,
        4.0 / 16.0,
        14.0 / 16.0,
    )
}

fn coral_wall_fan_north_shape() -> LocalShape {
    local_box(0.0, 4.0 / 16.0, 5.0 / 16.0, 1.0, 12.0 / 16.0, 1.0)
}

fn coral_wall_fan_south_shape() -> LocalShape {
    local_box(0.0, 4.0 / 16.0, 0.0, 1.0, 12.0 / 16.0, 11.0 / 16.0)
}

fn coral_wall_fan_west_shape() -> LocalShape {
    local_box(5.0 / 16.0, 4.0 / 16.0, 0.0, 1.0, 12.0 / 16.0, 1.0)
}

fn coral_wall_fan_east_shape() -> LocalShape {
    local_box(0.0, 4.0 / 16.0, 0.0, 11.0 / 16.0, 12.0 / 16.0, 1.0)
}

fn torch_shape() -> LocalShape {
    local_box(
        6.0 / 16.0,
        0.0,
        6.0 / 16.0,
        10.0 / 16.0,
        10.0 / 16.0,
        10.0 / 16.0,
    )
}

fn wall_torch_north_shape() -> LocalShape {
    local_box(
        5.5 / 16.0,
        3.0 / 16.0,
        11.0 / 16.0,
        10.5 / 16.0,
        13.0 / 16.0,
        1.0,
    )
}

fn wall_torch_south_shape() -> LocalShape {
    local_box(
        5.5 / 16.0,
        3.0 / 16.0,
        0.0,
        10.5 / 16.0,
        13.0 / 16.0,
        5.0 / 16.0,
    )
}

fn wall_torch_west_shape() -> LocalShape {
    local_box(
        11.0 / 16.0,
        3.0 / 16.0,
        5.5 / 16.0,
        1.0,
        13.0 / 16.0,
        10.5 / 16.0,
    )
}

fn wall_torch_east_shape() -> LocalShape {
    local_box(
        0.0,
        3.0 / 16.0,
        5.5 / 16.0,
        5.0 / 16.0,
        13.0 / 16.0,
        10.5 / 16.0,
    )
}

impl LocalShape {
    fn with_offset(self, offset: OffsetKind) -> Self {
        Self { offset, ..self }
    }

    fn world_aabb(self, pos: BlockPos) -> Aabb {
        let offset = match self.offset {
            OffsetKind::None => Vec3d::ZERO,
            OffsetKind::Xz => java_block_offset_xz(pos),
        };
        self.bounds.move_by(Vec3d::new(
            pos.x as f64 + offset.x,
            pos.y as f64 + offset.y,
            pos.z as f64 + offset.z,
        ))
    }
}

fn java_block_offset_xz(pos: BlockPos) -> Vec3d {
    let seed = java_mth_seed(pos.x, 0, pos.z);
    let max_horizontal_offset = 0.25;
    let x = ((((seed & 15) as f64) / 15.0 - 0.5) * 0.5)
        .clamp(-max_horizontal_offset, max_horizontal_offset);
    let z = ((((seed >> 8 & 15) as f64) / 15.0 - 0.5) * 0.5)
        .clamp(-max_horizontal_offset, max_horizontal_offset);
    Vec3d::new(x, 0.0, z)
}

fn java_mth_seed(x: i32, y: i32, z: i32) -> i64 {
    let x_term = x.wrapping_mul(3_129_871) as i64;
    let z_term = (z as i64).wrapping_mul(116_129_781);
    let mut seed = x_term ^ z_term ^ y as i64;
    seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42_317_861)
        .wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Direction;

    fn state(id: u32) -> BlockStateId {
        BlockStateId(id)
    }

    #[test]
    fn snow_layer_one_uses_java_outline_and_collision_shapes() {
        let pos = BlockPos::new(4, 2, 1);

        assert_eq!(
            shape_for(state(terrain_id::SNOW), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(4.0, 2.0, 1.0, 5.0, 2.125, 2.0))
        );
        assert_eq!(block_collision_aabb(state(terrain_id::SNOW), pos), None);
    }

    #[test]
    fn java_no_collision_blocks_have_empty_collision_shapes() {
        for id in [
            terrain_id::AIR,
            terrain_id::CAVE_AIR,
            terrain_id::WATER,
            terrain_id::WATER_LEVEL_8,
            terrain_id::LAVA,
            terrain_id::LAVA_LEVEL_8,
            terrain_id::SNOW,
            terrain_id::GRASS,
            terrain_id::FERN,
            terrain_id::DANDELION,
            terrain_id::POPPY,
            terrain_id::ALLIUM,
            terrain_id::AZURE_BLUET,
            terrain_id::RED_TULIP,
            terrain_id::ORANGE_TULIP,
            terrain_id::WHITE_TULIP,
            terrain_id::PINK_TULIP,
            terrain_id::OXEYE_DAISY,
            terrain_id::CORNFLOWER,
            terrain_id::LILY_OF_THE_VALLEY,
            terrain_id::BLUE_ORCHID,
            terrain_id::BROWN_MUSHROOM,
            terrain_id::RED_MUSHROOM,
            terrain_id::DEAD_BUSH,
            terrain_id::LARGE_FERN_LOWER,
            terrain_id::LARGE_FERN_UPPER,
            terrain_id::LILAC_LOWER,
            terrain_id::LILAC_UPPER,
            terrain_id::ROSE_BUSH_LOWER,
            terrain_id::ROSE_BUSH_UPPER,
            terrain_id::PEONY_LOWER,
            terrain_id::PEONY_UPPER,
            terrain_id::SUNFLOWER_LOWER,
            terrain_id::SUNFLOWER_UPPER,
            terrain_id::GLOW_LICHEN,
            terrain_id::SUGAR_CANE,
            terrain_id::SWEET_BERRY_BUSH,
            terrain_id::SEAGRASS,
            terrain_id::TALL_SEAGRASS_LOWER,
            terrain_id::TALL_SEAGRASS_UPPER,
            terrain_id::KELP,
            terrain_id::KELP_PLANT,
            terrain_id::SEA_PICKLE_1,
            terrain_id::SEA_PICKLE_2,
            terrain_id::SEA_PICKLE_3,
            terrain_id::SEA_PICKLE_4,
            terrain_id::TUBE_CORAL,
            terrain_id::BRAIN_CORAL,
            terrain_id::BUBBLE_CORAL,
            terrain_id::FIRE_CORAL,
            terrain_id::HORN_CORAL,
            terrain_id::TUBE_CORAL_FAN,
            terrain_id::BRAIN_CORAL_FAN,
            terrain_id::BUBBLE_CORAL_FAN,
            terrain_id::FIRE_CORAL_FAN,
            terrain_id::HORN_CORAL_FAN,
            terrain_id::TUBE_CORAL_WALL_FAN_NORTH,
            terrain_id::TUBE_CORAL_WALL_FAN_EAST,
            terrain_id::TUBE_CORAL_WALL_FAN_SOUTH,
            terrain_id::TUBE_CORAL_WALL_FAN_WEST,
            terrain_id::BRAIN_CORAL_WALL_FAN_NORTH,
            terrain_id::BRAIN_CORAL_WALL_FAN_EAST,
            terrain_id::BRAIN_CORAL_WALL_FAN_SOUTH,
            terrain_id::BRAIN_CORAL_WALL_FAN_WEST,
            terrain_id::BUBBLE_CORAL_WALL_FAN_NORTH,
            terrain_id::BUBBLE_CORAL_WALL_FAN_EAST,
            terrain_id::BUBBLE_CORAL_WALL_FAN_SOUTH,
            terrain_id::BUBBLE_CORAL_WALL_FAN_WEST,
            terrain_id::FIRE_CORAL_WALL_FAN_NORTH,
            terrain_id::FIRE_CORAL_WALL_FAN_EAST,
            terrain_id::FIRE_CORAL_WALL_FAN_SOUTH,
            terrain_id::FIRE_CORAL_WALL_FAN_WEST,
            terrain_id::HORN_CORAL_WALL_FAN_NORTH,
            terrain_id::HORN_CORAL_WALL_FAN_EAST,
            terrain_id::HORN_CORAL_WALL_FAN_SOUTH,
            terrain_id::HORN_CORAL_WALL_FAN_WEST,
            terrain_id::TORCH,
            terrain_id::WALL_TORCH_NORTH,
            terrain_id::WALL_TORCH_EAST,
            terrain_id::WALL_TORCH_SOUTH,
            terrain_id::WALL_TORCH_WEST,
        ] {
            assert_eq!(
                block_collision_aabb(state(id), BlockPos::ZERO),
                None,
                "state {id}"
            );
        }
    }

    #[test]
    fn terrain_solids_use_full_cube_collision_shapes() {
        for id in [
            1,
            3,
            7,
            40,
            42,
            49,
            terrain_id::DRIPSTONE_BLOCK,
            terrain_id::DARK_OAK_LOG,
            terrain_id::DARK_OAK_LEAVES,
            terrain_id::BROWN_MUSHROOM_BLOCK,
            terrain_id::RED_MUSHROOM_BLOCK,
            terrain_id::MUSHROOM_STEM,
            terrain_id::ACACIA_LOG,
            terrain_id::ACACIA_LEAVES,
            terrain_id::JUNGLE_LOG,
            terrain_id::JUNGLE_LEAVES,
        ] {
            assert_eq!(
                block_collision_aabb(state(id), BlockPos::new(1, 2, 3)),
                Some(Aabb::new(1.0, 2.0, 3.0, 2.0, 3.0, 4.0)),
                "state {id}"
            );
        }
    }

    #[test]
    fn plant_outline_boxes_match_java_block_classes() {
        let pos = BlockPos::new(0, 0, 0);

        assert_eq!(
            shape_for(state(terrain_id::GRASS), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.8125, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::DEAD_BUSH), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.8125, 0.875))
        );
        let flower_offset = java_block_offset_xz(pos);
        assert_eq!(
            shape_for(state(terrain_id::ALLIUM), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                flower_offset.x + 5.0 / 16.0,
                0.0,
                flower_offset.z + 5.0 / 16.0,
                flower_offset.x + 11.0 / 16.0,
                10.0 / 16.0,
                flower_offset.z + 11.0 / 16.0,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::BROWN_MUSHROOM), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                5.0 / 16.0,
                0.0,
                5.0 / 16.0,
                11.0 / 16.0,
                6.0 / 16.0,
                11.0 / 16.0,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::RED_MUSHROOM), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                5.0 / 16.0,
                0.0,
                5.0 / 16.0,
                11.0 / 16.0,
                6.0 / 16.0,
                11.0 / 16.0,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::LARGE_FERN_LOWER), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::LILAC_LOWER), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::SUNFLOWER_LOWER), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::SWEET_BERRY_BUSH), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::SWEET_BERRY_BUSH), pos),
            None
        );
        assert_eq!(
            shape_for(state(terrain_id::SUGAR_CANE), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 1.0, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::SEAGRASS), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.75, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::TALL_SEAGRASS_UPPER), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 1.0, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::KELP), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 0.5625, 1.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::KELP_PLANT), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::SEA_PICKLE_1), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.375, 0.0, 0.375, 0.625, 0.375, 0.625))
        );
        assert_eq!(
            shape_for(state(terrain_id::SEA_PICKLE_4), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.4375, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::TUBE_CORAL), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.9375, 0.875))
        );
        assert_eq!(
            shape_for(state(terrain_id::HORN_CORAL_FAN), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.125, 0.0, 0.125, 0.875, 0.25, 0.875))
        );
        assert_eq!(
            shape_for(
                state(terrain_id::BRAIN_CORAL_WALL_FAN_EAST),
                ShapeUse::Outline,
            )
            .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0, 0.25, 0.0, 0.6875, 0.75, 1.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::CACTUS), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0625, 0.0, 0.0625, 0.9375, 1.0, 0.9375))
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::CACTUS), pos),
            Some(Aabb::new(0.0625, 0.0, 0.0625, 0.9375, 0.9375, 0.9375))
        );
        let offset = java_block_offset_xz(pos);
        assert_eq!(
            shape_for(state(terrain_id::BAMBOO), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                offset.x + 0.3125,
                0.0,
                offset.z + 0.3125,
                offset.x + 0.6875,
                1.0,
                offset.z + 0.6875,
            ))
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::BAMBOO), pos),
            Some(Aabb::new(
                offset.x + 0.40625,
                0.0,
                offset.z + 0.40625,
                offset.x + 0.59375,
                1.0,
                offset.z + 0.59375,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::BAMBOO_TOP_SMALL), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                offset.x + 0.3125,
                0.0,
                offset.z + 0.3125,
                offset.x + 0.6875,
                1.0,
                offset.z + 0.6875,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::BAMBOO_TOP_LARGE), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                offset.x + 0.1875,
                0.0,
                offset.z + 0.1875,
                offset.x + 0.8125,
                1.0,
                offset.z + 0.8125,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::BAMBOO_FINAL_LARGE), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                offset.x + 0.1875,
                0.0,
                offset.z + 0.1875,
                offset.x + 0.8125,
                1.0,
                offset.z + 0.8125,
            ))
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::BAMBOO_FINAL_LARGE), pos),
            Some(Aabb::new(
                offset.x + 0.40625,
                0.0,
                offset.z + 0.40625,
                offset.x + 0.59375,
                1.0,
                offset.z + 0.59375,
            ))
        );
        assert_eq!(
            shape_for(state(terrain_id::LILY_PAD), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(0.0625, 0.0, 0.0625, 0.9375, 0.09375, 0.9375))
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::LILY_PAD), pos),
            Some(Aabb::new(0.0625, 0.0, 0.0625, 0.9375, 0.09375, 0.9375))
        );
    }

    #[test]
    fn flower_outline_uses_java_position_offset() {
        let pos = BlockPos::new(4, 2, 1);
        let offset = java_block_offset_xz(pos);

        assert_eq!(
            shape_for(state(terrain_id::DANDELION), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(
                pos.x as f64 + offset.x + 5.0 / 16.0,
                pos.y as f64,
                pos.z as f64 + offset.z + 5.0 / 16.0,
                pos.x as f64 + offset.x + 11.0 / 16.0,
                pos.y as f64 + 10.0 / 16.0,
                pos.z as f64 + offset.z + 11.0 / 16.0,
            ))
        );
    }

    #[test]
    fn pointed_dripstone_uses_partial_center_shape() {
        let pos = BlockPos::new(1, 2, 3);
        let expected = Aabb::new(1.3125, 2.0, 3.3125, 1.6875, 3.0, 3.6875);

        assert_eq!(
            shape_for(state(terrain_id::POINTED_DRIPSTONE), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(expected)
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::POINTED_DRIPSTONE), pos),
            Some(expected)
        );
    }

    #[test]
    fn torch_shapes_match_java_block_classes() {
        let pos = BlockPos::new(1, 2, 3);

        assert_eq!(
            shape_for(state(terrain_id::TORCH), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(1.375, 2.0, 3.375, 1.625, 2.625, 3.625))
        );
        assert_eq!(
            shape_for(state(terrain_id::WALL_TORCH_NORTH), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(1.34375, 2.1875, 3.6875, 1.65625, 2.8125, 4.0))
        );
        assert_eq!(
            shape_for(state(terrain_id::WALL_TORCH_SOUTH), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(1.34375, 2.1875, 3.0, 1.65625, 2.8125, 3.3125))
        );
        assert_eq!(
            shape_for(state(terrain_id::WALL_TORCH_EAST), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(1.0, 2.1875, 3.34375, 1.3125, 2.8125, 3.65625))
        );
        assert_eq!(
            shape_for(state(terrain_id::WALL_TORCH_WEST), ShapeUse::Outline)
                .map(|shape| shape.world_aabb(pos)),
            Some(Aabb::new(1.6875, 2.1875, 3.34375, 2.0, 2.8125, 3.65625))
        );
        assert_eq!(block_collision_aabb(state(terrain_id::TORCH), pos), None);
        assert_eq!(
            block_collision_aabb(state(terrain_id::WALL_TORCH_NORTH), pos),
            None
        );
    }

    #[test]
    fn clipping_outline_reports_face_and_inside_hits() {
        let pos = BlockPos::new(0, 0, 0);

        let hit = clip_block_outline(
            state(1),
            Vec3d::new(-1.0, 0.5, 0.5),
            Vec3d::new(2.0, 0.5, 0.5),
            pos,
        )
        .expect("ray should hit full block outline");
        assert!(!hit.miss);
        assert_eq!(hit.direction, Direction::West);
        assert_eq!(hit.block_pos, pos);

        let inside = clip_block_outline(
            state(1),
            Vec3d::new(0.5, 0.5, 0.5),
            Vec3d::new(2.0, 0.5, 0.5),
            pos,
        )
        .expect("inside ray should produce a hit");
        assert!(inside.inside);
    }

    #[test]
    fn spruce_slabs_and_straight_stairs_use_java_shaped_boxes() {
        let pos = BlockPos::new(1, 2, 3);

        assert_eq!(
            block_collision_aabbs(state(terrain_id::SPRUCE_SLAB_BOTTOM), pos).collect::<Vec<_>>(),
            vec![Aabb::new(1.0, 2.0, 3.0, 2.0, 2.5, 4.0)]
        );
        assert_eq!(
            block_collision_aabbs(state(terrain_id::SPRUCE_SLAB_TOP), pos).collect::<Vec<_>>(),
            vec![Aabb::new(1.0, 2.5, 3.0, 2.0, 3.0, 4.0)]
        );
        assert_eq!(
            block_collision_aabbs(state(terrain_id::SPRUCE_STAIRS_NORTH), pos).collect::<Vec<_>>(),
            vec![
                Aabb::new(1.0, 2.0, 3.0, 2.0, 2.5, 4.0),
                Aabb::new(1.0, 2.5, 3.0, 2.0, 3.0, 3.5),
            ]
        );
        assert_eq!(
            block_collision_aabbs(state(terrain_id::SPRUCE_STAIRS_EAST), pos).collect::<Vec<_>>(),
            vec![
                Aabb::new(1.0, 2.0, 3.0, 2.0, 2.5, 4.0),
                Aabb::new(1.5, 2.5, 3.0, 2.0, 3.0, 4.0),
            ]
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::SPRUCE_STAIRS_NORTH), pos),
            Some(Aabb::unit_block(pos))
        );
    }

    #[test]
    fn farming_shapes_match_java_farmland_and_crop_contracts() {
        let pos = BlockPos::new(2, 64, -3);
        assert_eq!(
            block_collision_aabb(state(terrain_id::FARMLAND_MOISTURE_0), pos),
            Some(Aabb::new(2.0, 64.0, -3.0, 3.0, 64.9375, -2.0))
        );
        assert_eq!(
            block_outline_aabbs(state(terrain_id::WHEAT_AGE_0), pos),
            vec![Aabb::new(2.0, 64.0, -3.0, 3.0, 64.125, -2.0)]
        );
        assert_eq!(
            block_outline_aabbs(state(terrain_id::WHEAT_AGE_7), pos),
            vec![Aabb::new(2.0, 64.0, -3.0, 3.0, 65.0, -2.0)]
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::WHEAT_AGE_7), pos),
            None
        );
        assert_eq!(
            block_outline_aabbs(state(terrain_id::CARROTS_AGE_0), pos),
            vec![Aabb::new(2.0, 64.0, -3.0, 3.0, 64.125, -2.0)]
        );
        assert_eq!(
            block_outline_aabbs(state(terrain_id::CARROTS_AGE_7), pos),
            vec![Aabb::new(2.0, 64.0, -3.0, 3.0, 65.0, -2.0)]
        );
        assert_eq!(
            block_collision_aabb(state(terrain_id::CARROTS_AGE_7), pos),
            None
        );
    }

    #[test]
    fn fence_and_gate_shapes_match_java_connection_and_open_contracts() {
        let pos = BlockPos::new(2, 64, -3);
        let north_east_fence = state(terrain_id::OAK_FENCE_STATE_START + 1 + 2);
        assert_eq!(
            block_outline_aabbs(north_east_fence, pos),
            vec![
                Aabb::new(2.375, 64.0, -2.625, 2.625, 65.0, -2.375),
                Aabb::new(2.375, 64.0, -3.0, 2.625, 65.0, -2.375),
                Aabb::new(2.375, 64.0, -2.625, 3.0, 65.0, -2.375),
            ]
        );
        assert_eq!(
            block_collision_aabb(north_east_fence, pos),
            Some(Aabb::new(2.375, 64.0, -3.0, 3.0, 65.5, -2.375))
        );

        let closed_north = state(terrain_id::OAK_FENCE_GATE_STATE_START);
        let open_north = state(terrain_id::OAK_FENCE_GATE_STATE_START + 4);
        assert_eq!(
            block_collision_aabb(closed_north, pos),
            Some(Aabb::new(2.0, 64.0, -2.625, 3.0, 65.5, -2.375))
        );
        assert_eq!(block_collision_aabb(open_north, pos), None);
        assert_eq!(
            block_outline_aabbs(open_north, pos),
            vec![Aabb::new(2.0, 64.0, -2.625, 3.0, 65.0, -2.375)]
        );
    }
}
