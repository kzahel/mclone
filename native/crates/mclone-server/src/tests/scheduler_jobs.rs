use super::support::*;

#[test]
fn scheduled_fluid_tick_survives_fully_unloaded_chunk_until_reload() {
    let root = unique_temp_dir("scheduled_fluid_tick_survives_fully_unloaded_chunk_until_reload");
    let original_interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };
    let source = WorldBlockPos::new(8, 120, 8);
    let below = source.below();
    let mut server = LocalRealmSession::with_world_store(
        12_345,
        Box::new(FilesystemChunkSnapshotStore::new(&root)),
    );
    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(original_interest.clone()),
    );
    server.liquid_ticks = FluidTickList::new();
    server.scheduler_mut().set_block_at_world(source, WATER);
    server.scheduler_mut().set_block_at_world(below, AIR);
    server.schedule_fluid_tick(source, FluidKind::Water, 0);

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(100, 100),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    let deferred = server.simulation_tick_report();
    assert_eq!(deferred.fluid_ticks_executed, 0);
    assert_eq!(deferred.deferred_fluid_ticks, 1);
    assert_eq!(deferred.scheduled_fluid_ticks, 1);
    assert!(
        server.scheduler().holder(ChunkPos::new(0, 0)).is_some(),
        "dirty pending-unload holders should stay resident until the durable save acks"
    );
    try_poll_server_until_persistence_idle(&mut server).unwrap();
    for _ in 0..32 {
        if server.scheduler().holder(ChunkPos::new(0, 0)).is_none() {
            break;
        }
        server.simulation_tick_report();
        try_poll_server_until_persistence_idle(&mut server).unwrap();
    }
    assert!(
        server.scheduler().holder(ChunkPos::new(0, 0)).is_none(),
        "the original chunk should be fully removed from scheduler holders"
    );
    assert!(
        !server
            .liquid_ticks
            .has_scheduled_tick(source, FluidKind::Water),
        "live fluid ticks for a fully unloaded chunk should be packed into storage and pruned"
    );

    let updates =
        handle_command_and_poll(&mut server, ClientCommand::SetChunkView(original_interest));
    assert!(
        snapshot_update_for(&updates, ChunkPos::new(0, 0)).is_some(),
        "the original chunk should reload from the snapshot store"
    );
    assert!(
        server
            .liquid_ticks
            .has_scheduled_tick(source, FluidKind::Water),
        "the saved due fluid tick should hydrate from the stored chunk record"
    );
    let executed = server.simulation_tick_report();

    assert!(
        executed.fluid_ticks_executed >= 1,
        "the hydrated source tick should execute after the chunk becomes entity-ticking"
    );
    assert_eq!(
        server.scheduler().block_at_world(below),
        Some(WATER_LEVEL_8),
        "the overdue fluid tick should run after the chunk becomes entity-ticking again"
    );
    let pending_after_execution = server
        .liquid_ticks
        .scheduled_tick_entries(server.simulation_tick());
    assert!(
        !pending_after_execution.contains(&(source, FluidKind::Water, 0)),
        "the overdue zero-delay source tick should be consumed: {pending_after_execution:?}"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn integrated_server_tick_uses_simulation_layer() {
    let mut server = LocalRealmSession::new(12_345);

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    assert_eq!(server.simulation_tick(), 0);
    let _ = server.tick();
    assert_eq!(server.simulation_tick(), 1);
    let _ = server.tick();
    assert_eq!(server.simulation_tick(), 2);
}

#[test]
fn scheduler_tick_report_counts_bounded_pending_unload_work() {
    let mut scheduler = ChunkScheduler::new(12_345);

    scheduler
        .add_region_ticket(ChunkTicketType::Unknown, ChunkPos::new(0, 0), 0)
        .unwrap();
    poll_scheduler_until_idle(&mut scheduler);

    let first_report = scheduler.tick_report().unwrap();
    assert_eq!(first_report.ticket_tick, 1);
    assert_eq!(first_report.pending_unloads_processed, 0);
    assert!(first_report.block_ticking_chunks.is_empty());
    assert!(first_report.entity_ticking_chunks.is_empty());
    assert!(first_report.events.is_empty());

    let second_report = scheduler.tick_report().unwrap();
    assert_eq!(second_report.ticket_tick, 2);
    assert_eq!(
        second_report.pending_unloads_processed,
        DEFAULT_PENDING_UNLOAD_BUDGET
    );
    assert!(second_report.block_ticking_chunks.is_empty());
    assert!(second_report.entity_ticking_chunks.is_empty());
    assert_eq!(
        second_report
            .events
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::HolderUnloaded { .. }))
            .count(),
        DEFAULT_PENDING_UNLOAD_BUDGET
    );
    assert_eq!(
        scheduler.pending_unload_count(),
        active_ticket_square_count(CHUNK_LEVEL_FULL) - DEFAULT_PENDING_UNLOAD_BUDGET
    );
}

#[test]
fn duplicate_interest_does_not_regenerate_loaded_chunks() {
    let mut server = LocalRealmSession::new(12_345);
    let interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };

    let first_updates =
        handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest.clone()));
    assert!(snapshot_update_for(&first_updates, ChunkPos::new(0, 0)).is_some());
    assert_eq!(server.scheduler().job_count(), 1);
    assert_eq!(
        handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest)).len(),
        0
    );
    assert_eq!(server.scheduler().job_count(), 1);
}

#[test]
fn adjacent_interest_reuses_retained_dependency_chunks() {
    let mut server = LocalRealmSession::new(12_345);

    let first_updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    assert!(snapshot_update_for(&first_updates, ChunkPos::new(0, 0)).is_some());
    assert_eq!(
        server.scheduler().job(ChunkJobId(1)).map(|job| (
            job.seeded_dependency_chunks,
            job.dependency_cache_hits,
            job.dependency_cache_misses,
            job.retained_dependency_chunks
        )),
        Some((0, 0, 7 * 7, 7 * 7))
    );

    let moved_updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    assert!(snapshot_update_for(&moved_updates, ChunkPos::new(1, 0)).is_some());
    assert_eq!(
        server.scheduler().job(ChunkJobId(2)).map(|job| (
            job.seeded_dependency_chunks,
            job.dependency_cache_hits,
            job.dependency_cache_misses,
            job.retained_dependency_chunks
        )),
        Some((4 * 7, 4 * 7, 7, 5 * 7))
    );
}

#[test]
fn chunk_scheduler_apply_interest_enqueues_features_before_poll() {
    let mut scheduler = ChunkScheduler::new(12_345);

    let events = scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();

    // The cold Player promotion is admitted immediately but remains active
    // until its runtime dependency ring reaches Light.
    assert_eq!(scheduler.pending_job_count(), 1);
    assert_eq!(scheduler.pending_persistence_load_count(), 9);
    assert_eq!(scheduler.loaded_chunk_count(), 0);
    assert_eq!(scheduler.job_count(), 0);
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
    );
    assert_eq!(events.len(), 9 * 4);
    assert_eq!(
        status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
        0
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Scheduled),
        0
    );

    let mut ready_events = scheduler.poll().unwrap();
    assert_eq!(scheduler.pending_persistence_load_count(), 0);
    assert_eq!(scheduler.pending_job_count(), 2);
    assert_eq!(
        scheduler.job(ChunkJobId(1)).map(|job| job.state),
        Some(ChunkJobState::Running)
    );
    ready_events.extend(poll_scheduler_until_idle(&mut scheduler));
    assert_eq!(scheduler.pending_job_count(), 0);
    assert_eq!(scheduler.loaded_chunk_count(), 9);
    assert!(
        ready_events
            .iter()
            .any(|event| matches!(event, ChunkSchedulerEvent::FluidTickScheduled { .. }))
    );
    let ready_events_without_fluid = without_fluid_tick_events(&ready_events);
    assert_eq!(
        status_event_count(
            &ready_events_without_fluid,
            ChunkStatus::Features,
            ChunkStatusStep::Ready
        ),
        9
    );
    assert_eq!(
        status_event_count(
            &ready_events_without_fluid,
            ChunkStatus::Light,
            ChunkStatusStep::Ready
        ),
        9
    );
    assert_eq!(snapshot_ready_count(&ready_events_without_fluid), 1);
    assert!(ready_events_without_fluid.iter().any(|event| matches!(
        event,
        ChunkSchedulerEvent::SnapshotReady(snapshot)
            if snapshot.status == ChunkStatus::Light && snapshot.light_correct
    )));
}

#[test]
fn chunk_scheduler_can_publish_features_when_lighting_disabled() {
    let mut scheduler = ChunkScheduler::new(12_345);
    scheduler.set_lighting_enabled(false);

    let events = apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    assert_eq!(scheduler.pending_job_count(), 0);
    assert_eq!(scheduler.loaded_chunk_count(), 9);
    assert_eq!(
        status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Scheduled),
        9
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Features, ChunkStatusStep::Ready),
        9
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Scheduled),
        0
    );
    assert_eq!(
        status_event_count(&events, ChunkStatus::Light, ChunkStatusStep::Ready),
        0
    );
    assert!(events.iter().any(|event| matches!(
        event,
        ChunkSchedulerEvent::SnapshotReady(snapshot)
            if snapshot.pos == ChunkPos::new(0, 0)
                && snapshot.status == ChunkStatus::Features
                && !snapshot.light_correct
                && snapshot.light_sections.is_empty()
    )));

    let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
    assert_eq!(holder.target_status(), Some(ChunkStatus::Features));
    assert!(holder.status_slot(ChunkStatus::Light).is_none());
}

#[test]
fn chunk_scheduler_poll_slices_completed_publication() {
    let mut scheduler = ChunkScheduler::new(12_345);
    scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();
    let scheduled_events = scheduler.poll().unwrap();
    assert_eq!(
        status_event_count(
            &scheduled_events,
            ChunkStatus::Features,
            ChunkStatusStep::Scheduled
        ),
        9
    );
    assert!(scheduler.wait_for_worldgen_completion(std::time::Duration::from_secs(30)));

    let first_report = scheduler.tick_report().unwrap();
    let first_events = first_report.events;

    assert_eq!(
        scheduler.loaded_chunk_count(),
        DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET
    );
    assert_eq!(first_report.publication.completed_feature_jobs_drained, 1);
    assert_eq!(
        first_report.publication.feature_chunks_published,
        DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET
    );
    assert_eq!(first_report.publication.feature_chunks_skipped, 0);
    assert_eq!(first_report.publication.feature_jobs_completed, 0);
    assert_eq!(
        first_report.publication.pending_worldgen_publication_chunks,
        9 - DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET
    );
    assert!(scheduler.pending_publication_count() > 0);
    assert_eq!(scheduler.pending_job_count(), 3);
    assert!(
        without_fluid_tick_events(&first_events)
            .iter()
            .filter(|event| matches!(event, ChunkSchedulerEvent::StatusChanged { .. }))
            .count()
            <= DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET * 2
    );

    poll_scheduler_until_idle(&mut scheduler);
    assert_eq!(scheduler.pending_publication_count(), 0);
    assert_eq!(scheduler.pending_job_count(), 0);
    assert_eq!(scheduler.loaded_chunk_count(), 9);
}

#[test]
fn chunk_interest_updates_player_tickets_and_holder_levels() {
    let mut scheduler = ChunkScheduler::new(12_345);

    let events = scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();

    assert_eq!(scheduler.ticketed_chunk_count(), 1);
    assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 1);
    assert_eq!(
        scheduler.ticket_level_at(ChunkPos::new(0, 0)),
        PLAYER_TICKET_LEVEL
    );
    let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
    assert_eq!(holder.ticket_level(), PLAYER_TICKET_LEVEL);
    assert_eq!(holder.full_status(), FullChunkStatus::EntityTicking);
    assert!(holder.full_status().is_or_after(FullChunkStatus::Ticking));
    assert_eq!(events.len(), 9 * 4);

    let moved_events = scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();

    assert_eq!(scheduler.ticketed_chunk_count(), 1);
    assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
    assert_eq!(scheduler.ticket_count_at(ChunkPos::new(1, 0)), 1);
    assert_eq!(
        scheduler.ticket_level_at(ChunkPos::new(0, 0)),
        UNLOADED_CHUNK_LEVEL
    );
    assert_eq!(
        scheduler.active_ticket_level_at(ChunkPos::new(0, 0)),
        PLAYER_TICKET_LEVEL + 1
    );
    assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
    assert_eq!(
        scheduler
            .holder(ChunkPos::new(1, 0))
            .map(ChunkHolder::ticket_level),
        Some(PLAYER_TICKET_LEVEL)
    );
    assert!(
        moved_events
            .iter()
            .all(|event| !matches!(event, ChunkSchedulerEvent::Unloaded { .. }))
    );
}

#[test]
fn duplicate_interest_while_job_running_does_not_enqueue_second_job() {
    let mut scheduler = ChunkScheduler::new(12_345);
    let interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };

    let first_events = scheduler.apply_interest(interest.clone()).unwrap();
    assert_eq!(first_events.len(), 9 * 4);
    assert_eq!(scheduler.pending_persistence_load_count(), 9);
    let scheduled_events = scheduler.poll().unwrap();
    assert_eq!(
        status_event_count(
            &scheduled_events,
            ChunkStatus::Features,
            ChunkStatusStep::Scheduled
        ),
        9
    );
    assert_eq!(scheduler.pending_job_count(), 2);
    assert_eq!(scheduler.job_count(), 1);

    let second_events = scheduler.apply_interest(interest).unwrap();
    assert_eq!(second_events, Vec::new());
    assert_eq!(scheduler.pending_job_count(), 2);
    assert_eq!(scheduler.job_count(), 1);

    assert_eq!(
        without_fluid_tick_events(&poll_scheduler_until_idle(&mut scheduler)).len(),
        28
    );
    assert_eq!(scheduler.loaded_chunk_count(), 9);
    assert_eq!(scheduler.job_count(), 1);
}

#[test]
fn forced_ticket_keeps_chunk_resident_after_interest_moves() {
    let mut scheduler = ChunkScheduler::new(12_345);

    apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );
    assert!(
        scheduler
            .set_chunk_forced(ChunkPos::new(0, 0), true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 2);
    assert_eq!(
        scheduler.ticket_level_at(ChunkPos::new(0, 0)),
        FORCED_TICKET_LEVEL
    );

    let events = apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    assert!(events.iter().any(
        |event| matches!(event, ChunkSchedulerEvent::Unloaded { pos } if *pos == ChunkPos::new(0, 0))
    ));
    assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
    assert!(scheduler.holder(ChunkPos::new(1, 0)).is_some());
    assert_eq!(scheduler.loaded_chunk_count(), 12);

    let events = scheduler
        .set_chunk_forced(ChunkPos::new(0, 0), false)
        .unwrap();

    assert!(events.is_empty());
    assert_eq!(scheduler.ticket_count_at(ChunkPos::new(0, 0)), 0);
    assert_eq!(
        scheduler.active_ticket_level_at(ChunkPos::new(0, 0)),
        PLAYER_TICKET_LEVEL + 1
    );
    assert!(scheduler.holder(ChunkPos::new(0, 0)).is_some());
}
