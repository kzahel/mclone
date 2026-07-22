use super::fields::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample, McloneOverworldSampler,
};
use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID};

pub const MCLONE_OVERWORLD_FOREST_BIOME_ID: i32 = 4;
pub const MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y: i32 = 75;
pub const MCLONE_OVERWORLD_WOODED_MAX_SLOPE: f64 = 0.45;
pub const MCLONE_OVERWORLD_WOODED_MAX_EXPOSURE: f64 = 0.44;

pub fn mclone_overworld_biome_id(seed: i64, world_x: i32, world_z: i32) -> i32 {
    mclone_overworld_biome_id_for_sample(
        McloneOverworldSampler::new(seed).sample_landform(world_x, world_z),
    )
}

pub fn mclone_overworld_biome_id_for_sample(sample: McloneOverworldLandformSample) -> i32 {
    let terrain = sample.terrain;
    if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::mclone_overworld::fields::McloneOverworldTerrainSample;

    fn sample(surface_y: i32, slope: f64) -> McloneOverworldLandformSample {
        McloneOverworldLandformSample {
            terrain: McloneOverworldTerrainSample {
                continentalness: 0.25,
                relief: 0.0,
                ruggedness: 0.0,
                ridges: 0.0,
                mountain_detail: 0.0,
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
}
