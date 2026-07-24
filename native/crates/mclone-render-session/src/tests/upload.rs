use super::*;

#[test]
fn startup_seed_conversion_conserves_meshes_without_compile_releases() {
    let populated_key = RenderSectionKey::new(0, 4, 0);
    let empty_key = RenderSectionKey::new(0, 5, 0);
    let vertex = TexturedChunkVertex {
        position: [0.0, 0.0, 0.0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        packed_light: 0,
    };
    let populated = TexturedRenderSectionMesh {
        key: populated_key,
        mesh: TexturedVisibleChunkMesh {
            vertices: vec![vertex; 4],
            indices: vec![0, 1, 2, 2, 3, 0],
            solid_index_count: 6,
            opaque_index_count: 6,
        },
        grass_patches: Vec::new(),
        visibility: VisibilitySet::all_visible(),
    };
    let empty = test_section_mesh(empty_key);
    let update = RenderSectionCacheUpdate::from_startup_seed(vec![populated, empty]);

    assert_eq!(update.rebuilt_section_count(), 2);
    assert_eq!(update.rebuilt_vertex_count, 4);
    assert_eq!(update.rebuilt_index_count, 6);
    assert_eq!(update.accepted_compile_result_count, 0);
    assert_eq!(update.submitted_compile_section_count, 0);

    let mut coordinator = RenderSectionUploadCoordinator::default();
    let enqueue = coordinator.enqueue_cache_update(update);
    assert_eq!(enqueue.queued_lifecycle_items, 2);
    assert_eq!(enqueue.released_compile_jobs, 0);
    assert_eq!(coordinator.stats().held_compile_jobs, 0);

    let mut drained_sections = 0;
    let mut drained_vertices = 0;
    let mut drained_indices = 0;
    while coordinator.has_pending_work() {
        let drain = coordinator.drain_budgeted(Some(1), Some(1));
        drained_sections += drain.rebuilt_sections.len();
        for section in &drain.rebuilt_sections {
            let stats = section.stats();
            drained_vertices += stats.vertex_count;
            drained_indices += stats.index_count;
        }
        assert_eq!(
            coordinator.complete_applied_lifecycle_items(drain.lifecycle_item_count),
            0
        );
    }
    assert_eq!(drained_sections, 2);
    assert_eq!(drained_vertices, 4);
    assert_eq!(drained_indices, 6);
    assert_eq!(coordinator.stats().held_compile_jobs, 0);
}

#[test]
fn upload_coordinator_drains_removals_by_accept_budget_and_releases_after_batch_apply() {
    let first = RenderSectionKey::new(0, 4, 0);
    let second = RenderSectionKey::new(1, 4, 0);
    let mut update = RenderSectionCacheUpdate {
        accepted_compile_result_count: 2,
        ..RenderSectionCacheUpdate::default()
    };
    update.removed_section_keys = [first, second].into_iter().collect();
    let mut coordinator = RenderSectionUploadCoordinator::default();

    let enqueue = coordinator.enqueue_cache_update(update);
    assert_eq!(enqueue.released_compile_jobs, 0);
    assert_eq!(enqueue.queued_lifecycle_items, 2);
    assert_eq!(coordinator.queued_removed_section_count(), 2);
    assert_eq!(coordinator.stats().held_compile_jobs, 2);

    let drain = coordinator.drain_budgeted(None, Some(1));
    assert!(drain.rebuilt_sections.is_empty());
    assert_eq!(drain.removed_section_keys, [first].into_iter().collect());
    assert_eq!(drain.lifecycle_item_count, 1);
    assert_eq!(drain.phase_report.drained_lifecycle_items, 1);
    assert!(drain.phase_report.accept_limited);
    assert_eq!(
        coordinator.complete_applied_lifecycle_items(drain.lifecycle_item_count),
        0
    );
    assert_eq!(coordinator.queued_removed_section_count(), 1);
    assert_eq!(coordinator.stats().held_compile_jobs, 2);

    let drain = coordinator.drain_budgeted(None, Some(1));
    assert_eq!(drain.removed_section_keys, [second].into_iter().collect());
    assert!(!drain.phase_report.accept_limited);
    assert_eq!(
        coordinator.complete_applied_lifecycle_items(drain.lifecycle_item_count),
        2
    );
    assert!(!coordinator.has_pending_work());
    assert_eq!(coordinator.stats().held_compile_jobs, 0);
}

#[test]
fn upload_coordinator_superseded_removal_releases_prior_compile_job() {
    let key = RenderSectionKey::new(0, 4, 0);
    let mut removal = RenderSectionCacheUpdate {
        accepted_compile_result_count: 1,
        ..RenderSectionCacheUpdate::default()
    };
    removal.removed_section_keys = [key].into_iter().collect();
    let rebuild = RenderSectionCacheUpdate {
        rebuilt_sections: vec![test_section_mesh(key)],
        accepted_compile_result_count: 1,
        ..RenderSectionCacheUpdate::default()
    };
    let mut coordinator = RenderSectionUploadCoordinator::default();

    assert_eq!(
        coordinator
            .enqueue_cache_update(removal)
            .released_compile_jobs,
        0
    );
    assert_eq!(coordinator.queued_removed_section_count(), 1);
    let enqueue = coordinator.enqueue_cache_update(rebuild);
    assert_eq!(enqueue.released_compile_jobs, 1);
    assert_eq!(enqueue.superseded_lifecycle_items, 1);
    assert_eq!(coordinator.queued_removed_section_count(), 0);
    assert_eq!(coordinator.queued_upload_section_count(), 1);

    let drain = coordinator.drain_budgeted(Some(1), Some(1));
    assert_eq!(drain.rebuilt_sections.len(), 1);
    assert_eq!(drain.rebuilt_sections[0].key, key);
    assert!(drain.removed_section_keys.is_empty());
    assert_eq!(drain.phase_report.drained_upload_sections, 1);
    assert_eq!(
        coordinator.complete_applied_lifecycle_items(drain.lifecycle_item_count),
        1
    );
    assert!(!coordinator.has_pending_work());
}

#[test]
fn upload_coordinator_stats_track_pending_upload_mesh_bytes() {
    let key = RenderSectionKey::new(0, 4, 0);
    let mut vertices = Vec::with_capacity(2);
    vertices.push(TexturedChunkVertex {
        position: [0.0, 0.0, 0.0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        packed_light: 0,
    });
    let mut indices = Vec::with_capacity(6);
    indices.extend([0, 1, 2, 2, 3, 0]);
    let section = TexturedRenderSectionMesh {
        key,
        mesh: TexturedVisibleChunkMesh {
            vertices,
            indices,
            solid_index_count: 6,
            opaque_index_count: 6,
        },
        grass_patches: Vec::new(),
        visibility: VisibilitySet::all_visible(),
    };
    let expected_owned_bytes = section.estimated_owned_bytes();
    let update = RenderSectionCacheUpdate {
        rebuilt_sections: vec![section],
        accepted_compile_result_count: 1,
        ..RenderSectionCacheUpdate::default()
    };
    let mut coordinator = RenderSectionUploadCoordinator::default();

    coordinator.enqueue_cache_update(update);

    let stats = coordinator.stats();
    assert_eq!(stats.queued_upload_sections, 1);
    assert_eq!(stats.queued_upload_mesh_owned_bytes, expected_owned_bytes);

    let drain = coordinator.drain_budgeted(Some(1), Some(1));

    assert_eq!(
        drain.rebuilt_sections[0].estimated_owned_bytes(),
        expected_owned_bytes
    );
    assert_eq!(coordinator.stats().queued_upload_mesh_owned_bytes, 0);
}

#[test]
fn upload_frame_decision_unbudgeted_allows_runtime_sync_even_with_pending_work() {
    let key = RenderSectionKey::new(0, 4, 0);
    let mut update = RenderSectionCacheUpdate::default();
    update.removed_section_keys = [key].into_iter().collect();
    let mut coordinator = RenderSectionUploadCoordinator::default();
    coordinator.enqueue_cache_update(update);
    let policy = RenderSectionUploadFramePolicy::default();

    assert!(!coordinator.should_drain_before_runtime_sync(policy));
    let decision = coordinator.frame_decision_after_pre_sync_drain(policy, true, false);

    assert!(!decision.upload_backpressured);
    assert!(decision.should_sync_render_sections);
    assert!(decision.should_apply_section_update_after_sync);
    assert_eq!(decision.limit, RenderSectionUploadFrameLimit::None);
}

#[test]
fn upload_frame_decision_budgeted_backlog_blocks_runtime_sync_after_pre_drain() {
    let first = RenderSectionKey::new(0, 4, 0);
    let second = RenderSectionKey::new(1, 4, 0);
    let mut update = RenderSectionCacheUpdate::default();
    update.removed_section_keys = [first, second].into_iter().collect();
    let mut coordinator = RenderSectionUploadCoordinator::default();
    coordinator.enqueue_cache_update(update);
    let policy = RenderSectionUploadFramePolicy::new(None, Some(1));

    assert!(coordinator.should_drain_before_runtime_sync(policy));
    let drain = coordinator.drain_budgeted(policy.upload_budget, policy.accept_budget);
    assert_eq!(drain.lifecycle_item_count, 1);
    let decision = coordinator.frame_decision_after_pre_sync_drain(policy, true, true);

    assert!(decision.upload_backpressured);
    assert!(!decision.should_sync_render_sections);
    assert!(!decision.should_apply_section_update_after_sync);
    assert_eq!(decision.limit, RenderSectionUploadFrameLimit::UploadBacklog);
    assert_eq!(decision.queue_after_drain.queued_lifecycle_items, 1);
}

#[test]
fn upload_frame_decision_budgeted_empty_queue_allows_runtime_sync() {
    let coordinator = RenderSectionUploadCoordinator::default();
    let policy = RenderSectionUploadFramePolicy::new(Some(16), Some(64));

    assert!(!coordinator.should_drain_before_runtime_sync(policy));
    let decision = coordinator.frame_decision_after_pre_sync_drain(policy, true, false);

    assert!(!decision.upload_backpressured);
    assert!(decision.should_sync_render_sections);
    assert!(decision.should_apply_section_update_after_sync);
    assert_eq!(decision.limit, RenderSectionUploadFrameLimit::None);
}

#[test]
fn upload_phase_reports_ignore_empty_enqueue_and_empty_drain() {
    let mut coordinator = RenderSectionUploadCoordinator::default();

    assert_eq!(
        coordinator.enqueue_cache_update(RenderSectionCacheUpdate::default()),
        RenderSectionUploadPhaseReport::default()
    );
    let drain = coordinator.drain_budgeted(Some(16), Some(64));

    assert!(drain.is_empty());
    assert_eq!(
        drain.phase_report,
        RenderSectionUploadPhaseReport::default()
    );
}
