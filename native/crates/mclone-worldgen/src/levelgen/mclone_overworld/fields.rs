use mclone_core::{AxisTopology, ChunkPos, HorizontalTopology};

use crate::noise::{GradientNoise2d, SeedDomain, ValueNoise2d};

pub const MCLONE_OVERWORLD_SEA_LEVEL: i32 = 63;
pub const MCLONE_OVERWORLD_FIELD_REVISION: &str = "mclone-overworld-v1-fields-11";
pub const MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS: i32 = 2;
pub const MCLONE_OVERWORLD_PERIOD_BLOCKS: i32 = 6_144;
pub const MCLONE_OVERWORLD_PERIOD_CHUNKS: u32 = 384;

const CONTINENT_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_636f_6e31);
const CONTINENT_MEDIUM_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_636f_6e32);
const CONTINENT_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_636f_6e33);
const RELIEF_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7265_6c31);
const RELIEF_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7265_6c32);
const RELIEF_FINE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7265_6c33);
const RUGGEDNESS_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7275_6731);
const RUGGEDNESS_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7275_6732);
const RIDGE_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7269_6431);
const RIDGE_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7269_6432);
const MOUNTAIN_DETAIL_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6d64_7431);
const MOUNTAIN_DETAIL_FINE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6d64_7432);
const RIVER_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7269_7631);
const RIVER_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7269_7632);
const RIVER_WIDTH_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7269_7633);
const TRIBUTARY_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7472_6231);
const TRIBUTARY_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7472_6232);
const TRIBUTARY_SELECTOR_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7472_6233);
const WETLAND_POOL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_7765_7431);
const OCEAN_BASIN_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6261_7331);
const SEABED_LARGE_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6261_7332);
const SEABED_DETAIL_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6261_7333);

const CONTINENT_LARGE_SCALE: i32 = 2_048;
const CONTINENT_MEDIUM_SCALE: i32 = 1_024;
const CONTINENT_DETAIL_SCALE: i32 = 512;
const RELIEF_LARGE_SCALE: i32 = 384;
const RELIEF_DETAIL_SCALE: i32 = 128;
const RELIEF_FINE_SCALE: i32 = 48;
const RUGGEDNESS_LARGE_SCALE: i32 = 1_536;
const RUGGEDNESS_DETAIL_SCALE: i32 = 512;
const RIDGE_LARGE_SCALE: i32 = 384;
const RIDGE_DETAIL_SCALE: i32 = 128;
const MOUNTAIN_DETAIL_LARGE_SCALE: i32 = 32;
const MOUNTAIN_DETAIL_FINE_SCALE: i32 = 8;
const RIVER_LARGE_SCALE: i32 = 768;
const RIVER_DETAIL_SCALE: i32 = 192;
const RIVER_WIDTH_SCALE: i32 = 384;
const TRIBUTARY_LARGE_SCALE: i32 = 384;
const TRIBUTARY_DETAIL_SCALE: i32 = 128;
const TRIBUTARY_SELECTOR_SCALE: i32 = 768;
const WETLAND_POOL_SCALE: i32 = 96;
const OCEAN_BASIN_SCALE: i32 = 1_536;
const SEABED_LARGE_SCALE: i32 = 384;
const SEABED_DETAIL_SCALE: i32 = 96;
const RIVER_GRADE_SAMPLE_DISTANCE: f64 = 16.0;
const RIVER_MAX_RELEVANT_DISTANCE: f64 = 64.0;
const TRIBUTARY_RISE_BLOCKS: i32 = 4;
const TRIBUTARY_DROP_BOUNDARY_BLOCKS: f64 = 3.0;
const TRIBUTARY_HALF_WIDTH_BLOCKS: f64 = 2.75;
const TRIBUTARY_SOURCE_POOL_RADIUS_BLOCKS: f64 = 5.5;
const TRIBUTARY_BANK_SPAN_BLOCKS: f64 = 7.0;
const RIVER_FALL_HALF_WIDTH_BLOCKS: f64 = 1.75;
const RIVER_ROCK_LIP_RUN_BLOCKS: f64 = 2.0;
const MAX_REGION_SAMPLE_COUNT: usize = 16 * 1024 * 1024;
const SPAWN_SEARCH_RADIUS_CHUNKS: i32 = 128;
const SPAWN_MIN_SURFACE_Y: i32 = MCLONE_OVERWORLD_SEA_LEVEL + 5;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldSamplingTopology {
    #[default]
    Unbounded,
    PeriodicX,
}

impl McloneOverworldSamplingTopology {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unbounded => "plane",
            Self::PeriodicX => "cylinder-x:384",
        }
    }

    pub const fn horizontal_topology(self) -> HorizontalTopology {
        match self {
            Self::Unbounded => HorizontalTopology::UNBOUNDED,
            Self::PeriodicX => HorizontalTopology::cylinder_x(0, MCLONE_OVERWORLD_PERIOD_CHUNKS),
        }
    }

    pub fn from_horizontal_topology(topology: HorizontalTopology) -> Result<Self, String> {
        match topology {
            HorizontalTopology {
                x: AxisTopology::Unbounded,
                z: AxisTopology::Unbounded,
            } => Ok(Self::Unbounded),
            HorizontalTopology {
                x:
                    AxisTopology::Periodic {
                        minimum_chunk: 0,
                        period_chunks: MCLONE_OVERWORLD_PERIOD_CHUNKS,
                    },
                z: AxisTopology::Unbounded,
            } => Ok(Self::PeriodicX),
            _ => Err(format!(
                "mclone-overworld-v1 supports only unbounded topology or cylinder-x:0:{MCLONE_OVERWORLD_PERIOD_CHUNKS}"
            )),
        }
    }

    pub fn canonical_chunk_x(self, chunk_x: i32) -> i32 {
        self.horizontal_topology()
            .x
            .canonical_chunk(chunk_x)
            .expect("Mclone sampling topology has no finite X exclusion")
    }

    pub const fn cache_scope(self) -> u64 {
        match self {
            Self::Unbounded => 0,
            Self::PeriodicX => 1,
        }
    }

    fn value_noise(self, seed: i64, domain: SeedDomain, scale: i32) -> ValueNoise2d {
        match self {
            Self::Unbounded => ValueNoise2d::new(seed, domain, scale),
            Self::PeriodicX => {
                ValueNoise2d::new_periodic_x(seed, domain, scale, MCLONE_OVERWORLD_PERIOD_BLOCKS)
            }
        }
    }

    fn gradient_noise(self, seed: i64, domain: SeedDomain, scale: i32) -> GradientNoise2d {
        match self {
            Self::Unbounded => GradientNoise2d::new(seed, domain, scale),
            Self::PeriodicX => {
                GradientNoise2d::new_periodic_x(seed, domain, scale, MCLONE_OVERWORLD_PERIOD_BLOCKS)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldBathymetrySample {
    pub ocean_interior: f64,
    pub shelf_influence: f64,
    pub shelf_break_influence: f64,
    pub basin_influence: f64,
    pub seabed_relief: f64,
    pub water_depth: i32,
}

impl McloneOverworldBathymetrySample {
    pub const LAND: Self = Self {
        ocean_interior: 0.0,
        shelf_influence: 0.0,
        shelf_break_influence: 0.0,
        basin_influence: 0.0,
        seabed_relief: 0.0,
        water_depth: 0,
    };

    pub const fn floor_y(self) -> i32 {
        MCLONE_OVERWORLD_SEA_LEVEL - self.water_depth
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldWatercourseSample {
    pub distance: f64,
    pub channel_influence: f64,
    pub major_channel_influence: f64,
    pub raised_tributary_influence: f64,
    pub tributary_source_pool_influence: f64,
    pub bank_influence: f64,
    pub half_width: f64,
    pub water_surface_y: i32,
    pub bed_y: i32,
    pub tangent_x: f64,
    pub tangent_z: f64,
    pub flow_x: f64,
    pub flow_z: f64,
    pub grade: f64,
    pub drop_distance: f64,
    pub drop_height: i32,
    pub drop_upper_y: i32,
    pub drop_lower_y: i32,
    pub wetland_influence: f64,
    pub wetland_pool_influence: f64,
}

impl McloneOverworldWatercourseSample {
    pub fn is_channel(self) -> bool {
        self.channel_influence > 0.0
    }

    pub fn is_bank(self) -> bool {
        self.bank_influence > 0.0 && !self.is_channel()
    }

    pub fn is_major_channel(self) -> bool {
        self.major_channel_influence > 0.0
    }

    pub fn is_raised_tributary(self) -> bool {
        self.raised_tributary_influence > 0.0
    }

    pub fn is_tributary_source_pool(self) -> bool {
        self.tributary_source_pool_influence > 0.0
    }

    pub fn is_wetland_pool(self) -> bool {
        self.wetland_pool_influence >= 0.55 && !self.is_channel()
    }

    pub fn is_water(self) -> bool {
        self.is_channel() || self.is_wetland_pool()
    }

    pub fn is_drop_transition(self) -> bool {
        self.drop_height > 0
    }

    pub fn is_fall_column(self) -> bool {
        self.is_channel()
            && self.is_drop_transition()
            && self.drop_distance <= 0.0
            && self.drop_distance >= -RIVER_FALL_HALF_WIDTH_BLOCKS
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldTerrainSample {
    pub continentalness: f64,
    pub relief: f64,
    pub ruggedness: f64,
    pub ridges: f64,
    pub mountain_detail: f64,
    pub bathymetry: McloneOverworldBathymetrySample,
    pub base_surface_y: i32,
    pub watercourse: McloneOverworldWatercourseSample,
    pub surface_y: i32,
}

impl McloneOverworldTerrainSample {
    pub fn mountain_strength(self) -> f64 {
        mountain_strength(self.continentalness, self.ruggedness)
    }

    pub fn exposure(self) -> f64 {
        let altitude = smoothstep((f64::from(self.surface_y - 82) / 48.0).clamp(0.0, 1.0));
        let crest = smoothstep(((self.ridges - 0.35) / 0.65).clamp(0.0, 1.0));
        self.mountain_strength() * (altitude * 0.35 + crest * 0.65)
    }

    pub fn is_mountain_valley(self) -> bool {
        self.surface_y > MCLONE_OVERWORLD_SEA_LEVEL
            && self.mountain_strength() >= 0.35
            && self.ridges <= 0.28
    }

    pub fn is_open_mountain_shoulder(self) -> bool {
        self.surface_y > MCLONE_OVERWORLD_SEA_LEVEL
            && self.mountain_strength() >= 0.15
            && self.ridges >= 0.65
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldLandformSample {
    pub terrain: McloneOverworldTerrainSample,
    pub slope: f64,
}

impl McloneOverworldLandformSample {
    pub fn from_cardinal_samples(
        terrain: McloneOverworldTerrainSample,
        west: McloneOverworldTerrainSample,
        east: McloneOverworldTerrainSample,
        north: McloneOverworldTerrainSample,
        south: McloneOverworldTerrainSample,
    ) -> Self {
        let diameter = f64::from(MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS * 2);
        let gradient_x = f64::from(east.surface_y - west.surface_y) / diameter;
        let gradient_z = f64::from(south.surface_y - north.surface_y) / diameter;
        Self {
            terrain,
            slope: gradient_x.hypot(gradient_z),
        }
    }

    pub fn exposure(self) -> f64 {
        self.terrain.exposure()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneOverworldSampleRegionRequest {
    pub min_x: i32,
    pub min_z: i32,
    pub width: u32,
    pub depth: u32,
    pub step: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneOverworldSampleRegion {
    pub request: McloneOverworldSampleRegionRequest,
    pub samples: Vec<McloneOverworldTerrainSample>,
}

impl McloneOverworldSampleRegion {
    pub fn sample(&self, offset_x: u32, offset_z: u32) -> Option<McloneOverworldTerrainSample> {
        if offset_x >= self.request.width || offset_z >= self.request.depth {
            return None;
        }
        let index = usize::try_from(offset_z).ok()? * usize::try_from(self.request.width).ok()?
            + usize::try_from(offset_x).ok()?;
        self.samples.get(index).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneOverworldSampler {
    continent_large: ValueNoise2d,
    continent_medium: ValueNoise2d,
    continent_detail: ValueNoise2d,
    relief_large: ValueNoise2d,
    relief_detail: ValueNoise2d,
    relief_fine: ValueNoise2d,
    ruggedness_large: ValueNoise2d,
    ruggedness_detail: ValueNoise2d,
    ridge_large: ValueNoise2d,
    ridge_detail: ValueNoise2d,
    mountain_detail_large: GradientNoise2d,
    mountain_detail_fine: GradientNoise2d,
    river_large: GradientNoise2d,
    river_detail: GradientNoise2d,
    river_width: ValueNoise2d,
    tributary_large: GradientNoise2d,
    tributary_detail: GradientNoise2d,
    tributary_selector: ValueNoise2d,
    wetland_pool: GradientNoise2d,
    ocean_basin: GradientNoise2d,
    seabed_large: GradientNoise2d,
    seabed_detail: GradientNoise2d,
}

impl McloneOverworldSampler {
    pub fn new(seed: i64) -> Self {
        Self::new_with_topology(seed, McloneOverworldSamplingTopology::Unbounded)
    }

    pub fn new_with_topology(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        Self {
            continent_large: topology.value_noise(
                seed,
                CONTINENT_LARGE_DOMAIN,
                CONTINENT_LARGE_SCALE,
            ),
            continent_medium: topology.value_noise(
                seed,
                CONTINENT_MEDIUM_DOMAIN,
                CONTINENT_MEDIUM_SCALE,
            ),
            continent_detail: topology.value_noise(
                seed,
                CONTINENT_DETAIL_DOMAIN,
                CONTINENT_DETAIL_SCALE,
            ),
            relief_large: topology.value_noise(seed, RELIEF_LARGE_DOMAIN, RELIEF_LARGE_SCALE),
            relief_detail: topology.value_noise(seed, RELIEF_DETAIL_DOMAIN, RELIEF_DETAIL_SCALE),
            relief_fine: topology.value_noise(seed, RELIEF_FINE_DOMAIN, RELIEF_FINE_SCALE),
            ruggedness_large: topology.value_noise(
                seed,
                RUGGEDNESS_LARGE_DOMAIN,
                RUGGEDNESS_LARGE_SCALE,
            ),
            ruggedness_detail: topology.value_noise(
                seed,
                RUGGEDNESS_DETAIL_DOMAIN,
                RUGGEDNESS_DETAIL_SCALE,
            ),
            ridge_large: topology.value_noise(seed, RIDGE_LARGE_DOMAIN, RIDGE_LARGE_SCALE),
            ridge_detail: topology.value_noise(seed, RIDGE_DETAIL_DOMAIN, RIDGE_DETAIL_SCALE),
            mountain_detail_large: topology.gradient_noise(
                seed,
                MOUNTAIN_DETAIL_LARGE_DOMAIN,
                MOUNTAIN_DETAIL_LARGE_SCALE,
            ),
            mountain_detail_fine: topology.gradient_noise(
                seed,
                MOUNTAIN_DETAIL_FINE_DOMAIN,
                MOUNTAIN_DETAIL_FINE_SCALE,
            ),
            river_large: topology.gradient_noise(seed, RIVER_LARGE_DOMAIN, RIVER_LARGE_SCALE),
            river_detail: topology.gradient_noise(seed, RIVER_DETAIL_DOMAIN, RIVER_DETAIL_SCALE),
            river_width: topology.value_noise(seed, RIVER_WIDTH_DOMAIN, RIVER_WIDTH_SCALE),
            tributary_large: topology.gradient_noise(
                seed,
                TRIBUTARY_LARGE_DOMAIN,
                TRIBUTARY_LARGE_SCALE,
            ),
            tributary_detail: topology.gradient_noise(
                seed,
                TRIBUTARY_DETAIL_DOMAIN,
                TRIBUTARY_DETAIL_SCALE,
            ),
            tributary_selector: topology.value_noise(
                seed,
                TRIBUTARY_SELECTOR_DOMAIN,
                TRIBUTARY_SELECTOR_SCALE,
            ),
            wetland_pool: topology.gradient_noise(seed, WETLAND_POOL_DOMAIN, WETLAND_POOL_SCALE),
            ocean_basin: topology.gradient_noise(seed, OCEAN_BASIN_DOMAIN, OCEAN_BASIN_SCALE),
            seabed_large: topology.gradient_noise(seed, SEABED_LARGE_DOMAIN, SEABED_LARGE_SCALE),
            seabed_detail: topology.gradient_noise(seed, SEABED_DETAIL_DOMAIN, SEABED_DETAIL_SCALE),
        }
    }

    pub fn sample(self, world_x: i32, world_z: i32) -> McloneOverworldTerrainSample {
        let continentalness = (self.continent_large.sample(world_x, world_z) * 0.55
            + self.continent_medium.sample(world_x, world_z) * 0.30
            + self.continent_detail.sample(world_x, world_z) * 0.15)
            .clamp(-1.0, 1.0);
        let relief_large = self.relief_large.sample(world_x, world_z);
        let relief_detail = self.relief_detail.sample(world_x, world_z);
        let relief_fine = self.relief_fine.sample(world_x, world_z);
        let relief =
            (relief_large * 0.50 + relief_detail * 0.30 + relief_fine * 0.20).clamp(-1.0, 1.0);
        let ruggedness_large = self.ruggedness_large.sample(world_x, world_z);
        let ruggedness_detail = self.ruggedness_detail.sample(world_x, world_z);
        let ruggedness = (ruggedness_large * 0.72 + ruggedness_detail * 0.28).clamp(-1.0, 1.0);
        let ridge_source = self.ridge_large.sample(world_x, world_z) * 0.78
            + self.ridge_detail.sample(world_x, world_z) * 0.22;
        let ridge_linear = (1.0 - ridge_source.abs()).clamp(0.0, 1.0);
        let ridges = ridge_linear * ridge_linear;
        let world_x = f64::from(world_x);
        let world_z = f64::from(world_z);
        let large_warp_x = relief_detail * 18.0 + relief_fine * 4.0;
        let large_warp_z = ruggedness_detail * 18.0 - relief_fine * 4.0;
        let fine_warp_x = -ruggedness_detail * 7.0 + relief_detail * 3.0;
        let fine_warp_z = relief_fine * 7.0 + relief_detail * 3.0;
        let mountain_detail = (self
            .mountain_detail_large
            .sample_at(world_x + large_warp_x, world_z + large_warp_z)
            * 0.70
            + self
                .mountain_detail_fine
                .sample_at(world_x + fine_warp_x, world_z + fine_warp_z)
                * 0.30)
            .clamp(-1.0, 1.0);
        let bathymetry = self.sample_bathymetry(world_x as i32, world_z as i32, continentalness);
        let base_surface_y = if continentalness <= 0.0 {
            bathymetry.floor_y()
        } else {
            land_surface_height(continentalness, relief, ruggedness, ridges, mountain_detail)
        };
        let river_warp_x = relief_detail * 72.0 + ruggedness_detail * 24.0;
        let river_warp_z = ruggedness_detail * 72.0 - relief_large * 24.0;
        let river_geometry =
            self.sample_river_geometry(world_x, world_z, river_warp_x, river_warp_z);
        let (watercourse, surface_y) = self.complete_watercourse(
            world_x,
            world_z,
            continentalness,
            ruggedness,
            base_surface_y,
            river_geometry,
        );
        McloneOverworldTerrainSample {
            continentalness,
            relief,
            ruggedness,
            ridges,
            mountain_detail,
            bathymetry,
            base_surface_y,
            watercourse,
            surface_y,
        }
    }

    fn sample_bathymetry(
        self,
        world_x: i32,
        world_z: i32,
        continentalness: f64,
    ) -> McloneOverworldBathymetrySample {
        if continentalness > 0.0 {
            return McloneOverworldBathymetrySample::LAND;
        }
        let basin_selector = self.ocean_basin.sample(world_x, world_z) * 0.5 + 0.5;
        let seabed_relief = (self.seabed_large.sample(world_x, world_z) * 0.68
            + self.seabed_detail.sample(world_x, world_z) * 0.32)
            .clamp(-1.0, 1.0);
        bathymetry_sample(continentalness, basin_selector, seabed_relief)
    }

    fn sample_river_geometry(
        self,
        world_x: f64,
        world_z: f64,
        warp_x: f64,
        warp_z: f64,
    ) -> RiverGeometry {
        let large = self
            .river_large
            .sample_with_derivative(world_x + warp_x, world_z + warp_z);
        let detail = self
            .river_detail
            .sample_with_derivative(world_x + warp_x * 0.35, world_z + warp_z * 0.35);
        let center = (large.0 * 0.82 + detail.0 * 0.18).clamp(-1.0, 1.0);
        let gradient_x = large.1 * 0.82 + detail.1 * 0.18;
        let gradient_z = large.2 * 0.82 + detail.2 * 0.18;
        let gradient_length = gradient_x.hypot(gradient_z).max(1.0 / 2_048.0);
        let signed_distance = (center / gradient_length).clamp(-512.0, 512.0);
        let distance = signed_distance.abs();
        let normal_x = gradient_x / gradient_length;
        let normal_z = gradient_z / gradient_length;
        let mut tangent_x = -gradient_z / gradient_length;
        let mut tangent_z = gradient_x / gradient_length;
        if tangent_z < 0.0 || (tangent_z == 0.0 && tangent_x < 0.0) {
            tangent_x = -tangent_x;
            tangent_z = -tangent_z;
        }
        let width_noise = self.river_width.sample(world_x as i32, world_z as i32) * 0.5 + 0.5;
        RiverGeometry {
            signed_distance,
            distance,
            half_width: 4.5 + width_noise * 4.0,
            normal_x,
            normal_z,
            tangent_x,
            tangent_z,
            width_noise,
            center_x: world_x - normal_x * signed_distance,
            center_z: world_z - normal_z * signed_distance,
        }
    }

    fn sample_river_geometry_at(self, world_x: f64, world_z: f64) -> RiverGeometry {
        let sample_x = world_x.round() as i32;
        let sample_z = world_z.round() as i32;
        let relief_large = self.relief_large.sample(sample_x, sample_z);
        let relief_detail = self.relief_detail.sample(sample_x, sample_z);
        let ruggedness_detail = self.ruggedness_detail.sample(sample_x, sample_z);
        self.sample_river_geometry(
            world_x,
            world_z,
            relief_detail * 72.0 + ruggedness_detail * 24.0,
            ruggedness_detail * 72.0 - relief_large * 24.0,
        )
    }

    pub(super) fn sample_major_river_geometry(
        self,
        world_x: f64,
        world_z: f64,
    ) -> McloneOverworldMajorRiverGeometry {
        let geometry = self.sample_river_geometry_at(world_x, world_z);
        McloneOverworldMajorRiverGeometry {
            signed_distance: geometry.signed_distance,
            distance: geometry.distance,
            half_width: geometry.half_width,
            normal_x: geometry.normal_x,
            normal_z: geometry.normal_z,
            tangent_x: geometry.tangent_x,
            tangent_z: geometry.tangent_z,
            center_x: geometry.center_x,
            center_z: geometry.center_z,
        }
    }

    fn sample_tributary_geometry(self, world_x: f64, world_z: f64) -> RiverGeometry {
        let large = self
            .tributary_large
            .sample_with_derivative(world_x, world_z);
        let detail = self
            .tributary_detail
            .sample_with_derivative(world_x, world_z);
        let center = (large.0 * 0.76 + detail.0 * 0.24).clamp(-1.0, 1.0);
        let gradient_x = large.1 * 0.76 + detail.1 * 0.24;
        let gradient_z = large.2 * 0.76 + detail.2 * 0.24;
        let gradient_length = gradient_x.hypot(gradient_z).max(1.0 / 1_024.0);
        let signed_distance = (center / gradient_length).clamp(-256.0, 256.0);
        let normal_x = gradient_x / gradient_length;
        let normal_z = gradient_z / gradient_length;
        let mut tangent_x = -normal_z;
        let mut tangent_z = normal_x;
        if tangent_z < 0.0 || (tangent_z == 0.0 && tangent_x < 0.0) {
            tangent_x = -tangent_x;
            tangent_z = -tangent_z;
        }
        RiverGeometry {
            signed_distance,
            distance: signed_distance.abs(),
            half_width: TRIBUTARY_HALF_WIDTH_BLOCKS,
            normal_x,
            normal_z,
            tangent_x,
            tangent_z,
            width_noise: 0.0,
            center_x: world_x - normal_x * signed_distance,
            center_z: world_z - normal_z * signed_distance,
        }
    }

    fn solve_tributary_anchor(self, geometry: RiverGeometry) -> Option<TributaryAnchor> {
        // Reconstruct the nearest crossing of the major-river and tributary
        // zero contours from bounded local facts. Every column in the short
        // vignette converges on the same anchor instead of independently
        // accepting or rejecting a tributary as it moves away from the river.
        let origin_x = geometry.center_x;
        let origin_z = geometry.center_z;
        let mut anchor_x = origin_x;
        let mut anchor_z = origin_z;
        for _ in 0..2 {
            let major = self.sample_river_geometry_at(anchor_x, anchor_z);
            let tributary = self.sample_tributary_geometry(anchor_x, anchor_z);
            let determinant =
                major.normal_x * tributary.normal_z - major.normal_z * tributary.normal_x;
            if determinant.abs() < 0.68 {
                return None;
            }
            let delta_x = (-major.signed_distance * tributary.normal_z
                + major.normal_z * tributary.signed_distance)
                / determinant;
            let delta_z = (-major.normal_x * tributary.signed_distance
                + major.signed_distance * tributary.normal_x)
                / determinant;
            if delta_x.hypot(delta_z) > 64.0 {
                return None;
            }
            anchor_x += delta_x;
            anchor_z += delta_z;
        }
        if (anchor_x - origin_x).hypot(anchor_z - origin_z) > 72.0 {
            return None;
        }

        let major = self.sample_river_geometry_at(anchor_x, anchor_z);
        let tributary = self.sample_tributary_geometry(anchor_x, anchor_z);
        let crossing_alignment =
            (tributary.tangent_x * major.normal_x + tributary.tangent_z * major.normal_z).abs();
        if major.distance > 2.0 || tributary.distance > 2.0 || crossing_alignment < 0.68 {
            return None;
        }

        let away_sign =
            if tributary.tangent_x * major.normal_x + tributary.tangent_z * major.normal_z >= 0.0 {
                1.0
            } else {
                -1.0
            };
        Some(TributaryAnchor {
            x: anchor_x,
            z: anchor_z,
            major,
            away_x: tributary.tangent_x * away_sign,
            away_z: tributary.tangent_z * away_sign,
        })
    }

    fn complete_watercourse(
        self,
        world_x: f64,
        world_z: f64,
        continentalness: f64,
        ruggedness: f64,
        base_surface_y: i32,
        geometry: RiverGeometry,
    ) -> (McloneOverworldWatercourseSample, i32) {
        let depth = (2.0 + geometry.half_width * 0.24).round() as i32;
        if continentalness <= 0.0 || geometry.distance > RIVER_MAX_RELEVANT_DISTANCE {
            return inactive_watercourse(geometry, depth, base_surface_y);
        }

        // Major rivers deliberately share the global sea-level source plane.
        // The broad corridor may shape a valley, but it never tries to infer
        // an upstream/downstream elevation sequence from local terrain.
        let offset_x = geometry.tangent_x * RIVER_GRADE_SAMPLE_DISTANCE;
        let offset_z = geometry.tangent_z * RIVER_GRADE_SAMPLE_DISTANCE;
        let backward = self.sample_hydraulic_surface_height(
            (geometry.center_x - offset_x).round() as i32,
            (geometry.center_z - offset_z).round() as i32,
        );
        let forward = self.sample_hydraulic_surface_height(
            (geometry.center_x + offset_x).round() as i32,
            (geometry.center_z + offset_z).round() as i32,
        );
        let direction = if forward <= backward { 1.0 } else { -1.0 };
        let (flow_x, flow_z) = (
            geometry.tangent_x * direction,
            geometry.tangent_z * direction,
        );
        let grade = (forward - backward).abs() / (RIVER_GRADE_SAMPLE_DISTANCE * 2.0);
        let mountain_region = mountain_strength(continentalness, ruggedness);
        let low_grade = 1.0 - smoothstep(((grade - 0.02) / 0.16).clamp(0.0, 1.0));
        let low_mountain = 1.0 - smoothstep((mountain_region / 0.55).clamp(0.0, 1.0));
        let inland_wetland = smoothstep((continentalness / 0.18).clamp(0.0, 1.0));
        let wetland_selector = smoothstep(((geometry.width_noise - 0.42) / 0.58).clamp(0.0, 1.0));
        let wetland_influence = low_grade * low_mountain * inland_wetland * wetland_selector;
        let incision_depth =
            (f64::from(base_surface_y - MCLONE_OVERWORLD_SEA_LEVEL - 1)).clamp(0.0, 28.0);
        let bank_span = 12.0 + incision_depth * 1.4 + wetland_influence * 16.0;
        let major_channel_influence =
            compact_influence(geometry.distance, geometry.half_width - 0.5, 3.0);
        let major_bank_influence = 1.0
            - smoothstep(((geometry.distance - geometry.half_width) / bank_span).clamp(0.0, 1.0));
        let wetland_pool_texture = self.wetland_pool.sample_at(
            world_x + geometry.tangent_x * 19.0,
            world_z + geometry.tangent_z * 19.0,
        ) * 0.5
            + 0.5;
        let wetland_pool_influence = if major_channel_influence == 0.0 {
            wetland_influence
                * major_bank_influence
                * smoothstep(((wetland_pool_texture - 0.30) / 0.45).clamp(0.0, 1.0))
        } else {
            0.0
        };
        let mut surface_y = if major_channel_influence > 0.0 {
            let channel_depth = 1.0 + major_channel_influence * f64::from(depth - 1);
            base_surface_y
                .min((f64::from(MCLONE_OVERWORLD_SEA_LEVEL) - channel_depth).round() as i32)
        } else if wetland_pool_influence >= 0.55 {
            base_surface_y.min(MCLONE_OVERWORLD_SEA_LEVEL - 1)
        } else if major_bank_influence > 0.0 {
            let bank_target = (f64::from(base_surface_y) * (1.0 - major_bank_influence)
                + f64::from(MCLONE_OVERWORLD_SEA_LEVEL + 1) * major_bank_influence)
                .round() as i32;
            if geometry.distance <= geometry.half_width + 3.0 {
                MCLONE_OVERWORLD_SEA_LEVEL + 1
            } else {
                bank_target
            }
        } else {
            base_surface_y
        };

        let mut channel_influence = major_channel_influence;
        let mut bank_influence = major_bank_influence;
        let mut raised_tributary_influence = 0.0;
        let mut tributary_source_pool_influence = 0.0;
        let mut water_surface_y = MCLONE_OVERWORLD_SEA_LEVEL;
        let mut bed_y = MCLONE_OVERWORLD_SEA_LEVEL - depth;
        let mut sample_half_width = geometry.half_width;
        let mut tangent_x = geometry.tangent_x;
        let mut tangent_z = geometry.tangent_z;
        let mut sample_flow_x = flow_x;
        let mut sample_flow_z = flow_z;
        let mut drop_distance = f64::INFINITY;
        let mut drop_height = 0;
        let mut drop_upper_y = MCLONE_OVERWORLD_SEA_LEVEL;
        let mut drop_lower_y = MCLONE_OVERWORLD_SEA_LEVEL;

        // Raised water is allowed only as a bounded vignette whose sink is an
        // already-valid sea-level major river. An independent contour crosses
        // one chosen river bank, extends a short fixed distance, and closes in
        // a source pool. The whole source -> reach -> fall -> sink sequence is
        // therefore locally inspectable and cannot later turn uphill.
        let tributary_anchor = if geometry.signed_distance >= geometry.half_width - 1.0
            && geometry.distance <= RIVER_MAX_RELEVANT_DISTANCE
        {
            let crossing_hint =
                self.sample_tributary_geometry(geometry.center_x, geometry.center_z);
            let crossing_alignment_hint = (crossing_hint.tangent_x * geometry.normal_x
                + crossing_hint.tangent_z * geometry.normal_z)
                .abs();
            let selector_hint = self.tributary_selector.sample(
                geometry.center_x.round() as i32,
                geometry.center_z.round() as i32,
            ) * 0.5
                + 0.5;
            if crossing_hint.distance <= 40.0
                && crossing_alignment_hint >= 0.64
                && selector_hint >= 0.40
                && (65.0..=78.0).contains(&self.sample_hydraulic_surface_height(
                    geometry.center_x.round() as i32,
                    geometry.center_z.round() as i32,
                ))
            {
                self.solve_tributary_anchor(geometry)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(anchor) = tributary_anchor {
            let delta_x = world_x - anchor.x;
            let delta_z = world_z - anchor.z;
            let branch_run = delta_x * anchor.away_x + delta_z * anchor.away_z;
            let bank_run = branch_run - (anchor.major.half_width - 0.5);
            let selector = self
                .tributary_selector
                .sample(anchor.x.round() as i32, anchor.z.round() as i32)
                * 0.5
                + 0.5;
            let reach_length = 24.0 + selector * 16.0;
            let max_run =
                reach_length + TRIBUTARY_SOURCE_POOL_RADIUS_BLOCKS + TRIBUTARY_BANK_SPAN_BLOCKS;
            if selector >= 0.42
                && (66.0..=77.0).contains(&self.sample_hydraulic_surface_height(
                    anchor.x.round() as i32,
                    anchor.z.round() as i32,
                ))
                && bank_run >= 0.0
                && bank_run <= max_run
            {
                let tributary = self.sample_tributary_geometry(world_x, world_z);
                let reach_distance = if bank_run <= reach_length {
                    tributary.distance
                } else {
                    f64::INFINITY
                };
                let source_center_run = anchor.major.half_width - 0.5 + reach_length;
                let source_center_x = anchor.x + anchor.away_x * source_center_run;
                let source_center_z = anchor.z + anchor.away_z * source_center_run;
                let source_pool_distance =
                    (world_x - source_center_x).hypot(world_z - source_center_z);
                let reach_influence =
                    compact_influence(reach_distance, TRIBUTARY_HALF_WIDTH_BLOCKS - 0.25, 2.0);
                tributary_source_pool_influence = compact_influence(
                    source_pool_distance,
                    TRIBUTARY_SOURCE_POOL_RADIUS_BLOCKS,
                    2.0,
                );
                raised_tributary_influence = reach_influence.max(tributary_source_pool_influence);
                let outside_reach = reach_distance - TRIBUTARY_HALF_WIDTH_BLOCKS;
                let outside_pool = source_pool_distance - TRIBUTARY_SOURCE_POOL_RADIUS_BLOCKS;
                let outside_water = outside_reach.min(outside_pool);
                let tributary_bank_influence = if outside_water <= 0.0 {
                    1.0
                } else {
                    1.0 - smoothstep((outside_water / TRIBUTARY_BANK_SPAN_BLOCKS).clamp(0.0, 1.0))
                };

                if raised_tributary_influence > 0.0 || tributary_bank_influence > 0.0 {
                    let flow_alignment =
                        tributary.tangent_x * anchor.away_x + tributary.tangent_z * anchor.away_z;
                    let flow_sign = if flow_alignment >= 0.0 { -1.0 } else { 1.0 };
                    let tributary_flow_x = tributary.tangent_x * flow_sign;
                    let tributary_flow_z = tributary.tangent_z * flow_sign;
                    let candidate_drop_distance = bank_run - TRIBUTARY_DROP_BOUNDARY_BLOCKS;
                    let in_drop_context = candidate_drop_distance
                        >= -TRIBUTARY_DROP_BOUNDARY_BLOCKS
                        && candidate_drop_distance <= RIVER_ROCK_LIP_RUN_BLOCKS;
                    let upper_water_y = MCLONE_OVERWORLD_SEA_LEVEL + TRIBUTARY_RISE_BLOCKS;
                    let tributary_water_y = if candidate_drop_distance >= 0.0 {
                        upper_water_y
                    } else {
                        MCLONE_OVERWORLD_SEA_LEVEL
                    };

                    if raised_tributary_influence > 0.0 {
                        channel_influence = channel_influence.max(raised_tributary_influence);
                        water_surface_y = tributary_water_y;
                        sample_half_width = TRIBUTARY_HALF_WIDTH_BLOCKS;
                        tangent_x = tributary.tangent_x;
                        tangent_z = tributary.tangent_z;
                        sample_flow_x = tributary_flow_x;
                        sample_flow_z = tributary_flow_z;
                        if in_drop_context {
                            drop_distance = candidate_drop_distance;
                            drop_height = TRIBUTARY_RISE_BLOCKS;
                            drop_upper_y = upper_water_y;
                            drop_lower_y = MCLONE_OVERWORLD_SEA_LEVEL;
                        }
                        let upstream_rock_lip = drop_height > 0
                            && drop_distance > 0.0
                            && drop_distance <= RIVER_ROCK_LIP_RUN_BLOCKS;
                        let fall_column = drop_height > 0
                            && drop_distance <= 0.0
                            && drop_distance >= -RIVER_FALL_HALF_WIDTH_BLOCKS;
                        let receiving_pool =
                            drop_height > 0 && drop_distance < -RIVER_FALL_HALF_WIDTH_BLOCKS;
                        bed_y = if upstream_rock_lip {
                            upper_water_y - 1
                        } else if fall_column {
                            MCLONE_OVERWORLD_SEA_LEVEL - 4
                        } else if receiving_pool {
                            MCLONE_OVERWORLD_SEA_LEVEL - 3
                        } else if tributary_source_pool_influence > reach_influence {
                            upper_water_y - 3
                        } else {
                            upper_water_y - 2
                        };
                        surface_y = base_surface_y.min(bed_y);
                    } else if major_channel_influence == 0.0 {
                        let bank_water_y = if candidate_drop_distance >= 0.0 {
                            upper_water_y
                        } else {
                            MCLONE_OVERWORLD_SEA_LEVEL
                        };
                        // The complete seven-block support band is a physical
                        // containment berm, not just a visual grade. Keeping
                        // its top above the adjacent source plane makes the
                        // baked result agree with a later fluid wake even
                        // where the local contour solver's acceptance edge
                        // falls inside otherwise low major-river terrain.
                        surface_y = base_surface_y.max(bank_water_y + 1);
                    }
                    bank_influence = bank_influence.max(tributary_bank_influence);
                }
            }
        }

        (
            McloneOverworldWatercourseSample {
                distance: geometry.distance,
                channel_influence,
                major_channel_influence,
                raised_tributary_influence,
                tributary_source_pool_influence,
                bank_influence,
                half_width: sample_half_width,
                water_surface_y,
                bed_y,
                tangent_x,
                tangent_z,
                flow_x: sample_flow_x,
                flow_z: sample_flow_z,
                grade,
                drop_distance,
                drop_height,
                drop_upper_y,
                drop_lower_y,
                wetland_influence,
                wetland_pool_influence,
            },
            surface_y,
        )
    }

    fn sample_hydraulic_surface_height(self, world_x: i32, world_z: i32) -> f64 {
        let continentalness = (self.continent_large.sample(world_x, world_z) * 0.55
            + self.continent_medium.sample(world_x, world_z) * 0.30
            + self.continent_detail.sample(world_x, world_z) * 0.15)
            .clamp(-1.0, 1.0);
        let relief = (self.relief_large.sample(world_x, world_z) * 0.50
            + self.relief_detail.sample(world_x, world_z) * 0.30
            + self.relief_fine.sample(world_x, world_z) * 0.20)
            .clamp(-1.0, 1.0);
        let land_strength = smoothstep((continentalness / 0.45).clamp(0.0, 1.0));
        let hydraulic_relief = relief * (2.0 + land_strength * 7.0);
        (64.0 + land_strength * 18.0 + hydraulic_relief).clamp(63.0, 104.0)
    }

    pub fn sample_landform(self, world_x: i32, world_z: i32) -> McloneOverworldLandformSample {
        let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
        McloneOverworldLandformSample::from_cardinal_samples(
            self.sample(world_x, world_z),
            self.sample(world_x - radius, world_z),
            self.sample(world_x + radius, world_z),
            self.sample(world_x, world_z - radius),
            self.sample(world_x, world_z + radius),
        )
    }

    pub fn sample_region(
        self,
        request: McloneOverworldSampleRegionRequest,
    ) -> Result<McloneOverworldSampleRegion, String> {
        let width = usize::try_from(request.width)
            .map_err(|_| "mclone overworld sample width does not fit usize")?;
        let depth = usize::try_from(request.depth)
            .map_err(|_| "mclone overworld sample depth does not fit usize")?;
        let sample_count = width
            .checked_mul(depth)
            .ok_or("mclone overworld sample region area overflow")?;
        if sample_count > MAX_REGION_SAMPLE_COUNT {
            return Err(format!(
                "mclone overworld sample region has {sample_count} points, maximum is {MAX_REGION_SAMPLE_COUNT}"
            ));
        }
        let step = i32::try_from(request.step)
            .map_err(|_| "mclone overworld sample region step exceeds i32 coordinates")?;
        if step == 0 {
            return Err("mclone overworld sample region step must be at least 1".to_owned());
        }
        if let Some(max_offset_x) = request.width.checked_sub(1) {
            let max_offset_x = i32::try_from(max_offset_x)
                .map_err(|_| "mclone overworld sample region width exceeds i32 coordinates")?
                .checked_mul(step)
                .ok_or("mclone overworld sample region x step overflow")?;
            request
                .min_x
                .checked_add(max_offset_x)
                .ok_or("mclone overworld sample region x coordinate overflow")?;
        }
        if let Some(max_offset_z) = request.depth.checked_sub(1) {
            let max_offset_z = i32::try_from(max_offset_z)
                .map_err(|_| "mclone overworld sample region depth exceeds i32 coordinates")?
                .checked_mul(step)
                .ok_or("mclone overworld sample region z step overflow")?;
            request
                .min_z
                .checked_add(max_offset_z)
                .ok_or("mclone overworld sample region z coordinate overflow")?;
        }

        let mut samples = Vec::with_capacity(sample_count);
        for offset_z in 0..request.depth {
            for offset_x in 0..request.width {
                samples.push(
                    self.sample(
                        request.min_x
                            + i32::try_from(offset_x)
                                .expect("validated mclone overworld sample x offset")
                                * step,
                        request.min_z
                            + i32::try_from(offset_z)
                                .expect("validated mclone overworld sample z offset")
                                * step,
                    ),
                );
            }
        }
        Ok(McloneOverworldSampleRegion { request, samples })
    }
}

pub fn mclone_overworld_spawn_chunk(seed: i64) -> ChunkPos {
    mclone_overworld_spawn_chunk_with_topology(seed, McloneOverworldSamplingTopology::Unbounded)
}

pub fn mclone_overworld_spawn_chunk_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
) -> ChunkPos {
    let sampler = McloneOverworldSampler::new_with_topology(seed, topology);
    for radius in 0..=SPAWN_SEARCH_RADIUS_CHUNKS {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if radius > 0 && x.abs() != radius && z.abs() != radius {
                    continue;
                }
                let world_x = x * 16 + 8;
                let world_z = z * 16 + 8;
                let sample = sampler.sample(world_x, world_z);
                if sample.surface_y >= SPAWN_MIN_SURFACE_Y && !sample.watercourse.is_water() {
                    return ChunkPos::new(topology.canonical_chunk_x(x), z);
                }
            }
        }
    }
    ChunkPos::new(0, 0)
}

fn bathymetry_sample(
    continentalness: f64,
    basin_selector: f64,
    seabed_relief: f64,
) -> McloneOverworldBathymetrySample {
    if continentalness > 0.0 {
        return McloneOverworldBathymetrySample::LAND;
    }
    let ocean_depth_signal = -continentalness;
    let ocean_interior = smoothstep((ocean_depth_signal / 0.55).clamp(0.0, 1.0));
    let shelf_progress = smoothstep((ocean_depth_signal / 0.24).clamp(0.0, 1.0));
    let basin_influence = smoothstep(((ocean_depth_signal - 0.18) / 0.32).clamp(0.0, 1.0));
    let shelf_influence = 1.0 - basin_influence;
    let shelf_break_influence = 4.0 * basin_influence * shelf_influence;
    let depth = 2.0
        + shelf_progress * 10.0
        + basin_influence * (18.0 + basin_selector.clamp(0.0, 1.0) * 10.0)
        + seabed_relief.clamp(-1.0, 1.0) * (1.5 + basin_influence * 7.0);
    McloneOverworldBathymetrySample {
        ocean_interior,
        shelf_influence,
        shelf_break_influence,
        basin_influence,
        seabed_relief: seabed_relief.clamp(-1.0, 1.0),
        water_depth: depth.round().clamp(2.0, 52.0) as i32,
    }
}

fn land_surface_height(
    continentalness: f64,
    relief: f64,
    ruggedness: f64,
    ridges: f64,
    mountain_detail: f64,
) -> i32 {
    land_surface_height_f64(continentalness, relief, ruggedness, ridges, mountain_detail).round()
        as i32
}

fn land_surface_height_f64(
    continentalness: f64,
    relief: f64,
    ruggedness: f64,
    ridges: f64,
    mountain_detail: f64,
) -> f64 {
    let land_strength = smoothstep((continentalness / 0.45).clamp(0.0, 1.0));
    let base = 64.0 + land_strength * 18.0;
    let rolling_relief = relief * (2.0 + land_strength * 7.0);
    let mountain_strength = mountain_strength(continentalness, ruggedness);
    let ridge_shoulder = smoothstep(((ridges - 0.22) / 0.78).clamp(0.0, 1.0));
    let mountain_lift =
        mountain_strength * (4.0 + ridge_shoulder * 12.0 + ridge_shoulder * ridge_shoulder * 38.0);
    let mountain_texture = mountain_detail * mountain_strength * (6.0 + ridge_shoulder * 14.0);
    (base + rolling_relief + mountain_lift + mountain_texture).clamp(62.0, 160.0)
}

fn compact_influence(distance: f64, radius: f64, feather: f64) -> f64 {
    if distance >= radius {
        return 0.0;
    }
    1.0 - smoothstep(((distance - radius + feather) / feather).clamp(0.0, 1.0))
}

fn mountain_strength(continentalness: f64, ruggedness: f64) -> f64 {
    let inland = smoothstep(((continentalness - 0.08) / 0.42).clamp(0.0, 1.0));
    let region = smoothstep(((ruggedness + 0.20) / 0.90).clamp(0.0, 1.0));
    inland * region
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

#[derive(Clone, Copy, Debug)]
struct RiverGeometry {
    signed_distance: f64,
    distance: f64,
    half_width: f64,
    normal_x: f64,
    normal_z: f64,
    tangent_x: f64,
    tangent_z: f64,
    width_noise: f64,
    center_x: f64,
    center_z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct McloneOverworldMajorRiverGeometry {
    pub signed_distance: f64,
    pub distance: f64,
    pub half_width: f64,
    pub normal_x: f64,
    pub normal_z: f64,
    pub tangent_x: f64,
    pub tangent_z: f64,
    pub center_x: f64,
    pub center_z: f64,
}

#[derive(Clone, Copy, Debug)]
struct TributaryAnchor {
    x: f64,
    z: f64,
    major: RiverGeometry,
    away_x: f64,
    away_z: f64,
}

fn inactive_watercourse(
    geometry: RiverGeometry,
    depth: i32,
    base_surface_y: i32,
) -> (McloneOverworldWatercourseSample, i32) {
    (
        McloneOverworldWatercourseSample {
            distance: geometry.distance,
            channel_influence: 0.0,
            major_channel_influence: 0.0,
            raised_tributary_influence: 0.0,
            tributary_source_pool_influence: 0.0,
            bank_influence: 0.0,
            half_width: geometry.half_width,
            water_surface_y: MCLONE_OVERWORLD_SEA_LEVEL,
            bed_y: MCLONE_OVERWORLD_SEA_LEVEL - depth,
            tangent_x: geometry.tangent_x,
            tangent_z: geometry.tangent_z,
            flow_x: geometry.tangent_x,
            flow_z: geometry.tangent_z,
            grade: 0.0,
            drop_distance: f64::INFINITY,
            drop_height: 0,
            drop_upper_y: MCLONE_OVERWORLD_SEA_LEVEL,
            drop_lower_y: MCLONE_OVERWORLD_SEA_LEVEL,
            wetland_influence: 0.0,
            wetland_pool_influence: 0.0,
        },
        base_surface_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_samples_near(
        left: McloneOverworldTerrainSample,
        right: McloneOverworldTerrainSample,
    ) {
        assert!((left.continentalness - right.continentalness).abs() < 1.0e-12);
        assert!((left.relief - right.relief).abs() < 1.0e-12);
        assert!((left.ruggedness - right.ruggedness).abs() < 1.0e-12);
        assert!((left.ridges - right.ridges).abs() < 1.0e-12);
        assert!((left.mountain_detail - right.mountain_detail).abs() < 1.0e-12);
        assert!((left.bathymetry.ocean_interior - right.bathymetry.ocean_interior).abs() < 1.0e-12);
        assert!(
            (left.bathymetry.shelf_influence - right.bathymetry.shelf_influence).abs() < 1.0e-12
        );
        assert!(
            (left.bathymetry.shelf_break_influence - right.bathymetry.shelf_break_influence).abs()
                < 1.0e-12
        );
        assert!(
            (left.bathymetry.basin_influence - right.bathymetry.basin_influence).abs() < 1.0e-12
        );
        assert!((left.bathymetry.seabed_relief - right.bathymetry.seabed_relief).abs() < 1.0e-12);
        assert_eq!(left.bathymetry.water_depth, right.bathymetry.water_depth);
        assert_eq!(left.base_surface_y, right.base_surface_y);
        assert!((left.watercourse.distance - right.watercourse.distance).abs() < 1.0e-9);
        assert!(
            (left.watercourse.channel_influence - right.watercourse.channel_influence).abs()
                < 1.0e-9
        );
        assert!(
            (left.watercourse.major_channel_influence - right.watercourse.major_channel_influence)
                .abs()
                < 1.0e-9
        );
        assert!(
            (left.watercourse.raised_tributary_influence
                - right.watercourse.raised_tributary_influence)
                .abs()
                < 1.0e-9
        );
        assert!(
            (left.watercourse.tributary_source_pool_influence
                - right.watercourse.tributary_source_pool_influence)
                .abs()
                < 1.0e-9
        );
        assert!(
            (left.watercourse.bank_influence - right.watercourse.bank_influence).abs() < 1.0e-9
        );
        assert!((left.watercourse.half_width - right.watercourse.half_width).abs() < 1.0e-12);
        assert_eq!(
            left.watercourse.water_surface_y,
            right.watercourse.water_surface_y
        );
        assert_eq!(left.watercourse.bed_y, right.watercourse.bed_y);
        assert!((left.watercourse.tangent_x - right.watercourse.tangent_x).abs() < 1.0e-9);
        assert!((left.watercourse.tangent_z - right.watercourse.tangent_z).abs() < 1.0e-9);
        assert!((left.watercourse.flow_x - right.watercourse.flow_x).abs() < 1.0e-9);
        assert!((left.watercourse.flow_z - right.watercourse.flow_z).abs() < 1.0e-9);
        assert!((left.watercourse.grade - right.watercourse.grade).abs() < 1.0e-12);
        assert!(
            (left.watercourse.drop_distance.is_infinite()
                && right.watercourse.drop_distance.is_infinite())
                || (left.watercourse.drop_distance - right.watercourse.drop_distance).abs()
                    < 1.0e-9
        );
        assert_eq!(left.watercourse.drop_height, right.watercourse.drop_height);
        assert_eq!(
            left.watercourse.drop_upper_y,
            right.watercourse.drop_upper_y
        );
        assert_eq!(
            left.watercourse.drop_lower_y,
            right.watercourse.drop_lower_y
        );
        assert!(
            (left.watercourse.wetland_influence - right.watercourse.wetland_influence).abs()
                < 1.0e-12
        );
        assert!(
            (left.watercourse.wetland_pool_influence - right.watercourse.wetland_pool_influence)
                .abs()
                < 1.0e-12
        );
        assert_eq!(left.surface_y, right.surface_y);
    }

    #[test]
    fn sampling_topology_accepts_only_the_selected_cylinder() {
        assert_eq!(
            McloneOverworldSamplingTopology::from_horizontal_topology(
                HorizontalTopology::UNBOUNDED
            ),
            Ok(McloneOverworldSamplingTopology::Unbounded)
        );
        assert_eq!(
            McloneOverworldSamplingTopology::from_horizontal_topology(
                HorizontalTopology::cylinder_x(0, MCLONE_OVERWORLD_PERIOD_CHUNKS)
            ),
            Ok(McloneOverworldSamplingTopology::PeriodicX)
        );
        assert!(
            McloneOverworldSamplingTopology::from_horizontal_topology(
                HorizontalTopology::cylinder_x(0, 32)
            )
            .is_err()
        );
    }

    #[test]
    fn periodic_fields_and_seam_slopes_repeat_across_signed_lifts() {
        let sampler = McloneOverworldSampler::new_with_topology(
            -98_765,
            McloneOverworldSamplingTopology::PeriodicX,
        );
        for (x, z) in [(-6_145, -517), (-1, 0), (0, 0), (6_143, 929), (8_191, -73)] {
            assert_samples_near(
                sampler.sample(x, z),
                sampler.sample(x + MCLONE_OVERWORLD_PERIOD_BLOCKS, z),
            );
        }

        for z in [-997, 0, 1_337] {
            let before = sampler.sample(-1, z).surface_y;
            let seam = sampler.sample(0, z).surface_y;
            let repeated_before = sampler
                .sample(MCLONE_OVERWORLD_PERIOD_BLOCKS - 1, z)
                .surface_y;
            let repeated_seam = sampler.sample(MCLONE_OVERWORLD_PERIOD_BLOCKS, z).surface_y;
            assert_eq!(seam - before, repeated_seam - repeated_before);
        }
    }

    #[test]
    fn periodic_region_across_seam_matches_point_sampling() {
        let sampler = McloneOverworldSampler::new_with_topology(
            12_345,
            McloneOverworldSamplingTopology::PeriodicX,
        );
        let request = McloneOverworldSampleRegionRequest {
            min_x: MCLONE_OVERWORLD_PERIOD_BLOCKS - 3,
            min_z: -5,
            width: 7,
            depth: 3,
            step: 1,
        };
        let region = sampler.sample_region(request).unwrap();
        for offset_z in 0..request.depth {
            for offset_x in 0..request.width {
                assert_eq!(
                    region.sample(offset_x, offset_z),
                    Some(sampler.sample(
                        request.min_x + offset_x as i32,
                        request.min_z + offset_z as i32,
                    ))
                );
            }
        }
    }

    #[test]
    fn bounded_region_is_row_major_and_uses_the_point_sampler() {
        let sampler = McloneOverworldSampler::new(-98_765);
        let request = McloneOverworldSampleRegionRequest {
            min_x: -19,
            min_z: 27,
            width: 5,
            depth: 3,
            step: 7,
        };
        let region = sampler.sample_region(request).unwrap();

        assert_eq!(region.samples.len(), 15);
        for offset_z in 0..request.depth {
            for offset_x in 0..request.width {
                assert_eq!(
                    region.sample(offset_x, offset_z),
                    Some(sampler.sample(
                        request.min_x + offset_x as i32 * request.step as i32,
                        request.min_z + offset_z as i32 * request.step as i32
                    ))
                );
            }
        }
        assert_eq!(region.sample(request.width, 0), None);
    }

    #[test]
    fn mountain_strength_requires_inland_rugged_terrain() {
        assert_eq!(mountain_strength(-0.1, 1.0), 0.0);
        assert_eq!(mountain_strength(1.0, -0.2), 0.0);
        assert_eq!(mountain_strength(1.0, 0.7), 1.0);
        assert!(mountain_strength(0.3, 0.3) > 0.0);
        assert!(mountain_strength(0.3, 0.3) < 1.0);
    }

    #[test]
    fn mountain_fields_are_bounded_across_signed_coordinates() {
        let sampler = McloneOverworldSampler::new(-98_765);
        for (x, z) in [
            (i32::MIN + 1, i32::MAX),
            (-6_145, -6_143),
            (-1, 0),
            (0, -1),
            (6_143, 6_145),
        ] {
            let sample = sampler.sample(x, z);
            assert!((-1.0..=1.0).contains(&sample.ruggedness));
            assert!((0.0..=1.0).contains(&sample.ridges));
        }
    }

    #[test]
    fn selected_signed_points_pin_mountain_fields() {
        let samples = [
            (12_345, 0, 0),
            (-98_765, -3_176, 520),
            (8_675_309, 1_960, -1_528),
        ]
        .map(|(seed, x, z)| {
            let sample = McloneOverworldSampler::new(seed).sample(x, z);
            (sample.ruggedness.to_bits(), sample.ridges.to_bits())
        });
        assert_eq!(
            samples,
            [
                (13_826_408_511_758_480_080, 4_606_201_729_120_264_332),
                (4_602_433_175_868_524_518, 4_605_762_744_906_985_117),
                (13_827_819_183_377_043_075, 4_603_495_361_005_022_309),
            ]
        );
    }

    #[test]
    fn ocean_bathymetry_progresses_from_shelf_to_deep_basin() {
        let coast = bathymetry_sample(-0.01, 0.5, 0.0);
        let shelf = bathymetry_sample(-0.18, 0.5, 0.0);
        let shelf_break = bathymetry_sample(-0.34, 0.5, 0.0);
        let basin = bathymetry_sample(-0.55, 0.5, 0.0);

        assert!(coast.water_depth <= 4);
        assert!(shelf.water_depth >= 10);
        assert!(basin.water_depth >= 35);
        assert!(coast.shelf_influence > basin.shelf_influence);
        assert!(basin.basin_influence > shelf.basin_influence);
        assert!(shelf_break.shelf_break_influence > coast.shelf_break_influence);
        assert_eq!(McloneOverworldBathymetrySample::LAND.water_depth, 0);
    }

    #[test]
    fn selected_regions_exercise_shelves_breaks_and_deep_basins() {
        for seed in [12_345, -98_765, 8_675_309] {
            let sampler = McloneOverworldSampler::new(seed);
            let mut shelves = 0;
            let mut breaks = 0;
            let mut deep = 0;
            let mut max_depth = 0;
            for z in (-3_072..=3_072).step_by(32) {
                for x in (-3_072..=3_072).step_by(32) {
                    let sample = sampler.sample(x, z);
                    if sample.continentalness > 0.0 {
                        assert_eq!(sample.bathymetry, McloneOverworldBathymetrySample::LAND);
                        continue;
                    }
                    max_depth = max_depth.max(sample.bathymetry.water_depth);
                    shelves += usize::from(sample.bathymetry.basin_influence < 0.25);
                    breaks += usize::from(sample.bathymetry.shelf_break_influence >= 0.75);
                    deep += usize::from(sample.bathymetry.water_depth >= 24);
                }
            }
            assert!(shelves > 100, "seed {seed} shelves={shelves}");
            assert!(breaks > 100, "seed {seed} breaks={breaks}");
            assert!(deep > 100, "seed {seed} deep={deep}");
            assert!(max_depth >= 36, "seed {seed} max_depth={max_depth}");
        }
    }

    #[test]
    fn lowlands_ignore_mountain_detail_until_the_region_is_active() {
        assert_eq!(
            land_surface_height(0.7, 0.25, -0.2, 1.0, -1.0),
            land_surface_height(0.7, 0.25, -0.2, 1.0, 1.0)
        );
        assert_ne!(
            land_surface_height(0.7, 0.25, 0.7, 1.0, -1.0),
            land_surface_height(0.7, 0.25, 0.7, 1.0, 1.0)
        );
    }

    #[test]
    fn bounded_region_rejects_unbounded_or_wrapping_requests() {
        let sampler = McloneOverworldSampler::new(1);
        assert!(
            sampler
                .sample_region(McloneOverworldSampleRegionRequest {
                    min_x: 0,
                    min_z: 0,
                    width: 4_097,
                    depth: 4_097,
                    step: 1,
                })
                .is_err()
        );
        assert!(
            sampler
                .sample_region(McloneOverworldSampleRegionRequest {
                    min_x: i32::MAX,
                    min_z: 0,
                    width: 2,
                    depth: 1,
                    step: 1,
                })
                .is_err()
        );
        assert!(
            sampler
                .sample_region(McloneOverworldSampleRegionRequest {
                    min_x: 0,
                    min_z: 0,
                    width: 1,
                    depth: 1,
                    step: 0,
                })
                .is_err()
        );
    }

    #[test]
    fn selected_seeds_find_a_dry_spawn_chunk() {
        for seed in [12_345, -98_765, 8_675_309] {
            let spawn = mclone_overworld_spawn_chunk(seed);
            let sample = McloneOverworldSampler::new(seed)
                .sample(spawn.min_block_x() + 8, spawn.min_block_z() + 8);
            assert!(sample.surface_y >= SPAWN_MIN_SURFACE_Y, "{seed}: {spawn:?}");
        }
    }

    #[test]
    fn watercourse_facts_are_bounded_and_exercise_channels_and_wetlands() {
        for seed in [12_345, -98_765, 8_675_309] {
            let sampler = McloneOverworldSampler::new(seed);
            let mut channels = 0;
            let mut wetlands = 0;
            let mut wetland_pools = 0;
            for z in (-2_048..2_048).step_by(8) {
                for x in (-2_048..2_048).step_by(8) {
                    let sample = sampler.sample(x, z);
                    let river = sample.watercourse;
                    assert!((0.0..=512.0).contains(&river.distance));
                    assert!((0.0..=1.0).contains(&river.channel_influence));
                    assert!((0.0..=1.0).contains(&river.major_channel_influence));
                    assert!((0.0..=1.0).contains(&river.raised_tributary_influence));
                    assert!((0.0..=1.0).contains(&river.tributary_source_pool_influence));
                    assert!((0.0..=1.0).contains(&river.bank_influence));
                    assert!(
                        (4.5..=8.5).contains(&river.half_width)
                            || river.half_width == TRIBUTARY_HALF_WIDTH_BLOCKS
                    );
                    assert!(river.bed_y < river.water_surface_y);
                    assert!((0.0..=1.0).contains(&river.wetland_influence));
                    assert!((0.0..=1.0).contains(&river.wetland_pool_influence));
                    if river.is_channel() {
                        channels += 1;
                        assert!(
                            [
                                MCLONE_OVERWORLD_SEA_LEVEL,
                                MCLONE_OVERWORLD_SEA_LEVEL + TRIBUTARY_RISE_BLOCKS,
                            ]
                            .contains(&river.water_surface_y)
                        );
                        if river.is_major_channel() && !river.is_raised_tributary() {
                            assert_eq!(river.water_surface_y, MCLONE_OVERWORLD_SEA_LEVEL);
                        }
                        assert!(sample.surface_y < river.water_surface_y);
                    }
                    if river.is_drop_transition() {
                        assert!(river.is_raised_tributary());
                        assert_eq!(river.drop_height, TRIBUTARY_RISE_BLOCKS);
                        assert_eq!(
                            river.drop_upper_y - river.drop_lower_y,
                            TRIBUTARY_RISE_BLOCKS
                        );
                        assert!(
                            (-TRIBUTARY_DROP_BOUNDARY_BLOCKS..=RIVER_ROCK_LIP_RUN_BLOCKS)
                                .contains(&river.drop_distance)
                        );
                    } else {
                        assert!(river.drop_distance.is_infinite());
                    }
                    if river.wetland_influence > 0.25 && river.bank_influence > 0.0 {
                        wetlands += 1;
                    }
                    if river.is_wetland_pool() {
                        wetland_pools += 1;
                        assert!(
                            (MCLONE_OVERWORLD_SEA_LEVEL
                                ..=MCLONE_OVERWORLD_SEA_LEVEL + TRIBUTARY_RISE_BLOCKS)
                                .contains(&river.water_surface_y)
                        );
                    }
                }
            }
            assert!(channels > 0, "seed {seed} had no river channels");
            assert!(wetlands > 0, "seed {seed} had no wetland margins");
            assert!(wetland_pools > 0, "seed {seed} had no wetland pools");
        }

        let sampler = McloneOverworldSampler::new(-98_765);
        let mut raised = 0;
        let mut source_pool = 0;
        let mut drop_transitions = 0;
        for z in -2_900..-2_760 {
            for x in 2_860..3_000 {
                let river = sampler.sample(x, z).watercourse;
                raised += usize::from(river.is_raised_tributary());
                source_pool += usize::from(river.is_tributary_source_pool());
                drop_transitions += usize::from(river.is_drop_transition());
            }
        }
        assert!(
            raised > 100,
            "review vignette had no meaningful upper reach"
        );
        assert!(source_pool > 25, "review vignette had no source pool");
        assert!(drop_transitions > 10, "review vignette had no bounded drop");
    }
}
