use super::*;

pub(super) fn terrain_stage_blocks_from_oracle(
    oracle: &TerrainChunkOracleFixture,
) -> Vec<RawBlockId> {
    oracle
        .blocks
        .iter()
        .map(|block_id| {
            if *block_id == BEDROCK {
                STONE
            } else {
                RawBlockId::from(*block_id)
            }
        })
        .collect()
}

pub(super) fn assert_chunk_blocks_match(
    actual: &[RawBlockId],
    expected: &[RawBlockId],
    min_y: i32,
) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            actual,
            expected,
            "chunk block mismatch at local ({}, {}, {}): expected block id {}, got {}",
            index & 15,
            (index >> 8) as i32 + min_y,
            (index >> 4) & 15,
            expected,
            actual
        );
    }
}

pub(super) fn assert_terrain_chunk_matches_java_oracle(oracle: TerrainChunkOracleFixture) {
    assert_eq!(oracle.module, "terrain-chunk");
    assert_eq!(oracle.minecraft_version, "1.17.1");
    assert_eq!(
        oracle.generator_class,
        "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
    );
    assert_eq!(oracle.min_y, 0);
    assert_eq!(oracle.height, 256);
    assert_eq!(oracle.block_order, "y-major,z-major,x-minor");

    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    let expected = terrain_stage_blocks_from_oracle(&oracle);

    assert_eq!(chunk.chunk_x, oracle.chunk_x);
    assert_eq!(chunk.chunk_z, oracle.chunk_z);
    assert_eq!(chunk.min_y, oracle.min_y);
    assert_eq!(chunk.height, oracle.height);
    assert!(!chunk.blocks.contains(&RawBlockId::from(BEDROCK)));
    assert_chunk_blocks_match(&chunk.blocks, &expected, oracle.min_y);
}

pub(super) fn surface_top_signal(oracle: &TerrainChunkOracleFixture) -> SurfaceTopSignal {
    assert_eq!(oracle.block_order, "y-major,z-major,x-minor");
    let mut signal = SurfaceTopSignal {
        min_top_y: i32::MAX,
        ..SurfaceTopSignal::default()
    };
    for local_z in 0..GeneratedChunk::WIDTH {
        for local_x in 0..GeneratedChunk::WIDTH {
            for y in (oracle.min_y..oracle.min_y + oracle.height).rev() {
                let block_name = surface_fixture_block_name_at(oracle, local_x, y, local_z);
                if block_name == "minecraft:air" {
                    continue;
                }

                signal.min_top_y = signal.min_top_y.min(y);
                signal.max_top_y = signal.max_top_y.max(y);
                if y > 80 {
                    signal.columns_above_80 += 1;
                }
                if y > 100 {
                    signal.columns_above_100 += 1;
                }
                if is_badlands_surface_block_name(block_name) {
                    signal.badlands_top_columns += 1;
                }
                break;
            }
        }
    }
    signal
}

pub(super) fn macro_geometry_signal(
    chunk: &MutableChunkBlockBuffer,
    is_landmark_block: impl Fn(RawBlockId) -> bool,
    landmark_block_min_y: Option<i32>,
    landmark_column_min_y: Option<i32>,
) -> MacroGeometrySignal {
    let mut top_y_by_column = [chunk.min_y; CHUNK_WIDTH as usize * CHUNK_WIDTH as usize];
    let mut top_block_by_column = [AIR; CHUNK_WIDTH as usize * CHUNK_WIDTH as usize];
    let mut top_heights = Vec::with_capacity(CHUNK_WIDTH as usize * CHUNK_WIDTH as usize);
    let mut signal = MacroGeometrySignal {
        top_y_min: i32::MAX,
        top_y_max: i32::MIN,
        ..MacroGeometrySignal::default()
    };

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let column_index = (local_z * CHUNK_WIDTH + local_x) as usize;
            for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
                let block_id = chunk.get_block_at_y(local_x, y, local_z);
                if is_air_like(block_id) {
                    continue;
                }
                top_y_by_column[column_index] = y;
                top_block_by_column[column_index] = block_id;
                signal.top_y_min = signal.top_y_min.min(y);
                signal.top_y_max = signal.top_y_max.max(y);
                top_heights.push(y);
                break;
            }
        }
    }

    top_heights.sort_unstable();
    if let (Some(first), Some(last)) = (top_heights.first(), top_heights.last()) {
        signal.top_y_min = *first;
        signal.top_y_max = *last;
        signal.top_y_range = *last - *first;
        signal.top_y_p05 = percentile(&top_heights, 5);
        signal.top_y_p50 = percentile(&top_heights, 50);
        signal.top_y_p95 = percentile(&top_heights, 95);
    }

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let column_index = (local_z * CHUNK_WIDTH + local_x) as usize;
            if local_x + 1 < CHUNK_WIDTH {
                let neighbor_index = (local_z * CHUNK_WIDTH + local_x + 1) as usize;
                record_neighbor_top_delta(
                    &mut signal,
                    top_y_by_column[column_index],
                    top_y_by_column[neighbor_index],
                    top_block_by_column[column_index],
                    top_block_by_column[neighbor_index],
                );
            }
            if local_z + 1 < CHUNK_WIDTH {
                let neighbor_index = ((local_z + 1) * CHUNK_WIDTH + local_x) as usize;
                record_neighbor_top_delta(
                    &mut signal,
                    top_y_by_column[column_index],
                    top_y_by_column[neighbor_index],
                    top_block_by_column[column_index],
                    top_block_by_column[neighbor_index],
                );
            }
        }
    }

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let column_index = (local_z * CHUNK_WIDTH + local_x) as usize;
            if column_has_vertical_face_run(chunk, local_x, local_z, 4) {
                signal.vertical_face_columns += 1;
            }
            if !is_air_like(top_block_by_column[column_index])
                && column_has_surface_near_carved_air(
                    chunk,
                    local_x,
                    local_z,
                    top_y_by_column[column_index],
                )
            {
                signal.surface_near_carved_air_columns += 1;
            }
        }
    }

    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let mut carved_air_span = 0usize;
            let mut landmark_column_counted = false;
            for y in chunk.min_y..chunk.min_y + chunk.height {
                let block_id = chunk.get_block_at_y(local_x, y, local_z);
                if y > chunk.min_y
                    && is_macro_solid(block_id)
                    && is_air_like(chunk.get_block_at_y(local_x, y - 1, local_z))
                {
                    signal.solid_over_air_blocks += 1;
                }

                if block_id == crate::block::CAVE_AIR {
                    signal.carved_air_volume += 1;
                    signal.carved_air_y_min =
                        Some(signal.carved_air_y_min.map_or(y, |min| min.min(y)));
                    signal.carved_air_y_max =
                        Some(signal.carved_air_y_max.map_or(y, |max| max.max(y)));
                    carved_air_span += 1;
                } else if carved_air_span > 0 {
                    if carved_air_span >= 8 {
                        signal.long_vertical_air_spans += 1;
                    }
                    carved_air_span = 0;
                }

                if is_landmark_block(block_id)
                    && landmark_block_min_y.map_or(true, |min_y| y >= min_y)
                {
                    signal.landmark_block_volume += 1;
                    if !landmark_column_counted
                        && landmark_column_min_y.map_or(true, |min_y| y >= min_y)
                    {
                        signal.landmark_column_count += 1;
                        landmark_column_counted = true;
                    }
                }
            }
            if carved_air_span >= 8 {
                signal.long_vertical_air_spans += 1;
            }
        }
    }

    signal
}

pub(super) fn column_has_vertical_face_run(
    chunk: &MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    min_run: usize,
) -> bool {
    for (offset_x, offset_z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let neighbor_x = local_x + offset_x;
        let neighbor_z = local_z + offset_z;
        if !(0..CHUNK_WIDTH).contains(&neighbor_x) || !(0..CHUNK_WIDTH).contains(&neighbor_z) {
            continue;
        }

        let mut run = 0usize;
        for y in chunk.min_y..chunk.min_y + chunk.height {
            if is_macro_solid(chunk.get_block_at_y(local_x, y, local_z))
                && is_air_like(chunk.get_block_at_y(neighbor_x, y, neighbor_z))
            {
                run += 1;
                if run >= min_run {
                    return true;
                }
            } else {
                run = 0;
            }
        }
    }
    false
}

pub(super) fn column_has_surface_near_carved_air(
    chunk: &MutableChunkBlockBuffer,
    local_x: i32,
    local_z: i32,
    top_y: i32,
) -> bool {
    let min_y = (top_y - 6).max(chunk.min_y);
    let max_y = (top_y + 2).min(chunk.min_y + chunk.height - 1);
    for (offset_x, offset_z) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
        let sample_x = local_x + offset_x;
        let sample_z = local_z + offset_z;
        if !(0..CHUNK_WIDTH).contains(&sample_x) || !(0..CHUNK_WIDTH).contains(&sample_z) {
            continue;
        }

        for y in min_y..=max_y {
            if chunk.get_block_at_y(sample_x, y, sample_z) == crate::block::CAVE_AIR {
                return true;
            }
        }
    }
    false
}

pub(super) fn is_macro_solid(block_id: RawBlockId) -> bool {
    !is_air_like(block_id) && !crate::block::has_fluid(block_id)
}

pub(super) fn percentile(sorted: &[i32], percent: usize) -> i32 {
    sorted[(sorted.len() - 1) * percent / 100]
}

pub(super) fn record_neighbor_top_delta(
    signal: &mut MacroGeometrySignal,
    top_y: i32,
    neighbor_top_y: i32,
    top_block: RawBlockId,
    neighbor_top_block: RawBlockId,
) {
    let delta = top_y.abs_diff(neighbor_top_y);
    if delta >= 4 {
        signal.neighbor_delta_ge_4 += 1;
    }
    if delta >= 8 {
        signal.neighbor_delta_ge_8 += 1;
    }
    if delta >= 16 {
        signal.neighbor_delta_ge_16 += 1;
    }
    if is_water(top_block) != is_water(neighbor_top_block) {
        if delta >= 4 {
            signal.water_land_edge_delta_ge_4 += 1;
        }
        if delta >= 8 {
            signal.water_land_edge_delta_ge_8 += 1;
        }
    }
}

pub(super) fn is_badlands_landmark_block_id(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        RED_SAND
            | TERRACOTTA
            | crate::block::WHITE_TERRACOTTA
            | crate::block::ORANGE_TERRACOTTA
            | crate::block::MAGENTA_TERRACOTTA
            | crate::block::LIGHT_BLUE_TERRACOTTA
            | crate::block::YELLOW_TERRACOTTA
            | crate::block::LIME_TERRACOTTA
            | crate::block::PINK_TERRACOTTA
            | crate::block::GRAY_TERRACOTTA
            | crate::block::LIGHT_GRAY_TERRACOTTA
            | crate::block::CYAN_TERRACOTTA
            | crate::block::PURPLE_TERRACOTTA
            | crate::block::BLUE_TERRACOTTA
            | crate::block::BROWN_TERRACOTTA
            | crate::block::GREEN_TERRACOTTA
            | crate::block::RED_TERRACOTTA
            | crate::block::BLACK_TERRACOTTA
    )
}

pub(super) fn is_stone_shore_landmark_block_id(block_id: RawBlockId) -> bool {
    matches!(block_id, STONE | GRAVEL)
}

pub(super) fn is_mountains_landmark_block_id(block_id: RawBlockId) -> bool {
    matches!(block_id, GRASS_BLOCK | STONE | GRAVEL)
}

pub(super) fn is_shattered_savanna_landmark_block_id(block_id: RawBlockId) -> bool {
    matches!(block_id, GRASS_BLOCK | COARSE_DIRT | STONE)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SurfaceMaterialSignal {
    grass_top_columns: usize,
    stone_top_columns: usize,
    gravel_top_columns: usize,
    grass_block_volume: usize,
    stone_block_volume: usize,
    gravel_block_volume: usize,
}

fn surface_material_signal(chunk: &MutableChunkBlockBuffer, min_y: i32) -> SurfaceMaterialSignal {
    let mut signal = SurfaceMaterialSignal::default();
    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let mut top_recorded = false;
            for y in (min_y.max(chunk.min_y)..chunk.min_y + chunk.height).rev() {
                let block_id = chunk.get_block_at_y(local_x, y, local_z);
                match block_id {
                    GRASS_BLOCK => {
                        signal.grass_block_volume += 1;
                        if !top_recorded {
                            signal.grass_top_columns += 1;
                            top_recorded = true;
                        }
                    }
                    STONE => {
                        signal.stone_block_volume += 1;
                        if !top_recorded {
                            signal.stone_top_columns += 1;
                            top_recorded = true;
                        }
                    }
                    GRAVEL => {
                        signal.gravel_block_volume += 1;
                        if !top_recorded {
                            signal.gravel_top_columns += 1;
                            top_recorded = true;
                        }
                    }
                    block_id if !is_air_like(block_id) => {
                        top_recorded = true;
                    }
                    _ => {}
                }
            }
        }
    }
    signal
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SnowLayerSignal {
    snow_blocks: usize,
    ice_blocks: usize,
    snow_block_blocks: usize,
    snow_top_columns: usize,
    ice_top_columns: usize,
    snow_block_top_columns: usize,
}

fn snow_layer_signal(chunk: &GeneratedChunk) -> SnowLayerSignal {
    let mut signal = SnowLayerSignal {
        snow_blocks: chunk.block_count(SNOW),
        ice_blocks: chunk.block_count(ICE),
        snow_block_blocks: chunk.block_count(SNOW_BLOCK),
        ..SnowLayerSignal::default()
    };

    for local_z in 0..GeneratedChunk::WIDTH {
        for local_x in 0..GeneratedChunk::WIDTH {
            for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
                let block_id = chunk.block_at_y(local_x, y, local_z).raw();
                if is_air_like(block_id) {
                    continue;
                }
                match block_id {
                    SNOW => signal.snow_top_columns += 1,
                    ICE => signal.ice_top_columns += 1,
                    SNOW_BLOCK => signal.snow_block_top_columns += 1,
                    _ => {}
                }
                break;
            }
        }
    }

    signal
}

pub(super) fn surface_fixture_block_name_at(
    oracle: &TerrainChunkOracleFixture,
    local_x: i32,
    y: i32,
    local_z: i32,
) -> &str {
    let index = ((y - oracle.min_y) as usize) << 8 | (local_z as usize) << 4 | local_x as usize;
    let palette_index = oracle.blocks[index] as usize;
    &oracle.palette[palette_index]
}

pub(super) fn is_badlands_surface_block_name(block_name: &str) -> bool {
    block_name == "minecraft:red_sand" || block_name.ends_with("terracotta")
}

pub(super) fn compare_generated_chunk_to_full_fixture(
    actual: &GeneratedChunk,
    expected: &FullChunkEntryFixture,
) -> FullChunkDiffReport {
    let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);
    let actual_blocks = actual
        .blocks()
        .iter()
        .map(|block_id| crate::block::block_name(*block_id).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(actual_blocks.len(), expected_blocks.len());

    let mut mismatch_counts = BTreeMap::<(String, String), usize>::new();
    let mut first_mismatches = Vec::new();
    let mut matched_blocks = 0;

    for (index, (actual_name, expected_name)) in
        actual_blocks.iter().zip(expected_blocks.iter()).enumerate()
    {
        if actual_name == expected_name {
            matched_blocks += 1;
            continue;
        }

        *mismatch_counts
            .entry((actual_name.clone(), expected_name.clone()))
            .or_default() += 1;
        if first_mismatches.len() < 12 {
            first_mismatches.push(block_mismatch_at_index(
                index,
                actual.min_y,
                actual_name.clone(),
                expected_name.clone(),
            ));
        }
    }

    let mut top_mismatch_pairs = mismatch_counts
        .into_iter()
        .map(|((actual, expected), count)| MismatchBucket {
            actual,
            expected,
            count,
        })
        .collect::<Vec<_>>();
    top_mismatch_pairs.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.actual.cmp(&right.actual))
            .then_with(|| left.expected.cmp(&right.expected))
    });
    top_mismatch_pairs.truncate(16);

    FullChunkDiffReport {
        total_blocks: actual_blocks.len(),
        matched_blocks,
        mismatched_blocks: actual_blocks.len() - matched_blocks,
        top_mismatch_pairs,
        first_mismatches,
    }
}

pub(super) fn expand_full_fixture_blocks(
    fixture: &FullChunkEntryFixture,
    min_y: i32,
    height: i32,
) -> Vec<String> {
    let mut blocks = vec!["minecraft:air".to_owned(); height as usize * 16 * 16];

    for section in &fixture.sections {
        assert_eq!(section.block_order, "y-major,z-major,x-minor");
        assert_eq!(section.blocks.len(), 16 * 16 * 16);
        for (section_index, palette_index) in section.blocks.iter().copied().enumerate() {
            let local_y_in_section = section_index as i32 / 256;
            let within_layer = section_index as i32 % 256;
            let local_x = within_layer & 15;
            let local_z = (within_layer >> 4) & 15;
            let y = section.y * 16 + local_y_in_section;
            if !(min_y..min_y + height).contains(&y) {
                continue;
            }
            let block_name = section
                .palette
                .get(palette_index)
                .unwrap_or_else(|| panic!("palette index {palette_index} outside section palette"))
                .clone();
            let index = chunk_block_index(local_x, y - min_y, local_z);
            blocks[index] = block_name;
        }
    }

    blocks
}

pub(super) fn block_mismatch_at_index(
    index: usize,
    min_y: i32,
    actual: String,
    expected: String,
) -> BlockMismatch {
    BlockMismatch {
        local_x: (index & 15) as i32,
        y: ((index >> 8) as i32) + min_y,
        local_z: ((index >> 4) & 15) as i32,
        actual,
        expected,
    }
}

pub(super) fn assert_surface_chunk_matches_java_oracle(oracle: TerrainChunkOracleFixture) {
    assert_eq!(oracle.module, "surface-chunk");
    assert_eq!(oracle.minecraft_version, "1.17.1");
    assert_eq!(
        oracle.generator_class,
        "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
    );
    assert_eq!(oracle.min_y, 0);
    assert_eq!(oracle.height, 256);
    assert_eq!(oracle.block_order, "y-major,z-major,x-minor");

    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);

    assert_eq!(chunk.chunk_x, oracle.chunk_x);
    assert_eq!(chunk.chunk_z, oracle.chunk_z);
    assert_eq!(chunk.min_y, oracle.min_y);
    assert_eq!(chunk.height, oracle.height);
    let expected = oracle
        .blocks
        .iter()
        .copied()
        .map(RawBlockId::from)
        .collect::<Vec<_>>();
    assert_chunk_blocks_match(&chunk.blocks, &expected, oracle.min_y);
}

#[test]
pub(super) fn fixture_metadata_stays_consistent() {
    for fixture in fixtures() {
        assert_eq!(fixture.module, "noise");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(
            fixture.noise_class,
            "net.minecraft.world.level.levelgen.NoiseSampler"
        );
        assert_eq!(
            fixture.random_source_class,
            "net.minecraft.world.level.levelgen.WorldgenRandom"
        );
        assert_eq!(fixture.settings_preset, "overworld");
        assert_eq!(fixture.noise_modifier, "PASSTHROUGH");
        assert_eq!(fixture.cell_width, 4);
        assert_eq!(fixture.cell_height, 8);
        assert_eq!(fixture.cell_count_y, 32);
        assert_eq!(fixture.biome_y, 63);
        assert_eq!(fixture.min_cell_y, 0);
        assert_eq!(fixture.column_value_count, 33);
        assert_eq!(fixture.wire_format.coordinates, "integer");
        assert_eq!(fixture.wire_format.biome_keys, "string");
        assert_eq!(fixture.wire_format.biome_factors, "number");
        assert_eq!(fixture.wire_format.values, "number");
        assert!(fixture.noise_settings.use_simplex_surface_noise);
        assert!(fixture.noise_settings.random_density_offset);
        assert!(!fixture.noise_settings.island_noise_override);
        assert!(!fixture.noise_settings.is_amplified);

        for (name, sample_set) in &fixture.sample_sets {
            assert!(
                name == "constantPlains" || name == "mixedOverworld",
                "unexpected sample set {name}"
            );
            assert_eq!(sample_set.biome_pattern.grid_order, "x-major,z-minor");
            assert_eq!(
                sample_set.biome_pattern.sample_count,
                sample_set.biome_pattern.x.len() * sample_set.biome_pattern.z.len()
            );
            assert_eq!(
                sample_set.biome_pattern.keys.len(),
                sample_set.biome_pattern.sample_count
            );
            assert_eq!(
                sample_set.biome_pattern.depths.len(),
                sample_set.biome_pattern.sample_count
            );
            assert_eq!(
                sample_set.biome_pattern.scales.len(),
                sample_set.biome_pattern.sample_count
            );
            assert_eq!(
                sample_set.columns.noise_method,
                "fillNoiseColumn(noiseValues,cellX,cellZ,noiseSettings,biomeY,minCellY,cellCountY)"
            );
            assert_eq!(sample_set.columns.grid_order, "x-major,z-minor,y-minor");
            assert_eq!(
                sample_set.columns.column_value_count,
                fixture.column_value_count
            );
            assert_eq!(
                sample_set.columns.sample_count,
                sample_set.columns.x.len()
                    * sample_set.columns.z.len()
                    * sample_set.columns.column_value_count
            );
            assert_eq!(
                sample_set.columns.values.len(),
                sample_set.columns.sample_count
            );
        }
    }
}

#[test]
pub(super) fn default_overworld_settings_keep_dormant_caves_and_cliffs_flags_disabled() {
    let settings = NoiseGeneratorSettings::overworld();
    assert_eq!(settings.default_block(), STONE);
    assert_eq!(settings.default_fluid(), WATER);
    assert_eq!(settings.bedrock_floor_position(), 0);
    assert_eq!(settings.sea_level(), 63);
    assert!(!settings.is_aquifers_enabled());
    assert!(!settings.is_noise_caves_enabled());
    assert!(!settings.is_deepslate_enabled());
    assert!(!settings.is_ore_veins_enabled());
    assert!(!settings.is_noodle_caves_enabled());
}

#[test]
pub(super) fn terrain_only_oracle_fixture_stays_pinned_to_generator_target() {
    let oracle = terrain_fixture();
    assert_eq!(oracle.module, "terrain-chunk");
    assert_eq!(oracle.minecraft_version, "1.17.1");
    assert_eq!(
        oracle.generator_class,
        "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
    );
    assert_eq!(oracle.seed, "12345");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_eq!(oracle.min_y, 0);
    assert_eq!(oracle.height, 256);
    assert_eq!(oracle.block_order, "y-major,z-major,x-minor");
    assert_eq!(
        oracle.palette,
        [
            "minecraft:air",
            "minecraft:stone",
            "minecraft:water",
            "minecraft:bedrock",
        ]
    );
    assert_eq!(
        oracle.blocks.len(),
        CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 256
    );
}

#[test]
pub(super) fn fills_chunk_zero_zero_with_terrain_only_java_oracle() {
    let oracle = terrain_fixture();
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(
            oracle.seed.parse::<i64>().expect("i64 fixture seed"),
            false,
            false,
        ),
        oracle.seed.parse::<i64>().expect("i64 fixture seed"),
        NoiseGeneratorSettings::overworld(),
    );
    let chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    let expected = terrain_stage_blocks_from_oracle(&oracle);

    assert_eq!(chunk.chunk_x, oracle.chunk_x);
    assert_eq!(chunk.chunk_z, oracle.chunk_z);
    assert_eq!(chunk.min_y, oracle.min_y);
    assert_eq!(chunk.height, oracle.height);
    assert!(!chunk.blocks.contains(&RawBlockId::from(BEDROCK)));
    assert_eq!(chunk.blocks.len(), expected.len());

    for (index, (actual, expected)) in chunk.blocks.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            actual,
            expected,
            "terrain mismatch at local ({}, {}, {}): expected block id {}, got {}",
            index & 15,
            index >> 8,
            (index >> 4) & 15,
            expected,
            actual
        );
    }
}

#[test]
pub(super) fn fills_mountains_chunk_with_terrain_only_java_oracle() {
    let oracle = mountains_terrain_fixture();
    assert_eq!(oracle.seed, "33");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_mountains_relief_chunk_with_terrain_only_java_oracle() {
    let oracle = mountains_relief_terrain_fixture();
    assert_eq!(oracle.seed, "33");
    assert_eq!(oracle.chunk_x, -12);
    assert_eq!(oracle.chunk_z, 9);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_gravelly_mountains_chunk_with_terrain_only_java_oracle() {
    let oracle = gravelly_mountains_terrain_fixture();
    assert_eq!(oracle.seed, "250");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_gravelly_mountains_relief_chunk_with_terrain_only_java_oracle() {
    let oracle = gravelly_mountains_relief_terrain_fixture();
    assert_eq!(oracle.seed, "250");
    assert_eq!(oracle.chunk_x, -3);
    assert_eq!(oracle.chunk_z, 13);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_snowy_mountains_chunk_with_terrain_only_java_oracle() {
    let oracle = snowy_mountains_terrain_fixture();
    assert_eq!(oracle.seed, "326");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_snowy_mountains_relief_chunk_with_terrain_only_java_oracle() {
    let oracle = snowy_mountains_relief_terrain_fixture();
    assert_eq!(oracle.seed, "326");
    assert_eq!(oracle.chunk_x, 4);
    assert_eq!(oracle.chunk_z, -21);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_stone_shore_chunk_with_terrain_only_java_oracle() {
    let oracle = stone_shore_terrain_fixture();
    assert_eq!(oracle.seed, "74739");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_stone_shore_edge_chunk_with_terrain_only_java_oracle() {
    let oracle = stone_shore_edge_terrain_fixture();
    assert_eq!(oracle.seed, "74739");
    assert_eq!(oracle.chunk_x, 6);
    assert_eq!(oracle.chunk_z, 8);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_shattered_savanna_chunk_with_terrain_only_java_oracle() {
    let oracle = shattered_savanna_terrain_fixture();
    assert_eq!(oracle.seed, "68");
    assert_eq!(oracle.chunk_x, -6);
    assert_eq!(oracle.chunk_z, 0);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn fills_shattered_savanna_plateau_chunk_with_terrain_only_java_oracle() {
    let oracle = shattered_savanna_plateau_terrain_fixture();
    assert_eq!(oracle.seed, "153");
    assert_eq!(oracle.chunk_x, -8);
    assert_eq!(oracle.chunk_z, -2);
    assert_terrain_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn macro_geometry_signal_tracks_baseline_terrain_anchor() {
    let oracle = terrain_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    let signal = macro_geometry_signal(&chunk, |_| false, None, None);

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 86,
            top_y_max: 98,
            top_y_range: 12,
            top_y_p05: 86,
            top_y_p50: 88,
            top_y_p95: 93,
            neighbor_delta_ge_4: 0,
            neighbor_delta_ge_8: 0,
            neighbor_delta_ge_16: 0,
            vertical_face_columns: 0,
            solid_over_air_blocks: 0,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 0,
            landmark_column_count: 0,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_eroded_badlands_pillar_anchor() {
    let oracle = eroded_badlands_pillar_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let signal = macro_geometry_signal(&chunk, is_badlands_landmark_block_id, None, Some(80));

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 67,
            top_y_max: 122,
            top_y_range: 55,
            top_y_p05: 67,
            top_y_p50: 87,
            top_y_p95: 122,
            neighbor_delta_ge_4: 154,
            neighbor_delta_ge_8: 74,
            neighbor_delta_ge_16: 41,
            vertical_face_columns: 108,
            solid_over_air_blocks: 0,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 7023,
            landmark_column_count: 144,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_mountains_relief_anchor() {
    let oracle = mountains_relief_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let signal = macro_geometry_signal(
        &chunk,
        is_mountains_landmark_block_id,
        Some(NoiseGeneratorSettings::overworld().sea_level()),
        Some(NoiseGeneratorSettings::overworld().sea_level()),
    );

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 66,
            top_y_max: 122,
            top_y_range: 56,
            top_y_p05: 71,
            top_y_p50: 113,
            top_y_p95: 122,
            neighbor_delta_ge_4: 101,
            neighbor_delta_ge_8: 16,
            neighbor_delta_ge_16: 3,
            vertical_face_columns: 94,
            solid_over_air_blocks: 0,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 11483,
            landmark_column_count: 256,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_gravelly_mountains_relief_anchor() {
    let oracle = gravelly_mountains_relief_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let sea_level = NoiseGeneratorSettings::overworld().sea_level();
    let signal = macro_geometry_signal(
        &chunk,
        is_mountains_landmark_block_id,
        Some(sea_level),
        Some(sea_level),
    );
    let material_signal = surface_material_signal(&chunk, sea_level);

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 62,
            top_y_max: 111,
            top_y_range: 49,
            top_y_p05: 62,
            top_y_p50: 103,
            top_y_p95: 110,
            neighbor_delta_ge_4: 28,
            neighbor_delta_ge_8: 25,
            neighbor_delta_ge_16: 25,
            vertical_face_columns: 42,
            solid_over_air_blocks: 176,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 2,
            water_land_edge_delta_ge_8: 2,
            landmark_block_volume: 3972,
            landmark_column_count: 234,
        }
    );
    assert_eq!(
        material_signal,
        SurfaceMaterialSignal {
            grass_top_columns: 25,
            stone_top_columns: 134,
            gravel_top_columns: 75,
            grass_block_volume: 50,
            stone_block_volume: 3552,
            gravel_block_volume: 370,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_snowy_mountains_relief_anchor() {
    let oracle = snowy_mountains_relief_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let sea_level = NoiseGeneratorSettings::overworld().sea_level();
    let signal = macro_geometry_signal(
        &chunk,
        is_mountains_landmark_block_id,
        Some(sea_level),
        Some(sea_level),
    );
    let material_signal = surface_material_signal(&chunk, sea_level);
    let features_chunk = generate_overworld_features_chunk(seed, oracle.chunk_x, oracle.chunk_z);
    let snow_signal = snow_layer_signal(&features_chunk);

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 69,
            top_y_max: 95,
            top_y_range: 26,
            top_y_p05: 70,
            top_y_p50: 86,
            top_y_p95: 95,
            neighbor_delta_ge_4: 59,
            neighbor_delta_ge_8: 12,
            neighbor_delta_ge_16: 0,
            vertical_face_columns: 51,
            solid_over_air_blocks: 0,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 4578,
            landmark_column_count: 256,
        }
    );
    assert_eq!(
        material_signal,
        SurfaceMaterialSignal {
            grass_top_columns: 256,
            stone_top_columns: 0,
            gravel_top_columns: 0,
            grass_block_volume: 256,
            stone_block_volume: 4322,
            gravel_block_volume: 0,
        }
    );
    assert_eq!(
        snow_signal,
        SnowLayerSignal {
            snow_blocks: 256,
            ice_blocks: 0,
            snow_block_blocks: 0,
            snow_top_columns: 256,
            ice_top_columns: 0,
            snow_block_top_columns: 0,
        }
    );
}

#[test]
pub(super) fn snow_layer_signal_tracks_snowy_mountains_palette_anchor() {
    let oracle = snowy_mountains_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let chunk = generate_overworld_features_chunk(seed, oracle.chunk_x, oracle.chunk_z);

    assert_eq!(
        snow_layer_signal(&chunk),
        SnowLayerSignal {
            snow_blocks: 251,
            ice_blocks: 39,
            snow_block_blocks: 0,
            snow_top_columns: 251,
            ice_top_columns: 0,
            snow_block_top_columns: 0,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_stone_shore_steep_coast_anchor() {
    let oracle = stone_shore_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let signal = macro_geometry_signal(
        &chunk,
        is_stone_shore_landmark_block_id,
        Some(NoiseGeneratorSettings::overworld().sea_level()),
        Some(NoiseGeneratorSettings::overworld().sea_level()),
    );

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 62,
            top_y_max: 78,
            top_y_range: 16,
            top_y_p05: 62,
            top_y_p50: 65,
            top_y_p95: 76,
            neighbor_delta_ge_4: 11,
            neighbor_delta_ge_8: 0,
            neighbor_delta_ge_16: 0,
            vertical_face_columns: 9,
            solid_over_air_blocks: 0,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 1163,
            landmark_column_count: 137,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_stone_shore_water_edge_anchor() {
    let oracle = stone_shore_edge_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let signal = macro_geometry_signal(
        &chunk,
        is_stone_shore_landmark_block_id,
        Some(NoiseGeneratorSettings::overworld().sea_level()),
        Some(NoiseGeneratorSettings::overworld().sea_level()),
    );

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 62,
            top_y_max: 83,
            top_y_range: 21,
            top_y_p05: 62,
            top_y_p50: 62,
            top_y_p95: 81,
            neighbor_delta_ge_4: 69,
            neighbor_delta_ge_8: 58,
            neighbor_delta_ge_16: 23,
            vertical_face_columns: 35,
            solid_over_air_blocks: 117,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 57,
            water_land_edge_delta_ge_8: 57,
            landmark_block_volume: 798,
            landmark_column_count: 118,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_shattered_savanna_relief_anchor() {
    let oracle = shattered_savanna_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let signal = macro_geometry_signal(
        &chunk,
        is_shattered_savanna_landmark_block_id,
        Some(NoiseGeneratorSettings::overworld().sea_level()),
        Some(NoiseGeneratorSettings::overworld().sea_level()),
    );

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 62,
            top_y_max: 80,
            top_y_range: 18,
            top_y_p05: 62,
            top_y_p50: 67,
            top_y_p95: 77,
            neighbor_delta_ge_4: 9,
            neighbor_delta_ge_8: 0,
            neighbor_delta_ge_16: 0,
            vertical_face_columns: 8,
            solid_over_air_blocks: 0,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 699,
            landmark_column_count: 126,
        }
    );
}

#[test]
pub(super) fn macro_geometry_signal_tracks_shattered_savanna_plateau_relief_anchor() {
    let oracle = shattered_savanna_plateau_surface_fixture();
    let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    let signal = macro_geometry_signal(
        &chunk,
        is_shattered_savanna_landmark_block_id,
        Some(NoiseGeneratorSettings::overworld().sea_level()),
        Some(NoiseGeneratorSettings::overworld().sea_level()),
    );

    assert_eq!(
        signal,
        MacroGeometrySignal {
            top_y_min: 65,
            top_y_max: 155,
            top_y_range: 90,
            top_y_p05: 73,
            top_y_p50: 84,
            top_y_p95: 152,
            neighbor_delta_ge_4: 61,
            neighbor_delta_ge_8: 34,
            neighbor_delta_ge_16: 13,
            vertical_face_columns: 46,
            solid_over_air_blocks: 22,
            surface_near_carved_air_columns: 0,
            carved_air_volume: 0,
            carved_air_y_min: None,
            carved_air_y_max: None,
            long_vertical_air_spans: 0,
            water_land_edge_delta_ge_4: 0,
            water_land_edge_delta_ge_8: 0,
            landmark_block_volume: 5447,
            landmark_column_count: 256,
        }
    );
}

#[test]
pub(super) fn build_surface_and_bedrock_matches_java_oracle() {
    let oracle = surface_fixture();
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_mountains_surface_and_bedrock_matches_java_oracle() {
    let oracle = mountains_surface_fixture();
    assert_eq!(oracle.seed, "33");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_mountains_relief_surface_and_bedrock_matches_java_oracle() {
    let oracle = mountains_relief_surface_fixture();
    assert_eq!(oracle.seed, "33");
    assert_eq!(oracle.chunk_x, -12);
    assert_eq!(oracle.chunk_z, 9);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_gravelly_mountains_surface_and_bedrock_matches_java_oracle() {
    let oracle = gravelly_mountains_surface_fixture();
    assert_eq!(oracle.seed, "250");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_gravelly_mountains_relief_surface_and_bedrock_matches_java_oracle() {
    let oracle = gravelly_mountains_relief_surface_fixture();
    assert_eq!(oracle.seed, "250");
    assert_eq!(oracle.chunk_x, -3);
    assert_eq!(oracle.chunk_z, 13);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_snowy_mountains_surface_and_bedrock_matches_java_oracle() {
    let oracle = snowy_mountains_surface_fixture();
    assert_eq!(oracle.seed, "326");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_snowy_mountains_relief_surface_and_bedrock_matches_java_oracle() {
    let oracle = snowy_mountains_relief_surface_fixture();
    assert_eq!(oracle.seed, "326");
    assert_eq!(oracle.chunk_x, 4);
    assert_eq!(oracle.chunk_z, -21);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_stone_shore_surface_and_bedrock_matches_java_oracle() {
    let oracle = stone_shore_surface_fixture();
    assert_eq!(oracle.seed, "74739");
    assert_eq!(oracle.chunk_x, 0);
    assert_eq!(oracle.chunk_z, 0);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_stone_shore_edge_surface_and_bedrock_matches_java_oracle() {
    let oracle = stone_shore_edge_surface_fixture();
    assert_eq!(oracle.seed, "74739");
    assert_eq!(oracle.chunk_x, 6);
    assert_eq!(oracle.chunk_z, 8);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_shattered_savanna_surface_and_bedrock_matches_java_oracle() {
    let oracle = shattered_savanna_surface_fixture();
    assert_eq!(oracle.seed, "68");
    assert_eq!(oracle.chunk_x, -6);
    assert_eq!(oracle.chunk_z, 0);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_shattered_savanna_plateau_surface_and_bedrock_matches_java_oracle() {
    let oracle = shattered_savanna_plateau_surface_fixture();
    assert_eq!(oracle.seed, "153");
    assert_eq!(oracle.chunk_x, -8);
    assert_eq!(oracle.chunk_z, -2);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_frozen_ocean_surface_and_bedrock_matches_java_oracle() {
    let oracle = frozen_ocean_surface_fixture();
    assert_eq!(oracle.chunk_x, -247);
    assert_eq!(oracle.chunk_z, -247);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_badlands_surface_and_bedrock_matches_java_oracle() {
    let oracle = badlands_surface_fixture();
    assert_eq!(oracle.chunk_x, -320);
    assert_eq!(oracle.chunk_z, 99);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn build_eroded_badlands_pillar_surface_and_bedrock_matches_java_oracle() {
    let oracle = eroded_badlands_pillar_surface_fixture();
    assert_eq!(oracle.seed, "868");
    assert_eq!(oracle.chunk_x, 8);
    assert_eq!(oracle.chunk_z, -6);
    let signal = surface_top_signal(&oracle);
    assert_eq!(signal.max_top_y, 122);
    assert!(signal.columns_above_100 >= 70, "{signal:?}");
    assert_eq!(signal.badlands_top_columns, 256);
    assert_surface_chunk_matches_java_oracle(oracle);
}

#[test]
pub(super) fn generated_chunk_wraps_surface_buffer_for_render_consumers() {
    let chunk = generate_overworld_surface_chunk(12345, 0, 0);

    assert_eq!(chunk.chunk_x, 0);
    assert_eq!(chunk.chunk_z, 0);
    assert_eq!(chunk.min_y, 0);
    assert_eq!(chunk.height, 256);
    assert_eq!(
        chunk.blocks().len(),
        CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 256
    );
    assert!(chunk.non_air_block_count() > 0);
    assert_eq!(chunk.block_at_local(0, 0, 0).name(), "minecraft:bedrock");
}
