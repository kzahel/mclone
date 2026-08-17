use super::*;
use crate::{ChunkResidency, MemoryWorldStore};

fn mclone_calendar_session(seed: i64) -> LocalRealmSession {
    let definition =
        crate::DimensionDefinition::overworld(seed, WorldGenerationProfile::McloneOverworldV1);
    LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    )
}

fn time_update_count(updates: &[ServerUpdate]) -> usize {
    updates
        .iter()
        .filter(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        .count()
}

#[test]
fn typed_civil_time_mutations_are_checked_durable_and_publish_once() {
    let mut server = mclone_calendar_session(761);
    server
        .initialize_world_metadata_at_unix_millis(1_000)
        .unwrap();
    let _ = server.try_poll().unwrap();
    server.set_day_time(2 * mclone_core::time::DAY_LENGTH_TICKS + 500);
    let _ = server.try_poll().unwrap();

    let changed = server.set_time_of_day_preserving_date(1_250).unwrap();
    assert_eq!(changed, 2 * mclone_core::time::DAY_LENGTH_TICKS + 1_250);
    assert_eq!(server.day_time(), changed);
    assert_eq!(time_update_count(&server.try_poll().unwrap()), 1);
    assert_eq!(
        server.set_time_of_day_preserving_date(24_000),
        Err(CivilTimeMutationError::DayTickOutOfRange { day_tick: 24_000 })
    );
    assert_eq!(server.day_time(), changed);
    assert_eq!(time_update_count(&server.try_poll().unwrap()), 0);

    let last_day = server.set_calendar_date(1, 56, None).unwrap();
    assert_eq!(
        last_day,
        (56 + 55) * mclone_core::time::DAY_LENGTH_TICKS + 1_250
    );
    let sample = server.season_calendar_sample().unwrap().unwrap();
    assert_eq!(
        (sample.year_index, sample.day_of_year, sample.day_tick),
        (1, 56, 1_250)
    );
    assert_eq!(time_update_count(&server.try_poll().unwrap()), 1);

    let wrapped = server.set_calendar_date(2, 1, Some(0)).unwrap();
    assert_eq!(wrapped, 112 * mclone_core::time::DAY_LENGTH_TICKS);
    assert_eq!(time_update_count(&server.try_poll().unwrap()), 1);
    assert_eq!(
        server.set_calendar_date(0, 0, None),
        Err(CivilTimeMutationError::DayOfYearOutOfRange {
            day_of_year: 0,
            days_per_year: 56,
        })
    );
    assert_eq!(
        server.set_calendar_date(0, 57, None),
        Err(CivilTimeMutationError::DayOfYearOutOfRange {
            day_of_year: 57,
            days_per_year: 56,
        })
    );
    assert_eq!(
        server.set_calendar_date(u64::MAX, 1, Some(0)),
        Err(CivilTimeMutationError::ArithmeticOverflow)
    );
    assert_eq!(server.day_time(), wrapped);
    assert_eq!(time_update_count(&server.try_poll().unwrap()), 0);

    let morning = server.advance_to_next_morning().unwrap();
    assert_eq!(morning, 113 * mclone_core::time::DAY_LENGTH_TICKS);
    assert_eq!(time_update_count(&server.try_poll().unwrap()), 1);
    assert_eq!(server.game_time(), 0);

    assert_eq!(server.save_world_metadata_at_unix_millis(2_000).unwrap(), 1);
    let saved = server.world_metadata().unwrap();
    assert_eq!(saved.day_time, morning);
    assert_eq!(saved.game_time, 0);
}

#[test]
fn typed_calendar_mutation_rejects_disabled_policy_and_clock_overflow() {
    let mut disabled = LocalRealmSession::with_world_store(762, Box::new(MemoryWorldStore::new()));
    disabled
        .initialize_world_metadata_at_unix_millis(1_000)
        .unwrap();
    assert_eq!(
        disabled.set_calendar_date(0, 1, Some(0)),
        Err(CivilTimeMutationError::CalendarDisabled)
    );
    assert_eq!(disabled.day_time(), 0);

    let mut server = mclone_calendar_session(763);
    server.set_day_time(u64::MAX);
    assert_eq!(
        server.advance_to_next_morning(),
        Err(CivilTimeMutationError::ArithmeticOverflow)
    );
    assert_eq!(server.day_time(), u64::MAX);
    assert_eq!(
        server.set_time_of_day_preserving_date(23_999),
        Err(CivilTimeMutationError::ArithmeticOverflow)
    );
    assert_eq!(server.day_time(), u64::MAX);
}

#[test]
fn civil_time_discontinuities_do_not_replay_simulation_work() {
    let mut server = mclone_calendar_session(764);
    server
        .initialize_world_metadata_at_unix_millis(1_000)
        .unwrap();
    server.set_natural_spawning_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    load_center_chunk(&mut server);

    let crop = BlockPos::new(8, 65, 8);
    server
        .scheduler_mut()
        .set_block_at_world(crop, mclone_worldgen::block::CARROTS_AGE_4);
    let fluid = BlockPos::new(10, 70, 10);
    server.scheduler_mut().set_block_at_world(fluid, WATER);
    server
        .scheduler_mut()
        .set_block_at_world(fluid.below(), AIR);
    server.schedule_fluid_tick(fluid, FluidKind::Water, 200);

    let rabbit = server.entities.insert_passive_mob_for_test(
        EntityKind::Rabbit,
        Vec3d::new(8.5, 66.0, 8.5),
        0.0,
    );
    let forage_feet = BlockPos::new(12, 65, 12);
    server
        .scheduler_mut()
        .set_block_at_world(forage_feet.below(), GRASS_BLOCK);
    let resource_sample_tick = server.simulation_tick;
    {
        let runtime = &mut server.active_dimension;
        let scheduler = &runtime.scheduler;
        let intake = runtime.wildlife_resources.consume_diet_at(
            crate::wildlife_resources::WildlifeForageConsumer::Mallard,
            &crate::wildlife_resources::MALLARD_DIET,
            forage_feet,
            0,
            0,
            100,
            resource_sample_tick,
            &|pos| {
                scheduler
                    .block_at_world(pos)
                    .map(|block| BlockStateId(u32::from(block)))
            },
        );
        assert!(intake.units > 0);
    }

    let simulation_tick = server.simulation_tick();
    let crop_state = server.scheduler().block_at_world(crop);
    let fluid_ticks = server.scheduled_fluid_tick_count();
    let rabbit_state = server.entities.state(rabbit).unwrap();
    let life = server.wildlife_life_diagnostics();
    let resources = server.wildlife_forage_cells();
    assert!(!resources.is_empty());

    server.set_calendar_date(20, 40, Some(23_999)).unwrap();
    server.set_calendar_date(0, 1, Some(0)).unwrap();

    assert_eq!(server.simulation_tick(), simulation_tick);
    assert_eq!(server.scheduler().block_at_world(crop), crop_state);
    assert_eq!(server.scheduled_fluid_tick_count(), fluid_ticks);
    assert_eq!(server.entities.state(rabbit), Some(rabbit_state));
    assert_eq!(server.wildlife_life_diagnostics(), life);
    assert_eq!(server.wildlife_forage_cells(), resources);
}

#[test]
fn new_world_metadata_starts_at_vanilla_zero_and_tracks_both_clocks() {
    let mut server = LocalRealmSession::with_world_store(77, Box::new(MemoryWorldStore::new()));
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
    let mut server = LocalRealmSession::with_world_store(88, Box::new(store));

    let initialized = server
        .initialize_world_metadata_at_unix_millis(3_000)
        .unwrap();

    assert_eq!(initialized.game_time, 0);
    assert_eq!(initialized.day_time, INITIAL_DAY_TIME);
}

#[test]
fn stored_world_metadata_rejects_seed_profile_and_starter_mismatches() {
    let mut seed_store = MemoryWorldStore::new();
    seed_store
        .save_world_metadata(&WorldMetadata::new(
            1,
            WorldGenerationProfile::Overworld,
            WorldBehaviorProfile::Mutable,
            1_000,
        ))
        .unwrap();
    let mut seed_mismatch = LocalRealmSession::with_world_store(2, Box::new(seed_store));
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
    let mut profile_mismatch = LocalRealmSession::with_world_store(2, Box::new(profile_store));
    assert!(
        profile_mismatch
            .initialize_world_metadata_at_unix_millis(2_000)
            .unwrap_err()
            .to_string()
            .contains("world generation profile mismatch")
    );

    let mut starter_store = MemoryWorldStore::new();
    starter_store
        .save_world_metadata(&WorldMetadata::new(
            2,
            WorldGenerationProfile::Overworld,
            WorldBehaviorProfile::Mutable,
            1_000,
        ))
        .unwrap();
    let mut starter_mismatch = LocalRealmSession::with_world_store(2, Box::new(starter_store));
    starter_mismatch.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
    assert!(
        starter_mismatch
            .initialize_world_metadata_at_unix_millis(2_000)
            .unwrap_err()
            .to_string()
            .contains("starter content mismatch")
    );
}

#[test]
fn new_world_metadata_persists_realized_starter_plan_orthogonally() {
    let definition =
        crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);

    let initialized = server
        .initialize_world_metadata_at_unix_millis(1_000)
        .unwrap();

    assert_eq!(
        initialized.world_generation_profile,
        WorldGenerationProfile::McloneOverworldV1
    );
    assert_eq!(
        initialized.starter_content,
        StarterContentDescriptor::IntroHomesteadV1
    );
    assert_eq!(
        initialized.realized_starter_plan,
        Some(server.intro_homestead_plan().unwrap().identity().unwrap())
    );
    assert_eq!(
        server.intro_homestead_plan().unwrap().checksum_sha256,
        "5ea0fc552ed3b131a4329b3fae9ca658a831157f15679ed84e665c18c0a75064"
    );
}

#[test]
fn new_identified_player_arrives_at_the_persisted_homestead_spawn() {
    let definition =
        crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
    server.initialize_world_metadata_blocking().unwrap();
    server.set_debug_passive_showcase_enabled(false);
    let identity = ClientIdentity::new(PlayerProfileId::new([0x71; 16]), "Newcomer").unwrap();
    let key = player_record_key(identity.profile_id);
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();

    let intent = server.realm_primary_spawn_intent().unwrap();
    request_initial_chunk_view(&mut server);
    assert_eq!(
        server
            .chunk_tracking
            .accepted_view(server.player_id())
            .unwrap()
            .center,
        intent.center,
        "the client-requested provisional view must be retargeted before publication"
    );
    let arrival = wait_for_initial_spawn_update(&mut server);

    assert_eq!(arrival.position, intent.preferred_position.unwrap());
    assert_eq!(arrival.y_rot_degrees, intent.y_rot_degrees);
    assert_eq!(arrival.x_rot_degrees, 0.0);
    assert!(server.player_pose_is_safe_spawn(arrival.position));

    server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: arrival.teleport_id,
        }))
        .unwrap();
    let persisted = server
        .scheduler_mut()
        .load_player_record_blocking(key)
        .unwrap()
        .expect("accepted first arrival must immediately create a player record");
    assert_eq!(persisted.position, arrival.position);
    assert_eq!(persisted.y_rot_degrees, arrival.y_rot_degrees);
}

#[test]
fn asynchronous_returning_player_load_wins_over_homestead_arrival() {
    let definition =
        crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
    server.initialize_world_metadata_blocking().unwrap();
    server.set_debug_passive_showcase_enabled(false);
    let identity = ClientIdentity::new(PlayerProfileId::new([0x72; 16]), "Returner").unwrap();
    let control = server.intro_homestead_plan().unwrap().arrival_path.controls[1].pos;
    let returned_position = Vec3d::new(
        f64::from(control[0]) + 0.5,
        f64::from(control[1]),
        f64::from(control[2]) + 0.5,
    );
    let mut record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        9,
        identity.display_name.clone(),
        returned_position,
    );
    record.y_rot_degrees = 37.0;
    record.x_rot_degrees = -12.0;
    record.on_ground = true;
    server.scheduler_mut().save_player_record(record.clone());

    server.configure_local_player_identity(identity).unwrap();
    request_initial_chunk_view(&mut server);
    let resumed = wait_for_initial_spawn_update(&mut server);

    assert_eq!(resumed.position, returned_position);
    assert_eq!(resumed.y_rot_degrees, 37.0);
    assert_eq!(resumed.x_rot_degrees, -12.0);
    assert!(server.player().on_ground());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sqlite_reopen_retains_post_arrival_pose_instead_of_teleporting_again() {
    let root = world_time_temp_dir("sqlite-homestead-arrival-reopen");
    let identity = ClientIdentity::new(PlayerProfileId::new([0x74; 16]), "Settler").unwrap();
    let returned_position;
    {
        let definition =
            crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
        let mut realm =
            RealmServer::try_with_threaded_sqlite_world_dir_dimension_definition(definition, &root)
                .unwrap();
        realm.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
        realm.initialize_world_metadata_blocking().unwrap();
        let mut server = LocalRealmSession::from_server(realm);
        server.set_debug_passive_showcase_enabled(false);
        server
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        request_initial_chunk_view(&mut server);
        let arrival = wait_for_initial_spawn_update(&mut server);
        server
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: arrival.teleport_id,
            }))
            .unwrap();
        let control = server.intro_homestead_plan().unwrap().arrival_path.controls[1].pos;
        returned_position = Vec3d::new(
            f64::from(control[0]) + 0.5,
            f64::from(control[1]),
            f64::from(control[2]) + 0.5,
        );
        assert!(server.player_pose_is_safe_spawn(returned_position));
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
                position: returned_position,
                y_rot_degrees: 37.0,
                x_rot_degrees: -12.0,
                on_ground: true,
            }))
            .unwrap();
        server.shutdown_persistence().unwrap();
    }

    {
        let definition =
            crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
        let mut realm =
            RealmServer::try_with_threaded_sqlite_world_dir_dimension_definition(definition, &root)
                .unwrap();
        realm.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
        realm.initialize_world_metadata_blocking().unwrap();
        let mut reopened = LocalRealmSession::from_server(realm);
        reopened.set_debug_passive_showcase_enabled(false);
        reopened
            .configure_local_player_identity_blocking(identity)
            .unwrap();
        request_initial_chunk_view(&mut reopened);
        let resumed = wait_for_initial_spawn_update(&mut reopened);

        assert_eq!(resumed.position, returned_position);
        assert_eq!(resumed.y_rot_degrees, 37.0);
        assert_eq!(resumed.x_rot_degrees, -12.0);
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn blocked_homestead_spawn_respawns_safely_without_using_death_pose() {
    let definition =
        crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
    server.initialize_world_metadata_blocking().unwrap();
    server.set_debug_passive_showcase_enabled(false);
    let identity = ClientIdentity::new(PlayerProfileId::new([0x73; 16]), "Respawner").unwrap();
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let arrival = wait_for_initial_spawn_update(&mut server);
    server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: arrival.teleport_id,
        }))
        .unwrap();

    let death_pose = arrival.position;
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::containing(death_pose), LAVA)
    );
    server.try_simulation_tick_report().unwrap();
    assert!(server.player_vitals().is_dead());

    let player_id = server.player_id();
    let mut updates = server.try_handle_command(ClientCommand::Respawn).unwrap();
    let mut respawn = None;
    for _ in 0..60_000 {
        respawn = respawn.or_else(|| {
            updates.iter().find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(*update),
                _ => None,
            })
        });
        if respawn.is_some() {
            break;
        }
        server.try_tick_report_global().unwrap();
        updates.extend(server.try_drain_updates_for_player(player_id).unwrap());
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    let respawn = respawn.expect("blocked homestead respawn must find a safe nearby column");

    assert_ne!(respawn.position, death_pose);
    assert!(server.player_pose_is_safe_spawn(respawn.position));
    assert_eq!(
        chunk_pos_for_player_position(respawn.position),
        server.realm_primary_spawn_intent().unwrap().center
    );
    assert!(!server.player_vitals().is_dead());
}

#[test]
fn daylight_rule_and_debug_freeze_keep_distinct_durable_semantics() {
    let mut server = LocalRealmSession::with_world_store(99, Box::new(MemoryWorldStore::new()));
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
    let expected_realm_id;
    {
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        server
            .initialize_world_metadata_at_unix_millis(1_000)
            .unwrap();
        expected_realm_id = server.realm_id();
        assert_ne!(expected_realm_id, RealmId::LEGACY_SINGLE_REALM);
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
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        let metadata = reopened
            .initialize_world_metadata_at_unix_millis(3_000)
            .unwrap();
        assert_eq!((metadata.game_time, metadata.day_time), expected);
        assert_eq!(metadata.realm_id, expected_realm_id);
        assert_eq!(reopened.realm_id(), expected_realm_id);
        assert!(!metadata.do_daylight_cycle);
        reopened.try_simulation_tick_report().unwrap();
        assert_eq!(reopened.game_time(), expected.0 + 1);
        assert_eq!(reopened.day_time(), expected.1);
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sqlite_restart_restores_sleep_jump_and_mat_but_not_sleep_state() {
    let root = world_time_temp_dir("sqlite-sleep-mat-restart");
    let seed = 102;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x75; 16]), "Sleeper").unwrap();
    let expected_clock;
    let mat_persistent_id;
    {
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        server
            .set_world_generation_profile(WorldGenerationProfile::McloneOverworldV1)
            .unwrap();
        server
            .initialize_world_metadata_at_unix_millis(1_000)
            .unwrap();
        server
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 65.0, 10.5));
        let floor = BlockPos::new(8, 64, 8);
        server.scheduler_mut().set_block_at_world(floor, STONE);
        server
            .scheduler_mut()
            .set_block_at_world(floor.offset(0, 1, 0), AIR);
        server.scheduler_mut().drain_pending_block_delta_events();

        sync_carried_slot(&mut server, 5);
        let placed = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 65.0, 8.5),
                Direction::Up,
                floor,
                false,
            )))
            .unwrap();
        let mat = placed
            .iter()
            .find_map(|update| match update {
                ServerUpdate::EntitySnapshot(snapshot)
                    if snapshot.kind == EntityKind::SleepingMat =>
                {
                    Some(*snapshot)
                }
                _ => None,
            })
            .unwrap();
        mat_persistent_id = server.entities.state(mat.id).unwrap().persistent_id;

        server.set_day_time(u64::from(SLEEP_START_DAY_TICK));
        let _ = server.try_poll().unwrap();
        server
            .try_handle_command(ClientCommand::InteractEntity(InteractEntityCommand {
                target: mat.id,
                hand: InteractionHand::MainHand,
            }))
            .unwrap();
        assert!(server.player_is_sleeping(server.player_id()));
        server.try_simulation_tick_report().unwrap();
        expected_clock = (server.game_time(), server.day_time());
        assert_eq!(expected_clock.1, mclone_core::time::DAY_LENGTH_TICKS);
        assert_eq!(server.sleeping_player_count(), 0);
        server.save_world_metadata_at_unix_millis(2_000).unwrap();
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        reopened
            .set_world_generation_profile(WorldGenerationProfile::McloneOverworldV1)
            .unwrap();
        let metadata = reopened
            .initialize_world_metadata_at_unix_millis(3_000)
            .unwrap();
        reopened
            .configure_local_player_identity_blocking(identity)
            .unwrap();
        assert_eq!((metadata.game_time, metadata.day_time), expected_clock);
        assert_eq!(reopened.sleeping_player_count(), 0);
        load_center_chunk(&mut reopened);
        assert!(
            reopened
                .entities
                .state_by_persistent_id(mat_persistent_id)
                .is_some_and(|entity| entity.kind == EntityKind::SleepingMat)
        );
        assert_eq!(reopened.inventory().item_count(ItemKind::SleepingMat), 0);
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sqlite_restart_preserves_statistics_and_another_realm_is_independent() {
    let first_root = world_time_temp_dir("sqlite-statistics-restart");
    let second_root = world_time_temp_dir("sqlite-statistics-other-realm");
    let identity = ClientIdentity::new(PlayerProfileId::new([0x71; 16]), "Statistician").unwrap();
    let first_realm_id;
    {
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(12_345, &first_root).unwrap();
        server
            .initialize_world_metadata_at_unix_millis(1_000)
            .unwrap();
        first_realm_id = server.realm_id();
        server
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 82.0, 8.5));
        sync_carried_slot(&mut server, 1);
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        server.scheduler_mut().set_block_at_world(clicked, STONE);
        server.scheduler_mut().set_block_at_world(target, AIR);
        server.scheduler_mut().drain_pending_block_delta_events();
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .unwrap();
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(8.5, 82.42, 8.5),
                on_ground: false,
            }))
            .unwrap();
        assert_eq!(server.player_statistics().jump_count(), 1);
        assert_eq!(
            server
                .player_statistics()
                .successful_block_placement_count(),
            1
        );
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(12_345, &first_root).unwrap();
        reopened
            .initialize_world_metadata_at_unix_millis(2_000)
            .unwrap();
        reopened
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        assert_eq!(reopened.realm_id(), first_realm_id);
        assert_eq!(reopened.player_statistics().jump_count(), 1);
        assert_eq!(
            reopened
                .player_statistics()
                .successful_block_placement_count(),
            1
        );
        let updates = reopened.try_drain_updates().unwrap();
        assert!(updates.iter().any(|update| matches!(
            update,
            ServerUpdate::PlayerStatistics { statistics }
                if statistics.jump_count() == 1
                    && statistics.successful_block_placement_count() == 1
        )));
        reopened.shutdown_persistence().unwrap();
    }

    {
        let mut other_realm =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(12_345, &second_root).unwrap();
        other_realm
            .initialize_world_metadata_at_unix_millis(1_000)
            .unwrap();
        other_realm
            .configure_local_player_identity_blocking(identity)
            .unwrap();
        assert_ne!(other_realm.realm_id(), first_realm_id);
        assert!(other_realm.player_statistics().is_empty());
        other_realm.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(first_root).unwrap();
    std::fs::remove_dir_all(second_root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sqlite_restart_restores_dead_then_respawned_player_lifecycle() {
    let root = world_time_temp_dir("sqlite-player-death-respawn-restart");
    let seed = 12_345;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x72; 16]), "Persistent").unwrap();
    let death_position = Vec3d::new(8.5, 80.0, 8.5);

    {
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        server
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        load_center_chunk(&mut server);
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(BlockPos::containing(death_position), LAVA)
        );
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: death_position,
                on_ground: false,
            }))
            .unwrap();
        assert!(server.player_vitals().is_dead());
        assert_eq!(server.player_statistics().death_count(), 1);
        server.shutdown_persistence().unwrap();
    }

    let respawn_position;
    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        reopened
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        assert!(reopened.player_vitals().is_dead());
        assert_eq!(
            reopened.pending_death_cause(),
            Some(PlayerDamageCause::Lava)
        );
        assert_eq!(reopened.player_statistics().death_count(), 1);
        let joined = reopened.try_drain_updates().unwrap();
        assert!(joined.iter().any(|update| matches!(
            update,
            ServerUpdate::PlayerLife(life)
                if life.vitals().is_dead()
                    && life.death_cause() == Some(PlayerDamageCause::Lava)
        )));
        request_initial_chunk_view(&mut reopened);
        let restored_death_pose = wait_for_initial_spawn_update(&mut reopened);
        reopened
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: restored_death_pose.teleport_id,
            }))
            .unwrap();

        let mut updates = reopened.try_handle_command(ClientCommand::Respawn).unwrap();
        let mut position_update = None;
        for _ in 0..60_000 {
            position_update = position_update.or_else(|| {
                updates.iter().find_map(|update| match update {
                    ServerUpdate::PlayerPosition(update) => Some(*update),
                    _ => None,
                })
            });
            if position_update.is_some() {
                break;
            }
            reopened.try_tick_report_global().unwrap();
            updates.extend(reopened.try_drain_updates().unwrap());
            if reopened.pending_job_count() > 0 && reopened.pending_publication_count() == 0 {
                reopened.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        let update = position_update.expect("reopened dead player must safely respawn");
        respawn_position = update.position;
        assert!(updates.iter().any(|update| matches!(
            update,
            ServerUpdate::PlayerLife(life)
                if !life.vitals().is_dead() && life.death_cause().is_none()
        )));
        reopened
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: update.teleport_id,
            }))
            .unwrap();
        assert_eq!(reopened.player_vitals().health(), 20.0);
        assert_eq!(reopened.player_statistics().death_count(), 1);
        reopened.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        reopened
            .configure_local_player_identity_blocking(identity)
            .unwrap();
        assert_eq!(reopened.player_vitals().health(), 20.0);
        assert_eq!(reopened.pending_death_cause(), None);
        assert_eq!(reopened.player_statistics().death_count(), 1);
        request_initial_chunk_view(&mut reopened);
        let resumed = wait_for_initial_spawn_update(&mut reopened);
        assert_eq!(resumed.position, respawn_position);
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sqlite_restart_restores_mclone_profile_before_unseen_generation() {
    let root = world_time_temp_dir("sqlite-mclone-profile-restart");
    let seed = 12_345;
    let stored_chunk = ChunkPos::new(0, 0);
    let unseen_chunk = ChunkPos::new(128, -127);
    let edited = BlockPos::new(8, 64, 8);
    let replacement;

    {
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        server
            .set_world_generation_profile(WorldGenerationProfile::McloneOverworldV1)
            .unwrap();
        let metadata = server
            .initialize_world_metadata_at_unix_millis(1_000)
            .unwrap();
        assert_eq!(
            metadata.world_generation_profile,
            WorldGenerationProfile::McloneOverworldV1
        );
        load_chunk_view(&mut server, stored_chunk);
        replacement = if server.scheduler().block_at_world(edited) == Some(STONE) {
            DIRT
        } else {
            STONE
        };
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(edited, replacement)
        );
        server.scheduler_mut().flush_persistence().unwrap();
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        reopened
            .set_world_generation_profile(WorldGenerationProfile::McloneOverworldV1)
            .unwrap();
        let metadata = reopened
            .initialize_world_metadata_at_unix_millis(2_000)
            .unwrap();
        assert_eq!(
            metadata.world_generation_profile,
            WorldGenerationProfile::McloneOverworldV1
        );

        load_chunk_view(&mut reopened, stored_chunk);
        assert_eq!(
            reopened.scheduler().block_at_world(edited),
            Some(replacement)
        );
        assert_eq!(
            reopened
                .scheduler()
                .holder(stored_chunk)
                .map(ChunkHolder::residency),
            Some(ChunkResidency::LoadedFromStore)
        );
        assert!(reopened.scheduler().holder(unseen_chunk).is_none());

        let expected = mclone_worldgen::levelgen::generate_mclone_overworld_chunk(
            seed,
            unseen_chunk.x,
            unseen_chunk.z,
        );
        let local_x = 8;
        let local_z = 8;
        let expected_surface_y = (expected.min_y..expected.min_y + expected.height)
            .rev()
            .find(|y| expected.block_at_y(local_x, *y, local_z).0 != mclone_worldgen::block::AIR)
            .expect("Mclone unseen chunk column should contain terrain");
        let expected_state = expected.block_at_y(local_x, expected_surface_y, local_z).0;

        load_chunk_view(&mut reopened, unseen_chunk);
        let world_x = unseen_chunk.min_block_x() + local_x;
        let world_z = unseen_chunk.min_block_z() + local_z;
        let expected_pos = BlockPos::new(world_x, expected_surface_y, world_z);
        let mut generated_state = reopened.scheduler().block_at_world(expected_pos);
        for _ in 0..60_000 {
            if generated_state.is_some() {
                break;
            }
            let updates = reopened.try_poll().unwrap();
            accept_player_position_updates(&mut reopened, &updates);
            if reopened.pending_publication_count() == 0 {
                reopened.wait_for_worldgen_completion(Duration::from_millis(1));
            }
            std::thread::yield_now();
            generated_state = reopened.scheduler().block_at_world(expected_pos);
        }
        assert_eq!(generated_state, Some(expected_state));
        assert_eq!(
            reopened
                .scheduler()
                .holder(unseen_chunk)
                .map(ChunkHolder::residency),
            Some(ChunkResidency::Generated)
        );
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
    let mut server = LocalRealmSession::new(0);
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
    let mut server = LocalRealmSession::new(12345);
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
            .player()
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
    assert_eq!(server.player().awaiting_teleport(), None);
    assert_eq!(server.player().position(), spawn.position);
}

#[test]
fn seed_zero_mclone_wild_start_arrives_on_the_profile_selected_inland_surface() {
    let seed = 0;
    let profile = WorldGenerationProfile::McloneOverworldV1;
    let definition = crate::DimensionDefinition::overworld(seed, profile);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server.set_debug_passive_showcase_enabled(false);
    server.initialize_world_metadata_blocking().unwrap();
    server
        .configure_local_player_identity_blocking(
            ClientIdentity::new(PlayerProfileId::new([0x70; 16]), "Explorer").unwrap(),
        )
        .unwrap();

    let center = crate::initial_spawn_center_for_profile(seed, profile);
    server.set_lighting_enabled(false);
    let updates = server
        .try_handle_command(ClientCommand::SetChunkView(ChunkView {
            center,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }))
        .unwrap();
    assert!(
        updates
            .iter()
            .all(|update| !matches!(update, ServerUpdate::PlayerPosition(_)))
    );
    assert_eq!(
        server
            .chunk_tracking
            .accepted_view(server.player_id())
            .unwrap()
            .center,
        center
    );

    let spawn = wait_for_initial_spawn_update(&mut server);
    let feet = BlockPos::containing(spawn.position);
    assert_eq!(feet.chunk_pos(), center);
    assert_eq!(
        server.scheduler().block_at_world(feet.below()),
        Some(GRASS_BLOCK)
    );
    assert!(server.player_pose_is_safe_spawn(spawn.position));
}

#[test]
fn saved_player_pose_and_selected_slot_resume_for_stable_identity() {
    let seed = 12_345;
    let mut probe = LocalRealmSession::new(seed);
    request_initial_chunk_view(&mut probe);
    let safe_spawn = wait_for_initial_spawn_update(&mut probe).position;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x42; 16]), "Builder").unwrap();
    let mut record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        7,
        identity.display_name.clone(),
        safe_spawn,
    );
    record.y_rot_degrees = 135.0;
    record.x_rot_degrees = -22.5;
    record.on_ground = true;
    record.selected_hotbar_slot = 4;
    record.total_experience = 19;
    record.health = 13.5;
    record
        .deer_field_guide
        .observe(mclone_protocol::DeerObservationKind::FoundSign);
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = LocalRealmSession::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let provisional_position = Vec3d::new(-2_048.5, 200.0, 2_048.5);
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: provisional_position,
                on_ground: false,
            }))
            .unwrap()
            .is_empty(),
        "a premature move should wait when the saved destination is not ready"
    );
    assert!(
        !server.player().has_accepted_position(),
        "the provisional client camera must not claim initial-position authority"
    );
    let (resumed, arrival_batch) = wait_for_initial_spawn_update_batch(&mut server);

    assert_eq!(resumed.position, record.position);
    assert_eq!(resumed.y_rot_degrees, record.y_rot_degrees);
    assert_eq!(resumed.x_rot_degrees, record.x_rot_degrees);
    assert!(server.player().on_ground());
    assert_eq!(server.inventory().selected_hotbar_slot(), 4);
    assert_eq!(server.total_experience(), 19);
    assert_eq!(server.player_vitals().health(), 13.5);
    assert!(
        server
            .server()
            .players
            .get(server.player_id())
            .unwrap()
            .deer_field_guide
            .contains(mclone_protocol::DeerObservationKind::FoundSign)
    );
    assert_eq!(server.pending_death_cause(), None);
    let position_index = arrival_batch
        .iter()
        .position(|update| matches!(update, ServerUpdate::PlayerPosition(_)))
        .expect("arrival batch must contain the restored position");
    assert!(
        arrival_batch[..position_index]
            .iter()
            .all(|update| !matches!(update, ServerUpdate::ChunkSnapshot(_))),
        "the restored position must lead bulk chunk snapshots"
    );
}

#[test]
fn persisted_zero_health_restores_a_typed_dead_player_state() {
    let seed = 12_345;
    let mut probe = LocalRealmSession::new(seed);
    request_initial_chunk_view(&mut probe);
    let safe_spawn = wait_for_initial_spawn_update(&mut probe).position;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x47; 16]), "Swimmer").unwrap();
    let mut record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        9,
        identity.display_name.clone(),
        safe_spawn,
    );
    record.health = 0.0;
    record.pending_death_cause = Some(mclone_protocol::PlayerDamageCause::Lava);
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = LocalRealmSession::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let _ = wait_for_initial_spawn_update(&mut server);

    assert!(server.player_vitals().is_dead());
    assert_eq!(
        server.pending_death_cause(),
        Some(mclone_protocol::PlayerDamageCause::Lava)
    );
}

#[test]
fn accepted_lava_contact_kills_once_and_gates_physical_commands() {
    let mut server = LocalRealmSession::new(12_345);
    load_center_chunk(&mut server);
    let position = Vec3d::new(8.5, 80.0, 8.5);
    let lava_pos = BlockPos::containing(position);
    assert!(server.scheduler_mut().set_block_at_world(lava_pos, LAVA));

    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
            position,
            y_rot_degrees: 15.0,
            x_rot_degrees: 0.0,
            on_ground: false,
        }))
        .unwrap();

    assert!(server.player_vitals().is_dead());
    assert_eq!(
        server.pending_death_cause(),
        Some(mclone_protocol::PlayerDamageCause::Lava)
    );
    assert!(updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerLife(life)
            if life.epoch() == 1
                && life.vitals().is_dead()
                && life.death_cause() == Some(mclone_protocol::PlayerDamageCause::Lava)
    )));
    assert!(updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerStatistics { statistics }
            if statistics.death_count() == 1
    )));

    let dead_position = server.player().position();
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(9.5, 90.0, 9.5),
                on_ground: false,
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.player().position(), dead_position);

    let selected_slot = server.inventory().selected_hotbar_slot();
    assert!(
        server
            .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
                slot: selected_slot.saturating_add(1),
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.inventory().selected_hotbar_slot(), selected_slot);

    assert!(
        server
            .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
                pos: lava_pos,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.scheduler().block_at_world(lava_pos), Some(LAVA));
    let player_id = server.player_id();
    assert!(
        !server
            .transfer_player_dimension(
                player_id,
                DimensionKey::parse("mclone:unregistered").unwrap(),
                Vec3d::new(0.5, 80.0, 0.5),
            )
            .unwrap()
    );

    let repeated = server.try_simulation_tick_report().unwrap();
    assert!(!repeated.updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerStatistics { statistics }
            if statistics.death_count() > 1
    )));
    assert_eq!(server.player_statistics().death_count(), 1);
}

#[test]
fn server_tick_kills_a_stationary_player_when_lava_appears() {
    let mut server = LocalRealmSession::new(12_345);
    load_center_chunk(&mut server);
    let position = Vec3d::new(8.5, 80.0, 8.5);
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position,
            on_ground: true,
        }))
        .unwrap();
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::containing(position), LAVA)
    );

    let report = server.try_simulation_tick_report().unwrap();

    assert!(server.player_vitals().is_dead());
    assert!(report.updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerLife(life)
            if life.epoch() == 1
                && life.vitals().is_dead()
                && life.death_cause() == Some(mclone_protocol::PlayerDamageCause::Lava)
    )));
    assert!(report.updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerStatistics { statistics }
            if statistics.death_count() == 1
    )));
}

#[test]
fn lava_death_immediately_enqueues_the_identity_player_record() {
    let identity = ClientIdentity::new(PlayerProfileId::new([0x48; 16]), "Persistent").unwrap();
    let key = player_record_key(identity.profile_id);
    let mut server = LocalRealmSession::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    load_center_chunk(&mut server);
    let position = Vec3d::new(8.5, 80.0, 8.5);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::containing(position), LAVA)
    );

    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position,
            on_ground: false,
        }))
        .unwrap();
    server.scheduler_mut().flush_persistence().unwrap();
    let record = server
        .scheduler_mut()
        .load_player_record_blocking(key)
        .unwrap()
        .expect("death transition must persist an identity record");

    assert_eq!(record.health, 0.0);
    assert_eq!(
        record.pending_death_cause,
        Some(mclone_protocol::PlayerDamageCause::Lava)
    );
    assert_eq!(record.statistics.death_count(), 1);
}

#[test]
fn explicit_respawn_restores_one_safe_life_without_resetting_realm_state() {
    let identity = ClientIdentity::new(PlayerProfileId::new([0x49; 16]), "Returner").unwrap();
    let key = player_record_key(identity.profile_id);
    let mut server = LocalRealmSession::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    load_center_chunk(&mut server);
    server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: 4,
        }))
        .unwrap();
    let player_id = server.player_id();
    let player = server.server.players.get_mut(player_id).unwrap();
    player.total_experience = 23;
    player.statistics.increment(StatisticKey::jump(), 7);

    let death_position = Vec3d::new(8.5, 80.0, 8.5);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::containing(death_position), LAVA)
    );
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: death_position,
            on_ground: false,
        }))
        .unwrap();
    assert!(server.player_vitals().is_dead());

    let mut updates = server.try_handle_command(ClientCommand::Respawn).unwrap();
    assert!(
        server
            .try_handle_command(ClientCommand::Respawn)
            .unwrap()
            .is_empty(),
        "a duplicate in-flight respawn request must be ignored"
    );
    let mut position_update = None;
    for _ in 0..60_000 {
        position_update = position_update.or_else(|| {
            updates.iter().find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(*update),
                _ => None,
            })
        });
        if position_update.is_some() {
            break;
        }
        server.try_tick_report_global().unwrap();
        updates.extend(server.try_drain_updates_for_player(player_id).unwrap());
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    let position_update = position_update.expect("timed out waiting for safe respawn position");
    assert!(position_update.reset_continuity);

    let life_index = updates
        .iter()
        .position(|update| {
            matches!(
                update,
                ServerUpdate::PlayerLife(life)
                    if life.epoch() == 2
                        && !life.vitals().is_dead()
                        && life.death_cause().is_none()
            )
        })
        .expect("respawn must publish a newer living life epoch");
    let position_index = updates
        .iter()
        .position(|update| matches!(update, ServerUpdate::PlayerPosition(_)))
        .unwrap();
    assert!(life_index < position_index);
    assert!(
        updates
            .iter()
            .all(|update| !matches!(update, ServerUpdate::DimensionChange { .. }))
    );
    assert!(server.player_pose_has_clearance(position_update.position));
    assert_ne!(position_update.position, death_position);
    assert_eq!(server.player_vitals().health(), 20.0);
    assert_eq!(server.pending_death_cause(), None);
    assert_eq!(server.inventory().selected_hotbar_slot(), 4);
    assert_eq!(server.total_experience(), 23);
    assert_eq!(server.player_statistics().jump_count(), 7);
    assert_eq!(server.player_statistics().death_count(), 1);
    assert!(
        server
            .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
                slot: 1
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.inventory().selected_hotbar_slot(), 4);

    server.scheduler_mut().flush_persistence().unwrap();
    let record = server
        .scheduler_mut()
        .load_player_record_blocking(key)
        .unwrap()
        .expect("safe respawn publication must immediately persist the living pose");
    assert_eq!(record.position, position_update.position);
    assert_eq!(record.health, 20.0);
    assert_eq!(record.pending_death_cause, None);
    assert_eq!(record.selected_hotbar_slot, 4);
    assert_eq!(record.total_experience, 23);
    assert_eq!(record.statistics.jump_count(), 7);
    assert_eq!(record.statistics.death_count(), 1);

    assert!(
        server
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: position_update.teleport_id.wrapping_add(1),
            }))
            .unwrap()
            .is_empty()
    );
    assert!(!server.player().has_accepted_position());
    server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: position_update.teleport_id,
        }))
        .unwrap();
    assert!(server.player().has_accepted_position());
}

#[test]
fn respawn_waits_dead_until_the_safe_spawn_area_is_published() {
    let mut server = RealmServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_id = server.add_player();
    server
        .kill_player(CommandTarget::Player(player_id), PlayerDamageCause::Lava)
        .unwrap();

    let mut updates = server
        .try_handle_command_for_player(player_id, ClientCommand::Respawn)
        .unwrap();
    assert!(server.player_vitals(player_id).unwrap().is_dead());
    assert!(
        updates
            .iter()
            .all(|update| !matches!(update, ServerUpdate::PlayerPosition(_)))
    );
    assert!(updates.iter().all(|update| !matches!(
        update,
        ServerUpdate::PlayerLife(life) if life.epoch() >= 2 && !life.vitals().is_dead()
    )));

    let mut safe_position = None;
    for _ in 0..60_000 {
        server.try_tick_report_global().unwrap();
        updates.extend(server.try_drain_updates_for_player(player_id).unwrap());
        safe_position = safe_position.or_else(|| {
            updates.iter().find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(*update),
                _ => None,
            })
        });
        if safe_position.is_some() {
            break;
        }
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    let safe_position = safe_position.expect("safe spawn search must eventually complete");
    assert_eq!(server.player_vitals(player_id).unwrap().health(), 20.0);
    assert!(server.player_pose_has_clearance(safe_position.position));
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
    record.dimension = DimensionKey::parse("mclone:moon").unwrap();
    record.selected_hotbar_slot = 6;
    record.total_experience = 41;
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = LocalRealmSession::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let spawned = wait_for_initial_spawn_update(&mut server);

    assert_ne!(spawned.position, record.position);
    assert_eq!(server.inventory().selected_hotbar_slot(), 0);
    assert_eq!(server.total_experience(), 0);
    assert!(server.resume_record().is_none());
}

#[test]
fn blocked_saved_player_pose_falls_back_to_safe_surface_nearby() {
    let seed = 12_345;
    let mut probe = LocalRealmSession::new(seed);
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

    let mut server = LocalRealmSession::with_world_store(seed, Box::new(store));
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
fn unsupported_saved_player_pose_falls_back_to_safe_surface_nearby() {
    let seed = 12_345;
    let mut probe = LocalRealmSession::new(seed);
    request_initial_chunk_view(&mut probe);
    let safe_spawn = wait_for_initial_spawn_update(&mut probe).position;
    let identity = ClientIdentity::new(PlayerProfileId::new([0x25; 16]), "Faller").unwrap();
    let unsupported_position = safe_spawn.add(Vec3d::new(0.0, 2.0, 0.0));
    let record = PlayerRecord::new(
        player_record_key(identity.profile_id),
        4,
        identity.display_name.clone(),
        unsupported_position,
    );
    let mut store = MemoryWorldStore::new();
    store.save_player(&record).unwrap();

    let mut server = LocalRealmSession::with_world_store(seed, Box::new(store));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
    request_initial_chunk_view(&mut server);
    let resumed = wait_for_initial_spawn_update(&mut server);

    assert_ne!(resumed.position, unsupported_position);
    assert!(server.player_pose_has_clearance(resumed.position));
    assert_eq!(resumed.position.x.floor(), unsupported_position.x.floor());
    assert_eq!(resumed.position.z.floor(), unsupported_position.z.floor());
}

#[test]
fn seed_789_initial_spawn_uses_surface_not_underground_cave() {
    let mut server = LocalRealmSession::new(789);
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
    let mut server = LocalRealmSession::new(0);

    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(1.25, 63.0, -4.5),
            y_rot_degrees: 181.0,
            x_rot_degrees: -181.0,
            on_ground: true,
        }))
        .expect("move player");

    assert!(updates.is_empty());
    assert_eq!(server.player().position(), Vec3d::new(1.25, 63.0, -4.5));
    assert_eq!(server.player().y_rot_degrees(), -179.0);
    assert_eq!(server.player().x_rot_degrees(), 179.0);
    assert!(server.player().on_ground());
}

#[test]
fn jump_statistic_increments_once_per_accepted_upward_jump() {
    let mut server = LocalRealmSession::new(0);
    send_player_move(&mut server, Vec3d::new(1.0, 64.0, 1.0));

    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(1.0, 64.42, 1.0),
            on_ground: false,
        }))
        .unwrap();
    assert!(matches!(
        updates.as_slice(),
        [ServerUpdate::PlayerStatistics { statistics }]
            if statistics.jump_count() == 1
                && statistics.successful_block_placement_count() == 0
    ));

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
    assert_eq!(server.player_statistics().jump_count(), 1);
    assert_eq!(server.total_experience(), 0);
}

#[test]
fn rejected_movement_does_not_increment_jump_statistic() {
    let mut server = LocalRealmSession::new(0);
    send_player_move(&mut server, Vec3d::new(0.0, 64.0, 0.0));
    assert!(
        server
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(0.0, f64::NAN, 0.0),
                on_ground: false,
            }))
            .unwrap()
            .is_empty()
    );
    assert!(server.player_statistics().is_empty());
}

#[test]
fn awarded_jump_statistic_is_written_to_the_identity_player_record() {
    let identity = ClientIdentity::new(PlayerProfileId::new([0x55; 16]), "Jumper").unwrap();
    let key = player_record_key(identity.profile_id);
    let mut server = LocalRealmSession::with_world_store(0, Box::new(MemoryWorldStore::new()));
    server
        .configure_local_player_identity_blocking(identity)
        .unwrap();
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

    assert_eq!(record.statistics.jump_count(), 1);
    assert_eq!(record.total_experience, 0);
    assert_eq!(record.position, Vec3d::new(0.0, 64.42, 0.0));
}

#[test]
fn simulation_tick_records_java_shaped_movement_packet_boundary() {
    let mut server = LocalRealmSession::new(0);

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

    assert_eq!(server.player().position(), Vec3d::new(1.0, 64.0, 1.0));
    assert_eq!(server.player().received_move_packet_count(), 2);
    assert_eq!(server.player().known_move_packet_count(), 0);

    server
        .try_simulation_tick_report()
        .expect("simulation tick");

    assert_eq!(server.player().received_move_packet_count(), 2);
    assert_eq!(server.player().known_move_packet_count(), 2);
    assert_eq!(
        server.player().first_good_position(),
        Vec3d::new(1.0, 64.0, 1.0)
    );
    assert_eq!(
        server.player().last_good_position(),
        Vec3d::new(1.0, 64.0, 1.0)
    );
}

#[test]
fn pending_player_position_update_blocks_moves_until_ack_and_resends() {
    let mut server = LocalRealmSession::new(0);

    let simulation_tick = server.simulation_tick;
    let first = server.player_mut().initial_position_update(
        Vec3d::new(0.0, 64.0, 0.0),
        45.0,
        10.0,
        simulation_tick,
    );
    assert_eq!(first.teleport_id, 1);

    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move while awaiting teleport");
    assert!(updates.is_empty());
    assert_eq!(server.player().position(), Vec3d::new(0.0, 64.0, 0.0));
    assert_eq!(
        server
            .player()
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
            .player()
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
            .player()
            .awaiting_teleport()
            .map(|awaiting| awaiting.id),
        Some(2)
    );

    server
        .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
            id: resend.teleport_id,
        }))
        .expect("accept resent teleport");
    assert_eq!(server.player().awaiting_teleport(), None);

    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(32.0, 64.0, 0.0),
            on_ground: true,
        }))
        .expect("move after ack");
    assert!(updates.is_empty());
    assert_eq!(server.player().position(), Vec3d::new(32.0, 64.0, 0.0));
}

#[test]
fn set_carried_item_updates_server_selected_hotbar_slot_without_world_updates() {
    let mut server = LocalRealmSession::new(0);

    let updates = server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: 4,
        }))
        .expect("set carried item");

    assert!(updates.is_empty());
    assert_eq!(server.inventory().selected_hotbar_slot(), 4);

    let updates = server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: mclone_protocol::HOTBAR_SLOT_COUNT,
        }))
        .expect("invalid carried item");

    assert!(updates.is_empty());
    assert_eq!(server.inventory().selected_hotbar_slot(), 4);
}

#[test]
fn rabbit_temptation_signal_uses_only_the_selected_inventory_slot() {
    let mut server = LocalRealmSession::new(0);
    assert_eq!(server.inventory().item_count(ItemKind::Carrot), 8);
    assert!(
        server
            .mob_player_targets()
            .iter()
            .all(|target| !target.tempting_carrot),
        "carrots elsewhere in inventory must not suppress rabbit avoidance"
    );

    server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: 3,
        }))
        .expect("select carrot slot");

    assert!(
        server
            .mob_player_targets()
            .iter()
            .all(|target| target.tempting_carrot),
        "the selected carrot should retain ordinary temptation"
    );
}
