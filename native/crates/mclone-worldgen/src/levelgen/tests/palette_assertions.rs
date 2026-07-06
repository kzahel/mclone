use super::*;

pub(super) fn count_top_surface_family(chunk: &GeneratedChunk, family: SurfaceFamily) -> usize {
    let mut count = 0;
    for local_z in 0..GeneratedChunk::WIDTH {
        for local_x in 0..GeneratedChunk::WIDTH {
            if top_non_air_block(chunk, local_x, local_z)
                .is_some_and(|block| family.contains(block))
            {
                count += 1;
            }
        }
    }
    count
}

pub(super) fn top_surface_histogram(chunk: &GeneratedChunk) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for local_z in 0..GeneratedChunk::WIDTH {
        for local_x in 0..GeneratedChunk::WIDTH {
            if let Some(block) = top_non_air_block(chunk, local_x, local_z) {
                *counts.entry(crate::block::block_name(block)).or_insert(0) += 1;
            }
        }
    }
    counts
}

pub(super) fn feature_block_counts(
    chunk: &GeneratedChunk,
    family: FeatureFamily,
) -> BTreeMap<&'static str, usize> {
    family
        .blocks()
        .iter()
        .map(|block| (crate::block::block_name(*block), chunk.block_count(*block)))
        .collect()
}

pub(super) fn count_block_position_biomes_in_chunk(
    biome_source: &OverworldBiomeSource,
    seed: i64,
    case: &PaletteMatrixCase,
) -> usize {
    let min_x = chunk_min_block_coord(case.chunk_x);
    let min_z = chunk_min_block_coord(case.chunk_z);
    let mut count = 0;
    for local_z in 0..GeneratedChunk::WIDTH {
        for local_x in 0..GeneratedChunk::WIDTH {
            let world_x = min_x + local_x;
            let world_z = min_z + local_z;
            if biome_source
                .get_block_position_biome_definition(seed, world_x, world_z)
                .key()
                == case.biome_key
            {
                count += 1;
            }
        }
    }
    count
}

pub(super) fn exposed_spring_tick_block_count(chunk: &GeneratedChunk, fluid: RawBlockId) -> usize {
    let target = match fluid {
        WATER => "minecraft:water",
        LAVA => "minecraft:lava",
        other => panic!(
            "unexpected spring fluid block {}",
            crate::block::block_name(other)
        ),
    };
    chunk
        .liquid_ticks()
        .iter()
        .filter(|tick| {
            tick.target == target
                && block_at_tick_position(chunk, tick) == Some(fluid)
                && has_air_like_neighbor_at_tick_position(chunk, tick)
        })
        .count()
}

pub(super) fn block_at_tick_position(
    chunk: &GeneratedChunk,
    tick: &crate::levelgen::ScheduledTick,
) -> Option<RawBlockId> {
    block_at_world_position(chunk, tick.x, tick.y, tick.z)
}

pub(super) fn block_at_world_position(
    chunk: &GeneratedChunk,
    world_x: i32,
    y: i32,
    world_z: i32,
) -> Option<RawBlockId> {
    let local_x = world_x - chunk_min_block_coord(chunk.chunk_x);
    let local_z = world_z - chunk_min_block_coord(chunk.chunk_z);
    if !(0..GeneratedChunk::WIDTH).contains(&local_x)
        || !(chunk.min_y..chunk.min_y + chunk.height).contains(&y)
        || !(0..GeneratedChunk::WIDTH).contains(&local_z)
    {
        return None;
    }
    Some(chunk.block_at_y(local_x, y, local_z).raw())
}

pub(super) fn has_air_like_neighbor_at_tick_position(
    chunk: &GeneratedChunk,
    tick: &crate::levelgen::ScheduledTick,
) -> bool {
    const SPRING_VISIBLE_DIRECTIONS: [(i32, i32, i32); 5] =
        [(-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1), (0, -1, 0)];
    SPRING_VISIBLE_DIRECTIONS.iter().any(|(dx, dy, dz)| {
        block_at_world_position(chunk, tick.x + dx, tick.y + dy, tick.z + dz)
            .is_some_and(is_air_like)
    })
}

pub(super) fn pumpkins_on_grass_count(chunk: &GeneratedChunk) -> usize {
    let mut count = 0;
    for y in chunk.min_y + 1..chunk.min_y + chunk.height {
        for local_z in 0..GeneratedChunk::WIDTH {
            for local_x in 0..GeneratedChunk::WIDTH {
                if chunk.block_at_y(local_x, y, local_z).raw() == PUMPKIN
                    && chunk.block_at_y(local_x, y - 1, local_z).raw() == GRASS_BLOCK
                {
                    count += 1;
                }
            }
        }
    }
    count
}

pub(super) fn jungle_bush_shape_count(chunk: &GeneratedChunk) -> usize {
    let mut matches = 0;
    for y in chunk.min_y..chunk.min_y + chunk.height - 1 {
        for local_z in 2..GeneratedChunk::WIDTH - 2 {
            for local_x in 2..GeneratedChunk::WIDTH - 2 {
                if chunk.block_at_y(local_x, y, local_z).raw() != JUNGLE_LOG {
                    continue;
                }

                let bottom_skirt = [
                    (local_x - 2, local_z),
                    (local_x + 2, local_z),
                    (local_x, local_z - 2),
                    (local_x, local_z + 2),
                ]
                .iter()
                .filter(|(x, z)| chunk.block_at_y(*x, y, *z).raw() == OAK_LEAVES)
                .count();
                let middle_leaves = oak_leaf_count_around(chunk, local_x, y + 1, local_z, 1);

                if bottom_skirt >= 2 && middle_leaves >= 5 {
                    matches += 1;
                }
            }
        }
    }
    matches
}

pub(super) fn oak_leaf_count_around(
    chunk: &GeneratedChunk,
    center_x: i32,
    y: i32,
    center_z: i32,
    radius: i32,
) -> usize {
    let mut count = 0;
    for local_z in center_z - radius..=center_z + radius {
        for local_x in center_x - radius..=center_x + radius {
            if chunk.block_at_y(local_x, y, local_z).raw() == OAK_LEAVES {
                count += 1;
            }
        }
    }
    count
}

pub(super) fn top_non_air_block(
    chunk: &GeneratedChunk,
    local_x: i32,
    local_z: i32,
) -> Option<RawBlockId> {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        let block = chunk.block_at_y(local_x, y, local_z).raw();
        if !is_air_like(block) {
            return Some(block);
        }
    }
    None
}

pub(super) fn has_two_by_two_log_square(chunk: &GeneratedChunk, log: RawBlockId) -> bool {
    for y in chunk.min_y..chunk.min_y + chunk.height {
        for local_z in 0..GeneratedChunk::WIDTH - 1 {
            for local_x in 0..GeneratedChunk::WIDTH - 1 {
                if chunk.block_at_y(local_x, y, local_z).raw() == log
                    && chunk.block_at_y(local_x + 1, y, local_z).raw() == log
                    && chunk.block_at_y(local_x, y, local_z + 1).raw() == log
                    && chunk.block_at_y(local_x + 1, y, local_z + 1).raw() == log
                {
                    return true;
                }
            }
        }
    }
    false
}
