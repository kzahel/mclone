use super::*;

#[test]
fn engine_render_session_prepares_ready_work_from_client_snapshots() {
    let chunk = ChunkPos::new(2, -1);
    let key = RenderSectionKey::new(2, 0, -1);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
        chunk,
        0,
        SECTION_HEIGHT,
    )));
    let mut engine = EngineRenderSession::new(client);

    assert!(engine.mark_loaded_chunks_dirty_when_cache_empty());
    assert_eq!(
        engine.render_session().dirty().dirty_chunks,
        BTreeSet::from([chunk])
    );
    assert_eq!(engine.render_session().dirty().section_revision(key), 1);

    let sync_update = engine.prepare_sync_update(
        |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
        |dirty_work| {
            dirty_work
                .loaded_dirty_sections_by_chunk
                .keys()
                .copied()
                .collect()
        },
        usize::MAX,
        |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
        RenderSectionRemovalMode::Defer,
    );
    assert_eq!(
        sync_update.sync_plan.ready_plan.ready_section_keys,
        BTreeSet::from([key])
    );

    let snapshots = engine.client().chunk_snapshots().cloned().collect();
    let submission = engine
        .submit_prepared_sync_plan(&sync_update.sync_plan, snapshots, |_sync_plan, request| {
            Ok::<_, std::convert::Infallible>(request)
        })
        .expect("ready work should submit")
        .submission
        .expect("loaded dirty section should produce a request");

    assert_eq!(submission.output.target_sections, BTreeSet::from([key]));
    assert_eq!(
        engine.render_session().dirty().inflight_sections,
        BTreeSet::from([key])
    );
    assert!(engine.render_session().dirty().dirty_chunks.is_empty());
}

#[test]
fn engine_render_session_submits_when_compiler_capacity_is_available() {
    let chunk = ChunkPos::new(2, -1);
    let key = RenderSectionKey::new(2, 0, -1);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
        chunk,
        0,
        SECTION_HEIGHT,
    )));
    let mut engine = EngineRenderSession::new(client);
    let mut compiler = CapacityTestCompiler::new(1, 2);

    let update = engine
        .sync_render_sections_with_budget(
            &mut compiler,
            1,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
            |client, _compiler| client.chunk_snapshots().cloned().collect(),
        )
        .expect("sync should submit when compiler has capacity");

    assert_eq!(update.submitted_compile_section_count, 1);
    assert_eq!(update.pending_compile_jobs, 2);
    assert_eq!(compiler.pending_job_count(), 2);
    assert_eq!(compiler.submitted_requests.len(), 1);
    assert_eq!(
        compiler.submitted_requests[0].target_sections,
        BTreeSet::from([key])
    );
}

#[test]
fn engine_render_session_waits_when_compiler_capacity_is_full() {
    let chunk = ChunkPos::new(2, -1);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
        chunk,
        0,
        SECTION_HEIGHT,
    )));
    let mut engine = EngineRenderSession::new(client);
    let mut compiler = CapacityTestCompiler::new(1, 1);

    let update = engine
        .sync_render_sections_with_budget(
            &mut compiler,
            1,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
            |_, _| panic!("full compiler capacity must not collect submit snapshots"),
        )
        .expect("sync should wait when compiler capacity is full");

    assert_eq!(update.submitted_compile_section_count, 0);
    assert_eq!(update.pending_compile_jobs, 1);
    assert_eq!(compiler.pending_job_count(), 1);
    assert!(compiler.submitted_requests.is_empty());
    assert!(!engine.render_session().dirty_is_empty());
}

#[test]
fn engine_render_session_can_budget_completed_compile_result_acceptance() {
    let chunk = ChunkPos::new(0, 0);
    let first = RenderSectionKey::new(0, 0, 0);
    let second = RenderSectionKey::new(0, 1, 0);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
        chunk,
        0,
        SECTION_HEIGHT * 2,
    )));
    let mut engine = EngineRenderSession::new(client);
    let mut compiler = CapacityTestCompiler::with_completed_results(vec![
        RenderSectionCompileResult {
            target_sections: BTreeSet::from([first]),
            section_revisions: BTreeMap::from([(first, 0)]),
            result: Ok(test_build_report([first])),
        },
        RenderSectionCompileResult {
            target_sections: BTreeSet::from([second]),
            section_revisions: BTreeMap::from([(second, 0)]),
            result: Ok(test_build_report([second])),
        },
    ]);

    let first_update = engine
        .sync_render_sections_with_budget_and_completed_result_acceptance(
            &mut compiler,
            1,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
            Some(1),
            |_, _| panic!("queued completed results should backpressure new submissions"),
        )
        .expect("first sync should accept one completed result");

    assert_eq!(first_update.accepted_compile_result_count, 1);
    assert_eq!(first_update.queued_completed_compile_result_count, 1);
    assert_eq!(first_update.rebuilt_section_count(), 1);
    assert_eq!(first_update.completed_compile_section_count, 1);
    assert_eq!(engine.pending_completed_compile_result_count(), 1);
    assert!(engine.has_pending_render_work(0, |_client, _key| {
        RenderSectionNeighborReadiness::ReadyWithNeighbors
    }));

    let second_update = engine
        .sync_render_sections_with_budget_and_completed_result_acceptance(
            &mut compiler,
            1,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
            Some(1),
            |client, _compiler| client.chunk_snapshots().cloned().collect(),
        )
        .expect("second sync should accept the queued completed result");

    assert_eq!(second_update.accepted_compile_result_count, 1);
    assert_eq!(second_update.queued_completed_compile_result_count, 0);
    assert_eq!(second_update.rebuilt_section_count(), 1);
    assert_eq!(second_update.completed_compile_section_count, 1);
    assert_eq!(engine.pending_completed_compile_result_count(), 0);
    assert!(!engine.has_pending_render_work(0, |_client, _key| {
        RenderSectionNeighborReadiness::ReadyWithNeighbors
    }));
}

#[test]
fn engine_render_session_applies_server_updates_and_marks_render_dirty() {
    let chunk = ChunkPos::new(0, 0);
    let key = RenderSectionKey::new(0, 7, 0);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
        chunk,
        0,
        SECTION_HEIGHT * 16,
    )));
    let mut engine = EngineRenderSession::new(client);

    let report = engine.apply_server_updates(vec![
        ServerUpdate::TimeUpdate {
            game_time: 2,
            day_time: 1,
            daylight_cycle_running: true,
        },
        ServerUpdate::SectionBlockUpdates {
            pos: chunk,
            section_y: 7,
            updates: vec![SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: AIR_BLOCK_STATE_ID,
            }],
        },
    ]);

    assert_eq!(
        report,
        EngineServerUpdateReport {
            changed: true,
            updates: 2,
            snapshot_updates: 0,
            section_block_updates: 1,
            unload_updates: 0,
        }
    );
    assert_eq!(engine.client().day_time(), 1);
    assert!(engine.render_session().dirty().dirty_chunks.is_empty());
    assert_eq!(
        engine.render_session().dirty().dirty_sections,
        BTreeSet::from([key])
    );
}

#[test]
fn engine_render_session_retires_old_dimension_chunks_before_destination_updates() {
    let old_chunk = ChunkPos::new(0, 0);
    let destination_chunk = ChunkPos::new(4, -3);
    let old_key = RenderSectionKey::new(0, 0, 0);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
        old_chunk,
        0,
        SECTION_HEIGHT,
    )));
    let mut engine = EngineRenderSession::new(client);
    engine.render_session_mut().apply_finished_compile_report(
        &BTreeSet::from([old_key]),
        test_build_report([old_key]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    let report = engine.apply_server_updates(vec![
        ServerUpdate::DimensionChange {
            dimension: DimensionKey::parse("mclone:moon").expect("dimension key should be valid"),
            biome_zoom_seed: 41,
            topology: mclone_core::HorizontalTopology::UNBOUNDED,
            keep_player_state: true,
        },
        ServerUpdate::ChunkSnapshot(empty_test_snapshot(destination_chunk, 0, SECTION_HEIGHT)),
    ]);

    assert_eq!(engine.client().current_dimension().as_str(), "mclone:moon");
    assert!(engine.client().chunk_snapshot(old_chunk).is_none());
    assert!(engine.client().chunk_snapshot(destination_chunk).is_some());
    assert_eq!(report.updates, 2);
    assert_eq!(report.snapshot_updates, 1);
    assert!(report.changed);
    assert!(
        engine
            .render_session()
            .resident_dirty_section_keys()
            .any(|key| key == old_key),
        "the old dimension's resident mesh must be retired"
    );
    assert!(
        engine
            .render_session()
            .dirty()
            .dirty_chunks
            .contains(&destination_chunk),
        "the destination snapshot must be compiled"
    );
}

#[test]
fn engine_render_session_can_leave_snapshot_dirtying_to_view_sync_policy() {
    let chunk = ChunkPos::new(3, 4);
    let mut engine = EngineRenderSession::new(ClientRuntime::local_integrated());

    let report = engine.apply_server_updates_with_dirty_policy(
        vec![ServerUpdate::ChunkSnapshot(empty_test_snapshot(
            chunk,
            0,
            SECTION_HEIGHT,
        ))],
        EngineServerUpdateDirtyPolicy::SECTION_BLOCK_UPDATES_ONLY,
    );

    assert_eq!(
        report,
        EngineServerUpdateReport {
            changed: true,
            updates: 1,
            snapshot_updates: 1,
            section_block_updates: 0,
            unload_updates: 0,
        }
    );
    assert!(engine.client().chunk_snapshot(chunk).is_some());
    assert!(engine.render_session().dirty().dirty_chunks.is_empty());
    assert!(engine.render_session().dirty().dirty_sections.is_empty());
}

#[test]
fn engine_render_session_coalesces_duplicate_snapshot_dirtying_before_revision_bump() {
    let chunk = ChunkPos::new(0, 0);
    let key = RenderSectionKey::new(0, 0, 0);
    let snapshot = empty_test_snapshot(chunk, 0, SECTION_HEIGHT);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(snapshot.clone()));
    let mut engine = EngineRenderSession::new(client);

    let report = engine.mark_server_update_render_dirty(&[
        ServerUpdate::ChunkSnapshot(snapshot.clone()),
        ServerUpdate::ChunkSnapshot(snapshot),
    ]);

    assert_eq!(
        report,
        EngineServerUpdateReport {
            changed: true,
            updates: 2,
            snapshot_updates: 2,
            section_block_updates: 0,
            unload_updates: 0,
        }
    );
    assert_eq!(
        engine.render_session().dirty().dirty_chunks,
        BTreeSet::from([chunk])
    );
    assert_eq!(engine.render_session().dirty().section_revision(key), 1);
}
#[test]
fn engine_render_session_marks_resident_snapshot_sections_dirty_without_dirty_chunk_set() {
    let chunk = ChunkPos::new(0, 0);
    let key = RenderSectionKey::new(0, 0, 0);
    let snapshot = empty_test_snapshot(chunk, 0, SECTION_HEIGHT);
    let mut client = ClientRuntime::local_integrated();
    client.apply_update(ServerUpdate::ChunkSnapshot(snapshot.clone()));
    let mut engine = EngineRenderSession::new(client);
    engine.render_session_mut().apply_finished_compile_report(
        &BTreeSet::from([key]),
        test_build_report([key]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    engine.mark_server_update_render_dirty(&[ServerUpdate::ChunkSnapshot(snapshot)]);

    assert!(engine.render_session().dirty().dirty_chunks.is_empty());
    assert!(engine.render_session().dirty().dirty_sections.is_empty());
    assert_eq!(
        engine
            .render_session()
            .resident_dirty_section_keys()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([key])
    );
    assert_eq!(engine.render_session().dirty().section_revision(key), 1);
}

#[test]
fn engine_render_session_coalesces_duplicate_section_dirtying_before_revision_bump() {
    let chunk = ChunkPos::new(0, 0);
    let key = RenderSectionKey::new(0, 7, 0);
    let update = SectionBlockUpdate {
        local_x: 8,
        local_y: 8,
        local_z: 8,
        block_state: AIR_BLOCK_STATE_ID,
    };
    let mut engine = EngineRenderSession::new(ClientRuntime::local_integrated());

    let report = engine.mark_server_update_render_dirty(&[ServerUpdate::SectionBlockUpdates {
        pos: chunk,
        section_y: 7,
        updates: vec![update, update],
    }]);

    assert_eq!(
        report,
        EngineServerUpdateReport {
            changed: true,
            updates: 1,
            snapshot_updates: 0,
            section_block_updates: 1,
            unload_updates: 0,
        }
    );
    assert_eq!(
        engine.render_session().dirty().dirty_sections,
        BTreeSet::from([key])
    );
    assert_eq!(engine.render_session().dirty().section_revision(key), 1);
}

#[test]
fn engine_render_session_marks_resident_section_updates_dirty_without_dirty_section_set() {
    let chunk = ChunkPos::new(0, 0);
    let key = RenderSectionKey::new(0, 7, 0);
    let update = SectionBlockUpdate {
        local_x: 8,
        local_y: 8,
        local_z: 8,
        block_state: AIR_BLOCK_STATE_ID,
    };
    let mut engine = EngineRenderSession::new(ClientRuntime::local_integrated());
    engine.render_session_mut().apply_finished_compile_report(
        &BTreeSet::from([key]),
        test_build_report([key]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    engine.mark_server_update_render_dirty(&[ServerUpdate::SectionBlockUpdates {
        pos: chunk,
        section_y: 7,
        updates: vec![update],
    }]);

    assert!(engine.render_session().dirty().dirty_chunks.is_empty());
    assert!(engine.render_session().dirty().dirty_sections.is_empty());
    assert_eq!(
        engine
            .render_session()
            .resident_dirty_section_keys()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([key])
    );
    assert_eq!(engine.render_session().dirty().section_revision(key), 1);
}
