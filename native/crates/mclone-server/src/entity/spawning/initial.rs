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

const MEMBER_OFFSETS: [(i32, i32); 4] = [(0, 0), (2, 0), (-2, 1), (1, -2)];
const PLACEMENT_SEARCH_RADIUS: i32 = 6;

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

fn species_entity_kind(species: McloneWildlifeSpecies) -> EntityKind {
    match species {
        McloneWildlifeSpecies::Rabbit => EntityKind::Rabbit,
        McloneWildlifeSpecies::Deer => EntityKind::Deer,
        McloneWildlifeSpecies::Mallard => EntityKind::Mallard,
        McloneWildlifeSpecies::Bee => EntityKind::BeeNest,
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
    use mclone_worldgen::block::{AIR, GRASS_BLOCK};

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
}
