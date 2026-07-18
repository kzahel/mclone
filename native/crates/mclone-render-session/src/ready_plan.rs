use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSectionNeighborReadiness {
    ReadyWithNeighbors,
    ReadyNearCamera,
    DeferredMissingNeighbors,
}

impl RenderSectionNeighborReadiness {
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::ReadyWithNeighbors | Self::ReadyNearCamera)
    }

    pub const fn is_near_exception(self) -> bool {
        matches!(self, Self::ReadyNearCamera)
    }
}

/// Horizontal distance below which a render section is treated as ready even
/// without fully loaded neighbors.
const RENDER_NEIGHBOR_READY_DISTANCE: f32 = 24.0;

/// Horizontal distance (squared) below which a render section is treated as ready even
/// without fully loaded neighbors. Native keeps an exact client snapshot radius (unlike
/// Java's overfetched client chunk cache), so this near-camera exception keeps the
/// boundary ring meshing while flying above an otherwise loaded column.
const RENDER_NEIGHBOR_READY_DISTANCE_SQ: f32 =
    RENDER_NEIGHBOR_READY_DISTANCE * RENDER_NEIGHBOR_READY_DISTANCE;

/// Shared camera-distance neighbor readiness used by both the desktop and web streaming
/// loops (067 Stage 3). Sections near the camera are always ready; farther sections wait
/// until their four horizontal neighbors have snapshots so culling/AO/light is correct.
pub fn render_section_neighbor_readiness(
    client: &ClientRuntime,
    key: RenderSectionKey,
    camera_position: Vec3,
) -> RenderSectionNeighborReadiness {
    if render_section_horizontal_distance_sq_in(client.topology(), key, camera_position)
        <= RENDER_NEIGHBOR_READY_DISTANCE_SQ
    {
        return RenderSectionNeighborReadiness::ReadyNearCamera;
    }
    if has_horizontal_neighbor_snapshots(client, ChunkPos::new(key.chunk_x, key.chunk_z)) {
        RenderSectionNeighborReadiness::ReadyWithNeighbors
    } else {
        RenderSectionNeighborReadiness::DeferredMissingNeighbors
    }
}

/// Compact key for the part of neighbor readiness that depends on the camera.
/// Readiness only cares whether a render-section column is inside the fixed
/// near-camera exception radius, so callers can compare this small set instead
/// of invalidating traversal-ready state for every sub-block camera movement.
pub fn render_section_near_camera_readiness_columns(
    topology: HorizontalTopology,
    camera_position: Vec3,
) -> BTreeSet<ChunkPos> {
    if !camera_position.x.is_finite() || !camera_position.z.is_finite() {
        return BTreeSet::new();
    }
    let min_chunk_x =
        render_section_center_chunk_coord_floor(camera_position.x - RENDER_NEIGHBOR_READY_DISTANCE)
            - 1;
    let max_chunk_x =
        render_section_center_chunk_coord_floor(camera_position.x + RENDER_NEIGHBOR_READY_DISTANCE)
            + 1;
    let min_chunk_z =
        render_section_center_chunk_coord_floor(camera_position.z - RENDER_NEIGHBOR_READY_DISTANCE)
            - 1;
    let max_chunk_z =
        render_section_center_chunk_coord_floor(camera_position.z + RENDER_NEIGHBOR_READY_DISTANCE)
            + 1;
    let mut columns = BTreeSet::new();
    for chunk_x in min_chunk_x..=max_chunk_x {
        let center_x = chunk_middle_block_coord(chunk_x) as f32;
        for chunk_z in min_chunk_z..=max_chunk_z {
            let center_z = chunk_middle_block_coord(chunk_z) as f32;
            let dx = center_x - camera_position.x;
            let dz = center_z - camera_position.z;
            if dx * dx + dz * dz <= RENDER_NEIGHBOR_READY_DISTANCE_SQ {
                if let Some(canonical) =
                    topology.canonicalize_chunk(ChunkPos::new(chunk_x, chunk_z))
                {
                    columns.insert(canonical);
                }
            }
        }
    }
    columns
}

fn render_section_center_chunk_coord_floor(center_coord: f32) -> i32 {
    ((center_coord - CHUNK_WIDTH as f32 * 0.5) / CHUNK_WIDTH as f32).floor() as i32
}

/// World-space center of a render section, used for distance ordering/readiness.
pub fn render_section_center(key: RenderSectionKey) -> Vec3 {
    Vec3::new(
        chunk_middle_block_coord(key.chunk_x) as f32,
        (key.section_y * SECTION_HEIGHT) as f32 + SECTION_HEIGHT as f32 * 0.5,
        chunk_middle_block_coord(key.chunk_z) as f32,
    )
}

fn render_section_distance_sq_in(
    topology: HorizontalTopology,
    key: RenderSectionKey,
    camera_position: Vec3,
) -> f32 {
    let horizontal_distance =
        render_section_horizontal_distance_sq_in(topology, key, camera_position);
    let dy = render_section_center(key).y - camera_position.y;
    horizontal_distance + dy * dy
}

fn render_section_horizontal_distance_sq_in(
    topology: HorizontalTopology,
    key: RenderSectionKey,
    camera_position: Vec3,
) -> f32 {
    let lifted = topology.nearest_chunk_lift(
        ChunkPos::new(key.chunk_x, key.chunk_z),
        Vec3d::new(
            f64::from(camera_position.x),
            f64::from(camera_position.y),
            f64::from(camera_position.z),
        ),
    );
    let center = Vec3::new(
        lifted.x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5,
        render_section_center(key).y,
        lifted.z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5,
    );
    let dx = center.x - camera_position.x;
    let dz = center.z - camera_position.z;
    dx * dx + dz * dz
}

fn render_chunk_distance_sq_in(
    topology: HorizontalTopology,
    pos: ChunkPos,
    camera_position: Vec3,
) -> f32 {
    let lifted = topology.nearest_chunk_lift(
        pos,
        Vec3d::new(
            f64::from(camera_position.x),
            f64::from(camera_position.y),
            f64::from(camera_position.z),
        ),
    );
    let center = Vec3::new(
        chunk_middle_block_coord(lifted.x as i32) as f32,
        camera_position.y,
        chunk_middle_block_coord(lifted.z as i32) as f32,
    );
    center.distance_squared(camera_position)
}

fn has_horizontal_neighbor_snapshots(client: &ClientRuntime, pos: ChunkPos) -> bool {
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .all(|(dx, dz)| {
            client
                .topology()
                .neighbor_chunk(pos, dx, dz)
                .is_some_and(|neighbor| client.chunk_snapshot(neighbor).is_some())
        })
}

/// Distance-sort loaded dirty chunk positions nearest-first (camera-relative), with a
/// stable coordinate tiebreak. Shared by the desktop and web streaming loops so both
/// platforms compile the nearest pending chunk first.
pub fn sort_chunk_positions_by_distance(
    positions: impl IntoIterator<Item = ChunkPos>,
    topology: HorizontalTopology,
    camera_position: Vec3,
) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by(|left, right| {
        render_chunk_distance_sq_in(topology, *left, camera_position)
            .total_cmp(&render_chunk_distance_sq_in(
                topology,
                *right,
                camera_position,
            ))
            .then_with(|| left.x.cmp(&right.x))
            .then_with(|| left.z.cmp(&right.z))
    });
    positions
}

/// Distance-sort dirty-section chunks by their nearest dirty section, nearest-first.
pub fn sort_dirty_section_chunks_by_distance(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    topology: HorizontalTopology,
    camera_position: Vec3,
) -> Vec<ChunkPos> {
    let mut positions = sections_by_chunk.keys().copied().collect::<Vec<_>>();
    positions.sort_by(|left, right| {
        dirty_section_chunk_distance_sq_in(sections_by_chunk, topology, *left, camera_position)
            .total_cmp(&dirty_section_chunk_distance_sq_in(
                sections_by_chunk,
                topology,
                *right,
                camera_position,
            ))
            .then_with(|| left.x.cmp(&right.x))
            .then_with(|| left.z.cmp(&right.z))
    });
    positions
}

fn dirty_section_chunk_distance_sq_in(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    topology: HorizontalTopology,
    pos: ChunkPos,
    camera_position: Vec3,
) -> f32 {
    sections_by_chunk
        .get(&pos)
        .and_then(|keys| {
            keys.iter()
                .map(|key| render_section_distance_sq_in(topology, *key, camera_position))
                .min_by(f32::total_cmp)
        })
        .unwrap_or_else(|| render_chunk_distance_sq_in(topology, pos, camera_position))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionReadyPlan {
    pub budgeted_loaded_chunks: BTreeSet<ChunkPos>,
    pub budgeted_dirty_section_chunks: BTreeSet<ChunkPos>,
    pub ready_section_keys: BTreeSet<RenderSectionKey>,
    pub deferred_section_keys: BTreeSet<RenderSectionKey>,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
}

pub fn plan_ready_render_sections(
    sorted_loaded_dirty_chunks: impl IntoIterator<Item = ChunkPos>,
    sorted_dirty_section_chunks: impl IntoIterator<Item = ChunkPos>,
    loaded_dirty_sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    chunk_budget: usize,
    mut section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
    inflight_sections: &BTreeSet<RenderSectionKey>,
    mut section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
) -> RenderSectionReadyPlan {
    let mut plan = RenderSectionReadyPlan::default();
    if chunk_budget == 0 {
        return plan;
    }

    let mut budgeted_chunk_count = 0usize;
    for pos in sorted_loaded_dirty_chunks {
        let section_keys = section_keys_for_chunk(pos);
        if section_keys.is_empty() {
            // An all-air/empty snapshot has no compile request, but it is still
            // completed dirty work. Retaining the chunk forever makes idle
            // accounting depend on whether terrain happened to emit geometry.
            plan.budgeted_loaded_chunks.insert(pos);
            budgeted_chunk_count += 1;
            if budgeted_chunk_count >= chunk_budget {
                break;
            }
            continue;
        }
        let ready_before = plan.ready_section_keys.len();
        for key in section_keys {
            plan_ready_render_section_key(
                &mut plan,
                key,
                inflight_sections,
                &mut section_readiness,
            );
        }
        if plan.ready_section_keys.len() > ready_before {
            plan.budgeted_loaded_chunks.insert(pos);
            budgeted_chunk_count += 1;
            if budgeted_chunk_count >= chunk_budget {
                break;
            }
        }
    }

    if budgeted_chunk_count >= chunk_budget {
        return plan;
    }

    for pos in sorted_dirty_section_chunks {
        if plan.budgeted_loaded_chunks.contains(&pos) {
            continue;
        }
        if let Some(keys) = loaded_dirty_sections_by_chunk.get(&pos) {
            let ready_before = plan.ready_section_keys.len();
            for key in keys {
                plan_ready_render_section_key(
                    &mut plan,
                    *key,
                    inflight_sections,
                    &mut section_readiness,
                );
            }
            if plan.ready_section_keys.len() > ready_before {
                plan.budgeted_dirty_section_chunks.insert(pos);
                budgeted_chunk_count += 1;
                if budgeted_chunk_count >= chunk_budget {
                    break;
                }
            }
        }
    }

    plan
}

fn plan_ready_render_section_key(
    plan: &mut RenderSectionReadyPlan,
    key: RenderSectionKey,
    inflight_sections: &BTreeSet<RenderSectionKey>,
    section_readiness: &mut impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
) {
    if inflight_sections.contains(&key) {
        plan.deferred_section_keys.insert(key);
        return;
    }

    let readiness = section_readiness(key);
    if readiness.is_ready() {
        if readiness.is_near_exception() {
            plan.near_exception_section_count += 1;
        }
        plan.ready_section_keys.insert(key);
    } else {
        plan.deferred_section_count += 1;
        plan.deferred_section_keys.insert(key);
    }
}
