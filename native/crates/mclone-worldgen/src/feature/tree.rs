use crate::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, ALLIUM, AZURE_BLUET, BIRCH_LEAVES, BIRCH_LOG, CAVE_AIR,
    CORNFLOWER, DANDELION, DARK_OAK_LEAVES, DARK_OAK_LOG, DEAD_BUSH, DIRT, FERN, GLOW_LICHEN,
    GRASS, GRASS_BLOCK, JUNGLE_LEAVES, JUNGLE_LOG, LARGE_FERN_LOWER, LARGE_FERN_UPPER,
    LILY_OF_THE_VALLEY, MYCELIUM, OAK_LEAVES, OAK_LOG, ORANGE_TULIP, OXEYE_DAISY, PINK_TULIP,
    PODZOL, POPPY, RED_TULIP, SPRUCE_LEAVES, SPRUCE_LOG, WATER, WHITE_TULIP,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{
    BasicTreeConfiguration, FeatureWorld, FoliagePlacerConfiguration, TreeConfiguration,
    TrunkPlacerConfiguration, project_to_surface,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoliageAttachment {
    pos: BlockPos,
    radius_offset: i32,
    double_trunk: bool,
}

impl FoliageAttachment {
    const fn new(pos: BlockPos, radius_offset: i32, double_trunk: bool) -> Self {
        Self {
            pos,
            radius_offset,
            double_trunk,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FancyFoliageCoords {
    attachment: FoliageAttachment,
    branch_base_y: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TreePlacementBlocks {
    trunks: Vec<BlockPos>,
    leaves: Vec<BlockPos>,
}

impl TreePlacementBlocks {
    fn record_trunk(&mut self, pos: BlockPos) {
        if !self.trunks.contains(&pos) {
            self.trunks.push(pos);
        }
    }

    fn record_leaf(&mut self, pos: BlockPos) {
        if !self.leaves.contains(&pos) {
            self.leaves.push(pos);
        }
    }

    fn has_tree_blocks(&self) -> bool {
        !self.trunks.is_empty() || !self.leaves.is_empty()
    }
}

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
    if max_free_tree_height < tree_height
        && !matches!(
            config.minimum_size.min_clipped_height,
            Some(min_clipped_height) if max_free_tree_height >= min_clipped_height
        )
    {
        return false;
    }

    let mut placement = TreePlacementBlocks::default();
    let foliage_attachments = place_trunk(
        world,
        random,
        base,
        max_free_tree_height,
        config,
        &mut placement,
    );
    for foliage_attachment in foliage_attachments {
        create_foliage(
            world,
            random,
            config,
            foliage_attachment,
            foliage_height,
            foliage_radius,
            &mut placement,
        );
    }
    if placement.has_tree_blocks() {
        apply_tree_decorators(world, random, config, &placement);
    }
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

fn place_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> Vec<FoliageAttachment> {
    match config.trunk_placer {
        TrunkPlacerConfiguration::Straight(_) => {
            place_straight_trunk(world, random, base, height, config, placement)
        }
        TrunkPlacerConfiguration::Fancy(_) => {
            place_fancy_trunk(world, random, base, height, config, placement)
        }
        TrunkPlacerConfiguration::Forking(_) => {
            place_forking_trunk(world, random, base, height, config, placement)
        }
        TrunkPlacerConfiguration::DarkOak(_) => {
            place_dark_oak_trunk(world, random, base, height, config, placement)
        }
    }
}

fn place_straight_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> Vec<FoliageAttachment> {
    set_dirt_at(world, random, BlockPos::new(base.x, base.y - 1, base.z));

    for y_offset in 0..height {
        place_log(
            world,
            random,
            BlockPos::new(base.x, base.y + y_offset, base.z),
            config,
            placement,
        );
    }
    vec![FoliageAttachment::new(
        BlockPos::new(base.x, base.y + height, base.z),
        0,
        false,
    )]
}

fn place_fancy_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> Vec<FoliageAttachment> {
    let height_with_crown = height + 2;
    let trunk_height = (f64::from(height_with_crown) * 0.618).floor() as i32;
    set_dirt_at(world, random, BlockPos::new(base.x, base.y - 1, base.z));

    let cluster_count =
        1.min((1.382 + (f64::from(height_with_crown) / 13.0).powi(2)).floor() as i32);
    let branch_base_y = base.y + trunk_height;
    let mut layer = height_with_crown - 5;
    let mut foliage_coords = Vec::new();
    foliage_coords.push(FancyFoliageCoords {
        attachment: FoliageAttachment::new(BlockPos::new(base.x, base.y + layer, base.z), 0, false),
        branch_base_y,
    });

    while layer >= 0 {
        let shape = fancy_tree_shape(height_with_crown, layer);
        if shape >= 0.0 {
            for _ in 0..cluster_count {
                let distance = f64::from(shape) * (f64::from(random.next_float()) + 0.328);
                let angle = f64::from(random.next_float() * 2.0) * std::f64::consts::PI;
                let x_offset = distance * angle.sin() + 0.5;
                let z_offset = distance * angle.cos() + 0.5;
                let foliage_pos = offset_double(base, x_offset, f64::from(layer - 1), z_offset);
                let foliage_top = BlockPos::new(foliage_pos.x, foliage_pos.y + 5, foliage_pos.z);
                if make_limb(
                    world,
                    random,
                    foliage_pos,
                    foliage_top,
                    false,
                    config,
                    placement,
                ) {
                    let x_delta = base.x - foliage_pos.x;
                    let z_delta = base.z - foliage_pos.z;
                    let branch_base = f64::from(foliage_pos.y)
                        - f64::from(x_delta * x_delta + z_delta * z_delta).sqrt() * 0.381;
                    let branch_y = if branch_base > f64::from(branch_base_y) {
                        branch_base_y
                    } else {
                        branch_base as i32
                    };
                    let branch_base_pos = BlockPos::new(base.x, branch_y, base.z);
                    if make_limb(
                        world,
                        random,
                        branch_base_pos,
                        foliage_pos,
                        false,
                        config,
                        placement,
                    ) {
                        foliage_coords.push(FancyFoliageCoords {
                            attachment: FoliageAttachment::new(foliage_pos, 0, false),
                            branch_base_y: branch_y,
                        });
                    }
                }
            }
        }
        layer -= 1;
    }

    make_limb(
        world,
        random,
        base,
        BlockPos::new(base.x, base.y + trunk_height, base.z),
        true,
        config,
        placement,
    );
    for coords in &foliage_coords {
        let branch_base = BlockPos::new(base.x, coords.branch_base_y, base.z);
        if branch_base != coords.attachment.pos
            && trim_fancy_branches(height_with_crown, coords.branch_base_y - base.y)
        {
            make_limb(
                world,
                random,
                branch_base,
                coords.attachment.pos,
                true,
                config,
                placement,
            );
        }
    }

    foliage_coords
        .into_iter()
        .filter(|coords| trim_fancy_branches(height_with_crown, coords.branch_base_y - base.y))
        .map(|coords| coords.attachment)
        .collect()
}

fn place_forking_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> Vec<FoliageAttachment> {
    set_dirt_at(world, random, BlockPos::new(base.x, base.y - 1, base.z));

    let mut attachments = Vec::new();
    let first_step = random_horizontal_step(random);
    let bend_start = height - random.next_int_bound(4) - 1;
    let mut bend_steps = 3 - random.next_int_bound(3);
    let mut trunk_x = base.x;
    let mut trunk_z = base.z;
    let mut foliage_y = 0;

    for y_offset in 0..height {
        let y = base.y + y_offset;
        if y_offset >= bend_start && bend_steps > 0 {
            trunk_x += first_step.0;
            trunk_z += first_step.1;
            bend_steps -= 1;
        }

        if place_log(
            world,
            random,
            BlockPos::new(trunk_x, y, trunk_z),
            config,
            placement,
        ) {
            foliage_y = y + 1;
        }
    }

    attachments.push(FoliageAttachment::new(
        BlockPos::new(trunk_x, foliage_y, trunk_z),
        1,
        false,
    ));

    let second_step = random_horizontal_step(random);
    if second_step != first_step {
        let fork_start = bend_start - random.next_int_bound(2) - 1;
        let mut fork_steps = 1 + random.next_int_bound(3);
        let mut fork_x = base.x;
        let mut fork_z = base.z;
        foliage_y = 0;

        let mut y_offset = fork_start;
        while y_offset < height && fork_steps > 0 {
            if y_offset >= 1 {
                let y = base.y + y_offset;
                fork_x += second_step.0;
                fork_z += second_step.1;
                if place_log(
                    world,
                    random,
                    BlockPos::new(fork_x, y, fork_z),
                    config,
                    placement,
                ) {
                    foliage_y = y + 1;
                }
            }

            fork_steps -= 1;
            y_offset += 1;
        }

        if foliage_y > 1 {
            attachments.push(FoliageAttachment::new(
                BlockPos::new(fork_x, foliage_y, fork_z),
                0,
                false,
            ));
        }
    }

    attachments
}

fn place_dark_oak_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> Vec<FoliageAttachment> {
    let below = BlockPos::new(base.x, base.y - 1, base.z);
    set_dirt_at(world, random, below);
    set_dirt_at(world, random, BlockPos::new(below.x + 1, below.y, below.z));
    set_dirt_at(world, random, BlockPos::new(below.x, below.y, below.z + 1));
    set_dirt_at(
        world,
        random,
        BlockPos::new(below.x + 1, below.y, below.z + 1),
    );

    let (step_x, step_z) = random_horizontal_step(random);
    let bend_start = height - random.next_int_bound(4);
    let mut bend_steps = 2 - random.next_int_bound(3);
    let mut trunk_x = base.x;
    let mut trunk_z = base.z;
    let top_y = base.y + height - 1;

    for y_offset in 0..height {
        if y_offset >= bend_start && bend_steps > 0 {
            trunk_x += step_x;
            trunk_z += step_z;
            bend_steps -= 1;
        }

        let y = base.y + y_offset;
        let trunk = BlockPos::new(trunk_x, y, trunk_z);
        if is_air_or_leaves(world, trunk) {
            place_log(world, random, trunk, config, placement);
            place_log(
                world,
                random,
                BlockPos::new(trunk.x + 1, trunk.y, trunk.z),
                config,
                placement,
            );
            place_log(
                world,
                random,
                BlockPos::new(trunk.x, trunk.y, trunk.z + 1),
                config,
                placement,
            );
            place_log(
                world,
                random,
                BlockPos::new(trunk.x + 1, trunk.y, trunk.z + 1),
                config,
                placement,
            );
        }
    }

    let mut attachments = vec![FoliageAttachment::new(
        BlockPos::new(trunk_x, top_y, trunk_z),
        0,
        true,
    )];
    for x_offset in -1..=2 {
        for z_offset in -1..=2 {
            if (x_offset < 0 || x_offset > 1 || z_offset < 0 || z_offset > 1)
                && random.next_int_bound(3) <= 0
            {
                let branch_height = random.next_int_bound(3) + 2;
                for y_offset in 0..branch_height {
                    place_log(
                        world,
                        random,
                        BlockPos::new(base.x + x_offset, top_y - y_offset - 1, base.z + z_offset),
                        config,
                        placement,
                    );
                }
                attachments.push(FoliageAttachment::new(
                    BlockPos::new(trunk_x + x_offset, top_y, trunk_z + z_offset),
                    0,
                    false,
                ));
            }
        }
    }

    attachments
}

fn create_foliage<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    attachment: FoliageAttachment,
    foliage_height: i32,
    foliage_radius: i32,
    placement: &mut TreePlacementBlocks,
) {
    let offset = config.foliage_placer.offset(random);
    match config.foliage_placer {
        FoliagePlacerConfiguration::Blob { .. } => {
            for y_offset in ((offset - foliage_height)..=offset).rev() {
                let radius = (foliage_radius + attachment.radius_offset - 1 - y_offset / 2).max(0);
                place_leaves_row(
                    world, random, config, attachment, radius, y_offset, placement,
                );
            }
        }
        FoliagePlacerConfiguration::Fancy { .. } => {
            for y_offset in ((offset - foliage_height)..=offset).rev() {
                let radius = foliage_radius
                    + i32::from(y_offset != offset && y_offset != offset - foliage_height);
                place_leaves_row(
                    world, random, config, attachment, radius, y_offset, placement,
                );
            }
        }
        FoliagePlacerConfiguration::Spruce { .. } => {
            let mut radius = random.next_int_bound(2);
            let mut radius_limit = 1;
            let mut reset_radius = 0;

            for y_offset in (-(foliage_height)..=offset).rev() {
                place_leaves_row(
                    world, random, config, attachment, radius, y_offset, placement,
                );
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
                place_leaves_row(
                    world, random, config, attachment, radius, y_offset, placement,
                );
                if radius >= 1 && y_offset == offset - foliage_height + 1 {
                    radius -= 1;
                } else if radius < foliage_radius {
                    radius += 1;
                }
            }
        }
        FoliagePlacerConfiguration::Acacia { .. } => {
            let attachment = FoliageAttachment::new(
                BlockPos::new(
                    attachment.pos.x,
                    attachment.pos.y + offset,
                    attachment.pos.z,
                ),
                attachment.radius_offset,
                attachment.double_trunk,
            );
            place_leaves_row(
                world,
                random,
                config,
                attachment,
                foliage_radius + attachment.radius_offset,
                -1 - foliage_height,
                placement,
            );
            place_leaves_row(
                world,
                random,
                config,
                attachment,
                foliage_radius - 1,
                -foliage_height,
                placement,
            );
            place_leaves_row(
                world,
                random,
                config,
                attachment,
                foliage_radius + attachment.radius_offset - 1,
                0,
                placement,
            );
        }
        FoliagePlacerConfiguration::DarkOak { .. } => {
            if attachment.double_trunk {
                place_leaves_row(
                    world,
                    random,
                    config,
                    attachment,
                    foliage_radius + 2,
                    offset - 1,
                    placement,
                );
                place_leaves_row(
                    world,
                    random,
                    config,
                    attachment,
                    foliage_radius + 3,
                    offset,
                    placement,
                );
                place_leaves_row(
                    world,
                    random,
                    config,
                    attachment,
                    foliage_radius + 2,
                    offset + 1,
                    placement,
                );
                if random.next_boolean() {
                    place_leaves_row(
                        world,
                        random,
                        config,
                        attachment,
                        foliage_radius,
                        offset + 2,
                        placement,
                    );
                }
            } else {
                place_leaves_row(
                    world,
                    random,
                    config,
                    attachment,
                    foliage_radius + 2,
                    offset - 1,
                    placement,
                );
                place_leaves_row(
                    world,
                    random,
                    config,
                    attachment,
                    foliage_radius + 1,
                    offset,
                    placement,
                );
            }
        }
    }
}

fn place_leaves_row<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    attachment: FoliageAttachment,
    radius: i32,
    y_offset: i32,
    placement: &mut TreePlacementBlocks,
) {
    let double_trunk_width = i32::from(attachment.double_trunk);
    for x_offset in -radius..=radius + double_trunk_width {
        for z_offset in -radius..=radius + double_trunk_width {
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
                FoliagePlacerConfiguration::Fancy { .. } => {
                    if should_skip_fancy_leaf(x_offset, z_offset, radius, attachment.double_trunk) {
                        continue;
                    }
                }
                FoliagePlacerConfiguration::Spruce { .. }
                | FoliagePlacerConfiguration::Pine { .. } => {
                    if should_skip_conifer_leaf(x_offset.abs(), z_offset.abs(), radius) {
                        continue;
                    }
                }
                FoliagePlacerConfiguration::Acacia { .. } => {
                    if should_skip_acacia_leaf(
                        x_offset,
                        y_offset,
                        z_offset,
                        radius,
                        attachment.double_trunk,
                    ) {
                        continue;
                    }
                }
                FoliagePlacerConfiguration::DarkOak { .. } => {
                    if should_skip_dark_oak_leaf(
                        x_offset,
                        y_offset,
                        z_offset,
                        radius,
                        attachment.double_trunk,
                    ) {
                        continue;
                    }
                }
            }
            try_place_leaf(
                world,
                random,
                BlockPos::new(
                    attachment.pos.x + x_offset,
                    attachment.pos.y + y_offset,
                    attachment.pos.z + z_offset,
                ),
                config,
                placement,
            );
        }
    }
}

fn signed_leaf_offsets_abs(x_offset: i32, z_offset: i32, double_trunk: bool) -> (i32, i32) {
    if double_trunk {
        (
            x_offset.abs().min((x_offset - 1).abs()),
            z_offset.abs().min((z_offset - 1).abs()),
        )
    } else {
        (x_offset.abs(), z_offset.abs())
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

fn should_skip_fancy_leaf(x_offset: i32, z_offset: i32, radius: i32, double_trunk: bool) -> bool {
    let (abs_x, abs_z) = signed_leaf_offsets_abs(x_offset, z_offset, double_trunk);
    let x = abs_x as f32 + 0.5;
    let z = abs_z as f32 + 0.5;
    x * x + z * z > (radius * radius) as f32
}

fn should_skip_acacia_leaf(
    x_offset: i32,
    y_offset: i32,
    z_offset: i32,
    radius: i32,
    double_trunk: bool,
) -> bool {
    let (abs_x, abs_z) = signed_leaf_offsets_abs(x_offset, z_offset, double_trunk);
    if y_offset == 0 {
        (abs_x > 1 || abs_z > 1) && abs_x != 0 && abs_z != 0
    } else {
        abs_x == radius && abs_z == radius && radius > 0
    }
}

fn should_skip_dark_oak_leaf(
    x_offset: i32,
    y_offset: i32,
    z_offset: i32,
    radius: i32,
    double_trunk: bool,
) -> bool {
    if y_offset == 0
        && double_trunk
        && (x_offset == -radius || x_offset >= radius)
        && (z_offset == -radius || z_offset >= radius)
    {
        return true;
    }

    let (abs_x, abs_z) = signed_leaf_offsets_abs(x_offset, z_offset, double_trunk);

    if y_offset == -1 && !double_trunk {
        abs_x == radius && abs_z == radius
    } else {
        y_offset == 1 && abs_x + abs_z > radius * 2 - 2
    }
}

fn make_limb<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    from: BlockPos,
    to: BlockPos,
    place: bool,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> bool {
    if !place && from == to {
        return true;
    }

    let delta = BlockPos::new(to.x - from.x, to.y - from.y, to.z - from.z);
    let steps = delta.x.abs().max(delta.y.abs()).max(delta.z.abs());
    if steps == 0 {
        if place {
            place_log(world, random, from, config, placement);
        }
        return true;
    }

    let step_x = delta.x as f32 / steps as f32;
    let step_y = delta.y as f32 / steps as f32;
    let step_z = delta.z as f32 / steps as f32;
    for step in 0..=steps {
        let pos = offset_double(
            from,
            f64::from(0.5_f32 + step as f32 * step_x),
            f64::from(0.5_f32 + step as f32 * step_y),
            f64::from(0.5_f32 + step as f32 * step_z),
        );
        if place {
            place_log(world, random, pos, config, placement);
        } else if !is_free_tree_pos(world, pos) {
            return false;
        }
    }

    true
}

fn offset_double(pos: BlockPos, x: f64, y: f64, z: f64) -> BlockPos {
    BlockPos::new(
        (f64::from(pos.x) + x).floor() as i32,
        (f64::from(pos.y) + y).floor() as i32,
        (f64::from(pos.z) + z).floor() as i32,
    )
}

fn fancy_tree_shape(height: i32, layer: i32) -> f32 {
    if (layer as f32) < (height as f32) * 0.3 {
        return -1.0;
    }

    let half_height = height as f32 / 2.0;
    let y_delta = half_height - layer as f32;
    let mut radius = (half_height * half_height - y_delta * y_delta).sqrt();
    if y_delta == 0.0 {
        radius = half_height;
    } else if y_delta.abs() >= half_height {
        return 0.0;
    }
    radius * 0.5
}

fn trim_fancy_branches(height: i32, branch_base_offset: i32) -> bool {
    (branch_base_offset as f32) >= (height as f32) * 0.2
}

fn apply_tree_decorators<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    placement: &TreePlacementBlocks,
) {
    if let Some(probability) = config.beehive_probability {
        apply_beehive_decorator(world, random, probability, placement);
    }
}

fn apply_beehive_decorator<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    probability: f32,
    placement: &TreePlacementBlocks,
) {
    if random.next_float() >= probability || placement.trunks.is_empty() {
        return;
    }

    let (dx, dz) = match random.next_int_bound(3) {
        0 => (-1, 0),
        1 => (1, 0),
        _ => (0, 1),
    };

    let mut trunks = placement.trunks.clone();
    let mut leaves = placement.leaves.clone();
    trunks.sort_by_key(|pos| pos.y);
    leaves.sort_by_key(|pos| pos.y);

    let target_y = if let Some(first_leaf) = leaves.first() {
        (first_leaf.y - 1).max(trunks[0].y)
    } else {
        (trunks[0].y + 1 + random.next_int_bound(3)).min(trunks[trunks.len() - 1].y)
    };

    let trunks_at_y = trunks
        .iter()
        .copied()
        .filter(|pos| pos.y == target_y)
        .collect::<Vec<_>>();
    if trunks_at_y.is_empty() {
        return;
    }

    let trunk = trunks_at_y[random.next_int_bound(trunks_at_y.len() as i32) as usize];
    let hive_pos = BlockPos::new(trunk.x + dx, trunk.y, trunk.z + dz);
    let hive_south = BlockPos::new(hive_pos.x, hive_pos.y, hive_pos.z + 1);
    if is_air_block(world, hive_pos) && is_air_block(world, hive_south) {
        let bee_count = 2 + random.next_int_bound(2);
        for _ in 0..bee_count {
            let _release_tick = random.next_int_bound(599);
        }
    }
}

fn is_air_block<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    matches!(world.block_at_world(pos), Some(AIR | CAVE_AIR))
}

fn is_air_or_leaves<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    world
        .block_at_world(pos)
        .is_some_and(|block| matches!(block, AIR | CAVE_AIR) || is_tree_leaf(block))
}

fn place_log<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> bool {
    if valid_tree_pos(world, pos) {
        if world.set_block_world(pos, config.log) {
            placement.record_trunk(pos);
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn try_place_leaf<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
    placement: &mut TreePlacementBlocks,
) -> bool {
    if valid_tree_pos(world, pos) {
        if world.set_block_world(pos, config.leaves) {
            placement.record_leaf(pos);
            true
        } else {
            false
        }
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
            | ALLIUM
            | AZURE_BLUET
            | RED_TULIP
            | ORANGE_TULIP
            | WHITE_TULIP
            | PINK_TULIP
            | OXEYE_DAISY
            | CORNFLOWER
            | LILY_OF_THE_VALLEY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | BIRCH_LEAVES
            | SPRUCE_LEAVES
            | DARK_OAK_LEAVES
            | ACACIA_LEAVES
            | JUNGLE_LEAVES
    )
}

fn is_tree_leaf(block_id: crate::block::RawBlockId) -> bool {
    matches!(
        block_id,
        OAK_LEAVES | BIRCH_LEAVES | SPRUCE_LEAVES | DARK_OAK_LEAVES | ACACIA_LEAVES | JUNGLE_LEAVES
    )
}

fn is_free_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    if valid_tree_pos(world, pos) {
        return true;
    }
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        OAK_LOG | BIRCH_LOG | SPRUCE_LOG | DARK_OAK_LOG | ACACIA_LOG | JUNGLE_LOG
    )
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
            | ALLIUM
            | AZURE_BLUET
            | RED_TULIP
            | ORANGE_TULIP
            | WHITE_TULIP
            | PINK_TULIP
            | OXEYE_DAISY
            | CORNFLOWER
            | LILY_OF_THE_VALLEY
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
            | DARK_OAK_LEAVES
            | DARK_OAK_LOG
            | ACACIA_LEAVES
            | ACACIA_LOG
            | JUNGLE_LEAVES
            | JUNGLE_LOG
    )
}

fn random_horizontal_step(random: &mut impl RandomSource) -> (i32, i32) {
    match random.next_int_bound(4) {
        0 => (0, -1),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (1, 0),
    }
}
