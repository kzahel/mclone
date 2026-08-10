//! Minimal status-owned structure lifecycle.
//!
//! Minecraft 1.17.1 stores a start only on its owner chunk, stores references
//! on every intersected chunk, and lets each target chunk place only the
//! clipped piece slice during FEATURES. This module keeps that ownership shape
//! while exposing only the original cross-chunk canary needed to prove it.

use std::collections::BTreeSet;

use mclone_core::{BlockPos, CHUNK_WIDTH, ChunkPos, HorizontalTopology, local_block_coord};
use mclone_worldgen::block::{COBBLESTONE, OAK_LOG_X, RawBlockId, STONE_BRICKS};
use mclone_worldgen::levelgen::GeneratedChunk;

pub const CROSS_CHUNK_CANARY_STRUCTURE_ID: &str = "mclone:cross_chunk_canary_v1";
pub const CROSS_CHUNK_CANARY_PIECE_ID: &str = "mclone:wayside_arch_v1";
pub const CROSS_CHUNK_CANARY_START_CHUNK: ChunkPos = ChunkPos::new(0, 0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureOverlay {
    None,
    CrossChunkCanaryV1,
}

impl Default for StructureOverlay {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructureBoundingBox {
    pub min: BlockPos,
    pub max: BlockPos,
}

impl StructureBoundingBox {
    pub fn new(min: BlockPos, max: BlockPos) -> Result<Self, String> {
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(format!(
                "structure bounds are inverted: min={min:?} max={max:?}"
            ));
        }
        Ok(Self { min, max })
    }

    pub fn intersects_chunk(self, chunk: ChunkPos) -> bool {
        let min_x = chunk.min_block_x();
        let min_z = chunk.min_block_z();
        self.max.x >= min_x
            && self.min.x < min_x + CHUNK_WIDTH
            && self.max.z >= min_z
            && self.min.z < min_z + CHUNK_WIDTH
    }

    pub fn touched_chunks(self, topology: HorizontalTopology) -> Result<Vec<ChunkPos>, String> {
        let min = self.min.chunk_pos();
        let max = self.max.chunk_pos();
        let mut touched = BTreeSet::new();
        for z in min.z..=max.z {
            for x in min.x..=max.x {
                let canonical = topology
                    .canonicalize_chunk(ChunkPos::new(x, z))
                    .ok_or_else(|| {
                        format!("structure bounds include out-of-topology chunk ({x}, {z})")
                    })?;
                touched.insert(canonical);
            }
        }
        Ok(touched.into_iter().collect())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructureBlockPlacement {
    pub pos: BlockPos,
    pub block: RawBlockId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructurePieceRecord {
    pub piece_id: String,
    pub bounds: StructureBoundingBox,
    pub blocks: Vec<StructureBlockPlacement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureStartRecord {
    pub structure_id: String,
    pub start_chunk: ChunkPos,
    pub references: u32,
    pub bounds: StructureBoundingBox,
    pub pieces: Vec<StructurePieceRecord>,
}

impl StructureStartRecord {
    pub fn touched_chunks(&self, topology: HorizontalTopology) -> Result<Vec<ChunkPos>, String> {
        self.bounds.touched_chunks(topology)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StructureReference {
    pub structure_id: String,
    pub start_chunk: ChunkPos,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChunkStructureData {
    pub starts: Vec<StructureStartRecord>,
    pub references: Vec<StructureReference>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StructurePlacementReceipt {
    pub referenced_starts: usize,
    pub intersecting_pieces: usize,
    pub considered_blocks: usize,
    pub placed_blocks: usize,
    pub skipped_outside_target: usize,
}

impl StructureOverlay {
    pub fn starts_owned_by(self, pos: ChunkPos) -> Vec<StructureStartRecord> {
        match self {
            Self::None => Vec::new(),
            Self::CrossChunkCanaryV1 if pos == CROSS_CHUNK_CANARY_START_CHUNK => {
                vec![cross_chunk_canary_start()]
            }
            Self::CrossChunkCanaryV1 => Vec::new(),
        }
    }

    pub fn references_for(
        self,
        target: ChunkPos,
        topology: HorizontalTopology,
    ) -> Result<Vec<StructureReference>, String> {
        let target = topology
            .canonicalize_chunk(target)
            .ok_or_else(|| format!("structure target chunk {target:?} is outside topology"))?;
        let mut references = Vec::new();
        for start in self.all_starts() {
            if start.touched_chunks(topology)?.contains(&target) {
                references.push(StructureReference {
                    structure_id: start.structure_id.clone(),
                    start_chunk: topology
                        .canonicalize_chunk(start.start_chunk)
                        .expect("built-in structure start must lie inside topology"),
                });
            }
        }
        references.sort();
        references.dedup();
        Ok(references)
    }

    pub fn materialize_chunk(
        self,
        target: ChunkPos,
        topology: HorizontalTopology,
        references: &[StructureReference],
        chunk: &mut GeneratedChunk,
    ) -> Result<StructurePlacementReceipt, String> {
        if ChunkPos::new(chunk.chunk_x, chunk.chunk_z) != target {
            return Err(format!(
                "structure target {target:?} does not match generated chunk ({}, {})",
                chunk.chunk_x, chunk.chunk_z
            ));
        }
        let mut receipt = StructurePlacementReceipt::default();
        for reference in references {
            let start = self
                .start(reference)
                .ok_or_else(|| format!("unresolvable structure reference {reference:?}"))?;
            receipt.referenced_starts += 1;
            for piece in &start.pieces {
                if !piece.bounds.intersects_chunk(target) {
                    continue;
                }
                receipt.intersecting_pieces += 1;
                for placement in &piece.blocks {
                    receipt.considered_blocks += 1;
                    let placement_chunk = topology
                        .canonicalize_chunk(placement.pos.chunk_pos())
                        .ok_or_else(|| {
                            format!("structure block {:?} is outside topology", placement.pos)
                        })?;
                    if placement_chunk != target {
                        receipt.skipped_outside_target += 1;
                        continue;
                    }
                    if placement.pos.y < chunk.min_y
                        || placement.pos.y >= chunk.min_y + chunk.height
                    {
                        return Err(format!(
                            "structure block {:?} is outside generated vertical bounds",
                            placement.pos
                        ));
                    }
                    chunk.set_block_at_y(
                        local_block_coord(placement.pos.x),
                        placement.pos.y,
                        local_block_coord(placement.pos.z),
                        placement.block,
                    );
                    receipt.placed_blocks += 1;
                }
            }
        }
        Ok(receipt)
    }

    fn all_starts(self) -> Vec<StructureStartRecord> {
        match self {
            Self::None => Vec::new(),
            Self::CrossChunkCanaryV1 => vec![cross_chunk_canary_start()],
        }
    }

    fn start(self, reference: &StructureReference) -> Option<StructureStartRecord> {
        self.all_starts().into_iter().find(|start| {
            start.structure_id == reference.structure_id
                && start.start_chunk == reference.start_chunk
        })
    }
}

fn cross_chunk_canary_start() -> StructureStartRecord {
    let bounds = StructureBoundingBox::new(BlockPos::new(14, 4, 8), BlockPos::new(17, 6, 8))
        .expect("canary bounds are valid");
    let mut blocks = Vec::new();
    for x in 14..=17 {
        blocks.push(StructureBlockPlacement {
            pos: BlockPos::new(x, 4, 8),
            block: STONE_BRICKS,
        });
        blocks.push(StructureBlockPlacement {
            pos: BlockPos::new(x, 6, 8),
            block: OAK_LOG_X,
        });
    }
    for x in [14, 17] {
        blocks.push(StructureBlockPlacement {
            pos: BlockPos::new(x, 5, 8),
            block: COBBLESTONE,
        });
    }
    StructureStartRecord {
        structure_id: CROSS_CHUNK_CANARY_STRUCTURE_ID.to_owned(),
        start_chunk: CROSS_CHUNK_CANARY_START_CHUNK,
        references: 0,
        bounds,
        pieces: vec![StructurePieceRecord {
            piece_id: CROSS_CHUNK_CANARY_PIECE_ID.to_owned(),
            bounds,
            blocks,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::AIR;

    fn empty_chunk(pos: ChunkPos) -> GeneratedChunk {
        GeneratedChunk::from_raw_parts(pos.x, pos.z, 0, 16, vec![AIR; 16 * 16 * 16])
    }

    #[test]
    fn canary_has_one_owner_and_references_both_touched_chunks() {
        let overlay = StructureOverlay::CrossChunkCanaryV1;
        let topology = HorizontalTopology::UNBOUNDED;
        let start = overlay
            .starts_owned_by(CROSS_CHUNK_CANARY_START_CHUNK)
            .pop()
            .unwrap();

        assert_eq!(
            start.touched_chunks(topology).unwrap(),
            [ChunkPos::new(0, 0), ChunkPos::new(1, 0)]
        );
        assert_eq!(overlay.starts_owned_by(ChunkPos::new(1, 0)), []);
        assert_eq!(
            overlay
                .references_for(ChunkPos::new(0, 0), topology)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            overlay
                .references_for(ChunkPos::new(1, 0), topology)
                .unwrap()
                .len(),
            1
        );
        assert!(
            overlay
                .references_for(ChunkPos::new(2, 0), topology)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn canary_materialization_clips_exactly_to_each_target_chunk() {
        let overlay = StructureOverlay::CrossChunkCanaryV1;
        let topology = HorizontalTopology::UNBOUNDED;
        let mut left = empty_chunk(ChunkPos::new(0, 0));
        let mut right = empty_chunk(ChunkPos::new(1, 0));
        let left_receipt = overlay
            .materialize_chunk(
                ChunkPos::new(0, 0),
                topology,
                &overlay
                    .references_for(ChunkPos::new(0, 0), topology)
                    .unwrap(),
                &mut left,
            )
            .unwrap();
        let right_receipt = overlay
            .materialize_chunk(
                ChunkPos::new(1, 0),
                topology,
                &overlay
                    .references_for(ChunkPos::new(1, 0), topology)
                    .unwrap(),
                &mut right,
            )
            .unwrap();

        assert_eq!(left_receipt.placed_blocks, 5);
        assert_eq!(left_receipt.skipped_outside_target, 5);
        assert_eq!(right_receipt.placed_blocks, 5);
        assert_eq!(right_receipt.skipped_outside_target, 5);
        assert_eq!(left.block_count(STONE_BRICKS), 2);
        assert_eq!(right.block_count(STONE_BRICKS), 2);
        assert_eq!(left.block_count(COBBLESTONE), 1);
        assert_eq!(right.block_count(COBBLESTONE), 1);
    }
}
