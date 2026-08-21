//! Bounded feature-owned hydrography for the detached continental candidate.
//!
//! The ordinary product plane cannot derive one exact global watershed during
//! an arbitrary point query. This module instead reconstructs a small stable
//! owner neighborhood and exposes one compact directed catchment graph with
//! explicit influence bounds. Terrain realization and ecology consume these
//! facts; neither clipmap scale nor query history participates in them.

use std::collections::{BTreeMap, btree_map::Entry};

use serde::Serialize;

use crate::continental_ecoregion::{ContinentalEcoregionDescriptor, ContinentalEcoregionTopology};

pub const CONTINENTAL_HYDROGRAPHY_SCHEMA_REVISION: &str = "mclone-continental-hydrography-v3";
pub const CONTINENTAL_CATCHMENT_CELL_BLOCKS: i32 = 32_768;
pub const CONTINENTAL_CATCHMENT_MAX_REACHES: usize = 8;
pub const CONTINENTAL_CATCHMENT_MAX_NODES: usize = 11;

const CATCHMENT_HASH_DOMAIN: u64 = 0x6368_7964_726f_3031;
const OWNER_JITTER_BLOCKS: f64 = 2_048.0;
const BASE_HALF_WIDTH_BLOCKS: f64 = 10_800.0;
const BASE_UPSTREAM_BLOCKS: f64 = 15_000.0;
const BASE_DOWNSTREAM_BLOCKS: f64 = 15_000.0;
const NEIGHBORHOOD_RADIUS: i32 = 1;
const SQRT_HALF: f64 = std::f64::consts::FRAC_1_SQRT_2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum HydrographyFeatureFamily {
    Catchment,
    Reach,
    Confluence,
    Lake,
    Spill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HydrographyFeatureId {
    pub family: HydrographyFeatureFamily,
    pub owner_x: i32,
    pub owner_z: i32,
    pub slot: u8,
    pub hash: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalReachKind {
    Headwater,
    Tributary,
    Trunk,
    LakeInlet,
    Outlet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalReachDownstream {
    Reach(u8),
    Lake,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalShoreIntent {
    None,
    Depositional,
    Wetland,
    Ordinary,
    Gravel,
    Rocky,
    Inlet,
    Outlet,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchmentLocalPoint {
    pub across: f64,
    pub downstream: f64,
    pub bed_y: f64,
}

impl CatchmentLocalPoint {
    const fn new(across: f64, downstream: f64, bed_y: f64) -> Self {
        Self {
            across,
            downstream,
            bed_y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalCatchmentReach {
    pub id: HydrographyFeatureId,
    pub slot: u8,
    pub kind: ContinentalReachKind,
    pub start_node: u8,
    pub end_node: u8,
    pub control: CatchmentLocalPoint,
    pub downstream: ContinentalReachDownstream,
    pub order: u8,
    pub discharge: f32,
    pub head_water_y: f32,
    pub tail_water_y: f32,
    pub head_width_blocks: f32,
    pub tail_width_blocks: f32,
    pub influence_radius_blocks: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalCatchment {
    pub id: HydrographyFeatureId,
    pub owner_x: i32,
    pub owner_z: i32,
    pub center_x: i64,
    pub center_z: i64,
    pub downstream_axis_x: f64,
    pub downstream_axis_z: f64,
    pub across_axis_x: f64,
    pub across_axis_z: f64,
    pub half_width_blocks: f64,
    pub upstream_blocks: f64,
    pub downstream_blocks: f64,
    pub nodes: [CatchmentLocalPoint; CONTINENTAL_CATCHMENT_MAX_NODES],
    pub reaches: [ContinentalCatchmentReach; CONTINENTAL_CATCHMENT_MAX_REACHES],
    pub lake_id: HydrographyFeatureId,
    pub lake_center: CatchmentLocalPoint,
    pub lake_radius_across_blocks: f32,
    pub lake_radius_downstream_blocks: f32,
    pub lake_water_y: f32,
    pub spill_id: HydrographyFeatureId,
    pub spill_node: u8,
    pub outlet_reach: u8,
    pub open_basin: bool,
}

impl ContinentalCatchment {
    pub fn node_world_position(&self, node: u8) -> (f64, f64) {
        self.local_to_world(self.nodes[usize::from(node)])
    }

    pub fn local_to_world(&self, point: CatchmentLocalPoint) -> (f64, f64) {
        (
            self.center_x as f64
                + point.across * self.across_axis_x
                + point.downstream * self.downstream_axis_x,
            self.center_z as f64
                + point.across * self.across_axis_z
                + point.downstream * self.downstream_axis_z,
        )
    }

    pub fn reaches_lake(&self, start_slot: u8) -> bool {
        let mut slot = start_slot;
        for _ in 0..self.reaches.len() {
            match self.reaches[usize::from(slot)].downstream {
                ContinentalReachDownstream::Reach(next) => slot = next,
                ContinentalReachDownstream::Lake => return true,
                ContinentalReachDownstream::Terminal => return false,
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalHydrographyWork {
    pub owner_evaluations: u32,
    pub graph_constructions: u32,
    pub reach_evaluations: u32,
    pub exact_chunks: u32,
    pub raster_cells: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalHydrographySample {
    pub catchment_id: HydrographyFeatureId,
    pub local_across: f32,
    pub local_downstream: f32,
    pub catchment_weight: f32,
    pub divide_weight: f32,
    pub range_weight: f32,
    pub saddle_weight: f32,
    pub valley_weight: f32,
    pub floodplain_weight: f32,
    pub confluence_id: Option<HydrographyFeatureId>,
    pub confluence_distance_blocks: f32,
    pub confluence_weight: f32,
    pub confluence_bed_y: Option<f32>,
    pub reach_id: Option<HydrographyFeatureId>,
    pub reach_slot: Option<u8>,
    pub reach_kind: Option<ContinentalReachKind>,
    pub reach_order: u8,
    pub discharge: f32,
    pub channel_signed_distance_blocks: f32,
    pub channel_distance_blocks: f32,
    pub channel_width_blocks: f32,
    pub bankfull_width_blocks: f32,
    pub reach_progress: f32,
    pub bed_y: Option<f32>,
    pub water_level_y: Option<f32>,
    pub downstream_x: f32,
    pub downstream_z: f32,
    pub lake_id: Option<HydrographyFeatureId>,
    pub lake_signed_distance_blocks: f32,
    pub lake_weight: f32,
    pub lake_water_y: Option<f32>,
    pub spill_id: Option<HydrographyFeatureId>,
    pub shore_intent: ContinentalShoreIntent,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalHydrographyPointQuery {
    pub sample: Option<ContinentalHydrographySample>,
    pub work: ContinentalHydrographyWork,
}

#[derive(Clone, Copy, Debug)]
struct OwnerCandidate {
    owner_x: i32,
    owner_z: i32,
    score: f64,
    local_across: f64,
    local_downstream: f64,
}

#[derive(Clone, Copy, Debug)]
struct ReachDistance {
    signed_distance: f64,
    distance: f64,
    progress: f64,
    tangent_across: f64,
    tangent_downstream: f64,
}

#[derive(Clone, Debug)]
pub struct ContinentalHydrographyPlan {
    descriptor: ContinentalEcoregionDescriptor,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ContinentalHydrographyQueryCache {
    catchments: BTreeMap<(i32, i32), ContinentalCatchment>,
}

impl ContinentalHydrographyQueryCache {
    fn catchment<'a>(
        &'a mut self,
        plan: &ContinentalHydrographyPlan,
        owner_x: i32,
        owner_z: i32,
        work: &mut ContinentalHydrographyWork,
    ) -> &'a ContinentalCatchment {
        let owner_x = canonical_owner_x(plan.descriptor, owner_x);
        match self.catchments.entry((owner_x, owner_z)) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                work.graph_constructions += 1;
                entry.insert(catchment_for_owner(plan.descriptor, owner_x, owner_z))
            }
        }
    }

    #[cfg(test)]
    fn retained_catchments(&self) -> usize {
        self.catchments.len()
    }
}

impl ContinentalHydrographyPlan {
    pub const fn new(descriptor: ContinentalEcoregionDescriptor) -> Self {
        Self { descriptor }
    }

    pub const fn descriptor(&self) -> ContinentalEcoregionDescriptor {
        self.descriptor
    }

    pub fn catchment_for_owner(&self, owner_x: i32, owner_z: i32) -> ContinentalCatchment {
        catchment_for_owner(
            self.descriptor,
            canonical_owner_x(self.descriptor, owner_x),
            owner_z,
        )
    }

    pub fn query_point(&self, world_x: i32, world_z: i32) -> ContinentalHydrographyPointQuery {
        let canonical_x = canonical_world_x(self.descriptor, world_x);
        let base_owner_x = canonical_x.div_euclid(CONTINENTAL_CATCHMENT_CELL_BLOCKS);
        let base_owner_z = world_z.div_euclid(CONTINENTAL_CATCHMENT_CELL_BLOCKS);
        let mut selected: Option<OwnerCandidate> = None;
        let mut work = ContinentalHydrographyWork::default();

        for offset_z in -NEIGHBORHOOD_RADIUS..=NEIGHBORHOOD_RADIUS {
            for offset_x in -NEIGHBORHOOD_RADIUS..=NEIGHBORHOOD_RADIUS {
                work.owner_evaluations += 1;
                let owner_x = canonical_owner_x(self.descriptor, base_owner_x + offset_x);
                let owner_z = base_owner_z + offset_z;
                let catchment = catchment_for_owner(self.descriptor, owner_x, owner_z);
                work.graph_constructions += 1;
                let (local_across, local_downstream) =
                    world_to_local(self.descriptor, &catchment, canonical_x, world_z);
                let across_unit = local_across / catchment.half_width_blocks;
                let downstream_unit = if local_downstream < 0.0 {
                    local_downstream / catchment.upstream_blocks
                } else {
                    local_downstream / catchment.downstream_blocks
                };
                let score = across_unit * across_unit + downstream_unit * downstream_unit;
                if score <= 1.16
                    && selected.is_none_or(|current| {
                        score.total_cmp(&current.score).is_lt()
                            || (score == current.score
                                && (owner_z, owner_x) < (current.owner_z, current.owner_x))
                    })
                {
                    selected = Some(OwnerCandidate {
                        owner_x,
                        owner_z,
                        score,
                        local_across,
                        local_downstream,
                    });
                }
            }
        }

        let Some(selected) = selected else {
            return ContinentalHydrographyPointQuery { sample: None, work };
        };
        work.graph_constructions += 1;
        let catchment = catchment_for_owner(self.descriptor, selected.owner_x, selected.owner_z);
        let sample = evaluate_catchment(
            &catchment,
            selected.local_across,
            selected.local_downstream,
            &mut work,
        );
        ContinentalHydrographyPointQuery {
            sample: Some(sample),
            work,
        }
    }

    pub(crate) fn query_point_cached(
        &self,
        world_x: i32,
        world_z: i32,
        cache: &mut ContinentalHydrographyQueryCache,
    ) -> ContinentalHydrographyPointQuery {
        let canonical_x = canonical_world_x(self.descriptor, world_x);
        let base_owner_x = canonical_x.div_euclid(CONTINENTAL_CATCHMENT_CELL_BLOCKS);
        let base_owner_z = world_z.div_euclid(CONTINENTAL_CATCHMENT_CELL_BLOCKS);
        let mut selected: Option<OwnerCandidate> = None;
        let mut work = ContinentalHydrographyWork::default();

        for offset_z in -NEIGHBORHOOD_RADIUS..=NEIGHBORHOOD_RADIUS {
            for offset_x in -NEIGHBORHOOD_RADIUS..=NEIGHBORHOOD_RADIUS {
                work.owner_evaluations += 1;
                let owner_x = canonical_owner_x(self.descriptor, base_owner_x + offset_x);
                let owner_z = base_owner_z + offset_z;
                let catchment = cache.catchment(self, owner_x, owner_z, &mut work);
                let (local_across, local_downstream) =
                    world_to_local(self.descriptor, catchment, canonical_x, world_z);
                let across_unit = local_across / catchment.half_width_blocks;
                let downstream_unit = if local_downstream < 0.0 {
                    local_downstream / catchment.upstream_blocks
                } else {
                    local_downstream / catchment.downstream_blocks
                };
                let score = across_unit * across_unit + downstream_unit * downstream_unit;
                if score <= 1.16
                    && selected.is_none_or(|current| {
                        score.total_cmp(&current.score).is_lt()
                            || (score == current.score
                                && (owner_z, owner_x) < (current.owner_z, current.owner_x))
                    })
                {
                    selected = Some(OwnerCandidate {
                        owner_x,
                        owner_z,
                        score,
                        local_across,
                        local_downstream,
                    });
                }
            }
        }

        let Some(selected) = selected else {
            return ContinentalHydrographyPointQuery { sample: None, work };
        };
        let catchment = cache.catchment(self, selected.owner_x, selected.owner_z, &mut work);
        let sample = evaluate_catchment(
            catchment,
            selected.local_across,
            selected.local_downstream,
            &mut work,
        );
        ContinentalHydrographyPointQuery {
            sample: Some(sample),
            work,
        }
    }
}

fn catchment_for_owner(
    descriptor: ContinentalEcoregionDescriptor,
    owner_x: i32,
    owner_z: i32,
) -> ContinentalCatchment {
    let hash = coordinate_hash(descriptor.seed, CATCHMENT_HASH_DOMAIN, owner_x, owner_z, 0);
    let direction_index = ((hash >> 8) & 7) as usize;
    let (downstream_axis_x, downstream_axis_z) = direction(direction_index);
    let across_axis_x = -downstream_axis_z;
    let across_axis_z = downstream_axis_x;
    let jitter_across = signed_hash_unit(hash, 16) * OWNER_JITTER_BLOCKS;
    let jitter_downstream = signed_hash_unit(hash, 29) * OWNER_JITTER_BLOCKS;
    let cell_center_x = i64::from(owner_x) * i64::from(CONTINENTAL_CATCHMENT_CELL_BLOCKS)
        + i64::from(CONTINENTAL_CATCHMENT_CELL_BLOCKS / 2);
    let cell_center_z = i64::from(owner_z) * i64::from(CONTINENTAL_CATCHMENT_CELL_BLOCKS)
        + i64::from(CONTINENTAL_CATCHMENT_CELL_BLOCKS / 2);
    let center_x = cell_center_x
        + (jitter_across * across_axis_x + jitter_downstream * downstream_axis_x).round() as i64;
    let center_z = cell_center_z
        + (jitter_across * across_axis_z + jitter_downstream * downstream_axis_z).round() as i64;
    let width_scale = 0.90 + hash_unit(hash, 42) * 0.20;
    let length_scale = 0.92 + hash_unit(hash, 51) * 0.16;
    let across_warp = signed_hash_unit(hash, 4) * 720.0;
    let downstream_warp = signed_hash_unit(hash, 36) * 520.0;
    let lake_y = 69.0 + ((hash >> 60) & 3) as f64;

    let nodes = [
        CatchmentLocalPoint::new(-5_300.0 + across_warp, -12_400.0, lake_y + 66.0),
        CatchmentLocalPoint::new(-500.0, -13_200.0, lake_y + 73.0),
        CatchmentLocalPoint::new(5_100.0 - across_warp * 0.4, -11_700.0, lake_y + 61.0),
        CatchmentLocalPoint::new(-2_900.0, -6_700.0, lake_y + 39.0),
        CatchmentLocalPoint::new(2_800.0, -6_200.0, lake_y + 35.0),
        CatchmentLocalPoint::new(100.0 + downstream_warp, -2_500.0, lake_y + 22.0),
        CatchmentLocalPoint::new(650.0, 2_000.0, lake_y + 10.0),
        CatchmentLocalPoint::new(-250.0, 4_900.0, lake_y + 2.0),
        CatchmentLocalPoint::new(0.0, 7_200.0, lake_y - 4.0),
        CatchmentLocalPoint::new(1_250.0, 10_150.0, lake_y - 1.0),
        CatchmentLocalPoint::new(2_600.0, 13_100.0, lake_y - 6.0),
    ];

    let id = feature_id(
        HydrographyFeatureFamily::Catchment,
        owner_x,
        owner_z,
        0,
        hash,
    );
    let reach = |slot: u8,
                 kind: ContinentalReachKind,
                 start_node: u8,
                 end_node: u8,
                 control: CatchmentLocalPoint,
                 downstream: ContinentalReachDownstream,
                 order: u8,
                 discharge: f32,
                 head_width_blocks: f32,
                 tail_width_blocks: f32,
                 influence_radius_blocks: f32| {
        let reach_hash = stable_mix64(hash ^ u64::from(slot).rotate_left(23));
        ContinentalCatchmentReach {
            id: feature_id(
                HydrographyFeatureFamily::Reach,
                owner_x,
                owner_z,
                slot,
                reach_hash,
            ),
            slot,
            kind,
            start_node,
            end_node,
            control,
            downstream,
            order,
            discharge,
            head_water_y: nodes[usize::from(start_node)].bed_y as f32 + 1.5,
            tail_water_y: nodes[usize::from(end_node)].bed_y as f32 + 1.5,
            head_width_blocks,
            tail_width_blocks,
            influence_radius_blocks,
        }
    };
    let reaches = [
        reach(
            0,
            ContinentalReachKind::Headwater,
            0,
            3,
            CatchmentLocalPoint::new(-4_900.0, -9_100.0, lake_y + 53.0),
            ContinentalReachDownstream::Reach(3),
            1,
            1.0,
            2.0,
            5.0,
            1_100.0,
        ),
        reach(
            1,
            ContinentalReachKind::Headwater,
            1,
            3,
            CatchmentLocalPoint::new(-1_100.0, -9_500.0, lake_y + 55.0),
            ContinentalReachDownstream::Reach(3),
            1,
            1.2,
            2.0,
            5.0,
            1_100.0,
        ),
        reach(
            2,
            ContinentalReachKind::Headwater,
            2,
            4,
            CatchmentLocalPoint::new(4_600.0, -8_900.0, lake_y + 48.0),
            ContinentalReachDownstream::Reach(4),
            1,
            1.1,
            2.0,
            5.0,
            1_100.0,
        ),
        reach(
            3,
            ContinentalReachKind::Tributary,
            3,
            5,
            CatchmentLocalPoint::new(-2_200.0, -4_400.0, lake_y + 30.0),
            ContinentalReachDownstream::Reach(5),
            2,
            2.2,
            5.0,
            9.0,
            1_450.0,
        ),
        reach(
            4,
            ContinentalReachKind::Tributary,
            4,
            5,
            CatchmentLocalPoint::new(2_300.0, -4_000.0, lake_y + 28.0),
            ContinentalReachDownstream::Reach(5),
            2,
            2.0,
            5.0,
            9.0,
            1_450.0,
        ),
        reach(
            5,
            ContinentalReachKind::Trunk,
            5,
            6,
            CatchmentLocalPoint::new(-450.0, -100.0, lake_y + 16.0),
            ContinentalReachDownstream::Reach(6),
            3,
            4.2,
            9.0,
            15.0,
            2_200.0,
        ),
        reach(
            6,
            ContinentalReachKind::LakeInlet,
            6,
            7,
            CatchmentLocalPoint::new(1_000.0, 3_700.0, lake_y + 6.0),
            ContinentalReachDownstream::Lake,
            3,
            4.7,
            15.0,
            20.0,
            2_500.0,
        ),
        reach(
            7,
            ContinentalReachKind::Outlet,
            9,
            10,
            CatchmentLocalPoint::new(2_450.0, 11_100.0, lake_y - 3.0),
            ContinentalReachDownstream::Terminal,
            3,
            4.7,
            16.0,
            20.0,
            2_300.0,
        ),
    ];

    ContinentalCatchment {
        id,
        owner_x,
        owner_z,
        center_x,
        center_z,
        downstream_axis_x,
        downstream_axis_z,
        across_axis_x,
        across_axis_z,
        half_width_blocks: BASE_HALF_WIDTH_BLOCKS * width_scale,
        upstream_blocks: BASE_UPSTREAM_BLOCKS * length_scale,
        downstream_blocks: BASE_DOWNSTREAM_BLOCKS * length_scale,
        nodes,
        reaches,
        lake_id: feature_id(
            HydrographyFeatureFamily::Lake,
            owner_x,
            owner_z,
            0,
            stable_mix64(hash ^ 0x6c61_6b65),
        ),
        lake_center: nodes[8],
        lake_radius_across_blocks: (3_900.0 * width_scale) as f32,
        lake_radius_downstream_blocks: (2_650.0 * length_scale) as f32,
        lake_water_y: lake_y as f32,
        spill_id: feature_id(
            HydrographyFeatureFamily::Spill,
            owner_x,
            owner_z,
            0,
            stable_mix64(hash ^ 0x7370_696c_6c),
        ),
        spill_node: 9,
        outlet_reach: 7,
        open_basin: true,
    }
}

fn evaluate_catchment(
    catchment: &ContinentalCatchment,
    local_across: f64,
    local_downstream: f64,
    work: &mut ContinentalHydrographyWork,
) -> ContinentalHydrographySample {
    let point = CatchmentLocalPoint::new(local_across, local_downstream, 0.0);
    let normalized_extent = ((local_across / catchment.half_width_blocks).powi(2)
        + if local_downstream < 0.0 {
            (local_downstream / catchment.upstream_blocks).powi(2)
        } else {
            (local_downstream / catchment.downstream_blocks).powi(2)
        })
    .sqrt();
    let catchment_weight = inverse_smoothstep(0.72, 1.02, normalized_extent);

    let divide_center = -10_300.0 + local_across * 0.055;
    let divide_distance = (local_downstream - divide_center).abs();
    let divide_weight = inverse_smoothstep(500.0, 3_600.0, divide_distance) * catchment_weight;
    let crest_weight = inverse_smoothstep(80.0, 460.0, divide_distance)
        * inverse_smoothstep(8_200.0, 11_500.0, local_across.abs());
    let peak_weight = [
        (-5_200.0, -10_650.0, 760.0),
        (-1_550.0, -11_100.0, 620.0),
        (1_550.0, -10_850.0, 660.0),
        (5_200.0, -9_900.0, 820.0),
    ]
    .into_iter()
    .map(|(peak_across, peak_downstream, radius)| {
        inverse_smoothstep(
            radius * 0.04,
            radius,
            (local_across - peak_across).hypot(local_downstream - peak_downstream),
        )
    })
    .fold(0.0_f64, f64::max);
    let branch_weight = [
        ((-5_200.0, -10_650.0), (-6_600.0, -2_700.0)),
        ((-1_550.0, -11_100.0), (400.0, -3_700.0)),
        ((1_550.0, -10_850.0), (4_900.0, -2_500.0)),
    ]
    .into_iter()
    .map(
        |((start_across, start_downstream), (end_across, end_downstream))| {
            let distance = line_segment_distance(
                point,
                CatchmentLocalPoint::new(start_across, start_downstream, 0.0),
                CatchmentLocalPoint::new(end_across, end_downstream, 0.0),
            )
            .distance;
            let downstream_fade = inverse_smoothstep(-2_800.0, 1_200.0, local_downstream);
            inverse_smoothstep(60.0, 390.0, distance) * downstream_fade * 0.72
        },
    )
    .fold(0.0_f64, f64::max);
    let range_weight = peak_weight.max(crest_weight * 0.58).max(branch_weight) * catchment_weight;
    let saddle_weight = inverse_smoothstep(0.0, 1_350.0, local_across.abs())
        * inverse_smoothstep(0.0, 1_100.0, divide_distance)
        * catchment_weight;
    let confluences = [(3_usize, 0_u8, 120.0_f64), (5_usize, 1_u8, 220.0_f64)];
    let nearest_confluence = confluences
        .into_iter()
        .map(|(node, slot, radius)| {
            let distance = distance_to_node(point, catchment.nodes[node]);
            (node, slot, radius, distance)
        })
        .min_by(|left, right| left.3.total_cmp(&right.3));
    let (confluence_id, confluence_distance_blocks, confluence_weight, confluence_bed_y) =
        nearest_confluence.map_or(
            (None, f32::INFINITY, 0.0, None),
            |(node, slot, radius, distance)| {
                if distance >= radius {
                    return (None, distance as f32, 0.0, None);
                }
                (
                    Some(feature_id(
                        HydrographyFeatureFamily::Confluence,
                        catchment.owner_x,
                        catchment.owner_z,
                        slot,
                        stable_mix64(catchment.id.hash ^ 0x636f_6e66_0000_0000 ^ u64::from(slot)),
                    )),
                    distance as f32,
                    (inverse_smoothstep(radius * 0.28, radius, distance) * catchment_weight) as f32,
                    Some(catchment.nodes[node].bed_y as f32),
                )
            },
        );

    let mut nearest: Option<(usize, ReachDistance)> = None;
    let mut valley_weight = 0.0_f64;
    let mut floodplain_weight = 0.0_f64;
    for (index, reach) in catchment.reaches.iter().enumerate() {
        work.reach_evaluations += 1;
        let distance = quadratic_reach_distance(
            point,
            catchment.nodes[usize::from(reach.start_node)],
            reach.control,
            catchment.nodes[usize::from(reach.end_node)],
        );
        let influence = inverse_smoothstep(
            f64::from(reach.influence_radius_blocks) * 0.28,
            f64::from(reach.influence_radius_blocks),
            distance.distance,
        );
        valley_weight = valley_weight.max(influence);
        if matches!(
            reach.kind,
            ContinentalReachKind::Trunk | ContinentalReachKind::LakeInlet
        ) {
            floodplain_weight = floodplain_weight.max(inverse_smoothstep(
                f64::from(reach.influence_radius_blocks) * 0.45,
                f64::from(reach.influence_radius_blocks) * 1.45,
                distance.distance,
            ));
        }
        if nearest.is_none_or(|(_, current)| distance.distance < current.distance) {
            nearest = Some((index, distance));
        }
    }

    let lake_across = local_across - catchment.lake_center.across;
    let lake_downstream = local_downstream - catchment.lake_center.downstream;
    let normalized_lake = ((lake_across / f64::from(catchment.lake_radius_across_blocks)).powi(2)
        + (lake_downstream / f64::from(catchment.lake_radius_downstream_blocks)).powi(2))
    .sqrt();
    let lake_signed_distance_blocks = (normalized_lake - 1.0)
        * f64::from(
            catchment
                .lake_radius_across_blocks
                .min(catchment.lake_radius_downstream_blocks),
        );
    let lake_weight = inverse_smoothstep(-350.0, 700.0, lake_signed_distance_blocks);

    let (
        reach_id,
        reach_slot,
        reach_kind,
        reach_order,
        discharge,
        channel_signed_distance,
        channel_distance,
        channel_width,
        bankfull_width,
        reach_progress,
        bed_y,
        water_level_y,
        downstream_x,
        downstream_z,
    ) = nearest.map_or(
        (
            None,
            None,
            None,
            0,
            0.0,
            f32::INFINITY,
            f32::INFINITY,
            0.0,
            0.0,
            0.0,
            None,
            None,
            0.0,
            0.0,
        ),
        |(index, distance)| {
            let reach = catchment.reaches[index];
            let width = lerp(
                f64::from(reach.head_width_blocks),
                f64::from(reach.tail_width_blocks),
                distance.progress,
            );
            let bed = lerp(
                catchment.nodes[usize::from(reach.start_node)].bed_y,
                catchment.nodes[usize::from(reach.end_node)].bed_y,
                distance.progress,
            );
            let water = lerp(
                f64::from(reach.head_water_y),
                f64::from(reach.tail_water_y),
                distance.progress,
            );
            let tangent_length = distance
                .tangent_across
                .hypot(distance.tangent_downstream)
                .max(f64::EPSILON);
            let local_tangent_across = distance.tangent_across / tangent_length;
            let local_tangent_downstream = distance.tangent_downstream / tangent_length;
            let world_tangent_x = local_tangent_across * catchment.across_axis_x
                + local_tangent_downstream * catchment.downstream_axis_x;
            let world_tangent_z = local_tangent_across * catchment.across_axis_z
                + local_tangent_downstream * catchment.downstream_axis_z;
            (
                Some(reach.id),
                Some(reach.slot),
                Some(reach.kind),
                reach.order,
                reach.discharge,
                distance.signed_distance as f32,
                distance.distance as f32,
                width as f32,
                (width * (2.6 + f64::from(reach.order) * 0.45)) as f32,
                distance.progress as f32,
                Some(bed as f32),
                Some(water as f32),
                world_tangent_x as f32,
                world_tangent_z as f32,
            )
        },
    );

    let inlet_distance = distance_to_node(point, catchment.nodes[7]);
    let outlet_distance = distance_to_node(point, catchment.nodes[9]);
    let shore_intent = if lake_signed_distance_blocks.abs() > 1_250.0 {
        ContinentalShoreIntent::None
    } else if inlet_distance < 1_350.0 {
        ContinentalShoreIntent::Inlet
    } else if outlet_distance < 1_150.0 {
        ContinentalShoreIntent::Outlet
    } else if lake_across < -1_500.0 {
        ContinentalShoreIntent::Wetland
    } else if lake_downstream < -1_200.0 {
        ContinentalShoreIntent::Depositional
    } else if lake_across > 2_100.0 {
        ContinentalShoreIntent::Rocky
    } else if lake_downstream > 1_400.0 {
        ContinentalShoreIntent::Gravel
    } else {
        ContinentalShoreIntent::Ordinary
    };

    ContinentalHydrographySample {
        catchment_id: catchment.id,
        local_across: local_across as f32,
        local_downstream: local_downstream as f32,
        catchment_weight: catchment_weight as f32,
        divide_weight: divide_weight as f32,
        range_weight: range_weight as f32,
        saddle_weight: saddle_weight as f32,
        valley_weight: (valley_weight * catchment_weight) as f32,
        floodplain_weight: (floodplain_weight * catchment_weight) as f32,
        confluence_id,
        confluence_distance_blocks,
        confluence_weight,
        confluence_bed_y,
        reach_id,
        reach_slot,
        reach_kind,
        reach_order,
        discharge,
        channel_signed_distance_blocks: channel_signed_distance,
        channel_distance_blocks: channel_distance,
        channel_width_blocks: channel_width,
        bankfull_width_blocks: bankfull_width,
        reach_progress,
        bed_y,
        water_level_y,
        downstream_x,
        downstream_z,
        lake_id: (lake_signed_distance_blocks < 1_250.0).then_some(catchment.lake_id),
        lake_signed_distance_blocks: lake_signed_distance_blocks as f32,
        lake_weight: (lake_weight * catchment_weight) as f32,
        lake_water_y: (lake_signed_distance_blocks < 1_250.0).then_some(catchment.lake_water_y),
        spill_id: (outlet_distance < 1_150.0).then_some(catchment.spill_id),
        shore_intent,
    }
}

fn quadratic_reach_distance(
    point: CatchmentLocalPoint,
    start: CatchmentLocalPoint,
    control: CatchmentLocalPoint,
    end: CatchmentLocalPoint,
) -> ReachDistance {
    const SEGMENTS: usize = 8;
    let mut best = ReachDistance {
        signed_distance: f64::INFINITY,
        distance: f64::INFINITY,
        progress: 0.0,
        tangent_across: end.across - start.across,
        tangent_downstream: end.downstream - start.downstream,
    };
    let mut segment_start = start;
    for segment in 0..SEGMENTS {
        let start_progress = segment as f64 / SEGMENTS as f64;
        let end_progress = (segment + 1) as f64 / SEGMENTS as f64;
        let segment_end = quadratic_point(start, control, end, end_progress);
        let candidate = line_segment_distance(point, segment_start, segment_end);
        if candidate.distance < best.distance {
            best = ReachDistance {
                signed_distance: candidate.signed_distance,
                distance: candidate.distance,
                progress: lerp(start_progress, end_progress, candidate.progress),
                tangent_across: segment_end.across - segment_start.across,
                tangent_downstream: segment_end.downstream - segment_start.downstream,
            };
        }
        segment_start = segment_end;
    }
    best
}

fn quadratic_point(
    start: CatchmentLocalPoint,
    control: CatchmentLocalPoint,
    end: CatchmentLocalPoint,
    progress: f64,
) -> CatchmentLocalPoint {
    let inverse = 1.0 - progress;
    CatchmentLocalPoint::new(
        inverse * inverse * start.across
            + 2.0 * inverse * progress * control.across
            + progress * progress * end.across,
        inverse * inverse * start.downstream
            + 2.0 * inverse * progress * control.downstream
            + progress * progress * end.downstream,
        0.0,
    )
}

fn line_segment_distance(
    point: CatchmentLocalPoint,
    start: CatchmentLocalPoint,
    end: CatchmentLocalPoint,
) -> ReachDistance {
    let across = end.across - start.across;
    let downstream = end.downstream - start.downstream;
    let length_squared = across * across + downstream * downstream;
    let progress = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((point.across - start.across) * across
            + (point.downstream - start.downstream) * downstream)
            / length_squared)
            .clamp(0.0, 1.0)
    };
    let closest_across = start.across + across * progress;
    let closest_downstream = start.downstream + downstream * progress;
    ReachDistance {
        signed_distance: if length_squared <= f64::EPSILON {
            (point.across - closest_across).hypot(point.downstream - closest_downstream)
        } else {
            ((point.across - closest_across) * downstream
                - (point.downstream - closest_downstream) * across)
                / length_squared.sqrt()
        },
        distance: (point.across - closest_across).hypot(point.downstream - closest_downstream),
        progress,
        tangent_across: across,
        tangent_downstream: downstream,
    }
}

fn distance_to_node(point: CatchmentLocalPoint, node: CatchmentLocalPoint) -> f64 {
    (point.across - node.across).hypot(point.downstream - node.downstream)
}

fn world_to_local(
    descriptor: ContinentalEcoregionDescriptor,
    catchment: &ContinentalCatchment,
    canonical_world_x: i32,
    world_z: i32,
) -> (f64, f64) {
    let delta_x = match descriptor.topology {
        ContinentalEcoregionTopology::Plane => {
            f64::from(canonical_world_x) - catchment.center_x as f64
        }
        ContinentalEcoregionTopology::CylinderX { period_blocks } => {
            let raw = i64::from(canonical_world_x) - catchment.center_x;
            wrapped_delta(raw, i64::from(period_blocks)) as f64
        }
    };
    let delta_z = f64::from(world_z) - catchment.center_z as f64;
    (
        delta_x * catchment.across_axis_x + delta_z * catchment.across_axis_z,
        delta_x * catchment.downstream_axis_x + delta_z * catchment.downstream_axis_z,
    )
}

fn canonical_world_x(descriptor: ContinentalEcoregionDescriptor, world_x: i32) -> i32 {
    match descriptor.topology {
        ContinentalEcoregionTopology::Plane => world_x,
        ContinentalEcoregionTopology::CylinderX { period_blocks } => {
            world_x.rem_euclid(period_blocks)
        }
    }
}

fn canonical_owner_x(descriptor: ContinentalEcoregionDescriptor, owner_x: i32) -> i32 {
    match descriptor.topology {
        ContinentalEcoregionTopology::Plane => owner_x,
        ContinentalEcoregionTopology::CylinderX { period_blocks }
            if period_blocks % CONTINENTAL_CATCHMENT_CELL_BLOCKS == 0 =>
        {
            owner_x.rem_euclid(period_blocks / CONTINENTAL_CATCHMENT_CELL_BLOCKS)
        }
        ContinentalEcoregionTopology::CylinderX { .. } => owner_x,
    }
}

fn wrapped_delta(delta: i64, period: i64) -> i64 {
    let canonical = delta.rem_euclid(period);
    if canonical * 2 > period {
        canonical - period
    } else {
        canonical
    }
}

fn direction(index: usize) -> (f64, f64) {
    [
        (0.0, 1.0),
        (SQRT_HALF, SQRT_HALF),
        (1.0, 0.0),
        (SQRT_HALF, -SQRT_HALF),
        (0.0, -1.0),
        (-SQRT_HALF, -SQRT_HALF),
        (-1.0, 0.0),
        (-SQRT_HALF, SQRT_HALF),
    ][index]
}

fn feature_id(
    family: HydrographyFeatureFamily,
    owner_x: i32,
    owner_z: i32,
    slot: u8,
    hash: u64,
) -> HydrographyFeatureId {
    HydrographyFeatureId {
        family,
        owner_x,
        owner_z,
        slot,
        hash: hash | 1,
    }
}

fn coordinate_hash(seed: i64, domain: u64, x: i32, z: i32, slot: u8) -> u64 {
    let mut value = stable_mix64(seed as u64 ^ domain);
    value = stable_mix64(value ^ x as u32 as u64);
    value = stable_mix64(value ^ (z as u32 as u64).rotate_left(32));
    stable_mix64(value ^ u64::from(slot))
}

fn stable_mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn hash_unit(hash: u64, shift: u32) -> f64 {
    let bits = hash.rotate_right(shift) >> 11;
    bits as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn signed_hash_unit(hash: u64, shift: u32) -> f64 {
    hash_unit(hash, shift) * 2.0 - 1.0
}

fn inverse_smoothstep(inner: f64, outer: f64, distance: f64) -> f64 {
    1.0 - smoothstep(inner, outer, distance)
}

fn smoothstep(low: f64, high: f64, value: f64) -> f64 {
    let unit = ((value - low) / (high - low)).clamp(0.0, 1.0);
    unit * unit * (3.0 - 2.0 * unit)
}

fn lerp(left: f64, right: f64, weight: f64) -> f64 {
    left + (right - left) * weight
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: i64 = 12_345;

    #[test]
    fn catchment_graph_is_bounded_acyclic_and_source_to_lake_connected() {
        let plan = ContinentalHydrographyPlan::new(ContinentalEcoregionDescriptor::plane(SEED));
        let catchment = plan.catchment_for_owner(0, 0);
        assert_eq!(catchment.nodes.len(), CONTINENTAL_CATCHMENT_MAX_NODES);
        assert_eq!(catchment.reaches.len(), CONTINENTAL_CATCHMENT_MAX_REACHES);
        assert!(catchment.open_basin);
        assert_eq!(catchment.outlet_reach, 7);
        for slot in 0..=6 {
            assert!(catchment.reaches_lake(slot));
        }
        assert!(!catchment.reaches_lake(7));
        for reach in &catchment.reaches {
            assert!(reach.tail_water_y <= reach.head_water_y);
            if let ContinentalReachDownstream::Reach(next) = reach.downstream {
                assert!(
                    next > reach.slot,
                    "reach graph must be topologically ordered"
                );
            }
        }
    }

    #[test]
    fn direct_queries_find_headwaters_confluences_lake_and_outlet() {
        let plan = ContinentalHydrographyPlan::new(ContinentalEcoregionDescriptor::plane(SEED));
        let catchment = plan.catchment_for_owner(0, 0);
        for (slot, expected_kind) in [
            (0, ContinentalReachKind::Headwater),
            (3, ContinentalReachKind::Tributary),
            (5, ContinentalReachKind::Trunk),
            (6, ContinentalReachKind::LakeInlet),
            (7, ContinentalReachKind::Outlet),
        ] {
            let reach = catchment.reaches[slot];
            let midpoint = quadratic_point(
                catchment.nodes[usize::from(reach.start_node)],
                reach.control,
                catchment.nodes[usize::from(reach.end_node)],
                0.5,
            );
            let (world_x, world_z) = catchment.local_to_world(midpoint);
            let query = plan.query_point(world_x.round() as i32, world_z.round() as i32);
            let sample = query.sample.expect("catchment node remains in bounds");
            assert_eq!(sample.catchment_id, catchment.id);
            assert_eq!(sample.reach_slot, Some(slot as u8));
            assert_eq!(sample.reach_kind, Some(expected_kind));
            assert_eq!(query.work.owner_evaluations, 9);
            assert_eq!(query.work.graph_constructions, 10);
            assert_eq!(query.work.reach_evaluations, 8);
            assert_eq!(query.work.exact_chunks, 0);
            assert_eq!(query.work.raster_cells, 0);
        }
        let (lake_x, lake_z) = catchment.local_to_world(catchment.lake_center);
        let lake = plan
            .query_point(lake_x.round() as i32, lake_z.round() as i32)
            .sample
            .expect("lake center");
        assert_eq!(lake.lake_id, Some(catchment.lake_id));
        assert!(lake.lake_signed_distance_blocks < 0.0);
        assert_eq!(lake.lake_water_y, Some(catchment.lake_water_y));
    }

    #[test]
    fn arbitrary_query_order_does_not_change_facts() {
        let plan = ContinentalHydrographyPlan::new(ContinentalEcoregionDescriptor::plane(SEED));
        let points = [
            (-12_345, 98_765),
            (0, 0),
            (31_999, -47_001),
            (-65_536, -65_536),
            (8_192, 12_288),
        ];
        let forward = points.map(|(x, z)| plan.query_point(x, z));
        let mut reverse = points;
        reverse.reverse();
        let mut reversed = reverse.map(|(x, z)| plan.query_point(x, z));
        reversed.reverse();
        assert_eq!(forward, reversed);
    }

    #[test]
    fn bounded_query_cache_changes_cost_but_not_hydrography() {
        let plan = ContinentalHydrographyPlan::new(ContinentalEcoregionDescriptor::plane(12_345));
        let mut cache = ContinentalHydrographyQueryCache::default();
        let direct = plan.query_point(18_470, -49_535);
        let cold = plan.query_point_cached(18_470, -49_535, &mut cache);
        let warm = plan.query_point_cached(18_470, -49_535, &mut cache);
        assert_eq!(cold.sample, direct.sample);
        assert_eq!(warm.sample, direct.sample);
        assert_eq!(cold.work.graph_constructions, 9);
        assert_eq!(warm.work.graph_constructions, 0);
        assert_eq!(cache.retained_catchments(), 9);

        let mut reset = ContinentalHydrographyQueryCache::default();
        let rebuilt = plan.query_point_cached(18_470, -49_535, &mut reset);
        assert_eq!(rebuilt.sample, direct.sample);
        assert_eq!(rebuilt.work, cold.work);
    }

    #[test]
    fn supported_large_cylinder_repeats_graph_and_samples_at_the_seam() {
        let period = 196_608;
        let descriptor = ContinentalEcoregionDescriptor::new(
            SEED,
            ContinentalEcoregionTopology::cylinder_x(period),
        );
        let plan = ContinentalHydrographyPlan::new(descriptor);
        for (x, z) in [(-1, 0), (17_231, -8_234), (195_700, 63_221)] {
            let base = plan.query_point(x, z);
            let lifted = plan.query_point(x + period, z);
            assert_eq!(base, lifted);
        }
    }
}
