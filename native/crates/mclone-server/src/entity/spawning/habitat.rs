use mclone_blocks::{BlockFluidKind, block_collision_aabb, block_fluid_kind};
use mclone_core::{BlockPos, BlockStateId};
use mclone_worldgen::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, BIRCH_LEAVES, BIRCH_LOG, DANDELION, DARK_OAK_LEAVES,
    DARK_OAK_LOG, DIRT, FERN, GRASS, GRASS_BLOCK, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LILY_PAD,
    OAK_LEAVES, OAK_LOG, POPPY, RawBlockId, SPRUCE_LEAVES, SPRUCE_LOG, SUGAR_CANE,
    TALL_GRASS_LOWER, TALL_GRASS_UPPER, WATER, WATER_LEVEL_1, WATER_LEVEL_2, WATER_LEVEL_3,
    WATER_LEVEL_4, WATER_LEVEL_5, WATER_LEVEL_6, WATER_LEVEL_7, WATER_LEVEL_8,
    generated_block_state_id,
};
use mclone_worldgen::levelgen::McloneForestEdgeIntentSample;

pub(crate) const WETLAND_HABITAT_RADIUS: i32 = 6;
const WETLAND_HABITAT_MIN_WATER_COLUMNS: u16 = 2;
pub(crate) const FOREST_EDGE_HABITAT_RADIUS: i32 = 6;
pub(crate) const FLOWERING_HABITAT_RADIUS: i32 = 6;
pub(crate) const RABBIT_HABITAT_RADIUS: i32 = 6;
pub(crate) const SQUIRREL_HABITAT_RADIUS: i32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SquirrelRefugeCandidate {
    pub(crate) approach: BlockPos,
    pub(crate) trunk: BlockPos,
    pub(crate) refuge: BlockPos,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SquirrelHabitatSample {
    pub(crate) fitness: HabitatFitness,
    pub(crate) generated_suitability: u16,
    pub(crate) forest_cover: u16,
    pub(crate) woody_columns: u16,
    pub(crate) mast_leaf_blocks: u16,
    pub(crate) open_ground_columns: u16,
    pub(crate) max_floor_step: u8,
    pub(crate) mast_accessible: bool,
    pub(crate) refuge: Option<SquirrelRefugeCandidate>,
    pub(crate) recently_disturbed: bool,
}

impl SquirrelHabitatSample {
    pub(crate) fn suitable(self) -> bool {
        self.generated_suitability >= 90
            && (180..=850).contains(&self.forest_cover)
            && self.woody_columns >= 2
            && self.mast_leaf_blocks >= 4
            && self.open_ground_columns >= 6
            && self.max_floor_step <= 2
            && self.mast_accessible
            && self.refuge.is_some()
            && !self.recently_disturbed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SquirrelHabitatFailure {
    MissingBlockData,
}

pub(crate) fn sample_squirrel_habitat(
    feet: BlockPos,
    generated_suitability: u16,
    forest_cover: u16,
    mast_accessible: bool,
    recently_disturbed: bool,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Result<SquirrelHabitatSample, SquirrelHabitatFailure> {
    let mut sample = SquirrelHabitatSample {
        generated_suitability,
        forest_cover,
        mast_accessible,
        recently_disturbed,
        ..SquirrelHabitatSample::default()
    };
    let mut refuge_candidates = Vec::new();

    for dx in -SQUIRREL_HABITAT_RADIUS..=SQUIRREL_HABITAT_RADIUS {
        for dz in -SQUIRREL_HABITAT_RADIUS..=SQUIRREL_HABITAT_RADIUS {
            let distance = dx.abs() + dz.abs();
            if distance > SQUIRREL_HABITAT_RADIUS {
                continue;
            }
            let column = feet.offset(dx, 0, dz);
            let ground =
                block_at(column.below()).ok_or(SquirrelHabitatFailure::MissingBlockData)?;
            let body = block_at(column).ok_or(SquirrelHabitatFailure::MissingBlockData)?;
            sample.open_ground_columns = sample
                .open_ground_columns
                .saturating_add(u16::from(ground != AIR && body == AIR));

            let mut woody_column = false;
            for dy in -1..=8 {
                let pos = column.offset(0, dy, 0);
                let raw = block_at(pos).ok_or(SquirrelHabitatFailure::MissingBlockData)?;
                woody_column |= is_woody_cover(raw);
                sample.mast_leaf_blocks = sample
                    .mast_leaf_blocks
                    .saturating_add(u16::from(is_mast_leaf(raw)));
                if !is_tree_log(raw) || dy < 0 {
                    continue;
                }
                for (side_x, side_z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let refuge = pos.offset(side_x, 0, side_z);
                    if refuge.y < feet.y + 3
                        || block_at(refuge).ok_or(SquirrelHabitatFailure::MissingBlockData)? != AIR
                    {
                        continue;
                    }
                    let leaf_support = (-1..=1).any(|leaf_dx| {
                        (-1..=1).any(|leaf_dz| {
                            block_at(refuge.offset(leaf_dx, 1, leaf_dz)).is_some_and(is_mast_leaf)
                        })
                    });
                    if !leaf_support {
                        continue;
                    }
                    let approach = BlockPos::new(refuge.x, feet.y, refuge.z);
                    if block_at(approach).ok_or(SquirrelHabitatFailure::MissingBlockData)? == AIR
                        && block_at(approach.below())
                            .ok_or(SquirrelHabitatFailure::MissingBlockData)?
                            != AIR
                    {
                        let approach_distance =
                            (approach.x - feet.x).abs() + (approach.z - feet.z).abs();
                        refuge_candidates.push((
                            approach_distance,
                            refuge.y,
                            refuge.x,
                            refuge.z,
                            SquirrelRefugeCandidate {
                                approach,
                                trunk: pos,
                                refuge,
                            },
                        ));
                    }
                }
            }
            sample.woody_columns = sample.woody_columns.saturating_add(u16::from(woody_column));
        }
    }

    for (dx, dz) in [(3, 0), (-3, 0), (0, 3), (0, -3)] {
        let mut step = 3_u8;
        for dy in [0_i32, 1, -1, 2, -2] {
            let floor = feet.offset(dx, dy - 1, dz);
            if block_at(floor).ok_or(SquirrelHabitatFailure::MissingBlockData)? != AIR {
                step = dy.unsigned_abs().min(3) as u8;
                break;
            }
        }
        sample.max_floor_step = sample.max_floor_step.max(step);
    }
    refuge_candidates
        .sort_unstable_by_key(|candidate| (candidate.0, candidate.1, candidate.2, candidate.3));
    sample.refuge = refuge_candidates.first().map(|candidate| candidate.4);
    sample.fitness = HabitatFitness::from_dimensions(
        score(sample.mast_leaf_blocks, 12),
        score(sample.woody_columns, 8),
        u8::from(sample.refuge.is_some()) * 100,
        score(sample.open_ground_columns, 16),
        100_u8.saturating_sub(sample.max_floor_step.saturating_mul(34)),
    );
    Ok(sample)
}

pub(crate) fn find_squirrel_refuge_candidate(
    feet: BlockPos,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> Option<SquirrelRefugeCandidate> {
    let mut candidates = Vec::new();
    for radius in 1..=SQUIRREL_HABITAT_RADIUS {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() + dz.abs() != radius {
                    continue;
                }
                for dy in 3..=8 {
                    let trunk = feet.offset(dx, dy, dz);
                    if !block_state_at(trunk).is_some_and(is_tree_log_state) {
                        continue;
                    }
                    for (side_x, side_z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let refuge = trunk.offset(side_x, 0, side_z);
                        if !squirrel_space_is_clear(refuge, block_state_at)
                            || !squirrel_leaf_support_exists(refuge, block_state_at)
                        {
                            continue;
                        }
                        let approach = BlockPos::new(refuge.x, feet.y, refuge.z);
                        if squirrel_space_is_clear(approach, block_state_at)
                            && block_state_at(approach.below()).is_some_and(|state| {
                                block_collision_aabb(state, approach.below()).is_some()
                            })
                        {
                            let approach_distance =
                                (approach.x - feet.x).abs() + (approach.z - feet.z).abs();
                            candidates.push((
                                approach_distance,
                                refuge.y,
                                refuge.x,
                                refuge.z,
                                SquirrelRefugeCandidate {
                                    approach,
                                    trunk,
                                    refuge,
                                },
                            ));
                        }
                    }
                }
            }
        }
    }
    candidates
        .into_iter()
        .min_by_key(|candidate| (candidate.0, candidate.1, candidate.2, candidate.3))
        .map(|candidate| candidate.4)
}

pub(crate) fn squirrel_refuge_support_is_valid(
    candidate: SquirrelRefugeCandidate,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    let min_y = candidate.approach.y;
    let max_y = candidate.refuge.y;
    (min_y..=max_y).all(|y| {
        let side = BlockPos::new(candidate.refuge.x, y, candidate.refuge.z);
        let trunk = BlockPos::new(candidate.trunk.x, y, candidate.trunk.z);
        squirrel_space_is_clear(side, block_state_at)
            && block_state_at(trunk).is_some_and(is_tree_log_state)
    }) && squirrel_leaf_support_exists(candidate.refuge, block_state_at)
}

fn squirrel_space_is_clear(
    feet: BlockPos,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    block_state_at(feet).is_some_and(|state| block_collision_aabb(state, feet).is_none())
}

fn squirrel_leaf_support_exists(
    refuge: BlockPos,
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
) -> bool {
    (-1..=1).any(|dx| {
        (-1..=1).any(|dz| block_state_at(refuge.offset(dx, 1, dz)).is_some_and(is_mast_leaf_state))
    })
}

fn is_tree_log_state(state: BlockStateId) -> bool {
    [OAK_LOG, BIRCH_LOG, SPRUCE_LOG, DARK_OAK_LOG, ACACIA_LOG]
        .into_iter()
        .any(|raw| state == generated_block_state_id(raw))
}

fn is_mast_leaf_state(state: BlockStateId) -> bool {
    [OAK_LEAVES, BIRCH_LEAVES, DARK_OAK_LEAVES]
        .into_iter()
        .any(|raw| state == generated_block_state_id(raw))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RabbitHabitatSample {
    pub(crate) fitness: HabitatFitness,
    pub(crate) browse_blocks: u16,
    pub(crate) diggable_banks: u16,
    pub(crate) open_columns: u16,
    pub(crate) grass_floor: bool,
}

impl RabbitHabitatSample {
    pub(crate) const fn suitable(self) -> bool {
        self.grass_floor
            && self.browse_blocks >= 2
            && self.diggable_banks >= 1
            && self.open_columns >= 8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RabbitHabitatFailure {
    MissingBlockData,
}

pub(crate) fn sample_rabbit_habitat(
    feet: BlockPos,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Result<RabbitHabitatSample, RabbitHabitatFailure> {
    let grass_floor =
        block_at(feet.below()).ok_or(RabbitHabitatFailure::MissingBlockData)? == GRASS_BLOCK;
    let mut sample = RabbitHabitatSample {
        grass_floor,
        ..RabbitHabitatSample::default()
    };
    for dx in -RABBIT_HABITAT_RADIUS..=RABBIT_HABITAT_RADIUS {
        for dz in -RABBIT_HABITAT_RADIUS..=RABBIT_HABITAT_RADIUS {
            if dx.abs() + dz.abs() > RABBIT_HABITAT_RADIUS {
                continue;
            }
            let column = feet.offset(dx, 0, dz);
            let foot = block_at(column).ok_or(RabbitHabitatFailure::MissingBlockData)?;
            let head =
                block_at(column.offset(0, 1, 0)).ok_or(RabbitHabitatFailure::MissingBlockData)?;
            sample.open_columns = sample
                .open_columns
                .saturating_add(u16::from(foot == AIR && head == AIR));
            for dy in -1..=2 {
                let raw = block_at(column.offset(0, dy, 0))
                    .ok_or(RabbitHabitatFailure::MissingBlockData)?;
                sample.browse_blocks = sample
                    .browse_blocks
                    .saturating_add(u16::from(is_browse(raw)));
            }
            for dy in -2..=1 {
                let bank = column.offset(0, dy, 0);
                let bank_state = block_at(bank).ok_or(RabbitHabitatFailure::MissingBlockData)?;
                let roof =
                    block_at(bank.offset(0, 1, 0)).ok_or(RabbitHabitatFailure::MissingBlockData)?;
                if !matches!(bank_state, GRASS_BLOCK | DIRT) || !matches!(roof, GRASS_BLOCK | DIRT)
                {
                    continue;
                }
                let threshold =
                    [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .into_iter()
                        .any(|(front_x, front_z)| {
                            let front = bank.offset(front_x, 0, front_z);
                            let rear = bank.offset(-front_x, 0, -front_z);
                            block_at(front) == Some(AIR)
                                && block_at(front.offset(0, 1, 0)) == Some(AIR)
                                && block_at(front.below()).is_some_and(|raw| raw != AIR)
                                && block_at(rear).is_some_and(|raw| raw != AIR)
                        });
                if threshold {
                    sample.diggable_banks = sample.diggable_banks.saturating_add(1);
                    break;
                }
            }
        }
    }
    sample.fitness = HabitatFitness::from_dimensions(
        score(sample.browse_blocks, 8),
        score(sample.diggable_banks, 2),
        u8::from(sample.grass_floor) * 100,
        score(sample.open_columns, 16),
        100,
    );
    Ok(sample)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HabitatFitness {
    pub(crate) forage: u8,
    pub(crate) shelter: u8,
    pub(crate) substrate: u8,
    pub(crate) open_space: u8,
    pub(crate) continuity: u8,
    pub(crate) overall: u8,
}

impl HabitatFitness {
    fn from_dimensions(
        forage: u8,
        shelter: u8,
        substrate: u8,
        open_space: u8,
        continuity: u8,
    ) -> Self {
        Self {
            forage,
            shelter,
            substrate,
            open_space,
            continuity,
            overall: ((u16::from(forage)
                + u16::from(shelter)
                + u16::from(substrate)
                + u16::from(open_space)
                + u16::from(continuity))
                / 5) as u8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ForestEdgeHabitatSample {
    pub(crate) fitness: HabitatFitness,
    pub(crate) generated: McloneForestEdgeIntentSample,
    pub(crate) woody_cover_blocks: u16,
    pub(crate) browse_blocks: u16,
    pub(crate) open_sight_columns: u16,
    pub(crate) nearby_water_columns: u16,
    pub(crate) escape_cover_blocks: u16,
    pub(crate) grass_floor: bool,
    pub(crate) max_floor_step: u8,
    pub(crate) recently_disturbed: bool,
}

impl ForestEdgeHabitatSample {
    pub(crate) fn suitable(self) -> bool {
        self.generated.is_transitional_edge()
            && self.grass_floor
            && self.woody_cover_blocks >= 2
            && self.browse_blocks >= 2
            && self.open_sight_columns >= 4
            && self.escape_cover_blocks >= 1
            && self.max_floor_step <= 2
            && !self.recently_disturbed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ForestEdgeHabitatFailure {
    MissingBlockData,
}

pub(crate) fn sample_forest_edge_habitat(
    feet: BlockPos,
    generated: McloneForestEdgeIntentSample,
    recently_disturbed: bool,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Result<ForestEdgeHabitatSample, ForestEdgeHabitatFailure> {
    let grass_floor =
        block_at(feet.below()).ok_or(ForestEdgeHabitatFailure::MissingBlockData)? == GRASS_BLOCK;
    let mut sample = ForestEdgeHabitatSample {
        fitness: HabitatFitness::default(),
        generated,
        woody_cover_blocks: 0,
        browse_blocks: 0,
        open_sight_columns: 0,
        nearby_water_columns: 0,
        escape_cover_blocks: 0,
        grass_floor,
        max_floor_step: 0,
        recently_disturbed,
    };

    for dx in -FOREST_EDGE_HABITAT_RADIUS..=FOREST_EDGE_HABITAT_RADIUS {
        for dz in -FOREST_EDGE_HABITAT_RADIUS..=FOREST_EDGE_HABITAT_RADIUS {
            if dx.abs() + dz.abs() > FOREST_EDGE_HABITAT_RADIUS {
                continue;
            }
            let mut woody_column = false;
            let mut water_column = false;
            let mut browse_column = false;
            for dy in -2..=6 {
                let raw = block_at(feet.offset(dx, dy, dz))
                    .ok_or(ForestEdgeHabitatFailure::MissingBlockData)?;
                woody_column |= is_woody_cover(raw);
                water_column |= is_water(raw);
                browse_column |= is_browse(raw);
            }
            sample.woody_cover_blocks = sample
                .woody_cover_blocks
                .saturating_add(u16::from(woody_column));
            sample.nearby_water_columns = sample
                .nearby_water_columns
                .saturating_add(u16::from(water_column));
            sample.browse_blocks = sample
                .browse_blocks
                .saturating_add(u16::from(browse_column));

            if dx.abs() + dz.abs() >= 3 {
                let foot = block_at(feet.offset(dx, 0, dz))
                    .ok_or(ForestEdgeHabitatFailure::MissingBlockData)?;
                let head = block_at(feet.offset(dx, 1, dz))
                    .ok_or(ForestEdgeHabitatFailure::MissingBlockData)?;
                sample.open_sight_columns = sample
                    .open_sight_columns
                    .saturating_add(u16::from(foot == AIR && head == AIR));
            }
        }
    }

    for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
        let mut floor_step = None;
        for step in 0..=2 {
            for signed in [step, -step] {
                let floor = feet.offset(dx, signed - 1, dz);
                if block_at(floor).ok_or(ForestEdgeHabitatFailure::MissingBlockData)? == GRASS_BLOCK
                {
                    floor_step = Some(step as u8);
                    break;
                }
            }
            if floor_step.is_some() {
                break;
            }
        }
        sample.max_floor_step = sample.max_floor_step.max(floor_step.unwrap_or(3));
    }

    let cover = generated.cover_direction;
    for distance in 3..=FOREST_EDGE_HABITAT_RADIUS {
        for dy in -1..=5 {
            let pos = feet.offset(
                i32::from(cover.x) * distance,
                dy,
                i32::from(cover.z) * distance,
            );
            if is_woody_cover(block_at(pos).ok_or(ForestEdgeHabitatFailure::MissingBlockData)?) {
                sample.escape_cover_blocks = sample.escape_cover_blocks.saturating_add(1);
            }
        }
    }
    sample.fitness = HabitatFitness::from_dimensions(
        score(sample.browse_blocks, 8),
        score(sample.woody_cover_blocks, 8),
        u8::from(sample.grass_floor) * 100,
        score(sample.open_sight_columns, 12),
        100_u8.saturating_sub(sample.max_floor_step.saturating_mul(34)),
    );
    Ok(sample)
}

fn is_woody_cover(raw: RawBlockId) -> bool {
    matches!(
        raw,
        OAK_LOG
            | OAK_LEAVES
            | BIRCH_LOG
            | BIRCH_LEAVES
            | SPRUCE_LOG
            | SPRUCE_LEAVES
            | DARK_OAK_LOG
            | DARK_OAK_LEAVES
            | ACACIA_LOG
            | ACACIA_LEAVES
    )
}

pub(crate) fn is_tree_log(raw: RawBlockId) -> bool {
    matches!(
        raw,
        OAK_LOG | BIRCH_LOG | SPRUCE_LOG | DARK_OAK_LOG | ACACIA_LOG
    )
}

pub(crate) fn is_mast_leaf(raw: RawBlockId) -> bool {
    matches!(raw, OAK_LEAVES | BIRCH_LEAVES | DARK_OAK_LEAVES)
}

fn is_browse(raw: RawBlockId) -> bool {
    matches!(
        raw,
        GRASS
            | FERN
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | TALL_GRASS_LOWER
            | TALL_GRASS_UPPER
            | DANDELION
            | POPPY
    )
}

fn is_water(raw: RawBlockId) -> bool {
    matches!(
        raw,
        WATER
            | WATER_LEVEL_1
            | WATER_LEVEL_2
            | WATER_LEVEL_3
            | WATER_LEVEL_4
            | WATER_LEVEL_5
            | WATER_LEVEL_6
            | WATER_LEVEL_7
            | WATER_LEVEL_8
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WetlandHabitatSample {
    pub(crate) fitness: HabitatFitness,
    pub(crate) water_columns: u16,
    pub(crate) shallow_water_columns: u16,
    pub(crate) cover_blocks: u16,
    pub(crate) grass_floor: bool,
}

impl WetlandHabitatSample {
    pub(crate) const fn suitable(self) -> bool {
        self.grass_floor
            && self.water_columns >= WETLAND_HABITAT_MIN_WATER_COLUMNS
            && self.shallow_water_columns > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WetlandHabitatFailure {
    MissingBlockData,
}

pub(crate) fn sample_wetland_habitat(
    feet: BlockPos,
    block_state_at: &mut impl FnMut(BlockPos) -> Option<BlockStateId>,
) -> Result<WetlandHabitatSample, WetlandHabitatFailure> {
    let grass = generated_block_state_id(GRASS_BLOCK);
    let grass_floor =
        block_state_at(feet.below()).ok_or(WetlandHabitatFailure::MissingBlockData)? == grass;
    let mut sample = WetlandHabitatSample {
        grass_floor,
        ..WetlandHabitatSample::default()
    };

    for dx in -WETLAND_HABITAT_RADIUS..=WETLAND_HABITAT_RADIUS {
        for dz in -WETLAND_HABITAT_RADIUS..=WETLAND_HABITAT_RADIUS {
            if dx.abs() + dz.abs() > WETLAND_HABITAT_RADIUS {
                continue;
            }
            let mut column_has_water = false;
            let mut column_has_shallow_water = false;
            for dy in -2..=2 {
                let pos = feet.offset(dx, dy, dz);
                let state = block_state_at(pos).ok_or(WetlandHabitatFailure::MissingBlockData)?;
                if is_wetland_cover(state) {
                    sample.cover_blocks = sample.cover_blocks.saturating_add(1);
                }
                if block_fluid_kind(state) != BlockFluidKind::Water {
                    continue;
                }
                column_has_water = true;
                column_has_shallow_water |= (1..=2).any(|depth| {
                    let bed = pos.offset(0, -depth, 0);
                    block_state_at(bed)
                        .and_then(|state| block_collision_aabb(state, bed))
                        .is_some()
                });
            }
            sample.water_columns = sample
                .water_columns
                .saturating_add(u16::from(column_has_water));
            sample.shallow_water_columns = sample
                .shallow_water_columns
                .saturating_add(u16::from(column_has_shallow_water));
        }
    }
    sample.fitness = HabitatFitness::from_dimensions(
        score(sample.cover_blocks, 6),
        score(sample.cover_blocks, 4),
        u8::from(sample.grass_floor) * 100,
        score(sample.shallow_water_columns, 6),
        score(sample.water_columns, 10),
    );
    Ok(sample)
}

fn is_wetland_cover(state: BlockStateId) -> bool {
    matches!(
        state,
        value if value == generated_block_state_id(LILY_PAD)
            || value == generated_block_state_id(SUGAR_CANE)
            || value == generated_block_state_id(GRASS)
            || value == generated_block_state_id(FERN)
            || value == generated_block_state_id(LARGE_FERN_LOWER)
            || value == generated_block_state_id(LARGE_FERN_UPPER)
            || value == generated_block_state_id(TALL_GRASS_LOWER)
            || value == generated_block_state_id(TALL_GRASS_UPPER)
            || value == generated_block_state_id(OAK_LEAVES)
            || value == generated_block_state_id(BIRCH_LEAVES)
            || value == generated_block_state_id(SPRUCE_LEAVES)
            || value == generated_block_state_id(DARK_OAK_LEAVES)
            || value == generated_block_state_id(ACACIA_LEAVES)
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FloweringHabitatSample {
    pub(crate) fitness: HabitatFitness,
    pub(crate) flower_blocks: u16,
    pub(crate) grass_substrate_columns: u16,
    pub(crate) woody_cover_columns: u16,
    pub(crate) open_flight_columns: u16,
    pub(crate) max_floor_step: u8,
}

impl FloweringHabitatSample {
    pub(crate) const fn suitable(self) -> bool {
        self.flower_blocks >= 4
            && self.grass_substrate_columns >= 8
            && self.open_flight_columns >= 8
            && self.max_floor_step <= 2
            && self.fitness.overall >= 45
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FloweringHabitatFailure {
    MissingBlockData,
}

pub(crate) fn sample_flowering_habitat(
    feet: BlockPos,
    block_state_at: &mut impl FnMut(BlockPos) -> Option<BlockStateId>,
) -> Result<FloweringHabitatSample, FloweringHabitatFailure> {
    let grass = generated_block_state_id(GRASS_BLOCK);
    let dandelion = generated_block_state_id(DANDELION);
    let poppy = generated_block_state_id(POPPY);
    let mut sample = FloweringHabitatSample::default();
    for dx in -FLOWERING_HABITAT_RADIUS..=FLOWERING_HABITAT_RADIUS {
        for dz in -FLOWERING_HABITAT_RADIUS..=FLOWERING_HABITAT_RADIUS {
            if dx.abs() + dz.abs() > FLOWERING_HABITAT_RADIUS {
                continue;
            }
            let ground = block_state_at(feet.offset(dx, -1, dz))
                .ok_or(FloweringHabitatFailure::MissingBlockData)?;
            let body = block_state_at(feet.offset(dx, 0, dz))
                .ok_or(FloweringHabitatFailure::MissingBlockData)?;
            let head = block_state_at(feet.offset(dx, 1, dz))
                .ok_or(FloweringHabitatFailure::MissingBlockData)?;
            sample.grass_substrate_columns = sample
                .grass_substrate_columns
                .saturating_add(u16::from(ground == grass));
            sample.flower_blocks = sample
                .flower_blocks
                .saturating_add(u16::from(body == dandelion || body == poppy));
            sample.open_flight_columns = sample.open_flight_columns.saturating_add(u16::from(
                block_collision_aabb(body, feet.offset(dx, 0, dz)).is_none()
                    && block_collision_aabb(head, feet.offset(dx, 1, dz)).is_none(),
            ));
            let mut woody = false;
            for dy in -1..=4 {
                let state = block_state_at(feet.offset(dx, dy, dz))
                    .ok_or(FloweringHabitatFailure::MissingBlockData)?;
                woody |= is_woody_state(state);
            }
            sample.woody_cover_columns =
                sample.woody_cover_columns.saturating_add(u16::from(woody));
        }
    }
    for (dx, dz) in [(3, 0), (-3, 0), (0, 3), (0, -3)] {
        let mut step = 3;
        for dy in [0_i32, 1, -1, 2, -2] {
            if block_state_at(feet.offset(dx, dy - 1, dz))
                .ok_or(FloweringHabitatFailure::MissingBlockData)?
                == grass
            {
                step = dy.unsigned_abs().min(3) as u8;
                break;
            }
        }
        sample.max_floor_step = sample.max_floor_step.max(step);
    }
    sample.fitness = HabitatFitness::from_dimensions(
        score(sample.flower_blocks, 8),
        score(sample.woody_cover_columns, 4),
        score(sample.grass_substrate_columns, 16),
        score(sample.open_flight_columns, 20),
        100_u8.saturating_sub(sample.max_floor_step.saturating_mul(34)),
    );
    Ok(sample)
}

fn is_woody_state(state: BlockStateId) -> bool {
    [
        OAK_LOG,
        OAK_LEAVES,
        BIRCH_LOG,
        BIRCH_LEAVES,
        SPRUCE_LOG,
        SPRUCE_LEAVES,
        DARK_OAK_LOG,
        DARK_OAK_LEAVES,
        ACACIA_LOG,
        ACACIA_LEAVES,
    ]
    .into_iter()
    .any(|raw| state == generated_block_state_id(raw))
}

fn score(value: u16, full: u16) -> u8 {
    (value.saturating_mul(100) / full.max(1)).min(100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{AIR, DIRT, WATER};
    use mclone_worldgen::levelgen::{
        McloneForestDirection, McloneForestEdgeIntentSample, McloneForestIntentSample,
    };

    fn shallow_shore(pos: BlockPos) -> Option<BlockStateId> {
        let raw = if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if (2..=4).contains(&pos.x) && pos.y == 64 {
            WATER
        } else {
            AIR
        };
        Some(generated_block_state_id(raw))
    }

    #[test]
    fn shallow_grassy_shore_is_suitable() {
        let sample = sample_wetland_habitat(BlockPos::new(0, 64, 0), &mut shallow_shore).unwrap();

        assert!(sample.grass_floor);
        assert!(sample.water_columns >= 2);
        assert!(sample.shallow_water_columns >= 2);
        assert!(sample.suitable());
    }

    #[test]
    fn ordinary_bank_vegetation_counts_as_wetland_nest_cover() {
        let sample = sample_wetland_habitat(BlockPos::new(0, 64, 0), &mut |pos| {
            let raw = if pos.y <= 62 {
                DIRT
            } else if pos.y == 63 {
                GRASS_BLOCK
            } else if (2..=4).contains(&pos.x) && pos.y == 64 {
                WATER
            } else if pos == BlockPos::new(1, 64, 2) {
                FERN
            } else if pos == BlockPos::new(0, 65, 3) {
                OAK_LEAVES
            } else {
                AIR
            };
            Some(generated_block_state_id(raw))
        })
        .unwrap();

        assert!(sample.suitable());
        assert_eq!(sample.cover_blocks, 2);
    }

    #[test]
    fn dry_deep_and_missing_sites_are_rejected() {
        let dry = sample_wetland_habitat(BlockPos::new(0, 64, 0), &mut |pos| {
            Some(generated_block_state_id(if pos.y == 63 {
                GRASS_BLOCK
            } else {
                AIR
            }))
        })
        .unwrap();
        assert!(!dry.suitable());

        let deep = sample_wetland_habitat(BlockPos::new(0, 68, 0), &mut |pos| {
            Some(generated_block_state_id(if pos.y <= 63 {
                DIRT
            } else if pos.y == 67 && !(2..=4).contains(&pos.x) {
                GRASS_BLOCK
            } else if (2..=4).contains(&pos.x) && (64..=68).contains(&pos.y) {
                WATER
            } else {
                AIR
            }))
        })
        .unwrap();
        assert_eq!(deep.shallow_water_columns, 0);
        assert!(!deep.suitable());

        assert_eq!(
            sample_wetland_habitat(BlockPos::new(0, 64, 0), &mut |_| None),
            Err(WetlandHabitatFailure::MissingBlockData)
        );
    }

    #[test]
    fn forest_edge_combines_generated_intent_with_live_cover_and_browse() {
        let generated = McloneForestEdgeIntentSample {
            local: McloneForestIntentSample {
                coverage: 0.42,
                ..McloneForestIntentSample::EMPTY
            },
            nearby_min_coverage: 0.16,
            nearby_max_coverage: 0.72,
            edge_contrast: 0.56,
            clearing_direction: McloneForestDirection { x: -1, z: 0 },
            cover_direction: McloneForestDirection { x: 1, z: 0 },
        };
        let mut edge = |pos: BlockPos| {
            Some(if pos.y <= 62 {
                DIRT
            } else if pos.y == 63 {
                GRASS_BLOCK
            } else if pos.y == 64 && (pos.x + pos.z).rem_euclid(5) == 0 {
                GRASS
            } else if pos.y == 66 {
                OAK_LEAVES
            } else {
                AIR
            })
        };
        let sample =
            sample_forest_edge_habitat(BlockPos::new(0, 64, 0), generated, false, &mut edge)
                .unwrap();

        assert!(sample.suitable());
        assert!(sample.woody_cover_blocks > 0);
        assert!(sample.browse_blocks > 0);
        assert!(sample.open_sight_columns > 0);
        assert!(sample.escape_cover_blocks > 0);

        let disturbed =
            sample_forest_edge_habitat(BlockPos::new(0, 64, 0), generated, true, &mut edge)
                .unwrap();
        assert!(!disturbed.suitable());
    }

    #[test]
    fn flowering_habitat_is_driven_by_live_flowers_and_clear_air() {
        let flowering = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y <= 62 {
                DIRT
            } else if pos.y == 63 {
                GRASS_BLOCK
            } else if pos.y == 64 && (pos.x + pos.z).rem_euclid(3) == 0 {
                DANDELION
            } else if pos.y == 66 && pos.x == 5 {
                OAK_LOG
            } else {
                AIR
            }))
        };
        let mut flowering_sample = flowering;
        let sample =
            sample_flowering_habitat(BlockPos::new(0, 64, 0), &mut flowering_sample).unwrap();
        assert!(sample.suitable());
        assert!(sample.flower_blocks >= 4);
        assert!(sample.fitness.forage > 0);

        let sparse = sample_flowering_habitat(BlockPos::new(0, 64, 0), &mut |pos| {
            flowering(pos).map(|state| {
                if state == generated_block_state_id(DANDELION) {
                    generated_block_state_id(AIR)
                } else {
                    state
                }
            })
        })
        .unwrap();
        assert!(!sparse.suitable());
        assert_eq!(sparse.fitness.forage, 0);
    }

    #[test]
    fn rabbit_habitat_requires_browse_and_a_real_soil_bank() {
        let mut habitat = |pos: BlockPos| {
            Some(if pos.y <= 62 {
                DIRT
            } else if pos.y == 63 {
                GRASS_BLOCK
            } else if (pos.x, pos.y, pos.z) == (3, 64, 0)
                || (pos.x, pos.y, pos.z) == (3, 65, 0)
                || (pos.x, pos.y, pos.z) == (4, 64, 0)
            {
                DIRT
            } else if pos.y == 64 && (pos.x + pos.z).rem_euclid(4) == 0 {
                GRASS
            } else {
                AIR
            })
        };
        let sample = sample_rabbit_habitat(BlockPos::new(0, 64, 0), &mut habitat).unwrap();
        assert!(sample.suitable());
        assert!(sample.browse_blocks >= 2);
        assert!(sample.diggable_banks >= 1);

        let flat = sample_rabbit_habitat(BlockPos::new(0, 64, 0), &mut |pos| {
            Some(if pos.y == 63 {
                GRASS_BLOCK
            } else if pos.y == 64 && (pos.x + pos.z).rem_euclid(4) == 0 {
                GRASS
            } else {
                AIR
            })
        })
        .unwrap();
        assert!(!flat.suitable());
        assert_eq!(flat.diggable_banks, 0);
    }

    fn squirrel_edge(pos: BlockPos) -> Option<RawBlockId> {
        Some(if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos.x == 3 && pos.z == 0 && (64..=68).contains(&pos.y) {
            OAK_LOG
        } else if pos.y == 68 && (pos.x - 3).abs() <= 2 && pos.z.abs() <= 2 && pos.x != 3 {
            OAK_LEAVES
        } else {
            AIR
        })
    }

    #[test]
    fn squirrel_habitat_requires_edge_mast_and_a_supported_refuge() {
        let suitable = sample_squirrel_habitat(
            BlockPos::new(0, 64, 0),
            420,
            480,
            true,
            false,
            &mut squirrel_edge,
        )
        .unwrap();
        assert!(suitable.suitable());
        assert!(suitable.mast_leaf_blocks >= 4);
        assert_eq!(suitable.refuge.unwrap().trunk.x, 3);

        let no_mast = sample_squirrel_habitat(
            BlockPos::new(0, 64, 0),
            420,
            480,
            false,
            false,
            &mut squirrel_edge,
        )
        .unwrap();
        assert!(!no_mast.suitable());

        let dense = sample_squirrel_habitat(
            BlockPos::new(0, 64, 0),
            420,
            920,
            true,
            false,
            &mut squirrel_edge,
        )
        .unwrap();
        assert!(!dense.suitable());

        let disturbed = sample_squirrel_habitat(
            BlockPos::new(0, 64, 0),
            420,
            480,
            true,
            true,
            &mut squirrel_edge,
        )
        .unwrap();
        assert!(!disturbed.suitable());

        let open_plain =
            sample_squirrel_habitat(BlockPos::new(0, 64, 0), 420, 480, true, false, &mut |pos| {
                Some(if pos.y == 63 { GRASS_BLOCK } else { AIR })
            })
            .unwrap();
        assert!(!open_plain.suitable());
        assert!(open_plain.refuge.is_none());

        assert_eq!(
            sample_squirrel_habitat(BlockPos::new(0, 64, 0), 420, 480, true, false, &mut |_| {
                None
            },),
            Err(SquirrelHabitatFailure::MissingBlockData)
        );

        let candidate = find_squirrel_refuge_candidate(BlockPos::new(0, 64, 0), &|pos| {
            squirrel_edge(pos).map(generated_block_state_id)
        })
        .expect("supported tree should expose one refuge route");
        assert!(squirrel_refuge_support_is_valid(candidate, &|pos| {
            squirrel_edge(pos).map(generated_block_state_id)
        }));
        assert!(!squirrel_refuge_support_is_valid(candidate, &|pos| {
            squirrel_edge(pos).map(|raw| {
                generated_block_state_id(if pos == BlockPos::new(3, 65, 0) {
                    AIR
                } else {
                    raw
                })
            })
        }));
    }
}
