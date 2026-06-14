use crate::biome::BiomeDefinition;
use crate::levelgen::MutableChunkBlockBuffer;
use crate::noise::PerlinSimplexNoise;
use crate::prng::WorldgenRandom;
use std::sync::OnceLock;

const CHUNK_WIDTH: i32 = 16;

const AIR: u8 = 0;
const STONE: u8 = 1;
const WATER: u8 = 2;
const GRASS_BLOCK: u8 = 4;
const DIRT: u8 = 5;
const SAND: u8 = 6;
const GRAVEL: u8 = 7;
const COARSE_DIRT: u8 = 13;
const PODZOL: u8 = 14;
const MYCELIUM: u8 = 15;
const WHITE_TERRACOTTA: u8 = 17;
const SANDSTONE: u8 = 33;
const RED_SAND: u8 = 38;
const ICE: u8 = 39;
const SNOW_BLOCK: u8 = 40;

const BIOME_INFO_NOISE_OCTAVES: [i32; 1] = [0];

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
        | SurfaceBuilderKind::ErodedBadlands
        | SurfaceBuilderKind::FrozenOcean => {
            panic!(
                "surface builder {:?} is outside the current native surface slice",
                definition.builder
            );
        }
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

fn biome_temperature(biome: BiomeDefinition, _x: i32, _y: i32, _z: i32) -> f32 {
    match biome.key() {
        "minecraft:snowy_tundra"
        | "minecraft:snowy_mountains"
        | "minecraft:frozen_river"
        | "minecraft:ice_spikes" => 0.0,
        "minecraft:snowy_beach" => 0.05,
        "minecraft:snowy_taiga"
        | "minecraft:snowy_taiga_hills"
        | "minecraft:snowy_taiga_mountains" => -0.5,
        _ => 0.5,
    }
}

fn biome_info_noise() -> &'static PerlinSimplexNoise {
    static NOISE: OnceLock<PerlinSimplexNoise> = OnceLock::new();
    NOISE.get_or_init(|| {
        let mut random = WorldgenRandom::new(2345);
        PerlinSimplexNoise::from_octaves(&mut random, &BIOME_INFO_NOISE_OCTAVES)
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
