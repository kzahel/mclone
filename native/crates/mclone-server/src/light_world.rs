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
            let mut changed_chunks = Vec::new();
            let mut changed_blocks = Vec::new();
            timing.light_status_input_chunks = input_chunks.len();
            for (&pos, blocks) in &input_chunks {
                match world.upsert_chunk(pos, blocks) {
                    Some(RetainedChunkChange::Inserted) => {
                        timing.light_status_inserted_chunks += 1;
                        changed_chunks.push(pos);
                    }
                    Some(RetainedChunkChange::Replaced(block_changes)) => {
                        timing.light_status_replaced_chunks += 1;
                        timing.changed_block_raw_checks += block_changes.raw_checks;
                        timing.changed_block_light_property_changes +=
                            block_changes.light_property_changes;
                        timing.changed_block_opacity_changes += block_changes.opacity_changes;
                        timing.changed_block_emission_changes += block_changes.emission_changes;
                        timing.changed_block_raw_only_changes += block_changes.raw_only_changes;
                        changed_chunks.push(pos);
                        changed_blocks.extend(block_changes.positions);
                    }
                    None => {
                        timing.light_status_unchanged_chunks += 1;
                    }
                }
            }
            (changed_chunks, changed_blocks)
        };
        let (changed_chunks, changed_blocks) = changed_chunks;
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

        let setup_start = timing_start();
        let start = timing_start();
        for (section, is_empty) in active_sections {
            self.engine.update_section_status(section, is_empty);
        }
        timing.section_status_update_us = timing_elapsed_us(start);
        let start = timing_start();
        for chunk_pos in &changed_chunks {
            self.engine
                .enable_light_sources(section_as_long(chunk_pos.x, 0, chunk_pos.z), true);
        }
        timing.sky_column_enable_us = timing_elapsed_us(start);
        timing.section_setup_us = timing_elapsed_us(setup_start);
        timing.sky_source_scan_us = 0;
        timing.sky_source_enqueue_us = 0;
        let start = timing_start();
        for (source, emission) in block_sources {
            self.engine.on_block_emission_increase(source, emission);
        }
        timing.block_source_enqueue_us = timing_elapsed_us(start);
        let start = timing_start();
        for block in changed_blocks {
            self.engine.check_block(block);
        }
        timing.changed_block_check_us = timing_elapsed_us(start);
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
        timing.sky_source_update_count = run_report.sky.source_update_count;
        timing.sky_source_updates_us = run_report.sky.source_updates_us;
        timing.block_run_update_graph_us = run_report.block.graph_us;
        timing.sky_run_update_graph_us = run_report.sky.graph_us;
        timing.block_run_update_storage_swap_us = run_report.block.storage_swap_us;
        timing.sky_run_update_storage_swap_us = run_report.sky.storage_swap_us;
        timing.block_run_update_affected_sections = run_report.block.affected_sections;
        timing.sky_run_update_affected_sections = run_report.sky.affected_sections;

        let start = timing_start();
        let min_section_y = block_to_section_coord(min_y);
        let section_count = height / SECTION_HEIGHT;
        let mut sections_by_chunk = BTreeMap::new();
        for status in &statuses {
            sections_by_chunk.insert(
                status.pos,
                collect_light_sections(
                    &self.engine,
                    status.pos,
                    min_section_y,
                    section_count,
                    status.raw_blocks(),
                ),
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
        chunks.insert(status.pos, status.raw_blocks());
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
    raw_blocks: &[RawBlockId],
) -> Vec<PackedLightSection> {
    let mut sections = Vec::new();
    let envelope = light_section_envelope(min_section_y, section_count, raw_blocks);
    let max_data_section_y = envelope.as_ref().map(|range| range.end - 3);
    let section_range = envelope
        .clone()
        .unwrap_or(min_section_y..min_section_y + section_count);
    for section_y in section_range {
        let section = section_as_long(pos.x, section_y, pos.z);
        let sky = engine
            .sky_engine()
            .storage()
            .get_visible_data_layer(section)
            .map(|layer| layer.to_packed_bytes());
        let block = engine
            .block_engine()
            .storage()
            .get_visible_data_layer(section)
            .map(|layer| layer.to_packed_bytes());
        if sky.is_some() || block.is_some() || max_data_section_y.is_some() {
            let sky =
                sky.or_else(|| synthetic_sky_layer_for_envelope(section_y, max_data_section_y));
            let block = block
                .or_else(|| max_data_section_y.map(|_| vec![0; mclone_light::DATA_LAYER_SIZE]));
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

fn light_section_envelope(
    min_section_y: i32,
    section_count: i32,
    raw_blocks: &[RawBlockId],
) -> Option<std::ops::Range<i32>> {
    let mut min_data_section = None;
    let mut max_data_section = None;
    for section_offset in 0..section_count {
        if section_is_empty(raw_blocks, section_offset) {
            continue;
        }
        let section_y = min_section_y + section_offset;
        min_data_section =
            Some(min_data_section.map_or(section_y, |current: i32| current.min(section_y)));
        max_data_section =
            Some(max_data_section.map_or(section_y, |current: i32| current.max(section_y)));
    }
    let min_light_section = min_section_y - 1;
    let max_light_section = min_section_y + section_count + 1;
    Some(
        (min_data_section? - 1).max(min_light_section)
            ..(max_data_section? + 3).min(max_light_section),
    )
}

fn synthetic_sky_layer_for_envelope(
    section_y: i32,
    max_data_section_y: Option<i32>,
) -> Option<Vec<u8>> {
    let max_data_section_y = max_data_section_y?;
    let value = if section_y > max_data_section_y {
        0xFF
    } else {
        0
    };
    Some(vec![value; mclone_light::DATA_LAYER_SIZE])
}

#[derive(Debug, Default, Eq, PartialEq)]
struct ChangedBlockPositions {
    positions: Vec<BlockPosKey>,
    raw_checks: usize,
    light_property_changes: usize,
    opacity_changes: usize,
    emission_changes: usize,
    raw_only_changes: usize,
}

#[derive(Debug, Eq, PartialEq)]
enum RetainedChunkChange {
    Inserted,
    Replaced(ChangedBlockPositions),
}

fn changed_block_positions(
    chunk_pos: ChunkPos,
    min_y: i32,
    old_blocks: &[RawBlockId],
    new_blocks: &[RawBlockId],
) -> ChangedBlockPositions {
    let mut changes = ChangedBlockPositions::default();
    for (index, (old, new)) in old_blocks.iter().zip(new_blocks.iter()).enumerate() {
        if old == new {
            continue;
        }
        changes.raw_checks += 1;
        let opacity_changed = block_light_opacity(*old) != block_light_opacity(*new);
        let emission_changed = block_light_emission(*old) != block_light_emission(*new);
        if opacity_changed {
            changes.opacity_changes += 1;
        }
        if emission_changed {
            changes.emission_changes += 1;
        }
        if opacity_changed || emission_changed {
            changes.light_property_changes += 1;
            let local_x = (index & 15) as i32;
            let local_z = ((index >> 4) & 15) as i32;
            let local_y = (index >> 8) as i32;
            changes.positions.push(block_pos_as_long(
                chunk_pos.min_block_x() + local_x,
                min_y + local_y,
                chunk_pos.min_block_z() + local_z,
            ));
        } else {
            changes.raw_only_changes += 1;
        }
    }
    changes
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

    fn upsert_chunk(
        &mut self,
        pos: ChunkPos,
        blocks: &[RawBlockId],
    ) -> Option<RetainedChunkChange> {
        self.assert_configured();
        let expected_len = self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        assert_eq!(
            blocks.len(),
            expected_len,
            "chunk {pos:?} level light input had {} blocks instead of {expected_len}",
            blocks.len()
        );
        let changed_blocks = if let Some(existing) = self.chunks.get(&pos) {
            if existing.as_slice() == blocks {
                return None;
            }
            RetainedChunkChange::Replaced(changed_block_positions(
                pos, self.min_y, existing, blocks,
            ))
        } else {
            RetainedChunkChange::Inserted
        };

        self.chunks.insert(pos, blocks.to_vec());
        Some(changed_blocks)
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
    use mclone_core::{BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus};
    use mclone_worldgen::block::{AIR, DIRT, STONE};

    fn test_snapshot(pos: ChunkPos, min_y: i32, height: i32) -> mclone_core::ChunkSnapshot {
        let section_count = height / SECTION_HEIGHT;
        mclone_core::ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Features,
            ChunkRevision(1),
            min_y,
            height,
            &vec![BlockStateId(0); section_count as usize * CHUNK_SECTION_VOLUME],
        )
    }

    #[test]
    fn batch_input_chunks_prefers_target_blocks_over_neighbor_dependencies() {
        let target = ChunkPos::new(0, 0);
        let other = ChunkPos::new(1, 0);
        let len = (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize;
        let stale_target_blocks = vec![AIR; len];
        let mut final_target_blocks = vec![AIR; len];
        final_target_blocks[chunk_block_index(1, 1, 1)] = STONE;
        let other_blocks = vec![AIR; len];
        let statuses = vec![
            PendingLightStatus::from_parts(
                other,
                test_snapshot(other, 0, SECTION_HEIGHT),
                other_blocks,
                vec![(target, stale_target_blocks)],
            ),
            PendingLightStatus::from_parts(
                target,
                test_snapshot(target, 0, SECTION_HEIGHT),
                final_target_blocks,
                Vec::new(),
            ),
        ];

        let chunks = batch_input_chunks(&statuses);

        assert_eq!(
            chunks[&target][chunk_block_index(1, 1, 1)],
            STONE,
            "target status blocks should replace stale dependency blocks"
        );
    }

    #[test]
    fn retained_light_world_marks_air_sections_empty() {
        let mut world = RetainedLightWorld::default();
        world.configure(0, SECTION_HEIGHT * 2);
        let chunk_pos = ChunkPos::new(0, 0);
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * 2 * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1, 1)] = STONE;
        assert_eq!(
            world.upsert_chunk(chunk_pos, &blocks),
            Some(RetainedChunkChange::Inserted)
        );

        let statuses = world.section_statuses_for(&[chunk_pos]);

        assert_eq!(
            statuses,
            vec![
                (section_as_long(0, 0, 0), false),
                (section_as_long(0, 1, 0), true),
            ]
        );
    }

    #[test]
    fn retained_light_world_reports_changed_block_positions_after_initial_insert() {
        let mut world = RetainedLightWorld::default();
        world.configure(0, SECTION_HEIGHT);
        let chunk_pos = ChunkPos::new(0, 0);
        let mut blocks = vec![AIR; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];
        assert_eq!(
            world.upsert_chunk(chunk_pos, &blocks),
            Some(RetainedChunkChange::Inserted)
        );

        blocks[chunk_block_index(1, 1, 1)] = STONE;

        assert_eq!(
            world.upsert_chunk(chunk_pos, &blocks),
            Some(RetainedChunkChange::Replaced(ChangedBlockPositions {
                positions: vec![block_pos_as_long(1, 1, 1)],
                raw_checks: 1,
                light_property_changes: 1,
                opacity_changes: 1,
                emission_changes: 0,
                raw_only_changes: 0,
            }))
        );
        assert_eq!(world.upsert_chunk(chunk_pos, &blocks), None);
    }

    #[test]
    fn retained_light_world_skips_raw_only_replacement_rechecks() {
        let mut world = RetainedLightWorld::default();
        world.configure(0, SECTION_HEIGHT);
        let chunk_pos = ChunkPos::new(0, 0);
        let mut blocks = vec![STONE; (CHUNK_WIDTH * SECTION_HEIGHT * CHUNK_WIDTH) as usize];
        assert_eq!(
            world.upsert_chunk(chunk_pos, &blocks),
            Some(RetainedChunkChange::Inserted)
        );

        blocks[chunk_block_index(2, 3, 4)] = DIRT;

        assert_eq!(
            world.upsert_chunk(chunk_pos, &blocks),
            Some(RetainedChunkChange::Replaced(ChangedBlockPositions {
                positions: Vec::new(),
                raw_checks: 1,
                light_property_changes: 0,
                opacity_changes: 0,
                emission_changes: 0,
                raw_only_changes: 1,
            }))
        );
    }
}
