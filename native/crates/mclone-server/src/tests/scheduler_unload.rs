use super::support::*;

#[test]
fn stale_unknown_ticket_expires_on_scheduler_tick() {
    let mut scheduler = ChunkScheduler::new(12_345);

    let events = scheduler
        .add_region_ticket(ChunkTicketType::Unknown, ChunkPos::new(0, 0), 0)
        .unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(scheduler.ticketed_chunk_count(), 1);
    assert_eq!(scheduler.ticket_level_at(ChunkPos::new(0, 0)), 33);
    assert_eq!(
        without_fluid_tick_events(&poll_scheduler_until_idle(&mut scheduler)).len(),
        4
    );
    assert_eq!(scheduler.loaded_chunk_count(), 1);
    assert_eq!(scheduler.client_visible_chunk_count(), 0);

    assert!(scheduler.tick().unwrap().is_empty());
    assert_eq!(scheduler.ticket_tick(), 1);
    assert_eq!(scheduler.ticketed_chunk_count(), 1);

    let events = scheduler.tick().unwrap();

    assert_eq!(scheduler.ticket_tick(), 2);
    assert_eq!(scheduler.ticketed_chunk_count(), 0);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::HolderUnloaded { .. }))
            .count(),
        DEFAULT_PENDING_UNLOAD_BUDGET
    );
    assert_eq!(
        scheduler.pending_unload_count(),
        active_ticket_square_count(CHUNK_LEVEL_FULL) - DEFAULT_PENDING_UNLOAD_BUDGET
    );

    scheduler.process_pending_unloads(usize::MAX).unwrap();
    assert_eq!(scheduler.pending_unload_count(), 0);
    assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());
}

#[test]
fn expired_ticket_queues_pending_unload_until_processed() {
    let mut scheduler = ChunkScheduler::new(12_345);
    let pos = ChunkPos::new(0, 0);

    scheduler
        .add_region_ticket(ChunkTicketType::Unknown, pos, 0)
        .unwrap();
    poll_scheduler_until_idle(&mut scheduler);
    let active_count = active_ticket_square_count(CHUNK_LEVEL_FULL);
    assert_eq!(scheduler.holder_count(), active_count);
    assert_eq!(scheduler.pending_unload_count(), 0);
    assert_eq!(scheduler.loaded_chunk_count(), 1);

    scheduler.distance_manager.purge_stale_tickets();
    scheduler.distance_manager.purge_stale_tickets();
    assert!(scheduler.reconcile_ticketed_holders().unwrap().is_empty());

    assert_eq!(scheduler.ticketed_chunk_count(), 0);
    assert_eq!(scheduler.pending_unload_count(), active_count);
    assert_eq!(scheduler.holder_count(), active_count);
    assert!(scheduler.is_pending_unload(pos));
    assert_eq!(
        scheduler.holder(pos).map(ChunkHolder::ticket_level),
        Some(UNLOADED_CHUNK_LEVEL)
    );
    assert_eq!(scheduler.loaded_chunk_count(), 1);

    assert_eq!(scheduler.process_pending_unloads(10).unwrap(), 10);
    assert_eq!(scheduler.pending_unload_count(), active_count - 10);

    assert_eq!(
        scheduler.process_pending_unloads(usize::MAX).unwrap(),
        active_count - 10
    );
    assert_eq!(scheduler.pending_unload_count(), 0);
    assert_eq!(scheduler.holder_count(), 0);
    assert_eq!(scheduler.loaded_chunk_count(), 0);
    assert_eq!(scheduler.dirty_chunk_count(), 0);
    assert!(scheduler.holder(pos).is_none());
}

#[test]
fn pending_unload_holder_is_rescued_when_ticket_returns() {
    let mut scheduler = ChunkScheduler::new(12_345);
    let pos = ChunkPos::new(0, 0);

    scheduler
        .add_region_ticket(ChunkTicketType::Unknown, pos, 0)
        .unwrap();
    poll_scheduler_until_idle(&mut scheduler);

    scheduler.distance_manager.purge_stale_tickets();
    scheduler.distance_manager.purge_stale_tickets();
    scheduler.reconcile_ticketed_holders().unwrap();
    assert!(scheduler.is_pending_unload(pos));
    assert_eq!(
        scheduler.holder(pos).map(ChunkHolder::ticket_level),
        Some(UNLOADED_CHUNK_LEVEL)
    );

    scheduler.set_chunk_forced(pos, true).unwrap();
    poll_scheduler_until_idle(&mut scheduler);

    assert!(!scheduler.is_pending_unload(pos));
    assert_eq!(scheduler.pending_unload_count(), 0);
    assert_eq!(
        scheduler.holder(pos).map(ChunkHolder::ticket_level),
        Some(FORCED_TICKET_LEVEL)
    );
    assert_eq!(scheduler.loaded_chunk_count(), 9);
    scheduler.process_pending_unloads(usize::MAX).unwrap();
    assert!(scheduler.holder(pos).is_some());
}

#[test]
fn chunk_scheduler_records_holder_status_slots_in_order() {
    let mut scheduler = ChunkScheduler::new(12_345);

    let events = apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    assert_eq!(
        status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
        9
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Scheduled),
        9
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
        9
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Ready),
        9
    );
    assert!(events.iter().any(|event| matches!(
        event,
        ChunkSchedulerEvent::SnapshotReady(snapshot)
            if snapshot.pos == ChunkPos::new(0, 0)
                && snapshot.status == ChunkStatus::Light
                && snapshot.light_correct
    )));

    let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
    assert_eq!(holder.target_status(), Some(ChunkStatus::Light));
    assert_eq!(holder.ready_status_count(), 4);
    assert_eq!(
        holder.status_slot(ChunkStatus::Terrain),
        Some(&ChunkStatusSlot {
            status: ChunkStatus::Terrain,
            step: ChunkStatusStep::Ready,
            revision: None,
            job_id: None,
            light_request_token: None,
        })
    );
    assert_eq!(
        holder.status_slot(ChunkStatus::Surface),
        Some(&ChunkStatusSlot {
            status: ChunkStatus::Surface,
            step: ChunkStatusStep::Ready,
            revision: None,
            job_id: None,
            light_request_token: None,
        })
    );
    let features_slot = holder.status_slot(ChunkStatus::Features).unwrap();
    assert_eq!(features_slot.status, ChunkStatus::Features);
    assert_eq!(features_slot.step, ChunkStatusStep::Ready);
    assert_eq!(features_slot.job_id, Some(ChunkJobId(1)));
    let feature_revision = features_slot
        .revision
        .expect("features should have revision");

    let light_slot = holder.status_slot(ChunkStatus::Light).unwrap();
    assert_eq!(light_slot.status, ChunkStatus::Light);
    assert_eq!(light_slot.step, ChunkStatusStep::Ready);
    assert_eq!(light_slot.job_id, None);
    assert!(
        light_slot.revision.expect("light should have revision") > feature_revision,
        "light status must be published after the feature snapshot it consumes"
    );
    assert_eq!(
        first_status_event_pos(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
        Some(ChunkPos::new(0, 0))
    );
    assert_eq!(
        first_status_event_pos(&events, ChunkStatus::Light, ChunkStatusStep::Ready),
        Some(ChunkPos::new(0, 0))
    );
    assert_eq!(scheduler.job_count(), 1);
    let job = scheduler.job(ChunkJobId(1)).unwrap();
    assert_eq!(job.id, ChunkJobId(1));
    assert_eq!(job.status, ChunkStatus::Features);
    assert_eq!(job.state, ChunkJobState::Complete);
    assert_eq!(job.target_chunks.first(), Some(&ChunkPos::new(0, 0)));
    assert_eq!(
        job.target_chunks.iter().copied().collect::<BTreeSet<_>>(),
        {
            (-1..=1)
                .flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)))
                .collect()
        }
    );
    assert_eq!(job.feature_centers.first(), Some(&ChunkPos::new(0, 0)));
    assert_eq!(job.feature_centers.len(), 5 * 5);
    assert_eq!(job.dependency_chunks.first(), Some(&ChunkPos::new(0, 0)));
    assert_eq!(job.dependency_chunks.len(), 7 * 7);
    assert_eq!(job.seeded_dependency_chunks, 0);
    assert_eq!(job.dependency_cache_hits, 0);
    assert_eq!(job.dependency_cache_misses, 7 * 7);
    assert_eq!(job.retained_dependency_chunks, 7 * 7);
}

#[test]
fn chunk_scheduler_coalesces_duplicate_status_requests() {
    let mut scheduler = ChunkScheduler::new(12_345);
    let interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };

    assert_eq!(
        without_fluid_tick_events(&apply_interest_and_poll(&mut scheduler, interest.clone())).len(),
        73
    );
    assert_eq!(
        apply_interest_and_poll(&mut scheduler, interest),
        Vec::new()
    );
    assert_eq!(scheduler.loaded_chunk_count(), 9);
}
