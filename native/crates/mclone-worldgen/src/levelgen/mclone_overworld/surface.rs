use crate::block::{DIRT, GRASS_BLOCK, GRAVEL, SAND, STONE, WATER};
use crate::levelgen::MutableChunkBlockBuffer;

use super::fields::{MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample};

pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y: i32 = 80;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_SLOPE: f64 = 0.80;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_EXPOSURE: f64 = 0.76;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McloneOverworldSurfaceRecipe {
    OceanFloor,
    Beach,
    GrassSoil,
    ExposedStone,
}

pub fn mclone_overworld_surface_recipe(
    sample: McloneOverworldLandformSample,
) -> McloneOverworldSurfaceRecipe {
    let terrain = sample.terrain;
    if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 4 {
        McloneOverworldSurfaceRecipe::OceanFloor
    } else if terrain.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        McloneOverworldSurfaceRecipe::Beach
    } else if terrain.surface_y >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y
        && (sample.slope >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_SLOPE
            || sample.exposure() >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_EXPOSURE)
    {
        McloneOverworldSurfaceRecipe::ExposedStone
    } else {
        McloneOverworldSurfaceRecipe::GrassSoil
    }
}

pub(super) fn write_surface_column(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    sample: McloneOverworldLandformSample,
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
        McloneOverworldSurfaceRecipe::GrassSoil => {
            write_subsurface(buffer, local_x, local_z, surface_y - 1, DIRT, 2);
            buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
        }
        McloneOverworldSurfaceRecipe::ExposedStone => {
            for y in 1..=surface_y {
                buffer.set_block_at_y(local_x, y, local_z, STONE);
            }
        }
    }
    for y in surface_y + 1..=MCLONE_OVERWORLD_SEA_LEVEL {
        buffer.set_block_at_y(local_x, y, local_z, WATER);
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
    }
}
