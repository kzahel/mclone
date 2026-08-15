use super::*;
use crate::MemoryWorldStore;

#[test]
fn hunting_spear_damage_uses_fall_then_persistent_species_drops() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    server.set_natural_spawning_enabled(false);
    load_center_chunk(&mut server);
    let player_id = server.player_id();
    let player_position = server.player().position();
    let deer_position = player_position.add(Vec3d::new(0.0, 0.0, 3.0));
    let deer = server
        .entities
        .spawn_persistent_passive_mob(EntityKind::Deer, deer_position, 180.0);
    server.mark_entity_updates_dirty(&[deer]);
    server.reconcile_entity_subjects([deer], true);
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Rot {
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))
        .unwrap();

    for attack in 0_u64..3 {
        server.simulation_tick = attack * DEER_HUNTING_SPEAR_COOLDOWN_TICKS;
        server
            .try_handle_command(ClientCommand::AttackEntity(AttackEntityCommand {
                target: deer.id,
            }))
            .unwrap();
    }
    let fallen = server.entities.state(deer.id).unwrap();
    assert_eq!(fallen.deer.unwrap().health, 0);
    assert_eq!(
        fallen.deer.unwrap().behavior,
        mclone_protocol::DeerBehavior::Fall
    );
    assert_eq!(fallen.animation.unwrap().clip.as_str(), "fall");

    for _ in 0..30 {
        server.try_simulation_tick_report().unwrap();
    }
    assert!(server.entities.state(deer.id).is_none());
    let states = server.entities.states();
    assert!(states.iter().any(|entity| {
        entity.item_stack
            == Some(ItemStackSnapshot {
                kind: ItemKind::Venison,
                count: 3,
            })
    }));
    assert!(states.iter().any(|entity| {
        entity.item_stack
            == Some(ItemStackSnapshot {
                kind: ItemKind::DeerHide,
                count: 1,
            })
    }));
    assert!(
        server
            .active_dimension
            .deer_population
            .blocks_spawn(deer.chunk_pos())
    );
    assert!(
        server
            .players
            .get(player_id)
            .unwrap()
            .deer_field_guide
            .contains(mclone_protocol::DeerObservationKind::Harvested)
    );
}

#[test]
fn ordinary_attack_disturbs_then_collapses_a_burrow_without_deleting_residents() {
    let mut server = LocalRealmSession::new(54_321);
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    server.set_natural_spawning_enabled(false);
    load_center_chunk(&mut server);
    let player_position = Vec3d::new(8.5, 80.0, 8.5);
    sync_player(&mut server, player_position);
    let mouth_position = player_position.add(Vec3d::new(0.0, 0.0, 3.0));
    let rabbit_id =
        server
            .entities
            .insert_passive_mob_for_test(EntityKind::Rabbit, mouth_position, 180.0);
    let rabbit_persistent_id = server.entities.state(rabbit_id).unwrap().persistent_id;
    let mouth_block = BlockPos::containing(mouth_position);
    server.scheduler_mut().set_block_at_world(mouth_block, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();
    let burrow = server
        .entities
        .complete_rabbit_dig(rabbit_id, mouth_block)
        .into_iter()
        .find(|entity| entity.kind == EntityKind::RabbitBurrow)
        .unwrap();
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Rot {
            y_rot_degrees: 0.0,
            x_rot_degrees: 24.0,
            on_ground: true,
        }))
        .unwrap();
    for _ in 0..2 {
        server
            .try_handle_command(ClientCommand::AttackEntity(AttackEntityCommand {
                target: burrow.id,
            }))
            .expect("ordinary attack should disturb a visible burrow");
        assert!(server.entities.state(burrow.id).is_some());
        assert_eq!(
            server
                .entities
                .mob_state(rabbit_id)
                .unwrap()
                .rabbit_familiar_refuge()
                .map(|known| known.locator.persistent_id),
            Some(burrow.persistent_id)
        );
    }
    let collapse_updates = server
        .try_handle_command(ClientCommand::AttackEntity(AttackEntityCommand {
            target: burrow.id,
        }))
        .expect("third prompt attack should collapse the burrow");
    assert!(
        collapse_updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::EntityRemove { id } if *id == burrow.id))
    );

    assert!(server.entities.state(burrow.id).is_none());
    let rabbit = server
        .entities
        .state(rabbit_id)
        .expect("collapse must preserve the resident rabbit");
    assert_eq!(rabbit.persistent_id, rabbit_persistent_id);
    assert!(rabbit.alive);
    assert!(!rabbit.hidden_from_clients);
    assert_eq!(
        server
            .entities
            .mob_state(rabbit_id)
            .unwrap()
            .rabbit_familiar_refuge(),
        None
    );
}

#[test]
fn homestead_residents_realize_by_entity_chunk_once_with_stable_ids() {
    let definition =
        crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
    server.initialize_world_metadata_blocking().unwrap();
    server.set_debug_passive_showcase_enabled(true);
    server.set_natural_spawning_enabled(false);
    let plan = server.intro_homestead_plan().unwrap();
    let cow_chunk = BlockPos::new(
        plan.resident_markers[0].pos[0],
        plan.resident_markers[0].pos[1],
        plan.resident_markers[0].pos[2],
    )
    .chunk_pos();
    let chicken_chunk = BlockPos::new(
        plan.resident_markers[1].pos[0],
        plan.resident_markers[1].pos[1],
        plan.resident_markers[1].pos[2],
    )
    .chunk_pos();

    load_chunk_view(&mut server, cow_chunk);
    let cows = server
        .entities
        .states()
        .into_iter()
        .filter(|entity| entity.kind == EntityKind::Cow)
        .collect::<Vec<_>>();
    assert_eq!(cows.len(), 2);
    assert!(
        server
            .entities
            .states()
            .iter()
            .all(|entity| entity.kind != EntityKind::Chicken),
        "loading only the cow marker chunk must not realize chickens"
    );
    assert_eq!(
        cows.iter()
            .map(|entity| entity.persistent_id)
            .collect::<Vec<_>>(),
        vec![
            EntityPersistentId::new(0x434f_5701_0000_0000, 1),
            EntityPersistentId::new(0x434f_5701_0000_0000, 2),
        ]
    );

    load_chunk_view(&mut server, chicken_chunk);
    load_chunk_view(&mut server, cow_chunk);
    let states = server.entities.states();
    assert_eq!(
        states
            .iter()
            .map(|entity| (entity.kind, entity.position))
            .collect::<Vec<_>>(),
        vec![
            (EntityKind::Cow, Vec3d::new(-174.5, 90.0, -1412.5)),
            (EntityKind::Cow, Vec3d::new(-174.5, 90.0, -1411.5)),
            (EntityKind::Chicken, Vec3d::new(-177.5, 90.0, -1402.5)),
            (EntityKind::Chicken, Vec3d::new(-176.5, 90.0, -1402.5)),
            (EntityKind::Chicken, Vec3d::new(-176.5, 90.0, -1401.5)),
        ],
        "the accepted yard composition should resolve to readable safe positions"
    );
    assert_eq!(
        states
            .iter()
            .filter(|entity| entity.kind == EntityKind::Cow)
            .count(),
        2
    );
    assert_eq!(
        states
            .iter()
            .filter(|entity| entity.kind == EntityKind::Chicken)
            .count(),
        3
    );
    assert_eq!(states.len(), 5, "the debug showcase must stay suppressed");
    assert_eq!(
        states
            .iter()
            .filter(|entity| entity.kind == EntityKind::Chicken)
            .map(|entity| entity.persistent_id)
            .collect::<Vec<_>>(),
        vec![
            EntityPersistentId::new(0x4348_4943_4b01_0000, 1),
            EntityPersistentId::new(0x4348_4943_4b01_0000, 2),
            EntityPersistentId::new(0x4348_4943_4b01_0000, 3),
        ]
    );

    let first_runtime_ids = states.iter().map(|entity| entity.id).collect::<Vec<_>>();
    load_chunk_view(&mut server, ChunkPos::new(cow_chunk.x + 64, cow_chunk.z));
    for _ in 0..256 {
        server.try_simulation_tick_report().unwrap();
        if server.entities.states().is_empty() {
            break;
        }
    }
    assert!(
        server.entities.states().is_empty(),
        "ticket-driven unload must retire the resident runtime instances"
    );

    load_chunk_view(&mut server, cow_chunk);
    load_chunk_view(&mut server, chicken_chunk);
    let reloaded = server.entities.states();
    assert_eq!(reloaded.len(), 5);
    assert!(
        reloaded
            .iter()
            .all(|entity| !first_runtime_ids.contains(&entity.id))
    );
    assert!(
        cow_ids_for_states(&reloaded).iter().eq([
            EntityPersistentId::new(0x434f_5701_0000_0000, 1),
            EntityPersistentId::new(0x434f_5701_0000_0000, 2),
        ]
        .iter())
    );
}

fn cow_ids_for_states(states: &[ServerEntityState]) -> Vec<EntityPersistentId> {
    states
        .iter()
        .filter(|entity| entity.kind == EntityKind::Cow)
        .map(|entity| entity.persistent_id)
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn homestead_residents_reopen_once_and_removed_residents_stay_removed() {
    let root = std::env::temp_dir().join(format!(
        "mclone-homestead-residents-{}-{}",
        std::process::id(),
        current_unix_millis()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let definition =
        crate::DimensionDefinition::overworld(0, WorldGenerationProfile::McloneOverworldV1);
    let cow_ids = [
        EntityPersistentId::new(0x434f_5701_0000_0000, 1),
        EntityPersistentId::new(0x434f_5701_0000_0000, 2),
    ];
    let chicken_ids = [
        EntityPersistentId::new(0x4348_4943_4b01_0000, 1),
        EntityPersistentId::new(0x4348_4943_4b01_0000, 2),
        EntityPersistentId::new(0x4348_4943_4b01_0000, 3),
    ];
    let cow_chunk;
    let chicken_chunk;
    let edit_pos;

    {
        let mut realm = RealmServer::try_with_threaded_sqlite_world_dir_dimension_definition(
            definition.clone(),
            &root,
        )
        .unwrap();
        realm.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
        realm.initialize_world_metadata_blocking().unwrap();
        let plan = realm.intro_homestead_plan().unwrap();
        cow_chunk = BlockPos::new(
            plan.resident_markers[0].pos[0],
            plan.resident_markers[0].pos[1],
            plan.resident_markers[0].pos[2],
        )
        .chunk_pos();
        chicken_chunk = BlockPos::new(
            plan.resident_markers[1].pos[0],
            plan.resident_markers[1].pos[1],
            plan.resident_markers[1].pos[2],
        )
        .chunk_pos();
        edit_pos = BlockPos::new(
            cow_chunk.min_block_x() + 1,
            110,
            cow_chunk.min_block_z() + 1,
        );
        let mut server = LocalRealmSession::from_server(realm);
        server.set_debug_passive_showcase_enabled(false);
        server.set_natural_spawning_enabled(false);
        load_chunk_view(&mut server, cow_chunk);
        load_chunk_view(&mut server, chicken_chunk);
        assert!(server.scheduler_mut().set_block_at_world(edit_pos, BRICKS));
        assert_eq!(server.entities.states().len(), 5);
        server.shutdown_persistence().unwrap();
    }

    {
        let mut realm = RealmServer::try_with_threaded_sqlite_world_dir_dimension_definition(
            definition.clone(),
            &root,
        )
        .unwrap();
        realm.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
        realm.initialize_world_metadata_blocking().unwrap();
        let mut reopened = LocalRealmSession::from_server(realm);
        reopened.set_debug_passive_showcase_enabled(false);
        reopened.set_natural_spawning_enabled(false);
        load_chunk_view(&mut reopened, cow_chunk);
        load_chunk_view(&mut reopened, chicken_chunk);
        let states = reopened.entities.states();
        assert_eq!(states.len(), 5);
        assert!(cow_ids.iter().all(|persistent_id| {
            states
                .iter()
                .any(|entity| entity.persistent_id == *persistent_id)
        }));
        assert!(chicken_ids.iter().all(|persistent_id| {
            states
                .iter()
                .any(|entity| entity.persistent_id == *persistent_id)
        }));
        assert_eq!(reopened.scheduler().block_at_world(edit_pos), Some(BRICKS));

        let before = reopened.entities.persistent_entity_chunk_positions();
        for persistent_id in chicken_ids {
            reopened
                .entities
                .remove_persistent_entity_for_test(persistent_id)
                .expect("persisted chicken must be removable");
        }
        let after = reopened.entities.persistent_entity_chunk_positions();
        reopened.mark_entity_chunk_index_changes(before, after);
        reopened.shutdown_persistence().unwrap();
    }

    {
        let mut realm =
            RealmServer::try_with_threaded_sqlite_world_dir_dimension_definition(definition, &root)
                .unwrap();
        realm.set_starter_content(StarterContentDescriptor::IntroHomesteadV1);
        realm.initialize_world_metadata_blocking().unwrap();
        let mut reopened = LocalRealmSession::from_server(realm);
        reopened.set_debug_passive_showcase_enabled(false);
        reopened.set_natural_spawning_enabled(false);
        load_chunk_view(&mut reopened, chicken_chunk);
        assert!(
            reopened
                .entities
                .states()
                .iter()
                .all(|entity| entity.kind != EntityKind::Chicken)
        );
        load_chunk_view(&mut reopened, cow_chunk);
        assert_eq!(
            reopened
                .entities
                .states()
                .iter()
                .filter(|entity| entity.kind == EntityKind::Cow)
                .count(),
            2
        );
        assert_eq!(reopened.scheduler().block_at_world(edit_pos), Some(BRICKS));
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_and_dedicated_players_pair_symmetrically() {
    let mut server = LocalRealmSession::new(12_345);
    load_center_chunk(&mut server);
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(8.5, 80.0, 8.5),
            y_rot_degrees: 15.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))
        .expect("move local player into tracked chunk");

    let dedicated = server.add_player();
    let dedicated_updates =
        set_dedicated_chunk_view_and_poll(&mut server, dedicated, ChunkPos::new(0, 0), 0);
    let local_player = server.player_id();
    let local_for_dedicated = remote_player_add(&dedicated_updates, local_player)
        .expect("dedicated player should observe the local integrated player");
    assert_eq!(local_for_dedicated.id, RemotePlayerId(0));

    let local_updates = server.try_drain_updates().expect("drain local pair add");
    let dedicated_for_local = remote_player_add(&local_updates, dedicated)
        .expect("local integrated player should observe the dedicated peer");
    assert_eq!(dedicated_for_local.id, RemotePlayerId(dedicated.as_u64()));

    let local_moved = Vec3d::new(9.0, 80.0, 8.5);
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
            position: local_moved,
            y_rot_degrees: 30.0,
            x_rot_degrees: 5.0,
            on_ground: true,
        }))
        .expect("move paired local player");
    let dedicated_updates = server
        .try_drain_updates_for_player(dedicated)
        .expect("drain local movement for dedicated observer");
    let local_update = remote_player_update(&dedicated_updates, local_player)
        .expect("dedicated observer should receive local movement");
    assert_eq!(local_update.position, local_moved);

    let dedicated_moved = Vec3d::new(9.5, 80.0, 8.5);
    server
        .try_handle_command_for_player(
            dedicated,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
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
        remote_player_update(&dedicated_updates, local_player)
            .expect("dedicated observer should receive local appearance")
            .appearance
            .model,
        PlayerModelKind::UprightBear
    );

    assert!(server.remove_player(local_player));
    let dedicated_updates = server
        .try_drain_updates_for_player(dedicated)
        .expect("drain local removal");
    assert!(has_remote_player_remove(&dedicated_updates, local_player));
}

#[test]
fn lava_death_removes_the_remote_body_without_disconnecting_interest() {
    let mut server = LocalRealmSession::new(12_345);
    load_center_chunk(&mut server);
    let local_player = server.player_id();
    let position = Vec3d::new(8.5, 80.0, 8.5);
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::Pos {
            position,
            on_ground: true,
        }))
        .unwrap();

    let observer = server.add_player();
    let observer_updates =
        set_dedicated_chunk_view_and_poll(&mut server, observer, ChunkPos::new(0, 0), 0);
    assert!(remote_player_add(&observer_updates, local_player).is_some());
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::containing(position), LAVA)
    );

    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::StatusOnly {
            on_ground: false,
        }))
        .unwrap();
    let observer_updates = server.try_drain_updates_for_player(observer).unwrap();

    assert!(has_remote_player_remove(&observer_updates, local_player));
    assert_eq!(server.player_count(), 2);
    assert!(
        RealmServer::player_vitals(&server, local_player)
            .unwrap()
            .is_dead()
    );
}

#[test]
fn shared_auxiliary_player_script_uses_authoritative_commands() {
    let mut server = LocalRealmSession::new(12_345);
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
            ServerUpdate::EphemeralFallback(
                mclone_protocol::ServerEphemeralMessage::RemoteBodyPose(sample),
            ) if sample.id == RemotePlayerId(player_id.as_u64()) => Some(sample.pose.position),
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
    assert_eq!(server.player_count(), 1);
}

#[test]
fn dedicated_players_publish_remote_state_when_visible() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_player();
    let player_b = server.add_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
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
            ClientCommand::move_player(MovePlayerCommand::PosRot {
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
            ClientCommand::move_player(MovePlayerCommand::Rot {
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
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_player();
    let player_b = server.add_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
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
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_player();
    let player_b = server.add_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
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

    assert!(server.remove_player(player_a));
    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

    assert!(has_remote_player_remove(&updates_b, player_a));
}

#[test]
fn dedicated_player_receives_debug_passive_showcase_when_visible() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_player();

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
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    let player = server.add_player();

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);

    assert!(first_entity_snapshot(&updates).is_none());
}

#[test]
fn debug_passive_showcase_entity_updates_tick_count_on_simulation_tick() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_player();
    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);
    let snapshot = first_entity_snapshot(&updates).expect("entity snapshot");

    let report = server
        .try_simulation_tick_report_for_player(player)
        .expect("tick dedicated player");
    let update =
        first_entity_update(&report.updates, snapshot.id).expect("entity tick-count update");

    assert_eq!(update.id, snapshot.id);
    assert!(update.position.is_finite());
    assert!(update.tick_count > snapshot.tick_count);
}

#[test]
fn natural_spawning_diagnostics_use_live_players_chunks_and_creature_counts() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_player();
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
        spawning.dry_run_biome_supported_positions
            + spawning.dry_run_blocked_by_biome
            + spawning.dry_run_blocked_missing_biome_data,
        spawning.dry_run_positions_checked
    );
    assert_eq!(spawning.dry_run_valid_candidates, 0);
}

#[test]
fn transient_natural_spawning_creates_volatile_entities_from_generated_habitats() {
    let seed = 12_345;
    let mut server = LocalRealmSession::new(seed);
    server.set_debug_passive_showcase_enabled(false);
    server.set_lighting_enabled(true);
    let player = server.add_player();
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

    panic!("transient natural spawning did not reach a creature cadence tick");
}

#[test]
fn persistent_initial_wildlife_survives_reload_without_seed_resurrection() {
    let seed = 12_345;
    let definition =
        crate::DimensionDefinition::overworld(seed, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server
        .initialize_world_metadata_blocking()
        .expect("initialize persistent wild world");
    server.set_debug_passive_showcase_enabled(false);
    server.set_lighting_enabled(true);
    let player = server.add_player();
    let center = crate::spawn::initial_spawn_center_for_seed(seed);
    set_dedicated_chunk_view_and_poll(&mut server, player, center, 4);

    let spawned = server.entities.states();
    assert!(
        !spawned.is_empty(),
        "first realization should create wildlife"
    );
    assert!(spawned.iter().all(|entity| matches!(
        entity.kind,
        EntityKind::Rabbit
            | EntityKind::Deer
            | EntityKind::Mallard
            | EntityKind::Bee
            | EntityKind::BeeNest
    )));
    let spawning = server.natural_spawning_diagnostics(server.simulation_tick(), &[]);
    assert!(!spawning.live_attempts_enabled);
    assert_eq!(spawning.live_attempts, 0);
    let runtime_ids = spawned
        .iter()
        .map(|entity| entity.id)
        .collect::<BTreeSet<_>>();
    let target_chunk = spawned[0].chunk_pos();
    let removed_ids = spawned
        .iter()
        .filter(|entity| entity.chunk_pos() == target_chunk)
        .map(|entity| entity.persistent_id)
        .collect::<BTreeSet<_>>();
    let before = server.entities.persistent_entity_chunk_positions();
    for persistent_id in &removed_ids {
        server
            .entities
            .remove_persistent_entity_for_test(*persistent_id)
            .expect("initial wildlife must be removable");
    }
    let after = server.entities.persistent_entity_chunk_positions();
    server.mark_entity_chunk_index_changes(before, after);
    let expected_ids = spawned
        .iter()
        .map(|entity| entity.persistent_id)
        .filter(|persistent_id| !removed_ids.contains(persistent_id))
        .collect::<BTreeSet<_>>();

    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(64, 0), 0);
    for _ in 0..256 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("drain persistent entity unload");
        if server.entities.states().is_empty() {
            break;
        }
    }
    assert!(
        server.entities.states().is_empty(),
        "persistent initial wildlife should leave runtime state on full chunk unload"
    );

    set_dedicated_chunk_view_and_poll(&mut server, player, center, 4);
    for _ in 0..256 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("hydrate persistent initial wildlife");
        let reloaded = server.entities.states();
        let reloaded_persistent_ids = reloaded
            .iter()
            .map(|entity| entity.persistent_id)
            .collect::<BTreeSet<_>>();
        if reloaded_persistent_ids == expected_ids {
            assert!(
                reloaded
                    .iter()
                    .all(|entity| !runtime_ids.contains(&entity.id)),
                "runtime ids should be fresh after chunk hydration"
            );
            assert!(
                removed_ids
                    .iter()
                    .all(|persistent_id| !reloaded_persistent_ids.contains(persistent_id))
            );
            return;
        }
    }

    panic!("initial wildlife did not hydrate with stable identities");
}

#[test]
fn transient_initial_wildlife_realizes_each_chunk_once_per_session() {
    let seed = 12_345;
    let definition =
        crate::DimensionDefinition::overworld(seed, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_dimension_definition(definition);
    server.set_debug_passive_showcase_enabled(false);
    server.set_lighting_enabled(false);
    let player = server.add_player();
    let center = crate::spawn::initial_spawn_center_for_seed(seed);
    set_dedicated_chunk_view_and_poll(&mut server, player, center, 4);
    assert!(
        !server.entities.states().is_empty(),
        "first transient realization should create wildlife"
    );

    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(64, 0), 0);
    for _ in 0..256 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("discard transient wildlife on unload");
        if server.entities.states().is_empty() {
            break;
        }
    }
    assert!(server.entities.states().is_empty());

    set_dedicated_chunk_view_and_poll(&mut server, player, center, 4);
    for _ in 0..32 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("revisit transient wildlife chunks");
    }
    assert!(
        server.entities.states().is_empty(),
        "the same transient session must not repeatedly seed-populate revisited chunks"
    );
}

#[test]
fn generated_wetland_mallard_flock_and_due_egg_survive_reload() {
    let seed = 12_345;
    let wetland = ChunkPos::new(-142, -51);
    let definition =
        crate::DimensionDefinition::overworld(seed, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );
    server
        .initialize_world_metadata_blocking()
        .expect("initialize persistent Mclone world");
    server.set_debug_passive_showcase_enabled(false);
    server.set_natural_spawning_enabled(false);
    server.set_lighting_enabled(true);
    let player = server.add_player();
    set_dedicated_chunk_view_and_poll(&mut server, player, wetland, 4);

    // These positions are a pinned suitable wetland pair for this generated
    // seed. The test owns mallard/egg durability, not the retired live-spawn
    // search policy.
    let positions = [
        Vec3d::new(-2257.5, 65.0, -815.5),
        Vec3d::new(-2256.5, 65.0, -817.5),
    ];
    let spawned = positions
        .into_iter()
        .map(|position| {
            server
                .entities
                .spawn_persistent_passive_mob(EntityKind::Mallard, position, 180.0)
        })
        .collect::<Vec<_>>();
    server.mark_entity_updates_dirty(&spawned);
    server.reconcile_entity_subjects(spawned.iter().copied(), true);
    server
        .entities
        .set_mallard_egg_time_for_test(spawned[0].id, 1);
    server
        .entities
        .set_mallard_sex_for_test(spawned[0].id, mclone_protocol::MallardSex::Female);

    let mut egg = None;
    for _ in 0..400 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("tick due mallard in wetland habitat");
        egg = server.entities.states().into_iter().find(|entity| {
            entity.item_stack
                == Some(ItemStackSnapshot {
                    kind: ItemKind::MallardEgg,
                    count: 1,
                })
        });
        if egg.is_some() {
            break;
        }
    }
    let egg = egg.expect("due wetland female should lay a distinct persistent egg");
    let resident = server.entities.states();
    let persistent_ids = resident
        .iter()
        .map(|entity| entity.persistent_id)
        .collect::<BTreeSet<_>>();
    let runtime_ids = resident
        .iter()
        .map(|entity| entity.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(persistent_ids.len(), spawned.len() + 1);
    assert!(persistent_ids.contains(&egg.persistent_id));

    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(64, 0), 0);
    for _ in 0..256 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("drain mallard entity unload");
        if server.entities.states().is_empty() {
            break;
        }
    }
    assert!(server.entities.states().is_empty());

    set_dedicated_chunk_view_and_poll(&mut server, player, wetland, 4);
    for _ in 0..256 {
        server
            .try_simulation_tick_report_for_player(player)
            .expect("hydrate mallard wetland entities");
        let reloaded = server.entities.states();
        let reloaded_ids = reloaded
            .iter()
            .map(|entity| entity.persistent_id)
            .collect::<BTreeSet<_>>();
        if reloaded_ids == persistent_ids {
            assert!(
                reloaded
                    .iter()
                    .all(|entity| !runtime_ids.contains(&entity.id))
            );
            assert_eq!(
                reloaded
                    .iter()
                    .filter(|entity| entity.kind == EntityKind::Mallard)
                    .count(),
                spawned.len()
            );
            assert!(reloaded.iter().any(|entity| {
                entity.item_stack
                    == Some(ItemStackSnapshot {
                        kind: ItemKind::MallardEgg,
                        count: 1,
                    })
            }));
            return;
        }
    }

    panic!("mallard flock and egg did not hydrate with stable identities");
}

#[test]
fn volatile_entities_are_discarded_when_ticket_loaded_chunk_fully_unloads() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    server.set_debug_passive_showcase_enabled(false);
    server.set_natural_spawning_enabled(false);
    let player = server.add_player();
    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 0);
    let entity = server.entities.spawn_volatile_passive_mob(
        EntityKind::Cow,
        Vec3d::new(8.5, 64.0, 8.5),
        0.0,
    );
    server.reconcile_entity_subjects([entity], true);
    let updates = server
        .drain_chunk_updates_for_target(CommandTarget::Player(player))
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
fn natural_spawning_can_be_disabled() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_natural_spawning_enabled(false);

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
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    let player = server.add_player();
    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);
    let snapshot = first_entity_snapshot(&updates).expect("entity snapshot");

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(8, 0), 0);

    assert!(has_entity_remove(&updates, snapshot.id));
}

#[test]
fn local_player_picks_up_ready_item_entity_through_server_inventory() {
    let mut server = LocalRealmSession::new(12_345);
    server.set_lighting_enabled(false);
    load_center_chunk(&mut server);
    let player_position = server.player().position();
    let item_id = server.entities.insert_item_entity_for_test(
        ItemStackSnapshot {
            kind: ItemKind::MallardEgg,
            count: 1,
        },
        player_position,
    );
    server.entities.set_item_pickup_delay_for_test(item_id, 0);
    let item_state = server.entities.state(item_id).unwrap();
    server.reconcile_entity_subjects(std::iter::once(item_state), true);
    let player_id = server.player_id();
    let initial_updates = server
        .drain_chunk_updates_for_target(CommandTarget::Player(player_id))
        .expect("drain initial item snapshot");
    assert!(initial_updates.iter().any(|update| {
        matches!(update, ServerUpdate::EntitySnapshot(snapshot) if snapshot.id == item_id)
    }));

    let report = server
        .try_simulation_tick_report()
        .expect("simulation tick should pick up item");

    assert!(has_entity_remove(&report.updates, item_id));
    assert_eq!(server.inventory().item_count(ItemKind::MallardEgg), 1);
    assert_eq!(server.entities.state(item_id), None);
}
