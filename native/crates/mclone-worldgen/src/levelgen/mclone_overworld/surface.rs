use crate::block::{
    CLAY, COARSE_DIRT, DIRT, GRASS_BLOCK, GRAVEL, RawBlockId, SAND, SNOW, STONE, WATER,
    WATER_LEVEL_8, water_block_for_level,
};
use crate::levelgen::MutableChunkBlockBuffer;

use super::biomes::{McloneOverworldBiomeRecipe, mclone_overworld_biome_recipe};
use super::fields::{MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldLandformSample};

pub const MCLONE_OVERWORLD_ERODED_SLOPE_MIN_Y: i32 = 72;
pub const MCLONE_OVERWORLD_ERODED_SLOPE_MIN_STRENGTH: f64 = 0.18;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y: i32 = 84;
pub const MCLONE_OVERWORLD_EXPOSED_STONE_MIN_STRENGTH: f64 = 0.82;
pub const MCLONE_OVERWORLD_ALPINE_EXPOSED_STONE_MIN_SLOPE: f64 = 1.05;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McloneOverworldSurfaceRecipe {
    OceanFloor,
    Beach,
    RiverBed,
    WetlandBed,
    RiverBank,
    GrassSoil,
    ErodedSlope,
    AlpineSnow,
    ExposedStone,
}

impl McloneOverworldSurfaceRecipe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OceanFloor => "oceanFloor",
            Self::Beach => "beach",
            Self::RiverBed => "riverBed",
            Self::WetlandBed => "wetlandBed",
            Self::RiverBank => "riverBank",
            Self::GrassSoil => "grassSoil",
            Self::ErodedSlope => "erodedSlope",
            Self::AlpineSnow => "alpineSnow",
            Self::ExposedStone => "exposedStone",
        }
    }
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
    } else if mclone_overworld_biome_recipe(sample) == McloneOverworldBiomeRecipe::SnowyAlpine
        && sample.slope < MCLONE_OVERWORLD_ALPINE_EXPOSED_STONE_MIN_SLOPE
    {
        McloneOverworldSurfaceRecipe::AlpineSnow
    } else if terrain.surface_y >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y
        && erosion_strength(sample) >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_STRENGTH
    {
        McloneOverworldSurfaceRecipe::ExposedStone
    } else if terrain.surface_y >= MCLONE_OVERWORLD_ERODED_SLOPE_MIN_Y
        && erosion_strength(sample) >= MCLONE_OVERWORLD_ERODED_SLOPE_MIN_STRENGTH
    {
        McloneOverworldSurfaceRecipe::ErodedSlope
    } else {
        McloneOverworldSurfaceRecipe::GrassSoil
    }
}

/// Selects a stable macro-scale top material from the same production fields
/// available to CPU and GPU terrain previews.
///
/// The exact surface pass additionally reacts to block-scale slope and
/// watercourses. This contract deliberately classifies the uncarved base
/// surface so coarse preview sampling remains deterministic at every spacing.
pub fn mclone_overworld_macro_surface_top_material(
    terrain: super::fields::McloneOverworldTerrainSample,
) -> RawBlockId {
    if terrain.continentalness <= 0.0 {
        return WATER;
    }
    let surface_y = terrain.base_surface_y;
    if surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 3 {
        return SAND;
    }
    let adjusted_temperature = terrain.climate.altitude_adjusted_temperature(surface_y);
    if surface_y >= super::biomes::MCLONE_OVERWORLD_ALPINE_MIN_Y
        && adjusted_temperature <= super::biomes::MCLONE_OVERWORLD_ALPINE_MAX_TEMPERATURE
    {
        return SNOW;
    }
    let altitude = smoothstep((f64::from(surface_y - 82) / 48.0).clamp(0.0, 1.0));
    let crest = smoothstep(((terrain.ridges - 0.35) / 0.65).clamp(0.0, 1.0));
    let exposure = terrain.mountain_strength() * (altitude * 0.35 + crest * 0.65);
    let strength = ((exposure - 0.48) / 0.44).clamp(0.0, 1.0);
    if surface_y >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_Y
        && strength >= MCLONE_OVERWORLD_EXPOSED_STONE_MIN_STRENGTH
    {
        return STONE;
    }
    if surface_y >= MCLONE_OVERWORLD_ERODED_SLOPE_MIN_Y
        && strength >= MCLONE_OVERWORLD_ERODED_SLOPE_MIN_STRENGTH
    {
        let texture = (terrain.mountain_detail * 0.68
            + terrain.relief * 0.17
            + (terrain.ridges * 2.0 - 1.0) * 0.15)
            .clamp(-1.0, 1.0);
        return if strength >= 0.62 && texture >= 0.36 - strength * 0.28 {
            STONE
        } else if texture >= 0.02 - strength * 0.22 {
            GRAVEL
        } else if texture >= -0.48 - strength * 0.10 {
            COARSE_DIRT
        } else {
            GRASS_BLOCK
        };
    }
    GRASS_BLOCK
}

/// Selects the material visible from above after natural or planned
/// watercourse carving, without requiring block-scale slope samples.
///
/// This is a preview/LOD contract. Canonical chunks continue to use
/// [`mclone_overworld_surface_recipe`] and the full column writer.
pub fn mclone_overworld_preview_visible_material(
    terrain: super::fields::McloneOverworldTerrainSample,
) -> RawBlockId {
    if terrain.continentalness <= 0.0 || terrain.watercourse.is_water() {
        return WATER;
    }
    if terrain.watercourse.is_bank() && terrain.surface_y <= terrain.watercourse.water_surface_y + 3
    {
        return if terrain.base_surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 5
            && !terrain.watercourse.is_planned_stream()
        {
            SAND
        } else {
            GRASS_BLOCK
        };
    }
    mclone_overworld_macro_surface_top_material(terrain)
}

pub(super) fn mclone_overworld_surface_top_material(
    sample: McloneOverworldLandformSample,
) -> RawBlockId {
    match mclone_overworld_surface_recipe(sample) {
        McloneOverworldSurfaceRecipe::OceanFloor | McloneOverworldSurfaceRecipe::RiverBed => GRAVEL,
        McloneOverworldSurfaceRecipe::Beach => SAND,
        McloneOverworldSurfaceRecipe::WetlandBed => CLAY,
        McloneOverworldSurfaceRecipe::RiverBank
            if sample.terrain.base_surface_y <= MCLONE_OVERWORLD_SEA_LEVEL + 5
                && !sample.terrain.watercourse.is_planned_stream() =>
        {
            SAND
        }
        McloneOverworldSurfaceRecipe::RiverBank
        | McloneOverworldSurfaceRecipe::GrassSoil
        | McloneOverworldSurfaceRecipe::AlpineSnow => GRASS_BLOCK,
        McloneOverworldSurfaceRecipe::ErodedSlope => eroded_slope_material(sample),
        McloneOverworldSurfaceRecipe::ExposedStone => STONE,
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
        McloneOverworldSurfaceRecipe::ErodedSlope => write_eroded_slope_column(
            buffer,
            local_x,
            local_z,
            surface_y,
            mclone_overworld_surface_top_material(sample),
        ),
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

fn erosion_strength(sample: McloneOverworldLandformSample) -> f64 {
    let slope = ((sample.slope - 0.45) / 0.95).clamp(0.0, 1.0);
    let exposure = ((sample.exposure() - 0.48) / 0.44).clamp(0.0, 1.0);
    slope.max(exposure)
}

fn eroded_slope_material(sample: McloneOverworldLandformSample) -> RawBlockId {
    let terrain = sample.terrain;
    let texture = (terrain.mountain_detail * 0.68
        + terrain.relief * 0.17
        + (terrain.ridges * 2.0 - 1.0) * 0.15)
        .clamp(-1.0, 1.0);
    let strength = erosion_strength(sample);
    if strength >= 0.62 && texture >= 0.36 - strength * 0.28 {
        STONE
    } else if texture >= 0.02 - strength * 0.22 {
        GRAVEL
    } else if texture >= -0.48 - strength * 0.10 {
        COARSE_DIRT
    } else {
        GRASS_BLOCK
    }
}

fn write_eroded_slope_column(
    buffer: &mut MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    surface_y: i32,
    material: RawBlockId,
) {
    match material {
        GRASS_BLOCK => {
            write_subsurface(buffer, local_x, local_z, surface_y - 1, DIRT, 2);
            buffer.set_block_at_y(local_x, surface_y, local_z, GRASS_BLOCK);
        }
        COARSE_DIRT => {
            write_subsurface(buffer, local_x, local_z, surface_y - 1, DIRT, 2);
            buffer.set_block_at_y(local_x, surface_y, local_z, COARSE_DIRT);
        }
        GRAVEL => write_subsurface(buffer, local_x, local_z, surface_y, GRAVEL, 2),
        STONE => {
            for y in 1..=surface_y {
                buffer.set_block_at_y(local_x, y, local_z, STONE);
            }
        }
        material => unreachable!("unexpected eroded-slope material {material}"),
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

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::McloneOverworldCoastIntent;
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
                coast: McloneOverworldCoastIntent::INLAND,
                climate: McloneOverworldClimateSample::TEMPERATE,
                bathymetry: McloneOverworldBathymetrySample::LAND,
                base_surface_y: surface_y,
                watercourse: McloneOverworldWatercourseSample {
                    signed_distance: 512.0,
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
    fn macro_surface_materials_preserve_distinct_lod_regions() {
        let mut terrain = sample(70, 0.0).terrain;
        assert_eq!(
            mclone_overworld_macro_surface_top_material(terrain),
            GRASS_BLOCK
        );

        terrain.continentalness = -0.1;
        assert_eq!(mclone_overworld_macro_surface_top_material(terrain), WATER);

        terrain.continentalness = 0.5;
        terrain.base_surface_y = 65;
        assert_eq!(mclone_overworld_macro_surface_top_material(terrain), SAND);

        terrain.base_surface_y = 110;
        terrain.climate.temperature = -1.0;
        assert_eq!(mclone_overworld_macro_surface_top_material(terrain), SNOW);

        terrain.base_surface_y = 150;
        terrain.climate.temperature = 1.0;
        terrain.ruggedness = 1.0;
        terrain.ridges = 1.0;
        terrain.mountain_detail = 1.0;
        assert_eq!(mclone_overworld_macro_surface_top_material(terrain), STONE);

        terrain.base_surface_y = 80;
        terrain.mountain_detail = 0.0;
        assert_eq!(mclone_overworld_macro_surface_top_material(terrain), GRAVEL);
        terrain.mountain_detail = -0.5;
        assert_eq!(
            mclone_overworld_macro_surface_top_material(terrain),
            COARSE_DIRT
        );
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
            McloneOverworldSurfaceRecipe::ErodedSlope
        );
        assert_eq!(
            mclone_overworld_surface_recipe(sample(92, 1.4)),
            McloneOverworldSurfaceRecipe::ExposedStone
        );
        let mut mixed_materials = [false; 4];
        for detail in [-0.85, -0.25, 0.20, 0.85] {
            let mut eroded = sample(80, 1.0);
            eroded.terrain.mountain_detail = detail;
            let index = match mclone_overworld_surface_top_material(eroded) {
                GRASS_BLOCK => 0,
                COARSE_DIRT => 1,
                GRAVEL => 2,
                STONE => 3,
                material => panic!("unexpected eroded material {material}"),
            };
            mixed_materials[index] = true;
        }
        assert!(
            mixed_materials
                .into_iter()
                .filter(|present| *present)
                .count()
                >= 3
        );

        let mut alpine = sample(112, 0.0);
        alpine.terrain.climate = McloneOverworldClimateSample::TEMPERATE;
        assert_eq!(
            mclone_overworld_surface_recipe(alpine),
            McloneOverworldSurfaceRecipe::AlpineSnow
        );
    }
}
