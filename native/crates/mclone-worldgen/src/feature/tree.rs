use crate::block::{
    AIR, BIRCH_LEAVES, BIRCH_LOG, CAVE_AIR, DANDELION, DEAD_BUSH, DIRT, FERN, GLOW_LICHEN, GRASS,
    GRASS_BLOCK, LARGE_FERN_LOWER, LARGE_FERN_UPPER, MYCELIUM, OAK_LEAVES, OAK_LOG, PODZOL, POPPY,
    SPRUCE_LEAVES, SPRUCE_LOG, WATER,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{
    BasicTreeConfiguration, FeatureWorld, FoliagePlacerConfiguration, TreeConfiguration,
    project_to_surface,
};

pub(super) fn place_basic_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: BasicTreeConfiguration,
) -> bool {
    let Some(base) = project_to_surface(world, origin) else {
        return false;
    };
    let below = BlockPos::new(base.x, base.y - 1, base.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    if !matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM) {
        return false;
    }

    let height = config.min_height + random.next_int_bound(config.random_height.max(1));
    let leaves_center_y = base.y + height;
    if base.y < world.min_y() + 1 || leaves_center_y + 1 >= world.min_y() + world.height() {
        return false;
    }

    let mut targets = Vec::new();
    for y in base.y..base.y + height {
        targets.push((BlockPos::new(base.x, y, base.z), config.log));
    }

    for dy in -2_i32..=1 {
        let radius: i32 = if dy == 1 { 1 } else { 2 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let corner = dx.abs() == radius && dz.abs() == radius;
                if corner && (dy == 1 || random.next_boolean()) {
                    continue;
                }
                targets.push((
                    BlockPos::new(base.x + dx, leaves_center_y + dy, base.z + dz),
                    config.leaves,
                ));
            }
        }
    }

    if targets
        .iter()
        .any(|(pos, _)| !can_replace_tree_block(world, *pos))
    {
        return false;
    }

    world.set_block_world(below, DIRT);
    for (pos, block_id) in targets {
        world.set_block_world(pos, block_id);
    }
    true
}

pub(super) fn place_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: TreeConfiguration,
) -> bool {
    let base = origin;

    let tree_height = config.trunk_placer.tree_height(random);
    let foliage_height = config
        .foliage_placer
        .foliage_height(random, tree_height, config);
    let trunk_height = tree_height - foliage_height;
    let foliage_radius = config.foliage_placer.foliage_radius(random, trunk_height);

    if base.y < world.min_y() + 1 || base.y + tree_height + 1 > world.min_y() + world.height() {
        return false;
    }
    if !can_survive_tree_sapling(world, base) {
        return false;
    }
    let max_free_tree_height = get_max_free_tree_height(world, tree_height, base, config);
    if max_free_tree_height < tree_height {
        return false;
    }

    place_straight_trunk(world, random, base, tree_height, config);
    let foliage_attachment = BlockPos::new(base.x, base.y + tree_height, base.z);
    create_foliage(
        world,
        random,
        config,
        foliage_attachment,
        foliage_height,
        foliage_radius,
    );
    true
}

fn get_max_free_tree_height<W: FeatureWorld>(
    world: &mut W,
    tree_height: i32,
    base: BlockPos,
    config: TreeConfiguration,
) -> i32 {
    for y_offset in 0..=tree_height + 1 {
        let radius = config.minimum_size.size_at_height(y_offset);
        for x_offset in -radius..=radius {
            for z_offset in -radius..=radius {
                let pos = BlockPos::new(base.x + x_offset, base.y + y_offset, base.z + z_offset);
                if !is_free_tree_pos(world, pos) {
                    return y_offset - 2;
                }
            }
        }
    }

    tree_height
}

fn place_straight_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
) {
    set_dirt_at(world, random, BlockPos::new(base.x, base.y - 1, base.z));

    for y_offset in 0..height {
        place_log(
            world,
            random,
            BlockPos::new(base.x, base.y + y_offset, base.z),
            config,
        );
    }
}

fn create_foliage<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    attachment: BlockPos,
    foliage_height: i32,
    foliage_radius: i32,
) {
    let offset = config.foliage_placer.offset(random);
    match config.foliage_placer {
        FoliagePlacerConfiguration::Blob { .. } => {
            for y_offset in ((offset - foliage_height)..=offset).rev() {
                let radius = (foliage_radius - 1 - y_offset / 2).max(0);
                place_leaves_row(world, random, config, attachment, radius, y_offset);
            }
        }
        FoliagePlacerConfiguration::Spruce { .. } => {
            let mut radius = random.next_int_bound(2);
            let mut radius_limit = 1;
            let mut reset_radius = 0;

            for y_offset in (-(foliage_height)..=offset).rev() {
                place_leaves_row(world, random, config, attachment, radius, y_offset);
                if radius >= radius_limit {
                    radius = reset_radius;
                    reset_radius = 1;
                    radius_limit = (radius_limit + 1).min(foliage_radius);
                } else {
                    radius += 1;
                }
            }
        }
        FoliagePlacerConfiguration::Pine { .. } => {
            let mut radius = 0;

            for y_offset in ((offset - foliage_height)..=offset).rev() {
                place_leaves_row(world, random, config, attachment, radius, y_offset);
                if radius >= 1 && y_offset == offset - foliage_height + 1 {
                    radius -= 1;
                } else if radius < foliage_radius {
                    radius += 1;
                }
            }
        }
    }
}

fn place_leaves_row<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    center: BlockPos,
    radius: i32,
    y_offset: i32,
) {
    for x_offset in -radius..=radius {
        for z_offset in -radius..=radius {
            match config.foliage_placer {
                FoliagePlacerConfiguration::Blob { .. } => {
                    if should_skip_blob_leaf(
                        random,
                        x_offset.abs(),
                        y_offset,
                        z_offset.abs(),
                        radius,
                    ) {
                        continue;
                    }
                }
                FoliagePlacerConfiguration::Spruce { .. }
                | FoliagePlacerConfiguration::Pine { .. } => {
                    if should_skip_conifer_leaf(x_offset.abs(), z_offset.abs(), radius) {
                        continue;
                    }
                }
            }
            try_place_leaf(
                world,
                random,
                BlockPos::new(
                    center.x + x_offset,
                    center.y + y_offset,
                    center.z + z_offset,
                ),
                config,
            );
        }
    }
}

fn should_skip_conifer_leaf(abs_x: i32, abs_z: i32, radius: i32) -> bool {
    abs_x == radius && abs_z == radius && radius > 0
}

fn should_skip_blob_leaf(
    random: &mut impl RandomSource,
    abs_x: i32,
    y_offset: i32,
    abs_z: i32,
    radius: i32,
) -> bool {
    abs_x == radius && abs_z == radius && (random.next_int_bound(2) == 0 || y_offset == 0)
}

fn place_log<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
) -> bool {
    if valid_tree_pos(world, pos) {
        world.set_block_world(pos, config.log)
    } else {
        false
    }
}

fn try_place_leaf<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
) -> bool {
    if valid_tree_pos(world, pos) {
        world.set_block_world(pos, config.leaves)
    } else {
        false
    }
}

fn set_dirt_at<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
) -> bool {
    let Some(current) = world.block_at_world(pos) else {
        return false;
    };
    if matches!(current, DIRT | PODZOL) {
        true
    } else {
        world.set_block_world(pos, DIRT)
    }
}

fn can_survive_tree_sapling<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
}

fn valid_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | CAVE_AIR
            | WATER
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | BIRCH_LEAVES
            | SPRUCE_LEAVES
    )
}

fn is_free_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    if valid_tree_pos(world, pos) {
        return true;
    }
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(block_id, OAK_LOG | BIRCH_LOG | SPRUCE_LOG)
}

fn can_replace_tree_block<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | CAVE_AIR
            | WATER
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | OAK_LOG
            | BIRCH_LEAVES
            | BIRCH_LOG
            | SPRUCE_LEAVES
            | SPRUCE_LOG
    )
}
