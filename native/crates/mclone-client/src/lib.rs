#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod actor;
mod interaction;
mod inventory;
mod player;
mod remote_pose;
mod teleport;

pub mod block_facts {
    pub use mclone_blocks::{
        BlockFluidKind, LAVA_BLOCK_STATE_ID, WATER_BLOCK_STATE_ID, block_fluid_height,
        block_fluid_kind, is_fluid, terrain_id,
    };
}

mod block_shapes {
    pub(crate) use mclone_blocks::{
        block_collision_aabbs, block_outline_aabbs, clip_block_outline,
    };
}

use mclone_core::{
    BlockPos, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, HorizontalTopology, PackedChunkSection,
    PackedLightSection, SECTION_HEIGHT, local_block_coord, obfuscate_biome_zoom_seed,
};
use mclone_protocol::{
    BeeFieldGuideProgress, BeeSoundCue, ChunkView, ClientCommand, DeerFieldGuideProgress,
    DeerSoundCue, DimensionKey, DisconnectReason, DisconnectReasonCode, EntityId, EntitySnapshot,
    EntityUpdate, ItemStackSnapshot, MallardCallCue, MallardFieldGuideProgress,
    MallardNestSnapshotData, MallardSnapshotData, MallardTrackCue, PlayerLifeState,
    PlayerPositionUpdate, PlayerStatistics, RemotePlayerId, RemotePlayerUpdate, SectionBlockUpdate,
    ServerEphemeralMessage, ServerUpdate, SessionConfiguration, validate_body_pose_sample,
};

pub use actor::{
    ActorAppearance, ActorInterpolationConfig, ActorInterpolationState, ActorPresentation,
    ActorPresentationId, ActorPresentationKind,
};
pub use interaction::{
    BlockInteractionTarget, CREATIVE_PICK_RANGE, ClientInteractionController,
    EntityInteractionTarget,
};
pub use inventory::ClientInventory;
pub use player::{
    CollisionMovementResult, FlyingMovementStep, HAND_PUSH_DEFAULT_HAND_RADIUS,
    HAND_PUSH_DEFAULT_HEAD_RADIUS, HAND_PUSH_DEFAULT_JUMP_MULTIPLIER,
    HAND_PUSH_DEFAULT_MAX_ARM_LENGTH, HAND_PUSH_DEFAULT_MAX_JUMP_SPEED,
    HAND_PUSH_DEFAULT_UNSTICK_DISTANCE, HAND_PUSH_DEFAULT_VELOCITY_HISTORY_SIZE,
    HAND_PUSH_DEFAULT_VELOCITY_LIMIT, HandPushLocomotionController, HandPushLocomotionSettings,
    HandPushMovementResult, HandPushMovementStep, HandPushPose, LOCAL_PLAYER_AIR_SPEED,
    LOCAL_PLAYER_BASE_MOVEMENT_SPEED, LOCAL_PLAYER_GRAVITY, LOCAL_PLAYER_HAND_PUSH_EYE_HEIGHT,
    LOCAL_PLAYER_HAND_PUSH_HEIGHT, LOCAL_PLAYER_HAND_PUSH_WIDTH, LOCAL_PLAYER_JUMP_POWER,
    LOCAL_PLAYER_STANDING_EYE_HEIGHT, LOCAL_PLAYER_STANDING_HEIGHT, LOCAL_PLAYER_STANDING_WIDTH,
    LOCAL_PLAYER_TICKS_PER_SECOND, LOCAL_PLAYER_VERTICAL_DRAG, LOCAL_PLAYER_X_ROT_LIMIT_DEGREES,
    LocalPlayerController, LocalPlayerDimensions, LocalPlayerPose, MOVING_SLOW_FACTOR,
    NO_CLIP_BOOST_MULTIPLIER, NoClipMovementStep, PlayerInput, PlayerInputKey, PlayerInputKeys,
    THRUSTER_DRAG, THRUSTER_GRAVITY_SCALE, THRUSTER_GRAVITY_SI, THRUSTER_MAX_SPEED,
    THRUSTER_THRUST_SCALE, ThrusterHandInput, ThrusterMovementStep, ThrusterTuning,
    WalkingMovementResult, WalkingMovementStep, collide_movement, flying_displacement,
    no_clip_displacement, sphere_intersects_solid_blocks, thruster_acceleration,
    thruster_integrate, view_vector, view_vector_from_rot_degrees,
};
pub use remote_pose::RemotePoseTimelineDiagnostics;
#[cfg(not(target_arch = "wasm32"))]
pub use teleport::{
    NativeTeleportPreviewWorker, TeleportPreviewWorkerError, native_teleport_preview_capability,
};
pub use teleport::{
    StandingPoseFacts, TeleportCollisionSnapshot, TeleportCollisionSnapshotBounds,
    TeleportCollisionWorld, TeleportConfig, TeleportIntent, TeleportPreview,
    TeleportPreviewCapability, TeleportPreviewRequest, TeleportPreviewRequestId,
    TeleportPreviewResult, TeleportPreviewService, TeleportPreviewServiceError,
    TeleportResolverDiagnostics, TeleportValidityReason, resolve_teleport_preview,
    standing_pose_facts,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientHost {
    LocalIntegrated,
    RemoteDedicated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientSessionPhase {
    Connecting,
    Configuring,
    Playing,
    Disconnected,
}

/// Hard item bound for old chunk payloads awaiting destruction outside
/// immediate replica update application.
pub const DEFAULT_DEFERRED_CHUNK_DROP_MAX_ITEMS: usize = 4_096;

#[derive(Clone, Debug)]
pub struct ClientRuntime {
    host: ClientHost,
    session_phase: ClientSessionPhase,
    session_configuration: Option<SessionConfiguration>,
    disconnect_reason: Option<DisconnectReason>,
    current_dimension: DimensionKey,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
    chunk_view: Option<ChunkView>,
    chunks: BTreeMap<ChunkPos, ChunkSnapshot>,
    loaded_chunk_generation: u64,
    deferred_chunk_drops: VecDeque<ChunkSnapshot>,
    deferred_chunk_drop_items: usize,
    deferred_chunk_drop_inline_fallback_items: usize,
    game_time: u64,
    day_time: u64,
    daylight_cycle_running: bool,
    total_experience: u64,
    player_statistics: PlayerStatistics,
    player_inventory: [Option<ItemStackSnapshot>; mclone_protocol::HOTBAR_SLOT_COUNT_USIZE],
    mallard_field_guide: MallardFieldGuideProgress,
    deer_field_guide: DeerFieldGuideProgress,
    bee_field_guide: BeeFieldGuideProgress,
    mallard_calls: VecDeque<MallardCallCue>,
    deer_sounds: VecDeque<DeerSoundCue>,
    bee_sounds: VecDeque<BeeSoundCue>,
    mallard_tracks: VecDeque<MallardTrackCue>,
    player_life: PlayerLifeState,
    player_position_updates: VecDeque<PlayerPositionUpdate>,
    remote_players: BTreeMap<RemotePlayerId, RemotePlayerUpdate>,
    remote_player_walk_distances: BTreeMap<RemotePlayerId, f32>,
    remote_player_pose_timelines: BTreeMap<RemotePlayerId, remote_pose::RemotePlayerPoseTimeline>,
    entities: BTreeMap<EntityId, EntitySnapshot>,
    entity_seam_crossings: BTreeMap<EntityId, u64>,
    entity_chunks: BTreeMap<EntityId, ChunkPos>,
    entities_by_chunk: BTreeMap<ChunkPos, BTreeSet<EntityId>>,
}

impl ClientRuntime {
    pub fn new(host: ClientHost) -> Self {
        Self {
            host,
            session_phase: ClientSessionPhase::Connecting,
            session_configuration: None,
            disconnect_reason: None,
            current_dimension: DimensionKey::overworld(),
            biome_zoom_seed: None,
            topology: HorizontalTopology::UNBOUNDED,
            chunk_view: None,
            chunks: BTreeMap::new(),
            loaded_chunk_generation: 0,
            deferred_chunk_drops: VecDeque::new(),
            deferred_chunk_drop_items: 0,
            deferred_chunk_drop_inline_fallback_items: 0,
            game_time: 0,
            day_time: 0,
            daylight_cycle_running: true,
            total_experience: 0,
            player_statistics: PlayerStatistics::default(),
            player_inventory: [None; mclone_protocol::HOTBAR_SLOT_COUNT_USIZE],
            mallard_field_guide: MallardFieldGuideProgress::default(),
            deer_field_guide: DeerFieldGuideProgress::default(),
            bee_field_guide: BeeFieldGuideProgress::default(),
            mallard_calls: VecDeque::new(),
            deer_sounds: VecDeque::new(),
            bee_sounds: VecDeque::new(),
            mallard_tracks: VecDeque::new(),
            player_life: PlayerLifeState::default(),
            player_position_updates: VecDeque::new(),
            remote_players: BTreeMap::new(),
            remote_player_walk_distances: BTreeMap::new(),
            remote_player_pose_timelines: BTreeMap::new(),
            entities: BTreeMap::new(),
            entity_seam_crossings: BTreeMap::new(),
            entity_chunks: BTreeMap::new(),
            entities_by_chunk: BTreeMap::new(),
        }
    }

    pub fn local_integrated() -> Self {
        Self::new(ClientHost::LocalIntegrated)
    }

    pub fn local_integrated_with_seed(seed: i64) -> Self {
        Self::new(ClientHost::LocalIntegrated).with_biome_zoom_seed(obfuscate_biome_zoom_seed(seed))
    }

    pub fn with_biome_zoom_seed(mut self, seed: i64) -> Self {
        self.biome_zoom_seed = Some(seed);
        self
    }

    pub const fn host(&self) -> ClientHost {
        self.host
    }

    pub const fn session_phase(&self) -> ClientSessionPhase {
        self.session_phase
    }

    pub const fn session_configuration(&self) -> Option<SessionConfiguration> {
        self.session_configuration
    }

    pub fn disconnect_reason(&self) -> Option<&DisconnectReason> {
        self.disconnect_reason.as_ref()
    }

    pub const fn biome_zoom_seed(&self) -> Option<i64> {
        self.biome_zoom_seed
    }

    pub fn current_dimension(&self) -> &DimensionKey {
        &self.current_dimension
    }

    pub const fn topology(&self) -> HorizontalTopology {
        self.topology
    }

    pub fn set_chunk_view(&mut self, view: ChunkView) -> ClientCommand {
        self.chunk_view = Some(view.clone());
        ClientCommand::SetChunkView(view)
    }

    pub fn chunk_view(&self) -> Option<&ChunkView> {
        self.chunk_view.as_ref()
    }

    pub fn apply_update(&mut self, update: ServerUpdate) {
        self.apply_update_at(update, 0);
    }

    pub fn apply_update_at(&mut self, update: ServerUpdate, arrival_time_millis: u64) {
        match update {
            ServerUpdate::SessionConfiguration(configuration) => {
                if self.session_phase != ClientSessionPhase::Disconnected {
                    self.session_configuration = Some(configuration);
                    self.session_phase = ClientSessionPhase::Configuring;
                }
            }
            ServerUpdate::SessionReady => {
                if self.session_phase != ClientSessionPhase::Disconnected {
                    if self.session_configuration.is_some() {
                        self.session_phase = ClientSessionPhase::Playing;
                    } else {
                        self.apply_disconnect(DisconnectReason::new(
                            DisconnectReasonCode::ProtocolViolation,
                            "server marked the session ready before configuration",
                        ));
                    }
                }
            }
            ServerUpdate::WorldInfo {
                dimension,
                biome_zoom_seed,
                topology,
            } => {
                self.current_dimension = dimension;
                self.biome_zoom_seed = Some(biome_zoom_seed);
                self.topology = topology;
            }
            ServerUpdate::DimensionChange {
                dimension,
                biome_zoom_seed,
                topology,
                keep_player_state,
            } => {
                self.clear_server_replica();
                self.current_dimension = dimension;
                self.biome_zoom_seed = Some(biome_zoom_seed);
                self.topology = topology;
                if !keep_player_state {
                    self.total_experience = 0;
                    self.player_statistics = PlayerStatistics::default();
                    self.player_life = PlayerLifeState::default();
                }
            }
            ServerUpdate::ChunkSnapshot(snapshot) => {
                if let Some(previous) = self.chunks.insert(snapshot.pos, snapshot) {
                    self.defer_chunk_snapshot_drop(previous);
                }
                self.loaded_chunk_generation = self.loaded_chunk_generation.wrapping_add(1);
            }
            ServerUpdate::ChunkUnload { pos } => {
                if let Some(snapshot) = self.chunks.remove(&pos) {
                    self.defer_chunk_snapshot_drop(snapshot);
                    self.loaded_chunk_generation = self.loaded_chunk_generation.wrapping_add(1);
                }
                self.remove_entities_in_chunk(pos);
            }
            ServerUpdate::SectionBlockUpdates {
                pos,
                section_y,
                updates,
            } => {
                self.apply_section_block_updates(pos, section_y, &updates);
            }
            ServerUpdate::TimeUpdate {
                game_time,
                day_time,
                daylight_cycle_running,
            } => {
                self.game_time = game_time;
                self.day_time = day_time;
                self.daylight_cycle_running = daylight_cycle_running;
            }
            ServerUpdate::PlayerPosition(update) => {
                self.player_position_updates.push_back(update);
            }
            ServerUpdate::RemotePlayerAdd(update) => {
                self.remote_player_walk_distances
                    .entry(update.id)
                    .or_default();
                self.remote_player_pose_timelines
                    .insert(update.id, remote_pose::RemotePlayerPoseTimeline::default());
                self.remote_players.insert(update.id, update);
            }
            ServerUpdate::RemotePlayerUpdate(update) => {
                if let Some(previous) = self.remote_players.get(&update.id) {
                    let distance =
                        remote_player_horizontal_distance(self.topology, previous, &update);
                    *self
                        .remote_player_walk_distances
                        .entry(update.id)
                        .or_default() += distance;
                }
                self.remote_players.insert(update.id, update);
            }
            ServerUpdate::RemotePlayerRemove { id } => {
                self.remote_players.remove(&id);
                self.remote_player_walk_distances.remove(&id);
                self.remote_player_pose_timelines.remove(&id);
            }
            ServerUpdate::EntitySnapshot(snapshot) => {
                self.insert_entity_snapshot(snapshot);
            }
            ServerUpdate::EntityUpdate(update) => {
                self.apply_entity_update(update);
            }
            ServerUpdate::EntityRemove { id } => {
                self.remove_entity(id);
            }
            ServerUpdate::PlayerExperience { total_experience } => {
                self.total_experience = total_experience;
            }
            ServerUpdate::PlayerStatistics { statistics } => {
                self.player_statistics = statistics;
            }
            ServerUpdate::PlayerInventory { hotbar } => {
                self.player_inventory = hotbar;
            }
            ServerUpdate::MallardFieldGuide(progress) => {
                self.mallard_field_guide = progress;
            }
            ServerUpdate::DeerFieldGuide(progress) => {
                self.deer_field_guide = progress;
            }
            ServerUpdate::BeeFieldGuide(progress) => {
                self.bee_field_guide = progress;
            }
            ServerUpdate::BeeSound(cue) => self.bee_sounds.push_back(cue),
            ServerUpdate::DeerSound(cue) => self.deer_sounds.push_back(cue),
            ServerUpdate::MallardCall(cue) => self.mallard_calls.push_back(cue),
            ServerUpdate::MallardTrack(cue) => self.mallard_tracks.push_back(cue),
            ServerUpdate::PlayerLife(state) => {
                if state.epoch() >= self.player_life.epoch() {
                    self.player_life = state;
                }
            }
            ServerUpdate::EphemeralFallback(message) => {
                self.apply_ephemeral_message_at(message, arrival_time_millis);
            }
            ServerUpdate::KeepAlive { .. } => {}
            ServerUpdate::Disconnect(reason) => self.apply_disconnect(reason),
        }
    }

    fn apply_disconnect(&mut self, reason: DisconnectReason) {
        if self.disconnect_reason.is_none() {
            self.disconnect_reason = Some(reason);
            self.session_phase = ClientSessionPhase::Disconnected;
        }
    }

    pub fn apply_ephemeral_message(&mut self, message: ServerEphemeralMessage) -> bool {
        let arrival_time_millis = match message {
            ServerEphemeralMessage::RemoteBodyPose(sample) => {
                u64::from(sample.pose.sample_time_millis)
            }
        };
        self.apply_ephemeral_message_at(message, arrival_time_millis)
    }

    pub fn apply_ephemeral_message_at(
        &mut self,
        message: ServerEphemeralMessage,
        arrival_time_millis: u64,
    ) -> bool {
        match message {
            ServerEphemeralMessage::RemoteBodyPose(sample) => {
                if validate_body_pose_sample(sample.pose).is_err() {
                    return false;
                }
                let Some(previous) = self.remote_players.get(&sample.id).copied() else {
                    return false;
                };
                let timeline = self
                    .remote_player_pose_timelines
                    .entry(sample.id)
                    .or_default();
                if timeline.push(sample.pose, arrival_time_millis)
                    != remote_pose::RemotePosePushResult::Accepted
                {
                    return false;
                }

                let update = RemotePlayerUpdate {
                    position: sample.pose.position,
                    y_rot_degrees: sample.pose.y_rot_degrees,
                    x_rot_degrees: sample.pose.x_rot_degrees,
                    on_ground: sample.pose.on_ground,
                    ..previous
                };
                let distance = remote_player_horizontal_distance(self.topology, &previous, &update);
                *self
                    .remote_player_walk_distances
                    .entry(sample.id)
                    .or_default() += distance;
                self.remote_players.insert(sample.id, update);
                true
            }
        }
    }

    pub fn apply_updates(&mut self, updates: impl IntoIterator<Item = ServerUpdate>) {
        self.apply_updates_at(updates, 0);
    }

    pub fn apply_updates_at(
        &mut self,
        updates: impl IntoIterator<Item = ServerUpdate>,
        arrival_time_millis: u64,
    ) {
        for update in updates {
            self.apply_update_at(update, arrival_time_millis);
        }
    }

    pub const fn total_experience(&self) -> u64 {
        self.total_experience
    }

    pub fn player_statistics(&self) -> &PlayerStatistics {
        &self.player_statistics
    }

    pub const fn player_inventory(
        &self,
    ) -> &[Option<ItemStackSnapshot>; mclone_protocol::HOTBAR_SLOT_COUNT_USIZE] {
        &self.player_inventory
    }

    pub const fn mallard_field_guide(&self) -> MallardFieldGuideProgress {
        self.mallard_field_guide
    }

    pub const fn deer_field_guide(&self) -> DeerFieldGuideProgress {
        self.deer_field_guide
    }

    pub const fn bee_field_guide(&self) -> BeeFieldGuideProgress {
        self.bee_field_guide
    }

    pub fn drain_mallard_calls(&mut self) -> impl Iterator<Item = MallardCallCue> + '_ {
        self.mallard_calls.drain(..)
    }

    pub fn drain_deer_sounds(&mut self) -> impl Iterator<Item = DeerSoundCue> + '_ {
        self.deer_sounds.drain(..)
    }

    pub fn drain_bee_sounds(&mut self) -> impl Iterator<Item = BeeSoundCue> + '_ {
        self.bee_sounds.drain(..)
    }

    pub fn drain_mallard_tracks(&mut self) -> impl Iterator<Item = MallardTrackCue> + '_ {
        self.mallard_tracks.drain(..)
    }

    pub const fn player_life(&self) -> PlayerLifeState {
        self.player_life
    }

    pub const fn player_vitals(&self) -> mclone_protocol::PlayerVitals {
        self.player_life.vitals()
    }

    pub const fn player_death_cause(&self) -> Option<mclone_protocol::PlayerDamageCause> {
        self.player_life.death_cause()
    }

    pub fn player_is_dead(&self) -> bool {
        self.player_life.is_dead()
    }

    pub fn chunk_snapshot(&self, pos: ChunkPos) -> Option<&ChunkSnapshot> {
        self.topology
            .canonicalize_chunk(pos)
            .and_then(|canonical| self.chunks.get(&canonical))
    }

    pub fn chunk_snapshots(&self) -> impl Iterator<Item = &ChunkSnapshot> {
        self.chunks.values()
    }

    pub fn packed_light_at_world_or_fullbright(&self, pos: BlockPos) -> u32 {
        let Some(pos) = self.topology.canonicalize_block(pos) else {
            return mclone_light::FULL_BRIGHT;
        };
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

    pub const fn loaded_chunk_generation(&self) -> u64 {
        self.loaded_chunk_generation
    }

    pub fn deferred_chunk_drop_item_count(&self) -> usize {
        self.deferred_chunk_drop_items
    }

    pub fn deferred_chunk_drop_inline_fallback_item_count(&self) -> usize {
        self.deferred_chunk_drop_inline_fallback_items
    }

    pub fn drain_deferred_chunk_drop_items(&mut self, budget: usize) -> usize {
        let mut drained = 0;
        while drained < budget && self.drain_deferred_chunk_drop_item() {
            drained += 1;
        }
        drained
    }

    pub fn drain_deferred_chunk_drop_item(&mut self) -> bool {
        self.drain_one_deferred_chunk_drop_item()
    }

    pub fn take_deferred_chunk_drop_snapshot(&mut self) -> Option<(ChunkSnapshot, usize)> {
        let snapshot = self.deferred_chunk_drops.pop_front()?;
        let item_count = chunk_snapshot_drop_item_count(&snapshot);
        debug_assert!(item_count > 0);
        debug_assert!(self.deferred_chunk_drop_items >= item_count);
        self.deferred_chunk_drop_items -= item_count;
        Some((snapshot, item_count))
    }

    pub fn clear_server_replica(&mut self) {
        if !self.chunks.is_empty() {
            self.chunks.clear();
            self.loaded_chunk_generation = self.loaded_chunk_generation.wrapping_add(1);
        }
        self.deferred_chunk_drops.clear();
        self.deferred_chunk_drop_items = 0;
        self.deferred_chunk_drop_inline_fallback_items = 0;
        self.player_position_updates.clear();
        self.remote_players.clear();
        self.remote_player_walk_distances.clear();
        self.remote_player_pose_timelines.clear();
        self.entities.clear();
        self.entity_seam_crossings.clear();
        self.entity_chunks.clear();
        self.entities_by_chunk.clear();
    }

    pub fn remote_player(&self, id: RemotePlayerId) -> Option<&RemotePlayerUpdate> {
        self.remote_players.get(&id)
    }

    pub fn remote_player_count(&self) -> usize {
        self.remote_players.len()
    }

    pub fn remote_player_snapshots(&self) -> impl Iterator<Item = &RemotePlayerUpdate> {
        self.remote_players.values()
    }

    pub fn entity(&self, id: EntityId) -> Option<&EntitySnapshot> {
        self.entities.get(&id)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn entity_seam_crossing_count(&self, id: EntityId) -> u64 {
        self.entity_seam_crossings
            .get(&id)
            .copied()
            .unwrap_or_default()
    }

    pub fn entity_snapshots(&self) -> impl Iterator<Item = &EntitySnapshot> {
        self.entities.values()
    }

    pub fn actor_presentations(&self) -> Vec<ActorPresentation> {
        self.actor_presentations_at(0)
    }

    pub fn actor_presentations_at(&self, now_millis: u64) -> Vec<ActorPresentation> {
        let report_rate_hz = self.session_configuration.map_or(20, |configuration| {
            configuration.remote_pose_replication_rate_hz
        });
        self.remote_players
            .values()
            .copied()
            .map(|update| {
                let update = self
                    .remote_player_pose_timelines
                    .get(&update.id)
                    .and_then(|timeline| timeline.sample(self.topology, now_millis, report_rate_hz))
                    .map_or(update, |pose| RemotePlayerUpdate {
                        position: pose.position,
                        y_rot_degrees: pose.y_rot_degrees,
                        x_rot_degrees: pose.x_rot_degrees,
                        on_ground: pose.on_ground,
                        ..update
                    });
                ActorPresentation::remote_player(
                    update,
                    self.remote_player_walk_distances
                        .get(&update.id)
                        .copied()
                        .unwrap_or_default(),
                )
            })
            .chain(
                self.entities
                    .values()
                    .copied()
                    .map(|snapshot| ActorPresentation::entity_at(snapshot, self.game_time)),
            )
            .collect()
    }

    pub fn remote_pose_timeline_diagnostics(
        &self,
        id: RemotePlayerId,
        now_millis: u64,
    ) -> Option<RemotePoseTimelineDiagnostics> {
        let report_rate_hz = self.session_configuration.map_or(20, |configuration| {
            configuration.remote_pose_replication_rate_hz
        });
        self.remote_player_pose_timelines
            .get(&id)
            .map(|timeline| timeline.diagnostics(now_millis, report_rate_hz))
    }

    fn defer_chunk_snapshot_drop(&mut self, snapshot: ChunkSnapshot) {
        let item_count = chunk_snapshot_drop_item_count(&snapshot);
        if item_count == 0 {
            return;
        }
        let Some(next_items) = self.deferred_chunk_drop_items.checked_add(item_count) else {
            self.deferred_chunk_drop_inline_fallback_items = self
                .deferred_chunk_drop_inline_fallback_items
                .saturating_add(item_count);
            drop(snapshot);
            return;
        };
        if next_items > DEFAULT_DEFERRED_CHUNK_DROP_MAX_ITEMS {
            self.deferred_chunk_drop_inline_fallback_items = self
                .deferred_chunk_drop_inline_fallback_items
                .saturating_add(item_count);
            drop(snapshot);
            return;
        }
        self.deferred_chunk_drop_items = next_items;
        self.deferred_chunk_drops.push_back(snapshot);
    }

    fn drain_one_deferred_chunk_drop_item(&mut self) -> bool {
        loop {
            let Some(mut snapshot) = self.deferred_chunk_drops.pop_front() else {
                return false;
            };

            loop {
                if let Some(light_section) = snapshot.light_sections.last_mut() {
                    if let Some(sky_light) = light_section.sky.take() {
                        if chunk_snapshot_has_drop_items(&snapshot) {
                            self.deferred_chunk_drops.push_front(snapshot);
                        }
                        drop(sky_light);
                        self.deferred_chunk_drop_items -= 1;
                        return true;
                    }

                    if let Some(block_light) = light_section.block.take() {
                        if chunk_snapshot_has_drop_items(&snapshot) {
                            self.deferred_chunk_drops.push_front(snapshot);
                        }
                        drop(block_light);
                        self.deferred_chunk_drop_items -= 1;
                        return true;
                    }

                    snapshot.light_sections.pop();
                    continue;
                }

                if let Some(section) = snapshot.sections.last_mut() {
                    if !section.packed_block_indices.is_empty() {
                        let packed_block_indices =
                            std::mem::take(&mut section.packed_block_indices);
                        if chunk_snapshot_has_drop_items(&snapshot) {
                            self.deferred_chunk_drops.push_front(snapshot);
                        }
                        drop(packed_block_indices);
                        self.deferred_chunk_drop_items -= 1;
                        return true;
                    }

                    if !section.palette_state_ids.is_empty() {
                        let palette_state_ids = std::mem::take(&mut section.palette_state_ids);
                        if chunk_snapshot_has_drop_items(&snapshot) {
                            self.deferred_chunk_drops.push_front(snapshot);
                        }
                        drop(palette_state_ids);
                        self.deferred_chunk_drop_items -= 1;
                        return true;
                    }

                    snapshot.sections.pop();
                    continue;
                }

                if !snapshot.biomes.is_empty() {
                    let biomes = std::mem::take(&mut snapshot.biomes);
                    if chunk_snapshot_has_drop_items(&snapshot) {
                        self.deferred_chunk_drops.push_front(snapshot);
                    }
                    drop(biomes);
                    self.deferred_chunk_drop_items -= 1;
                    return true;
                }

                break;
            }
        }
    }

    pub fn drain_player_position_updates(
        &mut self,
    ) -> impl Iterator<Item = PlayerPositionUpdate> + '_ {
        self.player_position_updates.drain(..)
    }

    /// Locally advanced vanilla `gameTime`, corrected by periodic server
    /// samples.
    pub const fn game_time(&self) -> u64 {
        self.game_time
    }

    /// Locally advanced vanilla `dayTime`, corrected by periodic server
    /// samples.
    pub const fn day_time(&self) -> u64 {
        self.day_time
    }

    pub const fn daylight_cycle_running(&self) -> bool {
        self.daylight_cycle_running
    }

    /// Advance one vanilla client tick between authoritative clock samples.
    pub fn advance_time_tick(&mut self) {
        self.game_time = self.game_time.wrapping_add(1);
        if self.daylight_cycle_running {
            self.day_time = self.day_time.wrapping_add(1);
        }
    }

    /// Debug presentation hook that does not change the other clock state.
    pub fn force_day_time(&mut self, day_time: u64) {
        self.day_time = day_time;
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
        let changed = snapshot.patch_section_blocks(
            section_y,
            updates.iter().filter_map(|update| {
                if update.local_x as i32 >= CHUNK_WIDTH
                    || update.local_y as i32 >= SECTION_HEIGHT
                    || update.local_z as i32 >= CHUNK_WIDTH
                {
                    return None;
                }
                Some((
                    update.local_x as i32,
                    update.local_y as i32,
                    update.local_z as i32,
                    update.block_state,
                ))
            }),
        );
        if changed == 0 {
            return false;
        }
        // Section deltas mutate the resident snapshot without replacing it.
        // Advance its content revision so browser render-worker mirrors, and
        // any future revision-keyed snapshot consumers, receive an upsert
        // instead of rebuilding from their stale copy.
        snapshot.revision.0 = snapshot.revision.0.wrapping_add(1);
        true
    }

    fn apply_entity_update(&mut self, update: EntityUpdate) {
        let new_chunk = {
            let Some(snapshot) = self.entities.get_mut(&update.id) else {
                return;
            };
            let raw_displacement = update.position.subtract(snapshot.position);
            let shortest = self
                .topology
                .shortest_position_displacement(snapshot.position, update.position);
            if (raw_displacement.x - shortest.x).abs() > 1.0e-9
                || (raw_displacement.z - shortest.z).abs() > 1.0e-9
            {
                *self.entity_seam_crossings.entry(update.id).or_default() += 1;
            }
            if let Some(stack) = update.item_stack {
                snapshot.item_stack = Some(stack);
            }
            if let Some(mallard) = update.mallard {
                snapshot.mallard = Some(MallardSnapshotData {
                    life_stage: mallard.life_stage,
                    in_water: mallard.in_water,
                });
            }
            if let Some(nest) = update.mallard_nest {
                snapshot.mallard_nest = Some(MallardNestSnapshotData {
                    incubation_progress: nest.incubation_progress,
                    incubation_required: nest.incubation_required,
                    attended: nest.attended,
                });
            }
            if let Some(deer) = update.deer {
                snapshot.deer = Some(deer);
            }
            snapshot.animation = update.animation;
            snapshot.position = update.position;
            snapshot.y_rot_degrees = update.y_rot_degrees;
            snapshot.x_rot_degrees = update.x_rot_degrees;
            snapshot.rotation = update.rotation;
            snapshot.on_ground = update.on_ground;
            snapshot.tick_count = update.tick_count;
            entity_chunk_pos(snapshot)
        };
        self.set_entity_chunk(update.id, new_chunk);
    }

    fn remove_entities_in_chunk(&mut self, pos: ChunkPos) {
        let Some(ids) = self.entities_by_chunk.remove(&pos) else {
            return;
        };
        for id in ids {
            self.entities.remove(&id);
            self.entity_seam_crossings.remove(&id);
            self.entity_chunks.remove(&id);
        }
    }

    fn insert_entity_snapshot(&mut self, snapshot: EntitySnapshot) {
        let id = snapshot.id;
        let chunk = entity_chunk_pos(&snapshot);
        self.entities.insert(id, snapshot);
        self.entity_seam_crossings.entry(id).or_default();
        self.set_entity_chunk(id, chunk);
    }

    fn remove_entity(&mut self, id: EntityId) {
        self.entities.remove(&id);
        self.entity_seam_crossings.remove(&id);
        self.remove_entity_chunk_index(id);
    }

    fn set_entity_chunk(&mut self, id: EntityId, chunk: ChunkPos) {
        if self.entity_chunks.get(&id) == Some(&chunk) {
            self.entities_by_chunk.entry(chunk).or_default().insert(id);
            return;
        }

        self.remove_entity_chunk_index(id);
        self.entity_chunks.insert(id, chunk);
        self.entities_by_chunk.entry(chunk).or_default().insert(id);
    }

    fn remove_entity_chunk_index(&mut self, id: EntityId) {
        let Some(chunk) = self.entity_chunks.remove(&id) else {
            return;
        };
        let mut remove_chunk_entry = false;
        if let Some(ids) = self.entities_by_chunk.get_mut(&chunk) {
            ids.remove(&id);
            remove_chunk_entry = ids.is_empty();
        }
        if remove_chunk_entry {
            self.entities_by_chunk.remove(&chunk);
        }
    }
}

fn entity_chunk_pos(snapshot: &EntitySnapshot) -> ChunkPos {
    BlockPos::containing(snapshot.position).chunk_pos()
}

fn chunk_snapshot_has_drop_items(snapshot: &ChunkSnapshot) -> bool {
    chunk_snapshot_drop_item_count(snapshot) > 0
}

fn chunk_snapshot_drop_item_count(snapshot: &ChunkSnapshot) -> usize {
    usize::from(!snapshot.biomes.is_empty())
        + snapshot
            .sections
            .iter()
            .map(packed_chunk_section_drop_item_count)
            .sum::<usize>()
        + snapshot
            .light_sections
            .iter()
            .map(packed_light_section_drop_item_count)
            .sum::<usize>()
}

fn packed_chunk_section_drop_item_count(section: &PackedChunkSection) -> usize {
    usize::from(!section.palette_state_ids.is_empty())
        + usize::from(!section.packed_block_indices.is_empty())
}

fn packed_light_section_drop_item_count(section: &PackedLightSection) -> usize {
    usize::from(section.sky.is_some()) + usize::from(section.block.is_some())
}

fn remote_player_horizontal_distance(
    topology: HorizontalTopology,
    previous: &RemotePlayerUpdate,
    next: &RemotePlayerUpdate,
) -> f32 {
    if !previous.on_ground || !next.on_ground {
        return 0.0;
    }
    let displacement = topology.shortest_position_displacement(previous.position, next.position);
    let dx = displacement.x;
    let dz = displacement.z;
    (dx.mul_add(dx, dz * dz).sqrt()) as f32
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
        LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection, Vec3d, chunk_section_index,
    };
    use mclone_protocol::{PlayerAppearance, PlayerModelKind};

    #[test]
    fn client_runtime_enters_play_only_after_configuration_and_ready() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let configuration = SessionConfiguration::fixed_vanilla(
            11,
            11,
            mclone_protocol::SessionCapabilities::DEBUG_ACTIONS,
        );

        assert_eq!(runtime.session_phase(), ClientSessionPhase::Connecting);
        runtime.apply_update(ServerUpdate::SessionConfiguration(configuration));
        assert_eq!(runtime.session_phase(), ClientSessionPhase::Configuring);
        assert_eq!(runtime.session_configuration(), Some(configuration));

        runtime.apply_update(ServerUpdate::SessionReady);
        assert_eq!(runtime.session_phase(), ClientSessionPhase::Playing);
    }

    #[test]
    fn client_runtime_keeps_first_disconnect_reason() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let first = DisconnectReason::timeout("keepalive expired");
        runtime.apply_update(ServerUpdate::Disconnect(first.clone()));
        runtime.apply_update(ServerUpdate::Disconnect(DisconnectReason::transport_error(
            "socket closed",
        )));

        assert_eq!(runtime.session_phase(), ClientSessionPhase::Disconnected);
        assert_eq!(runtime.disconnect_reason(), Some(&first));
    }

    #[test]
    fn client_runtime_rejects_ready_before_configuration() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);

        runtime.apply_update(ServerUpdate::SessionReady);

        assert_eq!(runtime.session_phase(), ClientSessionPhase::Disconnected);
        assert_eq!(
            runtime.disconnect_reason().map(|reason| reason.code),
            Some(DisconnectReasonCode::ProtocolViolation)
        );
    }

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
    fn client_runtime_tracks_dimension_and_biome_zoom_seed_from_world_info() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        assert_eq!(runtime.biome_zoom_seed(), None);

        let moon = DimensionKey::parse("mclone:moon").unwrap();
        runtime.apply_update(ServerUpdate::WorldInfo {
            dimension: moon.clone(),
            biome_zoom_seed: -99,
            topology: HorizontalTopology::cylinder_x(0, 32),
        });

        assert_eq!(runtime.current_dimension(), &moon);
        assert_eq!(runtime.biome_zoom_seed(), Some(-99));
        assert_eq!(runtime.topology(), HorizontalTopology::cylinder_x(0, 32));
    }

    #[test]
    fn local_integrated_client_derives_biome_zoom_seed_from_world_seed() {
        let runtime = ClientRuntime::local_integrated_with_seed(1124);

        assert_eq!(
            runtime.biome_zoom_seed(),
            Some(obfuscate_biome_zoom_seed(1124))
        );
    }

    #[test]
    fn client_runtime_hydrates_and_unloads_chunk_snapshots() {
        let mut runtime = ClientRuntime::local_integrated();
        let mut blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        blocks[0] = BlockStateId(1);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &blocks,
        );

        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot.clone()));

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), Some(&snapshot));

        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(0, 0),
        });

        assert_eq!(runtime.loaded_chunk_count(), 0);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), None);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 2);
        assert_eq!(runtime.drain_deferred_chunk_drop_items(0), 0);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 2);
        assert_eq!(runtime.drain_deferred_chunk_drop_items(1), 1);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 1);
        assert_eq!(runtime.drain_deferred_chunk_drop_items(1), 1);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 0);
    }

    #[test]
    fn client_runtime_bounds_deferred_chunk_payload_ownership() {
        let mut runtime = ClientRuntime::local_integrated();
        let mut blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        blocks[0] = BlockStateId(1);
        let template = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &blocks,
        );
        let snapshot_count = DEFAULT_DEFERRED_CHUNK_DROP_MAX_ITEMS / 2 + 1;

        for chunk_x in 0..snapshot_count {
            let mut snapshot = template.clone();
            snapshot.pos = ChunkPos::new(chunk_x as i32, 0);
            runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot));
            runtime.apply_update(ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(chunk_x as i32, 0),
            });
        }

        assert_eq!(
            runtime.deferred_chunk_drop_item_count(),
            DEFAULT_DEFERRED_CHUNK_DROP_MAX_ITEMS
        );
        assert_eq!(runtime.deferred_chunk_drop_inline_fallback_item_count(), 2);
        assert_eq!(runtime.loaded_chunk_count(), 0);
    }

    #[test]
    fn periodic_client_replica_resolves_lifts_to_one_canonical_snapshot() {
        let mut runtime = ClientRuntime::local_integrated();
        runtime.apply_update(ServerUpdate::WorldInfo {
            dimension: DimensionKey::overworld(),
            biome_zoom_seed: 12_345,
            topology: HorizontalTopology::cylinder_x(0, 32),
        });
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(31, 0),
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );
        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot.clone()));

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(
            runtime.chunk_snapshot(ChunkPos::new(-1, 0)),
            Some(&snapshot)
        );
        assert_eq!(
            runtime.chunk_snapshot(ChunkPos::new(31, 0)),
            Some(&snapshot)
        );
    }

    #[test]
    fn client_runtime_defers_replaced_chunk_snapshot_drop() {
        let mut runtime = ClientRuntime::local_integrated();
        let mut first_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        first_blocks[0] = BlockStateId(1);
        let mut second_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        second_blocks[0] = BlockStateId(2);
        let first = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &first_blocks,
        );
        let second = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(2),
            0,
            16,
            &second_blocks,
        );

        runtime.apply_update(ServerUpdate::ChunkSnapshot(first));
        runtime.apply_update(ServerUpdate::ChunkSnapshot(second.clone()));

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), Some(&second));
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 2);
        assert_eq!(runtime.drain_deferred_chunk_drop_items(usize::MAX), 2);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 0);
    }

    #[test]
    fn client_runtime_takes_deferred_chunk_snapshot_for_external_drop() {
        let mut runtime = ClientRuntime::local_integrated();
        let mut blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        blocks[0] = BlockStateId(1);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &blocks,
        );

        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot));
        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(0, 0),
        });

        assert_eq!(runtime.deferred_chunk_drop_item_count(), 2);
        let (snapshot, item_count) = runtime
            .take_deferred_chunk_drop_snapshot()
            .expect("deferred snapshot should be available for external drop");
        assert_eq!(snapshot.pos, ChunkPos::new(0, 0));
        assert_eq!(item_count, 2);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 0);
        assert!(runtime.take_deferred_chunk_drop_snapshot().is_none());
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
    fn dimension_change_replaces_replica_without_dropping_view_or_player_state() {
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
            last_applied_move_sequence: 6,
            teleport_id: 7,
            dismount_vehicle: false,
            reset_continuity: true,
        }));
        runtime.apply_update(ServerUpdate::RemotePlayerAdd(RemotePlayerUpdate {
            id: RemotePlayerId(42),
            appearance: PlayerAppearance::default(),
            position: mclone_core::Vec3d::new(4.0, 64.0, 5.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }));
        runtime.apply_update(ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: EntityId(7),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, 7),
            kind: mclone_protocol::EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(4.0, 64.0, 5.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 0,
        }));
        runtime.apply_update(ServerUpdate::PlayerExperience {
            total_experience: 37,
        });
        let mut statistics = PlayerStatistics::default();
        statistics.set(mclone_protocol::StatisticKey::jump(), 11);
        runtime.apply_update(ServerUpdate::PlayerStatistics { statistics });

        assert_eq!(
            runtime.loaded_chunk_positions().collect::<Vec<_>>(),
            vec![ChunkPos::new(2, -3)]
        );
        let moon = DimensionKey::parse("mclone:moon").unwrap();
        runtime.apply_update(ServerUpdate::DimensionChange {
            dimension: moon.clone(),
            biome_zoom_seed: 987_654,
            topology: HorizontalTopology::UNBOUNDED,
            keep_player_state: true,
        });

        assert_eq!(runtime.host(), ClientHost::RemoteDedicated);
        assert_eq!(runtime.current_dimension(), &moon);
        assert_eq!(runtime.biome_zoom_seed(), Some(987_654));
        assert_eq!(runtime.chunk_view(), Some(&view));
        assert_eq!(runtime.total_experience(), 37);
        assert_eq!(runtime.player_statistics().jump_count(), 11);
        assert_eq!(runtime.loaded_chunk_count(), 0);
        assert_eq!(runtime.deferred_chunk_drop_item_count(), 0);
        assert_eq!(runtime.remote_player_count(), 0);
        assert_eq!(runtime.entity_count(), 0);
        assert_eq!(runtime.drain_player_position_updates().count(), 0);
    }

    #[test]
    fn client_runtime_tracks_day_time_from_server() {
        let mut runtime = ClientRuntime::local_integrated();
        assert_eq!(runtime.day_time(), 0);

        runtime.apply_update(ServerUpdate::TimeUpdate {
            game_time: 12_000,
            day_time: 6_000,
            daylight_cycle_running: true,
        });

        assert_eq!(runtime.game_time(), 12_000);
        assert_eq!(runtime.day_time(), 6_000);
        // dayTime 6000 is noon, which the smoothed curve maps to phase ~0.0.
        assert!(runtime.time_of_day().abs() < 1e-4);
        assert!(runtime.sun_angle().abs() < 1e-3);
    }

    #[test]
    fn client_runtime_advances_clocks_between_server_samples() {
        let mut runtime = ClientRuntime::local_integrated();
        runtime.apply_update(ServerUpdate::TimeUpdate {
            game_time: 50,
            day_time: 600,
            daylight_cycle_running: false,
        });

        runtime.advance_time_tick();
        assert_eq!(runtime.game_time(), 51);
        assert_eq!(runtime.day_time(), 600);

        runtime.apply_update(ServerUpdate::TimeUpdate {
            game_time: 80,
            day_time: 900,
            daylight_cycle_running: true,
        });
        runtime.advance_time_tick();
        assert_eq!(runtime.game_time(), 81);
        assert_eq!(runtime.day_time(), 901);
    }

    #[test]
    fn client_runtime_tracks_authoritative_player_experience() {
        let mut runtime = ClientRuntime::local_integrated();

        runtime.apply_update(ServerUpdate::PlayerExperience {
            total_experience: 37,
        });

        assert_eq!(runtime.total_experience(), 37);
    }

    #[test]
    fn client_runtime_tracks_authoritative_player_statistics() {
        let mut runtime = ClientRuntime::local_integrated();
        let mut statistics = PlayerStatistics::default();
        statistics.set(mclone_protocol::StatisticKey::jump(), 37);
        statistics.set(
            mclone_protocol::StatisticKey::successful_block_placement(),
            12,
        );

        runtime.apply_update(ServerUpdate::PlayerStatistics {
            statistics: statistics.clone(),
        });

        assert_eq!(runtime.player_statistics(), &statistics);
    }

    #[test]
    fn client_runtime_applies_atomic_life_and_rejects_stale_epochs() {
        let mut runtime = ClientRuntime::local_integrated();
        let dead = mclone_protocol::PlayerLifeState::new(
            3,
            mclone_protocol::PlayerVitals::new(0.0, 20.0).unwrap(),
            Some(mclone_protocol::PlayerDamageCause::Lava),
        )
        .unwrap();
        runtime.apply_update(ServerUpdate::PlayerLife(dead));

        assert!(runtime.player_is_dead());
        assert_eq!(
            runtime.player_death_cause(),
            Some(mclone_protocol::PlayerDamageCause::Lava)
        );

        runtime.apply_update(ServerUpdate::PlayerLife(
            mclone_protocol::PlayerLifeState::new(
                2,
                mclone_protocol::PlayerVitals::full_health(),
                None,
            )
            .unwrap(),
        ));
        assert!(runtime.player_is_dead());

        runtime.apply_update(ServerUpdate::PlayerLife(
            mclone_protocol::PlayerLifeState::new(
                4,
                mclone_protocol::PlayerVitals::full_health(),
                None,
            )
            .unwrap(),
        ));
        assert!(!runtime.player_is_dead());
        assert_eq!(runtime.player_vitals().health(), 20.0);
    }

    #[test]
    fn client_runtime_queues_player_position_updates_for_controller_ack() {
        let mut runtime = ClientRuntime::local_integrated();
        let update = PlayerPositionUpdate {
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: 10.0,
            relative: mclone_protocol::PlayerPositionRelativeFlags::ABSOLUTE,
            last_applied_move_sequence: 6,
            teleport_id: 7,
            dismount_vehicle: false,
            reset_continuity: false,
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
            appearance: PlayerAppearance::default(),
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 5.0,
            on_ground: true,
        };
        let moved = RemotePlayerUpdate {
            id,
            appearance: PlayerAppearance::default(),
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
    fn client_runtime_rejects_stale_ephemeral_remote_poses() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = RemotePlayerId(7);
        let initial = RemotePlayerUpdate {
            id,
            appearance: PlayerAppearance::default(),
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        };
        runtime.apply_update(ServerUpdate::RemotePlayerAdd(initial));
        let message = |epoch, sequence, x| {
            ServerEphemeralMessage::RemoteBodyPose(mclone_protocol::RemotePlayerBodyPoseSample {
                id,
                pose: mclone_protocol::PlayerBodyPoseSample::new(
                    epoch,
                    sequence,
                    sequence,
                    mclone_core::Vec3d::new(x, 64.0, 2.0),
                    0.0,
                    0.0,
                    true,
                ),
            })
        };

        assert!(runtime.apply_ephemeral_message(message(1, 5, 5.0)));
        assert!(!runtime.apply_ephemeral_message(message(1, 4, 4.0)));
        assert!(!runtime.apply_ephemeral_message(message(1, 5, 6.0)));
        assert!(runtime.apply_ephemeral_message(message(2, 1, 8.0)));
        assert!(!runtime.apply_ephemeral_message(message(1, 6, 6.0)));
        assert_eq!(
            runtime.remote_player(id).unwrap().position,
            mclone_core::Vec3d::new(8.0, 64.0, 2.0)
        );
    }

    #[test]
    fn actor_presentation_samples_the_delayed_remote_pose_timeline() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = RemotePlayerId(7);
        let initial = RemotePlayerUpdate {
            id,
            appearance: PlayerAppearance::default(),
            position: mclone_core::Vec3d::new(0.0, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        };
        runtime.apply_update(ServerUpdate::RemotePlayerAdd(initial));
        let message = |sequence, sample_time_millis, x| {
            ServerEphemeralMessage::RemoteBodyPose(mclone_protocol::RemotePlayerBodyPoseSample {
                id,
                pose: mclone_protocol::PlayerBodyPoseSample::new(
                    1,
                    sequence,
                    sample_time_millis,
                    mclone_core::Vec3d::new(x, 64.0, 2.0),
                    x as f32 * 10.0,
                    0.0,
                    true,
                ),
            })
        };
        assert!(runtime.apply_ephemeral_message_at(message(1, 1_000, 0.0), 10_000));
        assert!(runtime.apply_ephemeral_message_at(message(2, 1_050, 10.0), 10_050));

        assert_eq!(runtime.remote_player(id).unwrap().position.x, 10.0);
        let presentation = runtime.actor_presentations_at(10_125).remove(0);
        assert!((presentation.feet_position.x - 5.0).abs() < 1.0e-9);
        assert!((presentation.y_rot_degrees - 50.0).abs() < 1.0e-6);
        let diagnostics = runtime
            .remote_pose_timeline_diagnostics(id, 10_125)
            .unwrap();
        assert_eq!(diagnostics.accepted_samples, 2);
        assert_eq!(diagnostics.interpolation_delay_millis, 100);
    }

    #[test]
    fn client_runtime_accumulates_remote_player_walk_distance() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = RemotePlayerId(7);
        let initial = RemotePlayerUpdate {
            id,
            appearance: PlayerAppearance::default(),
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        };
        let moved = RemotePlayerUpdate {
            position: mclone_core::Vec3d::new(4.0, 64.0, 6.0),
            ..initial
        };

        runtime.apply_update(ServerUpdate::RemotePlayerAdd(initial));
        runtime.apply_update(ServerUpdate::RemotePlayerUpdate(moved));

        let presentation = runtime.actor_presentations().remove(0);
        assert!((presentation.walk_animation_distance - 5.0).abs() < 1.0e-6);

        runtime.apply_update(ServerUpdate::RemotePlayerRemove { id });
        runtime.apply_update(ServerUpdate::RemotePlayerAdd(initial));
        let presentation = runtime.actor_presentations().remove(0);
        assert_eq!(presentation.walk_animation_distance, 0.0);
    }

    #[test]
    fn periodic_remote_player_walk_distance_uses_the_shortest_seam_displacement() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        runtime.apply_update(ServerUpdate::WorldInfo {
            dimension: DimensionKey::overworld(),
            biome_zoom_seed: 12_345,
            topology: HorizontalTopology::cylinder_x(0, 32),
        });
        let initial = RemotePlayerUpdate {
            id: RemotePlayerId(7),
            appearance: PlayerAppearance::default(),
            position: Vec3d::new(511.75, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        };
        runtime.apply_update(ServerUpdate::RemotePlayerAdd(initial));
        runtime.apply_update(ServerUpdate::RemotePlayerUpdate(RemotePlayerUpdate {
            position: Vec3d::new(0.25, 64.0, 2.0),
            ..initial
        }));

        let presentation = runtime.actor_presentations().remove(0);

        assert!((presentation.walk_animation_distance - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn periodic_entity_replica_counts_canonical_seam_crossings() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        runtime.apply_update(ServerUpdate::WorldInfo {
            dimension: DimensionKey::overworld(),
            biome_zoom_seed: 12_345,
            topology: HorizontalTopology::cylinder_x(0, 32),
        });
        let id = EntityId(7);
        runtime.apply_update(ServerUpdate::EntitySnapshot(EntitySnapshot {
            id,
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id.0),
            kind: mclone_protocol::EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: Vec3d::new(511.75, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 0,
        }));
        runtime.apply_update(ServerUpdate::EntityUpdate(EntityUpdate {
            id,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: Vec3d::new(0.25, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            tick_count: 1,
        }));

        assert_eq!(runtime.entity_seam_crossing_count(id), 1);
        assert_eq!(runtime.entity(id).unwrap().position.x, 0.25);
    }

    #[test]
    fn client_runtime_tracks_entity_lifecycle_and_unloads_by_chunk() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = EntityId(7);
        let initial = EntitySnapshot {
            id,
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id.0),
            kind: mclone_protocol::EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 5.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 12,
        };
        let moved = EntityUpdate {
            id,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(17.0, 65.0, 4.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: -10.0,
            rotation: Some(mclone_protocol::EntityRotation {
                x: 0.0,
                y: 0.0,
                z: 0.382_683_43,
                w: 0.923_879_5,
            }),
            on_ground: false,
            tick_count: 13,
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
        assert_eq!(updated.rotation, moved.rotation);
        assert_eq!(updated.on_ground, moved.on_ground);
        assert_eq!(updated.tick_count, moved.tick_count);
        assert_eq!(updated.kind, initial.kind);
        assert_eq!(updated.persistent_id, initial.persistent_id);
        assert_eq!(updated.item_stack, initial.item_stack);
        assert_eq!(updated.width, initial.width);
        assert_eq!(updated.height, initial.height);

        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(0, 0),
        });
        assert_eq!(runtime.entity_count(), 1);
        assert_eq!(
            runtime.entity(id).map(|snapshot| snapshot.position),
            Some(moved.position)
        );

        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(1, 0),
        });
        assert_eq!(runtime.entity_count(), 0);
        assert_eq!(runtime.entity(id), None);

        runtime.apply_update(ServerUpdate::EntitySnapshot(initial));
        runtime.apply_update(ServerUpdate::EntityRemove { id });
        assert_eq!(runtime.entity_count(), 0);
        assert_eq!(runtime.entity(id), None);
    }

    #[test]
    fn client_runtime_applies_mallard_ecology_updates() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = EntityId(8);
        runtime.apply_update(ServerUpdate::EntitySnapshot(EntitySnapshot {
            id,
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id.0),
            kind: mclone_protocol::EntityKind::Mallard,
            item_stack: None,
            mallard: Some(mclone_protocol::MallardSnapshotData {
                life_stage: mclone_protocol::MallardLifeStage::Adult,
                in_water: false,
            }),
            mallard_nest: None,
            deer: None,
            animation: None,
            position: Vec3d::new(1.5, 64.0, 2.5),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.7,
            height: 0.75,
            tick_count: 1,
        }));

        runtime.apply_update(ServerUpdate::EntityUpdate(EntityUpdate {
            id,
            item_stack: None,
            mallard: Some(mclone_protocol::MallardUpdateData {
                life_stage: mclone_protocol::MallardLifeStage::Duckling,
                in_water: true,
            }),
            mallard_nest: None,
            deer: None,
            animation: None,
            position: Vec3d::new(1.75, 64.88, 2.5),
            y_rot_degrees: 12.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: false,
            tick_count: 2,
        }));

        let updated = runtime.entity(id).expect("updated mallard replica");
        assert_eq!(
            updated.mallard,
            Some(mclone_protocol::MallardSnapshotData {
                life_stage: mclone_protocol::MallardLifeStage::Duckling,
                in_water: true,
            })
        );
        let presentation = runtime.actor_presentations().remove(0);
        assert_eq!(
            presentation.mallard_life_stage,
            Some(mclone_protocol::MallardLifeStage::Duckling)
        );
        assert!(presentation.in_water);

        let nest_id = EntityId(9);
        runtime.apply_update(ServerUpdate::EntitySnapshot(EntitySnapshot {
            id: nest_id,
            persistent_id: mclone_protocol::EntityPersistentId::new(0, nest_id.0),
            kind: mclone_protocol::EntityKind::MallardNest,
            item_stack: None,
            mallard: None,
            mallard_nest: Some(mclone_protocol::MallardNestSnapshotData {
                incubation_progress: 10,
                incubation_required: 20,
                attended: false,
            }),
            deer: None,
            animation: None,
            position: Vec3d::new(2.5, 65.0, 3.5),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.8,
            height: 0.32,
            tick_count: 1,
        }));
        runtime.apply_update(ServerUpdate::EntityUpdate(EntityUpdate {
            id: nest_id,
            item_stack: None,
            mallard: None,
            mallard_nest: Some(mclone_protocol::MallardNestUpdateData {
                incubation_progress: 15,
                incubation_required: 20,
                attended: true,
            }),
            deer: None,
            animation: None,
            position: Vec3d::new(2.5, 65.0, 3.5),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            tick_count: 2,
        }));

        assert_eq!(
            runtime
                .entity(nest_id)
                .and_then(|entity| entity.mallard_nest),
            Some(mclone_protocol::MallardNestSnapshotData {
                incubation_progress: 15,
                incubation_required: 20,
                attended: true,
            })
        );
    }

    #[test]
    fn actor_presentations_expose_remote_player_snapshot() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let update = RemotePlayerUpdate {
            id: RemotePlayerId(3),
            appearance: PlayerAppearance {
                model: PlayerModelKind::UprightBear,
            },
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
                appearance: ActorAppearance::figure(mclone_assets::upright_bear_figure_id()),
                item_stack: None,
                mallard_life_stage: None,
                in_water: false,
                mallard_nest: None,
                deer: None,
                feet_position: update.position,
                y_rot_degrees: update.y_rot_degrees,
                x_rot_degrees: update.x_rot_degrees,
                rotation: None,
                on_ground: update.on_ground,
                width: 0.6,
                height: 1.8,
                animation: Some(mclone_core::AnimationState::distance(
                    mclone_core::AnimationClipId::from_static("walk"),
                    0,
                )),
                animation_clock_tick: 0,
                walk_animation_distance: 0.0,
                chicken_wing_flap_radians: None,
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
            persistent_id: mclone_protocol::EntityPersistentId::new(0, 11),
            kind: mclone_protocol::EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 0.0,
            rotation: Some(mclone_protocol::EntityRotation::IDENTITY),
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 0,
        };

        runtime.apply_update(ServerUpdate::EntitySnapshot(snapshot));

        assert_eq!(
            runtime.actor_presentations(),
            vec![ActorPresentation {
                id: ActorPresentationId::Entity(snapshot.id),
                kind: ActorPresentationKind::Entity(snapshot.kind),
                appearance: ActorAppearance::NONE,
                item_stack: snapshot.item_stack,
                mallard_life_stage: None,
                in_water: false,
                mallard_nest: None,
                deer: None,
                feet_position: snapshot.position,
                y_rot_degrees: snapshot.y_rot_degrees,
                x_rot_degrees: snapshot.x_rot_degrees,
                rotation: snapshot.rotation,
                on_ground: snapshot.on_ground,
                width: snapshot.width,
                height: snapshot.height,
                animation: snapshot.animation,
                animation_clock_tick: 0,
                walk_animation_distance: 0.0,
                chicken_wing_flap_radians: None,
            }]
        );
    }

    #[test]
    fn actor_presentations_include_item_stack_snapshots() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let stack = mclone_protocol::ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::Egg,
            count: 1,
        };
        let snapshot = EntitySnapshot {
            id: EntityId(12),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, 12),
            kind: mclone_protocol::EntityKind::Item,
            item_stack: Some(stack),
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: false,
            width: 0.25,
            height: 0.25,
            tick_count: 0,
        };

        runtime.apply_update(ServerUpdate::EntitySnapshot(snapshot));

        let presentation = runtime.actor_presentations().remove(0);
        assert_eq!(
            presentation.kind,
            ActorPresentationKind::Entity(snapshot.kind)
        );
        assert_eq!(presentation.item_stack, Some(stack));
        assert_eq!(presentation.width, 0.25);
        assert_eq!(presentation.height, 0.25);
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
            updates: vec![
                SectionBlockUpdate {
                    local_x: 1,
                    local_y: 2,
                    local_z: 3,
                    block_state: BlockStateId(42),
                },
                SectionBlockUpdate {
                    local_x: 4,
                    local_y: 5,
                    local_z: 6,
                    block_state: BlockStateId(43),
                },
            ],
        });

        let snapshot = runtime.chunk_snapshot(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(snapshot.revision, ChunkRevision(2));
        assert_eq!(snapshot.sections.len(), 1);
        let blocks = snapshot.sections[0].unpack_block_state_ids();
        assert_eq!(blocks[chunk_section_index(1, 2, 3)], BlockStateId(42));
        assert_eq!(blocks[chunk_section_index(4, 5, 6)], BlockStateId(43));

        assert!(!runtime.apply_section_block_updates(
            ChunkPos::new(0, 0),
            0,
            &[SectionBlockUpdate {
                local_x: 1,
                local_y: 2,
                local_z: 3,
                block_state: BlockStateId(42),
            }],
        ));
        assert_eq!(
            runtime
                .chunk_snapshot(ChunkPos::new(0, 0))
                .unwrap()
                .revision,
            ChunkRevision(2),
            "an idempotent delta must not force a render-mirror upsert"
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

    #[test]
    fn client_runtime_applies_item_stack_entity_updates() {
        let mut runtime = ClientRuntime::new(ClientHost::RemoteDedicated);
        let id = EntityId(9);
        runtime.apply_update(ServerUpdate::EntitySnapshot(EntitySnapshot {
            id,
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id.0),
            kind: mclone_protocol::EntityKind::Item,
            item_stack: Some(mclone_protocol::ItemStackSnapshot {
                kind: mclone_protocol::ItemKind::Egg,
                count: 1,
            }),
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.25,
            height: 0.25,
            tick_count: 1,
        }));

        runtime.apply_update(ServerUpdate::EntityUpdate(EntityUpdate {
            id,
            item_stack: Some(mclone_protocol::ItemStackSnapshot {
                kind: mclone_protocol::ItemKind::Egg,
                count: 2,
            }),
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: mclone_core::Vec3d::new(1.0, 64.0, 2.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            tick_count: 2,
        }));

        assert_eq!(
            runtime.entity(id).unwrap().item_stack,
            Some(mclone_protocol::ItemStackSnapshot {
                kind: mclone_protocol::ItemKind::Egg,
                count: 2,
            })
        );
    }
}
