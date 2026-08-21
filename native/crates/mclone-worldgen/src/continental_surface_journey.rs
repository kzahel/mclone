//! Deterministic broad-surface review journeys selected from one bounded scan.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    continental_ecoregion::{
        ContinentalEcoregionDescriptor, ContinentalEcoregionError, LandscapeFeatureId,
    },
    continental_surface::{
        CONTINENTAL_SURFACE_FAMILY_COUNT, ContinentalRegionalArchetype,
        ContinentalSurfaceConstructionCounts, ContinentalSurfacePlan, ContinentalSurfaceSample,
        ContinentalSurfaceSubstrate, ContinentalSurfaceWaterKind, ContinentalSurfaceWindowRequest,
        MesaLandformKind, TerrainCharacterFamily,
    },
};

pub const CONTINENTAL_SURFACE_JOURNEY_SCHEMA_REVISION: &str =
    "mclone-continental-surface-journeys-v2";
pub const CONTINENTAL_SURFACE_JOURNEY_SCAN_BLOCKS: u32 = 131_072;
pub const CONTINENTAL_SURFACE_JOURNEY_SCAN_STEP_BLOCKS: u32 = 512;
pub const CONTINENTAL_SURFACE_JOURNEY_SCAN_SAMPLES_PER_AXIS: u32 = 257;
const JOURNEY_HALF_STEPS: i32 = 16;
const JOURNEY_MIN_SEPARATION_BLOCKS: i64 = 16_384;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalSurfaceJourneyKind {
    CoastToWoodedInterior,
    ClearingBetweenForestCores,
    LongForestEdge,
    ConnectedWaterCountry,
    QuietRollingInterior,
    MesaDesert,
}

impl ContinentalSurfaceJourneyKind {
    pub const ALL: [Self; 6] = [
        Self::CoastToWoodedInterior,
        Self::ClearingBetweenForestCores,
        Self::LongForestEdge,
        Self::ConnectedWaterCountry,
        Self::QuietRollingInterior,
        Self::MesaDesert,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::CoastToWoodedInterior => "coast-to-wooded-interior",
            Self::ClearingBetweenForestCores => "clearing-between-forest-cores",
            Self::LongForestEdge => "long-forest-edge",
            Self::ConnectedWaterCountry => "connected-water-country",
            Self::QuietRollingInterior => "quiet-rolling-interior",
            Self::MesaDesert => "mesa-desert",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::CoastToWoodedInterior => "Coast into wooded interior",
            Self::ClearingBetweenForestCores => "Clearing between forest cores",
            Self::LongForestEdge => "Long forest-edge traverse",
            Self::ConnectedWaterCountry => "Connected river, wetland, and lake country",
            Self::QuietRollingInterior => "Quiet rolling ordinary interior",
            Self::MesaDesert => "Mesa desert tables, escarpments, washes, and basin",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        let value = value.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|kind| kind.label() == value)
            .ok_or_else(|| {
                format!(
                    "unsupported continental journey {value:?}; expected {}",
                    Self::ALL
                        .iter()
                        .map(|kind| kind.label())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalJourneyPlanIdentity {
    pub continent_id: Option<LandscapeFeatureId>,
    pub province_id: Option<LandscapeFeatureId>,
    pub ecoregion_id: Option<LandscapeFeatureId>,
    pub clearing_id: Option<LandscapeFeatureId>,
    pub route_id: Option<LandscapeFeatureId>,
}

impl From<ContinentalSurfaceSample> for ContinentalJourneyPlanIdentity {
    fn from(sample: ContinentalSurfaceSample) -> Self {
        Self {
            continent_id: sample.continent_id,
            province_id: sample.province_id,
            ecoregion_id: sample.ecoregion_id,
            clearing_id: sample.clearing_id,
            route_id: sample.route_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalJourneyCheckpoint {
    pub distance_blocks: u32,
    pub world_x: i32,
    pub world_z: i32,
    pub dominant_family: TerrainCharacterFamily,
    pub water_kind: ContinentalSurfaceWaterKind,
    pub substrate: ContinentalSurfaceSubstrate,
    pub solid_surface_y: f32,
    pub openness: f32,
    pub forest_core: f32,
    pub wetland: f32,
    pub aridity: f32,
    pub regional_archetype: ContinentalRegionalArchetype,
    pub mesa_landform: MesaLandformKind,
    pub mesa_caprock: f32,
    pub mesa_escarpment: f32,
    pub dry_wash: f32,
    pub alluvial_fan: f32,
    pub plan: ContinentalJourneyPlanIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalJourneyReviewFrames {
    pub map_locator_blocks: u32,
    pub overview_blocks: u32,
    pub oblique_blocks: u32,
    pub horizon_blocks: u32,
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceJourneyReceipt {
    pub kind: ContinentalSurfaceJourneyKind,
    pub title: &'static str,
    pub center_x: i32,
    pub center_z: i32,
    pub heading_x: i8,
    pub heading_z: i8,
    pub span_blocks: u32,
    pub fitness_score: f32,
    pub review_frames: ContinentalJourneyReviewFrames,
    pub start_plan: ContinentalJourneyPlanIdentity,
    pub center_plan: ContinentalJourneyPlanIdentity,
    pub end_plan: ContinentalJourneyPlanIdentity,
    pub family_weight_mean: [f32; CONTINENTAL_SURFACE_FAMILY_COUNT],
    pub family_weight_max: [f32; CONTINENTAL_SURFACE_FAMILY_COUNT],
    pub solid_surface_y_min: f32,
    pub solid_surface_y_max: f32,
    pub water_level_y_min: Option<f32>,
    pub water_level_y_max: Option<f32>,
    pub water_kind_counts: [u32; 5],
    pub sequence_summary: String,
    pub checkpoints: Vec<ContinentalJourneyCheckpoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceJourneyCatalog {
    pub schema_revision: &'static str,
    pub descriptor: ContinentalEcoregionDescriptor,
    pub scan_min_x: i32,
    pub scan_min_z: i32,
    pub scan_blocks: u32,
    pub scan_step_blocks: u32,
    pub scan_sample_count: u32,
    pub semantic_sha256: String,
    pub journeys: Vec<ContinentalSurfaceJourneyReceipt>,
    pub work: ContinentalSurfaceConstructionCounts,
}

impl ContinentalSurfaceJourneyCatalog {
    pub fn journey(
        &self,
        kind: ContinentalSurfaceJourneyKind,
    ) -> Option<&ContinentalSurfaceJourneyReceipt> {
        self.journeys.iter().find(|journey| journey.kind == kind)
    }
}

#[derive(Clone, Copy, Debug)]
struct SelectedCandidate {
    score: f32,
    sample_x: i32,
    sample_z: i32,
    direction_index: usize,
}

const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

pub fn compile_continental_surface_journeys(
    descriptor: ContinentalEcoregionDescriptor,
) -> Result<ContinentalSurfaceJourneyCatalog, ContinentalEcoregionError> {
    let surface = ContinentalSurfacePlan::new(descriptor)?;
    let scan_half = i32::try_from(CONTINENTAL_SURFACE_JOURNEY_SCAN_BLOCKS / 2)
        .expect("journey scan half fits i32");
    let request = ContinentalSurfaceWindowRequest::new(
        -scan_half,
        -scan_half,
        CONTINENTAL_SURFACE_JOURNEY_SCAN_SAMPLES_PER_AXIS,
        CONTINENTAL_SURFACE_JOURNEY_SCAN_SAMPLES_PER_AXIS,
        CONTINENTAL_SURFACE_JOURNEY_SCAN_STEP_BLOCKS,
    );
    let window = surface.query_window(request)?;
    let grid = JourneyGrid {
        samples: &window.samples,
        samples_per_axis: CONTINENTAL_SURFACE_JOURNEY_SCAN_SAMPLES_PER_AXIS as i32,
    };
    let mut journeys = Vec::with_capacity(ContinentalSurfaceJourneyKind::ALL.len());
    let mut occupied_centers = Vec::with_capacity(ContinentalSurfaceJourneyKind::ALL.len());
    for kind in ContinentalSurfaceJourneyKind::ALL {
        let selected = select_journey(kind, grid, &occupied_centers);
        let receipt = build_receipt(kind, selected, grid);
        occupied_centers.push((receipt.center_x, receipt.center_z));
        journeys.push(receipt);
    }
    let semantic_sha256 = journey_sha256(descriptor, &journeys);
    Ok(ContinentalSurfaceJourneyCatalog {
        schema_revision: CONTINENTAL_SURFACE_JOURNEY_SCHEMA_REVISION,
        descriptor,
        scan_min_x: request.min_x,
        scan_min_z: request.min_z,
        scan_blocks: CONTINENTAL_SURFACE_JOURNEY_SCAN_BLOCKS,
        scan_step_blocks: request.step_blocks,
        scan_sample_count: window.samples.len() as u32,
        semantic_sha256,
        journeys,
        work: window.work,
    })
}

#[derive(Clone, Copy)]
struct JourneyGrid<'a> {
    samples: &'a [ContinentalSurfaceSample],
    samples_per_axis: i32,
}

impl JourneyGrid<'_> {
    fn sample(self, sample_x: i32, sample_z: i32) -> ContinentalSurfaceSample {
        self.samples[(sample_z * self.samples_per_axis + sample_x) as usize]
    }
}

fn select_journey(
    kind: ContinentalSurfaceJourneyKind,
    grid: JourneyGrid<'_>,
    occupied_centers: &[(i32, i32)],
) -> SelectedCandidate {
    let margin = JOURNEY_HALF_STEPS;
    let mut best = None;
    for sample_z in margin..grid.samples_per_axis - margin {
        for sample_x in margin..grid.samples_per_axis - margin {
            let center = grid.sample(sample_x, sample_z);
            if occupied_centers.iter().any(|(occupied_x, occupied_z)| {
                let dx = i64::from(center.world_x) - i64::from(*occupied_x);
                let dz = i64::from(center.world_z) - i64::from(*occupied_z);
                dx * dx + dz * dz < JOURNEY_MIN_SEPARATION_BLOCKS * JOURNEY_MIN_SEPARATION_BLOCKS
            }) {
                continue;
            }
            for (direction_index, (dx, dz)) in DIRECTIONS.into_iter().enumerate() {
                let candidate = SelectedCandidate {
                    score: journey_score(kind, grid, sample_x, sample_z, dx, dz),
                    sample_x,
                    sample_z,
                    direction_index,
                };
                if best.is_none_or(|best| candidate_precedes(candidate, best)) {
                    best = Some(candidate);
                }
            }
        }
    }
    best.expect("the bounded journey scan has an interior candidate")
}

fn candidate_precedes(left: SelectedCandidate, right: SelectedCandidate) -> bool {
    left.score.total_cmp(&right.score).is_gt()
        || (left.score == right.score
            && (left.sample_z, left.sample_x, left.direction_index)
                < (right.sample_z, right.sample_x, right.direction_index))
}

fn journey_score(
    kind: ContinentalSurfaceJourneyKind,
    grid: JourneyGrid<'_>,
    sample_x: i32,
    sample_z: i32,
    dx: i32,
    dz: i32,
) -> f32 {
    let point = |step: i32| grid.sample(sample_x + dx * step, sample_z + dz * step);
    let start = point(-JOURNEY_HALF_STEPS);
    let inner_start = point(-8);
    let center = point(0);
    let inner_end = point(8);
    let late_end = point(12);
    let end = point(JOURNEY_HALF_STEPS);
    let family = |sample: ContinentalSurfaceSample, family: TerrainCharacterFamily| {
        sample.family_weights[family as usize]
    };
    let ocean = |sample: ContinentalSurfaceSample| {
        f32::from(sample.water_kind == ContinentalSurfaceWaterKind::Ocean)
    };
    let dry_land = |sample: ContinentalSurfaceSample| {
        f32::from(sample.water_kind == ContinentalSurfaceWaterKind::None)
    };
    match kind {
        ContinentalSurfaceJourneyKind::CoastToWoodedInterior => {
            ocean(start) * 2.4
                + family(inner_start, TerrainCharacterFamily::CoastAndHeadland) * 1.8
                + dry_land(end) * 0.8
                + end.forest_core * 1.8
                + inner_end.forest_core
                + (end.solid_surface_y - start.solid_surface_y).clamp(0.0, 48.0) / 48.0
        }
        ContinentalSurfaceJourneyKind::ClearingBetweenForestCores => {
            center.clearing * 2.8
                + center.openness * 0.8
                + inner_start.forest_core
                + inner_end.forest_core
                + inner_start.forest_core.min(inner_end.forest_core) * 1.6
                + dry_land(center) * 0.4
        }
        ContinentalSurfaceJourneyKind::LongForestEdge => {
            let edge_mean = [start, inner_start, center, inner_end, end]
                .into_iter()
                .map(|sample| sample.forest_edge)
                .sum::<f32>()
                / 5.0;
            let perpendicular_a = grid.sample(sample_x - dz * 8, sample_z + dx * 8);
            let perpendicular_b = grid.sample(sample_x + dz * 8, sample_z - dx * 8);
            let cross_contrast = (perpendicular_a.forest_core * perpendicular_b.openness)
                .max(perpendicular_b.forest_core * perpendicular_a.openness);
            edge_mean * 3.2 + cross_contrast * 2.4 + center.forest_edge
        }
        ContinentalSurfaceJourneyKind::ConnectedWaterCountry => {
            let points = [start, inner_start, center, inner_end, end];
            let lake = f32::from(
                points
                    .iter()
                    .any(|sample| sample.water_kind == ContinentalSurfaceWaterKind::Lake),
            );
            let river = f32::from(
                points
                    .iter()
                    .any(|sample| sample.water_kind == ContinentalSurfaceWaterKind::River),
            );
            let wetland_pool = f32::from(
                points
                    .iter()
                    .any(|sample| sample.water_kind == ContinentalSurfaceWaterKind::WetlandPool),
            );
            let wetland = points.iter().map(|sample| sample.wetland).sum::<f32>() / 5.0;
            let route = points.iter().map(|sample| sample.route).sum::<f32>() / 5.0;
            lake * 2.2 + river * 1.4 + wetland_pool + wetland * 1.8 + route * 1.2
        }
        ContinentalSurfaceJourneyKind::QuietRollingInterior => {
            let points = [start, inner_start, center, inner_end, end];
            let rolling = points
                .iter()
                .map(|sample| family(*sample, TerrainCharacterFamily::RollingInterior))
                .sum::<f32>()
                / 5.0;
            let height_min = points
                .iter()
                .map(|sample| sample.solid_surface_y)
                .fold(f32::INFINITY, f32::min);
            let height_max = points
                .iter()
                .map(|sample| sample.solid_surface_y)
                .fold(f32::NEG_INFINITY, f32::max);
            let water_penalty = points
                .iter()
                .map(|sample| f32::from(sample.is_water()))
                .sum::<f32>()
                / 5.0;
            rolling * 3.0 + center.openness * 0.8 + dry_land(center)
                - ((height_max - height_min) / 64.0).clamp(0.0, 1.5)
                - water_penalty * 2.0
                - center.aridity * 0.5
        }
        ContinentalSurfaceJourneyKind::MesaDesert => {
            let points = [start, inner_start, center, inner_end, late_end, end];
            let mesa = points
                .iter()
                .map(|sample| {
                    sample.mesa_caprock
                        + sample.mesa_escarpment
                        + sample.mesa_bench * 0.5
                        + sample.mesa_butte * 0.8
                })
                .fold(0.0_f32, f32::max);
            let centered_mesa = center.mesa_caprock * 0.35
                + center.mesa_escarpment * 2.4
                + center.mesa_bench * 0.55
                + center.mesa_butte * 0.8;
            let drainage = points
                .iter()
                .map(|sample| sample.dry_wash.max(sample.alluvial_fan))
                .fold(0.0_f32, f32::max);
            let high = points
                .iter()
                .map(|sample| sample.solid_surface_y)
                .fold(f32::NEG_INFINITY, f32::max);
            let low = points
                .iter()
                .map(|sample| sample.solid_surface_y)
                .fold(f32::INFINITY, f32::min);
            family(center, TerrainCharacterFamily::AridRainShadow) * 1.6
                + points
                    .iter()
                    .map(|sample| {
                        f32::from(
                            sample.regional_archetype == ContinentalRegionalArchetype::MesaDesert,
                        ) * sample.regional_archetype_weight
                    })
                    .sum::<f32>()
                    / points.len() as f32
                    * 2.4
                + mesa * 2.8
                + centered_mesa * 4.2
                + drainage * 1.6
                + ((high - low) / 54.0).clamp(0.0, 1.5)
                + center.aridity * 0.6
        }
    }
}

fn build_receipt(
    kind: ContinentalSurfaceJourneyKind,
    selected: SelectedCandidate,
    grid: JourneyGrid<'_>,
) -> ContinentalSurfaceJourneyReceipt {
    let (dx, dz) = DIRECTIONS[selected.direction_index];
    let (start_step, end_step) = match kind {
        ContinentalSurfaceJourneyKind::ClearingBetweenForestCores => (-8, 16),
        ContinentalSurfaceJourneyKind::MesaDesert => (-16, 16),
        _ => (-JOURNEY_HALF_STEPS, JOURNEY_HALF_STEPS),
    };
    let sequence = (start_step..=end_step)
        .map(|step| grid.sample(selected.sample_x + dx * step, selected.sample_z + dz * step))
        .collect::<Vec<_>>();
    let start = sequence[0];
    let center = sequence[(-start_step) as usize];
    let end = sequence[sequence.len() - 1];
    let step_blocks = if dx != 0 && dz != 0 {
        (f64::from(CONTINENTAL_SURFACE_JOURNEY_SCAN_STEP_BLOCKS) * 2.0_f64.sqrt()).round() as u32
    } else {
        CONTINENTAL_SURFACE_JOURNEY_SCAN_STEP_BLOCKS
    };
    let span_blocks = step_blocks * (sequence.len() as u32 - 1);
    let mut family_weight_mean = [0.0; CONTINENTAL_SURFACE_FAMILY_COUNT];
    let mut family_weight_max = [0.0_f32; CONTINENTAL_SURFACE_FAMILY_COUNT];
    let mut solid_surface_y_min = f32::INFINITY;
    let mut solid_surface_y_max = f32::NEG_INFINITY;
    let mut water_level_y_min = None::<f32>;
    let mut water_level_y_max = None::<f32>;
    let mut water_kind_counts = [0_u32; 5];
    for sample in &sequence {
        for family in 0..CONTINENTAL_SURFACE_FAMILY_COUNT {
            family_weight_mean[family] += sample.family_weights[family];
            family_weight_max[family] =
                family_weight_max[family].max(sample.family_weights[family]);
        }
        solid_surface_y_min = solid_surface_y_min.min(sample.solid_surface_y);
        solid_surface_y_max = solid_surface_y_max.max(sample.solid_surface_y);
        if let Some(water) = sample.water_level_y {
            water_level_y_min = Some(water_level_y_min.map_or(water, |value| value.min(water)));
            water_level_y_max = Some(water_level_y_max.map_or(water, |value| value.max(water)));
        }
        water_kind_counts[sample.water_kind as usize] += 1;
    }
    for weight in &mut family_weight_mean {
        *weight /= sequence.len() as f32;
    }
    let checkpoints = sequence
        .iter()
        .enumerate()
        .step_by(4)
        .map(|(index, sample)| ContinentalJourneyCheckpoint {
            distance_blocks: index as u32 * step_blocks,
            world_x: sample.world_x,
            world_z: sample.world_z,
            dominant_family: sample.dominant_family,
            water_kind: sample.water_kind,
            substrate: sample.substrate,
            solid_surface_y: sample.solid_surface_y,
            openness: sample.openness,
            forest_core: sample.forest_core,
            wetland: sample.wetland,
            aridity: sample.aridity,
            regional_archetype: sample.regional_archetype,
            mesa_landform: sample.mesa_landform,
            mesa_caprock: sample.mesa_caprock,
            mesa_escarpment: sample.mesa_escarpment,
            dry_wash: sample.dry_wash,
            alluvial_fan: sample.alluvial_fan,
            plan: (*sample).into(),
        })
        .collect::<Vec<_>>();
    let sequence_summary = checkpoints
        .iter()
        .map(|checkpoint| {
            let water = if checkpoint.water_kind == ContinentalSurfaceWaterKind::None {
                "land"
            } else {
                checkpoint.water_kind.label()
            };
            format!("{}:{}", checkpoint.dominant_family.label(), water)
        })
        .collect::<Vec<_>>()
        .join(" -> ");
    let yaw_radians = (dz as f32).atan2(dx as f32);
    ContinentalSurfaceJourneyReceipt {
        kind,
        title: kind.title(),
        center_x: center.world_x,
        center_z: center.world_z,
        heading_x: dx as i8,
        heading_z: dz as i8,
        span_blocks,
        fitness_score: selected.score,
        review_frames: ContinentalJourneyReviewFrames {
            map_locator_blocks: 65_536,
            overview_blocks: 16_384,
            oblique_blocks: 8_192,
            horizon_blocks: 512,
            yaw_radians,
            pitch_radians: 0.52,
        },
        start_plan: start.into(),
        center_plan: center.into(),
        end_plan: end.into(),
        family_weight_mean,
        family_weight_max,
        solid_surface_y_min,
        solid_surface_y_max,
        water_level_y_min,
        water_level_y_max,
        water_kind_counts,
        sequence_summary,
        checkpoints,
    }
}

fn journey_sha256(
    descriptor: ContinentalEcoregionDescriptor,
    journeys: &[ContinentalSurfaceJourneyReceipt],
) -> String {
    let mut digest = Sha256::new();
    digest.update(CONTINENTAL_SURFACE_JOURNEY_SCHEMA_REVISION.as_bytes());
    digest.update(descriptor.seed.to_le_bytes());
    digest.update(
        serde_json::to_vec(journeys).expect("continental journey receipts serialize canonically"),
    );
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_trip_and_reject_unknown_journeys() {
        for kind in ContinentalSurfaceJourneyKind::ALL {
            assert_eq!(
                ContinentalSurfaceJourneyKind::parse_label(kind.label()),
                Ok(kind)
            );
        }
        assert!(ContinentalSurfaceJourneyKind::parse_label("pretty-place").is_err());
    }

    #[test]
    fn selector_publishes_six_separated_bounded_journeys() {
        let catalog =
            compile_continental_surface_journeys(ContinentalEcoregionDescriptor::plane(12_345))
                .unwrap();
        assert_eq!(
            catalog.schema_revision,
            CONTINENTAL_SURFACE_JOURNEY_SCHEMA_REVISION
        );
        assert_eq!(
            catalog.journeys.len(),
            ContinentalSurfaceJourneyKind::ALL.len()
        );
        assert_eq!(catalog.scan_sample_count, 257 * 257);
        assert_eq!(catalog.work.exact_chunks, 0);
        assert_eq!(catalog.work.density_volumes, 0);
        assert_eq!(catalog.work.feature_batches, 0);
        for (index, journey) in catalog.journeys.iter().enumerate() {
            assert_eq!(journey.kind, ContinentalSurfaceJourneyKind::ALL[index]);
            assert!((10_000..=24_000).contains(&journey.span_blocks));
            assert!((7..=9).contains(&journey.checkpoints.len()));
            assert!(journey.fitness_score.is_finite());
            for prior in &catalog.journeys[..index] {
                let dx = i64::from(journey.center_x) - i64::from(prior.center_x);
                let dz = i64::from(journey.center_z) - i64::from(prior.center_z);
                assert!(
                    dx * dx + dz * dz
                        >= JOURNEY_MIN_SEPARATION_BLOCKS * JOURNEY_MIN_SEPARATION_BLOCKS
                );
            }
        }
    }

    #[test]
    fn selected_seed_exercises_each_named_story() {
        let catalog =
            compile_continental_surface_journeys(ContinentalEcoregionDescriptor::plane(12_345))
                .unwrap();
        let coast = catalog
            .journey(ContinentalSurfaceJourneyKind::CoastToWoodedInterior)
            .unwrap();
        assert!(coast.water_kind_counts[ContinentalSurfaceWaterKind::Ocean as usize] > 0);
        assert!(coast.family_weight_max[TerrainCharacterFamily::CoastAndHeadland as usize] > 0.5);
        let clearing = catalog
            .journey(ContinentalSurfaceJourneyKind::ClearingBetweenForestCores)
            .unwrap();
        assert!(
            clearing
                .checkpoints
                .iter()
                .any(|point| point.openness > 0.7)
        );
        assert!(
            clearing
                .checkpoints
                .iter()
                .any(|point| point.forest_core > 0.35)
        );
        let edge = catalog
            .journey(ContinentalSurfaceJourneyKind::LongForestEdge)
            .unwrap();
        assert!(
            edge.checkpoints
                .iter()
                .any(|point| { point.forest_core > 0.2 && point.openness > 0.2 })
        );
        let water = catalog
            .journey(ContinentalSurfaceJourneyKind::ConnectedWaterCountry)
            .unwrap();
        assert!(water.water_kind_counts.iter().skip(1).sum::<u32>() > 0);
        let rolling = catalog
            .journey(ContinentalSurfaceJourneyKind::QuietRollingInterior)
            .unwrap();
        assert!(
            rolling.family_weight_mean[TerrainCharacterFamily::RollingInterior as usize] > 0.45
        );
        let arid = catalog
            .journey(ContinentalSurfaceJourneyKind::MesaDesert)
            .unwrap();
        assert!(arid.checkpoints.iter().any(|point| {
            point.regional_archetype == ContinentalRegionalArchetype::MesaDesert
                && point.mesa_landform != MesaLandformKind::None
                && point.aridity > 0.68
        }));
    }

    #[test]
    fn traversal_and_threads_publish_one_catalog() {
        let descriptor = ContinentalEcoregionDescriptor::plane(12_345);
        let expected = compile_continental_surface_journeys(descriptor).unwrap();
        let handles = (0..3)
            .map(|_| std::thread::spawn(move || compile_continental_surface_journeys(descriptor)))
            .collect::<Vec<_>>();
        for handle in handles {
            let actual = handle.join().unwrap().unwrap();
            assert_eq!(actual.semantic_sha256, expected.semantic_sha256);
            assert_eq!(actual.journeys, expected.journeys);
        }
    }
}
