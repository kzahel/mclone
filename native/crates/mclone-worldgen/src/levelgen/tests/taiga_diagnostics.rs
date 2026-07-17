use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TaigaTreeIndexShiftCenterDiagnostic {
    pub(super) center: ChunkPos,
    pub(super) biome_key: &'static str,
    pub(super) current_tree_blocks: usize,
    pub(super) java_tree_blocks: usize,
    pub(super) shared_tree_blocks: usize,
    pub(super) current_only_tree_blocks: usize,
    pub(super) java_only_tree_blocks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TaigaFullTableTreeDeltaDiagnostic {
    pub(super) center: ChunkPos,
    pub(super) biome_key: &'static str,
    pub(super) before_tree_blocks: usize,
    pub(super) after_tree_blocks: usize,
    pub(super) added_tree_blocks: usize,
    pub(super) removed_tree_blocks: usize,
    pub(super) feature_random_calls: usize,
    pub(super) random_count: usize,
    pub(super) added_tree_block_samples: Vec<TreeBlockSample>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TreeBlockSample {
    pub(super) local_x: i32,
    pub(super) y: i32,
    pub(super) local_z: i32,
    pub(super) block_id: RawBlockId,
}

pub(super) fn chunk_primary_biome_key(
    biome_source: &OverworldBiomeSource,
    pos: ChunkPos,
) -> &'static str {
    biome_source
        .get_primary_biome_definition(pos.x, pos.z)
        .key()
}

pub(super) fn is_taiga_vegetation_biome(key: &str) -> bool {
    matches!(
        key,
        "minecraft:taiga"
            | "minecraft:taiga_hills"
            | "minecraft:taiga_mountains"
            | "minecraft:giant_tree_taiga"
            | "minecraft:giant_tree_taiga_hills"
            | "minecraft:giant_spruce_taiga"
            | "minecraft:giant_spruce_taiga_hills"
    )
}

pub(super) fn target_tree_blocks_after_taiga_vegetation_center(
    seed: i64,
    target: ChunkPos,
    center: ChunkPos,
    feature_index: i32,
) -> BTreeMap<(i32, i32, i32), RawBlockId> {
    let biome_source = OverworldBiomeSource::new(seed, false, false);
    let chunks =
        (center.z - FEATURES_WRITE_RADIUS_CUTOFF..=center.z + FEATURES_WRITE_RADIUS_CUTOFF)
            .flat_map(|chunk_z| {
                let biome_source = biome_source.clone();
                (center.x - FEATURES_WRITE_RADIUS_CUTOFF..=center.x + FEATURES_WRITE_RADIUS_CUTOFF)
                    .map(move |chunk_x| {
                        generate_overworld_liquid_carved_buffer_with_biome_source(
                            seed,
                            chunk_x,
                            chunk_z,
                            biome_source.clone(),
                        )
                    })
            })
            .collect::<Vec<_>>();
    let mut region = FeatureRegion::with_radii(
        center.x,
        center.z,
        FEATURES_WRITE_RADIUS_CUTOFF,
        FEATURES_WRITE_RADIUS_CUTOFF,
        chunks,
    );

    crate::feature::test_support::place_taiga_vegetation_with_feature_index(
        seed,
        &mut region,
        feature_index,
    );
    let chunk = region
        .remove_chunk(target.x, target.z)
        .expect("target chunk should be inside center write window");
    tree_blocks_in_chunk(&chunk)
}

pub(super) fn tree_blocks_in_chunk(
    chunk: &MutableChunkBlockBuffer,
) -> BTreeMap<(i32, i32, i32), RawBlockId> {
    let mut blocks = BTreeMap::new();
    for y in chunk.min_y..chunk.min_y + chunk.height {
        for z in 0..CHUNK_WIDTH {
            for x in 0..CHUNK_WIDTH {
                let block_id = chunk.get_block_at_y(x, y, z);
                if matches!(
                    block_id,
                    crate::block::SPRUCE_LOG | crate::block::SPRUCE_LEAVES
                ) {
                    blocks.insert((x, y, z), block_id);
                }
            }
        }
    }
    blocks
}

pub(super) fn tree_blocks_in_target_region(
    region: &FeatureRegion,
    target: ChunkPos,
) -> BTreeMap<(i32, i32, i32), RawBlockId> {
    tree_blocks_in_chunk(
        region
            .chunk(target.x, target.z)
            .expect("target chunk should be inside feature region"),
    )
}

pub(super) fn tree_block_delta(
    before: &BTreeMap<(i32, i32, i32), RawBlockId>,
    after: &BTreeMap<(i32, i32, i32), RawBlockId>,
) -> (usize, usize) {
    let added = after
        .iter()
        .filter(|(pos, block_id)| before.get(pos) != Some(block_id))
        .count();
    let removed = before
        .iter()
        .filter(|(pos, block_id)| after.get(pos) != Some(block_id))
        .count();
    (added, removed)
}

pub(super) fn added_tree_block_samples(
    before: &BTreeMap<(i32, i32, i32), RawBlockId>,
    after: &BTreeMap<(i32, i32, i32), RawBlockId>,
    limit: usize,
) -> Vec<TreeBlockSample> {
    let added = after
        .iter()
        .filter(|(pos, block_id)| before.get(pos) != Some(block_id))
        .collect::<Vec<_>>();
    if added.len() > limit {
        return Vec::new();
    }

    added
        .into_iter()
        .map(|(&(local_x, y, local_z), &block_id)| TreeBlockSample {
            local_x,
            y,
            local_z,
            block_id,
        })
        .collect()
}

pub(super) fn taiga_tree_index_shift_diagnostics(
    seed: i64,
    target: ChunkPos,
) -> Vec<TaigaTreeIndexShiftCenterDiagnostic> {
    let biome_source = OverworldBiomeSource::new(seed, false, false);
    sorted_chunk_positions_z_major(
        ChunkGenerationPlan::overworld_features([target])
            .backend_work_chunks()
            .iter()
            .copied(),
    )
    .into_iter()
    .filter_map(|center| {
        let biome_key = chunk_primary_biome_key(&biome_source, center);
        if !is_taiga_vegetation_biome(biome_key) {
            return None;
        }

        let current = target_tree_blocks_after_taiga_vegetation_center(
            seed,
            target,
            center,
            crate::feature::test_support::CURRENT_TAIGA_VEGETATION_FEATURE_INDEX,
        );
        let java = target_tree_blocks_after_taiga_vegetation_center(
            seed,
            target,
            center,
            crate::feature::test_support::JAVA_TAIGA_VEGETATION_FEATURE_INDEX,
        );
        let shared_tree_blocks = current
            .iter()
            .filter(|(pos, block_id)| java.get(pos) == Some(block_id))
            .count();
        Some(TaigaTreeIndexShiftCenterDiagnostic {
            center,
            biome_key,
            current_tree_blocks: current.len(),
            java_tree_blocks: java.len(),
            shared_tree_blocks,
            current_only_tree_blocks: current.len() - shared_tree_blocks,
            java_only_tree_blocks: java.len() - shared_tree_blocks,
        })
    })
    .collect()
}

pub(super) fn taiga_full_table_tree_delta_diagnostics(
    seed: i64,
    target: ChunkPos,
) -> Vec<TaigaFullTableTreeDeltaDiagnostic> {
    let biome_source = OverworldBiomeSource::new(seed, false, false);
    let plan = ChunkGenerationPlan::overworld_features([target]);
    let chunks = sorted_chunk_positions_z_major(
        plan.prerequisites()
            .iter()
            .map(|requirement| requirement.pos),
    )
    .into_iter()
    .map(|pos| {
        generate_overworld_liquid_carved_buffer_with_biome_source(
            seed,
            pos.x,
            pos.z,
            biome_source.clone(),
        )
    })
    .collect::<Vec<_>>();
    let first_target = *plan
        .output_chunks()
        .iter()
        .next()
        .expect("non-empty target plan");
    let mut region = FeatureRegion::new(first_target.x, first_target.z, chunks);
    let feature_biomes = crate::feature::OverworldFeatureBiomeResolver::new(seed, &biome_source);
    let mut diagnostics = Vec::new();

    for center in sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied()) {
        region.set_center(center.x, center.z);
        let biome = biome_source.get_primary_biome_definition(center.x, center.z);
        let biome_key = biome.key();
        let min_block_x = chunk_min_block_coord(center.x);
        let min_block_z = chunk_min_block_coord(center.z);
        let origin = crate::placement::BlockPos::new(min_block_x, region.min_y(), min_block_z);
        let features = crate::feature::overworld_features_for_biome(biome);
        let mut random = WorldgenRandom::default();
        let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);

        for step_index in 0..=crate::feature::DecorationStep::TopLayerModification.index() {
            let mut feature_index = 0;
            for feature in features
                .iter()
                .filter(|feature| feature.step.index() == step_index)
            {
                random.set_feature_seed(decoration_seed, feature_index, step_index);
                let capture = is_taiga_vegetation_biome(biome_key)
                    && step_index == crate::feature::DecorationStep::VegetalDecoration.index()
                    && feature_index
                        == crate::feature::test_support::JAVA_TAIGA_VEGETATION_FEATURE_INDEX;
                let random_count_before_feature = random.get_count();
                let before = capture.then(|| tree_blocks_in_target_region(&region, target));
                feature.place_with_biomes(&mut region, &feature_biomes, &mut random, origin);
                if let Some(before) = before {
                    let after = tree_blocks_in_target_region(&region, target);
                    let (added_tree_blocks, removed_tree_blocks) =
                        tree_block_delta(&before, &after);
                    let random_count = random.get_count();
                    diagnostics.push(TaigaFullTableTreeDeltaDiagnostic {
                        center,
                        biome_key,
                        before_tree_blocks: before.len(),
                        after_tree_blocks: after.len(),
                        added_tree_blocks,
                        removed_tree_blocks,
                        feature_random_calls: random_count - random_count_before_feature,
                        random_count,
                        added_tree_block_samples: added_tree_block_samples(&before, &after, 20),
                    });
                }
                feature_index += 1;
            }
        }
    }

    diagnostics
}
