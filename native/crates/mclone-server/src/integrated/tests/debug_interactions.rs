use super::*;

#[test]
fn wooden_hoe_seed_and_harvest_form_an_authoritative_inventory_loop() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 67.0, 10.5));
    let soil = BlockPos::new(8, 64, 8);
    let crop = soil.offset(0, 1, 0);
    assert!(server.scheduler_mut().set_block_at_world(soil, GRASS_BLOCK));
    server.scheduler_mut().set_block_at_world(crop, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();

    sync_carried_slot(&mut server, 7);
    let tilled = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.0, 8.5),
            Direction::Up,
            soil,
            false,
        )))
        .expect("till grass");
    assert_eq!(
        server.scheduler().block_at_world(soil),
        Some(mclone_worldgen::block::FARMLAND_MOISTURE_0)
    );
    assert!(has_section_block_updates(&tilled));

    sync_carried_slot(&mut server, 8);
    let planted = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.0, 8.5),
            Direction::Up,
            soil,
            false,
        )))
        .expect("plant wheat");
    assert_eq!(
        server.scheduler().block_at_world(crop),
        Some(mclone_worldgen::block::WHEAT_AGE_0)
    );
    assert_eq!(server.inventory().item_count(ItemKind::WheatSeeds), 7);
    assert!(planted.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerInventory { hotbar }
            if hotbar[8] == Some(ItemStackSnapshot { kind: ItemKind::WheatSeeds, count: 7 })
    )));

    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(crop, mclone_worldgen::block::WHEAT_AGE_7)
    );
    server.scheduler_mut().drain_pending_block_delta_events();
    let harvested = server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: crop,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("harvest mature wheat");
    assert_eq!(server.scheduler().block_at_world(crop), Some(AIR));
    assert_eq!(server.inventory().item_count(ItemKind::Wheat), 0);
    let drops = server
        .entities
        .states()
        .into_iter()
        .filter(|entity| entity.kind == EntityKind::Item)
        .collect::<Vec<_>>();
    assert!(drops.iter().any(|entity| {
        entity.item_stack
            == Some(ItemStackSnapshot {
                kind: ItemKind::Wheat,
                count: 1,
            })
    }));
    assert!(harvested.iter().any(|update| {
        matches!(
            update,
            ServerUpdate::EntitySnapshot(snapshot)
                if snapshot.kind == EntityKind::Item
                    && snapshot.item_stack == Some(ItemStackSnapshot {
                        kind: ItemKind::Wheat,
                        count: 1,
                    })
        )
    }));

    let dropped_seed_count = drops
        .iter()
        .filter_map(|entity| entity.item_stack)
        .filter(|stack| stack.kind == ItemKind::WheatSeeds)
        .map(|stack| stack.count)
        .sum::<u8>();
    for drop in &drops {
        server.entities.set_item_pickup_delay_for_test(drop.id, 0);
    }
    sync_player(&mut server, Vec3d::new(8.5, 65.0, 8.5));
    server
        .try_simulation_tick_report()
        .expect("ordinary item pickup tick");
    assert_eq!(server.inventory().item_count(ItemKind::Wheat), 1);
    assert_eq!(
        server.inventory().item_count(ItemKind::WheatSeeds),
        7 + u32::from(dropped_seed_count)
    );
    assert!(
        drops
            .iter()
            .all(|drop| server.entities.state(drop.id).is_none())
    );
}

#[test]
fn carrot_plant_and_harvest_use_the_shared_crop_and_item_drop_loop() {
    let mut server = LocalRealmSession::new(19);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 67.0, 10.5));
    let soil = BlockPos::new(8, 64, 8);
    let crop = soil.offset(0, 1, 0);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(soil, mclone_worldgen::block::FARMLAND_MOISTURE_7)
    );
    server.scheduler_mut().set_block_at_world(crop, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();

    sync_carried_slot(&mut server, 3);
    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.0, 8.5),
            Direction::Up,
            soil,
            false,
        )))
        .expect("plant carrots");
    assert_eq!(
        server.scheduler().block_at_world(crop),
        Some(mclone_worldgen::block::CARROTS_AGE_0)
    );
    assert_eq!(server.inventory().item_count(ItemKind::Carrot), 7);

    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(crop, mclone_worldgen::block::CARROTS_AGE_7)
    );
    server.scheduler_mut().drain_pending_block_delta_events();
    server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: crop,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("harvest mature carrots");

    assert_eq!(server.scheduler().block_at_world(crop), Some(AIR));
    let carrot_drop_count = server
        .entities
        .states()
        .into_iter()
        .filter_map(|entity| entity.item_stack)
        .filter(|stack| stack.kind == ItemKind::Carrot)
        .map(|stack| stack.count)
        .sum::<u8>();
    assert!((1..=4).contains(&carrot_drop_count));
}

#[test]
fn carrot_feeding_accepts_the_client_selected_adult_rabbit_within_reach() {
    let mut server = LocalRealmSession::new(20);
    load_center_chunk(&mut server);
    let rabbit = server.entities.insert_passive_mob_for_test(
        EntityKind::Rabbit,
        Vec3d::new(10.5, 65.0, 10.5),
        0.0,
    );
    sync_player(&mut server, Vec3d::new(8.5, 65.0, 10.5));
    sync_carried_slot(&mut server, 3);
    let before = server.inventory().item_count(ItemKind::Carrot);

    let updates = server
        .try_handle_command(ClientCommand::InteractEntity(InteractEntityCommand {
            target: rabbit,
            hand: InteractionHand::MainHand,
        }))
        .expect("feed nearby adult rabbit selected by the client");

    assert_eq!(server.inventory().item_count(ItemKind::Carrot), before - 1);
    assert!(updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerInventory { hotbar }
            if hotbar[3]
                == Some(ItemStackSnapshot {
                    kind: ItemKind::Carrot,
                    count: u8::try_from(before - 1).unwrap(),
                })
    )));
    assert!(
        server
            .entities
            .mob_state(rabbit)
            .is_some_and(|mob| mob.rabbit_can_breed())
    );
}

#[test]
fn fences_connect_and_gate_toggles_authoritatively() {
    let mut server = LocalRealmSession::new(23);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 67.0, 10.5));
    let west = BlockPos::new(7, 65, 8);
    let gate = BlockPos::new(8, 65, 8);
    let east = BlockPos::new(9, 65, 8);
    for pos in [west, gate, east] {
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(pos.below(), STONE)
        );
        server.scheduler_mut().set_block_at_world(pos, AIR);
    }
    server.scheduler_mut().drain_pending_block_delta_events();

    sync_carried_slot(&mut server, 1);
    for pos in [west, east] {
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(
                    f64::from(pos.x) + 0.5,
                    f64::from(pos.y),
                    f64::from(pos.z) + 0.5,
                ),
                Direction::Up,
                pos.below(),
                false,
            )))
            .expect("place fence");
    }
    sync_carried_slot(&mut server, 2);
    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.0, 8.5),
            Direction::Up,
            gate.below(),
            false,
        )))
        .expect("place gate");

    let west_state =
        mclone_worldgen::block::oak_fence_state(server.scheduler().block_at_world(west).unwrap())
            .unwrap();
    let east_state =
        mclone_worldgen::block::oak_fence_state(server.scheduler().block_at_world(east).unwrap())
            .unwrap();
    assert!(west_state.east);
    assert!(east_state.west);
    assert!(
        !mclone_worldgen::block::oak_fence_gate_state(
            server.scheduler().block_at_world(gate).unwrap()
        )
        .unwrap()
        .open
    );

    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.5, 8.5),
            Direction::North,
            gate,
            false,
        )))
        .expect("open gate");
    assert!(
        mclone_worldgen::block::oak_fence_gate_state(
            server.scheduler().block_at_world(gate).unwrap()
        )
        .unwrap()
        .open
    );
    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.5, 8.5),
            Direction::North,
            gate,
            false,
        )))
        .expect("close gate");
    assert!(
        !mclone_worldgen::block::oak_fence_gate_state(
            server.scheduler().block_at_world(gate).unwrap()
        )
        .unwrap()
        .open
    );
}

#[test]
fn full_inventory_leaves_harvest_loot_in_the_world() {
    let mut server = LocalRealmSession::new(33);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 65.0, 8.5));
    let crop = BlockPos::new(8, 65, 8);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(crop, mclone_worldgen::block::WHEAT_AGE_7)
    );
    server.scheduler_mut().drain_pending_block_delta_events();
    for slot in 0..server.inventory().item_stacks().len() {
        server.inventory_mut().set_item_stack_for_test(
            slot,
            Some(ItemStackSnapshot {
                kind: ItemKind::Wheat,
                count: 64,
            }),
        );
    }

    server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: crop,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("harvest into a full inventory");

    assert_eq!(server.scheduler().block_at_world(crop), Some(AIR));
    let wheat_drop = server
        .entities
        .states()
        .into_iter()
        .find(|entity| {
            entity.item_stack
                == Some(ItemStackSnapshot {
                    kind: ItemKind::Wheat,
                    count: 1,
                })
        })
        .expect("full inventory must leave wheat in the world");
    server
        .entities
        .set_item_pickup_delay_for_test(wheat_drop.id, 0);
    server
        .try_simulation_tick_report()
        .expect("rejected pickup tick");
    assert_eq!(server.inventory().item_count(ItemKind::Wheat), 64 * 36);
    assert!(server.entities.state(wheat_drop.id).is_some());
}

#[test]
fn protected_lobby_rejects_farming_items() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 67.0, 10.5));
    let soil = BlockPos::new(8, 64, 8);
    assert!(server.scheduler_mut().set_block_at_world(soil, GRASS_BLOCK));
    server
        .scheduler_mut()
        .set_block_at_world(soil.offset(0, 1, 0), AIR);
    server.scheduler_mut().drain_pending_block_delta_events();
    server.set_world_behavior_profile(WorldBehaviorProfile::ProtectedLobby);
    sync_carried_slot(&mut server, 7);

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 65.0, 8.5),
            Direction::Up,
            soil,
            false,
        )))
        .expect("reject protected till");
    assert_eq!(server.scheduler().block_at_world(soil), Some(GRASS_BLOCK));
    assert!(updates.is_empty());
}

#[test]
fn loaded_random_ticks_hydrate_a_real_farmland_block() {
    let mut server = LocalRealmSession::new(44);
    load_center_chunk(&mut server);
    let soil = BlockPos::new(8, 64, 8);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(soil, mclone_worldgen::block::FARMLAND_MOISTURE_0)
    );
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(soil.offset(4, 0, 0), WATER)
    );
    for tick in 1..=10_000 {
        server.tick_random_farming_blocks(tick, &[soil.chunk_pos()]);
        if server.scheduler().block_at_world(soil)
            == Some(mclone_worldgen::block::FARMLAND_MOISTURE_7)
        {
            return;
        }
    }
    panic!("loaded random ticks never selected and hydrated the field");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn farming_field_and_inventory_survive_sqlite_restart() {
    let root = actor_tool_temp_dir("wheat-farming-restart");
    let identity = ClientIdentity::new(PlayerProfileId::new([0x57; 16]), "Farmer").unwrap();
    let soil = BlockPos::new(8, 64, 8);
    let crop = soil.offset(0, 1, 0);
    {
        let mut server = LocalRealmSession::try_with_threaded_sqlite_world_dir(71, &root).unwrap();
        server
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 67.0, 10.5));
        assert!(server.scheduler_mut().set_block_at_world(soil, GRASS_BLOCK));
        server.scheduler_mut().set_block_at_world(crop, AIR);
        server.scheduler_mut().drain_pending_block_delta_events();
        sync_carried_slot(&mut server, 7);
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 65.0, 8.5),
                Direction::Up,
                soil,
                false,
            )))
            .unwrap();
        sync_carried_slot(&mut server, 8);
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 65.0, 8.5),
                Direction::Up,
                soil,
                false,
            )))
            .unwrap();
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(soil, mclone_worldgen::block::FARMLAND_MOISTURE_7)
        );
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(crop, mclone_worldgen::block::WHEAT_AGE_4)
        );
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(71, &root).unwrap();
        reopened
            .configure_local_player_identity_blocking(identity)
            .unwrap();
        load_center_chunk(&mut reopened);
        assert_eq!(
            reopened.scheduler().block_at_world(soil),
            Some(mclone_worldgen::block::FARMLAND_MOISTURE_7)
        );
        assert_eq!(
            reopened.scheduler().block_at_world(crop),
            Some(mclone_worldgen::block::WHEAT_AGE_4)
        );
        assert_eq!(reopened.inventory().item_count(ItemKind::WheatSeeds), 7);
        assert_eq!(reopened.inventory().selected_hotbar_slot(), 8);
        reopened.shutdown_persistence().unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn open_garden_gate_and_carrot_crop_survive_sqlite_restart() {
    let root = actor_tool_temp_dir("functional-garden-restart");
    let identity = ClientIdentity::new(PlayerProfileId::new([0x47; 16]), "Gardener").unwrap();
    let fence = BlockPos::new(7, 65, 8);
    let gate = BlockPos::new(8, 65, 8);
    let soil = BlockPos::new(9, 64, 8);
    let crop = soil.offset(0, 1, 0);
    {
        let mut server = LocalRealmSession::try_with_threaded_sqlite_world_dir(73, &root).unwrap();
        server
            .configure_local_player_identity_blocking(identity.clone())
            .unwrap();
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 67.0, 10.5));
        for pos in [fence.below(), gate.below()] {
            assert!(server.scheduler_mut().set_block_at_world(pos, STONE));
        }
        server.scheduler_mut().set_block_at_world(fence, AIR);
        server.scheduler_mut().set_block_at_world(gate, AIR);
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(soil, mclone_worldgen::block::FARMLAND_MOISTURE_7)
        );
        server.scheduler_mut().set_block_at_world(crop, AIR);
        server.scheduler_mut().drain_pending_block_delta_events();

        sync_carried_slot(&mut server, 1);
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(7.5, 65.0, 8.5),
                Direction::Up,
                fence.below(),
                false,
            )))
            .unwrap();
        sync_carried_slot(&mut server, 2);
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 65.0, 8.5),
                Direction::Up,
                gate.below(),
                false,
            )))
            .unwrap();
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 65.5, 8.5),
                Direction::North,
                gate,
                false,
            )))
            .unwrap();
        sync_carried_slot(&mut server, 3);
        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(9.5, 65.0, 8.5),
                Direction::Up,
                soil,
                false,
            )))
            .unwrap();
        assert!(
            server
                .scheduler_mut()
                .set_block_at_world(crop, mclone_worldgen::block::CARROTS_AGE_4)
        );
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(73, &root).unwrap();
        reopened
            .configure_local_player_identity_blocking(identity)
            .unwrap();
        load_center_chunk(&mut reopened);
        assert!(
            mclone_worldgen::block::oak_fence_state(
                reopened.scheduler().block_at_world(fence).unwrap()
            )
            .unwrap()
            .east
        );
        assert!(
            mclone_worldgen::block::oak_fence_gate_state(
                reopened.scheduler().block_at_world(gate).unwrap()
            )
            .unwrap()
            .open
        );
        assert_eq!(
            reopened.scheduler().block_at_world(crop),
            Some(mclone_worldgen::block::CARROTS_AGE_4)
        );
        assert_eq!(reopened.inventory().item_count(ItemKind::Carrot), 7);
        reopened.shutdown_persistence().unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn debug_break_command_mutates_block_and_returns_section_delta() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    let pos = BlockPos::new(8, 80, 8);
    assert!(server.scheduler_mut().set_block_at_world(pos, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("break command");

    assert_eq!(server.scheduler().block_at_world(pos), Some(0));
    assert!(updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == AIR_BLOCK_STATE_ID),
            _ => false,
        }
    }));
}

#[test]
fn protected_lobby_rejects_forged_break_and_place_commands() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 1);
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();
    server.set_world_behavior_profile(WorldBehaviorProfile::ProtectedLobby);

    let break_updates = server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: clicked,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("forged protected break command");
    let place_updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("forged protected place command");

    assert_eq!(
        server.world_behavior_profile(),
        WorldBehaviorProfile::ProtectedLobby
    );
    assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
    assert_eq!(server.scheduler().block_at_world(target), Some(AIR));
    assert!(break_updates.is_empty());
    assert!(place_updates.is_empty());
    assert_eq!(
        server
            .player_statistics()
            .successful_block_placement_count(),
        0
    );
}

#[test]
fn debug_break_command_refreshes_lighting_after_opacity_change() {
    let mut server = LocalRealmSession::new(0);
    load_chunk_view_with_lighting(&mut server, ChunkPos::new(0, 0), true);
    let pos = BlockPos::new(8, 120, 8);
    let chunk_pos = pos.chunk_pos();
    if server.scheduler().block_at_world(pos) != Some(AIR) {
        assert!(server.scheduler_mut().set_block_at_world(pos, AIR));
    }
    assert!(server.scheduler_mut().set_block_at_world(pos, STONE));
    assert!(
        server
            .scheduler_mut()
            .refresh_runtime_lighting_after_block_change(pos, AIR, STONE)
    );
    server.scheduler_mut().drain_pending_block_delta_events();
    let covered = server
        .scheduler()
        .client_visible_snapshot(chunk_pos)
        .expect("lit chunk should stay visible");
    assert_eq!(sample_sky_light(&covered.light_sections, pos), 0);

    sync_player(&mut server, Vec3d::new(8.5, 120.0, 8.5));
    let updates = server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("break command");

    assert_eq!(server.scheduler().block_at_world(pos), Some(AIR));
    assert!(has_section_block_updates(&updates));
    let snapshot = snapshot_update_for(&updates, chunk_pos)
        .expect("opacity-changing break should publish refreshed light");
    assert_eq!(sample_sky_light(&snapshot.light_sections, pos), 15);
}

#[test]
fn debug_break_reschedules_neighbor_water_to_refill_removed_block() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    server.liquid_ticks = FluidTickList::new();
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 1);
    server.inventory_mut().set_item_stack_for_test(1, None);

    let target = BlockPos::new(8, 80, 8);
    for x in 6..=10 {
        for z in 6..=10 {
            server
                .scheduler_mut()
                .set_block_at_world(BlockPos::new(x, 79, z), STONE);
            server
                .scheduler_mut()
                .set_block_at_world(BlockPos::new(x, 80, z), STONE);
        }
    }
    for x in 7..=9 {
        for z in 7..=9 {
            server
                .scheduler_mut()
                .set_block_at_world(BlockPos::new(x, 80, z), WATER);
        }
    }
    server.scheduler_mut().drain_pending_block_delta_events();

    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            target,
            false,
        )))
        .expect("place command");
    assert_eq!(server.scheduler().block_at_world(target), Some(DIRT));
    assert!(
        server.scheduled_fluid_tick_count() > 0,
        "placing into water should schedule adjacent source water"
    );

    for _ in 0..FluidKind::Water.tick_delay() {
        server
            .try_simulation_tick_report()
            .expect("fluid tick while block is present");
    }
    assert_eq!(server.scheduler().block_at_world(target), Some(DIRT));
    assert_eq!(server.scheduled_fluid_tick_count(), 0);

    server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: target,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("break command");
    assert_eq!(server.scheduler().block_at_world(target), Some(AIR));
    assert!(
        server.scheduled_fluid_tick_count() > 0,
        "removing the block should reschedule adjacent source water"
    );

    for _ in 0..FluidKind::Water.tick_delay() {
        server
            .try_simulation_tick_report()
            .expect("fluid tick after break");
    }
    assert_eq!(server.scheduler().block_at_world(target), Some(WATER));
}

#[test]
fn debug_place_command_places_adjacent_to_hit_face() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 1);
    server.inventory_mut().set_item_stack_for_test(1, None);
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(target), Some(DIRT));
    assert!(updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == BlockStateId(DIRT as u32)),
            _ => false,
        }
    }));
    assert!(updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerStatistics { statistics }
            if statistics.successful_block_placement_count() == 1
    )));
    assert_eq!(
        server
            .player_statistics()
            .successful_block_placement_count(),
        1
    );
}

#[test]
fn debug_place_schedules_basic_falling_block_tick() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 82.0, 8.5));
    sync_carried_slot(&mut server, 0);
    assign_debug_hotbar_slot(&mut server, 0, generated_block_state_id(SAND));

    let side_support = BlockPos::new(7, 82, 8);
    let target = side_support.relative(Direction::East);
    let landing = BlockPos::new(8, 80, 8);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(side_support, STONE)
    );
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::new(8, 79, 8), STONE)
    );
    for y in 80..=82 {
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::new(8, y, 8), AIR);
    }
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.0, 82.5, 8.5),
            Direction::East,
            side_support,
            false,
        )))
        .expect("place sand");

    assert_eq!(server.scheduler().block_at_world(target), Some(SAND));
    assert!(has_section_block_updates(&updates));
    assert_eq!(server.scheduled_block_tick_count(), 7);

    let first = server
        .try_simulation_tick_report()
        .expect("first falling delay tick");
    assert!(!has_section_block_updates(&first.updates));
    assert_eq!(server.scheduler().block_at_world(target), Some(SAND));

    let second = server.try_simulation_tick_report().expect("falling tick");
    assert!(has_section_block_updates(&second.updates));
    assert_eq!(server.scheduler().block_at_world(target), Some(AIR));
    assert_eq!(server.scheduler().block_at_world(landing), Some(SAND));
}

#[test]
fn debug_break_schedules_basic_falling_block_above() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 82.0, 8.5));

    let support = BlockPos::new(8, 81, 8);
    let sand = support.relative(Direction::Up);
    let landing = BlockPos::new(8, 80, 8);
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::new(8, 79, 8), STONE)
    );
    assert!(server.scheduler_mut().set_block_at_world(support, STONE));
    assert!(server.scheduler_mut().set_block_at_world(sand, SAND));
    server.scheduler_mut().set_block_at_world(landing, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: support,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("break support");

    assert_eq!(server.scheduler().block_at_world(support), Some(AIR));
    assert_eq!(server.scheduler().block_at_world(sand), Some(SAND));
    assert!(has_section_block_updates(&updates));
    assert_eq!(server.scheduled_block_tick_count(), 6);

    server
        .try_simulation_tick_report()
        .expect("first falling delay tick");
    let second = server.try_simulation_tick_report().expect("falling tick");

    assert!(has_section_block_updates(&second.updates));
    assert_eq!(server.scheduler().block_at_world(sand), Some(AIR));
    assert_eq!(server.scheduler().block_at_world(landing), Some(SAND));
}

#[test]
fn debug_place_command_places_block_from_selected_default_hotbar_slot() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 0);
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(target), Some(STONE));
    assert!(updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == BlockStateId(STONE as u32)),
            _ => false,
        }
    }));
}

#[test]
fn set_debug_hotbar_slot_command_changes_placed_block() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 0);
    assign_debug_hotbar_slot(&mut server, 0, BlockStateId(BRICKS as u32));
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(target), Some(BRICKS));
    assert!(updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == BlockStateId(BRICKS as u32)),
            _ => false,
        }
    }));
}

#[test]
fn actor_hotbar_tools_spawn_authoritative_chicken_and_mannequin() {
    let mut server = LocalRealmSession::new(0);
    server.inventory_mut().set_item_stack_for_test(7, None);
    server.inventory_mut().set_item_stack_for_test(8, None);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 82.0, 10.5));
    for clicked in [BlockPos::new(8, 80, 8), BlockPos::new(9, 80, 8)] {
        server.scheduler_mut().set_block_at_world(clicked, STONE);
        server
            .scheduler_mut()
            .set_block_at_world(clicked.relative(Direction::Up), AIR);
        server
            .scheduler_mut()
            .set_block_at_world(clicked.offset(0, 2, 0), AIR);
    }
    server.scheduler_mut().drain_pending_block_delta_events();

    sync_carried_slot(&mut server, 7);
    let chicken_updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            BlockPos::new(8, 80, 8),
            false,
        )))
        .expect("spawn chicken");
    let chicken = first_entity_snapshot_of_kind(&chicken_updates, EntityKind::Chicken)
        .expect("authoritative chicken snapshot");
    assert_eq!(chicken.position, Vec3d::new(8.5, 81.0, 8.5));

    sync_carried_slot(&mut server, 8);
    let mannequin_updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(9.5, 81.0, 8.5),
            Direction::Up,
            BlockPos::new(9, 80, 8),
            false,
        )))
        .expect("spawn mannequin");
    let mannequin = first_entity_snapshot_of_kind(&mannequin_updates, EntityKind::Mannequin)
        .expect("authoritative mannequin snapshot");
    assert_eq!(mannequin.position, Vec3d::new(9.5, 81.0, 8.5));
    assert_eq!((mannequin.width, mannequin.height), (0.6, 1.8));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn placed_chicken_and_mannequin_survive_sqlite_restart() {
    let root = actor_tool_temp_dir("actor-tools-restart");
    let seed = 0;
    {
        let mut server =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        server.set_debug_passive_showcase_enabled(false);
        server.inventory_mut().set_item_stack_for_test(7, None);
        server.inventory_mut().set_item_stack_for_test(8, None);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 82.0, 10.5));
        for clicked in [BlockPos::new(8, 80, 8), BlockPos::new(9, 80, 8)] {
            server.scheduler_mut().set_block_at_world(clicked, STONE);
            server
                .scheduler_mut()
                .set_block_at_world(clicked.relative(Direction::Up), AIR);
            server
                .scheduler_mut()
                .set_block_at_world(clicked.offset(0, 2, 0), AIR);
        }
        server.scheduler_mut().drain_pending_block_delta_events();

        for (slot, clicked) in [(7, BlockPos::new(8, 80, 8)), (8, BlockPos::new(9, 80, 8))] {
            sync_carried_slot(&mut server, slot);
            server
                .try_handle_command(use_held_item_on(BlockHitResult::new(
                    Vec3d::new(clicked.x as f64 + 0.5, 81.0, clicked.z as f64 + 0.5),
                    Direction::Up,
                    clicked,
                    false,
                )))
                .expect("spawn persistent actor tool entity");
        }
        let states = server.entities.states();
        assert!(
            states
                .iter()
                .any(|entity| entity.kind == EntityKind::Chicken)
        );
        assert!(
            states
                .iter()
                .any(|entity| entity.kind == EntityKind::Mannequin)
        );
        server.shutdown_persistence().unwrap();
    }

    {
        let mut reopened =
            LocalRealmSession::try_with_threaded_sqlite_world_dir(seed, &root).unwrap();
        reopened.set_debug_passive_showcase_enabled(false);
        load_center_chunk(&mut reopened);
        let restored = reopened.entities.states();
        let chicken = restored
            .iter()
            .find(|entity| entity.kind == EntityKind::Chicken)
            .expect("restored chicken");
        let mannequin = restored
            .iter()
            .find(|entity| entity.kind == EntityKind::Mannequin)
            .expect("restored mannequin");
        assert_eq!(chicken.position, Vec3d::new(8.5, 81.0, 8.5));
        assert_eq!(mannequin.position, Vec3d::new(9.5, 81.0, 8.5));
        assert_eq!(
            reopened
                .entities
                .mob_state(mannequin.id)
                .expect("restored mannequin goals")
                .available_goal_count(),
            3
        );
        reopened.shutdown_persistence().unwrap();
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
fn actor_tool_temp_dir(name: &str) -> std::path::PathBuf {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("mclone-{name}-{}-{id}", std::process::id()))
}

#[test]
fn actor_hotbar_tools_require_debug_capability_and_valid_clear_target() {
    let mut unauthorized = LocalRealmSession::from_server_with_capabilities(
        RealmServer::new(0),
        SessionCapabilities::NONE,
    );
    unauthorized
        .inventory_mut()
        .set_item_stack_for_test(7, None);
    load_center_chunk(&mut unauthorized);
    sync_player(&mut unauthorized, Vec3d::new(8.5, 82.0, 10.5));
    let clicked = BlockPos::new(8, 80, 8);
    unauthorized
        .scheduler_mut()
        .set_block_at_world(clicked, STONE);
    unauthorized
        .scheduler_mut()
        .set_block_at_world(clicked.relative(Direction::Up), AIR);
    unauthorized
        .scheduler_mut()
        .set_block_at_world(clicked.offset(0, 2, 0), AIR);
    unauthorized
        .scheduler_mut()
        .drain_pending_block_delta_events();
    sync_carried_slot(&mut unauthorized, 7);

    let unauthorized_updates = unauthorized
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("reject unauthorized actor tool");
    assert!(first_entity_snapshot_of_kind(&unauthorized_updates, EntityKind::Chicken).is_none());

    let mut blocked = LocalRealmSession::new(0);
    blocked.inventory_mut().set_item_stack_for_test(8, None);
    load_center_chunk(&mut blocked);
    sync_player(&mut blocked, Vec3d::new(8.5, 82.0, 10.5));
    blocked.scheduler_mut().set_block_at_world(clicked, STONE);
    blocked
        .scheduler_mut()
        .set_block_at_world(clicked.relative(Direction::Up), STONE);
    blocked.scheduler_mut().drain_pending_block_delta_events();
    sync_carried_slot(&mut blocked, 8);

    let blocked_updates = blocked
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("reject blocked mannequin placement");
    assert!(first_entity_snapshot_of_kind(&blocked_updates, EntityKind::Mannequin).is_none());
}

#[test]
fn debug_place_command_orients_log_to_clicked_face_axis() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 4);
    let clicked = BlockPos::new(8, 80, 8);
    let east_target = clicked.relative(Direction::East);
    let north_target = clicked.relative(Direction::North);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let east_updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(9.0, 80.5, 8.5),
            Direction::East,
            clicked,
            false,
        )))
        .expect("place east-facing log");

    assert_eq!(
        server.scheduler().block_at_world(east_target),
        Some(OAK_LOG_X)
    );
    assert!(east_updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == BlockStateId(OAK_LOG_X as u32)),
            _ => false,
        }
    }));

    let north_updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 80.5, 8.0),
            Direction::North,
            clicked,
            false,
        )))
        .expect("place north-facing log");

    assert_eq!(
        server.scheduler().block_at_world(north_target),
        Some(OAK_LOG_Z)
    );
    assert!(north_updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == BlockStateId(OAK_LOG_Z as u32)),
            _ => false,
        }
    }));
}

#[test]
fn debug_place_command_places_floor_torch_from_torch_item() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 0);
    assign_debug_hotbar_slot(&mut server, 0, generated_block_state_id(TORCH));
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().set_block_at_world(target, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place floor torch");

    assert_eq!(server.scheduler().block_at_world(target), Some(TORCH));
    assert!(has_section_block_update_with_state(
        &updates,
        generated_block_state_id(TORCH)
    ));
}

#[test]
fn debug_place_command_places_wall_torch_matching_clicked_side() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 0);
    assign_debug_hotbar_slot(&mut server, 0, generated_block_state_id(TORCH));
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::East);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().set_block_at_world(target, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(9.0, 80.5, 8.5),
            Direction::East,
            clicked,
            false,
        )))
        .expect("place wall torch");

    assert_eq!(
        server.scheduler().block_at_world(target),
        Some(WALL_TORCH_EAST)
    );
    assert!(has_section_block_update_with_state(
        &updates,
        generated_block_state_id(WALL_TORCH_EAST)
    ));
}

#[test]
fn debug_place_command_rejects_bottom_face_torch_placement() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 0);
    assign_debug_hotbar_slot(&mut server, 0, generated_block_state_id(TORCH));
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Down);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().set_block_at_world(target, AIR);
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 80.0, 8.5),
            Direction::Down,
            clicked,
            false,
        )))
        .expect("reject bottom-face torch placement");

    assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
    assert_eq!(server.scheduler().block_at_world(target), Some(AIR));
    assert!(updates.is_empty());
}

#[test]
fn debug_place_command_replaces_clicked_replaceable_block() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 1);
    server.inventory_mut().set_item_stack_for_test(1, None);
    let clicked = BlockPos::new(8, 80, 8);
    let adjacent = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, GRASS));
    assert!(server.scheduler_mut().set_block_at_world(adjacent, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(clicked), Some(DIRT));
    assert_eq!(server.scheduler().block_at_world(adjacent), Some(STONE));
    assert!(updates.iter().any(|update| {
        match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                .iter()
                .any(|update| update.block_state == BlockStateId(DIRT as u32)),
            _ => false,
        }
    }));
}

#[test]
fn debug_place_command_does_not_overwrite_solid_relative_target() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 1);
    server.inventory_mut().set_item_stack_for_test(1, None);
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    assert!(server.scheduler_mut().set_block_at_world(target, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
    assert_eq!(server.scheduler().block_at_world(target), Some(STONE));
    assert!(updates.is_empty());
}

#[test]
fn debug_place_command_rejects_cleared_selected_hotbar_slot() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 8);
    clear_debug_hotbar_slot(&mut server, 8);
    let clicked = BlockPos::new(8, 80, 8);
    assert!(server.scheduler_mut().set_block_at_world(clicked, GRASS));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(clicked), Some(GRASS));
    assert!(updates.is_empty());
}

#[test]
fn debug_interaction_commands_reject_far_server_player_positions() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(100.0, 80.0, 100.0));
    sync_carried_slot(&mut server, 1);
    let clicked = BlockPos::new(8, 80, 8);
    let target = clicked.relative(Direction::Up);
    assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
    server.scheduler_mut().drain_pending_block_delta_events();

    let break_updates = server
        .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: clicked,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))
        .expect("break command");
    let place_updates = server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 81.0, 8.5),
            Direction::Up,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
    assert_eq!(server.scheduler().block_at_world(target), Some(0));
    assert!(break_updates.is_empty());
    assert!(place_updates.is_empty());
}

#[test]
fn debug_place_command_replaces_one_layer_snow_in_place() {
    let mut server = LocalRealmSession::new(0);
    load_center_chunk(&mut server);
    sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
    sync_carried_slot(&mut server, 1);
    server.inventory_mut().set_item_stack_for_test(1, None);
    let clicked = BlockPos::new(8, 80, 8);
    assert!(server.scheduler_mut().set_block_at_world(clicked, SNOW));
    server.scheduler_mut().drain_pending_block_delta_events();

    server
        .try_handle_command(use_held_item_on(BlockHitResult::new(
            Vec3d::new(8.5, 80.125, 8.5),
            Direction::North,
            clicked,
            false,
        )))
        .expect("place command");

    assert_eq!(server.scheduler().block_at_world(clicked), Some(DIRT));
}
