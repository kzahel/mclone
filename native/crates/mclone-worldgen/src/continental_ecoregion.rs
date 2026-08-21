//! Research candidate for authored continental and ecoregional planning.
//!
//! The candidate is deliberately disconnected from production chunk
//! generation. It exposes coordinate-pure typed facts and bounded point/window
//! queries so Terrain Lab can judge the geography before it reaches the live
//! Overworld.

use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::noise::{GradientNoise2d, SeedDomain};

pub const CONTINENTAL_ECOREGION_SCHEMA_REVISION: &str = "mclone-continental-ecoregion-plan-v9";
pub const CONTINENTAL_ECOREGION_DIMENSION_ID: &str = "mclone:overworld";
pub const CONTINENTAL_ECOREGION_STORED_PROFILE: &str =
    "mclone-overworld-v1-control-field-revision-21";
pub const CONTINENTAL_CELL_BLOCKS: i32 = 32_768;
pub const PROVINCE_CELL_BLOCKS: i32 = 16_384;
pub const ECOREGION_CELL_BLOCKS: i32 = 8_192;
pub const MOSAIC_CELL_BLOCKS: i32 = 4_096;
pub const MIN_SUPPORTED_CYLINDER_BLOCKS: i32 = 131_072;
pub const MAX_WINDOW_SAMPLES: usize = 262_144;

const CONTINENT_OWNER_RADIUS: i32 = 2;
const LOCAL_OWNER_RADIUS: i32 = 1;
const CONTINENT_EDGE_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_6564_6731);
const CLIMATE_MOISTURE_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_6d6f_6931);
const CLIMATE_TEMPERATURE_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_7465_6d31);
const CORRIDOR_WARP_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_636f_7231);
const LOCAL_OPENNESS_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_6f70_6531);
const OWNER_WARP_X_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_7778_5f31);
const OWNER_WARP_Z_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_777a_5f31);

const CONTINENT_HASH_DOMAIN: u64 = 0x6365_636f_636f_6e31;
const PROVINCE_HASH_DOMAIN: u64 = 0x6365_636f_7072_6f31;
const ECOREGION_HASH_DOMAIN: u64 = 0x6365_636f_6563_6f31;
const MOSAIC_HASH_DOMAIN: u64 = 0x6365_636f_6d6f_7331;
const LOCAL_HASH_DOMAIN: u64 = 0x6365_636f_6c6f_6331;
const HABITAT_ROUTE_HASH_DOMAIN: u64 = 0x6365_636f_726f_7574;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ContinentalEcoregionTopology {
    Plane,
    CylinderX { period_blocks: i32 },
}

impl ContinentalEcoregionTopology {
    pub const fn plane() -> Self {
        Self::Plane
    }

    pub const fn cylinder_x(period_blocks: i32) -> Self {
        Self::CylinderX { period_blocks }
    }

    pub fn compatibility(self) -> PlanTopologyCompatibility {
        match self {
            Self::Plane => PlanTopologyCompatibility {
                supported: true,
                period_blocks: None,
                minimum_period_blocks: MIN_SUPPORTED_CYLINDER_BLOCKS,
                alignment_blocks: CONTINENTAL_CELL_BLOCKS,
                reason: "unbounded plane uses lazy coordinate-pure owners",
            },
            Self::CylinderX { period_blocks } if period_blocks < MIN_SUPPORTED_CYLINDER_BLOCKS => {
                PlanTopologyCompatibility {
                    supported: false,
                    period_blocks: Some(period_blocks),
                    minimum_period_blocks: MIN_SUPPORTED_CYLINDER_BLOCKS,
                    alignment_blocks: CONTINENTAL_CELL_BLOCKS,
                    reason: "period is too small for Revision 1 continental influence",
                }
            }
            Self::CylinderX { period_blocks }
                if period_blocks.rem_euclid(CONTINENTAL_CELL_BLOCKS) != 0 =>
            {
                PlanTopologyCompatibility {
                    supported: false,
                    period_blocks: Some(period_blocks),
                    minimum_period_blocks: MIN_SUPPORTED_CYLINDER_BLOCKS,
                    alignment_blocks: CONTINENTAL_CELL_BLOCKS,
                    reason: "period must align to the continental owner vocabulary",
                }
            }
            Self::CylinderX { period_blocks } => PlanTopologyCompatibility {
                supported: true,
                period_blocks: Some(period_blocks),
                minimum_period_blocks: MIN_SUPPORTED_CYLINDER_BLOCKS,
                alignment_blocks: CONTINENTAL_CELL_BLOCKS,
                reason: "period supports distinct continental owners and seam-safe influence",
            },
        }
    }

    fn canonical_owner_x(self, owner_x: i32, scale: i32) -> i32 {
        match self {
            Self::Plane => owner_x,
            Self::CylinderX { period_blocks } => owner_x.rem_euclid(period_blocks / scale),
        }
    }

    fn canonical_world_x(self, world_x: i32) -> i32 {
        match self {
            Self::Plane => world_x,
            Self::CylinderX { period_blocks } => world_x.rem_euclid(period_blocks),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTopologyCompatibility {
    pub supported: bool,
    pub period_blocks: Option<i32>,
    pub minimum_period_blocks: i32,
    pub alignment_blocks: i32,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalEcoregionDescriptor {
    pub seed: i64,
    pub topology: ContinentalEcoregionTopology,
}

impl ContinentalEcoregionDescriptor {
    pub const fn new(seed: i64, topology: ContinentalEcoregionTopology) -> Self {
        Self { seed, topology }
    }

    pub const fn plane(seed: i64) -> Self {
        Self::new(seed, ContinentalEcoregionTopology::Plane)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LandscapePlanDetail {
    Continental,
    Province,
    Ecoregion,
    Mosaic,
}

impl LandscapePlanDetail {
    pub const ALL: [Self; 4] = [
        Self::Continental,
        Self::Province,
        Self::Ecoregion,
        Self::Mosaic,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Continental => "continental",
            Self::Province => "province",
            Self::Ecoregion => "ecoregion",
            Self::Mosaic => "mosaic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LandscapeFeatureFamily {
    ContinentalDistrict,
    PhysiographicProvince,
    Ecoregion,
    Clearing,
    HabitatRoute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandscapeFeatureId {
    pub family: LandscapeFeatureFamily,
    pub owner_x: i32,
    pub owner_z: i32,
    pub slot: u8,
    pub hash: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalStory {
    RiverValley,
    LakeDistrict,
    Escarpment,
    OpenHighland,
}

impl ContinentalStory {
    pub const ALL: [Self; 4] = [
        Self::RiverValley,
        Self::LakeDistrict,
        Self::Escarpment,
        Self::OpenHighland,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::RiverValley => "river-valley",
            Self::LakeDistrict => "lake-district",
            Self::Escarpment => "escarpment",
            Self::OpenHighland => "open-highland",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum PhysiographicProvinceKind {
    RiverLowland,
    LakeBasin,
    RollingHills,
    WoodedUpland,
    RockyRidge,
    QuietBench,
}

impl PhysiographicProvinceKind {
    pub const ALL: [Self; 6] = [
        Self::RiverLowland,
        Self::LakeBasin,
        Self::RollingHills,
        Self::WoodedUpland,
        Self::RockyRidge,
        Self::QuietBench,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::RiverLowland => "river-lowland",
            Self::LakeBasin => "lake-basin",
            Self::RollingHills => "rolling-hills",
            Self::WoodedUpland => "wooded-upland",
            Self::RockyRidge => "rocky-ridge",
            Self::QuietBench => "quiet-bench",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum EcoregionKind {
    OldForestCore,
    BroadMeadow,
    RiparianWoodland,
    ConnectedWetland,
    MixedWoodland,
    ExposedUpland,
    QuietTransition,
}

impl EcoregionKind {
    pub const ALL: [Self; 7] = [
        Self::OldForestCore,
        Self::BroadMeadow,
        Self::RiparianWoodland,
        Self::ConnectedWetland,
        Self::MixedWoodland,
        Self::ExposedUpland,
        Self::QuietTransition,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::OldForestCore => "old-forest-core",
            Self::BroadMeadow => "broad-meadow",
            Self::RiparianWoodland => "riparian-woodland",
            Self::ConnectedWetland => "connected-wetland",
            Self::MixedWoodland => "mixed-woodland",
            Self::ExposedUpland => "exposed-upland",
            Self::QuietTransition => "quiet-transition",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ClearingCause {
    GrazingLawn,
    OldBurn,
    Windthrow,
    FloodMeadow,
    ShallowSoil,
}

impl ClearingCause {
    pub const ALL: [Self; 5] = [
        Self::GrazingLawn,
        Self::OldBurn,
        Self::Windthrow,
        Self::FloodMeadow,
        Self::ShallowSoil,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::GrazingLawn => "grazing-lawn",
            Self::OldBurn => "old-burn",
            Self::Windthrow => "windthrow",
            Self::FloodMeadow => "flood-meadow",
            Self::ShallowSoil => "shallow-soil",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum HabitatRouteKind {
    RiparianSpine,
    WetlandChain,
    WoodlandPass,
    OpenRangeLink,
}

impl HabitatRouteKind {
    pub const ALL: [Self; 4] = [
        Self::RiparianSpine,
        Self::WetlandChain,
        Self::WoodlandPass,
        Self::OpenRangeLink,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::RiparianSpine => "riparian-spine",
            Self::WetlandChain => "wetland-chain",
            Self::WoodlandPass => "woodland-pass",
            Self::OpenRangeLink => "open-range-link",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalDistrictSample {
    pub id: LandscapeFeatureId,
    pub story: ContinentalStory,
    pub center_x: i64,
    pub center_z: i64,
    pub axis_x: f32,
    pub axis_z: f32,
    pub prevailing_wind_x: f32,
    pub prevailing_wind_z: f32,
    pub rain_shadow_potential: f32,
    pub core_weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvincePlanSample {
    pub id: LandscapeFeatureId,
    pub continent_id: LandscapeFeatureId,
    pub kind: PhysiographicProvinceKind,
    pub core_weight: f32,
    pub relief: f32,
    pub major_water: f32,
    pub leeward_exposure: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcoregionPlanSample {
    pub id: LandscapeFeatureId,
    pub province_id: LandscapeFeatureId,
    pub kind: EcoregionKind,
    pub transition_peer_kind: Option<EcoregionKind>,
    pub core_weight: f32,
    pub transition_weight: f32,
    pub transition_width_blocks: f32,
    pub base_openness: f32,
    pub base_canopy: f32,
    pub moisture: f32,
    pub temperature: f32,
    pub aridity: f32,
    pub drainage_permanence: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandscapeMosaicSample {
    pub clearing_id: Option<LandscapeFeatureId>,
    pub clearing_cause: Option<ClearingCause>,
    pub clearing_core: f32,
    pub clearing_shoulder: f32,
    pub openness: f32,
    pub forest_core: f32,
    pub wetland: f32,
    pub corridor: f32,
    pub corridor_id: Option<LandscapeFeatureId>,
    pub corridor_kind: Option<HabitatRouteKind>,
    pub local_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandscapePlanSample {
    pub world_x: i32,
    pub world_z: i32,
    pub detail: LandscapePlanDetail,
    pub land_weight: f32,
    pub inland_distance_blocks: f32,
    pub continent: Option<ContinentalDistrictSample>,
    pub province: Option<ProvincePlanSample>,
    pub ecoregion: Option<EcoregionPlanSample>,
    pub mosaic: Option<LandscapeMosaicSample>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanConstructionCounts {
    pub requested_samples: u64,
    pub continental_owner_evaluations: u64,
    pub province_owner_evaluations: u64,
    pub ecoregion_owner_evaluations: u64,
    pub mosaic_owner_evaluations: u64,
    pub local_field_evaluations: u64,
    pub exact_chunks: u64,
}

impl PlanConstructionCounts {
    fn add_assign(&mut self, other: Self) {
        self.requested_samples += other.requested_samples;
        self.continental_owner_evaluations += other.continental_owner_evaluations;
        self.province_owner_evaluations += other.province_owner_evaluations;
        self.ecoregion_owner_evaluations += other.ecoregion_owner_evaluations;
        self.mosaic_owner_evaluations += other.mosaic_owner_evaluations;
        self.local_field_evaluations += other.local_field_evaluations;
        self.exact_chunks += other.exact_chunks;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandscapePointQuery {
    pub sample: LandscapePlanSample,
    pub work: PlanConstructionCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandscapeWindowRequest {
    pub min_x: i32,
    pub min_z: i32,
    pub width_samples: u32,
    pub depth_samples: u32,
    pub step_blocks: u32,
    pub detail: LandscapePlanDetail,
}

impl LandscapeWindowRequest {
    pub const fn new(
        min_x: i32,
        min_z: i32,
        width_samples: u32,
        depth_samples: u32,
        step_blocks: u32,
        detail: LandscapePlanDetail,
    ) -> Self {
        Self {
            min_x,
            min_z,
            width_samples,
            depth_samples,
            step_blocks,
            detail,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandscapePlanWindow {
    pub descriptor: ContinentalEcoregionDescriptor,
    pub request: LandscapeWindowRequest,
    pub samples: Vec<LandscapePlanSample>,
    pub work: PlanConstructionCounts,
    pub semantic_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinentalEcoregionError {
    UnsupportedTopology(PlanTopologyCompatibility),
    InvalidWindow(&'static str),
    CoordinateOverflow,
}

impl fmt::Display for ContinentalEcoregionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTopology(compatibility) => write!(
                formatter,
                "unsupported continental/ecoregion topology: {}",
                compatibility.reason
            ),
            Self::InvalidWindow(reason) => write!(formatter, "invalid plan window: {reason}"),
            Self::CoordinateOverflow => formatter.write_str("plan window coordinate overflow"),
        }
    }
}

impl std::error::Error for ContinentalEcoregionError {}

#[derive(Clone, Copy, Debug)]
struct PlanFields {
    continent_edge: GradientNoise2d,
    moisture: GradientNoise2d,
    temperature: GradientNoise2d,
    corridor_warp: GradientNoise2d,
    local_openness: GradientNoise2d,
    owner_warp_x: GradientNoise2d,
    owner_warp_z: GradientNoise2d,
}

impl PlanFields {
    fn new(descriptor: ContinentalEcoregionDescriptor) -> Self {
        let gradient_noise = |domain, scale| match descriptor.topology {
            ContinentalEcoregionTopology::Plane => {
                GradientNoise2d::new(descriptor.seed, domain, scale)
            }
            ContinentalEcoregionTopology::CylinderX { period_blocks } => {
                GradientNoise2d::new_periodic_x(descriptor.seed, domain, scale, period_blocks)
            }
        };
        Self {
            continent_edge: gradient_noise(CONTINENT_EDGE_DOMAIN, 8_192),
            moisture: gradient_noise(CLIMATE_MOISTURE_DOMAIN, 24_576),
            temperature: gradient_noise(CLIMATE_TEMPERATURE_DOMAIN, 32_768),
            corridor_warp: gradient_noise(CORRIDOR_WARP_DOMAIN, 8_192),
            local_openness: gradient_noise(LOCAL_OPENNESS_DOMAIN, 1_024),
            owner_warp_x: gradient_noise(OWNER_WARP_X_DOMAIN, 12_288),
            owner_warp_z: gradient_noise(OWNER_WARP_Z_DOMAIN, 12_288),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContinentalEcoregionPlan {
    descriptor: ContinentalEcoregionDescriptor,
    fields: PlanFields,
}

impl ContinentalEcoregionPlan {
    pub fn new(
        descriptor: ContinentalEcoregionDescriptor,
    ) -> Result<Self, ContinentalEcoregionError> {
        let compatibility = descriptor.topology.compatibility();
        if !compatibility.supported {
            return Err(ContinentalEcoregionError::UnsupportedTopology(
                compatibility,
            ));
        }
        Ok(Self {
            descriptor,
            fields: PlanFields::new(descriptor),
        })
    }

    pub const fn descriptor(&self) -> ContinentalEcoregionDescriptor {
        self.descriptor
    }

    pub fn query_point(
        &self,
        detail: LandscapePlanDetail,
        world_x: i32,
        world_z: i32,
    ) -> LandscapePointQuery {
        let mut work = PlanConstructionCounts {
            requested_samples: 1,
            ..PlanConstructionCounts::default()
        };
        let land = self.continental_sample(world_x, world_z, &mut work);
        let continent = (land.land_weight >= 0.5).then_some(land.sample);
        let mut sample = LandscapePlanSample {
            world_x,
            world_z,
            detail,
            land_weight: land.land_weight,
            inland_distance_blocks: land.inland_distance_blocks,
            continent,
            province: None,
            ecoregion: None,
            mosaic: None,
        };

        let Some(continent) = continent else {
            return LandscapePointQuery { sample, work };
        };
        if detail == LandscapePlanDetail::Continental {
            return LandscapePointQuery { sample, work };
        }

        let route = self.habitat_route_sample(world_x, world_z, continent);
        let province = self.province_sample(world_x, world_z, continent, route, &mut work);
        sample.province = Some(province);
        if detail == LandscapePlanDetail::Province {
            return LandscapePointQuery { sample, work };
        }

        let ecoregion = self.ecoregion_sample(world_x, world_z, province, &mut work);
        sample.ecoregion = Some(ecoregion);
        if detail == LandscapePlanDetail::Ecoregion {
            return LandscapePointQuery { sample, work };
        }

        sample.mosaic =
            Some(self.mosaic_sample(world_x, world_z, province, ecoregion, route, &mut work));
        LandscapePointQuery { sample, work }
    }

    pub fn query_window(
        &self,
        request: LandscapeWindowRequest,
    ) -> Result<LandscapePlanWindow, ContinentalEcoregionError> {
        validate_window(request)?;
        let sample_count = (request.width_samples as usize)
            .checked_mul(request.depth_samples as usize)
            .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
        let mut samples = Vec::with_capacity(sample_count);
        let mut work = PlanConstructionCounts::default();
        for sample_z in 0..request.depth_samples {
            let world_z = window_coordinate(request.min_z, sample_z, request.step_blocks)?;
            for sample_x in 0..request.width_samples {
                let world_x = window_coordinate(request.min_x, sample_x, request.step_blocks)?;
                let query = self.query_point(request.detail, world_x, world_z);
                work.add_assign(query.work);
                samples.push(query.sample);
            }
        }
        let semantic_sha256 = semantic_sha256(self.descriptor, request, &samples, work);
        Ok(LandscapePlanWindow {
            descriptor: self.descriptor,
            request,
            samples,
            work,
            semantic_sha256,
        })
    }

    fn continental_sample(
        &self,
        world_x: i32,
        world_z: i32,
        work: &mut PlanConstructionCounts,
    ) -> ContinentalInternalSample {
        let canonical_x = self.descriptor.topology.canonical_world_x(world_x);
        let base_owner_x = canonical_x.div_euclid(CONTINENTAL_CELL_BLOCKS);
        let base_owner_z = world_z.div_euclid(CONTINENTAL_CELL_BLOCKS);
        let mut best: Option<(f64, ContinentalSite)> = None;
        let mut second: Option<(f64, ContinentalSite)> = None;

        for offset_z in -CONTINENT_OWNER_RADIUS..=CONTINENT_OWNER_RADIUS {
            for offset_x in -CONTINENT_OWNER_RADIUS..=CONTINENT_OWNER_RADIUS {
                work.continental_owner_evaluations += 1;
                let raw_owner_x = base_owner_x + offset_x;
                let owner_z = base_owner_z + offset_z;
                let site = self.continental_site(raw_owner_x, owner_z);
                let dx = (i64::from(canonical_x) - site.center_x) as f64;
                let dz = (i64::from(world_z) - site.center_z) as f64;
                let along = dx * site.axis_x + dz * site.axis_z;
                let across = -dx * site.axis_z + dz * site.axis_x;
                let distance = ((along / site.radius_along).powi(2)
                    + (across / site.radius_across).powi(2))
                .sqrt();
                let edge_warp = self.fields.continent_edge.sample(canonical_x, world_z) * 0.24;
                let score = if site.active {
                    1.0 - distance + edge_warp
                } else {
                    -1.25 - distance * 0.08
                };
                let replace = best.as_ref().is_none_or(|(best_score, best_site)| {
                    score > *best_score
                        || (score == *best_score && site.id.hash < best_site.id.hash)
                });
                if replace {
                    second = best;
                    best = Some((score, site));
                } else if second.as_ref().is_none_or(|(second_score, second_site)| {
                    score > *second_score
                        || (score == *second_score && site.id.hash < second_site.id.hash)
                }) {
                    second = Some((score, site));
                }
            }
        }

        let (score, site) = best.expect("continental owner neighborhood is non-empty");
        let land_weight = smoothstep(-0.08, 0.10, score) as f32;
        let (prevailing_wind_x, prevailing_wind_z) = prevailing_wind(site);
        let point_shadow = continental_rain_shadow(site, canonical_x, world_z);
        let rain_shadow_potential = second.map_or(point_shadow, |(second_score, second_site)| {
            let second_shadow = continental_rain_shadow(second_site, canonical_x, world_z);
            let owner_interior = smoothstep(0.0, 0.18, score - second_score);
            lerp_f64(second_shadow, point_shadow, 0.5 + owner_interior * 0.5)
        });
        let core_weight = second.map_or(1.0, |(second_score, _)| {
            smoothstep(0.0, 0.18, score - second_score)
        });
        ContinentalInternalSample {
            sample: ContinentalDistrictSample {
                id: site.id,
                story: site.story,
                center_x: site.center_x,
                center_z: site.center_z,
                axis_x: site.axis_x as f32,
                axis_z: site.axis_z as f32,
                prevailing_wind_x: prevailing_wind_x as f32,
                prevailing_wind_z: prevailing_wind_z as f32,
                rain_shadow_potential: rain_shadow_potential as f32,
                core_weight: core_weight as f32,
            },
            land_weight,
            inland_distance_blocks: (score * site.radius_along.min(site.radius_across)) as f32,
        }
    }

    fn continental_site(&self, raw_owner_x: i32, owner_z: i32) -> ContinentalSite {
        let owner_x = self
            .descriptor
            .topology
            .canonical_owner_x(raw_owner_x, CONTINENTAL_CELL_BLOCKS);
        let hash = coordinate_hash(
            self.descriptor.seed,
            CONTINENT_HASH_DOMAIN,
            owner_x,
            owner_z,
            0,
        );
        let center_x = i64::from(raw_owner_x) * i64::from(CONTINENTAL_CELL_BLOCKS)
            + i64::from(CONTINENTAL_CELL_BLOCKS / 2)
            + i64::from(signed_hash_offset(hash, 8, 4_800));
        let center_z = i64::from(owner_z) * i64::from(CONTINENTAL_CELL_BLOCKS)
            + i64::from(CONTINENTAL_CELL_BLOCKS / 2)
            + i64::from(signed_hash_offset(hash, 24, 4_800));
        let (axis_x, axis_z) = direction(hash, 40);
        ContinentalSite {
            id: feature_id(
                LandscapeFeatureFamily::ContinentalDistrict,
                owner_x,
                owner_z,
                0,
                hash,
            ),
            center_x,
            center_z,
            radius_along: 27_000.0 + hash_unit(hash, 48) * 17_000.0,
            radius_across: 22_000.0 + hash_unit(hash, 12) * 16_000.0,
            axis_x,
            axis_z,
            active: (hash & 7) < 3,
            story: match (hash >> 60) & 3 {
                0 => ContinentalStory::RiverValley,
                1 => ContinentalStory::LakeDistrict,
                2 => ContinentalStory::Escarpment,
                _ => ContinentalStory::OpenHighland,
            },
        }
    }

    fn province_sample(
        &self,
        world_x: i32,
        world_z: i32,
        continent: ContinentalDistrictSample,
        route: HabitatRouteSample,
        work: &mut PlanConstructionCounts,
    ) -> ProvincePlanSample {
        let site = self.nearest_site(
            world_x,
            world_z,
            PROVINCE_CELL_BLOCKS,
            PROVINCE_HASH_DOMAIN,
            work,
            OwnerLevel::Province,
        );
        let id_hash = stable_mix64(site.hash ^ continent.id.hash.rotate_left(19));
        let id = feature_id(
            LandscapeFeatureFamily::PhysiographicProvince,
            site.owner_x,
            site.owner_z,
            0,
            id_hash,
        );
        let dx = (site.center_x - continent.center_x) as f64;
        let dz = (site.center_z - continent.center_z) as f64;
        let axis_x = f64::from(continent.axis_x);
        let axis_z = f64::from(continent.axis_z);
        let along = (dx * axis_x + dz * axis_z) / 32_000.0;
        let across = (-dx * axis_z + dz * axis_x) / 24_000.0;
        let kind = province_kind(continent.story, along, across, id_hash);
        let peer_id_hash = stable_mix64(site.second_hash ^ continent.id.hash.rotate_left(19));
        let peer_dx = (site.second_center_x - continent.center_x) as f64;
        let peer_dz = (site.second_center_z - continent.center_z) as f64;
        let peer_along = (peer_dx * axis_x + peer_dz * axis_z) / 32_000.0;
        let peer_across = (-peer_dx * axis_z + peer_dz * axis_x) / 24_000.0;
        let peer_kind = province_kind(continent.story, peer_along, peer_across, peer_id_hash);
        // Region identity is intentionally discrete. Scalar terrain and
        // ecology facts are symmetric blends with the nearest peer so an
        // ownership bisector cannot become a visible content seam.
        let owner_blend = 0.5 + site.core_weight * 0.5;
        let major_water = lerp_f64(
            province_major_water(peer_kind, route.weight),
            province_major_water(kind, route.weight),
            owner_blend,
        );
        let relief = lerp_f32(
            province_relief(peer_kind),
            province_relief(kind),
            owner_blend as f32,
        );
        let leeward_exposure = lerp_f64(
            province_leeward_exposure(continent, peer_kind, peer_id_hash),
            province_leeward_exposure(continent, kind, id_hash),
            owner_blend,
        );
        let continent_identity = smoothstep(0.0, 0.42, f64::from(continent.core_weight));
        let major_water = lerp_f64(route.weight * 0.65, major_water, continent_identity);
        let relief = lerp_f32(0.40, relief, continent_identity as f32);
        let leeward_exposure = lerp_f64(
            f64::from(continent.rain_shadow_potential) * 0.85,
            leeward_exposure,
            continent_identity,
        );
        ProvincePlanSample {
            id,
            continent_id: continent.id,
            kind,
            core_weight: (site.core_weight as f32).min(continent.core_weight),
            relief,
            major_water: major_water as f32,
            leeward_exposure: leeward_exposure as f32,
        }
    }

    fn ecoregion_sample(
        &self,
        world_x: i32,
        world_z: i32,
        province: ProvincePlanSample,
        work: &mut PlanConstructionCounts,
    ) -> EcoregionPlanSample {
        let site = self.nearest_site(
            world_x,
            world_z,
            ECOREGION_CELL_BLOCKS,
            ECOREGION_HASH_DOMAIN,
            work,
            OwnerLevel::Ecoregion,
        );
        let id_hash = stable_mix64(site.hash ^ province.id.hash.rotate_left(23));
        let id = feature_id(
            LandscapeFeatureFamily::Ecoregion,
            site.owner_x,
            site.owner_z,
            0,
            id_hash,
        );
        work.local_field_evaluations += 4;
        let (owner_moisture, owner_temperature) =
            self.ecoregion_climate(site.owner_x, site.owner_z);
        let owner_aridity = ecoregion_aridity(province, owner_moisture, owner_temperature);
        let kind = ecoregion_kind(
            province.kind,
            owner_moisture,
            owner_temperature,
            owner_aridity,
            id_hash,
        );
        let peer_id_hash = stable_mix64(site.second_hash ^ province.id.hash.rotate_left(23));
        let (peer_moisture, peer_temperature) =
            self.ecoregion_climate(site.second_owner_x, site.second_owner_z);
        let peer_aridity = ecoregion_aridity(province, peer_moisture, peer_temperature);
        let peer_kind = ecoregion_kind(
            province.kind,
            peer_moisture,
            peer_temperature,
            peer_aridity,
            peer_id_hash,
        );
        let transition_width_blocks = 4_096.0 + ecoregion_transition_width_blocks(kind, peer_kind);
        let transition_weight =
            (1.0 - site.boundary_distance_blocks / (transition_width_blocks * 0.5)).clamp(0.0, 1.0);
        let blend = transition_weight * 0.5;
        let (base_openness, base_canopy) = ecoregion_cover(kind);
        let (peer_openness, peer_canopy) = ecoregion_cover(peer_kind);
        // Climate is continuous within and across the authored ecoregion
        // ownership cells. Owner-center climate still selects each region's
        // character, while these point samples drive gradual surface response.
        let (moisture, temperature) = self.climate_at(world_x, world_z);
        let aridity = ecoregion_aridity(province, moisture, temperature);
        let drainage = drainage_permanence(province, moisture, aridity);
        let blended_openness = lerp_f32(base_openness, peer_openness, blend as f32);
        let blended_canopy = lerp_f32(base_canopy, peer_canopy, blend as f32);
        let (boundary_openness, boundary_canopy) = ecoregion_cover(EcoregionKind::QuietTransition);
        let province_identity = smoothstep(0.0, 0.42, f64::from(province.core_weight)) as f32;
        EcoregionPlanSample {
            id,
            province_id: province.id,
            kind,
            transition_peer_kind: (kind != peer_kind).then_some(peer_kind),
            core_weight: (1.0 - transition_weight) as f32,
            transition_weight: transition_weight as f32,
            transition_width_blocks: transition_width_blocks as f32,
            base_openness: lerp_f32(boundary_openness, blended_openness, province_identity),
            base_canopy: lerp_f32(boundary_canopy, blended_canopy, province_identity),
            moisture: moisture as f32,
            temperature: temperature as f32,
            aridity: aridity as f32,
            drainage_permanence: drainage as f32,
        }
    }

    fn ecoregion_climate(&self, owner_x: i32, owner_z: i32) -> (f64, f64) {
        let climate_x = owner_center_coordinate(owner_x, ECOREGION_CELL_BLOCKS);
        let climate_z = owner_center_coordinate(owner_z, ECOREGION_CELL_BLOCKS);
        self.climate_at(climate_x, climate_z)
    }

    fn climate_at(&self, world_x: i32, world_z: i32) -> (f64, f64) {
        let climate_x = self.descriptor.topology.canonical_world_x(world_x);
        let climate_z = world_z;
        let moisture = unit_field(self.fields.moisture.sample(climate_x, climate_z));
        let temperature =
            (0.5 + self.fields.temperature.sample(climate_x, climate_z) * 0.16).clamp(0.25, 0.75);
        (moisture, temperature)
    }

    fn mosaic_sample(
        &self,
        world_x: i32,
        world_z: i32,
        province: ProvincePlanSample,
        ecoregion: EcoregionPlanSample,
        route: HabitatRouteSample,
        work: &mut PlanConstructionCounts,
    ) -> LandscapeMosaicSample {
        let canonical_x = self.descriptor.topology.canonical_world_x(world_x);
        let base_owner_x = canonical_x.div_euclid(MOSAIC_CELL_BLOCKS);
        let base_owner_z = world_z.div_euclid(MOSAIC_CELL_BLOCKS);
        let mut best: Option<(f64, LandscapeFeatureId, ClearingCause)> = None;
        for offset_z in -LOCAL_OWNER_RADIUS..=LOCAL_OWNER_RADIUS {
            for offset_x in -LOCAL_OWNER_RADIUS..=LOCAL_OWNER_RADIUS {
                work.mosaic_owner_evaluations += 1;
                let raw_owner_x = base_owner_x + offset_x;
                let owner_z = base_owner_z + offset_z;
                let owner_x = self
                    .descriptor
                    .topology
                    .canonical_owner_x(raw_owner_x, MOSAIC_CELL_BLOCKS);
                let hash = coordinate_hash(
                    self.descriptor.seed,
                    MOSAIC_HASH_DOMAIN,
                    owner_x,
                    owner_z,
                    0,
                );
                if !clearing_enabled(hash) {
                    continue;
                }
                let center_x = i64::from(raw_owner_x) * i64::from(MOSAIC_CELL_BLOCKS)
                    + i64::from(MOSAIC_CELL_BLOCKS / 2)
                    + i64::from(signed_hash_offset(hash, 8, 700));
                let center_z = i64::from(owner_z) * i64::from(MOSAIC_CELL_BLOCKS)
                    + i64::from(MOSAIC_CELL_BLOCKS / 2)
                    + i64::from(signed_hash_offset(hash, 24, 700));
                let (axis_x, axis_z) = direction(hash, 40);
                let local_x = (i64::from(canonical_x) - center_x) as f64;
                let local_z = (i64::from(world_z) - center_z) as f64;
                let along = local_x * axis_x + local_z * axis_z;
                let across = -local_x * axis_z + local_z * axis_x;
                let radius_along = 1_400.0 + hash_unit(hash, 48) * 2_400.0;
                let radius_across = 900.0 + hash_unit(hash, 16) * 1_700.0;
                let distance =
                    ((along / radius_along).powi(2) + (across / radius_across).powi(2)).sqrt();
                let influence = (1.0 - distance).clamp(0.0, 1.0);
                let id_hash = stable_mix64(hash);
                let id = feature_id(
                    LandscapeFeatureFamily::Clearing,
                    owner_x,
                    owner_z,
                    0,
                    id_hash,
                );
                let cause = clearing_cause(ecoregion.kind, hash);
                let replace = best.as_ref().is_none_or(|(best_influence, best_id, _)| {
                    influence > *best_influence
                        || (influence == *best_influence && id.hash < best_id.hash)
                });
                if replace {
                    best = Some((influence, id, cause));
                }
            }
        }

        work.local_field_evaluations += 2;
        let local_variation = self.fields.local_openness.sample(canonical_x, world_z) * 0.08;
        let (clearing_influence, clearing_id, clearing_cause) = best
            .filter(|(influence, _, _)| *influence > 0.0)
            .map_or((0.0, None, None), |(influence, id, cause)| {
                (influence, Some(id), Some(cause))
            });
        let clearing_core = smoothstep(0.35, 0.82, clearing_influence);
        let clearing_shoulder =
            (smoothstep(0.02, 0.55, clearing_influence) - clearing_core).max(0.0);
        let mut openness =
            (f64::from(ecoregion.base_openness) + clearing_influence * 0.72 + local_variation)
                .clamp(0.0, 1.0);
        let mut forest_core = (f64::from(ecoregion.base_canopy)
            * (1.0 - clearing_influence)
            * (0.84 + f64::from(ecoregion.core_weight) * 0.16))
            .clamp(0.0, 1.0);
        let corridor = (route.weight
            * habitat_route_context(
                route.kind,
                ecoregion,
                province,
                clearing_influence,
                local_variation,
            ))
        .clamp(0.0, 1.0);
        match route.kind {
            HabitatRouteKind::WoodlandPass => {
                forest_core = forest_core.max(corridor * (0.82 - clearing_influence * 0.38));
            }
            HabitatRouteKind::RiparianSpine => {
                forest_core = forest_core.max(corridor * 0.42);
            }
            HabitatRouteKind::OpenRangeLink => {
                openness = openness.max(corridor * 0.88);
            }
            HabitatRouteKind::WetlandChain => {}
        }
        let arid_cover = smoothstep(0.48, 0.84, f64::from(ecoregion.aridity));
        openness = openness.max(arid_cover * 0.92);
        forest_core *= 1.0 - arid_cover * 0.92;
        let owner_wetland_affinity = ecoregion_wetland_affinity(ecoregion.kind);
        let peer_wetland_affinity = ecoregion
            .transition_peer_kind
            .map_or(owner_wetland_affinity, ecoregion_wetland_affinity);
        let ecoregion_blend = f64::from(ecoregion.transition_weight) * 0.5;
        let regional_wetland_affinity = lerp_f64(
            owner_wetland_affinity,
            peer_wetland_affinity,
            ecoregion_blend,
        );
        let province_identity = smoothstep(0.0, 0.42, f64::from(province.core_weight));
        let wetland_affinity = lerp_f64(0.22, regional_wetland_affinity, province_identity);
        let wetland = (wetland_affinity
            * f64::from(ecoregion.moisture)
            * f64::from(ecoregion.drainage_permanence)
            * f64::from(province.major_water.max(corridor as f32)))
        .clamp(0.0, 1.0);
        let wetland = match route.kind {
            HabitatRouteKind::WetlandChain => wetland.max(
                corridor
                    * (0.55 + f64::from(ecoregion.moisture) * 0.45)
                    * (1.0 - clearing_influence * 0.22),
            ),
            HabitatRouteKind::RiparianSpine => wetland.max(corridor * 0.48),
            _ => wetland,
        };
        let local_cell_x = canonical_x.div_euclid(256);
        let local_cell_z = world_z.div_euclid(256);
        let local_fingerprint = coordinate_hash(
            self.descriptor.seed,
            LOCAL_HASH_DOMAIN,
            local_cell_x,
            local_cell_z,
            0,
        );

        LandscapeMosaicSample {
            clearing_id,
            clearing_cause,
            clearing_core: clearing_core as f32,
            clearing_shoulder: clearing_shoulder as f32,
            openness: openness as f32,
            forest_core: forest_core as f32,
            wetland: wetland as f32,
            corridor: corridor as f32,
            corridor_id: (corridor > 0.0).then_some(route.id),
            corridor_kind: (corridor > 0.0).then_some(route.kind),
            local_fingerprint,
        }
    }

    fn nearest_site(
        &self,
        world_x: i32,
        world_z: i32,
        scale: i32,
        domain: u64,
        work: &mut PlanConstructionCounts,
        level: OwnerLevel,
    ) -> NearestSite {
        let canonical_x = self.descriptor.topology.canonical_world_x(world_x);
        work.local_field_evaluations += 2;
        let warp_amplitude = f64::from(scale) * 0.18;
        let query_x = i64::from(canonical_x)
            + (self.fields.owner_warp_x.sample(canonical_x, world_z) * warp_amplitude).round()
                as i64;
        let query_z = i64::from(world_z)
            + (self.fields.owner_warp_z.sample(canonical_x, world_z) * warp_amplitude).round()
                as i64;
        let base_owner_x = canonical_x.div_euclid(scale);
        let base_owner_z = world_z.div_euclid(scale);
        let mut nearest: Option<(f64, NearestSite)> = None;
        let mut second: Option<(f64, NearestSite)> = None;
        for offset_z in -LOCAL_OWNER_RADIUS..=LOCAL_OWNER_RADIUS {
            for offset_x in -LOCAL_OWNER_RADIUS..=LOCAL_OWNER_RADIUS {
                match level {
                    OwnerLevel::Province => work.province_owner_evaluations += 1,
                    OwnerLevel::Ecoregion => work.ecoregion_owner_evaluations += 1,
                }
                let raw_owner_x = base_owner_x + offset_x;
                let owner_z = base_owner_z + offset_z;
                let owner_x = self
                    .descriptor
                    .topology
                    .canonical_owner_x(raw_owner_x, scale);
                let hash = coordinate_hash(self.descriptor.seed, domain, owner_x, owner_z, 0);
                let jitter = scale * 7 / 25;
                let center_x = i64::from(raw_owner_x) * i64::from(scale)
                    + i64::from(scale / 2)
                    + i64::from(signed_hash_offset(hash, 8, jitter));
                let center_z = i64::from(owner_z) * i64::from(scale)
                    + i64::from(scale / 2)
                    + i64::from(signed_hash_offset(hash, 24, jitter));
                let distance = ((query_x - center_x) as f64).hypot((query_z - center_z) as f64);
                let site = NearestSite {
                    owner_x,
                    owner_z,
                    center_x,
                    center_z,
                    hash,
                    core_weight: 0.0,
                    boundary_distance_blocks: 0.0,
                    second_owner_x: 0,
                    second_owner_z: 0,
                    second_center_x: 0,
                    second_center_z: 0,
                    second_hash: 0,
                };
                if nearest.is_none_or(|(nearest_distance, nearest_site)| {
                    distance < nearest_distance
                        || (distance == nearest_distance && hash < nearest_site.hash)
                }) {
                    second = nearest;
                    nearest = Some((distance, site));
                } else if second.is_none_or(|(second_distance, second_site)| {
                    distance < second_distance
                        || (distance == second_distance && hash < second_site.hash)
                }) {
                    second = Some((distance, site));
                }
            }
        }
        let (nearest_distance, mut site) = nearest.expect("site neighborhood is non-empty");
        let (second_distance, second_site) = second.expect("site neighborhood has a peer");
        site.core_weight =
            ((second_distance - nearest_distance) / (f64::from(scale) * 0.48)).clamp(0.0, 1.0);
        site.boundary_distance_blocks = (second_distance - nearest_distance) * 0.5;
        site.second_owner_x = second_site.owner_x;
        site.second_owner_z = second_site.owner_z;
        site.second_center_x = second_site.center_x;
        site.second_center_z = second_site.center_z;
        site.second_hash = second_site.hash;
        site
    }

    fn habitat_route_sample(
        &self,
        world_x: i32,
        world_z: i32,
        continent: ContinentalDistrictSample,
    ) -> HabitatRouteSample {
        let canonical_x = self.descriptor.topology.canonical_world_x(world_x);
        let dx = (i64::from(canonical_x) - continent.center_x) as f64;
        let dz = (i64::from(world_z) - continent.center_z) as f64;
        let along = dx * f64::from(continent.axis_x) + dz * f64::from(continent.axis_z);
        let across = -dx * f64::from(continent.axis_z) + dz * f64::from(continent.axis_x);
        let warp = self.fields.corridor_warp.sample(canonical_x, world_z);
        let position = RoutePoint { along, across };
        let main_curve = main_habitat_route_curve(continent);
        let main_kind = main_habitat_route_kind(continent.story);
        let main_id = habitat_route_id(continent, 0);
        let main_distance = cubic_route_distance(
            warped_route_position(position, warp, main_id.hash),
            main_curve,
        );
        let mut best = HabitatRouteSample {
            id: main_id,
            kind: main_kind,
            weight: habitat_route_weight(main_distance, main_kind, warp, main_id.hash),
        };

        for slot in 1..=3 {
            let kind = branch_habitat_route_kind(continent.story, slot);
            let id = habitat_route_id(continent, slot);
            let branch_curve = branch_habitat_route_curve(continent, slot, main_curve);
            let branch_distance = quadratic_route_distance(
                warped_route_position(position, warp, id.hash),
                branch_curve,
            );
            let weight = habitat_route_weight(branch_distance, kind, warp, id.hash) * 0.94;
            if weight > best.weight || (weight == best.weight && id.hash < best.id.hash) {
                best = HabitatRouteSample { id, kind, weight };
            }
        }
        best
    }
}

#[derive(Clone, Copy, Debug)]
enum OwnerLevel {
    Province,
    Ecoregion,
}

#[derive(Clone, Copy, Debug)]
struct ContinentalInternalSample {
    sample: ContinentalDistrictSample,
    land_weight: f32,
    inland_distance_blocks: f32,
}

#[derive(Clone, Copy, Debug)]
struct HabitatRouteSample {
    id: LandscapeFeatureId,
    kind: HabitatRouteKind,
    weight: f64,
}

#[derive(Clone, Copy, Debug)]
struct RoutePoint {
    along: f64,
    across: f64,
}

#[derive(Clone, Copy, Debug)]
struct ContinentalSite {
    id: LandscapeFeatureId,
    center_x: i64,
    center_z: i64,
    radius_along: f64,
    radius_across: f64,
    axis_x: f64,
    axis_z: f64,
    active: bool,
    story: ContinentalStory,
}

#[derive(Clone, Copy, Debug)]
struct NearestSite {
    owner_x: i32,
    owner_z: i32,
    center_x: i64,
    center_z: i64,
    hash: u64,
    core_weight: f64,
    boundary_distance_blocks: f64,
    second_owner_x: i32,
    second_owner_z: i32,
    second_center_x: i64,
    second_center_z: i64,
    second_hash: u64,
}

fn validate_window(request: LandscapeWindowRequest) -> Result<(), ContinentalEcoregionError> {
    if request.width_samples == 0 || request.depth_samples == 0 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "sample dimensions must be non-zero",
        ));
    }
    if request.step_blocks == 0 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "sample step must be non-zero",
        ));
    }
    let sample_count = (request.width_samples as usize)
        .checked_mul(request.depth_samples as usize)
        .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
    if sample_count > MAX_WINDOW_SAMPLES {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "sample count exceeds the bounded atlas cap",
        ));
    }
    Ok(())
}

fn window_coordinate(
    minimum: i32,
    sample: u32,
    step_blocks: u32,
) -> Result<i32, ContinentalEcoregionError> {
    let offset = i64::from(sample) * i64::from(step_blocks);
    i32::try_from(i64::from(minimum) + offset)
        .map_err(|_| ContinentalEcoregionError::CoordinateOverflow)
}

fn province_kind(
    story: ContinentalStory,
    along: f64,
    across: f64,
    hash: u64,
) -> PhysiographicProvinceKind {
    let alternate = (hash >> 57) & 3;
    match story {
        ContinentalStory::RiverValley if across.abs() < 0.23 => {
            PhysiographicProvinceKind::RiverLowland
        }
        ContinentalStory::RiverValley if across.abs() > 0.78 => {
            PhysiographicProvinceKind::RockyRidge
        }
        ContinentalStory::RiverValley if alternate == 0 => PhysiographicProvinceKind::QuietBench,
        ContinentalStory::RiverValley => PhysiographicProvinceKind::WoodedUpland,
        ContinentalStory::LakeDistrict if along.hypot(across) < 0.42 => {
            PhysiographicProvinceKind::LakeBasin
        }
        ContinentalStory::LakeDistrict if along.hypot(across) > 0.88 => {
            PhysiographicProvinceKind::WoodedUpland
        }
        ContinentalStory::LakeDistrict => PhysiographicProvinceKind::RollingHills,
        ContinentalStory::Escarpment if across > 0.28 => PhysiographicProvinceKind::RockyRidge,
        ContinentalStory::Escarpment if across < -0.45 => PhysiographicProvinceKind::RiverLowland,
        ContinentalStory::Escarpment => PhysiographicProvinceKind::RollingHills,
        ContinentalStory::OpenHighland if along.abs() < 0.35 => {
            PhysiographicProvinceKind::RollingHills
        }
        ContinentalStory::OpenHighland if alternate == 0 => PhysiographicProvinceKind::QuietBench,
        ContinentalStory::OpenHighland => PhysiographicProvinceKind::WoodedUpland,
    }
}

fn province_relief(kind: PhysiographicProvinceKind) -> f32 {
    match kind {
        PhysiographicProvinceKind::RiverLowland => 0.16,
        PhysiographicProvinceKind::LakeBasin => 0.22,
        PhysiographicProvinceKind::RollingHills => 0.48,
        PhysiographicProvinceKind::WoodedUpland => 0.62,
        PhysiographicProvinceKind::RockyRidge => 0.92,
        PhysiographicProvinceKind::QuietBench => 0.32,
    }
}

fn province_major_water(kind: PhysiographicProvinceKind, route_weight: f64) -> f64 {
    match kind {
        PhysiographicProvinceKind::RiverLowland => route_weight.max(0.45),
        PhysiographicProvinceKind::LakeBasin => (0.55 + route_weight * 0.35).min(1.0),
        _ => route_weight * 0.65,
    }
}

fn prevailing_wind(site: ContinentalSite) -> (f64, f64) {
    if site.story == ContinentalStory::Escarpment {
        // Moist air crosses the high side of the escarpment before continuing
        // toward the low basin on the negative-across side.
        (site.axis_z, -site.axis_x)
    } else {
        direction(site.id.hash, 18)
    }
}

fn continental_rain_shadow(site: ContinentalSite, world_x: i32, world_z: i32) -> f64 {
    if !site.active {
        return 0.0;
    }
    let (wind_x, wind_z) = prevailing_wind(site);
    let dx = (i64::from(world_x) - site.center_x) as f64;
    let dz = (i64::from(world_z) - site.center_z) as f64;
    let downwind = (dx * wind_x + dz * wind_z) / 28_000.0;
    let story_strength = match site.story {
        ContinentalStory::Escarpment => 1.0,
        ContinentalStory::OpenHighland => 0.66,
        ContinentalStory::RiverValley => 0.38,
        ContinentalStory::LakeDistrict => 0.30,
    };
    smoothstep(-0.08, 0.72, downwind) * story_strength
}

fn province_leeward_exposure(
    continent: ContinentalDistrictSample,
    kind: PhysiographicProvinceKind,
    hash: u64,
) -> f64 {
    let province_affinity = match kind {
        PhysiographicProvinceKind::RiverLowland => 1.0,
        PhysiographicProvinceKind::QuietBench => 0.96,
        PhysiographicProvinceKind::RollingHills => 0.90,
        PhysiographicProvinceKind::WoodedUpland => 0.82,
        PhysiographicProvinceKind::LakeBasin => 0.76,
        PhysiographicProvinceKind::RockyRidge => 0.72,
    };
    let authored_variation = 0.90 + hash_unit(hash, 39) * 0.10;
    (f64::from(continent.rain_shadow_potential) * province_affinity * authored_variation)
        .clamp(0.0, 1.0)
}

fn ecoregion_aridity(province: ProvincePlanSample, moisture: f64, temperature: f64) -> f64 {
    let atmospheric_dryness = 1.0 - moisture;
    let warm_evaporation = smoothstep(0.44, 0.72, temperature);
    (atmospheric_dryness * 0.56
        + f64::from(province.leeward_exposure) * 0.62
        + warm_evaporation * 0.12
        - f64::from(province.major_water) * 0.28)
        .clamp(0.0, 1.0)
}

fn drainage_permanence(province: ProvincePlanSample, moisture: f64, aridity: f64) -> f64 {
    (moisture * 0.52 + f64::from(province.major_water) * 0.38 + (1.0 - aridity) * 0.16
        - f64::from(province.leeward_exposure) * 0.24)
        .clamp(0.0, 1.0)
}

fn ecoregion_kind(
    kind: PhysiographicProvinceKind,
    moisture: f64,
    temperature: f64,
    aridity: f64,
    hash: u64,
) -> EcoregionKind {
    let alternate = (hash >> 57) & 3;
    if aridity > 0.72 {
        return if matches!(
            kind,
            PhysiographicProvinceKind::RockyRidge | PhysiographicProvinceKind::WoodedUpland
        ) {
            EcoregionKind::ExposedUpland
        } else {
            EcoregionKind::BroadMeadow
        };
    }
    match kind {
        PhysiographicProvinceKind::RiverLowland if moisture > 0.60 => {
            EcoregionKind::ConnectedWetland
        }
        PhysiographicProvinceKind::RiverLowland if moisture < 0.34 => EcoregionKind::BroadMeadow,
        PhysiographicProvinceKind::RiverLowland => EcoregionKind::RiparianWoodland,
        PhysiographicProvinceKind::LakeBasin if moisture > 0.46 => EcoregionKind::ConnectedWetland,
        PhysiographicProvinceKind::LakeBasin if alternate == 0 => EcoregionKind::QuietTransition,
        PhysiographicProvinceKind::LakeBasin => EcoregionKind::RiparianWoodland,
        PhysiographicProvinceKind::RollingHills if moisture < 0.36 => EcoregionKind::BroadMeadow,
        PhysiographicProvinceKind::RollingHills if moisture > 0.68 => EcoregionKind::OldForestCore,
        PhysiographicProvinceKind::RollingHills if alternate <= 1 => EcoregionKind::MixedWoodland,
        PhysiographicProvinceKind::RollingHills => EcoregionKind::QuietTransition,
        PhysiographicProvinceKind::WoodedUpland if moisture > 0.58 => EcoregionKind::OldForestCore,
        PhysiographicProvinceKind::WoodedUpland if moisture < 0.32 && temperature > 0.48 => {
            EcoregionKind::BroadMeadow
        }
        PhysiographicProvinceKind::WoodedUpland => EcoregionKind::MixedWoodland,
        PhysiographicProvinceKind::RockyRidge if moisture > 0.66 && alternate == 0 => {
            EcoregionKind::OldForestCore
        }
        PhysiographicProvinceKind::RockyRidge => EcoregionKind::ExposedUpland,
        PhysiographicProvinceKind::QuietBench if moisture < 0.38 => EcoregionKind::BroadMeadow,
        PhysiographicProvinceKind::QuietBench if moisture > 0.68 => EcoregionKind::OldForestCore,
        PhysiographicProvinceKind::QuietBench if alternate == 0 => EcoregionKind::MixedWoodland,
        PhysiographicProvinceKind::QuietBench => EcoregionKind::QuietTransition,
    }
}

fn ecoregion_cover(kind: EcoregionKind) -> (f32, f32) {
    match kind {
        EcoregionKind::OldForestCore => (0.10, 0.94),
        EcoregionKind::BroadMeadow => (0.88, 0.10),
        EcoregionKind::RiparianWoodland => (0.34, 0.76),
        EcoregionKind::ConnectedWetland => (0.72, 0.20),
        EcoregionKind::MixedWoodland => (0.44, 0.68),
        EcoregionKind::ExposedUpland => (0.80, 0.18),
        EcoregionKind::QuietTransition => (0.54, 0.50),
    }
}

fn ecoregion_wetland_affinity(kind: EcoregionKind) -> f64 {
    match kind {
        EcoregionKind::ConnectedWetland => 1.0,
        EcoregionKind::RiparianWoodland => 0.72,
        _ => 0.22,
    }
}

fn ecoregion_transition_width_blocks(left: EcoregionKind, right: EcoregionKind) -> f64 {
    use EcoregionKind::{
        BroadMeadow, ConnectedWetland, ExposedUpland, MixedWoodland, OldForestCore,
        QuietTransition, RiparianWoodland,
    };
    if left == right {
        return 0.0;
    }
    let pair = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    match pair {
        (OldForestCore, BroadMeadow) => 800.0,
        (BroadMeadow, MixedWoodland) | (OldForestCore, MixedWoodland) => 1_100.0,
        (OldForestCore, RiparianWoodland) | (RiparianWoodland, MixedWoodland) => 1_400.0,
        (BroadMeadow, ConnectedWetland)
        | (OldForestCore, ConnectedWetland)
        | (ConnectedWetland, MixedWoodland)
        | (RiparianWoodland, ConnectedWetland) => 2_600.0,
        (BroadMeadow, ExposedUpland) => 1_600.0,
        (MixedWoodland, ExposedUpland)
        | (OldForestCore, ExposedUpland)
        | (RiparianWoodland, ExposedUpland) => 2_200.0,
        (_, QuietTransition) => 1_800.0,
        _ => 1_400.0,
    }
}

fn lerp_f32(left: f32, right: f32, amount: f32) -> f32 {
    left + (right - left) * amount
}

fn lerp_f64(left: f64, right: f64, amount: f64) -> f64 {
    left + (right - left) * amount
}

fn clearing_enabled(hash: u64) -> bool {
    // The candidate lattice must not be reselected when an abstract region
    // owner changes. Ecoregion cover still controls whether a candidate reads
    // as a dramatic clearing or only as local openness.
    (hash & 15) < 2
}

fn clearing_cause(kind: EcoregionKind, hash: u64) -> ClearingCause {
    if kind == EcoregionKind::ConnectedWetland || kind == EcoregionKind::RiparianWoodland {
        return ClearingCause::FloodMeadow;
    }
    if kind == EcoregionKind::ExposedUpland {
        return ClearingCause::ShallowSoil;
    }
    match (hash >> 52) % 3 {
        0 => ClearingCause::GrazingLawn,
        1 => ClearingCause::OldBurn,
        _ => ClearingCause::Windthrow,
    }
}

fn main_habitat_route_kind(story: ContinentalStory) -> HabitatRouteKind {
    match story {
        ContinentalStory::RiverValley => HabitatRouteKind::RiparianSpine,
        ContinentalStory::LakeDistrict => HabitatRouteKind::WetlandChain,
        ContinentalStory::Escarpment => HabitatRouteKind::WoodlandPass,
        ContinentalStory::OpenHighland => HabitatRouteKind::OpenRangeLink,
    }
}

fn branch_habitat_route_kind(story: ContinentalStory, slot: u8) -> HabitatRouteKind {
    match (story, slot) {
        (ContinentalStory::RiverValley, 2) => HabitatRouteKind::WetlandChain,
        (ContinentalStory::LakeDistrict, 2) => HabitatRouteKind::RiparianSpine,
        (ContinentalStory::Escarpment, 2) => HabitatRouteKind::OpenRangeLink,
        (ContinentalStory::OpenHighland, 2) => HabitatRouteKind::WoodlandPass,
        _ => main_habitat_route_kind(story),
    }
}

fn habitat_route_half_width(kind: HabitatRouteKind) -> f64 {
    match kind {
        HabitatRouteKind::RiparianSpine => 1_500.0,
        HabitatRouteKind::WetlandChain => 1_850.0,
        HabitatRouteKind::WoodlandPass => 1_300.0,
        HabitatRouteKind::OpenRangeLink => 2_100.0,
    }
}

fn main_habitat_route_curve(continent: ContinentalDistrictSample) -> [RoutePoint; 4] {
    let hash = habitat_route_id(continent, 0).hash;
    let bend = match continent.story {
        ContinentalStory::RiverValley | ContinentalStory::LakeDistrict => 8_500.0,
        ContinentalStory::Escarpment => 6_000.0,
        ContinentalStory::OpenHighland => 7_200.0,
    };
    [
        RoutePoint {
            along: -30_000.0 + signed_hash_unit(hash, 5) * 1_800.0,
            across: signed_hash_unit(hash, 13) * 2_800.0,
        },
        RoutePoint {
            along: -11_000.0 + signed_hash_unit(hash, 21) * 3_800.0,
            across: signed_hash_unit(hash, 29) * bend,
        },
        RoutePoint {
            along: 11_000.0 + signed_hash_unit(hash, 37) * 3_800.0,
            across: signed_hash_unit(hash, 45) * bend,
        },
        RoutePoint {
            along: 30_000.0 + signed_hash_unit(hash, 53) * 1_800.0,
            across: signed_hash_unit(hash, 61) * 2_800.0,
        },
    ]
}

fn branch_habitat_route_curve(
    continent: ContinentalDistrictSample,
    slot: u8,
    main_curve: [RoutePoint; 4],
) -> [RoutePoint; 3] {
    let hash = habitat_route_id(continent, slot).hash;
    let base_progress = match slot {
        1 => 0.25,
        2 => 0.50,
        _ => 0.75,
    };
    let start = cubic_route_point(
        main_curve,
        (base_progress + signed_hash_unit(hash, 7) * 0.055).clamp(0.08, 0.92),
    );
    let side = match slot {
        1 => -1.0,
        2 => 1.0,
        _ if hash & 1 == 0 => -1.0,
        _ => 1.0,
    };
    let reach = 12_000.0 + hash_unit(hash, 19) * 11_000.0;
    let end = RoutePoint {
        along: start.along + signed_hash_unit(hash, 35) * 7_000.0,
        across: start.across + side * reach,
    };
    let control = RoutePoint {
        along: (start.along + end.along) * 0.5 + signed_hash_unit(hash, 47) * 5_500.0,
        across: start.across + side * reach * (0.28 + hash_unit(hash, 57) * 0.32),
    };
    [start, control, end]
}

fn habitat_route_weight(
    distance: f64,
    kind: HabitatRouteKind,
    local_warp: f64,
    route_hash: u64,
) -> f64 {
    let width_variation = 0.50 + unit_field(local_warp) * 0.70;
    let authored_variation = 0.90 + hash_unit(route_hash, 27) * 0.20;
    let longitudinal_texture = 0.92 + unit_field(local_warp) * 0.08;
    (1.0 - distance / (habitat_route_half_width(kind) * width_variation * authored_variation))
        .clamp(0.0, 1.0)
        * longitudinal_texture.min(1.0)
}

fn habitat_route_context(
    kind: HabitatRouteKind,
    ecoregion: EcoregionPlanSample,
    province: ProvincePlanSample,
    clearing_influence: f64,
    local_variation: f64,
) -> f64 {
    let moisture = f64::from(ecoregion.moisture);
    let canopy = f64::from(ecoregion.base_canopy);
    let openness = f64::from(ecoregion.base_openness);
    let water = f64::from(province.major_water);
    let suitability = match kind {
        HabitatRouteKind::RiparianSpine => 0.52 + moisture * 0.26 + water * 0.22,
        HabitatRouteKind::WetlandChain => 0.44 + moisture * 0.32 + water * 0.24,
        HabitatRouteKind::WoodlandPass => 0.48 + canopy * 0.38 + f64::from(province.relief) * 0.14,
        HabitatRouteKind::OpenRangeLink => 0.50 + openness * 0.38 + (1.0 - canopy) * 0.12,
    };
    let interruption = match kind {
        HabitatRouteKind::WoodlandPass => 1.0 - smoothstep(0.42, 0.86, clearing_influence) * 0.80,
        HabitatRouteKind::RiparianSpine | HabitatRouteKind::WetlandChain => {
            1.0 - smoothstep(0.60, 0.92, clearing_influence) * 0.28
        }
        HabitatRouteKind::OpenRangeLink => 1.0,
    };
    let local_patch = unit_field((local_variation / 0.08).clamp(-1.0, 1.0));
    let continuity = match kind {
        HabitatRouteKind::RiparianSpine => 0.68 + smoothstep(0.16, 0.84, local_patch) * 0.32,
        HabitatRouteKind::WetlandChain => 0.18 + smoothstep(0.30, 0.72, local_patch) * 0.82,
        HabitatRouteKind::WoodlandPass => 0.12 + smoothstep(0.26, 0.76, local_patch) * 0.88,
        HabitatRouteKind::OpenRangeLink => 0.42 + smoothstep(0.20, 0.80, local_patch) * 0.58,
    };
    (suitability * interruption * continuity).clamp(0.0, 1.0)
}

fn warped_route_position(position: RoutePoint, local_warp: f64, route_hash: u64) -> RoutePoint {
    let along_direction = if route_hash & 2 == 0 { 1.0 } else { -1.0 };
    let across_direction = if route_hash & 4 == 0 { 1.0 } else { -1.0 };
    RoutePoint {
        along: position.along
            + local_warp * along_direction * (550.0 + hash_unit(route_hash, 11) * 650.0),
        across: position.across
            + local_warp * across_direction * (1_650.0 + hash_unit(route_hash, 33) * 1_100.0),
    }
}

fn cubic_route_distance(position: RoutePoint, curve: [RoutePoint; 4]) -> f64 {
    let mut previous = curve[0];
    let mut distance = f64::INFINITY;
    for segment in 1..=16 {
        let next = cubic_route_point(curve, f64::from(segment) / 16.0);
        distance = distance.min(route_segment_distance(position, previous, next));
        previous = next;
    }
    distance
}

fn quadratic_route_distance(position: RoutePoint, curve: [RoutePoint; 3]) -> f64 {
    let mut previous = curve[0];
    let mut distance = f64::INFINITY;
    for segment in 1..=10 {
        let next = quadratic_route_point(curve, f64::from(segment) / 10.0);
        distance = distance.min(route_segment_distance(position, previous, next));
        previous = next;
    }
    distance
}

fn cubic_route_point(curve: [RoutePoint; 4], progress: f64) -> RoutePoint {
    let inverse = 1.0 - progress;
    RoutePoint {
        along: inverse.powi(3) * curve[0].along
            + 3.0 * inverse.powi(2) * progress * curve[1].along
            + 3.0 * inverse * progress.powi(2) * curve[2].along
            + progress.powi(3) * curve[3].along,
        across: inverse.powi(3) * curve[0].across
            + 3.0 * inverse.powi(2) * progress * curve[1].across
            + 3.0 * inverse * progress.powi(2) * curve[2].across
            + progress.powi(3) * curve[3].across,
    }
}

fn quadratic_route_point(curve: [RoutePoint; 3], progress: f64) -> RoutePoint {
    let inverse = 1.0 - progress;
    RoutePoint {
        along: inverse.powi(2) * curve[0].along
            + 2.0 * inverse * progress * curve[1].along
            + progress.powi(2) * curve[2].along,
        across: inverse.powi(2) * curve[0].across
            + 2.0 * inverse * progress * curve[1].across
            + progress.powi(2) * curve[2].across,
    }
}

fn route_segment_distance(position: RoutePoint, start: RoutePoint, end: RoutePoint) -> f64 {
    let along = end.along - start.along;
    let across = end.across - start.across;
    let length_squared = along * along + across * across;
    if length_squared == 0.0 {
        return (position.along - start.along).hypot(position.across - start.across);
    }
    let progress = (((position.along - start.along) * along
        + (position.across - start.across) * across)
        / length_squared)
        .clamp(0.0, 1.0);
    (position.along - (start.along + along * progress))
        .hypot(position.across - (start.across + across * progress))
}

fn signed_hash_unit(hash: u64, shift: u32) -> f64 {
    hash_unit(hash, shift) * 2.0 - 1.0
}

fn habitat_route_id(continent: ContinentalDistrictSample, slot: u8) -> LandscapeFeatureId {
    feature_id(
        LandscapeFeatureFamily::HabitatRoute,
        continent.id.owner_x,
        continent.id.owner_z,
        slot,
        stable_mix64(
            continent.id.hash ^ HABITAT_ROUTE_HASH_DOMAIN ^ u64::from(slot).rotate_left(41),
        ),
    )
}

fn feature_id(
    family: LandscapeFeatureFamily,
    owner_x: i32,
    owner_z: i32,
    slot: u8,
    hash: u64,
) -> LandscapeFeatureId {
    LandscapeFeatureId {
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

fn signed_hash_offset(hash: u64, shift: u32, maximum: i32) -> i32 {
    let unit = ((hash.rotate_right(shift) & 0xffff) as f64) / 65_535.0;
    ((unit * 2.0 - 1.0) * f64::from(maximum)).round() as i32
}

fn owner_center_coordinate(owner: i32, scale: i32) -> i32 {
    let center = i64::from(owner) * i64::from(scale) + i64::from(scale / 2);
    center.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn hash_unit(hash: u64, shift: u32) -> f64 {
    ((hash.rotate_right(shift) & 0xffff) as f64) / 65_535.0
}

fn direction(hash: u64, shift: u32) -> (f64, f64) {
    const DIAGONAL: f64 = std::f64::consts::FRAC_1_SQRT_2;
    const DIRECTIONS: [(f64, f64); 8] = [
        (1.0, 0.0),
        (DIAGONAL, DIAGONAL),
        (0.0, 1.0),
        (-DIAGONAL, DIAGONAL),
        (-1.0, 0.0),
        (-DIAGONAL, -DIAGONAL),
        (0.0, -1.0),
        (DIAGONAL, -DIAGONAL),
    ];
    DIRECTIONS[((hash >> shift) & 7) as usize]
}

fn unit_field(value: f64) -> f64 {
    (value * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn smoothstep(low: f64, high: f64, value: f64) -> f64 {
    let unit = ((value - low) / (high - low)).clamp(0.0, 1.0);
    unit * unit * (3.0 - 2.0 * unit)
}

fn semantic_sha256(
    descriptor: ContinentalEcoregionDescriptor,
    request: LandscapeWindowRequest,
    samples: &[LandscapePlanSample],
    work: PlanConstructionCounts,
) -> String {
    let mut digest = Sha256::new();
    digest.update(CONTINENTAL_ECOREGION_SCHEMA_REVISION.as_bytes());
    digest.update(CONTINENTAL_ECOREGION_STORED_PROFILE.as_bytes());
    digest.update(CONTINENTAL_ECOREGION_DIMENSION_ID.as_bytes());
    digest.update(descriptor.seed.to_le_bytes());
    match descriptor.topology {
        ContinentalEcoregionTopology::Plane => digest.update([0]),
        ContinentalEcoregionTopology::CylinderX { period_blocks } => {
            digest.update([1]);
            digest.update(period_blocks.to_le_bytes());
        }
    }
    digest.update(request.min_x.to_le_bytes());
    digest.update(request.min_z.to_le_bytes());
    digest.update(request.width_samples.to_le_bytes());
    digest.update(request.depth_samples.to_le_bytes());
    digest.update(request.step_blocks.to_le_bytes());
    digest.update([request.detail as u8]);
    for sample in samples {
        hash_sample(&mut digest, sample);
    }
    digest.update(work.requested_samples.to_le_bytes());
    digest.update(work.continental_owner_evaluations.to_le_bytes());
    digest.update(work.province_owner_evaluations.to_le_bytes());
    digest.update(work.ecoregion_owner_evaluations.to_le_bytes());
    digest.update(work.mosaic_owner_evaluations.to_le_bytes());
    digest.update(work.local_field_evaluations.to_le_bytes());
    digest.update(work.exact_chunks.to_le_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hash_sample(digest: &mut Sha256, sample: &LandscapePlanSample) {
    digest.update(sample.world_x.to_le_bytes());
    digest.update(sample.world_z.to_le_bytes());
    digest.update([sample.detail as u8]);
    hash_float(digest, sample.land_weight);
    hash_float(digest, sample.inland_distance_blocks);
    hash_option(digest, sample.continent, |digest, continent| {
        hash_id(digest, continent.id);
        digest.update([continent.story as u8]);
        digest.update(continent.center_x.to_le_bytes());
        digest.update(continent.center_z.to_le_bytes());
        hash_float(digest, continent.axis_x);
        hash_float(digest, continent.axis_z);
        hash_float(digest, continent.prevailing_wind_x);
        hash_float(digest, continent.prevailing_wind_z);
        hash_float(digest, continent.rain_shadow_potential);
        hash_float(digest, continent.core_weight);
    });
    hash_option(digest, sample.province, |digest, province| {
        hash_id(digest, province.id);
        hash_id(digest, province.continent_id);
        digest.update([province.kind as u8]);
        hash_float(digest, province.core_weight);
        hash_float(digest, province.relief);
        hash_float(digest, province.major_water);
        hash_float(digest, province.leeward_exposure);
    });
    hash_option(digest, sample.ecoregion, |digest, ecoregion| {
        hash_id(digest, ecoregion.id);
        hash_id(digest, ecoregion.province_id);
        digest.update([ecoregion.kind as u8]);
        hash_option(digest, ecoregion.transition_peer_kind, |digest, kind| {
            digest.update([kind as u8]);
        });
        hash_float(digest, ecoregion.core_weight);
        hash_float(digest, ecoregion.transition_weight);
        hash_float(digest, ecoregion.transition_width_blocks);
        hash_float(digest, ecoregion.base_openness);
        hash_float(digest, ecoregion.base_canopy);
        hash_float(digest, ecoregion.moisture);
        hash_float(digest, ecoregion.temperature);
        hash_float(digest, ecoregion.aridity);
        hash_float(digest, ecoregion.drainage_permanence);
    });
    hash_option(digest, sample.mosaic, |digest, mosaic| {
        hash_option(digest, mosaic.clearing_id, hash_id);
        hash_option(digest, mosaic.clearing_cause, |digest, cause| {
            digest.update([cause as u8]);
        });
        hash_option(digest, mosaic.corridor_id, hash_id);
        hash_option(digest, mosaic.corridor_kind, |digest, kind| {
            digest.update([kind as u8]);
        });
        hash_float(digest, mosaic.clearing_core);
        hash_float(digest, mosaic.clearing_shoulder);
        hash_float(digest, mosaic.openness);
        hash_float(digest, mosaic.forest_core);
        hash_float(digest, mosaic.wetland);
        hash_float(digest, mosaic.corridor);
        digest.update(mosaic.local_fingerprint.to_le_bytes());
    });
}

fn hash_option<T: Copy>(
    digest: &mut Sha256,
    value: Option<T>,
    hash_value: impl FnOnce(&mut Sha256, T),
) {
    match value {
        Some(value) => {
            digest.update([1]);
            hash_value(digest, value);
        }
        None => digest.update([0]),
    }
}

fn hash_id(digest: &mut Sha256, id: LandscapeFeatureId) {
    digest.update([id.family as u8]);
    digest.update(id.owner_x.to_le_bytes());
    digest.update(id.owner_z.to_le_bytes());
    digest.update([id.slot]);
    digest.update(id.hash.to_le_bytes());
}

fn hash_float(digest: &mut Sha256, value: f32) {
    let quantized = (f64::from(value) * 10_000.0).round() as i64;
    digest.update(quantized.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: i64 = 12_345;

    fn plan() -> ContinentalEcoregionPlan {
        ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::plane(SEED)).unwrap()
    }

    #[test]
    fn tiny_proof_cylinder_is_explicitly_incompatible() {
        let topology = ContinentalEcoregionTopology::cylinder_x(6_144);
        let compatibility = topology.compatibility();
        assert!(!compatibility.supported);
        assert_eq!(compatibility.minimum_period_blocks, 131_072);
        assert!(
            ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::new(SEED, topology))
                .is_err()
        );
    }

    #[test]
    fn direct_queries_stop_at_the_requested_level() {
        let plan = plan();
        let continental = plan.query_point(LandscapePlanDetail::Continental, 0, 0);
        assert!(continental.sample.province.is_none());
        assert_eq!(continental.work.province_owner_evaluations, 0);
        assert_eq!(continental.work.ecoregion_owner_evaluations, 0);
        assert_eq!(continental.work.mosaic_owner_evaluations, 0);
        assert_eq!(continental.work.local_field_evaluations, 0);
        assert_eq!(continental.work.exact_chunks, 0);

        let province = plan.query_point(LandscapePlanDetail::Province, 0, 0);
        assert!(province.sample.ecoregion.is_none());
        assert_eq!(province.work.ecoregion_owner_evaluations, 0);
        assert_eq!(province.work.mosaic_owner_evaluations, 0);
        assert_eq!(province.work.local_field_evaluations, 2);
        assert_eq!(province.work.exact_chunks, 0);
    }

    #[test]
    fn ecotone_widths_are_typed_by_the_adjacent_ecoregions() {
        assert_eq!(
            ecoregion_transition_width_blocks(
                EcoregionKind::BroadMeadow,
                EcoregionKind::BroadMeadow,
            ),
            0.0
        );
        assert_eq!(
            ecoregion_transition_width_blocks(
                EcoregionKind::BroadMeadow,
                EcoregionKind::OldForestCore,
            ),
            800.0
        );
        assert_eq!(
            ecoregion_transition_width_blocks(
                EcoregionKind::ConnectedWetland,
                EcoregionKind::BroadMeadow,
            ),
            2_600.0
        );
        assert_eq!(
            ecoregion_transition_width_blocks(
                EcoregionKind::QuietTransition,
                EcoregionKind::ExposedUpland,
            ),
            1_800.0
        );
    }

    #[test]
    fn habitat_route_spines_and_branches_have_stable_typed_identities() {
        let continent = ContinentalDistrictSample {
            id: feature_id(
                LandscapeFeatureFamily::ContinentalDistrict,
                -2,
                7,
                0,
                0x1234,
            ),
            story: ContinentalStory::LakeDistrict,
            center_x: 0,
            center_z: 0,
            axis_x: 1.0,
            axis_z: 0.0,
            prevailing_wind_x: 0.0,
            prevailing_wind_z: -1.0,
            rain_shadow_potential: 0.0,
            core_weight: 1.0,
        };
        assert_eq!(
            main_habitat_route_kind(continent.story),
            HabitatRouteKind::WetlandChain
        );
        assert_eq!(
            branch_habitat_route_kind(continent.story, 2),
            HabitatRouteKind::RiparianSpine
        );
        let spine = habitat_route_id(continent, 0);
        let branch = habitat_route_id(continent, 2);
        assert_eq!(spine.family, LandscapeFeatureFamily::HabitatRoute);
        assert_eq!((spine.owner_x, spine.owner_z), (-2, 7));
        assert_ne!(spine, branch);
        assert_eq!(spine, habitat_route_id(continent, 0));

        let main = main_habitat_route_curve(continent);
        let left_branch = branch_habitat_route_curve(continent, 1, main);
        let right_branch = branch_habitat_route_curve(continent, 2, main);
        assert_eq!(cubic_route_distance(main[0], main), 0.0);
        assert_eq!(quadratic_route_distance(left_branch[0], left_branch), 0.0);
        assert_eq!(quadratic_route_distance(right_branch[0], right_branch), 0.0);
        assert!(left_branch[2].across < left_branch[0].across);
        assert!(right_branch[2].across > right_branch[0].across);
    }

    #[test]
    fn point_queries_equal_window_samples() {
        let plan = plan();
        let request = LandscapeWindowRequest::new(
            -32_768,
            -16_384,
            17,
            11,
            1_024,
            LandscapePlanDetail::Mosaic,
        );
        let window = plan.query_window(request).unwrap();
        for sample_z in 0..request.depth_samples {
            for sample_x in 0..request.width_samples {
                let index = (sample_z * request.width_samples + sample_x) as usize;
                let point = plan.query_point(
                    request.detail,
                    request.min_x + (sample_x * request.step_blocks) as i32,
                    request.min_z + (sample_z * request.step_blocks) as i32,
                );
                assert_eq!(point.sample, window.samples[index]);
            }
        }
    }

    #[test]
    fn regional_identity_boundaries_do_not_reset_cover_or_local_content() {
        let plan = plan();
        let mut province_crossings = 0_u32;
        let mut ecoregion_crossings = 0_u32;
        let mut max_openness_jump = 0.0_f32;
        let mut max_canopy_jump = 0.0_f32;

        for z in (-65_536..=65_536).step_by(512) {
            for x in (-65_536..=65_536).step_by(512) {
                let left = plan.query_point(LandscapePlanDetail::Mosaic, x, z).sample;
                let right = plan
                    .query_point(LandscapePlanDetail::Mosaic, x + 64, z)
                    .sample;
                let (Some(left_province), Some(right_province)) = (left.province, right.province)
                else {
                    continue;
                };
                let (Some(left_ecoregion), Some(right_ecoregion)) =
                    (left.ecoregion, right.ecoregion)
                else {
                    continue;
                };
                let (Some(left_mosaic), Some(right_mosaic)) = (left.mosaic, right.mosaic) else {
                    continue;
                };

                // Both points deliberately remain in one absolute 256-block
                // content cell. Abstract region ownership must not reseed it.
                assert_eq!(
                    left_mosaic.local_fingerprint,
                    right_mosaic.local_fingerprint
                );
                if left_province.id != right_province.id {
                    province_crossings += 1;
                }
                if left_ecoregion.id != right_ecoregion.id {
                    ecoregion_crossings += 1;
                    max_openness_jump = max_openness_jump
                        .max((left_ecoregion.base_openness - right_ecoregion.base_openness).abs());
                    max_canopy_jump = max_canopy_jump
                        .max((left_ecoregion.base_canopy - right_ecoregion.base_canopy).abs());
                }
            }
        }

        assert!(
            province_crossings > 20,
            "province crossings: {province_crossings}"
        );
        assert!(
            ecoregion_crossings > 50,
            "ecoregion crossings: {ecoregion_crossings}"
        );
        assert!(
            max_openness_jump < 0.10,
            "openness jumped {max_openness_jump} across a region identity"
        );
        assert!(
            max_canopy_jump < 0.10,
            "canopy jumped {max_canopy_jump} across a region identity"
        );
    }

    #[test]
    fn partitioned_windows_equal_the_whole_window() {
        let plan = plan();
        let whole_request = LandscapeWindowRequest::new(
            -65_536,
            -32_768,
            32,
            18,
            2_048,
            LandscapePlanDetail::Mosaic,
        );
        let whole = plan.query_window(whole_request).unwrap();
        let left = plan
            .query_window(LandscapeWindowRequest {
                width_samples: 13,
                ..whole_request
            })
            .unwrap();
        let right = plan
            .query_window(LandscapeWindowRequest {
                min_x: whole_request.min_x + 13 * whole_request.step_blocks as i32,
                width_samples: whole_request.width_samples - 13,
                ..whole_request
            })
            .unwrap();
        for row in 0..whole_request.depth_samples as usize {
            let whole_start = row * whole_request.width_samples as usize;
            let left_start = row * left.request.width_samples as usize;
            let right_start = row * right.request.width_samples as usize;
            assert_eq!(
                &whole.samples[whole_start..whole_start + 13],
                &left.samples[left_start..left_start + 13]
            );
            assert_eq!(
                &whole.samples[whole_start + 13..whole_start + 32],
                &right.samples[right_start..right_start + 19]
            );
        }
    }

    #[test]
    fn supported_cylinder_has_exact_periodic_lifts() {
        let period = 196_608;
        let plan = ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::new(
            SEED,
            ContinentalEcoregionTopology::cylinder_x(period),
        ))
        .unwrap();
        for &(x, z) in &[(-17_331, 2_049), (0, 0), (65_537, -44_000)] {
            let original = plan.query_point(LandscapePlanDetail::Mosaic, x, z);
            let lifted = plan.query_point(LandscapePlanDetail::Mosaic, x + period, z);
            let mut expected = original.sample;
            expected.world_x += period;
            assert_eq!(expected, lifted.sample);
            assert_eq!(original.work, lifted.work);
        }
    }

    #[test]
    fn window_caps_and_coordinate_overflow_fail_closed() {
        let plan = plan();
        assert!(matches!(
            plan.query_window(LandscapeWindowRequest::new(
                0,
                0,
                513,
                512,
                1,
                LandscapePlanDetail::Continental,
            )),
            Err(ContinentalEcoregionError::InvalidWindow(_))
        ));
        assert!(matches!(
            plan.query_window(LandscapeWindowRequest::new(
                i32::MAX,
                0,
                2,
                1,
                1,
                LandscapePlanDetail::Continental,
            )),
            Err(ContinentalEcoregionError::CoordinateOverflow)
        ));
    }
}
