use std::collections::BTreeSet;

use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_protocol::EntityKind;
use mclone_worldgen::{
    block::RawBlockId,
    levelgen::{McloneWildlifeEncounter, McloneWildlifeSpecies},
};

use super::{
    dry_run::top_motion_blocking_no_leaves_feet_y, placements::check_debug_actor_placement,
};
use crate::entity::spawning::habitat::{SquirrelHabitatSample, sample_squirrel_habitat};

const MEMBER_OFFSETS: [(i32, i32); 4] = [(0, 0), (2, 0), (-2, 1), (1, -2)];
const PLACEMENT_SEARCH_RADIUS: i32 = 6;
const SQUIRREL_PLACEMENT_SEARCH_RADIUS: i32 = 13;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InitialWildlifePlacement {
    Group {
        kind: EntityKind,
        positions: Vec<Vec3d>,
    },
    BeeColony {
        position: Vec3d,
        bee_positions: Vec<Vec3d>,
    },
}

pub(crate) fn plan_initial_wildlife_placement<F>(
    encounter: McloneWildlifeEncounter,
    mut block_at: F,
) -> Option<InitialWildlifePlacement>
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
{
    let kind = species_entity_kind(encounter.species);
    let placement_count = if encounter.species == McloneWildlifeSpecies::Bee {
        1
    } else {
        usize::from(encounter.group_size)
    };
    let mut positions = Vec::with_capacity(placement_count);
    let mut occupied = BTreeSet::new();
    for index in 0..placement_count {
        let offset = MEMBER_OFFSETS[index % MEMBER_OFFSETS.len()];
        let preferred_x = encounter.anchor_x.saturating_add(offset.0);
        let preferred_z = encounter.anchor_z.saturating_add(offset.1);
        let feet = find_safe_position_in_chunk(
            kind,
            encounter.owner_chunk,
            preferred_x,
            preferred_z,
            &occupied,
            &mut block_at,
        )?;
        occupied.insert((feet.x, feet.z));
        positions.push(Vec3d::new(
            f64::from(feet.x) + 0.5,
            f64::from(feet.y),
            f64::from(feet.z) + 0.5,
        ));
    }

    if encounter.species == McloneWildlifeSpecies::Bee {
        let position = positions[0];
        let bee_positions = (0..usize::from(encounter.group_size))
            .map(|index| {
                let angle = index as f64 / f64::from(encounter.group_size) * std::f64::consts::TAU;
                position.add(Vec3d::new(
                    angle.sin() * 1.4,
                    0.9 + index as f64 * 0.22,
                    angle.cos() * 1.4,
                ))
            })
            .collect();
        Some(InitialWildlifePlacement::BeeColony {
            position,
            bee_positions,
        })
    } else {
        Some(InitialWildlifePlacement::Group { kind, positions })
    }
}

pub(crate) fn plan_initial_squirrel_placement<F, G>(
    encounter: McloneWildlifeEncounter,
    generated_suitability: u16,
    forest_cover: u16,
    recently_disturbed: bool,
    mut mast_accessible_at: G,
    mut block_at: F,
) -> Option<(InitialWildlifePlacement, SquirrelHabitatSample)>
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
    G: FnMut(BlockPos) -> bool,
{
    if encounter.species != McloneWildlifeSpecies::Squirrel {
        return None;
    }
    let (feet, habitat) = find_suitable_squirrel_founder(
        encounter,
        generated_suitability,
        forest_cover,
        recently_disturbed,
        &mut mast_accessible_at,
        &mut block_at,
    )?;
    let relocated = McloneWildlifeEncounter {
        anchor_x: feet.x,
        anchor_z: feet.z,
        ..encounter
    };
    let placement = plan_initial_wildlife_placement(relocated, &mut block_at)?;
    Some((placement, habitat))
}

fn find_suitable_squirrel_founder(
    encounter: McloneWildlifeEncounter,
    generated_suitability: u16,
    forest_cover: u16,
    recently_disturbed: bool,
    mast_accessible_at: &mut impl FnMut(BlockPos) -> bool,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<(BlockPos, SquirrelHabitatSample)> {
    let min_x = encounter.owner_chunk.min_block_x() + 1;
    let min_z = encounter.owner_chunk.min_block_z() + 1;
    let max_x = encounter.owner_chunk.min_block_x() + 14;
    let max_z = encounter.owner_chunk.min_block_z() + 14;
    let preferred_x = encounter.anchor_x.clamp(min_x, max_x);
    let preferred_z = encounter.anchor_z.clamp(min_z, max_z);
    for radius in 0..=SQUIRREL_PLACEMENT_SEARCH_RADIUS {
        for z_offset in -radius..=radius {
            for x_offset in -radius..=radius {
                if x_offset.abs().max(z_offset.abs()) != radius {
                    continue;
                }
                let x = preferred_x + x_offset;
                let z = preferred_z + z_offset;
                if !(min_x..=max_x).contains(&x) || !(min_z..=max_z).contains(&z) {
                    continue;
                }
                let Ok(feet_y) = top_motion_blocking_no_leaves_feet_y(x, z, &mut *block_at) else {
                    continue;
                };
                let feet = BlockPos::new(x, feet_y, z);
                if check_debug_actor_placement(EntityKind::Squirrel, feet, &mut *block_at).is_err()
                {
                    continue;
                }
                let Ok(habitat) = sample_squirrel_habitat(
                    feet,
                    generated_suitability,
                    forest_cover,
                    mast_accessible_at(feet),
                    recently_disturbed,
                    &mut *block_at,
                ) else {
                    continue;
                };
                if habitat.suitable() {
                    return Some((feet, habitat));
                }
            }
        }
    }
    None
}

fn species_entity_kind(species: McloneWildlifeSpecies) -> EntityKind {
    match species {
        McloneWildlifeSpecies::Rabbit => EntityKind::Rabbit,
        McloneWildlifeSpecies::Deer => EntityKind::Deer,
        McloneWildlifeSpecies::Mallard => EntityKind::Mallard,
        McloneWildlifeSpecies::Bee => EntityKind::BeeNest,
        McloneWildlifeSpecies::Squirrel => EntityKind::Squirrel,
    }
}

fn find_safe_position_in_chunk(
    kind: EntityKind,
    chunk: ChunkPos,
    preferred_x: i32,
    preferred_z: i32,
    occupied: &BTreeSet<(i32, i32)>,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Option<BlockPos> {
    let min_x = chunk.min_block_x() + 1;
    let min_z = chunk.min_block_z() + 1;
    let max_x = chunk.min_block_x() + 14;
    let max_z = chunk.min_block_z() + 14;
    let preferred_x = preferred_x.clamp(min_x, max_x);
    let preferred_z = preferred_z.clamp(min_z, max_z);
    for radius in 0..=PLACEMENT_SEARCH_RADIUS {
        for z_offset in -radius..=radius {
            for x_offset in -radius..=radius {
                if x_offset.abs().max(z_offset.abs()) != radius {
                    continue;
                }
                let x = preferred_x + x_offset;
                let z = preferred_z + z_offset;
                if !(min_x..=max_x).contains(&x)
                    || !(min_z..=max_z).contains(&z)
                    || occupied.contains(&(x, z))
                {
                    continue;
                }
                let Ok(feet_y) = top_motion_blocking_no_leaves_feet_y(x, z, &mut *block_at) else {
                    continue;
                };
                let feet = BlockPos::new(x, feet_y, z);
                if check_debug_actor_placement(kind, feet, &mut *block_at).is_ok() {
                    return Some(feet);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use mclone_worldgen::block::{AIR, DIRT, GRASS_BLOCK, OAK_LEAVES, OAK_LOG};

    use super::*;

    fn grass_world(pos: BlockPos) -> Option<RawBlockId> {
        Some(if pos.y == 63 { GRASS_BLOCK } else { AIR })
    }

    #[test]
    fn group_members_are_distinct_and_stay_in_the_owner_chunk() {
        let encounter = McloneWildlifeEncounter {
            species: McloneWildlifeSpecies::Rabbit,
            group_size: 4,
            anchor_x: 15,
            anchor_z: 15,
            owner_chunk: ChunkPos::new(0, 0),
        };
        let InitialWildlifePlacement::Group { kind, positions } =
            plan_initial_wildlife_placement(encounter, grass_world).unwrap()
        else {
            panic!("rabbit encounter should create a group");
        };
        assert_eq!(kind, EntityKind::Rabbit);
        assert_eq!(positions.len(), 4);
        assert_eq!(
            positions
                .iter()
                .map(|position| { (position.x.floor() as i32, position.z.floor() as i32) })
                .collect::<BTreeSet<_>>()
                .len(),
            4
        );
        assert!(positions.iter().all(|position| {
            ChunkPos::from_block_coords(position.x.floor() as i32, position.z.floor() as i32)
                == encounter.owner_chunk
        }));
    }

    #[test]
    fn bee_encounter_builds_one_colony_with_the_planned_bee_count() {
        let encounter = McloneWildlifeEncounter {
            species: McloneWildlifeSpecies::Bee,
            group_size: 3,
            anchor_x: 8,
            anchor_z: 8,
            owner_chunk: ChunkPos::new(0, 0),
        };
        let InitialWildlifePlacement::BeeColony {
            position,
            bee_positions,
        } = plan_initial_wildlife_placement(encounter, grass_world).unwrap()
        else {
            panic!("bee encounter should create a colony");
        };
        assert_eq!(position, Vec3d::new(8.5, 64.0, 8.5));
        assert_eq!(bee_positions.len(), 3);
    }

    fn squirrel_edge(pos: BlockPos) -> Option<RawBlockId> {
        Some(if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos.x == 3 && pos.z == 0 && (64..=68).contains(&pos.y) {
            OAK_LOG
        } else if pos.y == 68 && (pos.x - 3).abs() <= 2 && pos.z.abs() <= 2 && pos.x != 3 {
            OAK_LEAVES
        } else {
            AIR
        })
    }

    #[test]
    fn squirrel_placement_requires_live_mast_refuge_and_quiet_edge() {
        let encounter = McloneWildlifeEncounter {
            species: McloneWildlifeSpecies::Squirrel,
            group_size: 2,
            anchor_x: 0,
            anchor_z: 0,
            owner_chunk: ChunkPos::new(0, 0),
        };
        let (placement, habitat) =
            plan_initial_squirrel_placement(encounter, 420, 480, false, |_| true, squirrel_edge)
                .expect("woodland edge with reached mast should admit squirrels");
        let InitialWildlifePlacement::Group { kind, positions } = placement else {
            panic!("squirrels should use ordinary group placement");
        };
        assert_eq!(kind, EntityKind::Squirrel);
        assert_eq!(positions.len(), 2);
        assert!(habitat.suitable());
        assert!(habitat.refuge.is_some());

        assert!(
            plan_initial_squirrel_placement(encounter, 420, 480, false, |_| false, squirrel_edge,)
                .is_none(),
            "unreachable mast must not admit the encounter"
        );
        assert!(
            plan_initial_squirrel_placement(encounter, 420, 480, true, |_| true, squirrel_edge,)
                .is_none(),
            "recent player disturbance must suppress initial materialization"
        );
    }
}
