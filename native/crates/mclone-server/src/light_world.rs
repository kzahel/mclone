//! Retained initial light-world owner.
//!
//! This is the native server's first small step toward Java's
//! `ThreadedLevelLightEngine` shape: the light worker owns world block facts and
//! a persistent `LevelLightEngine`, while the scheduler keeps chunk-status
//! publication.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use mclone_core::{
    CHUNK_WIDTH, ChunkPos, PackedLightSection, SECTION_HEIGHT, block_to_section_coord,
    chunk_block_index, local_block_coord,
};
use mclone_light::{
    BlockLightWorld, BlockPosKey, LevelLightEngine, SectionPosKey, SkyLightWorld,
    block_pos_as_long, block_pos_get_x, block_pos_get_y, block_pos_get_z, section_as_long,
};
use mclone_worldgen::block::{RawBlockId, block_light_emission, block_light_opacity, is_air_like};

use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_status::{PendingLightStatus, PendingLightStatusBatch};
use crate::timing::{timing_elapsed_us, timing_start};

#[derive(Debug)]
pub(crate) struct RetainedInitialLightState {
    world: SharedRetainedLightWorld,
    engine: LevelLightEngine<SharedRetainedLightWorld, SharedRetainedLightWorld>,
}

impl RetainedInitialLightState {
    pub(crate) fn new() -> Self {
        let world = SharedRetainedLightWorld::default();
        let engine = LevelLightEngine::new(world.clone(), world.clone());
        Self { world, engine }
    }

    pub(crate) fn compute_batch(
        &mut self,
        batch: PendingLightStatusBatch,
    ) -> Vec<(
        PendingLightStatus,
        Vec<PackedLightSection>,
        LevelLightComputationTiming,
    )> {
        let statuses = batch.into_statuses();
        if statuses.is_empty() {
            return Vec::new();
        }

        let total_start = timing_start();
        let mut timing = LevelLightComputationTiming::default();
        let min_y = statuses[0].feature_snapshot.min_y;
        let height = statuses[0].feature_snapshot.height;
        let input_chunks = batch_input_chunks(&statuses);

        let start = timing_start();
        let changed_chunks = {
            let mut world = self.world.borrow_mut();
            world.configure(min_y, height);
            input_chunks
                .iter()
                .filter_map(|(&pos, blocks)| world.upsert_chunk(pos, blocks).then_some(pos))
                .collect::<Vec<_>>()
        };
        timing.world_init_us = timing_elapsed_us(start);

        let start = timing_start();
        let active_sections = self.world.borrow().section_statuses_for(&changed_chunks);
        timing.active_sections_us = timing_elapsed_us(start);
        let start = timing_start();
        let block_sources = self
            .world
            .borrow()
            .block_emission_sources_for(&changed_chunks);
        timing.block_source_scan_us = timing_elapsed_us(start);

        let start = timing_start();
        for (section, is_empty) in active_sections {
            self.engine.update_section_status(section, is_empty);
        }
        for chunk_pos in &changed_chunks {
            self.engine
                .enable_light_sources(section_as_long(chunk_pos.x, 0, chunk_pos.z), true);
        }
        timing.section_setup_us = timing_elapsed_us(start);
        timing.sky_source_scan_us = 0;
        timing.sky_source_enqueue_us = 0;
        let start = timing_start();
        for (source, emission) in block_sources {
            self.engine.on_block_emission_increase(source, emission);
        }
        timing.block_source_enqueue_us = timing_elapsed_us(start);
        let start = timing_start();
        let run_report = self.engine.run_all_updates_report();
        timing.run_updates_us = timing_elapsed_us(start);
        timing.run_update_iterations = run_report.iterations;
        timing.block_run_update_calls = run_report.block.calls;
        timing.sky_run_update_calls = run_report.sky.calls;
        timing.block_run_update_processed_nodes = run_report.block.processed_nodes;
        timing.sky_run_update_processed_nodes = run_report.sky.processed_nodes;
        timing.max_block_run_update_queue_before = run_report.block.queue_before;
        timing.max_sky_run_update_queue_before = run_report.sky.queue_before;
        timing.final_block_run_update_queue_after = run_report.block.queue_after;
        timing.final_sky_run_update_queue_after = run_report.sky.queue_after;
        timing.block_run_updates_us = run_report.block.run_updates_us;
        timing.sky_run_updates_us = run_report.sky.run_updates_us;

        let start = timing_start();
        let min_section_y = block_to_section_coord(min_y);
        let section_count = height / SECTION_HEIGHT;
        let mut sections_by_chunk = BTreeMap::new();
        for status in &statuses {
            sections_by_chunk.insert(
                status.pos,
                collect_light_sections(&self.engine, status.pos, min_section_y, section_count),
            );
        }
        timing.collect_sections_us = timing_elapsed_us(start);
        timing.total_us = timing_elapsed_us(total_start);

        statuses
            .into_iter()
            .enumerate()
            .map(|(index, status)| {
                let pos = status.pos;
                (
                    status,
                    sections_by_chunk.remove(&pos).unwrap_or_default(),
                    if index == 0 {
                        timing
                    } else {
                        LevelLightComputationTiming::default()
                    },
                )
            })
            .collect()
    }
}

fn batch_input_chunks(statuses: &[PendingLightStatus]) -> BTreeMap<ChunkPos, &[RawBlockId]> {
    let mut chunks = BTreeMap::new();
    for status in statuses {
        debug_assert_eq!(
            status.feature_snapshot.min_y,
            statuses[0].feature_snapshot.min_y
        );
        debug_assert_eq!(
            status.feature_snapshot.height,
            statuses[0].feature_snapshot.height
        );
        chunks.entry(status.pos).or_insert(status.raw_blocks());
        for (neighbor_pos, blocks) in status.neighbor_blocks() {
            chunks.entry(*neighbor_pos).or_insert(blocks.as_slice());
        }
    }
    chunks
}

fn collect_light_sections(
    engine: &LevelLightEngine<SharedRetainedLightWorld, SharedRetainedLightWorld>,
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
    if sections.is_empty() && section_count > 0 {
        sections.push(PackedLightSection::new(
            min_section_y,
            Some(vec![0; mclone_light::DATA_LAYER_SIZE]),
            None,
        ));
    }
    sections
}

#[derive(Clone, Debug, Default)]
struct SharedRetainedLightWorld(Rc<RefCell<RetainedLightWorld>>);

impl SharedRetainedLightWorld {
    fn borrow(&self) -> std::cell::Ref<'_, RetainedLightWorld> {
        self.0.borrow()
    }

    fn borrow_mut(&self) -> std::cell::RefMut<'_, RetainedLightWorld> {
        self.0.borrow_mut()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RetainedLightWorld {
    min_y: i32,
    height: i32,
    configured: bool,
    chunks: BTreeMap<ChunkPos, Vec<RawBlockId>>,
}

impl RetainedLightWorld {
    fn configure(&mut self, min_y: i32, height: i32) {
        assert!(
            min_y % SECTION_HEIGHT == 0,
            "level light min_y {min_y} must be a multiple of {SECTION_HEIGHT}"
        );
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "level light height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        if self.configured {
            assert_eq!(
                self.min_y, min_y,
                "retained light world cannot change min_y after initialization"
            );
            assert_eq!(
                self.height, height,
                "retained light world cannot change height after initialization"
            );
            return;
        }
        self.min_y = min_y;
        self.height = height;
        self.configured = true;
    }

    fn upsert_chunk(&mut self, pos: ChunkPos, blocks: &[RawBlockId]) -> bool {
        self.assert_configured();
        let expected_len = self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        assert_eq!(
            blocks.len(),
            expected_len,
            "chunk {pos:?} level light input had {} blocks instead of {expected_len}",
            blocks.len()
        );
        if self
            .chunks
            .get(&pos)
            .is_some_and(|existing| existing.as_slice() == blocks)
        {
            return false;
        }
        self.chunks.insert(pos, blocks.to_vec());
        true
    }

    fn section_statuses_for(&self, chunks: &[ChunkPos]) -> Vec<(SectionPosKey, bool)> {
        self.assert_configured();
        let min_section_y = block_to_section_coord(self.min_y);
        let section_count = self.height / SECTION_HEIGHT;
        chunks
            .iter()
            .filter_map(|chunk_pos| {
                self.chunks
                    .get(chunk_pos)
                    .map(|blocks| (*chunk_pos, blocks.as_slice()))
            })
            .flat_map(|(chunk_pos, blocks)| {
                (0..section_count).map(move |section_offset| {
                    (
                        section_as_long(chunk_pos.x, min_section_y + section_offset, chunk_pos.z),
                        section_is_empty(blocks, section_offset),
                    )
                })
            })
            .collect()
    }

    fn block_emission_sources_for(&self, chunks: &[ChunkPos]) -> Vec<(BlockPosKey, u8)> {
        self.assert_configured();
        let mut sources = Vec::new();
        for chunk_pos in chunks {
            let Some(blocks) = self.chunks.get(chunk_pos) else {
                continue;
            };
            for local_y in 0..self.height {
                for local_z in 0..CHUNK_WIDTH {
                    for local_x in 0..CHUNK_WIDTH {
                        let block = blocks[chunk_block_index(local_x, local_y, local_z)];
                        let emission = block_light_emission(block);
                        if emission == 0 {
                            continue;
                        }
                        sources.push((
                            block_pos_as_long(
                                chunk_pos.min_block_x() + local_x,
                                self.min_y + local_y,
                                chunk_pos.min_block_z() + local_z,
                            ),
                            emission,
                        ));
                    }
                }
            }
        }
        sources
    }

    fn block_at(&self, pos: BlockPosKey) -> Option<RawBlockId> {
        if !self.configured {
            return None;
        }
        let x = block_pos_get_x(pos);
        let y = block_pos_get_y(pos);
        let z = block_pos_get_z(pos);
        if y < self.min_y || y >= self.min_y + self.height {
            return None;
        }
        let chunk_pos = ChunkPos::from_block_coords(x, z);
        let blocks = self.chunks.get(&chunk_pos)?;
        Some(blocks[chunk_block_index(local_block_coord(x), y - self.min_y, local_block_coord(z))])
    }

    fn assert_configured(&self) {
        assert!(self.configured, "retained light world is not configured");
    }
}

fn section_is_empty(blocks: &[RawBlockId], section_offset: i32) -> bool {
    let base_y = section_offset * SECTION_HEIGHT;
    (0..SECTION_HEIGHT).all(|dy| {
        (0..CHUNK_WIDTH).all(|local_z| {
            (0..CHUNK_WIDTH).all(|local_x| {
                is_air_like(blocks[chunk_block_index(local_x, base_y + dy, local_z)])
            })
        })
    })
}

impl BlockLightWorld for SharedRetainedLightWorld {
    fn light_emission(&self, pos: BlockPosKey) -> u8 {
        self.borrow().block_at(pos).map_or(0, block_light_emission)
    }

    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
        self.borrow().block_at(pos).map(block_light_opacity)
    }
}

impl SkyLightWorld for SharedRetainedLightWorld {
    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
        self.borrow().block_at(pos).map(block_light_opacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{AIR, STONE};

    #[test]
    fn retained_light_world_marks_air_sections_empty() {
        let mut world = RetainedLightWorld::default();
        world.configure(0, SECTION_HEIGHT * 2);
        let chunk_pos = ChunkPos::new(0, 0);
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * 2 * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1, 1)] = STONE;
        assert!(world.upsert_chunk(chunk_pos, &blocks));

        let statuses = world.section_statuses_for(&[chunk_pos]);

        assert_eq!(
            statuses,
            vec![
                (section_as_long(0, 0, 0), false),
                (section_as_long(0, 1, 0), true),
            ]
        );
    }
}
