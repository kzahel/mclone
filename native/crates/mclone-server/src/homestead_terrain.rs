//! Bounded terrain adaptation for a persisted intro-homestead plan.
//!
//! The overlay is applied to each generated target chunk independently after
//! base FEATURES generation and before structure pieces. It never writes into
//! a neighboring chunk. Persisted chunk records therefore remain authoritative
//! over regeneration and preserve later player edits.

use std::collections::BTreeSet;

use mclone_core::{ChunkPos, HorizontalTopology};
use mclone_worldgen::block::{
    ACACIA_LOG, AIR, BAMBOO, BIRCH_LOG, BROWN_MUSHROOM_BLOCK, COARSE_DIRT, DARK_OAK_LOG, DIRT,
    GRASS_BLOCK, GRAVEL, JUNGLE_LOG, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LILAC_LOWER, LILAC_UPPER,
    MUSHROOM_STEM, OAK_LOG, PEONY_LOWER, PEONY_UPPER, RED_MUSHROOM_BLOCK, ROSE_BUSH_LOWER,
    ROSE_BUSH_UPPER, RawBlockId, SPRUCE_LOG, SUNFLOWER_LOWER, SUNFLOWER_UPPER, TALL_GRASS_LOWER,
    TALL_GRASS_UPPER, WATER, base_block_id, is_air_like, is_bamboo, is_cocoa, is_leaves, is_vine,
    material_blocks_motion,
};
use mclone_worldgen::homestead_site::HomesteadBounds2d;
use mclone_worldgen::levelgen::GeneratedChunk;

use crate::{
    HomesteadGradeRegion, HomesteadPathPlan, IntroHomesteadPlanRecord,
    validate_intro_homestead_plan_for_world,
};

#[derive(Clone, Debug)]
pub struct IntroHomesteadTerrainOverlay {
    plan: IntroHomesteadPlanRecord,
    touched_chunks: BTreeSet<ChunkPos>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HomesteadTerrainPlacementReceipt {
    pub affected: bool,
    pub graded_columns: u32,
    pub cut_blocks: u32,
    pub fill_blocks: u32,
    pub maximum_cut_depth: u16,
    pub maximum_fill_depth: u16,
    pub cleared_reserved_decorations: u32,
    pub path_surface_blocks: u32,
    pub pond_excavated_blocks: u32,
    pub pond_water_blocks: u32,
    pub pond_bottom_blocks: u32,
    pub pond_bank_fill_blocks: u32,
    pub changed_blocks: u32,
}

impl HomesteadTerrainPlacementReceipt {
    pub fn merge(&mut self, other: Self) {
        self.affected |= other.affected;
        self.graded_columns = self.graded_columns.saturating_add(other.graded_columns);
        self.cut_blocks = self.cut_blocks.saturating_add(other.cut_blocks);
        self.fill_blocks = self.fill_blocks.saturating_add(other.fill_blocks);
        self.maximum_cut_depth = self.maximum_cut_depth.max(other.maximum_cut_depth);
        self.maximum_fill_depth = self.maximum_fill_depth.max(other.maximum_fill_depth);
        self.cleared_reserved_decorations = self
            .cleared_reserved_decorations
            .saturating_add(other.cleared_reserved_decorations);
        self.path_surface_blocks = self
            .path_surface_blocks
            .saturating_add(other.path_surface_blocks);
        self.pond_excavated_blocks = self
            .pond_excavated_blocks
            .saturating_add(other.pond_excavated_blocks);
        self.pond_water_blocks = self
            .pond_water_blocks
            .saturating_add(other.pond_water_blocks);
        self.pond_bottom_blocks = self
            .pond_bottom_blocks
            .saturating_add(other.pond_bottom_blocks);
        self.pond_bank_fill_blocks = self
            .pond_bank_fill_blocks
            .saturating_add(other.pond_bank_fill_blocks);
        self.changed_blocks = self.changed_blocks.saturating_add(other.changed_blocks);
    }

    pub const fn earthwork_blocks(self) -> u32 {
        self.cut_blocks + self.fill_blocks + self.pond_excavated_blocks + self.pond_bank_fill_blocks
    }
}

impl IntroHomesteadTerrainOverlay {
    pub fn new(plan: IntroHomesteadPlanRecord) -> Result<Self, String> {
        validate_intro_homestead_plan_for_world(
            &plan,
            plan.base_descriptor.seed,
            plan.base_descriptor.generation_profile,
            topology_from_plan(&plan)?,
        )?;
        let touched_chunks = bounds_touched_chunks(
            plan.planting.generic_decoration_reservation,
            topology_from_plan(&plan)?,
        )?;
        Ok(Self {
            plan,
            touched_chunks,
        })
    }

    pub fn plan(&self) -> &IntroHomesteadPlanRecord {
        &self.plan
    }

    pub fn touched_chunks(&self) -> impl ExactSizeIterator<Item = ChunkPos> + '_ {
        self.touched_chunks.iter().copied()
    }

    pub fn affects_chunk(&self, pos: ChunkPos) -> bool {
        self.touched_chunks.contains(&pos)
    }

    pub fn materialize_chunk(
        &self,
        target: ChunkPos,
        chunk: &mut GeneratedChunk,
    ) -> Result<HomesteadTerrainPlacementReceipt, String> {
        if ChunkPos::new(chunk.chunk_x, chunk.chunk_z) != target {
            return Err(format!(
                "homestead terrain target {target:?} does not match generated chunk ({}, {})",
                chunk.chunk_x, chunk.chunk_z
            ));
        }
        let mut receipt = HomesteadTerrainPlacementReceipt {
            affected: self.affects_chunk(target),
            ..HomesteadTerrainPlacementReceipt::default()
        };
        if !receipt.affected {
            return Ok(receipt);
        }

        self.clear_reserved_decorations(chunk, &mut receipt);
        self.grade_foundations_and_path(chunk, &mut receipt)?;
        self.carve_authored_pond(chunk, &mut receipt)?;
        Ok(receipt)
    }

    fn clear_reserved_decorations(
        &self,
        chunk: &mut GeneratedChunk,
        receipt: &mut HomesteadTerrainPlacementReceipt,
    ) {
        let reservation = self.plan.planting.generic_decoration_reservation;
        for local_z in 0..16 {
            let world_z = chunk.chunk_z * 16 + local_z;
            if world_z < reservation.min_z || world_z > reservation.max_z {
                continue;
            }
            for local_x in 0..16 {
                let world_x = chunk.chunk_x * 16 + local_x;
                if world_x < reservation.min_x || world_x > reservation.max_x {
                    continue;
                }
                for y in chunk.min_y..chunk.min_y + chunk.height {
                    let block = chunk.block_at_y(local_x, y, local_z).0;
                    if is_reserved_decoration(block) {
                        set_block(chunk, local_x, y, local_z, AIR, receipt);
                        receipt.cleared_reserved_decorations =
                            receipt.cleared_reserved_decorations.saturating_add(1);
                    }
                }
            }
        }
    }

    fn grade_foundations_and_path(
        &self,
        chunk: &mut GeneratedChunk,
        receipt: &mut HomesteadTerrainPlacementReceipt,
    ) -> Result<(), String> {
        for local_z in 0..16 {
            let world_z = chunk.chunk_z * 16 + local_z;
            for local_x in 0..16 {
                let world_x = chunk.chunk_x * 16 + local_x;
                let Some(natural_surface_y) = terrain_surface_y(chunk, local_x, local_z) else {
                    continue;
                };
                let foundation = best_foundation_grade(
                    world_x,
                    world_z,
                    natural_surface_y,
                    &self.plan.grade_regions,
                );
                let path = path_grade(world_x, world_z, natural_surface_y, &self.plan.arrival_path);
                let selected = foundation.or(path);
                let Some(grade) = selected else {
                    continue;
                };
                if !grade.path_surface && grade.desired_surface_y == natural_surface_y {
                    continue;
                }
                let depth = natural_surface_y.abs_diff(grade.desired_surface_y) as u16;
                let declared_limit = self.plan.selected_site.metrics.max_cut_or_fill_depth.max(1);
                if depth > declared_limit {
                    return Err(format!(
                        "homestead grade at ({world_x},{world_z}) needs depth {depth}, above declared limit {declared_limit}"
                    ));
                }
                receipt.graded_columns = receipt.graded_columns.saturating_add(1);
                if grade.desired_surface_y < natural_surface_y {
                    let cut = natural_surface_y.abs_diff(grade.desired_surface_y);
                    receipt.cut_blocks = receipt.cut_blocks.saturating_add(cut);
                    receipt.maximum_cut_depth = receipt.maximum_cut_depth.max(cut as u16);
                } else {
                    let fill = grade.desired_surface_y.abs_diff(natural_surface_y);
                    receipt.fill_blocks = receipt.fill_blocks.saturating_add(fill);
                    receipt.maximum_fill_depth = receipt.maximum_fill_depth.max(fill as u16);
                }
                grade_column(
                    chunk,
                    local_x,
                    local_z,
                    natural_surface_y,
                    grade.desired_surface_y,
                    grade
                        .path_surface
                        .then(|| path_surface_block(world_x, world_z)),
                    receipt,
                )?;
                if grade.path_surface {
                    receipt.path_surface_blocks = receipt.path_surface_blocks.saturating_add(1);
                }
            }
        }
        Ok(())
    }

    fn carve_authored_pond(
        &self,
        chunk: &mut GeneratedChunk,
        receipt: &mut HomesteadTerrainPlacementReceipt,
    ) -> Result<(), String> {
        let water = &self.plan.water;
        for local_z in 0..16 {
            let world_z = chunk.chunk_z * 16 + local_z;
            for local_x in 0..16 {
                let world_x = chunk.chunk_x * 16 + local_x;
                if !bounds_contains(water.bounds, world_x, world_z) {
                    continue;
                }
                if pond_water_cell(water.bounds, world_x, world_z) {
                    let depth = pond_depth(water.bounds, world_x, world_z, water.maximum_depth);
                    let bottom_y = water.surface_y - i32::from(depth);
                    require_vertical(chunk, bottom_y, water.surface_y + 1)?;
                    for y in bottom_y + 1..chunk.min_y + chunk.height {
                        if material_blocks_motion(chunk.block_at_y(local_x, y, local_z).0) {
                            receipt.pond_excavated_blocks =
                                receipt.pond_excavated_blocks.saturating_add(1);
                        }
                    }
                    clear_column_above(chunk, local_x, local_z, water.surface_y, receipt);
                    set_block(chunk, local_x, bottom_y, local_z, DIRT, receipt);
                    receipt.pond_bottom_blocks = receipt.pond_bottom_blocks.saturating_add(1);
                    for y in bottom_y + 1..=water.surface_y {
                        set_block(chunk, local_x, y, local_z, WATER, receipt);
                        receipt.pond_water_blocks = receipt.pond_water_blocks.saturating_add(1);
                    }
                } else if adjacent_to_pond_water(water.bounds, world_x, world_z) {
                    let Some(surface_y) = terrain_surface_y(chunk, local_x, local_z) else {
                        continue;
                    };
                    if surface_y < water.surface_y {
                        require_vertical(chunk, surface_y, water.surface_y + 1)?;
                        for y in surface_y + 1..water.surface_y {
                            set_block(chunk, local_x, y, local_z, DIRT, receipt);
                            receipt.pond_bank_fill_blocks =
                                receipt.pond_bank_fill_blocks.saturating_add(1);
                        }
                        set_block(
                            chunk,
                            local_x,
                            water.surface_y,
                            local_z,
                            GRASS_BLOCK,
                            receipt,
                        );
                        receipt.pond_bank_fill_blocks =
                            receipt.pond_bank_fill_blocks.saturating_add(1);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct ColumnGrade {
    desired_surface_y: i32,
    weight_numerator: i32,
    weight_denominator: i32,
    path_surface: bool,
}

fn best_foundation_grade(
    x: i32,
    z: i32,
    natural_surface_y: i32,
    regions: &[HomesteadGradeRegion],
) -> Option<ColumnGrade> {
    regions
        .iter()
        .filter_map(|region| {
            let denominator = i32::from(region.feather_blocks) + 1;
            let distance = distance_outside_bounds(region.bounds, x, z);
            if distance > i32::from(region.feather_blocks) {
                return None;
            }
            let numerator = denominator - distance;
            Some(ColumnGrade {
                desired_surface_y: blend_height(
                    natural_surface_y,
                    region.target_surface_y,
                    numerator,
                    denominator,
                ),
                weight_numerator: numerator,
                weight_denominator: denominator,
                path_surface: false,
            })
        })
        .max_by(|left, right| compare_weights(*left, *right))
}

fn path_grade(
    x: i32,
    z: i32,
    natural_surface_y: i32,
    path: &HomesteadPathPlan,
) -> Option<ColumnGrade> {
    let sample = nearest_path_sample(x, z, path)?;
    let radius = i32::from(path.width_blocks / 2);
    let feather = i32::from(path.feather_blocks);
    let mut band = None;
    for distance in 0..=radius + feather {
        let threshold = i128::from(distance * distance) * sample.distance_denominator;
        if sample.distance_numerator <= threshold {
            band = Some(distance);
            break;
        }
    }
    let distance = band?;
    let (numerator, denominator, path_surface) = if distance <= radius {
        (feather + 1, feather + 1, true)
    } else {
        (feather + 1 - (distance - radius), feather + 1, false)
    };
    Some(ColumnGrade {
        desired_surface_y: blend_height(
            natural_surface_y,
            sample.target_surface_y,
            numerator,
            denominator,
        ),
        weight_numerator: numerator,
        weight_denominator: denominator,
        path_surface,
    })
}

#[derive(Clone, Copy, Debug)]
struct PathSample {
    target_surface_y: i32,
    distance_numerator: i128,
    distance_denominator: i128,
}

fn nearest_path_sample(x: i32, z: i32, path: &HomesteadPathPlan) -> Option<PathSample> {
    path.controls
        .windows(2)
        .map(|controls| {
            let start = controls[0].pos;
            let end = controls[1].pos;
            let dx = i128::from(end[0] - start[0]);
            let dz = i128::from(end[2] - start[2]);
            let px = i128::from(x - start[0]);
            let pz = i128::from(z - start[2]);
            let length_squared = dx * dx + dz * dz;
            if length_squared == 0 {
                return PathSample {
                    target_surface_y: start[1] - 1,
                    distance_numerator: px * px + pz * pz,
                    distance_denominator: 1,
                };
            }
            let projection = (px * dx + pz * dz).clamp(0, length_squared);
            let offset_x = px * length_squared - dx * projection;
            let offset_z = pz * length_squared - dz * projection;
            let y_delta = i128::from(end[1] - start[1]);
            PathSample {
                target_surface_y: start[1] - 1
                    + round_ratio_signed(y_delta * projection, length_squared) as i32,
                distance_numerator: offset_x * offset_x + offset_z * offset_z,
                distance_denominator: length_squared * length_squared,
            }
        })
        .min_by(|left, right| {
            (left.distance_numerator * right.distance_denominator)
                .cmp(&(right.distance_numerator * left.distance_denominator))
        })
        .or_else(|| {
            path.controls.first().map(|control| {
                let dx = i128::from(x - control.pos[0]);
                let dz = i128::from(z - control.pos[2]);
                PathSample {
                    target_surface_y: control.pos[1] - 1,
                    distance_numerator: dx * dx + dz * dz,
                    distance_denominator: 1,
                }
            })
        })
}

fn grade_column(
    chunk: &mut GeneratedChunk,
    local_x: i32,
    local_z: i32,
    natural_surface_y: i32,
    target_surface_y: i32,
    surface_block: Option<RawBlockId>,
    receipt: &mut HomesteadTerrainPlacementReceipt,
) -> Result<(), String> {
    require_vertical(chunk, target_surface_y - 2, target_surface_y + 1)?;
    clear_column_above(chunk, local_x, local_z, target_surface_y, receipt);
    if target_surface_y > natural_surface_y {
        for y in natural_surface_y + 1..target_surface_y {
            set_block(chunk, local_x, y, local_z, DIRT, receipt);
        }
    }
    for y in target_surface_y - 2..target_surface_y {
        if is_air_like(chunk.block_at_y(local_x, y, local_z).0) {
            set_block(chunk, local_x, y, local_z, DIRT, receipt);
        }
    }
    set_block(
        chunk,
        local_x,
        target_surface_y,
        local_z,
        surface_block.unwrap_or(GRASS_BLOCK),
        receipt,
    );
    Ok(())
}

fn clear_column_above(
    chunk: &mut GeneratedChunk,
    local_x: i32,
    local_z: i32,
    surface_y: i32,
    receipt: &mut HomesteadTerrainPlacementReceipt,
) {
    for y in surface_y + 1..chunk.min_y + chunk.height {
        let block = chunk.block_at_y(local_x, y, local_z).0;
        if !is_air_like(block) {
            set_block(chunk, local_x, y, local_z, AIR, receipt);
        }
    }
}

fn terrain_surface_y(chunk: &GeneratedChunk, local_x: i32, local_z: i32) -> Option<i32> {
    (chunk.min_y..chunk.min_y + chunk.height).rev().find(|y| {
        let block = chunk.block_at_y(local_x, *y, local_z).0;
        material_blocks_motion(block) && !is_reserved_decoration(block)
    })
}

fn set_block(
    chunk: &mut GeneratedChunk,
    local_x: i32,
    y: i32,
    local_z: i32,
    block: RawBlockId,
    receipt: &mut HomesteadTerrainPlacementReceipt,
) {
    if chunk.block_at_y(local_x, y, local_z).0 != block {
        chunk.set_block_at_y(local_x, y, local_z, block);
        receipt.changed_blocks = receipt.changed_blocks.saturating_add(1);
    }
}

fn require_vertical(
    chunk: &GeneratedChunk,
    min_y: i32,
    max_y_exclusive: i32,
) -> Result<(), String> {
    if min_y < chunk.min_y || max_y_exclusive > chunk.min_y + chunk.height {
        Err(format!(
            "homestead terrain edit {min_y}..{max_y_exclusive} is outside generated vertical bounds {}..{}",
            chunk.min_y,
            chunk.min_y + chunk.height
        ))
    } else {
        Ok(())
    }
}

fn compare_weights(left: ColumnGrade, right: ColumnGrade) -> std::cmp::Ordering {
    (left.weight_numerator * right.weight_denominator)
        .cmp(&(right.weight_numerator * left.weight_denominator))
}

fn blend_height(natural: i32, target: i32, numerator: i32, denominator: i32) -> i32 {
    natural
        + round_ratio_signed(
            i128::from(target - natural) * i128::from(numerator),
            i128::from(denominator),
        ) as i32
}

fn round_ratio_signed(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

fn distance_outside_bounds(bounds: HomesteadBounds2d, x: i32, z: i32) -> i32 {
    let dx = if x < bounds.min_x {
        bounds.min_x - x
    } else if x > bounds.max_x {
        x - bounds.max_x
    } else {
        0
    };
    let dz = if z < bounds.min_z {
        bounds.min_z - z
    } else if z > bounds.max_z {
        z - bounds.max_z
    } else {
        0
    };
    dx.max(dz)
}

fn bounds_contains(bounds: HomesteadBounds2d, x: i32, z: i32) -> bool {
    x >= bounds.min_x && x <= bounds.max_x && z >= bounds.min_z && z <= bounds.max_z
}

fn pond_water_cell(bounds: HomesteadBounds2d, x: i32, z: i32) -> bool {
    let center_x2 = bounds.min_x + bounds.max_x;
    let center_z2 = bounds.min_z + bounds.max_z;
    let radius_x2 = (bounds.max_x - bounds.min_x - 2).max(2);
    let radius_z2 = (bounds.max_z - bounds.min_z - 2).max(2);
    let dx2 = 2 * x - center_x2;
    let dz2 = 2 * z - center_z2;
    let radius_x_squared = i64::from(radius_x2) * i64::from(radius_x2);
    let radius_z_squared = i64::from(radius_z2) * i64::from(radius_z2);
    i64::from(dx2) * i64::from(dx2) * radius_z_squared
        + i64::from(dz2) * i64::from(dz2) * radius_x_squared
        <= radius_x_squared * radius_z_squared
}

fn adjacent_to_pond_water(bounds: HomesteadBounds2d, x: i32, z: i32) -> bool {
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .any(|(dx, dz)| pond_water_cell(bounds, x + dx, z + dz))
}

fn pond_depth(bounds: HomesteadBounds2d, x: i32, z: i32, maximum_depth: u8) -> u8 {
    if maximum_depth <= 1 {
        return 1;
    }
    let center_x2 = bounds.min_x + bounds.max_x;
    let center_z2 = bounds.min_z + bounds.max_z;
    let radius_x2 = (bounds.max_x - bounds.min_x - 2).max(2);
    let radius_z2 = (bounds.max_z - bounds.min_z - 2).max(2);
    let dx2 = (2 * x - center_x2).abs();
    let dz2 = (2 * z - center_z2).abs();
    if dx2 * 3 <= radius_x2 * 2 && dz2 * 3 <= radius_z2 * 2 {
        maximum_depth
    } else {
        1
    }
}

fn path_surface_block(x: i32, z: i32) -> RawBlockId {
    if (x.wrapping_mul(31) ^ z.wrapping_mul(17)).rem_euclid(5) == 0 {
        GRAVEL
    } else {
        COARSE_DIRT
    }
}

fn is_reserved_decoration(block: RawBlockId) -> bool {
    is_leaves(block)
        || is_vine(block)
        || is_cocoa(block)
        || is_bamboo(block)
        || matches!(
            base_block_id(block),
            OAK_LOG
                | BIRCH_LOG
                | SPRUCE_LOG
                | DARK_OAK_LOG
                | ACACIA_LOG
                | JUNGLE_LOG
                | BROWN_MUSHROOM_BLOCK
                | RED_MUSHROOM_BLOCK
                | MUSHROOM_STEM
                | BAMBOO
                | LARGE_FERN_LOWER
                | LARGE_FERN_UPPER
                | TALL_GRASS_LOWER
                | TALL_GRASS_UPPER
                | LILAC_LOWER
                | LILAC_UPPER
                | ROSE_BUSH_LOWER
                | ROSE_BUSH_UPPER
                | PEONY_LOWER
                | PEONY_UPPER
                | SUNFLOWER_LOWER
                | SUNFLOWER_UPPER
        )
}

fn bounds_touched_chunks(
    bounds: HomesteadBounds2d,
    topology: HorizontalTopology,
) -> Result<BTreeSet<ChunkPos>, String> {
    let mut touched = BTreeSet::new();
    for z in bounds.min_z.div_euclid(16)..=bounds.max_z.div_euclid(16) {
        for x in bounds.min_x.div_euclid(16)..=bounds.max_x.div_euclid(16) {
            let canonical = topology
                .canonicalize_chunk(ChunkPos::new(x, z))
                .ok_or_else(|| {
                    format!("homestead terrain touches out-of-topology chunk ({x},{z})")
                })?;
            touched.insert(canonical);
        }
    }
    Ok(touched)
}

fn topology_from_plan(plan: &IntroHomesteadPlanRecord) -> Result<HorizontalTopology, String> {
    use crate::HomesteadPlanAxisTopology;
    use mclone_core::AxisTopology;

    let axis = |value| match value {
        HomesteadPlanAxisTopology::Unbounded => AxisTopology::Unbounded,
        HomesteadPlanAxisTopology::Finite {
            minimum_chunk,
            maximum_chunk_exclusive,
        } => AxisTopology::finite(minimum_chunk, maximum_chunk_exclusive),
        HomesteadPlanAxisTopology::Periodic {
            minimum_chunk,
            period_chunks,
        } => AxisTopology::periodic(minimum_chunk, period_chunks),
    };
    Ok(HorizontalTopology::new(
        axis(plan.base_descriptor.topology.x),
        axis(plan.base_descriptor.topology.z),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{BlockPos, HorizontalTopology};
    use mclone_worldgen::homestead_site::{
        FlatGrassHomesteadSurveySource, HomesteadScoutRequest, scout_homestead_site,
    };
    use mclone_worldgen::levelgen::McloneOverworldFeatureDependencyCache;
    use mclone_worldgen::levelgen::MutableChunkBlockBuffer;

    fn flat_plan() -> IntroHomesteadPlanRecord {
        let request = HomesteadScoutRequest {
            seed: 8_675_309,
            provisional_spawn: BlockPos::new(8, 4, 8),
            topology: HorizontalTopology::UNBOUNDED,
        };
        let receipt = scout_homestead_site(&mut FlatGrassHomesteadSurveySource, request).unwrap();
        crate::compile_intro_homestead_plan(
            &mut FlatGrassHomesteadSurveySource,
            crate::WorldGenerationProfile::FlatGrassV1,
            request.topology,
            &receipt,
        )
        .unwrap()
    }

    fn sloped_chunk(pos: ChunkPos, base_y: i32) -> GeneratedChunk {
        let mut chunk = MutableChunkBlockBuffer::new(pos.x, pos.z, 0, 16);
        for z in 0..16 {
            for x in 0..16 {
                let surface = base_y + (pos.x * 16 + x).rem_euclid(3) - 1;
                for y in 0..surface {
                    chunk.set_block_at_y(x, y, z, DIRT);
                }
                chunk.set_block_at_y(x, surface, z, GRASS_BLOCK);
            }
        }
        GeneratedChunk::from_mutable_buffer(chunk)
    }

    #[test]
    fn overlay_touched_chunks_are_exactly_the_persisted_reservation() {
        let overlay = IntroHomesteadTerrainOverlay::new(flat_plan()).unwrap();
        let expected = bounds_touched_chunks(
            overlay.plan.planting.generic_decoration_reservation,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        assert_eq!(overlay.touched_chunks().collect::<BTreeSet<_>>(), expected);
    }

    #[test]
    fn foundation_path_and_pond_edits_are_chunk_clipped_and_idempotent() {
        let plan = flat_plan();
        let overlay = IntroHomesteadTerrainOverlay {
            touched_chunks: bounds_touched_chunks(
                plan.planting.generic_decoration_reservation,
                HorizontalTopology::UNBOUNDED,
            )
            .unwrap(),
            plan,
        };
        let chunks = overlay.touched_chunks().collect::<Vec<_>>();
        let mut first_pass = Vec::new();
        for pos in chunks.iter().copied() {
            let mut chunk = sloped_chunk(pos, 3);
            let receipt = overlay.materialize_chunk(pos, &mut chunk).unwrap();
            assert!(receipt.affected);
            first_pass.push((pos, chunk.blocks().to_vec()));
        }
        let mut reversed = Vec::new();
        for pos in chunks.iter().rev().copied() {
            let mut chunk = sloped_chunk(pos, 3);
            overlay.materialize_chunk(pos, &mut chunk).unwrap();
            reversed.push((pos, chunk.blocks().to_vec()));
        }
        reversed.sort_by_key(|(pos, _)| *pos);
        first_pass.sort_by_key(|(pos, _)| *pos);
        assert_eq!(first_pass, reversed);

        let pond_chunk = ChunkPos::new(
            overlay.plan.water.bounds.min_x.div_euclid(16),
            overlay.plan.water.bounds.min_z.div_euclid(16),
        );
        let mut pond = sloped_chunk(pond_chunk, 3);
        overlay.materialize_chunk(pond_chunk, &mut pond).unwrap();
        let once = pond.blocks().to_vec();
        overlay.materialize_chunk(pond_chunk, &mut pond).unwrap();
        assert_eq!(pond.blocks(), once);
    }

    #[test]
    fn decoration_reservation_does_not_touch_an_outside_chunk() {
        let overlay = IntroHomesteadTerrainOverlay::new(flat_plan()).unwrap();
        let reservation = overlay.plan.planting.generic_decoration_reservation;
        let outside = ChunkPos::new(reservation.max_x.div_euclid(16) + 2, 0);
        let mut chunk = sloped_chunk(outside, 3);
        chunk.set_block_at_y(0, 4, 0, OAK_LOG);
        let before = chunk.blocks().to_vec();
        let receipt = overlay.materialize_chunk(outside, &mut chunk).unwrap();
        assert!(!receipt.affected);
        assert_eq!(chunk.blocks(), before);
    }

    #[test]
    fn accepted_mclone_grade_is_order_independent_bounded_and_water_closed() {
        let plan = crate::realize_intro_homestead_plan(
            0,
            crate::WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let overlay = IntroHomesteadTerrainOverlay::new(plan.clone()).unwrap();
        let reservation_chunks = overlay.touched_chunks().collect::<Vec<_>>();
        assert_eq!(reservation_chunks.len(), 121);
        assert_eq!(reservation_chunks.first(), Some(&ChunkPos::new(-18, -92)));
        assert_eq!(reservation_chunks.last(), Some(&ChunkPos::new(-8, -82)));
        let targets = plan
            .pieces
            .iter()
            .find(|piece| piece.kind == crate::HomesteadPlanPieceKind::Grading)
            .unwrap()
            .touched_chunks
            .iter()
            .map(|chunk| ChunkPos::new(chunk[0], chunk[1]))
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 25);
        assert_eq!(targets.first(), Some(&ChunkPos::new(-15, -89)));
        assert_eq!(targets.last(), Some(&ChunkPos::new(-11, -85)));
        let generated = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(0, targets.iter().copied())
            .chunks;

        let apply = |order: Vec<ChunkPos>| {
            let mut chunks = generated.clone();
            let mut receipt = HomesteadTerrainPlacementReceipt::default();
            for pos in order {
                let chunk = chunks.get_mut(&pos).unwrap();
                receipt.merge(overlay.materialize_chunk(pos, chunk).unwrap());
            }
            (chunks, receipt)
        };
        let (forward, receipt) = apply(targets.clone());
        let (reverse, reverse_receipt) = apply(targets.iter().rev().copied().collect());
        assert_eq!(forward, reverse);
        assert_eq!(receipt, reverse_receipt);
        assert!(receipt.graded_columns > 0);
        assert!(receipt.path_surface_blocks > 0);
        assert!(receipt.pond_water_blocks > 0);
        assert!(receipt.maximum_cut_depth <= 3);
        assert!(receipt.maximum_fill_depth <= 3);
        assert_eq!(
            receipt,
            HomesteadTerrainPlacementReceipt {
                affected: true,
                graded_columns: 1_469,
                cut_blocks: 767,
                fill_blocks: 916,
                maximum_cut_depth: 3,
                maximum_fill_depth: 2,
                cleared_reserved_decorations: 2_439,
                path_surface_blocks: 75,
                pond_excavated_blocks: 76,
                pond_water_blocks: 118,
                pond_bottom_blocks: 78,
                pond_bank_fill_blocks: 24,
                changed_blocks: 5_053,
            }
        );

        for z in plan.water.bounds.min_z..=plan.water.bounds.max_z {
            for x in plan.water.bounds.min_x..=plan.water.bounds.max_x {
                if !pond_water_cell(plan.water.bounds, x, z) {
                    continue;
                }
                assert_eq!(block_at(&forward, x, plan.water.surface_y, z), Some(WATER));
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let neighbor = block_at(&forward, x + dx, plan.water.surface_y, z + dz)
                        .expect("pond neighbor must be in the generated grading map");
                    assert!(neighbor == WATER || material_blocks_motion(neighbor));
                }
            }
        }
    }

    fn block_at(
        chunks: &std::collections::BTreeMap<ChunkPos, GeneratedChunk>,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<RawBlockId> {
        chunks
            .get(&ChunkPos::new(x.div_euclid(16), z.div_euclid(16)))
            .map(|chunk| chunk.block_at_y(x.rem_euclid(16), y, z.rem_euclid(16)).0)
    }
}
