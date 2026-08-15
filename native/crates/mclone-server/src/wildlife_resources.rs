//! Persisted, bounded loaded forage accounting for materialized wildlife.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, BlockStateId, ChunkPos};
use mclone_worldgen::block::{
    DANDELION, DIRT, GRASS_BLOCK, OAK_LEAVES, POPPY, generated_block_state_id,
};
use serde::{Deserialize, Serialize};

use crate::{ChunkStoreError, ChunkStoreResult, SavedDataRecord};

pub(crate) const WILDLIFE_RESOURCE_SAVED_DATA_KEY: &str = "mclone:wildlife-forage-v1";
pub const WILDLIFE_RESOURCE_RULE_REVISION: u32 = 1;
const WILDLIFE_RESOURCE_CODEC_VERSION: u32 = 1;
pub const WILDLIFE_RESOURCE_CELL_WIDTH_BLOCKS: i32 = 64;
const RECOVERY_DENOMINATOR: u32 = 1_200;
const RESAMPLE_INTERVAL_TICKS: u64 = 1_200;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifeForageCellPos {
    pub x: i32,
    pub z: i32,
}

impl WildlifeForageCellPos {
    pub const fn from_block(pos: BlockPos) -> Self {
        Self {
            x: pos.x.div_euclid(WILDLIFE_RESOURCE_CELL_WIDTH_BLOCKS),
            z: pos.z.div_euclid(WILDLIFE_RESOURCE_CELL_WIDTH_BLOCKS),
        }
    }

    fn overlaps_any_chunk(self, chunks: &BTreeSet<ChunkPos>) -> bool {
        let min_chunk_x = self.x * 4;
        let min_chunk_z = self.z * 4;
        (min_chunk_x..min_chunk_x + 4).any(|chunk_x| {
            (min_chunk_z..min_chunk_z + 4)
                .any(|chunk_z| chunks.contains(&ChunkPos::new(chunk_x, chunk_z)))
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifeForageCellSnapshot {
    pub position: WildlifeForageCellPos,
    pub potential: u32,
    pub available: u32,
    pub recovered: u64,
    pub rabbit_consumed: u64,
    pub deer_consumed: u64,
    pub terrain_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WildlifeForageConsumer {
    Rabbit,
    Deer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct WildlifeResourceLedger {
    revision: u64,
    cells: BTreeMap<WildlifeForageCellPos, WildlifeForageCell>,
    dirty: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeForageCell {
    potential: u32,
    available: u32,
    recovery_remainder: u32,
    recovered: u64,
    rabbit_consumed: u64,
    deer_consumed: u64,
    terrain_revision: u64,
    last_sample_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeResourcePayload {
    rule_revision: u32,
    cells: Vec<WildlifeForageCellRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeForageCellRecord {
    position: WildlifeForageCellPos,
    cell: WildlifeForageCell,
}

impl WildlifeResourceLedger {
    pub(crate) fn from_saved(record: SavedDataRecord) -> ChunkStoreResult<Self> {
        if record.key != WILDLIFE_RESOURCE_SAVED_DATA_KEY
            || record.codec_version != WILDLIFE_RESOURCE_CODEC_VERSION
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "unsupported wildlife resource record {} codec {}",
                record.key, record.codec_version
            )));
        }
        let payload: WildlifeResourcePayload =
            serde_json::from_slice(&record.bytes).map_err(|error| {
                ChunkStoreError::InvalidData(format!("invalid wildlife resource JSON: {error}"))
            })?;
        if payload.rule_revision != WILDLIFE_RESOURCE_RULE_REVISION {
            return Err(ChunkStoreError::InvalidData(format!(
                "unsupported wildlife resource rule revision {}",
                payload.rule_revision
            )));
        }
        let mut cells = BTreeMap::new();
        for record in payload.cells {
            if record.cell.potential == 0 || record.cell.available > record.cell.potential {
                return Err(ChunkStoreError::InvalidData(
                    "wildlife forage cell has invalid availability".to_owned(),
                ));
            }
            if cells.insert(record.position, record.cell).is_some() {
                return Err(ChunkStoreError::InvalidData(
                    "wildlife resource record contains a duplicate cell".to_owned(),
                ));
            }
        }
        Ok(Self {
            revision: record.revision,
            cells,
            dirty: false,
        })
    }

    pub(crate) fn saved_record(&self) -> ChunkStoreResult<SavedDataRecord> {
        let payload = WildlifeResourcePayload {
            rule_revision: WILDLIFE_RESOURCE_RULE_REVISION,
            cells: self
                .cells
                .iter()
                .map(|(position, cell)| WildlifeForageCellRecord {
                    position: *position,
                    cell: *cell,
                })
                .collect(),
        };
        SavedDataRecord::new(
            WILDLIFE_RESOURCE_SAVED_DATA_KEY,
            WILDLIFE_RESOURCE_CODEC_VERSION,
            self.revision.saturating_add(1),
            serde_json::to_vec(&payload).map_err(|error| {
                ChunkStoreError::InvalidData(format!(
                    "failed to encode wildlife resources: {error}"
                ))
            })?,
        )
    }

    pub(crate) const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn mark_saved(&mut self, revision: u64) {
        self.revision = revision;
        self.dirty = false;
    }

    pub(crate) fn recover_loaded(&mut self, chunks: &BTreeSet<ChunkPos>) -> u32 {
        let mut total = 0_u32;
        for (position, cell) in &mut self.cells {
            if !position.overlaps_any_chunk(chunks) || cell.available >= cell.potential {
                continue;
            }
            let numerator = cell
                .recovery_remainder
                .saturating_add((cell.potential / 4).max(1));
            let recovered = numerator / RECOVERY_DENOMINATOR;
            cell.recovery_remainder = numerator % RECOVERY_DENOMINATOR;
            let recovered = recovered.min(cell.potential - cell.available);
            if recovered > 0 {
                cell.available += recovered;
                cell.recovered = cell.recovered.saturating_add(u64::from(recovered));
                total = total.saturating_add(recovered);
                self.dirty = true;
            }
        }
        total
    }

    pub(crate) fn consume_at<F>(
        &mut self,
        consumer: WildlifeForageConsumer,
        feet: BlockPos,
        requested: u16,
        simulation_tick: u64,
        block_state_at: &F,
    ) -> u16
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let position = WildlifeForageCellPos::from_block(feet);
        let needs_sample = self.cells.get(&position).is_none_or(|cell| {
            simulation_tick.saturating_sub(cell.last_sample_tick) >= RESAMPLE_INTERVAL_TICKS
        });
        if needs_sample {
            let potential = sample_forage_potential(feet, block_state_at);
            let cell = self.cells.entry(position).or_default();
            let old_potential = cell.potential;
            cell.potential = potential;
            cell.available = if old_potential == 0 {
                potential
            } else {
                cell.available.min(potential)
            };
            cell.terrain_revision = cell.terrain_revision.saturating_add(1);
            cell.last_sample_tick = simulation_tick;
            self.dirty = true;
        }
        let Some(cell) = self.cells.get_mut(&position) else {
            return 0;
        };
        let consumed = u32::from(requested).min(cell.available);
        cell.available -= consumed;
        match consumer {
            WildlifeForageConsumer::Rabbit => {
                cell.rabbit_consumed = cell.rabbit_consumed.saturating_add(u64::from(consumed));
            }
            WildlifeForageConsumer::Deer => {
                cell.deer_consumed = cell.deer_consumed.saturating_add(u64::from(consumed));
            }
        }
        self.dirty |= consumed > 0;
        consumed as u16
    }

    pub(crate) fn snapshots(&self) -> Vec<WildlifeForageCellSnapshot> {
        self.cells
            .iter()
            .map(|(position, cell)| WildlifeForageCellSnapshot {
                position: *position,
                potential: cell.potential,
                available: cell.available,
                recovered: cell.recovered,
                rabbit_consumed: cell.rabbit_consumed,
                deer_consumed: cell.deer_consumed,
                terrain_revision: cell.terrain_revision,
            })
            .collect()
    }
}

fn sample_forage_potential<F>(feet: BlockPos, block_state_at: &F) -> u32
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let mut potential = 0_u32;
    for x in feet.x - 4..=feet.x + 4 {
        for z in feet.z - 4..=feet.z + 4 {
            for y in feet.y - 1..=feet.y + 2 {
                let Some(state) = block_state_at(BlockPos::new(x, y, z)) else {
                    continue;
                };
                potential = potential.saturating_add(if state == generated_block_state_id(GRASS_BLOCK) {
                    12
                } else if state == generated_block_state_id(DIRT) {
                    2
                } else if matches!(state, value if value == generated_block_state_id(DANDELION) || value == generated_block_state_id(POPPY)) {
                    20
                } else if state == generated_block_state_id(OAK_LEAVES) {
                    4
                } else {
                    0
                });
            }
        }
    }
    potential.clamp(100, 4_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forage_consumption_recovery_and_persistence_conserve_bounds() {
        let grass = |pos: BlockPos| (pos.y == 63).then_some(generated_block_state_id(GRASS_BLOCK));
        let mut ledger = WildlifeResourceLedger::default();
        let feet = BlockPos::new(2, 64, 2);
        let eaten = ledger.consume_at(WildlifeForageConsumer::Rabbit, feet, 50, 20, &grass);
        assert_eq!(eaten, 50);
        let before = ledger.snapshots()[0];
        for _ in 0..1_200 {
            ledger.recover_loaded(&BTreeSet::from([ChunkPos::new(0, 0)]));
        }
        let after = ledger.snapshots()[0];
        assert!(after.available > before.available);
        assert!(after.available <= after.potential);

        let restored = WildlifeResourceLedger::from_saved(ledger.saved_record().unwrap()).unwrap();
        assert_eq!(restored.snapshots(), ledger.snapshots());
    }
}
