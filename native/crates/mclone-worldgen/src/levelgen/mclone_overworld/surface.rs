use crate::block::{
    CLAY, DIRT, GRASS_BLOCK, GRAVEL, SAND, SNOW, STONE, WATER, WATER_LEVEL_8, water_block_for_level,
};
use crate::levelgen::MutableChunkBlockBuffer;

use super::biomes::{McloneOverworldBiomeRecipe, mclone_overworld_biome_recipe};
use super::fields::{MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample};

pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y: i32 = 80;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_SLOPE: f64 = 0.80;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_EXPOSURE: f64 = 0.76;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McloneOverworldSurfaceRecipe {
    OceanFloor,
    Beach,
    RiverBed,
    WetlandBed,
    RiverBank,
    GrassSoil,
    AlpineSnow,
    ExposedStone,
}

pub fn mclone_overworld_surface_recipe(
    sample: McloneOverworldLandformSample,
) -> McloneOverworldSurfaceRecipe {
    let terrain = sample.terrain;
    if terrain.watercourse.is_channel() {
        McloneOverworldSurfaceRecipe::RiverBed
    } else if terrain.watercourse.is_wetland_pool() {
        McloneOverworldSurfaceRecipe::WetlandBed
    } else if terrain.watercourse.is_bank()
        && terrain.surface_y <= terrain.watercourse.water_surface_y + 3
    {
        McloneOverworldSurfaceRecipe::RiverBank
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 4 {
        McloneOverworldSurfaceRecipe::OceanFloor
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        McloneOverworldSurfaceRecipe::Beach
    } else if terrain.surface_y >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y
        && (sample.slope >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_SLOPE
            || sample.exposure() >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_EXPOSURE)
    {
        McloneOverworldSurfaceRecipe::ExposedStone
    } else if mclone_overworld_biome_recipe(sample) == McloneOverworldBiomeRecipe::SnowyAlpine {
        McloneOverworldSurfaceRecipe::AlpineSnow
    } else {
        McloneOverworldSurfaceRecipe::GrassSoil
    }
}

pub(super) fn write_surface_column(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    sample: McloneOverworldLandformSample,
    fall_top_flow_level: u8,
) {
    buffer.set_block_at_y(local_x, 0, local_z, crate::block::BEDROCK);
    let surface_y = sample.terrain.surface_y;
    match mclone_overworld_surface_recipe(sample) {
        McloneOverworldSurfaceRecipe::OceanFloor => {
            write_subsurface(buffer, local_x, local_z, surface_y, GRAVEL, 3)
        }
        McloneOverworldSurfaceRecipe::Beach => {
            write_subsurface(buffer, local_x, local_z, surface_y, SAND, 4)
        }
        McloneOverworldSurfaceRecipe::RiverBed => {
            write_subsurface(buffer, local_x, local_z, surface_y, GRAVEL, 3)
        }
        McloneOverworldSurfaceRecipe::WetlandBed => {
            write_subsurface(buffer, local_x, local_z, surface_y, CLAY, 2)
        }
        McloneOverworldSurfaceRecipe::RiverBank => {
            if sample.terrain.base_surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 5
                && !sample.terrain.watercourse.is_planned_stream()
            {
                write_subsurface(buffer, local_x, local_z, surface_y, SAND, 4);
            } else {
                write_subsurface(buffer, local_x, local_z, surface_y - 1, DIRT, 3);
                buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
            }
        }
        McloneOverworldSurfaceRecipe::GrassSoil => {
            write_subsurface(buffer, local_x, local_z, surface_y - 1, DIRT, 2);
            buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
        }
        McloneOverworldSurfaceRecipe::AlpineSnow => {
            write_subsurface(buffer, local_x, local_z, surface_y - 1, DIRT, 2);
            buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
            buffer.set_block_at_y(local_x, surface_y + 1, local_z, SNOW);
        }
        McloneOverworldSurfaceRecipe::ExposedStone => {
            for y in 1..=surface_y {
                buffer.set_block_at_y(local_x, y, local_z, STONE);
            }
        }
    }
    let watercourse = sample.terrain.watercourse;
    let water_fill_y = if watercourse.is_water() {
        watercourse.water_surface_y
    } else {
        MCLONE_OVERWORLD_SEA_LEVEL
    };
    if watercourse.is_fall_column() {
        for y in surface_y + 1..=watercourse.drop_lower_y {
            buffer.set_block_at_y(local_x, y, local_z, WATER);
        }
        for y in (surface_y + 1).max(watercourse.drop_lower_y + 1)..watercourse.drop_upper_y {
            buffer.set_block_at_y(local_x, y, local_z, WATER_LEVEL_8);
        }
        let top_water = if fall_top_flow_level == 0 {
            WATER
        } else {
            water_block_for_level(fall_top_flow_level.min(7))
                .expect("bounded Mclone waterfall top level must map to water")
        };
        buffer.set_block_at_y(local_x, watercourse.drop_upper_y, local_z, top_water);
    } else {
        for y in surface_y + 1..=water_fill_y {
            buffer.set_block_at_y(local_x, y, local_z, WATER);
        }
    }
}

fn write_subsurface(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    top_y: i32,
    material: u8,
    depth: i32,
) {
    let material_min_y = (top_y - depth + 1).max(1);
    for y in 1..material_min_y {
        buffer.set_block_at_y(local_x, y, local_z, STONE);
    }
    for y in material_min_y..=top_y {
        buffer.set_block_at_y(local_x, y, local_z, material);
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
    fn surface_language_has_distinct_floor_shore_soil_and_exposure_recipes() {
        assert_eq!(
            mclone_overworld_surface_recipe(sample(59, 0.0)),
            McloneOverworldSurfaceRecipe::OceanFloor
        );
        assert_eq!(
            mclone_overworld_surface_recipe(sample(60, 0.0)),
            McloneOverworldSurfaceRecipe::Beach
        );
        assert_eq!(
            mclone_overworld_surface_recipe(sample(79, 1.0)),
            McloneOverworldSurfaceRecipe::GrassSoil
        );
        assert_eq!(
            mclone_overworld_surface_recipe(sample(80, MCLONE_OVERWORLD_EXPOSED_STONE_MIN_SLOPE)),
            McloneOverworldSurfaceRecipe::ExposedStone
        );

        let mut alpine = sample(112, 0.0);
        alpine.terrain.climate = McloneOverworldClimateSample::TEMPERATE;
        assert_eq!(
            mclone_overworld_surface_recipe(alpine),
            McloneOverworldSurfaceRecipe::AlpineSnow
        );
    }
}
