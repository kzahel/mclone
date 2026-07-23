use std::sync::OnceLock;

use mclone_core::chunk_min_block_coord;

use crate::biome::get_layered_biome_by_id;
use crate::block::{
    DANDELION, DIRT, FERN, GRASS, GRASS_BLOCK, LARGE_FERN_LOWER, POPPY, RawBlockId,
    SWEET_BERRY_BUSH, TALL_GRASS_LOWER,
};
use crate::feature::{
    BasicTreeConfiguration, ConfiguredFeature, DecorationStep, FeatureRegion, FeatureWorld,
    PlacedFeature, RandomFeatureConfiguration, RandomPatchConfiguration, RandomPatchStateProvider,
    TreeConfiguration, WeightedConfiguredFeature, apply_feature_table_to_region_timed,
    flower_patch, grass_patch, tree_feature,
};
use crate::levelgen::profile::PLAINS_BIOME_ID;
use crate::noise::SeedDomain;
use crate::placement::{ConfiguredDecorator, HeightmapType};

use super::biomes::{
    MCLONE_OVERWORLD_FOREST_BIOME_ID, MCLONE_OVERWORLD_SAVANNA_BIOME_ID,
    MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID, MCLONE_OVERWORLD_TAIGA_BIOME_ID,
    mclone_overworld_biome_id_with_topology,
};
use super::fields::McloneOverworldSamplingTopology;

pub const MCLONE_OVERWORLD_DECORATION_REVISION: &str = "mclone-overworld-v1-decoration-10";

const MCLONE_OVERWORLD_DECORATION_DOMAIN: SeedDomain = SeedDomain::new(0x6d63_6f76_6465_6331);

pub(super) fn decorate_mclone_overworld_center(seed: i64, region: &mut FeatureRegion) {
    decorate_mclone_overworld_center_with_topology(
        seed,
        McloneOverworldSamplingTopology::Unbounded,
        region,
    );
}

pub(super) fn decorate_mclone_overworld_center_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    region: &mut FeatureRegion,
) {
    let min_x = chunk_min_block_coord(region.decoration_chunk_x());
    let min_z = chunk_min_block_coord(region.decoration_chunk_z());
    let biome_id = mclone_overworld_biome_id_with_topology(seed, topology, min_x + 8, min_z + 8);
    let features = feature_table(biome_id);
    if features.is_empty() {
        return;
    }
    apply_feature_table_to_region_timed(
        MCLONE_OVERWORLD_DECORATION_DOMAIN.derive(seed),
        get_layered_biome_by_id(biome_id),
        features,
        region,
    );
}

fn feature_table(biome_id: i32) -> &'static [PlacedFeature] {
    match biome_id {
        PLAINS_BIOME_ID => open_lowland_features(),
        MCLONE_OVERWORLD_FOREST_BIOME_ID => wooded_upland_features(),
        MCLONE_OVERWORLD_TAIGA_BIOME_ID => cool_wet_conifer_features(),
        MCLONE_OVERWORLD_SAVANNA_BIOME_ID => warm_dry_steppe_features(),
        MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID => &[],
        _ => &[],
    }
}

fn open_lowland_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                tree_feature(BasicTreeConfiguration::oak(), 0, 0.20, 1),
                grass_patch(GRASS, 4),
                occasional_flower_patch(DANDELION, 3),
                occasional_flower_patch(POPPY, 5),
            ]
        })
        .as_slice()
}

fn wooded_upland_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                tree_feature(BasicTreeConfiguration::oak(), 4, 0.35, 1),
                grass_patch(GRASS, 2),
                occasional_flower_patch(DANDELION, 4),
                occasional_flower_patch(POPPY, 6),
            ]
        })
        .as_slice()
}

fn cool_wet_conifer_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                conifer_tree_feature(),
                grass_patch(FERN, 3),
                grass_patch(GRASS, 1),
                rare_large_fern_patch(),
                rare_berry_patch(),
            ]
        })
        .as_slice()
}

fn warm_dry_steppe_features() -> &'static [PlacedFeature] {
    static FEATURES: OnceLock<Vec<PlacedFeature>> = OnceLock::new();
    FEATURES
        .get_or_init(|| {
            vec![
                acacia_tree_feature(),
                tall_grass_patch(),
                grass_patch(GRASS, 5),
                occasional_flower_patch(DANDELION, 5),
                occasional_flower_patch(POPPY, 8),
            ]
        })
        .as_slice()
}

fn conifer_tree_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_selector(RandomFeatureConfiguration::new(
            [WeightedConfiguredFeature::new(
                ConfiguredFeature::tree(TreeConfiguration::pine()),
                0.36,
            )],
            ConfiguredFeature::tree(TreeConfiguration::spruce()),
        )),
        tree_decorators(6, 0.2, 1),
    )
}

fn acacia_tree_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::tree(TreeConfiguration::acacia()),
        tree_decorators(1, 0.25, 1),
    )
}

fn tree_decorators(count: i32, extra_chance: f32, extra_count: i32) -> Vec<ConfiguredDecorator> {
    vec![
        ConfiguredDecorator::count_extra(count, extra_chance, extra_count),
        ConfiguredDecorator::square(),
        ConfiguredDecorator::water_depth_threshold(0),
        ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
    ]
}

fn tall_grass_patch() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: TALL_GRASS_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 48,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK, DIRT],
        }),
        vec![
            ConfiguredDecorator::count(2),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn rare_large_fern_patch() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 32,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK, DIRT],
        }),
        vec![
            ConfiguredDecorator::chance(3),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn rare_berry_patch() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: SWEET_BERRY_BUSH,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 64,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK],
        }),
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ],
    )
}

fn occasional_flower_patch(block_id: RawBlockId, rarity: i32) -> PlacedFeature {
    let mut patch = flower_patch(block_id, 1);
    patch
        .decorators
        .insert(0, ConfiguredDecorator::chance(rarity));
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::profile::{BEACH_BIOME_ID, OCEAN_BIOME_ID};

    #[test]
    fn mclone_tables_own_temperate_conifer_and_steppe_language() {
        assert_eq!(feature_table(PLAINS_BIOME_ID).len(), 4);
        assert_eq!(feature_table(MCLONE_OVERWORLD_FOREST_BIOME_ID).len(), 4);
        assert_eq!(feature_table(MCLONE_OVERWORLD_TAIGA_BIOME_ID).len(), 5);
        assert_eq!(feature_table(MCLONE_OVERWORLD_SAVANNA_BIOME_ID).len(), 5);
        assert!(feature_table(MCLONE_OVERWORLD_SNOWY_MOUNTAINS_BIOME_ID).is_empty());
        assert!(feature_table(OCEAN_BIOME_ID).is_empty());
        assert!(feature_table(BEACH_BIOME_ID).is_empty());
    }
}
