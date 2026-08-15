//! Persisted, bounded loaded resource accounting for materialized wildlife.

use std::collections::{BTreeMap, BTreeSet};

use mclone_blocks::{BlockFluidKind, block_fluid_kind};
use mclone_core::{BlockPos, BlockStateId, ChunkPos};
use mclone_worldgen::block::{
    ACACIA_LEAVES, BIRCH_LEAVES, DANDELION, DARK_OAK_LEAVES, FERN, GRASS, GRASS_BLOCK,
    JUNGLE_LEAVES, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LILY_PAD, OAK_LEAVES, POPPY, SPRUCE_LEAVES,
    SUGAR_CANE, TALL_GRASS_LOWER, TALL_GRASS_UPPER, generated_block_state_id,
};
use serde::{Deserialize, Serialize};

use crate::{ChunkStoreError, ChunkStoreResult, SavedDataRecord};

pub(crate) const WILDLIFE_RESOURCE_SAVED_DATA_KEY: &str = "mclone:wildlife-forage-v1";
pub const WILDLIFE_RESOURCE_RULE_REVISION: u32 = 2;
const WILDLIFE_RESOURCE_CODEC_VERSION: u32 = 2;
pub const WILDLIFE_RESOURCE_CELL_WIDTH_BLOCKS: i32 = 64;
pub const WILDLIFE_RESOURCE_KIND_COUNT: usize = 5;
const RECOVERY_DENOMINATOR: u32 = 1_200;
const RESAMPLE_INTERVAL_TICKS: u64 = 1_200;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "camelCase")]
pub enum WildlifeResourceKind {
    LowHerbaceous = 0,
    WoodyBrowse = 1,
    SeedsAndSoftMast = 2,
    AquaticVegetation = 3,
    AquaticInvertebrates = 4,
}

impl WildlifeResourceKind {
    pub const ALL: [Self; WILDLIFE_RESOURCE_KIND_COUNT] = [
        Self::LowHerbaceous,
        Self::WoodyBrowse,
        Self::SeedsAndSoftMast,
        Self::AquaticVegetation,
        Self::AquaticInvertebrates,
    ];

    const fn index(self) -> usize {
        self as usize
    }

    const fn recovery_days(self) -> u32 {
        match self {
            Self::LowHerbaceous => 4,
            Self::WoodyBrowse => 8,
            Self::SeedsAndSoftMast => 5,
            Self::AquaticVegetation => 6,
            Self::AquaticInvertebrates => 3,
        }
    }
}

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
pub struct WildlifeResourceStratumSnapshot {
    pub potential: u32,
    pub available: u32,
    pub recovered: u64,
    pub rabbit_consumed: u64,
    pub deer_consumed: u64,
    pub mallard_consumed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WildlifeForageCellSnapshot {
    pub position: WildlifeForageCellPos,
    pub strata: [WildlifeResourceStratumSnapshot; WILDLIFE_RESOURCE_KIND_COUNT],
    pub terrain_revision: u64,
}

impl WildlifeForageCellSnapshot {
    pub fn stratum(self, kind: WildlifeResourceKind) -> WildlifeResourceStratumSnapshot {
        self.strata[kind.index()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WildlifeForageConsumer {
    Rabbit,
    Deer,
    Mallard,
}

impl WildlifeForageConsumer {
    const fn index(self) -> usize {
        match self {
            Self::Rabbit => 0,
            Self::Deer => 1,
            Self::Mallard => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WildlifeDietEntry {
    pub(crate) resource: WildlifeResourceKind,
    pub(crate) energy_per_unit: u16,
    pub(crate) maximum_bite: u16,
}

pub(crate) const RABBIT_DIET: [WildlifeDietEntry; 1] = [WildlifeDietEntry {
    resource: WildlifeResourceKind::LowHerbaceous,
    energy_per_unit: 20,
    maximum_bite: 6,
}];

pub(crate) const DEER_DIET: [WildlifeDietEntry; 2] = [
    WildlifeDietEntry {
        resource: WildlifeResourceKind::WoodyBrowse,
        energy_per_unit: 32,
        maximum_bite: 10,
    },
    WildlifeDietEntry {
        resource: WildlifeResourceKind::LowHerbaceous,
        energy_per_unit: 24,
        maximum_bite: 10,
    },
];

pub(crate) const MALLARD_DIET: [WildlifeDietEntry; 3] = [
    WildlifeDietEntry {
        resource: WildlifeResourceKind::AquaticInvertebrates,
        energy_per_unit: 28,
        maximum_bite: 7,
    },
    WildlifeDietEntry {
        resource: WildlifeResourceKind::AquaticVegetation,
        energy_per_unit: 22,
        maximum_bite: 7,
    },
    WildlifeDietEntry {
        resource: WildlifeResourceKind::SeedsAndSoftMast,
        energy_per_unit: 20,
        maximum_bite: 6,
    },
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WildlifeDietIntake {
    pub(crate) resource: Option<WildlifeResourceKind>,
    pub(crate) units: u16,
    pub(crate) energy: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct WildlifeResourceLedger {
    revision: u64,
    cells: BTreeMap<WildlifeForageCellPos, WildlifeResourceCell>,
    dirty: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeResourceStratum {
    potential: u32,
    available: u32,
    recovery_remainder: u32,
    recovered: u64,
    consumed: [u64; 3],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeResourceCell {
    strata: [WildlifeResourceStratum; WILDLIFE_RESOURCE_KIND_COUNT],
    terrain_revision: u64,
    last_sample_tick: u64,
    sample_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeResourcePayload {
    rule_revision: u32,
    cells: Vec<WildlifeResourceCellRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WildlifeResourceCellRecord {
    position: WildlifeForageCellPos,
    cell: WildlifeResourceCell,
}

impl WildlifeResourceLedger {
    pub(crate) fn from_saved(record: SavedDataRecord) -> ChunkStoreResult<Self> {
        if record.key != WILDLIFE_RESOURCE_SAVED_DATA_KEY
            || record.codec_version != WILDLIFE_RESOURCE_CODEC_VERSION
        {
            return Err(ChunkStoreError::InvalidData(format!(
                "unsupported wildlife resource record {} codec {}; disposable internal worlds with scalar forage must be regenerated",
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
            if record
                .cell
                .strata
                .iter()
                .any(|stratum| stratum.available > stratum.potential)
            {
                return Err(ChunkStoreError::InvalidData(
                    "wildlife resource cell has invalid availability".to_owned(),
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
                .map(|(position, cell)| WildlifeResourceCellRecord {
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
            if !position.overlaps_any_chunk(chunks) {
                continue;
            }
            for kind in WildlifeResourceKind::ALL {
                let stratum = &mut cell.strata[kind.index()];
                if stratum.available >= stratum.potential {
                    continue;
                }
                let numerator = stratum
                    .recovery_remainder
                    .saturating_add((stratum.potential / kind.recovery_days()).max(1));
                let recovered = numerator / RECOVERY_DENOMINATOR;
                stratum.recovery_remainder = numerator % RECOVERY_DENOMINATOR;
                let recovered = recovered.min(stratum.potential - stratum.available);
                if recovered > 0 {
                    stratum.available += recovered;
                    stratum.recovered = stratum.recovered.saturating_add(u64::from(recovered));
                    total = total.saturating_add(recovered);
                    self.dirty = true;
                }
            }
        }
        total
    }

    pub(crate) fn consume_diet_at<F>(
        &mut self,
        consumer: WildlifeForageConsumer,
        diet: &[WildlifeDietEntry],
        feet: BlockPos,
        energy: u16,
        activity_cost: u16,
        maximum_energy: u16,
        simulation_tick: u64,
        block_state_at: &F,
    ) -> WildlifeDietIntake
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        self.resample_if_due(feet, simulation_tick, block_state_at);
        let position = WildlifeForageCellPos::from_block(feet);
        let Some(cell) = self.cells.get_mut(&position) else {
            return WildlifeDietIntake::default();
        };
        for entry in diet {
            if !resource_site_is_compatible(entry.resource, feet, block_state_at) {
                continue;
            }
            let requested = wildlife_forage_request(
                energy,
                activity_cost,
                maximum_energy,
                entry.energy_per_unit,
                entry.maximum_bite,
            );
            let stratum = &mut cell.strata[entry.resource.index()];
            let consumed = u32::from(requested).min(stratum.available) as u16;
            if consumed == 0 {
                continue;
            }
            stratum.available -= u32::from(consumed);
            stratum.consumed[consumer.index()] =
                stratum.consumed[consumer.index()].saturating_add(u64::from(consumed));
            self.dirty = true;
            return WildlifeDietIntake {
                resource: Some(entry.resource),
                units: consumed,
                energy: consumed.saturating_mul(entry.energy_per_unit),
            };
        }
        WildlifeDietIntake::default()
    }

    fn resample_if_due<F>(&mut self, feet: BlockPos, simulation_tick: u64, block_state_at: &F)
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let position = WildlifeForageCellPos::from_block(feet);
        let needs_sample = self.cells.get(&position).is_none_or(|cell| {
            cell.sample_count == 0
                || simulation_tick.saturating_sub(cell.last_sample_tick) >= RESAMPLE_INTERVAL_TICKS
        });
        if !needs_sample {
            return;
        }
        let sampled = sample_resource_potential(feet, block_state_at);
        let cell = self.cells.entry(position).or_default();
        for kind in WildlifeResourceKind::ALL {
            let stratum = &mut cell.strata[kind.index()];
            let potential = sampled[kind.index()];
            stratum.available = if cell.sample_count == 0 {
                potential
            } else {
                stratum.available.min(potential)
            };
            stratum.potential = potential;
        }
        cell.terrain_revision = cell.terrain_revision.saturating_add(1);
        cell.last_sample_tick = simulation_tick;
        cell.sample_count = cell.sample_count.saturating_add(1);
        self.dirty = true;
    }

    pub(crate) fn snapshots(&self) -> Vec<WildlifeForageCellSnapshot> {
        self.cells
            .iter()
            .map(|(position, cell)| WildlifeForageCellSnapshot {
                position: *position,
                strata: std::array::from_fn(|index| {
                    let stratum = cell.strata[index];
                    WildlifeResourceStratumSnapshot {
                        potential: stratum.potential,
                        available: stratum.available,
                        recovered: stratum.recovered,
                        rabbit_consumed: stratum.consumed[0],
                        deer_consumed: stratum.consumed[1],
                        mallard_consumed: stratum.consumed[2],
                    }
                }),
                terrain_revision: cell.terrain_revision,
            })
            .collect()
    }
}

fn sample_resource_potential<F>(
    feet: BlockPos,
    block_state_at: &F,
) -> [u32; WILDLIFE_RESOURCE_KIND_COUNT]
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    let mut potential = [0_u32; WILDLIFE_RESOURCE_KIND_COUNT];
    for x in feet.x - 4..=feet.x + 4 {
        for z in feet.z - 4..=feet.z + 4 {
            for y in feet.y - 2..=feet.y + 3 {
                let Some(state) = block_state_at(BlockPos::new(x, y, z)) else {
                    continue;
                };
                let add = |values: &mut [u32; WILDLIFE_RESOURCE_KIND_COUNT],
                           kind: WildlifeResourceKind,
                           amount: u32| {
                    values[kind.index()] = values[kind.index()].saturating_add(amount);
                };
                if state == generated_block_state_id(GRASS_BLOCK) {
                    add(&mut potential, WildlifeResourceKind::LowHerbaceous, 10);
                    add(&mut potential, WildlifeResourceKind::SeedsAndSoftMast, 2);
                } else if is_low_plant(state) {
                    add(&mut potential, WildlifeResourceKind::LowHerbaceous, 18);
                    add(&mut potential, WildlifeResourceKind::SeedsAndSoftMast, 6);
                } else if is_flower(state) {
                    add(&mut potential, WildlifeResourceKind::LowHerbaceous, 20);
                    add(&mut potential, WildlifeResourceKind::SeedsAndSoftMast, 10);
                } else if is_woody_browse(state) {
                    add(&mut potential, WildlifeResourceKind::WoodyBrowse, 14);
                    add(&mut potential, WildlifeResourceKind::SeedsAndSoftMast, 2);
                } else if state == generated_block_state_id(SUGAR_CANE) {
                    add(&mut potential, WildlifeResourceKind::AquaticVegetation, 24);
                    add(&mut potential, WildlifeResourceKind::SeedsAndSoftMast, 4);
                } else if state == generated_block_state_id(LILY_PAD) {
                    add(&mut potential, WildlifeResourceKind::AquaticVegetation, 30);
                } else if block_fluid_kind(state) == BlockFluidKind::Water {
                    add(
                        &mut potential,
                        WildlifeResourceKind::AquaticInvertebrates,
                        3,
                    );
                }
            }
        }
    }
    potential.map(|value| value.min(4_000))
}

fn resource_site_is_compatible<F>(
    kind: WildlifeResourceKind,
    feet: BlockPos,
    block_state_at: &F,
) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    match kind {
        WildlifeResourceKind::LowHerbaceous => {
            block_state_at(feet.below()) == Some(generated_block_state_id(GRASS_BLOCK))
                || block_state_at(feet).is_some_and(|state| is_low_plant(state) || is_flower(state))
        }
        WildlifeResourceKind::WoodyBrowse => {
            nearby_state(feet, 2, 3, block_state_at, |state| is_woody_browse(state))
        }
        WildlifeResourceKind::SeedsAndSoftMast => {
            block_state_at(feet.below()) == Some(generated_block_state_id(GRASS_BLOCK))
                || nearby_state(feet, 2, 2, block_state_at, |state| {
                    is_low_plant(state)
                        || is_flower(state)
                        || state == generated_block_state_id(SUGAR_CANE)
                })
        }
        WildlifeResourceKind::AquaticVegetation => {
            at_or_adjacent_water(feet, block_state_at)
                && nearby_state(feet, 3, 2, block_state_at, |state| {
                    state == generated_block_state_id(SUGAR_CANE)
                        || state == generated_block_state_id(LILY_PAD)
                })
        }
        WildlifeResourceKind::AquaticInvertebrates => at_or_adjacent_water(feet, block_state_at),
    }
}

fn at_or_adjacent_water<F>(feet: BlockPos, blocks: &F) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    [
        feet,
        feet.below(),
        feet.offset(1, 0, 0),
        feet.offset(-1, 0, 0),
        feet.offset(0, 0, 1),
        feet.offset(0, 0, -1),
    ]
    .into_iter()
    .any(|pos| blocks(pos).is_some_and(|state| block_fluid_kind(state) == BlockFluidKind::Water))
}

fn nearby_state<F, P>(feet: BlockPos, radius: i32, height: i32, blocks: &F, predicate: P) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
    P: Fn(BlockStateId) -> bool,
{
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            for dy in -1..=height {
                if blocks(feet.offset(dx, dy, dz)).is_some_and(&predicate) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_low_plant(state: BlockStateId) -> bool {
    matches!(
        state,
        value if value == generated_block_state_id(GRASS)
            || value == generated_block_state_id(FERN)
            || value == generated_block_state_id(LARGE_FERN_LOWER)
            || value == generated_block_state_id(LARGE_FERN_UPPER)
            || value == generated_block_state_id(TALL_GRASS_LOWER)
            || value == generated_block_state_id(TALL_GRASS_UPPER)
    )
}

fn is_flower(state: BlockStateId) -> bool {
    state == generated_block_state_id(DANDELION) || state == generated_block_state_id(POPPY)
}

fn is_woody_browse(state: BlockStateId) -> bool {
    matches!(
        state,
        value if value == generated_block_state_id(OAK_LEAVES)
            || value == generated_block_state_id(BIRCH_LEAVES)
            || value == generated_block_state_id(SPRUCE_LEAVES)
            || value == generated_block_state_id(DARK_OAK_LEAVES)
            || value == generated_block_state_id(ACACIA_LEAVES)
            || value == generated_block_state_id(JUNGLE_LEAVES)
    )
}

fn wildlife_forage_request(
    energy: u16,
    activity_cost: u16,
    maximum_energy: u16,
    energy_per_forage: u16,
    maximum_bite: u16,
) -> u16 {
    if energy_per_forage == 0 || maximum_bite == 0 {
        return 0;
    }
    maximum_energy
        .saturating_sub(energy)
        .saturating_add(activity_cost)
        .saturating_add(energy_per_forage - 1)
        .div_euclid(energy_per_forage)
        .min(maximum_bite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{AIR, DIRT, WATER};

    fn mixed_habitat(pos: BlockPos) -> Option<BlockStateId> {
        let raw = if pos.y < 63 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos == BlockPos::new(3, 64, 2) {
            OAK_LEAVES
        } else if pos == BlockPos::new(2, 64, 3) {
            DANDELION
        } else if pos.y == 64 && pos.x >= 6 {
            WATER
        } else if pos == BlockPos::new(5, 64, 2) {
            SUGAR_CANE
        } else {
            AIR
        };
        Some(generated_block_state_id(raw))
    }

    #[test]
    fn typed_consumption_recovery_and_persistence_conserve_bounds() {
        let mut ledger = WildlifeResourceLedger::default();
        let feet = BlockPos::new(2, 64, 2);
        let intake = ledger.consume_diet_at(
            WildlifeForageConsumer::Deer,
            &DEER_DIET,
            feet,
            0,
            1,
            1_000,
            20,
            &mixed_habitat,
        );
        assert_eq!(intake.resource, Some(WildlifeResourceKind::WoodyBrowse));
        assert!(intake.units > 0);
        let before = ledger.snapshots()[0];
        for _ in 0..1_200 {
            ledger.recover_loaded(&BTreeSet::from([ChunkPos::new(0, 0)]));
        }
        let after = ledger.snapshots()[0];
        let before_browse = before.stratum(WildlifeResourceKind::WoodyBrowse);
        let after_browse = after.stratum(WildlifeResourceKind::WoodyBrowse);
        assert!(after_browse.available > before_browse.available);
        assert!(after_browse.available <= after_browse.potential);
        assert_eq!(after_browse.deer_consumed, u64::from(intake.units));

        let restored = WildlifeResourceLedger::from_saved(ledger.saved_record().unwrap()).unwrap();
        assert_eq!(restored.snapshots(), ledger.snapshots());
    }

    #[test]
    fn absent_resource_has_zero_potential_and_cannot_be_eaten() {
        let air = |_pos: BlockPos| Some(generated_block_state_id(AIR));
        let mut ledger = WildlifeResourceLedger::default();
        let intake = ledger.consume_diet_at(
            WildlifeForageConsumer::Rabbit,
            &RABBIT_DIET,
            BlockPos::new(1, 64, 1),
            0,
            1,
            1_000,
            20,
            &air,
        );
        assert_eq!(intake, WildlifeDietIntake::default());
        assert!(
            ledger.snapshots()[0]
                .strata
                .iter()
                .all(|stratum| stratum.potential == 0 && stratum.available == 0)
        );
    }

    #[test]
    fn mallards_draw_aquatic_food_only_at_reachable_water() {
        let mut wetland = WildlifeResourceLedger::default();
        let intake = wetland.consume_diet_at(
            WildlifeForageConsumer::Mallard,
            &MALLARD_DIET,
            BlockPos::new(5, 64, 2),
            0,
            2,
            1_000,
            20,
            &mixed_habitat,
        );
        assert_eq!(
            intake.resource,
            Some(WildlifeResourceKind::AquaticInvertebrates)
        );
        assert!(intake.units > 0);
        assert_eq!(
            wetland.snapshots()[0]
                .stratum(WildlifeResourceKind::AquaticInvertebrates)
                .mallard_consumed,
            u64::from(intake.units)
        );

        let dry = |pos: BlockPos| {
            Some(generated_block_state_id(if pos.y == 63 {
                GRASS_BLOCK
            } else {
                AIR
            }))
        };
        let mut dry_ledger = WildlifeResourceLedger::default();
        let dry_intake = dry_ledger.consume_diet_at(
            WildlifeForageConsumer::Mallard,
            &MALLARD_DIET[..2],
            BlockPos::new(5, 64, 2),
            0,
            2,
            1_000,
            20,
            &dry,
        );
        assert_eq!(dry_intake, WildlifeDietIntake::default());
    }

    #[test]
    fn resampling_does_not_refill_consumed_resources() {
        let mut ledger = WildlifeResourceLedger::default();
        let feet = BlockPos::new(2, 64, 2);
        let first = ledger.consume_diet_at(
            WildlifeForageConsumer::Rabbit,
            &RABBIT_DIET,
            feet,
            0,
            1,
            1_000,
            20,
            &mixed_habitat,
        );
        assert!(first.units > 0);
        let before = ledger.snapshots()[0]
            .stratum(WildlifeResourceKind::LowHerbaceous)
            .available;
        let _ = ledger.consume_diet_at(
            WildlifeForageConsumer::Rabbit,
            &RABBIT_DIET,
            feet,
            1_000,
            0,
            1_000,
            1_220,
            &mixed_habitat,
        );
        let after = ledger.snapshots()[0]
            .stratum(WildlifeResourceKind::LowHerbaceous)
            .available;
        assert_eq!(after, before);
    }

    #[test]
    fn scalar_codec_is_rejected_with_an_actionable_error() {
        let old =
            SavedDataRecord::new(WILDLIFE_RESOURCE_SAVED_DATA_KEY, 1, 1, b"{}".to_vec()).unwrap();
        let error = WildlifeResourceLedger::from_saved(old).unwrap_err();
        assert!(error.to_string().contains("must be regenerated"));
    }
}
