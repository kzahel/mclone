use std::sync::OnceLock;

use crate::biome::BiomeDefinition;
use crate::block::{
    ANDESITE, CACTUS, CLAY, COAL_ORE, COPPER_ORE, DANDELION, DEAD_BUSH, DEEPSLATE,
    DEEPSLATE_COAL_ORE, DEEPSLATE_COPPER_ORE, DEEPSLATE_DIAMOND_ORE, DEEPSLATE_GOLD_ORE,
    DEEPSLATE_IRON_ORE, DEEPSLATE_LAPIS_ORE, DEEPSLATE_REDSTONE_ORE, DIAMOND_ORE, DIORITE, DIRT,
    FERN, GOLD_ORE, GRANITE, GRASS, GRASS_BLOCK, GRAVEL, IRON_ORE, LAPIS_ORE, LARGE_FERN_LOWER,
    LAVA, MYCELIUM, PODZOL, POPPY, RED_SAND, REDSTONE_ORE, RawBlockId, SAND, SUGAR_CANE,
    TERRACOTTA, TUFF, WATER,
};
use crate::placement::{
    ConfiguredDecorator, CountConfiguration, HeightProvider, HeightmapType, IntProvider,
    VerticalAnchor,
};

use super::{
    BasicTreeConfiguration, ConfiguredFeature, CoralShape, DecorationStep, DiskConfiguration,
    DripstoneClusterConfiguration, FloatProvider, GlowLichenConfiguration,
    HugeMushroomConfiguration, LakeConfiguration, OreConfiguration, PlacedFeature,
    RandomFeatureConfiguration, RandomPatchConfiguration, SeagrassConfiguration,
    SimpleRandomFeatureConfiguration, SmallDripstoneConfiguration, SpringConfiguration,
    TreeConfiguration, WeightedBlockState, WeightedConfiguredFeature,
};

pub(super) const TAIGA_GRASS_STATES: [WeightedBlockState; 2] = [
    WeightedBlockState::new(GRASS, 1),
    WeightedBlockState::new(FERN, 4),
];
pub(super) const DEFAULT_FLOWER_STATES: [WeightedBlockState; 2] = [
    WeightedBlockState::new(POPPY, 2),
    WeightedBlockState::new(DANDELION, 1),
];
const DISK_SAND_TARGETS: [RawBlockId; 2] = [DIRT, GRASS_BLOCK];
const DISK_CLAY_TARGETS: [RawBlockId; 2] = [DIRT, CLAY];
const DISK_GRAVEL_TARGETS: [RawBlockId; 2] = [DIRT, GRASS_BLOCK];

pub fn overworld_features_for_biome(biome: BiomeDefinition) -> Vec<PlacedFeature> {
    overworld_features_for_biome_cached(biome).to_vec()
}

pub(super) fn overworld_features_for_biome_cached(
    biome: BiomeDefinition,
) -> &'static [PlacedFeature] {
    match biome.key() {
        "minecraft:plains" | "minecraft:sunflower_plains" => plains_feature_table(),
        "minecraft:forest" | "minecraft:wooded_hills" | "minecraft:flower_forest" => {
            forest_feature_table()
        }
        "minecraft:birch_forest"
        | "minecraft:birch_forest_hills"
        | "minecraft:tall_birch_forest"
        | "minecraft:tall_birch_hills" => birch_forest_feature_table(),
        "minecraft:taiga"
        | "minecraft:taiga_hills"
        | "minecraft:taiga_mountains"
        | "minecraft:giant_tree_taiga"
        | "minecraft:giant_tree_taiga_hills"
        | "minecraft:giant_spruce_taiga"
        | "minecraft:giant_spruce_taiga_hills" => taiga_feature_table(),
        "minecraft:snowy_taiga"
        | "minecraft:snowy_taiga_hills"
        | "minecraft:snowy_taiga_mountains"
        | "minecraft:snowy_tundra"
        | "minecraft:snowy_mountains" => snowy_feature_table(),
        "minecraft:mountains"
        | "minecraft:wooded_mountains"
        | "minecraft:mountain_edge"
        | "minecraft:gravelly_mountains"
        | "minecraft:modified_gravelly_mountains" => mountain_feature_table(),
        "minecraft:desert" | "minecraft:desert_hills" | "minecraft:desert_lakes" => {
            desert_feature_table()
        }
        "minecraft:badlands"
        | "minecraft:badlands_plateau"
        | "minecraft:wooded_badlands_plateau"
        | "minecraft:modified_badlands_plateau"
        | "minecraft:modified_wooded_badlands_plateau"
        | "minecraft:eroded_badlands" => badlands_feature_table(),
        "minecraft:swamp" | "minecraft:swamp_hills" => swamp_feature_table(),
        "minecraft:mushroom_fields" | "minecraft:mushroom_field_shore" => {
            mushroom_field_feature_table()
        }
        "minecraft:dark_forest" => dark_forest_feature_table(false),
        "minecraft:dark_forest_hills" => dark_forest_feature_table(true),
        "minecraft:ocean" => ocean_feature_table(false),
        "minecraft:deep_ocean" => ocean_feature_table(true),
        "minecraft:cold_ocean" => cold_ocean_feature_table(false),
        "minecraft:deep_cold_ocean" => cold_ocean_feature_table(true),
        "minecraft:lukewarm_ocean" => lukewarm_ocean_feature_table(false),
        "minecraft:deep_lukewarm_ocean" => lukewarm_ocean_feature_table(true),
        "minecraft:warm_ocean" => warm_ocean_feature_table(false),
        "minecraft:deep_warm_ocean" => warm_ocean_feature_table(true),
        "minecraft:frozen_ocean" | "minecraft:deep_frozen_ocean" => frozen_ocean_feature_table(),
        _ => default_land_feature_table(),
    }
}

fn build_overworld_feature_table(
    biome_key: &str,
    mut biome_features: Vec<PlacedFeature>,
) -> Vec<PlacedFeature> {
    let mut features = default_lake_features(biome_key);
    features.extend(default_underground_variety_features());
    features.extend(default_ore_features());
    features.extend(default_disk_features(biome_key));
    features.append(&mut biome_features);
    features.extend(default_top_layer_features());
    features
}

fn plains_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:plains", plains_features()))
        .as_slice()
}

fn forest_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:forest", forest_features()))
        .as_slice()
}

fn birch_forest_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            build_overworld_feature_table("minecraft:birch_forest", birch_forest_features())
        })
        .as_slice()
}

fn taiga_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:taiga", taiga_features()))
        .as_slice()
}

fn snowy_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:snowy_taiga", snowy_features()))
        .as_slice()
}

fn mountain_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:mountains", mountain_features()))
        .as_slice()
}

fn desert_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:desert", desert_features()))
        .as_slice()
}

fn badlands_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:badlands", badlands_features()))
        .as_slice()
}

fn swamp_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:swamp", swamp_features()))
        .as_slice()
}

fn mushroom_field_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            build_overworld_feature_table("minecraft:mushroom_fields", mushroom_field_features())
        })
        .as_slice()
}

fn dark_forest_feature_table(red_mushrooms_first: bool) -> &'static [PlacedFeature] {
    static DARK_FOREST: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    static DARK_FOREST_HILLS: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    let features = if red_mushrooms_first {
        &DARK_FOREST_HILLS
    } else {
        &DARK_FOREST
    };
    features
        .get_or_init(|| {
            build_overworld_feature_table(
                "minecraft:dark_forest",
                dark_forest_features(red_mushrooms_first),
            )
        })
        .as_slice()
}

fn ocean_feature_table(deep: bool) -> &'static [PlacedFeature] {
    static OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    static DEEP_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    let features = if deep { &DEEP_OCEAN } else { &OCEAN };
    features
        .get_or_init(|| build_overworld_feature_table("minecraft:ocean", ocean_features(deep)))
        .as_slice()
}

fn cold_ocean_feature_table(deep: bool) -> &'static [PlacedFeature] {
    static COLD_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    static DEEP_COLD_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    let features = if deep { &DEEP_COLD_OCEAN } else { &COLD_OCEAN };
    features
        .get_or_init(|| {
            build_overworld_feature_table("minecraft:cold_ocean", cold_ocean_features(deep))
        })
        .as_slice()
}

fn lukewarm_ocean_feature_table(deep: bool) -> &'static [PlacedFeature] {
    static LUKEWARM_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    static DEEP_LUKEWARM_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    let features = if deep {
        &DEEP_LUKEWARM_OCEAN
    } else {
        &LUKEWARM_OCEAN
    };
    features
        .get_or_init(|| {
            build_overworld_feature_table("minecraft:lukewarm_ocean", lukewarm_ocean_features(deep))
        })
        .as_slice()
}

fn warm_ocean_feature_table(deep: bool) -> &'static [PlacedFeature] {
    static WARM_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    static DEEP_WARM_OCEAN: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    let features = if deep { &DEEP_WARM_OCEAN } else { &WARM_OCEAN };
    features
        .get_or_init(|| {
            build_overworld_feature_table("minecraft:warm_ocean", warm_ocean_features(deep))
        })
        .as_slice()
}

fn frozen_ocean_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:frozen_ocean", Vec::new()))
        .as_slice()
}

fn default_land_feature_table() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| build_overworld_feature_table("minecraft:plains", default_land_features()))
        .as_slice()
}

fn plains_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 0, 0.35, 1),
        grass_patch(GRASS, 4),
        flower_patch(DANDELION, 1),
        flower_patch(POPPY, 1),
    ]
}

fn default_lake_features(biome_key: &str) -> Vec<PlacedFeature> {
    if matches!(
        biome_key,
        "minecraft:desert" | "minecraft:desert_hills" | "minecraft:desert_lakes"
    ) {
        vec![lava_lake_feature()]
    } else {
        vec![water_lake_feature(), lava_lake_feature()]
    }
}

fn water_lake_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::Lakes,
        ConfiguredFeature::lake(LakeConfiguration::new(WATER)),
        vec![
            ConfiguredDecorator::chance(4),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::uniform(
                VerticalAnchor::bottom(),
                VerticalAnchor::top(),
            )),
        ],
    )
}

fn lava_lake_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::Lakes,
        ConfiguredFeature::lake(LakeConfiguration::new(LAVA)),
        vec![
            ConfiguredDecorator::chance(8),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                VerticalAnchor::bottom(),
                VerticalAnchor::top(),
                8,
            )),
            ConfiguredDecorator::lava_lake(80),
        ],
    )
}

fn default_underground_variety_features() -> Vec<PlacedFeature> {
    vec![
        underground_ore_feature(
            DIRT,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::top(),
            10,
        ),
        underground_ore_feature(
            GRAVEL,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::top(),
            8,
        ),
        underground_ore_feature(
            GRANITE,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(79),
            10,
        ),
        underground_ore_feature(
            DIORITE,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(79),
            10,
        ),
        underground_ore_feature(
            ANDESITE,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(79),
            10,
        ),
        underground_ore_feature(
            TUFF,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(16),
            1,
        ),
        underground_ore_feature(
            DEEPSLATE,
            64,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(16),
            2,
        ),
        rare_dripstone_cluster_feature(),
        rare_small_dripstone_feature(),
    ]
}

fn rare_dripstone_cluster_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundDecoration,
        ConfiguredFeature::dripstone_cluster(DripstoneClusterConfiguration::new(
            12,
            IntProvider::uniform(3, 3),
            IntProvider::uniform(2, 6),
            1,
            3,
            IntProvider::uniform(2, 2),
            FloatProvider::uniform(0.3, 0.4),
            FloatProvider::clamped_normal(0.1, 0.3, 0.1, 0.9),
            0.1,
            3,
            8,
        )),
        vec![
            ConfiguredDecorator::chance(25),
            ConfiguredDecorator::Count(CountConfiguration::from_provider(IntProvider::uniform(
                10, 10,
            ))),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::uniform(
                VerticalAnchor::bottom(),
                VerticalAnchor::absolute(59),
            )),
        ],
    )
}

fn rare_small_dripstone_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundDecoration,
        ConfiguredFeature::small_dripstone(SmallDripstoneConfiguration::new(5, 10, 2, 0.2)),
        vec![
            ConfiguredDecorator::chance(30),
            ConfiguredDecorator::Count(CountConfiguration::from_provider(IntProvider::uniform(
                40, 80,
            ))),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::uniform(
                VerticalAnchor::bottom(),
                VerticalAnchor::absolute(59),
            )),
        ],
    )
}

fn default_ore_features() -> Vec<PlacedFeature> {
    vec![
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(COAL_ORE, DEEPSLATE_COAL_ORE, 17),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(127)),
            20,
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(IRON_ORE, DEEPSLATE_IRON_ORE, 9),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(63)),
            20,
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(GOLD_ORE, DEEPSLATE_GOLD_ORE, 9),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(31)),
            2,
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(REDSTONE_ORE, DEEPSLATE_REDSTONE_ORE, 8),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
            8,
        ),
        single_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(DIAMOND_ORE, DEEPSLATE_DIAMOND_ORE, 8),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
        ),
        single_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(LAPIS_ORE, DEEPSLATE_LAPIS_ORE, 7),
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(30), 0),
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(COPPER_ORE, DEEPSLATE_COPPER_ORE, 10),
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(96), 0),
            6,
        ),
    ]
}

fn default_top_layer_features() -> Vec<PlacedFeature> {
    vec![freeze_top_layer_feature()]
}

fn freeze_top_layer_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::TopLayerModification,
        ConfiguredFeature::freeze_top_layer(),
        Vec::new(),
    )
}

fn underground_ore_feature(
    block_id: RawBlockId,
    size: i32,
    min_inclusive: VerticalAnchor,
    max_inclusive: VerticalAnchor,
    count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::ore(OreConfiguration::natural_stone(block_id, size)),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::uniform(min_inclusive, max_inclusive)),
        ],
    )
}

fn counted_ore_feature(
    config: OreConfiguration,
    height: HeightProvider,
    count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::ore(config),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(height),
        ],
    )
}

fn single_ore_feature(config: OreConfiguration, height: HeightProvider) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::ore(config),
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(height),
        ],
    )
}

fn default_disk_features(biome_key: &str) -> Vec<PlacedFeature> {
    if matches!(biome_key, "minecraft:swamp" | "minecraft:swamp_hills") {
        vec![disk_clay_feature()]
    } else {
        vec![
            disk_sand_feature(),
            disk_clay_feature(),
            disk_gravel_feature(),
        ]
    }
}

fn disk_sand_feature() -> PlacedFeature {
    disk_feature(
        DiskConfiguration::new(SAND, IntProvider::uniform(2, 6), 2, &DISK_SAND_TARGETS),
        Some(3),
    )
}

fn disk_clay_feature() -> PlacedFeature {
    disk_feature(
        DiskConfiguration::new(CLAY, IntProvider::uniform(2, 3), 1, &DISK_CLAY_TARGETS),
        None,
    )
}

fn disk_gravel_feature() -> PlacedFeature {
    disk_feature(
        DiskConfiguration::new(GRAVEL, IntProvider::uniform(2, 5), 2, &DISK_GRAVEL_TARGETS),
        None,
    )
}

fn disk_feature(config: DiskConfiguration, count: Option<i32>) -> PlacedFeature {
    let mut decorators = Vec::new();
    if let Some(count) = count {
        decorators.push(ConfiguredDecorator::count(count));
    }
    decorators.push(ConfiguredDecorator::square());
    decorators.push(ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg));
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::disk(config),
        decorators,
    )
}

fn forest_features() -> Vec<PlacedFeature> {
    vec![
        omitted_vegetal_feature(),
        glow_lichen_feature(),
        forest_birch_other_feature(),
        default_flower_feature(),
        forest_grass_patch_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        spring_water_feature(),
        spring_lava_feature(),
    ]
}

fn birch_forest_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::birch(), 7, 0.3, 2),
        grass_patch(GRASS, 3),
        flower_patch(DANDELION, 1),
        flower_patch(POPPY, 1),
    ]
}

fn taiga_features() -> Vec<PlacedFeature> {
    vec![
        large_fern_patch_feature(),
        glow_lichen_feature(),
        taiga_vegetation_feature(),
        default_flower_feature(),
        taiga_grass_patch_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        spring_water_feature(),
        spring_lava_feature(),
        omitted_vegetal_feature(),
    ]
}

fn snowy_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::spruce(), 3, 0.2, 1),
        grass_patch(FERN, 1),
    ]
}

fn mountain_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::spruce(), 1, 0.25, 1),
        tree_feature(BasicTreeConfiguration::oak(), 0, 0.2, 1),
        grass_patch(GRASS, 1),
    ]
}

fn desert_features() -> Vec<PlacedFeature> {
    vec![dead_bush_patch(2), sugar_cane_patch(60), cactus_patch(10)]
}

fn badlands_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 1, 0.1, 1),
        dead_bush_patch(2),
        sugar_cane_patch(13),
        cactus_patch(5),
    ]
}

fn swamp_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 2, 0.25, 1),
        grass_patch(GRASS, 2),
        flower_patch(POPPY, 1),
        dead_bush_patch(1),
        sugar_cane_patch(20),
    ]
}

fn mushroom_field_features() -> Vec<PlacedFeature> {
    vec![grass_patch(GRASS, 1)]
}

fn dark_forest_features(red_mushrooms_first: bool) -> Vec<PlacedFeature> {
    vec![
        dark_forest_vegetation_feature(red_mushrooms_first),
        default_flower_feature(),
        forest_grass_patch_feature(),
        omitted_vegetal_feature(),
        omitted_vegetal_feature(),
        spring_water_feature(),
        spring_lava_feature(),
    ]
}

fn ocean_features(deep: bool) -> Vec<PlacedFeature> {
    vec![
        seagrass_feature(48, if deep { 0.8 } else { 0.3 }),
        kelp_feature(120),
    ]
}

fn cold_ocean_features(deep: bool) -> Vec<PlacedFeature> {
    vec![
        seagrass_feature(if deep { 40 } else { 32 }, if deep { 0.8 } else { 0.3 }),
        kelp_feature(120),
    ]
}

fn lukewarm_ocean_features(deep: bool) -> Vec<PlacedFeature> {
    vec![
        seagrass_feature(80, if deep { 0.8 } else { 0.3 }),
        kelp_feature(80),
    ]
}

fn warm_ocean_features(deep: bool) -> Vec<PlacedFeature> {
    if deep {
        vec![seagrass_feature(80, 0.8)]
    } else {
        vec![
            warm_ocean_vegetation_feature(),
            seagrass_feature(80, 0.3),
            sea_pickle_feature(),
        ]
    }
}

fn default_land_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 1, 0.1, 1),
        grass_patch(GRASS, 2),
        flower_patch(DANDELION, 1),
    ]
}

fn tree_feature(
    config: BasicTreeConfiguration,
    count: i32,
    extra_chance: f32,
    extra_count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::basic_tree(config),
        vec![
            ConfiguredDecorator::count_extra(count, extra_chance, extra_count),
            ConfiguredDecorator::square(),
        ],
    )
}

fn omitted_vegetal_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::noop(),
        Vec::new(),
    )
}

pub(super) fn taiga_vegetation_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_selector(RandomFeatureConfiguration::new(
            [WeightedConfiguredFeature::new(
                ConfiguredFeature::tree(TreeConfiguration::pine()),
                0.33333334,
            )],
            ConfiguredFeature::tree(TreeConfiguration::spruce()),
        )),
        vec![
            ConfiguredDecorator::count_extra(10, 0.1, 1),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ],
    )
}

fn forest_birch_other_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_selector(RandomFeatureConfiguration::new(
            [
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002()),
                    0.2,
                ),
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::fancy_oak_bees_0002()),
                    0.1,
                ),
            ],
            ConfiguredFeature::tree(TreeConfiguration::oak_bees_0002()),
        )),
        tree_threshold_decorators(10, 0.1, 1),
    )
}

fn dark_forest_vegetation_feature(red_mushrooms_first: bool) -> PlacedFeature {
    let first = if red_mushrooms_first {
        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red())
    } else {
        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown())
    };
    let second = if red_mushrooms_first {
        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown())
    } else {
        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red())
    };
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_selector(RandomFeatureConfiguration::new(
            [
                WeightedConfiguredFeature::new(first, 0.025),
                WeightedConfiguredFeature::new(second, 0.05),
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::dark_oak()),
                    0.6666667,
                ),
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::birch()),
                    0.2,
                ),
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::fancy_oak()),
                    0.1,
                ),
            ],
            ConfiguredFeature::tree(TreeConfiguration::oak()),
        )),
        vec![
            ConfiguredDecorator::dark_oak_tree(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ],
    )
}

fn tree_threshold_decorators(
    count: i32,
    extra_chance: f32,
    extra_count: i32,
) -> Vec<ConfiguredDecorator> {
    vec![
        ConfiguredDecorator::count_extra(count, extra_chance, extra_count),
        ConfiguredDecorator::square(),
        ConfiguredDecorator::water_depth_threshold(0),
        ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
    ]
}

fn default_flower_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::flower(RandomPatchConfiguration {
            state: POPPY,
            weighted_states: &DEFAULT_FLOWER_STATES,
            tries: 64,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[],
        }),
        vec![
            ConfiguredDecorator::count(2),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn forest_grass_patch_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: GRASS,
            weighted_states: &[],
            tries: 32,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[],
        }),
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ],
    )
}

fn taiga_grass_patch_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: GRASS,
            weighted_states: &TAIGA_GRASS_STATES,
            tries: 32,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[],
        }),
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ],
    )
}

fn large_fern_patch_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            weighted_states: &[],
            tries: 64,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK, DIRT, PODZOL, MYCELIUM],
        }),
        vec![
            ConfiguredDecorator::count(7),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn glow_lichen_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld()),
        vec![
            ConfiguredDecorator::Count(crate::placement::CountConfiguration::from_provider(
                IntProvider::uniform(20, 30),
            )),
            ConfiguredDecorator::range(HeightProvider::uniform(
                VerticalAnchor::bottom(),
                VerticalAnchor::absolute(54),
            )),
            ConfiguredDecorator::square(),
        ],
    )
}

fn spring_water_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::spring(SpringConfiguration::water()),
        vec![
            ConfiguredDecorator::count(50),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                VerticalAnchor::bottom(),
                VerticalAnchor::below_top(8),
                8,
            )),
        ],
    )
}

fn spring_lava_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::spring(SpringConfiguration::lava()),
        vec![
            ConfiguredDecorator::count(20),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::very_biased_to_bottom(
                VerticalAnchor::bottom(),
                VerticalAnchor::below_top(8),
                8,
            )),
        ],
    )
}

fn grass_patch(block_id: RawBlockId, count: i32) -> PlacedFeature {
    random_patch_feature(
        RandomPatchConfiguration {
            state: block_id,
            weighted_states: &[],
            tries: 48,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK, DIRT, PODZOL, MYCELIUM],
        },
        count,
    )
}

fn flower_patch(block_id: RawBlockId, count: i32) -> PlacedFeature {
    random_patch_feature(RandomPatchConfiguration::new(block_id), count)
}

fn dead_bush_patch(count: i32) -> PlacedFeature {
    random_patch_feature(
        RandomPatchConfiguration {
            state: DEAD_BUSH,
            weighted_states: &[],
            tries: 16,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[SAND, RED_SAND, TERRACOTTA, DIRT, GRASS_BLOCK, PODZOL],
        },
        count,
    )
}

fn cactus_patch(count: i32) -> PlacedFeature {
    heightmap_double_random_patch_feature(
        RandomPatchConfiguration {
            state: CACTUS,
            weighted_states: &[],
            tries: 10,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: Some(IntProvider::biased_to_bottom(1, 3)),
            need_water: false,
            place_on: &[],
        },
        count,
    )
}

fn sugar_cane_patch(count: i32) -> PlacedFeature {
    heightmap_double_random_patch_feature(
        RandomPatchConfiguration {
            state: SUGAR_CANE,
            weighted_states: &[],
            tries: 20,
            xspread: 4,
            yspread: 0,
            zspread: 4,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: Some(IntProvider::biased_to_bottom(2, 4)),
            need_water: true,
            place_on: &[],
        },
        count,
    )
}

fn seagrass_feature(count: i32, tall_probability: f32) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::seagrass(SeagrassConfiguration::new(tall_probability)),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ],
    )
}

fn kelp_feature(noise_to_count_ratio: i32) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::kelp(),
        vec![
            ConfiguredDecorator::count_noise_biased(noise_to_count_ratio, 80.0, 0.0),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ],
    )
}

fn warm_ocean_vegetation_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::simple_random_selector(SimpleRandomFeatureConfiguration::new([
            ConfiguredFeature::coral(CoralShape::Tree),
            ConfiguredFeature::coral(CoralShape::Claw),
            ConfiguredFeature::coral(CoralShape::Mushroom),
        ])),
        vec![
            ConfiguredDecorator::count_noise_biased(20, 400.0, 0.0),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ],
    )
}

fn sea_pickle_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::sea_pickle(CountConfiguration::new(20)),
        vec![
            ConfiguredDecorator::chance(16),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ],
    )
}

fn random_patch_feature(config: RandomPatchConfiguration, count: i32) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(config),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
        ],
    )
}

fn heightmap_double_random_patch_feature(
    config: RandomPatchConfiguration,
    count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(config),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ],
    )
}
