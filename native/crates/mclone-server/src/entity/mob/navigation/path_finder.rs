use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use mclone_core::{BlockPos, BlockStateId};

use super::{BlockPathType, WalkNodeEvaluator, path::GroundPath, path_service::PathRequest};

const PATH_HEURISTIC_FUDGE: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PathSearchLimits {
    pub(super) max_visited_nodes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NodeRecord {
    pos: BlockPos,
    came_from: Option<BlockPos>,
    g: f32,
    f: f32,
    walked_distance: f32,
    closed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TargetRecord {
    pos: BlockPos,
    best_heuristic: f32,
    best_node: Option<BlockPos>,
    reached: bool,
}

impl TargetRecord {
    fn new(pos: BlockPos) -> Self {
        Self {
            pos,
            best_heuristic: f32::INFINITY,
            best_node: None,
            reached: false,
        }
    }

    fn update_best(&mut self, heuristic: f32, node: BlockPos) {
        if heuristic < self.best_heuristic {
            self.best_heuristic = heuristic;
            self.best_node = Some(node);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct OpenEntry {
    pos: BlockPos,
    f: f32,
    h: f32,
    sequence: usize,
}

impl Eq for OpenEntry {}

impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.h.total_cmp(&self.h))
            .then_with(|| other.sequence.cmp(&self.sequence))
            .then_with(|| other.pos.cmp(&self.pos))
    }
}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PathFinder {
    limits: PathSearchLimits,
}

impl PathFinder {
    pub(super) const fn new(limits: PathSearchLimits) -> Self {
        Self { limits }
    }

    pub(super) fn find_path<F, M>(
        self,
        request: PathRequest,
        block_state_at: &F,
        pathfinding_malus: M,
    ) -> Option<GroundPath>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let evaluator = WalkNodeEvaluator::new(request.mob_width, request.mob_height);
        let start =
            evaluator.get_start(request.start_position, block_state_at, pathfinding_malus)?;
        let mut target = TargetRecord::new(request.target_position);
        let start_h = distance(start, target.pos);
        target.update_best(start_h, start);
        if manhattan_distance(start, target.pos) <= request.reach_range {
            return Some(GroundPath::from_nodes(
                vec![start],
                request.target_position,
                true,
            ));
        }

        let mut open_set = BinaryHeap::new();
        let mut records = HashMap::new();
        let mut sequence = 0_usize;
        records.insert(
            start,
            NodeRecord {
                pos: start,
                came_from: None,
                g: 0.0,
                f: start_h,
                walked_distance: 0.0,
                closed: false,
            },
        );
        open_set.push(OpenEntry {
            pos: start,
            f: start_h,
            h: start_h,
            sequence,
        });

        let max_visited_nodes = ((self.limits.max_visited_nodes as f32
            * request.max_visited_nodes_multiplier)
            .floor()
            .max(1.0)) as usize;
        let mut visited_nodes = 0_usize;

        loop {
            if visited_nodes + 1 >= max_visited_nodes {
                break;
            }
            let Some(entry) = open_set.pop() else {
                break;
            };
            let Some(open_record) = records.get(&entry.pos) else {
                continue;
            };
            if open_record.closed || entry.f > open_record.f {
                continue;
            }

            let current = *open_record;
            if let Some(record) = records.get_mut(&entry.pos) {
                record.closed = true;
            }
            visited_nodes += 1;

            let current_h = distance(current.pos, target.pos);
            target.update_best(current_h, current.pos);
            if manhattan_distance(current.pos, target.pos) <= request.reach_range {
                target.reached = true;
                target.best_node = Some(current.pos);
                break;
            }

            if distance(start, current.pos) >= request.follow_range {
                continue;
            }

            for neighbor in evaluator.get_neighbors(current.pos, block_state_at, pathfinding_malus)
            {
                if records
                    .get(&neighbor.pos)
                    .is_some_and(|record| record.closed)
                {
                    continue;
                }

                let step_distance = distance(current.pos, neighbor.pos);
                let walked_distance = current.walked_distance + step_distance;
                if walked_distance >= request.follow_range {
                    continue;
                }

                let tentative_g = current.g + step_distance + neighbor.cost_malus;
                let should_update = records
                    .get(&neighbor.pos)
                    .is_none_or(|record| tentative_g < record.g);
                if !should_update {
                    continue;
                }

                let heuristic = distance(neighbor.pos, target.pos);
                target.update_best(heuristic, neighbor.pos);
                let weighted_heuristic = heuristic * PATH_HEURISTIC_FUDGE;
                let f = tentative_g + weighted_heuristic;
                records.insert(
                    neighbor.pos,
                    NodeRecord {
                        pos: neighbor.pos,
                        came_from: Some(current.pos),
                        g: tentative_g,
                        f,
                        walked_distance,
                        closed: false,
                    },
                );
                sequence = sequence.wrapping_add(1);
                open_set.push(OpenEntry {
                    pos: neighbor.pos,
                    f,
                    h: weighted_heuristic,
                    sequence,
                });
            }
        }

        let best_node = target.best_node?;
        let nodes = reconstruct_path(best_node, &records)?;
        Some(GroundPath::from_nodes(
            nodes,
            request.target_position,
            target.reached,
        ))
    }
}

fn reconstruct_path(
    mut pos: BlockPos,
    records: &HashMap<BlockPos, NodeRecord>,
) -> Option<Vec<BlockPos>> {
    let mut nodes = vec![pos];
    while let Some(previous) = records.get(&pos)?.came_from {
        pos = previous;
        nodes.push(pos);
    }
    nodes.reverse();
    Some(nodes)
}

fn distance(first: BlockPos, second: BlockPos) -> f32 {
    let dx = (first.x - second.x) as f32;
    let dy = (first.y - second.y) as f32;
    let dz = (first.z - second.z) as f32;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn manhattan_distance(first: BlockPos, second: BlockPos) -> i32 {
    (first.x - second.x).abs() + (first.y - second.y).abs() + (first.z - second.z).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Vec3d;

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn obstacle_ground(pos: BlockPos) -> Option<BlockStateId> {
        if pos.y == 63 || pos == BlockPos::new(1, 64, 0) {
            Some(BlockStateId(1))
        } else {
            Some(BlockStateId(mclone_blocks::terrain_id::AIR))
        }
    }

    fn request_to(target_position: BlockPos, max_visited_nodes_multiplier: f32) -> PathRequest {
        PathRequest {
            start_position: Vec3d::new(0.5, 64.0, 0.5),
            target_position,
            mob_width: 0.9,
            mob_height: 1.4,
            follow_range: 16.0,
            reach_range: 0,
            max_visited_nodes_multiplier,
        }
    }

    #[test]
    fn path_finder_returns_direct_flat_path() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 128,
        })
        .find_path(
            request_to(BlockPos::new(3, 64, 0), 1.0),
            &flat_ground,
            BlockPathType::default_malus,
        )
        .expect("flat ground should produce a path");

        assert!(path.can_reach());
        assert_eq!(path.nodes().first(), Some(&BlockPos::new(0, 64, 0)));
        assert_eq!(path.nodes().last(), Some(&BlockPos::new(3, 64, 0)));
        assert!(path.node_count() >= 2);
    }

    #[test]
    fn path_finder_routes_around_blocking_column() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 128,
        })
        .find_path(
            request_to(BlockPos::new(2, 64, 0), 1.0),
            &obstacle_ground,
            BlockPathType::default_malus,
        )
        .expect("flat ground with one obstacle should produce a path");

        assert!(path.can_reach());
        assert_eq!(path.nodes().first(), Some(&BlockPos::new(0, 64, 0)));
        assert_eq!(path.nodes().last(), Some(&BlockPos::new(2, 64, 0)));
        assert!(!path.nodes().contains(&BlockPos::new(1, 64, 0)));
        assert!(path.node_count() > 3);
    }

    #[test]
    fn path_finder_returns_best_partial_path_when_budget_exhausts() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 1,
        })
        .find_path(
            request_to(BlockPos::new(8, 64, 0), 1.0),
            &flat_ground,
            BlockPathType::default_malus,
        )
        .expect("bounded search should still return the best partial path");

        assert!(!path.can_reach());
        assert_eq!(path.nodes(), &[BlockPos::new(0, 64, 0)]);
    }
}
