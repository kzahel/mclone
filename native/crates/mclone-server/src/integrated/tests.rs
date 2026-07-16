use super::*;
use mclone_protocol::{PlayerAppearance, PlayerModelKind};
use std::collections::BTreeSet;

use crate::ChunkHolder;
use mclone_core::{
    BlockHitResult, BlockStateId, ChunkSnapshot, Direction, PackedLightSection, Vec3d,
    block_to_section_coord, local_block_coord, local_section_block_coord,
};
use mclone_light::LightLayer;
use mclone_protocol::{
    AcceptTeleportCommand, EntityId, EntityKind, EntitySnapshot, EntityUpdate, ItemKind,
    ItemStackSnapshot, PlayerPositionRelativeFlags, RemotePlayerId, RemotePlayerUpdate,
    ServerUpdate,
};
use mclone_worldgen::block::{
    AIR, BRICKS, DIRT, GRASS, OAK_LOG_X, OAK_LOG_Z, SAND, SNOW, STONE, TORCH, WALL_TORCH_EAST,
    WATER, generated_block_state_id, has_fluid, material_blocks_motion,
};

fn last_time_update(report: &ServerSimulationTickReport) -> u64 {
    report
        .updates
        .iter()
        .rev()
        .find_map(|update| match update {
            ServerUpdate::TimeUpdate { day_time, .. } => Some(*day_time),
            _ => None,
        })
        .expect("simulation tick should emit a TimeUpdate")
}

fn first_biome_zoom_seed(updates: &[ServerUpdate]) -> Option<i64> {
    updates.iter().find_map(|update| match update {
        ServerUpdate::WorldInfo { biome_zoom_seed } => Some(*biome_zoom_seed),
        _ => None,
    })
}

fn request_initial_chunk_view(server: &mut IntegratedServer) {
    server.set_lighting_enabled(false);
    let updates = server
        .try_handle_command(ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }))
        .expect("set chunk view");
    assert!(
        updates
            .iter()
            .all(|update| !matches!(update, ServerUpdate::PlayerPosition(_)))
    );
}

fn wait_for_initial_spawn_update(
    server: &mut IntegratedServer,
) -> mclone_protocol::PlayerPositionUpdate {
    let mut spawn = None;
    for _ in 0..60_000 {
        let updates = server.try_poll().expect("poll");
        spawn = spawn.or_else(|| {
            updates.iter().find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(*update),
                _ => None,
            })
        });
        if let Some(spawn) = spawn {
            return spawn;
        }
        if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    panic!("timed out waiting for initial spawn position update");
}

fn load_chunk_view_with_lighting(
    server: &mut IntegratedServer,
    center: ChunkPos,
    lighting_enabled: bool,
) {
    server.set_lighting_enabled(lighting_enabled);
    let updates = server
        .try_handle_command(ClientCommand::SetChunkView(ChunkView {
            center,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }))
        .expect("set chunk view");
    accept_player_position_updates(server, &updates);
    for _ in 0..60_000 {
        let updates = server.try_poll().expect("poll");
        accept_player_position_updates(server, &updates);
        if server.pending_job_count() == 0 {
            return;
        }
        if server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
            server.wait_for_light_completion(Duration::from_secs(1));
        }
    }
    panic!("timed out loading center chunk");
}

fn load_chunk_view(server: &mut IntegratedServer, center: ChunkPos) {
    load_chunk_view_with_lighting(server, center, false);
}

fn load_center_chunk(server: &mut IntegratedServer) {
    load_chunk_view(server, ChunkPos::new(0, 0));
}

#[cfg(feature = "physics-rapier")]
fn prepare_debug_physics_floor(server: &mut IntegratedServer) {
    load_center_chunk(server);

    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let block = if y == 0 { STONE } else { AIR };
                server
                    .scheduler_mut()
                    .set_block_at_world(BlockPos::new(x, y, z), block);
            }
        }
    }
    server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(8.0, 3.0, 6.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: false,
        }))
        .expect("move player before debug shot");
}

fn accept_player_position_updates(server: &mut IntegratedServer, updates: &[ServerUpdate]) {
    for update in updates {
        let ServerUpdate::PlayerPosition(update) = update else {
            continue;
        };
        let ack_updates = server
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: update.teleport_id,
            }))
            .expect("accept initial player position");
        assert!(ack_updates.is_empty());
    }
}

fn set_dedicated_chunk_view_and_poll(
    server: &mut IntegratedServer,
    player_id: ServerPlayerId,
    center: ChunkPos,
    radius: u32,
) -> Vec<ServerUpdate> {
    let mut updates = server
        .try_handle_command_for_player(
            player_id,
            ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance: radius,
                chunk_tracking_radius: radius,
            }),
        )
        .expect("set dedicated chunk view");
    accept_dedicated_player_position_updates(server, player_id, &updates);
    for _ in 0..60_000 {
        let polled = server.try_poll_for_player(player_id).expect("poll player");
        accept_dedicated_player_position_updates(server, player_id, &polled);
        updates.extend(polled);
        if server.pending_job_count() == 0 {
            return updates;
        }
        if server.pending_publication_count() == 0 {
            server.wait_for_worldgen_completion(Duration::from_secs(1));
        }
    }
    panic!("timed out loading dedicated player chunk view");
}

fn accept_dedicated_player_position_updates(
    server: &mut IntegratedServer,
    player_id: ServerPlayerId,
    updates: &[ServerUpdate],
) {
    for update in updates {
        let ServerUpdate::PlayerPosition(update) = update else {
            continue;
        };
        let ack_updates = server
            .try_handle_command_for_player(
                player_id,
                ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                    id: update.teleport_id,
                }),
            )
            .expect("accept dedicated player position");
        assert!(ack_updates.is_empty());
    }
}

fn snapshot_positions(updates: &[ServerUpdate]) -> BTreeSet<ChunkPos> {
    updates
        .iter()
        .filter_map(|update| match update {
            ServerUpdate::ChunkSnapshot(snapshot) => Some(snapshot.pos),
            _ => None,
        })
        .collect()
}

fn snapshot_update_for(updates: &[ServerUpdate], pos: ChunkPos) -> Option<&ChunkSnapshot> {
    updates.iter().rev().find_map(|update| match update {
        ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
        _ => None,
    })
}

fn sample_sky_light(sections: &[PackedLightSection], pos: BlockPos) -> u8 {
    let section_y = block_to_section_coord(pos.y);
    let Some(section) = sections
        .iter()
        .find(|section| section.section_y == section_y)
    else {
        return 0;
    };
    let Some(data_layer) =
        mclone_light::packed_light_section_layer(section, LightLayer::Sky).unwrap()
    else {
        return 0;
    };
    data_layer.get(
        local_block_coord(pos.x),
        local_section_block_coord(pos.y),
        local_block_coord(pos.z),
    )
}

fn has_chunk_unload(updates: &[ServerUpdate], pos: ChunkPos) -> bool {
    updates.iter().any(
        |update| matches!(update, ServerUpdate::ChunkUnload { pos: unloaded } if *unloaded == pos),
    )
}

fn has_section_block_updates(updates: &[ServerUpdate]) -> bool {
    updates
        .iter()
        .any(|update| matches!(update, ServerUpdate::SectionBlockUpdates { .. }))
}

fn has_section_block_update_with_state(updates: &[ServerUpdate], state: BlockStateId) -> bool {
    updates.iter().any(|update| match update {
        ServerUpdate::SectionBlockUpdates { updates, .. } => {
            updates.iter().any(|update| update.block_state == state)
        }
        _ => false,
    })
}

fn remote_player_add(
    updates: &[ServerUpdate],
    player_id: ServerPlayerId,
) -> Option<RemotePlayerUpdate> {
    updates.iter().find_map(|update| match update {
        ServerUpdate::RemotePlayerAdd(update)
            if update.id == RemotePlayerId(player_id.as_u64()) =>
        {
            Some(*update)
        }
        _ => None,
    })
}

fn remote_player_update(
    updates: &[ServerUpdate],
    player_id: ServerPlayerId,
) -> Option<RemotePlayerUpdate> {
    updates.iter().find_map(|update| match update {
        ServerUpdate::RemotePlayerUpdate(update)
            if update.id == RemotePlayerId(player_id.as_u64()) =>
        {
            Some(*update)
        }
        _ => None,
    })
}

fn has_remote_player_remove(updates: &[ServerUpdate], player_id: ServerPlayerId) -> bool {
    updates.iter().any(|update| {
        matches!(
            update,
            ServerUpdate::RemotePlayerRemove { id }
                if *id == RemotePlayerId(player_id.as_u64())
        )
    })
}

fn first_entity_snapshot(updates: &[ServerUpdate]) -> Option<EntitySnapshot> {
    updates.iter().find_map(|update| match update {
        ServerUpdate::EntitySnapshot(snapshot) => Some(*snapshot),
        _ => None,
    })
}

fn first_entity_snapshot_of_kind(
    updates: &[ServerUpdate],
    kind: EntityKind,
) -> Option<EntitySnapshot> {
    updates.iter().find_map(|update| match update {
        ServerUpdate::EntitySnapshot(snapshot) if snapshot.kind == kind => Some(*snapshot),
        _ => None,
    })
}

fn first_entity_update(updates: &[ServerUpdate], id: EntityId) -> Option<EntityUpdate> {
    updates.iter().find_map(|update| match update {
        ServerUpdate::EntityUpdate(update) if update.id == id => Some(*update),
        _ => None,
    })
}

fn has_entity_remove(updates: &[ServerUpdate], id: EntityId) -> bool {
    updates.iter().any(
        |update| matches!(update, ServerUpdate::EntityRemove { id: removed } if *removed == id),
    )
}

fn sync_player(server: &mut IntegratedServer, position: Vec3d) {
    let mut current = server.player.position();
    for _ in 0..64 {
        let delta = position.subtract(current);
        if delta.length_sqr() <= 64.0 {
            send_player_move(server, position);
            return;
        }
        let length = delta.length_sqr().sqrt();
        current = current.add(delta.scale(8.0 / length));
        send_player_move(server, current);
        server
            .try_simulation_tick_report()
            .expect("movement helper tick");
    }
    panic!("timed out walking test player to {position:?}");
}

fn send_player_move(server: &mut IntegratedServer, position: Vec3d) {
    let updates = server
        .try_handle_command(ClientCommand::move_player(MovePlayerCommand::PosRot {
            position,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))
        .expect("move player");
    assert!(updates.is_empty());
}

fn sync_carried_slot(server: &mut IntegratedServer, slot: u8) {
    let updates = server
        .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot,
        }))
        .expect("set carried item");
    assert!(updates.is_empty());
}

fn assign_debug_hotbar_slot(server: &mut IntegratedServer, slot: u8, block_state: BlockStateId) {
    let updates = server
        .try_handle_command(ClientCommand::SetDebugHotbarSlot(
            SetDebugHotbarSlotCommand {
                slot,
                block_state: Some(block_state),
            },
        ))
        .expect("set debug hotbar slot");
    assert!(updates.is_empty());
}

fn clear_debug_hotbar_slot(server: &mut IntegratedServer, slot: u8) {
    let updates = server
        .try_handle_command(ClientCommand::SetDebugHotbarSlot(
            SetDebugHotbarSlotCommand {
                slot,
                block_state: None,
            },
        ))
        .expect("clear debug hotbar slot");
    assert!(updates.is_empty());
}

fn use_held_item_on(hit: BlockHitResult) -> ClientCommand {
    ClientCommand::UseItemOn(UseItemOnCommand {
        hand: InteractionHand::MainHand,
        hit,
    })
}

mod chunk_view;
mod debug_interactions;
mod dedicated_players;
mod diagnostics;
mod entities;
mod player_state;
mod simulation_physics;
