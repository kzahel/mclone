use super::*;

#[test]
fn local_chunk_view_routes_status_events_into_loading_progress() {
    let mut server = LocalRealmSession::new(0);
    assert_eq!(server.loading_progress_stats(), None);

    load_center_chunk(&mut server);
    for _ in 0..4 {
        if server
            .loading_progress_stats()
            .is_some_and(|stats| stats.playable_chunk_ready)
        {
            break;
        }
        let updates = server.try_poll().expect("poll");
        accept_player_position_updates(&mut server, &updates);
    }

    let stats = server
        .loading_progress_stats()
        .expect("local chunk view should create loading progress stats");
    assert_eq!(stats.center, ChunkPos::new(0, 0));
    assert_eq!(stats.target_radius, 0);
    assert_eq!(stats.target_status, mclone_core::ChunkStatus::Features);
    assert_eq!(stats.target_chunk_count, 1);
    assert_eq!(stats.target_ready_chunks, 1);
    assert_eq!(stats.playable_chunk, ChunkPos::new(0, 0));
    assert_eq!(stats.playable_gate_radius, 1);
    assert_eq!(stats.playable_gate_chunk_count, 9);
    assert_eq!(stats.playable_gate_ready_chunks, 9);
    assert!(stats.playable_chunk_ready);
}

#[test]
fn view_readiness_snapshot_tracks_current_accepted_view() {
    let mut server = LocalRealmSession::new(0);
    assert_eq!(server.view_readiness_snapshot(), None);

    load_chunk_view(&mut server, ChunkPos::new(0, 0));
    let first = server
        .view_readiness_snapshot()
        .expect("loaded local view should expose readiness snapshot");
    assert_eq!(first.stats.center, ChunkPos::new(0, 0));
    assert_eq!(first.stats.target_radius, 0);
    assert_eq!(first.stats.target_chunk_count, 1);
    assert_eq!(first.stats.target_ready_chunks, 1);
    assert_eq!(first.stats.playable_gate_ready_chunks, 9);
    assert_eq!(first.cells.len(), 1);
    assert!(first.stats.playable_chunk_ready);

    load_chunk_view(&mut server, ChunkPos::new(2, 0));
    let moved = server
        .view_readiness_snapshot()
        .expect("moved local view should expose readiness snapshot");
    assert_eq!(moved.stats.center, ChunkPos::new(2, 0));
    assert_eq!(moved.stats.target_radius, 0);
    assert_eq!(moved.stats.target_chunk_count, 1);
    assert_eq!(moved.stats.target_ready_chunks, 1);
    assert_eq!(moved.stats.playable_gate_ready_chunks, 9);
    assert_eq!(moved.cells.len(), 1);
    assert_eq!(moved.cells[0].relative_x, 0);
    assert_eq!(moved.cells[0].relative_z, 0);
    assert!(moved.cells[0].playable);
    assert!(moved.stats.playable_chunk_ready);
}

#[test]
fn view_readiness_snapshot_uses_runtime_target_status() {
    let mut unlit_server = LocalRealmSession::new(0);
    load_chunk_view_with_lighting(&mut unlit_server, ChunkPos::new(0, 0), false);
    let unlit = unlit_server
        .view_readiness_snapshot()
        .expect("unlit loaded view should expose readiness snapshot");
    assert_eq!(
        unlit.stats.target_status,
        mclone_core::ChunkStatus::Features
    );
    assert_eq!(unlit.stats.target_ready_chunks, 1);
    assert_eq!(unlit.stats.playable_gate_ready_chunks, 9);
    assert!(unlit.stats.playable_chunk_ready);

    let mut lit_server = LocalRealmSession::new(0);
    load_chunk_view_with_lighting(&mut lit_server, ChunkPos::new(0, 0), true);
    let lit = lit_server
        .view_readiness_snapshot()
        .expect("lit loaded view should expose readiness snapshot");
    assert_eq!(lit.stats.target_status, mclone_core::ChunkStatus::Light);
    assert_eq!(lit.stats.target_ready_chunks, 1);
    assert_eq!(lit.stats.playable_gate_ready_chunks, 9);
    assert!(lit.stats.playable_chunk_ready);
}
