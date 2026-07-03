#[cfg(test)]
use crate::block::AIR;
#[cfg(test)]
use crate::feature::{FEATURES_WRITE_RADIUS_CUTOFF, FeatureRegion};
#[cfg(test)]
use crate::noise::{BlendedNoise, PerlinNoise, SimplexNoise};
#[cfg(test)]
use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_block_index, chunk_min_block_coord};

mod chunk;
mod feature_batch;
mod generator;
mod sampler;
mod settings;
mod timing;

pub use crate::biome::{ConstantBiomeSource, NoiseBiome, NoiseBiomeSource};
pub use chunk::{GeneratedChunk, MutableChunkBlockBuffer, ScheduledTick};
pub use feature_batch::{
    OverworldFeatureBatchResult, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, generate_overworld_features_chunk,
    generate_overworld_features_chunks, generate_overworld_surface_chunk,
};
pub use generator::NoiseBasedChunkGenerator;
pub use sampler::NoiseSampler;
pub use settings::{
    NoiseGeneratorSettings, NoiseModifier, NoiseSamplingSettings, NoiseSettings, NoiseSlideSettings,
};
pub use timing::{
    OverworldDependencyGenerationTiming, OverworldFeatureBatchTiming, SurfaceFillTiming,
};

#[cfg(test)]
use feature_batch::{
    FeatureBatchPlan, generate_overworld_liquid_carved_buffer_with_biome_source,
    sorted_chunk_positions_z_major,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::OverworldBiomeSource;
    use crate::block::{
        CLAY, COARSE_DIRT, DANDELION, DEAD_BUSH, FERN, GRASS, GRASS_BLOCK, GRAVEL, ICE,
        LARGE_FERN_LOWER, LARGE_FERN_UPPER, MYCELIUM, OAK_LEAVES, OAK_LOG, PACKED_ICE, PODZOL,
        POPPY, RED_SAND, RawBlockId, SAND, SNOW, SNOW_BLOCK, SPRUCE_LEAVES, SPRUCE_LOG, STONE,
        TERRACOTTA, WATER, is_air_like, is_water,
    };
    use crate::feature::FeatureWorld;
    use crate::prng::WorldgenRandom;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::panic;

    const DEPTH_NOISE_OCTAVES: [i32; 16] = [
        -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
    ];
    const BEDROCK: u8 = 3;

    #[derive(Clone, Copy, Debug)]
    struct PaletteMatrixCase {
        seed: i64,
        chunk_x: i32,
        chunk_z: i32,
        biome_key: &'static str,
        surface_family: SurfaceFamily,
        feature_family: Option<FeatureFamily>,
    }

    #[derive(Clone, Copy, Debug)]
    enum SurfaceFamily {
        Grass,
        Sand,
        Snow,
        Badlands,
        Swamp,
        Water,
        FrozenWater,
        Mountain,
        Mycelium,
        GiantTaiga,
        ShatteredSavanna,
        SnowySand,
    }

    impl SurfaceFamily {
        const fn name(self) -> &'static str {
            match self {
                Self::Grass => "grass",
                Self::Sand => "sand",
                Self::Snow => "snow",
                Self::Badlands => "badlands",
                Self::Swamp => "swamp grass/water",
                Self::Water => "water",
                Self::FrozenWater => "frozen water/ice",
                Self::Mountain => "mountain grass/stone/gravel",
                Self::Mycelium => "mycelium",
                Self::GiantTaiga => "giant taiga grass/podzol/coarse dirt",
                Self::ShatteredSavanna => "shattered savanna grass/coarse dirt/stone",
                Self::SnowySand => "snowy sand",
            }
        }

        const fn contains(self, block: RawBlockId) -> bool {
            match self {
                Self::Grass => block == GRASS_BLOCK,
                Self::Sand => block == SAND,
                Self::Snow => matches!(block, SNOW | SNOW_BLOCK | ICE | PACKED_ICE),
                Self::Badlands => matches!(block, RED_SAND | TERRACOTTA),
                Self::Swamp => block == GRASS_BLOCK || is_water(block),
                Self::Water => is_water(block),
                Self::FrozenWater => {
                    matches!(block, ICE | PACKED_ICE | SNOW | SNOW_BLOCK) || is_water(block)
                }
                Self::Mountain => matches!(block, GRASS_BLOCK | STONE | GRAVEL),
                Self::Mycelium => block == MYCELIUM,
                Self::GiantTaiga => matches!(block, GRASS_BLOCK | PODZOL | COARSE_DIRT),
                Self::ShatteredSavanna => matches!(block, GRASS_BLOCK | COARSE_DIRT | STONE),
                Self::SnowySand => matches!(block, SAND | SNOW | SNOW_BLOCK | ICE | PACKED_ICE),
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum FeatureFamily {
        PlainsVegetation,
        DesertDeadBush,
        SwampNativeSubset,
        TaigaSpruceFern,
        SnowySpruceFern,
        BadlandsDeadBush,
    }

    impl FeatureFamily {
        fn name(self) -> &'static str {
            match self {
                Self::PlainsVegetation => "plains grass/flower/oak",
                Self::DesertDeadBush => "desert dead bush",
                Self::SwampNativeSubset => "native swamp vegetation/clay subset",
                Self::TaigaSpruceFern => "taiga spruce/fern",
                Self::SnowySpruceFern => "snowy spruce/fern",
                Self::BadlandsDeadBush => "badlands dead bush",
            }
        }

        fn blocks(self) -> &'static [RawBlockId] {
            match self {
                Self::PlainsVegetation => &[OAK_LOG, OAK_LEAVES, GRASS, DANDELION, POPPY],
                Self::DesertDeadBush => &[DEAD_BUSH],
                Self::SwampNativeSubset => &[OAK_LOG, OAK_LEAVES, GRASS, POPPY, DEAD_BUSH, CLAY],
                Self::TaigaSpruceFern => &[
                    SPRUCE_LOG,
                    SPRUCE_LEAVES,
                    FERN,
                    LARGE_FERN_LOWER,
                    LARGE_FERN_UPPER,
                ],
                Self::SnowySpruceFern => &[SPRUCE_LOG, SPRUCE_LEAVES, FERN],
                Self::BadlandsDeadBush => &[DEAD_BUSH],
            }
        }
    }

    const PALETTE_MATRIX_CASES: &[PaletteMatrixCase] = &[
        PaletteMatrixCase {
            seed: 1,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 16,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:plains",
            surface_family: SurfaceFamily::Grass,
            feature_family: Some(FeatureFamily::PlainsVegetation),
        },
        PaletteMatrixCase {
            seed: 38,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:desert",
            surface_family: SurfaceFamily::Sand,
            feature_family: Some(FeatureFamily::DesertDeadBush),
        },
        PaletteMatrixCase {
            seed: 31,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:mountains",
            surface_family: SurfaceFamily::Mountain,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 0,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:forest",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 7,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:swamp",
            surface_family: SurfaceFamily::Swamp,
            feature_family: Some(FeatureFamily::SwampNativeSubset),
        },
        PaletteMatrixCase {
            seed: 39,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:river",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 333,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:frozen_ocean",
            surface_family: SurfaceFamily::FrozenWater,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 44,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:dark_forest",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 14,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:snowy_taiga",
            surface_family: SurfaceFamily::Snow,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 19,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:giant_tree_taiga",
            surface_family: SurfaceFamily::GiantTaiga,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 978,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:mushroom_fields",
            surface_family: SurfaceFamily::Mycelium,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 45,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:beach",
            surface_family: SurfaceFamily::Sand,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 330,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:snowy_beach",
            surface_family: SurfaceFamily::SnowySand,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 71,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:jungle",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 2235,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:jungle_edge",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 10,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:birch_forest",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 125,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:taiga",
            surface_family: SurfaceFamily::Grass,
            feature_family: Some(FeatureFamily::TaigaSpruceFern),
        },
        PaletteMatrixCase {
            seed: 42,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:snowy_tundra",
            surface_family: SurfaceFamily::Snow,
            feature_family: Some(FeatureFamily::SnowySpruceFern),
        },
        PaletteMatrixCase {
            seed: 147,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:badlands",
            surface_family: SurfaceFamily::Badlands,
            feature_family: Some(FeatureFamily::BadlandsDeadBush),
        },
        PaletteMatrixCase {
            seed: 26,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:warm_ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 6,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:lukewarm_ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 5,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:cold_ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 103,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:deep_frozen_ocean",
            surface_family: SurfaceFamily::FrozenWater,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 2923,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:giant_spruce_taiga",
            surface_family: SurfaceFamily::GiantTaiga,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 62,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:savanna",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 126,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:savanna_plateau",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 68,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:shattered_savanna",
            surface_family: SurfaceFamily::ShatteredSavanna,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 252,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:frozen_river",
            surface_family: SurfaceFamily::FrozenWater,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 326,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:snowy_mountains",
            surface_family: SurfaceFamily::Snow,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 1554,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:mushroom_field_shore",
            surface_family: SurfaceFamily::Mycelium,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 120,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:desert_hills",
            surface_family: SurfaceFamily::Sand,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 2,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:wooded_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 29,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:taiga_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 146,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:jungle_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 4,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:deep_ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 167,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:stone_shore",
            surface_family: SurfaceFamily::Mountain,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 30,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:birch_forest_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 886,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:snowy_taiga_hills",
            surface_family: SurfaceFamily::Snow,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 93,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:giant_tree_taiga_hills",
            surface_family: SurfaceFamily::GiantTaiga,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 3,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:wooded_mountains",
            surface_family: SurfaceFamily::Mountain,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 86,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:wooded_badlands_plateau",
            surface_family: SurfaceFamily::Badlands,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 84,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:badlands_plateau",
            surface_family: SurfaceFamily::Badlands,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 56,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:deep_lukewarm_ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 13,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:deep_cold_ocean",
            surface_family: SurfaceFamily::Water,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 25,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:sunflower_plains",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 98,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:desert_lakes",
            surface_family: SurfaceFamily::Sand,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 212,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:gravelly_mountains",
            surface_family: SurfaceFamily::Mountain,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 135,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:flower_forest",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 1326,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:taiga_mountains",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 1094,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:swamp_hills",
            surface_family: SurfaceFamily::Swamp,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 59,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:ice_spikes",
            surface_family: SurfaceFamily::Snow,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 1374,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:modified_jungle",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 314_096,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:modified_jungle_edge",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 48,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:tall_birch_forest",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 1557,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:tall_birch_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 410,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:dark_forest_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 12_006,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:snowy_taiga_mountains",
            surface_family: SurfaceFamily::Snow,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 282,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:giant_spruce_taiga_hills",
            surface_family: SurfaceFamily::GiantTaiga,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 83,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:modified_gravelly_mountains",
            surface_family: SurfaceFamily::Mountain,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 175,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:shattered_savanna_plateau",
            surface_family: SurfaceFamily::ShatteredSavanna,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 8464,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:eroded_badlands",
            surface_family: SurfaceFamily::Badlands,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 3823,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:modified_wooded_badlands_plateau",
            surface_family: SurfaceFamily::Badlands,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 18_441,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:modified_badlands_plateau",
            surface_family: SurfaceFamily::Badlands,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 626,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:bamboo_jungle",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
        PaletteMatrixCase {
            seed: 1000,
            chunk_x: 0,
            chunk_z: 0,
            biome_key: "minecraft:bamboo_jungle_hills",
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        },
    ];

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        settings_preset: String,
        noise_modifier: String,
        seed: String,
        cell_width: i32,
        cell_height: i32,
        cell_count_y: i32,
        biome_y: i32,
        min_cell_y: i32,
        column_value_count: usize,
        wire_format: NoiseSamplerWireFormatFixture,
        noise_settings: NoiseSettingsFixture,
        sample_sets: BTreeMap<String, NoiseSamplerSampleSetFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerWireFormatFixture {
        coordinates: String,
        biome_keys: String,
        biome_factors: String,
        values: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSettingsFixture {
        min_y: i32,
        height: i32,
        sampling: NoiseSamplingSettingsFixture,
        top_slide: NoiseSlideSettingsFixture,
        bottom_slide: NoiseSlideSettingsFixture,
        noise_size_horizontal: i32,
        noise_size_vertical: i32,
        density_factor: f64,
        density_offset: f64,
        use_simplex_surface_noise: bool,
        random_density_offset: bool,
        island_noise_override: bool,
        is_amplified: bool,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplingSettingsFixture {
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
    }

    #[derive(Debug, Deserialize)]
    struct NoiseSlideSettingsFixture {
        target: i32,
        size: i32,
        offset: i32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerSampleSetFixture {
        biome_pattern: NoiseSamplerBiomePatternFixture,
        columns: NoiseSamplerColumnFixture,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerBiomePatternFixture {
        grid_order: String,
        sample_count: usize,
        x: Vec<i32>,
        z: Vec<i32>,
        keys: Vec<String>,
        depths: Vec<f32>,
        scales: Vec<f32>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerColumnFixture {
        noise_method: String,
        grid_order: String,
        column_value_count: usize,
        sample_count: usize,
        x: Vec<i32>,
        z: Vec<i32>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TerrainChunkOracleFixture {
        module: String,
        minecraft_version: String,
        generator_class: String,
        seed: String,
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        block_order: String,
        palette: Vec<String>,
        blocks: Vec<u8>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkOracleFixture {
        module: String,
        minecraft_version: String,
        seed: String,
        generator: String,
        generate_structures: bool,
        wire_format: FullChunkWireFormatFixture,
        chunks: Vec<FullChunkEntryFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkWireFormatFixture {
        block_order: String,
        palette_entries: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkEntryFixture {
        chunk_x: i32,
        chunk_z: i32,
        status: String,
        sections: Vec<FullChunkSectionFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SchedulerTraceFixture {
        module: String,
        minecraft_version: String,
        seed: String,
        target_chunk_x: i32,
        target_chunk_z: i32,
        target_radius: i32,
        stop_status: String,
        #[serde(rename = "featureCompletionOrder3x3")]
        feature_completion_order_3x3: Vec<SchedulerTraceChunkFixture>,
        #[serde(default)]
        chunks: Vec<FullChunkEntryFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SchedulerTraceChunkFixture {
        chunk_x: i32,
        chunk_z: i32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkSectionFixture {
        y: i32,
        palette: Vec<String>,
        block_order: String,
        blocks: Vec<usize>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct FullChunkDiffReport {
        total_blocks: usize,
        matched_blocks: usize,
        mismatched_blocks: usize,
        top_mismatch_pairs: Vec<MismatchBucket>,
        first_mismatches: Vec<BlockMismatch>,
    }

    impl FullChunkDiffReport {
        fn is_exact(&self) -> bool {
            self.mismatched_blocks == 0
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MismatchBucket {
        actual: String,
        expected: String,
        count: usize,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct BlockMismatch {
        local_x: i32,
        y: i32,
        local_z: i32,
        actual: String,
        expected: String,
    }

    #[derive(Clone, Debug)]
    struct RepeatingPatternBiomeSource {
        width: usize,
        height: usize,
        biomes: Vec<NoiseBiome>,
    }

    impl RepeatingPatternBiomeSource {
        fn new(pattern: &NoiseSamplerBiomePatternFixture) -> Self {
            Self {
                width: pattern.x.len(),
                height: pattern.z.len(),
                biomes: pattern
                    .depths
                    .iter()
                    .zip(&pattern.scales)
                    .map(|(depth, scale)| NoiseBiome::new(*depth, *scale))
                    .collect(),
            }
        }
    }

    impl NoiseBiomeSource for RepeatingPatternBiomeSource {
        fn get_noise_biome(&self, x: i32, _y: i32, z: i32) -> NoiseBiome {
            let wrapped_x = x.rem_euclid(self.width as i32) as usize;
            let wrapped_z = z.rem_euclid(self.height as i32) as usize;
            self.biomes[wrapped_x * self.height + wrapped_z]
        }
    }

    fn fixtures() -> Vec<NoiseSamplerFixture> {
        [
            include_str!("../../../../test/fixtures/noise/noise-sampler-overworld-seed-0.json"),
            include_str!("../../../../test/fixtures/noise/noise-sampler-overworld-seed-1.json"),
            include_str!("../../../../test/fixtures/noise/noise-sampler-overworld-seed-12345.json"),
            include_str!(
                "../../../../test/fixtures/noise/noise-sampler-overworld-seed-2151901553968352745.json"
            ),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid NoiseSampler fixture"))
        .collect()
    }

    fn terrain_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json"
        ))
        .expect("valid terrain chunk oracle fixture")
    }

    fn surface_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json"
        ))
        .expect("valid surface chunk oracle fixture")
    }

    fn frozen_ocean_surface_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json"
        ))
        .expect("valid frozen ocean surface chunk oracle fixture")
    }

    fn badlands_surface_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json"
        ))
        .expect("valid badlands surface chunk oracle fixture")
    }

    fn full_chunk_fixture() -> FullChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0.json"
        ))
        .expect("valid full chunk oracle fixture")
    }

    fn scheduler_trace_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json"
        ))
        .expect("valid vanilla scheduler trace fixture")
    }

    fn scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-12345-chunk-0-0.json"
        ))
        .expect("valid vanilla scheduler features snapshot fixture")
    }

    fn plains_scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-16-chunk-0-0-plains.json"
        ))
        .expect("valid vanilla plains scheduler features snapshot fixture")
    }

    fn inland_plains_scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-17-chunk-0-0-plains-inland.json"
        ))
        .expect("valid vanilla inland plains scheduler features snapshot fixture")
    }

    fn forest_seed_23823_scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-23823-chunk--13-8-forest.json"
        ))
        .expect("valid vanilla forest scheduler features snapshot fixture")
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TaigaTreeIndexShiftCenterDiagnostic {
        center: ChunkPos,
        biome_key: &'static str,
        current_tree_blocks: usize,
        java_tree_blocks: usize,
        shared_tree_blocks: usize,
        current_only_tree_blocks: usize,
        java_only_tree_blocks: usize,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TaigaFullTableTreeDeltaDiagnostic {
        center: ChunkPos,
        biome_key: &'static str,
        before_tree_blocks: usize,
        after_tree_blocks: usize,
        added_tree_blocks: usize,
        removed_tree_blocks: usize,
        feature_random_calls: usize,
        random_count: usize,
        added_tree_block_samples: Vec<TreeBlockSample>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TreeBlockSample {
        local_x: i32,
        y: i32,
        local_z: i32,
        block_id: RawBlockId,
    }

    fn chunk_primary_biome_key(biome_source: &OverworldBiomeSource, pos: ChunkPos) -> &'static str {
        biome_source
            .get_primary_biome_definition(pos.x, pos.z)
            .key()
    }

    fn is_taiga_vegetation_biome(key: &str) -> bool {
        matches!(
            key,
            "minecraft:taiga"
                | "minecraft:taiga_hills"
                | "minecraft:taiga_mountains"
                | "minecraft:giant_tree_taiga"
                | "minecraft:giant_tree_taiga_hills"
                | "minecraft:giant_spruce_taiga"
                | "minecraft:giant_spruce_taiga_hills"
        )
    }

    fn target_tree_blocks_after_taiga_vegetation_center(
        seed: i64,
        target: ChunkPos,
        center: ChunkPos,
        feature_index: i32,
    ) -> BTreeMap<(i32, i32, i32), RawBlockId> {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let chunks = (center.z - FEATURES_WRITE_RADIUS_CUTOFF
            ..=center.z + FEATURES_WRITE_RADIUS_CUTOFF)
            .flat_map(|chunk_z| {
                let biome_source = biome_source.clone();
                (center.x - FEATURES_WRITE_RADIUS_CUTOFF..=center.x + FEATURES_WRITE_RADIUS_CUTOFF)
                    .map(move |chunk_x| {
                        generate_overworld_liquid_carved_buffer_with_biome_source(
                            seed,
                            chunk_x,
                            chunk_z,
                            biome_source.clone(),
                        )
                    })
            })
            .collect::<Vec<_>>();
        let mut region = FeatureRegion::with_radii(
            center.x,
            center.z,
            FEATURES_WRITE_RADIUS_CUTOFF,
            FEATURES_WRITE_RADIUS_CUTOFF,
            chunks,
        );

        crate::feature::test_support::place_taiga_vegetation_with_feature_index(
            seed,
            &mut region,
            feature_index,
        );
        let chunk = region
            .remove_chunk(target.x, target.z)
            .expect("target chunk should be inside center write window");
        tree_blocks_in_chunk(&chunk)
    }

    fn tree_blocks_in_chunk(
        chunk: &MutableChunkBlockBuffer,
    ) -> BTreeMap<(i32, i32, i32), RawBlockId> {
        let mut blocks = BTreeMap::new();
        for y in chunk.min_y..chunk.min_y + chunk.height {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let block_id = chunk.get_block_at_y(x, y, z);
                    if matches!(
                        block_id,
                        crate::block::SPRUCE_LOG | crate::block::SPRUCE_LEAVES
                    ) {
                        blocks.insert((x, y, z), block_id);
                    }
                }
            }
        }
        blocks
    }

    fn tree_blocks_in_target_region(
        region: &FeatureRegion,
        target: ChunkPos,
    ) -> BTreeMap<(i32, i32, i32), RawBlockId> {
        tree_blocks_in_chunk(
            region
                .chunk(target.x, target.z)
                .expect("target chunk should be inside feature region"),
        )
    }

    fn tree_block_delta(
        before: &BTreeMap<(i32, i32, i32), RawBlockId>,
        after: &BTreeMap<(i32, i32, i32), RawBlockId>,
    ) -> (usize, usize) {
        let added = after
            .iter()
            .filter(|(pos, block_id)| before.get(pos) != Some(block_id))
            .count();
        let removed = before
            .iter()
            .filter(|(pos, block_id)| after.get(pos) != Some(block_id))
            .count();
        (added, removed)
    }

    fn added_tree_block_samples(
        before: &BTreeMap<(i32, i32, i32), RawBlockId>,
        after: &BTreeMap<(i32, i32, i32), RawBlockId>,
        limit: usize,
    ) -> Vec<TreeBlockSample> {
        let added = after
            .iter()
            .filter(|(pos, block_id)| before.get(pos) != Some(block_id))
            .collect::<Vec<_>>();
        if added.len() > limit {
            return Vec::new();
        }

        added
            .into_iter()
            .map(|(&(local_x, y, local_z), &block_id)| TreeBlockSample {
                local_x,
                y,
                local_z,
                block_id,
            })
            .collect()
    }

    fn taiga_tree_index_shift_diagnostics(
        seed: i64,
        target: ChunkPos,
    ) -> Vec<TaigaTreeIndexShiftCenterDiagnostic> {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        FeatureBatchPlan::new([target])
            .ordered_feature_centers()
            .into_iter()
            .filter_map(|center| {
                let biome_key = chunk_primary_biome_key(&biome_source, center);
                if !is_taiga_vegetation_biome(biome_key) {
                    return None;
                }

                let current = target_tree_blocks_after_taiga_vegetation_center(
                    seed,
                    target,
                    center,
                    crate::feature::test_support::CURRENT_TAIGA_VEGETATION_FEATURE_INDEX,
                );
                let java = target_tree_blocks_after_taiga_vegetation_center(
                    seed,
                    target,
                    center,
                    crate::feature::test_support::JAVA_TAIGA_VEGETATION_FEATURE_INDEX,
                );
                let shared_tree_blocks = current
                    .iter()
                    .filter(|(pos, block_id)| java.get(pos) == Some(block_id))
                    .count();
                Some(TaigaTreeIndexShiftCenterDiagnostic {
                    center,
                    biome_key,
                    current_tree_blocks: current.len(),
                    java_tree_blocks: java.len(),
                    shared_tree_blocks,
                    current_only_tree_blocks: current.len() - shared_tree_blocks,
                    java_only_tree_blocks: java.len() - shared_tree_blocks,
                })
            })
            .collect()
    }

    fn taiga_full_table_tree_delta_diagnostics(
        seed: i64,
        target: ChunkPos,
    ) -> Vec<TaigaFullTableTreeDeltaDiagnostic> {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let plan = FeatureBatchPlan::new([target]);
        let chunks = sorted_chunk_positions_z_major(plan.dependency_chunks.iter().copied())
            .into_iter()
            .map(|pos| {
                generate_overworld_liquid_carved_buffer_with_biome_source(
                    seed,
                    pos.x,
                    pos.z,
                    biome_source.clone(),
                )
            })
            .collect::<Vec<_>>();
        let first_target = *plan.targets.iter().next().expect("non-empty target plan");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, chunks);
        let feature_biomes =
            crate::feature::OverworldFeatureBiomeResolver::new(seed, &biome_source);
        let mut diagnostics = Vec::new();

        for center in plan.ordered_feature_centers() {
            region.set_center(center.x, center.z);
            let biome = biome_source.get_primary_biome_definition(center.x, center.z);
            let biome_key = biome.key();
            let min_block_x = chunk_min_block_coord(center.x);
            let min_block_z = chunk_min_block_coord(center.z);
            let origin = crate::placement::BlockPos::new(min_block_x, region.min_y(), min_block_z);
            let features = crate::feature::overworld_features_for_biome(biome);
            let mut random = WorldgenRandom::default();
            let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);

            for step_index in 0..=crate::feature::DecorationStep::TopLayerModification.index() {
                let mut feature_index = 0;
                for feature in features
                    .iter()
                    .filter(|feature| feature.step.index() == step_index)
                {
                    random.set_feature_seed(decoration_seed, feature_index, step_index);
                    let capture = is_taiga_vegetation_biome(biome_key)
                        && step_index == crate::feature::DecorationStep::VegetalDecoration.index()
                        && feature_index
                            == crate::feature::test_support::JAVA_TAIGA_VEGETATION_FEATURE_INDEX;
                    let random_count_before_feature = random.get_count();
                    let before = capture.then(|| tree_blocks_in_target_region(&region, target));
                    feature.place_with_biomes(&mut region, &feature_biomes, &mut random, origin);
                    if let Some(before) = before {
                        let after = tree_blocks_in_target_region(&region, target);
                        let (added_tree_blocks, removed_tree_blocks) =
                            tree_block_delta(&before, &after);
                        let random_count = random.get_count();
                        diagnostics.push(TaigaFullTableTreeDeltaDiagnostic {
                            center,
                            biome_key,
                            before_tree_blocks: before.len(),
                            after_tree_blocks: after.len(),
                            added_tree_blocks,
                            removed_tree_blocks,
                            feature_random_calls: random_count - random_count_before_feature,
                            random_count,
                            added_tree_block_samples: added_tree_block_samples(&before, &after, 20),
                        });
                    }
                    feature_index += 1;
                }
            }
        }

        diagnostics
    }

    fn create_noise_settings(fixture: &NoiseSamplerFixture) -> NoiseSettings {
        let settings = &fixture.noise_settings;
        NoiseSettings::create(
            settings.min_y,
            settings.height,
            NoiseSamplingSettings::new(
                settings.sampling.xz_scale,
                settings.sampling.y_scale,
                settings.sampling.xz_factor,
                settings.sampling.y_factor,
            ),
            NoiseSlideSettings::new(
                settings.top_slide.target,
                settings.top_slide.size,
                settings.top_slide.offset,
            ),
            NoiseSlideSettings::new(
                settings.bottom_slide.target,
                settings.bottom_slide.size,
                settings.bottom_slide.offset,
            ),
            settings.noise_size_horizontal,
            settings.noise_size_vertical,
            settings.density_factor,
            settings.density_offset,
            settings.use_simplex_surface_noise,
            settings.random_density_offset,
            settings.island_noise_override,
            settings.is_amplified,
        )
    }

    fn create_noise_sampler(
        fixture: &NoiseSamplerFixture,
        pattern: &NoiseSamplerBiomePatternFixture,
    ) -> NoiseSampler<RepeatingPatternBiomeSource> {
        let settings = create_noise_settings(fixture);
        let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
        let mut random = WorldgenRandom::new(seed);
        let blended_noise = BlendedNoise::new(&mut random);
        random.consume_count(2620);
        let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);

        NoiseSampler::new(
            RepeatingPatternBiomeSource::new(pattern),
            fixture.cell_width,
            fixture.cell_height,
            fixture.cell_count_y,
            settings,
            blended_noise,
            None,
            depth_noise,
            NoiseModifier::Passthrough,
        )
    }

    fn terrain_stage_blocks_from_oracle(oracle: &TerrainChunkOracleFixture) -> Vec<u8> {
        oracle
            .blocks
            .iter()
            .map(|block_id| {
                if *block_id == BEDROCK {
                    STONE
                } else {
                    *block_id
                }
            })
            .collect()
    }

    fn assert_chunk_blocks_match(actual: &[u8], expected: &[u8], min_y: i32) {
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual,
                expected,
                "chunk block mismatch at local ({}, {}, {}): expected block id {}, got {}",
                index & 15,
                (index >> 8) as i32 + min_y,
                (index >> 4) & 15,
                expected,
                actual
            );
        }
    }

    fn compare_generated_chunk_to_full_fixture(
        actual: &GeneratedChunk,
        expected: &FullChunkEntryFixture,
    ) -> FullChunkDiffReport {
        let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);
        let actual_blocks = actual
            .blocks()
            .iter()
            .map(|block_id| crate::block::block_name(*block_id).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(actual_blocks.len(), expected_blocks.len());

        let mut mismatch_counts = BTreeMap::<(String, String), usize>::new();
        let mut first_mismatches = Vec::new();
        let mut matched_blocks = 0;

        for (index, (actual_name, expected_name)) in
            actual_blocks.iter().zip(expected_blocks.iter()).enumerate()
        {
            if actual_name == expected_name {
                matched_blocks += 1;
                continue;
            }

            *mismatch_counts
                .entry((actual_name.clone(), expected_name.clone()))
                .or_default() += 1;
            if first_mismatches.len() < 12 {
                first_mismatches.push(block_mismatch_at_index(
                    index,
                    actual.min_y,
                    actual_name.clone(),
                    expected_name.clone(),
                ));
            }
        }

        let mut top_mismatch_pairs = mismatch_counts
            .into_iter()
            .map(|((actual, expected), count)| MismatchBucket {
                actual,
                expected,
                count,
            })
            .collect::<Vec<_>>();
        top_mismatch_pairs.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.actual.cmp(&right.actual))
                .then_with(|| left.expected.cmp(&right.expected))
        });
        top_mismatch_pairs.truncate(16);

        FullChunkDiffReport {
            total_blocks: actual_blocks.len(),
            matched_blocks,
            mismatched_blocks: actual_blocks.len() - matched_blocks,
            top_mismatch_pairs,
            first_mismatches,
        }
    }

    fn expand_full_fixture_blocks(
        fixture: &FullChunkEntryFixture,
        min_y: i32,
        height: i32,
    ) -> Vec<String> {
        let mut blocks = vec!["minecraft:air".to_owned(); height as usize * 16 * 16];

        for section in &fixture.sections {
            assert_eq!(section.block_order, "y-major,z-major,x-minor");
            assert_eq!(section.blocks.len(), 16 * 16 * 16);
            for (section_index, palette_index) in section.blocks.iter().copied().enumerate() {
                let local_y_in_section = section_index as i32 / 256;
                let within_layer = section_index as i32 % 256;
                let local_x = within_layer & 15;
                let local_z = (within_layer >> 4) & 15;
                let y = section.y * 16 + local_y_in_section;
                if !(min_y..min_y + height).contains(&y) {
                    continue;
                }
                let block_name = section
                    .palette
                    .get(palette_index)
                    .unwrap_or_else(|| {
                        panic!("palette index {palette_index} outside section palette")
                    })
                    .clone();
                let index = chunk_block_index(local_x, y - min_y, local_z);
                blocks[index] = block_name;
            }
        }

        blocks
    }

    fn block_mismatch_at_index(
        index: usize,
        min_y: i32,
        actual: String,
        expected: String,
    ) -> BlockMismatch {
        BlockMismatch {
            local_x: (index & 15) as i32,
            y: ((index >> 8) as i32) + min_y,
            local_z: ((index >> 4) & 15) as i32,
            actual,
            expected,
        }
    }

    fn assert_surface_chunk_matches_java_oracle(oracle: TerrainChunkOracleFixture) {
        assert_eq!(oracle.module, "surface-chunk");
        assert_eq!(oracle.minecraft_version, "1.17.1");
        assert_eq!(
            oracle.generator_class,
            "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
        );
        assert_eq!(oracle.seed, "12345");
        assert_eq!(oracle.min_y, 0);
        assert_eq!(oracle.height, 256);
        assert_eq!(oracle.block_order, "y-major,z-major,x-minor");

        let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
        let generator = NoiseBasedChunkGenerator::new(
            OverworldBiomeSource::new(seed, false, false),
            seed,
            NoiseGeneratorSettings::overworld(),
        );
        let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
        generator.build_surface_and_bedrock(&mut chunk);

        assert_eq!(chunk.chunk_x, oracle.chunk_x);
        assert_eq!(chunk.chunk_z, oracle.chunk_z);
        assert_eq!(chunk.min_y, oracle.min_y);
        assert_eq!(chunk.height, oracle.height);
        assert_chunk_blocks_match(&chunk.blocks, &oracle.blocks, oracle.min_y);
    }

    #[test]
    fn fixture_metadata_stays_consistent() {
        for fixture in fixtures() {
            assert_eq!(fixture.module, "noise");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.noise_class,
                "net.minecraft.world.level.levelgen.NoiseSampler"
            );
            assert_eq!(
                fixture.random_source_class,
                "net.minecraft.world.level.levelgen.WorldgenRandom"
            );
            assert_eq!(fixture.settings_preset, "overworld");
            assert_eq!(fixture.noise_modifier, "PASSTHROUGH");
            assert_eq!(fixture.cell_width, 4);
            assert_eq!(fixture.cell_height, 8);
            assert_eq!(fixture.cell_count_y, 32);
            assert_eq!(fixture.biome_y, 63);
            assert_eq!(fixture.min_cell_y, 0);
            assert_eq!(fixture.column_value_count, 33);
            assert_eq!(fixture.wire_format.coordinates, "integer");
            assert_eq!(fixture.wire_format.biome_keys, "string");
            assert_eq!(fixture.wire_format.biome_factors, "number");
            assert_eq!(fixture.wire_format.values, "number");
            assert!(fixture.noise_settings.use_simplex_surface_noise);
            assert!(fixture.noise_settings.random_density_offset);
            assert!(!fixture.noise_settings.island_noise_override);
            assert!(!fixture.noise_settings.is_amplified);

            for (name, sample_set) in &fixture.sample_sets {
                assert!(
                    name == "constantPlains" || name == "mixedOverworld",
                    "unexpected sample set {name}"
                );
                assert_eq!(sample_set.biome_pattern.grid_order, "x-major,z-minor");
                assert_eq!(
                    sample_set.biome_pattern.sample_count,
                    sample_set.biome_pattern.x.len() * sample_set.biome_pattern.z.len()
                );
                assert_eq!(
                    sample_set.biome_pattern.keys.len(),
                    sample_set.biome_pattern.sample_count
                );
                assert_eq!(
                    sample_set.biome_pattern.depths.len(),
                    sample_set.biome_pattern.sample_count
                );
                assert_eq!(
                    sample_set.biome_pattern.scales.len(),
                    sample_set.biome_pattern.sample_count
                );
                assert_eq!(
                    sample_set.columns.noise_method,
                    "fillNoiseColumn(noiseValues,cellX,cellZ,noiseSettings,biomeY,minCellY,cellCountY)"
                );
                assert_eq!(sample_set.columns.grid_order, "x-major,z-minor,y-minor");
                assert_eq!(
                    sample_set.columns.column_value_count,
                    fixture.column_value_count
                );
                assert_eq!(
                    sample_set.columns.sample_count,
                    sample_set.columns.x.len()
                        * sample_set.columns.z.len()
                        * sample_set.columns.column_value_count
                );
                assert_eq!(
                    sample_set.columns.values.len(),
                    sample_set.columns.sample_count
                );
            }
        }
    }

    #[test]
    fn default_overworld_settings_keep_dormant_caves_and_cliffs_flags_disabled() {
        let settings = NoiseGeneratorSettings::overworld();
        assert_eq!(settings.default_block(), STONE);
        assert_eq!(settings.default_fluid(), WATER);
        assert_eq!(settings.bedrock_floor_position(), 0);
        assert_eq!(settings.sea_level(), 63);
        assert!(!settings.is_aquifers_enabled());
        assert!(!settings.is_noise_caves_enabled());
        assert!(!settings.is_deepslate_enabled());
        assert!(!settings.is_ore_veins_enabled());
        assert!(!settings.is_noodle_caves_enabled());
    }

    #[test]
    fn terrain_only_oracle_fixture_stays_pinned_to_generator_target() {
        let oracle = terrain_fixture();
        assert_eq!(oracle.module, "terrain-chunk");
        assert_eq!(oracle.minecraft_version, "1.17.1");
        assert_eq!(
            oracle.generator_class,
            "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
        );
        assert_eq!(oracle.seed, "12345");
        assert_eq!(oracle.chunk_x, 0);
        assert_eq!(oracle.chunk_z, 0);
        assert_eq!(oracle.min_y, 0);
        assert_eq!(oracle.height, 256);
        assert_eq!(oracle.block_order, "y-major,z-major,x-minor");
        assert_eq!(
            oracle.palette,
            [
                "minecraft:air",
                "minecraft:stone",
                "minecraft:water",
                "minecraft:bedrock",
            ]
        );
        assert_eq!(
            oracle.blocks.len(),
            CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 256
        );
    }

    #[test]
    fn fills_chunk_zero_zero_with_terrain_only_java_oracle() {
        let oracle = terrain_fixture();
        let generator = NoiseBasedChunkGenerator::new(
            OverworldBiomeSource::new(
                oracle.seed.parse::<i64>().expect("i64 fixture seed"),
                false,
                false,
            ),
            oracle.seed.parse::<i64>().expect("i64 fixture seed"),
            NoiseGeneratorSettings::overworld(),
        );
        let chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
        let expected = terrain_stage_blocks_from_oracle(&oracle);

        assert_eq!(chunk.chunk_x, oracle.chunk_x);
        assert_eq!(chunk.chunk_z, oracle.chunk_z);
        assert_eq!(chunk.min_y, oracle.min_y);
        assert_eq!(chunk.height, oracle.height);
        assert!(!chunk.blocks.contains(&BEDROCK));
        assert_eq!(chunk.blocks.len(), expected.len());

        for (index, (actual, expected)) in chunk.blocks.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual,
                expected,
                "terrain mismatch at local ({}, {}, {}): expected block id {}, got {}",
                index & 15,
                index >> 8,
                (index >> 4) & 15,
                expected,
                actual
            );
        }
    }

    #[test]
    fn build_surface_and_bedrock_matches_java_oracle() {
        let oracle = surface_fixture();
        assert_eq!(oracle.chunk_x, 0);
        assert_eq!(oracle.chunk_z, 0);
        assert_surface_chunk_matches_java_oracle(oracle);
    }

    #[test]
    fn build_frozen_ocean_surface_and_bedrock_matches_java_oracle() {
        let oracle = frozen_ocean_surface_fixture();
        assert_eq!(oracle.chunk_x, -247);
        assert_eq!(oracle.chunk_z, -247);
        assert_surface_chunk_matches_java_oracle(oracle);
    }

    #[test]
    fn build_badlands_surface_and_bedrock_matches_java_oracle() {
        let oracle = badlands_surface_fixture();
        assert_eq!(oracle.chunk_x, -320);
        assert_eq!(oracle.chunk_z, 99);
        assert_surface_chunk_matches_java_oracle(oracle);
    }

    #[test]
    fn generated_chunk_wraps_surface_buffer_for_render_consumers() {
        let chunk = generate_overworld_surface_chunk(12345, 0, 0);

        assert_eq!(chunk.chunk_x, 0);
        assert_eq!(chunk.chunk_z, 0);
        assert_eq!(chunk.min_y, 0);
        assert_eq!(chunk.height, 256);
        assert_eq!(
            chunk.blocks().len(),
            CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 256
        );
        assert!(chunk.non_air_block_count() > 0);
        assert_eq!(chunk.block_at_local(0, 0, 0).name(), "minecraft:bedrock");
    }

    #[test]
    fn palette_matrix_rows_have_expected_biome_surface_and_supported_feature_family() {
        for case in PALETTE_MATRIX_CASES {
            let biome_source = OverworldBiomeSource::new(case.seed, false, false);
            let primary = biome_source.get_primary_biome_definition(case.chunk_x, case.chunk_z);
            assert_eq!(
                primary.key(),
                case.biome_key,
                "primary biome for seed {} chunk ({}, {})",
                case.seed,
                case.chunk_x,
                case.chunk_z
            );

            let block_position_biomes =
                count_block_position_biomes_in_chunk(&biome_source, case.seed, case);
            assert!(
                block_position_biomes > 0,
                "seed {} chunk ({}, {}) had no block-position {} samples",
                case.seed,
                case.chunk_x,
                case.chunk_z,
                case.biome_key
            );

            let chunk = generate_overworld_features_chunk(case.seed, case.chunk_x, case.chunk_z);
            let family_columns = count_top_surface_family(&chunk, case.surface_family);
            assert!(
                family_columns > 0,
                "seed {} chunk ({}, {}) had no top-surface {} columns; top surface histogram: {:?}",
                case.seed,
                case.chunk_x,
                case.chunk_z,
                case.surface_family.name(),
                top_surface_histogram(&chunk)
            );

            if let Some(feature_family) = case.feature_family {
                let feature_blocks = count_feature_family(&chunk, feature_family);
                assert!(
                    feature_blocks > 0,
                    "seed {} chunk ({}, {}) had no {} blocks",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    feature_family.name()
                );
            }
        }
    }

    #[test]
    fn generated_features_chunk_adds_visible_decoration_blocks() {
        let features = generate_overworld_features_chunk(12345, 0, 0);
        let feature_block_count = features.block_count(crate::block::OAK_LOG)
            + features.block_count(crate::block::OAK_LEAVES)
            + features.block_count(crate::block::BIRCH_LOG)
            + features.block_count(crate::block::BIRCH_LEAVES)
            + features.block_count(crate::block::SPRUCE_LOG)
            + features.block_count(crate::block::SPRUCE_LEAVES)
            + features.block_count(crate::block::GRASS)
            + features.block_count(crate::block::FERN)
            + features.block_count(crate::block::DANDELION)
            + features.block_count(crate::block::POPPY)
            + features.block_count(crate::block::DEAD_BUSH);

        assert!(feature_block_count > 0);
        assert!(
            features.block_count(crate::block::GRANITE)
                + features.block_count(crate::block::DIORITE)
                + features.block_count(crate::block::ANDESITE)
                + features.block_count(crate::block::TUFF)
                + features.block_count(crate::block::DEEPSLATE)
                > 0
        );
        assert!(
            features.block_count(crate::block::COAL_ORE)
                + features.block_count(crate::block::DEEPSLATE_COAL_ORE)
                + features.block_count(crate::block::IRON_ORE)
                + features.block_count(crate::block::DEEPSLATE_IRON_ORE)
                + features.block_count(crate::block::COPPER_ORE)
                + features.block_count(crate::block::DEEPSLATE_COPPER_ORE)
                > 0
        );
        assert!(features.block_count(crate::block::LAVA) > 0);
    }

    #[test]
    fn feature_batch_plan_reuses_overlapping_dependency_windows() {
        let single = FeatureBatchPlan::new([ChunkPos::new(0, 0)]);
        assert_eq!(single.targets.len(), 1);
        assert_eq!(single.feature_centers.len(), 3 * 3);
        assert_eq!(single.dependency_chunks.len(), 19 * 19);

        let radius_one_targets = (-1..=1).flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)));
        let radius_one = FeatureBatchPlan::new(radius_one_targets);

        assert_eq!(radius_one.targets.len(), 3 * 3);
        assert_eq!(radius_one.feature_centers.len(), 5 * 5);
        assert_eq!(radius_one.dependency_chunks.len(), 21 * 21);
        assert!(
            radius_one
                .dependency_chunks
                .contains(&ChunkPos::new(-10, -10))
        );
        assert!(
            radius_one
                .dependency_chunks
                .contains(&ChunkPos::new(10, 10))
        );
    }

    #[test]
    fn feature_center_order_matches_vanilla_scheduler_trace_for_spawn_bootstrap() {
        let trace = scheduler_trace_fixture();
        assert_eq!(trace.module, "scheduler-trace");
        assert_eq!(trace.minecraft_version, "1.17.1");
        assert_eq!(trace.seed, "12345");
        assert_eq!(trace.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
        assert_eq!(trace.stop_status, "FEATURES");

        let plan =
            FeatureBatchPlan::new([ChunkPos::new(trace.target_chunk_x, trace.target_chunk_z)]);
        let expected = trace
            .feature_completion_order_3x3
            .into_iter()
            .map(|entry| ChunkPos::new(entry.chunk_x, entry.chunk_z))
            .collect::<Vec<_>>();

        assert_eq!(plan.ordered_feature_centers(), expected);
    }

    #[test]
    fn taiga_vegetation_feature_index_shift_is_isolated_by_center() {
        let diagnostics = taiga_tree_index_shift_diagnostics(12_345, ChunkPos::new(0, 0));

        assert_eq!(
            diagnostics,
            vec![
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(-1, -1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(0, -1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 9,
                    java_tree_blocks: 9,
                    shared_tree_blocks: 9,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(1, -1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(-1, 0),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(0, 0),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 236,
                    java_tree_blocks: 236,
                    shared_tree_blocks: 236,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(1, 0),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 12,
                    java_tree_blocks: 12,
                    shared_tree_blocks: 12,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(0, 1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 14,
                    java_tree_blocks: 14,
                    shared_tree_blocks: 14,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(1, 1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
            ]
        );
    }

    #[test]
    fn taiga_full_table_tree_deltas_match_vanilla_scheduler_probe() {
        let diagnostics = taiga_full_table_tree_delta_diagnostics(12_345, ChunkPos::new(0, 0));
        let tree_deltas = diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.center,
                    diagnostic.biome_key,
                    diagnostic.before_tree_blocks,
                    diagnostic.after_tree_blocks,
                    diagnostic.added_tree_blocks,
                    diagnostic.removed_tree_blocks,
                    diagnostic.feature_random_calls,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            tree_deltas,
            vec![
                (
                    ChunkPos::new(-1, -1),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    75
                ),
                (
                    ChunkPos::new(0, -1),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    68
                ),
                (
                    ChunkPos::new(1, -1),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    75
                ),
                (
                    ChunkPos::new(-1, 0),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    77
                ),
                (
                    ChunkPos::new(0, 0),
                    "minecraft:taiga_mountains",
                    0,
                    236,
                    236,
                    0,
                    77
                ),
                (
                    ChunkPos::new(1, 0),
                    "minecraft:taiga_mountains",
                    236,
                    246,
                    10,
                    0,
                    86
                ),
                (
                    ChunkPos::new(0, 1),
                    "minecraft:taiga_mountains",
                    246,
                    246,
                    0,
                    0,
                    73
                ),
                (
                    ChunkPos::new(1, 1),
                    "minecraft:taiga_mountains",
                    246,
                    246,
                    0,
                    0,
                    73
                ),
            ]
        );
    }

    #[test]
    fn overworld_feature_dependency_cache_reuses_overlapping_windows() {
        let mut cache = OverworldFeatureDependencyCache::new();

        let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        assert_eq!(
            first.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 0,
                generated_dependency_chunks: 19 * 19,
                retained_dependency_chunks: 19 * 19,
            }
        );

        let second = cache.generate_features_chunks(12_345, [ChunkPos::new(1, 0)]);
        assert_eq!(
            second.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 18 * 19,
                generated_dependency_chunks: 19,
                retained_dependency_chunks: 19 * 19,
            }
        );
        assert_eq!(cache.retained_chunk_count(), 19 * 19);
        assert_eq!(
            second.chunks.get(&ChunkPos::new(1, 0)),
            Some(&generate_overworld_features_chunk(12_345, 1, 0))
        );
    }

    #[test]
    fn overworld_feature_dependency_cache_keeps_clean_lower_status_chunks() {
        let mut cache = OverworldFeatureDependencyCache::new();

        let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        let second = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);

        assert_eq!(
            second.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 19 * 19,
                generated_dependency_chunks: 0,
                retained_dependency_chunks: 19 * 19,
            }
        );
        assert_eq!(
            second.chunks.get(&ChunkPos::new(0, 0)),
            first.chunks.get(&ChunkPos::new(0, 0))
        );
    }

    #[test]
    fn overworld_feature_dependency_cache_resets_when_seed_changes() {
        let mut cache = OverworldFeatureDependencyCache::new();

        cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        let changed_seed = cache.generate_features_chunks(54_321, [ChunkPos::new(0, 0)]);

        assert_eq!(
            changed_seed.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 0,
                generated_dependency_chunks: 19 * 19,
                retained_dependency_chunks: 19 * 19,
            }
        );
    }

    #[test]
    fn full_decorated_chunk_gauntlet_reports_current_native_gap() {
        let fixture = full_chunk_fixture();
        assert_eq!(fixture.module, "integration");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "12345");
        assert_eq!(fixture.generator, "default");
        assert!(!fixture.generate_structures);
        assert_eq!(fixture.wire_format.block_order, "y-major,z-major,x-minor");
        assert_eq!(fixture.wire_format.palette_entries, "resource-key");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, 0);
        assert_eq!(expected.chunk_z, 0);
        assert_eq!(expected.status, "full");
        let actual = generate_overworld_features_chunk(12_345, expected.chunk_x, expected.chunk_z);
        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert_eq!(report.total_blocks, 16 * 16 * 256);
        assert!(report.matched_blocks < report.total_blocks);
        assert_eq!(report.mismatched_blocks, 5, "{report:#?}");
        assert_eq!(
            report.top_mismatch_pairs,
            vec![
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:water".to_owned(),
                    count: 2,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:glow_lichen".to_owned(),
                    count: 1,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:lava".to_owned(),
                    count: 1,
                },
                MismatchBucket {
                    actual: "minecraft:glow_lichen".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn features_status_chunk_snapshot_excludes_runtime_liquid_tick_results() {
        let fixture = scheduler_features_snapshot_fixture();
        assert_eq!(fixture.module, "scheduler-trace");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "12345");
        assert_eq!(fixture.target_chunk_x, 0);
        assert_eq!(fixture.target_chunk_z, 0);
        assert_eq!(fixture.target_radius, 1);
        assert_eq!(fixture.stop_status, "FEATURES");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, 0);
        assert_eq!(expected.chunk_z, 0);
        assert_eq!(expected.status, "features");

        let actual = generate_overworld_features_chunk(
            fixture.seed.parse::<i64>().expect("fixture seed is i64"),
            expected.chunk_x,
            expected.chunk_z,
        );
        let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);
        for (local_x, y, local_z, expected_name) in [
            (9, 12, 15, "minecraft:air"),
            (10, 17, 2, "minecraft:air"),
            (7, 17, 10, "minecraft:glow_lichen"),
            (9, 18, 2, "minecraft:water"),
            (10, 18, 2, "minecraft:air"),
        ] {
            let index = ((y - actual.min_y) << 8) | (local_z << 4) | local_x;
            assert_eq!(expected_blocks[index as usize], expected_name);
            assert_eq!(actual.block_at_y(local_x, y, local_z).name(), expected_name);
        }

        let report = compare_generated_chunk_to_full_fixture(&actual, expected);
        assert!(report.is_exact(), "{report:#?}");
    }

    #[test]
    fn plains_features_snapshot_reports_current_native_gap() {
        let fixture = plains_scheduler_features_snapshot_fixture();
        assert_eq!(fixture.module, "scheduler-trace");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "16");
        assert_eq!(fixture.target_chunk_x, 0);
        assert_eq!(fixture.target_chunk_z, 0);
        assert_eq!(fixture.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
        assert_eq!(fixture.stop_status, "FEATURES");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, 0);
        assert_eq!(expected.chunk_z, 0);
        assert_eq!(expected.status, "features");

        let actual = generate_overworld_features_chunk(16, expected.chunk_x, expected.chunk_z);
        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert_eq!(report.total_blocks, 16 * 16 * 256);
        assert_eq!(report.mismatched_blocks, 69, "{report:#?}");
        assert_eq!(
            report.top_mismatch_pairs,
            vec![
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:grass".to_owned(),
                    count: 29,
                },
                MismatchBucket {
                    actual: "minecraft:grass".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 16,
                },
                MismatchBucket {
                    actual: "minecraft:poppy".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 5,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:poppy".to_owned(),
                    count: 4,
                },
                MismatchBucket {
                    actual: "minecraft:dandelion".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 4,
                },
                MismatchBucket {
                    actual: "minecraft:poppy".to_owned(),
                    expected: "minecraft:grass".to_owned(),
                    count: 4,
                },
                MismatchBucket {
                    actual: "minecraft:water".to_owned(),
                    expected: "minecraft:glow_lichen".to_owned(),
                    count: 4,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:tall_grass".to_owned(),
                    count: 2,
                },
                MismatchBucket {
                    actual: "minecraft:dandelion".to_owned(),
                    expected: "minecraft:grass".to_owned(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn inland_plains_features_snapshot_reports_current_native_gap() {
        let fixture = inland_plains_scheduler_features_snapshot_fixture();
        assert_eq!(fixture.module, "scheduler-trace");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "17");
        assert_eq!(fixture.target_chunk_x, 0);
        assert_eq!(fixture.target_chunk_z, 0);
        assert_eq!(fixture.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
        assert_eq!(fixture.stop_status, "FEATURES");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, 0);
        assert_eq!(expected.chunk_z, 0);
        assert_eq!(expected.status, "features");

        let actual = generate_overworld_features_chunk(17, expected.chunk_x, expected.chunk_z);
        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert_eq!(report.total_blocks, 16 * 16 * 256);
        assert_eq!(report.mismatched_blocks, 110, "{report:#?}");
        assert_eq!(
            report.top_mismatch_pairs,
            vec![
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:grass".to_owned(),
                    count: 38,
                },
                MismatchBucket {
                    actual: "minecraft:grass".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 26,
                },
                MismatchBucket {
                    actual: "minecraft:dandelion".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 14,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:dandelion".to_owned(),
                    count: 13,
                },
                MismatchBucket {
                    actual: "minecraft:dandelion".to_owned(),
                    expected: "minecraft:grass".to_owned(),
                    count: 7,
                },
                MismatchBucket {
                    actual: "minecraft:poppy".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 4,
                },
                MismatchBucket {
                    actual: "minecraft:poppy".to_owned(),
                    expected: "minecraft:dandelion".to_owned(),
                    count: 3,
                },
                MismatchBucket {
                    actual: "minecraft:grass".to_owned(),
                    expected: "minecraft:dandelion".to_owned(),
                    count: 2,
                },
                MismatchBucket {
                    actual: "minecraft:deepslate_redstone_ore".to_owned(),
                    expected: "minecraft:redstone_ore".to_owned(),
                    count: 1,
                },
                MismatchBucket {
                    actual: "minecraft:granite".to_owned(),
                    expected: "minecraft:water".to_owned(),
                    count: 1,
                },
                MismatchBucket {
                    actual: "minecraft:poppy".to_owned(),
                    expected: "minecraft:grass".to_owned(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn forest_seed_23823_features_snapshot_reports_current_native_gap() {
        let fixture = forest_seed_23823_scheduler_features_snapshot_fixture();
        assert_eq!(fixture.module, "scheduler-trace");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "23823");
        assert_eq!(fixture.target_chunk_x, -13);
        assert_eq!(fixture.target_chunk_z, 8);
        assert_eq!(fixture.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
        assert_eq!(fixture.stop_status, "FEATURES");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, -13);
        assert_eq!(expected.chunk_z, 8);
        assert_eq!(expected.status, "features");

        let actual = generate_overworld_features_chunk(23823, expected.chunk_x, expected.chunk_z);
        let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);

        let canopy_index = ((67 - actual.min_y) << 8) | (8 << 4) | 8;
        assert_eq!(
            expected_blocks[canopy_index as usize],
            "minecraft:oak_leaves"
        );
        assert_eq!(actual.block_at_y(8, 67, 8).name(), "minecraft:oak_leaves");

        let fancy_oak_index = ((70 - actual.min_y) << 8) | (8 << 4) | 8;
        assert_eq!(
            expected_blocks[fancy_oak_index as usize],
            "minecraft:oak_leaves"
        );
        assert_eq!(actual.block_at_y(8, 70, 8).name(), "minecraft:oak_leaves");

        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert_eq!(report.total_blocks, 16 * 16 * 256);
        assert_eq!(report.mismatched_blocks, 3, "{report:#?}");
        assert_eq!(
            report.top_mismatch_pairs,
            vec![MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 3,
            }]
        );
    }

    #[test]
    #[ignore = "active gauntlet: native full decorated chunk parity is not expected to pass yet"]
    fn full_decorated_chunk_zero_zero_matches_java_oracle() {
        let fixture = full_chunk_fixture();
        let expected = &fixture.chunks[0];
        let actual = generate_overworld_features_chunk(
            fixture.seed.parse::<i64>().expect("fixture seed is i64"),
            expected.chunk_x,
            expected.chunk_z,
        );
        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert!(report.is_exact(), "{report:#?}");
    }

    #[test]
    fn matches_java_oracle_across_sampled_cell_columns() {
        for fixture in fixtures() {
            let settings = create_noise_settings(&fixture);

            for (sample_set_name, sample_set) in &fixture.sample_sets {
                let sampler = create_noise_sampler(&fixture, &sample_set.biome_pattern);
                let mut column = vec![0.0; fixture.column_value_count];
                let mut index = 0;

                for cell_x in &sample_set.columns.x {
                    for cell_z in &sample_set.columns.z {
                        sampler.fill_noise_column(
                            &mut column,
                            *cell_x,
                            *cell_z,
                            &settings,
                            fixture.biome_y,
                            fixture.min_cell_y,
                            fixture.cell_count_y,
                        );

                        for (y_index, actual) in column.iter().enumerate() {
                            let expected = sample_set.columns.values[index];
                            assert_eq!(
                                actual.to_bits(),
                                expected.to_bits(),
                                "noise-sampler(seed={}, sampleSet={sample_set_name}) mismatch at flattened index {index} for cell ({cell_x}, {cell_z}) and column index {y_index}: expected {expected}, got {actual}",
                                fixture.seed
                            );
                            index += 1;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn noise_settings_create_enforces_java_min_y_height_guard() {
        let sampling = NoiseSamplingSettings::new(1.0, 1.0, 80.0, 160.0);
        let top_slide = NoiseSlideSettings::new(-10, 3, 0);
        let bottom_slide = NoiseSlideSettings::new(15, 3, 0);

        assert_panic_message(
            || {
                NoiseSettings::create(
                    0,
                    255,
                    sampling,
                    top_slide,
                    bottom_slide,
                    1,
                    2,
                    1.0,
                    -0.46875,
                    true,
                    true,
                    false,
                    false,
                );
            },
            "height has to be a multiple of 16",
        );
        assert_panic_message(
            || {
                NoiseSettings::create(
                    1,
                    256,
                    sampling,
                    top_slide,
                    bottom_slide,
                    1,
                    2,
                    1.0,
                    -0.46875,
                    true,
                    true,
                    false,
                    false,
                );
            },
            "min_y has to be a multiple of 16",
        );
        assert_panic_message(
            || {
                NoiseSettings::create(
                    0,
                    2048 + 16,
                    sampling,
                    top_slide,
                    bottom_slide,
                    1,
                    2,
                    1.0,
                    -0.46875,
                    true,
                    true,
                    false,
                    false,
                );
            },
            "min_y + height cannot be higher than: 2032",
        );
    }

    #[test]
    fn island_noise_override_is_rejected_in_this_overworld_slice() {
        let fixture = fixtures()
            .into_iter()
            .find(|fixture| fixture.seed == "12345")
            .expect("seed 12345 fixture");
        let sample_set = fixture
            .sample_sets
            .get("constantPlains")
            .expect("constantPlains sample set");
        let settings = create_noise_settings(&fixture);
        let mut random = WorldgenRandom::new(12_345);
        let blended_noise = BlendedNoise::new(&mut random);
        random.consume_count(2620);
        let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);
        let island_noise = SimplexNoise::new(&mut WorldgenRandom::new(12_345));
        let sampler = NoiseSampler::new(
            RepeatingPatternBiomeSource::new(&sample_set.biome_pattern),
            fixture.cell_width,
            fixture.cell_height,
            fixture.cell_count_y,
            settings.clone(),
            blended_noise,
            Some(island_noise),
            depth_noise,
            NoiseModifier::Passthrough,
        );

        assert_panic_message(
            || {
                let mut column = vec![0.0; fixture.column_value_count];
                sampler.fill_noise_column(
                    &mut column,
                    0,
                    0,
                    &settings,
                    fixture.biome_y,
                    fixture.min_cell_y,
                    fixture.cell_count_y,
                );
            },
            "NoiseSampler island noise override is out of scope for the 1.17.1 overworld target",
        );
    }

    #[test]
    fn generated_chunk_preserves_scheduled_ticks_from_mutable_buffer() {
        let mut buffer = MutableChunkBlockBuffer::new(2, -3, 0, 32);
        buffer.schedule_block_tick(33, 8, -47, "minecraft:stone", 2);
        buffer.schedule_liquid_tick(34, 9, -46, "minecraft:water", 0);

        let chunk = GeneratedChunk::from_mutable_buffer(buffer);

        assert_eq!(
            chunk.block_ticks(),
            &[ScheduledTick::new(33, 8, -47, "minecraft:stone", 2)]
        );
        assert_eq!(
            chunk.liquid_ticks(),
            &[ScheduledTick::new(34, 9, -46, "minecraft:water", 0)]
        );
    }

    #[test]
    fn generated_chunk_converts_to_packed_snapshot() {
        let mut blocks = vec![AIR; 32 * 16 * 16];
        blocks[16 * 16 * 16] = STONE;
        let biomes = vec![6; mclone_core::expected_chunk_biome_count(32)];
        let chunk = GeneratedChunk::from_raw_parts_with_ticks_and_biomes(
            2,
            -3,
            0,
            32,
            blocks,
            biomes.clone(),
            Vec::new(),
            Vec::new(),
        );

        let snapshot = chunk.to_chunk_snapshot(
            mclone_core::ChunkRevision(9),
            mclone_core::ChunkStatus::Surface,
        );

        assert_eq!(snapshot.pos, mclone_core::ChunkPos::new(2, -3));
        assert_eq!(snapshot.revision, mclone_core::ChunkRevision(9));
        assert_eq!(snapshot.status, mclone_core::ChunkStatus::Surface);
        assert_eq!(snapshot.biomes, biomes);
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(snapshot.sections[0].section_y, 1);
        assert_eq!(
            snapshot.sections[0].unpack_block_state_ids()[0],
            mclone_core::BlockStateId(STONE as u32)
        );
    }

    fn count_top_surface_family(chunk: &GeneratedChunk, family: SurfaceFamily) -> usize {
        let mut count = 0;
        for local_z in 0..GeneratedChunk::WIDTH {
            for local_x in 0..GeneratedChunk::WIDTH {
                if top_non_air_block(chunk, local_x, local_z)
                    .is_some_and(|block| family.contains(block))
                {
                    count += 1;
                }
            }
        }
        count
    }

    fn top_surface_histogram(chunk: &GeneratedChunk) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for local_z in 0..GeneratedChunk::WIDTH {
            for local_x in 0..GeneratedChunk::WIDTH {
                if let Some(block) = top_non_air_block(chunk, local_x, local_z) {
                    *counts.entry(crate::block::block_name(block)).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    fn count_feature_family(chunk: &GeneratedChunk, family: FeatureFamily) -> usize {
        family
            .blocks()
            .iter()
            .map(|block| chunk.block_count(*block))
            .sum()
    }

    fn count_block_position_biomes_in_chunk(
        biome_source: &OverworldBiomeSource,
        seed: i64,
        case: &PaletteMatrixCase,
    ) -> usize {
        let min_x = chunk_min_block_coord(case.chunk_x);
        let min_z = chunk_min_block_coord(case.chunk_z);
        let mut count = 0;
        for local_z in 0..GeneratedChunk::WIDTH {
            for local_x in 0..GeneratedChunk::WIDTH {
                let world_x = min_x + local_x;
                let world_z = min_z + local_z;
                if biome_source
                    .get_block_position_biome_definition(seed, world_x, world_z)
                    .key()
                    == case.biome_key
                {
                    count += 1;
                }
            }
        }
        count
    }

    fn top_non_air_block(chunk: &GeneratedChunk, local_x: i32, local_z: i32) -> Option<RawBlockId> {
        for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
            let block = chunk.block_at_y(local_x, y, local_z).raw();
            if !is_air_like(block) {
                return Some(block);
            }
        }
        None
    }

    fn assert_panic_message(work: impl FnOnce() + panic::UnwindSafe, expected: &str) {
        let panic = panic::catch_unwind(work).expect_err("expected panic");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .expect("panic message");
        assert_eq!(message, expected);
    }
}
