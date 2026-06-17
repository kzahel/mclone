//! Provisional ("seed") lighting computed at worldgen publication time.
//!
//! Move-only home for the provisional sky/block light propagation used to seed
//! chunk snapshots before the full lighting engine runs: the per-chunk flood-fill
//! (`ProvisionalSkyLightWorld`), the section assembly helpers, and the light-level
//! propagation/opacity/emission rules. This is distinct from chunk scheduling and
//! fluid ticking; the scheduler and worldgen mailbox call into it via the
//! `pub(crate)` entry points.

use std::collections::{BTreeMap, VecDeque};

use mclone_core::{
    CHUNK_WIDTH, ChunkPos, PackedLightSection, SECTION_HEIGHT, block_to_section_coord,
    chunk_block_index, local_section_block_coord,
};
use mclone_light::{DataLayer, LightLayer};
use mclone_worldgen::block::{RawBlockId, is_lava, material_blocks_motion};

#[cfg(test)]
use mclone_core::ChunkSnapshot;

#[cfg(test)]
pub(crate) fn snapshot_with_provisional_lighting(
    snapshot: ChunkSnapshot,
    raw_blocks: &[RawBlockId],
) -> ChunkSnapshot {
    snapshot_with_provisional_lighting_from_neighbors(snapshot, raw_blocks, std::iter::empty())
}

#[cfg(test)]
pub(crate) fn snapshot_with_provisional_lighting_from_neighbors<'a>(
    snapshot: ChunkSnapshot,
    raw_blocks: &'a [RawBlockId],
    neighbor_blocks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> ChunkSnapshot {
    let light_sections = provisional_light_sections_from_neighbors(
        snapshot.pos,
        snapshot.min_y,
        snapshot.height,
        raw_blocks,
        neighbor_blocks,
    );
    snapshot.with_light_sections(false, light_sections)
}

pub(crate) fn provisional_light_sections_from_neighbors<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    raw_blocks: &'a [RawBlockId],
    neighbor_blocks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    assert_eq!(
        raw_blocks.len(),
        expected_len,
        "chunk {:?} lighting input had {} blocks instead of {expected_len}",
        target_pos,
        raw_blocks.len()
    );
    let mut chunks = neighbor_blocks.into_iter().collect::<Vec<_>>();
    chunks.push((target_pos, raw_blocks));
    let sky_sections =
        provisional_sky_light_sections_for_chunk(target_pos, min_y, height, chunks.iter().copied());
    let block_sections = provisional_block_light_sections_for_chunk(
        target_pos,
        min_y,
        height,
        chunks.iter().copied(),
    );
    merge_light_sections(sky_sections, block_sections)
}

#[cfg(test)]
pub(crate) fn provisional_sky_light_sections(
    min_y: i32,
    height: i32,
    raw_blocks: &[RawBlockId],
) -> Vec<PackedLightSection> {
    provisional_sky_light_sections_for_chunk(
        ChunkPos::new(0, 0),
        min_y,
        height,
        std::iter::once((ChunkPos::new(0, 0), raw_blocks)),
    )
}

pub(crate) fn provisional_sky_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let world = ProvisionalSkyLightWorld::new(target_pos, min_y, height, chunks);
    world.light_sections()
}

pub(crate) fn provisional_block_light_sections_for_chunk<'a>(
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
) -> Vec<PackedLightSection> {
    let world = ProvisionalSkyLightWorld::new(target_pos, min_y, height, chunks);
    world.block_light_sections()
}

fn merge_light_sections(
    sky_sections: Vec<PackedLightSection>,
    block_sections: Vec<PackedLightSection>,
) -> Vec<PackedLightSection> {
    let mut sections = BTreeMap::<i32, (Option<Vec<u8>>, Option<Vec<u8>>)>::new();
    for section in sky_sections.into_iter().chain(block_sections) {
        let entry = sections.entry(section.section_y).or_default();
        if section.sky.is_some() {
            entry.0 = section.sky;
        }
        if section.block.is_some() {
            entry.1 = section.block;
        }
    }

    sections
        .into_iter()
        .map(|(section_y, (sky, block))| PackedLightSection::new(section_y, sky, block))
        .collect()
}

pub(crate) fn provisional_sky_light_includes_chunk(
    target_pos: ChunkPos,
    chunk_pos: ChunkPos,
) -> bool {
    (chunk_pos.x - target_pos.x).abs() <= 1 && (chunk_pos.z - target_pos.z).abs() <= 1
}

struct ProvisionalSkyLightWorld<'a> {
    target_pos: ChunkPos,
    min_y: i32,
    height: i32,
    chunks: BTreeMap<ChunkPos, &'a [RawBlockId]>,
}

impl<'a> ProvisionalSkyLightWorld<'a> {
    fn new(
        target_pos: ChunkPos,
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (ChunkPos, &'a [RawBlockId])>,
    ) -> Self {
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "provisional light height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        let chunks = chunks
            .into_iter()
            .map(|(pos, blocks)| {
                assert_eq!(
                    blocks.len(),
                    expected_len,
                    "chunk {pos:?} lighting input had {} blocks instead of {expected_len}",
                    blocks.len()
                );
                (pos, blocks)
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            chunks.contains_key(&target_pos),
            "provisional light target chunk {target_pos:?} was missing from input chunks"
        );

        Self {
            target_pos,
            min_y,
            height,
            chunks,
        }
    }

    fn light_sections(&self) -> Vec<PackedLightSection> {
        let mut sky_values = self
            .chunks
            .keys()
            .map(|pos| {
                (
                    *pos,
                    vec![0_u8; self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut queue = VecDeque::new();

        for (&chunk_pos, blocks) in &self.chunks {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let mut sky_open = true;
                    for local_y in (0..self.height).rev() {
                        let index = chunk_block_index(local_x, local_y, local_z);
                        let block_id = blocks[index];
                        if sky_open && sky_light_opacity(block_id) >= 15 {
                            sky_open = false;
                            continue;
                        }
                        if sky_open {
                            sky_values.get_mut(&chunk_pos).unwrap()[index] = 15;
                            queue.push_back((chunk_pos, local_x, local_y, local_z));
                        }
                    }
                }
            }
        }

        while let Some((chunk_pos, local_x, local_y, local_z)) = queue.pop_front() {
            let source_level = sky_values[&chunk_pos][chunk_block_index(local_x, local_y, local_z)];
            if source_level <= 1 {
                continue;
            }

            for [dx, dy, dz] in SKY_LIGHT_DIRECTIONS {
                let Some((next_pos, next_x, next_y, next_z)) =
                    self.offset_cell(chunk_pos, local_x, local_y, local_z, dx, dy, dz)
                else {
                    continue;
                };

                let next_index = chunk_block_index(next_x, next_y, next_z);
                let target_block = self.chunks[&next_pos][next_index];
                let Some(next_level) = propagated_sky_light_level(source_level, dy, target_block)
                else {
                    continue;
                };
                let next_values = sky_values.get_mut(&next_pos).unwrap();
                if next_level > next_values[next_index] {
                    next_values[next_index] = next_level;
                    queue.push_back((next_pos, next_x, next_y, next_z));
                }
            }
        }

        self.target_light_sections(&sky_values[&self.target_pos], LightLayer::Sky)
    }

    fn block_light_sections(&self) -> Vec<PackedLightSection> {
        let mut block_values = self
            .chunks
            .keys()
            .map(|pos| {
                (
                    *pos,
                    vec![0_u8; self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut queue = VecDeque::new();

        for (&chunk_pos, blocks) in &self.chunks {
            for local_y in 0..self.height {
                for local_z in 0..CHUNK_WIDTH {
                    for local_x in 0..CHUNK_WIDTH {
                        let index = chunk_block_index(local_x, local_y, local_z);
                        let emission = block_light_emission(blocks[index]);
                        if emission > 0 {
                            block_values.get_mut(&chunk_pos).unwrap()[index] = emission;
                            queue.push_back((chunk_pos, local_x, local_y, local_z));
                        }
                    }
                }
            }
        }

        while let Some((chunk_pos, local_x, local_y, local_z)) = queue.pop_front() {
            let source_level =
                block_values[&chunk_pos][chunk_block_index(local_x, local_y, local_z)];
            if source_level <= 1 {
                continue;
            }

            for [dx, dy, dz] in SKY_LIGHT_DIRECTIONS {
                let Some((next_pos, next_x, next_y, next_z)) =
                    self.offset_cell(chunk_pos, local_x, local_y, local_z, dx, dy, dz)
                else {
                    continue;
                };

                let next_index = chunk_block_index(next_x, next_y, next_z);
                let target_block = self.chunks[&next_pos][next_index];
                let Some(next_level) = propagated_block_light_level(source_level, target_block)
                else {
                    continue;
                };
                let next_values = block_values.get_mut(&next_pos).unwrap();
                if next_level > next_values[next_index] {
                    next_values[next_index] = next_level;
                    queue.push_back((next_pos, next_x, next_y, next_z));
                }
            }
        }

        self.target_light_sections(&block_values[&self.target_pos], LightLayer::Block)
    }

    fn offset_cell(
        &self,
        chunk_pos: ChunkPos,
        local_x: i32,
        local_y: i32,
        local_z: i32,
        dx: i32,
        dy: i32,
        dz: i32,
    ) -> Option<(ChunkPos, i32, i32, i32)> {
        let next_y = local_y + dy;
        if !(0..self.height).contains(&next_y) {
            return None;
        }

        let mut next_pos = chunk_pos;
        let mut next_x = local_x + dx;
        if next_x < 0 {
            next_pos.x -= 1;
            next_x += CHUNK_WIDTH;
        } else if next_x >= CHUNK_WIDTH {
            next_pos.x += 1;
            next_x -= CHUNK_WIDTH;
        }

        let mut next_z = local_z + dz;
        if next_z < 0 {
            next_pos.z -= 1;
            next_z += CHUNK_WIDTH;
        } else if next_z >= CHUNK_WIDTH {
            next_pos.z += 1;
            next_z -= CHUNK_WIDTH;
        }

        self.chunks
            .contains_key(&next_pos)
            .then_some((next_pos, next_x, next_y, next_z))
    }

    fn target_light_sections(
        &self,
        light_values: &[u8],
        layer: LightLayer,
    ) -> Vec<PackedLightSection> {
        debug_assert_eq!(
            light_values.len(),
            self.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize
        );
        let section_count = self.height / SECTION_HEIGHT;
        let min_section_y = block_to_section_coord(self.min_y);

        let mut layers = (0..section_count)
            .map(|_| DataLayer::new())
            .collect::<Vec<_>>();
        for local_y in 0..self.height {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let level = light_values[chunk_block_index(local_x, local_y, local_z)];
                    if level == 0 {
                        continue;
                    }
                    let section_offset = block_to_section_coord(local_y);
                    let section_local_y = local_section_block_coord(local_y);
                    layers[section_offset as usize].set(local_x, section_local_y, local_z, level);
                }
            }
        }

        let mut sections = layers
            .into_iter()
            .enumerate()
            .filter_map(|(offset, data_layer)| {
                data_layer.into_bytes().map(|bytes| {
                    let (sky, block) = match layer {
                        LightLayer::Sky => (Some(bytes), None),
                        LightLayer::Block => (None, Some(bytes)),
                    };
                    PackedLightSection::new(min_section_y + offset as i32, sky, block)
                })
            })
            .collect::<Vec<_>>();
        if layer == LightLayer::Sky && sections.is_empty() && section_count > 0 {
            sections.push(PackedLightSection::new(
                min_section_y,
                Some(vec![0; mclone_light::DATA_LAYER_SIZE]),
                None,
            ));
        }
        sections
    }
}

const SKY_LIGHT_DIRECTIONS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

fn propagated_sky_light_level(
    source_level: u8,
    direction_y: i32,
    target_block: RawBlockId,
) -> Option<u8> {
    let opacity = sky_light_opacity(target_block);
    if source_level == 0 || opacity >= 15 {
        return None;
    }

    let attenuation = if direction_y == -1 && source_level == 15 && opacity == 0 {
        0
    } else {
        opacity.max(1)
    };
    let level = source_level.saturating_sub(attenuation);
    (level > 0).then_some(level)
}

fn propagated_block_light_level(source_level: u8, target_block: RawBlockId) -> Option<u8> {
    let opacity = sky_light_opacity(target_block);
    if source_level == 0 || opacity >= 15 {
        return None;
    }

    let level = source_level.saturating_sub(opacity.max(1));
    (level > 0).then_some(level)
}

fn block_light_emission(block_id: RawBlockId) -> u8 {
    if is_lava(block_id) { 15 } else { 0 }
}

fn sky_light_opacity(block_id: RawBlockId) -> u8 {
    if material_blocks_motion(block_id) {
        15
    } else {
        0
    }
}
