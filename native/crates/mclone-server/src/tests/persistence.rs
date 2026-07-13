use super::support::*;

#[test]
fn stored_light_chunks_satisfy_interest_without_worldgen() {
    let store = SharedMemoryWorldStore::new();
    for z in -1..=1 {
        for x in -1..=1 {
            let pos = ChunkPos::new(x, z);
            let snapshot = ChunkSnapshot::from_block_state_ids(
                pos,
                ChunkStatus::Light,
                ChunkRevision(1),
                0,
                16,
                &vec![mclone_core::AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
            )
            .with_light_sections(true, Vec::new());
            store
                .chunks
                .borrow_mut()
                .insert(pos, ChunkRecord::from_snapshot(snapshot));
        }
    }
    let mut scheduler = ChunkScheduler::with_world_store(12_345, Box::new(store));

    let events = apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    assert_eq!(scheduler.loaded_chunk_count(), 9);
    assert_eq!(scheduler.job_count(), 0);
    assert_eq!(scheduler.worldgen_mailbox_pending_count(), 0);
    assert_eq!(snapshot_ready_count(&events), 1);
    assert!(events.iter().any(|event| matches!(
        event,
        ChunkSchedulerEvent::SnapshotReady(snapshot)
            if snapshot.pos == ChunkPos::new(0, 0)
                && snapshot.status == ChunkStatus::Light
    )));
}

#[test]
fn loaded_chunk_record_hydrates_scheduled_fluid_ticks() {
    let store = SharedMemoryWorldStore::new();
    let pos = ChunkPos::new(0, 0);
    let source = WorldBlockPos::new(8, 120, 8);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Light,
        ChunkRevision(7),
        0,
        16,
        &vec![mclone_core::AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
    )
    .with_light_sections(true, Vec::new());
    store.chunks.borrow_mut().insert(
        pos,
        ChunkRecord::from_snapshot(snapshot).with_scheduled_fluid_ticks(vec![
            ScheduledTickRecord::new(source, "minecraft:water", 0),
        ]),
    );
    let mut server = IntegratedServer::with_world_store(12_345, Box::new(store));

    let updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: pos,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    assert!(
        snapshot_update_for(&updates, pos).is_some(),
        "stored chunk should publish without regenerating"
    );
    assert!(
        server
            .liquid_ticks
            .has_scheduled_tick(source, FluidKind::Water),
        "stored scheduled fluid tick should hydrate into the host tick queue"
    );
}

#[test]
fn loaded_chunk_record_hydrates_scheduled_block_ticks() {
    let store = SharedMemoryWorldStore::new();
    let pos = ChunkPos::new(0, 0);
    let tick_pos = WorldBlockPos::new(8, 120, 8);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Light,
        ChunkRevision(8),
        0,
        16,
        &vec![mclone_core::AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
    )
    .with_light_sections(true, Vec::new());
    store.chunks.borrow_mut().insert(
        pos,
        ChunkRecord::from_snapshot(snapshot).with_scheduled_block_ticks(vec![
            ScheduledTickRecord::new(tick_pos, "minecraft:sand", 0),
        ]),
    );
    let mut server = IntegratedServer::with_world_store(12_345, Box::new(store));

    let updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: pos,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    assert!(
        snapshot_update_for(&updates, pos).is_some(),
        "stored chunk should publish without regenerating"
    );
    assert_eq!(
        server.scheduled_block_tick_count(),
        1,
        "stored scheduled block tick should hydrate into the host tick queue"
    );
}

#[test]
fn loaded_entity_chunk_record_hydrates_entities() {
    let store = SharedMemoryWorldStore::new();
    let pos = ChunkPos::new(0, 0);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Light,
        ChunkRevision(9),
        0,
        16,
        &vec![mclone_core::AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
    )
    .with_light_sections(true, Vec::new());
    store
        .chunks
        .borrow_mut()
        .insert(pos, ChunkRecord::from_snapshot(snapshot));
    store
        .entity_chunks
        .borrow_mut()
        .insert(pos, stored_egg_item_entity_record(pos, 3, 1));
    let mut server = IntegratedServer::with_world_store(12_345, Box::new(store));
    server.set_debug_passive_showcase_enabled(false);

    let updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: pos,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );

    assert!(snapshot_update_for(&updates, pos).is_some());
    let entity = entity_snapshot_for(&updates, mclone_protocol::EntityKind::Item)
        .expect("stored item entity should publish to the tracking player");
    assert_eq!(
        entity.item_stack,
        Some(mclone_protocol::ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::Egg,
            count: 3,
        })
    );
    assert_eq!(entity.age_ticks, 37);
}

#[test]
fn entity_chunk_record_survives_holder_unload_and_reload() {
    let store = SharedMemoryWorldStore::new();
    let entity_chunks = store.entity_chunks.clone();
    let pos = ChunkPos::new(0, 0);
    let snapshot = ChunkSnapshot::from_block_state_ids(
        pos,
        ChunkStatus::Light,
        ChunkRevision(10),
        0,
        16,
        &vec![mclone_core::AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
    )
    .with_light_sections(true, Vec::new());
    store
        .chunks
        .borrow_mut()
        .insert(pos, ChunkRecord::from_snapshot(snapshot));
    store
        .entity_chunks
        .borrow_mut()
        .insert(pos, stored_egg_item_entity_record(pos, 4, 2));
    let mut server = IntegratedServer::with_world_store(12_345, Box::new(store));
    server.set_debug_passive_showcase_enabled(false);

    let initial_updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: pos,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    assert!(
        entity_snapshot_for(&initial_updates, mclone_protocol::EntityKind::Item).is_some(),
        "preloaded entity should be visible before unload"
    );

    handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(100, 100),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    for _ in 0..200 {
        let _ = server.try_tick_report().unwrap();
        try_poll_server_until_persistence_idle(&mut server).unwrap();
        if server.scheduler().holder(pos).is_none() {
            break;
        }
    }
    assert!(
        server.scheduler().holder(pos).is_none(),
        "old holder should be fully unloaded after entity save completes"
    );
    let saved = entity_chunks
        .borrow()
        .get(&pos)
        .cloned()
        .expect("unload should durably save the entity chunk");
    assert_eq!(saved.entities.len(), 1);

    let reloaded_updates = handle_command_and_poll(
        &mut server,
        ClientCommand::SetChunkView(ChunkView {
            center: pos,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    );
    let reloaded = entity_snapshot_for(&reloaded_updates, mclone_protocol::EntityKind::Item)
        .expect("saved entity should hydrate after returning to the chunk");
    assert_eq!(
        reloaded.item_stack,
        Some(mclone_protocol::ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::Egg,
            count: 3,
        })
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn generated_chunks_queue_clean_cache_saves() {
    let mut scheduler = ChunkScheduler::new(12_345);
    apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
    assert_eq!(holder.residency(), ChunkResidency::Generated);
    assert!(!holder.is_dirty());
    assert_eq!(scheduler.dirty_chunk_count(), 0);

    assert_eq!(scheduler.save_dirty_chunks().unwrap(), 0);
    poll_scheduler_until_persistence_idle(&mut scheduler);

    let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
    assert_eq!(holder.residency(), ChunkResidency::Generated);
    assert!(!holder.is_dirty());
    assert_eq!(scheduler.dirty_chunk_count(), 0);
}

#[test]
fn generated_light_cache_record_keeps_generated_fluid_ticks() {
    let store = SharedMemoryWorldStore::new();
    let chunks = store.chunks.clone();
    let center = ChunkPos::new(117, -128);
    let mut scheduler = ChunkScheduler::with_world_store(12_345, Box::new(store));

    apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center,
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );
    poll_scheduler_until_persistence_idle(&mut scheduler);

    let record = chunks
        .borrow()
        .get(&center)
        .cloned()
        .expect("center chunk should have a cache record");
    assert_eq!(record.snapshot.status, ChunkStatus::Light);
    assert!(
        record.scheduled_fluid_ticks.len() >= 90,
        "watery oracle chunk should persist generated fluid ticks, got {}",
        record.scheduled_fluid_ticks.len()
    );
    assert!(
        record
            .scheduled_fluid_ticks
            .iter()
            .all(|tick| tick.pos.chunk_pos() == center)
    );
}

#[test]
fn dirty_block_edit_survives_unload_reload_through_memory_store() {
    let store = SharedMemoryWorldStore::new();
    let interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };
    let edited = WorldBlockPos::new(8, 80, 8);

    let mut scheduler = ChunkScheduler::with_world_store(12_345, Box::new(store.clone()));
    apply_interest_and_poll(&mut scheduler, interest.clone());
    let replacement = if scheduler.block_at_world(edited) == Some(STONE) {
        AIR
    } else {
        STONE
    };
    assert!(scheduler.set_block_at_world(edited, replacement));
    assert_eq!(scheduler.dirty_chunk_count(), 1);

    scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(100, 100),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();
    scheduler.process_pending_unloads(usize::MAX).unwrap();
    assert!(
        scheduler.holder(ChunkPos::new(0, 0)).is_some(),
        "dirty pending-unload holder should wait for durable save completion"
    );
    poll_scheduler_until_persistence_idle(&mut scheduler);
    scheduler.process_pending_unloads(usize::MAX).unwrap();
    assert!(scheduler.holder(ChunkPos::new(0, 0)).is_none());

    let mut reloaded = ChunkScheduler::with_world_store(12_345, Box::new(store));
    apply_interest_and_poll(&mut reloaded, interest);
    assert_eq!(reloaded.block_at_world(edited), Some(replacement));
    assert_eq!(
        reloaded
            .holder(ChunkPos::new(0, 0))
            .map(ChunkHolder::residency),
        Some(ChunkResidency::LoadedFromStore)
    );
}

#[test]
fn clean_generated_cache_miss_regenerates_without_store_error() {
    let mut scheduler =
        ChunkScheduler::with_world_store(12_345, Box::new(SharedMemoryWorldStore::new()));
    let events = apply_interest_and_poll(
        &mut scheduler,
        ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        },
    );

    assert_eq!(scheduler.job_count(), 1);
    assert_eq!(scheduler.loaded_chunk_count(), 9);
    assert_eq!(snapshot_ready_count(&without_fluid_tick_events(&events)), 1);
}

#[test]
fn dirty_pending_unload_holder_is_rescued_before_save_ack() {
    let store = SharedMemoryWorldStore::new();
    let interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };
    let edited = WorldBlockPos::new(8, 80, 8);
    let mut scheduler = ChunkScheduler::with_world_store(12_345, Box::new(store));

    apply_interest_and_poll(&mut scheduler, interest.clone());
    let replacement = if scheduler.block_at_world(edited) == Some(STONE) {
        AIR
    } else {
        STONE
    };
    assert!(scheduler.set_block_at_world(edited, replacement));
    scheduler
        .apply_interest(ChunkView {
            center: ChunkPos::new(100, 100),
            render_distance: 0,
            chunk_tracking_radius: 0,
        })
        .unwrap();
    scheduler.process_pending_unloads(usize::MAX).unwrap();
    assert!(scheduler.is_pending_unload(ChunkPos::new(0, 0)));
    assert_eq!(scheduler.pending_persistence_save_count(), 1);

    scheduler.apply_interest(interest).unwrap();
    assert!(!scheduler.is_pending_unload(ChunkPos::new(0, 0)));
    assert_eq!(scheduler.block_at_world(edited), Some(replacement));
    scheduler.process_pending_unloads(usize::MAX).unwrap();
    poll_scheduler_until_persistence_idle(&mut scheduler);
    assert_eq!(scheduler.block_at_world(edited), Some(replacement));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn integrated_server_saves_and_reloads_resident_chunk() {
    let root = unique_temp_dir("integrated_server_saves_and_reloads_resident_chunk");
    let interest = ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    };
    let first_snapshot = {
        let mut server = IntegratedServer::with_chunk_store(
            12_345,
            Box::new(FilesystemChunkSnapshotStore::new(&root)),
        );
        let updates =
            try_handle_command_and_poll(&mut server, ClientCommand::SetChunkView(interest.clone()))
                .unwrap();
        let snapshot = snapshot_update_for(&updates, ChunkPos::new(0, 0))
            .expect("updates should include the resident chunk snapshot");
        assert_eq!(server.scheduler().job_count(), 1);
        assert_eq!(server.scheduler().dirty_chunk_count(), 0);
        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(0, 0))
                .unwrap()
                .residency(),
            ChunkResidency::Generated
        );

        assert_eq!(server.save_dirty_chunks().unwrap(), 0);
        try_poll_server_until_persistence_idle(&mut server).unwrap();
        assert_eq!(server.scheduler().dirty_chunk_count(), 0);
        assert_eq!(
            server
                .scheduler()
                .holder(ChunkPos::new(0, 0))
                .unwrap()
                .residency(),
            ChunkResidency::Generated
        );
        snapshot.clone()
    };

    let mut reloaded = IntegratedServer::with_chunk_store(
        12_345,
        Box::new(FilesystemChunkSnapshotStore::new(&root)),
    );
    let updates =
        try_handle_command_and_poll(&mut reloaded, ClientCommand::SetChunkView(interest)).unwrap();

    assert!(updates.contains(&ServerUpdate::ChunkSnapshot(first_snapshot)));
    assert_eq!(
        updates
            .iter()
            .filter(|update| matches!(update, ServerUpdate::EntitySnapshot(_)))
            .count(),
        2
    );
    assert_eq!(reloaded.scheduler().job_count(), 0);
    let holder = reloaded.scheduler().holder(ChunkPos::new(0, 0)).unwrap();
    assert_eq!(holder.residency(), ChunkResidency::LoadedFromStore);
    assert!(!holder.is_dirty());
    assert_eq!(reloaded.scheduler().dirty_chunk_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}
