use std::collections::HashMap;

use crate::{BitStorage, local_palette_bits_for};

pub const CHUNK_WIDTH: i32 = 16;
pub const SECTION_HEIGHT: i32 = 16;
pub const CHUNK_SECTION_VOLUME: usize = 4096;
pub const LIGHT_DATA_LAYER_BYTE_COUNT: usize = 2048;
pub const AIR_BLOCK_STATE_ID: BlockStateId = BlockStateId(0);
pub const DEFAULT_BIOME_ID: i32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct BlockStateId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub fn from_block_coords(x: i32, z: i32) -> Self {
        Self {
            x: block_to_chunk_coord(x),
            z: block_to_chunk_coord(z),
        }
    }

    pub fn min_block_x(self) -> i32 {
        chunk_min_block_coord(self.x)
    }

    pub fn min_block_z(self) -> i32 {
        chunk_min_block_coord(self.z)
    }

    pub fn middle_block_x(self) -> i32 {
        chunk_middle_block_coord(self.x)
    }

    pub fn middle_block_z(self) -> i32 {
        chunk_middle_block_coord(self.z)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ChunkRevision(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ChunkStatus {
    Terrain,
    Surface,
    Features,
    Light,
    Full,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedChunkSection {
    pub section_y: i32,
    pub palette_state_ids: Vec<BlockStateId>,
    pub bits_per_block: u8,
    pub packed_block_indices: Vec<u64>,
}

impl PackedChunkSection {
    pub fn pack(section_y: i32, block_state_ids: &[BlockStateId]) -> Self {
        assert_eq!(
            block_state_ids.len(),
            CHUNK_SECTION_VOLUME,
            "chunk section {section_y} had {} blocks instead of {CHUNK_SECTION_VOLUME}",
            block_state_ids.len()
        );

        let mut palette_state_ids = Vec::new();
        let mut palette_index_by_state_id = HashMap::new();
        let mut local_indices = Vec::with_capacity(CHUNK_SECTION_VOLUME);

        for block_state_id in block_state_ids {
            let next_index = palette_state_ids.len() as u32;
            let palette_index = *palette_index_by_state_id
                .entry(*block_state_id)
                .or_insert_with(|| {
                    palette_state_ids.push(*block_state_id);
                    next_index
                });
            local_indices.push(palette_index);
        }

        let bits_per_block = local_palette_bits_for(palette_state_ids.len());
        let mut storage = BitStorage::new(bits_per_block, CHUNK_SECTION_VOLUME);
        for (index, palette_index) in local_indices.into_iter().enumerate() {
            storage.set(index, palette_index);
        }

        Self {
            section_y,
            palette_state_ids,
            bits_per_block,
            packed_block_indices: storage.into_raw(),
        }
    }

    pub fn unpack_block_state_ids(&self) -> Vec<BlockStateId> {
        self.validate_encoding();
        (0..CHUNK_SECTION_VOLUME)
            .map(|index| self.block_state_id_at_unchecked(index))
            .collect()
    }

    /// Read one section-local block without allocating and unpacking the full
    /// 4,096-entry section. Spatial queries and collision probes routinely need
    /// only a handful of cells.
    pub fn block_state_id_at(&self, index: usize) -> BlockStateId {
        self.validate_encoding();
        assert!(
            index < CHUNK_SECTION_VOLUME,
            "packed chunk section {} index {index} is out of bounds",
            self.section_y
        );
        self.block_state_id_at_unchecked(index)
    }

    fn validate_encoding(&self) {
        assert!(
            !self.palette_state_ids.is_empty(),
            "packed chunk section {} has an empty palette",
            self.section_y
        );
        assert_eq!(
            self.bits_per_block,
            local_palette_bits_for(self.palette_state_ids.len()),
            "packed chunk section {} has invalid bits_per_block",
            self.section_y
        );
        let values_per_word = 64 / usize::from(self.bits_per_block);
        assert_eq!(
            self.packed_block_indices.len(),
            CHUNK_SECTION_VOLUME.div_ceil(values_per_word),
            "packed chunk section {} has invalid packed word count",
            self.section_y,
        );
    }

    fn block_state_id_at_unchecked(&self, index: usize) -> BlockStateId {
        let bits = usize::from(self.bits_per_block);
        let values_per_word = 64 / bits;
        let word_index = index / values_per_word;
        let bit_index = (index - word_index * values_per_word) * bits;
        let mask = (1_u64 << self.bits_per_block) - 1;
        let palette_index = ((self.packed_block_indices[word_index] >> bit_index) & mask) as usize;
        *self
            .palette_state_ids
            .get(palette_index)
            .unwrap_or_else(|| {
                panic!(
                    "packed chunk section {} referenced palette entry {palette_index}",
                    self.section_y
                )
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedLightSection {
    pub section_y: i32,
    pub sky: Option<Vec<u8>>,
    pub block: Option<Vec<u8>>,
}

impl PackedLightSection {
    pub fn new(section_y: i32, sky: Option<Vec<u8>>, block: Option<Vec<u8>>) -> Self {
        if let Some(sky) = &sky {
            assert_eq!(
                sky.len(),
                LIGHT_DATA_LAYER_BYTE_COUNT,
                "sky light section {section_y} had {} bytes instead of {LIGHT_DATA_LAYER_BYTE_COUNT}",
                sky.len()
            );
        }
        if let Some(block) = &block {
            assert_eq!(
                block.len(),
                LIGHT_DATA_LAYER_BYTE_COUNT,
                "block light section {section_y} had {} bytes instead of {LIGHT_DATA_LAYER_BYTE_COUNT}",
                block.len()
            );
        }
        Self {
            section_y,
            sky,
            block,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sky.is_none() && self.block.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkSnapshot {
    pub pos: ChunkPos,
    pub status: ChunkStatus,
    pub revision: ChunkRevision,
    pub min_y: i32,
    pub height: i32,
    pub biomes: Vec<i32>,
    pub sections: Vec<PackedChunkSection>,
    pub light_correct: bool,
    pub light_sections: Vec<PackedLightSection>,
}

impl ChunkSnapshot {
    pub fn from_block_state_ids(
        pos: ChunkPos,
        status: ChunkStatus,
        revision: ChunkRevision,
        min_y: i32,
        height: i32,
        block_state_ids: &[BlockStateId],
    ) -> Self {
        assert!(
            min_y % SECTION_HEIGHT == 0,
            "chunk min_y {min_y} must be a multiple of {SECTION_HEIGHT}"
        );
        assert!(
            height > 0 && height % SECTION_HEIGHT == 0,
            "chunk height {height} must be a positive multiple of {SECTION_HEIGHT}"
        );
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        assert_eq!(
            block_state_ids.len(),
            expected_len,
            "chunk snapshot had {} blocks instead of {expected_len}",
            block_state_ids.len()
        );

        let section_count = height / SECTION_HEIGHT;
        let min_section_y = min_y / SECTION_HEIGHT;
        let mut sections = Vec::new();
        for section_offset in 0..section_count {
            let start = section_offset as usize * CHUNK_SECTION_VOLUME;
            let end = start + CHUNK_SECTION_VOLUME;
            let section_blocks = &block_state_ids[start..end];
            if section_blocks
                .iter()
                .all(|state_id| *state_id == AIR_BLOCK_STATE_ID)
            {
                continue;
            }
            sections.push(PackedChunkSection::pack(
                min_section_y + section_offset,
                section_blocks,
            ));
        }

        Self {
            pos,
            status,
            revision,
            min_y,
            height,
            biomes: Vec::new(),
            sections,
            light_correct: false,
            light_sections: Vec::new(),
        }
    }

    pub fn with_biomes(mut self, biomes: Vec<i32>) -> Self {
        validate_chunk_biomes(self.height, &biomes);
        self.biomes = biomes;
        self
    }

    pub fn with_light_sections(
        mut self,
        light_correct: bool,
        light_sections: Vec<PackedLightSection>,
    ) -> Self {
        self.light_correct = light_correct;
        self.light_sections = light_sections
            .into_iter()
            .filter(|section| !section.is_empty())
            .collect();
        self
    }

    pub fn patch_section_block(
        &mut self,
        section_y: i32,
        local_x: i32,
        local_y: i32,
        local_z: i32,
        block_state: BlockStateId,
    ) -> bool {
        self.patch_section_blocks(section_y, [(local_x, local_y, local_z, block_state)]) > 0
    }

    pub fn patch_section_blocks(
        &mut self,
        section_y: i32,
        updates: impl IntoIterator<Item = (i32, i32, i32, BlockStateId)>,
    ) -> usize {
        let updates = updates.into_iter().collect::<Vec<_>>();
        if updates.is_empty() {
            return 0;
        }
        for (local_x, local_y, local_z, _) in updates.iter().copied() {
            assert!(
                (0..CHUNK_WIDTH).contains(&local_x),
                "local_x {local_x} out of section bounds"
            );
            assert!(
                (0..SECTION_HEIGHT).contains(&local_y),
                "local_y {local_y} out of section bounds"
            );
            assert!(
                (0..CHUNK_WIDTH).contains(&local_z),
                "local_z {local_z} out of section bounds"
            );
        }
        let section_base_y = section_y * SECTION_HEIGHT;
        if section_base_y < self.min_y || section_base_y >= self.min_y + self.height {
            return 0;
        }

        let section_index = self
            .sections
            .iter()
            .position(|section| section.section_y == section_y);
        let mut blocks = section_index.map_or_else(
            || vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
            |index| self.sections[index].unpack_block_state_ids(),
        );

        let mut changed = 0;
        for (local_x, local_y, local_z, block_state) in updates {
            let block_index = chunk_section_index(local_x, local_y, local_z);
            if blocks[block_index] == block_state {
                continue;
            }
            blocks[block_index] = block_state;
            changed += 1;
        }
        if changed == 0 {
            return 0;
        }

        if blocks
            .iter()
            .all(|state_id| *state_id == AIR_BLOCK_STATE_ID)
        {
            if let Some(index) = section_index {
                self.sections.remove(index);
            }
            return changed;
        }

        let packed = PackedChunkSection::pack(section_y, &blocks);
        if let Some(index) = section_index {
            self.sections[index] = packed;
        } else {
            self.sections.push(packed);
            self.sections.sort_by_key(|section| section.section_y);
        }
        changed
    }
}

pub fn expected_chunk_biome_count(height: i32) -> usize {
    assert!(
        height > 0 && height % SECTION_HEIGHT == 0,
        "chunk height {height} must be a positive multiple of {SECTION_HEIGHT}"
    );
    (height as usize / 4) * 4 * 4
}

pub fn validate_chunk_biomes(height: i32, biomes: &[i32]) {
    if biomes.is_empty() {
        return;
    }
    let expected_len = expected_chunk_biome_count(height);
    assert_eq!(
        biomes.len(),
        expected_len,
        "chunk biome container had {} entries instead of {expected_len}",
        biomes.len()
    );
}

pub fn chunk_section_index(local_x: i32, local_y: i32, local_z: i32) -> usize {
    assert!(
        (0..CHUNK_WIDTH).contains(&local_x),
        "local_x {local_x} out of section bounds"
    );
    assert!(
        (0..SECTION_HEIGHT).contains(&local_y),
        "local_y {local_y} out of section bounds"
    );
    assert!(
        (0..CHUNK_WIDTH).contains(&local_z),
        "local_z {local_z} out of section bounds"
    );
    ((local_y << 8) | (local_z << 4) | local_x) as usize
}

pub fn chunk_block_index(local_x: i32, local_y: i32, local_z: i32) -> usize {
    assert!(
        (0..CHUNK_WIDTH).contains(&local_x),
        "local_x {local_x} out of chunk bounds"
    );
    assert!(local_y >= 0, "local_y {local_y} out of chunk bounds");
    assert!(
        (0..CHUNK_WIDTH).contains(&local_z),
        "local_z {local_z} out of chunk bounds"
    );
    ((local_y << 8) | (local_z << 4) | local_x) as usize
}

pub fn block_to_chunk_coord(block_coord: i32) -> i32 {
    block_coord.div_euclid(CHUNK_WIDTH)
}

pub fn local_block_coord(block_coord: i32) -> i32 {
    block_coord.rem_euclid(CHUNK_WIDTH)
}

pub fn chunk_min_block_coord(chunk_coord: i32) -> i32 {
    chunk_coord * CHUNK_WIDTH
}

pub fn chunk_middle_block_coord(chunk_coord: i32) -> i32 {
    chunk_min_block_coord(chunk_coord) + CHUNK_WIDTH / 2
}

pub fn chunk_block_coord(chunk_coord: i32, local_coord: i32) -> i32 {
    chunk_min_block_coord(chunk_coord) + local_coord
}

pub fn block_to_section_coord(block_y: i32) -> i32 {
    block_y.div_euclid(SECTION_HEIGHT)
}

pub fn local_section_block_coord(block_y: i32) -> i32 {
    block_y.rem_euclid(SECTION_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_index_matches_vanilla_order() {
        assert_eq!(chunk_section_index(0, 0, 0), 0);
        assert_eq!(chunk_section_index(15, 0, 0), 15);
        assert_eq!(chunk_section_index(0, 0, 1), 16);
        assert_eq!(chunk_section_index(0, 1, 0), 256);
    }

    #[test]
    fn chunk_block_index_matches_vanilla_order() {
        assert_eq!(chunk_block_index(0, 0, 0), 0);
        assert_eq!(chunk_block_index(15, 0, 0), 15);
        assert_eq!(chunk_block_index(0, 0, 1), 16);
        assert_eq!(chunk_block_index(0, 1, 0), 256);
        assert_eq!(chunk_block_index(0, 16, 0), 4096);
    }

    #[test]
    fn block_and_section_coordinate_helpers_floor_negative_positions() {
        assert_eq!(block_to_chunk_coord(0), 0);
        assert_eq!(block_to_chunk_coord(15), 0);
        assert_eq!(block_to_chunk_coord(16), 1);
        assert_eq!(block_to_chunk_coord(-1), -1);
        assert_eq!(block_to_chunk_coord(-16), -1);
        assert_eq!(block_to_chunk_coord(-17), -2);

        assert_eq!(local_block_coord(0), 0);
        assert_eq!(local_block_coord(15), 15);
        assert_eq!(local_block_coord(16), 0);
        assert_eq!(local_block_coord(-1), 15);
        assert_eq!(local_block_coord(-16), 0);
        assert_eq!(local_block_coord(-17), 15);

        assert_eq!(block_to_section_coord(-1), -1);
        assert_eq!(local_section_block_coord(-1), 15);
    }

    #[test]
    fn chunk_position_reports_vanilla_block_extents() {
        let pos = ChunkPos::from_block_coords(-1, 32);

        assert_eq!(pos, ChunkPos::new(-1, 2));
        assert_eq!(pos.min_block_x(), -16);
        assert_eq!(pos.middle_block_x(), -8);
        assert_eq!(pos.min_block_z(), 32);
        assert_eq!(pos.middle_block_z(), 40);
        assert_eq!(chunk_block_coord(pos.x, 15), -1);
    }

    #[test]
    fn packed_section_preserves_block_state_ids() {
        let mut blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        blocks[chunk_section_index(1, 2, 3)] = BlockStateId(5);
        blocks[chunk_section_index(15, 15, 15)] = BlockStateId(17);

        let section = PackedChunkSection::pack(0, &blocks);

        assert_eq!(section.bits_per_block, 4);
        assert_eq!(
            section.block_state_id_at(chunk_section_index(1, 2, 3)),
            BlockStateId(5)
        );
        assert_eq!(
            section.unpack_block_state_ids()[chunk_section_index(1, 2, 3)],
            BlockStateId(5)
        );
        assert_eq!(section.unpack_block_state_ids(), blocks);
    }

    #[test]
    fn packed_section_direct_lookup_handles_word_boundaries() {
        let blocks = (0..CHUNK_SECTION_VOLUME)
            .map(|index| BlockStateId((index % 17) as u32))
            .collect::<Vec<_>>();
        let section = PackedChunkSection::pack(0, &blocks);

        assert_eq!(section.bits_per_block, 5);
        for index in [0, 11, 12, 23, 24, CHUNK_SECTION_VOLUME - 1] {
            assert_eq!(section.block_state_id_at(index), blocks[index]);
        }
        assert_eq!(section.unpack_block_state_ids(), blocks);
    }

    #[test]
    fn chunk_snapshot_omits_empty_air_sections() {
        let mut blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        blocks[CHUNK_SECTION_VOLUME + chunk_section_index(0, 0, 0)] = BlockStateId(1);

        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(2, -3),
            ChunkStatus::Surface,
            ChunkRevision(7),
            0,
            32,
            &blocks,
        );

        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(snapshot.sections[0].section_y, 1);
        assert_eq!(
            snapshot.sections[0].unpack_block_state_ids()[0],
            BlockStateId(1)
        );
        assert!(!snapshot.light_correct);
        assert!(snapshot.light_sections.is_empty());
    }

    #[test]
    fn chunk_snapshot_patches_section_blocks() {
        let mut snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );

        assert!(snapshot.patch_section_block(0, 1, 2, 3, BlockStateId(7)));
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(
            snapshot.sections[0].unpack_block_state_ids()[chunk_section_index(1, 2, 3)],
            BlockStateId(7)
        );
        assert!(!snapshot.patch_section_block(0, 1, 2, 3, BlockStateId(7)));

        assert!(snapshot.patch_section_block(0, 1, 2, 3, AIR_BLOCK_STATE_ID));
        assert!(snapshot.sections.is_empty());
    }

    #[test]
    fn chunk_snapshot_batches_section_block_patches() {
        let mut snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );

        assert_eq!(
            snapshot.patch_section_blocks(
                0,
                [
                    (1, 2, 3, BlockStateId(7)),
                    (4, 5, 6, BlockStateId(9)),
                ],
            ),
            2
        );
        assert_eq!(snapshot.sections.len(), 1);
        let blocks = snapshot.sections[0].unpack_block_state_ids();
        assert_eq!(blocks[chunk_section_index(1, 2, 3)], BlockStateId(7));
        assert_eq!(blocks[chunk_section_index(4, 5, 6)], BlockStateId(9));

        assert_eq!(
            snapshot.patch_section_blocks(
                0,
                [
                    (1, 2, 3, BlockStateId(7)),
                    (4, 5, 6, BlockStateId(9)),
                ],
            ),
            0
        );

        assert_eq!(
            snapshot.patch_section_blocks(
                0,
                [(1, 2, 3, AIR_BLOCK_STATE_ID), (4, 5, 6, AIR_BLOCK_STATE_ID),],
            ),
            2
        );
        assert!(snapshot.sections.is_empty());
    }

    #[test]
    fn chunk_snapshot_batch_patch_preserves_order_for_duplicate_blocks() {
        let mut snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );

        assert_eq!(
            snapshot.patch_section_blocks(
                0,
                [(1, 2, 3, BlockStateId(7)), (1, 2, 3, AIR_BLOCK_STATE_ID),],
            ),
            2
        );
        assert!(snapshot.sections.is_empty());
    }

    #[test]
    fn packed_light_section_validates_layer_lengths() {
        let section = PackedLightSection::new(2, Some(vec![0xFF; 2048]), None);

        assert_eq!(section.section_y, 2);
        assert!(section.sky.is_some());
        assert!(section.block.is_none());
    }

    #[test]
    fn chunk_snapshot_preserves_explicit_empty_light_layers() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Light,
            ChunkRevision(1),
            0,
            SECTION_HEIGHT,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
        .with_light_sections(
            true,
            vec![PackedLightSection::new(
                0,
                Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT]),
                None,
            )],
        );

        assert_eq!(snapshot.light_sections.len(), 1);
        assert_eq!(snapshot.light_sections[0].section_y, 0);
        let sky = snapshot.light_sections[0].sky.as_ref().unwrap();
        assert_eq!(sky.len(), LIGHT_DATA_LAYER_BYTE_COUNT);
        assert!(sky.iter().all(|value| *value == 0));
    }
}
