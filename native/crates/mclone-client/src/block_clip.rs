use mclone_core::{Aabb, BlockHitResult, BlockPos, Direction, Vec3d};

pub(crate) fn clip_aabb(
    from: Vec3d,
    to: Vec3d,
    pos: BlockPos,
    bounds: Aabb,
) -> Option<BlockHitResult> {
    let delta = to.subtract(from);
    if delta.length_sqr() < 1.0e-7 {
        return None;
    }
    if contains_aabb(bounds, from) {
        return Some(BlockHitResult::new(
            from.add(delta.scale(0.001)),
            Direction::nearest(delta.x, delta.y, delta.z).opposite(),
            pos,
            true,
        ));
    }

    let mut t_min = 0.0;
    let mut t_max = 1.0;
    let mut face = None;

    if !clip_axis(
        from.x,
        delta.x,
        bounds.min_x,
        bounds.max_x,
        Direction::West,
        Direction::East,
        &mut t_min,
        &mut t_max,
        &mut face,
    ) || !clip_axis(
        from.y,
        delta.y,
        bounds.min_y,
        bounds.max_y,
        Direction::Down,
        Direction::Up,
        &mut t_min,
        &mut t_max,
        &mut face,
    ) || !clip_axis(
        from.z,
        delta.z,
        bounds.min_z,
        bounds.max_z,
        Direction::North,
        Direction::South,
        &mut t_min,
        &mut t_max,
        &mut face,
    ) {
        return None;
    }

    face.map(|direction| BlockHitResult::new(from.add(delta.scale(t_min)), direction, pos, false))
}

#[allow(clippy::too_many_arguments)]
fn clip_axis(
    origin: f64,
    delta: f64,
    min: f64,
    max: f64,
    low_face: Direction,
    high_face: Direction,
    t_min: &mut f64,
    t_max: &mut f64,
    face: &mut Option<Direction>,
) -> bool {
    const EPSILON: f64 = 1.0e-7;
    if delta.abs() < EPSILON {
        return origin >= min && origin <= max;
    }

    let inv = 1.0 / delta;
    let mut near = (min - origin) * inv;
    let mut far = (max - origin) * inv;
    let mut near_face = low_face;
    if near > far {
        std::mem::swap(&mut near, &mut far);
        near_face = high_face;
    }
    if near > *t_min {
        *t_min = near;
        *face = Some(near_face);
    }
    *t_max = t_max.min(far);
    *t_min <= *t_max && *t_max >= 0.0 && *t_min <= 1.0
}

fn contains_aabb(bounds: Aabb, point: Vec3d) -> bool {
    point.x >= bounds.min_x
        && point.x < bounds.max_x
        && point.y >= bounds.min_y
        && point.y < bounds.max_y
        && point.z >= bounds.min_z
        && point.z < bounds.max_z
}
