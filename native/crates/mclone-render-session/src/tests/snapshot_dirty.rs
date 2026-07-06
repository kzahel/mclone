use super::*;

#[test]
fn snapshot_mesh_block_state_ids_rehydrates_omitted_air_sections() {
    let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
    block_state_ids[CHUNK_SECTION_VOLUME] = BlockStateId(1);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        ChunkPos::new(0, 0),
        ChunkStatus::Surface,
        ChunkRevision(1),
        0,
        32,
        &block_state_ids,
    );

    let mesh_blocks = snapshot_mesh_block_state_ids(&snapshot).unwrap();

    assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
    assert_eq!(mesh_blocks.blocks[0], AIR_BLOCK_STATE_ID);
    assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], BlockStateId(1));
}

#[test]
fn render_section_view_sync_marks_added_chunks_and_removed_neighbors_dirty() {
    let previous_chunks = BTreeSet::from([
        ChunkPos::new(0, 0),
        ChunkPos::new(1, 0),
        ChunkPos::new(2, 0),
    ]);
    let current_chunks = BTreeSet::from([
        ChunkPos::new(1, 0),
        ChunkPos::new(2, 0),
        ChunkPos::new(3, 0),
    ]);

    let sync = RenderSectionViewSync::from_loaded_chunks(&previous_chunks, current_chunks);

    assert_eq!(
        sync.dirty_chunks,
        BTreeSet::from([ChunkPos::new(1, 0), ChunkPos::new(3, 0)])
    );
    assert_eq!(sync.removal_chunks, BTreeSet::from([ChunkPos::new(0, 0)]));
}

#[test]
fn render_section_session_marks_chunk_neighborhood_dirty_with_loaded_sections() {
    let center = ChunkPos::new(5, -3);
    let east = ChunkPos::new(6, -3);
    let center_key = RenderSectionKey::new(5, 4, -3);
    let east_key = RenderSectionKey::new(6, 4, -3);
    let mut session = RenderSectionSession::default();

    let marked = session.mark_chunk_neighborhood_dirty_with_loaded_sections(center, |pos| {
        if pos == center {
            vec![center_key]
        } else if pos == east {
            vec![east_key]
        } else {
            Vec::new()
        }
    });

    assert_eq!(marked, 5);
    assert_eq!(session.dirty().dirty_chunks, BTreeSet::from([center, east]));
    assert_eq!(session.dirty().section_revision(center_key), 1);
    assert_eq!(session.dirty().section_revision(east_key), 1);
}

#[test]
fn block_delta_dirty_sections_cross_chunk_and_section_boundaries() {
    let keys = render_dirty_section_keys_for_block_update(
        ChunkPos::new(0, 0),
        0,
        &SectionBlockUpdate {
            local_x: 0,
            local_y: 0,
            local_z: 15,
            block_state: BlockStateId(42),
        },
    );

    assert_eq!(
        keys,
        BTreeSet::from([
            RenderSectionKey::new(-1, -1, 0),
            RenderSectionKey::new(-1, -1, 1),
            RenderSectionKey::new(-1, 0, 0),
            RenderSectionKey::new(-1, 0, 1),
            RenderSectionKey::new(0, -1, 0),
            RenderSectionKey::new(0, -1, 1),
            RenderSectionKey::new(0, 0, 0),
            RenderSectionKey::new(0, 0, 1),
        ])
    );
}
