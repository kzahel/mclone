use super::*;
use crate::biome::OverworldBiomeSource;
use crate::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, ALLIUM, AZURE_BLUET, BAMBOO, BAMBOO_FINAL_LARGE,
    BAMBOO_TOP_LARGE, BAMBOO_TOP_SMALL, BIRCH_LEAVES, BIRCH_LOG, BLUE_ICE, BLUE_ORCHID,
    BRAIN_CORAL, BRAIN_CORAL_BLOCK, BRAIN_CORAL_FAN, BRAIN_CORAL_WALL_FAN_EAST,
    BRAIN_CORAL_WALL_FAN_NORTH, BRAIN_CORAL_WALL_FAN_SOUTH, BRAIN_CORAL_WALL_FAN_WEST,
    BROWN_MUSHROOM, BROWN_MUSHROOM_BLOCK, BUBBLE_CORAL, BUBBLE_CORAL_BLOCK, BUBBLE_CORAL_FAN,
    BUBBLE_CORAL_WALL_FAN_EAST, BUBBLE_CORAL_WALL_FAN_NORTH, BUBBLE_CORAL_WALL_FAN_SOUTH,
    BUBBLE_CORAL_WALL_FAN_WEST, CACTUS, CLAY, COARSE_DIRT, COCOA_AGE0_EAST, COCOA_AGE0_NORTH,
    COCOA_AGE0_SOUTH, COCOA_AGE0_WEST, COCOA_AGE1_EAST, COCOA_AGE1_NORTH, COCOA_AGE1_SOUTH,
    COCOA_AGE1_WEST, COCOA_AGE2_EAST, COCOA_AGE2_NORTH, COCOA_AGE2_SOUTH, COCOA_AGE2_WEST,
    CORNFLOWER, DANDELION, DARK_OAK_LEAVES, DARK_OAK_LOG, DEAD_BUSH, FERN, FIRE_CORAL,
    FIRE_CORAL_BLOCK, FIRE_CORAL_FAN, FIRE_CORAL_WALL_FAN_EAST, FIRE_CORAL_WALL_FAN_NORTH,
    FIRE_CORAL_WALL_FAN_SOUTH, FIRE_CORAL_WALL_FAN_WEST, GRASS, GRASS_BLOCK, GRAVEL, HORN_CORAL,
    HORN_CORAL_BLOCK, HORN_CORAL_FAN, HORN_CORAL_WALL_FAN_EAST, HORN_CORAL_WALL_FAN_NORTH,
    HORN_CORAL_WALL_FAN_SOUTH, HORN_CORAL_WALL_FAN_WEST, ICE, JUNGLE_LEAVES, JUNGLE_LOG, KELP,
    KELP_PLANT, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LAVA, LILAC_LOWER, LILAC_UPPER,
    LILY_OF_THE_VALLEY, LILY_PAD, MOSSY_COBBLESTONE, MUSHROOM_STEM, MYCELIUM, OAK_LEAVES, OAK_LOG,
    ORANGE_TULIP, OXEYE_DAISY, PACKED_ICE, PEONY_LOWER, PEONY_UPPER, PINK_TULIP, PODZOL, POPPY,
    PUMPKIN, RED_MUSHROOM, RED_MUSHROOM_BLOCK, RED_SAND, RED_TULIP, ROSE_BUSH_LOWER,
    ROSE_BUSH_UPPER, RawBlockId, SAND, SEA_PICKLE_1, SEA_PICKLE_2, SEA_PICKLE_3, SEA_PICKLE_4,
    SEAGRASS, SNOW, SNOW_BLOCK, SPRUCE_LEAVES, SPRUCE_LOG, STONE, SUGAR_CANE, SUNFLOWER_LOWER,
    SUNFLOWER_UPPER, SWEET_BERRY_BUSH, TALL_SEAGRASS_LOWER, TALL_SEAGRASS_UPPER, TERRACOTTA,
    TUBE_CORAL, TUBE_CORAL_BLOCK, TUBE_CORAL_FAN, TUBE_CORAL_WALL_FAN_EAST,
    TUBE_CORAL_WALL_FAN_NORTH, TUBE_CORAL_WALL_FAN_SOUTH, TUBE_CORAL_WALL_FAN_WEST, VINE_EAST,
    VINE_NORTH, VINE_SOUTH, VINE_UP, VINE_WEST, WATER, WHITE_TULIP, is_air_like, is_water,
};
use crate::feature::{FEATURES_WRITE_RADIUS_CUTOFF, FeatureRegion, FeatureWorld};
use crate::levelgen::ChunkGenerationPlan;
use crate::levelgen::feature_batch::{
    generate_overworld_liquid_carved_buffer_with_biome_source, sorted_chunk_positions_z_major,
};
use crate::noise::{BlendedNoise, PerlinNoise, SimplexNoise};
use crate::prng::WorldgenRandom;
use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_block_index, chunk_min_block_coord};
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
struct LowVisibilityFeatureCase {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    biome_key: &'static str,
    expectation: LowVisibilityFeatureExpectation,
}

#[derive(Clone, Copy, Debug)]
enum LowVisibilityFeatureExpectation {
    VisibleSmallMushrooms {
        min_count: usize,
    },
    JungleCocoaVines {
        min_cocoa: usize,
        min_vines: usize,
    },
    JungleBushShape {
        min_matches: usize,
    },
    VisiblePumpkins {
        min_count: usize,
    },
    ForestRockBoulders {
        min_count: usize,
    },
    DefaultSpringVisibleFluids {
        min_water_blocks: usize,
        min_lava_blocks: usize,
    },
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
    SunflowerPlainsSunflowers,
    DesertDeadBushCactusSugarCane,
    ForestOakBirchTrees,
    FlowerForestFlowers,
    SwampNativeSubsetSugarCaneLilyPad,
    TaigaSpruceFernBerry,
    GiantTaigaSpruceTrees,
    SnowySpruceFern,
    MountainTrees,
    BadlandsDeadBushCactusSugarCane,
    WoodedBadlandsTrees,
    IceSpikesPackedIce,
    FrozenOceanIcebergs,
    DefaultExtraSugarCane,
    RiverSeagrass,
    OceanWaterPlants,
    WarmOceanCoralSeaPickles,
    DarkForestCanopyMushrooms,
    MushroomFieldHugeMushrooms,
    BirchTrees,
    SavannaAcacia,
    JungleTrees,
    SparseJungleTrees,
    BambooJungle,
}

impl FeatureFamily {
    fn name(self) -> &'static str {
        match self {
            Self::PlainsVegetation => "plains grass/flower/oak",
            Self::SunflowerPlainsSunflowers => "sunflower plains sunflower patches",
            Self::DesertDeadBushCactusSugarCane => "desert dead bush plus cactus/sugar cane",
            Self::ForestOakBirchTrees => "forest oak/birch trees",
            Self::FlowerForestFlowers => "flower forest dense small flowers",
            Self::SwampNativeSubsetSugarCaneLilyPad => {
                "native swamp vegetation/clay subset plus sugar cane/lily pad"
            }
            Self::TaigaSpruceFernBerry => "taiga spruce/fern plus berry bushes",
            Self::GiantTaigaSpruceTrees => "giant taiga spruce log/leaves plus podzol",
            Self::SnowySpruceFern => "snowy spruce/fern",
            Self::MountainTrees => "mountain oak/spruce trees",
            Self::BadlandsDeadBushCactusSugarCane => "badlands dead bush plus cactus/sugar cane",
            Self::WoodedBadlandsTrees => "wooded badlands oak plus dead bush/cactus/sugar cane",
            Self::IceSpikesPackedIce => "ice spikes packed ice",
            Self::FrozenOceanIcebergs => "frozen ocean packed/blue icebergs",
            Self::DefaultExtraSugarCane => "default extra sugar cane",
            Self::RiverSeagrass => "river seagrass water plants",
            Self::OceanWaterPlants => "ocean seagrass/kelp water plants",
            Self::WarmOceanCoralSeaPickles => {
                "warm ocean coral blocks, live sidecars, and sea pickles"
            }
            Self::DarkForestCanopyMushrooms => "dark forest dark oak plus huge mushrooms",
            Self::MushroomFieldHugeMushrooms => "mushroom field huge mushrooms",
            Self::BirchTrees => "birch log/leaves trees",
            Self::SavannaAcacia => "savanna acacia trees",
            Self::JungleTrees => "jungle log/leaves trees",
            Self::SparseJungleTrees => "sparse jungle tree blocks",
            Self::BambooJungle => "bamboo plus jungle log/leaves vegetation",
        }
    }

    fn blocks(self) -> &'static [RawBlockId] {
        match self {
            Self::PlainsVegetation => &[OAK_LOG, OAK_LEAVES, GRASS, DANDELION, POPPY],
            Self::SunflowerPlainsSunflowers => &[SUNFLOWER_LOWER, SUNFLOWER_UPPER],
            Self::DesertDeadBushCactusSugarCane => &[DEAD_BUSH, CACTUS, SUGAR_CANE],
            Self::ForestOakBirchTrees => &[OAK_LOG, OAK_LEAVES, BIRCH_LOG, BIRCH_LEAVES],
            Self::FlowerForestFlowers => &[
                DANDELION,
                POPPY,
                ALLIUM,
                AZURE_BLUET,
                RED_TULIP,
                ORANGE_TULIP,
                WHITE_TULIP,
                PINK_TULIP,
                OXEYE_DAISY,
                CORNFLOWER,
                LILY_OF_THE_VALLEY,
                LILAC_LOWER,
                LILAC_UPPER,
                ROSE_BUSH_LOWER,
                ROSE_BUSH_UPPER,
                PEONY_LOWER,
                PEONY_UPPER,
            ],
            Self::SwampNativeSubsetSugarCaneLilyPad => &[
                OAK_LOG,
                OAK_LEAVES,
                GRASS,
                POPPY,
                DEAD_BUSH,
                BLUE_ORCHID,
                BROWN_MUSHROOM,
                RED_MUSHROOM,
                CLAY,
                SUGAR_CANE,
                LILY_PAD,
            ],
            Self::TaigaSpruceFernBerry => &[
                SPRUCE_LOG,
                SPRUCE_LEAVES,
                FERN,
                LARGE_FERN_LOWER,
                LARGE_FERN_UPPER,
                SWEET_BERRY_BUSH,
            ],
            Self::GiantTaigaSpruceTrees => &[SPRUCE_LOG, SPRUCE_LEAVES, PODZOL],
            Self::SnowySpruceFern => &[SPRUCE_LOG, SPRUCE_LEAVES, FERN],
            Self::MountainTrees => &[OAK_LOG, OAK_LEAVES, SPRUCE_LOG, SPRUCE_LEAVES],
            Self::BadlandsDeadBushCactusSugarCane => &[DEAD_BUSH, CACTUS, SUGAR_CANE],
            Self::WoodedBadlandsTrees => &[OAK_LOG, OAK_LEAVES, DEAD_BUSH, CACTUS, SUGAR_CANE],
            Self::IceSpikesPackedIce => &[PACKED_ICE],
            Self::FrozenOceanIcebergs => &[PACKED_ICE, BLUE_ICE],
            Self::DefaultExtraSugarCane => &[SUGAR_CANE],
            Self::RiverSeagrass => &[SEAGRASS, TALL_SEAGRASS_LOWER, TALL_SEAGRASS_UPPER],
            Self::OceanWaterPlants => &[
                SEAGRASS,
                TALL_SEAGRASS_LOWER,
                TALL_SEAGRASS_UPPER,
                KELP,
                KELP_PLANT,
            ],
            Self::WarmOceanCoralSeaPickles => &[
                TUBE_CORAL_BLOCK,
                BRAIN_CORAL_BLOCK,
                BUBBLE_CORAL_BLOCK,
                FIRE_CORAL_BLOCK,
                HORN_CORAL_BLOCK,
                TUBE_CORAL,
                BRAIN_CORAL,
                BUBBLE_CORAL,
                FIRE_CORAL,
                HORN_CORAL,
                TUBE_CORAL_FAN,
                BRAIN_CORAL_FAN,
                BUBBLE_CORAL_FAN,
                FIRE_CORAL_FAN,
                HORN_CORAL_FAN,
                TUBE_CORAL_WALL_FAN_NORTH,
                TUBE_CORAL_WALL_FAN_EAST,
                TUBE_CORAL_WALL_FAN_SOUTH,
                TUBE_CORAL_WALL_FAN_WEST,
                BRAIN_CORAL_WALL_FAN_NORTH,
                BRAIN_CORAL_WALL_FAN_EAST,
                BRAIN_CORAL_WALL_FAN_SOUTH,
                BRAIN_CORAL_WALL_FAN_WEST,
                BUBBLE_CORAL_WALL_FAN_NORTH,
                BUBBLE_CORAL_WALL_FAN_EAST,
                BUBBLE_CORAL_WALL_FAN_SOUTH,
                BUBBLE_CORAL_WALL_FAN_WEST,
                FIRE_CORAL_WALL_FAN_NORTH,
                FIRE_CORAL_WALL_FAN_EAST,
                FIRE_CORAL_WALL_FAN_SOUTH,
                FIRE_CORAL_WALL_FAN_WEST,
                HORN_CORAL_WALL_FAN_NORTH,
                HORN_CORAL_WALL_FAN_EAST,
                HORN_CORAL_WALL_FAN_SOUTH,
                HORN_CORAL_WALL_FAN_WEST,
                SEA_PICKLE_1,
                SEA_PICKLE_2,
                SEA_PICKLE_3,
                SEA_PICKLE_4,
            ],
            Self::DarkForestCanopyMushrooms => &[
                DARK_OAK_LOG,
                DARK_OAK_LEAVES,
                BROWN_MUSHROOM_BLOCK,
                RED_MUSHROOM_BLOCK,
                MUSHROOM_STEM,
            ],
            Self::MushroomFieldHugeMushrooms => {
                &[BROWN_MUSHROOM_BLOCK, RED_MUSHROOM_BLOCK, MUSHROOM_STEM]
            }
            Self::BirchTrees => &[BIRCH_LOG, BIRCH_LEAVES],
            Self::SavannaAcacia => &[ACACIA_LOG, ACACIA_LEAVES],
            Self::JungleTrees => &[JUNGLE_LOG, JUNGLE_LEAVES],
            Self::SparseJungleTrees => &[JUNGLE_LOG, JUNGLE_LEAVES],
            Self::BambooJungle => &[
                BAMBOO,
                BAMBOO_TOP_SMALL,
                BAMBOO_TOP_LARGE,
                BAMBOO_FINAL_LARGE,
                JUNGLE_LOG,
                JUNGLE_LEAVES,
            ],
        }
    }

    fn is_present(self, chunk: &GeneratedChunk) -> bool {
        match self {
            Self::DesertDeadBushCactusSugarCane | Self::BadlandsDeadBushCactusSugarCane => {
                chunk.block_count(DEAD_BUSH) > 0
                    && (chunk.block_count(CACTUS) + chunk.block_count(SUGAR_CANE)) > 0
            }
            Self::WoodedBadlandsTrees => {
                chunk.block_count(OAK_LOG) > 0
                    && chunk.block_count(OAK_LEAVES) > 0
                    && chunk.block_count(DEAD_BUSH) > 0
                    && (chunk.block_count(CACTUS) + chunk.block_count(SUGAR_CANE)) > 0
            }
            Self::SwampNativeSubsetSugarCaneLilyPad => {
                [OAK_LOG, OAK_LEAVES, GRASS, POPPY, DEAD_BUSH, CLAY]
                    .iter()
                    .any(|block| chunk.block_count(*block) > 0)
                    && chunk.block_count(BLUE_ORCHID) > 0
                    && (chunk.block_count(BROWN_MUSHROOM) + chunk.block_count(RED_MUSHROOM)) > 0
                    && chunk.block_count(SUGAR_CANE) > 0
                    && chunk.block_count(LILY_PAD) > 0
            }
            Self::ForestOakBirchTrees => {
                (chunk.block_count(OAK_LOG) > 0 && chunk.block_count(OAK_LEAVES) > 0)
                    || (chunk.block_count(BIRCH_LOG) > 0 && chunk.block_count(BIRCH_LEAVES) > 0)
            }
            Self::MountainTrees => {
                (chunk.block_count(OAK_LOG) > 0 && chunk.block_count(OAK_LEAVES) > 0)
                    || (chunk.block_count(SPRUCE_LOG) > 0 && chunk.block_count(SPRUCE_LEAVES) > 0)
            }
            Self::SunflowerPlainsSunflowers => {
                chunk.block_count(SUNFLOWER_LOWER) > 0 && chunk.block_count(SUNFLOWER_UPPER) > 0
            }
            Self::FlowerForestFlowers => {
                let small_flower_count = [
                    DANDELION,
                    POPPY,
                    ALLIUM,
                    AZURE_BLUET,
                    RED_TULIP,
                    ORANGE_TULIP,
                    WHITE_TULIP,
                    PINK_TULIP,
                    OXEYE_DAISY,
                    CORNFLOWER,
                    LILY_OF_THE_VALLEY,
                ]
                .iter()
                .filter(|block| chunk.block_count(**block) > 0)
                .count();
                let has_double_flower = [
                    (LILAC_LOWER, LILAC_UPPER),
                    (ROSE_BUSH_LOWER, ROSE_BUSH_UPPER),
                    (PEONY_LOWER, PEONY_UPPER),
                ]
                .iter()
                .any(|(lower, upper)| {
                    chunk.block_count(*lower) > 0 && chunk.block_count(*upper) > 0
                });
                small_flower_count >= 4 && has_double_flower
            }
            Self::TaigaSpruceFernBerry => {
                [
                    SPRUCE_LOG,
                    SPRUCE_LEAVES,
                    FERN,
                    LARGE_FERN_LOWER,
                    LARGE_FERN_UPPER,
                ]
                .iter()
                .any(|block| chunk.block_count(*block) > 0)
                    && chunk.block_count(SWEET_BERRY_BUSH) > 0
            }
            Self::GiantTaigaSpruceTrees => {
                chunk.block_count(SPRUCE_LOG) >= 20
                    && chunk.block_count(SPRUCE_LEAVES) > 0
                    && chunk.block_count(PODZOL) > 0
                    && has_two_by_two_log_square(chunk, SPRUCE_LOG)
            }
            Self::FrozenOceanIcebergs => {
                chunk.block_count(PACKED_ICE) > 0 && chunk.block_count(BLUE_ICE) > 0
            }
            Self::WarmOceanCoralSeaPickles => {
                let has_coral_block = [
                    TUBE_CORAL_BLOCK,
                    BRAIN_CORAL_BLOCK,
                    BUBBLE_CORAL_BLOCK,
                    FIRE_CORAL_BLOCK,
                    HORN_CORAL_BLOCK,
                ]
                .iter()
                .any(|block| chunk.block_count(*block) > 0);
                let has_coral_plant = [
                    TUBE_CORAL,
                    BRAIN_CORAL,
                    BUBBLE_CORAL,
                    FIRE_CORAL,
                    HORN_CORAL,
                ]
                .iter()
                .any(|block| chunk.block_count(*block) > 0);
                let has_coral_fan = [
                    TUBE_CORAL_FAN,
                    BRAIN_CORAL_FAN,
                    BUBBLE_CORAL_FAN,
                    FIRE_CORAL_FAN,
                    HORN_CORAL_FAN,
                ]
                .iter()
                .any(|block| chunk.block_count(*block) > 0);
                let has_wall_fan = [
                    TUBE_CORAL_WALL_FAN_NORTH,
                    TUBE_CORAL_WALL_FAN_EAST,
                    TUBE_CORAL_WALL_FAN_SOUTH,
                    TUBE_CORAL_WALL_FAN_WEST,
                    BRAIN_CORAL_WALL_FAN_NORTH,
                    BRAIN_CORAL_WALL_FAN_EAST,
                    BRAIN_CORAL_WALL_FAN_SOUTH,
                    BRAIN_CORAL_WALL_FAN_WEST,
                    BUBBLE_CORAL_WALL_FAN_NORTH,
                    BUBBLE_CORAL_WALL_FAN_EAST,
                    BUBBLE_CORAL_WALL_FAN_SOUTH,
                    BUBBLE_CORAL_WALL_FAN_WEST,
                    FIRE_CORAL_WALL_FAN_NORTH,
                    FIRE_CORAL_WALL_FAN_EAST,
                    FIRE_CORAL_WALL_FAN_SOUTH,
                    FIRE_CORAL_WALL_FAN_WEST,
                    HORN_CORAL_WALL_FAN_NORTH,
                    HORN_CORAL_WALL_FAN_EAST,
                    HORN_CORAL_WALL_FAN_SOUTH,
                    HORN_CORAL_WALL_FAN_WEST,
                ]
                .iter()
                .any(|block| chunk.block_count(*block) > 0);
                let has_sea_pickle = [SEA_PICKLE_1, SEA_PICKLE_2, SEA_PICKLE_3, SEA_PICKLE_4]
                    .iter()
                    .any(|block| chunk.block_count(*block) > 0);

                has_coral_block
                    && has_coral_plant
                    && has_coral_fan
                    && has_wall_fan
                    && has_sea_pickle
            }
            Self::DarkForestCanopyMushrooms => {
                [DARK_OAK_LOG, DARK_OAK_LEAVES]
                    .iter()
                    .any(|block| chunk.block_count(*block) > 0)
                    && [BROWN_MUSHROOM_BLOCK, RED_MUSHROOM_BLOCK, MUSHROOM_STEM]
                        .iter()
                        .any(|block| chunk.block_count(*block) > 0)
            }
            Self::MushroomFieldHugeMushrooms => {
                chunk.block_count(MUSHROOM_STEM) > 0
                    && (chunk.block_count(BROWN_MUSHROOM_BLOCK)
                        + chunk.block_count(RED_MUSHROOM_BLOCK))
                        > 0
            }
            Self::BirchTrees => {
                chunk.block_count(BIRCH_LOG) > 0 && chunk.block_count(BIRCH_LEAVES) > 0
            }
            Self::SavannaAcacia => {
                chunk.block_count(ACACIA_LOG) > 0 && chunk.block_count(ACACIA_LEAVES) > 0
            }
            Self::JungleTrees => {
                chunk.block_count(JUNGLE_LOG) > 0 && chunk.block_count(JUNGLE_LEAVES) > 0
            }
            Self::SparseJungleTrees => {
                chunk.block_count(JUNGLE_LOG) > 0 || chunk.block_count(JUNGLE_LEAVES) > 0
            }
            Self::BambooJungle => {
                (chunk.block_count(BAMBOO)
                    + chunk.block_count(BAMBOO_TOP_SMALL)
                    + chunk.block_count(BAMBOO_TOP_LARGE)
                    + chunk.block_count(BAMBOO_FINAL_LARGE))
                    > 0
                    && chunk.block_count(BAMBOO_TOP_SMALL) > 0
                    && chunk.block_count(BAMBOO_TOP_LARGE) > 0
                    && chunk.block_count(BAMBOO_FINAL_LARGE) > 0
                    && chunk.block_count(JUNGLE_LOG) > 0
                    && chunk.block_count(JUNGLE_LEAVES) > 0
            }
            _ => self
                .blocks()
                .iter()
                .any(|block| chunk.block_count(*block) > 0),
        }
    }
}
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

#[derive(Debug, Default, PartialEq, Eq)]
struct SurfaceTopSignal {
    min_top_y: i32,
    max_top_y: i32,
    columns_above_80: usize,
    columns_above_100: usize,
    badlands_top_columns: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MacroGeometrySignal {
    top_y_min: i32,
    top_y_max: i32,
    top_y_range: i32,
    top_y_p05: i32,
    top_y_p50: i32,
    top_y_p95: i32,
    neighbor_delta_ge_4: usize,
    neighbor_delta_ge_8: usize,
    neighbor_delta_ge_16: usize,
    vertical_face_columns: usize,
    solid_over_air_blocks: usize,
    surface_near_carved_air_columns: usize,
    carved_air_volume: usize,
    carved_air_y_min: Option<i32>,
    carved_air_y_max: Option<i32>,
    long_vertical_air_spans: usize,
    water_land_edge_delta_ge_4: usize,
    water_land_edge_delta_ge_8: usize,
    landmark_block_volume: usize,
    landmark_column_count: usize,
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
mod features;
mod fixtures;
mod generated_chunk;
mod low_visibility_cases;
mod noise;
mod palette;
mod palette_assertions;
mod palette_matrix_cases;
mod taiga_diagnostics;
mod terrain;

use fixtures::*;
use low_visibility_cases::LOW_VISIBILITY_FEATURE_CASES;
use palette_assertions::*;
use palette_matrix_cases::PALETTE_MATRIX_CASES;
use taiga_diagnostics::*;
use terrain::*;
