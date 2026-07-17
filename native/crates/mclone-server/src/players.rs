use std::collections::BTreeMap;
use std::fmt;

use mclone_core::{ChunkPos, Vec3d};
use mclone_protocol::{
    ClientIdentity, DimensionKey, PlayerAppearance, PlayerDamageCause, PlayerStatistics,
    PlayerVitals, SessionCapabilities,
};

use crate::inventory::ServerInventory;
use crate::persistence::PlayerRecord;
use crate::player::ServerPlayerState;

const FIRST_PLAYER_ID: u64 = 0;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServerPlayerId(u64);

impl ServerPlayerId {
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn from_raw_for_tests(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for ServerPlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ServerPlayerEntry {
    pub(crate) dimension: DimensionKey,
    pub(crate) state: ServerPlayerState,
    pub(crate) inventory: ServerInventory,
    pub(crate) appearance: PlayerAppearance,
    pub(crate) initial_spawn_center: Option<ChunkPos>,
    pub(crate) identity: Option<ClientIdentity>,
    pub(crate) resume_record: Option<PlayerRecord>,
    pub(crate) total_experience: u64,
    pub(crate) statistics: PlayerStatistics,
    pub(crate) vitals: PlayerVitals,
    pub(crate) pending_death_cause: Option<PlayerDamageCause>,
    pub(crate) player_record_revision: u64,
    pub(crate) capabilities: SessionCapabilities,
}

impl Default for ServerPlayerEntry {
    fn default() -> Self {
        Self {
            dimension: DimensionKey::overworld(),
            state: ServerPlayerState::default(),
            inventory: ServerInventory::default(),
            appearance: PlayerAppearance::default(),
            initial_spawn_center: None,
            identity: None,
            resume_record: None,
            total_experience: 0,
            statistics: PlayerStatistics::default(),
            vitals: PlayerVitals::default(),
            pending_death_cause: None,
            player_record_revision: 0,
            capabilities: SessionCapabilities::NONE,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ServerPlayerList {
    next_id: u64,
    players: BTreeMap<ServerPlayerId, ServerPlayerEntry>,
}

impl Default for ServerPlayerList {
    fn default() -> Self {
        Self {
            next_id: FIRST_PLAYER_ID,
            players: BTreeMap::new(),
        }
    }
}

impl ServerPlayerList {
    pub(crate) fn add_in_dimension(
        &mut self,
        dimension: DimensionKey,
        capabilities: SessionCapabilities,
    ) -> ServerPlayerId {
        let id = ServerPlayerId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("exhausted realm server player ids");
        self.players.insert(
            id,
            ServerPlayerEntry {
                dimension,
                capabilities,
                ..ServerPlayerEntry::default()
            },
        );
        id
    }

    pub(crate) fn remove(&mut self, id: ServerPlayerId) -> Option<ServerPlayerEntry> {
        self.players.remove(&id)
    }

    pub(crate) fn contains(&self, id: ServerPlayerId) -> bool {
        self.players.contains_key(&id)
    }

    pub(crate) fn get(&self, id: ServerPlayerId) -> Option<&ServerPlayerEntry> {
        self.players.get(&id)
    }

    pub(crate) fn get_mut(&mut self, id: ServerPlayerId) -> Option<&mut ServerPlayerEntry> {
        self.players.get_mut(&id)
    }

    pub(crate) fn len(&self) -> usize {
        self.players.len()
    }

    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut ServerPlayerEntry> {
        self.players.values_mut()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (ServerPlayerId, &ServerPlayerEntry)> {
        self.players
            .iter()
            .map(|(player_id, entry)| (*player_id, entry))
    }

    pub(crate) fn position(&self, id: ServerPlayerId) -> Option<Vec3d> {
        self.players.get(&id).map(|entry| entry.state.position())
    }

    pub(crate) fn dimension(&self, id: ServerPlayerId) -> Option<&DimensionKey> {
        self.players.get(&id).map(|entry| &entry.dimension)
    }
}
