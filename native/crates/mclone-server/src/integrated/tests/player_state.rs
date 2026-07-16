use super::*;
use crate::MemoryWorldStore;

#[test]
fn frozen_day_time_holds_a_forced_value() {
    let mut server = IntegratedServer::new(0);
    server.set_day_time(23000);
    server.set_day_time_frozen(true);
    for tick in 1..=5 {
        let report = server.try_simulation_tick_report().expect("tick");
        assert_eq!(server.day_time(), 23000);
        if tick == 1 {
            assert_eq!(last_time_update(&report), 23000);
        } else {
            assert!(
                report
                    .updates
                    .iter()
                    .all(|update| !matches!(update, ServerUpdate::TimeUpdate { .. }))
            );
        }
    }
}

#[test]
fn first_chunk_view_sends_safe_surface_spawn_position() {
    let mut server = IntegratedServer::new(12345);
    request_initial_chunk_view(&mut server);
    let spawn = wait_for_initial_spawn_update(&mut server);

    assert_eq!(spawn.relative, PlayerPositionRelativeFlags::ABSOLUTE);
    assert_eq!(spawn.teleport_id, 1);
    assert_eq!(spawn.y_rot_degrees, 0.0);
    assert_eq!(spawn.x_rot_degrees, 0.0);
    assert_eq!(spawn.position.x - spawn.position.x.floor(), 0.5);
    assert_eq!(spawn.position.z - spawn.position.z.floor(), 0.5);

    let feet = BlockPos::new(
        spawn.position.x.floor() as i32,
        spawn.position.y as i32,
        spawn.position.z.floor() as i32,
    );
    let floor = feet.below();
    let floor_block = server
        .scheduler()
        .block_at_world(floor)
        .expect("spawn floor block");
    let feet_block = server
        .scheduler()
        .block_at_world(feet)
        .expect("spawn feet block");
    let head_block = server
        .scheduler()
        .block_at_world(feet.offset(0, 1, 0))
        .expect("spawn head block");
    assert!(material_blocks_motion(floor_block));
    assert!(!has_fluid(floor_block));
    assert!(!material_blocks_motion(feet_block));
    assert!(!has_fluid(feet_block));
    assert!(!material_blocks_motion(head_block));
    assert!(!has_fluid(head_block));

    assert_eq!(
        server
            .player
            .awaiting_teleport()
            .map(|awaiting| awaiting.id),
        Some(spawn.teleport_id)
    );
    let updates = server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: spawn.teleport_id,
        }))
        .expect("accept spawn");
    assert!(updates.is_empty());
    assert_eq!(server.player.awaiting_teleport(), None);
    assert_eq!(server.player.position(), spawn.position);
}

#[test]
fn saved_player_pose_and_selected_slot_resume_for_stable_identity() {
    let seed = 12_345;
    let mut probe = IntegratedServer::new(seed);
    request_initial_chunk_view(&mut probe);
    let safe_spawn = wait_for_initial_spawn_update(&mut probe).position;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x42; 16]), "Builder").unwrap();
    let mut record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        7,
        identity.display_name.clone(),
        safe_spawn.add(Vec3d::new(0.0, 2.0, 0.0)),
    );
    record.y_rot_degrees = 135.0;
    record.x_rot_degrees = -22.5;
    record.on_ground = false;
    record.selected_hotbar_slot = 4;
    record.total_experience = 19;
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = IntegratedServer::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let resumed = wait_for_initial_spawn_update(&mut server);

    assert_eq!(resumed.position, record.position);
    assert_eq!(resumed.y_rot_degrees, record.y_rot_degrees);
    assert_eq!(resumed.x_rot_degrees, record.x_rot_degrees);
    assert!(!server.player.on_ground());
    assert_eq!(server.inventory.selected_hotbar_slot(), 4);
    assert_eq!(server.local_player_total_experience, 19);
}

#[test]
fn blocked_saved_player_pose_falls_back_to_safe_surface_nearby() {
    let seed = 12_345;
    let mut probe = IntegratedServer::new(seed);
    request_initial_chunk_view(&mut probe);
    let safe_spawn = wait_for_initial_spawn_update(&mut probe).position;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x24; 16]), "Explorer").unwrap();
    let blocked_position = safe_spawn.add(Vec3d::new(0.0, -1.0, 0.0));
    let record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        3,
        identity.display_name.clone(),
        blocked_position,
    );
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = IntegratedServer::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let resumed = wait_for_initial_spawn_update(&mut server);

    assert_ne!(resumed.position, blocked_position);
    assert!(server.player_pose_has_clearance(resumed.position));
    assert_eq!(resumed.position.x.floor(), blocked_position.x.floor());
    assert_eq!(resumed.position.z.floor(), blocked_position.z.floor());
}

#[test]
fn seed_789_initial_spawn_uses_surface_not_underground_cave() {
    let mut server = IntegratedServer::new(789);
    request_initial_chunk_view(&mut server);
    let spawn = wait_for_initial_spawn_update(&mut server);
    let feet = BlockPos::new(
        spawn.position.x.floor() as i32,
        spawn.position.y as i32,
        spawn.position.z.floor() as i32,
    );

    for y in feet.y..crate::game_mode::JAVA_OVERWORLD_MAX_BUILD_HEIGHT {
        let block = server
            .scheduler()
            .block_at_world(BlockPos::new(feet.x, y, feet.z))
            .expect("spawn column block above feet");
        assert!(
            !material_blocks_motion(block) && !has_fluid(block),
            "seed 789 spawn selected underground column with blocking/fluid block {block} at y={y}; spawn={:?}",
            spawn.position
        );
    }
}

#[test]
fn move_player_command_updates_server_player_state_without_world_updates() {
    let mut server = IntegratedServer::new(0);

    let updates = server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position: Vec3d::new(1.25, 63.0, -4.5),
            y_rot_degrees: 181.0,
            x_rot_degrees: -181.0,
            on_ground: true,
        }))
        .expect("move player");

    assert!(updates.is_empty());
    assert_eq!(server.player.position(), Vec3d::new(1.25, 63.0, -4.5));
    assert_eq!(server.player.y_rot_degrees(), -179.0);
    assert_eq!(server.player.x_rot_degrees(), 179.0);
    assert!(server.player.on_ground());
}

#[test]
fn simulation_tick_records_java_shaped_movement_packet_boundary() {
    let mut server = IntegratedServer::new(0);

    server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
            position: Vec3d::new(1.0, 64.0, 1.0),
            on_ground: true,
        }))
        .expect("move player");
    server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Rot {
            y_rot_degrees: 90.0,
            x_rot_degrees: 10.0,
            on_ground: false,
        }))
        .expect("rotate player");

    assert_eq!(server.player.position(), Vec3d::new(1.0, 64.0, 1.0));
    assert_eq!(server.player.received_move_packet_count(), 2);
    assert_eq!(server.player.known_move_packet_count(), 0);

    server
        .try_simulation_tick_report()
        .expect("simulation tick");

    assert_eq!(server.player.received_move_packet_count(), 2);
    assert_eq!(server.player.known_move_packet_count(), 2);
    assert_eq!(
        server.player.first_good_position(),
        Vec3d::new(1.0, 64.0, 1.0)
    );
    assert_eq!(
        server.player.last_good_position(),
        Vec3d::new(1.0, 64.0, 1.0)
    );
}

#[test]
fn pending_player_position_update_blocks_moves_until_ack_and_resends() {
    let mut server = IntegratedServer::new(0);

    let first = server.player.initial_position_update(
        Vec3d::new(0.0, 64.0, 0.0),
        45.0,
        10.0,
        server.simulation_tick,
    );
    assert_eq!(first.teleport_id, 1);

    let updates = server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move while awaiting teleport");
    assert!(updates.is_empty());
    assert_eq!(server.player.position(), Vec3d::new(0.0, 64.0, 0.0));
    assert_eq!(
        server
            .player
            .awaiting_teleport()
            .map(|awaiting| awaiting.id),
        Some(first.teleport_id)
    );

    for _ in 0..20 {
        server
            .try_simulation_tick_report()
            .expect("pending teleport tick");
    }
    let updates = server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move while awaiting teleport at threshold");
    assert!(updates.is_empty());

    server
        .try_simulation_tick_report()
        .expect("pending teleport resend tick");
    let updates = server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move while awaiting stale teleport");
    assert_eq!(updates.len(), 1);
    let resend = match &updates[0] {
        ServerUpdate::PlayerPosition(update) => *update,
        _ => panic!("stale pending teleport should resend a player position update"),
    };
    assert_eq!(resend.position, Vec3d::new(0.0, 64.0, 0.0));
    assert_eq!(resend.y_rot_degrees, 45.0);
    assert_eq!(resend.x_rot_degrees, 10.0);
    assert_eq!(resend.relative, PlayerPositionRelativeFlags::ABSOLUTE);
    assert_eq!(resend.teleport_id, 2);
    assert_eq!(
        server
            .player
            .awaiting_teleport()
            .map(|awaiting| awaiting.id),
        Some(2)
    );

    server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: first.teleport_id,
        }))
        .expect("accept stale teleport");
    assert_eq!(
        server
            .player
            .awaiting_teleport()
            .map(|awaiting| awaiting.id),
        Some(2)
    );

    server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: resend.teleport_id,
        }))
        .expect("accept resent teleport");
    assert_eq!(server.player.awaiting_teleport(), None);

    let updates = server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move after ack");
    assert!(updates.is_empty());
    assert_eq!(server.player.position(), Vec3d::new(32.0, 64.0, 0.0));
}

#[test]
fn set_carried_item_updates_server_selected_hotbar_slot_without_world_updates() {
    let mut server = IntegratedServer::new(0);

    let updates = server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: 4,
        }))
        .expect("set carried item");

    assert!(updates.is_empty());
    assert_eq!(server.inventory.selected_hotbar_slot(), 4);

    let updates = server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: mclone_protocol::HOTBAR_SLOT_COUNT,
        }))
        .expect("invalid carried item");

    assert!(updates.is_empty());
    assert_eq!(server.inventory.selected_hotbar_slot(), 4);
}
