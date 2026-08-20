//! Research candidate for authored continental and ecoregional planning.
//!
//! The candidate is deliberately disconnected from production chunk
//! generation. It exposes coordinate-pure typed facts and bounded point/window
//! queries so Terrain Lab can judge the geography before it reaches the live
//! Overworld.

use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::noise::{SeedDomain, ValueNoise2d};

pub const CONTINENTAL_ECOREGION_SCHEMA_REVISION: &str = "mclone-continental-ecoregion-plan-v1";
pub const CONTINENTAL_ECOREGION_DIMENSION_ID: &str = "mclone:overworld";
pub const CONTINENTAL_ECOREGION_STORED_PROFILE: &str =
    "mclone-overworld-v1-control-field-revision-21";
pub const CONTINENTAL_CELL_BLOCKS: i32 = 32_768;
pub const PROVINCE_CELL_BLOCKS: i32 = 16_384;
pub const ECOREGION_CELL_BLOCKS: i32 = 4_096;
pub const MOSAIC_CELL_BLOCKS: i32 = 2_048;
pub const MIN_SUPPORTED_CYLINDER_BLOCKS: i32 = 131_072;
pub const MAX_WINDOW_SAMPLES: usize = 262_144;

const CONTINENT_OWNER_RADIUS: i32 = 2;
const LOCAL_OWNER_RADIUS: i32 = 1;
const CONTINENT_EDGE_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_6564_6731);
const CLIMATE_MOISTURE_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_6d6f_6931);
const CLIMATE_TEMPERATURE_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_7465_6d31);
const CORRIDOR_WARP_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_636f_7231);
const LOCAL_OPENNESS_DOMAIN: SeedDomain = SeedDomain::new(0x6365_636f_6f70_6531);

const CONTINENT_HASH_DOMAIN: u64 = 0x6365_636f_636f_6e31;
const PROVINCE_HASH_DOMAIN: u64 = 0x6365_636f_7072_6f31;
const ECOREGION_HASH_DOMAIN: u64 = 0x6365_636f_6563_6f31;
const MOSAIC_HASH_DOMAIN: u64 = 0x6365_636f_6d6f_7331;
const LOCAL_HASH_DOMAIN: u64 = 0x6365_636f_6c6f_6331;

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
#[serde(rename_all = "kebab-case")]
pub enum ContinentalStory {
    RiverValley,
    LakeDistrict,
    Escarpment,
    OpenHighland,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhysiographicProvinceKind {
    RiverLowland,
    LakeBasin,
    RollingHills,
    WoodedUpland,
    RockyRidge,
    QuietBench,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClearingCause {
    GrazingLawn,
    OldBurn,
    Windthrow,
    FloodMeadow,
    ShallowSoil,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcoregionPlanSample {
    pub id: LandscapeFeatureId,
    pub province_id: LandscapeFeatureId,
    pub kind: EcoregionKind,
    pub core_weight: f32,
    pub transition_weight: f32,
    pub base_openness: f32,
    pub base_canopy: f32,
    pub moisture: f32,
    pub temperature: f32,
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
    continent_edge: ValueNoise2d,
    moisture: ValueNoise2d,
    temperature: ValueNoise2d,
    corridor_warp: ValueNoise2d,
    local_openness: ValueNoise2d,
}

impl PlanFields {
    fn new(descriptor: ContinentalEcoregionDescriptor) -> Self {
        let value_noise = |domain, scale| match descriptor.topology {
            ContinentalEcoregionTopology::Plane => {
                ValueNoise2d::new(descriptor.seed, domain, scale)
            }
            ContinentalEcoregionTopology::CylinderX { period_blocks } => {
                ValueNoise2d::new_periodic_x(descriptor.seed, domain, scale, period_blocks)
            }
        };
        Self {
            continent_edge: value_noise(CONTINENT_EDGE_DOMAIN, 8_192),
            moisture: value_noise(CLIMATE_MOISTURE_DOMAIN, 8_192),
            temperature: value_noise(CLIMATE_TEMPERATURE_DOMAIN, 16_384),
            corridor_warp: value_noise(CORRIDOR_WARP_DOMAIN, 8_192),
            local_openness: value_noise(LOCAL_OPENNESS_DOMAIN, 1_024),
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

        let province = self.province_sample(world_x, world_z, continent, &mut work);
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
            Some(self.mosaic_sample(world_x, world_z, continent, province, ecoregion, &mut work));
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
                let edge_warp = self.fields.continent_edge.sample(canonical_x, world_z) * 0.13;
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
                    best = Some((score, site));
                }
            }
        }

        let (score, site) = best.expect("continental owner neighborhood is non-empty");
        let land_weight = smoothstep(-0.08, 0.10, score) as f32;
        ContinentalInternalSample {
            sample: ContinentalDistrictSample {
                id: site.id,
                story: site.story,
                center_x: site.center_x,
                center_z: site.center_z,
                axis_x: site.axis_x as f32,
                axis_z: site.axis_z as f32,
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
        let corridor = self.corridor_weight(world_x, world_z, continent);
        ProvincePlanSample {
            id,
            continent_id: continent.id,
            kind,
            core_weight: site.core_weight as f32,
            relief: province_relief(kind),
            major_water: match kind {
                PhysiographicProvinceKind::RiverLowland => corridor.max(0.45),
                PhysiographicProvinceKind::LakeBasin => (0.55 + corridor * 0.35).min(1.0),
                _ => corridor * 0.65,
            } as f32,
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
        let kind = ecoregion_kind(province.kind, id_hash);
        work.local_field_evaluations += 2;
        let canonical_x = self.descriptor.topology.canonical_world_x(world_x);
        let moisture = unit_field(self.fields.moisture.sample(canonical_x, world_z));
        let temperature =
            (0.5 + self.fields.temperature.sample(canonical_x, world_z) * 0.16).clamp(0.25, 0.75);
        let (base_openness, base_canopy) = ecoregion_cover(kind);
        EcoregionPlanSample {
            id,
            province_id: province.id,
            kind,
            core_weight: site.core_weight as f32,
            transition_weight: (1.0 - site.core_weight) as f32,
            base_openness,
            base_canopy,
            moisture: moisture as f32,
            temperature: temperature as f32,
        }
    }

    fn mosaic_sample(
        &self,
        world_x: i32,
        world_z: i32,
        continent: ContinentalDistrictSample,
        province: ProvincePlanSample,
        ecoregion: EcoregionPlanSample,
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
                    MOSAIC_HASH_DOMAIN ^ ecoregion.id.hash,
                    owner_x,
                    owner_z,
                    0,
                );
                if !clearing_enabled(ecoregion.kind, hash) {
                    continue;
                }
                let center_x = i64::from(raw_owner_x) * i64::from(MOSAIC_CELL_BLOCKS)
                    + i64::from(MOSAIC_CELL_BLOCKS / 2)
                    + i64::from(signed_hash_offset(hash, 8, 400));
                let center_z = i64::from(owner_z) * i64::from(MOSAIC_CELL_BLOCKS)
                    + i64::from(MOSAIC_CELL_BLOCKS / 2)
                    + i64::from(signed_hash_offset(hash, 24, 400));
                let (axis_x, axis_z) = direction(hash, 40);
                let local_x = (i64::from(canonical_x) - center_x) as f64;
                let local_z = (i64::from(world_z) - center_z) as f64;
                let along = local_x * axis_x + local_z * axis_z;
                let across = -local_x * axis_z + local_z * axis_x;
                let radius_along = 650.0 + hash_unit(hash, 48) * 800.0;
                let radius_across = 420.0 + hash_unit(hash, 16) * 680.0;
                let distance =
                    ((along / radius_along).powi(2) + (across / radius_across).powi(2)).sqrt();
                let influence = (1.0 - distance).clamp(0.0, 1.0);
                let id_hash = stable_mix64(hash ^ ecoregion.id.hash.rotate_left(31));
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
        let corridor = self.corridor_weight(world_x, world_z, continent);
        let (clearing_influence, clearing_id, clearing_cause) = best
            .filter(|(influence, _, _)| *influence > 0.0)
            .map_or((0.0, None, None), |(influence, id, cause)| {
                (influence, Some(id), Some(cause))
            });
        let clearing_core = smoothstep(0.35, 0.82, clearing_influence);
        let clearing_shoulder =
            (smoothstep(0.02, 0.55, clearing_influence) - clearing_core).max(0.0);
        let openness =
            (f64::from(ecoregion.base_openness) + clearing_influence * 0.72 + local_variation)
                .clamp(0.0, 1.0);
        let forest_affinity = match ecoregion.kind {
            EcoregionKind::OldForestCore => 1.0,
            EcoregionKind::RiparianWoodland | EcoregionKind::MixedWoodland => 0.82,
            EcoregionKind::QuietTransition => 0.58,
            _ => 0.28,
        };
        let forest_core = (f64::from(ecoregion.base_canopy)
            * forest_affinity
            * (1.0 - clearing_influence)
            * f64::from(ecoregion.core_weight))
        .clamp(0.0, 1.0);
        let wetland_affinity = match ecoregion.kind {
            EcoregionKind::ConnectedWetland => 1.0,
            EcoregionKind::RiparianWoodland => 0.72,
            _ => 0.22,
        };
        let wetland = (wetland_affinity
            * f64::from(ecoregion.moisture)
            * f64::from(province.major_water.max(corridor as f32)))
        .clamp(0.0, 1.0);
        let local_cell_x = canonical_x.div_euclid(256);
        let local_cell_z = world_z.div_euclid(256);
        let local_fingerprint = coordinate_hash(
            self.descriptor.seed,
            LOCAL_HASH_DOMAIN ^ ecoregion.id.hash,
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
        let base_owner_x = canonical_x.div_euclid(scale);
        let base_owner_z = world_z.div_euclid(scale);
        let mut nearest: Option<(f64, NearestSite)> = None;
        let mut second_distance = f64::INFINITY;
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
                let distance = ((i64::from(canonical_x) - center_x) as f64)
                    .hypot((i64::from(world_z) - center_z) as f64);
                let site = NearestSite {
                    owner_x,
                    owner_z,
                    center_x,
                    center_z,
                    hash,
                    core_weight: 0.0,
                };
                match nearest {
                    None => nearest = Some((distance, site)),
                    Some((nearest_distance, nearest_site))
                        if distance < nearest_distance
                            || (distance == nearest_distance && hash < nearest_site.hash) =>
                    {
                        second_distance = nearest_distance;
                        nearest = Some((distance, site));
                    }
                    Some(_) if distance < second_distance => second_distance = distance,
                    Some(_) => {}
                }
            }
        }
        let (nearest_distance, mut site) = nearest.expect("site neighborhood is non-empty");
        site.core_weight =
            ((second_distance - nearest_distance) / (f64::from(scale) * 0.48)).clamp(0.0, 1.0);
        site
    }

    fn corridor_weight(
        &self,
        world_x: i32,
        world_z: i32,
        continent: ContinentalDistrictSample,
    ) -> f64 {
        let canonical_x = self.descriptor.topology.canonical_world_x(world_x);
        let dx = (i64::from(canonical_x) - continent.center_x) as f64;
        let dz = (i64::from(world_z) - continent.center_z) as f64;
        let across = -dx * f64::from(continent.axis_z) + dz * f64::from(continent.axis_x);
        let warp = self.fields.corridor_warp.sample(canonical_x, world_z) * 2_400.0;
        (1.0 - (across - warp).abs() / 2_800.0).clamp(0.0, 1.0)
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

fn ecoregion_kind(kind: PhysiographicProvinceKind, hash: u64) -> EcoregionKind {
    let slot = ((hash >> 55) % 6) as usize;
    let choices: &[EcoregionKind] = match kind {
        PhysiographicProvinceKind::RiverLowland => &[
            EcoregionKind::RiparianWoodland,
            EcoregionKind::ConnectedWetland,
            EcoregionKind::BroadMeadow,
            EcoregionKind::RiparianWoodland,
            EcoregionKind::QuietTransition,
            EcoregionKind::ConnectedWetland,
        ],
        PhysiographicProvinceKind::LakeBasin => &[
            EcoregionKind::ConnectedWetland,
            EcoregionKind::RiparianWoodland,
            EcoregionKind::QuietTransition,
            EcoregionKind::MixedWoodland,
            EcoregionKind::ConnectedWetland,
            EcoregionKind::BroadMeadow,
        ],
        PhysiographicProvinceKind::RollingHills => &[
            EcoregionKind::BroadMeadow,
            EcoregionKind::MixedWoodland,
            EcoregionKind::QuietTransition,
            EcoregionKind::BroadMeadow,
            EcoregionKind::MixedWoodland,
            EcoregionKind::OldForestCore,
        ],
        PhysiographicProvinceKind::WoodedUpland => &[
            EcoregionKind::OldForestCore,
            EcoregionKind::MixedWoodland,
            EcoregionKind::OldForestCore,
            EcoregionKind::BroadMeadow,
            EcoregionKind::QuietTransition,
            EcoregionKind::MixedWoodland,
        ],
        PhysiographicProvinceKind::RockyRidge => &[
            EcoregionKind::ExposedUpland,
            EcoregionKind::OldForestCore,
            EcoregionKind::ExposedUpland,
            EcoregionKind::QuietTransition,
            EcoregionKind::MixedWoodland,
            EcoregionKind::ExposedUpland,
        ],
        PhysiographicProvinceKind::QuietBench => &[
            EcoregionKind::QuietTransition,
            EcoregionKind::MixedWoodland,
            EcoregionKind::BroadMeadow,
            EcoregionKind::QuietTransition,
            EcoregionKind::OldForestCore,
            EcoregionKind::MixedWoodland,
        ],
    };
    choices[slot]
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

fn clearing_enabled(kind: EcoregionKind, hash: u64) -> bool {
    let threshold = match kind {
        EcoregionKind::OldForestCore => 2,
        EcoregionKind::RiparianWoodland => 3,
        EcoregionKind::MixedWoodland => 4,
        EcoregionKind::QuietTransition => 3,
        EcoregionKind::BroadMeadow => 1,
        EcoregionKind::ConnectedWetland | EcoregionKind::ExposedUpland => 1,
    };
    (hash & 7) < threshold
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
    });
    hash_option(digest, sample.province, |digest, province| {
        hash_id(digest, province.id);
        hash_id(digest, province.continent_id);
        digest.update([province.kind as u8]);
        hash_float(digest, province.core_weight);
        hash_float(digest, province.relief);
        hash_float(digest, province.major_water);
    });
    hash_option(digest, sample.ecoregion, |digest, ecoregion| {
        hash_id(digest, ecoregion.id);
        hash_id(digest, ecoregion.province_id);
        digest.update([ecoregion.kind as u8]);
        hash_float(digest, ecoregion.core_weight);
        hash_float(digest, ecoregion.transition_weight);
        hash_float(digest, ecoregion.base_openness);
        hash_float(digest, ecoregion.base_canopy);
        hash_float(digest, ecoregion.moisture);
        hash_float(digest, ecoregion.temperature);
    });
    hash_option(digest, sample.mosaic, |digest, mosaic| {
        hash_option(digest, mosaic.clearing_id, hash_id);
        hash_option(digest, mosaic.clearing_cause, |digest, cause| {
            digest.update([cause as u8]);
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
        assert_eq!(province.work.local_field_evaluations, 0);
        assert_eq!(province.work.exact_chunks, 0);
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
