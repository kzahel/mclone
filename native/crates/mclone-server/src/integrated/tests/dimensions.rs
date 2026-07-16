use super::*;
use crate::MemoryWorldStore;

fn moon_record() -> DimensionRecord {
    DimensionRecord {
        key: DimensionKey::parse("mclone:moon").unwrap(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition: crate::DimensionDefinition::overworld(
            54_321,
            WorldGenerationProfile::authored_only(),
        ),
    }
}

fn request_zero_radius_view(server: &mut RealmServer, player: ServerPlayerId) {
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .unwrap();
}

fn wait_for_origin_in_both_dimensions(server: &mut RealmServer, moon: &DimensionKey) {
    for _ in 0..60_000 {
        server.try_tick_report_global().unwrap();
        let overworld_ready = server
            .dimension_runtime(&DimensionKey::overworld())
            .unwrap()
            .scheduler()
            .client_visible_snapshot(ChunkPos::new(0, 0))
            .is_some();
        let moon_ready = server
            .dimension_runtime(moon)
            .unwrap()
            .scheduler()
            .client_visible_snapshot(ChunkPos::new(0, 0))
            .is_some();
        if overworld_ready && moon_ready {
            return;
        }
        for key in [DimensionKey::overworld(), moon.clone()] {
            server.activate_dimension(&key).unwrap();
            if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
    }
    panic!("timed out loading equal origin chunks in both dimensions");
}

fn move_player(server: &mut RealmServer, player: ServerPlayerId, position: Vec3d) {
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
                position,
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .unwrap();
}

fn snapshot_block(record: ChunkRecord, pos: BlockPos) -> u8 {
    crate::mutable_buffer_from_snapshot(&record.snapshot).get_block_at_y(
        local_block_coord(pos.x),
        pos.y,
        local_block_coord(pos.z),
    )
}

fn accept_initial_position(server: &mut RealmServer, player: ServerPlayerId) {
    let updates = server.try_drain_updates_for_player(player).unwrap();
    let teleport_id = updates.iter().find_map(|update| match update {
        ServerUpdate::PlayerPosition(update) => Some(update.teleport_id),
        _ => None,
    });
    if let Some(id) = teleport_id {
        server
            .try_handle_command_for_player(
                player,
                ClientCommand::AcceptTeleport(AcceptTeleportCommand { id }),
            )
            .unwrap();
    }
}

#[test]
fn concurrent_dimension_runtimes_route_players_ticks_and_mutations() {
    let dimension_record = moon_record();
    let moon = dimension_record.key.clone();
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);
    assert!(server.register_dimension(dimension_record).unwrap());

    let overworld_player = server.add_player();
    let moon_player = server.add_player_in_dimension(moon.clone()).unwrap();
    assert_eq!(
        server.player_dimension(overworld_player),
        Some(&DimensionKey::overworld())
    );
    assert_eq!(server.player_dimension(moon_player), Some(&moon));

    request_zero_radius_view(&mut server, overworld_player);
    request_zero_radius_view(&mut server, moon_player);
    wait_for_origin_in_both_dimensions(&mut server, &moon);
    accept_initial_position(&mut server, overworld_player);
    accept_initial_position(&mut server, moon_player);

    let overworld_tick_before = server
        .dimension_runtime(&DimensionKey::overworld())
        .unwrap()
        .scheduler()
        .ticket_tick();
    let moon_tick_before = server
        .dimension_runtime(&moon)
        .unwrap()
        .scheduler()
        .ticket_tick();
    let realm_tick_before = server.simulation_tick();
    server.try_simulation_tick_report_global().unwrap();
    assert_eq!(server.simulation_tick(), realm_tick_before + 1);
    assert_eq!(
        server
            .dimension_runtime(&DimensionKey::overworld())
            .unwrap()
            .scheduler()
            .ticket_tick(),
        overworld_tick_before + 1
    );
    assert_eq!(
        server
            .dimension_runtime(&moon)
            .unwrap()
            .scheduler()
            .ticket_tick(),
        moon_tick_before + 1
    );

    let position = Vec3d::new(8.5, 80.0, 8.5);
    let block = BlockPos::new(8, 80, 8);
    move_player(&mut server, overworld_player, position);
    assert!(server.scheduler_mut().set_block_at_world(block, DIRT));
    server.scheduler_mut().drain_pending_block_delta_events();
    move_player(&mut server, moon_player, Vec3d::new(9.5, 80.0, 8.5));
    assert!(server.scheduler_mut().set_block_at_world(block, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let overworld_updates = server
        .try_handle_command_for_player(
            overworld_player,
            ClientCommand::PlayerAction(PlayerActionCommand {
                pos: block,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }),
        )
        .unwrap();
    assert!(has_section_block_updates(&overworld_updates));
    assert_eq!(server.scheduler().block_at_world(block), Some(AIR));

    server.activate_player_dimension(moon_player).unwrap();
    assert_eq!(server.scheduler().block_at_world(block), Some(STONE));
    let moon_updates = server.try_drain_updates_for_player(moon_player).unwrap();
    assert!(!has_section_block_updates(&moon_updates));
    assert!(moon_updates.iter().all(|update| !matches!(
        update,
        ServerUpdate::RemotePlayerAdd(_)
            | ServerUpdate::RemotePlayerUpdate(_)
            | ServerUpdate::RemotePlayerRemove { .. }
    )));
    assert_eq!(server.player_position(overworld_player), Some(position));
    assert_eq!(
        server.player_position(moon_player),
        Some(Vec3d::new(9.5, 80.0, 8.5))
    );

    assert!(!server.unload_dimension(&moon).unwrap());
    assert!(server.remove_player(moon_player));
    assert!(server.unload_dimension(&moon).unwrap());
    assert!(!server.loaded_dimension_keys().contains(&moon));
    assert!(server.register_dimension(moon_record()).unwrap());
    assert!(server.loaded_dimension_keys().contains(&moon));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dimension_membership_and_runtime_chunks_resume_from_one_realm_store() {
    let root = std::env::temp_dir().join(format!(
        "mclone-realm-dimension-runtime-{}-{}",
        std::process::id(),
        current_unix_millis()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let moon_record = moon_record();
    let moon = moon_record.key.clone();
    let overworld_identity =
        ClientIdentity::new(PlayerProfileId::new([0x31; 16]), "Overworld Player").unwrap();
    let moon_identity =
        ClientIdentity::new(PlayerProfileId::new([0x42; 16]), "Moon Player").unwrap();
    let block = BlockPos::new(8, 80, 8);

    {
        let mut server = RealmServer::try_with_threaded_sqlite_world_dir(12_345, &root).unwrap();
        server.initialize_world_metadata_blocking().unwrap();
        server
            .set_world_generation_profile(WorldGenerationProfile::authored_only())
            .unwrap();
        server.set_lighting_enabled(false);
        server.register_dimension(moon_record.clone()).unwrap();
        let overworld_player = server
            .add_player_with_identity(overworld_identity.clone())
            .unwrap();
        let moon_player = server.add_player_in_dimension(moon.clone()).unwrap();
        server
            .configure_player_identity_blocking(moon_player, moon_identity.clone())
            .unwrap();
        request_zero_radius_view(&mut server, overworld_player);
        request_zero_radius_view(&mut server, moon_player);
        wait_for_origin_in_both_dimensions(&mut server, &moon);
        accept_initial_position(&mut server, overworld_player);
        accept_initial_position(&mut server, moon_player);

        move_player(&mut server, overworld_player, Vec3d::new(8.5, 80.0, 8.5));
        assert!(server.scheduler_mut().set_block_at_world(block, DIRT));
        move_player(&mut server, moon_player, Vec3d::new(24.5, 90.0, 24.5));
        assert!(server.scheduler_mut().set_block_at_world(block, STONE));
        server.save_dirty_chunks().unwrap();
        server.shutdown_persistence().unwrap();
    }

    {
        let mut store = crate::SqliteWorldStore::open_world_dir(&root).unwrap();
        assert_eq!(
            snapshot_block(
                store
                    .load_chunk(&DimensionKey::overworld(), ChunkPos::new(0, 0))
                    .unwrap()
                    .unwrap(),
                block,
            ),
            DIRT
        );
        assert_eq!(
            snapshot_block(
                store
                    .load_chunk(&moon, ChunkPos::new(0, 0))
                    .unwrap()
                    .unwrap(),
                block,
            ),
            STONE
        );
    }

    {
        let mut server = RealmServer::try_with_threaded_sqlite_world_dir(12_345, &root).unwrap();
        server.initialize_world_metadata_blocking().unwrap();
        server.register_dimension(moon_record).unwrap();
        let overworld_player = server.add_player_with_identity(overworld_identity).unwrap();
        let moon_player = server.add_player_with_identity(moon_identity).unwrap();
        assert_eq!(
            server.player_dimension(overworld_player),
            Some(&DimensionKey::overworld())
        );
        assert_eq!(server.player_dimension(moon_player), Some(&moon));
        assert_eq!(
            server
                .players
                .get(overworld_player)
                .unwrap()
                .resume_record
                .as_ref()
                .unwrap()
                .position,
            Vec3d::new(8.5, 80.0, 8.5)
        );
        assert_eq!(
            server
                .players
                .get(moon_player)
                .unwrap()
                .resume_record
                .as_ref()
                .unwrap()
                .position,
            Vec3d::new(24.5, 90.0, 24.5)
        );
        server.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}
