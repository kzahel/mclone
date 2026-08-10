//! Terrain-matched semantic pieces for the intro homestead composition.
//!
//! Revision one deliberately uses only the honest generated-block vocabulary:
//! path surfacing, soil and flowers, a low open hedge, hay, and an authored oak.
//! Fence/gate and crop gameplay remain later content work.

use mclone_core::{BlockPos, ChunkPos};
use mclone_worldgen::block::{
    COARSE_DIRT, DANDELION, DIRT, GRAVEL, HAY_BLOCK, OAK_LEAVES, OAK_LOG, OAK_LOG_X, OAK_LOG_Z,
    POPPY, RawBlockId, material_blocks_motion,
};
use mclone_worldgen::homestead_site::HomesteadRotation;
use mclone_worldgen::levelgen::GeneratedChunk;

use crate::homestead_plan::{HomesteadPlanPieceKind, IntroHomesteadPlanRecord};
use crate::{
    StructureBlockPlacement, StructureBoundingBox, StructurePieceRecord, StructurePlacementReceipt,
};

pub(super) const CIRCULATION_PIECE_ID: &str = "circulation-v1";

pub(super) fn compile_landscape_pieces(
    plan: &IntroHomesteadPlanRecord,
) -> Result<Vec<StructurePieceRecord>, String> {
    let mut pieces = plan
        .pieces
        .iter()
        .filter(|piece| {
            matches!(
                piece.kind,
                HomesteadPlanPieceKind::Garden
                    | HomesteadPlanPieceKind::AnimalYard
                    | HomesteadPlanPieceKind::AuthoredWater
                    | HomesteadPlanPieceKind::FocalOak
            )
        })
        .map(|piece| {
            let blocks = (piece.kind == HomesteadPlanPieceKind::FocalOak)
                .then(|| focal_oak_blocks(plan))
                .unwrap_or_default();
            Ok(StructurePieceRecord {
                piece_id: piece.piece_id.clone(),
                bounds: StructureBoundingBox::new(
                    BlockPos::new(
                        piece.bounds.min[0],
                        piece.bounds.min[1],
                        piece.bounds.min[2],
                    ),
                    BlockPos::new(
                        piece.bounds.max[0],
                        piece.bounds.max[1],
                        piece.bounds.max[2],
                    ),
                )?,
                blocks,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if pieces.len() != 4 {
        return Err(format!(
            "intro homestead plan must contain four landscape roles, found {}",
            pieces.len()
        ));
    }

    let core = plan.selected_site.core_bounds;
    pieces.push(StructurePieceRecord {
        piece_id: CIRCULATION_PIECE_ID.to_owned(),
        bounds: StructureBoundingBox::new(
            BlockPos::new(
                core.min_x,
                plan.selected_site.metrics.surface_min_y,
                core.min_z,
            ),
            BlockPos::new(
                core.max_x,
                plan.selected_site.metrics.surface_max_y + 1,
                core.max_z,
            ),
        )?,
        blocks: Vec::new(),
    });
    Ok(pieces)
}

pub(super) fn is_dynamic_landscape_piece(piece: &StructurePieceRecord) -> bool {
    matches!(
        piece.piece_id.as_str(),
        "garden-v1" | "animal-yard-v1" | CIRCULATION_PIECE_ID
    )
}

pub(super) fn materialize_dynamic_landscape_piece(
    plan: &IntroHomesteadPlanRecord,
    piece: &StructurePieceRecord,
    target: ChunkPos,
    chunk: &mut GeneratedChunk,
    receipt: &mut StructurePlacementReceipt,
) -> Result<(), String> {
    match piece.piece_id.as_str() {
        "garden-v1" => materialize_garden(plan, piece, target, chunk, receipt),
        "animal-yard-v1" => materialize_animal_yard(plan, piece, target, chunk, receipt),
        CIRCULATION_PIECE_ID => materialize_circulation(plan, piece, target, chunk, receipt),
        _ => Ok(()),
    }
}

fn materialize_garden(
    plan: &IntroHomesteadPlanRecord,
    piece: &StructurePieceRecord,
    target: ChunkPos,
    chunk: &mut GeneratedChunk,
    receipt: &mut StructurePlacementReceipt,
) -> Result<(), String> {
    for_each_piece_column(piece, target, |world_x, world_z| {
        let (forward, right) = world_to_local(plan, world_x, world_z);
        let bed = (-26..=-14).contains(&forward) && matches!(right, 12..=14 | 18..=20 | 24..=26);
        if !bed {
            return Ok(());
        }
        let Some(surface_y) = terrain_surface_y(chunk, world_x, world_z) else {
            return Ok(());
        };
        let soil = if (forward + right).rem_euclid(5) == 0 {
            COARSE_DIRT
        } else {
            DIRT
        };
        place_dynamic(chunk, world_x, surface_y, world_z, soil, receipt)?;
        if (forward + 2 * right).rem_euclid(3) == 0 {
            let flower = if (forward - right).rem_euclid(2) == 0 {
                DANDELION
            } else {
                POPPY
            };
            place_dynamic(chunk, world_x, surface_y + 1, world_z, flower, receipt)?;
        }
        Ok(())
    })
}

fn materialize_animal_yard(
    plan: &IntroHomesteadPlanRecord,
    piece: &StructurePieceRecord,
    target: ChunkPos,
    chunk: &mut GeneratedChunk,
    receipt: &mut StructurePlacementReceipt,
) -> Result<(), String> {
    for_each_piece_column(piece, target, |world_x, world_z| {
        let (forward, right) = world_to_local(plan, world_x, world_z);
        let on_edge = matches!(forward, 8 | 30) || matches!(right, 11 | 30);
        let open_entry = forward == 8 && (19..=23).contains(&right);
        if on_edge && !open_entry {
            if let Some(surface_y) = terrain_surface_y(chunk, world_x, world_z) {
                place_dynamic(chunk, world_x, surface_y + 1, world_z, OAK_LEAVES, receipt)?;
            }
        } else if (forward, right) == (29, 13) || (forward, right) == (29, 14) {
            if let Some(surface_y) = terrain_surface_y(chunk, world_x, world_z) {
                place_dynamic(chunk, world_x, surface_y + 1, world_z, HAY_BLOCK, receipt)?;
            }
        } else if (9..=29).contains(&forward)
            && (12..=29).contains(&right)
            && (forward * 31 + right * 17).rem_euclid(19) == 0
        {
            if let Some(surface_y) = terrain_surface_y(chunk, world_x, world_z) {
                place_dynamic(chunk, world_x, surface_y, world_z, COARSE_DIRT, receipt)?;
            }
        }
        Ok(())
    })
}

fn materialize_circulation(
    plan: &IntroHomesteadPlanRecord,
    piece: &StructurePieceRecord,
    target: ChunkPos,
    chunk: &mut GeneratedChunk,
    receipt: &mut StructurePlacementReceipt,
) -> Result<(), String> {
    const BRANCHES: [((i32, i32), (i32, i32)); 3] = [
        ((-29, -2), (-29, -16)),
        ((0, -2), (5, -17)),
        ((0, 2), (10, 15)),
    ];
    for_each_piece_column(piece, target, |world_x, world_z| {
        let local = world_to_local(plan, world_x, world_z);
        if !BRANCHES
            .iter()
            .any(|(start, end)| within_segment_radius(local, *start, *end, 1))
        {
            return Ok(());
        }
        let Some(surface_y) = terrain_surface_y(chunk, world_x, world_z) else {
            return Ok(());
        };
        let surface = if (world_x * 13 + world_z * 7).rem_euclid(6) == 0 {
            GRAVEL
        } else {
            COARSE_DIRT
        };
        place_dynamic(chunk, world_x, surface_y, world_z, surface, receipt)
    })
}

fn focal_oak_blocks(plan: &IntroHomesteadPlanRecord) -> Vec<StructureBlockPlacement> {
    let [root_x, root_y, root_z] = plan.planting.focal_oak;
    let mut blocks = Vec::new();
    for (dy, radius) in [(4, 3_i32), (5, 4), (6, 3), (7, 2)] {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let edge = dx.abs().max(dz.abs()) == radius;
                if dx.abs() + dz.abs() > radius + 2
                    || (edge && (dx * 17 + dz * 31 + dy * 13).rem_euclid(7) == 0)
                {
                    continue;
                }
                blocks.push(StructureBlockPlacement {
                    pos: BlockPos::new(root_x + dx, root_y + dy, root_z + dz),
                    block: OAK_LEAVES,
                });
            }
        }
    }
    for y in root_y..=root_y + 6 {
        blocks.push(StructureBlockPlacement {
            pos: BlockPos::new(root_x, y, root_z),
            block: OAK_LOG,
        });
    }
    for dx in -3..=3 {
        blocks.push(StructureBlockPlacement {
            pos: BlockPos::new(root_x + dx, root_y + 4, root_z),
            block: OAK_LOG_X,
        });
    }
    for dz in -2..=2 {
        blocks.push(StructureBlockPlacement {
            pos: BlockPos::new(root_x, root_y + 5, root_z + dz),
            block: OAK_LOG_Z,
        });
    }
    blocks
}

fn for_each_piece_column(
    piece: &StructurePieceRecord,
    target: ChunkPos,
    mut visit: impl FnMut(i32, i32) -> Result<(), String>,
) -> Result<(), String> {
    let min_x = piece.bounds.min.x.max(target.min_block_x());
    let max_x = piece.bounds.max.x.min(target.min_block_x() + 15);
    let min_z = piece.bounds.min.z.max(target.min_block_z());
    let max_z = piece.bounds.max.z.min(target.min_block_z() + 15);
    for world_z in min_z..=max_z {
        for world_x in min_x..=max_x {
            visit(world_x, world_z)?;
        }
    }
    Ok(())
}

fn terrain_surface_y(chunk: &GeneratedChunk, world_x: i32, world_z: i32) -> Option<i32> {
    let local_x = world_x.rem_euclid(16);
    let local_z = world_z.rem_euclid(16);
    (chunk.min_y..chunk.min_y + chunk.height)
        .rev()
        .find(|y| material_blocks_motion(chunk.block_at_y(local_x, *y, local_z).0))
}

fn place_dynamic(
    chunk: &mut GeneratedChunk,
    world_x: i32,
    y: i32,
    world_z: i32,
    block: RawBlockId,
    receipt: &mut StructurePlacementReceipt,
) -> Result<(), String> {
    receipt.considered_blocks += 1;
    if y < chunk.min_y || y >= chunk.min_y + chunk.height {
        return Err(format!(
            "homestead landscape block ({world_x}, {y}, {world_z}) is outside generated vertical bounds"
        ));
    }
    chunk.set_block_at_y(world_x.rem_euclid(16), y, world_z.rem_euclid(16), block);
    receipt.placed_blocks += 1;
    Ok(())
}

fn world_to_local(plan: &IntroHomesteadPlanRecord, x: i32, z: i32) -> (i32, i32) {
    let anchor_x = plan.selected_site.candidate.anchor_x;
    let anchor_z = plan.selected_site.candidate.anchor_z;
    match plan.selected_site.facing {
        HomesteadRotation::East => (x - anchor_x, z - anchor_z),
        HomesteadRotation::South => (z - anchor_z, anchor_x - x),
        HomesteadRotation::West => (anchor_x - x, anchor_z - z),
        HomesteadRotation::North => (anchor_z - z, x - anchor_x),
    }
}

fn within_segment_radius(
    point: (i32, i32),
    start: (i32, i32),
    end: (i32, i32),
    radius: i32,
) -> bool {
    let dx = i64::from(end.0 - start.0);
    let dz = i64::from(end.1 - start.1);
    let px = i64::from(point.0 - start.0);
    let pz = i64::from(point.1 - start.1);
    let length_squared = dx * dx + dz * dz;
    if length_squared == 0 {
        return px * px + pz * pz <= i64::from(radius * radius);
    }
    let projection = (px * dx + pz * dz).clamp(0, length_squared);
    let offset_x = px * length_squared - dx * projection;
    let offset_z = pz * length_squared - dz * projection;
    offset_x * offset_x + offset_z * offset_z
        <= i64::from(radius * radius) * length_squared * length_squared
}
