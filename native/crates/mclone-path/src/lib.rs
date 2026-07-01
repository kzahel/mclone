#![forbid(unsafe_code)]

use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use mclone_core::BlockPos;

pub const DEFAULT_PATH_HEURISTIC_WEIGHT: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathSearchLimits {
    pub max_visited_nodes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathSearchQuery {
    pub start: BlockPos,
    pub target: BlockPos,
    pub follow_range: f32,
    pub heuristic_weight: f32,
}

impl PathSearchQuery {
    pub const fn new(start: BlockPos, target: BlockPos) -> Self {
        Self {
            start,
            target,
            follow_range: f32::INFINITY,
            heuristic_weight: DEFAULT_PATH_HEURISTIC_WEIGHT,
        }
    }

    pub const fn with_follow_range(mut self, follow_range: f32) -> Self {
        self.follow_range = follow_range;
        self
    }

    pub const fn with_heuristic_weight(mut self, heuristic_weight: f32) -> Self {
        self.heuristic_weight = heuristic_weight;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathNeighbor {
    pub pos: BlockPos,
    pub cost: f32,
}

impl PathNeighbor {
    pub const fn new(pos: BlockPos, cost: f32) -> Self {
        Self { pos, cost }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathSearchDiagnostics {
    pub visited_nodes: usize,
    pub opened_nodes: usize,
    pub max_visited_nodes: usize,
    pub best_heuristic: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathResult {
    nodes: Vec<BlockPos>,
    target: BlockPos,
    reached: bool,
    diagnostics: PathSearchDiagnostics,
}

impl PathResult {
    pub fn from_nodes(
        nodes: Vec<BlockPos>,
        target: BlockPos,
        reached: bool,
        diagnostics: PathSearchDiagnostics,
    ) -> Self {
        Self {
            nodes,
            target,
            reached,
            diagnostics,
        }
    }

    pub fn nodes(&self) -> &[BlockPos] {
        &self.nodes
    }

    pub fn into_nodes(self) -> Vec<BlockPos> {
        self.nodes
    }

    pub const fn target(&self) -> BlockPos {
        self.target
    }

    pub const fn can_reach(&self) -> bool {
        self.reached
    }

    pub const fn diagnostics(&self) -> PathSearchDiagnostics {
        self.diagnostics
    }
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
    best_node: BlockPos,
    reached: bool,
}

impl TargetRecord {
    fn new(pos: BlockPos, start: BlockPos, start_heuristic: f32) -> Self {
        Self {
            pos,
            best_heuristic: start_heuristic,
            best_node: start,
            reached: false,
        }
    }

    fn update_best(&mut self, heuristic: f32, node: BlockPos) {
        if heuristic < self.best_heuristic {
            self.best_heuristic = heuristic;
            self.best_node = node;
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
pub struct PathSearch {
    limits: PathSearchLimits,
}

impl PathSearch {
    pub const fn new(limits: PathSearchLimits) -> Self {
        Self { limits }
    }

    pub fn find_path<N, A>(
        self,
        query: PathSearchQuery,
        mut neighbors: N,
        accepts_target: A,
    ) -> PathResult
    where
        N: FnMut(BlockPos) -> Vec<PathNeighbor>,
        A: Fn(BlockPos, BlockPos) -> bool,
    {
        let start_h = distance(query.start, query.target);
        let max_visited_nodes = self.limits.max_visited_nodes.max(1);
        let mut diagnostics = PathSearchDiagnostics {
            visited_nodes: 0,
            opened_nodes: 1,
            max_visited_nodes,
            best_heuristic: start_h,
        };
        if accepts_target(query.start, query.target) {
            return PathResult::from_nodes(vec![query.start], query.target, true, diagnostics);
        }

        let mut target = TargetRecord::new(query.target, query.start, start_h);
        let mut open_set = BinaryHeap::new();
        let mut records = HashMap::new();
        let mut sequence = 0_usize;
        records.insert(
            query.start,
            NodeRecord {
                pos: query.start,
                came_from: None,
                g: 0.0,
                f: start_h,
                walked_distance: 0.0,
                closed: false,
            },
        );
        open_set.push(OpenEntry {
            pos: query.start,
            f: start_h,
            h: start_h,
            sequence,
        });

        loop {
            if diagnostics.visited_nodes + 1 >= max_visited_nodes {
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
            diagnostics.visited_nodes += 1;

            let current_h = distance(current.pos, target.pos);
            target.update_best(current_h, current.pos);
            diagnostics.best_heuristic = target.best_heuristic;
            if accepts_target(current.pos, target.pos) {
                target.reached = true;
                target.best_node = current.pos;
                break;
            }

            if distance(query.start, current.pos) >= query.follow_range {
                continue;
            }

            for neighbor in neighbors(current.pos) {
                if records
                    .get(&neighbor.pos)
                    .is_some_and(|record| record.closed)
                {
                    continue;
                }

                let step_distance = distance(current.pos, neighbor.pos);
                let walked_distance = current.walked_distance + step_distance;
                if walked_distance >= query.follow_range {
                    continue;
                }

                let tentative_g = current.g + step_distance + neighbor.cost;
                let should_update = records
                    .get(&neighbor.pos)
                    .is_none_or(|record| tentative_g < record.g);
                if !should_update {
                    continue;
                }

                let heuristic = distance(neighbor.pos, target.pos);
                target.update_best(heuristic, neighbor.pos);
                diagnostics.best_heuristic = target.best_heuristic;
                let weighted_heuristic = heuristic * query.heuristic_weight;
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
                diagnostics.opened_nodes = records.len();
                sequence = sequence.wrapping_add(1);
                open_set.push(OpenEntry {
                    pos: neighbor.pos,
                    f,
                    h: weighted_heuristic,
                    sequence,
                });
            }
        }

        let nodes =
            reconstruct_path(target.best_node, &records).unwrap_or_else(|| vec![query.start]);
        PathResult::from_nodes(nodes, query.target, target.reached, diagnostics)
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

pub fn distance(first: BlockPos, second: BlockPos) -> f32 {
    let dx = (first.x - second.x) as f32;
    let dy = (first.y - second.y) as f32;
    let dz = (first.z - second.z) as f32;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

pub fn manhattan_distance(first: BlockPos, second: BlockPos) -> i32 {
    (first.x - second.x).abs() + (first.y - second.y).abs() + (first.z - second.z).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(max_visited_nodes: usize) -> PathSearch {
        PathSearch::new(PathSearchLimits { max_visited_nodes })
    }

    fn query_to(target: BlockPos) -> PathSearchQuery {
        PathSearchQuery::new(BlockPos::ZERO, target).with_follow_range(16.0)
    }

    fn cardinal_neighbors(pos: BlockPos, blocked: &[BlockPos]) -> Vec<PathNeighbor> {
        [
            pos.offset(1, 0, 0),
            pos.offset(0, 0, -1),
            pos.offset(0, 0, 1),
            pos.offset(-1, 0, 0),
        ]
        .into_iter()
        .filter(|candidate| {
            (-2..=4).contains(&candidate.x)
                && (-2..=2).contains(&candidate.z)
                && !blocked.contains(candidate)
        })
        .map(|candidate| PathNeighbor::new(candidate, 0.0))
        .collect()
    }

    #[test]
    fn search_returns_direct_path() {
        let result = search(128).find_path(
            query_to(BlockPos::new(3, 0, 0)),
            |pos| cardinal_neighbors(pos, &[]),
            |pos, target| pos == target,
        );

        assert!(result.can_reach());
        assert_eq!(result.nodes().first(), Some(&BlockPos::ZERO));
        assert_eq!(result.nodes().last(), Some(&BlockPos::new(3, 0, 0)));
    }

    #[test]
    fn search_routes_around_obstacle() {
        let blocked = [BlockPos::new(1, 0, 0)];
        let result = search(128).find_path(
            query_to(BlockPos::new(2, 0, 0)),
            |pos| cardinal_neighbors(pos, &blocked),
            |pos, target| pos == target,
        );

        assert!(result.can_reach());
        assert_eq!(result.nodes().first(), Some(&BlockPos::ZERO));
        assert_eq!(result.nodes().last(), Some(&BlockPos::new(2, 0, 0)));
        assert!(!result.nodes().contains(&BlockPos::new(1, 0, 0)));
        assert!(result.nodes().len() > 3);
    }

    #[test]
    fn search_returns_best_partial_path_when_budget_exhausts() {
        let result = search(1).find_path(
            query_to(BlockPos::new(8, 0, 0)),
            |pos| cardinal_neighbors(pos, &[]),
            |pos, target| pos == target,
        );

        assert!(!result.can_reach());
        assert_eq!(result.nodes(), &[BlockPos::ZERO]);
        assert_eq!(result.diagnostics().max_visited_nodes, 1);
    }

    #[test]
    fn search_returns_start_when_frontier_has_no_neighbors() {
        let result = search(128).find_path(
            query_to(BlockPos::new(5, 0, 0)),
            |_| Vec::new(),
            |pos, target| pos == target,
        );

        assert!(!result.can_reach());
        assert_eq!(result.nodes(), &[BlockPos::ZERO]);
    }

    #[test]
    fn search_uses_deterministic_tie_breaking() {
        let blocked = [BlockPos::new(1, 0, 0)];
        let result = search(128).find_path(
            query_to(BlockPos::new(2, 0, 0)),
            |pos| cardinal_neighbors(pos, &blocked),
            |pos, target| pos == target,
        );

        assert!(result.can_reach());
        assert_eq!(result.nodes().get(1), Some(&BlockPos::new(0, 0, -1)));
    }

    #[test]
    fn search_accepts_target_callback() {
        let result = search(128).find_path(
            query_to(BlockPos::new(3, 0, 0)),
            |pos| cardinal_neighbors(pos, &[]),
            |pos, target| manhattan_distance(pos, target) <= 1,
        );

        assert!(result.can_reach());
        assert_eq!(result.nodes().last(), Some(&BlockPos::new(2, 0, 0)));
    }
}
