use super::*;
use crate::{
    AuthoredWorldFixtureKind, MemoryWorldStore, PlayerDimensionTransferPhase, WorldStore,
    authored_world_fixture_records,
};

fn moon_record() -> DimensionRecord {
    DimensionRecord {
        key: DimensionKey::parse("mclone:transfer_moon").unwrap(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition: crate::DimensionDefinition::overworld(
            54_321,
            WorldGenerationProfile::authored_only(),
        ),
    }
}

fn fixture_store(moon: &DimensionKey) -> MemoryWorldStore {
    let mut store = MemoryWorldStore::new();
    let (_, chunks, entity_chunks) =
        authored_world_fixture_records(AuthoredWorldFixtureKind::Table).unwrap();
    for record in chunks {
        store
            .save_chunk(&DimensionKey::overworld(), &record)
            .unwrap();
        store.save_chunk(moon, &record).unwrap();
    }
    for record in entity_chunks {
        store
            .save_entity_chunk(&DimensionKey::overworld(), &record)
            .unwrap();
        store.save_entity_chunk(moon, &record).unwrap();
    }
    store
}

fn zero_radius_view() -> ChunkView {
    ChunkView {
        center: ChunkPos::new(0, 0),
        render_distance: 0,
        chunk_tracking_radius: 0,
    }
}

fn admit_player(server: &mut RealmServer, player_id: ServerPlayerId) -> Vec3d {
    let mut updates = server
        .try_handle_command_for_player(player_id, ClientCommand::SetChunkView(zero_radius_view()))
        .unwrap();
    for _ in 0..60_000 {
        if let Some(position_update) = updates.iter().find_map(|update| match update {
            ServerUpdate::PlayerPosition(update) => Some(*update),
            _ => None,
        }) {
            server
                .try_handle_command_for_player(
                    player_id,
                    ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                        id: position_update.teleport_id,
                    }),
                )
                .unwrap();
            return position_update.position;
        }
        server.try_tick_report_global().unwrap();
        updates.extend(server.try_drain_updates_for_player(player_id).unwrap());
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    panic!("timed out admitting player {player_id}");
}

fn wait_for_transfer_updates(
    server: &mut RealmServer,
    player_id: ServerPlayerId,
) -> Vec<ServerUpdate> {
    let mut updates = server.try_drain_updates_for_player(player_id).unwrap();
    for _ in 0..60_000 {
        let has_position = updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::PlayerPosition(_)));
        let has_snapshot = updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)));
        if has_position && has_snapshot {
            return updates;
        }
        server.try_tick_report_global().unwrap();
        updates.extend(server.try_drain_updates_for_player(player_id).unwrap());
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    panic!("timed out waiting for player {player_id} transfer");
}

fn transfer_position_update(updates: &[ServerUpdate]) -> mclone_protocol::PlayerPositionUpdate {
    updates
        .iter()
        .find_map(|update| match update {
            ServerUpdate::PlayerPosition(update) => Some(*update),
            _ => None,
        })
        .expect("transfer must finish with a teleport update")
}

fn assert_dimension_change_precedes_destination_replica(
    updates: &[ServerUpdate],
    destination: &DimensionKey,
) {
    let change_index = updates
        .iter()
        .position(|update| {
            matches!(
                update,
                ServerUpdate::DimensionChange {
                    dimension,
                    keep_player_state: true,
                    ..
                } if dimension == destination
            )
        })
        .expect("transfer must publish its destination boundary");
    let first_replica_index = updates
        .iter()
        .position(|update| {
            matches!(
                update,
                ServerUpdate::ChunkSnapshot(_)
                    | ServerUpdate::SectionBlockUpdates { .. }
                    | ServerUpdate::PlayerPosition(_)
                    | ServerUpdate::RemotePlayerAdd(_)
                    | ServerUpdate::RemotePlayerUpdate(_)
                    | ServerUpdate::EntitySnapshot(_)
                    | ServerUpdate::EntityUpdate(_)
            )
        })
        .expect("transfer must publish destination replica state");
    assert!(change_index < first_replica_index);
}

#[test]
fn player_transfers_a_to_b_to_a_with_one_realm_record_and_replica_reset() {
    let moon_record = moon_record();
    let moon = moon_record.key.clone();
    let mut server = RealmServer::with_world_store(12_345, Box::new(fixture_store(&moon)));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);
    server.register_dimension(moon_record).unwrap();
    let identity = ClientIdentity::new(PlayerProfileId::new([0x61; 16]), "Traveler").unwrap();
    let traveler = server.add_player_with_identity(identity).unwrap();
    let overworld_peer = server.add_player();
    let moon_peer = server.add_player_in_dimension(moon.clone()).unwrap();
    let overworld_position = admit_player(&mut server, traveler);
    admit_player(&mut server, overworld_peer);
    admit_player(&mut server, moon_peer);
    server.players.get_mut(traveler).unwrap().total_experience = 37;
    server
        .players
        .get_mut(traveler)
        .unwrap()
        .statistics
        .increment(StatisticKey::jump(), 4);
    server
        .players
        .get_mut(traveler)
        .unwrap()
        .statistics
        .increment(StatisticKey::successful_block_placement(), 9);

    server
        .try_handle_command_for_player(
            traveler,
            ClientCommand::move_player(MovePlayerCommand::PosRot {
                position: overworld_position.add(Vec3d::new(0.1, 0.0, 0.0)),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }),
        )
        .unwrap();
    let peer_updates = server.try_drain_updates_for_player(overworld_peer).unwrap();
    assert!(peer_updates.iter().any(|update| matches!(
        update,
        ServerUpdate::RemotePlayerAdd(remote) | ServerUpdate::RemotePlayerUpdate(remote)
            if remote.id == RemotePlayerId(traveler.as_u64())
    )));
    server.try_drain_updates_for_player(moon_peer).unwrap();

    assert!(
        server
            .transfer_player_dimension(
                traveler,
                moon.clone(),
                Vec3d::new(overworld_position.x, -100.0, overworld_position.z),
            )
            .unwrap()
    );
    assert_eq!(server.player_count(), 3);
    assert_eq!(server.player_dimension(traveler), Some(&moon));
    assert!(matches!(
        server.realm_interest_diagnostics().transfers[0].phase,
        PlayerDimensionTransferPhase::LoadingDestination
            | PlayerDimensionTransferPhase::AwaitingTeleportAck
    ));
    let moon_updates = wait_for_transfer_updates(&mut server, traveler);
    assert_dimension_change_precedes_destination_replica(&moon_updates, &moon);
    let moon_position = transfer_position_update(&moon_updates);
    assert!(moon_position.position.y > -64.0);
    assert!(moon_updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerExperience {
            total_experience: 37
        }
    )));
    assert!(moon_updates.iter().any(|update| matches!(
        update,
        ServerUpdate::PlayerStatistics { statistics }
            if statistics.jump_count() == 4
                && statistics.successful_block_placement_count() == 9
    )));
    assert_eq!(
        server.realm_interest_diagnostics().transfers[0].phase,
        PlayerDimensionTransferPhase::AwaitingTeleportAck
    );
    server
        .try_handle_command_for_player(
            traveler,
            ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: moon_position.teleport_id,
            }),
        )
        .unwrap();
    assert!(server.realm_interest_diagnostics().transfers.is_empty());
    assert_eq!(server.player_count(), 3);

    let source_peer_updates = server.try_drain_updates_for_player(overworld_peer).unwrap();
    assert!(source_peer_updates.iter().any(|update| matches!(
        update,
        ServerUpdate::RemotePlayerRemove { id }
            if *id == RemotePlayerId(traveler.as_u64())
    )));
    let destination_peer_updates = server.try_drain_updates_for_player(moon_peer).unwrap();
    assert!(remote_player_add(&destination_peer_updates, traveler).is_some());

    assert!(
        server
            .transfer_player_dimension(traveler, DimensionKey::overworld(), overworld_position,)
            .unwrap()
    );
    let overworld_updates = wait_for_transfer_updates(&mut server, traveler);
    assert_dimension_change_precedes_destination_replica(
        &overworld_updates,
        &DimensionKey::overworld(),
    );
    let returned_position = transfer_position_update(&overworld_updates);
    server
        .try_handle_command_for_player(
            traveler,
            ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: returned_position.teleport_id,
            }),
        )
        .unwrap();
    assert_eq!(server.player_count(), 3);
    assert_eq!(
        server.player_dimension(traveler),
        Some(&DimensionKey::overworld())
    );
    assert_eq!(server.players.get(traveler).unwrap().total_experience, 37);
    assert_eq!(server.player_statistics(traveler).unwrap().jump_count(), 4);
    assert_eq!(
        server
            .player_statistics(traveler)
            .unwrap()
            .successful_block_placement_count(),
        9
    );
    let record = player_record_from_entry(server.players.get_mut(traveler).unwrap()).unwrap();
    assert_eq!(record.dimension, DimensionKey::overworld());
    assert_eq!(record.total_experience, 37);
    assert_eq!(record.statistics.jump_count(), 4);
    assert_eq!(record.statistics.successful_block_placement_count(), 9);
}

#[test]
fn disconnect_during_transfer_cancels_pending_membership_cleanly() {
    let moon_record = moon_record();
    let moon = moon_record.key.clone();
    let mut server = RealmServer::with_world_store(12_345, Box::new(fixture_store(&moon)));
    server
        .set_world_generation_profile(WorldGenerationProfile::authored_only())
        .unwrap();
    server.set_lighting_enabled(false);
    server.register_dimension(moon_record).unwrap();
    let traveler = server.add_player();
    let moon_peer = server.add_player_in_dimension(moon.clone()).unwrap();
    let position = admit_player(&mut server, traveler);
    admit_player(&mut server, moon_peer);

    assert!(
        server
            .transfer_player_dimension(traveler, moon.clone(), position)
            .unwrap()
    );
    assert_eq!(server.realm_interest_diagnostics().transfers.len(), 1);
    assert!(server.remove_player(traveler));
    assert!(server.realm_interest_diagnostics().transfers.is_empty());
    assert_eq!(server.player_count(), 1);
    assert_eq!(server.player_dimension(moon_peer), Some(&moon));
}

#[test]
fn initial_world_info_names_a_non_overworld_join_dimension() {
    let moon_record = moon_record();
    let moon = moon_record.key.clone();
    let mut server = RealmServer::with_world_store(12_345, Box::new(fixture_store(&moon)));
    server.register_dimension(moon_record).unwrap();

    let player = server.add_player_in_dimension(moon.clone()).unwrap();
    let updates = server.try_drain_updates_for_player(player).unwrap();

    assert!(updates.iter().any(|update| matches!(
        update,
        ServerUpdate::WorldInfo { dimension, .. } if dimension == &moon
    )));
}
