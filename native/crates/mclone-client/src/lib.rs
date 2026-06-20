#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};

mod actor;
mod block_clip;
mod block_shapes;
mod interaction;
mod inventory;
mod player;

use mclone_core::{
    BlockPos, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, SECTION_HEIGHT, local_block_coord,
};
use mclone_protocol::{
    ChunkView, ClientCommand, EntityId, EntitySnapshot, EntityUpdate, PlayerPositionUpdate,
    RemotePlayerId, RemotePlayerUpdate, SectionBlockUpdate, ServerUpdate,
};

pub use actor::{
    ActorInterpolationConfig, ActorInterpolationState, ActorPresentation, ActorPresentationId,
    ActorPresentationKind,
};
pub use interaction::{CREATIVE_PICK_RANGE, ClientInteractionController};
pub use inventory::ClientInventory;
pub use player::{
    CollisionMovementResult, LOCAL_PLAYER_AIR_SPEED, LOCAL_PLAYER_BASE_MOVEMENT_SPEED,
    LOCAL_PLAYER_GRAVITY, LOCAL_PLAYER_JUMP_POWER, LOCAL_PLAYER_STANDING_EYE_HEIGHT,
    LOCAL_PLAYER_STANDING_HEIGHT, LOCAL_PLAYER_STANDING_WIDTH, LOCAL_PLAYER_TICKS_PER_SECOND,
    LOCAL_PLAYER_VERTICAL_DRAG, LOCAL_PLAYER_X_ROT_LIMIT_DEGREES, LocalPlayerController,
    LocalPlayerPose, MOVING_SLOW_FACTOR, NO_CLIP_BOOST_MULTIPLIER, NoClipMovementStep, PlayerInput,
    PlayerInputKey, PlayerInputKeys, WalkingMovementResult, WalkingMovementStep, collide_movement,
    no_clip_displacement, view_vector, view_vector_from_rot_degrees,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientHost {
    LocalIntegrated,
    RemoteDedicated,
}

#[derive(Clone, Debug)]
pub struct ClientRuntime {
    host: ClientHost,
    chunk_view: Option<ChunkView>,
    chunks: BTreeMap<ChunkPos, ChunkSnapshot>,
    day_time: u64,
    player_position_updates: VecDeque<PlayerPositionUpdate>,
    remote_players: BTreeMap<RemotePlayerId, RemotePlayerUpdate>,
    entities: BTreeMap<EntityId, EntitySnapshot>,
}

impl ClientRuntime {
    pub fn new(host: ClientHost) -> Self {
        Self {
            host,
            chunk_view: None,
            chunks: BTreeMap::new(),
            day_time: 0,
            player_position_updates: VecDeque::new(),
            remote_players: BTreeMap::new(),
            entities: BTreeMap::new(),
        }
    }

    pub fn local_integrated() -> Self {
        Self::new(ClientHost::LocalIntegrated)
    }

    pub const fn host(&self) -> ClientHost {
        self.host
    }

    pub fn set_chunk_view(&mut self, view: ChunkView) -> ClientCommand {
        self.chunk_view = Some(view.clone());
        ClientCommand::SetChunkView(view)
    }

    pub fn chunk_view(&self) -> Option<&ChunkView> {
        self.chunk_view.as_ref()
    }

    pub fn apply_update(&mut self, update: ServerUpdate) {
        match update {
            ServerUpdate::ChunkSnapshot(snapshot) => {
                self.chunks.insert(snapshot.pos, snapshot);
            }
            ServerUpdate::ChunkUnload { pos } => {
                self.chunks.remove(&pos);
                self.remove_entities_in_chunk(pos);
            }
            ServerUpdate::SectionBlockUpdates {
                pos,
                section_y,
                updates,
            } => {
                self.apply_section_block_updates(pos, section_y, &updates);
            }
            ServerUpdate::TimeUpdate { day_time } => {
                self.day_time = day_time;
            }
            ServerUpdate::PlayerPosition(update) => {
                self.player_position_updates.push_back(update);
            }
            ServerUpdate::RemotePlayerAdd(update) | ServerUpdate::RemotePlayerUpdate(update) => {
                self.remote_players.insert(update.id, update);
            }
            ServerUpdate::RemotePlayerRemove { id } => {
                self.remote_players.remove(&id);
            }
            ServerUpdate::EntitySnapshot(snapshot) => {
                self.entities.insert(snapshot.id, snapshot);
            }
            ServerUpdate::EntityUpdate(update) => {
                self.apply_entity_update(update);
            }
            ServerUpdate::EntityRemove { id } => {
                self.entities.remove(&id);
            }
        }
    }

    pub fn apply_updates(&mut self, updates: impl IntoIterator<Item = ServerUpdate>) {
        for update in updates {
            self.apply_update(update);
        }
    }

    pub fn chunk_snapshot(&self, pos: ChunkPos) -> Option<&ChunkSnapshot> {
        self.chunks.get(&pos)
    }

    pub fn chunk_snapshots(&self) -> impl Iterator<Item = &ChunkSnapshot> {
        self.chunks.values()
    }

    pub fn packed_light_at_world_or_fullbright(&self, pos: BlockPos) -> u32 {
        let Some(snapshot) = self.chunks.get(&pos.chunk_pos()) else {
            return mclone_light::FULL_BRIGHT;
        };
        packed_light_from_snapshot_or_fullbright(snapshot, pos)
    }

    pub fn loaded_chunk_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.chunks.keys().copied()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn clear_server_replica(&mut self) {
        self.chunks.clear();
        self.player_position_updates.clear();
        self.remote_players.clear();
        self.entities.clear();
    }

    pub fn remote_player(&self, id: RemotePlayerId) -> Option<&RemotePlayerUpdate> {
        self.remote_players.get(&id)
    }

    pub fn remote_player_count(&self) -> usize {
        self.remote_players.len()
    }

    pub fn entity(&self, id: EntityId) -> Option<&EntitySnapshot> {
        self.entities.get(&id)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn entity_snapshots(&self) -> impl Iterator<Item = &EntitySnapshot> {
        self.entities.values()
    }

    pub fn actor_presentations(&self) -> Vec<ActorPresentation> {
        self.remote_players
            .values()
            .copied()
            .map(ActorPresentation::remote_player)
            .chain(
                self.entities
                    .values()
                    .copied()
                    .map(ActorPresentation::entity),
            )
            .collect()
    }

    pub fn drain_player_position_updates(
        &mut self,
    ) -> impl Iterator<Item = PlayerPositionUpdate> + '_ {
        self.player_position_updates.drain(..)
    }

    /// Latest authoritative world day-time (ticks) from the server.
    pub const fn day_time(&self) -> u64 {
        self.day_time
    }

    /// Celestial phase in `[0, 1)` for the current day-time. See
    /// [`mclone_core::time::time_of_day`].
    pub fn time_of_day(&self) -> f32 {
        mclone_core::time::time_of_day(self.day_time)
    }

    /// Celestial rig rotation in radians. See [`mclone_core::time::sun_angle`].
    pub fn sun_angle(&self) -> f32 {
        mclone_core::time::sun_angle(self.day_time)
    }

    pub fn apply_section_block_updates(
        &mut self,
        pos: ChunkPos,
        section_y: i32,
        updates: &[SectionBlockUpdate],
    ) -> bool {
        let Some(snapshot) = self.chunks.get_mut(&pos) else {
            return false;
        };
        let mut changed = false;
        for update in updates {
            if update.local_x as i32 >= CHUNK_WIDTH
                || update.local_y as i32 >= SECTION_HEIGHT
                || update.local_z as i32 >= CHUNK_WIDTH
            {
                continue;
            }
            changed |= snapshot.patch_section_block(
                section_y,
                update.local_x as i32,
                update.local_y as i32,
                update.local_z as i32,
                update.block_state,
            );
        }
        changed
    }

    fn apply_entity_update(&mut self, update: EntityUpdate) {
        let Some(snapshot) = self.entities.get_mut(&update.id) else {
            return;
        };
        snapshot.position = update.position;
        snapshot.y_rot_degrees = update.y_rot_degrees;
        snapshot.x_rot_degrees = update.x_rot_degrees;
        snapshot.on_ground = update.on_ground;
        snapshot.age_ticks = update.age_ticks;
    }

    fn remove_entities_in_chunk(&mut self, pos: ChunkPos) {
        self.entities
            .retain(|_, snapshot| BlockPos::containing(snapshot.position).chunk_pos() != pos);
    }
}

fn packed_light_from_snapshot_or_fullbright(snapshot: &ChunkSnapshot, pos: BlockPos) -> u32 {
    mclone_light::packed_light_at_local_block_or_fullbright(
        &snapshot.light_sections,
        snapshot.min_y,
        snapshot.height,
        local_block_coord(pos.x),
        pos.y,
        local_block_coord(pos.z),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus,
        LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection, chunk_section_index,
    };

    #[test]
    fn distinguishes_local_and_remote_hosts() {
        assert_ne!(ClientHost::LocalIntegrated, ClientHost::RemoteDedicated);
    }

    #[test]
    fn set_chunk_view_returns_protocol_command() {
        let mut runtime = ClientRuntime::local_integrated();
        let view = ChunkView {
            center: ChunkPos::new(2, -3),
            render_distance: 4,
            chunk_tracking_radius: 5,
        };

        assert_eq!(
            runtime.set_chunk_view(view.clone()),
            ClientCommand::SetChunkView(view.clone())
        );
        assert_eq!(runtime.chunk_view(), Some(&view));
    }

    #[test]
    fn client_runtime_hydrates_and_unloads_chunk_snapshots() {
        let mut runtime = ClientRuntime::local_integrated();
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );

        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot.clone()));

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), Some(&snapshot));

        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(0, 0),
        });

        assert_eq!(runtime.loaded_chunk_count(), 0);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), None);
    }

    #[test]
    fn client_runtime_samples_packed_light_from_loaded_chunk() {
        let mut runtime = ClientRuntime::local_integrated();
        let local_index = chunk_section_index(2, 4, 3);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                0,
                Some(light_layer_with_value(local_index, 12)),
                Some(light_layer_with_value(local_index, 5)),
            )],
        );

        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot));

        assert_eq!(
            runtime.packed_light_at_world_or_fullbright(BlockPos::new(2, 4, 3)),
            mclone_light::pack_light(5, 12)
        );
    }

    #[test]
    fn client_runtime_packed_light_falls_back_to_fullbright_without_light_payload() {
        let mut runtime = ClientRuntime::local_integrated();
        runtime.apply_update(ServerUpdate::ChunkSnapshot(
            ChunkSnapshot::from_block_state_ids(
                ChunkPos::new(0, 0),
                ChunkStatus::Surface,
                ChunkRevision(1),
                0,
                16,
                &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
            ),
        ));

        assert_eq!(
            runtime.packed_light_at_world_or_fullbright(BlockPos::new(2, 4, 3)),
            mclone_light::FULL_BRIGHT
        );
        assert_eq!(
            runtime.packed_light_at_world_or_fullbright(BlockPos::new(32, 4, 3)),
            mclone_light::FULL_BRIGHT
        );
    }

    #[test]
    fn client_runtime_clears_stale_server_replica_without_dropping_view() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let view = ChunkView {
            center: ChunkPos::new(2, -3),
            render_distance: 1,
            chunk_tracking_radius: 1,
        };
        runtime.set_chunk_view(view.clone());
        runtime.apply_update(ServerUpdate::ChunkSnapshot(
            ChunkSnapshot::from_block_state_ids(
                ChunkPos::new(2, -3),
                ChunkStatus::Surface,
                ChunkRevision(1),
                0,
                16,
                &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
            ),
        ));
        runtime.apply_update(ServerUpdate::PlayerPosition(PlayerPositionUpdate {
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: 10.0,
            relative: mclone_protocol::PlayerPositionRelativeFlags::ABSOLUTE,
            teleport_id: 7,
            dismount_vehicle: false,
        }));
        runtime.apply_update(ServerUpdate::RemotePlayerAdd(RemotePlayerUpdate {
            id: RemotePlayerId(42),
            position: mclone_core::Vec3d::new(4.0, 64.0, 5.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }));
        runtime.apply_update(ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(7),
            kind: mclone_protocol::EntityKind::Cow,
            position: mclone_core::Vec3d::new(4.0, 64.0, 5.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            age_ticks: 0,
        }));

        assert_eq!(
            runtime.loaded_chunk_positions().collect::<Vec<_>>(),
            vec![ChunkPos::new(2, -3)]
        );
        runtime.clear_server_replica();

        assert_eq!(runtime.host(), ClientHost::RemoteDedicated);
        assert_eq!(runtime.chunk_view(), Some(&view));
        assert_eq!(runtime.loaded_chunk_count(), 0);
        assert_eq!(runtime.remote_player_count(), 0);
        assert_eq!(runtime.entity_count(), 0);
        assert_eq!(runtime.drain_player_position_updates().count(), 0);
    }

    #[test]
    fn client_runtime_tracks_day_time_from_server() {
        let mut runtime = ClientRuntime::local_integrated();
        assert_eq!(runtime.day_time(), 0);

        runtime.apply_update(ServerUpdate::TimeUpdate { day_time: 6_000 });

        assert_eq!(runtime.day_time(), 6_000);
        // dayTime 6000 is noon, which the smoothed curve maps to phase ~0.0.
        assert!(runtime.time_of_day().abs() < 1e-4);
        assert!(runtime.sun_angle().abs() < 1e-3);
    }

    #[test]
    fn client_runtime_queues_player_position_updates_for_controller_ack() {
        let mut runtime = ClientRuntime::local_integrated();
        let update = PlayerPositionUpdate {
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: 10.0,
            relative: mclone_protocol::PlayerPositionRelativeFlags::ABSOLUTE,
            teleport_id: 7,
            dismount_vehicle: false,
        };

        runtime.apply_update(ServerUpdate::PlayerPosition(update));

        assert_eq!(
            runtime.drain_player_position_updates().collect::<Vec<_>>(),
            vec![update]
        );
        assert_eq!(
            runtime.drain_player_position_updates().collect::<Vec<_>>(),
            Vec::<PlayerPositionUpdate>::new()
        );
    }

    #[test]
    fn client_runtime_tracks_remote_player_lifecycle() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = RemotePlayerId(7);
        let initial = RemotePlayerUpdate {
            id,
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 5.0,
            on_ground: true,
        };
        let moved = RemotePlayerUpdate {
            id,
            position: mclone_core::Vec3d::new(3.0, 65.0, 4.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: -10.0,
            on_ground: false,
        };

        runtime.apply_update(ServerUpdate::RemotePlayerAdd(initial));
        assert_eq!(runtime.remote_player_count(), 1);
        assert_eq!(runtime.remote_player(id), Some(&initial));

        runtime.apply_update(ServerUpdate::RemotePlayerUpdate(moved));
        assert_eq!(runtime.remote_player_count(), 1);
        assert_eq!(runtime.remote_player(id), Some(&moved));

        runtime.apply_update(ServerUpdate::RemotePlayerRemove { id });
        assert_eq!(runtime.remote_player_count(), 0);
        assert_eq!(runtime.remote_player(id), None);
    }

    #[test]
    fn client_runtime_tracks_entity_lifecycle_and_unloads_by_chunk() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = EntityId(7);
        let initial = EntitySnapshot {
            id,
            kind: mclone_protocol::EntityKind::Cow,
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 5.0,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            age_ticks: 12,
        };
        let moved = EntityUpdate {
            id,
            position: mclone_core::Vec3d::new(3.0, 65.0, 4.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: -10.0,
            on_ground: false,
            age_ticks: 13,
        };

        runtime.apply_update(ServerUpdate::EntitySnapshot(initial));
        assert_eq!(runtime.entity_count(), 1);
        assert_eq!(runtime.entity(id), Some(&initial));

        runtime.apply_update(ServerUpdate::EntityUpdate(moved));
        assert_eq!(runtime.entity_count(), 1);
        let updated = runtime.entity(id).copied().unwrap();
        assert_eq!(updated.position, moved.position);
        assert_eq!(updated.y_rot_degrees, moved.y_rot_degrees);
        assert_eq!(updated.x_rot_degrees, moved.x_rot_degrees);
        assert_eq!(updated.on_ground, moved.on_ground);
        assert_eq!(updated.age_ticks, moved.age_ticks);
        assert_eq!(updated.kind, initial.kind);
        assert_eq!(updated.width, initial.width);
        assert_eq!(updated.height, initial.height);

        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(0, 0),
        });
        assert_eq!(runtime.entity_count(), 0);
        assert_eq!(runtime.entity(id), None);

        runtime.apply_update(ServerUpdate::EntitySnapshot(initial));
        runtime.apply_update(ServerUpdate::EntityRemove { id });
        assert_eq!(runtime.entity_count(), 0);
        assert_eq!(runtime.entity(id), None);
    }

    #[test]
    fn actor_presentations_expose_remote_player_snapshot() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let update = RemotePlayerUpdate {
            id: RemotePlayerId(3),
            position: mclone_core::Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 15.0,
            on_ground: true,
        };

        runtime.apply_update(ServerUpdate::RemotePlayerAdd(update));

        assert_eq!(
            runtime.actor_presentations(),
            vec![ActorPresentation {
                id: ActorPresentationId::RemotePlayer(update.id),
                kind: ActorPresentationKind::RemotePlayer,
                feet_position: update.position,
                y_rot_degrees: update.y_rot_degrees,
                x_rot_degrees: update.x_rot_degrees,
                on_ground: update.on_ground,
                width: 0.6,
                height: 1.8,
            }]
        );

        runtime.apply_update(ServerUpdate::RemotePlayerRemove { id: update.id });

        assert!(runtime.actor_presentations().is_empty());
    }

    #[test]
    fn actor_presentations_include_entity_snapshots() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let snapshot = EntitySnapshot {
            id: EntityId(11),
            kind: mclone_protocol::EntityKind::Cow,
            position: mclone_core::Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 0.0,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            age_ticks: 0,
        };

        runtime.apply_update(ServerUpdate::EntitySnapshot(snapshot));

        assert_eq!(
            runtime.actor_presentations(),
            vec![ActorPresentation {
                id: ActorPresentationId::Entity(snapshot.id),
                kind: ActorPresentationKind::Entity(snapshot.kind),
                feet_position: snapshot.position,
                y_rot_degrees: snapshot.y_rot_degrees,
                x_rot_degrees: snapshot.x_rot_degrees,
                on_ground: snapshot.on_ground,
                width: snapshot.width,
                height: snapshot.height,
            }]
        );
    }

    #[test]
    fn client_runtime_applies_section_block_updates_to_loaded_snapshot() {
        let mut runtime = ClientRuntime::local_integrated();
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );
        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot));

        runtime.apply_update(ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 0,
            updates: vec![SectionBlockUpdate {
                local_x: 1,
                local_y: 2,
                local_z: 3,
                block_state: BlockStateId(42),
            }],
        });

        let snapshot = runtime.chunk_snapshot(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(
            snapshot.sections[0].unpack_block_state_ids()[chunk_section_index(1, 2, 3)],
            BlockStateId(42)
        );
    }

    #[test]
    fn client_runtime_ignores_section_block_updates_for_unloaded_chunks() {
        let mut runtime = ClientRuntime::local_integrated();

        assert!(!runtime.apply_section_block_updates(
            ChunkPos::new(5, 6),
            0,
            &[SectionBlockUpdate {
                local_x: 1,
                local_y: 2,
                local_z: 3,
                block_state: BlockStateId(42),
            }],
        ));
        assert_eq!(runtime.loaded_chunk_count(), 0);
    }

    fn light_layer_with_value(index: usize, value: u8) -> Vec<u8> {
        let mut layer = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
        let byte_index = index >> 1;
        let shift = 4 * (index & 1);
        layer[byte_index] |= (value & 15) << shift;
        layer
    }
}
