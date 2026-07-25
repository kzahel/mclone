use std::collections::HashSet;

use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_MAX_SAMPLE_SPACING,
    TERRAIN_PREVIEW_MIN_SAMPLE_SPACING,
};

pub const TERRAIN_CLIPMAP_DEFAULT_TILES_PER_AXIS: u32 = 4;
pub const TERRAIN_CLIPMAP_DEFAULT_LEVEL_COUNT: u32 = 10;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TerrainClipmapTile {
    pub level: u32,
    pub tile_x: i32,
    pub tile_z: i32,
    pub sample_spacing: u32,
    pub physical_x: u32,
    pub physical_z: u32,
    pub physical_slot: u32,
}

impl TerrainClipmapTile {
    pub fn footprint_blocks(self) -> u32 {
        TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS * self.sample_spacing
    }

    pub fn min_x(self) -> i64 {
        i64::from(self.tile_x) * i64::from(self.footprint_blocks())
    }

    pub fn min_z(self) -> i64 {
        i64::from(self.tile_z) * i64::from(self.footprint_blocks())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainClipmapBounds {
    pub min_x: i64,
    pub min_z: i64,
    pub max_x: i64,
    pub max_z: i64,
}

impl TerrainClipmapBounds {
    pub const fn width(self) -> u64 {
        self.min_x.abs_diff(self.max_x)
    }

    pub const fn height(self) -> u64 {
        self.min_z.abs_diff(self.max_z)
    }

    pub fn contains(self, x: i64, z: i64) -> bool {
        x >= self.min_x && x < self.max_x && z >= self.min_z && z < self.max_z
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainClipmapConfig {
    pub level_count: u32,
    pub tiles_per_axis: u32,
    pub base_sample_spacing: u32,
}

impl Default for TerrainClipmapConfig {
    fn default() -> Self {
        Self {
            level_count: TERRAIN_CLIPMAP_DEFAULT_LEVEL_COUNT,
            tiles_per_axis: TERRAIN_CLIPMAP_DEFAULT_TILES_PER_AXIS,
            base_sample_spacing: TERRAIN_PREVIEW_MIN_SAMPLE_SPACING,
        }
    }
}

impl TerrainClipmapConfig {
    pub fn validate(self) -> Result<Self, String> {
        if self.level_count == 0 {
            return Err("terrain clipmap must contain at least one level".to_owned());
        }
        if self.tiles_per_axis < 4 || !self.tiles_per_axis.is_multiple_of(2) {
            return Err(
                "terrain clipmap tile axis must be an even value of at least four".to_owned(),
            );
        }
        if !self.base_sample_spacing.is_power_of_two()
            || !(TERRAIN_PREVIEW_MIN_SAMPLE_SPACING..=TERRAIN_PREVIEW_MAX_SAMPLE_SPACING)
                .contains(&self.base_sample_spacing)
        {
            return Err(format!(
                "terrain clipmap base spacing must be a power of two from {} through {}",
                TERRAIN_PREVIEW_MIN_SAMPLE_SPACING, TERRAIN_PREVIEW_MAX_SAMPLE_SPACING
            ));
        }
        let maximum_spacing = u64::from(self.base_sample_spacing)
            .checked_shl(self.level_count.saturating_sub(1))
            .ok_or("terrain clipmap level count overflows its sample spacing")?;
        if maximum_spacing > u64::from(TERRAIN_PREVIEW_MAX_SAMPLE_SPACING) {
            return Err(format!(
                "terrain clipmap coarsest spacing {maximum_spacing} exceeds {}",
                TERRAIN_PREVIEW_MAX_SAMPLE_SPACING
            ));
        }
        self.slots_per_level()
            .checked_mul(self.level_count)
            .ok_or("terrain clipmap slot count overflow")?;
        Ok(self)
    }

    pub const fn slots_per_level(self) -> u32 {
        self.tiles_per_axis * self.tiles_per_axis
    }

    pub const fn allocation_slots(self) -> u32 {
        self.slots_per_level() * self.level_count
    }

    pub fn sample_spacing(self, level: u32) -> u32 {
        self.base_sample_spacing << level
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainClipmapDiagnostics {
    pub revision: u64,
    pub allocation_slots: u32,
    pub valid_slots: u32,
    pub pending_refills: u32,
    pub last_refills: u32,
    pub total_refills: u64,
    pub last_rebases: u32,
    pub total_rebases: u64,
    pub retained_tiles: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainClipmapLevelSnapshot {
    pub level: u32,
    pub sample_spacing: u32,
    pub origin_tile_x: i32,
    pub origin_tile_z: i32,
    pub bounds: TerrainClipmapBounds,
    pub inner_hole: Option<TerrainClipmapBounds>,
    pub tiles: Vec<TerrainClipmapTile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainClipmapUpdate {
    pub revision: u64,
    pub center_x: i32,
    pub center_z: i32,
    pub refills: Vec<TerrainClipmapTile>,
    pub retained_tiles: u32,
    pub rebased_levels: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerrainClipmapLevelState {
    level: u32,
    sample_spacing: u32,
    origin_tile_x: i32,
    origin_tile_z: i32,
    initialized: bool,
}

impl TerrainClipmapLevelState {
    fn footprint_blocks(self) -> i64 {
        i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS) * i64::from(self.sample_spacing)
    }

    fn bounds(self, tiles_per_axis: u32) -> TerrainClipmapBounds {
        let footprint = self.footprint_blocks();
        TerrainClipmapBounds {
            min_x: i64::from(self.origin_tile_x) * footprint,
            min_z: i64::from(self.origin_tile_z) * footprint,
            max_x: (i64::from(self.origin_tile_x) + i64::from(tiles_per_axis)) * footprint,
            max_z: (i64::from(self.origin_tile_z) + i64::from(tiles_per_axis)) * footprint,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TerrainClipmap {
    config: TerrainClipmapConfig,
    levels: Vec<TerrainClipmapLevelState>,
    center_x: i32,
    center_z: i32,
    revision: u64,
    valid_slots: u32,
    pending_refills: u32,
    last_refills: u32,
    total_refills: u64,
    last_rebases: u32,
    total_rebases: u64,
    retained_tiles: u32,
}

impl TerrainClipmap {
    pub fn new(config: TerrainClipmapConfig) -> Result<Self, String> {
        let config = config.validate()?;
        let levels = (0..config.level_count)
            .map(|level| TerrainClipmapLevelState {
                level,
                sample_spacing: config.sample_spacing(level),
                origin_tile_x: 0,
                origin_tile_z: 0,
                initialized: false,
            })
            .collect();
        Ok(Self {
            config,
            levels,
            center_x: 0,
            center_z: 0,
            revision: 0,
            valid_slots: 0,
            pending_refills: 0,
            last_refills: 0,
            total_refills: 0,
            last_rebases: 0,
            total_rebases: 0,
            retained_tiles: 0,
        })
    }

    pub const fn config(&self) -> TerrainClipmapConfig {
        self.config
    }

    pub const fn center(&self) -> (i32, i32) {
        (self.center_x, self.center_z)
    }

    pub fn update_center(&mut self, center_x: i32, center_z: i32) -> TerrainClipmapUpdate {
        self.center_x = center_x;
        self.center_z = center_z;
        self.revision = self.revision.saturating_add(1);

        let mut refills = Vec::new();
        let mut retained_tiles = 0_u32;
        let mut rebased_levels = Vec::new();
        for level_index in (0..self.levels.len()).rev() {
            let level = &mut self.levels[level_index];
            let footprint =
                i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS) * i64::from(level.sample_spacing);
            let center_tile_x = i64::from(center_x).div_euclid(footprint);
            let center_tile_z = i64::from(center_z).div_euclid(footprint);
            let half_axis = i64::from(self.config.tiles_per_axis / 2);
            let new_origin_x = i32::try_from(center_tile_x - half_axis)
                .expect("validated clipmap center produces an i32 tile X");
            let new_origin_z = i32::try_from(center_tile_z - half_axis)
                .expect("validated clipmap center produces an i32 tile Z");
            let delta_x = i64::from(new_origin_x) - i64::from(level.origin_tile_x);
            let delta_z = i64::from(new_origin_z) - i64::from(level.origin_tile_z);
            let rebase = !level.initialized
                || delta_x.unsigned_abs() >= u64::from(self.config.tiles_per_axis)
                || delta_z.unsigned_abs() >= u64::from(self.config.tiles_per_axis);

            let old_origin = (level.origin_tile_x, level.origin_tile_z);
            level.origin_tile_x = new_origin_x;
            level.origin_tile_z = new_origin_z;
            if rebase {
                rebased_levels.push(level.level);
            }
            for local_z in 0..self.config.tiles_per_axis {
                for local_x in 0..self.config.tiles_per_axis {
                    let tile_x = new_origin_x
                        .checked_add(local_x as i32)
                        .expect("clipmap tile X remains in range");
                    let tile_z = new_origin_z
                        .checked_add(local_z as i32)
                        .expect("clipmap tile Z remains in range");
                    let retained = level.initialized
                        && !rebase
                        && tile_x >= old_origin.0
                        && tile_x
                            < old_origin
                                .0
                                .saturating_add(self.config.tiles_per_axis as i32)
                        && tile_z >= old_origin.1
                        && tile_z
                            < old_origin
                                .1
                                .saturating_add(self.config.tiles_per_axis as i32);
                    if retained {
                        retained_tiles = retained_tiles.saturating_add(1);
                    } else {
                        refills.push(tile_for(
                            self.config,
                            level.level,
                            level.sample_spacing,
                            tile_x,
                            tile_z,
                        ));
                    }
                }
            }
            level.initialized = true;
        }

        debug_assert_eq!(
            refills.iter().copied().collect::<HashSet<_>>().len(),
            refills.len()
        );
        self.valid_slots = self.config.allocation_slots();
        self.pending_refills = refills.len() as u32;
        self.last_refills = refills.len() as u32;
        self.total_refills = self.total_refills.saturating_add(refills.len() as u64);
        self.last_rebases = rebased_levels.len() as u32;
        self.total_rebases = self
            .total_rebases
            .saturating_add(rebased_levels.len() as u64);
        self.retained_tiles = retained_tiles;

        TerrainClipmapUpdate {
            revision: self.revision,
            center_x,
            center_z,
            refills,
            retained_tiles,
            rebased_levels,
        }
    }

    pub fn note_refills_completed(&mut self, count: u32) {
        self.pending_refills = self.pending_refills.saturating_sub(count);
    }

    pub const fn diagnostics(&self) -> TerrainClipmapDiagnostics {
        TerrainClipmapDiagnostics {
            revision: self.revision,
            allocation_slots: self.config.allocation_slots(),
            valid_slots: self.valid_slots,
            pending_refills: self.pending_refills,
            last_refills: self.last_refills,
            total_refills: self.total_refills,
            last_rebases: self.last_rebases,
            total_rebases: self.total_rebases,
            retained_tiles: self.retained_tiles,
        }
    }

    pub fn levels(&self) -> Vec<TerrainClipmapLevelSnapshot> {
        let mut snapshots = self
            .levels
            .iter()
            .map(|level| TerrainClipmapLevelSnapshot {
                level: level.level,
                sample_spacing: level.sample_spacing,
                origin_tile_x: level.origin_tile_x,
                origin_tile_z: level.origin_tile_z,
                bounds: level.bounds(self.config.tiles_per_axis),
                inner_hole: None,
                tiles: level_tiles(self.config, *level),
            })
            .collect::<Vec<_>>();
        for coarse_index in 1..snapshots.len() {
            snapshots[coarse_index].inner_hole = Some(snapshots[coarse_index - 1].bounds);
        }
        snapshots
    }
}

fn level_tiles(
    config: TerrainClipmapConfig,
    level: TerrainClipmapLevelState,
) -> Vec<TerrainClipmapTile> {
    let mut tiles = Vec::with_capacity(config.slots_per_level() as usize);
    for local_z in 0..config.tiles_per_axis {
        for local_x in 0..config.tiles_per_axis {
            tiles.push(tile_for(
                config,
                level.level,
                level.sample_spacing,
                level.origin_tile_x + local_x as i32,
                level.origin_tile_z + local_z as i32,
            ));
        }
    }
    tiles
}

fn tile_for(
    config: TerrainClipmapConfig,
    level: u32,
    sample_spacing: u32,
    tile_x: i32,
    tile_z: i32,
) -> TerrainClipmapTile {
    let axis = config.tiles_per_axis as i32;
    let physical_x = tile_x.rem_euclid(axis) as u32;
    let physical_z = tile_z.rem_euclid(axis) as u32;
    TerrainClipmapTile {
        level,
        tile_x,
        tile_z,
        sample_spacing,
        physical_x,
        physical_z,
        physical_slot: level * config.slots_per_level()
            + physical_z * config.tiles_per_axis
            + physical_x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> TerrainClipmap {
        TerrainClipmap::new(TerrainClipmapConfig {
            level_count: 3,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        })
        .unwrap()
    }

    #[test]
    fn initial_fill_uses_every_fixed_slot_once() {
        let mut clipmap = small();
        let update = clipmap.update_center(0, 0);
        assert_eq!(update.refills.len(), 48);
        assert_eq!(update.rebased_levels, vec![2, 1, 0]);
        assert_eq!(
            update
                .refills
                .iter()
                .map(|tile| tile.physical_slot)
                .collect::<HashSet<_>>()
                .len(),
            48
        );
        assert_eq!(clipmap.diagnostics().allocation_slots, 48);
        assert_eq!(clipmap.diagnostics().valid_slots, 48);
    }

    #[test]
    fn sub_cell_motion_retains_every_slot() {
        let mut clipmap = small();
        clipmap.update_center(0, 0);
        let update = clipmap.update_center(63, 63);
        assert!(update.refills.is_empty());
        assert_eq!(update.retained_tiles, 48);
        assert!(update.rebased_levels.is_empty());
    }

    #[test]
    fn axial_and_diagonal_shifts_refill_bands_without_corner_duplicates() {
        let mut clipmap = small();
        clipmap.update_center(0, 0);
        let axial = clipmap.update_center(64, 0);
        assert_eq!(
            axial.refills.iter().filter(|tile| tile.level == 0).count(),
            4
        );
        assert_eq!(axial.retained_tiles, 44);

        let diagonal = clipmap.update_center(128, 64);
        let finest = diagonal
            .refills
            .iter()
            .filter(|tile| tile.level == 0)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(finest.len(), 7);
        assert_eq!(finest.iter().copied().collect::<HashSet<_>>().len(), 7);
    }

    #[test]
    fn euclidean_slots_wrap_identically_across_negative_coordinates() {
        let config = TerrainClipmapConfig {
            level_count: 1,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        };
        for tile_z in -12..=12 {
            for tile_x in -12..=12 {
                let tile = tile_for(config, 0, 1, tile_x, tile_z);
                assert!(tile.physical_x < 4);
                assert!(tile.physical_z < 4);
                let wrapped = tile_for(config, 0, 1, tile_x + 4, tile_z - 8);
                assert_eq!(tile.physical_slot, wrapped.physical_slot);
            }
        }
    }

    #[test]
    fn one_cell_shift_preserves_physical_slots_for_retained_tiles() {
        let mut clipmap = small();
        clipmap.update_center(0, -1);
        let before = clipmap.levels()[0].tiles.clone();
        clipmap.update_center(64, -1);
        let after = clipmap.levels()[0].tiles.clone();
        let retained = before
            .iter()
            .filter(|first| {
                after.iter().any(|second| {
                    first.tile_x == second.tile_x
                        && first.tile_z == second.tile_z
                        && first.physical_slot == second.physical_slot
                })
            })
            .count();
        assert_eq!(retained, 12);
    }

    #[test]
    fn teleport_rebases_every_level_without_changing_allocation() {
        let mut clipmap = small();
        clipmap.update_center(0, 0);
        let allocation = clipmap.diagnostics().allocation_slots;
        let update = clipmap.update_center(1_000_000, -1_000_000);
        assert_eq!(update.rebased_levels, vec![2, 1, 0]);
        assert_eq!(update.refills.len(), allocation as usize);
        assert_eq!(clipmap.diagnostics().allocation_slots, allocation);
    }

    #[test]
    fn coarser_levels_expose_the_finer_coverage_as_their_hole() {
        let mut clipmap = small();
        clipmap.update_center(321, -654);
        let levels = clipmap.levels();
        assert_eq!(levels[0].inner_hole, None);
        assert_eq!(levels[1].inner_hole, Some(levels[0].bounds));
        assert_eq!(levels[2].inner_hole, Some(levels[1].bounds));
        assert_eq!(levels[0].bounds.width(), levels[1].bounds.width() / 2);
        assert_eq!(levels[1].bounds.width(), levels[2].bounds.width() / 2);
        assert!(levels[0].bounds.contains(321, -654));
    }

    #[test]
    fn long_walk_never_changes_allocation_or_duplicates_level_slots() {
        let mut clipmap = small();
        let allocation = clipmap.config().allocation_slots();
        for step in -128..=128 {
            clipmap.update_center(step * 37, step * -53);
            assert_eq!(clipmap.diagnostics().allocation_slots, allocation);
            for level in clipmap.levels() {
                assert_eq!(level.tiles.len(), 16);
                assert_eq!(
                    level
                        .tiles
                        .iter()
                        .map(|tile| tile.physical_slot)
                        .collect::<HashSet<_>>()
                        .len(),
                    16
                );
            }
        }
    }
}
