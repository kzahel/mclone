//! Scheduler-owned initial `ChunkStatus::Light` inputs.
//!
//! Worldgen produces block facts for `FEATURES`; the scheduler owns the
//! follow-up light status. This module keeps the raw input carriers and bridge
//! call out of the already large scheduler module.

use mclone_core::{
    ChunkPos, ChunkSnapshot, ChunkStatus, PackedLightSection, SECTION_HEIGHT,
    block_to_section_coord,
};
use mclone_light::{
    BlockLightWorld, BlockPosKey, DataLayer, LevelLightEngine, LightLayer, SkyLightWorld,
    section_as_long,
};
use mclone_worldgen::block::RawBlockId;
use mclone_worldgen::levelgen::{GeneratedChunk, MutableChunkBlockBuffer};

use crate::lighting_seed::provisional_sky_light_includes_chunk;
use crate::persistence::{ChunkStoreError, ChunkStoreResult};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingLightStatus {
    pub(crate) pos: ChunkPos,
    pub(crate) feature_snapshot: ChunkSnapshot,
    raw_blocks: Vec<RawBlockId>,
    neighbor_blocks: Vec<(ChunkPos, Vec<RawBlockId>)>,
}

impl PendingLightStatus {
    pub(crate) fn from_parts(
        pos: ChunkPos,
        feature_snapshot: ChunkSnapshot,
        raw_blocks: Vec<RawBlockId>,
        neighbor_blocks: Vec<(ChunkPos, Vec<RawBlockId>)>,
    ) -> Self {
        Self {
            pos,
            feature_snapshot,
            raw_blocks,
            neighbor_blocks,
        }
    }

    pub(crate) fn from_feature_publication<'a>(
        pos: ChunkPos,
        feature_snapshot: ChunkSnapshot,
        chunk: &GeneratedChunk,
        generated_chunks: impl IntoIterator<Item = (&'a ChunkPos, &'a GeneratedChunk)>,
        retained_dependencies: impl IntoIterator<Item = (&'a ChunkPos, &'a MutableChunkBlockBuffer)>,
    ) -> Self {
        let neighbor_blocks = retained_dependencies
            .into_iter()
            .filter_map(|(neighbor_pos, buffer)| {
                (*neighbor_pos != pos && provisional_sky_light_includes_chunk(pos, *neighbor_pos))
                    .then_some((*neighbor_pos, buffer.blocks.as_slice()))
            })
            .chain(
                generated_chunks
                    .into_iter()
                    .filter_map(|(neighbor_pos, chunk)| {
                        (*neighbor_pos != pos
                            && provisional_sky_light_includes_chunk(pos, *neighbor_pos))
                        .then_some((*neighbor_pos, chunk.blocks()))
                    }),
            )
            .map(|(neighbor_pos, blocks)| (neighbor_pos, blocks.to_vec()))
            .collect();

        Self {
            pos,
            feature_snapshot,
            raw_blocks: chunk.blocks().to_vec(),
            neighbor_blocks,
        }
    }

    pub(crate) fn raw_blocks(&self) -> &[RawBlockId] {
        &self.raw_blocks
    }

    pub(crate) fn neighbor_blocks(&self) -> &[(ChunkPos, Vec<RawBlockId>)] {
        &self.neighbor_blocks
    }
}

#[derive(Debug)]
pub(crate) struct PendingLightStatusBatch {
    statuses: Vec<PendingLightStatus>,
}

impl PendingLightStatusBatch {
    pub(crate) fn new(statuses: Vec<PendingLightStatus>) -> Self {
        Self { statuses }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    pub(crate) fn target_count(&self) -> usize {
        self.statuses.len()
    }

    pub(crate) fn into_statuses(self) -> Vec<PendingLightStatus> {
        self.statuses
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
    snapshot.light_sections =
        collect_hydrated_light_sections(&engine, snapshot.pos, min_section_y, section_count);
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
) -> Vec<PackedLightSection> {
    let mut sections = Vec::new();
    for section_offset in 0..section_count {
        let section_y = min_section_y + section_offset;
        let section = section_as_long(pos.x, section_y, pos.z);
        let sky = engine
            .sky_engine()
            .storage()
            .get_visible_data_layer(section)
            .and_then(|layer| layer.clone().into_bytes());
        let block = engine
            .block_engine()
            .storage()
            .get_visible_data_layer(section)
            .and_then(|layer| layer.clone().into_bytes());
        if sky.is_some() || block.is_some() {
            sections.push(PackedLightSection::new(section_y, sky, block));
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
}
