use crate::block::{GLOW_LICHEN, RawBlockId, WATER, is_air_like, material_blocks_motion};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{Direction, FeatureWorld, GlowLichenConfiguration, offset_pos};

pub(super) fn place_glow_lichen<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: GlowLichenConfiguration,
) -> bool {
    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    if !is_air_or_water(current) {
        return false;
    }

    let directions = shuffled_directions(&Direction::GLOW_LICHEN_VALID, random);
    if place_glow_lichen_if_possible(world, random, origin, current, config, &directions) {
        return true;
    }

    for direction in &directions {
        let candidate = offset_pos(origin, *direction);
        let side_directions =
            shuffled_directions_except(&Direction::GLOW_LICHEN_VALID, random, direction.opposite());

        for _ in 0..config.search_range {
            let Some(candidate_state) = world.block_at_world(candidate) else {
                break;
            };
            if !is_air_or_water(candidate_state) && candidate_state != GLOW_LICHEN {
                break;
            }
            if place_glow_lichen_if_possible(
                world,
                random,
                candidate,
                candidate_state,
                config,
                &side_directions,
            ) {
                return true;
            }
        }
    }

    false
}

fn place_glow_lichen_if_possible<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    pos: BlockPos,
    current: RawBlockId,
    config: GlowLichenConfiguration,
    directions: &[Direction],
) -> bool {
    if !is_air_or_water(current) && current != GLOW_LICHEN {
        return false;
    }

    for direction in directions {
        let neighbor = offset_pos(pos, *direction);
        let Some(neighbor_state) = world.block_at_world(neighbor) else {
            continue;
        };
        if !config.can_be_placed_on.contains(&neighbor_state) {
            continue;
        }

        let Some(next_faces) = glow_lichen_faces_for_placement(world, pos, current, *direction)
        else {
            return false;
        };
        if !world.set_glow_lichen_faces_world(pos, next_faces) {
            continue;
        }
        if random.next_float() < config.chance_of_spreading {
            spread_glow_lichen_from_face_toward_random_direction(world, pos, *direction, random);
        }
        return true;
    }

    false
}

fn shuffled_directions(directions: &[Direction], random: &mut impl RandomSource) -> Vec<Direction> {
    let mut shuffled = directions.to_vec();
    shuffle_java_style(&mut shuffled, random);
    shuffled
}

fn shuffled_directions_except(
    directions: &[Direction],
    random: &mut impl RandomSource,
    excluded: Direction,
) -> Vec<Direction> {
    let mut filtered = directions
        .iter()
        .copied()
        .filter(|direction| *direction != excluded)
        .collect::<Vec<_>>();
    shuffle_java_style(&mut filtered, random);
    filtered
}

fn shuffle_java_style<T>(items: &mut [T], random: &mut impl RandomSource) {
    for i in (2..=items.len()).rev() {
        let swap_with = random.next_int_bound(i as i32) as usize;
        items.swap(i - 1, swap_with);
    }
}

fn spread_glow_lichen_from_face_toward_random_direction<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    source_face: Direction,
    random: &mut impl RandomSource,
) -> bool {
    // Java mutates MultifaceBlock.DIRECTIONS via Arrays.asList(DIRECTIONS) before iterating it.
    let mut directions = world.glow_lichen_spread_direction_order();
    shuffle_java_style(&mut directions, random);
    world.set_glow_lichen_spread_direction_order(directions);
    for direction in directions {
        if spread_glow_lichen_from_face_toward_direction(world, pos, source_face, direction) {
            return true;
        }
    }
    false
}

pub(super) fn spread_glow_lichen_from_face_toward_direction<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    source_face: Direction,
    toward: Direction,
) -> bool {
    if toward.axis() == source_face.axis() {
        return false;
    }
    let current_faces = world.glow_lichen_faces_world(pos);
    if current_faces & source_face.bit() == 0 || current_faces & toward.bit() != 0 {
        return false;
    }

    if spread_glow_lichen_to_face(world, pos, toward) {
        return true;
    }

    let side_pos = offset_pos(pos, toward);
    if spread_glow_lichen_to_face(world, side_pos, source_face) {
        return true;
    }

    let corner_pos = offset_pos(side_pos, source_face);
    spread_glow_lichen_to_face(world, corner_pos, toward.opposite())
}

fn spread_glow_lichen_to_face<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    face: Direction,
) -> bool {
    let Some(current) = world.block_at_world(pos) else {
        return false;
    };
    if !can_glow_lichen_spread_into(current) {
        return false;
    }
    let Some(next_faces) = glow_lichen_faces_for_placement(world, pos, current, face) else {
        return false;
    };

    let support = offset_pos(pos, face);
    if !world
        .block_at_world(support)
        .is_some_and(material_blocks_motion)
    {
        return false;
    }

    world.set_glow_lichen_faces_world(pos, next_faces)
}

fn glow_lichen_faces_for_placement<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    current: RawBlockId,
    face: Direction,
) -> Option<u8> {
    if current == GLOW_LICHEN {
        let existing = world.glow_lichen_faces_world(pos);
        if existing & face.bit() != 0 {
            return None;
        }
        return Some(existing | face.bit());
    }

    if is_air_or_water(current) {
        Some(face.bit())
    } else {
        None
    }
}

fn can_glow_lichen_spread_into(block_id: RawBlockId) -> bool {
    is_air_like(block_id) || block_id == GLOW_LICHEN || block_id == WATER
}

fn is_air_or_water(block_id: RawBlockId) -> bool {
    is_air_like(block_id) || block_id == WATER
}
