use super::*;

#[test]
fn generated_features_chunk_adds_visible_decoration_blocks() {
    let features = generate_overworld_features_chunk(12345, 0, 0);
    let feature_block_count = features.block_count(crate::block::OAK_LOG)
        + features.block_count(crate::block::OAK_LEAVES)
        + features.block_count(crate::block::BIRCH_LOG)
        + features.block_count(crate::block::BIRCH_LEAVES)
        + features.block_count(crate::block::SPRUCE_LOG)
        + features.block_count(crate::block::SPRUCE_LEAVES)
        + features.block_count(crate::block::GRASS)
        + features.block_count(crate::block::FERN)
        + features.block_count(crate::block::DANDELION)
        + features.block_count(crate::block::POPPY)
        + features.block_count(crate::block::DEAD_BUSH);

    assert!(feature_block_count > 0);
    assert!(
        features.block_count(crate::block::GRANITE)
            + features.block_count(crate::block::DIORITE)
            + features.block_count(crate::block::ANDESITE)
            + features.block_count(crate::block::TUFF)
            + features.block_count(crate::block::DEEPSLATE)
            > 0
    );
    assert!(
        features.block_count(crate::block::COAL_ORE)
            + features.block_count(crate::block::DEEPSLATE_COAL_ORE)
            + features.block_count(crate::block::IRON_ORE)
            + features.block_count(crate::block::DEEPSLATE_IRON_ORE)
            + features.block_count(crate::block::COPPER_ORE)
            + features.block_count(crate::block::DEEPSLATE_COPPER_ORE)
            > 0
    );
    assert!(features.block_count(crate::block::LAVA) > 0);
}

#[test]
fn chunk_generation_plan_reuses_overlapping_dependency_windows() {
    let single = ChunkGenerationPlan::overworld_features([ChunkPos::new(0, 0)]);
    assert_eq!(single.output_chunks().len(), 1);
    assert_eq!(single.backend_work_chunks().len(), 3 * 3);
    assert_eq!(single.prerequisites().len(), 5 * 5);

    let radius_one_targets = (-1..=1).flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)));
    let radius_one = ChunkGenerationPlan::overworld_features(radius_one_targets);

    assert_eq!(radius_one.output_chunks().len(), 3 * 3);
    assert_eq!(radius_one.backend_work_chunks().len(), 5 * 5);
    assert_eq!(radius_one.prerequisites().len(), 7 * 7);
    assert!(
        radius_one
            .prerequisites()
            .iter()
            .any(|requirement| requirement.pos == ChunkPos::new(-3, -3))
    );
    assert!(
        radius_one
            .prerequisites()
            .iter()
            .any(|requirement| requirement.pos == ChunkPos::new(3, 3))
    );
}

#[test]
fn feature_center_order_matches_vanilla_scheduler_trace_for_spawn_bootstrap() {
    let trace = scheduler_trace_fixture();
    assert_eq!(trace.module, "scheduler-trace");
    assert_eq!(trace.minecraft_version, "1.17.1");
    assert_eq!(trace.seed, "12345");
    assert_eq!(trace.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
    assert_eq!(trace.stop_status, "FEATURES");

    let plan = ChunkGenerationPlan::overworld_features([ChunkPos::new(
        trace.target_chunk_x,
        trace.target_chunk_z,
    )]);
    let expected = trace
        .feature_completion_order_3x3
        .into_iter()
        .map(|entry| ChunkPos::new(entry.chunk_x, entry.chunk_z))
        .collect::<Vec<_>>();

    assert_eq!(
        sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied()),
        expected
    );
}

#[test]
fn taiga_vegetation_feature_index_shift_is_isolated_by_center() {
    let diagnostics = taiga_tree_index_shift_diagnostics(12_345, ChunkPos::new(0, 0));

    assert_eq!(
        diagnostics,
        vec![
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(-1, -1),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 0,
                java_tree_blocks: 0,
                shared_tree_blocks: 0,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(0, -1),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 9,
                java_tree_blocks: 9,
                shared_tree_blocks: 9,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(1, -1),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 0,
                java_tree_blocks: 0,
                shared_tree_blocks: 0,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(-1, 0),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 0,
                java_tree_blocks: 0,
                shared_tree_blocks: 0,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(0, 0),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 236,
                java_tree_blocks: 236,
                shared_tree_blocks: 236,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(1, 0),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 12,
                java_tree_blocks: 12,
                shared_tree_blocks: 12,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(0, 1),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 14,
                java_tree_blocks: 14,
                shared_tree_blocks: 14,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
            TaigaTreeIndexShiftCenterDiagnostic {
                center: ChunkPos::new(1, 1),
                biome_key: "minecraft:taiga_mountains",
                current_tree_blocks: 0,
                java_tree_blocks: 0,
                shared_tree_blocks: 0,
                current_only_tree_blocks: 0,
                java_only_tree_blocks: 0,
            },
        ]
    );
}

#[test]
fn taiga_full_table_tree_deltas_match_vanilla_scheduler_probe() {
    let diagnostics = taiga_full_table_tree_delta_diagnostics(12_345, ChunkPos::new(0, 0));
    let tree_deltas = diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.center,
                diagnostic.biome_key,
                diagnostic.before_tree_blocks,
                diagnostic.after_tree_blocks,
                diagnostic.added_tree_blocks,
                diagnostic.removed_tree_blocks,
                diagnostic.feature_random_calls,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        tree_deltas,
        vec![
            (
                ChunkPos::new(-1, -1),
                "minecraft:taiga_mountains",
                0,
                0,
                0,
                0,
                75
            ),
            (
                ChunkPos::new(0, -1),
                "minecraft:taiga_mountains",
                0,
                0,
                0,
                0,
                68
            ),
            (
                ChunkPos::new(1, -1),
                "minecraft:taiga_mountains",
                0,
                0,
                0,
                0,
                75
            ),
            (
                ChunkPos::new(-1, 0),
                "minecraft:taiga_mountains",
                0,
                0,
                0,
                0,
                77
            ),
            (
                ChunkPos::new(0, 0),
                "minecraft:taiga_mountains",
                0,
                236,
                236,
                0,
                77
            ),
            (
                ChunkPos::new(1, 0),
                "minecraft:taiga_mountains",
                236,
                246,
                10,
                0,
                86
            ),
            (
                ChunkPos::new(0, 1),
                "minecraft:taiga_mountains",
                246,
                246,
                0,
                0,
                73
            ),
            (
                ChunkPos::new(1, 1),
                "minecraft:taiga_mountains",
                246,
                246,
                0,
                0,
                73
            ),
        ]
    );
}

#[test]
fn overworld_feature_dependency_cache_reuses_overlapping_windows() {
    let mut cache = OverworldFeatureDependencyCache::new();

    let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
    assert_eq!(
        first.cache_report,
        OverworldFeatureDependencyCacheReport {
            requested_dependency_chunks: 5 * 5,
            cache_hits: 0,
            generated_dependency_chunks: 5 * 5,
            retained_dependency_chunks: 5 * 5,
        }
    );

    let second = cache.generate_features_chunks(12_345, [ChunkPos::new(1, 0)]);
    assert_eq!(
        second.cache_report,
        OverworldFeatureDependencyCacheReport {
            requested_dependency_chunks: 5 * 5,
            cache_hits: 4 * 5,
            generated_dependency_chunks: 5,
            retained_dependency_chunks: 5 * 5,
        }
    );
    assert_eq!(cache.retained_chunk_count(), 5 * 5);
    assert_eq!(
        second.chunks.get(&ChunkPos::new(1, 0)),
        Some(&generate_overworld_features_chunk(12_345, 1, 0))
    );
}

#[test]
fn overworld_feature_dependency_cache_keeps_clean_lower_status_chunks() {
    let mut cache = OverworldFeatureDependencyCache::new();

    let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
    let second = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);

    assert_eq!(
        second.cache_report,
        OverworldFeatureDependencyCacheReport {
            requested_dependency_chunks: 5 * 5,
            cache_hits: 5 * 5,
            generated_dependency_chunks: 0,
            retained_dependency_chunks: 5 * 5,
        }
    );
    assert_eq!(
        second.chunks.get(&ChunkPos::new(0, 0)),
        first.chunks.get(&ChunkPos::new(0, 0))
    );
}

#[test]
fn overworld_feature_dependency_cache_resets_when_seed_changes() {
    let mut cache = OverworldFeatureDependencyCache::new();

    cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
    let changed_seed = cache.generate_features_chunks(54_321, [ChunkPos::new(0, 0)]);

    assert_eq!(
        changed_seed.cache_report,
        OverworldFeatureDependencyCacheReport {
            requested_dependency_chunks: 5 * 5,
            cache_hits: 0,
            generated_dependency_chunks: 5 * 5,
            retained_dependency_chunks: 5 * 5,
        }
    );
}

#[test]
fn full_decorated_chunk_gauntlet_reports_current_native_gap() {
    let fixture = full_chunk_fixture();
    assert_eq!(fixture.module, "integration");
    assert_eq!(fixture.minecraft_version, "1.17.1");
    assert_eq!(fixture.seed, "12345");
    assert_eq!(fixture.generator, "default");
    assert!(!fixture.generate_structures);
    assert_eq!(fixture.wire_format.block_order, "y-major,z-major,x-minor");
    assert_eq!(fixture.wire_format.palette_entries, "resource-key");
    assert_eq!(fixture.chunks.len(), 1);

    let expected = &fixture.chunks[0];
    assert_eq!(expected.chunk_x, 0);
    assert_eq!(expected.chunk_z, 0);
    assert_eq!(expected.status, "full");
    let actual = generate_overworld_features_chunk(12_345, expected.chunk_x, expected.chunk_z);
    let report = compare_generated_chunk_to_full_fixture(&actual, expected);

    assert_eq!(report.total_blocks, 16 * 16 * 256);
    assert!(report.matched_blocks < report.total_blocks);
    assert_eq!(report.mismatched_blocks, 6, "{report:#?}");
    assert_eq!(
        report.top_mismatch_pairs,
        vec![
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:water".to_owned(),
                count: 2,
            },
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:glow_lichen".to_owned(),
                count: 1,
            },
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:lava".to_owned(),
                count: 1,
            },
            MismatchBucket {
                actual: "minecraft:brown_mushroom".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 1,
            },
            MismatchBucket {
                actual: "minecraft:glow_lichen".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 1,
            },
        ]
    );
}

#[test]
fn features_status_chunk_snapshot_excludes_runtime_liquid_tick_results() {
    let fixture = scheduler_features_snapshot_fixture();
    assert_eq!(fixture.module, "scheduler-trace");
    assert_eq!(fixture.minecraft_version, "1.17.1");
    assert_eq!(fixture.seed, "12345");
    assert_eq!(fixture.target_chunk_x, 0);
    assert_eq!(fixture.target_chunk_z, 0);
    assert_eq!(fixture.target_radius, 1);
    assert_eq!(fixture.stop_status, "FEATURES");
    assert_eq!(fixture.chunks.len(), 1);

    let expected = &fixture.chunks[0];
    assert_eq!(expected.chunk_x, 0);
    assert_eq!(expected.chunk_z, 0);
    assert_eq!(expected.status, "features");

    let actual = generate_overworld_features_chunk(
        fixture.seed.parse::<i64>().expect("fixture seed is i64"),
        expected.chunk_x,
        expected.chunk_z,
    );
    let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);
    for (local_x, y, local_z, expected_name) in [
        (9, 12, 15, "minecraft:air"),
        (10, 17, 2, "minecraft:air"),
        (7, 17, 10, "minecraft:glow_lichen"),
        (9, 18, 2, "minecraft:water"),
        (10, 18, 2, "minecraft:air"),
    ] {
        let index = ((y - actual.min_y) << 8) | (local_z << 4) | local_x;
        assert_eq!(expected_blocks[index as usize], expected_name);
        assert_eq!(actual.block_at_y(local_x, y, local_z).name(), expected_name);
    }

    let report = compare_generated_chunk_to_full_fixture(&actual, expected);
    assert_eq!(report.mismatched_blocks, 1, "{report:#?}");
    assert_eq!(
        report.top_mismatch_pairs,
        vec![MismatchBucket {
            actual: "minecraft:brown_mushroom".to_owned(),
            expected: "minecraft:air".to_owned(),
            count: 1,
        }]
    );
}

#[test]
fn plains_features_snapshot_reports_current_native_gap() {
    let fixture = plains_scheduler_features_snapshot_fixture();
    assert_eq!(fixture.module, "scheduler-trace");
    assert_eq!(fixture.minecraft_version, "1.17.1");
    assert_eq!(fixture.seed, "16");
    assert_eq!(fixture.target_chunk_x, 0);
    assert_eq!(fixture.target_chunk_z, 0);
    assert_eq!(fixture.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
    assert_eq!(fixture.stop_status, "FEATURES");
    assert_eq!(fixture.chunks.len(), 1);

    let expected = &fixture.chunks[0];
    assert_eq!(expected.chunk_x, 0);
    assert_eq!(expected.chunk_z, 0);
    assert_eq!(expected.status, "features");

    let actual = generate_overworld_features_chunk(16, expected.chunk_x, expected.chunk_z);
    let report = compare_generated_chunk_to_full_fixture(&actual, expected);

    assert_eq!(report.total_blocks, 16 * 16 * 256);
    assert_eq!(report.mismatched_blocks, 67, "{report:#?}");
    assert_eq!(
        report.top_mismatch_pairs,
        vec![
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 29,
            },
            MismatchBucket {
                actual: "minecraft:grass".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 14,
            },
            MismatchBucket {
                actual: "minecraft:poppy".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 5,
            },
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:poppy".to_owned(),
                count: 4,
            },
            MismatchBucket {
                actual: "minecraft:dandelion".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 4,
            },
            MismatchBucket {
                actual: "minecraft:poppy".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 4,
            },
            MismatchBucket {
                actual: "minecraft:water".to_owned(),
                expected: "minecraft:glow_lichen".to_owned(),
                count: 4,
            },
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:tall_grass".to_owned(),
                count: 2,
            },
            MismatchBucket {
                actual: "minecraft:dandelion".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 1,
            },
        ]
    );
}

#[test]
fn inland_plains_features_snapshot_reports_current_native_gap() {
    let fixture = inland_plains_scheduler_features_snapshot_fixture();
    assert_eq!(fixture.module, "scheduler-trace");
    assert_eq!(fixture.minecraft_version, "1.17.1");
    assert_eq!(fixture.seed, "17");
    assert_eq!(fixture.target_chunk_x, 0);
    assert_eq!(fixture.target_chunk_z, 0);
    assert_eq!(fixture.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
    assert_eq!(fixture.stop_status, "FEATURES");
    assert_eq!(fixture.chunks.len(), 1);

    let expected = &fixture.chunks[0];
    assert_eq!(expected.chunk_x, 0);
    assert_eq!(expected.chunk_z, 0);
    assert_eq!(expected.status, "features");

    let actual = generate_overworld_features_chunk(17, expected.chunk_x, expected.chunk_z);
    let report = compare_generated_chunk_to_full_fixture(&actual, expected);

    assert_eq!(report.total_blocks, 16 * 16 * 256);
    assert_eq!(report.mismatched_blocks, 110, "{report:#?}");
    assert_eq!(
        report.top_mismatch_pairs,
        vec![
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 38,
            },
            MismatchBucket {
                actual: "minecraft:grass".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 26,
            },
            MismatchBucket {
                actual: "minecraft:dandelion".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 14,
            },
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:dandelion".to_owned(),
                count: 13,
            },
            MismatchBucket {
                actual: "minecraft:dandelion".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 7,
            },
            MismatchBucket {
                actual: "minecraft:poppy".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 4,
            },
            MismatchBucket {
                actual: "minecraft:poppy".to_owned(),
                expected: "minecraft:dandelion".to_owned(),
                count: 3,
            },
            MismatchBucket {
                actual: "minecraft:grass".to_owned(),
                expected: "minecraft:dandelion".to_owned(),
                count: 2,
            },
            MismatchBucket {
                actual: "minecraft:deepslate_redstone_ore".to_owned(),
                expected: "minecraft:redstone_ore".to_owned(),
                count: 1,
            },
            MismatchBucket {
                actual: "minecraft:granite".to_owned(),
                expected: "minecraft:water".to_owned(),
                count: 1,
            },
            MismatchBucket {
                actual: "minecraft:poppy".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 1,
            },
        ]
    );
}

#[test]
fn forest_seed_23823_features_snapshot_reports_current_native_gap() {
    let fixture = forest_seed_23823_scheduler_features_snapshot_fixture();
    assert_eq!(fixture.module, "scheduler-trace");
    assert_eq!(fixture.minecraft_version, "1.17.1");
    assert_eq!(fixture.seed, "23823");
    assert_eq!(fixture.target_chunk_x, -13);
    assert_eq!(fixture.target_chunk_z, 8);
    assert_eq!(fixture.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
    assert_eq!(fixture.stop_status, "FEATURES");
    assert_eq!(fixture.chunks.len(), 1);

    let expected = &fixture.chunks[0];
    assert_eq!(expected.chunk_x, -13);
    assert_eq!(expected.chunk_z, 8);
    assert_eq!(expected.status, "features");

    let actual = generate_overworld_features_chunk(23823, expected.chunk_x, expected.chunk_z);
    let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);

    let canopy_index = ((67 - actual.min_y) << 8) | (8 << 4) | 8;
    assert_eq!(
        expected_blocks[canopy_index as usize],
        "minecraft:oak_leaves"
    );
    assert_eq!(actual.block_at_y(8, 67, 8).name(), "minecraft:oak_leaves");

    let fancy_oak_index = ((70 - actual.min_y) << 8) | (8 << 4) | 8;
    assert_eq!(
        expected_blocks[fancy_oak_index as usize],
        "minecraft:oak_leaves"
    );
    assert_eq!(actual.block_at_y(8, 70, 8).name(), "minecraft:oak_leaves");

    let report = compare_generated_chunk_to_full_fixture(&actual, expected);

    assert_eq!(report.total_blocks, 16 * 16 * 256);
    assert_eq!(report.mismatched_blocks, 6, "{report:#?}");
    assert_eq!(
        report.top_mismatch_pairs,
        vec![
            MismatchBucket {
                actual: "minecraft:air".to_owned(),
                expected: "minecraft:grass".to_owned(),
                count: 3,
            },
            MismatchBucket {
                actual: "minecraft:grass".to_owned(),
                expected: "minecraft:air".to_owned(),
                count: 3,
            },
        ]
    );
}

#[test]
#[ignore = "active gauntlet: native full decorated chunk parity is not expected to pass yet"]
fn full_decorated_chunk_zero_zero_matches_java_oracle() {
    let fixture = full_chunk_fixture();
    let expected = &fixture.chunks[0];
    let actual = generate_overworld_features_chunk(
        fixture.seed.parse::<i64>().expect("fixture seed is i64"),
        expected.chunk_x,
        expected.chunk_z,
    );
    let report = compare_generated_chunk_to_full_fixture(&actual, expected);

    assert!(report.is_exact(), "{report:#?}");
}
