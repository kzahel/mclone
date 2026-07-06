use super::*;

pub(super) fn assert_swamp_mushroom_patch(
    swamp: &[PlacedFeature],
    state: RawBlockId,
    expected_decorators: Vec<ConfiguredDecorator>,
) {
    let feature = swamp
        .iter()
        .find(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: patch_state,
                    ..
                }) if patch_state == state
            )
        })
        .expect("swamp mushroom feature");

    assert_eq!(feature.decorators, expected_decorators);
    match &feature.feature {
        ConfiguredFeature::RandomPatch(config) => {
            assert_eq!(
                *config,
                RandomPatchConfiguration {
                    state,
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
                    place_on: &[],
                }
            );
        }
        other => panic!("expected swamp mushroom random patch, got {other:?}"),
    }
}

pub(super) fn has_random_patch(features: &[PlacedFeature], state: RawBlockId, count: i32) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators.first() == Some(&ConfiguredDecorator::count(count))
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: patch_state,
                    ..
                }) if patch_state == state
            )
    })
}

pub(super) fn has_pumpkin_patch(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::chance(32),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: PUMPKIN,
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
                })
            )
    })
}

pub(super) fn assert_badlands_common_features(features: &[PlacedFeature]) {
    assert!(has_default_grass_patch_feature(features));
    assert!(has_badlands_dead_bush_patch_feature(features));
    assert!(has_normal_mushroom_patch(features, BROWN_MUSHROOM, 4));
    assert!(has_normal_mushroom_patch(features, RED_MUSHROOM, 8));
    assert!(has_random_patch(features, SUGAR_CANE, 13));
    assert!(has_pumpkin_patch(features));
    assert!(has_random_patch(features, CACTUS, 5));
    assert!(has_default_water_spring(features));
    assert!(has_default_lava_spring(features));
}

pub(super) fn has_badlands_tree_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators == tables::tree_threshold_decorators(5, 0.1, 1)
            && feature.feature == ConfiguredFeature::tree(TreeConfiguration::oak())
    })
}

pub(super) fn has_badlands_dead_bush_patch_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::count(20),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: DEAD_BUSH,
                    weighted_states: &[],
                    state_provider: RandomPatchStateProvider::Simple,
                    tries: 16,
                    xspread: 7,
                    yspread: 3,
                    zspread: 7,
                    project: true,
                    can_replace: false,
                    double_plant: false,
                    column_height: None,
                    need_water: false,
                    place_on,
                }) if place_on == &[SAND, RED_SAND, TERRACOTTA, DIRT, GRASS_BLOCK, PODZOL]
            )
    })
}

pub(super) fn has_jungle_melon_patch(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: MELON,
                    weighted_states: &[],
                    state_provider: RandomPatchStateProvider::Simple,
                    tries: 64,
                    xspread: 7,
                    yspread: 3,
                    zspread: 7,
                    project: false,
                    can_replace: true,
                    double_plant: false,
                    column_height: None,
                    need_water: false,
                    place_on: &[GRASS_BLOCK],
                })
            )
    })
}

pub(super) fn has_jungle_vines_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.feature == ConfiguredFeature::vines()
            && feature.decorators
                == vec![
                    ConfiguredDecorator::count(50),
                    ConfiguredDecorator::square(),
                ]
    })
}

pub(super) fn has_default_water_spring(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.feature == ConfiguredFeature::spring(SpringConfiguration::water())
            && feature.decorators
                == vec![
                    ConfiguredDecorator::count(50),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                        VerticalAnchor::bottom(),
                        VerticalAnchor::below_top(8),
                        8,
                    )),
                ]
    })
}

pub(super) fn has_default_lava_spring(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.feature == ConfiguredFeature::spring(SpringConfiguration::lava())
            && feature.decorators
                == vec![
                    ConfiguredDecorator::count(20),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::range(HeightProvider::very_biased_to_bottom(
                        VerticalAnchor::bottom(),
                        VerticalAnchor::below_top(8),
                        8,
                    )),
                ]
    })
}

pub(super) fn has_normal_mushroom_patch(
    features: &[PlacedFeature],
    state: RawBlockId,
    rarity: i32,
) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::chance(rarity),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: patch_state,
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
                    place_on: &[],
                }) if patch_state == state
            )
    })
}

pub(super) fn has_taiga_mushroom_patch(
    features: &[PlacedFeature],
    state: RawBlockId,
    rarity: i32,
    count: Option<i32>,
    heightmap_double_square: bool,
) -> bool {
    let mut decorators = Vec::new();
    if let Some(count) = count {
        decorators.push(ConfiguredDecorator::count(count));
    }
    decorators.push(ConfiguredDecorator::square());
    decorators.push(if heightmap_double_square {
        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking)
    } else {
        ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking)
    });
    decorators.push(ConfiguredDecorator::chance(rarity));

    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators == decorators
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: patch_state,
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
                    place_on: &[],
                }) if patch_state == state
            )
    })
}

pub(super) fn has_forest_rock_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::LocalModifications
            && feature.decorators
                == vec![
                    ConfiguredDecorator::Count(CountConfiguration::from_provider(
                        IntProvider::uniform(0, 2),
                    )),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::BlockBlob(BlockStateConfiguration {
                    state: MOSSY_COBBLESTONE,
                })
            )
    })
}

pub(super) fn has_water_tree_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators == tables::tree_threshold_decorators(0, 0.1, 1)
            && matches!(
                &feature.feature,
                ConfiguredFeature::RandomSelector(RandomFeatureConfiguration {
                    features: weighted,
                    default_feature,
                }) if weighted.len() == 1
                    && weighted[0].chance == 0.1
                    && *weighted[0].feature == ConfiguredFeature::tree(TreeConfiguration::fancy_oak())
                    && **default_feature == ConfiguredFeature::tree(TreeConfiguration::oak())
            )
    })
}

pub(super) fn has_default_flower_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && matches!(
                &feature.feature,
                ConfiguredFeature::Flower(RandomPatchConfiguration {
                    state: POPPY,
                    weighted_states,
                    state_provider: RandomPatchStateProvider::Weighted,
                    tries: 64,
                    ..
                }) if weighted_states == &tables::DEFAULT_FLOWER_STATES
            )
    })
}

pub(super) fn has_warm_flower_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators.first() == Some(&ConfiguredDecorator::count(4))
            && matches!(
                &feature.feature,
                ConfiguredFeature::Flower(RandomPatchConfiguration {
                    state: POPPY,
                    weighted_states,
                    state_provider: RandomPatchStateProvider::Weighted,
                    tries: 64,
                    ..
                }) if weighted_states == &tables::DEFAULT_FLOWER_STATES
            )
    })
}

pub(super) fn has_default_grass_patch_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: GRASS,
                    tries: 32,
                    ..
                })
            )
    })
}

pub(super) fn has_counted_default_grass_patch_feature(
    features: &[PlacedFeature],
    count: i32,
) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::count(count),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: GRASS,
                    tries: 32,
                    ..
                })
            )
    })
}

pub(super) fn has_savanna_tall_grass_patch_feature(features: &[PlacedFeature]) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators
                == vec![
                    ConfiguredDecorator::count(7),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                    ConfiguredDecorator::spread_32_above(),
                ]
            && matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: TALL_GRASS_LOWER,
                    tries: 64,
                    double_plant: true,
                    ..
                })
            )
    })
}

pub(super) fn has_seagrass_feature(
    features: &[PlacedFeature],
    count: i32,
    tall_probability: f32,
) -> bool {
    features.iter().any(|feature| {
        feature.step == DecorationStep::VegetalDecoration
            && feature.decorators.first() == Some(&ConfiguredDecorator::count(count))
            && matches!(
                feature.feature,
                ConfiguredFeature::Seagrass(SeagrassConfiguration {
                    tall_probability: actual,
                }) if actual == tall_probability
            )
    })
}
