use std::collections::BTreeMap;
use std::fmt;

use mclone_core::{ChunkPos, Vec3d};
use mclone_protocol::PlayerAppearance;

use crate::inventory::ServerInventory;
use crate::player::ServerPlayerState;

const FIRST_DEDICATED_PLAYER_ID: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServerPlayerId(u64);

impl ServerPlayerId {
    pub(crate) const LOCAL: Self = Self(0);

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
    pub(crate) state: ServerPlayerState,
    pub(crate) inventory: ServerInventory,
    pub(crate) appearance: PlayerAppearance,
    pub(crate) initial_spawn_center: Option<ChunkPos>,
}

impl Default for ServerPlayerEntry {
    fn default() -> Self {
        Self {
            state: ServerPlayerState::default(),
            inventory: ServerInventory::default(),
            appearance: PlayerAppearance::default(),
            initial_spawn_center: None,
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
            next_id: FIRST_DEDICATED_PLAYER_ID,
            players: BTreeMap::new(),
        }
    }
}

impl ServerPlayerList {
    pub(crate) fn add(&mut self) -> ServerPlayerId {
        let id = ServerPlayerId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("exhausted dedicated server player ids");
        self.players.insert(id, ServerPlayerEntry::default());
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
}
