use crate::biome::BiomeDefinition;
use crate::block::{
    AIR, BLACK_TERRACOTTA, BLUE_TERRACOTTA, BROWN_TERRACOTTA, COARSE_DIRT, CYAN_TERRACOTTA, DIRT,
    GRASS_BLOCK, GRAVEL, GRAY_TERRACOTTA, GREEN_TERRACOTTA, ICE, LIGHT_BLUE_TERRACOTTA,
    LIGHT_GRAY_TERRACOTTA, LIME_TERRACOTTA, MAGENTA_TERRACOTTA, MYCELIUM, ORANGE_TERRACOTTA,
    PACKED_ICE, PINK_TERRACOTTA, PODZOL, PURPLE_TERRACOTTA, RED_SAND, RED_SANDSTONE,
    RED_TERRACOTTA, SAND, SANDSTONE, SNOW_BLOCK, STONE, TERRACOTTA, WATER, WHITE_TERRACOTTA,
    YELLOW_TERRACOTTA,
};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::noise::PerlinSimplexNoise;
use crate::prng::WorldgenRandom;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const CHUNK_WIDTH: i32 = 16;
const BADLANDS_BAND_LENGTH: usize = 64;
const MAX_CLAY_DEPTH: i32 = 15;

const BIOME_INFO_NOISE_OCTAVES: [i32; 1] = [0];
const TEMPERATURE_NOISE_OCTAVES: [i32; 1] = [0];
const FROZEN_TEMPERATURE_NOISE_OCTAVES: [i32; 3] = [-2, -1, 0];
const BADLANDS_PILLAR_OCTAVES: [i32; 4] = [-3, -2, -1, 0];
const BADLANDS_BAND_OFFSET_OCTAVES: [i32; 1] = [0];
const FROZEN_OCEAN_ICEBERG_OCTAVES: [i32; 4] = [-3, -2, -1, 0];
const FROZEN_OCEAN_ICEBERG_ROOF_OCTAVES: [i32; 1] = [0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SurfaceBuilderConfiguration {
    top_material: u8,
    under_material: u8,
    underwater_material: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceBuilderKind {
    Default,
    Mountain,
    GravellyMountain,
    Swamp,
    GiantTreeTaiga,
    ShatteredSavanna,
    Badlands,
    WoodedBadlands,
    ErodedBadlands,
    FrozenOcean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SurfaceBiomeDefinition {
    builder: SurfaceBuilderKind,
    config: SurfaceBuilderConfiguration,
}

#[derive(Clone, Debug)]
struct BadlandsNoiseState {
    clay_bands: Vec<u8>,
    pillar_noise: PerlinSimplexNoise,
    pillar_roof_noise: PerlinSimplexNoise,
    clay_bands_offset_noise: PerlinSimplexNoise,
}

#[derive(Clone, Debug)]
struct FrozenOceanNoiseState {
    iceberg_noise: PerlinSimplexNoise,
    iceberg_roof_noise: PerlinSimplexNoise,
}

const CONFIG_GRASS: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: GRASS_BLOCK,
    under_material: DIRT,
    underwater_material: GRAVEL,
};
const CONFIG_STONE: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: STONE,
    under_material: STONE,
    underwater_material: GRAVEL,
};
const CONFIG_GRAVEL: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: GRAVEL,
    under_material: GRAVEL,
    underwater_material: GRAVEL,
};
const CONFIG_DESERT: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: SAND,
    under_material: SAND,
    underwater_material: GRAVEL,
};
const CONFIG_OCEAN_SAND: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: GRASS_BLOCK,
    under_material: DIRT,
    underwater_material: SAND,
};
const CONFIG_FULL_SAND: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: SAND,
    under_material: SAND,
    underwater_material: SAND,
};
const CONFIG_BADLANDS: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: RED_SAND,
    under_material: WHITE_TERRACOTTA,
    underwater_material: GRAVEL,
};
const CONFIG_COARSE_DIRT: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: COARSE_DIRT,
    under_material: DIRT,
    underwater_material: GRAVEL,
};
const CONFIG_PODZOL: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: PODZOL,
    under_material: DIRT,
    underwater_material: GRAVEL,
};
const CONFIG_MYCELIUM: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: MYCELIUM,
    under_material: DIRT,
    underwater_material: GRAVEL,
};
const CONFIG_ICE_SPIKES: SurfaceBuilderConfiguration = SurfaceBuilderConfiguration {
    top_material: SNOW_BLOCK,
    under_material: DIRT,
    underwater_material: GRAVEL,
};

#[allow(clippy::too_many_arguments)]
pub fn apply_overworld_surface(
    random: &mut WorldgenRandom,
    chunk: &mut MutableChunkBlockBuffer,
    biome: BiomeDefinition,
    world_x: i32,
    world_z: i32,
    height: i32,
    noise: f64,
    sea_level: i32,
    min_surface_level: i32,
    seed: i64,
) {
    let definition = resolve_surface_biome_definition(biome);
    match definition.builder {
        SurfaceBuilderKind::Default => apply_default_surface(
            random,
            chunk,
            biome,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            definition.config,
        ),
        SurfaceBuilderKind::Mountain => apply_default_surface(
            random,
            chunk,
            biome,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            if noise > 1.0 {
                CONFIG_STONE
            } else {
                CONFIG_GRASS
            },
        ),
        SurfaceBuilderKind::GravellyMountain => apply_default_surface(
            random,
            chunk,
            biome,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            if noise < -1.0 || noise > 2.0 {
                CONFIG_GRAVEL
            } else if noise > 1.0 {
                CONFIG_STONE
            } else {
                CONFIG_GRASS
            },
        ),
        SurfaceBuilderKind::GiantTreeTaiga => apply_default_surface(
            random,
            chunk,
            biome,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            if noise > 1.75 {
                CONFIG_COARSE_DIRT
            } else if noise > -0.95 {
                CONFIG_PODZOL
            } else {
                CONFIG_GRASS
            },
        ),
        SurfaceBuilderKind::ShatteredSavanna => apply_default_surface(
            random,
            chunk,
            biome,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            if noise > 1.75 {
                CONFIG_STONE
            } else if noise > -0.5 {
                CONFIG_COARSE_DIRT
            } else {
                CONFIG_GRASS
            },
        ),
        SurfaceBuilderKind::Swamp => {
            if biome_info_noise().get_value(world_x as f64 * 0.25, world_z as f64 * 0.25, false)
                > 0.0
            {
                let local_x = local_coord(world_x);
                let local_z = local_coord(world_z);
                for y in (min_surface_level..=height).rev() {
                    let block_id = get_block_at_y_or_air(chunk, local_x, y, local_z);
                    if block_id == AIR {
                        continue;
                    }

                    if y == 62 && block_id != WATER {
                        set_block_at_y_if_inside(chunk, local_x, y, local_z, WATER);
                    }
                    break;
                }
            }

            apply_default_surface(
                random,
                chunk,
                biome,
                world_x,
                world_z,
                height,
                noise,
                sea_level,
                min_surface_level,
                definition.config,
            );
        }
        SurfaceBuilderKind::Badlands
        | SurfaceBuilderKind::WoodedBadlands
        | SurfaceBuilderKind::ErodedBadlands => apply_badlands_surface(
            random,
            chunk,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            definition.config,
            seed,
            matches!(definition.builder, SurfaceBuilderKind::WoodedBadlands),
            matches!(definition.builder, SurfaceBuilderKind::ErodedBadlands),
        ),
        SurfaceBuilderKind::FrozenOcean => apply_frozen_ocean_surface(
            random,
            chunk,
            biome,
            world_x,
            world_z,
            height,
            noise,
            sea_level,
            min_surface_level,
            definition.config,
            seed,
        ),
    }
}

fn resolve_surface_biome_definition(biome: BiomeDefinition) -> SurfaceBiomeDefinition {
    match biome.key() {
        "minecraft:mountains" | "minecraft:mountain_edge" | "minecraft:wooded_mountains" => {
            SurfaceBiomeDefinition {
                builder: SurfaceBuilderKind::Mountain,
                config: CONFIG_GRASS,
            }
        }
        "minecraft:gravelly_mountains" | "minecraft:modified_gravelly_mountains" => {
            SurfaceBiomeDefinition {
                builder: SurfaceBuilderKind::GravellyMountain,
                config: CONFIG_GRASS,
            }
        }
        "minecraft:desert"
        | "minecraft:desert_hills"
        | "minecraft:desert_lakes"
        | "minecraft:beach"
        | "minecraft:snowy_beach" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_DESERT,
        },
        "minecraft:ocean"
        | "minecraft:deep_ocean"
        | "minecraft:cold_ocean"
        | "minecraft:deep_cold_ocean" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_GRASS,
        },
        "minecraft:lukewarm_ocean" | "minecraft:deep_lukewarm_ocean" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_OCEAN_SAND,
        },
        "minecraft:warm_ocean" | "minecraft:deep_warm_ocean" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_FULL_SAND,
        },
        "minecraft:swamp" | "minecraft:swamp_hills" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Swamp,
            config: CONFIG_GRASS,
        },
        "minecraft:frozen_ocean" | "minecraft:deep_frozen_ocean" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::FrozenOcean,
            config: CONFIG_GRASS,
        },
        "minecraft:badlands"
        | "minecraft:badlands_plateau"
        | "minecraft:modified_badlands_plateau" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Badlands,
            config: CONFIG_BADLANDS,
        },
        "minecraft:wooded_badlands_plateau" | "minecraft:modified_wooded_badlands_plateau" => {
            SurfaceBiomeDefinition {
                builder: SurfaceBuilderKind::WoodedBadlands,
                config: CONFIG_BADLANDS,
            }
        }
        "minecraft:eroded_badlands" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::ErodedBadlands,
            config: CONFIG_BADLANDS,
        },
        "minecraft:giant_tree_taiga"
        | "minecraft:giant_tree_taiga_hills"
        | "minecraft:giant_spruce_taiga"
        | "minecraft:giant_spruce_taiga_hills" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::GiantTreeTaiga,
            config: CONFIG_GRASS,
        },
        "minecraft:shattered_savanna" | "minecraft:shattered_savanna_plateau" => {
            SurfaceBiomeDefinition {
                builder: SurfaceBuilderKind::ShatteredSavanna,
                config: CONFIG_GRASS,
            }
        }
        "minecraft:mushroom_fields" | "minecraft:mushroom_field_shore" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_MYCELIUM,
        },
        "minecraft:ice_spikes" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_ICE_SPIKES,
        },
        "minecraft:stone_shore" => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_STONE,
        },
        _ => SurfaceBiomeDefinition {
            builder: SurfaceBuilderKind::Default,
            config: CONFIG_GRASS,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_default_surface(
    random: &mut WorldgenRandom,
    chunk: &mut MutableChunkBlockBuffer,
    biome: BiomeDefinition,
    world_x: i32,
    world_z: i32,
    height: i32,
    noise: f64,
    sea_level: i32,
    min_surface_level: i32,
    config: SurfaceBuilderConfiguration,
) {
    let local_x = local_coord(world_x);
    let local_z = local_coord(world_z);
    let surface_depth = (noise / 3.0 + 3.0 + random.next_double() * 0.25) as i32;

    if surface_depth == 0 {
        let mut found_default_surface = false;
        for y in (min_surface_level..=height).rev() {
            let block_id = get_block_at_y_or_air(chunk, local_x, y, local_z);
            if block_id == AIR {
                found_default_surface = false;
            } else if block_id == STONE {
                if !found_default_surface {
                    let replacement = if y >= sea_level {
                        AIR
                    } else if y == sea_level - 1 {
                        if biome_temperature(biome, world_x, y, world_z) < 0.15 {
                            ICE
                        } else {
                            WATER
                        }
                    } else if y >= sea_level - (7 + surface_depth) {
                        STONE
                    } else {
                        config.underwater_material
                    };
                    set_block_at_y_if_inside(chunk, local_x, y, local_z, replacement);
                }

                found_default_surface = true;
            }
        }
        return;
    }

    let mut under_material = config.under_material;
    let mut remaining_depth = -1;
    for y in (min_surface_level..=height).rev() {
        let block_id = get_block_at_y_or_air(chunk, local_x, y, local_z);
        if block_id == AIR {
            remaining_depth = -1;
            continue;
        }

        if block_id != STONE {
            continue;
        }

        if remaining_depth == -1 {
            remaining_depth = surface_depth;
            let top_material = if y >= sea_level + 2 {
                config.top_material
            } else if y >= sea_level - 1 {
                under_material = config.under_material;
                config.top_material
            } else if y >= sea_level - 4 {
                under_material = config.under_material;
                config.under_material
            } else if y >= sea_level - (7 + surface_depth) {
                under_material
            } else {
                under_material = STONE;
                config.underwater_material
            };
            set_block_at_y_if_inside(chunk, local_x, y, local_z, top_material);
        } else if remaining_depth > 0 {
            remaining_depth -= 1;
            set_block_at_y_if_inside(chunk, local_x, y, local_z, under_material);
            if remaining_depth == 0 && under_material == SAND && surface_depth > 1 {
                remaining_depth = random.next_int_bound(4) + (y - sea_level).max(0);
                under_material = SANDSTONE;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_badlands_surface(
    random: &mut WorldgenRandom,
    chunk: &mut MutableChunkBlockBuffer,
    world_x: i32,
    world_z: i32,
    height: i32,
    noise: f64,
    sea_level: i32,
    min_surface_level: i32,
    config: SurfaceBuilderConfiguration,
    seed: i64,
    wooded: bool,
    eroded: bool,
) {
    let local_x = local_coord(world_x);
    let local_z = local_coord(world_z);
    let state = badlands_noise_state(seed);
    let mut pillar_height = 0.0;

    if eroded {
        let pillar_noise = noise.abs().min(
            state
                .pillar_noise
                .get_value(world_x as f64 * 0.25, world_z as f64 * 0.25, false)
                * 15.0,
        );
        if pillar_noise > 0.0 {
            let roof_noise = state
                .pillar_roof_noise
                .get_value(
                    world_x as f64 * 0.001_953_125,
                    world_z as f64 * 0.001_953_125,
                    false,
                )
                .abs();
            pillar_height = pillar_noise * pillar_noise * 2.5;
            let max_pillar_height = (roof_noise * 50.0).ceil() + 14.0;
            if pillar_height > max_pillar_height {
                pillar_height = max_pillar_height;
            }
            pillar_height += 64.0;
        }
    }

    let mut top_material = WHITE_TERRACOTTA;
    let biome_under_material = config.under_material;
    let biome_top_material = config.top_material;
    let mut under_material = biome_under_material;
    let surface_depth = (noise / 3.0 + 3.0 + random.next_double() * 0.25) as i32;
    let cosine_bands = (noise / 3.0 * std::f64::consts::PI).cos() > 0.0;
    let mut remaining_depth = -1;
    let mut top_placed = false;
    let mut stone_depth = 0;

    for y in (min_surface_level..=height.max(pillar_height as i32 + 1)).rev() {
        if !eroded && stone_depth >= MAX_CLAY_DEPTH {
            break;
        }

        let mut block_id = get_block_at_y_or_air(chunk, local_x, y, local_z);
        if eroded && block_id == AIR && (y as f64) < pillar_height {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, STONE);
            block_id = STONE;
        }

        if block_id == AIR {
            remaining_depth = -1;
            continue;
        }

        if block_id != STONE {
            continue;
        }

        if remaining_depth == -1 {
            top_placed = false;
            if surface_depth <= 0 {
                top_material = AIR;
                under_material = STONE;
            } else if y >= sea_level - 4 && y <= sea_level + 1 {
                top_material = WHITE_TERRACOTTA;
                under_material = biome_under_material;
            }

            if y < sea_level && top_material == AIR {
                top_material = WATER;
            }

            remaining_depth = surface_depth + (y - sea_level).max(0);
            if wooded {
                apply_wooded_badlands_top(
                    chunk,
                    &state,
                    local_x,
                    local_z,
                    world_x,
                    world_z,
                    y,
                    sea_level,
                    surface_depth,
                    cosine_bands,
                    biome_top_material,
                    under_material,
                    &mut top_placed,
                );
            } else {
                apply_badlands_top(
                    chunk,
                    &state,
                    local_x,
                    local_z,
                    world_x,
                    world_z,
                    y,
                    sea_level,
                    surface_depth,
                    cosine_bands,
                    biome_top_material,
                    under_material,
                    &mut top_placed,
                );
            }
        } else if remaining_depth > 0 {
            remaining_depth -= 1;
            let replacement = if top_placed {
                ORANGE_TERRACOTTA
            } else {
                get_badlands_band(&state, world_x, y, world_z)
            };
            set_block_at_y_if_inside(chunk, local_x, y, local_z, replacement);
        }

        stone_depth += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_badlands_top(
    chunk: &mut MutableChunkBlockBuffer,
    state: &BadlandsNoiseState,
    local_x: i32,
    local_z: i32,
    world_x: i32,
    world_z: i32,
    y: i32,
    sea_level: i32,
    surface_depth: i32,
    cosine_bands: bool,
    biome_top_material: u8,
    under_material: u8,
    top_placed: &mut bool,
) {
    if y >= sea_level - 1 {
        if y <= sea_level + 3 + surface_depth {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, biome_top_material);
            *top_placed = true;
        } else {
            let replacement = get_badlands_ceiling_block(state, world_x, y, world_z, cosine_bands);
            set_block_at_y_if_inside(chunk, local_x, y, local_z, replacement);
        }
    } else {
        set_block_at_y_if_inside(chunk, local_x, y, local_z, under_material);
        if is_terracotta(under_material) {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, ORANGE_TERRACOTTA);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_wooded_badlands_top(
    chunk: &mut MutableChunkBlockBuffer,
    state: &BadlandsNoiseState,
    local_x: i32,
    local_z: i32,
    world_x: i32,
    world_z: i32,
    y: i32,
    sea_level: i32,
    surface_depth: i32,
    cosine_bands: bool,
    biome_top_material: u8,
    under_material: u8,
    top_placed: &mut bool,
) {
    if y < sea_level - 1 {
        set_block_at_y_if_inside(chunk, local_x, y, local_z, under_material);
        if under_material == WHITE_TERRACOTTA {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, ORANGE_TERRACOTTA);
        }
    } else if y > 86 + surface_depth * 2 {
        set_block_at_y_if_inside(
            chunk,
            local_x,
            y,
            local_z,
            if cosine_bands {
                COARSE_DIRT
            } else {
                GRASS_BLOCK
            },
        );
    } else if y <= sea_level + 3 + surface_depth {
        set_block_at_y_if_inside(chunk, local_x, y, local_z, biome_top_material);
        *top_placed = true;
    } else {
        let replacement = get_badlands_ceiling_block(state, world_x, y, world_z, cosine_bands);
        set_block_at_y_if_inside(chunk, local_x, y, local_z, replacement);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_frozen_ocean_surface(
    random: &mut WorldgenRandom,
    chunk: &mut MutableChunkBlockBuffer,
    biome: BiomeDefinition,
    world_x: i32,
    world_z: i32,
    height: i32,
    noise: f64,
    sea_level: i32,
    min_surface_level: i32,
    config: SurfaceBuilderConfiguration,
    seed: i64,
) {
    let mut iceberg_height = 0.0;
    let mut iceberg_base_y = 0.0;
    let state = frozen_ocean_noise_state(seed);
    let surface_temperature = biome_temperature(biome, world_x, 63, world_z);
    let iceberg_noise = noise.abs().min(
        state
            .iceberg_noise
            .get_value(world_x as f64 * 0.1, world_z as f64 * 0.1, false)
            * 15.0,
    );

    if iceberg_noise > 1.8 {
        let roof_noise = state
            .iceberg_roof_noise
            .get_value(
                world_x as f64 * 0.097_656_25,
                world_z as f64 * 0.097_656_25,
                false,
            )
            .abs();
        iceberg_height = iceberg_noise * iceberg_noise * 1.2;
        let max_iceberg_height = (roof_noise * 40.0).ceil() + 14.0;
        if iceberg_height > max_iceberg_height {
            iceberg_height = max_iceberg_height;
        }

        if surface_temperature > 0.1 {
            iceberg_height -= 2.0;
        }

        if iceberg_height > 2.0 {
            iceberg_base_y = sea_level as f64 - iceberg_height - 7.0;
            iceberg_height += sea_level as f64;
        } else {
            iceberg_height = 0.0;
        }
    }

    let iceberg_height_int = iceberg_height as i32;
    let iceberg_base_y_int = iceberg_base_y as i32;
    let local_x = local_coord(world_x);
    let local_z = local_coord(world_z);
    let mut under_material = config.under_material;
    let mut top_material = config.top_material;
    let surface_depth = (noise / 3.0 + 3.0 + random.next_double() * 0.25) as i32;
    let mut remaining_depth = -1;
    let mut snow_layers_placed = 0;
    let max_snow_layers = 2 + random.next_int_bound(4);
    let snow_start_y = sea_level + 18 + random.next_int_bound(10);

    for y in (min_surface_level..=height.max(iceberg_height_int + 1)).rev() {
        let mut block_id = get_block_at_y_or_air(chunk, local_x, y, local_z);
        if block_id == AIR && y < iceberg_height_int && random.next_double() > 0.01 {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, PACKED_ICE);
            block_id = PACKED_ICE;
        } else if block_id == WATER
            && y > iceberg_base_y_int
            && y < sea_level
            && iceberg_base_y != 0.0
            && random.next_double() > 0.15
        {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, PACKED_ICE);
            block_id = PACKED_ICE;
        }

        if block_id == AIR {
            remaining_depth = -1;
            continue;
        }

        if block_id == STONE {
            if remaining_depth == -1 {
                if surface_depth <= 0 {
                    top_material = AIR;
                    under_material = STONE;
                } else if y >= sea_level - 4 && y <= sea_level + 1 {
                    top_material = config.top_material;
                    under_material = config.under_material;
                }

                if y < sea_level && top_material == AIR {
                    top_material = if biome_temperature(biome, world_x, y, world_z) < 0.15 {
                        ICE
                    } else {
                        WATER
                    };
                }

                remaining_depth = surface_depth;
                if y >= sea_level - 1 {
                    set_block_at_y_if_inside(chunk, local_x, y, local_z, top_material);
                } else if y < sea_level - 7 - surface_depth {
                    top_material = AIR;
                    under_material = STONE;
                    set_block_at_y_if_inside(chunk, local_x, y, local_z, GRAVEL);
                } else {
                    set_block_at_y_if_inside(chunk, local_x, y, local_z, under_material);
                }
            } else if remaining_depth > 0 {
                remaining_depth -= 1;
                set_block_at_y_if_inside(chunk, local_x, y, local_z, under_material);
                if remaining_depth == 0
                    && (under_material == SAND || under_material == RED_SAND)
                    && surface_depth > 1
                {
                    remaining_depth = random.next_int_bound(4) + (y - 63).max(0);
                    under_material = if under_material == RED_SAND {
                        RED_SANDSTONE
                    } else {
                        SANDSTONE
                    };
                }
            }
        } else if block_id == PACKED_ICE
            && snow_layers_placed <= max_snow_layers
            && y > snow_start_y
        {
            set_block_at_y_if_inside(chunk, local_x, y, local_z, SNOW_BLOCK);
            snow_layers_placed += 1;
        }
    }
}

fn is_terracotta(block_id: u8) -> bool {
    matches!(
        block_id,
        TERRACOTTA
            | WHITE_TERRACOTTA
            | ORANGE_TERRACOTTA
            | MAGENTA_TERRACOTTA
            | LIGHT_BLUE_TERRACOTTA
            | YELLOW_TERRACOTTA
            | LIME_TERRACOTTA
            | PINK_TERRACOTTA
            | GRAY_TERRACOTTA
            | LIGHT_GRAY_TERRACOTTA
            | CYAN_TERRACOTTA
            | PURPLE_TERRACOTTA
            | BLUE_TERRACOTTA
            | BROWN_TERRACOTTA
            | GREEN_TERRACOTTA
            | RED_TERRACOTTA
            | BLACK_TERRACOTTA
    )
}

fn get_badlands_ceiling_block(
    state: &BadlandsNoiseState,
    world_x: i32,
    y: i32,
    world_z: i32,
    cosine_bands: bool,
) -> u8 {
    if !(64..=127).contains(&y) {
        ORANGE_TERRACOTTA
    } else if cosine_bands {
        TERRACOTTA
    } else {
        get_badlands_band(state, world_x, y, world_z)
    }
}

fn get_badlands_band(state: &BadlandsNoiseState, world_x: i32, y: i32, world_z: i32) -> u8 {
    let offset = java_round(
        state.clay_bands_offset_noise.get_value(
            world_x as f64 / 512.0,
            world_z as f64 / 512.0,
            false,
        ) * 2.0,
    );
    let index = (y + offset + BADLANDS_BAND_LENGTH as i32).rem_euclid(BADLANDS_BAND_LENGTH as i32);
    state.clay_bands[index as usize]
}

fn create_badlands_noise_state(seed: i64) -> BadlandsNoiseState {
    let mut clay_bands = vec![TERRACOTTA; BADLANDS_BAND_LENGTH];
    let mut band_random = WorldgenRandom::new(seed);
    let clay_bands_offset_noise =
        PerlinSimplexNoise::from_octaves(&mut band_random, &BADLANDS_BAND_OFFSET_OCTAVES);

    let mut index = 0;
    while index < BADLANDS_BAND_LENGTH as i32 {
        index += band_random.next_int_bound(5) + 1;
        if index < BADLANDS_BAND_LENGTH as i32 {
            clay_bands[index as usize] = ORANGE_TERRACOTTA;
        }
        index += 1;
    }

    let yellow_bands = band_random.next_int_bound(4) + 2;
    for _ in 0..yellow_bands {
        let band_length = band_random.next_int_bound(3) + 1;
        let start = band_random.next_int_bound(BADLANDS_BAND_LENGTH as i32);
        for offset in 0..band_length {
            let band_index = start + offset;
            if band_index < BADLANDS_BAND_LENGTH as i32 {
                clay_bands[band_index as usize] = YELLOW_TERRACOTTA;
            }
        }
    }

    let brown_bands = band_random.next_int_bound(4) + 2;
    for _ in 0..brown_bands {
        let band_length = band_random.next_int_bound(3) + 2;
        let start = band_random.next_int_bound(BADLANDS_BAND_LENGTH as i32);
        for offset in 0..band_length {
            let band_index = start + offset;
            if band_index < BADLANDS_BAND_LENGTH as i32 {
                clay_bands[band_index as usize] = BROWN_TERRACOTTA;
            }
        }
    }

    let red_bands = band_random.next_int_bound(4) + 2;
    for _ in 0..red_bands {
        let band_length = band_random.next_int_bound(3) + 1;
        let start = band_random.next_int_bound(BADLANDS_BAND_LENGTH as i32);
        for offset in 0..band_length {
            let band_index = start + offset;
            if band_index < BADLANDS_BAND_LENGTH as i32 {
                clay_bands[band_index as usize] = RED_TERRACOTTA;
            }
        }
    }

    let white_bands = band_random.next_int_bound(3) + 3;
    let mut start = 0;
    for _ in 0..white_bands {
        start += band_random.next_int_bound(16) + 4;
        if start < BADLANDS_BAND_LENGTH as i32 {
            clay_bands[start as usize] = WHITE_TERRACOTTA;
            if start > 1 && band_random.next_boolean() {
                clay_bands[(start - 1) as usize] = LIGHT_GRAY_TERRACOTTA;
            }
            if start < BADLANDS_BAND_LENGTH as i32 - 1 && band_random.next_boolean() {
                clay_bands[(start + 1) as usize] = LIGHT_GRAY_TERRACOTTA;
            }
        }
    }

    let mut pillar_random = WorldgenRandom::new(seed);
    BadlandsNoiseState {
        clay_bands,
        clay_bands_offset_noise,
        pillar_noise: PerlinSimplexNoise::from_octaves(
            &mut pillar_random,
            &BADLANDS_PILLAR_OCTAVES,
        ),
        pillar_roof_noise: PerlinSimplexNoise::from_octaves(
            &mut pillar_random,
            &FROZEN_OCEAN_ICEBERG_ROOF_OCTAVES,
        ),
    }
}

fn badlands_noise_state(seed: i64) -> Arc<BadlandsNoiseState> {
    static STATES: OnceLock<Mutex<HashMap<i64, Arc<BadlandsNoiseState>>>> = OnceLock::new();
    let states = STATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut states = states.lock().expect("badlands noise state mutex poisoned");
    if let Some(state) = states.get(&seed) {
        return Arc::clone(state);
    }

    let state = Arc::new(create_badlands_noise_state(seed));
    states.insert(seed, Arc::clone(&state));
    state
}

fn create_frozen_ocean_noise_state(seed: i64) -> FrozenOceanNoiseState {
    let mut random = WorldgenRandom::new(seed);
    FrozenOceanNoiseState {
        iceberg_noise: PerlinSimplexNoise::from_octaves(&mut random, &FROZEN_OCEAN_ICEBERG_OCTAVES),
        iceberg_roof_noise: PerlinSimplexNoise::from_octaves(
            &mut random,
            &FROZEN_OCEAN_ICEBERG_ROOF_OCTAVES,
        ),
    }
}

fn frozen_ocean_noise_state(seed: i64) -> Arc<FrozenOceanNoiseState> {
    static STATES: OnceLock<Mutex<HashMap<i64, Arc<FrozenOceanNoiseState>>>> = OnceLock::new();
    let states = STATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut states = states
        .lock()
        .expect("frozen ocean noise state mutex poisoned");
    if let Some(state) = states.get(&seed) {
        return Arc::clone(state);
    }

    let state = Arc::new(create_frozen_ocean_noise_state(seed));
    states.insert(seed, Arc::clone(&state));
    state
}

fn java_round(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

fn biome_temperature(biome: BiomeDefinition, x: i32, y: i32, z: i32) -> f32 {
    let mut temperature = biome_base_temperature(biome);
    if matches!(
        biome.key(),
        "minecraft:frozen_ocean" | "minecraft:deep_frozen_ocean"
    ) {
        temperature = frozen_temperature_modifier(temperature, x, z);
    }

    if y > 64 {
        let noise = temperature_noise().get_value(x as f64 / 8.0, z as f64 / 8.0, false) * 4.0;
        temperature -= ((noise as f32 + y as f32) - 64.0) * 0.05 / 30.0;
    }

    temperature
}

fn biome_base_temperature(biome: BiomeDefinition) -> f32 {
    match biome.key() {
        "minecraft:desert"
        | "minecraft:desert_hills"
        | "minecraft:desert_lakes"
        | "minecraft:badlands"
        | "minecraft:badlands_plateau"
        | "minecraft:wooded_badlands_plateau"
        | "minecraft:eroded_badlands"
        | "minecraft:modified_wooded_badlands_plateau"
        | "minecraft:modified_badlands_plateau" => 2.0,
        "minecraft:frozen_ocean"
        | "minecraft:frozen_river"
        | "minecraft:snowy_tundra"
        | "minecraft:snowy_mountains"
        | "minecraft:ice_spikes" => 0.0,
        "minecraft:snowy_beach" => 0.05,
        "minecraft:snowy_taiga"
        | "minecraft:snowy_taiga_hills"
        | "minecraft:snowy_taiga_mountains" => -0.5,
        "minecraft:mountains" | "minecraft:mountain_edge" | "minecraft:wooded_mountains" => 0.2,
        "minecraft:taiga" | "minecraft:taiga_hills" | "minecraft:taiga_mountains" => 0.25,
        "minecraft:deep_frozen_ocean" => 0.5,
        _ => 0.5,
    }
}

fn frozen_temperature_modifier(base_temperature: f32, x: i32, z: i32) -> f32 {
    let frozen_noise =
        frozen_temperature_noise().get_value(x as f64 * 0.05, z as f64 * 0.05, false) * 7.0;
    let biome_noise = biome_info_noise().get_value(x as f64 * 0.2, z as f64 * 0.2, false);
    if frozen_noise + biome_noise < 0.3 {
        let detail_noise = biome_info_noise().get_value(x as f64 * 0.09, z as f64 * 0.09, false);
        if detail_noise < 0.8 {
            return 0.2;
        }
    }

    base_temperature
}

fn biome_info_noise() -> &'static PerlinSimplexNoise {
    static NOISE: OnceLock<PerlinSimplexNoise> = OnceLock::new();
    NOISE.get_or_init(|| {
        let mut random = WorldgenRandom::new(2345);
        PerlinSimplexNoise::from_octaves(&mut random, &BIOME_INFO_NOISE_OCTAVES)
    })
}

fn temperature_noise() -> &'static PerlinSimplexNoise {
    static NOISE: OnceLock<PerlinSimplexNoise> = OnceLock::new();
    NOISE.get_or_init(|| {
        let mut random = WorldgenRandom::new(1234);
        PerlinSimplexNoise::from_octaves(&mut random, &TEMPERATURE_NOISE_OCTAVES)
    })
}

fn frozen_temperature_noise() -> &'static PerlinSimplexNoise {
    static NOISE: OnceLock<PerlinSimplexNoise> = OnceLock::new();
    NOISE.get_or_init(|| {
        let mut random = WorldgenRandom::new(3456);
        PerlinSimplexNoise::from_octaves(&mut random, &FROZEN_TEMPERATURE_NOISE_OCTAVES)
    })
}

fn local_coord(world_coord: i32) -> i32 {
    world_coord & (CHUNK_WIDTH - 1)
}

fn is_inside_chunk_y(chunk: &MutableChunkBlockBuffer, y: i32) -> bool {
    y >= chunk.min_y && y < chunk.min_y + chunk.height
}

fn get_block_at_y_or_air(
    chunk: &MutableChunkBlockBuffer,
    local_x: i32,
    y: i32,
    local_z: i32,
) -> u8 {
    if is_inside_chunk_y(chunk, y) {
        chunk.get_block_at_y(local_x, y, local_z)
    } else {
        AIR
    }
}

fn set_block_at_y_if_inside(
    chunk: &mut MutableChunkBlockBuffer,
    local_x: i32,
    y: i32,
    local_z: i32,
    block_id: u8,
) {
    if is_inside_chunk_y(chunk, y) {
        chunk.set_block_at_y(local_x, y, local_z, block_id);
    }
}
