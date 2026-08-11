use mclone_blocks::{BlockFluidKind, block_collision_aabb, block_fluid_kind};
use mclone_core::{BlockPos, BlockStateId};
use mclone_worldgen::block::{GRASS_BLOCK, LILY_PAD, SUGAR_CANE, generated_block_state_id};

pub(crate) const WETLAND_HABITAT_RADIUS: i32 = 6;
const WETLAND_HABITAT_MIN_WATER_COLUMNS: u16 = 2;

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
    block_state_at: &impl Fn(BlockPos) -> Option<BlockStateId>,
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
        let sample = sample_wetland_habitat(BlockPos::new(0, 64, 0), &shallow_shore).unwrap();

        assert!(sample.grass_floor);
        assert!(sample.water_columns >= 2);
        assert!(sample.shallow_water_columns >= 2);
        assert!(sample.suitable());
    }

    #[test]
    fn dry_deep_and_missing_sites_are_rejected() {
        let dry = sample_wetland_habitat(BlockPos::new(0, 64, 0), &|pos| {
            Some(generated_block_state_id(if pos.y == 63 {
                GRASS_BLOCK
            } else {
                AIR
            }))
        })
        .unwrap();
        assert!(!dry.suitable());

        let deep = sample_wetland_habitat(BlockPos::new(0, 68, 0), &|pos| {
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
            sample_wetland_habitat(BlockPos::new(0, 64, 0), &|_| None),
            Err(WetlandHabitatFailure::MissingBlockData)
        );
    }
}
