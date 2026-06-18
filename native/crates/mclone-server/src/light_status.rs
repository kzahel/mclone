//! Scheduler-owned initial `ChunkStatus::Light` inputs.
//!
//! Worldgen produces block facts for `FEATURES`; the scheduler owns the
//! follow-up light status. This module keeps the raw input carrier and bridge
//! call out of the already large scheduler module.

use mclone_core::{ChunkPos, ChunkSnapshot, PackedLightSection};
use mclone_worldgen::block::RawBlockId;
use mclone_worldgen::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};

use crate::lighting_seed::{
    provisional_light_sections_from_neighbors, provisional_sky_light_includes_chunk,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingLightStatus {
    pub(crate) pos: ChunkPos,
    pub(crate) feature_snapshot: ChunkSnapshot,
    raw_blocks: Vec<RawBlockId>,
    neighbor_blocks: Vec<(ChunkPos, Vec<RawBlockId>)>,
}

impl PendingLightStatus {
    pub(crate) fn from_feature_publication<'a>(
        pos: ChunkPos,
        feature_snapshot: ChunkSnapshot,
        chunk: &GeneratedChunk,
        generated_chunks: impl IntoIterator<Item = (&'a ChunkPos, &'a GeneratedChunk)>,
        retained_dependencies: impl IntoIterator<Item = (&'a ChunkPos, &'a MutableChunkBlockBuffer)>,
    ) -> Self {
        let neighbor_blocks = retained_dependencies
            .into_iter()
            .filter_map(|(neighbor_pos, buffer)| {
                (*neighbor_pos != pos && provisional_sky_light_includes_chunk(pos, *neighbor_pos))
                    .then_some((*neighbor_pos, buffer.blocks.as_slice()))
            })
            .chain(
                generated_chunks
                    .into_iter()
                    .filter_map(|(neighbor_pos, chunk)| {
                        (*neighbor_pos != pos
                            && provisional_sky_light_includes_chunk(pos, *neighbor_pos))
                        .then_some((*neighbor_pos, chunk.blocks()))
                    }),
            )
            .map(|(neighbor_pos, blocks)| (neighbor_pos, blocks.to_vec()))
            .collect();

        Self {
            pos,
            feature_snapshot,
            raw_blocks: chunk.blocks().to_vec(),
            neighbor_blocks,
        }
    }

    pub(crate) fn compute_light_sections(&self) -> Vec<PackedLightSection> {
        provisional_light_sections_from_neighbors(
            self.pos,
            self.feature_snapshot.min_y,
            self.feature_snapshot.height,
            &self.raw_blocks,
            self.neighbor_blocks
                .iter()
                .map(|(pos, blocks)| (*pos, blocks.as_slice())),
        )
    }
}
