use super::*;

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
            assert!(
                feature_family.is_present(&chunk),
                "seed {} chunk ({}, {}) did not satisfy {}; feature block counts: {:?}",
                case.seed,
                case.chunk_x,
                case.chunk_z,
                feature_family.name(),
                feature_block_counts(&chunk, feature_family)
            );
        }
    }
}

#[test]
fn palette_matrix_low_visibility_feature_slots_have_deterministic_fixtures() {
    for case in LOW_VISIBILITY_FEATURE_CASES {
        let biome_source = OverworldBiomeSource::new(case.seed, false, false);
        assert_eq!(
            biome_source
                .get_primary_biome_definition(case.chunk_x, case.chunk_z)
                .key(),
            case.biome_key,
            "primary biome for seed {} chunk ({}, {})",
            case.seed,
            case.chunk_x,
            case.chunk_z
        );

        let matrix_case = PaletteMatrixCase {
            seed: case.seed,
            chunk_x: case.chunk_x,
            chunk_z: case.chunk_z,
            biome_key: case.biome_key,
            surface_family: SurfaceFamily::Grass,
            feature_family: None,
        };
        let block_position_biomes =
            count_block_position_biomes_in_chunk(&biome_source, case.seed, &matrix_case);
        assert!(
            block_position_biomes > 0,
            "seed {} chunk ({}, {}) had no block-position {} samples",
            case.seed,
            case.chunk_x,
            case.chunk_z,
            case.biome_key
        );

        let chunk = generate_overworld_features_chunk(case.seed, case.chunk_x, case.chunk_z);
        match case.expectation {
            LowVisibilityFeatureExpectation::VisibleSmallMushrooms { min_count } => {
                let actual = chunk.block_count(BROWN_MUSHROOM) + chunk.block_count(RED_MUSHROOM);
                assert!(
                    actual >= min_count,
                    "seed {} chunk ({}, {}) expected at least {} visible small mushrooms, found {}",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    min_count,
                    actual
                );
            }
            LowVisibilityFeatureExpectation::JungleCocoaVines {
                min_cocoa,
                min_vines,
            } => {
                let cocoa = [
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
                ]
                .iter()
                .map(|block| chunk.block_count(*block))
                .sum::<usize>();
                let vines = [VINE_UP, VINE_NORTH, VINE_EAST, VINE_SOUTH, VINE_WEST]
                    .iter()
                    .map(|block| chunk.block_count(*block))
                    .sum::<usize>();
                assert!(
                    cocoa >= min_cocoa && vines >= min_vines,
                    "seed {} chunk ({}, {}) expected jungle cocoa>={} vines>={}, found cocoa={} vines={}",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    min_cocoa,
                    min_vines,
                    cocoa,
                    vines
                );
            }
            LowVisibilityFeatureExpectation::JungleBushShape { min_matches } => {
                let actual = jungle_bush_shape_count(&chunk);
                assert!(
                    actual >= min_matches,
                    "seed {} chunk ({}, {}) expected at least {} jungle bush shape matches, found {}",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    min_matches,
                    actual
                );
            }
            LowVisibilityFeatureExpectation::VisiblePumpkins { min_count } => {
                let actual = pumpkins_on_grass_count(&chunk);
                assert!(
                    actual >= min_count,
                    "seed {} chunk ({}, {}) expected at least {} visible pumpkins on grass, found {}",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    min_count,
                    actual
                );
            }
            LowVisibilityFeatureExpectation::ForestRockBoulders { min_count } => {
                let actual = chunk.block_count(MOSSY_COBBLESTONE);
                assert!(
                    actual >= min_count,
                    "seed {} chunk ({}, {}) expected at least {} mossy cobblestone forest-rock boulder blocks, found {}",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    min_count,
                    actual
                );
            }
            LowVisibilityFeatureExpectation::DefaultSpringVisibleFluids {
                min_water_blocks,
                min_lava_blocks,
            } => {
                let water_blocks = exposed_spring_tick_block_count(&chunk, WATER);
                let lava_blocks = exposed_spring_tick_block_count(&chunk, LAVA);
                assert!(
                    water_blocks >= min_water_blocks && lava_blocks >= min_lava_blocks,
                    "seed {} chunk ({}, {}) expected exposed spring fluid blocks water>={} lava>={}, found water={} lava={}; liquid ticks: {:?}",
                    case.seed,
                    case.chunk_x,
                    case.chunk_z,
                    min_water_blocks,
                    min_lava_blocks,
                    water_blocks,
                    lava_blocks,
                    chunk.liquid_ticks()
                );
            }
        }
    }
}
