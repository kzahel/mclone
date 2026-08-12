use mclone_blocks::{BlockFluidKind, block_collision_aabb, block_fluid_kind};
use mclone_core::{BlockPos, BlockStateId};
use mclone_worldgen::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, BIRCH_LEAVES, BIRCH_LOG, DANDELION, DARK_OAK_LEAVES,
    DARK_OAK_LOG, FERN, GRASS, GRASS_BLOCK, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LILY_PAD,
    OAK_LEAVES, OAK_LOG, POPPY, RawBlockId, SPRUCE_LEAVES, SPRUCE_LOG, SUGAR_CANE,
    TALL_GRASS_LOWER, TALL_GRASS_UPPER, WATER, WATER_LEVEL_1, WATER_LEVEL_2, WATER_LEVEL_3,
    WATER_LEVEL_4, WATER_LEVEL_5, WATER_LEVEL_6, WATER_LEVEL_7, WATER_LEVEL_8,
    generated_block_state_id,
};
use mclone_worldgen::levelgen::McloneForestEdgeIntentSample;

pub(crate) const WETLAND_HABITAT_RADIUS: i32 = 6;
const WETLAND_HABITAT_MIN_WATER_COLUMNS: u16 = 2;
pub(crate) const FOREST_EDGE_HABITAT_RADIUS: i32 = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ForestEdgeHabitatSample {
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
    let lily_pad = generated_block_state_id(LILY_PAD);
    let sugar_cane = generated_block_state_id(SUGAR_CANE);
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
                if state == lily_pad || state == sugar_cane {
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
    Ok(sample)
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
}
