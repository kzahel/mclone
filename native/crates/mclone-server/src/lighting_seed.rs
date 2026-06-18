//! Provisional ("seed") lighting computed at worldgen publication time.
//!
//! Thin orchestration home for initial light sections generated at chunk
//! publication time before `ChunkStatus::Light` exists. The actual graph-backed
//! propagation lives in Java-shaped light bridge modules.

use crate::level_light_bridge::graph_level_light_sections_for_chunk;
use mclone_core::{CHUNK_WIDTH, ChunkPos, PackedLightSection};
use mclone_worldgen::block::RawBlockId;

#[cfg(test)]
use crate::block_light_bridge::graph_block_light_sections_for_chunk;
#[cfg(test)]
use crate::sky_light_bridge::graph_sky_light_sections_for_chunk;
#[cfg(test)]
use mclone_core::ChunkSnapshot;

#[cfg(test)]
pub(crate) fn snapshot_with_provisional_lighting(
    snapshot: ChunkSnapshot,
    raw_blocks: &[RawBlockId],
) -> ChunkSnapshot {
    snapshot_with_provisional_lighting_from_neighbors(snapshot, raw_blocks, std::iter::empty())
}

#[cfg(test)]
pub(crate) fn snapshot_with_provisional_lighting_from_neighbors<'a>(
    snapshot: ChunkSnapshot,
    raw_blocks: &'a [RawBlockId],
    neighbor_blocks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> ChunkSnapshot {
    let light_sections = provisional_light_sections_from_neighbors(
        snapshot.pos,
        snapshot.min_y,
        snapshot.height,
        raw_blocks,
        neighbor_blocks,
    );
    snapshot.with_light_sections(false, light_sections)
}

pub(crate) fn provisional_light_sections_from_neighbors<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    raw_blocks: &'a [RawBlockId],
    neighbor_blocks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    assert_eq!(
        raw_blocks.len(),
        expected_len,
        "chunk {:?} lighting input had {} blocks instead of {expected_len}",
        target_pos,
        raw_blocks.len()
    );
    let mut chunks = neighbor_blocks.into_iter().collect::<Vec<_>>();
    chunks.push((target_pos, raw_blocks));
    graph_level_light_sections_for_chunk(target_pos, min_y, height, chunks)
}

#[cfg(test)]
pub(crate) fn provisional_sky_light_sections(
    min_y: i32,
    height: i32,
    raw_blocks: &[RawBlockId],
) -> Vec<PackedLightSection> {
    provisional_sky_light_sections_for_chunk(
        ChunkPos::new(0, 0),
        min_y,
        height,
        std::iter::once((ChunkPos::new(0, 0), raw_blocks)),
    )
}

#[cfg(test)]
pub(crate) fn provisional_sky_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    graph_sky_light_sections_for_chunk(target_pos, min_y, height, chunks)
}

#[cfg(test)]
pub(crate) fn provisional_block_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    graph_block_light_sections_for_chunk(target_pos, min_y, height, chunks)
}

pub(crate) fn provisional_sky_light_includes_chunk(
    target_pos: ChunkPos,
    chunk_pos: ChunkPos,
) -> bool {
    (chunk_pos.x - target_pos.x).abs() <= 1 && (chunk_pos.z - target_pos.z).abs() <= 1
}
