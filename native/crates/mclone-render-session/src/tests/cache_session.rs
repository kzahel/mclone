use super::*;

#[test]
fn cached_sections_apply_build_report_and_remove_unloaded_chunks() {
    let mut cache = CachedTexturedRenderSections::default();
    let key = RenderSectionKey::new(2, 4, -1);
    let second_key = RenderSectionKey::new(2, 5, -1);
    let chunk = ChunkPos::new(2, -1);
    let report = test_build_report([key, second_key]);

    let update = cache.apply_build_report(
        &BTreeSet::from([key, second_key]),
        report,
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    assert_eq!(update.rebuilt_section_count(), 2);
    assert!(cache.contains_section(key));
    assert!(cache.contains_section(second_key));
    assert_eq!(cache.generation(), 2);
    assert!(cache.contains_chunk(chunk));
    assert_eq!(
        cache.section_keys_for_chunk(chunk).collect::<BTreeSet<_>>(),
        BTreeSet::from([key, second_key])
    );

    let replacement = cache.apply_build_report(
        &BTreeSet::from([key]),
        test_build_report([key]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    assert_eq!(replacement.rebuilt_section_count(), 1);
    assert_eq!(cache.generation(), 2);

    let section_removal = cache.remove_sections(&BTreeSet::new(), &BTreeSet::from([key]));

    assert_eq!(section_removal.removed_section_count(), 1);
    assert!(!cache.contains_section(key));
    assert!(cache.contains_section(second_key));
    assert_eq!(cache.generation(), 3);
    assert!(cache.contains_chunk(chunk));
    assert_eq!(
        cache.section_keys_for_chunk(chunk).collect::<BTreeSet<_>>(),
        BTreeSet::from([second_key])
    );

    let removal = cache.remove_sections(&BTreeSet::from([chunk]), &BTreeSet::new());

    assert_eq!(removal.removed_section_count(), 1);
    assert!(!cache.contains_section(second_key));
    assert!(!cache.contains_chunk(chunk));
    assert!(cache.section_keys_for_chunk(chunk).next().is_none());
    assert_eq!(cache.generation(), 4);
}

#[test]
fn near_camera_readiness_columns_track_exception_membership() {
    let camera = Vec3::new(8.0, 64.0, 8.0);
    let near_columns = render_section_near_camera_readiness_columns(camera);

    assert!(near_columns.contains(&ChunkPos::new(0, 0)));
    assert!(near_columns.contains(&ChunkPos::new(1, 0)));
    assert!(!near_columns.contains(&ChunkPos::new(2, 0)));

    let key = RenderSectionKey::new(1, 4, 0);
    assert_eq!(
        render_section_neighbor_readiness(&ClientRuntime::local_integrated(), key, camera),
        RenderSectionNeighborReadiness::ReadyNearCamera
    );
}

#[test]
fn render_section_session_owns_dirty_compile_and_cache_updates() {
    let mut session = RenderSectionSession::default();
    let key = RenderSectionKey::new(2, 4, -1);
    let pos = ChunkPos::new(2, -1);
    let ready_plan = RenderSectionReadyPlan {
        ready_section_keys: BTreeSet::from([key]),
        ..RenderSectionReadyPlan::default()
    };

    session.mark_section_dirty(key);
    let submission = session
        .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |request| {
            Ok::<_, std::convert::Infallible>(request)
        })
        .expect("compile submission should not fail")
        .expect("ready section should build a compile request");
    assert_eq!(submission.submitted_section_count, 1);
    let request = submission.output;
    assert!(session.dirty().inflight_sections.contains(&key));

    let finish = session
        .finish_compile_result(
            RenderSectionCompileResult {
                target_sections: request.target_sections,
                section_revisions: request.section_revisions,
                result: Ok(TexturedRenderSectionBuildReport {
                    sections: vec![TexturedRenderSectionMesh {
                        key,
                        mesh: TexturedVisibleChunkMesh {
                            vertices: vec![TexturedChunkVertex {
                                position: [0.0, 0.0, 0.0],
                                uv: [0.0, 0.0],
                                color: [1.0, 1.0, 1.0, 1.0],
                                packed_light: 0,
                            }],
                            indices: vec![0],
                            solid_index_count: 1,
                            opaque_index_count: 1,
                        },
                        visibility: VisibilitySet::all_visible(),
                    }],
                    visibility_graph: VisibilityGraphBuildStats::default(),
                }),
            },
            9,
        )
        .expect("compile finish should be accepted");

    assert_eq!(finish.accepted_sections, BTreeSet::from([key]));
    assert!(finish.stale_sections.is_empty());
    let update = session.apply_finished_compile_report(
        &finish.accepted_sections,
        finish.build_report.expect("accepted build report"),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    assert_eq!(update.rebuilt_section_count(), 1);
    assert!(session.contains_section(key));
    assert!(session.contains_chunk(pos));
    assert!(session.dirty_is_empty());

    session.mark_section_dirty(key);
    let removal = session.apply_removals(&BTreeSet::from([pos]), &BTreeSet::new());

    assert_eq!(removal.removed_section_count(), 1);
    assert!(!session.contains_section(key));
    assert!(!session.contains_chunk(pos));
    assert!(session.dirty_is_empty());
}

#[test]
fn render_section_session_collects_known_keys_and_requeues_stale_sections() {
    let mut session = RenderSectionSession::default();
    let pos = ChunkPos::new(2, -1);
    let cached = RenderSectionKey::new(2, 4, -1);
    let loaded = RenderSectionKey::new(2, 5, -1);
    let dirty = RenderSectionKey::new(2, 6, -1);
    let inflight = RenderSectionKey::new(2, 7, -1);
    let stale_unloaded = RenderSectionKey::new(2, 8, -1);

    session.apply_finished_compile_report(
        &BTreeSet::from([cached]),
        TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key: cached,
                mesh: TexturedVisibleChunkMesh::default(),
                visibility: VisibilitySet::all_visible(),
            }],
            visibility_graph: VisibilityGraphBuildStats::default(),
        },
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    session.mark_section_dirty(dirty);
    session.mark_section_dirty(inflight);
    let inflight_plan = RenderSectionReadyPlan {
        ready_section_keys: BTreeSet::from([inflight]),
        ..RenderSectionReadyPlan::default()
    };
    session
        .submit_ready_plan_compile_request(&inflight_plan, Vec::new(), |request| {
            Ok::<_, std::convert::Infallible>(request)
        })
        .expect("compile submission should not fail")
        .expect("inflight section should build a compile request");

    assert_eq!(
        session.known_section_keys_for_chunk(pos, [loaded]),
        BTreeSet::from([cached, loaded, dirty, inflight])
    );

    let requeued = session
        .requeue_stale_sections(BTreeSet::from([cached, loaded, stale_unloaded]), |key| {
            key == loaded
        });

    assert_eq!(requeued, 2);
    assert_eq!(
        session
            .resident_dirty_section_keys()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([cached])
    );
    assert!(!session.dirty().dirty_sections.contains(&cached));
    assert!(session.dirty().dirty_sections.contains(&loaded));
    assert!(!session.dirty().dirty_sections.contains(&stale_unloaded));
}

#[test]
fn render_section_session_finish_compile_update_applies_and_requeues_stale_sections() {
    let accepted = RenderSectionKey::new(0, 4, 0);
    let stale = RenderSectionKey::new(0, 5, 0);
    let mut session = RenderSectionSession::default();
    let ready_plan = RenderSectionReadyPlan {
        ready_section_keys: BTreeSet::from([accepted, stale]),
        ..RenderSectionReadyPlan::default()
    };

    session.mark_section_dirty(accepted);
    session.mark_section_dirty(stale);
    let request = session
        .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |request| {
            Ok::<_, std::convert::Infallible>(request)
        })
        .expect("compile submission should not fail")
        .expect("ready sections should build a compile request")
        .output;
    session.mark_section_dirty(stale);
    let finished = session
        .finish_compile_update(
            RenderSectionCompileResult {
                target_sections: request.target_sections,
                section_revisions: request.section_revisions,
                result: Ok(test_build_report([accepted, stale])),
            },
            17,
            &BTreeSet::new(),
            &BTreeSet::new(),
            |key| key == stale,
        )
        .expect("compile update should finish");

    assert_eq!(
        finished.acceptance_report,
        RenderSectionCompileAcceptanceReport {
            request_id: 17,
            submitted_section_count: 2,
            accepted_section_count: 1,
            stale_section_count: 1,
        }
    );
    assert_eq!(finished.accepted_sections, BTreeSet::from([accepted]));
    assert_eq!(finished.stale_sections, BTreeSet::from([stale]));
    assert_eq!(finished.cache_update.rebuilt_section_count(), 1);
    assert_eq!(finished.cache_update.completed_compile_section_count, 1);
    assert_eq!(finished.cache_update.stale_compile_section_count, 1);
    assert!(session.contains_section(accepted));
    assert!(!session.contains_section(stale));
    assert!(!session.dirty().dirty_sections.contains(&accepted));
    assert!(session.dirty().dirty_sections.contains(&stale));
    assert!(session.dirty().inflight_sections.is_empty());
}

#[test]
fn render_section_session_finish_compile_update_applies_removals_without_accepted_sections() {
    let key = RenderSectionKey::new(1, 4, 0);
    let pos = ChunkPos::new(1, 0);
    let mut session = RenderSectionSession::default();
    session.apply_finished_compile_report(
        &BTreeSet::from([key]),
        test_build_report([key]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    let ready_plan = RenderSectionReadyPlan {
        ready_section_keys: BTreeSet::from([key]),
        ..RenderSectionReadyPlan::default()
    };

    session.mark_section_dirty(key);
    let request = session
        .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |request| {
            Ok::<_, std::convert::Infallible>(request)
        })
        .expect("compile submission should not fail")
        .expect("ready section should build a compile request")
        .output;
    session.mark_section_dirty(key);
    let finished = session
        .finish_compile_update(
            RenderSectionCompileResult {
                target_sections: request.target_sections,
                section_revisions: request.section_revisions,
                result: Ok(TexturedRenderSectionBuildReport::default()),
            },
            18,
            &BTreeSet::from([pos]),
            &BTreeSet::new(),
            |_| false,
        )
        .expect("removal-only compile update should finish");

    assert!(finished.accepted_sections.is_empty());
    assert_eq!(finished.stale_sections, BTreeSet::from([key]));
    assert_eq!(finished.cache_update.rebuilt_section_count(), 0);
    assert_eq!(finished.cache_update.removed_section_count(), 1);
    assert_eq!(finished.cache_update.stale_compile_section_count, 1);
    assert!(!session.contains_section(key));
    assert!(!session.dirty().dirty_sections.contains(&key));
    assert!(session.dirty().inflight_sections.is_empty());
}

#[test]
fn render_section_session_keeps_ready_work_dirty_when_submit_fails() {
    let mut session = RenderSectionSession::default();
    let key = RenderSectionKey::new(2, 4, -1);
    let ready_plan = RenderSectionReadyPlan {
        ready_section_keys: BTreeSet::from([key]),
        ..RenderSectionReadyPlan::default()
    };

    session.mark_section_dirty(key);
    let result: std::result::Result<Option<RenderSectionCompileSubmission<()>>, &str> = session
        .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |_request| {
            Err("submit failed")
        });

    assert_eq!(result, Err("submit failed"));
    assert!(session.dirty().dirty_sections.contains(&key));
    assert!(session.dirty().inflight_sections.is_empty());
}

#[test]
fn render_section_session_submit_prepared_sync_plan_applies_empty_ready_work() {
    let chunk = ChunkPos::new(0, 0);
    let deferred = RenderSectionKey::new(0, 4, 0);
    let mut session = RenderSectionSession::default();
    session.mark_chunk_dirty(chunk, [deferred]);
    let sync_update = session.prepare_sync_update(
        |pos| pos == chunk,
        |key| key == deferred,
        |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
        |dirty_work| {
            dirty_work
                .loaded_dirty_sections_by_chunk
                .keys()
                .copied()
                .collect()
        },
        1,
        |_| vec![deferred],
        |_| RenderSectionNeighborReadiness::DeferredMissingNeighbors,
        RenderSectionRemovalMode::Defer,
    );

    let result: std::result::Result<RenderSectionReadyWorkSubmission<()>, &str> = session
        .submit_prepared_sync_plan(
            &sync_update.sync_plan,
            Vec::new(),
            |_sync_plan, _request| panic!("empty ready plans must not submit compile requests"),
        );

    let update = result.expect("empty ready plan should be applied");
    assert!(update.submission.is_none());
    assert_eq!(update.cache_update.submitted_compile_section_count, 0);
    assert_eq!(update.cache_update.deferred_section_count, 1);
    assert_eq!(session.dirty().dirty_chunks, BTreeSet::from([chunk]));
    assert_eq!(session.dirty().dirty_sections, BTreeSet::from([deferred]));
    assert!(session.dirty().inflight_sections.is_empty());
}

#[test]
fn render_section_session_submit_prepared_sync_plan_marks_ready_work_inflight() {
    let key = RenderSectionKey::new(0, 4, 0);
    let mut session = RenderSectionSession::default();
    session.mark_section_dirty(key);
    let sync_update = session.prepare_sync_update(
        |_| true,
        |candidate| candidate == key,
        |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
        |dirty_work| {
            dirty_work
                .loaded_dirty_sections_by_chunk
                .keys()
                .copied()
                .collect()
        },
        1,
        |_| Vec::new(),
        |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
        RenderSectionRemovalMode::Defer,
    );

    let update = session
        .submit_prepared_sync_plan(&sync_update.sync_plan, Vec::new(), |_sync_plan, request| {
            Ok::<_, std::convert::Infallible>(request)
        })
        .expect("ready plan should submit");
    let submission = update
        .submission
        .expect("ready section should produce a compile request");

    assert_eq!(submission.submitted_section_count, 1);
    assert_eq!(submission.output.target_sections, BTreeSet::from([key]));
    assert_eq!(update.cache_update.submitted_compile_section_count, 1);
    assert!(session.dirty().dirty_sections.is_empty());
    assert_eq!(session.dirty().inflight_sections, BTreeSet::from([key]));
}
