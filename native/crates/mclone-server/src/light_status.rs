//! Scheduler-owned initial `ChunkStatus::Light` inputs.
//!
//! Worldgen produces block facts for `FEATURES`; the scheduler owns the
//! follow-up light status. This module keeps the raw input carriers and bridge
//! call out of the already large scheduler module.

use std::collections::BTreeMap;
use std::sync::Arc;

use mclone_core::{
    BlockStateId, ChunkPos, ChunkSnapshot, ChunkStatus, PackedChunkSection, PackedLightSection,
    SECTION_HEIGHT, block_to_section_coord,
};
use mclone_light::{
    BlockLightWorld, BlockPosKey, DataLayer, LevelLightEngine, LightLayer, LightSectionRange,
    SkyLightWorld, section_as_long,
};
use mclone_worldgen::block::RawBlockId;

use crate::ChunkJobId;
use crate::persistence::{ChunkStoreError, ChunkStoreResult, ScheduledTickRecord};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct LightRequestToken {
    pub(crate) id: u64,
    pub(crate) pos: ChunkPos,
    pub(crate) feature_revision: mclone_core::ChunkRevision,
}

impl LightRequestToken {
    pub(crate) const fn new(
        id: u64,
        pos: ChunkPos,
        feature_revision: mclone_core::ChunkRevision,
    ) -> Self {
        Self {
            id,
            pos,
            feature_revision,
        }
    }

    fn synthetic(pos: ChunkPos, feature_revision: mclone_core::ChunkRevision) -> Self {
        Self::new(0, pos, feature_revision)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingLightDemand {
    pub(crate) token: LightRequestToken,
    pub(crate) feature_snapshot: ChunkSnapshot,
    pub(crate) scheduled_block_ticks: Vec<ScheduledTickRecord>,
    pub(crate) scheduled_fluid_ticks: Vec<ScheduledTickRecord>,
    pub(crate) source_job: Option<ChunkJobId>,
}

impl PendingLightDemand {
    pub(crate) fn new(
        token: LightRequestToken,
        feature_snapshot: ChunkSnapshot,
        scheduled_block_ticks: Vec<ScheduledTickRecord>,
        scheduled_fluid_ticks: Vec<ScheduledTickRecord>,
        source_job: Option<ChunkJobId>,
    ) -> Self {
        debug_assert_eq!(token.pos, feature_snapshot.pos);
        debug_assert_eq!(token.feature_revision, feature_snapshot.revision);
        Self {
            token,
            feature_snapshot,
            scheduled_block_ticks,
            scheduled_fluid_ticks,
            source_job,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingLightStatus {
    pub(crate) token: LightRequestToken,
    pub(crate) pos: ChunkPos,
    pub(crate) feature_snapshot: ChunkSnapshot,
    pub(crate) scheduled_block_ticks: Vec<ScheduledTickRecord>,
    pub(crate) scheduled_fluid_ticks: Vec<ScheduledTickRecord>,
    raw_blocks: Arc<[RawBlockId]>,
    neighbor_blocks: Vec<(ChunkPos, Arc<[RawBlockId]>)>,
}

impl PendingLightStatus {
    pub(crate) fn from_shared_demand(
        demand: PendingLightDemand,
        raw_blocks: Arc<[RawBlockId]>,
        neighbor_blocks: Vec<(ChunkPos, Arc<[RawBlockId]>)>,
    ) -> Self {
        debug_assert_eq!(demand.token.pos, demand.feature_snapshot.pos);
        debug_assert_eq!(
            demand.token.feature_revision,
            demand.feature_snapshot.revision
        );
        Self {
            token: demand.token,
            pos: demand.token.pos,
            feature_snapshot: demand.feature_snapshot,
            scheduled_block_ticks: demand.scheduled_block_ticks,
            scheduled_fluid_ticks: demand.scheduled_fluid_ticks,
            raw_blocks,
            neighbor_blocks,
        }
    }

    pub(crate) fn from_parts(
        pos: ChunkPos,
        feature_snapshot: ChunkSnapshot,
        raw_blocks: Vec<RawBlockId>,
        neighbor_blocks: Vec<(ChunkPos, Vec<RawBlockId>)>,
    ) -> Self {
        let token = LightRequestToken::synthetic(pos, feature_snapshot.revision);
        Self::from_parts_with_token(token, feature_snapshot, raw_blocks, neighbor_blocks)
    }

    pub(crate) fn from_parts_with_token(
        token: LightRequestToken,
        feature_snapshot: ChunkSnapshot,
        raw_blocks: Vec<RawBlockId>,
        neighbor_blocks: Vec<(ChunkPos, Vec<RawBlockId>)>,
    ) -> Self {
        debug_assert_eq!(token.pos, feature_snapshot.pos);
        debug_assert_eq!(token.feature_revision, feature_snapshot.revision);
        Self {
            token,
            pos: token.pos,
            feature_snapshot,
            scheduled_block_ticks: Vec::new(),
            scheduled_fluid_ticks: Vec::new(),
            raw_blocks: raw_blocks.into(),
            neighbor_blocks: neighbor_blocks
                .into_iter()
                .map(|(pos, blocks)| (pos, blocks.into()))
                .collect(),
        }
    }

    pub(crate) fn raw_blocks(&self) -> &[RawBlockId] {
        &self.raw_blocks
    }

    pub(crate) fn neighbor_blocks(&self) -> &[(ChunkPos, Arc<[RawBlockId]>)] {
        &self.neighbor_blocks
    }

    fn owned_metadata_bytes_estimate(&self) -> usize {
        std::mem::size_of_val(self)
            .saturating_add(snapshot_heap_bytes_estimate(&self.feature_snapshot))
            .saturating_add(ticks_heap_bytes_estimate(&self.scheduled_block_ticks))
            .saturating_add(ticks_heap_bytes_estimate(&self.scheduled_fluid_ticks))
            .saturating_add(
                self.neighbor_blocks
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(ChunkPos, Arc<[RawBlockId]>)>()),
            )
    }

    fn maximum_light_output_bytes_estimate(&self) -> usize {
        let section_count = usize::try_from(self.feature_snapshot.height / SECTION_HEIGHT)
            .unwrap_or_default()
            .saturating_add(2);
        section_count.saturating_mul(
            std::mem::size_of::<PackedLightSection>()
                .saturating_add(2 * mclone_core::LIGHT_DATA_LAYER_BYTE_COUNT),
        )
    }
}

#[derive(Debug)]
pub(crate) struct PendingLightStatusBatch {
    statuses: Vec<PendingLightStatus>,
    unique_input_chunks: BTreeMap<ChunkPos, Arc<[RawBlockId]>>,
    owned_input_bytes: usize,
    lifecycle_owned_bytes_estimate: usize,
}

impl PendingLightStatusBatch {
    pub(crate) fn new(mut statuses: Vec<PendingLightStatus>) -> Self {
        let mut unique_input_chunks = BTreeMap::<ChunkPos, Arc<[RawBlockId]>>::new();
        for status in &mut statuses {
            status.raw_blocks =
                canonical_light_input(&mut unique_input_chunks, status.pos, &status.raw_blocks);
        }
        for status in &mut statuses {
            for (pos, blocks) in &mut status.neighbor_blocks {
                *blocks = if let Some(target_or_neighbor) = unique_input_chunks.get(pos) {
                    Arc::clone(target_or_neighbor)
                } else {
                    canonical_light_input(&mut unique_input_chunks, *pos, blocks)
                };
            }
        }
        Self::from_shared_parts(statuses, unique_input_chunks)
    }

    pub(crate) fn from_shared_parts(
        statuses: Vec<PendingLightStatus>,
        unique_input_chunks: BTreeMap<ChunkPos, Arc<[RawBlockId]>>,
    ) -> Self {
        debug_assert!(statuses.iter().all(|status| {
            unique_input_chunks
                .get(&status.pos)
                .is_some_and(|blocks| Arc::ptr_eq(blocks, &status.raw_blocks))
                && status.neighbor_blocks.iter().all(|(pos, blocks)| {
                    unique_input_chunks
                        .get(pos)
                        .is_some_and(|shared| Arc::ptr_eq(shared, blocks))
                })
        }));
        let owned_input_bytes = unique_input_chunks.values().fold(0_usize, |bytes, blocks| {
            bytes.saturating_add(
                blocks
                    .len()
                    .saturating_mul(std::mem::size_of::<RawBlockId>()),
            )
        });
        let lifecycle_owned_bytes_estimate = statuses
            .iter()
            .fold(owned_input_bytes, |bytes, status| {
                bytes
                    .saturating_add(status.owned_metadata_bytes_estimate())
                    .saturating_add(status.maximum_light_output_bytes_estimate())
            })
            .saturating_add(
                unique_input_chunks
                    .len()
                    .saturating_mul(std::mem::size_of::<(ChunkPos, Arc<[RawBlockId]>)>()),
            );
        Self {
            statuses,
            unique_input_chunks,
            owned_input_bytes,
            lifecycle_owned_bytes_estimate,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    pub(crate) fn target_count(&self) -> usize {
        self.statuses.len()
    }

    pub(crate) fn unique_input_count(&self) -> usize {
        self.unique_input_chunks.len()
    }

    pub(crate) const fn owned_input_bytes(&self) -> usize {
        self.owned_input_bytes
    }

    pub(crate) const fn lifecycle_owned_bytes_estimate(&self) -> usize {
        self.lifecycle_owned_bytes_estimate
    }

    pub(crate) fn tokens(&self) -> impl ExactSizeIterator<Item = LightRequestToken> + '_ {
        self.statuses.iter().map(|status| status.token)
    }

    pub(crate) fn into_statuses(self) -> Vec<PendingLightStatus> {
        self.statuses
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<PendingLightStatus>,
        BTreeMap<ChunkPos, Arc<[RawBlockId]>>,
    ) {
        (self.statuses, self.unique_input_chunks)
    }
}

pub(crate) fn snapshot_heap_bytes_estimate(snapshot: &ChunkSnapshot) -> usize {
    let section_bytes = snapshot.sections.iter().fold(0_usize, |bytes, section| {
        bytes
            .saturating_add(
                section
                    .palette_state_ids
                    .capacity()
                    .saturating_mul(std::mem::size_of::<BlockStateId>()),
            )
            .saturating_add(
                section
                    .packed_block_indices
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u64>()),
            )
    });
    let light_bytes = snapshot
        .light_sections
        .iter()
        .fold(0_usize, |bytes, section| {
            bytes
                .saturating_add(section.sky.as_ref().map_or(0, |values| values.capacity()))
                .saturating_add(section.block.as_ref().map_or(0, |values| values.capacity()))
        });
    snapshot
        .biomes
        .capacity()
        .saturating_mul(std::mem::size_of::<i32>())
        .saturating_add(
            snapshot
                .sections
                .capacity()
                .saturating_mul(std::mem::size_of::<PackedChunkSection>()),
        )
        .saturating_add(section_bytes)
        .saturating_add(
            snapshot
                .light_sections
                .capacity()
                .saturating_mul(std::mem::size_of::<PackedLightSection>()),
        )
        .saturating_add(light_bytes)
}

pub(crate) fn ticks_heap_bytes_estimate(ticks: &Vec<ScheduledTickRecord>) -> usize {
    ticks.iter().fold(
        ticks
            .capacity()
            .saturating_mul(std::mem::size_of::<ScheduledTickRecord>()),
        |bytes, tick| bytes.saturating_add(tick.target.capacity()),
    )
}

fn canonical_light_input(
    unique_input_chunks: &mut BTreeMap<ChunkPos, Arc<[RawBlockId]>>,
    pos: ChunkPos,
    blocks: &Arc<[RawBlockId]>,
) -> Arc<[RawBlockId]> {
    if let Some(existing) = unique_input_chunks.get(&pos) {
        debug_assert_eq!(
            existing.as_ref(),
            blocks.as_ref(),
            "Light batch supplied conflicting raw blocks for ({}, {})",
            pos.x,
            pos.z
        );
        return Arc::clone(existing);
    }
    unique_input_chunks.insert(pos, Arc::clone(blocks));
    Arc::clone(blocks)
}

#[cfg(test)]
mod shared_input_tests {
    use super::*;
    use mclone_core::{ChunkRevision, ChunkStatus};

    fn snapshot(pos: ChunkPos) -> ChunkSnapshot {
        ChunkSnapshot {
            pos,
            status: ChunkStatus::Features,
            revision: ChunkRevision(1),
            min_y: 0,
            height: 16,
            biomes: Vec::new(),
            sections: Vec::new(),
            light_correct: false,
            light_sections: Vec::new(),
        }
    }

    #[test]
    fn overlapping_statuses_share_each_unique_raw_input_once() {
        let center = ChunkPos::new(0, 0);
        let east = ChunkPos::new(1, 0);
        let center_blocks = vec![1_u16; 16];
        let east_blocks = vec![2_u16; 16];
        let center_status = PendingLightStatus::from_parts(
            center,
            snapshot(center),
            center_blocks.clone(),
            vec![(east, east_blocks.clone())],
        );
        let east_status = PendingLightStatus::from_parts(
            east,
            snapshot(east),
            east_blocks,
            vec![(center, center_blocks)],
        );

        let batch = PendingLightStatusBatch::new(vec![center_status, east_status]);

        assert_eq!(batch.target_count(), 2);
        assert_eq!(batch.unique_input_count(), 2);
        assert_eq!(batch.owned_input_bytes(), 64);
        assert!(batch.lifecycle_owned_bytes_estimate() > 64);
        let statuses = batch.into_statuses();
        assert!(Arc::ptr_eq(
            &statuses[0].raw_blocks,
            &statuses[1].neighbor_blocks[0].1
        ));
        assert!(Arc::ptr_eq(
            &statuses[1].raw_blocks,
            &statuses[0].neighbor_blocks[0].1
        ));
    }
}

pub(crate) fn hydrate_loaded_light_snapshot(
    mut snapshot: ChunkSnapshot,
) -> ChunkStoreResult<ChunkSnapshot> {
    if snapshot.status < ChunkStatus::Light || !snapshot.light_correct {
        return Ok(snapshot);
    }

    let mut engine = LevelLightEngine::new(LoadedLightWorld, LoadedLightWorld);
    let min_section_y = block_to_section_coord(snapshot.min_y);
    let section_count = snapshot.height / SECTION_HEIGHT;
    let column = section_as_long(snapshot.pos.x, 0, snapshot.pos.z);
    engine.retain_data(column, true);

    for section_offset in 0..section_count {
        let section_y = min_section_y + section_offset;
        let section = section_as_long(snapshot.pos.x, section_y, snapshot.pos.z);
        engine.update_section_status(section, false);
        engine.enable_light_sources(section, true);
    }

    for section in &snapshot.light_sections {
        let section_key = section_as_long(snapshot.pos.x, section.section_y, snapshot.pos.z);
        if let Some(sky) = &section.sky {
            engine.queue_section_data(
                LightLayer::Sky,
                section_key,
                Some(data_layer_from_packed_bytes("sky", section.section_y, sky)?),
                true,
            );
        }
        if let Some(block) = &section.block {
            engine.queue_section_data(
                LightLayer::Block,
                section_key,
                Some(data_layer_from_packed_bytes(
                    "block",
                    section.section_y,
                    block,
                )?),
                true,
            );
        }
    }

    engine.accept_queued_section_data();
    engine.retain_data(column, false);
    snapshot.light_sections = collect_hydrated_light_sections(
        &engine,
        snapshot.pos,
        min_section_y,
        section_count,
        &snapshot.light_sections,
    );
    Ok(snapshot)
}

fn data_layer_from_packed_bytes(
    layer_name: &str,
    section_y: i32,
    bytes: &[u8],
) -> ChunkStoreResult<DataLayer> {
    DataLayer::from_vec(bytes.to_vec()).map_err(|error| {
        ChunkStoreError::InvalidData(format!(
            "invalid {layer_name} light data for section {section_y}: {error}"
        ))
    })
}

fn collect_hydrated_light_sections(
    engine: &LevelLightEngine<LoadedLightWorld, LoadedLightWorld>,
    pos: ChunkPos,
    min_section_y: i32,
    section_count: i32,
    persisted_sections: &[PackedLightSection],
) -> Vec<PackedLightSection> {
    let mut sections = Vec::new();
    let light_range = LightSectionRange::from_world(
        min_section_y * SECTION_HEIGHT,
        section_count * SECTION_HEIGHT,
    );
    for persisted in persisted_sections {
        if !light_range.contains_light_section(persisted.section_y) {
            continue;
        }
        let section = section_as_long(pos.x, persisted.section_y, pos.z);
        let sky = persisted.sky.as_ref().and_then(|_| {
            engine
                .sky_engine()
                .storage()
                .get_visible_data_layer(section)
                .map(|layer| layer.to_packed_bytes())
                .or_else(|| persisted.sky.clone())
        });
        let block = persisted.block.as_ref().and_then(|_| {
            engine
                .block_engine()
                .storage()
                .get_visible_data_layer(section)
                .map(|layer| layer.to_packed_bytes())
                .or_else(|| persisted.block.clone())
        });
        if sky.is_some() || block.is_some() {
            sections.push(PackedLightSection::new(persisted.section_y, sky, block));
        }
    }
    sections
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoadedLightWorld;

impl BlockLightWorld for LoadedLightWorld {
    fn light_emission(&self, _pos: BlockPosKey) -> u8 {
        0
    }

    fn light_opacity(&self, _pos: BlockPosKey) -> Option<u8> {
        None
    }
}

impl SkyLightWorld for LoadedLightWorld {
    fn light_opacity(&self, _pos: BlockPosKey) -> Option<u8> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkRevision, LIGHT_DATA_LAYER_BYTE_COUNT,
    };

    #[test]
    fn loaded_light_hydration_roundtrips_persisted_section_bytes() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(7),
            0,
            SECTION_HEIGHT,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                0,
                Some(vec![0xAB; LIGHT_DATA_LAYER_BYTE_COUNT]),
                Some(vec![0xCD; LIGHT_DATA_LAYER_BYTE_COUNT]),
            )],
        );

        let hydrated = hydrate_loaded_light_snapshot(snapshot.clone()).unwrap();

        assert_eq!(hydrated.status, ChunkStatus::Light);
        assert!(hydrated.light_correct);
        assert_eq!(hydrated.light_sections, snapshot.light_sections);
    }

    #[test]
    fn loaded_light_hydration_does_not_manufacture_unpersisted_layers() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(7),
            0,
            SECTION_HEIGHT * 2,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2],
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                1,
                Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT]),
                None,
            )],
        );

        let hydrated = hydrate_loaded_light_snapshot(snapshot.clone()).unwrap();

        assert_eq!(hydrated.light_sections, snapshot.light_sections);
    }

    #[test]
    fn loaded_light_hydration_preserves_persisted_boundary_layers() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(7),
            0,
            SECTION_HEIGHT,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
        .with_light_sections(
            true,
            vec![
                PackedLightSection::new(
                    -1,
                    Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT]),
                ),
                PackedLightSection::new(
                    0,
                    Some(vec![0x44; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    Some(vec![0x55; LIGHT_DATA_LAYER_BYTE_COUNT]),
                ),
                PackedLightSection::new(
                    1,
                    Some(vec![0xFF; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT]),
                ),
            ],
        );

        let hydrated = hydrate_loaded_light_snapshot(snapshot.clone()).unwrap();

        assert_eq!(hydrated.light_sections, snapshot.light_sections);
    }
}
