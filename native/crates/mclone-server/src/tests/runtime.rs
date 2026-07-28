use super::support::*;

#[test]
fn distinguishes_integrated_and_dedicated_modes() {
    assert_ne!(ServerMode::Integrated, ServerMode::Dedicated);
}

#[test]
fn chunk_scheduler_uses_platform_worldgen_mailbox() {
    let scheduler = ChunkScheduler::new(12_345);

    #[cfg(not(target_arch = "wasm32"))]
    {
        assert_eq!(
            scheduler.worldgen_mailbox_kind(),
            WorldgenMailboxKind::NativeThread
        );
        assert_eq!(
            scheduler.light_status_mailbox_kind(),
            LightStatusMailboxKind::NativeThread
        );
    }
    #[cfg(target_arch = "wasm32")]
    {
        assert_eq!(
            scheduler.worldgen_mailbox_kind(),
            WorldgenMailboxKind::Inline
        );
        assert_eq!(
            scheduler.light_status_mailbox_kind(),
            LightStatusMailboxKind::Inline
        );
    }
}

#[test]
fn integrated_server_publishes_interested_chunks() {
    let mut server = LocalRealmSession::new(12_345);

    let updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 1,
            chunk_tracking_radius: 1,
        }),
    );

    assert!(matches!(
        updates.first(),
        Some(ServerUpdate::SessionConfiguration(_))
    ));
    assert!(matches!(updates.get(1), Some(ServerUpdate::SessionReady)));
    assert!(matches!(
        updates.get(2),
        Some(ServerUpdate::WorldInfo { .. })
    ));
    assert!(matches!(
        updates.get(3),
        Some(ServerUpdate::TimeUpdate { .. })
    ));
    assert!(matches!(
        updates.get(4),
        Some(ServerUpdate::PlayerLife(life)) if !life.vitals().is_dead()
    ));

    assert_eq!(
        updates
            .iter()
            .filter(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
            .count(),
        9
    );
    assert_eq!(
        updates
            .iter()
            .filter(|update| matches!(update, ServerUpdate::EntitySnapshot(_)))
            .count(),
        2
    );
    assert_eq!(server.loaded_chunk_count(), 25);
    assert_eq!(server.scheduler().client_visible_chunk_count(), 9);
    assert_eq!(server.scheduler().holder_count(), 29 * 29);
    assert_eq!(server.scheduler().active_ticketed_chunk_count(), 29 * 29);
    assert_eq!(
        (
            server
                .scheduler()
                .full_status_chunk_count(FullChunkStatus::Inaccessible),
            server
                .scheduler()
                .full_status_chunk_count(FullChunkStatus::Border),
            server
                .scheduler()
                .full_status_chunk_count(FullChunkStatus::Ticking),
            server
                .scheduler()
                .full_status_chunk_count(FullChunkStatus::EntityTicking),
            server.scheduler().block_ticking_chunk_count(),
            server.scheduler().entity_ticking_chunk_count(),
        ),
        (792, 24, 16, 9, 25, 9)
    );
    assert_eq!(server.scheduler().ready_dependency_chunk_count(), 9 * 9);
    assert_eq!(
        server.scheduler().metrics(),
        ChunkSchedulerMetrics {
            player_promotion_desired: 9,
            player_promotion_queued: 0,
            player_promotion_active: 0,
            player_promotion_max_active: 4,
            player_promotion_admitted_total: 9,
            player_promotion_cancelled_before_admission: 0,
            player_promotion_oldest_age_ticks: 0,
            direct_ticket_chunks: 9,
            active_ticket_chunks: 29 * 29,
            holder_chunks: 29 * 29,
            pending_unload_chunks: 0,
            inaccessible_status_chunks: 792,
            border_status_chunks: 24,
            ticking_status_chunks: 16,
            entity_ticking_status_chunks: 9,
            block_ticking_chunks: 25,
            client_visible_chunks: 9,
            loaded_snapshot_chunks: 25,
            dependency_holder_chunks: 29 * 29 - 25,
            ready_dependency_chunks: 9 * 9,
            dirty_chunks: 0,
            pending_jobs: 0,
            completed_jobs: 3,
            total_seeded_dependency_chunks: 105,
            total_dependency_cache_hits: 105,
            total_dependency_cache_misses: 9 * 9,
            total_retained_dependency_chunks: 186,
            max_feature_job_target_chunks: 18,
            max_feature_job_feature_centers: 40,
            max_feature_job_dependency_chunks: 9 * 9,
            latest_feature_job_id: Some(ChunkJobId(3)),
            latest_feature_job_target_chunks: 4,
            latest_feature_job_feature_centers: 36,
            latest_feature_job_dependency_chunks: 9 * 9,
            latest_feature_job_first_target: Some(ChunkPos::new(-2, -2)),
            light_ticket_count: 0,
            light_tickets_added: 25,
            light_tickets_released: 25,
            light_ticket_conservation_failures: 0,
            completed_light_statuses: 25,
            completed_light_batches: server.scheduler().metrics().completed_light_batches,
            total_light_status_compute_us: server
                .scheduler()
                .metrics()
                .total_light_status_compute_us,
            max_light_status_compute_us: server.scheduler().metrics().max_light_status_compute_us,
            total_light_status_world_init_us: server
                .scheduler()
                .metrics()
                .total_light_status_world_init_us,
            total_light_status_active_sections_us: server
                .scheduler()
                .metrics()
                .total_light_status_active_sections_us,
            total_light_status_sky_source_scan_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_source_scan_us,
            total_light_status_block_source_scan_us: server
                .scheduler()
                .metrics()
                .total_light_status_block_source_scan_us,
            total_light_status_engine_init_us: server
                .scheduler()
                .metrics()
                .total_light_status_engine_init_us,
            total_light_status_section_setup_us: server
                .scheduler()
                .metrics()
                .total_light_status_section_setup_us,
            total_light_status_section_status_update_us: server
                .scheduler()
                .metrics()
                .total_light_status_section_status_update_us,
            total_light_status_sky_column_enable_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_column_enable_us,
            total_light_status_sky_source_enqueue_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_source_enqueue_us,
            total_light_status_block_source_enqueue_us: server
                .scheduler()
                .metrics()
                .total_light_status_block_source_enqueue_us,
            total_light_status_input_chunks: server
                .scheduler()
                .metrics()
                .total_light_status_input_chunks,
            total_light_status_inserted_chunks: server
                .scheduler()
                .metrics()
                .total_light_status_inserted_chunks,
            retained_light_world_chunks: server.scheduler().metrics().retained_light_world_chunks,
            total_light_status_replaced_chunks: server
                .scheduler()
                .metrics()
                .total_light_status_replaced_chunks,
            total_light_status_unchanged_chunks: server
                .scheduler()
                .metrics()
                .total_light_status_unchanged_chunks,
            total_light_status_changed_block_raw_checks: server
                .scheduler()
                .metrics()
                .total_light_status_changed_block_raw_checks,
            total_light_status_changed_block_light_property_changes: server
                .scheduler()
                .metrics()
                .total_light_status_changed_block_light_property_changes,
            total_light_status_changed_block_opacity_changes: server
                .scheduler()
                .metrics()
                .total_light_status_changed_block_opacity_changes,
            total_light_status_changed_block_emission_changes: server
                .scheduler()
                .metrics()
                .total_light_status_changed_block_emission_changes,
            total_light_status_changed_block_raw_only_changes: server
                .scheduler()
                .metrics()
                .total_light_status_changed_block_raw_only_changes,
            total_light_status_changed_block_check_us: server
                .scheduler()
                .metrics()
                .total_light_status_changed_block_check_us,
            total_light_status_run_updates_us: server
                .scheduler()
                .metrics()
                .total_light_status_run_updates_us,
            total_light_status_run_update_iterations: server
                .scheduler()
                .metrics()
                .total_light_status_run_update_iterations,
            total_light_status_block_run_update_calls: server
                .scheduler()
                .metrics()
                .total_light_status_block_run_update_calls,
            total_light_status_sky_run_update_calls: server
                .scheduler()
                .metrics()
                .total_light_status_sky_run_update_calls,
            total_light_status_block_run_update_processed_nodes: server
                .scheduler()
                .metrics()
                .total_light_status_block_run_update_processed_nodes,
            total_light_status_sky_run_update_processed_nodes: server
                .scheduler()
                .metrics()
                .total_light_status_sky_run_update_processed_nodes,
            max_light_status_block_run_update_queue_before: server
                .scheduler()
                .metrics()
                .max_light_status_block_run_update_queue_before,
            max_light_status_sky_run_update_queue_before: server
                .scheduler()
                .metrics()
                .max_light_status_sky_run_update_queue_before,
            final_light_status_block_run_update_queue_after: server
                .scheduler()
                .metrics()
                .final_light_status_block_run_update_queue_after,
            final_light_status_sky_run_update_queue_after: server
                .scheduler()
                .metrics()
                .final_light_status_sky_run_update_queue_after,
            total_light_status_block_run_updates_us: server
                .scheduler()
                .metrics()
                .total_light_status_block_run_updates_us,
            total_light_status_sky_run_updates_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_run_updates_us,
            total_light_status_sky_source_update_count: server
                .scheduler()
                .metrics()
                .total_light_status_sky_source_update_count,
            total_light_status_sky_source_updates_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_source_updates_us,
            total_light_status_block_run_update_graph_us: server
                .scheduler()
                .metrics()
                .total_light_status_block_run_update_graph_us,
            total_light_status_sky_run_update_graph_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_run_update_graph_us,
            total_light_status_block_run_update_storage_swap_us: server
                .scheduler()
                .metrics()
                .total_light_status_block_run_update_storage_swap_us,
            total_light_status_sky_run_update_storage_swap_us: server
                .scheduler()
                .metrics()
                .total_light_status_sky_run_update_storage_swap_us,
            total_light_status_block_run_update_affected_sections: server
                .scheduler()
                .metrics()
                .total_light_status_block_run_update_affected_sections,
            total_light_status_sky_run_update_affected_sections: server
                .scheduler()
                .metrics()
                .total_light_status_sky_run_update_affected_sections,
            total_light_status_collect_sections_us: server
                .scheduler()
                .metrics()
                .total_light_status_collect_sections_us,
            total_light_status_publication_us: server
                .scheduler()
                .metrics()
                .total_light_status_publication_us,
            max_light_status_publication_us: server
                .scheduler()
                .metrics()
                .max_light_status_publication_us,
            total_light_status_publication_units: server
                .scheduler()
                .metrics()
                .total_light_status_publication_units,
        }
    );
    assert_eq!(server.scheduler().job_count(), 3);
    let jobs = server.scheduler().jobs().collect::<Vec<_>>();
    assert_eq!(
        jobs.iter()
            .map(|job| job.target_chunks.len())
            .sum::<usize>(),
        25
    );
    for job in jobs {
        assert_eq!(job.status, ChunkStatus::Features);
        assert_eq!(job.state, ChunkJobState::Complete);
        assert!(job.target_chunks.len() <= 4 * 9);
        assert!(job.dependency_chunks.len() <= 9 * 9);
        for target in &job.target_chunks {
            assert_eq!(
                server
                    .scheduler()
                    .holder(*target)
                    .unwrap()
                    .status_slot(ChunkStatus::Features)
                    .unwrap()
                    .job_id,
                Some(job.id)
            );
        }
    }
    assert!(updates.iter().skip(5).all(|update| {
        matches!(
            update,
            ServerUpdate::ChunkSnapshot(_) | ServerUpdate::EntitySnapshot(_)
        )
    }));
}

#[test]
fn player_ticket_levels_define_runtime_status_lanes() {
    let mut scheduler = ChunkScheduler::new(12_345);

    scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();

    assert_eq!(player_status_counts(0), (704, 16, 8, 1, 9));
    assert_eq!(
        (
            scheduler.full_status_chunk_count(FullChunkStatus::Inaccessible),
            scheduler.full_status_chunk_count(FullChunkStatus::Border),
            scheduler.full_status_chunk_count(FullChunkStatus::Ticking),
            scheduler.full_status_chunk_count(FullChunkStatus::EntityTicking),
            scheduler.block_ticking_chunk_count(),
            scheduler.entity_ticking_chunk_count(),
        ),
        (704, 16, 8, 1, 9, 1)
    );
    assert_eq!(
        scheduler
            .holder(ChunkPos::new(0, 0))
            .map(ChunkHolder::full_status),
        Some(FullChunkStatus::EntityTicking)
    );
    assert_eq!(
        scheduler
            .holder(ChunkPos::new(1, 0))
            .map(ChunkHolder::full_status),
        Some(FullChunkStatus::Ticking)
    );
    assert_eq!(
        scheduler
            .holder(ChunkPos::new(2, 0))
            .map(ChunkHolder::full_status),
        Some(FullChunkStatus::Border)
    );
    assert_eq!(
        scheduler
            .holder(ChunkPos::new(3, 0))
            .map(ChunkHolder::full_status),
        Some(FullChunkStatus::Inaccessible)
    );
    assert!(scheduler.holder(ChunkPos::new(14, 0)).is_none());
}

#[test]
fn client_visibility_is_separate_from_ticking_status() {
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
            .holder(ChunkPos::new(0, 0))
            .unwrap()
            .is_client_visible()
    );

    apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    let old_center = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
    assert!(!old_center.is_client_visible());
    assert_eq!(old_center.full_status(), FullChunkStatus::Ticking);
    assert_eq!(scheduler.client_visible_chunk_count(), 1);
    assert_eq!(scheduler.entity_ticking_chunk_count(), 1);
    assert_eq!(scheduler.block_ticking_chunk_count(), 9);
}

#[test]
fn scheduler_tick_report_lists_runtime_lanes_without_simulation() {
    let mut scheduler = ChunkScheduler::new(12_345);

    apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    let report = scheduler.tick_report().unwrap();

    assert_eq!(report.ticket_tick, 1);
    assert_eq!(
        report.block_ticking_chunks,
        chunk_square(ChunkPos::new(0, 0), 1)
    );
    assert_eq!(report.entity_ticking_chunks, vec![ChunkPos::new(0, 0)]);
    assert_eq!(report.pending_unloads_processed, 0);
    assert!(report.events.is_empty());
}

#[test]
fn integrated_server_tick_report_exposes_protocol_updates_and_lanes() {
    let mut server = LocalRealmSession::new(12_345);

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    let report = server.tick_report();

    assert_eq!(report.ticket_tick, 1);
    assert_eq!(
        report.block_ticking_chunks,
        chunk_square(ChunkPos::new(0, 0), 1)
    );
    assert_eq!(report.entity_ticking_chunks, vec![ChunkPos::new(0, 0)]);
    assert_eq!(report.pending_unloads_processed, 0);
    assert!(report.updates.is_empty());
}

#[test]
fn integrated_server_simulation_tick_report_records_phases() {
    let mut server = LocalRealmSession::new(12_345);

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    let before_metrics = server.scheduler().metrics();

    let report = server.simulation_tick_report();

    assert_eq!(report.simulation_tick, 1);
    assert_eq!(report.chunk_tick, 1);
    assert_eq!(report.block_tick_chunks, 9);
    assert!(report.fluid_ticks_executed > 0);
    assert_eq!(report.entity_tick_chunks, 1);
    #[cfg(not(feature = "physics-rapier"))]
    assert_eq!(report.physics, ServerPhysicsTickDiagnostics::default());
    #[cfg(feature = "physics-rapier")]
    {
        assert!(report.physics.enabled);
        assert_eq!(report.physics.body_count, 0);
        assert_eq!(report.physics.collider_count, 0);
        assert!(!report.physics.test_cube_spawned);
    }
    assert_eq!(report.pending_unloads_processed, 0);
    assert_eq!(
        server.scheduler().metrics().block_ticking_chunks,
        before_metrics.block_ticking_chunks
    );
}

#[test]
fn changed_interest_unloads_chunks_outside_view() {
    let mut server = LocalRealmSession::new(12_345);
    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    let updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    assert!(updates.iter().any(
        |update| matches!(update, ServerUpdate::ChunkUnload { pos } if *pos == ChunkPos::new(0, 0))
    ));
    assert!(updates.iter().any(|update| matches!(
        update,
        ServerUpdate::ChunkSnapshot(snapshot)
            if snapshot.pos == ChunkPos::new(1, 0)
                && snapshot.status == ChunkStatus::Light
                && snapshot.light_correct
    )));
}
