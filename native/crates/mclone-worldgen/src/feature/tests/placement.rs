use super::*;

#[test]
fn simple_block_feature_places_on_grass_surface() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(0);
    let feature = ConfiguredFeature::simple_block(
        SimpleBlockConfiguration::new(DANDELION)
            .place_on(&[GRASS_BLOCK])
            .place_in(&[AIR]),
    );

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(4, 3, 5)));
    assert_eq!(chunk.get_block_at_y(4, 3, 5), DANDELION);
}

#[test]
fn block_blob_feature_places_mossy_cobble_on_surface_support() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            for y in 0..5 {
                chunk.set_block_at_y(x, y, z, STONE);
            }
            chunk.set_block_at_y(x, 5, z, GRASS_BLOCK);
        }
    }
    let mut random = WorldgenRandom::new(0);
    let feature = ConfiguredFeature::block_blob(BlockStateConfiguration::new(MOSSY_COBBLESTONE));

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 10, 8)));
    assert!(count_blocks(&chunk, MOSSY_COBBLESTONE) > 0);
}

#[test]
fn random_boolean_selector_uses_java_next_boolean_branching() {
    let feature =
        ConfiguredFeature::random_boolean_selector(RandomBooleanFeatureConfiguration::new(
            ConfiguredFeature::simple_block(SimpleBlockConfiguration::new(POPPY)),
            ConfiguredFeature::simple_block(SimpleBlockConfiguration::new(DANDELION)),
        ));

    let mut true_chunk = flat_grass_chunk();
    assert!(feature.place(
        &mut true_chunk,
        &mut BooleanRandom::new(true),
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(true_chunk.get_block_at_y(8, 3, 8), POPPY);

    let mut false_chunk = flat_grass_chunk();
    assert!(feature.place(
        &mut false_chunk,
        &mut BooleanRandom::new(false),
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(false_chunk.get_block_at_y(8, 3, 8), DANDELION);
}

#[test]
fn spring_feature_places_fluid_and_schedules_liquid_tick() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    let origin = BlockPos::new(8, 8, 8);
    for pos in [
        BlockPos::new(8, 9, 8),
        BlockPos::new(8, 7, 8),
        BlockPos::new(7, 8, 8),
        BlockPos::new(9, 8, 8),
        BlockPos::new(8, 8, 7),
    ] {
        chunk.set_block_at_y(pos.x, pos.y, pos.z, STONE);
    }
    let mut random = WorldgenRandom::new(0);
    let feature = ConfiguredFeature::spring(SpringConfiguration::water());

    assert!(feature.place(&mut chunk, &mut random, origin));
    assert_eq!(chunk.get_block_at_y(8, 8, 8), WATER);
    assert_eq!(
        chunk.liquid_ticks(),
        &[crate::levelgen::ScheduledTick::new(
            8,
            8,
            8,
            "minecraft:water",
            0
        )]
    );
}

#[test]
fn random_patch_projects_to_surface_and_places_multiple_blocks() {
    let mut chunk = flat_grass_chunk();
    let before = chunk.non_air_block_count();
    let mut random = WorldgenRandom::new(12_345);
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: GRASS,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 16,
        xspread: 3,
        yspread: 1,
        zspread: 3,
        project: true,
        can_replace: false,
        double_plant: false,
        column_height: None,
        need_water: false,
        place_on: &[GRASS_BLOCK],
    });

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
    assert!(chunk.non_air_block_count() > before);
    assert!(chunk.blocks.iter().any(|block_id| *block_id == GRASS));
}

#[test]
fn random_patch_double_plant_writes_large_fern_halves() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(3);
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: LARGE_FERN_LOWER,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: true,
        column_height: None,
        need_water: false,
        place_on: &[],
    });

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert_eq!(chunk.get_block_at_y(8, 3, 8), LARGE_FERN_LOWER);
    assert_eq!(chunk.get_block_at_y(8, 4, 8), LARGE_FERN_UPPER);
}

#[test]
fn random_patch_double_plant_writes_tall_grass_halves() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(3);
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: TALL_GRASS_LOWER,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: true,
        column_height: None,
        need_water: false,
        place_on: &[],
    });

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert_eq!(chunk.get_block_at_y(8, 3, 8), TALL_GRASS_LOWER);
    assert_eq!(chunk.get_block_at_y(8, 4, 8), TALL_GRASS_UPPER);
}

#[test]
fn random_patch_double_plant_writes_requested_flower_halves() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(3);
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: ROSE_BUSH_LOWER,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: true,
        column_height: None,
        need_water: false,
        place_on: &[],
    });

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
    assert_eq!(chunk.get_block_at_y(8, 3, 8), ROSE_BUSH_LOWER);
    assert_eq!(chunk.get_block_at_y(8, 4, 8), ROSE_BUSH_UPPER);
}

#[test]
fn random_patch_places_sweet_berry_bush_on_grass_whitelist() {
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: SWEET_BERRY_BUSH,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: false,
        column_height: None,
        need_water: false,
        place_on: &[GRASS_BLOCK],
    });
    let mut grass_chunk = flat_grass_chunk();
    let mut grass_random = WorldgenRandom::new(0);

    assert!(feature.place(&mut grass_chunk, &mut grass_random, BlockPos::new(8, 3, 8)));
    assert_eq!(grass_chunk.get_block_at_y(8, 3, 8), SWEET_BERRY_BUSH);

    let mut sand_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    sand_chunk.set_block_at_y(8, 2, 8, SAND);
    let mut sand_random = WorldgenRandom::new(0);

    assert!(!feature.place(&mut sand_chunk, &mut sand_random, BlockPos::new(8, 3, 8)));
    assert_eq!(sand_chunk.get_block_at_y(8, 3, 8), AIR);
}

#[test]
fn random_patch_small_mushrooms_follow_vanilla_light_and_substrate_gate() {
    let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: BROWN_MUSHROOM,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: false,
        column_height: None,
        need_water: false,
        place_on: &[],
    });

    let mut exposed_grass = flat_grass_chunk();
    let mut exposed_random = WorldgenRandom::new(0);
    assert!(!feature.place(
        &mut exposed_grass,
        &mut exposed_random,
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(exposed_grass.get_block_at_y(8, 3, 8), AIR);

    let mut podzol = flat_grass_chunk();
    podzol.set_block_at_y(8, 2, 8, PODZOL);
    let mut podzol_random = WorldgenRandom::new(0);
    assert!(feature.place(&mut podzol, &mut podzol_random, BlockPos::new(8, 3, 8)));
    assert_eq!(podzol.get_block_at_y(8, 3, 8), BROWN_MUSHROOM);

    let mut mycelium = flat_grass_chunk();
    mycelium.set_block_at_y(8, 2, 8, MYCELIUM);
    let mut mycelium_random = WorldgenRandom::new(0);
    assert!(feature.place(&mut mycelium, &mut mycelium_random, BlockPos::new(8, 3, 8)));
    assert_eq!(mycelium.get_block_at_y(8, 3, 8), BROWN_MUSHROOM);

    let mut shaded_grass = flat_grass_chunk();
    for y in 4..=6 {
        shaded_grass.set_block_at_y(8, y, 8, SPRUCE_LEAVES);
    }
    let mut shaded_random = WorldgenRandom::new(0);
    assert!(feature.place(
        &mut shaded_grass,
        &mut shaded_random,
        BlockPos::new(8, 3, 8)
    ));
    assert_eq!(shaded_grass.get_block_at_y(8, 3, 8), BROWN_MUSHROOM);
}

#[test]
fn random_patch_column_placer_supports_cactus_and_water_gated_sugar_cane() {
    let mut cactus_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    cactus_chunk.set_block_at_y(8, 2, 8, SAND);
    let mut cactus_random = WorldgenRandom::new(4);
    let cactus = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: CACTUS,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: false,
        column_height: Some(IntProvider::biased_to_bottom(1, 3)),
        need_water: false,
        place_on: &[],
    });

    assert!(cactus.place(
        &mut cactus_chunk,
        &mut cactus_random,
        BlockPos::new(8, 3, 8)
    ));
    let cactus_height = (3..=5)
        .filter(|y| cactus_chunk.get_block_at_y(8, *y, 8) == CACTUS)
        .count();
    assert!((1..=3).contains(&cactus_height));

    let mut dry_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    dry_chunk.set_block_at_y(8, 2, 8, SAND);
    let sugar_cane = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: SUGAR_CANE,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: false,
        can_replace: false,
        double_plant: false,
        column_height: Some(IntProvider::constant(2)),
        need_water: true,
        place_on: &[],
    });
    let mut dry_random = WorldgenRandom::new(0);

    assert!(!sugar_cane.place(&mut dry_chunk, &mut dry_random, BlockPos::new(8, 3, 8)));

    let mut wet_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    wet_chunk.set_block_at_y(8, 2, 8, SAND);
    wet_chunk.set_block_at_y(9, 2, 8, WATER);
    let mut wet_random = WorldgenRandom::new(0);

    assert!(sugar_cane.place(&mut wet_chunk, &mut wet_random, BlockPos::new(8, 3, 8)));
    assert_eq!(wet_chunk.get_block_at_y(8, 3, 8), SUGAR_CANE);
    assert_eq!(wet_chunk.get_block_at_y(8, 4, 8), SUGAR_CANE);
}

#[test]
fn random_patch_places_lily_pad_on_projected_water_surface() {
    let lily_pad = ConfiguredFeature::random_patch(RandomPatchConfiguration {
        state: LILY_PAD,
        weighted_states: &[],
        state_provider: RandomPatchStateProvider::Simple,
        tries: 1,
        xspread: 0,
        yspread: 0,
        zspread: 0,
        project: true,
        can_replace: false,
        double_plant: false,
        column_height: None,
        need_water: false,
        place_on: &[],
    });

    let mut ocean_chunk = flat_ocean_chunk();
    let mut ocean_random = WorldgenRandom::new(0);

    assert!(lily_pad.place(&mut ocean_chunk, &mut ocean_random, BlockPos::new(8, 0, 8)));
    assert_eq!(ocean_chunk.get_block_at_y(8, 11, 8), LILY_PAD);

    let mut grass_chunk = flat_grass_chunk();
    let mut grass_random = WorldgenRandom::new(0);

    assert!(!lily_pad.place(&mut grass_chunk, &mut grass_random, BlockPos::new(8, 0, 8)));
    assert_eq!(grass_chunk.get_block_at_y(8, 3, 8), AIR);
}

#[test]
fn seagrass_feature_places_single_or_tall_water_plant_on_ocean_floor() {
    let mut short_chunk = flat_ocean_chunk();
    let mut short_random = WorldgenRandom::new(1);
    let short = ConfiguredFeature::seagrass(SeagrassConfiguration::new(0.0));
    assert!(short.place(&mut short_chunk, &mut short_random, BlockPos::new(8, 0, 8)));
    assert_eq!(count_blocks(&short_chunk, SEAGRASS), 1);

    let mut tall_chunk = flat_ocean_chunk();
    let mut tall_random = WorldgenRandom::new(1);
    let tall = ConfiguredFeature::seagrass(SeagrassConfiguration::new(1.0));
    assert!(tall.place(&mut tall_chunk, &mut tall_random, BlockPos::new(8, 0, 8)));
    assert_eq!(count_blocks(&tall_chunk, TALL_SEAGRASS_LOWER), 1);
    assert_eq!(count_blocks(&tall_chunk, TALL_SEAGRASS_UPPER), 1);
}

#[test]
fn kelp_feature_places_body_column_with_head_in_water() {
    let mut chunk = flat_ocean_chunk();
    let mut random = WorldgenRandom::new(2);
    let feature = ConfiguredFeature::kelp();

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
    assert_eq!(count_blocks(&chunk, KELP), 1);
    assert!(count_blocks(&chunk, KELP_PLANT) > 0);
}

#[test]
fn sea_pickle_feature_places_waterlogged_pickles_on_ocean_floor() {
    let mut chunk = flat_ocean_chunk();
    let mut random = WorldgenRandom::new(3);
    let feature = ConfiguredFeature::sea_pickle(CountConfiguration::new(20));

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
    assert!(
        [SEA_PICKLE_1, SEA_PICKLE_2, SEA_PICKLE_3, SEA_PICKLE_4]
            .iter()
            .any(|state| count_blocks(&chunk, *state) > 0)
    );
}

#[test]
fn blue_ice_feature_spreads_from_water_next_to_packed_ice() {
    let mut chunk = flat_ocean_chunk();
    chunk.set_block_at_y(9, 5, 8, PACKED_ICE);
    let mut random = WorldgenRandom::new(4);
    let feature = ConfiguredFeature::blue_ice();

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 5, 8)));
    assert!(count_blocks(&chunk, BLUE_ICE) > 0);
}

#[test]
fn coral_features_place_live_coral_blocks_in_water() {
    for (shape, seed) in [
        (CoralShape::Tree, 4),
        (CoralShape::Claw, 5),
        (CoralShape::Mushroom, 6),
    ] {
        let mut chunk = flat_ocean_chunk();
        let mut random = WorldgenRandom::new(seed);
        let feature = ConfiguredFeature::coral(shape);

        assert!(
            feature.place(&mut chunk, &mut random, BlockPos::new(8, 2, 8)),
            "{shape:?}"
        );
        assert!(
            [
                TUBE_CORAL_BLOCK,
                BRAIN_CORAL_BLOCK,
                BUBBLE_CORAL_BLOCK,
                FIRE_CORAL_BLOCK,
                HORN_CORAL_BLOCK,
            ]
            .iter()
            .any(|state| count_blocks(&chunk, *state) > 0),
            "{shape:?}"
        );
    }
}

#[test]
fn coral_feature_places_java_sidecar_plants_and_wall_fans() {
    let mut chunk = flat_ocean_chunk();
    let mut random = ScriptedRandom::new(
        vec![
            0, // coral block type
            0, // tree height: one trunk block
            0, // coral plant type
            0, // north wall-fan type
        ],
        vec![
            0.0, // place a coral plant above the trunk block
            0.0, // place a wall fan north of the trunk block
            1.0, 1.0, 1.0,
        ],
    );
    let feature = ConfiguredFeature::coral(CoralShape::Tree);

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 2, 8)));
    assert!(count_blocks(&chunk, TUBE_CORAL_BLOCK) > 0);
    assert_eq!(chunk.get_block_at_y(8, 3, 8), TUBE_CORAL);
    assert_eq!(chunk.get_block_at_y(8, 2, 7), TUBE_CORAL_WALL_FAN_NORTH);
}

#[test]
fn glow_lichen_feature_places_against_stone_face() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    chunk.set_block_at_y(8, 9, 8, STONE);
    let mut random = WorldgenRandom::new(12_345);
    let feature = ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld());

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 8, 8)));
    assert_eq!(chunk.get_block_at_y(8, 8, 8), GLOW_LICHEN);
    assert_eq!(chunk.glow_lichen_faces_at_y(8, 8, 8), Direction::Up.bit());
}

#[test]
fn glow_lichen_spread_can_write_visible_neighbor_block() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    chunk.set_glow_lichen_faces_at_y(8, 8, 8, Direction::North.bit());
    chunk.set_block_at_y(9, 8, 7, STONE);

    assert!(glow_lichen::spread_glow_lichen_from_face_toward_direction(
        &mut chunk,
        BlockPos::new(8, 8, 8),
        Direction::North,
        Direction::East,
    ));
    assert_eq!(chunk.get_block_at_y(9, 8, 8), GLOW_LICHEN);
}

#[test]
fn glow_lichen_spread_stops_when_toward_face_already_exists() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    chunk.set_glow_lichen_faces_at_y(8, 8, 8, Direction::North.bit() | Direction::East.bit());
    chunk.set_block_at_y(9, 8, 7, STONE);

    assert!(!glow_lichen::spread_glow_lichen_from_face_toward_direction(
        &mut chunk,
        BlockPos::new(8, 8, 8),
        Direction::North,
        Direction::East,
    ));
    assert_eq!(chunk.get_block_at_y(9, 8, 8), AIR);
    assert_eq!(
        chunk.glow_lichen_faces_at_y(8, 8, 8),
        Direction::North.bit() | Direction::East.bit(),
    );
}

#[test]
fn lake_feature_uses_cave_air_for_upper_cavity() {
    let mut chunk = solid_stone_chunk();
    let mut random = WorldgenRandom::new(12_345);
    let feature = ConfiguredFeature::lake(LakeConfiguration::new(WATER));

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(0, 20, 0)));
    assert!(count_blocks(&chunk, CAVE_AIR) > 0);
    assert!(count_blocks(&chunk, WATER) > 0);
}

#[test]
fn placed_feature_applies_count_then_square_decorators() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(12_345);
    let placed = PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: POPPY,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 8,
            xspread: 1,
            yspread: 1,
            zspread: 1,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK],
        }),
        vec![ConfiguredDecorator::count(2), ConfiguredDecorator::square()],
    );

    assert!(placed.place(&mut chunk, &mut random, BlockPos::new(0, 0, 0)));
    assert!(chunk.blocks.iter().any(|block_id| *block_id == POPPY));
}

#[test]
fn feature_world_heightmaps_follow_reduced_java_predicates() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    chunk.set_block_at_y(8, 2, 8, DIRT);
    chunk.set_block_at_y(8, 3, 8, WATER);
    chunk.set_block_at_y(8, 4, 8, GRASS);
    chunk.set_block_at_y(8, 5, 8, SPRUCE_LEAVES);
    chunk.set_block_at_y(8, 6, 8, SNOW);

    assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 8, 8), Some(7));
    assert_eq!(chunk.height_at(HeightmapType::OceanFloor, 8, 8), Some(6));
    assert_eq!(
        chunk.height_at(HeightmapType::MotionBlocking, 8, 8),
        Some(6)
    );
    assert_eq!(
        chunk.height_at(HeightmapType::MotionBlockingNoLeaves, 8, 8),
        Some(4)
    );
}

#[test]
fn primed_worldgen_heightmaps_ignore_later_feature_writes() {
    let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
    chunk.set_block_at_y(8, 2, 8, DIRT);
    chunk.prime_worldgen_heightmaps();
    chunk.set_block_at_y(8, 5, 8, SPRUCE_LEAVES);

    assert_eq!(
        chunk.height_at(HeightmapType::WorldSurfaceWg, 8, 8),
        Some(3)
    );
    assert_eq!(chunk.height_at(HeightmapType::OceanFloorWg, 8, 8), Some(3));
    assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 8, 8), Some(6));
    assert_eq!(chunk.height_at(HeightmapType::OceanFloor, 8, 8), Some(6));
}

#[test]
fn placed_feature_uses_world_heightmap_and_water_depth_decorators() {
    let feature = ConfiguredFeature::simple_block(
        SimpleBlockConfiguration::new(POPPY)
            .place_on(&[GRASS_BLOCK])
            .place_in(&[AIR]),
    );
    let placed = PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        feature,
        vec![
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ],
    );
    let origin = BlockPos::new(8, 0, 8);

    let mut dry_chunk = flat_grass_chunk();
    let mut dry_random = WorldgenRandom::new(12_345);
    assert!(placed.place(&mut dry_chunk, &mut dry_random, origin));
    assert_eq!(dry_chunk.get_block_at_y(8, 3, 8), POPPY);

    let mut wet_chunk = flat_grass_chunk();
    wet_chunk.set_block_at_y(8, 3, 8, WATER);
    let mut wet_random = WorldgenRandom::new(12_345);
    assert!(!placed.place(&mut wet_chunk, &mut wet_random, origin));
    assert_eq!(wet_chunk.get_block_at_y(8, 3, 8), WATER);
    assert_eq!(dry_random.get_count(), wet_random.get_count());
}

#[test]
fn decorated_configured_feature_applies_nested_decorators() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(12_345);
    let feature = ConfiguredFeature::decorated(DecoratedFeatureConfiguration::new(
        ConfiguredFeature::simple_block(
            SimpleBlockConfiguration::new(DANDELION)
                .place_on(&[GRASS_BLOCK])
                .place_in(&[AIR]),
        ),
        [ConfiguredDecorator::heightmap(HeightmapType::OceanFloor)],
    ));

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
    assert_eq!(chunk.get_block_at_y(8, 3, 8), DANDELION);
    assert_eq!(random.get_count(), 0);
}

#[test]
fn freeze_top_layer_places_snow_on_cold_motion_blocking_surface() {
    let mut chunk = flat_grass_chunk();
    let mut random = WorldgenRandom::new(12_345);
    let biomes = ConstantFeatureBiomeResolver::new(get_layered_biome_by_id(12));
    let feature = ConfiguredFeature::freeze_top_layer();

    assert!(feature.place_with_biomes(&mut chunk, &biomes, &mut random, BlockPos::new(0, 0, 0),));
    assert_eq!(chunk.get_block_at_y(8, 3, 8), SNOW);
    assert_eq!(random.get_count(), 0);
}

#[test]
fn freeze_top_layer_freezes_surface_water_before_snow_check() {
    let mut chunk = flat_grass_chunk();
    chunk.set_block_at_y(8, 2, 8, WATER);
    let mut random = WorldgenRandom::new(12_345);
    let biomes = ConstantFeatureBiomeResolver::new(get_layered_biome_by_id(12));
    let feature = ConfiguredFeature::freeze_top_layer();

    assert!(feature.place_with_biomes(&mut chunk, &biomes, &mut random, BlockPos::new(0, 0, 0),));
    assert_eq!(chunk.get_block_at_y(8, 2, 8), ICE);
    assert_eq!(chunk.get_block_at_y(8, 3, 8), SNOW);
}

#[test]
fn ore_feature_replaces_natural_stone_blob() {
    let mut chunk = solid_stone_chunk();
    let mut random = WorldgenRandom::new(12_345);
    let feature = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));

    assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 32, 8)));
    assert!(count_blocks(&chunk, DIORITE) > 0);
    assert!(count_blocks(&chunk, STONE) < CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 64);
}

#[test]
fn ore_feature_uses_ocean_floor_wg_height_gate() {
    let mut world = OreHeightmapProbeWorld::default();
    let mut random = WorldgenRandom::new(12_345);
    let feature = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));

    assert!(feature.place(&mut world, &mut random, BlockPos::new(8, 32, 8)));
    assert!(world.writes > 0);
    assert_eq!(world.world_surface_queries, 0);
    assert_eq!(world.height_queries, vec![HeightmapType::OceanFloorWg]);
}

#[test]
fn placed_feature_interleaves_decorator_branches_with_feature_random() {
    let origin = BlockPos::new(0, 0, 0);
    let range = HeightProvider::uniform(VerticalAnchor::absolute(8), VerticalAnchor::absolute(48));
    let ore = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));
    let placed = PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ore.clone(),
        vec![
            ConfiguredDecorator::count(2),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(range),
        ],
    );
    let mut placed_chunk = solid_stone_chunk();
    let mut placed_random = WorldgenRandom::new(12_345);

    placed.place(&mut placed_chunk, &mut placed_random, origin);

    let mut manual_chunk = solid_stone_chunk();
    let mut manual_random = WorldgenRandom::new(12_345);
    let context = DecorationContext::new(manual_chunk.min_y, manual_chunk.height);
    for _ in 0..2 {
        for square_pos in
            ConfiguredDecorator::square().get_positions(&context, &mut manual_random, origin)
        {
            for range_pos in ConfiguredDecorator::range(range).get_positions(
                &context,
                &mut manual_random,
                square_pos,
            ) {
                ore.place(&mut manual_chunk, &mut manual_random, range_pos);
            }
        }
    }

    assert_eq!(placed_chunk.blocks, manual_chunk.blocks);
    assert_eq!(placed_random.get_count(), manual_random.get_count());
}
