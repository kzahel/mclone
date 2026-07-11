use std::collections::{BTreeMap, BTreeSet};

use mclone_app_runtime::scene_session_runtime::FarLodRuntimeSettleSnapshot;
use mclone_core::{ChunkPos, LodTileKey};
use mclone_mesh::RenderSectionKey;

/// One self-explaining row in the far-LOD settle ledger.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FarLodChunkLedgerRow {
    pub loaded: bool,
    pub traversal_ready: bool,
    pub paintable_in_frustum: bool,
    pub painted: bool,
    pub lod_desired_level: Option<u8>,
    pub lod_prefetch_level: Option<u8>,
    pub lod_resident_levels: BTreeSet<u8>,
    pub lod_uploaded_levels: BTreeSet<u8>,
    pub lod_published_level: Option<u8>,
    pub lod_visible_levels: BTreeSet<u8>,
    pub suppressed: bool,
    pub lod_pending_levels: BTreeSet<u8>,
    pub lod_inflight_levels: BTreeSet<u8>,
    pub lod_queued_upload_levels: BTreeSet<u8>,
    pub lod_queued_removal_levels: BTreeSet<u8>,
}

/// Pull-only combined runtime/render facts for the settle harness.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FarLodSettleSnapshot {
    pub runtime: FarLodRuntimeSettleSnapshot,
    pub paintable_frustum_sections: BTreeSet<RenderSectionKey>,
    pub paintable_frustum_chunks: BTreeSet<ChunkPos>,
    pub painted_sections: BTreeSet<RenderSectionKey>,
    pub painted_chunks: BTreeSet<ChunkPos>,
    pub chunks: BTreeMap<ChunkPos, FarLodChunkLedgerRow>,
}

impl FarLodSettleSnapshot {
    pub(crate) fn new(
        runtime: FarLodRuntimeSettleSnapshot,
        paintable_frustum_sections: BTreeSet<RenderSectionKey>,
        painted_sections: BTreeSet<RenderSectionKey>,
    ) -> Self {
        let paintable_frustum_chunks = section_chunks(&paintable_frustum_sections);
        let painted_chunks = section_chunks(&painted_sections);
        let traversal_ready_chunks = section_chunks(&runtime.traversal_ready_sections);
        let producer = &runtime.producer;
        let positions = runtime
            .loaded_chunks
            .iter()
            .copied()
            .chain(traversal_ready_chunks.iter().copied())
            .chain(paintable_frustum_chunks.iter().copied())
            .chain(painted_chunks.iter().copied())
            .chain(runtime.suppressed_chunks.iter().copied())
            .chain(producer.desired_tiles.keys().copied())
            .chain(producer.prefetch_tiles.keys().copied())
            .chain(tile_chunks(&producer.resident_tiles))
            .chain(tile_chunks(&producer.uploaded_tiles))
            .chain(producer.published_tiles_by_chunk.keys().copied())
            .chain(tile_chunks(&producer.visible_tiles))
            .chain(tile_chunks(&producer.pending_builds))
            .chain(tile_chunks(&producer.inflight_builds))
            .chain(tile_chunks(&producer.queued_uploads))
            .chain(tile_chunks(&producer.queued_removals))
            .collect::<BTreeSet<_>>();
        let chunks = positions
            .into_iter()
            .map(|pos| {
                let row = FarLodChunkLedgerRow {
                    loaded: runtime.loaded_chunks.contains(&pos),
                    traversal_ready: traversal_ready_chunks.contains(&pos),
                    paintable_in_frustum: paintable_frustum_chunks.contains(&pos),
                    painted: painted_chunks.contains(&pos),
                    lod_desired_level: producer.desired_tiles.get(&pos).copied(),
                    lod_prefetch_level: producer.prefetch_tiles.get(&pos).copied(),
                    lod_resident_levels: tile_levels(&producer.resident_tiles, pos),
                    lod_uploaded_levels: tile_levels(&producer.uploaded_tiles, pos),
                    lod_published_level: producer
                        .published_tiles_by_chunk
                        .get(&pos)
                        .map(|tile| tile.level),
                    lod_visible_levels: tile_levels(&producer.visible_tiles, pos),
                    suppressed: runtime.suppressed_chunks.contains(&pos),
                    lod_pending_levels: tile_levels(&producer.pending_builds, pos),
                    lod_inflight_levels: tile_levels(&producer.inflight_builds, pos),
                    lod_queued_upload_levels: tile_levels(&producer.queued_uploads, pos),
                    lod_queued_removal_levels: tile_levels(&producer.queued_removals, pos),
                };
                (pos, row)
            })
            .collect();
        Self {
            runtime,
            paintable_frustum_sections,
            paintable_frustum_chunks,
            painted_sections,
            painted_chunks,
            chunks,
        }
    }

    /// The exact D1/D2 failure signature: a ready, suppression-owning normal
    /// column with paintable geometry in the frustum that graph traversal did
    /// not actually paint.
    pub fn culled_but_suppressed_chunks(&self) -> BTreeSet<ChunkPos> {
        self.chunks
            .iter()
            .filter_map(|(pos, row)| {
                (row.loaded
                    && row.traversal_ready
                    && row.paintable_in_frustum
                    && row.suppressed
                    && !row.painted)
                    .then_some(*pos)
            })
            .collect()
    }
}

fn section_chunks(sections: &BTreeSet<RenderSectionKey>) -> BTreeSet<ChunkPos> {
    sections
        .iter()
        .map(|key| ChunkPos::new(key.chunk_x, key.chunk_z))
        .collect()
}

fn tile_chunks(tiles: &BTreeSet<LodTileKey>) -> impl Iterator<Item = ChunkPos> + '_ {
    tiles.iter().map(|tile| tile.chunk)
}

fn tile_levels(tiles: &BTreeSet<LodTileKey>, pos: ChunkPos) -> BTreeSet<u8> {
    tiles
        .iter()
        .filter_map(|tile| (tile.chunk == pos).then_some(tile.level))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_names_culled_but_suppressed_d1_d2_signature() {
        let pos = ChunkPos::new(4, -3);
        let mut runtime = FarLodRuntimeSettleSnapshot::default();
        runtime.loaded_chunks.insert(pos);
        runtime
            .traversal_ready_sections
            .insert(RenderSectionKey::new(pos.x, 5, pos.z));
        runtime.suppressed_chunks.insert(pos);

        let snapshot = FarLodSettleSnapshot::new(
            runtime,
            BTreeSet::from([RenderSectionKey::new(pos.x, 5, pos.z)]),
            BTreeSet::new(),
        );
        let row = &snapshot.chunks[&pos];
        assert!(row.loaded);
        assert!(row.traversal_ready);
        assert!(!row.painted);
        assert!(row.suppressed);
        assert!(row.lod_visible_levels.is_empty());
        assert_eq!(
            snapshot.culled_but_suppressed_chunks(),
            BTreeSet::from([pos])
        );
    }

    #[test]
    fn ledger_preserves_all_lifecycle_levels_for_one_chunk() {
        let pos = ChunkPos::new(2, 7);
        let level_one = LodTileKey::new(pos, 1);
        let level_two = LodTileKey::new(pos, 2);
        let mut runtime = FarLodRuntimeSettleSnapshot::default();
        runtime.producer.desired_tiles.insert(pos, 2);
        runtime
            .producer
            .resident_tiles
            .extend([level_one, level_two]);
        runtime.producer.uploaded_tiles.insert(level_one);
        runtime
            .producer
            .published_tiles_by_chunk
            .insert(pos, level_one);
        runtime.producer.visible_tiles.insert(level_one);
        runtime.producer.inflight_builds.insert(level_two);
        runtime.producer.queued_uploads.insert(level_two);

        let snapshot = FarLodSettleSnapshot::new(runtime, BTreeSet::new(), BTreeSet::new());
        let row = &snapshot.chunks[&pos];
        assert_eq!(row.lod_desired_level, Some(2));
        assert_eq!(row.lod_resident_levels, BTreeSet::from([1, 2]));
        assert_eq!(row.lod_uploaded_levels, BTreeSet::from([1]));
        assert_eq!(row.lod_published_level, Some(1));
        assert_eq!(row.lod_visible_levels, BTreeSet::from([1]));
        assert_eq!(row.lod_inflight_levels, BTreeSet::from([2]));
        assert_eq!(row.lod_queued_upload_levels, BTreeSet::from([2]));
    }
}
