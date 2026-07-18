//! Initial lighting bridge helpers.
//!
//! Thin orchestration home for initial light sections used by the native
//! `ChunkStatus::Light` bridge. The actual graph-backed propagation lives in
//! Java-shaped light bridge modules.

#[cfg(test)]
use mclone_core::{CHUNK_WIDTH, PackedLightSection};
use mclone_core::{ChunkPos, HorizontalTopology, Vec3d, chunk_middle_block_coord};
#[cfg(test)]
use mclone_worldgen::block::RawBlockId;

#[cfg(test)]
use crate::block_light_bridge::graph_block_light_sections_for_chunk;
#[cfg(test)]
use crate::level_light_bridge::graph_level_light_sections_for_chunk;
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

#[cfg(test)]
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

pub(crate) fn provisional_light_neighbor_lift(
    topology: HorizontalTopology,
    target: ChunkPos,
    canonical_neighbor: ChunkPos,
) -> Option<ChunkPos> {
    if canonical_neighbor == target {
        return None;
    }
    let lifted = topology.nearest_chunk_lift(
        canonical_neighbor,
        Vec3d::new(
            f64::from(chunk_middle_block_coord(target.x)),
            0.0,
            f64::from(chunk_middle_block_coord(target.z)),
        ),
    );
    let lifted = ChunkPos::new(i32::try_from(lifted.x).ok()?, i32::try_from(lifted.z).ok()?);
    provisional_sky_light_includes_chunk(target, lifted).then_some(lifted)
}

#[cfg(test)]
mod topology_tests {
    use super::*;

    #[test]
    fn cylinder_light_neighbors_unfold_to_the_adjacent_seam_column() {
        let topology = HorizontalTopology::cylinder_x(0, 32);

        assert_eq!(
            provisional_light_neighbor_lift(topology, ChunkPos::new(0, 0), ChunkPos::new(31, 0),),
            Some(ChunkPos::new(-1, 0))
        );
        assert_eq!(
            provisional_light_neighbor_lift(topology, ChunkPos::new(31, 0), ChunkPos::new(0, 0),),
            Some(ChunkPos::new(32, 0))
        );
        assert_eq!(
            provisional_light_neighbor_lift(topology, ChunkPos::new(0, 0), ChunkPos::new(2, 0),),
            None
        );
    }
}
