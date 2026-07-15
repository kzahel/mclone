use super::*;

#[test]
fn local_and_dedicated_players_pair_symmetrically() {
    let mut server = IntegratedServer::new(12_345);
    load_center_chunk(&mut server);
    server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position: Vec3d::new(8.5, 80.0, 8.5),
            y_rot_degrees: 15.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))
        .expect("move local player into tracked chunk");

    let dedicated = server.add_dedicated_player();
    let dedicated_updates =
        set_dedicated_chunk_view_and_poll(&mut server, dedicated, ChunkPos::new(0, 0), 0);
    let local_for_dedicated = remote_player_add(&dedicated_updates, ServerPlayerId::LOCAL)
        .expect("dedicated player should observe the local integrated player");
    assert_eq!(local_for_dedicated.id, RemotePlayerId(0));

    let local_updates = server.try_drain_updates().expect("drain local pair add");
    let dedicated_for_local = remote_player_add(&local_updates, dedicated)
        .expect("local integrated player should observe the dedicated peer");
    assert_eq!(dedicated_for_local.id, RemotePlayerId(dedicated.as_u64()));

    let local_moved = Vec3d::new(9.0, 80.0, 8.5);
    server
        .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position: local_moved,
            y_rot_degrees: 30.0,
            x_rot_degrees: 5.0,
            on_ground: true,
        }))
        .expect("move paired local player");
    let dedicated_updates = server
        .try_drain_updates_for_player(dedicated)
        .expect("drain local movement for dedicated observer");
    let local_update = remote_player_update(&dedicated_updates, ServerPlayerId::LOCAL)
        .expect("dedicated observer should receive local movement");
    assert_eq!(local_update.position, local_moved);

    let dedicated_moved = Vec3d::new(9.5, 80.0, 8.5);
    server
        .try_handle_command_for_player(
            dedicated,
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: dedicated_moved,
                y_rot_degrees: -45.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .expect("move paired dedicated player");
    let local_updates = server
        .try_drain_updates()
        .expect("drain dedicated movement");
    let dedicated_update = remote_player_update(&local_updates, dedicated)
        .expect("local observer should receive dedicated movement");
    assert_eq!(dedicated_update.position, dedicated_moved);

    server
        .try_handle_command(ClientCommand::SetPlayerAppearance(
            SetPlayerAppearanceCommand {
                appearance: PlayerAppearance {
                    model: PlayerModelKind::UprightBear,
                },
            },
        ))
        .expect("change local appearance");
    let dedicated_updates = server
        .try_drain_updates_for_player(dedicated)
        .expect("drain local appearance");
    assert_eq!(
        remote_player_update(&dedicated_updates, ServerPlayerId::LOCAL)
            .expect("dedicated observer should receive local appearance")
            .appearance
            .model,
        PlayerModelKind::UprightBear
    );

    server.disable_local_player();
    let dedicated_updates = server
        .try_drain_updates_for_player(dedicated)
        .expect("drain local removal");
    assert!(has_remote_player_remove(
        &dedicated_updates,
        ServerPlayerId::LOCAL
    ));
}

#[test]
fn shared_auxiliary_player_script_uses_authoritative_commands() {
    let mut server = IntegratedServer::new(12_345);
    load_center_chunk(&mut server);
    server.set_debug_auxiliary_player_script_enabled(true);
    let player_id = server
        .debug_auxiliary_player_id()
        .expect("auxiliary script should join one dedicated player");
    let mut add = None;
    let mut positions = Vec::new();
    for _ in 0..240 {
        let updates = server.try_tick().expect("tick auxiliary player script");
        if add.is_none() {
            add = remote_player_add(&updates, player_id);
        }
        positions.extend(updates.iter().filter_map(|update| match update {
            ServerUpdate::RemotePlayerUpdate(update)
                if update.id == RemotePlayerId(player_id.as_u64()) =>
            {
                Some(update.position)
            }
            _ => None,
        }));
        if add.is_some() && positions.windows(2).any(|pair| pair.first() != pair.get(1)) {
            break;
        }
    }
    let add = add.expect("local source client should receive auxiliary add");
    assert_eq!(add.appearance.model, PlayerModelKind::UprightBear);
    assert!(positions.windows(2).any(|pair| pair[0] != pair[1]));

    server.set_debug_auxiliary_player_script_enabled(false);
    let updates = server.try_drain_updates().expect("drain auxiliary removal");
    assert!(has_remote_player_remove(&updates, player_id));
    assert_eq!(server.dedicated_player_count(), 0);
}

#[test]
fn dedicated_players_publish_remote_state_when_visible() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(8.5, 80.0, 8.5),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .expect("move player a into tracked chunk");
    let updates_b =
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);

    let add_a =
        remote_player_add(&updates_b, player_a).expect("player b should receive player a add");
    assert_eq!(add_a.id, RemotePlayerId(player_a.as_u64()));
    assert_eq!(add_a.appearance, PlayerAppearance::default());

    let updates_a = server.try_poll_for_player(player_a).expect("poll player a");
    let add_b = remote_player_add(&updates_a, player_b)
        .expect("player a should receive player b add after b accepts spawn");
    assert_eq!(add_b.id, RemotePlayerId(player_b.as_u64()));

    let moved = Vec3d::new(9.5, 80.0, 8.5);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: moved,
                y_rot_degrees: 90.0,
                x_rot_degrees: -15.0,
                on_ground: true,
            }),
        )
        .expect("move player a");

    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");
    let moved_a = remote_player_update(&updates_b, player_a)
        .expect("player b should receive player a movement update");
    assert_eq!(moved_a.position, moved);
    assert_eq!(moved_a.y_rot_degrees, 90.0);
    assert_eq!(moved_a.x_rot_degrees, -15.0);
    assert!(moved_a.on_ground);

    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::MovePlayer(MovePlayerCommand::Rot {
                y_rot_degrees: 135.0,
                x_rot_degrees: 20.0,
                on_ground: true,
            }),
        )
        .expect("rotate stationary player a");

    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");
    let rotated_a = remote_player_update(&updates_b, player_a)
        .expect("player b should receive player a rotation-only update");
    assert_eq!(rotated_a.position, moved);
    assert_eq!(rotated_a.y_rot_degrees, 135.0);
    assert_eq!(rotated_a.x_rot_degrees, 20.0);
    assert!(rotated_a.on_ground);

    let bear_appearance = PlayerAppearance {
        model: PlayerModelKind::UprightBear,
    };
    let updates_a = server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::SetPlayerAppearance(SetPlayerAppearanceCommand {
                appearance: bear_appearance,
            }),
        )
        .expect("set player a appearance");
    assert!(updates_a.is_empty());

    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");
    let appearance_a = remote_player_update(&updates_b, player_a)
        .expect("player b should receive player a appearance update");
    assert_eq!(appearance_a.position, moved);
    assert_eq!(appearance_a.appearance, bear_appearance);
}

#[test]
fn dedicated_remote_players_are_removed_when_the_observer_view_stops_tracking_them() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(8.5, 80.0, 8.5),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .expect("move player a into tracked chunk");
    let updates_b =
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);
    assert!(remote_player_add(&updates_b, player_a).is_some());

    let updates_b =
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(4, 0), 0);

    assert!(has_remote_player_remove(&updates_b, player_a));
}

#[test]
fn dedicated_remote_players_are_removed_on_disconnect() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(8.5, 80.0, 8.5),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .expect("move player a into tracked chunk");
    let updates_b =
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);
    assert!(remote_player_add(&updates_b, player_a).is_some());

    assert!(server.remove_dedicated_player(player_a));
    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

    assert!(has_remote_player_remove(&updates_b, player_a));
}

#[test]
fn dedicated_player_receives_debug_passive_showcase_when_visible() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_dedicated_player();

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);

    let snapshot = first_entity_snapshot(&updates)
        .expect("dedicated player should receive starter passive entity snapshot");
    assert_eq!(snapshot.kind, EntityKind::Cow);
    assert_eq!(snapshot.width, 0.9);
    assert_eq!(snapshot.height, 1.4);
    let chunk = BlockPos::containing(snapshot.position).chunk_pos();
    assert!(chunk.x.abs() <= 2);
    assert!(chunk.z.abs() <= 2);
}

#[test]
fn debug_passive_showcase_can_be_disabled() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    let player = server.add_dedicated_player();

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);

    assert!(first_entity_snapshot(&updates).is_none());
}

#[test]
fn debug_passive_showcase_entity_updates_age_on_simulation_tick() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_dedicated_player();
    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);
    let snapshot = first_entity_snapshot(&updates).expect("entity snapshot");

    let report = server
        .try_simulation_tick_report_for_player(player)
        .expect("tick dedicated player");
    let update = first_entity_update(&report.updates, snapshot.id).expect("entity age update");

    assert_eq!(update.id, snapshot.id);
    assert!(update.position.is_finite());
    assert!(update.age_ticks > snapshot.age_ticks);
}

#[test]
fn natural_spawning_diagnostics_use_live_players_chunks_and_creature_counts() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);

    let report = server
        .try_simulation_tick_report_for_player(player)
        .expect("tick dedicated player");
    let spawning = report.natural_spawning;

    assert!(spawning.live_attempts_enabled);
    assert!(spawning.live_spawns_are_volatile);
    assert!(!spawning.ready_for_live_attempts);
    assert_eq!(spawning.blocker_count, 1);
    assert_eq!(spawning.player_distance_spawnable_chunks, 17 * 17);
    assert!(spawning.eligible_entity_ticking_spawn_chunks > 0);
    assert_eq!(spawning.creature_count, 2);
    assert_eq!(spawning.creature_cap, 10);
    assert!(spawning.creature_cap_has_room);
    assert!(!spawning.creature_cadence_ready);
    assert!(!spawning.creature_should_attempt_if_enabled);
    assert_eq!(spawning.live_attempts, 0);
    assert_eq!(spawning.live_spawned, 0);
    assert!(spawning.dry_run_chunks_checked > 0);
    assert!(spawning.dry_run_chunks_checked <= 8);
    assert_eq!(
        spawning.dry_run_biome_supported_positions + spawning.dry_run_blocked_by_biome,
        spawning.dry_run_chunks_checked * 4
    );
    assert!(spawning.dry_run_positions_checked <= spawning.dry_run_biome_supported_positions);
    assert_eq!(spawning.dry_run_valid_candidates, 0);
}

#[test]
fn volatile_natural_spawning_creates_entities_from_ticket_loaded_chunks() {
    let seed = 12_345;
    let mut server = IntegratedServer::new(seed);
    server.set_debug_passive_showcase_enabled(false);
    server.set_lighting_enabled(true);
    let player = server.add_dedicated_player();
    let center = crate::spawn::initial_spawn_center_for_seed(seed);
    set_dedicated_chunk_view_and_poll(&mut server, player, center, 4);

    for _ in 0..400 {
        let report = server
            .try_simulation_tick_report_for_player(player)
            .expect("tick dedicated player");
        let spawning = report.natural_spawning;
        if !spawning.creature_cadence_ready {
            continue;
        }

        assert!(spawning.live_attempts_enabled);
        assert!(spawning.live_spawns_are_volatile);
        assert!(spawning.ready_for_live_attempts);
        assert_eq!(spawning.blocker_count, 0);
        assert_eq!(spawning.creature_count, 0);
        assert_eq!(spawning.creature_cap, 10);
        assert!(spawning.creature_cap_has_room);
        assert!(spawning.creature_should_attempt_if_enabled);
        assert!(spawning.live_attempts > 0);
        assert!(spawning.live_spawned > 0);

        let cow = first_entity_snapshot_of_kind(&report.updates, EntityKind::Cow);
        let chicken = first_entity_snapshot_of_kind(&report.updates, EntityKind::Chicken);
        let snapshot = cow
            .or(chicken)
            .expect("spawned passive mob should be visible to tracking player");
        assert!(matches!(
            snapshot.kind,
            EntityKind::Cow | EntityKind::Chicken
        ));

        let next_report = server
            .try_simulation_tick_report_for_player(player)
            .expect("tick after volatile spawn");
        assert!(
            next_report.natural_spawning.creature_count >= spawning.live_spawned as u32,
            "spawned passive mobs should count toward the next creature cap"
        );
        return;
    }

    panic!("volatile natural spawning did not reach a creature cadence tick");
}

#[test]
fn volatile_entities_are_discarded_when_ticket_loaded_chunk_fully_unloads() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    server.set_volatile_natural_spawning_enabled(false);
    let player = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 0);
    let entity = server.entities.spawn_volatile_passive_mob(
        EntityKind::Cow,
        Vec3d::new(8.5, 64.0, 8.5),
        0.0,
    );
    server.reconcile_entity_subjects([entity], true);
    let updates = server
        .drain_chunk_updates_for_target(CommandTarget::Dedicated(player))
        .expect("drain volatile spawn update");
    assert_eq!(
        first_entity_snapshot_of_kind(&updates, EntityKind::Cow).map(|snapshot| snapshot.id),
        Some(entity.id)
    );

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(64, 0), 0);

    assert!(has_entity_remove(&updates, entity.id));
    assert!(
        server.entities.state(entity.id).is_some(),
        "leaving the visible range should remove the observer pair, not discard the entity"
    );

    for _ in 0..128 {
        let report = server
            .try_simulation_tick_report_for_player(player)
            .expect("tick dedicated player while pending unloads drain");
        if server.entities.state(entity.id).is_none() {
            assert!(report.pending_unloads_processed > 0);
            return;
        }
    }

    panic!("volatile entity was not discarded after its chunk fully unloaded");
}

#[test]
fn volatile_natural_spawning_can_be_disabled() {
    let mut server = IntegratedServer::new(12_345);
    server.set_volatile_natural_spawning_enabled(false);

    let spawning = server.natural_spawning_diagnostics(400, &[]);

    assert!(!spawning.live_attempts_enabled);
    assert!(!spawning.live_spawns_are_volatile);
    assert!(!spawning.ready_for_live_attempts);
    assert_eq!(spawning.blocker_count, 1);
    assert_eq!(spawning.live_attempts, 0);
    assert_eq!(spawning.live_spawned, 0);
}

#[test]
fn debug_passive_showcase_entity_is_removed_when_observer_view_stops_tracking_it() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_dedicated_player();
    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);
    let snapshot = first_entity_snapshot(&updates).expect("entity snapshot");

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(8, 0), 0);

    assert!(has_entity_remove(&updates, snapshot.id));
}

#[test]
fn local_player_picks_up_ready_item_entity_through_server_inventory() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    load_center_chunk(&mut server);
    let player_position = server.player.position();
    let item_id = server.entities.insert_item_entity_for_test(
        ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 1,
        },
        player_position,
    );
    server.entities.set_item_pickup_delay_for_test(item_id, 0);
    server.reconcile_entity_subjects(
        std::iter::once(server.entities.state(item_id).unwrap()),
        true,
    );
    let initial_updates = server
        .drain_chunk_updates_for_target(CommandTarget::Local)
        .expect("drain initial item snapshot");
    assert!(initial_updates.iter().any(|update| {
        matches!(update, ServerUpdate::EntitySnapshot(snapshot) if snapshot.id == item_id)
    }));

    let report = server
        .try_simulation_tick_report()
        .expect("simulation tick should pick up item");

    assert!(has_entity_remove(&report.updates, item_id));
    assert_eq!(server.inventory.item_count(ItemKind::Egg), 1);
    assert_eq!(server.entities.state(item_id), None);
}
