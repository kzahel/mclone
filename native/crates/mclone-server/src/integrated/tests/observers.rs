use super::*;
use crate::{
    CHUNK_LEVEL_FULL, FullChunkStatus, MemoryWorldStore, ObserverSimulationInterest,
    PLAYER_TICKET_LEVEL,
};

fn zero_radius_view(center: ChunkPos) -> ChunkView {
    ChunkView {
        center,
        render_distance: 0,
        chunk_tracking_radius: 0,
    }
}

fn wait_for_observer_snapshot(
    server: &mut RealmServer,
    observer_id: ObserverId,
) -> Vec<ServerUpdate> {
    let mut updates = Vec::new();
    for _ in 0..60_000 {
        server.try_tick_report_global().unwrap();
        updates.extend(server.try_drain_updates_for_observer(observer_id).unwrap());
        if updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
        {
            return updates;
        }
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    panic!("timed out waiting for observer chunk snapshot");
}

#[test]
fn one_two_and_four_interest_sources_union_overlap_and_remove_cleanly() {
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);

    let shared = ChunkPos::new(0, 0);
    let separated_a = ChunkPos::new(8, 0);
    let separated_b = ChunkPos::new(-8, 0);
    let player = server.add_player();
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::SetChunkView(zero_radius_view(shared)),
        )
        .unwrap();

    let one = server.chunk_tracking_diagnostics();
    assert_eq!((one.player_count, one.observer_count), (1, 0));
    assert_eq!(one.aggregate_resident_chunks, 1);
    assert_eq!(server.scheduler().ticket_count_at(shared), 1);

    let overlap = server
        .add_observer(
            DimensionKey::overworld(),
            zero_radius_view(shared),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    let two = server.chunk_tracking_diagnostics();
    assert_eq!((two.player_count, two.observer_count), (1, 1));
    assert_eq!(two.aggregate_resident_chunks, 1);
    assert_eq!(two.aggregate_player_ticket_chunks, 1);
    assert_eq!(server.scheduler().ticket_count_at(shared), 1);

    let far_a = server
        .add_observer(
            DimensionKey::overworld(),
            zero_radius_view(separated_a),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    let far_b = server
        .add_observer(
            DimensionKey::overworld(),
            zero_radius_view(separated_b),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    let four = server.chunk_tracking_diagnostics();
    assert_eq!((four.player_count, four.observer_count), (1, 3));
    assert_eq!(four.aggregate_resident_chunks, 3);
    assert_eq!(server.scheduler().ticket_count_at(shared), 1);
    assert_eq!(server.scheduler().ticket_count_at(separated_a), 1);
    assert_eq!(server.scheduler().ticket_count_at(separated_b), 1);

    assert!(server.remove_observer(far_a).unwrap());
    let after_far_removal = server.chunk_tracking_diagnostics();
    assert_eq!(after_far_removal.aggregate_resident_chunks, 2);
    assert_eq!(server.scheduler().ticket_count_at(separated_a), 0);
    assert_eq!(server.scheduler().ticket_count_at(shared), 1);
    assert_eq!(server.scheduler().ticket_count_at(separated_b), 1);

    assert!(server.remove_observer(overlap).unwrap());
    assert_eq!(
        server
            .chunk_tracking_diagnostics()
            .aggregate_resident_chunks,
        2
    );
    assert_eq!(server.scheduler().ticket_count_at(shared), 1);
    assert!(server.remove_player(player));
    assert_eq!(
        server
            .chunk_tracking_diagnostics()
            .aggregate_resident_chunks,
        1
    );
    assert_eq!(server.scheduler().ticket_count_at(shared), 0);
    assert_eq!(server.scheduler().ticket_count_at(separated_b), 1);

    assert!(server.remove_observer(far_b).unwrap());
    let empty = server.chunk_tracking_diagnostics();
    assert_eq!((empty.player_count, empty.observer_count), (0, 0));
    assert_eq!(empty.aggregate_resident_chunks, 0);
    assert_eq!(server.scheduler().ticket_count_at(separated_b), 0);
}

#[test]
fn residency_observer_overlaps_player_without_becoming_a_player() {
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);
    let player = server.add_player();
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::SetChunkView(zero_radius_view(ChunkPos::new(0, 0))),
        )
        .unwrap();
    let observer = server
        .add_observer(
            DimensionKey::overworld(),
            zero_radius_view(ChunkPos::new(0, 0)),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();

    let diagnostics = server.chunk_tracking_diagnostics();
    assert_eq!(server.player_count(), 1);
    assert_eq!(server.observer_count(), 1);
    assert_eq!(diagnostics.player_count, 1);
    assert_eq!(diagnostics.observer_count, 1);
    assert_eq!(diagnostics.aggregate_resident_chunks, 1);
    assert_eq!(diagnostics.aggregate_player_ticket_chunks, 1);
    assert_eq!(diagnostics.aggregate_simulation_ticket_chunks, 1);
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
    let realm_diagnostics = server.realm_interest_diagnostics();
    assert_eq!(realm_diagnostics.realm_id, server.realm_id());
    assert_eq!(realm_diagnostics.loaded_dimension_count, 1);
    assert_eq!(realm_diagnostics.player_count, 1);
    assert_eq!(realm_diagnostics.observer_count, 1);
    assert_eq!(
        realm_diagnostics.dimensions[0].dimension,
        DimensionKey::overworld()
    );

    let updates = wait_for_observer_snapshot(&mut server, observer);
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::SessionReady))
    );
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::WorldInfo { .. }))
    );
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
    );
    assert!(updates.iter().all(|update| !matches!(
        update,
        ServerUpdate::PlayerPosition(_)
            | ServerUpdate::PlayerLife(_)
            | ServerUpdate::PlayerExperience { .. }
            | ServerUpdate::PlayerStatistics { .. }
    )));

    let player_updates = server.try_drain_updates_for_player(player).unwrap();
    if let Some(teleport_id) = player_updates.iter().find_map(|update| match update {
        ServerUpdate::PlayerPosition(update) => Some(update.teleport_id),
        _ => None,
    }) {
        server
            .try_handle_command_for_player(
                player,
                ClientCommand::AcceptTeleport(AcceptTeleportCommand { id: teleport_id }),
            )
            .unwrap();
    }
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
                position: Vec3d::new(8.5, 80.0, 8.5),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .unwrap();
    let observed_player = server.try_drain_updates_for_observer(observer).unwrap();
    assert!(observed_player.iter().any(|update| matches!(
        update,
        ServerUpdate::RemotePlayerAdd(_) | ServerUpdate::RemotePlayerUpdate(_)
    )));
    let owner_updates = server
        .try_handle_command_for_player(
            player,
            ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(8.5, 80.42, 8.5),
                on_ground: false,
            }),
        )
        .unwrap();
    assert!(owner_updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerStatistics { statistics } if statistics.jump_count() == 1
    )));
    assert!(
        server
            .try_drain_updates_for_observer(observer)
            .unwrap()
            .iter()
            .all(|update| !matches!(
                update,
                ServerUpdate::PlayerLife(_) | ServerUpdate::PlayerStatistics { .. }
            ))
    );
    let block = BlockPos::new(8, 80, 8);
    assert!(server.scheduler_mut().set_block_at_world(block, DIRT));
    server.scheduler_mut().drain_pending_block_delta_events();
    server
        .try_handle_command_for_player(
            player,
            ClientCommand::PlayerAction(PlayerActionCommand {
                pos: block,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }),
        )
        .unwrap();
    assert!(has_section_block_updates(
        &server.try_drain_updates_for_observer(observer).unwrap()
    ));

    assert!(server.remove_player(player));
    assert_eq!(server.player_count(), 0);
    assert_eq!(server.observer_count(), 1);
    assert_eq!(server.save_all_player_records().unwrap(), 0);
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
    assert_eq!(
        server.scheduler().ticket_level_at(ChunkPos::new(0, 0)),
        CHUNK_LEVEL_FULL
    );
    assert_eq!(
        server
            .scheduler()
            .holder(ChunkPos::new(0, 0))
            .unwrap()
            .full_status(),
        FullChunkStatus::Border
    );
    server
        .set_observer_interest(
            observer,
            zero_radius_view(ChunkPos::new(0, 0)),
            ObserverSimulationInterest::BlockAndEntityTicking,
        )
        .unwrap();
    assert_eq!(
        server.scheduler().ticket_level_at(ChunkPos::new(0, 0)),
        PLAYER_TICKET_LEVEL
    );
    assert_eq!(
        server
            .scheduler()
            .holder(ChunkPos::new(0, 0))
            .unwrap()
            .full_status(),
        FullChunkStatus::EntityTicking
    );
    server
        .set_observer_interest(
            observer,
            zero_radius_view(ChunkPos::new(0, 0)),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();

    assert!(server.remove_observer(observer).unwrap());
    assert_eq!(server.observer_count(), 0);
    assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 0);
}

#[test]
fn observers_are_dimension_local_cancel_cleanly_and_gate_idle_unload() {
    let moon = DimensionKey::parse("mclone:observer_moon").unwrap();
    let mut server = RealmServer::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);
    server
        .register_dimension(DimensionRecord {
            key: moon.clone(),
            codec_version: crate::DIMENSION_RECORD_VERSION,
            revision: 1,
            definition: crate::DimensionDefinition::overworld(
                54_321,
                WorldGenerationProfile::authored_only(),
            ),
        })
        .unwrap();
    let observer = server
        .add_observer(
            moon.clone(),
            zero_radius_view(ChunkPos::new(0, 0)),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    assert_eq!(server.observer_dimension(observer), Some(&moon));
    assert!(!server.unload_dimension(&moon).unwrap());

    let overworld_player = server.add_player();
    server
        .try_handle_command_for_player(
            overworld_player,
            ClientCommand::SetChunkView(zero_radius_view(ChunkPos::new(0, 0))),
        )
        .unwrap();
    wait_for_observer_snapshot(&mut server, observer);
    server.activate_player_dimension(overworld_player).unwrap();
    assert!(
        server
            .scheduler_mut()
            .set_block_at_world(BlockPos::new(8, 80, 8), DIRT)
    );
    server.try_tick_report_global().unwrap();
    let moon_updates = server.try_drain_updates_for_observer(observer).unwrap();
    assert!(!has_section_block_updates(&moon_updates));

    assert!(server.remove_observer(observer).unwrap());
    assert!(server.unload_dimension(&moon).unwrap());
    assert!(!server.loaded_dimension_keys().contains(&moon));

    assert!(
        server
            .register_dimension(DimensionRecord {
                key: moon.clone(),
                codec_version: crate::DIMENSION_RECORD_VERSION,
                revision: 1,
                definition: crate::DimensionDefinition::overworld(
                    54_321,
                    WorldGenerationProfile::authored_only(),
                ),
            })
            .unwrap()
    );
    let cancelled = server
        .add_observer(
            moon.clone(),
            zero_radius_view(ChunkPos::new(30, 30)),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    assert!(server.remove_observer(cancelled).unwrap());
    for _ in 0..4 {
        server.try_tick_report_global().unwrap();
    }
    server.activate_dimension(&moon).unwrap();
    let diagnostics = server.chunk_tracking_diagnostics();
    assert_eq!(diagnostics.observer_count, 0);
    assert_eq!(diagnostics.total_observer_outbound_queue_depth, 0);
    assert_eq!(diagnostics.aggregate_resident_chunks, 0);
    assert!(server.unload_dimension(&moon).unwrap());
}

#[test]
fn local_observer_promotes_to_one_identity_player_without_preview_persistence() {
    let mut session =
        LocalRealmSession::with_world_store(12_345, Box::new(MemoryWorldStore::new()));
    session
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    session.set_lighting_enabled(false);
    let observer = session
        .begin_observing(
            DimensionKey::overworld(),
            zero_radius_view(ChunkPos::new(0, 0)),
            ObserverSimulationInterest::ResidencyOnly,
        )
        .unwrap();
    let identity = ClientIdentity::new(PlayerProfileId::new([0x74; 16]), "Traveler").unwrap();
    session
        .configure_local_player_identity(identity.clone())
        .unwrap();

    assert_eq!(session.role(), LocalRealmSessionRole::Observer(observer));
    assert_eq!(session.player_count(), 0);
    assert_eq!(session.observer_count(), 1);
    assert_eq!(session.save_all_player_records().unwrap(), 0);
    assert!(
        session
            .try_handle_command(ClientCommand::move_player(MovePlayerCommand::StatusOnly {
                on_ground: true
            }))
            .is_err()
    );

    let player = session.promote_observer_to_player_blocking().unwrap();

    assert_eq!(session.role(), LocalRealmSessionRole::Player(player));
    assert_eq!(session.player_count(), 1);
    assert_eq!(session.observer_count(), 0);
    assert_eq!(
        session.players.get(player).unwrap().identity.as_ref(),
        Some(&identity)
    );
    assert_eq!(session.save_all_player_records().unwrap(), 0);

    let return_observer = session.demote_player_to_observer_blocking().unwrap();

    assert_eq!(
        session.role(),
        LocalRealmSessionRole::Observer(return_observer)
    );
    assert_eq!(session.player_count(), 0);
    assert_eq!(session.observer_count(), 1);
    assert_eq!(session.save_all_player_records().unwrap(), 0);

    let returning_player = session.promote_observer_to_player_blocking().unwrap();
    assert_eq!(
        session.role(),
        LocalRealmSessionRole::Player(returning_player)
    );
    assert_eq!(session.player_count(), 1);
    assert_eq!(session.observer_count(), 0);
    assert_eq!(
        session
            .players
            .get(returning_player)
            .unwrap()
            .identity
            .as_ref(),
        Some(&identity)
    );
}
