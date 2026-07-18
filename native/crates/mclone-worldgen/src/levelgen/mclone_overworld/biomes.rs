use super::fields::{
    MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler, McloneOverworldTerrainSample,
};
use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID, PLAINS_BIOME_ID};

pub const MCLONE_OVERWORLD_FOREST_BIOME_ID: i32 = 4;
pub const MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y: i32 = 75;

pub fn mclone_overworld_biome_id(seed: i64, world_x: i32, world_z: i32) -> i32 {
    mclone_overworld_biome_id_for_sample(McloneOverworldSampler::new(seed).sample(world_x, world_z))
}

pub fn mclone_overworld_biome_id_for_sample(sample: McloneOverworldTerrainSample) -> i32 {
    if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 2 {
        OCEAN_BIOME_ID
    } else if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        BEACH_BIOME_ID
    } else if sample.surface_y >= MCLONE_OVERWORLD_WOODED_UPLAND_MIN_Y {
        MCLONE_OVERWORLD_FOREST_BIOME_ID
    } else {
        PLAINS_BIOME_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(surface_y: i32) -> McloneOverworldTerrainSample {
        McloneOverworldTerrainSample {
            continentalness: 0.25,
            relief: 0.0,
            surface_y,
        }
    }

    #[test]
    fn biome_language_separates_water_shore_lowland_and_wooded_upland() {
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(61)),
            OCEAN_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(62)),
            BEACH_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(66)),
            BEACH_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(67)),
            PLAINS_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(74)),
            PLAINS_BIOME_ID
        );
        assert_eq!(
            mclone_overworld_biome_id_for_sample(sample(75)),
            MCLONE_OVERWORLD_FOREST_BIOME_ID
        );
    }
}
