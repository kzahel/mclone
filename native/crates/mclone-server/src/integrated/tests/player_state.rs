use super::*;
use crate::MemoryWorldStore;

#[test]
fn new_world_metadata_starts_at_vanilla_zero_and_tracks_both_clocks() {
    let mut server = IntegratedServer::with_world_store(77, Box::new(MemoryWorldStore::new()));
    let initialized = server
        .initialize_world_metadata_at_unix_millis(1_000)
        .unwrap();

    assert_eq!(initialized.game_time, 0);
    assert_eq!(initialized.day_time, 0);
    assert!(initialized.do_daylight_cycle);
    for _ in 0..25 {
        server.try_simulation_tick_report().unwrap();
    }
    assert_eq!(server.game_time(), 25);
    assert_eq!(server.day_time(), 25);
    assert_eq!(server.save_world_metadata_at_unix_millis(2_000).unwrap(), 1);
    let saved = server.world_metadata().unwrap();
    assert_eq!(saved.game_time, 25);
    assert_eq!(saved.day_time, 25);
    assert_eq!(saved.last_played_unix_millis, 2_000);
}

#[test]
fn legacy_world_records_initialize_once_at_mclone_morning() {
    let identity = ClientIdentity::new(PlayerProfileId::new([0x33; 16]), "Legacy").unwrap();
    let mut store = MemoryWorldStore::new();
    store
        .save_player(&PlayerRecord::new(
            player_record_key(identity.profile_id),
            1,
            identity.display_name,
            Vec3d::new(0.5, 64.0, 0.5),
        ))
        .unwrap();
    let mut server = IntegratedServer::with_world_store(88, Box::new(store));

    let initialized = server
        .initialize_world_metadata_at_unix_millis(3_000)
        .unwrap();

    assert_eq!(initialized.game_time, 0);
    assert_eq!(initialized.day_time, INITIAL_DAY_TIME);
}

#[test]
fn stored_world_metadata_rejects_seed_and_profile_mismatches() {
    let mut seed_store = MemoryWorldStore::new();
    seed_store
        .save_world_metadata(&WorldMetadata::new(
            1,
            WorldGenerationProfile::Overworld,
            WorldBehaviorProfile::Mutable,
            1_000,
        ))
        .unwrap();
    let mut seed_mismatch = IntegratedServer::with_world_store(2, Box::new(seed_store));
    assert!(
        seed_mismatch
            .initialize_world_metadata_at_unix_millis(2_000)
            .unwrap_err()
            .to_string()
            .contains("world seed mismatch")
    );

    let mut profile_store = MemoryWorldStore::new();
    profile_store
        .save_world_metadata(&WorldMetadata::new(
            2,
            WorldGenerationProfile::authored_only(),
            WorldBehaviorProfile::ProtectedLobby,
            1_000,
        ))
        .unwrap();
    let mut profile_mismatch = IntegratedServer::with_world_store(2, Box::new(profile_store));
    assert!(
        profile_mismatch
            .initialize_world_metadata_at_unix_millis(2_000)
            .unwrap_err()
            .to_string()
            .contains("world generation profile mismatch")
    );
}

#[test]
fn daylight_rule_and_debug_freeze_keep_distinct_durable_semantics() {
    let mut server = IntegratedServer::with_world_store(99, Box::new(MemoryWorldStore::new()));
    server
        .initialize_world_metadata_at_unix_millis(1_000)
        .unwrap();
    server.set_do_daylight_cycle(false);
    for _ in 0..3 {
        server.try_simulation_tick_report().unwrap();
    }
    server.save_world_metadata_at_unix_millis(2_000).unwrap();
    assert_eq!(server.game_time(), 3);
    assert_eq!(server.day_time(), 0);
    assert!(!server.world_metadata().unwrap().do_daylight_cycle);

    server.set_do_daylight_cycle(true);
    server.set_day_time_frozen(true);
    server.try_simulation_tick_report().unwrap();
    server.save_world_metadata_at_unix_millis(3_000).unwrap();
    assert_eq!(server.game_time(), 4);
    assert_eq!(server.day_time(), 0);
    assert!(server.world_metadata().unwrap().do_daylight_cycle);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sqlite_restart_resumes_exact_game_and_day_time() {
    let root = world_time_temp_dir("sqlite-clock-restart");
    let seed = 101;
    let expected;
    {
        let mut server = IntegratedServer::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        server
            .initialize_world_metadata_at_unix_millis(1_000)
            .unwrap();
        for _ in 0..25 {
            server.try_simulation_tick_report().unwrap();
        }
        server.set_do_daylight_cycle(false);
        for _ in 0..4 {
            server.try_simulation_tick_report().unwrap();
        }
        expected = (server.game_time(), server.day_time());
        server.save_world_metadata_at_unix_millis(2_000).unwrap();
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            IntegratedServer::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        let metadata = reopened
            .initialize_world_metadata_at_unix_millis(3_000)
            .unwrap();
        assert_eq!((metadata.game_time, metadata.day_time), expected);
        assert!(!metadata.do_daylight_cycle);
        reopened.try_simulation_tick_report().unwrap();
        assert_eq!(reopened.game_time(), expected.0 + 1);
        assert_eq!(reopened.day_time(), expected.1);
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
fn world_time_temp_dir(name: &str) -> std::path::PathBuf {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("mclone-{name}-{}-{id}", std::process::id()))
}

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
fn current_single_dimension_runtime_rejects_non_overworld_resume_records() {
    let seed = 12_345;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x43; 16]), "Traveler").unwrap();
    let mut record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        8,
        identity.display_name.clone(),
        Vec3d::new(4_096.5, 200.0, -4_096.5),
    );
    record.dimension = "mclone:moon".to_owned();
    record.selected_hotbar_slot = 6;
    record.total_experience = 41;
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = IntegratedServer::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let spawned = wait_for_initial_spawn_update(&mut server);

    assert_ne!(spawned.position, record.position);
    assert_eq!(server.inventory.selected_hotbar_slot(), 0);
    assert_eq!(server.local_player_total_experience, 0);
    assert!(server.local_player_resume_record.is_none());
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
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
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
fn persistence_demo_awards_exactly_one_point_per_accepted_upward_jump() {
    let mut server = IntegratedServer::new(0);
    server.set_persistence_demo_jump_experience_enabled(true);
    send_player_move(&mut server, Vec3d::new(1.0, 64.0, 1.0));

    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(1.0, 64.42, 1.0),
            on_ground: false,
        }))
        .unwrap();
    assert_eq!(
        updates,
        vec![ServerUpdate::PlayerExperience {
            total_experience: 1
        }]
    );

    let duplicate = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(1.0, 64.42, 1.0),
            on_ground: false,
        }))
        .unwrap();
    assert!(duplicate.is_empty());
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::StatusOnly {
                on_ground: true
            }))
            .unwrap()
            .is_empty()
    );
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Rot {
                y_rot_degrees: 90.0,
                x_rot_degrees: 0.0,
                on_ground: false,
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.local_player_total_experience, 1);
}

#[test]
fn persistence_demo_is_explicit_and_protected_worlds_never_award_jump_experience() {
    let mut server = IntegratedServer::new(0);
    send_player_move(&mut server, Vec3d::new(0.0, 64.0, 0.0));
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(0.0, 64.42, 0.0),
                on_ground: false,
            }))
            .unwrap()
            .is_empty()
    );

    server.set_persistence_demo_jump_experience_enabled(true);
    server.set_world_behavior_profile(WorldBehaviorProfile::ProtectedLobby);
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::StatusOnly {
            on_ground: true,
        }))
        .unwrap();
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(0.0, 64.84, 0.0),
                on_ground: false,
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.local_player_total_experience, 0);
}

#[test]
fn awarded_demo_experience_is_written_to_the_identity_player_record() {
    let identity = ClientIdentity::new(PlayerProfileId::new([0x55; 16]), "Jumper").unwrap();
    let key = player_record_key(identity.profile_id);
    let mut server = IntegratedServer::with_world_store(0, Box::new(MemoryWorldStore::new()));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    server.set_persistence_demo_jump_experience_enabled(true);
    send_player_move(&mut server, Vec3d::new(0.0, 64.0, 0.0));
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(0.0, 64.42, 0.0),
            on_ground: false,
        }))
        .unwrap();

    assert_eq!(server.save_all_player_records().unwrap(), 1);
    let record = server
        .scheduler_mut()
        .load_player_record_blocking(key)
        .unwrap()
        .expect("saved player record");

    assert_eq!(record.total_experience, 1);
    assert_eq!(record.position, Vec3d::new(0.0, 64.42, 0.0));
}

#[test]
fn simulation_tick_records_java_shaped_movement_packet_boundary() {
    let mut server = IntegratedServer::new(0);

    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(1.0, 64.0, 1.0),
            on_ground: true,
        }))
        .expect("move player");
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Rot {
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
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
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
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move while awaiting teleport at threshold");
    assert!(updates.is_empty());

    server
        .try_simulation_tick_report()
        .expect("pending teleport resend tick");
    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
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
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
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
