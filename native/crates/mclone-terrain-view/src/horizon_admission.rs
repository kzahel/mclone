use std::collections::HashSet;

use super::{TerrainClipmapLevelSnapshot, TerrainClipmapTile};

pub(crate) const TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL: u32 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerrainHorizonResourceTile {
    pub tile: TerrainClipmapTile,
    pub resource_slot: u32,
    pub slot_generation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerrainHorizonLevelPresentation {
    pub snapshot: TerrainClipmapLevelSnapshot,
    pub tiles: Vec<TerrainHorizonResourceTile>,
}

impl TerrainHorizonLevelPresentation {
    fn resource_for(&self, tile: TerrainClipmapTile) -> Option<TerrainHorizonResourceTile> {
        self.tiles
            .iter()
            .copied()
            .find(|candidate| same_semantic_tile(candidate.tile, tile))
    }
}

#[derive(Clone, Debug)]
struct TerrainHorizonLevelAdmission {
    pool_start: u32,
    pool_len: u32,
    terrain_committed: Option<TerrainHorizonLevelPresentation>,
    terrain_staged: Option<TerrainHorizonLevelPresentation>,
    vegetation_committed: Option<TerrainHorizonLevelPresentation>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TerrainHorizonAdmissionDiagnostics {
    pub logical_slots: u32,
    pub staging_slots: u32,
    pub requested_levels: u32,
    pub staged_levels: u32,
    pub committed_levels: u32,
    pub vegetation_committed_levels: u32,
    pub atomic_level_commits: u64,
    pub deferred_transition_attempts: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerrainHorizonBeginTransition {
    Started,
    Unchanged,
    Deferred,
}

#[derive(Clone, Debug)]
pub(crate) struct TerrainHorizonAdmission {
    levels: Vec<TerrainHorizonLevelAdmission>,
    assignments: Vec<Option<TerrainClipmapTile>>,
    ready: Vec<bool>,
    generations: Vec<u32>,
    logical_slots: u32,
    atomic_level_commits: u64,
    deferred_transition_attempts: u64,
}

impl TerrainHorizonAdmission {
    pub fn new(level_count: u32, slots_per_level: u32) -> Result<Self, String> {
        let pool_len = slots_per_level
            .checked_add(TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL)
            .ok_or("terrain horizon level resource count overflow")?;
        let resource_slots = level_count
            .checked_mul(pool_len)
            .ok_or("terrain horizon resource count overflow")?;
        let logical_slots = level_count
            .checked_mul(slots_per_level)
            .ok_or("terrain horizon logical slot count overflow")?;
        let levels = (0..level_count)
            .map(|level| TerrainHorizonLevelAdmission {
                pool_start: level * pool_len,
                pool_len,
                terrain_committed: None,
                terrain_staged: None,
                vegetation_committed: None,
            })
            .collect();
        Ok(Self {
            levels,
            assignments: vec![None; resource_slots as usize],
            ready: vec![false; resource_slots as usize],
            generations: vec![0; resource_slots as usize],
            logical_slots,
            atomic_level_commits: 0,
            deferred_transition_attempts: 0,
        })
    }

    pub fn resource_slots(&self) -> u32 {
        self.assignments.len() as u32
    }

    pub fn source_reset(&mut self) {
        for level in &mut self.levels {
            level.terrain_committed = None;
            level.terrain_staged = None;
            level.vegetation_committed = None;
        }
        self.assignments.fill(None);
        self.ready.fill(false);
    }

    /// Resize outer resource pools without disturbing common level ownership.
    pub fn resize_levels(
        &mut self,
        level_count: u32,
        slots_per_level: u32,
    ) -> Result<bool, String> {
        let pool_len = slots_per_level
            .checked_add(TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL)
            .ok_or("terrain horizon level resource count overflow")?;
        if self
            .levels
            .first()
            .is_some_and(|level| level.pool_len != pool_len)
        {
            return Err("terrain horizon level resize cannot change pool width".to_owned());
        }
        let old_level_count = self.levels.len() as u32;
        if old_level_count == level_count {
            return Ok(false);
        }
        let resource_slots = level_count
            .checked_mul(pool_len)
            .ok_or("terrain horizon resource count overflow")?;
        self.logical_slots = level_count
            .checked_mul(slots_per_level)
            .ok_or("terrain horizon logical slot count overflow")?;
        if level_count < old_level_count {
            self.levels.truncate(level_count as usize);
            self.assignments.truncate(resource_slots as usize);
            self.ready.truncate(resource_slots as usize);
            self.generations.truncate(resource_slots as usize);
        } else {
            self.levels
                .extend(
                    (old_level_count..level_count).map(|level| TerrainHorizonLevelAdmission {
                        pool_start: level * pool_len,
                        pool_len,
                        terrain_committed: None,
                        terrain_staged: None,
                        vegetation_committed: None,
                    }),
                );
            self.assignments.resize(resource_slots as usize, None);
            self.ready.resize(resource_slots as usize, false);
            self.generations.resize(resource_slots as usize, 0);
        }
        Ok(true)
    }

    pub fn begin_transition(
        &mut self,
        requested: &[TerrainClipmapLevelSnapshot],
        rebased_levels: &[u32],
    ) -> Result<
        (
            TerrainHorizonBeginTransition,
            Vec<TerrainHorizonResourceTile>,
        ),
        String,
    > {
        if requested.len() != self.levels.len() {
            return Err(format!(
                "terrain horizon requested {} levels for {} admission levels",
                requested.len(),
                self.levels.len()
            ));
        }
        if self
            .levels
            .iter()
            .any(|level| level.terrain_staged.is_some())
        {
            self.deferred_transition_attempts = self.deferred_transition_attempts.saturating_add(1);
            return Ok((TerrainHorizonBeginTransition::Deferred, Vec::new()));
        }

        let mut next = self.clone();
        let rebased = rebased_levels.iter().copied().collect::<HashSet<_>>();
        let mut changed = false;
        let mut entering = Vec::new();
        for snapshot in requested {
            let level_index = snapshot.level as usize;
            let Some(level) = next.levels.get_mut(level_index) else {
                return Err(format!(
                    "terrain horizon requested invalid level {}",
                    snapshot.level
                ));
            };
            if level
                .terrain_committed
                .as_ref()
                .is_some_and(|committed| same_level_origin(&committed.snapshot, snapshot))
            {
                continue;
            }
            changed = true;
            if rebased.contains(&snapshot.level) {
                level.terrain_committed = None;
                level.vegetation_committed = None;
            }

            let mut retained = Vec::new();
            if let Some(committed) = level.terrain_committed.as_ref() {
                for tile in &snapshot.tiles {
                    if let Some(resource) = committed.resource_for(*tile) {
                        retained.push(resource);
                    }
                }
            }
            let reserved = reserved_resources(level);
            let mut free = (level.pool_start..level.pool_start + level.pool_len)
                .filter(|slot| !reserved.contains(slot))
                .collect::<Vec<_>>();
            free.sort_unstable();

            let missing = snapshot
                .tiles
                .iter()
                .filter(|tile| {
                    !retained
                        .iter()
                        .any(|candidate| same_semantic_tile(candidate.tile, **tile))
                })
                .copied()
                .collect::<Vec<_>>();
            if missing.len() > free.len() {
                self.deferred_transition_attempts =
                    self.deferred_transition_attempts.saturating_add(1);
                return Ok((TerrainHorizonBeginTransition::Deferred, Vec::new()));
            }
            let mut staged_tiles = retained;
            for (tile, resource_slot) in missing.into_iter().zip(free) {
                let generation = next.generations[resource_slot as usize]
                    .checked_add(1)
                    .ok_or("terrain horizon resource generation exhausted")?;
                next.generations[resource_slot as usize] = generation;
                next.assignments[resource_slot as usize] = Some(tile);
                next.ready[resource_slot as usize] = false;
                let resource = TerrainHorizonResourceTile {
                    tile,
                    resource_slot,
                    slot_generation: generation,
                };
                staged_tiles.push(resource);
                entering.push(resource);
            }
            staged_tiles.sort_by_key(resource_order_key);
            level.terrain_staged = Some(TerrainHorizonLevelPresentation {
                snapshot: snapshot.clone(),
                tiles: staged_tiles,
            });
        }
        if !changed {
            return Ok((TerrainHorizonBeginTransition::Unchanged, Vec::new()));
        }
        entering.sort_by_key(|resource| {
            (
                std::cmp::Reverse(resource.tile.level),
                resource.tile.tile_z,
                resource.tile.tile_x,
            )
        });
        *self = next;
        Ok((TerrainHorizonBeginTransition::Started, entering))
    }

    pub fn mark_ready(&mut self, resource_slot: u32) -> Result<(), String> {
        let Some(ready) = self.ready.get_mut(resource_slot as usize) else {
            return Err(format!(
                "terrain horizon resource slot {resource_slot} is out of range"
            ));
        };
        *ready = true;
        Ok(())
    }

    pub fn commit_ready_terrain(&mut self) -> u32 {
        let mut committed = 0_u32;
        for level in &mut self.levels {
            let ready = level.terrain_staged.as_ref().is_some_and(|staged| {
                staged
                    .tiles
                    .iter()
                    .all(|tile| self.ready[tile.resource_slot as usize])
            });
            if ready {
                level.terrain_committed = level.terrain_staged.take();
                committed = committed.saturating_add(1);
            }
        }
        self.atomic_level_commits = self
            .atomic_level_commits
            .saturating_add(u64::from(committed));
        committed
    }

    pub fn commit_ready_vegetation(
        &mut self,
        mut is_ready: impl FnMut(TerrainHorizonResourceTile) -> bool,
    ) -> u32 {
        let mut committed = 0_u32;
        for level in &mut self.levels {
            let Some(terrain) = level.terrain_committed.as_ref() else {
                continue;
            };
            if terrain.tiles.iter().copied().all(&mut is_ready)
                && level.vegetation_committed.as_ref() != Some(terrain)
            {
                level.vegetation_committed = Some(terrain.clone());
                committed = committed.saturating_add(1);
            }
        }
        committed
    }

    pub fn clear_vegetation_above_sample_spacing(&mut self, maximum_sample_spacing: u32) -> u32 {
        let mut cleared = 0_u32;
        for level in &mut self.levels {
            if level
                .vegetation_committed
                .as_ref()
                .is_some_and(|presentation| {
                    presentation.snapshot.sample_spacing > maximum_sample_spacing
                })
            {
                level.vegetation_committed = None;
                cleared = cleared.saturating_add(1);
            }
        }
        cleared
    }

    pub fn assignment(&self, resource_slot: u32) -> Option<TerrainClipmapTile> {
        self.assignments
            .get(resource_slot as usize)
            .copied()
            .flatten()
    }

    pub fn slot_generation(&self, resource_slot: u32) -> Option<u32> {
        self.generations.get(resource_slot as usize).copied()
    }

    pub fn has_staged_levels(&self) -> bool {
        self.levels
            .iter()
            .any(|level| level.terrain_staged.is_some())
    }

    pub fn terrain_presentations(&self) -> Vec<TerrainHorizonLevelPresentation> {
        self.levels
            .iter()
            .filter_map(|level| level.terrain_committed.clone())
            .collect()
    }

    pub fn vegetation_presentations(&self) -> Vec<TerrainHorizonLevelPresentation> {
        self.levels
            .iter()
            .filter_map(|level| level.vegetation_committed.clone())
            .collect()
    }

    pub fn current_requested_presentations(&self) -> Vec<TerrainHorizonLevelPresentation> {
        self.levels
            .iter()
            .filter_map(|level| {
                level
                    .terrain_staged
                    .as_ref()
                    .or(level.terrain_committed.as_ref())
                    .cloned()
            })
            .collect()
    }

    pub fn diagnostics(&self) -> TerrainHorizonAdmissionDiagnostics {
        TerrainHorizonAdmissionDiagnostics {
            logical_slots: self.logical_slots,
            staging_slots: self.resource_slots().saturating_sub(self.logical_slots),
            requested_levels: self
                .levels
                .iter()
                .filter(|level| level.terrain_staged.is_some() || level.terrain_committed.is_some())
                .count() as u32,
            staged_levels: self
                .levels
                .iter()
                .filter(|level| level.terrain_staged.is_some())
                .count() as u32,
            committed_levels: self
                .levels
                .iter()
                .filter(|level| level.terrain_committed.is_some())
                .count() as u32,
            vegetation_committed_levels: self
                .levels
                .iter()
                .filter(|level| level.vegetation_committed.is_some())
                .count() as u32,
            atomic_level_commits: self.atomic_level_commits,
            deferred_transition_attempts: self.deferred_transition_attempts,
        }
    }
}

fn reserved_resources(level: &TerrainHorizonLevelAdmission) -> HashSet<u32> {
    level
        .terrain_committed
        .iter()
        .chain(level.vegetation_committed.iter())
        .chain(level.terrain_staged.iter())
        .flat_map(|presentation| presentation.tiles.iter())
        .map(|tile| tile.resource_slot)
        .collect()
}

fn same_level_origin(
    first: &TerrainClipmapLevelSnapshot,
    second: &TerrainClipmapLevelSnapshot,
) -> bool {
    first.level == second.level
        && first.sample_spacing == second.sample_spacing
        && first.origin_tile_x == second.origin_tile_x
        && first.origin_tile_z == second.origin_tile_z
}

fn same_semantic_tile(first: TerrainClipmapTile, second: TerrainClipmapTile) -> bool {
    first.level == second.level
        && first.tile_x == second.tile_x
        && first.tile_z == second.tile_z
        && first.sample_spacing == second.sample_spacing
}

fn resource_order_key(resource: &TerrainHorizonResourceTile) -> (i32, i32) {
    (resource.tile.tile_z, resource.tile.tile_x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TerrainClipmap, TerrainClipmapConfig};

    fn clipmap() -> TerrainClipmap {
        TerrainClipmap::new(TerrainClipmapConfig {
            level_count: 3,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        })
        .unwrap()
    }

    fn ready_all(
        admission: &mut TerrainHorizonAdmission,
        resources: &[TerrainHorizonResourceTile],
    ) {
        for resource in resources {
            admission.mark_ready(resource.resource_slot).unwrap();
        }
        admission.commit_ready_terrain();
    }

    #[test]
    fn committed_coverage_survives_a_partially_ready_aligned_transition() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(255, 255);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        let before = admission.terrain_presentations();

        let update = clipmap.update_center(256, 256);
        let (_, entering) = admission
            .begin_transition(&clipmap.levels(), &update.rebased_levels)
            .unwrap();
        assert!(!entering.is_empty());
        for resource in entering.iter().filter(|tile| tile.tile.level == 2) {
            admission.mark_ready(resource.resource_slot).unwrap();
        }
        admission.commit_ready_terrain();

        let during = admission.terrain_presentations();
        assert_eq!(during[0].snapshot, before[0].snapshot);
        assert_eq!(during[1].snapshot, before[1].snapshot);
        assert_ne!(during[2].snapshot, before[2].snapshot);
    }

    #[test]
    fn vegetation_retains_the_previous_level_until_every_replacement_is_ready() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(0, 0);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        admission.commit_ready_vegetation(|_| true);
        let before = admission.vegetation_presentations();

        let update = clipmap.update_center(64, 0);
        let (_, entering) = admission
            .begin_transition(&clipmap.levels(), &update.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &entering);
        let current = admission.terrain_presentations();
        assert_ne!(current[0].snapshot, before[0].snapshot);
        assert_eq!(admission.vegetation_presentations()[0], before[0]);

        let missing = current[0].tiles.last().unwrap().resource_slot;
        admission.commit_ready_vegetation(|resource| resource.resource_slot != missing);
        assert_eq!(admission.vegetation_presentations()[0], before[0]);
        admission.commit_ready_vegetation(|_| true);
        assert_eq!(admission.vegetation_presentations()[0], current[0]);
    }

    #[test]
    fn cold_vegetation_waits_for_a_complete_product_level() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(0, 0);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        assert_eq!(admission.terrain_presentations().len(), 3);
        assert!(admission.vegetation_presentations().is_empty());

        let missing = resources.last().unwrap().resource_slot;
        admission.commit_ready_vegetation(|resource| resource.resource_slot != missing);
        assert_eq!(admission.vegetation_presentations().len(), 2);
        admission.commit_ready_vegetation(|_| true);
        assert_eq!(admission.vegetation_presentations().len(), 3);
    }

    #[test]
    fn one_step_diagonal_transition_fits_the_fixed_guard_pool() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(-1, -1);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        admission.commit_ready_vegetation(|_| true);

        let update = clipmap.update_center(64, 64);
        let (state, entering) = admission
            .begin_transition(&clipmap.levels(), &update.rebased_levels)
            .unwrap();
        assert_eq!(state, TerrainHorizonBeginTransition::Started);
        assert_eq!(
            entering
                .iter()
                .filter(|resource| resource.tile.level == 0)
                .count(),
            7
        );
        assert_eq!(admission.diagnostics().staging_slots, 21);
    }

    #[test]
    fn reduced_vegetation_bound_releases_coarse_transition_guards() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(-184, -184);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        admission.commit_ready_vegetation(|_| true);
        assert_eq!(admission.diagnostics().vegetation_committed_levels, 3);

        assert_eq!(admission.clear_vegetation_above_sample_spacing(1), 2);
        assert_eq!(admission.diagnostics().vegetation_committed_levels, 1);

        let first = clipmap.update_center(8, 8);
        let (state, entering) = admission
            .begin_transition(&clipmap.levels(), &first.rebased_levels)
            .unwrap();
        assert_eq!(state, TerrainHorizonBeginTransition::Started);
        ready_all(&mut admission, &entering);
        admission.commit_ready_vegetation(|resource| resource.tile.sample_spacing == 1);

        let second = clipmap.update_center(8, 8);
        let (state, entering) = admission
            .begin_transition(&clipmap.levels(), &second.rebased_levels)
            .unwrap();
        assert_eq!(state, TerrainHorizonBeginTransition::Started);
        assert!(!entering.is_empty());
    }

    #[test]
    fn teleport_rebase_drops_old_coverage_and_reuses_the_bounded_pool() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(0, 0);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        admission.commit_ready_vegetation(|_| true);

        let update = clipmap.update_center(1_000_000, -1_000_000);
        let (state, entering) = admission
            .begin_transition(&clipmap.levels(), &update.rebased_levels)
            .unwrap();
        assert_eq!(state, TerrainHorizonBeginTransition::Started);
        assert_eq!(entering.len(), 48);
        assert!(admission.terrain_presentations().is_empty());
        assert!(admission.vegetation_presentations().is_empty());
        assert_eq!(admission.resource_slots(), 69);
    }

    #[test]
    fn outer_pool_resize_preserves_common_committed_levels() {
        let mut clipmap = clipmap();
        let initial = clipmap.update_center(12, -34);
        let mut admission = TerrainHorizonAdmission::new(3, 16).unwrap();
        let (_, resources) = admission
            .begin_transition(&clipmap.levels(), &initial.rebased_levels)
            .unwrap();
        ready_all(&mut admission, &resources);
        admission.commit_ready_vegetation(|_| true);
        let committed = admission.terrain_presentations();

        assert!(admission.resize_levels(2, 16).unwrap());
        assert_eq!(admission.terrain_presentations(), committed[..2]);
        assert_eq!(admission.resource_slots(), 46);

        clipmap.reconfigure_level_count(2).unwrap();
        clipmap.reconfigure_level_count(4).unwrap();
        assert!(admission.resize_levels(4, 16).unwrap());
        let update = clipmap.update_center(12, -34);
        let (_, entering) = admission
            .begin_transition(&clipmap.levels(), &update.rebased_levels)
            .unwrap();
        assert_eq!(admission.terrain_presentations(), committed[..2]);
        assert_eq!(entering.len(), 32);
        assert!(entering.iter().all(|resource| resource.tile.level >= 2));
    }
}
