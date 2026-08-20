use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_core::{ChunkPos, HorizontalTopology};
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_SAMPLE_FLOATS,
};

use crate::{
    ExactPaintedCoverageSnapshot, TERRAIN_EXACT_BOUNDARY_MAX_BLOCKS_PER_AXIS,
    TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS, TERRAIN_EXACT_TRANSITION_BLOCKS_PER_TEXEL,
    TERRAIN_EXACT_TRANSITION_HALO_TEXELS, TERRAIN_EXACT_TRANSITION_MAX_TEXELS_PER_AXIS,
    TerrainClipmapLevelSnapshot, TerrainCompositionSourceIdentity, TerrainExactBoundaryProfile,
};

pub const TERRAIN_FRONTIER_FINE_SUPPORT_BAND_BLOCKS: u32 = 32;
pub const TERRAIN_FRONTIER_DIAGNOSTIC_FINE_TILE_CAPACITY: u32 = 128;
pub const TERRAIN_FRONTIER_SPACING_BUCKETS: usize = 10;
pub const TERRAIN_FRONTIER_CONNECTOR_INSTANCE_BYTES: u64 = 12;
pub const TERRAIN_FRONTIER_CONNECTOR_VERTICES_PER_SEGMENT: u32 = 6;

const TERRAIN_FRONTIER_NORMAL_HALO_RADIUS: u32 = 2;
const TERRAIN_FRONTIER_CURRENT_FINEST_LOGICAL_TILES: u32 = 16;
const TERRAIN_FRONTIER_CURRENT_STAGING_TILES: u32 = 7;

pub const TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES: u64 = {
    let drawn_samples = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1;
    let semantic_bytes = drawn_samples as u64
        * drawn_samples as u64
        * TERRAIN_PREVIEW_SAMPLE_FLOATS as u64
        * size_of::<f32>() as u64;
    let normal_samples = drawn_samples + TERRAIN_FRONTIER_NORMAL_HALO_RADIUS * 2;
    let normal_height_bytes =
        normal_samples as u64 * normal_samples as u64 * size_of::<f32>() as u64;
    semantic_bytes + normal_height_bytes + crate::TERRAIN_PREVIEW_UNIFORM_BYTES * 2
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TerrainFrontierDirection {
    #[default]
    West,
    East,
    North,
    South,
}

impl TerrainFrontierDirection {
    const ALL: [(Self, i32, i32); 4] = [
        (Self::West, -1, 0),
        (Self::East, 1, 0),
        (Self::North, 0, -1),
        (Self::South, 0, 1),
    ];
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainFrontierBoundaryKind {
    Solid,
    Water,
    MissingExactProfile,
    #[default]
    WorldBoundary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainFrontierCurrentClosure {
    SpacingOneConnector,
    UncertifiedWater,
    UnsupportedProceduralSpacing,
    MissingProceduralCoverage,
    AmbiguousProceduralCoverage,
    MissingExactProfile,
    #[default]
    WorldBoundary,
}

impl TerrainFrontierCurrentClosure {
    pub const fn certified(self) -> bool {
        matches!(self, Self::SpacingOneConnector | Self::WorldBoundary)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TerrainFrontierFineTileKey {
    pub tile_x: i64,
    pub tile_z: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrontierPresentationIdentity {
    pub level_count: u32,
    pub semantic_hash: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrontierFormatCapacityReceipt {
    pub painted_chunks: u32,
    pub canonical_chunk_width: u32,
    pub canonical_chunk_height: u32,
    pub lifted_chunk_width: u32,
    pub lifted_chunk_height: u32,
    pub boundary_width_blocks: u32,
    pub boundary_height_blocks: u32,
    pub transition_width_texels: u32,
    pub transition_height_texels: u32,
    pub mask_valid: bool,
    pub boundary_valid: bool,
    pub transition_valid: bool,
    pub all_valid: bool,
    pub canonical_packing_expands_local_extent: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrontierCandidateCostReceipt {
    pub full_finest_tiles_per_axis: u32,
    pub full_finest_logical_tiles: u32,
    pub full_finest_staging_tiles: u32,
    pub full_finest_added_bytes: u64,
    pub sparse_desired_fine_tiles: u32,
    pub sparse_base_resident_fine_tiles: u32,
    pub sparse_additional_fine_tiles: u32,
    pub sparse_admitted_fine_tiles: u32,
    pub sparse_rejected_fine_tiles: u32,
    pub sparse_additional_bytes: u64,
    pub sparse_outer_edge_segments: u32,
    pub sparse_vertex_upper_bound: u64,
    pub resolution_aware_connector_segments: u32,
    pub resolution_aware_connector_bytes: u64,
    pub resolution_aware_connector_vertices: u64,
    pub resolution_aware_unresolved_segments: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainFrontierPlanState {
    #[default]
    Empty,
    CurrentComplete,
    CurrentIncomplete,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrontierPlanReceipt {
    pub enabled: bool,
    pub state: TerrainFrontierPlanState,
    pub exact_generation: u64,
    pub presentation: TerrainFrontierPresentationIdentity,
    pub preparation_micros: u64,
    pub exact_chunks: u32,
    pub exposed_segments: u32,
    pub solid_segments: u32,
    pub water_segments: u32,
    pub missing_exact_profile_segments: u32,
    pub world_boundary_segments: u32,
    pub current_certified_segments: u32,
    pub current_unclosed_segments: u32,
    pub spacing_one_connector_segments: u32,
    pub unsupported_spacing_segments: u32,
    pub uncertified_water_segments: u32,
    pub missing_procedural_segments: u32,
    pub ambiguous_procedural_segments: u32,
    pub maximum_adjacent_spacing: u32,
    pub adjacent_segments_by_spacing: [u32; TERRAIN_FRONTIER_SPACING_BUCKETS],
    pub format: TerrainFrontierFormatCapacityReceipt,
    pub candidates: TerrainFrontierCandidateCostReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainFrontierPlanOptions {
    pub fine_support_band_blocks: u32,
    pub hypothetical_fine_tile_capacity: u32,
}

impl Default for TerrainFrontierPlanOptions {
    fn default() -> Self {
        Self {
            fine_support_band_blocks: TERRAIN_FRONTIER_FINE_SUPPORT_BAND_BLOCKS,
            hypothetical_fine_tile_capacity: TERRAIN_FRONTIER_DIAGNOSTIC_FINE_TILE_CAPACITY,
        }
    }
}

impl TerrainFrontierPlanOptions {
    fn validate(self) -> Result<Self, String> {
        if self.fine_support_band_blocks == 0 {
            return Err("terrain frontier fine-support band must be non-zero".to_owned());
        }
        if self.hypothetical_fine_tile_capacity == 0 {
            return Err(
                "terrain frontier diagnostic fine-tile capacity must be non-zero".to_owned(),
            );
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainFrontierSegment {
    pub canonical_exact_block: [i32; 2],
    pub lifted_exact_block: [i64; 2],
    pub procedural_block: [i64; 2],
    pub direction: TerrainFrontierDirection,
    pub boundary: TerrainFrontierBoundaryKind,
    pub procedural_level: Option<u32>,
    pub procedural_sample_spacing: Option<u32>,
    pub procedural_owner_count: u32,
    pub current_closure: TerrainFrontierCurrentClosure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainFrontierPlan {
    source: TerrainCompositionSourceIdentity,
    topology: HorizontalTopology,
    observer_blocks: [i64; 2],
    segments: Vec<TerrainFrontierSegment>,
    desired_fine_tiles: BTreeSet<TerrainFrontierFineTileKey>,
    missing_fine_tiles: BTreeSet<TerrainFrontierFineTileKey>,
    receipt: TerrainFrontierPlanReceipt,
}

impl TerrainFrontierPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        coverage: &ExactPaintedCoverageSnapshot,
        boundary: &TerrainExactBoundaryProfile,
        topology: HorizontalTopology,
        observer_blocks: [i64; 2],
        committed_levels: &[TerrainClipmapLevelSnapshot],
        options: TerrainFrontierPlanOptions,
    ) -> Result<Self, String> {
        let started = frontier_timing_now();
        let options = options.validate()?;
        topology
            .validate()
            .map_err(|error| format!("invalid terrain frontier topology: {error}"))?;
        if boundary.source() != coverage.source() || boundary.generation() != coverage.generation()
        {
            return Err("terrain frontier boundary does not match exact coverage".to_owned());
        }
        validate_committed_levels(committed_levels)?;
        let presentation = terrain_frontier_presentation_identity(committed_levels);
        if coverage.chunks().is_empty() {
            return Ok(Self {
                source: coverage.source(),
                topology,
                observer_blocks,
                segments: Vec::new(),
                desired_fine_tiles: BTreeSet::new(),
                missing_fine_tiles: BTreeSet::new(),
                receipt: TerrainFrontierPlanReceipt {
                    enabled: true,
                    exact_generation: coverage.generation(),
                    presentation,
                    preparation_micros: frontier_timing_elapsed_micros(started),
                    format: terrain_frontier_format_capacity(coverage.chunks())?,
                    ..Default::default()
                },
            });
        }

        let lifted_chunks = lift_connected_coverage(coverage, topology, observer_blocks)?;
        let format = terrain_frontier_format_capacity_with_lifts(
            coverage.chunks(),
            lifted_chunks.values().copied(),
        )?;
        let mut segments = Vec::new();
        for chunk in coverage.chunks() {
            let lifted = lifted_chunks
                .get(chunk)
                .copied()
                .expect("connected coverage lift contains every exact chunk");
            append_chunk_segments(
                &mut segments,
                coverage,
                boundary,
                topology,
                *chunk,
                lifted,
                committed_levels,
            )?;
        }

        let desired_fine_tiles =
            desired_fine_tiles(&segments, i64::from(options.fine_support_band_blocks));
        let resident_fine_tiles = resident_fine_tiles(committed_levels);
        let missing_fine_tiles = desired_fine_tiles
            .difference(&resident_fine_tiles)
            .copied()
            .collect::<BTreeSet<_>>();
        let receipt = summarize_plan(
            coverage,
            presentation,
            format,
            &segments,
            &desired_fine_tiles,
            &resident_fine_tiles,
            &missing_fine_tiles,
            options,
            frontier_timing_elapsed_micros(started),
        )?;
        Ok(Self {
            source: coverage.source(),
            topology,
            observer_blocks,
            segments,
            desired_fine_tiles,
            missing_fine_tiles,
            receipt,
        })
    }

    pub const fn source(&self) -> TerrainCompositionSourceIdentity {
        self.source
    }

    pub const fn topology(&self) -> HorizontalTopology {
        self.topology
    }

    pub const fn observer_blocks(&self) -> [i64; 2] {
        self.observer_blocks
    }

    pub fn segments(&self) -> &[TerrainFrontierSegment] {
        &self.segments
    }

    pub fn desired_fine_tiles(&self) -> &BTreeSet<TerrainFrontierFineTileKey> {
        &self.desired_fine_tiles
    }

    pub fn missing_fine_tiles(&self) -> &BTreeSet<TerrainFrontierFineTileKey> {
        &self.missing_fine_tiles
    }

    pub const fn receipt(&self) -> TerrainFrontierPlanReceipt {
        self.receipt
    }
}

pub fn terrain_frontier_format_capacity(
    chunks: &BTreeSet<ChunkPos>,
) -> Result<TerrainFrontierFormatCapacityReceipt, String> {
    terrain_frontier_format_capacity_with_lifts(
        chunks,
        chunks
            .iter()
            .map(|chunk| [i64::from(chunk.x), i64::from(chunk.z)]),
    )
}

pub fn terrain_frontier_presentation_identity(
    committed_levels: &[TerrainClipmapLevelSnapshot],
) -> TerrainFrontierPresentationIdentity {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    let mut levels = committed_levels.iter().collect::<Vec<_>>();
    levels.sort_by_key(|level| level.level);
    for level in &levels {
        for value in [
            u64::from(level.level),
            u64::from(level.sample_spacing),
            level.origin_tile_x as i64 as u64,
            level.origin_tile_z as i64 as u64,
            level.bounds.min_x as u64,
            level.bounds.min_z as u64,
            level.bounds.max_x as u64,
            level.bounds.max_z as u64,
        ] {
            hash ^= value;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    TerrainFrontierPresentationIdentity {
        level_count: levels.len().try_into().unwrap_or(u32::MAX),
        semantic_hash: hash,
    }
}

fn validate_committed_levels(levels: &[TerrainClipmapLevelSnapshot]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for level in levels {
        if !level.sample_spacing.is_power_of_two() {
            return Err(format!(
                "terrain frontier level {} has invalid spacing {}",
                level.level, level.sample_spacing
            ));
        }
        if !seen.insert(level.level) {
            return Err(format!(
                "terrain frontier presentation repeats level {}",
                level.level
            ));
        }
    }
    Ok(())
}

fn lift_connected_coverage(
    coverage: &ExactPaintedCoverageSnapshot,
    topology: HorizontalTopology,
    observer_blocks: [i64; 2],
) -> Result<BTreeMap<ChunkPos, [i64; 2]>, String> {
    let observer_chunk = ChunkPos::new(
        clamp_i64_to_i32(observer_blocks[0].div_euclid(16)),
        clamp_i64_to_i32(observer_blocks[1].div_euclid(16)),
    );
    let seed = coverage
        .chunks()
        .iter()
        .min_by_key(|chunk| {
            let [dx, dz] = topology.shortest_chunk_displacement(observer_chunk, **chunk);
            (
                dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)),
                chunk.x,
                chunk.z,
            )
        })
        .copied()
        .expect("non-empty exact coverage has a lift seed");
    let seed_lift = [
        topology
            .x
            .nearest_chunk_lift(seed.x, observer_blocks[0] as f64 / 16.0),
        topology
            .z
            .nearest_chunk_lift(seed.z, observer_blocks[1] as f64 / 16.0),
    ];
    let mut lifted = BTreeMap::from([(seed, seed_lift)]);
    let mut pending = VecDeque::from([seed]);
    while let Some(chunk) = pending.pop_front() {
        let current = lifted[&chunk];
        for (_, dx, dz) in TerrainFrontierDirection::ALL {
            let Some(neighbor) = topology.neighbor_chunk(chunk, dx, dz) else {
                continue;
            };
            if !coverage.contains(neighbor) {
                continue;
            }
            let candidate = [current[0] + i64::from(dx), current[1] + i64::from(dz)];
            if let Some(existing) = lifted.get(&neighbor) {
                if *existing != candidate {
                    return Err(
                        "terrain frontier exact coverage has an ambiguous periodic lift".to_owned(),
                    );
                }
            } else {
                lifted.insert(neighbor, candidate);
                pending.push_back(neighbor);
            }
        }
    }
    if lifted.len() != coverage.chunks().len() {
        return Err("terrain frontier exact coverage is disconnected".to_owned());
    }
    Ok(lifted)
}

#[allow(clippy::too_many_arguments)]
fn append_chunk_segments(
    segments: &mut Vec<TerrainFrontierSegment>,
    coverage: &ExactPaintedCoverageSnapshot,
    boundary: &TerrainExactBoundaryProfile,
    topology: HorizontalTopology,
    chunk: ChunkPos,
    lifted_chunk: [i64; 2],
    committed_levels: &[TerrainClipmapLevelSnapshot],
) -> Result<(), String> {
    let canonical_origin = [
        i64::from(chunk.x)
            .checked_mul(16)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or("terrain frontier canonical chunk X exceeds block coordinates")?,
        i64::from(chunk.z)
            .checked_mul(16)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or("terrain frontier canonical chunk Z exceeds block coordinates")?,
    ];
    let lifted_origin = [lifted_chunk[0] * 16, lifted_chunk[1] * 16];
    for (direction, dx, dz) in TerrainFrontierDirection::ALL {
        let neighbor = topology.neighbor_chunk(chunk, dx, dz);
        if neighbor.is_some_and(|neighbor| coverage.contains(neighbor)) {
            continue;
        }
        for offset in 0..16_i32 {
            let (local_x, local_z) = match direction {
                TerrainFrontierDirection::West => (0, offset),
                TerrainFrontierDirection::East => (15, offset),
                TerrainFrontierDirection::North => (offset, 0),
                TerrainFrontierDirection::South => (offset, 15),
            };
            let canonical_exact = [
                canonical_origin[0].saturating_add(local_x),
                canonical_origin[1].saturating_add(local_z),
            ];
            let lifted_exact = [
                lifted_origin[0] + i64::from(local_x),
                lifted_origin[1] + i64::from(local_z),
            ];
            let procedural_block = [
                lifted_exact[0] + i64::from(dx),
                lifted_exact[1] + i64::from(dz),
            ];
            let boundary_kind = if neighbor.is_none() {
                TerrainFrontierBoundaryKind::WorldBoundary
            } else {
                boundary
                    .column_at_world(canonical_exact[0], canonical_exact[1])
                    .map_or(TerrainFrontierBoundaryKind::MissingExactProfile, |column| {
                        if column.water {
                            TerrainFrontierBoundaryKind::Water
                        } else {
                            TerrainFrontierBoundaryKind::Solid
                        }
                    })
            };
            let owners = if neighbor.is_none() {
                Vec::new()
            } else {
                procedural_owners(procedural_block, committed_levels)
            };
            let (procedural_level, procedural_spacing) =
                owners.first().map_or((None, None), |level| {
                    (Some(level.level), Some(level.sample_spacing))
                });
            let current_closure = current_closure(boundary_kind, &owners);
            segments.push(TerrainFrontierSegment {
                canonical_exact_block: canonical_exact,
                lifted_exact_block: lifted_exact,
                procedural_block,
                direction,
                boundary: boundary_kind,
                procedural_level,
                procedural_sample_spacing: procedural_spacing,
                procedural_owner_count: owners.len().try_into().unwrap_or(u32::MAX),
                current_closure,
            });
        }
    }
    Ok(())
}

fn procedural_owners<'a>(
    block: [i64; 2],
    levels: &'a [TerrainClipmapLevelSnapshot],
) -> Vec<&'a TerrainClipmapLevelSnapshot> {
    let mut sorted = levels.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|level| level.level);
    sorted
        .iter()
        .copied()
        .filter(|level| {
            if !level.bounds.contains(block[0], block[1]) {
                return false;
            }
            level
                .level
                .checked_sub(1)
                .and_then(|finer| sorted.iter().find(|candidate| candidate.level == finer))
                .is_none_or(|finer| !finer.bounds.contains(block[0], block[1]))
        })
        .collect()
}

fn current_closure(
    boundary: TerrainFrontierBoundaryKind,
    owners: &[&TerrainClipmapLevelSnapshot],
) -> TerrainFrontierCurrentClosure {
    if boundary == TerrainFrontierBoundaryKind::WorldBoundary {
        return TerrainFrontierCurrentClosure::WorldBoundary;
    }
    if owners.is_empty() {
        return TerrainFrontierCurrentClosure::MissingProceduralCoverage;
    }
    if owners.len() != 1 {
        return TerrainFrontierCurrentClosure::AmbiguousProceduralCoverage;
    }
    match boundary {
        TerrainFrontierBoundaryKind::Solid if owners[0].sample_spacing == 1 => {
            TerrainFrontierCurrentClosure::SpacingOneConnector
        }
        TerrainFrontierBoundaryKind::Solid => {
            TerrainFrontierCurrentClosure::UnsupportedProceduralSpacing
        }
        TerrainFrontierBoundaryKind::Water => TerrainFrontierCurrentClosure::UncertifiedWater,
        TerrainFrontierBoundaryKind::MissingExactProfile => {
            TerrainFrontierCurrentClosure::MissingExactProfile
        }
        TerrainFrontierBoundaryKind::WorldBoundary => TerrainFrontierCurrentClosure::WorldBoundary,
    }
}

fn desired_fine_tiles(
    segments: &[TerrainFrontierSegment],
    band_blocks: i64,
) -> BTreeSet<TerrainFrontierFineTileKey> {
    let footprint = i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS);
    let mut tiles = BTreeSet::new();
    for segment in segments {
        if segment.boundary == TerrainFrontierBoundaryKind::WorldBoundary {
            continue;
        }
        let min_x = segment.procedural_block[0] - band_blocks;
        let max_x = segment.procedural_block[0] + band_blocks - 1;
        let min_z = segment.procedural_block[1] - band_blocks;
        let max_z = segment.procedural_block[1] + band_blocks - 1;
        for tile_z in min_z.div_euclid(footprint)..=max_z.div_euclid(footprint) {
            for tile_x in min_x.div_euclid(footprint)..=max_x.div_euclid(footprint) {
                tiles.insert(TerrainFrontierFineTileKey { tile_x, tile_z });
            }
        }
    }
    tiles
}

fn resident_fine_tiles(
    levels: &[TerrainClipmapLevelSnapshot],
) -> BTreeSet<TerrainFrontierFineTileKey> {
    levels
        .iter()
        .filter(|level| level.sample_spacing == 1)
        .flat_map(|level| level.tiles.iter())
        .map(|tile| TerrainFrontierFineTileKey {
            tile_x: i64::from(tile.tile_x),
            tile_z: i64::from(tile.tile_z),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn summarize_plan(
    coverage: &ExactPaintedCoverageSnapshot,
    presentation: TerrainFrontierPresentationIdentity,
    format: TerrainFrontierFormatCapacityReceipt,
    segments: &[TerrainFrontierSegment],
    desired_fine_tiles: &BTreeSet<TerrainFrontierFineTileKey>,
    resident_fine_tiles: &BTreeSet<TerrainFrontierFineTileKey>,
    missing_fine_tiles: &BTreeSet<TerrainFrontierFineTileKey>,
    options: TerrainFrontierPlanOptions,
    preparation_micros: u64,
) -> Result<TerrainFrontierPlanReceipt, String> {
    let mut receipt = TerrainFrontierPlanReceipt {
        enabled: true,
        state: TerrainFrontierPlanState::CurrentComplete,
        exact_generation: coverage.generation(),
        presentation,
        preparation_micros,
        exact_chunks: coverage.chunks().len().try_into().unwrap_or(u32::MAX),
        exposed_segments: segments.len().try_into().unwrap_or(u32::MAX),
        format,
        ..Default::default()
    };
    for segment in segments {
        match segment.boundary {
            TerrainFrontierBoundaryKind::Solid => {
                receipt.solid_segments = receipt.solid_segments.saturating_add(1)
            }
            TerrainFrontierBoundaryKind::Water => {
                receipt.water_segments = receipt.water_segments.saturating_add(1)
            }
            TerrainFrontierBoundaryKind::MissingExactProfile => {
                receipt.missing_exact_profile_segments =
                    receipt.missing_exact_profile_segments.saturating_add(1)
            }
            TerrainFrontierBoundaryKind::WorldBoundary => {
                receipt.world_boundary_segments = receipt.world_boundary_segments.saturating_add(1)
            }
        }
        if segment.current_closure.certified() {
            receipt.current_certified_segments =
                receipt.current_certified_segments.saturating_add(1);
        } else {
            receipt.current_unclosed_segments = receipt.current_unclosed_segments.saturating_add(1);
        }
        match segment.current_closure {
            TerrainFrontierCurrentClosure::SpacingOneConnector => {
                receipt.spacing_one_connector_segments =
                    receipt.spacing_one_connector_segments.saturating_add(1)
            }
            TerrainFrontierCurrentClosure::UnsupportedProceduralSpacing => {
                receipt.unsupported_spacing_segments =
                    receipt.unsupported_spacing_segments.saturating_add(1)
            }
            TerrainFrontierCurrentClosure::UncertifiedWater => {
                receipt.uncertified_water_segments =
                    receipt.uncertified_water_segments.saturating_add(1)
            }
            TerrainFrontierCurrentClosure::MissingProceduralCoverage => {
                receipt.missing_procedural_segments =
                    receipt.missing_procedural_segments.saturating_add(1)
            }
            TerrainFrontierCurrentClosure::AmbiguousProceduralCoverage => {
                receipt.ambiguous_procedural_segments =
                    receipt.ambiguous_procedural_segments.saturating_add(1)
            }
            TerrainFrontierCurrentClosure::MissingExactProfile
            | TerrainFrontierCurrentClosure::WorldBoundary => {}
        }
        if let Some(spacing) = segment.procedural_sample_spacing {
            receipt.maximum_adjacent_spacing = receipt.maximum_adjacent_spacing.max(spacing);
            let bucket = spacing.ilog2() as usize;
            if let Some(count) = receipt.adjacent_segments_by_spacing.get_mut(bucket) {
                *count = count.saturating_add(1);
            }
        }
    }
    if receipt.current_unclosed_segments > 0 {
        receipt.state = TerrainFrontierPlanState::CurrentIncomplete;
    }
    receipt.candidates = candidate_costs(
        format,
        segments,
        desired_fine_tiles,
        resident_fine_tiles,
        missing_fine_tiles,
        options,
    )?;
    Ok(receipt)
}

fn candidate_costs(
    format: TerrainFrontierFormatCapacityReceipt,
    segments: &[TerrainFrontierSegment],
    desired_fine_tiles: &BTreeSet<TerrainFrontierFineTileKey>,
    resident_fine_tiles: &BTreeSet<TerrainFrontierFineTileKey>,
    missing_fine_tiles: &BTreeSet<TerrainFrontierFineTileKey>,
    options: TerrainFrontierPlanOptions,
) -> Result<TerrainFrontierCandidateCostReceipt, String> {
    let footprint = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS;
    let halo = options.fine_support_band_blocks;
    let required_axis = chunk_bounds(
        desired_fine_tiles
            .iter()
            .map(|tile| [tile.tile_x, tile.tile_z]),
    )
    .map(|(min_x, max_x, min_z, max_z)| {
        inclusive_span_u32(min_x, max_x, "full-fine tile width").and_then(|width| {
            inclusive_span_u32(min_z, max_z, "full-fine tile height")
                .map(|height| width.max(height).max(4))
        })
    })
    .transpose()?
    .unwrap_or_else(|| {
        let required_width_blocks = format
            .lifted_chunk_width
            .saturating_mul(16)
            .saturating_add(halo.saturating_mul(2));
        let required_height_blocks = format
            .lifted_chunk_height
            .saturating_mul(16)
            .saturating_add(halo.saturating_mul(2));
        div_ceil(required_width_blocks.max(required_height_blocks), footprint).max(4)
    });
    let full_axis = required_axis.next_multiple_of(2);
    let full_logical = full_axis.saturating_mul(full_axis);
    let full_staging = full_axis.saturating_mul(2).saturating_sub(1);
    let full_added_resources = full_logical.saturating_add(full_staging).saturating_sub(
        TERRAIN_FRONTIER_CURRENT_FINEST_LOGICAL_TILES
            .saturating_add(TERRAIN_FRONTIER_CURRENT_STAGING_TILES),
    );
    let sparse_desired = desired_fine_tiles.len().try_into().unwrap_or(u32::MAX);
    let sparse_resident = desired_fine_tiles
        .intersection(resident_fine_tiles)
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    let sparse_additional = missing_fine_tiles.len().try_into().unwrap_or(u32::MAX);
    let sparse_admitted = sparse_additional.min(options.hypothetical_fine_tile_capacity);
    let sparse_rejected = sparse_additional.saturating_sub(sparse_admitted);
    let sparse_outer_edges = desired_fine_tiles.iter().fold(0_u32, |count, tile| {
        let exposed_sides = [(-1_i64, 0_i64), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .filter(|(dx, dz)| {
                !desired_fine_tiles.contains(&TerrainFrontierFineTileKey {
                    tile_x: tile.tile_x + dx,
                    tile_z: tile.tile_z + dz,
                })
            })
            .count() as u32;
        count.saturating_add(exposed_sides.saturating_mul(footprint))
    });
    let resolution_aware_segments = segments
        .iter()
        .filter(|segment| {
            segment.boundary == TerrainFrontierBoundaryKind::Solid
                && segment.procedural_owner_count == 1
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    let resolution_aware_unresolved = segments
        .len()
        .saturating_sub(resolution_aware_segments as usize)
        .try_into()
        .unwrap_or(u32::MAX);
    Ok(TerrainFrontierCandidateCostReceipt {
        full_finest_tiles_per_axis: full_axis,
        full_finest_logical_tiles: full_logical,
        full_finest_staging_tiles: full_staging,
        full_finest_added_bytes: u64::from(full_added_resources)
            .saturating_mul(TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES),
        sparse_desired_fine_tiles: sparse_desired,
        sparse_base_resident_fine_tiles: sparse_resident,
        sparse_additional_fine_tiles: sparse_additional,
        sparse_admitted_fine_tiles: sparse_admitted,
        sparse_rejected_fine_tiles: sparse_rejected,
        sparse_additional_bytes: u64::from(sparse_additional)
            .saturating_mul(TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES),
        sparse_outer_edge_segments: sparse_outer_edges,
        sparse_vertex_upper_bound: u64::from(sparse_desired)
            .saturating_mul(u64::from(footprint))
            .saturating_mul(u64::from(footprint))
            .saturating_mul(6),
        resolution_aware_connector_segments: resolution_aware_segments,
        resolution_aware_connector_bytes: u64::from(resolution_aware_segments)
            .saturating_mul(TERRAIN_FRONTIER_CONNECTOR_INSTANCE_BYTES),
        resolution_aware_connector_vertices: u64::from(resolution_aware_segments)
            .saturating_mul(u64::from(TERRAIN_FRONTIER_CONNECTOR_VERTICES_PER_SEGMENT)),
        resolution_aware_unresolved_segments: resolution_aware_unresolved,
    })
}

fn terrain_frontier_format_capacity_with_lifts(
    chunks: &BTreeSet<ChunkPos>,
    lifted_chunks: impl IntoIterator<Item = [i64; 2]>,
) -> Result<TerrainFrontierFormatCapacityReceipt, String> {
    let Some((min_x, max_x, min_z, max_z)) = chunk_bounds(
        chunks
            .iter()
            .map(|chunk| [i64::from(chunk.x), i64::from(chunk.z)]),
    ) else {
        return Ok(TerrainFrontierFormatCapacityReceipt {
            mask_valid: true,
            boundary_valid: true,
            transition_valid: true,
            all_valid: true,
            ..Default::default()
        });
    };
    let canonical_width = inclusive_span_u32(min_x, max_x, "canonical chunk width")?;
    let canonical_height = inclusive_span_u32(min_z, max_z, "canonical chunk height")?;
    let (lifted_min_x, lifted_max_x, lifted_min_z, lifted_max_z) =
        chunk_bounds(lifted_chunks).ok_or("terrain frontier lost lifted exact chunks")?;
    let lifted_width = inclusive_span_u32(lifted_min_x, lifted_max_x, "lifted chunk width")?;
    let lifted_height = inclusive_span_u32(lifted_min_z, lifted_max_z, "lifted chunk height")?;
    let boundary_width = canonical_width.saturating_mul(16);
    let boundary_height = canonical_height.saturating_mul(16);
    let transition_width = canonical_width
        .saturating_mul(16 / TERRAIN_EXACT_TRANSITION_BLOCKS_PER_TEXEL)
        .saturating_add(TERRAIN_EXACT_TRANSITION_HALO_TEXELS * 2);
    let transition_height = canonical_height
        .saturating_mul(16 / TERRAIN_EXACT_TRANSITION_BLOCKS_PER_TEXEL)
        .saturating_add(TERRAIN_EXACT_TRANSITION_HALO_TEXELS * 2);
    let mask_valid = canonical_width <= TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS
        && canonical_height <= TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS;
    let boundary_valid = boundary_width <= TERRAIN_EXACT_BOUNDARY_MAX_BLOCKS_PER_AXIS
        && boundary_height <= TERRAIN_EXACT_BOUNDARY_MAX_BLOCKS_PER_AXIS;
    let transition_valid = transition_width <= TERRAIN_EXACT_TRANSITION_MAX_TEXELS_PER_AXIS
        && transition_height <= TERRAIN_EXACT_TRANSITION_MAX_TEXELS_PER_AXIS;
    Ok(TerrainFrontierFormatCapacityReceipt {
        painted_chunks: chunks.len().try_into().unwrap_or(u32::MAX),
        canonical_chunk_width: canonical_width,
        canonical_chunk_height: canonical_height,
        lifted_chunk_width: lifted_width,
        lifted_chunk_height: lifted_height,
        boundary_width_blocks: boundary_width,
        boundary_height_blocks: boundary_height,
        transition_width_texels: transition_width,
        transition_height_texels: transition_height,
        mask_valid,
        boundary_valid,
        transition_valid,
        all_valid: mask_valid && boundary_valid && transition_valid,
        canonical_packing_expands_local_extent: canonical_width > lifted_width
            || canonical_height > lifted_height,
    })
}

fn chunk_bounds(chunks: impl IntoIterator<Item = [i64; 2]>) -> Option<(i64, i64, i64, i64)> {
    let mut chunks = chunks.into_iter();
    let first = chunks.next()?;
    Some(chunks.fold(
        (first[0], first[0], first[1], first[1]),
        |(min_x, max_x, min_z, max_z), chunk| {
            (
                min_x.min(chunk[0]),
                max_x.max(chunk[0]),
                min_z.min(chunk[1]),
                max_z.max(chunk[1]),
            )
        },
    ))
}

fn inclusive_span_u32(minimum: i64, maximum: i64, label: &str) -> Result<u32, String> {
    maximum
        .checked_sub(minimum)
        .and_then(|span| span.checked_add(1))
        .and_then(|span| u32::try_from(span).ok())
        .ok_or_else(|| format!("terrain frontier {label} exceeds u32"))
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
    value / divisor + u32::from(!value.is_multiple_of(divisor))
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

#[cfg(target_arch = "wasm32")]
fn frontier_timing_now() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn frontier_timing_now() -> std::time::Instant {
    std::time::Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn frontier_timing_elapsed_micros(started: f64) -> u64 {
    ((js_sys::Date::now() - started).max(0.0) * 1_000.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn frontier_timing_elapsed_micros(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerrainClipmap, TerrainClipmapConfig, TerrainExactBoundaryColumn,
        terrain_exact_exposed_boundary_blocks,
    };
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

    fn solid_boundary(
        coverage: &ExactPaintedCoverageSnapshot,
        topology: HorizontalTopology,
    ) -> TerrainExactBoundaryProfile {
        let columns = terrain_exact_exposed_boundary_blocks(coverage, topology)
            .unwrap()
            .into_iter()
            .map(|[world_x, world_z]| TerrainExactBoundaryColumn {
                world_x,
                world_z,
                solid_top_y: 72,
                side_material: Some(4),
                water: false,
            });
        TerrainExactBoundaryProfile::from_columns(coverage, columns).unwrap()
    }

    fn plan_square(radius: i32, phase: [i32; 2]) -> TerrainFrontierPlan {
        let center = ChunkPos::new(phase[0], phase[1]);
        let coverage =
            ExactPaintedCoverageSnapshot::new(source(), 7, square(radius, center)).unwrap();
        let boundary = solid_boundary(&coverage, HorizontalTopology::UNBOUNDED);
        let center_blocks = [center.x * 16 + 8, center.z * 16 + 8];
        TerrainFrontierPlan::prepare(
            &coverage,
            &boundary,
            HorizontalTopology::UNBOUNDED,
            [i64::from(center_blocks[0]), i64::from(center_blocks[1])],
            &clipmap_levels(center_blocks),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn terrain_resource_receipt_matches_renderer_allocation() {
        assert_eq!(TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES, 560_612);
    }

    #[test]
    fn radius_two_is_spacing_one_supported_in_every_tile_phase() {
        for phase_z in 0..4 {
            for phase_x in 0..4 {
                let plan = plan_square(2, [phase_x, phase_z]);
                let receipt = plan.receipt();
                assert_eq!(receipt.exposed_segments, 320);
                assert_eq!(receipt.spacing_one_connector_segments, 320);
                assert_eq!(receipt.unsupported_spacing_segments, 0);
                assert_eq!(receipt.maximum_adjacent_spacing, 1);
                assert_eq!(receipt.state, TerrainFrontierPlanState::CurrentComplete);
            }
        }
    }

    #[test]
    fn render_distance_matrix_reports_the_worst_phase_spacing() {
        for (radius, expected_spacing) in [(2, 1), (5, 2), (8, 4), (13, 4)] {
            let mut observed_maximum = 0;
            // Sweep enough chunk phases to cover the tile phase of every
            // potentially adjacent level, not only level zero's four phases.
            for phase in 0..16 {
                observed_maximum = observed_maximum.max(
                    plan_square(radius, [phase, phase])
                        .receipt()
                        .maximum_adjacent_spacing,
                );
            }
            assert_eq!(observed_maximum, expected_spacing, "radius {radius}");
        }
    }

    #[test]
    fn radius_eight_exposes_currently_unclosed_segments() {
        let receipt = plan_square(8, [0, 0]).receipt();
        assert_eq!(receipt.exposed_segments, 1_088);
        assert!(receipt.unsupported_spacing_segments > 0);
        assert!(receipt.current_unclosed_segments > 0);
        assert_eq!(receipt.state, TerrainFrontierPlanState::CurrentIncomplete);
        assert!(receipt.candidates.sparse_additional_fine_tiles > 0);
        assert!(receipt.candidates.resolution_aware_connector_segments > 0);
    }

    #[test]
    fn radius_thirty_one_full_finest_candidate_includes_atomic_staging() {
        let candidates = plan_square(31, [0, 0]).receipt().candidates;
        assert_eq!(candidates.full_finest_tiles_per_axis, 18);
        assert_eq!(candidates.full_finest_logical_tiles, 324);
        assert_eq!(candidates.full_finest_staging_tiles, 35);
        assert_eq!(candidates.full_finest_added_bytes, 188_365_632);
    }

    #[test]
    fn radius_thirty_two_is_reported_format_invalid_before_snapshot_packing() {
        let receipt = terrain_frontier_format_capacity(&square(32, ChunkPos::new(0, 0))).unwrap();
        assert_eq!(
            (
                receipt.canonical_chunk_width,
                receipt.canonical_chunk_height
            ),
            (65, 65)
        );
        assert_eq!(
            (
                receipt.boundary_width_blocks,
                receipt.boundary_height_blocks
            ),
            (1_040, 1_040)
        );
        assert_eq!(
            (
                receipt.transition_width_texels,
                receipt.transition_height_texels
            ),
            (276, 276)
        );
        assert!(!receipt.mask_valid);
        assert!(!receipt.boundary_valid);
        assert!(!receipt.transition_valid);
        assert!(!receipt.all_valid);
    }

    #[test]
    fn irregular_connected_shapes_preserve_every_directed_edge() {
        let fixtures = [
            BTreeSet::from([
                ChunkPos::new(0, 0),
                ChunkPos::new(1, 0),
                ChunkPos::new(0, 1),
            ]),
            BTreeSet::from([
                ChunkPos::new(0, 0),
                ChunkPos::new(1, 0),
                ChunkPos::new(1, 1),
                ChunkPos::new(2, 1),
                ChunkPos::new(2, 2),
            ]),
            (-2..=2)
                .flat_map(|z| {
                    (-2..=2)
                        .filter(move |x| !(*x == 0 && z == 0))
                        .map(move |x| ChunkPos::new(x, z))
                })
                .collect(),
            (-4..=4)
                .flat_map(|x| {
                    let tooth = (x % 2 == 0).then_some(ChunkPos::new(x, 1));
                    std::iter::once(ChunkPos::new(x, 0)).chain(tooth)
                })
                .collect(),
        ];
        for (generation, chunks) in fixtures.into_iter().enumerate() {
            let coverage = ExactPaintedCoverageSnapshot::new(
                source(),
                generation as u64 + 1,
                chunks.iter().copied(),
            )
            .unwrap();
            let boundary = solid_boundary(&coverage, HorizontalTopology::UNBOUNDED);
            let plan = TerrainFrontierPlan::prepare(
                &coverage,
                &boundary,
                HorizontalTopology::UNBOUNDED,
                [0, 0],
                &clipmap_levels([0, 0]),
                TerrainFrontierPlanOptions::default(),
            )
            .unwrap();
            let expected_chunk_sides = chunks
                .iter()
                .flat_map(|chunk| {
                    TerrainFrontierDirection::ALL.map(|(_, dx, dz)| {
                        !chunks.contains(&ChunkPos::new(chunk.x + dx, chunk.z + dz))
                    })
                })
                .filter(|exposed| *exposed)
                .count();
            assert_eq!(plan.segments().len(), expected_chunk_sides * 16);
        }
    }

    #[test]
    fn missing_profiles_and_water_are_not_silently_certified() {
        let coverage =
            ExactPaintedCoverageSnapshot::new(source(), 3, [ChunkPos::new(0, 0)]).unwrap();
        let empty = TerrainExactBoundaryProfile::empty(&coverage).unwrap();
        let missing = TerrainFrontierPlan::prepare(
            &coverage,
            &empty,
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            &clipmap_levels([8, 8]),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap()
        .receipt();
        assert_eq!(missing.missing_exact_profile_segments, 64);
        assert_eq!(missing.current_unclosed_segments, 64);

        let water_columns =
            terrain_exact_exposed_boundary_blocks(&coverage, HorizontalTopology::UNBOUNDED)
                .unwrap()
                .into_iter()
                .map(|[world_x, world_z]| TerrainExactBoundaryColumn {
                    world_x,
                    world_z,
                    solid_top_y: 61,
                    side_material: Some(3),
                    water: true,
                });
        let water = TerrainExactBoundaryProfile::from_columns(&coverage, water_columns).unwrap();
        let water_receipt = TerrainFrontierPlan::prepare(
            &coverage,
            &water,
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            &clipmap_levels([8, 8]),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap()
        .receipt();
        assert_eq!(water_receipt.water_segments, 64);
        assert_eq!(water_receipt.uncertified_water_segments, 64);
        assert_eq!(water_receipt.spacing_one_connector_segments, 0);
    }

    #[test]
    fn cylinder_seam_uses_one_compact_observer_local_lift() {
        let topology = HorizontalTopology::cylinder_x(-16, 32);
        let coverage = ExactPaintedCoverageSnapshot::new(
            source(),
            9,
            [ChunkPos::new(15, 0), ChunkPos::new(-16, 0)],
        )
        .unwrap();
        let boundary = solid_boundary(&coverage, topology);
        let plan = TerrainFrontierPlan::prepare(
            &coverage,
            &boundary,
            topology,
            [15 * 16 + 8, 8],
            &clipmap_levels([15 * 16 + 8, 8]),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap();
        assert_eq!(plan.receipt().format.canonical_chunk_width, 32);
        assert_eq!(plan.receipt().format.lifted_chunk_width, 2);
        assert!(plan.receipt().format.canonical_packing_expands_local_extent);
        assert!(
            plan.segments()
                .iter()
                .any(|segment| segment.lifted_exact_block[0] >= 16 * 16)
        );
    }

    #[test]
    fn disconnected_exact_coverage_is_rejected_by_the_plan() {
        let coverage = ExactPaintedCoverageSnapshot::new(
            source(),
            4,
            [ChunkPos::new(0, 0), ChunkPos::new(4, 0)],
        )
        .unwrap();
        let boundary = solid_boundary(&coverage, HorizontalTopology::UNBOUNDED);
        let error = TerrainFrontierPlan::prepare(
            &coverage,
            &boundary,
            HorizontalTopology::UNBOUNDED,
            [0, 0],
            &clipmap_levels([0, 0]),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap_err();
        assert_eq!(error, "terrain frontier exact coverage is disconnected");
    }

    #[test]
    fn committed_presentation_identity_changes_across_a_tile_rebase() {
        let before = clipmap_levels([63, 0]);
        let after = clipmap_levels([64, 0]);
        assert_ne!(
            terrain_frontier_presentation_identity(&before),
            terrain_frontier_presentation_identity(&after)
        );
    }
}
