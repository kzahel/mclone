use super::*;

#[test]
fn render_section_session_applies_loaded_view_sync_dirty_marks() {
    let removed_chunk = ChunkPos::new(0, 0);
    let edge_chunk = ChunkPos::new(1, 0);
    let retained_chunk = ChunkPos::new(2, 0);
    let added_chunk = ChunkPos::new(3, 0);
    let previous_chunks = BTreeSet::from([removed_chunk, edge_chunk, retained_chunk]);
    let current_chunks = BTreeSet::from([edge_chunk, retained_chunk, added_chunk]);
    let cached_removed_key = RenderSectionKey::new(0, 4, 0);
    let edge_key = RenderSectionKey::new(1, 4, 0);
    let added_key = RenderSectionKey::new(3, 4, 0);
    let mut session = RenderSectionSession::default();
    session.apply_finished_compile_report(
        &BTreeSet::from([cached_removed_key]),
        test_build_report([cached_removed_key]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    let sync = session.apply_loaded_view_sync(&previous_chunks, current_chunks, |pos| {
        if pos == edge_chunk {
            vec![edge_key]
        } else if pos == added_chunk {
            vec![added_key]
        } else {
            Vec::new()
        }
    });

    assert_eq!(sync.dirty_chunks, BTreeSet::from([edge_chunk, added_chunk]));
    assert_eq!(sync.removal_chunks, BTreeSet::from([removed_chunk]));
    assert_eq!(
        session.dirty().dirty_chunks,
        BTreeSet::from([removed_chunk, edge_chunk, added_chunk])
    );
    assert_eq!(session.dirty().section_revision(cached_removed_key), 1);
    assert_eq!(session.dirty().section_revision(edge_key), 1);
    assert_eq!(session.dirty().section_revision(added_key), 1);
}

#[test]
fn render_section_dirty_state_tracks_dirty_inflight_and_stale_revisions() {
    let first = RenderSectionKey::new(0, 4, 0);
    let second = RenderSectionKey::new(0, 5, 0);
    let mut dirty = RenderSectionDirtyState::default();
    let initial_generation = dirty.work_generation();

    dirty.mark_chunk_dirty(ChunkPos::new(0, 0), [first, second]);
    assert_ne!(dirty.work_generation(), initial_generation);
    assert_eq!(dirty.dirty_chunks, BTreeSet::from([ChunkPos::new(0, 0)]));
    assert_eq!(dirty.section_revision(first), 1);
    assert_eq!(dirty.section_revision(second), 1);

    let request = dirty.build_compile_request(BTreeSet::from([first, second]), Vec::new());
    let dirty_generation = dirty.work_generation();
    dirty.mark_compile_submitted(&request.target_sections);
    assert_ne!(dirty.work_generation(), dirty_generation);
    assert_eq!(dirty.inflight_sections, BTreeSet::from([first, second]));

    dirty.mark_section_dirty(second);
    let acceptance = dirty.accept_completed_compile_result(&RenderSectionCompileResult {
        target_sections: request.target_sections,
        section_revisions: request.section_revisions,
        result: Ok(TexturedRenderSectionBuildReport::default()),
    });

    assert!(dirty.inflight_sections.is_empty());
    assert_eq!(acceptance.accepted_sections, BTreeSet::from([first]));
    assert_eq!(acceptance.stale_sections, BTreeSet::from([second]));
    assert_eq!(dirty.dirty_sections, BTreeSet::from([second]));
}

#[test]
fn render_section_dirty_work_classifies_loaded_removal_and_stale_items() {
    let loaded_chunk = ChunkPos::new(0, 0);
    let removal_chunk = ChunkPos::new(1, 0);
    let stale_chunk = ChunkPos::new(2, 0);
    let loaded_section = RenderSectionKey::new(0, 4, 0);
    let chunk_removal_section = RenderSectionKey::new(1, 4, 0);
    let removal_section = RenderSectionKey::new(3, 4, 0);
    let stale_section = RenderSectionKey::new(4, 4, 0);
    let loaded_chunks = BTreeSet::from([loaded_chunk]);
    let cached_chunks = BTreeSet::from([removal_chunk]);
    let loaded_sections = BTreeSet::from([loaded_section]);
    let cached_sections = BTreeSet::from([chunk_removal_section, removal_section]);
    let mut dirty = RenderSectionDirtyState {
        dirty_chunks: BTreeSet::from([loaded_chunk, removal_chunk, stale_chunk]),
        dirty_sections: BTreeSet::from([
            loaded_section,
            chunk_removal_section,
            removal_section,
            stale_section,
        ]),
        inflight_sections: BTreeSet::from([chunk_removal_section, removal_section]),
        ..RenderSectionDirtyState::default()
    };

    let work = classify_render_section_dirty_work(
        &dirty,
        |pos| loaded_chunks.contains(&pos),
        |pos| cached_chunks.contains(&pos),
        |key| loaded_sections.contains(&key),
        |key| cached_sections.contains(&key),
    );

    assert_eq!(work.loaded_dirty_chunks, BTreeSet::from([loaded_chunk]));
    assert_eq!(work.removal_dirty_chunks, BTreeSet::from([removal_chunk]));
    assert_eq!(work.stale_dirty_chunks, BTreeSet::from([stale_chunk]));
    assert_eq!(
        work.loaded_dirty_sections_by_chunk,
        BTreeMap::from([(loaded_chunk, BTreeSet::from([loaded_section]))])
    );
    assert_eq!(
        work.removal_dirty_sections,
        BTreeSet::from([chunk_removal_section, removal_section])
    );
    assert_eq!(work.stale_dirty_sections, BTreeSet::from([stale_section]));

    dirty.discard_stale_dirty_work(&work.stale_dirty_chunks, &work.stale_dirty_sections);
    dirty.discard_removed_dirty_work(&work.removal_dirty_chunks, &work.removal_dirty_sections);

    assert_eq!(dirty.dirty_chunks, BTreeSet::from([loaded_chunk]));
    assert_eq!(dirty.dirty_sections, BTreeSet::from([loaded_section]));
    assert!(dirty.inflight_sections.is_empty());
}

#[test]
fn render_section_ready_plan_selects_budgeted_ready_and_deferred_sections() {
    let loaded_chunk = ChunkPos::new(0, 0);
    let dirty_section_chunk = ChunkPos::new(2, 0);
    let skipped_section_chunk = ChunkPos::new(3, 0);
    let ready_from_chunk = RenderSectionKey::new(0, 4, 0);
    let deferred_from_chunk = RenderSectionKey::new(0, 5, 0);
    let inflight_from_chunk = RenderSectionKey::new(0, 6, 0);
    let near_dirty_section = RenderSectionKey::new(2, 4, 0);
    let skipped_dirty_section = RenderSectionKey::new(3, 4, 0);
    let sections_by_chunk = BTreeMap::from([
        (dirty_section_chunk, BTreeSet::from([near_dirty_section])),
        (
            skipped_section_chunk,
            BTreeSet::from([skipped_dirty_section]),
        ),
    ]);

    let plan = plan_ready_render_sections(
        [loaded_chunk],
        [dirty_section_chunk, skipped_section_chunk],
        &sections_by_chunk,
        2,
        |_| vec![ready_from_chunk, deferred_from_chunk, inflight_from_chunk],
        &BTreeSet::from([inflight_from_chunk]),
        |key| match key {
            key if key == ready_from_chunk => RenderSectionNeighborReadiness::ReadyWithNeighbors,
            key if key == deferred_from_chunk => {
                RenderSectionNeighborReadiness::DeferredMissingNeighbors
            }
            key if key == near_dirty_section => RenderSectionNeighborReadiness::ReadyNearCamera,
            unexpected => panic!("unexpected readiness probe for {unexpected:?}"),
        },
    );

    assert_eq!(plan.budgeted_loaded_chunks, BTreeSet::from([loaded_chunk]));
    assert_eq!(
        plan.budgeted_dirty_section_chunks,
        BTreeSet::from([dirty_section_chunk])
    );
    assert_eq!(
        plan.ready_section_keys,
        BTreeSet::from([ready_from_chunk, near_dirty_section])
    );
    assert_eq!(
        plan.deferred_section_keys,
        BTreeSet::from([deferred_from_chunk, inflight_from_chunk])
    );
    assert_eq!(plan.near_exception_section_count, 1);
    assert_eq!(plan.deferred_section_count, 1);

    let mut dirty = RenderSectionDirtyState {
        dirty_chunks: BTreeSet::from([loaded_chunk]),
        dirty_sections: BTreeSet::from([
            ready_from_chunk,
            deferred_from_chunk,
            inflight_from_chunk,
            near_dirty_section,
        ]),
        ..RenderSectionDirtyState::default()
    };
    dirty.apply_ready_plan(&plan);

    assert!(dirty.dirty_chunks.is_empty());
    assert_eq!(
        dirty.dirty_sections,
        BTreeSet::from([deferred_from_chunk, inflight_from_chunk])
    );
}

#[test]
fn render_section_ready_plan_skips_deferred_only_chunks_for_budget() {
    let deferred_chunk = ChunkPos::new(1, 0);
    let ready_chunk = ChunkPos::new(2, 0);
    let deferred = RenderSectionKey::new(1, 4, 0);
    let ready = RenderSectionKey::new(2, 4, 0);
    let sections_by_chunk = BTreeMap::from([
        (deferred_chunk, BTreeSet::from([deferred])),
        (ready_chunk, BTreeSet::from([ready])),
    ]);

    let plan = plan_ready_render_sections(
        [],
        [deferred_chunk, ready_chunk],
        &sections_by_chunk,
        1,
        |_| Vec::new(),
        &BTreeSet::new(),
        |key| {
            if key == ready {
                RenderSectionNeighborReadiness::ReadyWithNeighbors
            } else {
                RenderSectionNeighborReadiness::DeferredMissingNeighbors
            }
        },
    );

    assert!(plan.budgeted_loaded_chunks.is_empty());
    assert_eq!(
        plan.budgeted_dirty_section_chunks,
        BTreeSet::from([ready_chunk])
    );
    assert_eq!(plan.ready_section_keys, BTreeSet::from([ready]));
    assert_eq!(plan.deferred_section_keys, BTreeSet::from([deferred]));
    assert_eq!(plan.deferred_section_count, 1);
}

#[test]
fn render_section_ready_plan_completes_empty_loaded_chunk_work() {
    let empty_chunk = ChunkPos::new(3, -2);
    let plan = plan_ready_render_sections(
        [empty_chunk],
        [],
        &BTreeMap::new(),
        1,
        |_| Vec::new(),
        &BTreeSet::new(),
        |_| panic!("empty chunk must not probe section readiness"),
    );

    assert_eq!(plan.budgeted_loaded_chunks, BTreeSet::from([empty_chunk]));
    assert!(plan.ready_section_keys.is_empty());

    let mut dirty = RenderSectionDirtyState {
        dirty_chunks: BTreeSet::from([empty_chunk]),
        ..RenderSectionDirtyState::default()
    };
    dirty.apply_ready_plan(&plan);
    assert!(dirty.dirty_chunks.is_empty());
}

#[test]
fn render_section_sync_plan_discards_stale_and_plans_ready_work() {
    let loaded_chunk = ChunkPos::new(0, 0);
    let stale_chunk = ChunkPos::new(1, 0);
    let ready = RenderSectionKey::new(0, 4, 0);
    let stale = RenderSectionKey::new(1, 4, 0);
    let mut dirty = RenderSectionDirtyState {
        dirty_chunks: BTreeSet::from([loaded_chunk, stale_chunk]),
        dirty_sections: BTreeSet::from([stale]),
        ..RenderSectionDirtyState::default()
    };
    let dirty_work = RenderSectionDirtyWork {
        loaded_dirty_chunks: BTreeSet::from([loaded_chunk]),
        stale_dirty_chunks: BTreeSet::from([stale_chunk]),
        stale_dirty_sections: BTreeSet::from([stale]),
        ..RenderSectionDirtyWork::default()
    };

    let plan = prepare_render_section_sync_plan(
        &mut dirty,
        dirty_work,
        [loaded_chunk],
        [],
        1,
        |_| vec![ready],
        |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
    );

    assert_eq!(dirty.dirty_chunks, BTreeSet::from([loaded_chunk]));
    assert!(dirty.dirty_sections.is_empty());
    assert_eq!(plan.ready_plan.ready_section_keys, BTreeSet::from([ready]));
    assert!(!plan.has_removals());
    assert_eq!(plan.ready_update(1).submitted_compile_section_count, 1);
}

#[test]
fn render_view_compile_queue_starts_when_idle_and_skips_loaded_center() {
    let mut queue = RenderViewCompileQueue::default();
    let center = ChunkPos::new(2, -3);

    assert_eq!(
        queue.request(
            QueuedRenderViewCompile::new(center, false, "movement"),
            None,
            false
        ),
        RenderViewCompileQueueDecision::Start(QueuedRenderViewCompile::new(
            center, false, "movement"
        ))
    );
    assert_eq!(
        queue.request(
            QueuedRenderViewCompile::new(center, false, "movement"),
            Some(center),
            false
        ),
        RenderViewCompileQueueDecision::Skipped
    );
}

#[test]
fn render_view_compile_queue_coalesces_while_busy_and_preserves_force() {
    let mut queue = RenderViewCompileQueue::default();
    let first = ChunkPos::new(0, 0);
    let second = ChunkPos::new(1, 0);

    assert_eq!(
        queue.request(
            QueuedRenderViewCompile::new(first, true, "interaction"),
            None,
            true
        ),
        RenderViewCompileQueueDecision::Queued(QueuedRenderViewCompile::new(
            first,
            true,
            "interaction"
        ))
    );
    assert_eq!(
        queue.request(
            QueuedRenderViewCompile::new(second, false, "movement"),
            None,
            true
        ),
        RenderViewCompileQueueDecision::Queued(QueuedRenderViewCompile::new(
            second, true, "movement"
        ))
    );
    assert_eq!(
        queue.take_next(None, false),
        RenderViewCompileQueueDecision::Start(QueuedRenderViewCompile::new(
            second, true, "movement"
        ))
    );
    assert_eq!(
        queue.take_next(None, false),
        RenderViewCompileQueueDecision::Idle
    );
}

#[test]
fn render_view_compile_queue_latest_idle_or_loaded_request_discards_stale_queue() {
    let mut queue = RenderViewCompileQueue::default();
    let loaded = ChunkPos::new(0, 0);
    let stale = ChunkPos::new(1, 0);
    let fresh = ChunkPos::new(2, 0);

    assert!(matches!(
        queue.request(
            QueuedRenderViewCompile::new(stale, false, "movement"),
            Some(loaded),
            true
        ),
        RenderViewCompileQueueDecision::Queued(_)
    ));
    assert!(queue.has_queued());
    assert_eq!(
        queue.request(
            QueuedRenderViewCompile::new(loaded, false, "movement"),
            Some(loaded),
            true
        ),
        RenderViewCompileQueueDecision::Skipped
    );
    assert!(!queue.has_queued());

    assert!(matches!(
        queue.request(
            QueuedRenderViewCompile::new(stale, false, "movement"),
            Some(loaded),
            true
        ),
        RenderViewCompileQueueDecision::Queued(_)
    ));
    assert_eq!(
        queue.request(
            QueuedRenderViewCompile::new(fresh, false, "movement"),
            Some(loaded),
            false
        ),
        RenderViewCompileQueueDecision::Start(QueuedRenderViewCompile::new(
            fresh, false, "movement"
        ))
    );
    assert!(!queue.has_queued());
}

#[test]
fn render_section_sync_update_applies_removals_for_native_style_sync() {
    let loaded_chunk = ChunkPos::new(0, 0);
    let removal_chunk = ChunkPos::new(1, 0);
    let ready = RenderSectionKey::new(0, 4, 0);
    let cached = RenderSectionKey::new(1, 4, 0);
    let mut session = RenderSectionSession::default();
    session.apply_finished_compile_report(
        &BTreeSet::from([cached]),
        TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key: cached,
                mesh: TexturedVisibleChunkMesh::default(),
                grass_patches: Vec::new(),
                visibility: VisibilitySet::all_visible(),
            }],
            visibility_graph: VisibilityGraphBuildStats::default(),
        },
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    session.mark_chunk_dirty(loaded_chunk, [ready]);
    session.mark_chunk_dirty(removal_chunk, [cached]);

    let update = session.prepare_sync_update(
        |pos| pos == loaded_chunk,
        |key| key == ready,
        |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
        |dirty_work| {
            dirty_work
                .loaded_dirty_sections_by_chunk
                .keys()
                .copied()
                .collect()
        },
        1,
        |_| vec![ready],
        |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
        RenderSectionRemovalMode::ApplyImmediately,
    );

    assert_eq!(update.cache_update.removed_section_count(), 1);
    assert!(!session.contains_section(cached));
    assert_eq!(
        update.sync_plan.ready_plan.ready_section_keys,
        BTreeSet::from([ready])
    );
    assert!(!session.dirty().dirty_chunks.contains(&removal_chunk));
}

#[test]
fn render_section_sync_update_can_defer_removals_for_combined_web_uploads() {
    let loaded_chunk = ChunkPos::new(0, 0);
    let removal_chunk = ChunkPos::new(1, 0);
    let ready = RenderSectionKey::new(0, 4, 0);
    let cached = RenderSectionKey::new(1, 4, 0);
    let mut session = RenderSectionSession::default();
    session.apply_finished_compile_report(
        &BTreeSet::from([cached]),
        TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key: cached,
                mesh: TexturedVisibleChunkMesh::default(),
                grass_patches: Vec::new(),
                visibility: VisibilitySet::all_visible(),
            }],
            visibility_graph: VisibilityGraphBuildStats::default(),
        },
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    session.mark_chunk_dirty(loaded_chunk, [ready]);
    session.mark_chunk_dirty(removal_chunk, [cached]);

    let update = session.prepare_sync_update(
        |pos| pos == loaded_chunk,
        |key| key == ready,
        |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
        |dirty_work| {
            dirty_work
                .loaded_dirty_sections_by_chunk
                .keys()
                .copied()
                .collect()
        },
        1,
        |_| vec![ready],
        |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
        RenderSectionRemovalMode::Defer,
    );

    assert_eq!(update.cache_update.removed_section_count(), 0);
    assert!(session.contains_section(cached));
    assert_eq!(
        update.sync_plan.dirty_work.removal_dirty_chunks,
        BTreeSet::from([removal_chunk])
    );
    assert!(session.dirty().dirty_chunks.contains(&removal_chunk));
}

#[test]
fn render_section_compile_finish_reports_accepted_and_stale_sections() {
    let accepted = RenderSectionKey::new(0, 4, 0);
    let stale = RenderSectionKey::new(0, 5, 0);
    let mut dirty = RenderSectionDirtyState::default();
    dirty.mark_section_dirty(accepted);
    dirty.mark_section_dirty(stale);
    let request = dirty.build_compile_request(BTreeSet::from([accepted, stale]), Vec::new());
    dirty.mark_compile_submitted(&request.target_sections);
    dirty.mark_section_dirty(stale);

    let finish = finish_render_section_compile_result(
        &mut dirty,
        RenderSectionCompileResult {
            target_sections: request.target_sections,
            section_revisions: request.section_revisions,
            result: Ok(TexturedRenderSectionBuildReport::default()),
        },
        7,
    )
    .unwrap();

    assert_eq!(finish.accepted_sections, BTreeSet::from([accepted]));
    assert_eq!(finish.stale_sections, BTreeSet::from([stale]));
    assert!(finish.build_report.is_some());
    assert_eq!(
        finish.acceptance_report,
        RenderSectionCompileAcceptanceReport {
            request_id: 7,
            submitted_section_count: 2,
            accepted_section_count: 1,
            stale_section_count: 1,
        }
    );
    assert!(dirty.inflight_sections.is_empty());
}

#[test]
fn render_dirty_chunk_neighborhood_includes_cardinal_neighbors() {
    assert_eq!(
        render_dirty_chunk_neighborhood(ChunkPos::new(4, -2)),
        [
            ChunkPos::new(4, -2),
            ChunkPos::new(3, -2),
            ChunkPos::new(5, -2),
            ChunkPos::new(4, -3),
            ChunkPos::new(4, -1),
        ]
    );
}

#[test]
fn render_section_compile_request_state_tracks_pending_revisions_and_stale_results() {
    let key = RenderSectionKey::new(0, 4, 0);
    let mut state = RenderSectionCompileRequestState::default();

    let first = state.begin_request("first", BTreeSet::from([key]));
    assert_eq!(
        first,
        RenderSectionCompileRequestInfo {
            request_id: 1,
            submitted_section_count: 1,
            pending_compile_jobs: 1,
        }
    );
    assert_eq!(state.section_revision(key), 1);
    let pending = state.remove_pending_request(first.request_id).unwrap();
    assert_eq!(pending.context, "first");
    let (_, completed) = pending.into_compile_result(Ok(TexturedRenderSectionBuildReport {
        sections: Vec::new(),
        visibility_graph: VisibilityGraphBuildStats::default(),
    }));
    let accepted = completed.partition_by_revision(|key| state.section_revision(key));
    assert_eq!(accepted.accepted_sections, BTreeSet::from([key]));
    assert!(accepted.stale_sections.is_empty());

    let second = state.begin_request("second", BTreeSet::from([key]));
    let pending = state.remove_pending_request(second.request_id).unwrap();
    state.bump_section_revisions(&BTreeSet::from([key]));
    let (_, completed) =
        pending.into_compile_result(Ok(TexturedRenderSectionBuildReport::default()));
    let stale = completed.partition_by_revision(|key| state.section_revision(key));
    assert!(stale.accepted_sections.is_empty());
    assert_eq!(stale.stale_sections, BTreeSet::from([key]));
}

#[test]
fn render_section_compile_request_state_accepts_prepared_dirty_request() {
    let key = RenderSectionKey::new(0, 4, 0);
    let mut dirty = RenderSectionDirtyState::default();
    dirty.mark_section_dirty(key);
    let request = dirty.build_compile_request(BTreeSet::from([key]), Vec::new());
    let mut state = RenderSectionCompileRequestState::default();

    let info = state.begin_compile_request("prepared", request);

    assert_eq!(
        info,
        RenderSectionCompileRequestInfo {
            request_id: 1,
            submitted_section_count: 1,
            pending_compile_jobs: 1,
        }
    );
    let pending = state.remove_pending_request(info.request_id).unwrap();
    assert_eq!(pending.context, "prepared");
    let (_, completed) =
        pending.into_compile_result(Ok(TexturedRenderSectionBuildReport::default()));
    let accepted = dirty.accept_completed_compile_result(&completed);

    assert_eq!(accepted.accepted_sections, BTreeSet::from([key]));
    assert!(accepted.stale_sections.is_empty());
}

#[test]
fn render_section_key_helpers_select_dirty_snapshot_sections() {
    let blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
    let clean = ChunkSnapshot::from_block_state_ids(
        ChunkPos::new(0, 0),
        ChunkStatus::Surface,
        ChunkRevision(1),
        0,
        32,
        &blocks,
    );
    let dirty = ChunkSnapshot::from_block_state_ids(
        ChunkPos::new(1, 0),
        ChunkStatus::Surface,
        ChunkRevision(1),
        -16,
        32,
        &blocks,
    );

    let keys = target_section_keys_for_dirty_chunks([&clean, &dirty], &BTreeSet::from([dirty.pos]));

    assert_eq!(
        keys,
        BTreeSet::from([
            RenderSectionKey::new(1, -1, 0),
            RenderSectionKey::new(1, 0, 0),
        ])
    );
    assert!(snapshot_contains_render_section(
        &dirty,
        RenderSectionKey::new(1, -1, 0)
    ));
    assert_eq!(
        render_section_chunk_pos(RenderSectionKey::new(1, 0, 0)),
        dirty.pos
    );
}
