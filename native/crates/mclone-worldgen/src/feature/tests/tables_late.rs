use super::*;

#[test]
fn forest_feature_table_uses_vanilla_birch_other_slot() {
    let forest = overworld_features_for_biome(get_layered_biome_by_id(4));
    let vegetal_features = forest
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();

    assert_eq!(vegetal_features.len(), 11);
    assert_eq!(
        vegetal_features[0].decorators,
        vec![
            ConfiguredDecorator::count(5),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
            ConfiguredDecorator::Count(CountConfiguration::from_provider(
                IntProvider::clamped_uniform(-3, 1, 0, 1),
            )),
        ]
    );
    assert!(matches!(
        vegetal_features[0].feature,
        ConfiguredFeature::SimpleRandomSelector(_)
    ));
    assert_eq!(
        vegetal_features[1].feature,
        ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld())
    );
    assert_eq!(
        vegetal_features[2].decorators,
        vec![
            ConfiguredDecorator::count_extra(10, 0.1, 1),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ]
    );
    match &vegetal_features[2].feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002()),
                        0.2,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::fancy_oak_bees_0002()),
                        0.1,
                    ),
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::oak_bees_0002())
            );
        }
        other => panic!("expected forest birch_other random selector, got {other:?}"),
    }
    assert_eq!(
        vegetal_features[3].decorators,
        vec![
            ConfiguredDecorator::count(2),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ]
    );
    assert_eq!(
        vegetal_features[4].decorators,
        vec![
            ConfiguredDecorator::count(2),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );
    assert_eq!(
        vegetal_features[7].decorators.first(),
        Some(&ConfiguredDecorator::count(10))
    );
    assert!(matches!(
        vegetal_features[7].feature,
        ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
            state: SUGAR_CANE,
            ..
        })
    ));
    assert_eq!(
        vegetal_features[8].decorators,
        vec![
            ConfiguredDecorator::chance(32),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );
    assert!(matches!(
        vegetal_features[8].feature,
        ConfiguredFeature::RandomPatch(RandomPatchConfiguration { state: PUMPKIN, .. })
    ));
}

#[test]
fn sunflower_plains_feature_table_includes_java_sunflower_patch() {
    let sunflower_plains = overworld_features_for_biome(get_layered_biome_by_id(129));
    let vegetal_features = sunflower_plains
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();

    assert_eq!(
        vegetal_features[0].decorators,
        vec![
            ConfiguredDecorator::count(10),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ]
    );
    match &vegetal_features[0].feature {
        ConfiguredFeature::RandomPatch(config) => {
            assert_eq!(config.state, SUNFLOWER_LOWER);
            assert_eq!(config.weighted_states, &[]);
            assert_eq!(config.state_provider, RandomPatchStateProvider::Simple);
            assert_eq!(config.tries, 64);
            assert_eq!(config.xspread, 7);
            assert_eq!(config.yspread, 3);
            assert_eq!(config.zspread, 7);
            assert!(!config.project);
            assert!(!config.can_replace);
            assert!(config.double_plant);
            assert_eq!(config.column_height, None);
            assert!(!config.need_water);
            assert!(config.place_on.is_empty());
        }
        other => panic!("expected sunflower random patch, got {other:?}"),
    }
}

#[test]
fn flower_forest_feature_table_includes_java_flower_provider() {
    let flower_forest = overworld_features_for_biome(get_layered_biome_by_id(132));
    let vegetal_features = flower_forest
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();

    assert_eq!(
        vegetal_features[0].decorators,
        vec![
            ConfiguredDecorator::count(5),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
            ConfiguredDecorator::Count(CountConfiguration::from_provider(
                IntProvider::clamped_uniform(-1, 3, 0, 3),
            )),
        ]
    );
    match &vegetal_features[0].feature {
        ConfiguredFeature::SimpleRandomSelector(config) => {
            let double_patch = |state| {
                ConfiguredFeature::random_patch(RandomPatchConfiguration {
                    state,
                    weighted_states: &[],
                    state_provider: RandomPatchStateProvider::Simple,
                    tries: 64,
                    xspread: 7,
                    yspread: 3,
                    zspread: 7,
                    project: false,
                    can_replace: false,
                    double_plant: true,
                    column_height: None,
                    need_water: false,
                    place_on: &[],
                })
            };
            assert_eq!(
                config.features,
                vec![
                    double_patch(LILAC_LOWER),
                    double_patch(ROSE_BUSH_LOWER),
                    double_patch(PEONY_LOWER),
                    ConfiguredFeature::flower(RandomPatchConfiguration::new(LILY_OF_THE_VALLEY,)),
                ]
            );
        }
        other => panic!("expected common forest flower simple random selector, got {other:?}"),
    }

    match &vegetal_features[2].feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002()),
                        0.2,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::fancy_oak_bees_0002()),
                        0.1,
                    ),
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::oak_bees_0002())
            );
        }
        other => panic!("expected flower forest tree random selector, got {other:?}"),
    }
    assert_eq!(
        vegetal_features[2].decorators,
        vec![
            ConfiguredDecorator::count_extra(6, 0.1, 1),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ]
    );

    let dense_flowers = vegetal_features[3];
    assert_eq!(
        dense_flowers.decorators,
        vec![
            ConfiguredDecorator::count(100),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ]
    );
    match &dense_flowers.feature {
        ConfiguredFeature::Flower(config) => {
            assert_eq!(
                config.state_provider,
                RandomPatchStateProvider::ForestFlower
            );
            assert_eq!(config.tries, 64);
            assert_eq!(config.xspread, 7);
            assert_eq!(config.yspread, 3);
            assert_eq!(config.zspread, 7);
            assert!(config.project);
            assert!(!config.can_replace);
            assert!(config.place_on.is_empty());
        }
        other => panic!("expected flower forest provider feature, got {other:?}"),
    }
}

#[test]
fn taiga_feature_table_uses_vanilla_taiga_vegetation_selector() {
    let taiga = overworld_features_for_biome(get_layered_biome_by_id(133));
    let vegetal_features = taiga
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();

    assert!(matches!(
        vegetal_features[0].feature,
        ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            double_plant: true,
            ..
        })
    ));
    assert_eq!(
        vegetal_features[1].feature,
        ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld())
    );
    let feature = vegetal_features[2];

    assert_eq!(feature.step, DecorationStep::VegetalDecoration);
    assert_eq!(
        feature.decorators,
        vec![
            ConfiguredDecorator::count_extra(10, 0.1, 1),
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
                    ConfiguredFeature::tree(TreeConfiguration::pine()),
                    0.33333334,
                )]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::spruce())
            );
        }
        other => panic!("expected random selector, got {other:?}"),
    }

    assert_eq!(
        vegetal_features[3].decorators,
        vec![
            ConfiguredDecorator::count(2),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ]
    );
    match &vegetal_features[3].feature {
        ConfiguredFeature::Flower(config) => {
            assert_eq!(
                config.weighted_states,
                tables::DEFAULT_FLOWER_STATES.as_slice()
            );
            assert_eq!(config.tries, 64);
        }
        other => panic!("expected default flower feature, got {other:?}"),
    }
    assert_eq!(
        vegetal_features[4].decorators,
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );
    match &vegetal_features[4].feature {
        ConfiguredFeature::RandomPatch(config) => {
            assert_eq!(
                config.weighted_states,
                tables::TAIGA_GRASS_STATES.as_slice()
            );
            assert_eq!(config.tries, 32);
        }
        other => panic!("expected taiga grass random patch, got {other:?}"),
    }
    assert_eq!(
        vegetal_features[9].decorators.first(),
        Some(&ConfiguredDecorator::count(10))
    );
    assert!(matches!(
        vegetal_features[9].feature,
        ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
            state: SUGAR_CANE,
            ..
        })
    ));
    assert_eq!(
        vegetal_features[10].decorators,
        vec![
            ConfiguredDecorator::chance(32),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );
    assert!(matches!(
        vegetal_features[10].feature,
        ConfiguredFeature::RandomPatch(RandomPatchConfiguration { state: PUMPKIN, .. })
    ));

    let water_spring =
        vegetal_features[test_support::JAVA_TAIGA_WATER_SPRING_FEATURE_INDEX as usize];
    assert_eq!(
        water_spring.feature,
        ConfiguredFeature::spring(SpringConfiguration::water())
    );
    assert_eq!(
        water_spring.decorators,
        vec![
            ConfiguredDecorator::count(50),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                VerticalAnchor::bottom(),
                VerticalAnchor::below_top(8),
                8,
            )),
        ]
    );

    let lava_spring = vegetal_features[test_support::JAVA_TAIGA_LAVA_SPRING_FEATURE_INDEX as usize];
    assert_eq!(
        lava_spring.feature,
        ConfiguredFeature::spring(SpringConfiguration::lava())
    );
    assert_eq!(
        lava_spring.decorators,
        vec![
            ConfiguredDecorator::count(20),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::very_biased_to_bottom(
                VerticalAnchor::bottom(),
                VerticalAnchor::below_top(8),
                8,
            )),
        ]
    );
    let berry_patch = vegetal_features[13];
    assert_eq!(
        berry_patch.decorators,
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );
    match &berry_patch.feature {
        ConfiguredFeature::RandomPatch(config) => {
            assert_eq!(config.state, SWEET_BERRY_BUSH);
            assert_eq!(config.weighted_states, &[]);
            assert_eq!(config.tries, 64);
            assert_eq!(config.xspread, 7);
            assert_eq!(config.yspread, 3);
            assert_eq!(config.zspread, 7);
            assert!(!config.project);
            assert!(!config.can_replace);
            assert!(!config.double_plant);
            assert_eq!(config.column_height, None);
            assert!(!config.need_water);
            assert_eq!(config.place_on, &[GRASS_BLOCK]);
        }
        other => panic!("expected sweet berry random patch, got {other:?}"),
    }

    let snowy_taiga = overworld_features_for_biome(get_layered_biome_by_id(30));
    let snowy_taiga_vegetal_features = snowy_taiga
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();
    let snowy_berry_patch = snowy_taiga_vegetal_features
        .iter()
        .find(|feature| {
            matches!(
                &feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: SWEET_BERRY_BUSH,
                    ..
                })
            )
        })
        .expect("snowy taiga should include Java berry bush patch");
    assert_eq!(
        snowy_berry_patch.decorators,
        vec![
            ConfiguredDecorator::chance(12),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
        ]
    );

    let snowy_tundra = overworld_features_for_biome(get_layered_biome_by_id(12));
    assert!(
        !snowy_tundra.iter().any(|feature| {
            matches!(
                &feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: SWEET_BERRY_BUSH,
                    ..
                })
            )
        }),
        "snowy tundra should not inherit snowy taiga berry bushes"
    );
}

#[test]
fn giant_taiga_feature_table_uses_vanilla_mega_tree_selectors() {
    let giant_tree_taiga = overworld_features_for_biome(get_layered_biome_by_id(32));
    let giant_spruce_taiga = overworld_features_for_biome(get_layered_biome_by_id(160));

    let giant_tree_vegetal_features = giant_tree_taiga
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();
    let giant_spruce_vegetal_features = giant_spruce_taiga
        .iter()
        .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
        .collect::<Vec<_>>();

    let giant_tree_feature = giant_tree_vegetal_features[2];
    assert_eq!(
        giant_tree_feature.decorators,
        tables::tree_threshold_decorators(10, 0.1, 1)
    );
    match &giant_tree_feature.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::mega_spruce()),
                        0.025641026,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::mega_pine()),
                        0.30769232,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::pine()),
                        0.33333334,
                    ),
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::spruce())
            );
        }
        other => panic!("expected giant tree taiga random selector, got {other:?}"),
    }

    let giant_spruce_feature = giant_spruce_vegetal_features[2];
    assert_eq!(
        giant_spruce_feature.decorators,
        tables::tree_threshold_decorators(10, 0.1, 1)
    );
    match &giant_spruce_feature.feature {
        ConfiguredFeature::RandomSelector(config) => {
            assert_eq!(
                config.features,
                vec![
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::mega_spruce()),
                        0.33333334,
                    ),
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::pine()),
                        0.33333334,
                    ),
                ]
            );
            assert_eq!(
                *config.default_feature,
                ConfiguredFeature::tree(TreeConfiguration::spruce())
            );
        }
        other => panic!("expected giant spruce taiga random selector, got {other:?}"),
    }
}
