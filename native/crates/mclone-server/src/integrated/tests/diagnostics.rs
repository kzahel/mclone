use super::*;

#[test]
fn chunk_tracking_diagnostics_reports_outbound_queue_depth() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
    set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);
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

    assert!(has_section_block_updates(&updates_a));
    let diagnostics = server.chunk_tracking_diagnostics();
    assert_eq!(diagnostics.aggregate_player_ticket_chunks, 1);
    assert_eq!(diagnostics.total_player_visible_chunks, 2);
    assert_eq!(diagnostics.total_outbound_queue_depth, 2);
    assert_eq!(diagnostics.max_outbound_queue_depth, 2);
    assert_eq!(
        diagnostics
            .players
            .iter()
            .find(|player| player.player_id == player_b)
            .map(|player| player.outbound_queue_depth),
        Some(2)
    );

    let updates_b = server.try_poll_for_player(player_b).expect("poll player b");
    assert!(has_section_block_updates(&updates_b));
    assert!(
        remote_player_add(&updates_b, player_a).is_some()
            || remote_player_update(&updates_b, player_a).is_some()
    );
    assert_eq!(
        server
            .chunk_tracking_diagnostics()
            .total_outbound_queue_depth,
        0
    );
}

#[test]
fn disconnect_removes_player_chunk_ticket_contribution() {
    let mut server = IntegratedServer::new(12_345);
    server.set_lighting_enabled(false);
    let player_a = server.add_dedicated_player();
    let player_b = server.add_dedicated_player();
    server
        .try_handle_command_for_player(
            player_a,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .expect("set view a");
    server
        .try_handle_command_for_player(
            player_b,
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(4, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .expect("set view b");
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(4, 0)), 1);

    assert!(server.remove_dedicated_player(player_a));

    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 0);
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(4, 0)), 1);
}
