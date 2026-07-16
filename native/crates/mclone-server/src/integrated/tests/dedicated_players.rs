use super::*;

#[test]
fn dedicated_join_orders_negotiated_configuration_before_world_state() {
    let mut server = IntegratedServer::new(0);
    let player = server.add_dedicated_player_with_capabilities(SessionCapabilities::NONE);

    let updates = server
        .try_drain_updates_for_player(player)
        .expect("drain dedicated join updates");

    assert!(matches!(
        updates.as_slice(),
        [
            ServerUpdate::SessionConfiguration(SessionConfiguration {
                gameplay_rate_hz: 20,
                publication_rate_hz: 20,
                max_render_distance: 11,
                max_chunk_tracking_radius: 11,
                capabilities: SessionCapabilities::NONE,
            }),
            ServerUpdate::SessionReady,
            ServerUpdate::WorldInfo { .. },
            ServerUpdate::TimeUpdate { .. },
        ]
    ));
}

fn time_update(updates: &[ServerUpdate]) -> Option<u64> {
    updates.iter().rev().find_map(|update| match update {
        ServerUpdate::TimeUpdate { day_time, .. } => Some(*day_time),
        _ => None,
    })
}

#[test]
fn global_simulation_tick_routes_without_draining_player_streams() {
    let mut server = IntegratedServer::new(0);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    server
        .try_drain_updates_for_player(player_a)
        .expect("drain player a join updates");
    server
        .try_drain_updates_for_player(player_b)
        .expect("drain player b join updates");

    let start_day_time = server.day_time();
    let report = server
        .try_simulation_tick_report_global()
        .expect("global simulation tick");

    assert_eq!(report.simulation_tick, 1);
    assert_eq!(server.simulation_tick(), 1);
    assert_eq!(server.day_time(), start_day_time + 1);
    assert!(report.updates.is_empty());

    let updates_a = server
        .try_drain_updates_for_player(player_a)
        .expect("drain player a");
    assert_eq!(time_update(&updates_a), Some(start_day_time + 1));
    assert!(updates_a.iter().any(|update| matches!(
        update,
        ServerUpdate::TimeUpdate {
            game_time: 1,
            daylight_cycle_running: true,
            ..
        }
    )));

    // Draining A neither consumes B nor performs another simulation step.
    assert_eq!(server.simulation_tick(), 1);
    assert_eq!(server.day_time(), start_day_time + 1);
    let updates_b = server
        .try_drain_updates_for_player(player_b)
        .expect("drain player b");
    assert_eq!(time_update(&updates_b), Some(start_day_time + 1));
    assert!(
        server
            .try_drain_updates_for_player(player_a)
            .expect("repeat drain player a")
            .is_empty()
    );
    assert_eq!(server.simulation_tick(), 1);
    assert_eq!(server.day_time(), start_day_time + 1);
}

#[test]
fn time_publication_broadcasts_at_twenty_tick_period() {
    let mut server = IntegratedServer::new(0);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    server
        .try_drain_updates_for_player(player_a)
        .expect("drain player a join updates");
    server
        .try_drain_updates_for_player(player_b)
        .expect("drain player b join updates");

    for tick in 1..=20 {
        server
            .try_simulation_tick_report_global()
            .expect("global simulation tick");
        let updates_a = server
            .try_drain_updates_for_player(player_a)
            .expect("drain player a");
        let updates_b = server
            .try_drain_updates_for_player(player_b)
            .expect("drain player b");
        let expected = (tick == 1 || tick == 20).then_some(server.day_time());
        assert_eq!(time_update(&updates_a), expected, "player a tick {tick}");
        assert_eq!(time_update(&updates_b), expected, "player b tick {tick}");
    }
}

#[test]
fn dedicated_player_receives_world_info_before_chunk_view_snapshots() {
    let mut server = IntegratedServer::new(1124);
    server.set_lighting_enabled(false);
    let player = server.add_dedicated_player();

    let updates = set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 0);

    assert_eq!(
        first_biome_zoom_seed(&updates),
        Some(obfuscate_biome_zoom_seed(1124))
    );
    assert!(
        updates
            .iter()
            .position(|update| matches!(update, ServerUpdate::WorldInfo { .. }))
            < updates
                .iter()
                .position(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
    );
}

#[test]
fn dedicated_player_views_keep_disjoint_ticket_sets() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();

    let updates_a =
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    let updates_b =
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(4, 0), 0);

    let snapshots_a = snapshot_positions(&updates_a);
    let snapshots_b = snapshot_positions(&updates_b);
    assert!(snapshots_a.contains(&ChunkPos::new(0, 0)));
    assert!(!snapshots_a.contains(&ChunkPos::new(4, 0)));
    assert!(snapshots_b.contains(&ChunkPos::new(4, 0)));
    assert!(!snapshots_b.contains(&ChunkPos::new(0, 0)));
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(4, 0)), 1);
}

#[test]
fn overlapping_player_views_unload_only_for_player_leaving_chunk() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);

    let updates_a =
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(1, 0), 0);
    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

    assert!(has_chunk_unload(&updates_a, ChunkPos::new(0, 0)));
    assert!(!has_chunk_unload(&updates_b, ChunkPos::new(0, 0)));
    assert!(
        server
            .scheduler()
            .holder(ChunkPos::new(0, 0))
            .is_some_and(ChunkHolder::is_client_visible)
    );
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
}

#[test]
fn smaller_player_view_receives_fewer_snapshots_than_larger_view() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();

    let updates_a =
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    let updates_b =
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 1);

    let snapshots_a = snapshot_positions(&updates_a);
    let snapshots_b = snapshot_positions(&updates_b);
    assert_eq!(snapshots_a.len(), 1);
    assert_eq!(snapshots_b.len(), 9);
    assert!(snapshots_b.len() > snapshots_a.len());
}

#[test]
fn block_delta_routing_sends_only_to_players_tracking_changed_chunk() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(2, 0), 0);
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
        .expect("move player a");
    let pos = BlockPos::new(8, 80, 8);
    assert!(server.scheduler_mut().set_block_at_world(pos, DIRT));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates_a = server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::PlayerAction(PlayerActionCommand {
                pos,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }),
        )
        .expect("break block");
    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

    assert!(has_section_block_updates(&updates_a));
    assert!(!has_section_block_updates(&updates_b));
}

#[test]
fn protected_lobby_rejects_forged_dedicated_player_break() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    server.set_world_behavior_profile(WorldBehaviorProfile::ProtectedLobby);
    let player = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 0);
    let pos = BlockPos::new(8, 80, 8);
    assert!(server.scheduler_mut().set_block_at_world(pos, DIRT));
    server.scheduler_mut().drain_pending_block_delta_events();

    let updates = server
        .try_handle_command_for_player(
            player,
            ClientCommand::PlayerAction(PlayerActionCommand {
                pos,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }),
        )
        .expect("forged dedicated protected break");

    assert_eq!(server.scheduler().block_at_world(pos), Some(DIRT));
    assert!(!has_section_block_updates(&updates));
}
