use super::*;

#[test]
fn basic_tree_places_configured_log_and_leaf_blocks() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(1);
    let feature = ConfiguredFeature::basic_tree(BasicTreeConfiguration::birch());

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
    assert!(chunk.blocks.iter().any(|block_id| *block_id == BIRCH_LOG));
    assert!(
        chunk
            .blocks
            .iter()
            .any(|block_id| *block_id == BIRCH_LEAVES)
    );
}

#[test]
fn dark_oak_tree_places_two_by_two_trunk_and_dark_leaves() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(2);
    let feature = ConfiguredFeature::tree(TreeConfiguration::dark_oak());

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert!(count_blocks(&chunk, DARK_OAK_LOG) >= 4);
    assert!(count_blocks(&chunk, DARK_OAK_LEAVES) > 0);
}

#[test]
fn acacia_tree_places_forking_trunk_and_flat_canopy() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(5);
    let feature = ConfiguredFeature::tree(TreeConfiguration::acacia());

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert!(count_blocks(&chunk, ACACIA_LOG) > 0);
    assert!(count_blocks(&chunk, ACACIA_LEAVES) > 0);
}

#[test]
fn jungle_tree_places_jungle_log_and_leaves() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(6);
    let feature = ConfiguredFeature::tree(TreeConfiguration::jungle());

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert!(count_blocks(&chunk, JUNGLE_LOG) > 0);
    assert!(count_blocks(&chunk, JUNGLE_LEAVES) > 0);
}

#[test]
fn mega_jungle_tree_uses_java_two_by_two_trunk_branches_and_foliage() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(6);
    let config = TreeConfiguration::mega_jungle();
    let feature = ConfiguredFeature::tree(config);
    let base = BlockPos::new(6, 3, 6);

    assert_eq!(
        config.trunk_placer,
        TrunkPlacerConfiguration::mega_jungle(10, 2, 19)
    );
    assert_eq!(
        config.foliage_placer,
        FoliagePlacerConfiguration::MegaJungle {
            radius: IntProvider::constant(2),
            offset: IntProvider::constant(0),
            height: 2,
        }
    );
    assert!(feature.place(&mut chunk, &mut random, base));
    assert!(has_two_by_two_log_square_at(&chunk, base, JUNGLE_LOG));
    assert!(count_logs_outside_two_by_two_column(&chunk, base, JUNGLE_LOG) >= 5);
    assert!(count_blocks(&chunk, JUNGLE_LEAVES) >= 120);
}

#[test]
fn jungle_bush_uses_java_bush_foliage_shape() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(6);
    let config = TreeConfiguration::jungle_bush();
    let feature = ConfiguredFeature::tree(config);
    let base = BlockPos::new(8, 3, 8);

    assert_eq!(config.log, JUNGLE_LOG);
    assert_eq!(config.leaves, OAK_LEAVES);
    assert_eq!(
        config.trunk_placer,
        TrunkPlacerConfiguration::straight(1, 0, 0)
    );
    assert_eq!(
        config.foliage_placer,
        FoliagePlacerConfiguration::Bush {
            radius: IntProvider::constant(2),
            offset: IntProvider::constant(1),
            height: 2,
        }
    );

    assert!(feature.place(&mut chunk, &mut random, base));
    assert_eq!(chunk.get_block_at_y(base.x, base.y, base.z), JUNGLE_LOG);
    assert_eq!(count_blocks(&chunk, JUNGLE_LOG), 1);
    assert_eq!(count_blocks_at_y(&chunk, base.y + 2, OAK_LEAVES), 1);
    assert!(count_blocks_at_y(&chunk, base.y + 1, OAK_LEAVES) >= 5);
    assert!(count_blocks_at_y(&chunk, base.y, OAK_LEAVES) >= 20);
    assert_eq!(chunk.get_block_at_y(base.x + 2, base.y, base.z), OAK_LEAVES);
    assert_eq!(chunk.get_block_at_y(base.x, base.y, base.z + 2), OAK_LEAVES);
}

#[test]
fn jungle_tree_decorators_place_cocoa_and_face_vines() {
    let cocoa_states = [
        COCOA_AGE0_NORTH,
        COCOA_AGE0_EAST,
        COCOA_AGE0_SOUTH,
        COCOA_AGE0_WEST,
        COCOA_AGE1_NORTH,
        COCOA_AGE1_EAST,
        COCOA_AGE1_SOUTH,
        COCOA_AGE1_WEST,
        COCOA_AGE2_NORTH,
        COCOA_AGE2_EAST,
        COCOA_AGE2_SOUTH,
        COCOA_AGE2_WEST,
    ];
    let vine_states = [VINE_UP, VINE_NORTH, VINE_EAST, VINE_SOUTH, VINE_WEST];
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(6);
    let config = TreeConfiguration::jungle().with_cocoa_probability(1.0);
    let feature = ConfiguredFeature::tree(config);

    assert_eq!(TreeConfiguration::jungle().cocoa_probability, Some(0.2));
    assert!(TreeConfiguration::jungle().trunk_vines);
    assert!(TreeConfiguration::jungle().leaf_vines);
    assert!(TreeConfiguration::mega_jungle().trunk_vines);
    assert!(TreeConfiguration::mega_jungle().leaf_vines);
    assert_eq!(TreeConfiguration::mega_jungle().cocoa_probability, None);
    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert!(count_any_blocks(&chunk, &cocoa_states) > 0);
    assert!(count_any_blocks(&chunk, &vine_states) > 0);
}

#[test]
fn bamboo_feature_places_java_height_column() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(7);
    let feature = ConfiguredFeature::bamboo(BambooConfiguration::new(1.0));

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert!(count_blocks(&chunk, BAMBOO) >= 3);
    assert_eq!(count_blocks(&chunk, BAMBOO_TOP_SMALL), 1);
    assert_eq!(count_blocks(&chunk, BAMBOO_TOP_LARGE), 1);
    assert_eq!(count_blocks(&chunk, BAMBOO_FINAL_LARGE), 1);
}

#[test]
fn melon_patch_places_on_grass_and_honors_can_replace() {
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: MELON,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: true,
        double_plant: false,
        column_height: None,
        need_water: false,
        place_on: &[GRASS_BLOCK],
    });

    let mut air_chunk = flat_grass_chunk();
    let mut air_random = WorldgenRandom::new(8);
    assert!(feature.place(&mut air_chunk, &mut air_random, BlockPos::new(8, 3, 8)));
    assert_eq!(air_chunk.get_block_at_y(8, 3, 8), MELON);

    let mut grass_chunk = flat_grass_chunk();
    grass_chunk.set_block_at_y(8, 3, 8, GRASS);
    let mut grass_random = WorldgenRandom::new(8);
    assert!(feature.place(&mut grass_chunk, &mut grass_random, BlockPos::new(8, 3, 8)));
    assert_eq!(grass_chunk.get_block_at_y(8, 3, 8), MELON);
}

#[test]
fn vines_feature_places_against_solid_neighbor_only() {
    let feature = ConfiguredFeature::vines();

    let mut unsupported = flat_grass_chunk();
    let mut unsupported_random = WorldgenRandom::new(9);
    assert!(!feature.place(
        &mut unsupported,
        &mut unsupported_random,
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(unsupported.get_block_at_y(8, 3, 8), AIR);

    let mut supported = flat_grass_chunk();
    supported.set_block_at_y(9, 3, 8, STONE);
    let mut supported_random = WorldgenRandom::new(9);
    assert!(feature.place(
        &mut supported,
        &mut supported_random,
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(supported.get_block_at_y(8, 3, 8), VINE_EAST);

    let mut supported_above = flat_grass_chunk();
    supported_above.set_block_at_y(8, 4, 8, STONE);
    let mut supported_above_random = WorldgenRandom::new(9);
    assert!(feature.place(
        &mut supported_above,
        &mut supported_above_random,
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(supported_above.get_block_at_y(8, 3, 8), VINE_UP);

    let mut occupied = flat_grass_chunk();
    occupied.set_block_at_y(8, 3, 8, GRASS);
    occupied.set_block_at_y(9, 3, 8, STONE);
    let mut occupied_random = WorldgenRandom::new(9);
    assert!(!feature.place(&mut occupied, &mut occupied_random, BlockPos::new(8, 3, 8)));
    assert_eq!(occupied.get_block_at_y(8, 3, 8), GRASS);
}

#[test]
fn huge_mushrooms_place_cap_and_stem_blocks() {
    let mut brown_chunk = flat_grass_chunk();
    let mut brown_random = WorldgenRandom::new(3);
    let brown = ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown());

    assert!(brown.place(&mut brown_chunk, &mut brown_random, BlockPos::new(8, 3, 8)));
    assert!(count_blocks(&brown_chunk, BROWN_MUSHROOM_BLOCK) > 0);
    assert!(count_blocks(&brown_chunk, MUSHROOM_STEM) > 0);

    let mut red_chunk = flat_grass_chunk();
    let mut red_random = WorldgenRandom::new(4);
    let red = ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red());

    assert!(red.place(&mut red_chunk, &mut red_random, BlockPos::new(8, 3, 8)));
    assert!(count_blocks(&red_chunk, RED_MUSHROOM_BLOCK) > 0);
    assert!(count_blocks(&red_chunk, MUSHROOM_STEM) > 0);
}

#[test]
fn feature_region_allows_neighbor_tree_to_spill_into_center_chunk() {
    let mut region = FeatureRegion::with_radii(
        -1,
        0,
        1,
        1,
        vec![flat_grass_chunk_at(-1, 0), flat_grass_chunk_at(0, 0)],
    );
    let mut random = WorldgenRandom::new(1);
    let feature = ConfiguredFeature::basic_tree(BasicTreeConfiguration::oak());

    assert!(feature.place(&mut region, &mut random, BlockPos::new(-1, 0, 8)));

    let center = region.chunk(0, 0).expect("center chunk exists");
    assert!(center.blocks.iter().any(|block_id| *block_id == OAK_LEAVES));
    assert_eq!(region.metrics().blocked_block_writes, 0);
}

#[test]
fn feature_region_blocks_writes_beyond_cutoff() {
    let mut region = FeatureRegion::with_radii(
        0,
        0,
        2,
        1,
        vec![flat_grass_chunk_at(0, 0), flat_grass_chunk_at(2, 0)],
    );

    assert!(!region.set_block_world(BlockPos::new(32, 3, 0), POPPY));
    assert_eq!(
        region.metrics(),
        FeatureRegionMetrics {
            block_write_attempts: 1,
            blocked_block_writes: 1,
            ..FeatureRegionMetrics::default()
        }
    );
    assert_eq!(region.chunk(2, 0).unwrap().get_block_at_y(0, 3, 0), AIR);
}

#[test]
#[should_panic(expected = "outside dependency window")]
fn feature_region_rejects_reads_outside_dependency_window() {
    let mut region = FeatureRegion::with_radii(0, 0, 1, 1, vec![flat_grass_chunk()]);

    let _ = region.block_at_world(BlockPos::new(32, 3, 0));
}
