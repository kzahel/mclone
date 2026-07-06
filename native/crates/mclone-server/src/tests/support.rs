pub(super) use crate::lighting_seed::{
    provisional_block_light_sections_for_chunk, provisional_sky_light_sections,
    provisional_sky_light_sections_for_chunk, snapshot_with_provisional_lighting,
};
pub(super) use crate::persistence::{
    EntityChunkRecord, EntityPersistentId, EntitySavePayload, EntitySaveRecord, ItemStackSaveRecord,
};
pub(super) use crate::scheduler::{
    DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET, DEFAULT_PENDING_UNLOAD_BUDGET,
};
pub(super) use crate::*;
pub(super) use mclone_core::{
    ChunkPos, ChunkRevision, ChunkStatus, PackedLightSection, Vec3d, block_to_section_coord,
    chunk_block_index, local_block_coord, local_section_block_coord,
};
pub(super) use mclone_light::LightLayer;
pub(super) use mclone_protocol::{AcceptTeleportCommand, ChunkView, ClientCommand, ServerUpdate};
pub(super) use mclone_worldgen::block::{
    AIR, LAVA, LAVA_LEVEL_2, LAVA_LEVEL_8, OBSIDIAN, STONE, TORCH, WATER, WATER_LEVEL_1,
    WATER_LEVEL_2, WATER_LEVEL_8, generated_block_state_id, lava_block_for_level,
    water_block_for_level,
};
pub(super) use serde_json::Value;
pub(super) use std::collections::{BTreeMap, BTreeSet};
pub(super) use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, Default)]
pub(super) struct SharedMemoryWorldStore {
    pub(super) chunks: Rc<RefCell<BTreeMap<ChunkPos, ChunkRecord>>>,
    pub(super) entity_chunks: Rc<RefCell<BTreeMap<ChunkPos, EntityChunkRecord>>>,
}

impl SharedMemoryWorldStore {
    pub(super) fn new() -> Self {
        Self::default()
    }
}

impl WorldStore for SharedMemoryWorldStore {
    fn supports_entity_chunks(&self) -> bool {
        true
    }

    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        Ok(self.chunks.borrow().get(&pos).cloned())
    }

    fn save_chunk(&mut self, record: &ChunkRecord) -> ChunkStoreResult<()> {
        self.chunks
            .borrow_mut()
            .insert(record.pos(), record.clone());
        Ok(())
    }

    fn load_entity_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        Ok(self.entity_chunks.borrow().get(&pos).cloned())
    }

    fn save_entity_chunk(&mut self, record: &EntityChunkRecord) -> ChunkStoreResult<()> {
        self.entity_chunks
            .borrow_mut()
            .insert(record.pos, record.clone());
        Ok(())
    }
}

pub(super) fn apply_interest_and_poll(
    scheduler: &mut ChunkScheduler,
    interest: ChunkView,
) -> Vec<ChunkSchedulerEvent> {
    let mut events = scheduler.apply_interest(interest).unwrap();
    events.extend(poll_scheduler_until_idle(scheduler));
    events
}

pub(super) fn poll_scheduler_until_idle(
    scheduler: &mut ChunkScheduler,
) -> Vec<ChunkSchedulerEvent> {
    let mut events = Vec::new();
    for _ in 0..60_000 {
        events.extend(scheduler.poll().unwrap());
        if scheduler.pending_job_count() == 0 {
            return events;
        }
        if scheduler.pending_publication_count() == 0 {
            wait_for_scheduler_completion(scheduler);
        }
    }
    panic!("timed out waiting for scheduler worldgen jobs");
}

pub(super) fn poll_scheduler_until_persistence_idle(scheduler: &mut ChunkScheduler) {
    for _ in 0..60_000 {
        if scheduler.pending_persistence_load_count() == 0
            && scheduler.pending_persistence_save_count() == 0
        {
            return;
        }
        scheduler.poll().unwrap();
    }
    panic!("timed out waiting for scheduler persistence actor");
}

pub(super) fn handle_command_and_poll(
    server: &mut IntegratedServer,
    command: ClientCommand,
) -> Vec<ServerUpdate> {
    let mut updates = server.handle_command(command);
    updates.extend(poll_server_until_idle(server));
    accept_and_remove_player_position_updates(server, &mut updates);
    updates
}

pub(super) fn try_handle_command_and_poll(
    server: &mut IntegratedServer,
    command: ClientCommand,
) -> ChunkStoreResult<Vec<ServerUpdate>> {
    let mut updates = server.try_handle_command(command)?;
    updates.extend(try_poll_server_until_idle(server)?);
    try_accept_and_remove_player_position_updates(server, &mut updates)?;
    Ok(updates)
}

pub(super) fn accept_and_remove_player_position_updates(
    server: &mut IntegratedServer,
    updates: &mut Vec<ServerUpdate>,
) {
    try_accept_and_remove_player_position_updates(server, updates)
        .expect("failed to accept player position updates");
}

pub(super) fn try_accept_and_remove_player_position_updates(
    server: &mut IntegratedServer,
    updates: &mut Vec<ServerUpdate>,
) -> ChunkStoreResult<()> {
    let mut index = 0;
    while index < updates.len() {
        let teleport_id = match &updates[index] {
            ServerUpdate::PlayerPosition(update) => update.teleport_id,
            _ => {
                index += 1;
                continue;
            }
        };
        let ack_updates =
            server.try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: teleport_id,
            }))?;
        assert!(ack_updates.is_empty());
        updates.remove(index);
    }
    Ok(())
}

pub(super) fn poll_server_until_idle(server: &mut IntegratedServer) -> Vec<ServerUpdate> {
    try_poll_server_until_idle(server).unwrap()
}

pub(super) fn try_poll_server_until_idle(
    server: &mut IntegratedServer,
) -> ChunkStoreResult<Vec<ServerUpdate>> {
    let mut updates = Vec::new();
    for _ in 0..60_000 {
        updates.extend(server.try_poll()?);
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if server.pending_publication_count() == 0 {
            wait_for_server_completion(server);
        }
    }
    panic!("timed out waiting for integrated server worldgen jobs");
}

pub(super) fn try_poll_server_until_persistence_idle(
    server: &mut IntegratedServer,
) -> ChunkStoreResult<()> {
    for _ in 0..60_000 {
        if server.scheduler().pending_persistence_load_count() == 0
            && server.scheduler().pending_persistence_save_count() == 0
        {
            return Ok(());
        }
        let _ = server.try_poll()?;
    }
    panic!("timed out waiting for integrated server persistence actor");
}

pub(super) fn fixture_i32(value: &Value, key: &str) -> i32 {
    value[key]
        .as_i64()
        .unwrap_or_else(|| panic!("fixture field `{key}` must be an integer")) as i32
}

pub(super) fn wait_for_scheduler_completion(scheduler: &mut ChunkScheduler) {
    let timeout = std::time::Duration::from_millis(1);
    let _ = scheduler.wait_for_worldgen_completion(timeout);
    let _ = scheduler.wait_for_light_completion(timeout);
}

pub(super) fn wait_for_server_completion(server: &mut IntegratedServer) {
    let timeout = std::time::Duration::from_millis(1);
    let _ = server.wait_for_worldgen_completion(timeout);
    let _ = server.wait_for_light_completion(timeout);
}

pub(super) fn active_ticket_square_count(ticket_level: i32) -> usize {
    let radius = usize::try_from(MAX_CHUNK_DISTANCE - ticket_level)
        .expect("ticket level must be within active distance");
    let side = radius * 2 + 1;
    side * side
}

pub(super) fn square_side_for_radius(radius: u32) -> usize {
    usize::try_from(radius).expect("chunk radius must fit usize") * 2 + 1
}

pub(super) fn player_status_counts(radius: u32) -> (usize, usize, usize, usize, usize) {
    let entity_ticking = square_side_for_radius(radius).pow(2);
    let block_ticking = square_side_for_radius(radius + 1).pow(2);
    let ticking = block_ticking - entity_ticking;
    let border_outer = square_side_for_radius(radius + 2).pow(2);
    let border = border_outer - block_ticking;
    let active = square_side_for_radius(
        radius + u32::try_from(MAX_CHUNK_DISTANCE - PLAYER_TICKET_LEVEL).unwrap(),
    )
    .pow(2);
    let inaccessible = active - border_outer;
    (inaccessible, border, ticking, entity_ticking, block_ticking)
}

pub(super) fn chunk_square(center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
    (-radius..=radius)
        .flat_map(|z| (-radius..=radius).map(move |x| ChunkPos::new(center.x + x, center.z + z)))
        .collect()
}

pub(super) fn without_fluid_tick_events(
    events: &[ChunkSchedulerEvent],
) -> Vec<ChunkSchedulerEvent> {
    events
        .iter()
        .filter(|event| !matches!(event, ChunkSchedulerEvent::FluidTickScheduled { .. }))
        .cloned()
        .collect()
}

pub(super) fn status_event_count(
    events: &[ChunkSchedulerEvent],
    status: ChunkStatus,
    step: ChunkStatusStep,
) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ChunkSchedulerEvent::StatusChanged {
                    status: event_status,
                    step: event_step,
                    ..
                } if *event_status == status && *event_step == step
            )
        })
        .count()
}

pub(super) fn first_status_event_pos(
    events: &[ChunkSchedulerEvent],
    status: ChunkStatus,
    step: ChunkStatusStep,
) -> Option<ChunkPos> {
    events.iter().find_map(|event| match event {
        ChunkSchedulerEvent::StatusChanged {
            pos,
            status: event_status,
            step: event_step,
        } if *event_status == status && *event_step == step => Some(*pos),
        _ => None,
    })
}

pub(super) fn snapshot_ready_count(events: &[ChunkSchedulerEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, ChunkSchedulerEvent::SnapshotReady(_)))
        .count()
}

pub(super) fn snapshot_update_for(
    updates: &[ServerUpdate],
    pos: ChunkPos,
) -> Option<&ChunkSnapshot> {
    updates.iter().rev().find_map(|update| match update {
        ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
        _ => None,
    })
}

pub(super) fn entity_snapshot_for(
    updates: &[ServerUpdate],
    kind: mclone_protocol::EntityKind,
) -> Option<&mclone_protocol::EntitySnapshot> {
    updates.iter().rev().find_map(|update| match update {
        ServerUpdate::EntitySnapshot(snapshot) if snapshot.kind == kind => Some(snapshot),
        _ => None,
    })
}

pub(super) fn stored_egg_item_entity_record(
    pos: ChunkPos,
    revision: u64,
    persistent_id_least: u64,
) -> EntityChunkRecord {
    EntityChunkRecord::new(
        pos,
        revision,
        vec![EntitySaveRecord {
            persistent_id: EntityPersistentId::new(0x100, persistent_id_least),
            kind: "minecraft:item".to_owned(),
            position: Vec3d::new(
                f64::from(pos.min_block_x()) + 8.0,
                65.0,
                f64::from(pos.min_block_z()) + 8.0,
            ),
            delta_movement: Vec3d::ZERO,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            age_ticks: 37,
            payload: EntitySavePayload::Item {
                stack: ItemStackSaveRecord::new("minecraft:egg", 3),
                pickup_delay: 20,
            },
        }],
    )
}

pub(super) fn apply_section_updates_to_snapshot(
    snapshot: &mut ChunkSnapshot,
    updates: &[ServerUpdate],
) -> usize {
    let mut applied = 0;
    for update in updates {
        let ServerUpdate::SectionBlockUpdates {
            pos,
            section_y,
            updates,
        } = update
        else {
            continue;
        };
        if *pos != snapshot.pos {
            continue;
        }
        applied += snapshot.patch_section_blocks(
            *section_y,
            updates.iter().map(|update| {
                (
                    update.local_x as i32,
                    update.local_y as i32,
                    update.local_z as i32,
                    update.block_state,
                )
            }),
        );
    }
    applied
}

pub(super) fn snapshot_block_state(snapshot: &ChunkSnapshot, pos: WorldBlockPos) -> BlockStateId {
    assert_eq!(snapshot.pos, pos.chunk_pos());
    assert!(
        pos.y >= snapshot.min_y && pos.y < snapshot.min_y + snapshot.height,
        "world y {} is outside snapshot range {}..{}",
        pos.y,
        snapshot.min_y,
        snapshot.min_y + snapshot.height
    );

    let section_y = block_to_section_coord(pos.y);
    let section_local_y = local_section_block_coord(pos.y);
    let local_x = local_block_coord(pos.x);
    let local_z = local_block_coord(pos.z);
    let Some(section) = snapshot
        .sections
        .iter()
        .find(|section| section.section_y == section_y)
    else {
        return BlockStateId(0);
    };
    let blocks = section.unpack_block_state_ids();
    blocks[chunk_section_index(local_x, section_local_y, local_z)]
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("mclone-{name}-{}-{nanos}", std::process::id()));
    path
}
