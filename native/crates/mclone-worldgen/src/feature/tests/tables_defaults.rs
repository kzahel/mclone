use super::*;

#[test]
fn biome_feature_tables_start_with_default_lakes() {
    let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
    let desert = overworld_features_for_biome(get_layered_biome_by_id(2));

    assert_eq!(plains[0].step, DecorationStep::Lakes);
    assert_eq!(
        plains[0].feature,
        ConfiguredFeature::lake(LakeConfiguration::new(WATER))
    );
    assert_eq!(
        plains[0].decorators,
        vec![
            ConfiguredDecorator::chance(4),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::uniform(
                VerticalAnchor::bottom(),
                VerticalAnchor::top(),
            )),
        ]
    );
    assert_eq!(plains[1].step, DecorationStep::Lakes);
    assert_eq!(
        plains[1].feature,
        ConfiguredFeature::lake(LakeConfiguration::new(LAVA))
    );
    assert_eq!(
        plains[1].decorators,
        vec![
            ConfiguredDecorator::chance(8),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                VerticalAnchor::bottom(),
                VerticalAnchor::top(),
                8,
            )),
            ConfiguredDecorator::lava_lake(80),
        ]
    );

    assert_eq!(desert[0].step, DecorationStep::Lakes);
    assert_eq!(
        desert[0].feature,
        ConfiguredFeature::lake(LakeConfiguration::new(LAVA))
    );
}

#[test]
fn biome_feature_tables_start_with_default_underground_variety() {
    let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
    let expected = [
        (DIRT, 33, 10, VerticalAnchor::top()),
        (GRAVEL, 33, 8, VerticalAnchor::top()),
        (GRANITE, 33, 10, VerticalAnchor::absolute(79)),
        (DIORITE, 33, 10, VerticalAnchor::absolute(79)),
        (ANDESITE, 33, 10, VerticalAnchor::absolute(79)),
        (TUFF, 33, 1, VerticalAnchor::absolute(16)),
        (DEEPSLATE, 64, 2, VerticalAnchor::absolute(16)),
    ];

    for (feature, (expected_block, expected_size, expected_count, max_y)) in
        plains.iter().skip(2).zip(expected)
    {
        assert_eq!(feature.step, DecorationStep::UndergroundOres);
        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::count(expected_count),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(HeightProvider::uniform(
                    VerticalAnchor::absolute(0),
                    max_y,
                )),
            ]
        );
        match &feature.feature {
            ConfiguredFeature::Ore(config) => {
                assert_eq!(config.size, expected_size);
                assert_eq!(
                    config.target_states,
                    vec![OreTargetBlockState::new(
                        OreTarget::NaturalStone,
                        expected_block,
                    )]
                );
            }
            other => panic!("expected ore feature, got {other:?}"),
        }
    }
}

#[test]
fn biome_feature_tables_include_default_ores_after_variety() {
    let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
    let expected = [
        (
            COAL_ORE,
            DEEPSLATE_COAL_ORE,
            17,
            Some(20),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(127)),
        ),
        (
            IRON_ORE,
            DEEPSLATE_IRON_ORE,
            9,
            Some(20),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(63)),
        ),
        (
            GOLD_ORE,
            DEEPSLATE_GOLD_ORE,
            9,
            Some(2),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(31)),
        ),
        (
            REDSTONE_ORE,
            DEEPSLATE_REDSTONE_ORE,
            8,
            Some(8),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
        ),
        (
            DIAMOND_ORE,
            DEEPSLATE_DIAMOND_ORE,
            8,
            None,
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
        ),
        (
            LAPIS_ORE,
            DEEPSLATE_LAPIS_ORE,
            7,
            None,
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(30), 0),
        ),
        (
            COPPER_ORE,
            DEEPSLATE_COPPER_ORE,
            10,
            Some(6),
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(96), 0),
        ),
    ];

    for (feature, (stone_ore, deepslate_ore, size, count, height)) in
        plains.iter().skip(11).zip(expected)
    {
        assert_eq!(feature.step, DecorationStep::UndergroundOres);
        let mut expected_decorators = Vec::new();
        if let Some(count) = count {
            expected_decorators.push(ConfiguredDecorator::count(count));
        }
        expected_decorators.push(ConfiguredDecorator::square());
        expected_decorators.push(ConfiguredDecorator::range(height));
        assert_eq!(feature.decorators, expected_decorators);

        match &feature.feature {
            ConfiguredFeature::Ore(config) => {
                assert_eq!(config.size, size);
                assert_eq!(
                    config.target_states,
                    vec![
                        OreTargetBlockState::new(OreTarget::StoneOreReplaceables, stone_ore),
                        OreTargetBlockState::new(
                            OreTarget::DeepslateOreReplaceables,
                            deepslate_ore,
                        ),
                    ]
                );
            }
            other => panic!("expected ore feature, got {other:?}"),
        }
    }
}

#[test]
fn biome_feature_tables_include_default_soft_disks_after_ores() {
    let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
    let expected = [
        (
            SAND,
            IntProvider::uniform(2, 6),
            2,
            &[DIRT, GRASS_BLOCK][..],
            vec![
                ConfiguredDecorator::count(3),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
            ],
        ),
        (
            CLAY,
            IntProvider::uniform(2, 3),
            1,
            &[DIRT, CLAY][..],
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
            ],
        ),
        (
            GRAVEL,
            IntProvider::uniform(2, 5),
            2,
            &[DIRT, GRASS_BLOCK][..],
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
            ],
        ),
    ];

    for (feature, (state, radius, half_height, targets, decorators)) in
        plains.iter().skip(18).zip(expected)
    {
        assert_eq!(feature.step, DecorationStep::UndergroundOres);
        assert_eq!(feature.decorators, decorators);
        match &feature.feature {
            ConfiguredFeature::Disk(config) => {
                assert_eq!(config.state, state);
                assert_eq!(config.radius, radius);
                assert_eq!(config.half_height, half_height);
                assert_eq!(config.targets, targets);
            }
            other => panic!("expected disk feature, got {other:?}"),
        }
    }
}

#[test]
fn swamp_feature_table_uses_clay_only_soft_disk() {
    let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
    let disk_features: Vec<_> = swamp
        .iter()
        .filter_map(|feature| match &feature.feature {
            ConfiguredFeature::Disk(config) => Some((feature, config)),
            _ => None,
        })
        .collect();

    assert_eq!(disk_features.len(), 1);
    let (feature, config) = disk_features[0];
    assert_eq!(feature.step, DecorationStep::UndergroundOres);
    assert_eq!(config.state, CLAY);
    assert_eq!(config.radius, IntProvider::uniform(2, 3));
    assert_eq!(config.half_height, 1);
    assert_eq!(config.targets, &[DIRT, CLAY]);
    assert_eq!(
        feature.decorators,
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
        ]
    );
}

#[test]
fn biome_feature_tables_end_with_default_freeze_top_layer() {
    let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
    let feature = plains.last().expect("plains has features");

    assert_eq!(feature.step, DecorationStep::TopLayerModification);
    assert_eq!(feature.feature, ConfiguredFeature::freeze_top_layer());
    assert!(feature.decorators.is_empty());
}

#[test]
fn biome_overworld_decoration_reports_added_blocks() {
    let mut chunk = flat_grass_chunk();
    let report = apply_overworld_biome_features(12_345, get_layered_biome_by_id(4), &mut chunk);

    assert_eq!(report.biome_key, "minecraft:forest");
    assert_eq!(report.attempted_features, 33);
    assert!(report.placed_features > 0);
    assert!(report.added_non_air_blocks > 0);
}
