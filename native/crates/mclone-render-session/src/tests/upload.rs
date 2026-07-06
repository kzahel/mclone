use super::*;

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
