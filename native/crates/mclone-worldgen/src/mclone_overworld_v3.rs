//! Shared terrain source for the experimental Mclone Overworld V3.
//!
//! V3 deliberately does not call the V2 continental surface evaluator. It
//! keeps a cheap broad field spine, then evaluates a bounded neighborhood of
//! compact lived-scale landform primitives. Exact chunks and procedural LOD
//! consume the same point/window contract; requested spacing removes only
//! subordinate frequencies.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    block::{COARSE_DIRT, GRASS_BLOCK, GRAVEL, RawBlockId, SAND, SNOW_BLOCK, STONE},
    noise::{GradientNoise2d, SeedDomain},
};

pub const MCLONE_OVERWORLD_V3_SCHEMA_REVISION: &str = "mclone-overworld-v3-terrain-v1";
pub const MCLONE_OVERWORLD_V3_SEA_LEVEL: f32 = 63.0;
pub const MCLONE_OVERWORLD_V3_MAX_WINDOW_SAMPLES: usize = 262_144;

const LANDFORM_CELL_BLOCKS: i32 = 8_192;
const LANDFORM_OWNER_RADIUS: i32 = 1;

const CONTINENT_DOMAIN: SeedDomain = SeedDomain::new(0x7633_636f_6e74_3031);
const MACRO_DOMAIN: SeedDomain = SeedDomain::new(0x7633_6d61_6372_3031);
const ROLLING_DOMAIN: SeedDomain = SeedDomain::new(0x7633_726f_6c6c_3031);
const CLEARING_DOMAIN: SeedDomain = SeedDomain::new(0x7633_636c_6561_7231);
const LOCAL_DOMAIN: SeedDomain = SeedDomain::new(0x7633_6c6f_6361_6c31);
const WALKING_DOMAIN: SeedDomain = SeedDomain::new(0x7633_7761_6c6b_3031);
const MICRO_DOMAIN: SeedDomain = SeedDomain::new(0x7633_6d69_6372_3031);
const MOISTURE_DOMAIN: SeedDomain = SeedDomain::new(0x7633_6d6f_6973_7431);
const LANDFORM_HASH_DOMAIN: u64 = 0x7633_6c61_6e64_6631;

const DIRECTIONS: [(f64, f64); 16] = [
    (1.0, 0.0),
    (0.923_879_532_5, 0.382_683_432_4),
    (
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ),
    (0.382_683_432_4, 0.923_879_532_5),
    (0.0, 1.0),
    (-0.382_683_432_4, 0.923_879_532_5),
    (
        -std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ),
    (-0.923_879_532_5, 0.382_683_432_4),
    (-1.0, 0.0),
    (-0.923_879_532_5, -0.382_683_432_4),
    (
        -std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
    ),
    (-0.382_683_432_4, -0.923_879_532_5),
    (0.0, -1.0),
    (0.382_683_432_4, -0.923_879_532_5),
    (
        std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
    ),
    (0.923_879_532_5, -0.382_683_432_4),
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum V3LandformKind {
    Range = 0,
    Plateau = 1,
    Basin = 2,
    RollingConnector = 3,
    Plain = 4,
}

impl V3LandformKind {
    pub const ALL: [Self; 5] = [
        Self::Range,
        Self::Plateau,
        Self::Basin,
        Self::RollingConnector,
        Self::Plain,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Range => "range",
            Self::Plateau => "plateau",
            Self::Basin => "basin",
            Self::RollingConnector => "rolling-connector",
            Self::Plain => "plain",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3LandformId {
    pub owner_x: i32,
    pub owner_z: i32,
    pub hash: u64,
    pub kind: V3LandformKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum V3SurfaceSubstrate {
    Grass = 0,
    CoarseSoil = 1,
    Stone = 2,
    Sand = 3,
    Gravel = 4,
    Snow = 5,
}

impl V3SurfaceSubstrate {
    pub const fn block_id(self) -> RawBlockId {
        match self {
            Self::Grass => GRASS_BLOCK,
            Self::CoarseSoil => COARSE_DIRT,
            Self::Stone => STONE,
            Self::Sand => SAND,
            Self::Gravel => GRAVEL,
            Self::Snow => SNOW_BLOCK,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum V3WaterKind {
    None = 0,
    Ocean = 1,
    BasinLake = 2,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3TerrainWork {
    pub requested_samples: u64,
    pub landform_owner_evaluations: u64,
    pub field_evaluations: u64,
    pub exact_chunks: u64,
}

impl V3TerrainWork {
    fn add_assign(&mut self, other: Self) {
        self.requested_samples += other.requested_samples;
        self.landform_owner_evaluations += other.landform_owner_evaluations;
        self.field_evaluations += other.field_evaluations;
        self.exact_chunks += other.exact_chunks;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3TerrainSample {
    pub world_x: i32,
    pub world_z: i32,
    pub landform_id: Option<V3LandformId>,
    pub dominant_landform: V3LandformKind,
    pub landform_weights: [f32; 5],
    pub land_weight: f32,
    pub continental_height: f32,
    pub landform_height: f32,
    pub local_height: f32,
    pub solid_surface_y: f32,
    pub display_surface_y: f32,
    pub water_level_y: Option<f32>,
    pub water_kind: V3WaterKind,
    pub substrate: V3SurfaceSubstrate,
    pub range_strength: f32,
    pub high_axis: f32,
    pub saddle: f32,
    pub plateau: f32,
    pub escarpment: f32,
    pub basin: f32,
    pub valley: f32,
    pub rolling: f32,
    pub clearing: f32,
    pub openness: f32,
    pub forest_opportunity: f32,
    pub moisture: f32,
}

impl V3TerrainSample {
    pub fn is_water(self) -> bool {
        self.water_level_y
            .is_some_and(|water| water > self.solid_surface_y)
    }

    pub const fn visible_material(self) -> RawBlockId {
        self.substrate.block_id()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3TerrainPointQuery {
    pub sample: V3TerrainSample,
    pub work: V3TerrainWork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3TerrainWindowRequest {
    pub min_x: i32,
    pub min_z: i32,
    pub width_samples: u32,
    pub depth_samples: u32,
    pub step_blocks: u32,
}

impl V3TerrainWindowRequest {
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
pub struct V3TerrainWindow {
    pub seed: i64,
    pub request: V3TerrainWindowRequest,
    pub samples: Vec<V3TerrainSample>,
    pub work: V3TerrainWork,
    pub semantic_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3MountainReviewSite {
    pub center_x: i32,
    pub center_z: i32,
    pub landform_id: V3LandformId,
    pub fitness_score: f32,
    pub minimum_surface_y: f32,
    pub maximum_surface_y: f32,
    pub relief_blocks: f32,
    pub maximum_range_strength: f32,
    pub maximum_high_axis: f32,
    pub maximum_saddle: f32,
    pub maximum_valley: f32,
    pub maximum_openness: f32,
}

#[derive(Clone, Copy, Debug)]
struct V3Fields {
    continent: GradientNoise2d,
    macro_form: GradientNoise2d,
    rolling: GradientNoise2d,
    clearing: GradientNoise2d,
    local: GradientNoise2d,
    walking: GradientNoise2d,
    micro: GradientNoise2d,
    moisture: GradientNoise2d,
}

impl V3Fields {
    fn new(seed: i64) -> Self {
        Self {
            continent: GradientNoise2d::new(seed, CONTINENT_DOMAIN, 32_768),
            macro_form: GradientNoise2d::new(seed, MACRO_DOMAIN, 8_192),
            rolling: GradientNoise2d::new(seed, ROLLING_DOMAIN, 768),
            clearing: GradientNoise2d::new(seed, CLEARING_DOMAIN, 1_024),
            local: GradientNoise2d::new(seed, LOCAL_DOMAIN, 256),
            walking: GradientNoise2d::new(seed, WALKING_DOMAIN, 64),
            micro: GradientNoise2d::new(seed, MICRO_DOMAIN, 16),
            moisture: GradientNoise2d::new(seed, MOISTURE_DOMAIN, 4_096),
        }
    }
}

#[derive(Clone, Debug)]
pub struct McloneOverworldV3TerrainPlan {
    seed: i64,
    fields: V3Fields,
}

impl McloneOverworldV3TerrainPlan {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            fields: V3Fields::new(seed),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn query_point(&self, world_x: i32, world_z: i32) -> V3TerrainPointQuery {
        V3TerrainPointQuery {
            sample: self.realize(world_x, world_z, 1),
            work: V3TerrainWork {
                requested_samples: 1,
                landform_owner_evaluations: owner_evaluations_per_sample(),
                field_evaluations: 8,
                exact_chunks: 0,
            },
        }
    }

    pub fn query_window(&self, request: V3TerrainWindowRequest) -> Result<V3TerrainWindow, String> {
        self.query_window_with_detail_spacing(request, 1)
    }

    pub fn query_lod_window(
        &self,
        request: V3TerrainWindowRequest,
    ) -> Result<V3TerrainWindow, String> {
        self.query_window_with_detail_spacing(request, request.step_blocks)
    }

    /// Select one strong mountain-to-lowland review site from a fixed
    /// 131-kiloblock owner corpus. Selection examines ordinary generated facts
    /// and never changes terrain at the returned coordinate.
    pub fn select_mountain_review_site(&self) -> V3MountainReviewSite {
        let mut best = None;
        for owner_z in -8..=8 {
            for owner_x in -8..=8 {
                let feature = landform_feature(self.seed, owner_x, owner_z);
                if feature.id.kind != V3LandformKind::Range {
                    continue;
                }
                let center_x = feature.center_x.round() as i32;
                let center_z = feature.center_z.round() as i32;
                let center = self.query_point(center_x, center_z).sample;
                if center.land_weight < 0.62 {
                    continue;
                }
                let candidate = self.review_site(feature);
                if best.is_none_or(|current: V3MountainReviewSite| {
                    candidate
                        .fitness_score
                        .total_cmp(&current.fitness_score)
                        .is_gt()
                        || (candidate.fitness_score == current.fitness_score
                            && (candidate.center_z, candidate.center_x)
                                < (current.center_z, current.center_x))
                }) {
                    best = Some(candidate);
                }
            }
        }
        best.expect("the fixed V3 review corpus contains a land range")
    }

    fn query_window_with_detail_spacing(
        &self,
        request: V3TerrainWindowRequest,
        detail_spacing: u32,
    ) -> Result<V3TerrainWindow, String> {
        validate_window(request)?;
        let sample_count = (request.width_samples as usize)
            .checked_mul(request.depth_samples as usize)
            .ok_or_else(|| "V3 terrain window sample count overflowed".to_owned())?;
        let mut samples = Vec::with_capacity(sample_count);
        let mut work = V3TerrainWork::default();
        for sample_z in 0..request.depth_samples {
            let world_z = window_coordinate(request.min_z, sample_z, request.step_blocks)?;
            for sample_x in 0..request.width_samples {
                let world_x = window_coordinate(request.min_x, sample_x, request.step_blocks)?;
                samples.push(self.realize(world_x, world_z, detail_spacing));
                work.add_assign(V3TerrainWork {
                    requested_samples: 1,
                    landform_owner_evaluations: owner_evaluations_per_sample(),
                    field_evaluations: 8,
                    exact_chunks: 0,
                });
            }
        }
        let semantic_sha256 = semantic_sha256(self.seed, request, &samples, work);
        Ok(V3TerrainWindow {
            seed: self.seed,
            request,
            samples,
            work,
            semantic_sha256,
        })
    }

    fn realize(&self, world_x: i32, world_z: i32, detail_spacing: u32) -> V3TerrainSample {
        let x = f64::from(world_x);
        let z = f64::from(world_z);
        let continent = self.fields.continent.sample_at(x, z);
        let macro_form = self
            .fields
            .macro_form
            .sample_at(x + continent * 2_400.0, z - continent * 1_700.0);
        let land_weight = smoothstep(-0.34, 0.02, continent + macro_form * 0.22);
        let moisture = unit_field(
            self.fields
                .moisture
                .sample_at(x + macro_form * 680.0, z - continent * 540.0),
        );

        let base_owner_x = world_x.div_euclid(LANDFORM_CELL_BLOCKS);
        let base_owner_z = world_z.div_euclid(LANDFORM_CELL_BLOCKS);
        let mut composition = LandformComposition::default();
        for offset_z in -LANDFORM_OWNER_RADIUS..=LANDFORM_OWNER_RADIUS {
            for offset_x in -LANDFORM_OWNER_RADIUS..=LANDFORM_OWNER_RADIUS {
                let feature =
                    landform_feature(self.seed, base_owner_x + offset_x, base_owner_z + offset_z);
                composition.include(feature.sample(x, z));
            }
        }

        let rolling_detail = lod_detail_weight(detail_spacing, 96.0, 768.0);
        let local_detail = lod_detail_weight(detail_spacing, 24.0, 256.0);
        let walking_detail = lod_detail_weight(detail_spacing, 4.0, 64.0);
        let micro_detail = lod_detail_weight(detail_spacing, 1.0, 16.0);
        let rolling_noise = self.fields.rolling.sample_at(
            x + composition.range_strength * 190.0,
            z - composition.plateau * 170.0,
        ) * rolling_detail;
        let clearing_field = unit_field(
            self.fields
                .clearing
                .sample_at(x + rolling_noise * 230.0, z - macro_form * 310.0),
        );
        let local_noise = self.fields.local.sample_at(x, z) * local_detail;
        let walking_noise = self
            .fields
            .walking
            .sample_at(x + local_noise * 28.0, z - local_noise * 22.0)
            * walking_detail;
        let micro_noise = self
            .fields
            .micro
            .sample_at(x + walking_noise * 8.0, z - walking_noise * 8.0)
            * micro_detail;

        let clearing = (smoothstep(0.54, 0.82, clearing_field)
            * (0.30 + composition.plain * 0.70)
            * (1.0 - composition.high_axis * 0.86)
            * (1.0 - composition.escarpment * 0.64))
            .max(composition.basin * 0.52)
            .clamp(0.0, 1.0);
        let quieting = (composition.plain * 0.76)
            .max(composition.basin * 0.62)
            .max(clearing * 0.88)
            .clamp(0.0, 0.92);

        let continental_height = if land_weight < 0.5 {
            -8.0 - (1.0 - land_weight).powi(2) * 27.0 + macro_form * 2.0
        } else {
            5.0 + land_weight * 9.0 + macro_form * 6.0
        };
        let positive_landform = composition.range_height.max(composition.plateau_height);
        let landform_height = positive_landform - composition.basin_depth;
        let rolling_amplitude = 4.0 + composition.rolling * 9.0;
        let local_amplitude = 2.0
            + composition.range_strength * 8.0
            + composition.escarpment * 5.0
            + composition.rolling * 3.0;
        let walking_amplitude =
            1.8 + composition.range_strength * 5.5 + composition.escarpment * 3.5;
        let local_height = rolling_noise * rolling_amplitude * (1.0 - quieting)
            + local_noise * local_amplitude * (1.0 - quieting * 0.72)
            + walking_noise * walking_amplitude * (1.0 - quieting * 0.60)
            + micro_noise * (0.65 + composition.range_strength * 1.25);

        let sea_level = f64::from(MCLONE_OVERWORLD_V3_SEA_LEVEL);
        let ocean_floor = sea_level + continental_height;
        let land_surface = sea_level + continental_height + landform_height + local_height;
        let coast_blend = smoothstep(0.40, 0.62, land_weight);
        let mut solid_surface_y = lerp(ocean_floor, land_surface, coast_blend);
        let mut water_level_y = None;
        let mut water_kind = V3WaterKind::None;
        if land_weight < 0.50 && solid_surface_y < sea_level {
            water_level_y = Some(sea_level);
            water_kind = V3WaterKind::Ocean;
        } else if composition.basin > 0.72
            && composition.range_strength < 0.32
            && solid_surface_y < sea_level + 7.0
        {
            let lake_level = sea_level + 5.0;
            solid_surface_y = solid_surface_y.min(lake_level - 2.5);
            water_level_y = Some(lake_level);
            water_kind = V3WaterKind::BasinLake;
        }
        solid_surface_y = solid_surface_y.clamp(1.0, 248.0);
        let display_surface_y =
            water_level_y.map_or(solid_surface_y, |water| water.max(solid_surface_y));

        let openness = clearing
            .max(composition.plain * 0.78)
            .max(composition.basin * 0.42)
            .clamp(0.0, 1.0);
        let forest_opportunity = (moisture
            * (1.0 - openness * 0.84)
            * (1.0 - composition.high_axis * 0.55)
            * (1.0 - composition.escarpment * 0.72))
            .clamp(0.0, 1.0);
        let substrate = select_substrate(
            solid_surface_y,
            land_weight,
            water_kind,
            composition,
            openness,
        );
        let weights = composition.weights();
        let dominant_landform = dominant_landform(weights);

        V3TerrainSample {
            world_x,
            world_z,
            landform_id: composition.dominant_id,
            dominant_landform,
            landform_weights: weights.map(|weight| weight as f32),
            land_weight: land_weight as f32,
            continental_height: continental_height as f32,
            landform_height: landform_height as f32,
            local_height: local_height as f32,
            solid_surface_y: solid_surface_y as f32,
            display_surface_y: display_surface_y as f32,
            water_level_y: water_level_y.map(|water| water as f32),
            water_kind,
            substrate,
            range_strength: composition.range_strength as f32,
            high_axis: composition.high_axis as f32,
            saddle: composition.saddle as f32,
            plateau: composition.plateau as f32,
            escarpment: composition.escarpment as f32,
            basin: composition.basin as f32,
            valley: composition.valley as f32,
            rolling: composition.rolling as f32,
            clearing: clearing as f32,
            openness: openness as f32,
            forest_opportunity: forest_opportunity as f32,
            moisture: moisture as f32,
        }
    }

    fn review_site(&self, feature: LandformFeature) -> V3MountainReviewSite {
        let center_x = feature.center_x.round() as i32;
        let center_z = feature.center_z.round() as i32;
        let mut minimum_surface_y = f32::INFINITY;
        let mut maximum_surface_y = f32::NEG_INFINITY;
        let mut maximum_range_strength = 0.0_f32;
        let mut maximum_high_axis = 0.0_f32;
        let mut maximum_saddle = 0.0_f32;
        let mut maximum_valley = 0.0_f32;
        let mut maximum_openness = 0.0_f32;
        for offset_z in -8..=8 {
            for offset_x in -8..=8 {
                let sample = self
                    .query_point(center_x + offset_x * 512, center_z + offset_z * 512)
                    .sample;
                minimum_surface_y = minimum_surface_y.min(sample.solid_surface_y);
                maximum_surface_y = maximum_surface_y.max(sample.solid_surface_y);
                maximum_range_strength = maximum_range_strength.max(sample.range_strength);
                maximum_high_axis = maximum_high_axis.max(sample.high_axis);
                maximum_saddle = maximum_saddle.max(sample.saddle);
                maximum_valley = maximum_valley.max(sample.valley);
                maximum_openness = maximum_openness.max(sample.openness);
            }
        }
        let relief_blocks = maximum_surface_y - minimum_surface_y;
        let fitness_score = relief_blocks
            + maximum_high_axis * 28.0
            + maximum_saddle * 12.0
            + maximum_valley * 18.0
            + maximum_openness * 22.0;
        V3MountainReviewSite {
            center_x,
            center_z,
            landform_id: feature.id,
            fitness_score,
            minimum_surface_y,
            maximum_surface_y,
            relief_blocks,
            maximum_range_strength,
            maximum_high_axis,
            maximum_saddle,
            maximum_valley,
            maximum_openness,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LandformFeature {
    id: V3LandformId,
    center_x: f64,
    center_z: f64,
    axis_x: f64,
    axis_z: f64,
    length: f64,
    width: f64,
    amplitude: f64,
}

impl LandformFeature {
    fn sample(self, world_x: f64, world_z: f64) -> FeatureSample {
        let dx = world_x - self.center_x;
        let dz = world_z - self.center_z;
        let along = dx * self.axis_x + dz * self.axis_z;
        let across = -dx * self.axis_z + dz * self.axis_x;
        match self.id.kind {
            V3LandformKind::Range => self.sample_range(along, across),
            V3LandformKind::Plateau => self.sample_plateau(along, across),
            V3LandformKind::Basin => self.sample_basin(along, across),
            V3LandformKind::RollingConnector => {
                let radial = elliptical_radial(along, across, self.length, self.width);
                let strength = inverse_smoothstep(0.55, 1.10, radial);
                FeatureSample {
                    id: self.id,
                    score: strength * 0.72,
                    rolling: strength,
                    ..Default::default()
                }
            }
            V3LandformKind::Plain => {
                let radial = elliptical_radial(along, across, self.length, self.width);
                let strength = inverse_smoothstep(0.48, 1.08, radial);
                FeatureSample {
                    id: self.id,
                    score: strength * 0.66,
                    plain: strength,
                    ..Default::default()
                }
            }
        }
    }

    fn sample_range(self, along: f64, across: f64) -> FeatureSample {
        let side_scale = if across >= 0.0 { 1.18 } else { 0.82 };
        let normalized_along = along / self.length;
        let centerline = triangular_wave(normalized_along * 1.35 + hash_unit(self.id.hash, 41))
            * self.width
            * 0.18;
        let across_distance = (across - centerline).abs() / (self.width * side_scale);
        let length_mask = inverse_smoothstep(0.70, 1.04, normalized_along.abs());
        let axis = inverse_smoothstep(0.16, 0.96, across_distance) * length_mask;
        let shoulder = inverse_smoothstep(0.62, 2.10, across_distance)
            * inverse_smoothstep(0.76, 1.18, normalized_along.abs());
        let peak_wave = 1.0
            - triangular_wave(
                normalized_along * (2.2 + hash_unit(self.id.hash, 17) * 0.9)
                    + hash_unit(self.id.hash, 29),
            )
            .abs();
        let peak_factor = 0.62 + smoothstep(0.12, 0.90, peak_wave) * 0.38;
        let saddle = axis * inverse_smoothstep(0.00, 0.20, peak_wave);
        let height = axis.powf(0.68) * self.amplitude * peak_factor + shoulder * 22.0;
        FeatureSample {
            id: self.id,
            score: axis.max(shoulder * 0.56),
            range_height: height,
            range_strength: shoulder.max(axis),
            high_axis: axis,
            saddle,
            valley: inverse_smoothstep(1.0, 2.35, across_distance) * length_mask * (1.0 - axis),
            ..Default::default()
        }
    }

    fn sample_plateau(self, along: f64, across: f64) -> FeatureSample {
        let radial = elliptical_radial(along, across, self.length, self.width);
        let edge_warp = triangular_wave(radial * 3.0 + hash_unit(self.id.hash, 9)) * 0.055;
        let radial = radial + edge_warp;
        let core = inverse_smoothstep(0.50, 0.66, radial);
        let escarpment = smoothstep(0.48, 0.62, radial) * inverse_smoothstep(0.66, 0.86, radial);
        let bench = smoothstep(0.73, 0.84, radial) * inverse_smoothstep(0.96, 1.12, radial);
        let support = inverse_smoothstep(0.92, 1.34, radial);
        let height = core * self.amplitude + escarpment * self.amplitude * 0.62 + bench * 13.0;
        FeatureSample {
            id: self.id,
            score: core.max(escarpment).max(support * 0.42),
            plateau_height: height,
            plateau: core.max(support * 0.58),
            escarpment,
            valley: support * (1.0 - core) * 0.28,
            ..Default::default()
        }
    }

    fn sample_basin(self, along: f64, across: f64) -> FeatureSample {
        let radial = elliptical_radial(along, across, self.length, self.width);
        let basin = inverse_smoothstep(0.30, 1.02, radial);
        let rim = smoothstep(0.68, 0.88, radial) * inverse_smoothstep(1.02, 1.22, radial);
        FeatureSample {
            id: self.id,
            score: basin.max(rim * 0.72),
            basin_depth: basin * self.amplitude,
            basin,
            valley: basin.max(rim * 0.46),
            rolling: rim * 0.24,
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FeatureSample {
    id: V3LandformId,
    score: f64,
    range_height: f64,
    plateau_height: f64,
    basin_depth: f64,
    range_strength: f64,
    high_axis: f64,
    saddle: f64,
    plateau: f64,
    escarpment: f64,
    basin: f64,
    valley: f64,
    rolling: f64,
    plain: f64,
}

impl Default for FeatureSample {
    fn default() -> Self {
        Self {
            id: V3LandformId {
                owner_x: 0,
                owner_z: 0,
                hash: 0,
                kind: V3LandformKind::Plain,
            },
            score: 0.0,
            range_height: 0.0,
            plateau_height: 0.0,
            basin_depth: 0.0,
            range_strength: 0.0,
            high_axis: 0.0,
            saddle: 0.0,
            plateau: 0.0,
            escarpment: 0.0,
            basin: 0.0,
            valley: 0.0,
            rolling: 0.0,
            plain: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LandformComposition {
    dominant_id: Option<V3LandformId>,
    dominant_score: f64,
    range_height: f64,
    plateau_height: f64,
    basin_depth: f64,
    range_strength: f64,
    high_axis: f64,
    saddle: f64,
    plateau: f64,
    escarpment: f64,
    basin: f64,
    valley: f64,
    rolling: f64,
    plain: f64,
}

impl LandformComposition {
    fn include(&mut self, sample: FeatureSample) {
        if sample.score > self.dominant_score {
            self.dominant_score = sample.score;
            self.dominant_id = Some(sample.id);
        }
        self.range_height = self.range_height.max(sample.range_height);
        self.plateau_height = self.plateau_height.max(sample.plateau_height);
        self.basin_depth = self.basin_depth.max(sample.basin_depth);
        self.range_strength = self.range_strength.max(sample.range_strength);
        self.high_axis = self.high_axis.max(sample.high_axis);
        self.saddle = self.saddle.max(sample.saddle);
        self.plateau = self.plateau.max(sample.plateau);
        self.escarpment = self.escarpment.max(sample.escarpment);
        self.basin = self.basin.max(sample.basin);
        self.valley = self.valley.max(sample.valley);
        self.rolling = self.rolling.max(sample.rolling);
        self.plain = self.plain.max(sample.plain);
    }

    fn weights(self) -> [f64; 5] {
        normalize_weights([
            self.range_strength,
            self.plateau,
            self.basin,
            self.rolling,
            self.plain,
        ])
    }
}

fn landform_feature(seed: i64, owner_x: i32, owner_z: i32) -> LandformFeature {
    let hash = coordinate_hash(seed, owner_x, owner_z);
    let roll = hash % 100;
    let kind = match roll {
        0..=34 => V3LandformKind::Range,
        35..=51 => V3LandformKind::Plateau,
        52..=67 => V3LandformKind::Basin,
        68..=83 => V3LandformKind::RollingConnector,
        _ => V3LandformKind::Plain,
    };
    let center_x = f64::from(
        owner_x
            .saturating_mul(LANDFORM_CELL_BLOCKS)
            .saturating_add(LANDFORM_CELL_BLOCKS / 2),
    ) + signed_hash_unit(hash, 11) * 1_450.0;
    let center_z = f64::from(
        owner_z
            .saturating_mul(LANDFORM_CELL_BLOCKS)
            .saturating_add(LANDFORM_CELL_BLOCKS / 2),
    ) + signed_hash_unit(hash, 27) * 1_450.0;
    let direction = DIRECTIONS[((hash >> 38) & 15) as usize];
    let (length, width, amplitude) = match kind {
        V3LandformKind::Range => (
            3_700.0 + hash_unit(hash, 5) * 1_700.0,
            620.0 + hash_unit(hash, 19) * 520.0,
            92.0 + hash_unit(hash, 33) * 54.0,
        ),
        V3LandformKind::Plateau => (
            2_100.0 + hash_unit(hash, 5) * 1_350.0,
            1_450.0 + hash_unit(hash, 19) * 950.0,
            46.0 + hash_unit(hash, 33) * 38.0,
        ),
        V3LandformKind::Basin => (
            2_500.0 + hash_unit(hash, 5) * 1_500.0,
            1_800.0 + hash_unit(hash, 19) * 1_100.0,
            11.0 + hash_unit(hash, 33) * 15.0,
        ),
        V3LandformKind::RollingConnector | V3LandformKind::Plain => (
            2_800.0 + hash_unit(hash, 5) * 1_250.0,
            2_100.0 + hash_unit(hash, 19) * 1_050.0,
            0.0,
        ),
    };
    LandformFeature {
        id: V3LandformId {
            owner_x,
            owner_z,
            hash,
            kind,
        },
        center_x,
        center_z,
        axis_x: direction.0,
        axis_z: direction.1,
        length,
        width,
        amplitude,
    }
}

fn select_substrate(
    surface_y: f64,
    land_weight: f64,
    water_kind: V3WaterKind,
    composition: LandformComposition,
    openness: f64,
) -> V3SurfaceSubstrate {
    if water_kind == V3WaterKind::Ocean && land_weight > 0.32 {
        V3SurfaceSubstrate::Sand
    } else if water_kind == V3WaterKind::BasinLake {
        V3SurfaceSubstrate::Gravel
    } else if surface_y > 174.0 && composition.range_strength > 0.46 {
        V3SurfaceSubstrate::Snow
    } else if composition.high_axis > 0.52 || composition.escarpment > 0.48 {
        V3SurfaceSubstrate::Stone
    } else if composition.range_strength > 0.30 || composition.plateau > 0.45 {
        V3SurfaceSubstrate::CoarseSoil
    } else if openness > 0.76 {
        V3SurfaceSubstrate::Grass
    } else {
        V3SurfaceSubstrate::Grass
    }
}

fn dominant_landform(weights: [f64; 5]) -> V3LandformKind {
    let (index, _) = weights
        .into_iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap_or((4, 0.0));
    V3LandformKind::ALL[index]
}

fn normalize_weights(mut weights: [f64; 5]) -> [f64; 5] {
    let total = weights.iter().sum::<f64>();
    if total <= f64::EPSILON {
        weights[V3LandformKind::Plain as usize] = 1.0;
        return weights;
    }
    for weight in &mut weights {
        *weight /= total;
    }
    weights
}

fn owner_evaluations_per_sample() -> u64 {
    u64::from((LANDFORM_OWNER_RADIUS * 2 + 1).pow(2) as u32)
}

fn validate_window(request: V3TerrainWindowRequest) -> Result<(), String> {
    if request.width_samples == 0 || request.depth_samples == 0 {
        return Err("V3 terrain window dimensions must be non-zero".to_owned());
    }
    if request.step_blocks == 0 {
        return Err("V3 terrain window step must be non-zero".to_owned());
    }
    let samples = (request.width_samples as usize)
        .checked_mul(request.depth_samples as usize)
        .ok_or_else(|| "V3 terrain window sample count overflowed".to_owned())?;
    if samples > MCLONE_OVERWORLD_V3_MAX_WINDOW_SAMPLES {
        return Err(format!(
            "V3 terrain window requested {samples} samples; maximum is {MCLONE_OVERWORLD_V3_MAX_WINDOW_SAMPLES}"
        ));
    }
    let _ = window_coordinate(
        request.min_x,
        request.width_samples - 1,
        request.step_blocks,
    )?;
    let _ = window_coordinate(
        request.min_z,
        request.depth_samples - 1,
        request.step_blocks,
    )?;
    Ok(())
}

fn window_coordinate(origin: i32, index: u32, step: u32) -> Result<i32, String> {
    i64::from(origin)
        .checked_add(i64::from(index) * i64::from(step))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| "V3 terrain window coordinate overflowed i32".to_owned())
}

fn semantic_sha256(
    seed: i64,
    request: V3TerrainWindowRequest,
    samples: &[V3TerrainSample],
    work: V3TerrainWork,
) -> String {
    let mut digest = Sha256::new();
    digest.update(MCLONE_OVERWORLD_V3_SCHEMA_REVISION.as_bytes());
    digest.update(seed.to_le_bytes());
    digest.update(request.min_x.to_le_bytes());
    digest.update(request.min_z.to_le_bytes());
    digest.update(request.width_samples.to_le_bytes());
    digest.update(request.depth_samples.to_le_bytes());
    digest.update(request.step_blocks.to_le_bytes());
    for sample in samples {
        digest.update(sample.world_x.to_le_bytes());
        digest.update(sample.world_z.to_le_bytes());
        if let Some(id) = sample.landform_id {
            digest.update([1, id.kind as u8]);
            digest.update(id.owner_x.to_le_bytes());
            digest.update(id.owner_z.to_le_bytes());
            digest.update(id.hash.to_le_bytes());
        } else {
            digest.update([0, u8::MAX]);
        }
        digest.update([
            sample.dominant_landform as u8,
            sample.water_kind as u8,
            sample.substrate as u8,
        ]);
        for value in sample.landform_weights.into_iter().chain([
            sample.land_weight,
            sample.continental_height,
            sample.landform_height,
            sample.local_height,
            sample.solid_surface_y,
            sample.display_surface_y,
            sample.water_level_y.unwrap_or(f32::NAN),
            sample.range_strength,
            sample.high_axis,
            sample.saddle,
            sample.plateau,
            sample.escarpment,
            sample.basin,
            sample.valley,
            sample.rolling,
            sample.clearing,
            sample.openness,
            sample.forest_opportunity,
            sample.moisture,
        ]) {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    digest.update(work.requested_samples.to_le_bytes());
    digest.update(work.landform_owner_evaluations.to_le_bytes());
    digest.update(work.field_evaluations.to_le_bytes());
    digest.update(work.exact_chunks.to_le_bytes());
    format!("{:x}", digest.finalize())
}

fn coordinate_hash(seed: i64, owner_x: i32, owner_z: i32) -> u64 {
    let value = (seed as u64)
        ^ LANDFORM_HASH_DOMAIN
        ^ (owner_x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (owner_z as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    stable_mix64(value)
}

fn stable_mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn hash_unit(hash: u64, rotation: u32) -> f64 {
    let value = hash.rotate_left(rotation) >> 11;
    value as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn signed_hash_unit(hash: u64, rotation: u32) -> f64 {
    hash_unit(hash, rotation) * 2.0 - 1.0
}

fn elliptical_radial(along: f64, across: f64, length: f64, width: f64) -> f64 {
    ((along / length).powi(2) + (across / width).powi(2)).sqrt()
}

fn triangular_wave(value: f64) -> f64 {
    let phase = value - value.floor();
    1.0 - (phase * 2.0 - 1.0).abs() * 2.0
}

fn unit_field(value: f64) -> f64 {
    (value * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn smoothstep(low: f64, high: f64, value: f64) -> f64 {
    let value = ((value - low) / (high - low)).clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn inverse_smoothstep(inner: f64, outer: f64, distance: f64) -> f64 {
    1.0 - smoothstep(inner, outer, distance)
}

fn lerp(left: f64, right: f64, weight: f64) -> f64 {
    left + (right - left) * weight
}

fn lod_detail_weight(spacing: u32, full_until: f64, gone_at: f64) -> f64 {
    inverse_smoothstep(full_until, gone_at, f64::from(spacing))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: i64 = 12_345;

    #[test]
    fn point_and_spacing_one_window_are_identical() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        let request = V3TerrainWindowRequest::new(-511, 733, 17, 13, 29);
        let window = plan.query_window(request).unwrap();
        for z in 0..request.depth_samples {
            for x in 0..request.width_samples {
                let world_x = request.min_x + (x * request.step_blocks) as i32;
                let world_z = request.min_z + (z * request.step_blocks) as i32;
                let index = (z * request.width_samples + x) as usize;
                assert_eq!(
                    window.samples[index],
                    plan.query_point(world_x, world_z).sample
                );
            }
        }
    }

    #[test]
    fn repeated_and_reordered_windows_are_stable() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        let request = V3TerrainWindowRequest::new(-16_384, 8_192, 33, 33, 128);
        let first = plan.query_window(request).unwrap();
        let _other = plan
            .query_window(V3TerrainWindowRequest::new(9_000, -27_000, 8, 8, 64))
            .unwrap();
        let second = plan.query_window(request).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn coarse_queries_preserve_major_landform_identity() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        for spacing in [1, 4, 16, 64, 256, 1_024] {
            let request = V3TerrainWindowRequest::new(-32_768, -32_768, 32, 32, spacing);
            let coarse = plan.query_lod_window(request).unwrap();
            for sample in &coarse.samples {
                let exact = plan.query_point(sample.world_x, sample.world_z).sample;
                assert_eq!(sample.landform_id, exact.landform_id);
                assert_eq!(sample.dominant_landform, exact.dominant_landform);
                assert_eq!(sample.water_kind, exact.water_kind);
                assert_eq!(sample.substrate, exact.substrate);
            }
        }
    }

    #[test]
    fn distribution_contains_strong_and_quiet_country() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        let window = plan
            .query_window(V3TerrainWindowRequest::new(
                -65_536, -65_536, 129, 129, 1_024,
            ))
            .unwrap();
        let land = window
            .samples
            .iter()
            .filter(|sample| sample.land_weight > 0.62)
            .collect::<Vec<_>>();
        assert!(land.len() > 4_000, "land samples={}", land.len());
        let strong = land
            .iter()
            .filter(|sample| sample.range_strength > 0.46 || sample.plateau > 0.58)
            .count();
        let quiet = land
            .iter()
            .filter(|sample| sample.openness > 0.62 && sample.range_strength < 0.30)
            .count();
        let high = land
            .iter()
            .filter(|sample| sample.solid_surface_y > 145.0)
            .count();
        assert!(strong > 300, "strong={strong}");
        assert!(quiet > 250, "quiet={quiet}");
        assert!(high > 100, "high={high}");
    }

    #[test]
    fn local_refinement_adds_detail_without_changing_major_silhouette() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        let request = V3TerrainWindowRequest::new(-8_192, -8_192, 65, 65, 256);
        let coarse = plan.query_lod_window(request).unwrap();
        let mut maximum_delta = 0.0_f32;
        let mut detailed_points = 0;
        for sample in &coarse.samples {
            let exact = plan.query_point(sample.world_x, sample.world_z).sample;
            maximum_delta =
                maximum_delta.max((sample.solid_surface_y - exact.solid_surface_y).abs());
            if (sample.local_height - exact.local_height).abs() > 0.1 {
                detailed_points += 1;
            }
        }
        assert!(detailed_points > 500, "detailed_points={detailed_points}");
        assert!(maximum_delta < 20.0, "maximum_delta={maximum_delta}");
    }

    #[test]
    fn invalid_windows_fail_without_allocating() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        assert!(
            plan.query_window(V3TerrainWindowRequest::new(0, 0, 0, 1, 1))
                .is_err()
        );
        assert!(
            plan.query_window(V3TerrainWindowRequest::new(0, 0, 1_024, 1_024, 1))
                .is_err()
        );
        assert!(
            plan.query_window(V3TerrainWindowRequest::new(i32::MAX, 0, 2, 1, 1))
                .is_err()
        );
    }

    #[test]
    fn review_selector_finds_strong_terrain_with_negative_space() {
        let plan = McloneOverworldV3TerrainPlan::new(SEED);
        let site = plan.select_mountain_review_site();
        assert_eq!(site.landform_id.kind, V3LandformKind::Range);
        assert!(site.relief_blocks > 90.0, "site={site:?}");
        assert!(site.maximum_high_axis > 0.70, "site={site:?}");
        assert!(site.maximum_saddle > 0.20, "site={site:?}");
        assert!(site.maximum_valley > 0.35, "site={site:?}");
        assert!(site.maximum_openness > 0.55, "site={site:?}");
        assert_eq!(site, plan.select_mountain_review_site());
    }
}
