//! Standalone semantic-terrain reconstruction sandbox.
//!
//! This research-only sampler turns the multiscale witness's range and basin
//! facts into continuous heightfields. It is intentionally disconnected from
//! production profiles and chunk generation.

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::multiscale_terrain_witness::{
    MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS, MULTISCALE_WITNESS_PARENT_BLOCKS,
    MultiscaleWitnessDetail, MultiscaleWitnessFamily, build_feature_hierarchy,
    canonical_parent_owner, multiscale_feature_offsets,
};
use crate::streamed_plan_harness::{
    PlanRegion, STREAMED_PLAN_PERIOD_BLOCKS, STREAMED_PLAN_PHASE_ONE_SEEDS, StreamedPlanDescriptor,
    StreamedPlanTopology, StreamedSemanticFact,
};

pub const SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION: &str = "mclone-semantic-terrain-sandbox-v1";
pub const SEMANTIC_TERRAIN_SANDBOX_REVISION: &str = "semantic-terrain-reconstruction-v1";
pub const SEMANTIC_TERRAIN_DEFAULT_SAMPLES_ACROSS: u32 = 65;
pub const SEMANTIC_TERRAIN_MAX_SAMPLES_ACROSS: u32 = 129;
pub const SEMANTIC_TERRAIN_HEIGHT_QUANTIZATION: i32 = 256;
pub const SEMANTIC_TERRAIN_SANDBOX_SUITE_SHA256: &str =
    "14250ea1a92a72246abfd256d3ffb2caca021a5805a4be692299bfecc5017f8e";

const FLAT_HEIGHT: f64 = 64.0;
const WATER_HEIGHT: f64 = 56.0;
const RANGE_SUPPORT: f64 = 1_024.0;
const BASIN_SUPPORT: f64 = 768.0;
const RANGE_AMPLITUDE: f64 = 54.0;
const BASIN_AMPLITUDE: f64 = 29.0;
const BASIN_CHANNEL_WIDTH: f64 = 112.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticTerrainSubstrate {
    Flat,
    Quiet,
}

impl SemanticTerrainSubstrate {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Quiet => "quiet",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "flat" => Some(Self::Flat),
            "quiet" => Some(Self::Quiet),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticTerrainFeatureMode {
    Range,
    Basin,
    Combined,
}

impl SemanticTerrainFeatureMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Range => "range",
            Self::Basin => "basin",
            Self::Combined => "combined",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "range" => Some(Self::Range),
            "basin" => Some(Self::Basin),
            "combined" => Some(Self::Combined),
            _ => None,
        }
    }

    fn includes(self, family: MultiscaleWitnessFamily) -> bool {
        matches!(
            (self, family),
            (Self::Range, MultiscaleWitnessFamily::RangeAxis)
                | (Self::Basin, MultiscaleWitnessFamily::BasinRoute)
                | (Self::Combined, _)
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SemanticTerrainSandboxRequest {
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub center_x: f64,
    pub center_z: f64,
    pub blocks_across: f64,
    pub aspect_ratio: f64,
    pub samples_across: u32,
    pub substrate: SemanticTerrainSubstrate,
    pub features: SemanticTerrainFeatureMode,
}

impl SemanticTerrainSandboxRequest {
    pub const fn new(
        seed: i64,
        topology: StreamedPlanTopology,
        center_x: f64,
        center_z: f64,
        blocks_across: f64,
        aspect_ratio: f64,
        samples_across: u32,
        substrate: SemanticTerrainSubstrate,
        features: SemanticTerrainFeatureMode,
    ) -> Self {
        Self {
            seed,
            topology,
            center_x,
            center_z,
            blocks_across,
            aspect_ratio,
            samples_across,
            substrate,
            features,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticTerrainDetailMetrics {
    pub detail: MultiscaleWitnessDetail,
    pub owner_count: u32,
    pub feature_count: u32,
    pub segment_count: u32,
    pub distance_evaluation_count: u64,
    pub semantic_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticTerrainCorrectionMetrics {
    pub minimum: f32,
    pub maximum: f32,
    pub rms: f32,
    pub changed_sample_fraction: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticTerrainGuide {
    pub detail: MultiscaleWitnessDetail,
    pub family: MultiscaleWitnessFamily,
    pub start_x: f32,
    pub start_z: f32,
    pub end_x: f32,
    pub end_z: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticTerrainSandboxMetadata {
    pub receipt_schema: &'static str,
    pub revision: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub substrate: SemanticTerrainSubstrate,
    pub features: SemanticTerrainFeatureMode,
    pub center_x: f64,
    pub center_z: f64,
    pub blocks_across: f64,
    pub blocks_tall: f64,
    pub columns: u32,
    pub rows: u32,
    pub sample_count: u32,
    pub minimum_height: f32,
    pub maximum_height: f32,
    pub parent: SemanticTerrainDetailMetrics,
    pub regional: SemanticTerrainDetailMetrics,
    pub local: SemanticTerrainDetailMetrics,
    pub regional_correction: SemanticTerrainCorrectionMetrics,
    pub local_correction: SemanticTerrainCorrectionMetrics,
    pub semantic_sha256: String,
    pub terrain_sha256: String,
    pub guides: Vec<SemanticTerrainGuide>,
}

#[derive(Clone, Debug)]
pub struct SemanticTerrainSandboxGrid {
    pub foundation: Vec<f32>,
    pub parent_heights: Vec<f32>,
    pub regional_heights: Vec<f32>,
    pub local_heights: Vec<f32>,
    pub regional_correction: Vec<f32>,
    pub local_correction: Vec<f32>,
    pub parent_water: Vec<u8>,
    pub regional_water: Vec<u8>,
    pub local_water: Vec<u8>,
    pub metadata: SemanticTerrainSandboxMetadata,
}

#[derive(Clone, Debug)]
pub struct SemanticTerrainDetailGrid {
    pub heights: Vec<f32>,
    pub metrics: SemanticTerrainDetailMetrics,
    pub terrain_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticTerrainSandboxCaseReceipt {
    pub seed: i64,
    pub topology: StreamedPlanTopology,
    pub sample_count: u32,
    pub parent_feature_count: u32,
    pub regional_feature_count: u32,
    pub local_feature_count: u32,
    pub traversal_mismatch_count: u32,
    pub periodic_lift_mismatch_count: u32,
    pub terrain_sha256: String,
    pub passed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticTerrainSandboxSuiteReceipt {
    pub receipt_schema: &'static str,
    pub revision: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub cases: Vec<SemanticTerrainSandboxCaseReceipt>,
    pub suite_sha256: String,
    pub passed: bool,
}

#[derive(Clone, Copy, Debug)]
enum SampleTraversal {
    Raster,
    Reverse,
    EvenOdd,
}

#[derive(Clone, Debug)]
struct ReconstructionFeature {
    family: MultiscaleWitnessFamily,
    anchor_x: i32,
    anchor_z: i32,
    start_x: i32,
    start_z: i32,
    end_x: i32,
    end_z: i32,
    identity: Vec<u8>,
}

#[derive(Clone, Debug)]
struct DetailCourse {
    detail: MultiscaleWitnessDetail,
    owners: BTreeSet<PlanRegion>,
    features: Vec<ReconstructionFeature>,
    semantic_sha256: String,
}

pub fn compile_semantic_terrain_sandbox(
    request: SemanticTerrainSandboxRequest,
) -> Result<SemanticTerrainSandboxGrid, String> {
    compile_with_traversal(request, SampleTraversal::Raster)
}

pub fn compile_semantic_terrain_detail(
    request: SemanticTerrainSandboxRequest,
    detail: MultiscaleWitnessDetail,
) -> Result<SemanticTerrainDetailGrid, String> {
    validate_request(request)?;
    let columns = request.samples_across;
    let rows = ((f64::from(columns - 1) / request.aspect_ratio).round() as u32 + 1)
        .clamp(2, SEMANTIC_TERRAIN_MAX_SAMPLES_ACROSS);
    let blocks_tall = request.blocks_across / request.aspect_ratio;
    let descriptor = StreamedPlanDescriptor::new(request.seed, request.topology);
    let course = compile_course(&descriptor, request, detail)?;
    let sample_count = columns as usize * rows as usize;
    let mut heights = vec![0.0; sample_count];
    for (index, height) in heights.iter_mut().enumerate() {
        let column = index % columns as usize;
        let row = index / columns as usize;
        let world_x = lattice_coordinate(
            request.center_x,
            request.blocks_across,
            column as u32,
            columns,
        );
        let world_z = lattice_coordinate(request.center_z, blocks_tall, row as u32, rows);
        let base = substrate_height(request, world_x, world_z);
        *height = quantize_height(base + reconstruct(&course, request, world_x, world_z));
    }
    let mut digest = Sha256::new();
    digest.update(SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION.as_bytes());
    digest.update([detail.level()]);
    for height in &heights {
        let quantized = (*height * SEMANTIC_TERRAIN_HEIGHT_QUANTIZATION as f32).round() as i32;
        digest.update(quantized.to_le_bytes());
    }
    Ok(SemanticTerrainDetailGrid {
        metrics: detail_metrics(&course, sample_count),
        heights,
        terrain_sha256: sha256_hex(digest.finalize()),
    })
}

pub fn run_semantic_terrain_sandbox_suite() -> Result<SemanticTerrainSandboxSuiteReceipt, String> {
    let mut cases = Vec::new();
    for seed in STREAMED_PLAN_PHASE_ONE_SEEDS {
        for topology in StreamedPlanTopology::ALL {
            let request = SemanticTerrainSandboxRequest::new(
                seed,
                topology,
                1_024.0,
                -768.0,
                6_144.0,
                1.0,
                33,
                SemanticTerrainSubstrate::Quiet,
                SemanticTerrainFeatureMode::Combined,
            );
            let raster = compile_with_traversal(request, SampleTraversal::Raster)?;
            let mut traversal_mismatch_count = 0_u32;
            for traversal in [SampleTraversal::Reverse, SampleTraversal::EvenOdd] {
                let comparison = compile_with_traversal(request, traversal)?;
                if raster.metadata.terrain_sha256 != comparison.metadata.terrain_sha256
                    || raster.local_heights != comparison.local_heights
                {
                    traversal_mismatch_count = traversal_mismatch_count.saturating_add(1);
                }
            }
            let mut periodic_lift_mismatch_count = 0_u32;
            if topology != StreamedPlanTopology::Plane {
                let lifted = SemanticTerrainSandboxRequest {
                    center_x: request.center_x + f64::from(STREAMED_PLAN_PERIOD_BLOCKS),
                    center_z: if topology == StreamedPlanTopology::Torus {
                        request.center_z - f64::from(STREAMED_PLAN_PERIOD_BLOCKS)
                    } else {
                        request.center_z
                    },
                    ..request
                };
                let comparison = compile_semantic_terrain_sandbox(lifted)?;
                if raster.metadata.terrain_sha256 != comparison.metadata.terrain_sha256
                    || raster.local_heights != comparison.local_heights
                {
                    periodic_lift_mismatch_count = periodic_lift_mismatch_count.saturating_add(1);
                }
            }
            cases.push(SemanticTerrainSandboxCaseReceipt {
                seed,
                topology,
                sample_count: raster.metadata.sample_count,
                parent_feature_count: raster.metadata.parent.feature_count,
                regional_feature_count: raster.metadata.regional.feature_count,
                local_feature_count: raster.metadata.local.feature_count,
                traversal_mismatch_count,
                periodic_lift_mismatch_count,
                terrain_sha256: raster.metadata.terrain_sha256,
                passed: traversal_mismatch_count == 0 && periodic_lift_mismatch_count == 0,
            });
        }
    }
    let suite_sha256 = suite_sha256(&cases);
    let passed = cases.iter().all(|case| case.passed);
    Ok(SemanticTerrainSandboxSuiteReceipt {
        receipt_schema: SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION,
        revision: SEMANTIC_TERRAIN_SANDBOX_REVISION,
        research_only: true,
        production_terrain_unchanged: true,
        cases,
        suite_sha256,
        passed,
    })
}

fn compile_with_traversal(
    request: SemanticTerrainSandboxRequest,
    traversal: SampleTraversal,
) -> Result<SemanticTerrainSandboxGrid, String> {
    validate_request(request)?;
    let columns = request.samples_across;
    let rows = ((f64::from(columns - 1) / request.aspect_ratio).round() as u32 + 1)
        .clamp(2, SEMANTIC_TERRAIN_MAX_SAMPLES_ACROSS);
    let blocks_tall = request.blocks_across / request.aspect_ratio;
    let descriptor = StreamedPlanDescriptor::new(request.seed, request.topology);
    let courses = [
        compile_course(&descriptor, request, MultiscaleWitnessDetail::Parent)?,
        compile_course(&descriptor, request, MultiscaleWitnessDetail::Regional)?,
        compile_course(&descriptor, request, MultiscaleWitnessDetail::Local)?,
    ];
    let sample_count = columns as usize * rows as usize;
    let mut foundation = vec![0.0; sample_count];
    let mut parent_heights = vec![0.0; sample_count];
    let mut regional_heights = vec![0.0; sample_count];
    let mut local_heights = vec![0.0; sample_count];
    let order = sample_order(sample_count, traversal);
    for index in order {
        let column = index % columns as usize;
        let row = index / columns as usize;
        let world_x = lattice_coordinate(
            request.center_x,
            request.blocks_across,
            column as u32,
            columns,
        );
        let world_z = lattice_coordinate(request.center_z, blocks_tall, row as u32, rows);
        let base = substrate_height(request, world_x, world_z);
        foundation[index] = quantize_height(base);
        parent_heights[index] =
            quantize_height(base + reconstruct(&courses[0], request, world_x, world_z));
        regional_heights[index] =
            quantize_height(base + reconstruct(&courses[1], request, world_x, world_z));
        local_heights[index] =
            quantize_height(base + reconstruct(&courses[2], request, world_x, world_z));
    }
    let regional_correction = corrections(&regional_heights, &parent_heights);
    let local_correction = corrections(&local_heights, &regional_heights);
    let parent_water = water_occupancy(&parent_heights);
    let regional_water = water_occupancy(&regional_heights);
    let local_water = water_occupancy(&local_heights);
    let regional_correction_metrics = correction_metrics(&regional_correction);
    let local_correction_metrics = correction_metrics(&local_correction);
    let (minimum_height, maximum_height) = height_range([
        parent_heights.as_slice(),
        regional_heights.as_slice(),
        local_heights.as_slice(),
    ]);
    let semantic_sha256 = combined_semantic_sha256(&courses);
    let terrain_sha256 = terrain_sha256(
        request,
        columns,
        rows,
        &foundation,
        &parent_heights,
        &regional_heights,
        &local_heights,
    );
    let guides = courses
        .iter()
        .flat_map(|course| {
            course.features.iter().map(|feature| SemanticTerrainGuide {
                detail: course.detail,
                family: feature.family,
                start_x: feature.anchor_x as f32 + feature.start_x as f32,
                start_z: feature.anchor_z as f32 + feature.start_z as f32,
                end_x: feature.anchor_x as f32 + feature.end_x as f32,
                end_z: feature.anchor_z as f32 + feature.end_z as f32,
            })
        })
        .collect();
    let metrics = courses.map(|course| detail_metrics(&course, sample_count));
    Ok(SemanticTerrainSandboxGrid {
        foundation,
        parent_heights,
        regional_heights,
        local_heights,
        regional_correction,
        local_correction,
        parent_water,
        regional_water,
        local_water,
        metadata: SemanticTerrainSandboxMetadata {
            receipt_schema: SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION,
            revision: SEMANTIC_TERRAIN_SANDBOX_REVISION,
            research_only: true,
            production_terrain_unchanged: true,
            seed: request.seed,
            topology: request.topology,
            substrate: request.substrate,
            features: request.features,
            center_x: request.center_x,
            center_z: request.center_z,
            blocks_across: request.blocks_across,
            blocks_tall,
            columns,
            rows,
            sample_count: sample_count as u32,
            minimum_height,
            maximum_height,
            parent: metrics[0].clone(),
            regional: metrics[1].clone(),
            local: metrics[2].clone(),
            regional_correction: regional_correction_metrics,
            local_correction: local_correction_metrics,
            semantic_sha256,
            terrain_sha256,
            guides,
        },
    })
}

fn validate_request(request: SemanticTerrainSandboxRequest) -> Result<(), String> {
    if !request.center_x.is_finite()
        || !request.center_z.is_finite()
        || !request.blocks_across.is_finite()
        || !request.aspect_ratio.is_finite()
    {
        return Err("semantic terrain request contains a non-finite value".to_owned());
    }
    if request.blocks_across <= 0.0 || request.aspect_ratio <= 0.0 {
        return Err("semantic terrain extent and aspect must be positive".to_owned());
    }
    if !(2..=SEMANTIC_TERRAIN_MAX_SAMPLES_ACROSS).contains(&request.samples_across) {
        return Err(format!(
            "semantic terrain samples across must be between 2 and {}",
            SEMANTIC_TERRAIN_MAX_SAMPLES_ACROSS
        ));
    }
    Ok(())
}

fn compile_course(
    descriptor: &StreamedPlanDescriptor,
    request: SemanticTerrainSandboxRequest,
    detail: MultiscaleWitnessDetail,
) -> Result<DetailCourse, String> {
    let half_x = request.blocks_across * 0.5;
    let half_z = request.blocks_across / request.aspect_ratio * 0.5;
    let padding = f64::from(MULTISCALE_WITNESS_MAXIMUM_INFLUENCE_BLOCKS);
    let minimum_owner_x = ((request.center_x - half_x - padding)
        / f64::from(MULTISCALE_WITNESS_PARENT_BLOCKS))
    .floor() as i32;
    let maximum_owner_x = ((request.center_x + half_x + padding)
        / f64::from(MULTISCALE_WITNESS_PARENT_BLOCKS))
    .floor() as i32;
    let minimum_owner_z = ((request.center_z - half_z - padding)
        / f64::from(MULTISCALE_WITNESS_PARENT_BLOCKS))
    .floor() as i32;
    let maximum_owner_z = ((request.center_z + half_z + padding)
        / f64::from(MULTISCALE_WITNESS_PARENT_BLOCKS))
    .floor() as i32;
    let mut owners = BTreeSet::new();
    for z in minimum_owner_z..=maximum_owner_z {
        for x in minimum_owner_x..=maximum_owner_x {
            owners.insert(canonical_parent_owner(descriptor, PlanRegion::new(x, z))?);
        }
    }
    let mut features = Vec::new();
    for owner in &owners {
        for family in [
            MultiscaleWitnessFamily::RangeAxis,
            MultiscaleWitnessFamily::BasinRoute,
        ] {
            if !request.features.includes(family) {
                continue;
            }
            let hierarchy = build_feature_hierarchy(descriptor, *owner, family, detail)?;
            for fact in hierarchy
                .iter()
                .filter(|fact| fact.id.owner.level == detail.level())
            {
                features.push(reconstruction_feature(*owner, family, fact)?);
            }
        }
    }
    features.sort_by(|left, right| left.identity.cmp(&right.identity));
    let semantic_sha256 = semantic_course_sha256(detail, &features);
    Ok(DetailCourse {
        detail,
        owners,
        features,
        semantic_sha256,
    })
}

fn reconstruction_feature(
    owner: PlanRegion,
    family: MultiscaleWitnessFamily,
    fact: &StreamedSemanticFact,
) -> Result<ReconstructionFeature, String> {
    let offsets = multiscale_feature_offsets(fact)?;
    let mut identity = Vec::new();
    identity.extend_from_slice(family.label().as_bytes());
    identity.push(0);
    identity.push(fact.id.owner.level);
    identity.extend_from_slice(&owner.x.to_le_bytes());
    identity.extend_from_slice(&owner.z.to_le_bytes());
    identity.extend_from_slice(&fact.id.local_index.to_le_bytes());
    identity.extend_from_slice(&fact.world_x.to_le_bytes());
    identity.extend_from_slice(&fact.world_z.to_le_bytes());
    for value in &fact.payload {
        identity.extend_from_slice(&value.to_le_bytes());
    }
    Ok(ReconstructionFeature {
        family,
        anchor_x: fact.world_x,
        anchor_z: fact.world_z,
        start_x: offsets.start_x,
        start_z: offsets.start_z,
        end_x: offsets.end_x,
        end_z: offsets.end_z,
        identity,
    })
}

fn reconstruct(
    course: &DetailCourse,
    request: SemanticTerrainSandboxRequest,
    world_x: f64,
    world_z: f64,
) -> f64 {
    let topology = request.topology.horizontal();
    course
        .features
        .iter()
        .map(|feature| {
            let observer_x = world_x.round() as i32;
            let observer_z = world_z.round() as i32;
            let anchor_x = f64::from(observer_x)
                + topology
                    .x
                    .shortest_block_displacement(observer_x, feature.anchor_x)
                    as f64;
            let anchor_z = f64::from(observer_z)
                + topology
                    .z
                    .shortest_block_displacement(observer_z, feature.anchor_z)
                    as f64;
            let start_x = anchor_x + f64::from(feature.start_x);
            let start_z = anchor_z + f64::from(feature.start_z);
            let end_x = anchor_x + f64::from(feature.end_x);
            let end_z = anchor_z + f64::from(feature.end_z);
            let distance = point_segment_distance(world_x, world_z, start_x, start_z, end_x, end_z);
            match feature.family {
                MultiscaleWitnessFamily::RangeAxis => {
                    RANGE_AMPLITUDE * compact_kernel(distance / RANGE_SUPPORT)
                }
                MultiscaleWitnessFamily::BasinRoute => {
                    let broad = compact_kernel(distance / BASIN_SUPPORT);
                    let channel = compact_kernel(distance / BASIN_CHANNEL_WIDTH);
                    -BASIN_AMPLITUDE * (broad * 0.78 + channel * 0.22)
                }
            }
        })
        .sum()
}

fn substrate_height(request: SemanticTerrainSandboxRequest, world_x: f64, world_z: f64) -> f64 {
    match request.substrate {
        SemanticTerrainSubstrate::Flat => FLAT_HEIGHT,
        SemanticTerrainSubstrate::Quiet => {
            FLAT_HEIGHT
                + periodic_value_noise(request.seed ^ 0x32a7_4d19, world_x, world_z, 1_536.0) * 5.0
                + periodic_value_noise(request.seed ^ 0x6b13_908f, world_x, world_z, 768.0) * 2.0
        }
    }
}

fn periodic_value_noise(seed: i64, world_x: f64, world_z: f64, cell: f64) -> f64 {
    let period_cells = (f64::from(STREAMED_PLAN_PERIOD_BLOCKS) / cell) as i64;
    let grid_x = (world_x / cell).floor() as i64;
    let grid_z = (world_z / cell).floor() as i64;
    let fraction_x = world_x / cell - grid_x as f64;
    let fraction_z = world_z / cell - grid_z as f64;
    let smooth_x = smoothstep(fraction_x);
    let smooth_z = smoothstep(fraction_z);
    let corner = |x: i64, z: i64| {
        let canonical_x = x.rem_euclid(period_cells);
        let canonical_z = z.rem_euclid(period_cells);
        unit_hash(seed, canonical_x, canonical_z)
    };
    let bottom = lerp(corner(grid_x, grid_z), corner(grid_x + 1, grid_z), smooth_x);
    let top = lerp(
        corner(grid_x, grid_z + 1),
        corner(grid_x + 1, grid_z + 1),
        smooth_x,
    );
    lerp(bottom, top, smooth_z)
}

fn unit_hash(seed: i64, x: i64, z: i64) -> f64 {
    let hash = mix64(seed as u64 ^ (x as u64).rotate_left(21) ^ (z as u64).rotate_left(43));
    let fraction = (hash >> 11) as f64 / ((1_u64 << 53) - 1) as f64;
    fraction * 2.0 - 1.0
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn point_segment_distance(
    point_x: f64,
    point_z: f64,
    start_x: f64,
    start_z: f64,
    end_x: f64,
    end_z: f64,
) -> f64 {
    let segment_x = end_x - start_x;
    let segment_z = end_z - start_z;
    let length_squared = segment_x * segment_x + segment_z * segment_z;
    if length_squared == 0.0 {
        return ((point_x - start_x).powi(2) + (point_z - start_z).powi(2)).sqrt();
    }
    let t = (((point_x - start_x) * segment_x + (point_z - start_z) * segment_z) / length_squared)
        .clamp(0.0, 1.0);
    let closest_x = start_x + segment_x * t;
    let closest_z = start_z + segment_z * t;
    ((point_x - closest_x).powi(2) + (point_z - closest_z).powi(2)).sqrt()
}

fn compact_kernel(normalized_distance: f64) -> f64 {
    if normalized_distance >= 1.0 {
        0.0
    } else {
        let remaining = 1.0 - normalized_distance * normalized_distance;
        remaining * remaining
    }
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp(left: f64, right: f64, amount: f64) -> f64 {
    left + (right - left) * amount
}

fn lattice_coordinate(center: f64, extent: f64, index: u32, count: u32) -> f64 {
    center - extent * 0.5 + extent * f64::from(index) / f64::from(count - 1)
}

fn quantize_height(height: f64) -> f32 {
    (height * f64::from(SEMANTIC_TERRAIN_HEIGHT_QUANTIZATION)).round() as f32
        / SEMANTIC_TERRAIN_HEIGHT_QUANTIZATION as f32
}

fn corrections(finer: &[f32], coarser: &[f32]) -> Vec<f32> {
    finer
        .iter()
        .zip(coarser)
        .map(|(finer, coarser)| *finer - *coarser)
        .collect()
}

fn water_occupancy(heights: &[f32]) -> Vec<u8> {
    heights
        .iter()
        .map(|height| u8::from(*height < WATER_HEIGHT as f32))
        .collect()
}

fn correction_metrics(values: &[f32]) -> SemanticTerrainCorrectionMetrics {
    let mut minimum = f32::INFINITY;
    let mut maximum = f32::NEG_INFINITY;
    let mut sum_squared = 0.0_f64;
    let mut changed = 0_u32;
    for value in values {
        minimum = minimum.min(*value);
        maximum = maximum.max(*value);
        sum_squared += f64::from(*value) * f64::from(*value);
        changed += u32::from(*value != 0.0);
    }
    SemanticTerrainCorrectionMetrics {
        minimum,
        maximum,
        rms: (sum_squared / values.len() as f64).sqrt() as f32,
        changed_sample_fraction: changed as f32 / values.len() as f32,
    }
}

fn height_range<const N: usize>(sets: [&[f32]; N]) -> (f32, f32) {
    let mut minimum = f32::INFINITY;
    let mut maximum = f32::NEG_INFINITY;
    for value in sets.into_iter().flatten() {
        minimum = minimum.min(*value);
        maximum = maximum.max(*value);
    }
    (minimum, maximum)
}

fn detail_metrics(course: &DetailCourse, sample_count: usize) -> SemanticTerrainDetailMetrics {
    SemanticTerrainDetailMetrics {
        detail: course.detail,
        owner_count: course.owners.len() as u32,
        feature_count: course.features.len() as u32,
        segment_count: course.features.len() as u32,
        distance_evaluation_count: (course.features.len() * sample_count) as u64,
        semantic_sha256: course.semantic_sha256.clone(),
    }
}

fn semantic_course_sha256(
    detail: MultiscaleWitnessDetail,
    features: &[ReconstructionFeature],
) -> String {
    let mut digest = Sha256::new();
    digest.update(SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION.as_bytes());
    digest.update([detail.level()]);
    for feature in features {
        digest.update(&feature.identity);
    }
    sha256_hex(digest.finalize())
}

fn combined_semantic_sha256(courses: &[DetailCourse; 3]) -> String {
    let mut digest = Sha256::new();
    digest.update(SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION.as_bytes());
    for course in courses {
        digest.update(course.semantic_sha256.as_bytes());
    }
    sha256_hex(digest.finalize())
}

fn terrain_sha256(
    request: SemanticTerrainSandboxRequest,
    columns: u32,
    rows: u32,
    foundation: &[f32],
    parent: &[f32],
    regional: &[f32],
    local: &[f32],
) -> String {
    let mut digest = Sha256::new();
    digest.update(SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION.as_bytes());
    digest.update(request.seed.to_le_bytes());
    digest.update(request.topology.label().as_bytes());
    digest.update(request.substrate.label().as_bytes());
    digest.update(request.features.label().as_bytes());
    digest.update(columns.to_le_bytes());
    digest.update(rows.to_le_bytes());
    for values in [foundation, parent, regional, local] {
        for value in values {
            let quantized = (*value * SEMANTIC_TERRAIN_HEIGHT_QUANTIZATION as f32).round() as i32;
            digest.update(quantized.to_le_bytes());
        }
    }
    sha256_hex(digest.finalize())
}

fn suite_sha256(cases: &[SemanticTerrainSandboxCaseReceipt]) -> String {
    let mut digest = Sha256::new();
    digest.update(SEMANTIC_TERRAIN_SANDBOX_SCHEMA_REVISION.as_bytes());
    digest.update(SEMANTIC_TERRAIN_SANDBOX_REVISION.as_bytes());
    for case in cases {
        digest.update(case.seed.to_le_bytes());
        digest.update(case.topology.label().as_bytes());
        digest.update(case.sample_count.to_le_bytes());
        digest.update(case.parent_feature_count.to_le_bytes());
        digest.update(case.regional_feature_count.to_le_bytes());
        digest.update(case.local_feature_count.to_le_bytes());
        digest.update(case.traversal_mismatch_count.to_le_bytes());
        digest.update(case.periodic_lift_mismatch_count.to_le_bytes());
        digest.update(case.terrain_sha256.as_bytes());
        digest.update([u8::from(case.passed)]);
    }
    sha256_hex(digest.finalize())
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sample_order(sample_count: usize, traversal: SampleTraversal) -> Vec<usize> {
    match traversal {
        SampleTraversal::Raster => (0..sample_count).collect(),
        SampleTraversal::Reverse => (0..sample_count).rev().collect(),
        SampleTraversal::EvenOdd => (0..sample_count)
            .step_by(2)
            .chain((1..sample_count).step_by(2))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use std::thread;

    use super::*;

    fn request(
        topology: StreamedPlanTopology,
        substrate: SemanticTerrainSubstrate,
        features: SemanticTerrainFeatureMode,
    ) -> SemanticTerrainSandboxRequest {
        SemanticTerrainSandboxRequest::new(
            12_345, topology, 1_024.0, -768.0, 6_144.0, 1.0, 33, substrate, features,
        )
    }

    #[test]
    fn flat_range_is_attributable_and_refines() {
        let grid = compile_semantic_terrain_sandbox(request(
            StreamedPlanTopology::Plane,
            SemanticTerrainSubstrate::Flat,
            SemanticTerrainFeatureMode::Range,
        ))
        .unwrap();
        assert!(grid.foundation.iter().all(|height| *height == 64.0));
        assert!(grid.metadata.maximum_height > 80.0);
        assert!(grid.metadata.local_correction.changed_sample_fraction > 0.0);
        assert_eq!(grid.metadata.parent.feature_count % 1, 0);
        assert!(grid.metadata.regional.feature_count >= grid.metadata.parent.feature_count * 2);
        assert!(grid.metadata.local.feature_count >= grid.metadata.parent.feature_count * 4);
    }

    #[test]
    fn basin_carves_and_marks_visual_water() {
        let grid = compile_semantic_terrain_sandbox(request(
            StreamedPlanTopology::Plane,
            SemanticTerrainSubstrate::Flat,
            SemanticTerrainFeatureMode::Basin,
        ))
        .unwrap();
        assert!(grid.metadata.minimum_height < WATER_HEIGHT as f32);
        assert!(grid.local_water.contains(&1));
        assert!(grid.metadata.local_correction.changed_sample_fraction > 0.0);
    }

    #[test]
    fn traversal_order_is_exact() {
        let request = request(
            StreamedPlanTopology::Torus,
            SemanticTerrainSubstrate::Quiet,
            SemanticTerrainFeatureMode::Combined,
        );
        let raster = compile_with_traversal(request, SampleTraversal::Raster).unwrap();
        for traversal in [SampleTraversal::Reverse, SampleTraversal::EvenOdd] {
            let other = compile_with_traversal(request, traversal).unwrap();
            assert_eq!(
                raster.metadata.terrain_sha256,
                other.metadata.terrain_sha256
            );
            assert_eq!(raster.local_heights, other.local_heights);
        }
    }

    #[test]
    fn periodic_lifts_are_exact() {
        for topology in [StreamedPlanTopology::CylinderX, StreamedPlanTopology::Torus] {
            let base = request(
                topology,
                SemanticTerrainSubstrate::Quiet,
                SemanticTerrainFeatureMode::Combined,
            );
            let lifted = SemanticTerrainSandboxRequest {
                center_x: base.center_x + f64::from(STREAMED_PLAN_PERIOD_BLOCKS),
                center_z: if topology == StreamedPlanTopology::Torus {
                    base.center_z - f64::from(STREAMED_PLAN_PERIOD_BLOCKS)
                } else {
                    base.center_z
                },
                ..base
            };
            let left = compile_semantic_terrain_sandbox(base).unwrap();
            let right = compile_semantic_terrain_sandbox(lifted).unwrap();
            assert_eq!(left.metadata.terrain_sha256, right.metadata.terrain_sha256);
            assert_eq!(left.local_heights, right.local_heights);
        }
    }

    #[test]
    fn adjacent_viewports_agree_at_shared_edge() {
        let mut left_request = request(
            StreamedPlanTopology::Plane,
            SemanticTerrainSubstrate::Quiet,
            SemanticTerrainFeatureMode::Combined,
        );
        left_request.center_x = 0.0;
        let right_request = SemanticTerrainSandboxRequest {
            center_x: left_request.blocks_across,
            ..left_request
        };
        let left = compile_semantic_terrain_sandbox(left_request).unwrap();
        let right = compile_semantic_terrain_sandbox(right_request).unwrap();
        let columns = left.metadata.columns as usize;
        for row in 0..left.metadata.rows as usize {
            assert_eq!(
                left.local_heights[row * columns + columns - 1],
                right.local_heights[row * columns]
            );
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn serial_and_parallel_compilation_are_exact() {
        let request = request(
            StreamedPlanTopology::CylinderX,
            SemanticTerrainSubstrate::Quiet,
            SemanticTerrainFeatureMode::Combined,
        );
        let serial = compile_semantic_terrain_sandbox(request).unwrap();
        let parallel = thread::scope(|scope| {
            let first = scope.spawn(|| compile_semantic_terrain_sandbox(request));
            let second = scope.spawn(|| compile_semantic_terrain_sandbox(request));
            (
                first.join().unwrap().unwrap(),
                second.join().unwrap().unwrap(),
            )
        });
        assert_eq!(
            serial.metadata.terrain_sha256,
            parallel.0.metadata.terrain_sha256
        );
        assert_eq!(parallel.0.local_heights, parallel.1.local_heights);
    }

    #[test]
    fn compact_support_leaves_distant_flat_samples_unchanged() {
        assert_eq!(compact_kernel(1.0), 0.0);
        assert_eq!(compact_kernel(4.0), 0.0);
        assert!(compact_kernel(0.0) > 0.0);
    }

    #[test]
    fn pinned_suite_is_exact() {
        let receipt = run_semantic_terrain_sandbox_suite().unwrap();
        assert!(receipt.passed);
        assert_eq!(receipt.suite_sha256, SEMANTIC_TERRAIN_SANDBOX_SUITE_SHA256);
    }
}
