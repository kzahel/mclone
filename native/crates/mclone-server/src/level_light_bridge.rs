use std::collections::BTreeMap;
use std::time::Instant;

use mclone_core::{
    CHUNK_WIDTH, ChunkPos, PackedLightSection, SECTION_HEIGHT, block_to_section_coord,
    chunk_block_index, local_block_coord,
};
use mclone_light::{
    BlockLightWorld, BlockPosKey, LevelLightEngine, SkyLightWorld, block_pos_as_long,
    block_pos_get_x, block_pos_get_y, block_pos_get_z, section_as_long,
};
use mclone_worldgen::block::{RawBlockId, block_light_emission, block_light_opacity};

#[cfg(test)]
pub(crate) fn graph_level_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    graph_level_light_sections_for_chunk_timed(target_pos, min_y, height, chunks).0
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LevelLightComputationTiming {
    pub(crate) total_us: u128,
    pub(crate) world_init_us: u128,
    pub(crate) active_sections_us: u128,
    pub(crate) sky_source_scan_us: u128,
    pub(crate) block_source_scan_us: u128,
    pub(crate) engine_init_us: u128,
    pub(crate) section_setup_us: u128,
    pub(crate) sky_source_enqueue_us: u128,
    pub(crate) block_source_enqueue_us: u128,
    pub(crate) run_updates_us: u128,
    pub(crate) collect_sections_us: u128,
}

impl LevelLightComputationTiming {
    pub(crate) fn add_assign(&mut self, other: Self) {
        self.total_us += other.total_us;
        self.world_init_us += other.world_init_us;
        self.active_sections_us += other.active_sections_us;
        self.sky_source_scan_us += other.sky_source_scan_us;
        self.block_source_scan_us += other.block_source_scan_us;
        self.engine_init_us += other.engine_init_us;
        self.section_setup_us += other.section_setup_us;
        self.sky_source_enqueue_us += other.sky_source_enqueue_us;
        self.block_source_enqueue_us += other.block_source_enqueue_us;
        self.run_updates_us += other.run_updates_us;
        self.collect_sections_us += other.collect_sections_us;
    }
}

#[cfg(test)]
pub(crate) fn graph_level_light_sections_for_chunk_timed<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> (Vec<PackedLightSection>, LevelLightComputationTiming) {
    let (mut sections, timing) = graph_level_light_sections_for_chunks_timed(
        std::iter::once(target_pos),
        min_y,
        height,
        chunks,
    );
    (sections.remove(&target_pos).unwrap_or_default(), timing)
}

pub(crate) fn graph_level_light_sections_for_chunks_timed<'a>(
    targets: impl IntoIterator<Item = ChunkPos>,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> (
    BTreeMap<ChunkPos, Vec<PackedLightSection>>,
    LevelLightComputationTiming,
) {
    let total_start = Instant::now();
    let mut timing = LevelLightComputationTiming::default();
    let targets = targets.into_iter().collect::<Vec<_>>();

    let start = Instant::now();
    let world = RawChunkLightWorld::new(targets.iter().copied(), min_y, height, chunks);
    timing.world_init_us = start.elapsed().as_micros();
    let start = Instant::now();
    let active_sections = world.active_sections();
    timing.active_sections_us = start.elapsed().as_micros();
    let start = Instant::now();
    let sky_sources = world.sky_source_blocks();
    timing.sky_source_scan_us = start.elapsed().as_micros();
    let start = Instant::now();
    let block_sources = world.block_emission_sources();
    timing.block_source_scan_us = start.elapsed().as_micros();
    let start = Instant::now();
    let mut engine = LevelLightEngine::new(world.clone(), world);
    timing.engine_init_us = start.elapsed().as_micros();

    let start = Instant::now();
    for section in active_sections {
        engine.update_section_status(section, false);
        engine.enable_light_sources(section, true);
    }
    timing.section_setup_us = start.elapsed().as_micros();
    let start = Instant::now();
    for source in sky_sources {
        engine.check_sky_source(source);
    }
    timing.sky_source_enqueue_us = start.elapsed().as_micros();
    let start = Instant::now();
    for (source, emission) in block_sources {
        engine.on_block_emission_increase(source, emission);
    }
    timing.block_source_enqueue_us = start.elapsed().as_micros();
    let start = Instant::now();
    engine.run_all_updates();
    timing.run_updates_us = start.elapsed().as_micros();

    let start = Instant::now();
    let min_section_y = block_to_section_coord(min_y);
    let section_count = height / SECTION_HEIGHT;
    let mut sections_by_chunk = BTreeMap::new();
    for target_pos in targets {
        let mut sections = Vec::new();
        for section_offset in 0..section_count {
            let section_y = min_section_y + section_offset;
            let section = section_as_long(target_pos.x, section_y, target_pos.z);
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
        sections_by_chunk.insert(target_pos, sections);
    }
    timing.collect_sections_us = start.elapsed().as_micros();
    timing.total_us = total_start.elapsed().as_micros();
    (sections_by_chunk, timing)
}

#[derive(Clone, Debug)]
struct RawChunkLightWorld<'a> {
    min_y: i32,
    height: i32,
    chunks: BTreeMap<ChunkPos, &'a [RawBlockId]>,
}

impl<'a> RawChunkLightWorld<'a> {
    fn new(
        target_positions: impl IntoIterator<Item = ChunkPos>,
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
    ) -> Self {
        assert!(
            min_y % SECTION_HEIGHT == 0,
            "level light min_y {min_y} must be a multiple of {SECTION_HEIGHT}"
        );
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "level light height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        let chunks = chunks
            .into_iter()
            .map(|(pos, blocks)| {
                assert_eq!(
                    blocks.len(),
                    expected_len,
                    "chunk {pos:?} level light input had {} blocks instead of {expected_len}",
                    blocks.len()
                );
                (pos, blocks)
            })
            .collect::<BTreeMap<_, _>>();
        for target_pos in target_positions {
            assert!(
                chunks.contains_key(&target_pos),
                "level light target chunk {target_pos:?} was missing from input chunks"
            );
        }

        Self {
            min_y,
            height,
            chunks,
        }
    }

    fn active_sections(&self) -> Vec<i64> {
        let min_section_y = block_to_section_coord(self.min_y);
        let section_count = self.height / SECTION_HEIGHT;
        self.chunks
            .keys()
            .flat_map(|chunk_pos| {
                (0..section_count).map(move |section_offset| {
                    section_as_long(chunk_pos.x, min_section_y + section_offset, chunk_pos.z)
                })
            })
            .collect()
    }

    fn sky_source_blocks(&self) -> Vec<BlockPosKey> {
        let top_local_y = self.height - 1;
        let mut sources = Vec::new();
        for (&chunk_pos, blocks) in &self.chunks {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let block = blocks[chunk_block_index(local_x, top_local_y, local_z)];
                    if block_light_opacity(block) < 15 {
                        sources.push(block_pos_as_long(
                            chunk_pos.min_block_x() + local_x,
                            self.min_y + top_local_y,
                            chunk_pos.min_block_z() + local_z,
                        ));
                    }
                }
            }
        }
        sources
    }

    fn block_emission_sources(&self) -> Vec<(BlockPosKey, u8)> {
        let mut sources = Vec::new();
        for (&chunk_pos, blocks) in &self.chunks {
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
}

impl BlockLightWorld for RawChunkLightWorld<'_> {
    fn light_emission(&self, pos: BlockPosKey) -> u8 {
        self.block_at(pos).map_or(0, block_light_emission)
    }

    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
        self.block_at(pos).map(block_light_opacity)
    }
}

impl SkyLightWorld for RawChunkLightWorld<'_> {
    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
        self.block_at(pos).map(block_light_opacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::chunk_block_index;
    use mclone_worldgen::block::{AIR, LAVA, STONE};

    #[test]
    fn graph_level_light_packs_sky_and_block_layers_together() {
        let min_y = 0;
        let height = SECTION_HEIGHT;
        let mut blocks = vec![AIR; (CHUNK_WIDTH * height * CHUNK_WIDTH) as usize];
        blocks[chunk_block_index(1, 1, 1)] = LAVA;
        blocks[chunk_block_index(3, 15, 3)] = STONE;

        let sections = graph_level_light_sections_for_chunk(
            ChunkPos::new(0, 0),
            min_y,
            height,
            [(ChunkPos::new(0, 0), blocks.as_slice())],
        );

        assert_eq!(
            sample_layer(&sections, mclone_light::LightLayer::Block, 1, 1, 1),
            15
        );
        assert_eq!(
            sample_layer(&sections, mclone_light::LightLayer::Block, 2, 1, 1),
            14
        );
        assert_eq!(
            sample_layer(&sections, mclone_light::LightLayer::Sky, 1, 1, 1),
            15
        );
        assert_eq!(
            sample_layer(&sections, mclone_light::LightLayer::Sky, 3, 15, 3),
            0
        );
    }

    fn sample_layer(
        sections: &[PackedLightSection],
        layer: mclone_light::LightLayer,
        x: i32,
        y: i32,
        z: i32,
    ) -> u8 {
        let section_y = block_to_section_coord(y);
        let Some(section) = sections
            .iter()
            .find(|section| section.section_y == section_y)
        else {
            return 0;
        };
        let Some(data_layer) = mclone_light::packed_light_section_layer(section, layer).unwrap()
        else {
            return 0;
        };
        data_layer.get(local_block_coord(x), y & 15, local_block_coord(z))
    }
}
