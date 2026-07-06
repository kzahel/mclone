use super::tables_support::*;
use super::*;

#[test]
fn biome_feature_tables_select_distinct_visible_families() {
    let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
    let forest = overworld_features_for_biome(get_layered_biome_by_id(4));
    let birch = overworld_features_for_biome(get_layered_biome_by_id(27));
    let taiga = overworld_features_for_biome(get_layered_biome_by_id(5));
    let desert = overworld_features_for_biome(get_layered_biome_by_id(2));

    assert!(plains.iter().any(|feature| {
        matches!(
            feature.feature,
            ConfiguredFeature::BasicTree(BasicTreeConfiguration { log: OAK_LOG, .. })
        )
    }));
    assert!(birch.iter().any(|feature| {
        matches!(
            feature.feature,
            ConfiguredFeature::Tree(TreeConfiguration { log: BIRCH_LOG, .. })
        )
    }));
    assert!(
        forest
            .iter()
            .any(|feature| matches!(feature.feature, ConfiguredFeature::RandomSelector(_)))
    );
    assert!(
        taiga
            .iter()
            .any(|feature| matches!(feature.feature, ConfiguredFeature::RandomSelector(_)))
    );
    assert!(desert.iter().any(|feature| {
        matches!(
            feature.feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                state: DEAD_BUSH,
                ..
            })
        )
    }));
}

#[test]
fn biome_feature_tables_include_desert_badlands_and_swamp_extra_vegetation() {
    let desert = overworld_features_for_biome(get_layered_biome_by_id(2));
    let badlands = overworld_features_for_biome(get_layered_biome_by_id(37));
    let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));

    assert!(has_random_patch(&desert, SUGAR_CANE, 60));
    assert!(has_pumpkin_patch(&desert));
    assert!(has_random_patch(&desert, CACTUS, 10));
    assert!(has_default_water_spring(&desert));
    assert!(has_default_lava_spring(&desert));
    assert!(has_random_patch(&badlands, SUGAR_CANE, 13));
    assert!(has_pumpkin_patch(&badlands));
    assert!(has_random_patch(&badlands, CACTUS, 5));
    assert!(has_default_water_spring(&badlands));
    assert!(has_default_lava_spring(&badlands));
    assert!(has_random_patch(&swamp, SUGAR_CANE, 20));
    assert!(has_pumpkin_patch(&swamp));
    assert!(has_random_patch(&swamp, LILY_PAD, 4));
    assert!(has_default_water_spring(&swamp));
    assert!(has_default_lava_spring(&swamp));
}

#[test]
fn badlands_feature_tables_match_java_wooded_tree_split() {
    for biome_id in [37, 39, 165, 167] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert_badlands_common_features(&features);
        assert!(
            !has_badlands_tree_feature(&features),
            "non-wooded badlands biome {biome_id} should not include TREES_BADLANDS"
        );
    }

    for biome_id in [38, 166] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert_badlands_common_features(&features);
        assert!(
            has_badlands_tree_feature(&features),
            "wooded badlands biome {biome_id} should include TREES_BADLANDS"
        );
    }
}

#[test]
fn biome_feature_tables_include_java_normal_mushroom_patches_for_current_lanes() {
    for biome_id in [
        1, 3, 4, 5, 6, 12, 13, 14, 15, 18, 19, 20, 21, 22, 23, 27, 28, 29, 30, 31, 32, 33, 34, 35,
        36, 37, 38, 39, 129, 131, 132, 133, 134, 140, 149, 151, 155, 156, 157, 158, 160, 161, 162,
        163, 164, 165, 166, 167, 168, 169,
    ] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(has_normal_mushroom_patch(&features, BROWN_MUSHROOM, 4));
        assert!(has_normal_mushroom_patch(&features, RED_MUSHROOM, 8));
    }

    let fallback = overworld_features_for_biome(BiomeDefinition::new(
        999,
        "minecraft:test_fallback",
        0.0,
        0.0,
    ));
    assert!(has_normal_mushroom_patch(&fallback, BROWN_MUSHROOM, 4));
    assert!(has_normal_mushroom_patch(&fallback, RED_MUSHROOM, 8));
}

#[test]
fn biome_feature_tables_include_java_taiga_mushroom_patches_for_current_lanes() {
    for biome_id in [5, 14, 15, 19, 30, 31, 133, 158] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(has_taiga_mushroom_patch(
            &features,
            BROWN_MUSHROOM,
            4,
            None,
            false
        ));
        assert!(has_taiga_mushroom_patch(
            &features,
            RED_MUSHROOM,
            8,
            None,
            true
        ));
    }

    for biome_id in [32, 33, 160, 161] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(has_taiga_mushroom_patch(
            &features,
            BROWN_MUSHROOM,
            4,
            Some(3),
            false
        ));
        assert!(has_taiga_mushroom_patch(
            &features,
            RED_MUSHROOM,
            8,
            Some(3),
            true
        ));
    }
}

#[test]
fn biome_feature_tables_include_java_default_extra_vegetation_for_current_lanes() {
    for biome_id in [
        1, 3, 4, 5, 12, 13, 14, 15, 18, 19, 20, 21, 22, 23, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
        129, 131, 132, 133, 140, 149, 151, 155, 156, 157, 158, 160, 161, 162, 163, 164, 168, 169,
    ] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(has_random_patch(&features, SUGAR_CANE, 10));
        assert!(has_pumpkin_patch(&features));
    }

    let fallback = overworld_features_for_biome(BiomeDefinition::new(
        999,
        "minecraft:test_fallback",
        0.0,
        0.0,
    ));
    assert!(has_random_patch(&fallback, SUGAR_CANE, 10));
    assert!(has_pumpkin_patch(&fallback));
}

#[test]
fn biome_feature_tables_include_java_default_springs_for_current_land_lanes() {
    for biome_id in [
        1, 2, 3, 4, 5, 6, 7, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 25, 26, 27, 28,
        29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 129, 130, 131, 132, 133, 134, 140, 149, 151,
        155, 156, 157, 158, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169,
    ] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(has_default_water_spring(&features));
        assert!(has_default_lava_spring(&features));
    }

    let fallback = overworld_features_for_biome(BiomeDefinition::new(
        999,
        "minecraft:test_fallback",
        0.0,
        0.0,
    ));
    assert!(has_default_water_spring(&fallback));
    assert!(has_default_lava_spring(&fallback));
}

#[test]
fn giant_taiga_feature_tables_include_java_forest_rock_boulders() {
    for biome_id in [32, 33, 160, 161] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(has_forest_rock_feature(&features));
    }

    for biome_id in [3, 20, 34, 131, 162] {
        let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
        assert!(!has_forest_rock_feature(&features));
    }
}

#[test]
fn river_and_beach_feature_tables_include_java_default_vegetation_subset() {
    let river = overworld_features_for_biome(get_layered_biome_by_id(7));
    let frozen_river = overworld_features_for_biome(get_layered_biome_by_id(11));
    let beach = overworld_features_for_biome(get_layered_biome_by_id(16));
    let stone_shore = overworld_features_for_biome(get_layered_biome_by_id(25));
    let snowy_beach = overworld_features_for_biome(get_layered_biome_by_id(26));

    assert!(has_water_tree_feature(&river));
    assert!(has_water_tree_feature(&frozen_river));
    assert!(!has_water_tree_feature(&beach));
    assert!(!has_water_tree_feature(&stone_shore));
    assert!(!has_water_tree_feature(&snowy_beach));

    for features in [&river, &frozen_river, &beach, &stone_shore, &snowy_beach] {
        assert!(has_default_flower_feature(features));
        assert!(has_default_grass_patch_feature(features));
        assert!(has_normal_mushroom_patch(features, BROWN_MUSHROOM, 4));
        assert!(has_normal_mushroom_patch(features, RED_MUSHROOM, 8));
        assert!(has_random_patch(features, SUGAR_CANE, 10));
        assert!(has_pumpkin_patch(features));
    }

    assert!(has_seagrass_feature(&river, 48, 0.4));
    assert!(!has_seagrass_feature(&frozen_river, 48, 0.4));
}

#[test]
fn swamp_feature_table_includes_java_blue_orchid_patch() {
    let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
    let blue_orchid = swamp
        .iter()
        .find(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::Flower(RandomPatchConfiguration {
                    state: BLUE_ORCHID,
                    ..
                })
            )
        })
        .expect("swamp blue orchid feature");

    assert_eq!(
        blue_orchid.decorators,
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ]
    );
    match &blue_orchid.feature {
        ConfiguredFeature::Flower(config) => {
            assert_eq!(
                *config,
                RandomPatchConfiguration {
                    state: BLUE_ORCHID,
                    weighted_states: &[],
                    state_provider: RandomPatchStateProvider::Simple,
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
                }
            );
        }
        other => panic!("expected blue orchid flower feature, got {other:?}"),
    }
}

#[test]
fn swamp_feature_table_includes_java_small_mushroom_patches() {
    let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
    assert_swamp_mushroom_patch(
        &swamp,
        BROWN_MUSHROOM,
        vec![
            ConfiguredDecorator::count(8),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::chance(4),
        ],
    );
    assert_swamp_mushroom_patch(
        &swamp,
        RED_MUSHROOM,
        vec![
            ConfiguredDecorator::count(8),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ConfiguredDecorator::chance(8),
        ],
    );
}

#[test]
fn swamp_feature_table_includes_java_waterlily_patch() {
    let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
    let waterlily = swamp
        .iter()
        .find(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: LILY_PAD,
                    ..
                })
            )
        })
        .expect("swamp waterlily feature");

    assert_eq!(
        waterlily.decorators,
        vec![
            ConfiguredDecorator::count(4),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );
    match &waterlily.feature {
        ConfiguredFeature::RandomPatch(config) => {
            assert_eq!(
                *config,
                RandomPatchConfiguration {
                    state: LILY_PAD,
                    weighted_states: &[],
                    state_provider: RandomPatchStateProvider::Simple,
                    tries: 10,
                    xspread: 7,
                    yspread: 3,
                    zspread: 7,
                    project: true,
                    can_replace: false,
                    double_plant: false,
                    column_height: None,
                    need_water: false,
                    place_on: &[],
                }
            );
        }
        other => panic!("expected waterlily random patch, got {other:?}"),
    }
}

#[test]
fn warm_ocean_feature_table_includes_coral_and_sea_pickles() {
    let warm_ocean = overworld_features_for_biome(get_layered_biome_by_id(44));

    let coral = warm_ocean
        .iter()
        .find(|feature| matches!(feature.feature, ConfiguredFeature::SimpleRandomSelector(_)))
        .expect("warm ocean coral vegetation feature");
    assert_eq!(coral.step, DecorationStep::VegetalDecoration);
    assert_eq!(
        coral.decorators,
        vec![
            ConfiguredDecorator::count_noise_biased(20, 400.0, 0.0),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ]
    );
    match &coral.feature {
        ConfiguredFeature::SimpleRandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    ConfiguredFeature::coral(CoralShape::Tree),
                    ConfiguredFeature::coral(CoralShape::Claw),
                    ConfiguredFeature::coral(CoralShape::Mushroom),
                ]
            );
        }
        other => panic!("expected simple random coral selector, got {other:?}"),
    }

    let sea_pickle = warm_ocean
        .iter()
        .find(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::SeaPickle(config) if config == CountConfiguration::new(20)
            )
        })
        .expect("warm ocean sea pickle feature");
    assert_eq!(
        sea_pickle.decorators,
        vec![
            ConfiguredDecorator::chance(16),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ]
    );
}

#[test]
fn savanna_feature_table_uses_vanilla_acacia_selector() {
    let savanna = overworld_features_for_biome(get_layered_biome_by_id(35));
    let savanna_plateau = overworld_features_for_biome(get_layered_biome_by_id(36));
    let shattered = overworld_features_for_biome(get_layered_biome_by_id(163));
    assert!(has_savanna_tall_grass_patch_feature(&savanna));
    assert!(has_savanna_tall_grass_patch_feature(&savanna_plateau));
    assert!(has_warm_flower_feature(&savanna));
    assert!(has_warm_flower_feature(&savanna_plateau));
    assert!(has_counted_default_grass_patch_feature(&savanna, 20));
    assert!(has_counted_default_grass_patch_feature(
        &savanna_plateau,
        20
    ));
    let feature = savanna
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("savanna tree selector");

    assert_eq!(
        feature.decorators,
        vec![
            ConfiguredDecorator::count_extra(1, 0.1, 1),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ]
    );
    match &feature.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::acacia()),
                    0.8,
                )]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::oak())
            );
        }
        other => panic!("expected savanna random selector, got {other:?}"),
    }

    let shattered_feature = shattered
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("shattered savanna tree selector");
    assert_eq!(
        shattered_feature.decorators.first(),
        Some(&ConfiguredDecorator::count_extra(2, 0.1, 1))
    );
    assert!(!has_savanna_tall_grass_patch_feature(&shattered));
    assert!(has_default_flower_feature(&shattered));
    assert!(!has_warm_flower_feature(&shattered));
    assert!(has_counted_default_grass_patch_feature(&shattered, 5));
}

#[test]
fn jungle_feature_table_uses_vanilla_jungle_selectors() {
    let jungle = overworld_features_for_biome(get_layered_biome_by_id(21));
    let modified = overworld_features_for_biome(get_layered_biome_by_id(149));
    let edge = overworld_features_for_biome(get_layered_biome_by_id(23));

    let light_bamboo = jungle
        .iter()
        .find(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::Bamboo(BambooConfiguration { probability: 0.0 })
            )
        })
        .expect("jungle light bamboo feature");
    assert_eq!(
        light_bamboo.decorators,
        vec![
            ConfiguredDecorator::count(16),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );

    let jungle_tree = jungle
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("jungle tree selector");
    assert_eq!(
        jungle_tree.decorators,
        tables::tree_threshold_decorators(50, 0.1, 1)
    );
    match &jungle_tree.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::fancy_oak()),
                        0.1,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::jungle_bush()),
                        0.5,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::mega_jungle()),
                        0.33333334,
                    ),
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::jungle())
            );
        }
        other => panic!("expected jungle random selector, got {other:?}"),
    }

    assert!(!modified.iter().any(|feature| {
        matches!(
            feature.feature,
            ConfiguredFeature::Bamboo(BambooConfiguration { probability: 0.0 })
        )
    }));
    assert!(has_jungle_melon_patch(&jungle));
    assert!(has_jungle_vines_feature(&jungle));
    assert!(has_jungle_melon_patch(&modified));
    assert!(has_jungle_vines_feature(&modified));

    let edge_tree = edge
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("jungle edge tree selector");
    assert_eq!(
        edge_tree.decorators,
        tables::tree_threshold_decorators(2, 0.1, 1)
    );
    match &edge_tree.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(config.features.len(), 2);
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::jungle())
            );
        }
        other => panic!("expected jungle edge random selector, got {other:?}"),
    }
    assert!(has_jungle_melon_patch(&edge));
    assert!(has_jungle_vines_feature(&edge));
}

#[test]
fn bamboo_jungle_feature_table_includes_bamboo_and_vegetation_selector() {
    let bamboo_jungle = overworld_features_for_biome(get_layered_biome_by_id(168));

    let bamboo = bamboo_jungle
        .iter()
        .find(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::Bamboo(BambooConfiguration { probability: 0.2 })
            )
        })
        .expect("bamboo jungle bamboo feature");
    assert_eq!(
        bamboo.decorators,
        vec![
            ConfiguredDecorator::count_noise_biased(160, 80.0, 0.3),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::WorldSurface),
        ]
    );

    let vegetation = bamboo_jungle
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("bamboo vegetation selector");
    assert_eq!(
        vegetation.decorators,
        tables::tree_threshold_decorators(30, 0.1, 1)
    );
    match &vegetation.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::fancy_oak()),
                        0.05,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::jungle_bush()),
                        0.15,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::mega_jungle()),
                        0.7,
                    ),
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::random_patch(tables::jungle_grass_patch_config())
            );
        }
        other => panic!("expected bamboo random selector, got {other:?}"),
    }
    assert!(has_jungle_melon_patch(&bamboo_jungle));
    assert!(has_jungle_vines_feature(&bamboo_jungle));
}

#[test]
fn dark_forest_feature_table_uses_vanilla_dark_oak_selector() {
    let dark_forest = overworld_features_for_biome(get_layered_biome_by_id(29));
    let dark_hills = overworld_features_for_biome(get_layered_biome_by_id(157));
    let feature = dark_forest
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("dark forest vegetation feature");

    assert_eq!(
        feature.decorators,
        vec![
            ConfiguredDecorator::dark_oak_tree(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ]
    );
    match &feature.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown()),
                        0.025,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red()),
                        0.05,
                    ),
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
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::oak())
            );
        }
        other => panic!("expected dark forest random selector, got {other:?}"),
    }

    let hills_feature = dark_hills
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("dark forest hills vegetation feature");
    match &hills_feature.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features[0],
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red()),
                    0.025,
                )
            );
            assert_eq!(
                config.features[1],
                WeightedConfiguredFeature::new(
                    ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown()),
                    0.05,
                )
            );
        }
        other => panic!("expected dark forest hills random selector, got {other:?}"),
    }
}

#[test]
fn mushroom_field_feature_table_includes_java_huge_mushroom_selector() {
    let mushroom_fields = overworld_features_for_biome(get_layered_biome_by_id(14));
    let feature = mushroom_fields
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomBooleanSelector(_))
        })
        .expect("mushroom field vegetation feature");

    assert_eq!(
        feature.decorators,
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
        ]
    );
    match &feature.feature {
        ConfiguredFeature::RandomBooleanSelector(config) => {
            assert_eq!(
                *config.feature_true,
                ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red())
            );
            assert_eq!(
                *config.feature_false,
                ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown())
            );
        }
        other => panic!("expected random boolean selector, got {other:?}"),
    }
}

#[test]
fn birch_feature_tables_use_vanilla_normal_and_tall_slots() {
    let birch = overworld_features_for_biome(get_layered_biome_by_id(27));
    let tall_birch = overworld_features_for_biome(get_layered_biome_by_id(155));

    let normal_tree = birch
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::Tree(_))
        })
        .expect("birch tree feature");
    assert_eq!(
        normal_tree.decorators,
        vec![
            ConfiguredDecorator::count_extra(10, 0.1, 1),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ]
    );
    assert_eq!(
        normal_tree.feature,
        ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002())
    );

    let tall_tree = tall_birch
        .iter()
        .find(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
        })
        .expect("tall birch tree selector");
    assert_eq!(tall_tree.decorators, normal_tree.decorators);
    match &tall_tree.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![WeightedConfiguredFeature::new(
                    ConfiguredFeature::tree(TreeConfiguration::super_birch_bees_0002()),
                    0.5,
                )]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002())
            );
        }
        other => panic!("expected tall birch random selector, got {other:?}"),
    }
}
