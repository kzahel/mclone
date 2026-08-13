//! Structure-lifecycle adapter for the persisted intro homestead plan.
//!
//! The plan owns canonical content identities, transforms, bounds, and touched
//! chunks. This adapter resolves those facts into ordinary structure starts,
//! references, and per-target clipped piece placement.

use mclone_core::{BlockPos, ChunkPos, HorizontalTopology, local_block_coord};
use mclone_worldgen::levelgen::GeneratedChunk;
use mclone_worldgen::structure_template::{
    StructurePlaceSettings, TemplateMirror, TemplateRotation,
};

use crate::homestead_landscape::{
    compile_landscape_pieces, is_dynamic_landscape_piece, materialize_dynamic_landscape_piece,
};
use crate::homestead_plan::{
    HomesteadPlanPiece, HomesteadPlanPieceKind, HomesteadPlanTemplateRotation,
    IntroHomesteadPlanRecord, load_homestead_structure_record,
    validate_intro_homestead_plan_for_world,
};
use crate::structure::{
    StructureBlockPlacement, StructureBoundingBox, StructurePieceRecord, StructurePlacementReceipt,
    StructureReference, StructureStartRecord,
};

pub const INTRO_HOMESTEAD_STRUCTURE_ID: &str = "mclone:intro_homestead_v1";

#[derive(Clone, Debug)]
pub struct IntroHomesteadStructureOverlay {
    topology: HorizontalTopology,
    plan: IntroHomesteadPlanRecord,
    start: StructureStartRecord,
}

impl IntroHomesteadStructureOverlay {
    pub fn first_cottage(
        plan: IntroHomesteadPlanRecord,
        topology: HorizontalTopology,
    ) -> Result<Self, String> {
        Self::from_piece_kinds(&plan, topology, &[HomesteadPlanPieceKind::Cottage])
    }

    pub fn farmstead_buildings(
        plan: IntroHomesteadPlanRecord,
        topology: HorizontalTopology,
    ) -> Result<Self, String> {
        Self::from_piece_kinds(
            &plan,
            topology,
            &[
                HomesteadPlanPieceKind::Cottage,
                HomesteadPlanPieceKind::Barn,
                HomesteadPlanPieceKind::BarnLeanTo,
                HomesteadPlanPieceKind::ChickenCoop,
            ],
        )
    }

    pub fn farmstead_composition(
        plan: IntroHomesteadPlanRecord,
        topology: HorizontalTopology,
    ) -> Result<Self, String> {
        let mut overlay = Self::farmstead_buildings(plan, topology)?;
        overlay
            .start
            .pieces
            .extend(compile_landscape_pieces(&overlay.plan)?);
        overlay
            .start
            .pieces
            .sort_by(|left, right| left.piece_id.cmp(&right.piece_id));
        overlay.start.bounds = encompassing_bounds(&overlay.start.pieces)?;
        Ok(overlay)
    }

    fn from_piece_kinds(
        plan: &IntroHomesteadPlanRecord,
        topology: HorizontalTopology,
        kinds: &[HomesteadPlanPieceKind],
    ) -> Result<Self, String> {
        validate_intro_homestead_plan_for_world(
            plan,
            plan.base_descriptor.seed,
            plan.base_descriptor.generation_profile,
            topology,
        )?;
        let mut pieces = Vec::with_capacity(kinds.len());
        for kind in kinds {
            let matching = plan
                .pieces
                .iter()
                .filter(|piece| piece.kind == *kind)
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(format!(
                    "intro homestead plan must contain exactly one {kind:?}, found {}",
                    matching.len()
                ));
            }
            pieces.push(compile_piece(matching[0], topology)?);
        }
        pieces.sort_by(|left, right| left.piece_id.cmp(&right.piece_id));
        let bounds = encompassing_bounds(&pieces)?;
        let start_chunk = topology
            .canonicalize_chunk(ChunkPos::from_block_coords(
                plan.selected_site.candidate.anchor_x,
                plan.selected_site.candidate.anchor_z,
            ))
            .ok_or_else(|| "intro homestead start anchor is outside topology".to_owned())?;
        Ok(Self {
            topology,
            plan: plan.clone(),
            start: StructureStartRecord {
                structure_id: INTRO_HOMESTEAD_STRUCTURE_ID.to_owned(),
                start_chunk,
                references: 0,
                bounds,
                pieces,
            },
        })
    }

    pub fn starts_owned_by(&self, pos: ChunkPos) -> Vec<StructureStartRecord> {
        (pos == self.start.start_chunk)
            .then(|| self.start.clone())
            .into_iter()
            .collect()
    }

    pub fn references_for(&self, target: ChunkPos) -> Result<Vec<StructureReference>, String> {
        let target = self
            .topology
            .canonicalize_chunk(target)
            .ok_or_else(|| format!("homestead structure target {target:?} is outside topology"))?;
        Ok(self
            .start
            .touched_chunks(self.topology)?
            .contains(&target)
            .then(|| StructureReference {
                structure_id: self.start.structure_id.clone(),
                start_chunk: self.start.start_chunk,
            })
            .into_iter()
            .collect())
    }

    pub fn materialize_chunk(
        &self,
        target: ChunkPos,
        references: &[StructureReference],
        chunk: &mut GeneratedChunk,
    ) -> Result<StructurePlacementReceipt, String> {
        if ChunkPos::new(chunk.chunk_x, chunk.chunk_z) != target {
            return Err(format!(
                "homestead structure target {target:?} does not match generated chunk ({}, {})",
                chunk.chunk_x, chunk.chunk_z
            ));
        }
        let mut receipt = StructurePlacementReceipt::default();
        for reference in references
            .iter()
            .filter(|reference| reference.structure_id == INTRO_HOMESTEAD_STRUCTURE_ID)
        {
            if reference.start_chunk != self.start.start_chunk {
                return Err(format!(
                    "unresolvable homestead structure reference {reference:?}"
                ));
            }
            receipt.referenced_starts += 1;
            for piece in &self.start.pieces {
                if !piece.bounds.intersects_chunk(target) || !is_dynamic_landscape_piece(piece) {
                    continue;
                }
                receipt.intersecting_pieces += 1;
                materialize_dynamic_landscape_piece(
                    &self.plan,
                    piece,
                    target,
                    chunk,
                    &mut receipt,
                )?;
            }
            for piece in &self.start.pieces {
                if !piece.bounds.intersects_chunk(target) || is_dynamic_landscape_piece(piece) {
                    continue;
                }
                receipt.intersecting_pieces += 1;
                for placement in &piece.blocks {
                    receipt.considered_blocks += 1;
                    let placement_chunk = self
                        .topology
                        .canonicalize_chunk(placement.pos.chunk_pos())
                        .ok_or_else(|| {
                            format!(
                                "homestead structure block {:?} is outside topology",
                                placement.pos
                            )
                        })?;
                    if placement_chunk != target {
                        receipt.skipped_outside_target += 1;
                        continue;
                    }
                    if placement.pos.y < chunk.min_y
                        || placement.pos.y >= chunk.min_y + chunk.height
                    {
                        return Err(format!(
                            "homestead structure block {:?} is outside generated vertical bounds",
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

    pub fn start(&self) -> &StructureStartRecord {
        &self.start
    }
}

fn encompassing_bounds(pieces: &[StructurePieceRecord]) -> Result<StructureBoundingBox, String> {
    let first = pieces
        .first()
        .ok_or_else(|| "homestead structure requires at least one piece".to_owned())?;
    let mut min = first.bounds.min;
    let mut max = first.bounds.max;
    for piece in &pieces[1..] {
        min.x = min.x.min(piece.bounds.min.x);
        min.y = min.y.min(piece.bounds.min.y);
        min.z = min.z.min(piece.bounds.min.z);
        max.x = max.x.max(piece.bounds.max.x);
        max.y = max.y.max(piece.bounds.max.y);
        max.z = max.z.max(piece.bounds.max.z);
    }
    StructureBoundingBox::new(min, max)
}

fn compile_piece(
    piece: &HomesteadPlanPiece,
    topology: HorizontalTopology,
) -> Result<StructurePieceRecord, String> {
    let record = load_homestead_structure_record(&piece.content_id)?;
    if piece.semantic_sha256.as_deref() != Some(record.provenance.semantic_sha256.as_str()) {
        return Err(format!(
            "homestead piece {} semantic checksum does not match canonical content",
            piece.piece_id
        ));
    }
    let theme = record.default_theme().ok_or_else(|| {
        format!(
            "homestead content {} has no default material theme",
            piece.content_id
        )
    })?;
    let placed = record
        .template
        .place(&StructurePlaceSettings {
            origin: BlockPos::new(piece.origin[0], piece.origin[1], piece.origin[2]),
            rotation: template_rotation(piece.rotation),
            mirror: TemplateMirror::None,
            theme,
        })
        .map_err(|error| error.to_string())?;
    let bounds = StructureBoundingBox::new(
        placed.bounds.min,
        placed.bounds.max_exclusive.offset(-1, -1, -1),
    )?;
    let compiled_bounds = [
        bounds.min.x,
        bounds.min.y,
        bounds.min.z,
        bounds.max.x,
        bounds.max.y,
        bounds.max.z,
    ];
    let planned_bounds = [
        piece.bounds.min[0],
        piece.bounds.min[1],
        piece.bounds.min[2],
        piece.bounds.max[0],
        piece.bounds.max[1],
        piece.bounds.max[2],
    ];
    if compiled_bounds != planned_bounds {
        return Err(format!(
            "homestead piece {} compiled bounds {compiled_bounds:?} do not match plan {planned_bounds:?}",
            piece.piece_id
        ));
    }
    let touched = bounds
        .touched_chunks(topology)?
        .into_iter()
        .map(|chunk| [chunk.x, chunk.z])
        .collect::<Vec<_>>();
    if touched != piece.touched_chunks {
        return Err(format!(
            "homestead piece {} touched chunks do not match its plan",
            piece.piece_id
        ));
    }
    Ok(StructurePieceRecord {
        piece_id: piece.piece_id.clone(),
        bounds,
        blocks: placed
            .blocks
            .into_iter()
            .map(|block| StructureBlockPlacement {
                pos: block.pos,
                block: block.block,
            })
            .collect(),
    })
}

const fn template_rotation(rotation: HomesteadPlanTemplateRotation) -> TemplateRotation {
    match rotation {
        HomesteadPlanTemplateRotation::None => TemplateRotation::None,
        HomesteadPlanTemplateRotation::Clockwise90 => TemplateRotation::Clockwise90,
        HomesteadPlanTemplateRotation::Clockwise180 => TemplateRotation::Clockwise180,
        HomesteadPlanTemplateRotation::CounterClockwise90 => TemplateRotation::CounterClockwise90,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mclone_worldgen::block::{
        AIR, CARROTS_AGE_0, CARROTS_AGE_7, FARMLAND_MOISTURE_0, FARMLAND_MOISTURE_7,
        OAK_FENCE_GATE_STATE_END, OAK_FENCE_GATE_STATE_START, OAK_FENCE_STATE_END,
        OAK_FENCE_STATE_START, OAK_LEAVES, OAK_LOG, RawBlockId, WHEAT_AGE_0, WHEAT_AGE_7,
    };
    use mclone_worldgen::levelgen::McloneOverworldFeatureDependencyCache;

    use super::*;
    use crate::{
        IntroHomesteadTerrainOverlay, WorldGenerationProfile, realize_intro_homestead_plan,
    };

    #[test]
    fn accepted_cottage_is_one_clipped_persistable_structure_start() {
        let plan = realize_intro_homestead_plan(
            0,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let overlay = IntroHomesteadStructureOverlay::first_cottage(
            plan.clone(),
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let start = overlay.start();
        let cottage = plan
            .pieces
            .iter()
            .find(|piece| piece.kind == HomesteadPlanPieceKind::Cottage)
            .unwrap();
        assert_eq!(start.start_chunk, ChunkPos::new(-13, -87));
        assert_eq!(start.pieces.len(), 1);
        assert_eq!(start.pieces[0].piece_id, cottage.piece_id);
        assert_eq!(start.pieces[0].blocks.len(), 1_108);

        let touched = start.touched_chunks(HorizontalTopology::UNBOUNDED).unwrap();
        assert_eq!(
            touched,
            cottage
                .touched_chunks
                .iter()
                .map(|chunk| ChunkPos::new(chunk[0], chunk[1]))
                .collect::<Vec<_>>()
        );
        let generate = |order: &[ChunkPos]| {
            let mut chunks = BTreeMap::new();
            let mut receipt = StructurePlacementReceipt::default();
            for pos in order {
                let mut chunk =
                    GeneratedChunk::from_raw_parts(pos.x, pos.z, 0, 256, vec![AIR; 16 * 256 * 16]);
                let references = overlay.references_for(*pos).unwrap();
                let current = overlay
                    .materialize_chunk(*pos, &references, &mut chunk)
                    .unwrap();
                receipt.referenced_starts += current.referenced_starts;
                receipt.intersecting_pieces += current.intersecting_pieces;
                receipt.considered_blocks += current.considered_blocks;
                receipt.placed_blocks += current.placed_blocks;
                receipt.skipped_outside_target += current.skipped_outside_target;
                chunks.insert(*pos, chunk.blocks().to_vec());
            }
            (chunks, receipt)
        };
        let (forward, receipt) = generate(&touched);
        let (reverse, reverse_receipt) =
            generate(&touched.iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(forward, reverse);
        assert_eq!(receipt, reverse_receipt);
        assert_eq!(receipt.placed_blocks, start.pieces[0].blocks.len());
    }

    #[test]
    fn accepted_farmstead_buildings_share_one_clipped_structure_start() {
        let plan = realize_intro_homestead_plan(
            0,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let overlay = IntroHomesteadStructureOverlay::farmstead_buildings(
            plan,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let start = overlay.start();

        assert_eq!(start.start_chunk, ChunkPos::new(-13, -87));
        assert_eq!(start.pieces.len(), 4);
        assert_eq!(
            start
                .pieces
                .iter()
                .map(|piece| piece.piece_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "building-barn-lean-to-v1",
                "building-barn-v1",
                "building-chicken-coop-v1",
                "building-cottage-v1",
            ]
        );

        let touched = start.touched_chunks(HorizontalTopology::UNBOUNDED).unwrap();
        let expected_blocks = start
            .pieces
            .iter()
            .map(|piece| piece.blocks.len())
            .sum::<usize>();
        let placed_blocks = touched
            .into_iter()
            .map(|pos| {
                let mut chunk =
                    GeneratedChunk::from_raw_parts(pos.x, pos.z, 0, 256, vec![AIR; 16 * 256 * 16]);
                let references = overlay.references_for(pos).unwrap();
                overlay
                    .materialize_chunk(pos, &references, &mut chunk)
                    .unwrap()
                    .placed_blocks
            })
            .sum::<usize>();
        assert_eq!(placed_blocks, expected_blocks);
    }

    #[test]
    fn accepted_complete_composition_is_order_independent() {
        let plan = realize_intro_homestead_plan(
            0,
            WorldGenerationProfile::McloneOverworldV1,
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let terrain = IntroHomesteadTerrainOverlay::new(plan.clone()).unwrap();
        let overlay = IntroHomesteadStructureOverlay::farmstead_composition(
            plan.clone(),
            HorizontalTopology::UNBOUNDED,
        )
        .unwrap();
        let targets = overlay
            .start()
            .touched_chunks(HorizontalTopology::UNBOUNDED)
            .unwrap();
        let generated = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(0, targets.iter().copied())
            .chunks;
        let apply = |order: &[ChunkPos]| {
            let mut chunks = generated.clone();
            let mut receipt = StructurePlacementReceipt::default();
            for pos in order {
                let chunk = chunks.get_mut(pos).unwrap();
                terrain.materialize_chunk(*pos, chunk).unwrap();
                let references = overlay.references_for(*pos).unwrap();
                let current = overlay.materialize_chunk(*pos, &references, chunk).unwrap();
                receipt.referenced_starts += current.referenced_starts;
                receipt.intersecting_pieces += current.intersecting_pieces;
                receipt.considered_blocks += current.considered_blocks;
                receipt.placed_blocks += current.placed_blocks;
                receipt.skipped_outside_target += current.skipped_outside_target;
            }
            (chunks, receipt)
        };
        let (forward, receipt) = apply(&targets);
        let (reverse, reverse_receipt) = apply(&targets.iter().rev().copied().collect::<Vec<_>>());

        assert_eq!(forward, reverse);
        assert_eq!(receipt, reverse_receipt);
        assert_eq!(overlay.start().pieces.len(), 9);
        assert!(
            receipt.placed_blocks > 3_800,
            "expected building and landscape placements, got {receipt:?}"
        );
        assert_eq!(
            block_at(
                &forward,
                plan.planting.focal_oak[0],
                plan.planting.focal_oak[1],
                plan.planting.focal_oak[2],
            ),
            Some(OAK_LOG)
        );
        let authored_counts = forward
            .values()
            .flat_map(|chunk| chunk.blocks().iter().copied())
            .fold([0usize; 6], |mut counts, block| {
                if block == OAK_LEAVES {
                    counts[0] += 1;
                } else if (OAK_FENCE_STATE_START..=OAK_FENCE_STATE_END).contains(&block) {
                    counts[1] += 1;
                } else if (OAK_FENCE_GATE_STATE_START..=OAK_FENCE_GATE_STATE_END).contains(&block) {
                    counts[2] += 1;
                } else if (FARMLAND_MOISTURE_0..=FARMLAND_MOISTURE_7).contains(&block) {
                    counts[3] += 1;
                } else if (WHEAT_AGE_0..=WHEAT_AGE_7).contains(&block) {
                    counts[4] += 1;
                } else if (CARROTS_AGE_0..=CARROTS_AGE_7).contains(&block) {
                    counts[5] += 1;
                }
                counts
            });
        assert!(authored_counts[0] > 100);
        assert_eq!(&authored_counts[1..], &[35, 1, 48, 24, 24]);
        let garden = plan
            .pieces
            .iter()
            .find(|piece| piece.kind == HomesteadPlanPieceKind::Garden)
            .unwrap();
        assert_eq!(garden.content_id, "farmstead-kitchen-garden-v1");
        assert!(garden.semantic_sha256.is_some());
        assert!(plan.source_fingerprints.iter().any(|source| {
            source.content_id == garden.content_id
                && Some(source.semantic_sha256.as_str()) == garden.semantic_sha256.as_deref()
        }));
    }

    fn block_at(
        chunks: &BTreeMap<ChunkPos, GeneratedChunk>,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<RawBlockId> {
        chunks
            .get(&ChunkPos::new(x.div_euclid(16), z.div_euclid(16)))
            .map(|chunk| chunk.block_at_y(x.rem_euclid(16), y, z.rem_euclid(16)).0)
    }
}
