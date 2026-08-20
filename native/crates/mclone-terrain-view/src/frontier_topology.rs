use std::collections::{BTreeMap, BTreeSet};

use mclone_worldgen::terrain_preview::TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS;

use crate::{
    TERRAIN_FRONTIER_CONNECTOR_INSTANCE_BYTES, TERRAIN_FRONTIER_CONNECTOR_VERTICES_PER_SEGMENT,
    TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES, TERRAIN_PREVIEW_UNIFORM_BYTES,
    TerrainFrontierBoundaryKind, TerrainFrontierDirection, TerrainFrontierFineTileKey,
    TerrainFrontierPlan, TerrainFrontierPresentationIdentity,
};

pub const TERRAIN_FRONTIER_FINE_TILE_CAPACITY: u32 = 32;

const TERRAIN_FRONTIER_DIRECTION_STEPS: [(TerrainFrontierDirection, i64, i64); 4] = [
    (TerrainFrontierDirection::West, -1, 0),
    (TerrainFrontierDirection::East, 1, 0),
    (TerrainFrontierDirection::North, 0, -1),
    (TerrainFrontierDirection::South, 0, 1),
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainFrontierTopologyState {
    #[default]
    Empty,
    Complete,
    Incomplete,
}

impl TerrainFrontierTopologyState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainFrontierClosure {
    PreferredSolidConnector,
    ResolutionAwareSolidConnector,
    PreferredWaterCurtain,
    ResolutionAwareWaterCurtain,
    WorldBoundary,
    UnsupportedExactProfile,
    UnsupportedProceduralCoverage,
}

impl TerrainFrontierClosure {
    pub const fn certified(self) -> bool {
        matches!(
            self,
            Self::PreferredSolidConnector
                | Self::ResolutionAwareSolidConnector
                | Self::PreferredWaterCurtain
                | Self::ResolutionAwareWaterCurtain
                | Self::WorldBoundary
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainFrontierClosureSegment {
    pub canonical_exact_block: [i32; 2],
    pub lifted_exact_block: [i64; 2],
    pub procedural_block: [i64; 2],
    pub direction: TerrainFrontierDirection,
    pub boundary: TerrainFrontierBoundaryKind,
    pub procedural_level: Option<u32>,
    pub procedural_sample_spacing: Option<u32>,
    pub support_tile: TerrainFrontierFineTileKey,
    pub closure: TerrainFrontierClosure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainFrontierOuterEdge {
    pub tile: TerrainFrontierFineTileKey,
    pub direction: TerrainFrontierDirection,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrontierTopologyReceipt {
    pub state: TerrainFrontierTopologyState,
    pub presentation: TerrainFrontierPresentationIdentity,
    pub exact_generation: u64,
    pub preparation_micros: u64,
    pub exposed_segments: u32,
    pub certified_segments: u32,
    pub unresolved_segments: u32,
    pub preferred_solid_segments: u32,
    pub fallback_solid_segments: u32,
    pub preferred_water_segments: u32,
    pub fallback_water_segments: u32,
    pub world_boundary_segments: u32,
    pub unsupported_exact_profile_segments: u32,
    pub unsupported_procedural_segments: u32,
    pub support_pool_capacity: u32,
    pub desired_fine_tiles: u32,
    pub resident_fine_tiles: u32,
    pub selected_support_tiles: u32,
    pub rejected_support_tiles: u32,
    pub base_suppression_tiles: u32,
    pub outer_stitch_edges: u32,
    pub outer_stitch_segments: u32,
    pub support_pool_capacity_bytes: u64,
    pub active_support_resource_bytes: u64,
    pub support_uniform_upload_bytes: u64,
    pub support_compute_dispatches: u32,
    pub support_terrain_vertices: u64,
    pub connector_instance_bytes: u64,
    pub connector_vertices: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainFrontierTopologyOptions {
    pub fine_tile_capacity: u32,
}

impl Default for TerrainFrontierTopologyOptions {
    fn default() -> Self {
        Self {
            fine_tile_capacity: TERRAIN_FRONTIER_FINE_TILE_CAPACITY,
        }
    }
}

impl TerrainFrontierTopologyOptions {
    fn validate(self) -> Result<Self, String> {
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainFrontierTopology {
    selected_support_tiles: BTreeSet<TerrainFrontierFineTileKey>,
    active_fine_tiles: BTreeSet<TerrainFrontierFineTileKey>,
    base_suppression_tiles: BTreeSet<TerrainFrontierFineTileKey>,
    outer_edges: Vec<TerrainFrontierOuterEdge>,
    segments: Vec<TerrainFrontierClosureSegment>,
    receipt: TerrainFrontierTopologyReceipt,
}

impl TerrainFrontierTopology {
    pub fn prepare(
        plan: &TerrainFrontierPlan,
        options: TerrainFrontierTopologyOptions,
    ) -> Result<Self, String> {
        let started = topology_timing_now();
        let options = options.validate()?;
        let selected_support_tiles = select_support_tiles(plan, options.fine_tile_capacity);
        let mut active_fine_tiles = plan.resident_fine_tiles().clone();
        active_fine_tiles.extend(selected_support_tiles.iter().copied());
        let base_suppression_tiles = selected_support_tiles.clone();
        let outer_edges = support_outer_edges(&selected_support_tiles, &active_fine_tiles);
        let footprint = i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS);
        let mut segments = Vec::with_capacity(plan.segments().len());
        for segment in plan.segments() {
            let support_tile = TerrainFrontierFineTileKey {
                tile_x: segment.procedural_block[0].div_euclid(footprint),
                tile_z: segment.procedural_block[1].div_euclid(footprint),
            };
            let preferred = active_fine_tiles.contains(&support_tile);
            let closure = match segment.boundary {
                TerrainFrontierBoundaryKind::WorldBoundary => TerrainFrontierClosure::WorldBoundary,
                TerrainFrontierBoundaryKind::MissingExactProfile => {
                    TerrainFrontierClosure::UnsupportedExactProfile
                }
                TerrainFrontierBoundaryKind::Solid | TerrainFrontierBoundaryKind::Water
                    if segment.procedural_owner_count != 1 =>
                {
                    TerrainFrontierClosure::UnsupportedProceduralCoverage
                }
                TerrainFrontierBoundaryKind::Solid if preferred => {
                    TerrainFrontierClosure::PreferredSolidConnector
                }
                TerrainFrontierBoundaryKind::Solid => {
                    TerrainFrontierClosure::ResolutionAwareSolidConnector
                }
                TerrainFrontierBoundaryKind::Water if preferred => {
                    TerrainFrontierClosure::PreferredWaterCurtain
                }
                TerrainFrontierBoundaryKind::Water => {
                    TerrainFrontierClosure::ResolutionAwareWaterCurtain
                }
            };
            segments.push(TerrainFrontierClosureSegment {
                canonical_exact_block: segment.canonical_exact_block,
                lifted_exact_block: segment.lifted_exact_block,
                procedural_block: segment.procedural_block,
                direction: segment.direction,
                boundary: segment.boundary,
                procedural_level: segment.procedural_level,
                procedural_sample_spacing: segment.procedural_sample_spacing,
                support_tile,
                closure,
            });
        }
        let mut receipt = summarize(
            plan,
            options,
            &selected_support_tiles,
            &base_suppression_tiles,
            &outer_edges,
            &segments,
        );
        receipt.preparation_micros = topology_timing_elapsed_micros(started);
        let proof = Self {
            selected_support_tiles,
            active_fine_tiles,
            base_suppression_tiles,
            outer_edges,
            segments,
            receipt,
        };
        proof.validate()?;
        Ok(proof)
    }

    pub fn selected_support_tiles(&self) -> &BTreeSet<TerrainFrontierFineTileKey> {
        &self.selected_support_tiles
    }

    pub fn active_fine_tiles(&self) -> &BTreeSet<TerrainFrontierFineTileKey> {
        &self.active_fine_tiles
    }

    pub fn base_suppression_tiles(&self) -> &BTreeSet<TerrainFrontierFineTileKey> {
        &self.base_suppression_tiles
    }

    pub fn outer_edges(&self) -> &[TerrainFrontierOuterEdge] {
        &self.outer_edges
    }

    pub fn segments(&self) -> &[TerrainFrontierClosureSegment] {
        &self.segments
    }

    pub const fn receipt(&self) -> TerrainFrontierTopologyReceipt {
        self.receipt
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.selected_support_tiles != self.base_suppression_tiles {
            return Err("frontier support and base suppression tile sets diverge".to_owned());
        }
        if !self
            .selected_support_tiles
            .is_subset(&self.active_fine_tiles)
        {
            return Err(
                "frontier selected support is absent from active fine ownership".to_owned(),
            );
        }
        let expected_outer =
            support_outer_edges(&self.selected_support_tiles, &self.active_fine_tiles);
        if self.outer_edges != expected_outer {
            return Err("frontier support outer-closure topology is incomplete".to_owned());
        }
        for segment in &self.segments {
            let preferred = self.active_fine_tiles.contains(&segment.support_tile);
            match segment.closure {
                TerrainFrontierClosure::PreferredSolidConnector
                | TerrainFrontierClosure::PreferredWaterCurtain
                    if !preferred =>
                {
                    return Err("preferred frontier closure lacks fine ownership".to_owned());
                }
                TerrainFrontierClosure::ResolutionAwareSolidConnector
                | TerrainFrontierClosure::ResolutionAwareWaterCurtain
                    if preferred =>
                {
                    return Err("fallback frontier closure overlaps fine ownership".to_owned());
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn select_support_tiles(
    plan: &TerrainFrontierPlan,
    capacity: u32,
) -> BTreeSet<TerrainFrontierFineTileKey> {
    let footprint = i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS);
    let mut direct_edge_counts = BTreeMap::new();
    for segment in plan.segments() {
        let tile = TerrainFrontierFineTileKey {
            tile_x: segment.procedural_block[0].div_euclid(footprint),
            tile_z: segment.procedural_block[1].div_euclid(footprint),
        };
        if plan.missing_fine_tiles().contains(&tile) {
            let count = direct_edge_counts.entry(tile).or_insert(0_u32);
            *count = count.saturating_add(1);
        }
    }
    let observer = plan.observer_blocks();
    let mut candidates = plan
        .missing_fine_tiles()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_count = direct_edge_counts.get(left).copied().unwrap_or_default();
        let right_count = direct_edge_counts.get(right).copied().unwrap_or_default();
        right_count
            .cmp(&left_count)
            .then_with(|| tile_distance(*left, observer).cmp(&tile_distance(*right, observer)))
            .then_with(|| left.cmp(right))
    });
    candidates.into_iter().take(capacity as usize).collect()
}

fn tile_distance(tile: TerrainFrontierFineTileKey, observer: [i64; 2]) -> u128 {
    let footprint = i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS);
    let half = footprint / 2;
    let center_x = tile.tile_x.saturating_mul(footprint).saturating_add(half);
    let center_z = tile.tile_z.saturating_mul(footprint).saturating_add(half);
    let dx = u128::from(center_x.abs_diff(observer[0]));
    let dz = u128::from(center_z.abs_diff(observer[1]));
    dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz))
}

fn support_outer_edges(
    selected: &BTreeSet<TerrainFrontierFineTileKey>,
    active: &BTreeSet<TerrainFrontierFineTileKey>,
) -> Vec<TerrainFrontierOuterEdge> {
    let mut edges = Vec::new();
    for tile in selected {
        for (direction, dx, dz) in TERRAIN_FRONTIER_DIRECTION_STEPS {
            let neighbor = TerrainFrontierFineTileKey {
                tile_x: tile.tile_x.saturating_add(dx),
                tile_z: tile.tile_z.saturating_add(dz),
            };
            if !active.contains(&neighbor) {
                edges.push(TerrainFrontierOuterEdge {
                    tile: *tile,
                    direction,
                });
            }
        }
    }
    edges
}

fn summarize(
    plan: &TerrainFrontierPlan,
    options: TerrainFrontierTopologyOptions,
    selected: &BTreeSet<TerrainFrontierFineTileKey>,
    suppression: &BTreeSet<TerrainFrontierFineTileKey>,
    outer_edges: &[TerrainFrontierOuterEdge],
    segments: &[TerrainFrontierClosureSegment],
) -> TerrainFrontierTopologyReceipt {
    let mut receipt = TerrainFrontierTopologyReceipt {
        state: if segments.is_empty() {
            TerrainFrontierTopologyState::Empty
        } else {
            TerrainFrontierTopologyState::Complete
        },
        presentation: plan.receipt().presentation,
        exact_generation: plan.receipt().exact_generation,
        exposed_segments: segments.len().try_into().unwrap_or(u32::MAX),
        support_pool_capacity: options.fine_tile_capacity,
        desired_fine_tiles: plan
            .desired_fine_tiles()
            .len()
            .try_into()
            .unwrap_or(u32::MAX),
        resident_fine_tiles: plan
            .resident_fine_tiles()
            .len()
            .try_into()
            .unwrap_or(u32::MAX),
        selected_support_tiles: selected.len().try_into().unwrap_or(u32::MAX),
        rejected_support_tiles: plan
            .missing_fine_tiles()
            .len()
            .saturating_sub(selected.len())
            .try_into()
            .unwrap_or(u32::MAX),
        base_suppression_tiles: suppression.len().try_into().unwrap_or(u32::MAX),
        outer_stitch_edges: outer_edges.len().try_into().unwrap_or(u32::MAX),
        ..Default::default()
    };
    for segment in segments {
        if segment.closure.certified() {
            receipt.certified_segments = receipt.certified_segments.saturating_add(1);
        } else {
            receipt.unresolved_segments = receipt.unresolved_segments.saturating_add(1);
        }
        match segment.closure {
            TerrainFrontierClosure::PreferredSolidConnector => {
                receipt.preferred_solid_segments =
                    receipt.preferred_solid_segments.saturating_add(1)
            }
            TerrainFrontierClosure::ResolutionAwareSolidConnector => {
                receipt.fallback_solid_segments = receipt.fallback_solid_segments.saturating_add(1)
            }
            TerrainFrontierClosure::PreferredWaterCurtain => {
                receipt.preferred_water_segments =
                    receipt.preferred_water_segments.saturating_add(1)
            }
            TerrainFrontierClosure::ResolutionAwareWaterCurtain => {
                receipt.fallback_water_segments = receipt.fallback_water_segments.saturating_add(1)
            }
            TerrainFrontierClosure::WorldBoundary => {
                receipt.world_boundary_segments = receipt.world_boundary_segments.saturating_add(1)
            }
            TerrainFrontierClosure::UnsupportedExactProfile => {
                receipt.unsupported_exact_profile_segments =
                    receipt.unsupported_exact_profile_segments.saturating_add(1)
            }
            TerrainFrontierClosure::UnsupportedProceduralCoverage => {
                receipt.unsupported_procedural_segments =
                    receipt.unsupported_procedural_segments.saturating_add(1)
            }
        }
    }
    if receipt.unresolved_segments > 0 {
        receipt.state = TerrainFrontierTopologyState::Incomplete;
    }
    let selected_count = receipt.selected_support_tiles;
    let connector_count = receipt
        .preferred_solid_segments
        .saturating_add(receipt.fallback_solid_segments)
        .saturating_add(receipt.preferred_water_segments)
        .saturating_add(receipt.fallback_water_segments);
    receipt.outer_stitch_segments = receipt
        .outer_stitch_edges
        .saturating_mul(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS);
    receipt.support_pool_capacity_bytes = u64::from(options.fine_tile_capacity)
        .saturating_mul(TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES);
    receipt.active_support_resource_bytes =
        u64::from(selected_count).saturating_mul(TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES);
    receipt.support_uniform_upload_bytes =
        u64::from(selected_count).saturating_mul(TERRAIN_PREVIEW_UNIFORM_BYTES);
    receipt.support_compute_dispatches = selected_count;
    receipt.support_terrain_vertices = u64::from(selected_count)
        .saturating_mul(u64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS).pow(2))
        .saturating_mul(6);
    receipt.connector_instance_bytes =
        u64::from(connector_count).saturating_mul(TERRAIN_FRONTIER_CONNECTOR_INSTANCE_BYTES);
    receipt.connector_vertices = u64::from(connector_count)
        .saturating_mul(u64::from(TERRAIN_FRONTIER_CONNECTOR_VERTICES_PER_SEGMENT));
    receipt
}

#[cfg(target_arch = "wasm32")]
fn topology_timing_now() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn topology_timing_now() -> std::time::Instant {
    std::time::Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn topology_timing_elapsed_micros(started: f64) -> u64 {
    ((js_sys::Date::now() - started).max(0.0) * 1_000.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn topology_timing_elapsed_micros(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExactPaintedCoverageSnapshot, TerrainClipmap, TerrainClipmapConfig,
        TerrainClipmapLevelSnapshot, TerrainCompositionSourceIdentity, TerrainExactBoundaryColumn,
        TerrainExactBoundaryProfile, TerrainFrontierPlanOptions,
        terrain_exact_exposed_boundary_blocks,
    };
    use mclone_core::{AxisTopology, ChunkPos, HorizontalTopology};
    use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

    fn source() -> TerrainCompositionSourceIdentity {
        TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 12_345)
    }

    fn square(radius: i32, center: ChunkPos) -> BTreeSet<ChunkPos> {
        (-radius..=radius)
            .flat_map(|dz| {
                (-radius..=radius).map(move |dx| ChunkPos::new(center.x + dx, center.z + dz))
            })
            .collect()
    }

    fn clipmap_levels(center_blocks: [i32; 2]) -> Vec<TerrainClipmapLevelSnapshot> {
        let mut clipmap = TerrainClipmap::new(TerrainClipmapConfig {
            level_count: 6,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        })
        .unwrap();
        clipmap.update_center(center_blocks[0], center_blocks[1]);
        clipmap.levels()
    }

    fn boundary(
        coverage: &ExactPaintedCoverageSnapshot,
        topology: HorizontalTopology,
        water: bool,
    ) -> TerrainExactBoundaryProfile {
        let columns = terrain_exact_exposed_boundary_blocks(coverage, topology)
            .unwrap()
            .into_iter()
            .map(|[world_x, world_z]| TerrainExactBoundaryColumn {
                world_x,
                world_z,
                solid_top_y: 72,
                side_material: Some(4),
                water,
            });
        TerrainExactBoundaryProfile::from_columns(coverage, columns).unwrap()
    }

    fn plan(
        chunks: BTreeSet<ChunkPos>,
        topology: HorizontalTopology,
        center_blocks: [i32; 2],
        water: bool,
    ) -> TerrainFrontierPlan {
        let coverage = ExactPaintedCoverageSnapshot::new(source(), 7, chunks).unwrap();
        let boundary = boundary(&coverage, topology, water);
        TerrainFrontierPlan::prepare(
            &coverage,
            &boundary,
            topology,
            [i64::from(center_blocks[0]), i64::from(center_blocks[1])],
            &clipmap_levels(center_blocks),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn radius_eight_builds_one_complete_sparse_support_rectangle() {
        let plan = plan(
            square(8, ChunkPos::new(0, 0)),
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            false,
        );
        let proof =
            TerrainFrontierTopology::prepare(&plan, TerrainFrontierTopologyOptions::default())
                .unwrap();
        let receipt = proof.receipt();
        assert_eq!(receipt.state, TerrainFrontierTopologyState::Complete);
        assert_eq!(receipt.exposed_segments, 1_088);
        assert_eq!(receipt.certified_segments, 1_088);
        assert_eq!(receipt.unresolved_segments, 0);
        assert_eq!(receipt.preferred_solid_segments, 1_088);
        assert_eq!(receipt.fallback_solid_segments, 0);
        assert_eq!(receipt.desired_fine_tiles, 32);
        assert_eq!(receipt.resident_fine_tiles, 16);
        assert_eq!(receipt.selected_support_tiles, 20);
        assert_eq!(receipt.rejected_support_tiles, 0);
        assert_eq!(receipt.base_suppression_tiles, 20);
        assert_eq!(receipt.outer_stitch_edges, 24);
        assert_eq!(receipt.outer_stitch_segments, 1_536);
        assert_eq!(receipt.support_pool_capacity_bytes, 18_463_872);
        assert_eq!(receipt.active_support_resource_bytes, 11_539_920);
        assert_eq!(receipt.support_compute_dispatches, 20);
        assert_eq!(receipt.support_terrain_vertices, 491_520);
        assert_eq!(receipt.connector_instance_bytes, 13_056);
        assert_eq!(receipt.connector_vertices, 6_528);
    }

    #[test]
    fn bounded_pool_falls_back_without_leaving_an_edge_unowned() {
        let plan = plan(
            square(31, ChunkPos::new(0, 0)),
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            false,
        );
        let proof =
            TerrainFrontierTopology::prepare(&plan, TerrainFrontierTopologyOptions::default())
                .unwrap();
        let receipt = proof.receipt();
        assert_eq!(receipt.state, TerrainFrontierTopologyState::Complete);
        assert_eq!(receipt.exposed_segments, 4_032);
        assert_eq!(receipt.certified_segments, 4_032);
        assert_eq!(receipt.selected_support_tiles, 32);
        assert_eq!(receipt.rejected_support_tiles, 96);
        assert!(receipt.preferred_solid_segments > 0);
        assert!(receipt.fallback_solid_segments > 0);
        assert_eq!(
            receipt
                .preferred_solid_segments
                .saturating_add(receipt.fallback_solid_segments),
            4_032
        );
    }

    #[test]
    fn zero_fine_capacity_is_a_complete_synchronous_fallback() {
        let plan = plan(
            square(8, ChunkPos::new(0, 0)),
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            true,
        );
        let proof = TerrainFrontierTopology::prepare(
            &plan,
            TerrainFrontierTopologyOptions {
                fine_tile_capacity: 0,
            },
        )
        .unwrap();
        let receipt = proof.receipt();
        assert_eq!(receipt.state, TerrainFrontierTopologyState::Complete);
        assert_eq!(receipt.selected_support_tiles, 0);
        assert_eq!(receipt.base_suppression_tiles, 0);
        assert_eq!(receipt.outer_stitch_segments, 0);
        assert_eq!(receipt.preferred_solid_segments, 0);
        assert_eq!(receipt.preferred_water_segments, 0);
        assert_eq!(
            receipt
                .fallback_solid_segments
                .saturating_add(receipt.fallback_water_segments),
            1_088
        );
        assert_eq!(receipt.certified_segments, receipt.exposed_segments);
    }

    #[test]
    fn water_has_one_typed_preferred_or_fallback_curtain() {
        let preferred_plan = plan(
            BTreeSet::from([ChunkPos::new(0, 0)]),
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            true,
        );
        let preferred = TerrainFrontierTopology::prepare(
            &preferred_plan,
            TerrainFrontierTopologyOptions::default(),
        )
        .unwrap()
        .receipt();
        assert_eq!(preferred.state, TerrainFrontierTopologyState::Complete);
        assert_eq!(preferred.preferred_water_segments, 64);
        assert_eq!(preferred.fallback_water_segments, 0);

        let fallback_plan = plan(
            square(8, ChunkPos::new(0, 0)),
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            true,
        );
        let fallback = TerrainFrontierTopology::prepare(
            &fallback_plan,
            TerrainFrontierTopologyOptions {
                fine_tile_capacity: 1,
            },
        )
        .unwrap()
        .receipt();
        assert_eq!(fallback.state, TerrainFrontierTopologyState::Complete);
        assert!(fallback.preferred_water_segments > 0);
        assert!(fallback.fallback_water_segments > 0);
        assert_eq!(
            fallback
                .preferred_water_segments
                .saturating_add(fallback.fallback_water_segments),
            1_088
        );
    }

    #[test]
    fn missing_exact_profile_remains_explicitly_incomplete() {
        let coverage =
            ExactPaintedCoverageSnapshot::new(source(), 8, [ChunkPos::new(0, 0)]).unwrap();
        let empty = TerrainExactBoundaryProfile::empty(&coverage).unwrap();
        let plan = TerrainFrontierPlan::prepare(
            &coverage,
            &empty,
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            &clipmap_levels([8, 8]),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap();
        let receipt =
            TerrainFrontierTopology::prepare(&plan, TerrainFrontierTopologyOptions::default())
                .unwrap()
                .receipt();
        assert_eq!(receipt.state, TerrainFrontierTopologyState::Incomplete);
        assert_eq!(receipt.unsupported_exact_profile_segments, 64);
        assert_eq!(receipt.unresolved_segments, 64);
    }

    #[test]
    fn irregular_negative_shapes_have_complete_suppression_and_outer_edges() {
        let fixtures = [
            BTreeSet::from([
                ChunkPos::new(-5, -4),
                ChunkPos::new(-4, -4),
                ChunkPos::new(-5, -3),
            ]),
            (-3..=3)
                .flat_map(|z| {
                    (-3..=3)
                        .filter(move |x| !(*x == 0 && z == 0))
                        .map(move |x| ChunkPos::new(x - 8, z - 8))
                })
                .collect(),
            (-8..=8)
                .flat_map(|x| {
                    std::iter::once(ChunkPos::new(x, -5))
                        .chain((x % 2 == 0).then_some(ChunkPos::new(x, -4)))
                })
                .collect(),
        ];
        for chunks in fixtures {
            let proof = TerrainFrontierTopology::prepare(
                &plan(chunks, HorizontalTopology::UNBOUNDED, [-120, -120], false),
                TerrainFrontierTopologyOptions::default(),
            )
            .unwrap();
            assert_eq!(
                proof.receipt().state,
                TerrainFrontierTopologyState::Complete
            );
            assert_eq!(
                proof.selected_support_tiles(),
                proof.base_suppression_tiles()
            );
            for edge in proof.outer_edges() {
                let (_, dx, dz) = TERRAIN_FRONTIER_DIRECTION_STEPS
                    .into_iter()
                    .find(|(direction, _, _)| *direction == edge.direction)
                    .unwrap();
                assert!(
                    !proof
                        .active_fine_tiles()
                        .contains(&TerrainFrontierFineTileKey {
                            tile_x: edge.tile.tile_x + dx,
                            tile_z: edge.tile.tile_z + dz,
                        })
                );
            }
        }
    }

    #[test]
    fn finite_world_edges_and_periodic_lifts_remain_certified() {
        let finite =
            HorizontalTopology::new(AxisTopology::finite(0, 1), AxisTopology::finite(0, 1));
        let finite_proof = TerrainFrontierTopology::prepare(
            &plan(BTreeSet::from([ChunkPos::new(0, 0)]), finite, [8, 8], false),
            TerrainFrontierTopologyOptions::default(),
        )
        .unwrap()
        .receipt();
        assert_eq!(finite_proof.state, TerrainFrontierTopologyState::Complete);
        assert_eq!(finite_proof.world_boundary_segments, 64);
        assert_eq!(finite_proof.connector_vertices, 0);

        let cylinder = HorizontalTopology::cylinder_x(-16, 32);
        let periodic = TerrainFrontierTopology::prepare(
            &plan(
                BTreeSet::from([ChunkPos::new(15, 0), ChunkPos::new(-16, 0)]),
                cylinder,
                [15 * 16 + 8, 8],
                false,
            ),
            TerrainFrontierTopologyOptions::default(),
        )
        .unwrap();
        assert_eq!(
            periodic.receipt().state,
            TerrainFrontierTopologyState::Complete
        );
        assert!(
            periodic
                .segments()
                .iter()
                .any(|segment| segment.lifted_exact_block[0] >= 16 * 16)
        );
    }
}
