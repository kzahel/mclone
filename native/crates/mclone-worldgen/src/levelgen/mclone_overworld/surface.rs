use crate::block::{DIRT, GRASS_BLOCK, GRAVEL, SAND, STONE, WATER};
use crate::levelgen::MutableChunkBlockBuffer;

use super::fields::{MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldTerrainSample};

pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y: i32 = 80;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_RELIEF: f64 = 0.50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McloneOverworldSurfaceRecipe {
    OceanFloor,
    Beach,
    GrassSoil,
    ExposedStone,
}

pub fn mclone_overworld_surface_recipe(
    sample: McloneOverworldTerrainSample,
) -> McloneOverworldSurfaceRecipe {
    if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL - 4 {
        McloneOverworldSurfaceRecipe::OceanFloor
    } else if sample.surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        McloneOverworldSurfaceRecipe::Beach
    } else if sample.surface_y >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y
        && sample.relief >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_RELIEF
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
    sample: McloneOverworldTerrainSample,
) {
    buffer.set_block_at_y(local_x, 0, local_z, crate::block::BEDROCK);
    let surface_y = sample.surface_y;
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

    fn sample(surface_y: i32, relief: f64) -> McloneOverworldTerrainSample {
        McloneOverworldTerrainSample {
            continentalness: 0.25,
            relief,
            surface_y,
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
            mclone_overworld_surface_recipe(sample(79, 0.8)),
            McloneOverworldSurfaceRecipe::GrassSoil
        );
        assert_eq!(
            mclone_overworld_surface_recipe(sample(80, 0.50)),
            McloneOverworldSurfaceRecipe::ExposedStone
        );
    }
}
