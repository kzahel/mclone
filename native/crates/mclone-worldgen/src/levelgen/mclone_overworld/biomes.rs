use super::fields::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample, McloneOverworldSampler,
    McloneOverworldSamplingTopology,
};
use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID};

pub const MCLONE_OVERWORLD_FOREST_BIOME_ID: i32 = 4;
pub const MCLONE_OVERWORLD_TAIGA_BIOME_ID: i32 = 5;
pub const MCLONE_OVERWORLD_RIVER_BIOME_ID: i32 = 7;
pub const MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID: i32 = 13;
pub const MCLONE_OVERWORLD_SAVANNA_BIOME_ID: i32 = 35;
pub const MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y: i32 = 75;
pub const MCLONE_OVERWORLD_WOODED_MAX_SLOPE: f64 = 0.45;
pub const MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE: f64 = 0.44;
pub const MCLONE_OVERWORLD_ALPINE_MIN_Y: i32 = 96;
pub const MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE: f64 = -0.18;
pub const MCLONE_OVERWORLD_CONIFER_MAX_TEMPERATURE: f64 = -0.12;
pub const MCLONE_OVERWORLD_CONIFER_MIN_MOISTURE: f64 = -0.05;
pub const MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE: f64 = 0.18;
pub const MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE: f64 = -0.10;
pub const MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_TEMPERATURE: f64 = 0.08;
pub const MCLONE_OVERWORLD_STEPPE_SHOULDER_MAX_MOISTURE: f64 = 0.04;
pub const MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_SUITABILITY: f64 = 0.38;

const MCLONE_OVERWORLD_STEPPE_SUITABILITY_TEMPERATURE_START: f64 = 0.04;
const MCLONE_OVERWORLD_STEPPE_SUITABILITY_TEMPERATURE_RANGE: f64 = 0.32;
const MCLONE_OVERWORLD_STEPPE_SUITABILITY_MOISTURE_START: f64 = 0.08;
const MCLONE_OVERWORLD_STEPPE_SUITABILITY_MOISTURE_RANGE: f64 = 0.32;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldSteppeBand {
    #[default]
    Outside,
    Shoulder,
    Core,
}

impl McloneOverworldSteppeBand {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Outside => "outside",
            Self::Shoulder => "shoulder",
            Self::Core => "core",
        }
    }

    pub const fn is_steppe(self) -> bool {
        !matches!(self, Self::Outside)
    }
}

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldBiomeSelectionReason {
    RiverWater,
    WetlandBank,
    BelowSeaLevel,
    ShoreElevation,
    ColdAlpine,
    CoolWetConifer,
    WarmDrySteppeCore,
    WarmDrySteppeShoulder,
    ShelteredWoodedUpland,
    TemperateMeadowFallback,
}

impl McloneOverworldBiomeSelectionReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RiverWater => "watercourse water",
            Self::WetlandBank => "wetland bank",
            Self::BelowSeaLevel => "below sea level",
            Self::ShoreElevation => "shore elevation",
            Self::ColdAlpine => "cold alpine threshold",
            Self::CoolWetConifer => "cool and moist",
            Self::WarmDrySteppeCore => "warm-dry steppe core",
            Self::WarmDrySteppeShoulder => "warm-dry steppe shoulder",
            Self::ShelteredWoodedUpland => "sheltered wooded upland",
            Self::TemperateMeadowFallback => "temperate meadow fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneOverworldBiomeDecision {
    pub recipe: McloneOverworldBiomeRecipe,
    pub reason: McloneOverworldBiomeSelectionReason,
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
    match mclone_overworld_biome_recipe(sample) {
        McloneOverworldBiomeRecipe::Ocean => OCEAN_BIOME_ID,
        McloneOverworldBiomeRecipe::Shore => BEACH_BIOME_ID,
        McloneOverworldBiomeRecipe::River => MCLONE_OVERWORLD_RIVER_BIOME_ID,
        McloneOverworldBiomeRecipe::SnowyAlpine => MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID,
        McloneOverworldBiomeRecipe::CoolWetConifer => MCLONE_OVERWORLD_TAIGA_BIOME_ID,
        McloneOverworldBiomeRecipe::WarmDrySteppe => MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
        McloneOverworldBiomeRecipe::TemperateWoodland => MCLONE_OVERWORLD_FOREST_BIOME_ID,
        McloneOverworldBiomeRecipe::TemperateMeadow => PLAINS_BIOME_ID,
    }
}

pub fn mclone_overworld_steppe_suitability(
    climate: super::fields::McloneOverworldClimateSample,
) -> f64 {
    let warmth = ((climate.temperature - MCLONE_OVERWORLD_STEPPE_SUITABILITY_TEMPERATURE_START)
        / MCLONE_OVERWORLD_STEPPE_SUITABILITY_TEMPERATURE_RANGE)
        .clamp(0.0, 1.0);
    let dryness = ((MCLONE_OVERWORLD_STEPPE_SUITABILITY_MOISTURE_START - climate.moisture)
        / MCLONE_OVERWORLD_STEPPE_SUITABILITY_MOISTURE_RANGE)
        .clamp(0.0, 1.0);
    (warmth * dryness).sqrt()
}

pub fn mclone_overworld_steppe_band(
    climate: super::fields::McloneOverworldClimateSample,
) -> McloneOverworldSteppeBand {
    if climate.temperature >= MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE
        && climate.moisture <= MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE
    {
        McloneOverworldSteppeBand::Core
    } else if climate.temperature >= MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_TEMPERATURE
        && climate.moisture <= MCLONE_OVERWORLD_STEPPE_SHOULDER_MAX_MOISTURE
        && mclone_overworld_steppe_suitability(climate)
            >= MCLONE_OVERWORLD_STEPPE_SHOULDER_MIN_SUITABILITY
    {
        McloneOverworldSteppeBand::Shoulder
    } else {
        McloneOverworldSteppeBand::Outside
    }
}

pub fn mclone_overworld_biome_recipe(
    sample: McloneOverworldLandformSample,
) -> McloneOverworldBiomeRecipe {
    mclone_overworld_biome_decision(sample).recipe
}

pub fn mclone_overworld_biome_decision(
    sample: McloneOverworldLandformSample,
) -> McloneOverworldBiomeDecision {
    let terrain = sample.terrain;
    if terrain.watercourse.is_water() {
        McloneOverworldBiomeDecision {
            recipe: McloneOverworldBiomeRecipe::River,
            reason: McloneOverworldBiomeSelectionReason::RiverWater,
        }
    } else if terrain.watercourse.bank_influence > 0.0
        && terrain.watercourse.wetland_influence > 0.25
    {
        McloneOverworldBiomeDecision {
            recipe: McloneOverworldBiomeRecipe::TemperateMeadow,
            reason: McloneOverworldBiomeSelectionReason::WetlandBank,
        }
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
        McloneOverworldBiomeDecision {
            recipe: McloneOverworldBiomeRecipe::Ocean,
            reason: McloneOverworldBiomeSelectionReason::BelowSeaLevel,
        }
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        McloneOverworldBiomeDecision {
            recipe: McloneOverworldBiomeRecipe::Shore,
            reason: McloneOverworldBiomeSelectionReason::ShoreElevation,
        }
    } else {
        let adjusted_temperature = terrain
            .climate
            .altitude_adjusted_temperature(terrain.surface_y);
        if terrain.surface_y >= MCLONE_OVERWORLD_ALPINE_MIN_Y
            && adjusted_temperature <= MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE
        {
            McloneOverworldBiomeDecision {
                recipe: McloneOverworldBiomeRecipe::SnowyAlpine,
                reason: McloneOverworldBiomeSelectionReason::ColdAlpine,
            }
        } else if adjusted_temperature <= MCLONE_OVERWORLD_CONIFER_MAX_TEMPERATURE
            && terrain.climate.moisture >= MCLONE_OVERWORLD_CONIFER_MIN_MOISTURE
        {
            McloneOverworldBiomeDecision {
                recipe: McloneOverworldBiomeRecipe::CoolWetConifer,
                reason: McloneOverworldBiomeSelectionReason::CoolWetConifer,
            }
        } else if let steppe @ (McloneOverworldSteppeBand::Core
        | McloneOverworldSteppeBand::Shoulder) =
            mclone_overworld_steppe_band(terrain.climate)
        {
            McloneOverworldBiomeDecision {
                recipe: McloneOverworldBiomeRecipe::WarmDrySteppe,
                reason: match steppe {
                    McloneOverworldSteppeBand::Core => {
                        McloneOverworldBiomeSelectionReason::WarmDrySteppeCore
                    }
                    McloneOverworldSteppeBand::Shoulder => {
                        McloneOverworldBiomeSelectionReason::WarmDrySteppeShoulder
                    }
                    McloneOverworldSteppeBand::Outside => {
                        unreachable!("pattern above excludes outside steppe")
                    }
                },
            }
        } else if terrain.surface_y >= MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y
            && !terrain.is_mountain_valley()
            && !terrain.is_open_mountain_shoulder()
            && sample.slope < MCLONE_OVERWORLD_WOODED_MAX_SLOPE
            && sample.exposure() < MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE
        {
            McloneOverworldBiomeDecision {
                recipe: McloneOverworldBiomeRecipe::TemperateWoodland,
                reason: McloneOverworldBiomeSelectionReason::ShelteredWoodedUpland,
            }
        } else {
            McloneOverworldBiomeDecision {
                recipe: McloneOverworldBiomeRecipe::TemperateMeadow,
                reason: McloneOverworldBiomeSelectionReason::TemperateMeadowFallback,
            }
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
                    submerged_outlet_influence: 0.0,
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
            MCLONE_OVERWORLD_TAIGA_BIOME_ID
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

    #[test]
    fn warm_dry_suitability_adds_a_bounded_shoulder_around_the_old_core() {
        let old_core = McloneOverworldClimateSample {
            temperature: MCLONE_OVERWORLD_STEPPE_MIN_TEMPERATURE,
            moisture: MCLONE_OVERWORLD_STEPPE_MAX_MOISTURE,
        };
        assert_eq!(
            mclone_overworld_steppe_band(old_core),
            McloneOverworldSteppeBand::Core
        );

        let warm_shoulder = McloneOverworldClimateSample {
            temperature: 0.25,
            moisture: 0.0,
        };
        let dry_shoulder = McloneOverworldClimateSample {
            temperature: 0.12,
            moisture: -0.20,
        };
        assert_eq!(
            mclone_overworld_steppe_band(warm_shoulder),
            McloneOverworldSteppeBand::Shoulder
        );
        assert_eq!(
            mclone_overworld_steppe_band(dry_shoulder),
            McloneOverworldSteppeBand::Shoulder
        );

        for outside in [
            McloneOverworldClimateSample {
                temperature: 0.07,
                moisture: -0.40,
            },
            McloneOverworldClimateSample {
                temperature: 0.40,
                moisture: 0.05,
            },
            McloneOverworldClimateSample::TEMPERATE,
        ] {
            assert_eq!(
                mclone_overworld_steppe_band(outside),
                McloneOverworldSteppeBand::Outside
            );
        }
    }

    #[test]
    fn climate_recipes_map_to_compatible_biome_ids() {
        let mut landform = sample(80, 0.0);
        landform.terrain.climate = McloneOverworldClimateSample {
            temperature: -0.3,
            moisture: 0.4,
        };
        assert_eq!(
            mclone_overworld_biome_id_for_sample(landform),
            MCLONE_OVERWORLD_TAIGA_BIOME_ID
        );

        landform.terrain.climate = McloneOverworldClimateSample {
            temperature: 0.4,
            moisture: -0.4,
        };
        assert_eq!(
            mclone_overworld_biome_id_for_sample(landform),
            MCLONE_OVERWORLD_SAVANNA_BIOME_ID
        );

        landform.terrain.surface_y = 112;
        landform.terrain.base_surface_y = 112;
        landform.terrain.climate = McloneOverworldClimateSample::TEMPERATE;
        assert_eq!(
            mclone_overworld_biome_id_for_sample(landform),
            MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID
        );
    }
}
