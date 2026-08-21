//! Deterministic review-site selection for the first continental catchment.
//!
//! The sites are observations of one ordinary bounded catchment whose range,
//! reaches, lake, and control ground all survive the continental land mask.
//! They provide cameras and receipts only; generation never branches on them.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    continental_ecoregion::{ContinentalEcoregionDescriptor, ContinentalEcoregionTopology},
    continental_hydrography::{
        CatchmentLocalPoint, ContinentalCatchment, ContinentalHydrographyPlan, HydrographyFeatureId,
    },
    continental_surface::{
        ContinentalSurfacePlan, ContinentalSurfaceSample, ContinentalSurfaceWaterKind,
    },
};

pub const CONTINENTAL_CATCHMENT_REVIEW_SCHEMA_REVISION: &str =
    "mclone-continental-catchment-review-v1";
const REVIEW_OWNER_MIN: i32 = -2;
const REVIEW_OWNER_MAX: i32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalCatchmentReviewSiteKind {
    PassAndHeadwaters,
    TributaryConfluence,
    TrunkAndFloodplain,
    LakeShoreAndOutlet,
    QuietLowlandControl,
}

impl ContinentalCatchmentReviewSiteKind {
    pub const ALL: [Self; 5] = [
        Self::PassAndHeadwaters,
        Self::TributaryConfluence,
        Self::TrunkAndFloodplain,
        Self::LakeShoreAndOutlet,
        Self::QuietLowlandControl,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::PassAndHeadwaters => "pass-and-headwaters",
            Self::TributaryConfluence => "tributary-confluence",
            Self::TrunkAndFloodplain => "trunk-and-floodplain",
            Self::LakeShoreAndOutlet => "lake-shore-and-outlet",
            Self::QuietLowlandControl => "quiet-lowland-control",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::PassAndHeadwaters => "Range pass and branching headwaters",
            Self::TributaryConfluence => "Two tributaries joining below the range",
            Self::TrunkAndFloodplain => "Main river, terrace, and floodplain",
            Self::LakeShoreAndOutlet => "Varied lake shore, spill, and outlet",
            Self::QuietLowlandControl => "Quiet lowland outside strong drainage",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.label() == value)
            .ok_or_else(|| {
                format!(
                    "unsupported continental catchment site {value:?}; expected {}",
                    Self::ALL.map(Self::label).as_slice().join(", ")
                )
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalCatchmentReviewFrames {
    pub overview_blocks: u32,
    pub oblique_blocks: u32,
    pub exact_blocks: u32,
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalCatchmentReviewRelief {
    pub radius_blocks: u32,
    pub step_blocks: u32,
    pub minimum_surface_y: f32,
    pub maximum_surface_y: f32,
    pub relief_blocks: f32,
    pub maximum_x: i32,
    pub maximum_z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalCatchmentReviewSite {
    pub kind: ContinentalCatchmentReviewSiteKind,
    pub title: &'static str,
    pub center_x: i32,
    pub center_z: i32,
    pub surface_y: f32,
    pub water_kind: ContinentalSurfaceWaterKind,
    pub catchment_id: Option<HydrographyFeatureId>,
    pub reach_id: Option<HydrographyFeatureId>,
    pub lake_id: Option<HydrographyFeatureId>,
    pub review_frames: ContinentalCatchmentReviewFrames,
    pub relief: ContinentalCatchmentReviewRelief,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalCatchmentReviewCatalog {
    pub schema_revision: &'static str,
    pub descriptor: ContinentalEcoregionDescriptor,
    pub selected_owner_x: i32,
    pub selected_owner_z: i32,
    pub catchment_id: HydrographyFeatureId,
    pub fitness_score: f32,
    pub evaluated_catchments: u32,
    pub sites: Vec<ContinentalCatchmentReviewSite>,
    pub semantic_sha256: String,
}

impl ContinentalCatchmentReviewCatalog {
    pub fn site(
        &self,
        kind: ContinentalCatchmentReviewSiteKind,
    ) -> Option<&ContinentalCatchmentReviewSite> {
        self.sites.iter().find(|site| site.kind == kind)
    }
}

#[derive(Clone, Debug)]
struct ReviewCandidate {
    score: f32,
    catchment: ContinentalCatchment,
    points: [(ContinentalCatchmentReviewSiteKind, CatchmentLocalPoint); 5],
    samples: [ContinentalSurfaceSample; 5],
}

pub fn compile_continental_catchment_review(
    descriptor: ContinentalEcoregionDescriptor,
) -> Result<ContinentalCatchmentReviewCatalog, String> {
    if descriptor.topology != ContinentalEcoregionTopology::Plane {
        return Err(
            "continental catchment review catalog currently selects on the plane".to_owned(),
        );
    }
    let hydrography = ContinentalHydrographyPlan::new(descriptor);
    let surface = ContinentalSurfacePlan::new(descriptor).map_err(|error| error.to_string())?;
    let mut selected: Option<ReviewCandidate> = None;
    let mut evaluated_catchments = 0_u32;
    for owner_z in REVIEW_OWNER_MIN..=REVIEW_OWNER_MAX {
        for owner_x in REVIEW_OWNER_MIN..=REVIEW_OWNER_MAX {
            evaluated_catchments += 1;
            let catchment = hydrography.catchment_for_owner(owner_x, owner_z);
            let points = review_points(&catchment);
            let samples = points.map(|(_, point)| sample_local(&surface, &catchment, point));
            let score = candidate_score(&catchment, &samples);
            let candidate = ReviewCandidate {
                score,
                catchment,
                points,
                samples,
            };
            if selected.as_ref().is_none_or(|current| {
                candidate.score.total_cmp(&current.score).is_gt()
                    || (candidate.score == current.score
                        && (candidate.catchment.owner_z, candidate.catchment.owner_x)
                            < (current.catchment.owner_z, current.catchment.owner_x))
            }) {
                selected = Some(candidate);
            }
        }
    }
    let mut selected =
        selected.ok_or_else(|| "bounded catchment review scan is empty".to_owned())?;
    selected.points = selected.points.map(|(kind, point)| {
        let refined = match kind {
            ContinentalCatchmentReviewSiteKind::TributaryConfluence => {
                find_nearby_water_point(&surface, &selected.catchment, point, |sample| {
                    sample.water_kind == ContinentalSurfaceWaterKind::River
                })
            }
            ContinentalCatchmentReviewSiteKind::TrunkAndFloodplain => {
                find_nearby_water_point(&surface, &selected.catchment, point, |sample| {
                    sample.water_kind == ContinentalSurfaceWaterKind::River
                        && sample.reach_kind
                            == Some(crate::continental_hydrography::ContinentalReachKind::Trunk)
                })
            }
            ContinentalCatchmentReviewSiteKind::LakeShoreAndOutlet => {
                find_lake_shore_and_outlet_point(&surface, &selected.catchment)
            }
            _ => point,
        };
        (kind, refined)
    });
    selected.samples = selected
        .points
        .map(|(_, point)| sample_local(&surface, &selected.catchment, point));
    selected.score = candidate_score(&selected.catchment, &selected.samples);
    if selected.score < 0.0 {
        return Err(format!(
            "bounded catchment review scan found no coherent land catchment; best score {}",
            selected.score
        ));
    }

    // Orbit yaw maps to an eye vector of (cos(yaw), -sin(yaw)). Put the
    // camera downstream so the ordinary reach views look back toward their
    // sources. The range review turns a quarter orbit to expose the pass and
    // flanking summits in profile.
    let yaw_radians =
        (-selected.catchment.downstream_axis_z).atan2(selected.catchment.downstream_axis_x) as f32;
    let sites = selected
        .points
        .into_iter()
        .zip(selected.samples)
        .map(|((kind, point), sample)| {
            let (world_x, world_z) = selected.catchment.local_to_world(point);
            let center_x = world_x.round() as i32;
            let center_z = world_z.round() as i32;
            let review_frames = review_frames(kind, yaw_radians);
            ContinentalCatchmentReviewSite {
                kind,
                title: kind.title(),
                center_x,
                center_z,
                surface_y: sample.display_surface_y,
                water_kind: sample.water_kind,
                catchment_id: sample.catchment_id,
                reach_id: sample.reach_id,
                lake_id: sample.lake_id,
                review_frames,
                relief: sample_relief(
                    &surface,
                    center_x,
                    center_z,
                    review_frames.oblique_blocks / 2,
                ),
            }
        })
        .collect::<Vec<_>>();
    let semantic_sha256 = semantic_sha256(
        descriptor,
        selected.catchment.id,
        selected.score,
        evaluated_catchments,
        &sites,
    );
    Ok(ContinentalCatchmentReviewCatalog {
        schema_revision: CONTINENTAL_CATCHMENT_REVIEW_SCHEMA_REVISION,
        descriptor,
        selected_owner_x: selected.catchment.owner_x,
        selected_owner_z: selected.catchment.owner_z,
        catchment_id: selected.catchment.id,
        fitness_score: selected.score,
        evaluated_catchments,
        sites,
        semantic_sha256,
    })
}

fn review_points(
    catchment: &ContinentalCatchment,
) -> [(ContinentalCatchmentReviewSiteKind, CatchmentLocalPoint); 5] {
    [
        (
            ContinentalCatchmentReviewSiteKind::PassAndHeadwaters,
            CatchmentLocalPoint {
                across: 0.0,
                downstream: -10_300.0,
                bed_y: 0.0,
            },
        ),
        (
            ContinentalCatchmentReviewSiteKind::TributaryConfluence,
            catchment.nodes[5],
        ),
        (
            ContinentalCatchmentReviewSiteKind::TrunkAndFloodplain,
            reach_midpoint(catchment, 5),
        ),
        (
            ContinentalCatchmentReviewSiteKind::LakeShoreAndOutlet,
            catchment.nodes[usize::from(catchment.spill_node)],
        ),
        (
            ContinentalCatchmentReviewSiteKind::QuietLowlandControl,
            CatchmentLocalPoint {
                across: catchment.half_width_blocks * -0.64,
                downstream: 1_400.0,
                bed_y: 0.0,
            },
        ),
    ]
}

fn reach_midpoint(catchment: &ContinentalCatchment, slot: usize) -> CatchmentLocalPoint {
    let reach = catchment.reaches[slot];
    let start = catchment.nodes[usize::from(reach.start_node)];
    let end = catchment.nodes[usize::from(reach.end_node)];
    CatchmentLocalPoint {
        across: start.across * 0.25 + reach.control.across * 0.5 + end.across * 0.25,
        downstream: start.downstream * 0.25
            + reach.control.downstream * 0.5
            + end.downstream * 0.25,
        bed_y: 0.0,
    }
}

fn sample_local(
    surface: &ContinentalSurfacePlan,
    catchment: &ContinentalCatchment,
    point: CatchmentLocalPoint,
) -> ContinentalSurfaceSample {
    let (world_x, world_z) = catchment.local_to_world(point);
    surface
        .query_point(world_x.round() as i32, world_z.round() as i32)
        .sample
}

fn find_nearby_water_point(
    surface: &ContinentalSurfacePlan,
    catchment: &ContinentalCatchment,
    point: CatchmentLocalPoint,
    predicate: impl Fn(ContinentalSurfaceSample) -> bool,
) -> CatchmentLocalPoint {
    for offset in 0..=64_i32 {
        for sign in [1.0, -1.0] {
            if offset == 0 && sign < 0.0 {
                continue;
            }
            let candidate = CatchmentLocalPoint {
                across: point.across + f64::from(offset) * 4.0 * sign,
                ..point
            };
            if predicate(sample_local(surface, catchment, candidate)) {
                return candidate;
            }
        }
    }
    point
}

fn find_lake_shore_and_outlet_point(
    surface: &ContinentalSurfacePlan,
    catchment: &ContinentalCatchment,
) -> CatchmentLocalPoint {
    let spill = catchment.nodes[usize::from(catchment.spill_node)];
    let delta_across = spill.across - catchment.lake_center.across;
    let delta_downstream = spill.downstream - catchment.lake_center.downstream;
    let length = delta_across.hypot(delta_downstream).max(f64::EPSILON);
    let axis_across = delta_across / length;
    let axis_downstream = delta_downstream / length;
    let mut last_lake = catchment.lake_center;
    let mut found_lake = false;
    for step in 0..=480_i32 {
        let distance = f64::from(step) * 16.0;
        let candidate = CatchmentLocalPoint {
            across: catchment.lake_center.across + axis_across * distance,
            downstream: catchment.lake_center.downstream + axis_downstream * distance,
            bed_y: 0.0,
        };
        let sample = sample_local(surface, catchment, candidate);
        if sample.water_kind == ContinentalSurfaceWaterKind::Lake {
            last_lake = candidate;
            found_lake = true;
        } else if found_lake {
            // Keep the focus on the wet side of the transition. A 256-block
            // exact frame then contains the variable bank, flat lake, spill,
            // and the beginning of the outlet instead of only open water or
            // only downstream forest.
            return last_lake;
        }
    }
    spill
}

fn candidate_score(
    catchment: &ContinentalCatchment,
    samples: &[ContinentalSurfaceSample; 5],
) -> f32 {
    let same_owner = samples
        .iter()
        .filter(|sample| sample.catchment_id == Some(catchment.id))
        .count() as f32;
    let water_sites = samples[1..4]
        .iter()
        .filter(|sample| {
            matches!(
                sample.water_kind,
                ContinentalSurfaceWaterKind::River | ContinentalSurfaceWaterKind::Lake
            )
        })
        .count() as f32;
    let pass_relief = (samples[0].solid_surface_y - samples[4].solid_surface_y).clamp(-80.0, 120.0);
    let lake_bonus = if samples[3].lake_id == Some(catchment.lake_id) {
        12.0
    } else {
        -80.0
    };
    let quiet_bonus = if samples[4].water_kind == ContinentalSurfaceWaterKind::None
        && samples[4].channel_distance_blocks > 500.0
    {
        8.0
    } else {
        -30.0
    };
    same_owner * 4.0 + water_sites * 7.0 + pass_relief * 0.12 + lake_bonus + quiet_bonus
}

fn sample_relief(
    surface: &ContinentalSurfacePlan,
    center_x: i32,
    center_z: i32,
    radius_blocks: u32,
) -> ContinentalCatchmentReviewRelief {
    const GRID_INTERVALS: i32 = 12;
    let radius_blocks = radius_blocks.max(GRID_INTERVALS as u32);
    let step_blocks = (radius_blocks / GRID_INTERVALS as u32).max(1);
    let mut minimum_surface_y = f32::INFINITY;
    let mut maximum_surface_y = f32::NEG_INFINITY;
    let mut maximum_x = center_x;
    let mut maximum_z = center_z;
    for z_offset in -GRID_INTERVALS..=GRID_INTERVALS {
        for x_offset in -GRID_INTERVALS..=GRID_INTERVALS {
            let x = center_x.saturating_add(x_offset.saturating_mul(step_blocks as i32));
            let z = center_z.saturating_add(z_offset.saturating_mul(step_blocks as i32));
            let sample = surface.query_point(x, z).sample;
            minimum_surface_y = minimum_surface_y.min(sample.display_surface_y);
            if sample.display_surface_y > maximum_surface_y {
                maximum_surface_y = sample.display_surface_y;
                maximum_x = x;
                maximum_z = z;
            }
        }
    }
    ContinentalCatchmentReviewRelief {
        radius_blocks,
        step_blocks,
        minimum_surface_y,
        maximum_surface_y,
        relief_blocks: maximum_surface_y - minimum_surface_y,
        maximum_x,
        maximum_z,
    }
}

const fn review_frames(
    kind: ContinentalCatchmentReviewSiteKind,
    yaw_radians: f32,
) -> ContinentalCatchmentReviewFrames {
    match kind {
        ContinentalCatchmentReviewSiteKind::PassAndHeadwaters => ContinentalCatchmentReviewFrames {
            overview_blocks: 8_192,
            oblique_blocks: 3_072,
            exact_blocks: 160,
            yaw_radians: yaw_radians + std::f32::consts::FRAC_PI_2,
            pitch_radians: 0.18,
        },
        ContinentalCatchmentReviewSiteKind::TributaryConfluence => {
            ContinentalCatchmentReviewFrames {
                overview_blocks: 6_144,
                oblique_blocks: 2_048,
                exact_blocks: 128,
                yaw_radians,
                pitch_radians: 0.40,
            }
        }
        ContinentalCatchmentReviewSiteKind::TrunkAndFloodplain => {
            ContinentalCatchmentReviewFrames {
                overview_blocks: 6_144,
                oblique_blocks: 2_048,
                exact_blocks: 128,
                yaw_radians,
                pitch_radians: 0.42,
            }
        }
        ContinentalCatchmentReviewSiteKind::LakeShoreAndOutlet => {
            ContinentalCatchmentReviewFrames {
                overview_blocks: 8_192,
                oblique_blocks: 3_072,
                exact_blocks: 256,
                yaw_radians,
                pitch_radians: 0.38,
            }
        }
        ContinentalCatchmentReviewSiteKind::QuietLowlandControl => {
            ContinentalCatchmentReviewFrames {
                overview_blocks: 4_096,
                oblique_blocks: 1_536,
                exact_blocks: 128,
                yaw_radians,
                pitch_radians: 0.42,
            }
        }
    }
}

fn semantic_sha256(
    descriptor: ContinentalEcoregionDescriptor,
    catchment_id: HydrographyFeatureId,
    score: f32,
    evaluated_catchments: u32,
    sites: &[ContinentalCatchmentReviewSite],
) -> String {
    let mut digest = Sha256::new();
    digest.update(CONTINENTAL_CATCHMENT_REVIEW_SCHEMA_REVISION.as_bytes());
    digest.update(descriptor.seed.to_le_bytes());
    digest.update(catchment_id.hash.to_le_bytes());
    digest.update(score.to_bits().to_le_bytes());
    digest.update(evaluated_catchments.to_le_bytes());
    for site in sites {
        digest.update([site.kind as u8, site.water_kind as u8]);
        digest.update(site.center_x.to_le_bytes());
        digest.update(site.center_z.to_le_bytes());
        digest.update(site.surface_y.to_bits().to_le_bytes());
        for id in [site.catchment_id, site.reach_id, site.lake_id] {
            digest.update(id.map_or(0, |id| id.hash).to_le_bytes());
        }
        for value in [
            site.review_frames.overview_blocks,
            site.review_frames.oblique_blocks,
            site.review_frames.exact_blocks,
            site.relief.radius_blocks,
            site.relief.step_blocks,
        ] {
            digest.update(value.to_le_bytes());
        }
        digest.update(site.review_frames.yaw_radians.to_bits().to_le_bytes());
        digest.update(site.review_frames.pitch_radians.to_bits().to_le_bytes());
        digest.update(site.relief.minimum_surface_y.to_bits().to_le_bytes());
        digest.update(site.relief.maximum_surface_y.to_bits().to_le_bytes());
        digest.update(site.relief.relief_blocks.to_bits().to_le_bytes());
        digest.update(site.relief.maximum_x.to_le_bytes());
        digest.update(site.relief.maximum_z.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_review_catchment_exercises_all_five_realized_stories() {
        let catalog =
            compile_continental_catchment_review(ContinentalEcoregionDescriptor::plane(12_345))
                .unwrap();
        assert_eq!(catalog.evaluated_catchments, 16);
        assert_eq!(catalog.sites.len(), 5);
        assert!(catalog.fitness_score > 0.0);
        assert!(catalog.sites.iter().all(|site| {
            site.catchment_id == Some(catalog.catchment_id)
                && site.surface_y.is_finite()
                && site.review_frames.overview_blocks >= site.review_frames.oblique_blocks
                && site.review_frames.oblique_blocks >= site.review_frames.exact_blocks
        }));
        assert_eq!(
            catalog
                .site(ContinentalCatchmentReviewSiteKind::QuietLowlandControl)
                .unwrap()
                .water_kind,
            ContinentalSurfaceWaterKind::None
        );
    }

    #[test]
    fn review_catalog_is_reconstructible_and_label_complete() {
        let descriptor = ContinentalEcoregionDescriptor::plane(12_345);
        let first = compile_continental_catchment_review(descriptor).unwrap();
        let second = compile_continental_catchment_review(descriptor).unwrap();
        assert_eq!(first, second);
        for kind in ContinentalCatchmentReviewSiteKind::ALL {
            assert_eq!(
                ContinentalCatchmentReviewSiteKind::parse_label(kind.label()),
                Ok(kind)
            );
            assert!(first.site(kind).is_some());
        }
    }
}
