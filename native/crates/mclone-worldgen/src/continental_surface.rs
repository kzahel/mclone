//! Broad surface realization for the accepted continental/ecoregional plan.
//!
//! This research source is intentionally disconnected from exact chunk
//! generation. It lowers stable plan facts into directly queryable height,
//! water, substrate, and cover facts for World Explorer review.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    block::{COARSE_DIRT, GRASS_BLOCK, GRAVEL, SAND, STONE},
    continental_ecoregion::{
        ContinentalEcoregionDescriptor, ContinentalEcoregionError, ContinentalEcoregionPlan,
        ContinentalEcoregionTopology, HabitatRouteKind, LandscapeFeatureId, LandscapePlanDetail,
        LandscapePlanSample, LandscapeWindowRequest, PhysiographicProvinceKind,
        PlanConstructionCounts,
    },
    levelgen::MCLONE_OVERWORLD_SEA_LEVEL,
    noise::{SeedDomain, ValueNoise2d},
};

pub const CONTINENTAL_SURFACE_SCHEMA_REVISION: &str = "mclone-continental-surface-v1";
pub const CONTINENTAL_SURFACE_SOURCE_LABEL: &str = "continental-ecoregion-candidate-v1";
pub const CONTINENTAL_SURFACE_FAMILY_COUNT: usize = 5;
pub const CONTINENTAL_SURFACE_MAX_WINDOW_SAMPLES: usize = 262_144;

const MACRO_ROLL_DOMAIN: SeedDomain = SeedDomain::new(0x6373_7572_6d61_6331);
const RIDGE_FORM_DOMAIN: SeedDomain = SeedDomain::new(0x6373_7572_7269_6431);
const BASIN_FORM_DOMAIN: SeedDomain = SeedDomain::new(0x6373_7572_6261_7331);
const SHORE_FORM_DOMAIN: SeedDomain = SeedDomain::new(0x6373_7572_7368_6f31);
const LOCAL_FORM_DOMAIN: SeedDomain = SeedDomain::new(0x6373_7572_6c6f_6331);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum TerrainCharacterFamily {
    CoastAndHeadland,
    RollingInterior,
    FluvialAndWetland,
    UplandAndEscarpment,
    AridRainShadow,
}

impl TerrainCharacterFamily {
    pub const ALL: [Self; CONTINENTAL_SURFACE_FAMILY_COUNT] = [
        Self::CoastAndHeadland,
        Self::RollingInterior,
        Self::FluvialAndWetland,
        Self::UplandAndEscarpment,
        Self::AridRainShadow,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::CoastAndHeadland => "coast-and-headland",
            Self::RollingInterior => "rolling-interior",
            Self::FluvialAndWetland => "fluvial-and-wetland",
            Self::UplandAndEscarpment => "upland-and-escarpment",
            Self::AridRainShadow => "arid-rain-shadow",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalSurfaceWaterKind {
    #[default]
    None,
    Ocean,
    Lake,
    River,
    WetlandPool,
}

impl ContinentalSurfaceWaterKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ocean => "ocean",
            Self::Lake => "lake",
            Self::River => "river",
            Self::WetlandPool => "wetland-pool",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalSurfaceSubstrate {
    Grass,
    Sand,
    Gravel,
    Stone,
    CoarseSoil,
}

impl ContinentalSurfaceSubstrate {
    pub const fn block_id(self) -> u16 {
        match self {
            Self::Grass => GRASS_BLOCK,
            Self::Sand => SAND,
            Self::Gravel => GRAVEL,
            Self::Stone => STONE,
            Self::CoarseSoil => COARSE_DIRT,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Grass => "grass",
            Self::Sand => "sand",
            Self::Gravel => "gravel",
            Self::Stone => "stone",
            Self::CoarseSoil => "coarse-soil",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceConstructionCounts {
    pub requested_samples: u64,
    pub continental_owner_evaluations: u64,
    pub province_owner_evaluations: u64,
    pub ecoregion_owner_evaluations: u64,
    pub mosaic_owner_evaluations: u64,
    pub plan_field_evaluations: u64,
    pub surface_field_evaluations: u64,
    pub exact_chunks: u64,
    pub density_volumes: u64,
    pub feature_batches: u64,
}

impl ContinentalSurfaceConstructionCounts {
    fn from_plan(work: PlanConstructionCounts) -> Self {
        Self {
            requested_samples: work.requested_samples,
            continental_owner_evaluations: work.continental_owner_evaluations,
            province_owner_evaluations: work.province_owner_evaluations,
            ecoregion_owner_evaluations: work.ecoregion_owner_evaluations,
            mosaic_owner_evaluations: work.mosaic_owner_evaluations,
            plan_field_evaluations: work.local_field_evaluations,
            surface_field_evaluations: 5,
            exact_chunks: work.exact_chunks,
            density_volumes: 0,
            feature_batches: 0,
        }
    }

    fn add_assign(&mut self, other: Self) {
        self.requested_samples += other.requested_samples;
        self.continental_owner_evaluations += other.continental_owner_evaluations;
        self.province_owner_evaluations += other.province_owner_evaluations;
        self.ecoregion_owner_evaluations += other.ecoregion_owner_evaluations;
        self.mosaic_owner_evaluations += other.mosaic_owner_evaluations;
        self.plan_field_evaluations += other.plan_field_evaluations;
        self.surface_field_evaluations += other.surface_field_evaluations;
        self.exact_chunks += other.exact_chunks;
        self.density_volumes += other.density_volumes;
        self.feature_batches += other.feature_batches;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceSample {
    pub world_x: i32,
    pub world_z: i32,
    pub continent_id: Option<LandscapeFeatureId>,
    pub province_id: Option<LandscapeFeatureId>,
    pub ecoregion_id: Option<LandscapeFeatureId>,
    pub clearing_id: Option<LandscapeFeatureId>,
    pub route_id: Option<LandscapeFeatureId>,
    pub family_weights: [f32; CONTINENTAL_SURFACE_FAMILY_COUNT],
    pub dominant_family: TerrainCharacterFamily,
    pub continental_height: f32,
    pub province_height: f32,
    pub hydrologic_height: f32,
    pub local_height: f32,
    pub solid_surface_y: f32,
    pub display_surface_y: f32,
    pub water_level_y: Option<f32>,
    pub water_kind: ContinentalSurfaceWaterKind,
    pub substrate: ContinentalSurfaceSubstrate,
    pub openness: f32,
    pub forest_core: f32,
    pub forest_edge: f32,
    pub wetland: f32,
    pub clearing: f32,
    pub route: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub aridity: f32,
    pub leeward_exposure: f32,
}

impl ContinentalSurfaceSample {
    pub fn is_water(self) -> bool {
        self.water_level_y
            .is_some_and(|water| water > self.solid_surface_y)
    }

    pub const fn visible_material(self) -> u16 {
        self.substrate.block_id()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfacePointQuery {
    pub sample: ContinentalSurfaceSample,
    pub work: ContinentalSurfaceConstructionCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceWindowRequest {
    pub min_x: i32,
    pub min_z: i32,
    pub width_samples: u32,
    pub depth_samples: u32,
    pub step_blocks: u32,
}

impl ContinentalSurfaceWindowRequest {
    pub const fn new(
        min_x: i32,
        min_z: i32,
        width_samples: u32,
        depth_samples: u32,
        step_blocks: u32,
    ) -> Self {
        Self {
            min_x,
            min_z,
            width_samples,
            depth_samples,
            step_blocks,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceWindow {
    pub descriptor: ContinentalEcoregionDescriptor,
    pub request: ContinentalSurfaceWindowRequest,
    pub samples: Vec<ContinentalSurfaceSample>,
    pub work: ContinentalSurfaceConstructionCounts,
    pub semantic_sha256: String,
}

#[derive(Clone, Copy, Debug)]
struct ContinentalSurfaceFields {
    macro_roll: ValueNoise2d,
    ridge_form: ValueNoise2d,
    basin_form: ValueNoise2d,
    shore_form: ValueNoise2d,
    local_form: ValueNoise2d,
}

impl ContinentalSurfaceFields {
    fn new(descriptor: ContinentalEcoregionDescriptor) -> Self {
        let field = |domain, scale| match descriptor.topology {
            ContinentalEcoregionTopology::Plane => {
                ValueNoise2d::new(descriptor.seed, domain, scale)
            }
            ContinentalEcoregionTopology::CylinderX { period_blocks } => {
                ValueNoise2d::new_periodic_x(descriptor.seed, domain, scale, period_blocks)
            }
        };
        Self {
            macro_roll: field(MACRO_ROLL_DOMAIN, 8_192),
            ridge_form: field(RIDGE_FORM_DOMAIN, 4_096),
            basin_form: field(BASIN_FORM_DOMAIN, 4_096),
            shore_form: field(SHORE_FORM_DOMAIN, 2_048),
            local_form: field(LOCAL_FORM_DOMAIN, 512),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContinentalSurfacePlan {
    descriptor: ContinentalEcoregionDescriptor,
    plan: ContinentalEcoregionPlan,
    fields: ContinentalSurfaceFields,
}

impl ContinentalSurfacePlan {
    pub fn new(
        descriptor: ContinentalEcoregionDescriptor,
    ) -> Result<Self, ContinentalEcoregionError> {
        Ok(Self {
            descriptor,
            plan: ContinentalEcoregionPlan::new(descriptor)?,
            fields: ContinentalSurfaceFields::new(descriptor),
        })
    }

    pub const fn descriptor(&self) -> ContinentalEcoregionDescriptor {
        self.descriptor
    }

    pub fn query_point(&self, world_x: i32, world_z: i32) -> ContinentalSurfacePointQuery {
        let plan = self
            .plan
            .query_point(LandscapePlanDetail::Mosaic, world_x, world_z);
        let sample = self.realize(world_x, world_z, &plan.sample);
        ContinentalSurfacePointQuery {
            sample,
            work: ContinentalSurfaceConstructionCounts::from_plan(plan.work),
        }
    }

    pub fn query_window(
        &self,
        request: ContinentalSurfaceWindowRequest,
    ) -> Result<ContinentalSurfaceWindow, ContinentalEcoregionError> {
        validate_window(request)?;
        let sample_count = (request.width_samples as usize)
            .checked_mul(request.depth_samples as usize)
            .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
        let mut samples = Vec::with_capacity(sample_count);
        let mut work = ContinentalSurfaceConstructionCounts::default();
        for sample_z in 0..request.depth_samples {
            let world_z = window_coordinate(request.min_z, sample_z, request.step_blocks)?;
            for sample_x in 0..request.width_samples {
                let world_x = window_coordinate(request.min_x, sample_x, request.step_blocks)?;
                let query = self.query_point(world_x, world_z);
                work.add_assign(query.work);
                samples.push(query.sample);
            }
        }
        let semantic_sha256 = semantic_sha256(self.descriptor, request, &samples, work);
        Ok(ContinentalSurfaceWindow {
            descriptor: self.descriptor,
            request,
            samples,
            work,
            semantic_sha256,
        })
    }

    fn realize(
        &self,
        world_x: i32,
        world_z: i32,
        plan: &LandscapePlanSample,
    ) -> ContinentalSurfaceSample {
        let macro_roll = self.fields.macro_roll.sample(world_x, world_z);
        let ridge_form = self.fields.ridge_form.sample(world_x, world_z);
        let basin_form = self.fields.basin_form.sample(world_x, world_z);
        let shore_form = self.fields.shore_form.sample(world_x, world_z);
        let local_form = self.fields.local_form.sample(world_x, world_z);
        let land = f64::from(plan.land_weight);
        let coast_weight = 1.0 - smoothstep(0.54, 0.92, land);

        let province = plan.province;
        let ecoregion = plan.ecoregion;
        let mosaic = plan.mosaic;
        let rolling_weight = province.map_or(0.0, |province| match province.kind {
            PhysiographicProvinceKind::RollingHills => 0.92,
            PhysiographicProvinceKind::QuietBench => 0.82,
            PhysiographicProvinceKind::WoodedUpland => 0.48,
            PhysiographicProvinceKind::RiverLowland => 0.42,
            PhysiographicProvinceKind::LakeBasin => 0.36,
            PhysiographicProvinceKind::RockyRidge => 0.18,
        });
        let water_route = mosaic
            .and_then(|mosaic| mosaic.corridor_kind.map(|kind| (kind, mosaic.corridor)))
            .map_or(0.0, |(kind, weight)| match kind {
                HabitatRouteKind::RiparianSpine | HabitatRouteKind::WetlandChain => {
                    f64::from(weight)
                }
                HabitatRouteKind::WoodlandPass | HabitatRouteKind::OpenRangeLink => 0.0,
            });
        let wetland = mosaic.map_or(0.0, |mosaic| f64::from(mosaic.wetland));
        let major_water = province.map_or(0.0, |province| f64::from(province.major_water));
        let fluvial_weight = (water_route.max(wetland * 0.78).max(province.map_or(
            0.0,
            |province| match province.kind {
                PhysiographicProvinceKind::RiverLowland => 0.72,
                PhysiographicProvinceKind::LakeBasin => 0.86,
                _ => major_water * 0.36,
            },
        )))
        .clamp(0.0, 1.0);
        let upland_weight = province.map_or(0.0, |province| {
            let kind = match province.kind {
                PhysiographicProvinceKind::RockyRidge => 1.0,
                PhysiographicProvinceKind::WoodedUpland => 0.72,
                PhysiographicProvinceKind::RollingHills => 0.34,
                PhysiographicProvinceKind::QuietBench => 0.24,
                _ => 0.10,
            };
            kind * (0.62 + f64::from(province.core_weight) * 0.38)
        });
        let arid_weight = 0.0;
        let family_weights = normalize_weights([
            coast_weight,
            rolling_weight * land,
            fluvial_weight * land,
            upland_weight * land,
            arid_weight,
        ]);
        let dominant_family = dominant_family(family_weights);

        let inland = (f64::from(plan.inland_distance_blocks).max(0.0) / 18_000.0).clamp(0.0, 1.0);
        let continental_height = if land < 0.5 {
            -7.0 - (1.0 - land).powi(2) * 31.0 + shore_form * 1.8
        } else {
            4.0 + inland * 15.0 + macro_roll * (3.0 + inland * 2.5)
        };
        let province_core = province.map_or(0.0, |province| f64::from(province.core_weight));
        let province_amplitude = 0.30 + smoothstep(0.0, 0.72, province_core) * 0.70;
        let rolling_height = macro_roll * 7.0 + ridge_form * 2.5;
        let upland_height = 20.0 + ridge_form.abs().powf(1.35) * 30.0 + macro_roll * 7.0;
        let lowland_height = macro_roll * 3.0 - unit_field(basin_form) * 4.0;
        let province_height = province.map_or(0.0, |province| {
            let height = match province.kind {
                PhysiographicProvinceKind::RiverLowland => lowland_height,
                PhysiographicProvinceKind::LakeBasin => lowland_height - 2.0,
                PhysiographicProvinceKind::RollingHills => rolling_height,
                PhysiographicProvinceKind::WoodedUpland => {
                    rolling_height * 0.45 + upland_height * 0.55
                }
                PhysiographicProvinceKind::RockyRidge => upland_height,
                PhysiographicProvinceKind::QuietBench => rolling_height * 0.55 + 5.0,
            };
            height * province_amplitude
        });

        let clearing = mosaic.map_or(0.0, |mosaic| f64::from(mosaic.clearing_core));
        let openness = mosaic.map_or(0.0, |mosaic| f64::from(mosaic.openness));
        let forest_core = mosaic.map_or(0.0, |mosaic| f64::from(mosaic.forest_core));
        let route = mosaic.map_or(0.0, |mosaic| f64::from(mosaic.corridor));
        let local_amplitude = 1.2 + upland_weight * 5.5 + rolling_weight * 1.8;
        let quieting = 1.0 - clearing * 0.58 - smoothstep(0.72, 1.0, openness) * 0.18;
        let local_height = local_form * local_amplitude * quieting.clamp(0.28, 1.0);

        let mut hydrologic_height = 0.0;
        let mut water_level_y = None;
        let mut water_kind = ContinentalSurfaceWaterKind::None;
        let sea_level = f64::from(MCLONE_OVERWORLD_SEA_LEVEL);
        let land_surface = sea_level + continental_height + province_height + local_height;
        let ocean_floor = sea_level + continental_height;
        let coast_blend = smoothstep(0.42, 0.62, land);
        let mut solid_surface_y = lerp(ocean_floor, land_surface, coast_blend);

        if land < 0.54 && solid_surface_y < sea_level {
            water_level_y = Some(sea_level);
            water_kind = ContinentalSurfaceWaterKind::Ocean;
        }

        if let Some(province) = province {
            if province.kind == PhysiographicProvinceKind::LakeBasin {
                let lake_level = sea_level + 2.0 + f64::from((province.id.hash >> 17) as u8 % 5);
                let lake_shape = smoothstep(
                    0.54,
                    0.78,
                    major_water * 0.64 + unit_field(basin_form) * 0.36,
                ) * smoothstep(0.04, 0.48, province_core);
                if lake_shape > 0.42 {
                    let lake_bed = lake_level - 3.0 - lake_shape * 5.0;
                    if lake_bed < solid_surface_y {
                        hydrologic_height += lake_bed - solid_surface_y;
                        solid_surface_y = lake_bed;
                    }
                    water_level_y = Some(lake_level);
                    water_kind = ContinentalSurfaceWaterKind::Lake;
                }
            }
        }

        let river_channel = smoothstep(0.58, 0.90, water_route) * (0.72 + wetland * 0.28);
        if river_channel > 0.0 {
            let river_level = river_water_level(plan, self.descriptor.topology);
            let river_bed = river_level - 2.0 - river_channel * 4.0;
            if river_bed < solid_surface_y {
                hydrologic_height += river_bed - solid_surface_y;
                solid_surface_y = river_bed;
            }
            if river_channel > 0.38 {
                water_level_y = Some(river_level);
                water_kind = ContinentalSurfaceWaterKind::River;
            }
        } else if wetland > 0.58 && unit_field(basin_form) > 0.64 {
            let pool_level = solid_surface_y.floor() + 1.0;
            solid_surface_y = solid_surface_y.min(pool_level - 1.5);
            water_level_y = Some(pool_level);
            water_kind = ContinentalSurfaceWaterKind::WetlandPool;
        }

        let display_surface_y =
            water_level_y.map_or(solid_surface_y, |water| water.max(solid_surface_y));
        let substrate = surface_substrate(
            water_kind,
            dominant_family,
            upland_weight,
            ridge_form,
            wetland,
            openness,
        );
        let forest_edge = (forest_core * (1.0 - forest_core) * 4.0)
            .max(ecoregion.map_or(0.0, |ecoregion| f64::from(ecoregion.transition_weight)))
            .clamp(0.0, 1.0);
        let (temperature, moisture) = ecoregion.map_or((0.5, 0.5), |ecoregion| {
            (
                f64::from(ecoregion.temperature),
                f64::from(ecoregion.moisture),
            )
        });

        ContinentalSurfaceSample {
            world_x,
            world_z,
            continent_id: plan.continent.map(|continent| continent.id),
            province_id: province.map(|province| province.id),
            ecoregion_id: ecoregion.map(|ecoregion| ecoregion.id),
            clearing_id: mosaic.and_then(|mosaic| mosaic.clearing_id),
            route_id: mosaic.and_then(|mosaic| mosaic.corridor_id),
            family_weights: family_weights.map(|weight| weight as f32),
            dominant_family,
            continental_height: continental_height as f32,
            province_height: province_height as f32,
            hydrologic_height: hydrologic_height as f32,
            local_height: local_height as f32,
            solid_surface_y: solid_surface_y as f32,
            display_surface_y: display_surface_y as f32,
            water_level_y: water_level_y.map(|water| water as f32),
            water_kind,
            substrate,
            openness: openness as f32,
            forest_core: forest_core as f32,
            forest_edge: forest_edge as f32,
            wetland: wetland as f32,
            clearing: clearing as f32,
            route: route as f32,
            temperature: temperature as f32,
            moisture: moisture as f32,
            aridity: (1.0 - moisture) as f32,
            leeward_exposure: 0.0,
        }
    }
}

fn validate_window(
    request: ContinentalSurfaceWindowRequest,
) -> Result<(), ContinentalEcoregionError> {
    if request.width_samples == 0 || request.depth_samples == 0 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "surface sample dimensions must be non-zero",
        ));
    }
    if request.step_blocks == 0 {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "surface sample step must be non-zero",
        ));
    }
    let sample_count = (request.width_samples as usize)
        .checked_mul(request.depth_samples as usize)
        .ok_or(ContinentalEcoregionError::CoordinateOverflow)?;
    if sample_count > CONTINENTAL_SURFACE_MAX_WINDOW_SAMPLES {
        return Err(ContinentalEcoregionError::InvalidWindow(
            "surface sample count exceeds the bounded cap",
        ));
    }
    let _ = LandscapeWindowRequest::new(
        request.min_x,
        request.min_z,
        request.width_samples,
        request.depth_samples,
        request.step_blocks,
        LandscapePlanDetail::Mosaic,
    );
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

fn normalize_weights(
    mut weights: [f64; CONTINENTAL_SURFACE_FAMILY_COUNT],
) -> [f64; CONTINENTAL_SURFACE_FAMILY_COUNT] {
    let total: f64 = weights.iter().sum();
    if total <= f64::EPSILON {
        weights[TerrainCharacterFamily::RollingInterior as usize] = 1.0;
        return weights;
    }
    for weight in &mut weights {
        *weight /= total;
    }
    weights
}

fn dominant_family(weights: [f64; CONTINENTAL_SURFACE_FAMILY_COUNT]) -> TerrainCharacterFamily {
    let mut best = (weights[0], TerrainCharacterFamily::ALL[0]);
    for (index, family) in TerrainCharacterFamily::ALL.into_iter().enumerate().skip(1) {
        if weights[index] > best.0 {
            best = (weights[index], family);
        }
    }
    best.1
}

fn river_water_level(plan: &LandscapePlanSample, topology: ContinentalEcoregionTopology) -> f64 {
    let sea_level = f64::from(MCLONE_OVERWORLD_SEA_LEVEL);
    let Some(continent) = plan.continent else {
        return sea_level;
    };
    let canonical_x = match topology {
        ContinentalEcoregionTopology::Plane => plan.world_x,
        ContinentalEcoregionTopology::CylinderX { period_blocks } => {
            plan.world_x.rem_euclid(period_blocks)
        }
    };
    let dx = (i64::from(canonical_x) - continent.center_x) as f64;
    let dz = (i64::from(plan.world_z) - continent.center_z) as f64;
    let along = dx * f64::from(continent.axis_x) + dz * f64::from(continent.axis_z);
    (sea_level + 8.0 - along / 5_500.0).clamp(sea_level + 1.0, sea_level + 15.0)
}

fn surface_substrate(
    water: ContinentalSurfaceWaterKind,
    family: TerrainCharacterFamily,
    upland: f64,
    ridge_form: f64,
    wetland: f64,
    openness: f64,
) -> ContinentalSurfaceSubstrate {
    match water {
        ContinentalSurfaceWaterKind::Ocean => {
            if ridge_form > 0.36 {
                ContinentalSurfaceSubstrate::Gravel
            } else {
                ContinentalSurfaceSubstrate::Sand
            }
        }
        ContinentalSurfaceWaterKind::Lake | ContinentalSurfaceWaterKind::River => {
            if wetland > 0.58 {
                ContinentalSurfaceSubstrate::Sand
            } else {
                ContinentalSurfaceSubstrate::Gravel
            }
        }
        ContinentalSurfaceWaterKind::WetlandPool => ContinentalSurfaceSubstrate::CoarseSoil,
        ContinentalSurfaceWaterKind::None
            if family == TerrainCharacterFamily::UplandAndEscarpment
                && (upland > 0.68 || ridge_form > 0.54) =>
        {
            ContinentalSurfaceSubstrate::Stone
        }
        ContinentalSurfaceWaterKind::None if openness > 0.82 && ridge_form > 0.34 => {
            ContinentalSurfaceSubstrate::CoarseSoil
        }
        ContinentalSurfaceWaterKind::None => ContinentalSurfaceSubstrate::Grass,
    }
}

fn unit_field(value: f64) -> f64 {
    (value * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn smoothstep(low: f64, high: f64, value: f64) -> f64 {
    let unit = ((value - low) / (high - low)).clamp(0.0, 1.0);
    unit * unit * (3.0 - 2.0 * unit)
}

fn lerp(left: f64, right: f64, weight: f64) -> f64 {
    left + (right - left) * weight
}

fn semantic_sha256(
    descriptor: ContinentalEcoregionDescriptor,
    request: ContinentalSurfaceWindowRequest,
    samples: &[ContinentalSurfaceSample],
    work: ContinentalSurfaceConstructionCounts,
) -> String {
    let mut digest = Sha256::new();
    digest.update(CONTINENTAL_SURFACE_SCHEMA_REVISION.as_bytes());
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
    for sample in samples {
        digest.update(sample.world_x.to_le_bytes());
        digest.update(sample.world_z.to_le_bytes());
        for id in [
            sample.continent_id,
            sample.province_id,
            sample.ecoregion_id,
            sample.clearing_id,
            sample.route_id,
        ] {
            digest.update(id.map_or(0, |id| id.hash).to_le_bytes());
        }
        for weight in sample.family_weights {
            digest.update(weight.to_bits().to_le_bytes());
        }
        digest.update([sample.dominant_family as u8]);
        for value in [
            sample.continental_height,
            sample.province_height,
            sample.hydrologic_height,
            sample.local_height,
            sample.solid_surface_y,
            sample.display_surface_y,
            sample.water_level_y.unwrap_or(f32::NAN),
            sample.openness,
            sample.forest_core,
            sample.forest_edge,
            sample.wetland,
            sample.clearing,
            sample.route,
            sample.temperature,
            sample.moisture,
            sample.aridity,
            sample.leeward_exposure,
        ] {
            digest.update(value.to_bits().to_le_bytes());
        }
        digest.update([sample.water_kind as u8, sample.substrate as u8]);
    }
    for value in [
        work.requested_samples,
        work.continental_owner_evaluations,
        work.province_owner_evaluations,
        work.ecoregion_owner_evaluations,
        work.mosaic_owner_evaluations,
        work.plan_field_evaluations,
        work.surface_field_evaluations,
        work.exact_chunks,
        work.density_volumes,
        work.feature_batches,
    ] {
        digest.update(value.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_queries_match_bounded_windows_and_construct_no_exact_work() {
        let surface =
            ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(12_345)).unwrap();
        let request = ContinentalSurfaceWindowRequest::new(-8_192, -4_096, 17, 13, 512);
        let window = surface.query_window(request).unwrap();
        assert_eq!(window.samples.len(), 221);
        assert_eq!(window.work.requested_samples, 221);
        assert_eq!(window.work.surface_field_evaluations, 221 * 5);
        assert_eq!(window.work.exact_chunks, 0);
        assert_eq!(window.work.density_volumes, 0);
        assert_eq!(window.work.feature_batches, 0);
        for sample in &window.samples {
            assert_eq!(
                *sample,
                surface.query_point(sample.world_x, sample.world_z).sample
            );
            let total: f32 = sample.family_weights.iter().sum();
            assert!((total - 1.0).abs() < 1.0e-5);
            assert!(sample.solid_surface_y.is_finite());
            assert!(sample.display_surface_y.is_finite());
        }
    }

    #[test]
    fn supported_cylinder_repeats_surface_and_identity_exactly() {
        let period = 196_608;
        let surface = ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::new(
            -98_765,
            ContinentalEcoregionTopology::cylinder_x(period),
        ))
        .unwrap();
        for (x, z) in [(-88_000, -31_000), (-1, 0), (37_222, 81_903)] {
            let base = surface.query_point(x, z).sample;
            let lifted = surface.query_point(x + period, z).sample;
            assert_eq!(base.world_z, lifted.world_z);
            assert_eq!(base.solid_surface_y, lifted.solid_surface_y);
            assert_eq!(base.display_surface_y, lifted.display_surface_y);
            assert_eq!(base.family_weights, lifted.family_weights);
            assert_eq!(base.water_kind, lifted.water_kind);
            assert_eq!(base.substrate, lifted.substrate);
            assert_eq!(base.continent_id, lifted.continent_id);
            assert_eq!(base.province_id, lifted.province_id);
            assert_eq!(base.ecoregion_id, lifted.ecoregion_id);
        }
    }

    #[test]
    fn temperate_surface_exercises_four_character_families_and_flat_water() {
        let surface =
            ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(12_345)).unwrap();
        let window = surface
            .query_window(ContinentalSurfaceWindowRequest::new(
                -65_536, -65_536, 257, 257, 512,
            ))
            .unwrap();
        let mut seen = [false; CONTINENTAL_SURFACE_FAMILY_COUNT];
        let mut water = 0_u32;
        for sample in &window.samples {
            seen[sample.dominant_family as usize] = true;
            if let Some(level) = sample.water_level_y {
                water += 1;
                assert!(level.is_finite());
                assert!(sample.display_surface_y >= sample.solid_surface_y);
            }
        }
        assert!(seen[TerrainCharacterFamily::CoastAndHeadland as usize]);
        assert!(seen[TerrainCharacterFamily::RollingInterior as usize]);
        assert!(seen[TerrainCharacterFamily::FluvialAndWetland as usize]);
        assert!(seen[TerrainCharacterFamily::UplandAndEscarpment as usize]);
        assert!(!seen[TerrainCharacterFamily::AridRainShadow as usize]);
        assert!(water > 100);
    }
}
