//! Provisional ("seed") lighting computed at worldgen publication time.
//!
//! Thin orchestration home for initial light sections generated at chunk
//! publication time before `ChunkStatus::Light` exists. The actual graph-backed
//! sky/block propagation lives in the layer-specific bridge modules.

use std::collections::BTreeMap;

use crate::block_light_bridge::graph_block_light_sections_for_chunk;
use crate::sky_light_bridge::graph_sky_light_sections_for_chunk;
use mclone_core::{CHUNK_WIDTH, ChunkPos, PackedLightSection};
use mclone_worldgen::block::RawBlockId;

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
    let sky_sections =
        provisional_sky_light_sections_for_chunk(target_pos, min_y, height, chunks.iter().copied());
    let block_sections = provisional_block_light_sections_for_chunk(
        target_pos,
        min_y,
        height,
        chunks.iter().copied(),
    );
    merge_light_sections(sky_sections, block_sections)
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

pub(crate) fn provisional_sky_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    graph_sky_light_sections_for_chunk(target_pos, min_y, height, chunks)
}

pub(crate) fn provisional_block_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    graph_block_light_sections_for_chunk(target_pos, min_y, height, chunks)
}

fn merge_light_sections(
    sky_sections: Vec<PackedLightSection>,
    block_sections: Vec<PackedLightSection>,
) -> Vec<PackedLightSection> {
    let mut sections = BTreeMap::<i32, (Option<Vec<u8>>, Option<Vec<u8>>)>::new();
    for section in sky_sections.into_iter().chain(block_sections) {
        let entry = sections.entry(section.section_y).or_default();
        if section.sky.is_some() {
            entry.0 = section.sky;
        }
        if section.block.is_some() {
            entry.1 = section.block;
        }
    }

    sections
        .into_iter()
        .map(|(section_y, (sky, block))| PackedLightSection::new(section_y, sky, block))
        .collect()
}

pub(crate) fn provisional_sky_light_includes_chunk(
    target_pos: ChunkPos,
    chunk_pos: ChunkPos,
) -> bool {
    (chunk_pos.x - target_pos.x).abs() <= 1 && (chunk_pos.z - target_pos.z).abs() <= 1
}
