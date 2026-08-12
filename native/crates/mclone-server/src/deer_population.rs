use std::collections::{BTreeMap, BTreeSet};

use mclone_core::ChunkPos;
use serde::{Deserialize, Serialize};

use crate::persistence::{ChunkStoreError, ChunkStoreResult, SavedDataRecord};

pub(crate) const DEER_POPULATION_HISTORY_KEY: &str = "mclone:deer-population-history-v1";
const DEER_POPULATION_HISTORY_CODEC: u32 = 1;
pub(crate) const DEER_RECOLONIZATION_SPAWN_CYCLES: u16 = 60;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DeerPopulationHistory {
    revision: u64,
    depleted: BTreeMap<ChunkPos, u16>,
    dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeerPopulationHistoryPayload {
    depleted: Vec<DepletedHabitatRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DepletedHabitatRecord {
    chunk_x: i32,
    chunk_z: i32,
    remaining_spawn_cycles: u16,
}

impl DeerPopulationHistory {
    pub(crate) fn from_saved(record: SavedDataRecord) -> ChunkStoreResult<Self> {
        if record.key != DEER_POPULATION_HISTORY_KEY
            || record.codec_version != DEER_POPULATION_HISTORY_CODEC
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "unsupported deer population record {} codec {}",
                record.key, record.codec_version
            )));
        }
        let payload: DeerPopulationHistoryPayload =
            serde_json::from_slice(&record.bytes).map_err(|error| {
                ChunkStoreError::InvalidData(format!(
                    "invalid deer population history JSON: {error}"
                ))
            })?;
        let mut depleted = BTreeMap::new();
        for entry in payload.depleted {
            if entry.remaining_spawn_cycles == 0 {
                return Err(ChunkStoreError::InvalidData(
                    "deer population history contains an expired entry".to_owned(),
                ));
            }
            if depleted
                .insert(
                    ChunkPos::new(entry.chunk_x, entry.chunk_z),
                    entry.remaining_spawn_cycles,
                )
                .is_some()
            {
                return Err(ChunkStoreError::InvalidData(
                    "deer population history contains a duplicate chunk".to_owned(),
                ));
            }
        }
        Ok(Self {
            revision: record.revision,
            depleted,
            dirty: false,
        })
    }

    pub(crate) fn deplete(&mut self, chunk: ChunkPos) {
        self.depleted
            .insert(chunk, DEER_RECOLONIZATION_SPAWN_CYCLES);
        self.dirty = true;
    }

    pub(crate) fn blocks_spawn(&self, chunk: ChunkPos) -> bool {
        self.depleted.contains_key(&chunk)
    }

    pub(crate) fn advance_spawn_cycle(&mut self, eligible_chunks: &BTreeSet<ChunkPos>) {
        let mut changed = false;
        self.depleted.retain(|chunk, remaining| {
            if eligible_chunks.contains(chunk) {
                *remaining = remaining.saturating_sub(1);
                changed = true;
            }
            *remaining > 0
        });
        self.dirty |= changed;
    }

    pub(crate) const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn saved_record(&self) -> ChunkStoreResult<SavedDataRecord> {
        let payload = DeerPopulationHistoryPayload {
            depleted: self
                .depleted
                .iter()
                .map(|(chunk, remaining_spawn_cycles)| DepletedHabitatRecord {
                    chunk_x: chunk.x,
                    chunk_z: chunk.z,
                    remaining_spawn_cycles: *remaining_spawn_cycles,
                })
                .collect(),
        };
        SavedDataRecord::new(
            DEER_POPULATION_HISTORY_KEY,
            DEER_POPULATION_HISTORY_CODEC,
            self.revision.saturating_add(1),
            serde_json::to_vec(&payload).map_err(|error| {
                ChunkStoreError::InvalidData(format!(
                    "failed to encode deer population history: {error}"
                ))
            })?,
        )
    }

    pub(crate) fn mark_saved(&mut self, revision: u64) {
        self.revision = revision;
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depletion_round_trips_and_requires_slow_active_recolonization() {
        let chunk = ChunkPos::new(4, -7);
        let mut history = DeerPopulationHistory::default();
        history.deplete(chunk);
        let record = history.saved_record().unwrap();
        let mut restored = DeerPopulationHistory::from_saved(record).unwrap();
        assert!(restored.blocks_spawn(chunk));
        let eligible = BTreeSet::from([chunk]);
        for _ in 0..DEER_RECOLONIZATION_SPAWN_CYCLES - 1 {
            restored.advance_spawn_cycle(&eligible);
            assert!(restored.blocks_spawn(chunk));
        }
        restored.advance_spawn_cycle(&eligible);
        assert!(!restored.blocks_spawn(chunk));
    }
}
