use std::{collections::BTreeMap, error::Error, fmt};

use mclone_core::ChunkPos;
use mclone_input::{LocalParticipantSlot, MAX_LOCAL_PARTICIPANTS};
use mclone_protocol::{
    AcceptTeleportCommand, ChunkView, ClientCommand, ClientIdentity, PlayerProfileId,
};
use mclone_server::{
    ChunkStoreError, RealmServer, ServerPlayerId, ServerSimulationTickReport, ServerTickReport,
};

use crate::{
    RuntimeExchange, SingleViewRuntime,
    local_participant::{LocalParticipantAdmission, LocalParticipantId},
};

pub struct LocalClientEndpoint {
    participant: LocalParticipantAdmission,
    identity: ClientIdentity,
    player_id: ServerPlayerId,
    runtime: SingleViewRuntime,
}

impl LocalClientEndpoint {
    pub const fn participant(&self) -> LocalParticipantAdmission {
        self.participant
    }

    pub fn identity(&self) -> &ClientIdentity {
        &self.identity
    }

    pub const fn player_id(&self) -> ServerPlayerId {
        self.player_id
    }

    pub const fn runtime(&self) -> &SingleViewRuntime {
        &self.runtime
    }

    pub const fn runtime_mut(&mut self) -> &mut SingleViewRuntime {
        &mut self.runtime
    }
}

#[derive(Debug)]
pub enum LocalClientGroupError {
    ParticipantAlreadyJoined(LocalParticipantId),
    SlotOccupied(LocalParticipantSlot),
    DuplicateProfile(PlayerProfileId),
    ParticipantMissing(LocalParticipantId),
    RealmPlayerMissing(ServerPlayerId),
    Realm(ChunkStoreError),
}

impl fmt::Display for LocalClientGroupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParticipantAlreadyJoined(_) => write!(formatter, "participant already joined"),
            Self::SlotOccupied(slot) => {
                write!(formatter, "local client slot {} is occupied", slot.index())
            }
            Self::DuplicateProfile(profile_id) => {
                write!(
                    formatter,
                    "profile {:?} is already joined",
                    profile_id.bytes()
                )
            }
            Self::ParticipantMissing(_) => write!(formatter, "local client participant is missing"),
            Self::RealmPlayerMissing(player_id) => {
                write!(formatter, "local realm player {player_id} is missing")
            }
            Self::Realm(error) => write!(formatter, "local realm operation failed: {error}"),
        }
    }
}

impl Error for LocalClientGroupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Realm(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ChunkStoreError> for LocalClientGroupError {
    fn from(error: ChunkStoreError) -> Self {
        Self::Realm(error)
    }
}

/// Host-neutral 1-4 ordinary client endpoints sharing one realm authority.
///
/// Each endpoint retains an independent client replica. Server scheduler and
/// simulation methods advance once globally, then drain every player's ordered
/// stream independently.
pub struct LocalClientGroup {
    seed: i64,
    realm: RealmServer,
    endpoints: BTreeMap<LocalParticipantId, LocalClientEndpoint>,
    slots: [Option<LocalParticipantId>; MAX_LOCAL_PARTICIPANTS],
}

impl LocalClientGroup {
    pub fn new(seed: i64, realm: RealmServer) -> Self {
        Self {
            seed,
            realm,
            endpoints: BTreeMap::new(),
            slots: [None; MAX_LOCAL_PARTICIPANTS],
        }
    }

    pub fn transient(seed: i64) -> Self {
        Self::new(seed, RealmServer::local_integrated(seed))
    }

    pub const fn realm(&self) -> &RealmServer {
        &self.realm
    }

    pub fn realm_mut(&mut self) -> &mut RealmServer {
        &mut self.realm
    }

    pub fn len(&self) -> usize {
        self.endpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }

    pub fn endpoint(&self, participant_id: LocalParticipantId) -> Option<&LocalClientEndpoint> {
        self.endpoints.get(&participant_id)
    }

    pub fn endpoint_mut(
        &mut self,
        participant_id: LocalParticipantId,
    ) -> Option<&mut LocalClientEndpoint> {
        self.endpoints.get_mut(&participant_id)
    }

    pub fn endpoint_at(&self, slot: LocalParticipantSlot) -> Option<&LocalClientEndpoint> {
        self.slots[slot.index()].and_then(|id| self.endpoint(id))
    }

    pub fn iter(&self) -> impl Iterator<Item = &LocalClientEndpoint> {
        self.endpoints.values()
    }

    pub fn join(
        &mut self,
        participant: LocalParticipantAdmission,
        identity: ClientIdentity,
        initial_view: ChunkView,
    ) -> Result<ServerPlayerId, LocalClientGroupError> {
        if self.endpoints.contains_key(&participant.id) {
            return Err(LocalClientGroupError::ParticipantAlreadyJoined(
                participant.id,
            ));
        }
        if self.slots[participant.slot.index()].is_some() {
            return Err(LocalClientGroupError::SlotOccupied(participant.slot));
        }
        if self
            .endpoints
            .values()
            .any(|endpoint| endpoint.identity.profile_id == identity.profile_id)
        {
            return Err(LocalClientGroupError::DuplicateProfile(identity.profile_id));
        }

        let player_id = self.realm.add_player_with_identity(identity.clone())?;
        let endpoint = (|| {
            let mut runtime = SingleViewRuntime::local_integrated_with_seed(
                self.seed,
                initial_view.center,
                initial_view.render_distance,
                initial_view.chunk_tracking_radius,
            );
            let initial_updates = self.realm.try_drain_updates_for_player(player_id)?;
            runtime.apply_exchange(RuntimeExchange::updates(initial_updates, true));
            let command = runtime
                .set_chunk_view_command(
                    initial_view.center,
                    initial_view.render_distance,
                    initial_view.chunk_tracking_radius,
                )
                .expect("a new client replica has no accepted chunk view");
            let updates = self
                .realm
                .try_handle_command_for_player(player_id, command)?;
            runtime.apply_exchange(RuntimeExchange::command(updates));
            Ok::<_, ChunkStoreError>(LocalClientEndpoint {
                participant,
                identity,
                player_id,
                runtime,
            })
        })();
        let endpoint = match endpoint {
            Ok(endpoint) => endpoint,
            Err(error) => {
                self.realm.remove_player(player_id);
                return Err(error.into());
            }
        };

        self.slots[participant.slot.index()] = Some(participant.id);
        self.endpoints.insert(participant.id, endpoint);
        Ok(player_id)
    }

    pub fn set_chunk_view(
        &mut self,
        participant_id: LocalParticipantId,
        view: ChunkView,
    ) -> Result<bool, LocalClientGroupError> {
        let Some(endpoint) = self.endpoints.get_mut(&participant_id) else {
            return Err(LocalClientGroupError::ParticipantMissing(participant_id));
        };
        let Some(command) = endpoint.runtime.set_chunk_view_command(
            view.center,
            view.render_distance,
            view.chunk_tracking_radius,
        ) else {
            return Ok(false);
        };
        let updates = self
            .realm
            .try_handle_command_for_player(endpoint.player_id, command)?;
        endpoint
            .runtime
            .apply_exchange(RuntimeExchange::command(updates));
        Ok(true)
    }

    pub fn send_command(
        &mut self,
        participant_id: LocalParticipantId,
        command: ClientCommand,
    ) -> Result<(), LocalClientGroupError> {
        let Some(endpoint) = self.endpoints.get_mut(&participant_id) else {
            return Err(LocalClientGroupError::ParticipantMissing(participant_id));
        };
        let updates = self
            .realm
            .try_handle_command_for_player(endpoint.player_id, command)?;
        endpoint
            .runtime
            .apply_exchange(RuntimeExchange::command(updates));
        Ok(())
    }

    pub fn acknowledge_pending_positions(
        &mut self,
        participant_id: LocalParticipantId,
    ) -> Result<usize, LocalClientGroupError> {
        let pending = {
            let Some(endpoint) = self.endpoints.get_mut(&participant_id) else {
                return Err(LocalClientGroupError::ParticipantMissing(participant_id));
            };
            endpoint.runtime.drain_player_position_updates()
        };
        let count = pending.len();
        for update in pending {
            self.send_command(
                participant_id,
                ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                    id: update.teleport_id,
                }),
            )?;
        }
        Ok(count)
    }

    pub fn acknowledge_all_pending_positions(&mut self) -> Result<usize, LocalClientGroupError> {
        let ids = self.endpoints.keys().copied().collect::<Vec<_>>();
        let mut count = 0;
        for id in ids {
            count += self.acknowledge_pending_positions(id)?;
        }
        Ok(count)
    }

    pub fn drain_all_player_streams(&mut self) -> Result<usize, LocalClientGroupError> {
        let routes = self
            .endpoints
            .iter()
            .map(|(participant_id, endpoint)| (*participant_id, endpoint.player_id))
            .collect::<Vec<_>>();
        let mut update_count = 0;
        for (participant_id, player_id) in routes {
            let updates = self.realm.try_drain_updates_for_player(player_id)?;
            update_count += updates.len();
            self.endpoints
                .get_mut(&participant_id)
                .expect("collected endpoint remains present")
                .runtime
                .apply_exchange(RuntimeExchange::updates(updates, true));
        }
        Ok(update_count)
    }

    pub fn advance_scheduler_once(&mut self) -> Result<ServerTickReport, LocalClientGroupError> {
        let report = self.realm.try_tick_report_global()?;
        self.drain_all_player_streams()?;
        Ok(report)
    }

    pub fn advance_simulation_once(
        &mut self,
    ) -> Result<ServerSimulationTickReport, LocalClientGroupError> {
        let report = self.realm.try_simulation_tick_report_global()?;
        self.drain_all_player_streams()?;
        Ok(report)
    }

    pub fn leave(
        &mut self,
        participant_id: LocalParticipantId,
    ) -> Result<bool, LocalClientGroupError> {
        let Some(endpoint) = self.endpoints.get(&participant_id) else {
            return Ok(false);
        };
        if !self.realm.remove_player(endpoint.player_id) {
            return Err(LocalClientGroupError::RealmPlayerMissing(
                endpoint.player_id,
            ));
        }
        let endpoint = self
            .endpoints
            .remove(&participant_id)
            .expect("endpoint presence was checked");
        self.slots[endpoint.participant.slot.index()] = None;
        self.drain_all_player_streams()?;
        Ok(true)
    }

    pub fn zero_radius_view(center: ChunkPos) -> ChunkView {
        ChunkView {
            center,
            render_distance: 0,
            chunk_tracking_radius: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mclone_core::Vec3d;
    use mclone_protocol::{MovePlayerCommand, SetCarriedItemCommand, StatisticKey};
    use mclone_server::{
        MemoryWorldStore, PlayerRecord, PlayerRecordKey, WorldGenerationProfile, WorldStore,
    };

    use crate::local_participant::LocalParticipantGroup;

    use super::*;

    const SEED: i64 = 12_345;
    const SHARED_CENTER: ChunkPos = ChunkPos::new(0, 0);
    const FAR_CENTER: ChunkPos = ChunkPos::new(8, 0);

    fn identity(byte: u8, name: &str) -> ClientIdentity {
        ClientIdentity::new(PlayerProfileId::new([byte; 16]), name).unwrap()
    }

    fn fixture() -> (
        LocalClientGroup,
        LocalParticipantAdmission,
        LocalParticipantAdmission,
    ) {
        let first_identity = identity(0x11, "Fixture One");
        let second_identity = identity(0x22, "Fixture Two");
        let mut first_record = PlayerRecord::new(
            PlayerRecordKey::from_profile_id(first_identity.profile_id),
            1,
            first_identity.display_name.clone(),
            Vec3d::new(0.5, 64.0, 0.5),
        );
        first_record.selected_hotbar_slot = 5;
        first_record.total_experience = 31;
        first_record.statistics.increment(StatisticKey::jump(), 7);
        first_record.health = 13.5;
        let mut second_record = PlayerRecord::new(
            PlayerRecordKey::from_profile_id(second_identity.profile_id),
            1,
            second_identity.display_name.clone(),
            Vec3d::new(1.5, 64.0, 1.5),
        );
        second_record.selected_hotbar_slot = 1;
        second_record.total_experience = 2;

        let mut store = MemoryWorldStore::new();
        store.save_player(&first_record).unwrap();
        store.save_player(&second_record).unwrap();
        let mut realm = RealmServer::with_world_store(SEED, Box::new(store));
        realm
            .set_world_generation_profile(WorldGenerationProfile::FlatGrassV1)
            .unwrap();
        realm.set_lighting_enabled(false);
        let mut group = LocalClientGroup::new(SEED, realm);

        let mut participants = LocalParticipantGroup::new();
        let first = participants.admit_next(()).unwrap();
        let second = participants.admit_next(()).unwrap();
        group
            .join(
                first,
                first_identity,
                LocalClientGroup::zero_radius_view(SHARED_CENTER),
            )
            .unwrap();
        group
            .join(
                second,
                second_identity,
                LocalClientGroup::zero_radius_view(SHARED_CENTER),
            )
            .unwrap();
        (group, first, second)
    }

    fn warm_two_clients(group: &mut LocalClientGroup) {
        for _ in 0..60_000 {
            group.advance_scheduler_once().unwrap();
            group.acknowledge_all_pending_positions().unwrap();
            group.drain_all_player_streams().unwrap();
            if group.iter().all(|endpoint| {
                endpoint
                    .runtime()
                    .client()
                    .chunk_snapshot(SHARED_CENTER)
                    .is_some()
                    && endpoint.runtime().client().remote_player_count() == 1
            }) {
                return;
            }
            if group.realm().pending_job_count() > 0
                && group.realm().pending_publication_count() == 0
            {
                group
                    .realm_mut()
                    .wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        panic!("timed out warming two local client endpoints");
    }

    #[test]
    fn two_clients_share_one_realm_but_keep_owner_state_private() {
        let (mut group, first, second) = fixture();
        assert_eq!(group.len(), 2);
        assert_eq!(group.realm().player_count(), 2);
        for endpoint in group.iter() {
            assert_eq!(
                endpoint.runtime().client().session_phase(),
                mclone_client::ClientSessionPhase::Playing
            );
            assert_eq!(
                endpoint.runtime().client().current_dimension().as_str(),
                "minecraft:overworld"
            );
        }

        let first_endpoint = group.endpoint(first.id).unwrap();
        let second_endpoint = group.endpoint(second.id).unwrap();
        assert_ne!(first_endpoint.player_id(), second_endpoint.player_id());
        assert_eq!(first_endpoint.runtime().client().total_experience(), 31);
        assert_eq!(second_endpoint.runtime().client().total_experience(), 2);
        assert_eq!(
            first_endpoint
                .runtime()
                .client()
                .player_statistics()
                .jump_count(),
            7
        );
        assert!(
            second_endpoint
                .runtime()
                .client()
                .player_statistics()
                .is_empty()
        );
        assert_eq!(
            first_endpoint.runtime().client().player_vitals().health(),
            13.5
        );
        assert_eq!(
            second_endpoint.runtime().client().player_vitals().health(),
            20.0
        );

        warm_two_clients(&mut group);
        assert!(
            group
                .iter()
                .all(|endpoint| endpoint.runtime().client().remote_player_count() == 1)
        );

        let first_player = group.endpoint(first.id).unwrap().player_id();
        let second_player = group.endpoint(second.id).unwrap().player_id();
        group
            .send_command(
                first.id,
                ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 7 }),
            )
            .unwrap();
        assert_eq!(
            group.realm().player_selected_hotbar_slot(first_player),
            Some(7)
        );
        assert_eq!(
            group.realm().player_selected_hotbar_slot(second_player),
            Some(1)
        );

        let position = group.realm().player_position(first_player).unwrap();
        group
            .send_command(
                first.id,
                ClientCommand::move_player(MovePlayerCommand::Pos {
                    position,
                    on_ground: true,
                }),
            )
            .unwrap();
        group
            .send_command(
                first.id,
                ClientCommand::move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(position.x, position.y + 0.42, position.z),
                    on_ground: false,
                }),
            )
            .unwrap();
        assert_eq!(
            group
                .endpoint(first.id)
                .unwrap()
                .runtime()
                .client()
                .player_statistics()
                .jump_count(),
            8
        );
        assert!(
            group
                .endpoint(second.id)
                .unwrap()
                .runtime()
                .client()
                .player_statistics()
                .is_empty()
        );

        let first_tick = group.advance_simulation_once().unwrap().simulation_tick;
        let second_tick = group.advance_simulation_once().unwrap().simulation_tick;
        assert_eq!(second_tick, first_tick + 1);
    }

    #[test]
    fn separated_interest_and_one_leave_preserve_the_other_endpoint() {
        let (mut group, first, second) = fixture();
        warm_two_clients(&mut group);
        let overlap = group.realm().chunk_tracking_diagnostics();
        assert_eq!(overlap.player_count, 2);
        assert_eq!(overlap.aggregate_player_ticket_chunks, 1);
        assert_eq!(overlap.total_player_visible_chunks, 2);

        assert!(
            group
                .set_chunk_view(second.id, LocalClientGroup::zero_radius_view(FAR_CENTER))
                .unwrap()
        );
        let separated = group.realm().chunk_tracking_diagnostics();
        assert_eq!(separated.aggregate_player_ticket_chunks, 2);
        assert_eq!(separated.total_player_visible_chunks, 2);

        assert!(group.leave(second.id).unwrap());
        assert_eq!(group.len(), 1);
        assert_eq!(group.realm().player_count(), 1);
        assert_eq!(group.realm().chunk_tracking_diagnostics().player_count, 1);
        let survivor = group.endpoint(first.id).unwrap();
        assert_eq!(survivor.runtime().client().remote_player_count(), 0);
        assert!(
            survivor
                .runtime()
                .client()
                .chunk_snapshot(SHARED_CENTER)
                .is_some()
        );
        assert!(!group.leave(second.id).unwrap());
    }

    #[test]
    fn duplicate_live_profile_is_rejected_before_realm_admission() {
        let (mut group, first, _) = fixture();
        let mut participants = LocalParticipantGroup::new();
        let _ = participants.admit_next(()).unwrap();
        let _ = participants.admit_next(()).unwrap();
        let third = participants.admit_next(()).unwrap();
        let duplicate = group.endpoint(first.id).unwrap().identity().clone();
        assert!(matches!(
            group.join(
                third,
                duplicate,
                LocalClientGroup::zero_radius_view(SHARED_CENTER)
            ),
            Err(LocalClientGroupError::DuplicateProfile(_))
        ));
        assert_eq!(group.realm().player_count(), 2);
    }

    #[test]
    fn four_participants_map_to_four_distinct_ordinary_players() {
        let mut group = LocalClientGroup::transient(SEED);
        group.realm_mut().set_lighting_enabled(false);
        let mut participants = LocalParticipantGroup::new();
        let admissions = (0..MAX_LOCAL_PARTICIPANTS)
            .map(|_| participants.admit_next(()).unwrap())
            .collect::<Vec<_>>();

        for (index, admission) in admissions.iter().copied().enumerate() {
            group
                .join(
                    admission,
                    identity(index as u8 + 1, &format!("Seat {}", index + 1)),
                    LocalClientGroup::zero_radius_view(SHARED_CENTER),
                )
                .unwrap();
        }

        assert_eq!(group.len(), MAX_LOCAL_PARTICIPANTS);
        assert_eq!(group.realm().player_count(), MAX_LOCAL_PARTICIPANTS);
        let player_ids = group
            .iter()
            .map(LocalClientEndpoint::player_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(player_ids.len(), MAX_LOCAL_PARTICIPANTS);
        for admission in admissions {
            assert_eq!(
                group.endpoint_at(admission.slot).unwrap().participant(),
                admission
            );
        }
    }
}
