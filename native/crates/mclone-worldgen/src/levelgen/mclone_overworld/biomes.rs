use super::fields::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample, McloneOverworldSampler,
    McloneOverworldSamplingTopology,
};
use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID};

pub const MCLONE_OVERWORLD_FOREST_BIOME_ID: i32 = 4;
pub const MCLONE_OVERWORLD_RIVER_BIOME_ID: i32 = 7;
pub const MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y: i32 = 75;
pub const MCLONE_OVERWORLD_WOODED_MAX_SLOPE: f64 = 0.45;
pub const MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE: f64 = 0.44;
pub const MCLONE_OVERWORLD_ALPINE_MIN_Y: i32 = 96;
pub const MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE: f64 = -0.18;
pub const MCLONE_OVERWORLD_CONIFER_MAX_TEMPERATURE: f64 = -0.12;
pub const MCLONE_OVERWORLD_CONIFER_MIN_MOISTURE: f64 = -0.05;
pub const MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE: f64 = 0.18;
pub const MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE: f64 = -0.10;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldBiomeRecipe {
    Ocean,
    Shore,
    River,
    SnowyAlpine,
    CoolWetConifer,
    WarmDrySteppe,
    TemperateWoodland,
    TemperateMeadow,
}

impl McloneOverworldBiomeRecipe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::Shore => "shore",
            Self::River => "river",
            Self::SnowyAlpine => "snowyAlpine",
            Self::CoolWetConifer => "coolWetConifer",
            Self::WarmDrySteppe => "warmDrySteppe",
            Self::TemperateWoodland => "temperateWoodland",
            Self::TemperateMeadow => "temperateMeadow",
        }
    }
}

pub fn mclone_overworld_biome_id(seed: i64, world_x: i32, world_z: i32) -> i32 {
    mclone_overworld_biome_id_with_topology(
        seed,
        McloneOverworldSamplingTopology::Unbounded,
        world_x,
        world_z,
    )
}

pub fn mclone_overworld_biome_id_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    world_x: i32,
    world_z: i32,
) -> i32 {
    mclone_overworld_biome_id_for_sample(
        McloneOverworldSampler::new_with_topology(seed, topology).sample_landform(world_x, world_z),
    )
}

pub fn mclone_overworld_biome_id_for_sample(sample: McloneOverworldLandformSample) -> i32 {
    let terrain = sample.terrain;
    if terrain.watercourse.is_water() {
        MCLONE_OVERWORLD_RIVER_BIOME_ID
    } else if terrain.watercourse.bank_influence > 0.0
        && terrain.watercourse.wetland_influence > 0.25
    {
        PLAINS_BIOME_ID
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
        OCEAN_BIOME_ID
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        BEACH_BIOME_ID
    } else if terrain.surface_y >= MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y
        && !terrain.is_mountain_valley()
        && !terrain.is_open_mountain_shoulder()
        && sample.slope < MCLONE_OVERWORLD_WOODED_MAX_SLOPE
        && sample.exposure() < MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE
    {
        MCLONE_OVERWORLD_FOREST_BIOME_ID
    } else {
        PLAINS_BIOME_ID
    }
}

pub fn mclone_overworld_biome_recipe(
    sample: McloneOverworldLandformSample,
) -> McloneOverworldBiomeRecipe {
    let terrain = sample.terrain;
    if terrain.watercourse.is_water() {
        McloneOverworldBiomeRecipe::River
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
        McloneOverworldBiomeRecipe::Ocean
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        McloneOverworldBiomeRecipe::Shore
    } else {
        let adjusted_temperature = terrain
            .climate
            .altitude_adjusted_temperature(terrain.surface_y);
        if terrain.surface_y >= MCLONE_OVERWORLD_ALPINE_MIN_Y
            && adjusted_temperature <= MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE
        {
            McloneOverworldBiomeRecipe::SnowyAlpine
        } else if adjusted_temperature <= MCLONE_OVERWORLD_CONIFER_MAX_TEMPERATURE
            && terrain.climate.moisture >= MCLONE_OVERWORLD_CONIFER_MIN_MOISTURE
        {
            McloneOverworldBiomeRecipe::CoolWetConifer
        } else if terrain.climate.temperature >= MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE
            && terrain.climate.moisture <= MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE
        {
            McloneOverworldBiomeRecipe::WarmDrySteppe
        } else if terrain.surface_y >= MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y
            && !terrain.is_mountain_valley()
            && !terrain.is_open_mountain_shoulder()
            && sample.slope < MCLONE_OVERWORLD_WOODED_MAX_SLOPE
            && sample.exposure() < MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE
        {
            McloneOverworldBiomeRecipe::TemperateWoodland
        } else {
            McloneOverworldBiomeRecipe::TemperateMeadow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::mclone_overworld::fields::{
        McloneOverworldBathymetrySample, McloneOverworldClimateSample,
        McloneOverworldTerrainSample, McloneOverworldWatercourseSample,
    };

    fn sample(surface_y: i32, slope: f64) -> McloneOverworldLandformSample {
        McloneOverworldLandformSample {
            terrain: McloneOverworldTerrainSample {
                continentalness: 0.25,
                relief: 0.0,
                ruggedness: 0.0,
                ridges: 0.0,
                mountain_detail: 0.0,
                climate: McloneOverworldClimateSample::TEMPERATE,
                bathymetry: McloneOverworldBathymetrySample::LAND,
                base_surface_y: surface_y,
                watercourse: McloneOverworldWatercourseSample {
                    distance: 512.0,
                    channel_influence: 0.0,
                    major_channel_influence: 0.0,
                    planned_stream_influence: 0.0,
                    stream_headwater_influence: 0.0,
                    bank_influence: 0.0,
                    half_width: 6.0,
                    water_surface_y: MCLONE_OVERWORLD_SEA_LEVEL,
                    bed_y: MCLONE_OVERWORLD_SEA_LEVEL - 3,
                    tangent_x: 1.0,
                    tangent_z: 0.0,
                    flow_x: 1.0,
                    flow_z: 0.0,
                    grade: 0.0,
                    drop_distance: f64::INFINITY,
                    drop_height: 0,
                    drop_upper_y: MCLONE_OVERWORLD_SEA_LEVEL,
                    drop_lower_y: MCLONE_OVERWORLD_SEA_LEVEL,
                    wetland_influence: 0.0,
                    wetland_pool_influence: 0.0,
                },
                surface_y,
            },
            slope,
        }
    }

    #[test]
    fn biome_language_separates_water_shore_lowland_and_wooded_upland() {
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(61, 0.0)),
            OCEAN_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(62, 0.0)),
            BEACH_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(66, 0.0)),
            BEACH_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(67, 0.0)),
            PLAINS_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(74, 0.0)),
            PLAINS_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(75, 0.0)),
            MCLONE_OVERWORLD_FOREST_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(90, MCLONE_OVERWORLD_WOODED_MAX_SLOPE)),
            PLAINS_BIOME_ID
        );
    }

    #[test]
    fn climate_recipe_selects_bookends_without_changing_water_priority() {
        let mut landform = sample(80, 0.0);
        landform.terrain.climate = McloneOverworldClimateSample {
            temperature: -0.3,
            moisture: 0.4,
        };
        assert_eq!(
            mclone_overworld_biome_recipe(landform),
            McloneOverworldBiomeRecipe::CoolWetConifer
        );

        landform.terrain.climate = McloneOverworldClimateSample {
            temperature: 0.4,
            moisture: -0.4,
        };
        assert_eq!(
            mclone_overworld_biome_recipe(landform),
            McloneOverworldBiomeRecipe::WarmDrySteppe
        );

        landform.terrain.surface_y = 112;
        landform.terrain.base_surface_y = 112;
        landform.terrain.climate = McloneOverworldClimateSample {
            temperature: 0.0,
            moisture: 0.0,
        };
        assert_eq!(
            mclone_overworld_biome_recipe(landform),
            McloneOverworldBiomeRecipe::SnowyAlpine
        );

        landform.terrain.surface_y = 61;
        assert_eq!(
            mclone_overworld_biome_recipe(landform),
            McloneOverworldBiomeRecipe::Ocean
        );
    }
}
