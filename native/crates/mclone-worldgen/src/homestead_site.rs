//! Bounded, profile-neutral site survey for the intro homestead.
//!
//! Sources expose neutral terrain facts. The selector owns the fixed lattice,
//! fit tiers, quantized score, and stable tie-break contract; it never branches
//! on a generation-profile name and never generates a rejected full chunk.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, HorizontalTopology};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::levelgen::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldBiomeRecipe, McloneOverworldSamplingTopology,
    McloneOverworldSurveySampler, mclone_overworld_biome_recipe,
};

pub const HOMESTEAD_SCOUT_REVISION: &str = "intro-homestead-scout-v2";
pub const HOMESTEAD_FLAT_WASM_WITNESS_SHA256: &str =
    "b3a5541432ca00a743d92d1c9f69d94f572bbee3666736e9ad5e0c826c45db89";
pub const PRIMARY_SEARCH_RADIUS: i32 = 384;
pub const PRIMARY_LATTICE_SPACING: i32 = 32;
pub const FALLBACK_SEARCH_RADIUS: i32 = 768;
pub const FALLBACK_LATTICE_SPACING: i32 = 64;
pub const FULL_CORE_HALF_EXTENT: i32 = 48;
pub const COMPACT_CORE_HALF_EXTENT: i32 = 32;
pub const RESERVATION_HALF_EXTENT: i32 = 80;
pub const SCENIC_SURVEY_RADIUS: i32 = 256;
pub const MAX_CANDIDATE_ROTATION_EVALUATIONS: usize = 4_096;
const CORE_SAMPLE_STEP: i32 = 16;
const RETAINED_CANDIDATES: usize = 24;
const TIE_DOMAIN: u64 = 0x686f_6d65_7374_6561;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HomesteadFoundationGradeSpec {
    pub region_id: &'static str,
    pub forward_min: i32,
    pub forward_max: i32,
    pub right_min: i32,
    pub right_max: i32,
    pub feather_blocks: u8,
}

const FULL_FOUNDATION_GRADES: [HomesteadFoundationGradeSpec; 3] = [
    HomesteadFoundationGradeSpec {
        region_id: "cottage-foundation-v1",
        forward_min: -36,
        forward_max: -16,
        right_min: -45,
        right_max: -25,
        feather_blocks: 3,
    },
    HomesteadFoundationGradeSpec {
        region_id: "barn-foundation-v1",
        forward_min: 3,
        forward_max: 33,
        right_min: -46,
        right_max: -25,
        feather_blocks: 3,
    },
    HomesteadFoundationGradeSpec {
        region_id: "coop-foundation-v1",
        forward_min: 10,
        forward_max: 29,
        right_min: 17,
        right_max: 32,
        feather_blocks: 2,
    },
];

const COMPACT_FOUNDATION_GRADES: [HomesteadFoundationGradeSpec; 3] = [
    HomesteadFoundationGradeSpec {
        region_id: "cottage-foundation-v1",
        forward_min: -31,
        forward_max: -13,
        right_min: -32,
        right_max: -14,
        feather_blocks: 3,
    },
    HomesteadFoundationGradeSpec {
        region_id: "barn-foundation-v1",
        forward_min: 3,
        forward_max: 31,
        right_min: -32,
        right_max: -15,
        feather_blocks: 3,
    },
    HomesteadFoundationGradeSpec {
        region_id: "coop-foundation-v1",
        forward_min: 10,
        forward_max: 29,
        right_min: 13,
        right_max: 28,
        feather_blocks: 2,
    },
];

const FULL_PATH_CONTROL_RIGHT_OFFSETS: [i32; 7] = [0, -1, -3, -4, -2, 1, 0];
const COMPACT_PATH_CONTROL_RIGHT_OFFSETS: [i32; 5] = [0, -2, -3, -1, 0];

pub const fn homestead_foundation_grade_specs(
    tier: HomesteadCompositionTier,
) -> &'static [HomesteadFoundationGradeSpec] {
    match tier {
        HomesteadCompositionTier::FullV1 => &FULL_FOUNDATION_GRADES,
        HomesteadCompositionTier::CompactV1 => &COMPACT_FOUNDATION_GRADES,
    }
}

pub const fn homestead_path_min_forward(tier: HomesteadCompositionTier) -> i32 {
    match tier {
        HomesteadCompositionTier::FullV1 => -48,
        HomesteadCompositionTier::CompactV1 => -32,
    }
}

pub const fn homestead_path_control_right_offsets(
    tier: HomesteadCompositionTier,
) -> &'static [i32] {
    match tier {
        HomesteadCompositionTier::FullV1 => &FULL_PATH_CONTROL_RIGHT_OFFSETS,
        HomesteadCompositionTier::CompactV1 => &COMPACT_PATH_CONTROL_RIGHT_OFFSETS,
    }
}

pub const fn homestead_path_right_bounds(tier: HomesteadCompositionTier) -> (i32, i32) {
    match tier {
        HomesteadCompositionTier::FullV1 => (-8, 5),
        HomesteadCompositionTier::CompactV1 => (-7, 4),
    }
}

pub const fn homestead_pond_right_bounds(tier: HomesteadCompositionTier) -> (i32, i32) {
    match tier {
        HomesteadCompositionTier::FullV1 => (22, 35),
        HomesteadCompositionTier::CompactV1 => (18, 28),
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadScoutPass {
    Primary,
    Fallback,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadRotation {
    North,
    East,
    South,
    West,
}

impl HomesteadRotation {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    const fn code(self) -> u64 {
        match self {
            Self::North => 0,
            Self::East => 1,
            Self::South => 2,
            Self::West => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadCompositionTier {
    FullV1,
    CompactV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomesteadSiteRejection {
    OutsideTopology,
    PeriodicSelfOverlap,
    ProtectedContent,
    BuildingCoreWater,
    UnsafeArrival,
    ExcessiveEarthwork,
    InsufficientSupport,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HomesteadSurveyColumn {
    pub surface_y: i32,
    pub slope_milli: u16,
    pub fluid: bool,
    pub protected_content: bool,
    pub meadow_milli: u16,
    pub woodland_milli: u16,
    pub water_milli: u16,
    pub coast_milli: u16,
    pub relief_milli: u16,
}

pub trait HomesteadSurveySource {
    fn sample_column(
        &mut self,
        world_x: i32,
        world_z: i32,
    ) -> Result<HomesteadSurveyColumn, String>;
}

#[derive(Clone, Debug)]
pub struct McloneOverworldHomesteadSurveySource {
    sampler: McloneOverworldSurveySampler,
    cache: BTreeMap<(i32, i32), HomesteadSurveyColumn>,
}

impl McloneOverworldHomesteadSurveySource {
    pub fn new(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        Self {
            sampler: McloneOverworldSurveySampler::new(seed, topology),
            cache: BTreeMap::new(),
        }
    }

    pub fn sampled_column_count(&self) -> usize {
        self.cache.len()
    }
}

impl HomesteadSurveySource for McloneOverworldHomesteadSurveySource {
    fn sample_column(
        &mut self,
        world_x: i32,
        world_z: i32,
    ) -> Result<HomesteadSurveyColumn, String> {
        if let Some(sample) = self.cache.get(&(world_x, world_z)) {
            return Ok(*sample);
        }
        let (landform, planned_stream_start) = self.sampler.sample_landform(world_x, world_z)?;
        let terrain = landform.terrain;
        let biome = mclone_overworld_biome_recipe(landform);
        let woodland_milli = match biome {
            McloneOverworldBiomeRecipe::TemperateWoodland => 900,
            McloneOverworldBiomeRecipe::CoolWetConifer => 760,
            _ => 80,
        };
        let meadow_milli = match biome {
            McloneOverworldBiomeRecipe::TemperateMeadow => 900,
            McloneOverworldBiomeRecipe::WarmDrySteppe => 720,
            _ => 120,
        };
        let sample = HomesteadSurveyColumn {
            surface_y: terrain.surface_y,
            slope_milli: quantize_unit(landform.slope),
            fluid: terrain.watercourse.is_water()
                || terrain.continentalness <= 0.0
                || terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL,
            protected_content: planned_stream_start.is_some(),
            meadow_milli,
            woodland_milli,
            water_milli: quantize_unit(
                terrain
                    .watercourse
                    .channel_influence
                    .max(terrain.watercourse.wetland_influence),
            ),
            coast_milli: quantize_unit(terrain.coast.proximity),
            relief_milli: quantize_unit(terrain.relief.abs()),
        };
        self.cache.insert((world_x, world_z), sample);
        Ok(sample)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FlatGrassHomesteadSurveySource;

impl HomesteadSurveySource for FlatGrassHomesteadSurveySource {
    fn sample_column(
        &mut self,
        _world_x: i32,
        _world_z: i32,
    ) -> Result<HomesteadSurveyColumn, String> {
        Ok(HomesteadSurveyColumn {
            surface_y: 3,
            meadow_milli: 1_000,
            ..HomesteadSurveyColumn::default()
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HomesteadCandidate {
    pub pass: HomesteadScoutPass,
    pub offset_x: i32,
    pub offset_z: i32,
    pub anchor_x: i32,
    pub anchor_z: i32,
    pub rotation: HomesteadRotation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HomesteadSiteMetrics {
    pub sample_count: u16,
    pub dry_support_samples: u16,
    pub fluid_samples: u16,
    pub protected_samples: u16,
    pub surface_min_y: i32,
    pub surface_max_y: i32,
    pub surface_span: u16,
    pub target_surface_y: i32,
    pub arrival_surface_y: i32,
    pub estimated_cut_blocks: u32,
    pub estimated_fill_blocks: u32,
    pub max_cut_or_fill_depth: u16,
    pub slope_p90_milli: u16,
    pub maximum_slope_milli: u16,
    pub tree_clearing_milli: u32,
    pub meadow_milli: u16,
    pub woodland_milli: u16,
    pub nearby_water_milli: u16,
    pub coast_milli: u16,
    pub scenic_relief_milli: u16,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HomesteadScore {
    pub earthwork_blocks: u32,
    pub max_grade_depth: u16,
    pub surface_span: u16,
    pub slope_p90_milli: u16,
    pub tree_clearing_milli: u32,
    pub scenic_penalty: u32,
    pub distance_blocks: u16,
    pub tie_key: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HomesteadCandidateEvaluation {
    pub candidate: HomesteadCandidate,
    pub full_metrics: HomesteadSiteMetrics,
    pub compact_metrics: HomesteadSiteMetrics,
    pub full_rejection: Option<HomesteadSiteRejection>,
    pub compact_rejection: Option<HomesteadSiteRejection>,
    pub full_score: Option<HomesteadScore>,
    pub compact_score: Option<HomesteadScore>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HomesteadBounds2d {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectedHomesteadSite {
    pub candidate: HomesteadCandidate,
    pub tier: HomesteadCompositionTier,
    pub metrics: HomesteadSiteMetrics,
    pub score: HomesteadScore,
    pub arrival: [i32; 3],
    pub facing: HomesteadRotation,
    pub core_bounds: HomesteadBounds2d,
    pub reservation_bounds: HomesteadBounds2d,
    pub scenic_bounds: HomesteadBounds2d,
    pub checksum_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HomesteadScoutReceipt {
    pub revision: String,
    pub seed: i64,
    pub provisional_spawn: [i32; 3],
    pub evaluation_count: usize,
    pub unique_anchor_count: usize,
    pub survey_sample_count: usize,
    pub footprint_refinement_count: usize,
    pub footprint_sample_count: usize,
    pub primary_evaluations: usize,
    pub fallback_evaluations: usize,
    pub full_fit_count: usize,
    pub compact_fit_count: usize,
    pub rejected_count: usize,
    pub selected: Option<SelectedHomesteadSite>,
    pub retained_candidates: Vec<HomesteadCandidateEvaluation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HomesteadScoutError {
    InvalidTopology(String),
    Survey(String),
    NoSafeSite(Box<HomesteadScoutReceipt>),
}

impl std::fmt::Display for HomesteadScoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTopology(message) => {
                write!(formatter, "invalid homestead topology: {message}")
            }
            Self::Survey(message) => write!(formatter, "homestead survey failed: {message}"),
            Self::NoSafeSite(_) => formatter.write_str("no safe homestead site in bounded search"),
        }
    }
}

impl std::error::Error for HomesteadScoutError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HomesteadScoutRequest {
    pub seed: i64,
    pub provisional_spawn: BlockPos,
    pub topology: HorizontalTopology,
}

pub fn compile_homestead_candidates(
    request: HomesteadScoutRequest,
) -> Result<Vec<HomesteadCandidate>, HomesteadScoutError> {
    request
        .topology
        .validate()
        .map_err(|error| HomesteadScoutError::InvalidTopology(error.to_string()))?;
    for axis in [request.topology.x, request.topology.z] {
        if axis.period_chunks().is_some_and(|chunks| {
            i64::from(chunks) * 16 < i64::from(RESERVATION_HALF_EXTENT * 2 + 1)
        }) {
            return Err(HomesteadScoutError::InvalidTopology(
                "period is smaller than the reservation envelope".to_owned(),
            ));
        }
    }

    let mut candidates = Vec::with_capacity(MAX_CANDIDATE_ROTATION_EVALUATIONS);
    let mut seen = BTreeSet::new();
    append_candidate_pass(
        request,
        HomesteadScoutPass::Primary,
        PRIMARY_SEARCH_RADIUS,
        PRIMARY_LATTICE_SPACING,
        false,
        &mut candidates,
        &mut seen,
    );
    append_candidate_pass(
        request,
        HomesteadScoutPass::Fallback,
        FALLBACK_SEARCH_RADIUS,
        FALLBACK_LATTICE_SPACING,
        true,
        &mut candidates,
        &mut seen,
    );
    Ok(candidates)
}

fn append_candidate_pass(
    request: HomesteadScoutRequest,
    pass: HomesteadScoutPass,
    radius: i32,
    spacing: i32,
    outside_primary: bool,
    candidates: &mut Vec<HomesteadCandidate>,
    seen: &mut BTreeSet<(i32, i32, HomesteadRotation)>,
) {
    let mut offsets = Vec::new();
    for offset_z in (-radius..=radius).step_by(spacing as usize) {
        for offset_x in (-radius..=radius).step_by(spacing as usize) {
            if outside_primary
                && offset_x.abs() <= PRIMARY_SEARCH_RADIUS
                && offset_z.abs() <= PRIMARY_SEARCH_RADIUS
            {
                continue;
            }
            offsets.push((offset_x, offset_z));
        }
    }
    offsets.sort_by_key(|&(x, z)| (x.abs().max(z.abs()), x.abs() + z.abs(), z, x));
    for (offset_x, offset_z) in offsets {
        let Some(anchor) = request.topology.canonicalize_block(BlockPos::new(
            request.provisional_spawn.x.saturating_add(offset_x),
            0,
            request.provisional_spawn.z.saturating_add(offset_z),
        )) else {
            continue;
        };
        for rotation in HomesteadRotation::ALL {
            if candidates.len() == MAX_CANDIDATE_ROTATION_EVALUATIONS {
                return;
            }
            if seen.insert((anchor.x, anchor.z, rotation)) {
                candidates.push(HomesteadCandidate {
                    pass,
                    offset_x,
                    offset_z,
                    anchor_x: anchor.x,
                    anchor_z: anchor.z,
                    rotation,
                });
            }
        }
    }
}

pub fn evaluate_homestead_candidates(
    source: &mut impl HomesteadSurveySource,
    request: HomesteadScoutRequest,
    candidates: impl IntoIterator<Item = HomesteadCandidate>,
) -> Result<Vec<HomesteadCandidateEvaluation>, HomesteadScoutError> {
    let candidates = candidates.into_iter().collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<(HomesteadScoutPass, i32, i32, i32, i32), Vec<_>>::new();
    for candidate in candidates {
        grouped
            .entry((
                candidate.pass,
                candidate.offset_x,
                candidate.offset_z,
                candidate.anchor_x,
                candidate.anchor_z,
            ))
            .or_default()
            .push(candidate);
    }
    let mut evaluations = Vec::new();
    for (_, mut anchor_candidates) in grouped {
        anchor_candidates.sort();
        let anchor = anchor_candidates[0];
        let samples = sample_anchor(source, request.topology, anchor)?;
        for candidate in anchor_candidates {
            evaluations.push(evaluate_candidate(request.seed, candidate, &samples));
        }
    }
    evaluations.sort_by_key(|evaluation| evaluation.candidate);
    Ok(evaluations)
}

pub fn scout_homestead_site(
    source: &mut impl HomesteadSurveySource,
    request: HomesteadScoutRequest,
) -> Result<HomesteadScoutReceipt, HomesteadScoutError> {
    let candidates = compile_homestead_candidates(request)?;
    let mut evaluations = evaluate_homestead_candidates(source, request, candidates)?;
    let refinement = refine_homestead_footprints(source, request, &mut evaluations)?;
    let mut receipt = finalize_homestead_evaluations(request, evaluations)?;
    receipt.footprint_refinement_count = refinement.candidate_count;
    receipt.footprint_sample_count = refinement.sample_count;
    Ok(receipt)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct HomesteadFootprintRefinementStats {
    candidate_count: usize,
    sample_count: usize,
}

fn refine_homestead_footprints(
    source: &mut impl HomesteadSurveySource,
    request: HomesteadScoutRequest,
    evaluations: &mut [HomesteadCandidateEvaluation],
) -> Result<HomesteadFootprintRefinementStats, HomesteadScoutError> {
    let mut stats = HomesteadFootprintRefinementStats::default();
    for (pass, tier) in [
        (
            HomesteadScoutPass::Primary,
            HomesteadCompositionTier::FullV1,
        ),
        (
            HomesteadScoutPass::Primary,
            HomesteadCompositionTier::CompactV1,
        ),
        (
            HomesteadScoutPass::Fallback,
            HomesteadCompositionTier::FullV1,
        ),
        (
            HomesteadScoutPass::Fallback,
            HomesteadCompositionTier::CompactV1,
        ),
    ] {
        let mut candidates = evaluations
            .iter()
            .enumerate()
            .filter(|(_, evaluation)| evaluation.candidate.pass == pass)
            .filter(|(_, evaluation)| score_for_tier(evaluation, tier).is_some())
            .map(|(index, evaluation)| {
                (
                    index,
                    *score_for_tier(evaluation, tier).unwrap(),
                    evaluation.candidate,
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, score, candidate)| (*score, *candidate));

        for (index, _, candidate) in candidates {
            stats.candidate_count += 1;
            let (rejection, sample_count) =
                exact_footprint_rejection(source, request.topology, candidate, tier)?;
            stats.sample_count = stats.sample_count.saturating_add(sample_count);
            let Some(rejection) = rejection else {
                return Ok(stats);
            };
            let evaluation = &mut evaluations[index];
            match tier {
                HomesteadCompositionTier::FullV1 => {
                    evaluation.full_rejection = Some(rejection);
                    evaluation.full_score = None;
                }
                HomesteadCompositionTier::CompactV1 => {
                    evaluation.compact_rejection = Some(rejection);
                    evaluation.compact_score = None;
                }
            }
        }
    }
    Ok(stats)
}

fn exact_footprint_rejection(
    source: &mut impl HomesteadSurveySource,
    topology: HorizontalTopology,
    candidate: HomesteadCandidate,
    tier: HomesteadCompositionTier,
) -> Result<(Option<HomesteadSiteRejection>, usize), HomesteadScoutError> {
    let mut local_columns = BTreeSet::new();
    for grade in homestead_foundation_grade_specs(tier) {
        let feather = i32::from(grade.feather_blocks);
        append_local_bounds(
            &mut local_columns,
            grade.forward_min - feather,
            grade.forward_max + feather,
            grade.right_min - feather,
            grade.right_max + feather,
        );
    }
    let (path_right_min, path_right_max) = homestead_path_right_bounds(tier);
    append_local_bounds(
        &mut local_columns,
        homestead_path_min_forward(tier),
        2,
        path_right_min,
        path_right_max,
    );
    let (pond_right_min, pond_right_max) = homestead_pond_right_bounds(tier);
    append_local_bounds(
        &mut local_columns,
        -13,
        4,
        pond_right_min - 1,
        pond_right_max + 1,
    );

    let mut sampled = 0;
    for (forward, right) in local_columns {
        let (raw_x, raw_z) = local_to_world(candidate, forward, right);
        let Some(pos) = topology.canonicalize_block(BlockPos::new(raw_x, 0, raw_z)) else {
            return Ok((Some(HomesteadSiteRejection::OutsideTopology), sampled));
        };
        let sample = source
            .sample_column(pos.x, pos.z)
            .map_err(HomesteadScoutError::Survey)?;
        sampled += 1;
        if sample.protected_content {
            return Ok((Some(HomesteadSiteRejection::ProtectedContent), sampled));
        }
        if sample.fluid {
            return Ok((Some(HomesteadSiteRejection::BuildingCoreWater), sampled));
        }
    }
    Ok((None, sampled))
}

fn append_local_bounds(
    columns: &mut BTreeSet<(i32, i32)>,
    forward_min: i32,
    forward_max: i32,
    right_min: i32,
    right_max: i32,
) {
    for forward in forward_min..=forward_max {
        for right in right_min..=right_max {
            columns.insert((forward, right));
        }
    }
}

const fn local_to_world(candidate: HomesteadCandidate, forward: i32, right: i32) -> (i32, i32) {
    match candidate.rotation {
        HomesteadRotation::East => (candidate.anchor_x + forward, candidate.anchor_z + right),
        HomesteadRotation::South => (candidate.anchor_x - right, candidate.anchor_z + forward),
        HomesteadRotation::West => (candidate.anchor_x - forward, candidate.anchor_z - right),
        HomesteadRotation::North => (candidate.anchor_x + right, candidate.anchor_z - forward),
    }
}

pub fn finalize_homestead_evaluations(
    request: HomesteadScoutRequest,
    evaluations: impl IntoIterator<Item = HomesteadCandidateEvaluation>,
) -> Result<HomesteadScoutReceipt, HomesteadScoutError> {
    let mut evaluations = evaluations.into_iter().collect::<Vec<_>>();
    evaluations.sort_by_key(|evaluation| evaluation.candidate);
    evaluations.dedup_by_key(|evaluation| evaluation.candidate);
    let selected_pair = [
        (
            HomesteadScoutPass::Primary,
            HomesteadCompositionTier::FullV1,
        ),
        (
            HomesteadScoutPass::Primary,
            HomesteadCompositionTier::CompactV1,
        ),
        (
            HomesteadScoutPass::Fallback,
            HomesteadCompositionTier::FullV1,
        ),
        (
            HomesteadScoutPass::Fallback,
            HomesteadCompositionTier::CompactV1,
        ),
    ]
    .into_iter()
    .find_map(|(pass, tier)| {
        evaluations
            .iter()
            .filter(|evaluation| evaluation.candidate.pass == pass)
            .filter_map(|evaluation| {
                score_for_tier(evaluation, tier).map(|score| (evaluation, score))
            })
            .min_by_key(|(evaluation, score)| (**score, evaluation.candidate))
            .map(|(evaluation, _)| (evaluation.clone(), tier))
    });

    let mut ranked = evaluations.clone();
    ranked.sort_by_key(retained_rank_key);
    ranked.truncate(RETAINED_CANDIDATES);
    let anchors = evaluations
        .iter()
        .map(|evaluation| (evaluation.candidate.anchor_x, evaluation.candidate.anchor_z))
        .collect::<BTreeSet<_>>();
    let full_fit_count = evaluations
        .iter()
        .filter(|value| value.full_score.is_some())
        .count();
    let compact_fit_count = evaluations
        .iter()
        .filter(|value| value.compact_score.is_some())
        .count();
    let mut receipt = HomesteadScoutReceipt {
        revision: HOMESTEAD_SCOUT_REVISION.to_owned(),
        seed: request.seed,
        provisional_spawn: [
            request.provisional_spawn.x,
            request.provisional_spawn.y,
            request.provisional_spawn.z,
        ],
        evaluation_count: evaluations.len(),
        unique_anchor_count: anchors.len(),
        survey_sample_count: anchors.len() * 57,
        footprint_refinement_count: 0,
        footprint_sample_count: 0,
        primary_evaluations: evaluations
            .iter()
            .filter(|value| value.candidate.pass == HomesteadScoutPass::Primary)
            .count(),
        fallback_evaluations: evaluations
            .iter()
            .filter(|value| value.candidate.pass == HomesteadScoutPass::Fallback)
            .count(),
        full_fit_count,
        compact_fit_count,
        rejected_count: evaluations
            .iter()
            .filter(|value| value.compact_score.is_none())
            .count(),
        selected: selected_pair.map(|(evaluation, tier)| selected_site(&evaluation, tier)),
        retained_candidates: ranked,
    };
    if receipt.selected.is_none() {
        return Err(HomesteadScoutError::NoSafeSite(Box::new(receipt)));
    }
    receipt.selected.as_mut().unwrap().checksum_sha256 = selected_checksum(&receipt);
    Ok(receipt)
}

#[derive(Clone, Debug)]
struct AnchorSamples {
    core: BTreeMap<(i32, i32), HomesteadSurveyColumn>,
    scenic: Vec<HomesteadSurveyColumn>,
    outside_topology: bool,
}

fn sample_anchor(
    source: &mut impl HomesteadSurveySource,
    topology: HorizontalTopology,
    candidate: HomesteadCandidate,
) -> Result<AnchorSamples, HomesteadScoutError> {
    let mut core = BTreeMap::new();
    let mut outside_topology = false;
    for dz in (-FULL_CORE_HALF_EXTENT..=FULL_CORE_HALF_EXTENT).step_by(CORE_SAMPLE_STEP as usize) {
        for dx in
            (-FULL_CORE_HALF_EXTENT..=FULL_CORE_HALF_EXTENT).step_by(CORE_SAMPLE_STEP as usize)
        {
            match canonical_sample(topology, candidate.anchor_x, candidate.anchor_z, dx, dz) {
                Some((x, z)) => {
                    core.insert(
                        (dx, dz),
                        source
                            .sample_column(x, z)
                            .map_err(HomesteadScoutError::Survey)?,
                    );
                }
                None => outside_topology = true,
            }
        }
    }
    let mut scenic = Vec::new();
    for (dx, dz) in [
        (-SCENIC_SURVEY_RADIUS, 0),
        (SCENIC_SURVEY_RADIUS, 0),
        (0, -SCENIC_SURVEY_RADIUS),
        (0, SCENIC_SURVEY_RADIUS),
        (-181, -181),
        (181, -181),
        (-181, 181),
        (181, 181),
    ] {
        if let Some((x, z)) =
            canonical_sample(topology, candidate.anchor_x, candidate.anchor_z, dx, dz)
        {
            scenic.push(
                source
                    .sample_column(x, z)
                    .map_err(HomesteadScoutError::Survey)?,
            );
        } else {
            outside_topology = true;
        }
    }
    Ok(AnchorSamples {
        core,
        scenic,
        outside_topology,
    })
}

fn canonical_sample(
    topology: HorizontalTopology,
    anchor_x: i32,
    anchor_z: i32,
    dx: i32,
    dz: i32,
) -> Option<(i32, i32)> {
    let raw = BlockPos::new(anchor_x.checked_add(dx)?, 0, anchor_z.checked_add(dz)?);
    topology.canonicalize_block(raw).map(|pos| (pos.x, pos.z))
}

fn evaluate_candidate(
    seed: i64,
    candidate: HomesteadCandidate,
    samples: &AnchorSamples,
) -> HomesteadCandidateEvaluation {
    let full_metrics = measure_site(candidate, samples, FULL_CORE_HALF_EXTENT);
    let compact_metrics = measure_site(candidate, samples, COMPACT_CORE_HALF_EXTENT);
    let full_rejection = reject_site(candidate, samples, full_metrics, true);
    let compact_rejection = reject_site(candidate, samples, compact_metrics, false);
    HomesteadCandidateEvaluation {
        candidate,
        full_metrics,
        compact_metrics,
        full_rejection,
        compact_rejection,
        full_score: full_rejection
            .is_none()
            .then(|| score_site(seed, candidate, full_metrics)),
        compact_score: compact_rejection
            .is_none()
            .then(|| score_site(seed, candidate, compact_metrics)),
    }
}

fn measure_site(
    candidate: HomesteadCandidate,
    samples: &AnchorSamples,
    half_extent: i32,
) -> HomesteadSiteMetrics {
    let columns = samples
        .core
        .iter()
        .filter(|((dx, dz), _)| dx.abs() <= half_extent && dz.abs() <= half_extent)
        .map(|(_, column)| *column)
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return HomesteadSiteMetrics::default();
    }
    let mut heights = columns
        .iter()
        .map(|sample| sample.surface_y)
        .collect::<Vec<_>>();
    heights.sort_unstable();
    let target = heights[heights.len() / 2];
    let mut slopes = columns
        .iter()
        .map(|sample| sample.slope_milli)
        .collect::<Vec<_>>();
    slopes.sort_unstable();
    let p90 = slopes[(slopes.len() - 1) * 9 / 10];
    let scenic_len = samples.scenic.len().max(1) as u32;
    HomesteadSiteMetrics {
        sample_count: columns.len() as u16,
        dry_support_samples: columns.iter().filter(|sample| !sample.fluid).count() as u16,
        fluid_samples: columns.iter().filter(|sample| sample.fluid).count() as u16,
        protected_samples: columns
            .iter()
            .filter(|sample| sample.protected_content)
            .count() as u16,
        surface_min_y: *heights.first().unwrap(),
        surface_max_y: *heights.last().unwrap(),
        surface_span: (heights.last().unwrap() - heights.first().unwrap()) as u16,
        target_surface_y: target,
        arrival_surface_y: samples
            .core
            .get(&arrival_offset(candidate.rotation, half_extent))
            .map_or(target, |sample| sample.surface_y),
        estimated_cut_blocks: columns
            .iter()
            .map(|sample| sample.surface_y.saturating_sub(target).max(0) as u32 * 256)
            .sum(),
        estimated_fill_blocks: columns
            .iter()
            .map(|sample| target.saturating_sub(sample.surface_y).max(0) as u32 * 256)
            .sum(),
        max_cut_or_fill_depth: columns
            .iter()
            .map(|sample| sample.surface_y.abs_diff(target) as u16)
            .max()
            .unwrap_or(0),
        slope_p90_milli: p90,
        maximum_slope_milli: *slopes.last().unwrap(),
        tree_clearing_milli: columns
            .iter()
            .map(|sample| u32::from(sample.woodland_milli))
            .sum(),
        meadow_milli: average_u16(columns.iter().map(|sample| sample.meadow_milli)),
        woodland_milli: average_u16(columns.iter().map(|sample| sample.woodland_milli)),
        nearby_water_milli: (samples
            .scenic
            .iter()
            .map(|sample| u32::from(sample.water_milli))
            .sum::<u32>()
            / scenic_len) as u16,
        coast_milli: (samples
            .scenic
            .iter()
            .map(|sample| u32::from(sample.coast_milli))
            .sum::<u32>()
            / scenic_len) as u16,
        scenic_relief_milli: (samples
            .scenic
            .iter()
            .map(|sample| u32::from(sample.relief_milli))
            .sum::<u32>()
            / scenic_len) as u16,
    }
}

fn reject_site(
    candidate: HomesteadCandidate,
    samples: &AnchorSamples,
    metrics: HomesteadSiteMetrics,
    full: bool,
) -> Option<HomesteadSiteRejection> {
    if samples.outside_topology {
        return Some(HomesteadSiteRejection::OutsideTopology);
    }
    if metrics.protected_samples > 0 {
        return Some(HomesteadSiteRejection::ProtectedContent);
    }
    if metrics.fluid_samples > 0 {
        return Some(HomesteadSiteRejection::BuildingCoreWater);
    }
    let half_extent = if full {
        FULL_CORE_HALF_EXTENT
    } else {
        COMPACT_CORE_HALF_EXTENT
    };
    let arrival_offset = arrival_offset(candidate.rotation, half_extent);
    let Some(arrival) = samples.core.get(&arrival_offset) else {
        return Some(HomesteadSiteRejection::UnsafeArrival);
    };
    if arrival.fluid || arrival.protected_content {
        return Some(HomesteadSiteRejection::UnsafeArrival);
    }
    let (max_span, max_grade, max_slope) = if full { (12, 6, 500) } else { (16, 8, 700) };
    if metrics.surface_span > max_span || metrics.max_cut_or_fill_depth > max_grade {
        return Some(HomesteadSiteRejection::ExcessiveEarthwork);
    }
    if metrics.maximum_slope_milli > max_slope
        || metrics.dry_support_samples != metrics.sample_count
    {
        return Some(HomesteadSiteRejection::InsufficientSupport);
    }
    None
}

fn score_site(
    seed: i64,
    candidate: HomesteadCandidate,
    metrics: HomesteadSiteMetrics,
) -> HomesteadScore {
    let mixed_context_penalty = u32::from(metrics.meadow_milli.abs_diff(650))
        + u32::from(metrics.woodland_milli.abs_diff(300));
    let water_penalty = if metrics.nearby_water_milli == 0 {
        250
    } else {
        u32::from(metrics.nearby_water_milli.abs_diff(180))
    };
    let relief_penalty = u32::from(metrics.scenic_relief_milli.abs_diff(300));
    HomesteadScore {
        earthwork_blocks: metrics
            .estimated_cut_blocks
            .saturating_add(metrics.estimated_fill_blocks),
        max_grade_depth: metrics.max_cut_or_fill_depth,
        surface_span: metrics.surface_span,
        slope_p90_milli: metrics.slope_p90_milli,
        tree_clearing_milli: metrics.tree_clearing_milli,
        scenic_penalty: mixed_context_penalty + water_penalty + relief_penalty,
        distance_blocks: candidate
            .offset_x
            .unsigned_abs()
            .max(candidate.offset_z.unsigned_abs())
            .min(u32::from(u16::MAX)) as u16,
        tie_key: tie_key(seed, candidate),
    }
}

fn score_for_tier(
    evaluation: &HomesteadCandidateEvaluation,
    tier: HomesteadCompositionTier,
) -> Option<&HomesteadScore> {
    match tier {
        HomesteadCompositionTier::FullV1 => evaluation.full_score.as_ref(),
        HomesteadCompositionTier::CompactV1 => evaluation.compact_score.as_ref(),
    }
}

fn retained_rank_key(
    evaluation: &HomesteadCandidateEvaluation,
) -> (u8, HomesteadScore, HomesteadCandidate) {
    if let Some(score) = evaluation.full_score {
        (0, score, evaluation.candidate)
    } else if let Some(score) = evaluation.compact_score {
        (1, score, evaluation.candidate)
    } else {
        (
            2,
            HomesteadScore {
                earthwork_blocks: u32::MAX,
                max_grade_depth: u16::MAX,
                surface_span: u16::MAX,
                slope_p90_milli: u16::MAX,
                tree_clearing_milli: u32::MAX,
                scenic_penalty: u32::MAX,
                distance_blocks: u16::MAX,
                tie_key: tie_key(0, evaluation.candidate),
            },
            evaluation.candidate,
        )
    }
}

fn selected_site(
    evaluation: &HomesteadCandidateEvaluation,
    tier: HomesteadCompositionTier,
) -> SelectedHomesteadSite {
    let metrics = match tier {
        HomesteadCompositionTier::FullV1 => evaluation.full_metrics,
        HomesteadCompositionTier::CompactV1 => evaluation.compact_metrics,
    };
    let score = *score_for_tier(evaluation, tier).unwrap();
    let half_extent = match tier {
        HomesteadCompositionTier::FullV1 => FULL_CORE_HALF_EXTENT,
        HomesteadCompositionTier::CompactV1 => COMPACT_CORE_HALF_EXTENT,
    };
    let (arrival_dx, arrival_dz) = arrival_offset(evaluation.candidate.rotation, half_extent);
    SelectedHomesteadSite {
        candidate: evaluation.candidate,
        tier,
        metrics,
        score,
        arrival: [
            evaluation.candidate.anchor_x + arrival_dx,
            metrics.arrival_surface_y + 1,
            evaluation.candidate.anchor_z + arrival_dz,
        ],
        facing: evaluation.candidate.rotation,
        core_bounds: square_bounds(evaluation.candidate, half_extent),
        reservation_bounds: square_bounds(evaluation.candidate, RESERVATION_HALF_EXTENT),
        scenic_bounds: square_bounds(evaluation.candidate, SCENIC_SURVEY_RADIUS),
        checksum_sha256: String::new(),
    }
}

const fn arrival_offset(rotation: HomesteadRotation, half_extent: i32) -> (i32, i32) {
    match rotation {
        HomesteadRotation::North => (0, half_extent),
        HomesteadRotation::East => (-half_extent, 0),
        HomesteadRotation::South => (0, -half_extent),
        HomesteadRotation::West => (half_extent, 0),
    }
}

const fn square_bounds(candidate: HomesteadCandidate, half: i32) -> HomesteadBounds2d {
    HomesteadBounds2d {
        min_x: candidate.anchor_x - half,
        min_z: candidate.anchor_z - half,
        max_x: candidate.anchor_x + half,
        max_z: candidate.anchor_z + half,
    }
}

fn selected_checksum(receipt: &HomesteadScoutReceipt) -> String {
    let selected = receipt.selected.as_ref().unwrap();
    let bytes = serde_json::to_vec(&(
        HOMESTEAD_SCOUT_REVISION,
        receipt.seed,
        receipt.provisional_spawn,
        selected.candidate,
        selected.tier,
        selected.metrics,
        selected.score,
        selected.arrival,
        selected.core_bounds,
        selected.reservation_bounds,
    ))
    .expect("homestead selection facts must serialize");
    format!("{:x}", Sha256::digest(bytes))
}

fn tie_key(seed: i64, candidate: HomesteadCandidate) -> u64 {
    let mut value = seed as u64 ^ TIE_DOMAIN;
    value = mix64(value ^ candidate.anchor_x as u32 as u64);
    value = mix64(value ^ (candidate.anchor_z as u32 as u64).rotate_left(17));
    mix64(value ^ candidate.rotation.code())
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn quantize_unit(value: f64) -> u16 {
    (value.clamp(0.0, 1.0) * 1_000.0).round() as u16
}

fn average_u16(values: impl IntoIterator<Item = u16>) -> u16 {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    (values.iter().map(|value| u32::from(*value)).sum::<u32>() / values.len() as u32) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(seed: i64) -> HomesteadScoutRequest {
        HomesteadScoutRequest {
            seed,
            provisional_spawn: BlockPos::new(8, 4, 8),
            topology: HorizontalTopology::UNBOUNDED,
        }
    }

    #[test]
    fn candidate_budget_and_passes_are_exact() {
        let candidates = compile_homestead_candidates(request(7)).unwrap();
        assert_eq!(candidates.len(), MAX_CANDIDATE_ROTATION_EVALUATIONS);
        assert_eq!(
            candidates
                .iter()
                .filter(|value| value.pass == HomesteadScoutPass::Primary)
                .count(),
            2_500
        );
        assert_eq!(
            candidates
                .iter()
                .take(4)
                .map(|value| value.rotation)
                .collect::<Vec<_>>(),
            HomesteadRotation::ALL
        );
    }

    #[test]
    fn compact_arrival_starts_at_the_compact_path_edge() {
        for rotation in HomesteadRotation::ALL {
            let candidate = HomesteadCandidate {
                anchor_x: 123,
                anchor_z: -456,
                offset_x: 0,
                offset_z: 0,
                rotation,
                pass: HomesteadScoutPass::Primary,
            };
            let local_path_start = local_to_world(
                candidate,
                homestead_path_min_forward(HomesteadCompositionTier::CompactV1),
                0,
            );
            let (dx, dz) = arrival_offset(rotation, COMPACT_CORE_HALF_EXTENT);

            assert_eq!(
                local_path_start,
                (candidate.anchor_x + dx, candidate.anchor_z + dz)
            );
        }
    }

    #[test]
    fn flat_canary_selects_identically_across_orders_and_partitions() {
        let request = request(8_675_309);
        let candidates = compile_homestead_candidates(request).unwrap();
        let mut raster_source = FlatGrassHomesteadSurveySource;
        let raster =
            evaluate_homestead_candidates(&mut raster_source, request, candidates.iter().copied())
                .unwrap();
        let mut reverse_source = FlatGrassHomesteadSurveySource;
        let reverse = evaluate_homestead_candidates(
            &mut reverse_source,
            request,
            candidates.iter().rev().copied(),
        )
        .unwrap();
        let mut shuffled_candidates = candidates.clone();
        shuffled_candidates.sort_by_key(|candidate| tie_key(123_456, *candidate));
        let mut shuffled_source = FlatGrassHomesteadSurveySource;
        let shuffled =
            evaluate_homestead_candidates(&mut shuffled_source, request, shuffled_candidates)
                .unwrap();

        let raster_receipt = finalize_homestead_evaluations(request, raster.clone()).unwrap();
        assert_eq!(
            raster_receipt.selected.as_ref().unwrap().checksum_sha256,
            HOMESTEAD_FLAT_WASM_WITNESS_SHA256
        );
        assert_eq!(raster, reverse);
        assert_eq!(raster, shuffled);
        assert_eq!(
            raster_receipt,
            finalize_homestead_evaluations(request, reverse).unwrap()
        );

        let mut partitions = Vec::new();
        for partition in candidates.chunks(256) {
            let mut source = FlatGrassHomesteadSurveySource;
            partitions.extend(
                evaluate_homestead_candidates(&mut source, request, partition.iter().copied())
                    .unwrap(),
            );
        }
        partitions.reverse();
        assert_eq!(
            raster_receipt,
            finalize_homestead_evaluations(request, partitions).unwrap()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn flat_canary_selects_identically_on_native_threads() {
        let request = request(-77);
        let candidates = compile_homestead_candidates(request).unwrap();
        let handles = candidates
            .chunks(512)
            .map(|partition| {
                let partition = partition.to_vec();
                std::thread::spawn(move || {
                    evaluate_homestead_candidates(
                        &mut FlatGrassHomesteadSurveySource,
                        request,
                        partition,
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let threaded = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        let mut threaded = threaded;
        let refinement = refine_homestead_footprints(
            &mut FlatGrassHomesteadSurveySource,
            request,
            &mut threaded,
        )
        .unwrap();
        let mut threaded = finalize_homestead_evaluations(request, threaded).unwrap();
        threaded.footprint_refinement_count = refinement.candidate_count;
        threaded.footprint_sample_count = refinement.sample_count;
        let serial = scout_homestead_site(&mut FlatGrassHomesteadSurveySource, request).unwrap();
        assert_eq!(threaded, serial);
    }

    #[test]
    fn exact_footprint_refinement_rejects_water_between_coarse_samples() {
        #[derive(Clone, Copy)]
        struct SingleFluidColumn {
            x: i32,
            z: i32,
        }

        impl HomesteadSurveySource for SingleFluidColumn {
            fn sample_column(
                &mut self,
                world_x: i32,
                world_z: i32,
            ) -> Result<HomesteadSurveyColumn, String> {
                Ok(HomesteadSurveyColumn {
                    surface_y: 3,
                    fluid: world_x == self.x && world_z == self.z,
                    meadow_milli: 1_000,
                    ..HomesteadSurveyColumn::default()
                })
            }
        }

        let request = request(8_675_309);
        let coarse = scout_homestead_site(&mut FlatGrassHomesteadSurveySource, request).unwrap();
        let coarse_candidate = coarse.selected.unwrap().candidate;
        let (x, z) = local_to_world(
            coarse_candidate,
            homestead_path_min_forward(HomesteadCompositionTier::FullV1) + 1,
            0,
        );
        assert_ne!(
            x.rem_euclid(CORE_SAMPLE_STEP),
            request.provisional_spawn.x.rem_euclid(CORE_SAMPLE_STEP)
        );
        let refined = scout_homestead_site(&mut SingleFluidColumn { x, z }, request).unwrap();

        assert_ne!(refined.selected.unwrap().candidate, coarse_candidate);
        assert!(refined.footprint_refinement_count >= 2);
        assert!(refined.footprint_sample_count > 0);
    }

    #[test]
    fn production_survey_marks_sub_sea_foundation_columns_as_fluid() {
        let candidate = HomesteadCandidate {
            pass: HomesteadScoutPass::Primary,
            offset_x: 0,
            offset_z: 0,
            anchor_x: 744,
            anchor_z: -376,
            rotation: HomesteadRotation::East,
        };
        let mut source = McloneOverworldHomesteadSurveySource::new(
            8_675_309,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let mut submerged_columns = 0;
        for grade in homestead_foundation_grade_specs(HomesteadCompositionTier::FullV1) {
            for forward in grade.forward_min..=grade.forward_max {
                for right in grade.right_min..=grade.right_max {
                    let (x, z) = local_to_world(candidate, forward, right);
                    let sample = source.sample_column(x, z).unwrap();
                    if sample.surface_y < MCLONE_OVERWORLD_SEA_LEVEL {
                        submerged_columns += 1;
                        assert!(sample.fluid, "sub-sea column ({x}, {z}) must be wet");
                    }
                }
            }
        }

        assert!(
            submerged_columns > 0,
            "fixture must cover the rejected wet site"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn production_mclone_source_is_reverse_order_equal() {
        let seed = 42;
        let spawn = crate::levelgen::mclone_overworld_homestead_scout_origin_chunk_with_topology(
            seed,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let request = HomesteadScoutRequest {
            seed,
            provisional_spawn: BlockPos::new(spawn.min_block_x() + 8, 0, spawn.min_block_z() + 8),
            topology: HorizontalTopology::UNBOUNDED,
        };
        let candidates = compile_homestead_candidates(request).unwrap();
        let mut raster_source = McloneOverworldHomesteadSurveySource::new(
            seed,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let raster =
            evaluate_homestead_candidates(&mut raster_source, request, candidates.iter().copied())
                .unwrap();
        let mut reverse_source = McloneOverworldHomesteadSurveySource::new(
            seed,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let reverse = evaluate_homestead_candidates(
            &mut reverse_source,
            request,
            candidates.iter().rev().copied(),
        )
        .unwrap();

        assert_eq!(raster, reverse);
        assert_eq!(
            finalize_homestead_evaluations(request, raster).unwrap(),
            finalize_homestead_evaluations(request, reverse).unwrap()
        );
    }

    #[test]
    fn periodic_topology_rejects_self_overlapping_reservation() {
        let request = HomesteadScoutRequest {
            topology: HorizontalTopology::cylinder_x(0, 8),
            ..request(1)
        };
        assert!(matches!(
            compile_homestead_candidates(request),
            Err(HomesteadScoutError::InvalidTopology(_))
        ));
    }
}
