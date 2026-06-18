use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const NO_COMPUTED_LEVEL: u8 = 255;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeighborCheck {
    pub source: i64,
    pub target: i64,
    pub level: u8,
    pub is_decrease: bool,
}

pub trait DynamicGraphCallbacks {
    fn is_source(&self, node: i64) -> bool;
    fn get_computed_level(&self, target: i64, excluded_source: i64, candidate: u8) -> u8;
    fn neighbor_checks_after_update(
        &mut self,
        node: i64,
        level: u8,
        is_decrease: bool,
    ) -> Vec<NeighborCheck>;
    fn get_level(&self, node: i64) -> u8;
    fn set_level(&mut self, node: i64, level: u8);
    fn compute_level_from_neighbor(&self, source: i64, target: i64, source_level: u8) -> u8;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicGraphMinFixedPoint {
    level_count: u8,
    queues: Vec<LevelQueue>,
    computed_levels: BTreeMap<i64, u8>,
    first_queued_level: u8,
    has_work: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LevelQueue {
    order: VecDeque<i64>,
    members: BTreeSet<i64>,
}

impl LevelQueue {
    fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    fn insert(&mut self, node: i64) {
        if self.members.insert(node) {
            self.order.push_back(node);
        }
    }

    fn remove(&mut self, node: i64) {
        self.members.remove(&node);
    }

    fn pop_first(&mut self) -> Option<i64> {
        while let Some(node) = self.order.pop_front() {
            if self.members.remove(&node) {
                return Some(node);
            }
        }
        None
    }
}

impl DynamicGraphMinFixedPoint {
    pub fn new(level_count: u8) -> Self {
        assert!(
            level_count < 254,
            "DynamicGraphMinFixedPoint level count must be < 254"
        );
        Self {
            level_count,
            queues: (0..level_count).map(|_| LevelQueue::default()).collect(),
            computed_levels: BTreeMap::new(),
            first_queued_level: level_count,
            has_work: false,
        }
    }

    pub fn check_node(&mut self, callbacks: &mut impl DynamicGraphCallbacks, node: i64) {
        self.check_edge(
            callbacks,
            node,
            node,
            self.level_count.saturating_sub(1),
            false,
        );
    }

    pub fn check_edge(
        &mut self,
        callbacks: &mut impl DynamicGraphCallbacks,
        source: i64,
        target: i64,
        candidate: u8,
        is_decrease: bool,
    ) {
        let target_level = callbacks.get_level(target);
        let computed_level = self.computed_level(target);
        self.check_edge_with_levels(
            callbacks,
            source,
            target,
            candidate,
            target_level,
            computed_level,
            is_decrease,
        );
        self.has_work = self.first_queued_level < self.level_count;
    }

    pub fn check_neighbor(
        &mut self,
        callbacks: &mut impl DynamicGraphCallbacks,
        source: i64,
        target: i64,
        source_level: u8,
        is_decrease: bool,
    ) {
        let computed_level = self.computed_level(target);
        let neighbor_level = clamp_level(
            callbacks.compute_level_from_neighbor(source, target, source_level),
            self.level_count,
        );
        if is_decrease {
            self.check_edge_with_levels(
                callbacks,
                source,
                target,
                neighbor_level,
                callbacks.get_level(target),
                computed_level,
                true,
            );
        } else {
            let (target_level, had_no_computed_level) = if computed_level == NO_COMPUTED_LEVEL {
                (
                    clamp_level(callbacks.get_level(target), self.level_count),
                    true,
                )
            } else {
                (computed_level, false)
            };

            if neighbor_level == target_level {
                self.check_edge_with_levels(
                    callbacks,
                    source,
                    target,
                    self.level_count.saturating_sub(1),
                    if had_no_computed_level {
                        target_level
                    } else {
                        callbacks.get_level(target)
                    },
                    computed_level,
                    false,
                );
            }
        }
        self.has_work = self.first_queued_level < self.level_count;
    }

    pub fn remove_from_queue(&mut self, callbacks: &impl DynamicGraphCallbacks, node: i64) {
        let computed_level = self.computed_level(node);
        if computed_level != NO_COMPUTED_LEVEL {
            let current_level = callbacks.get_level(node);
            let queue_key = self.key(current_level, computed_level);
            self.dequeue(node, queue_key, self.level_count, true);
            self.has_work = self.first_queued_level < self.level_count;
        }
    }

    pub fn remove_if(
        &mut self,
        callbacks: &impl DynamicGraphCallbacks,
        mut predicate: impl FnMut(i64) -> bool,
    ) {
        let nodes = self
            .computed_levels
            .keys()
            .copied()
            .filter(|node| predicate(*node))
            .collect::<Vec<_>>();
        for node in nodes {
            self.remove_from_queue(callbacks, node);
        }
    }

    pub fn run_updates(
        &mut self,
        callbacks: &mut impl DynamicGraphCallbacks,
        mut budget: usize,
    ) -> usize {
        if self.first_queued_level >= self.level_count {
            return budget;
        }

        while self.first_queued_level < self.level_count && budget > 0 {
            budget -= 1;
            let queue_index = self.first_queued_level as usize;
            let Some(node) = self.queues[queue_index].pop_first() else {
                self.check_first_queued_level(self.level_count);
                continue;
            };
            let current_level = clamp_level(callbacks.get_level(node), self.level_count);
            if self.queues[queue_index].is_empty() {
                self.check_first_queued_level(self.level_count);
            }

            let computed_level = self
                .computed_levels
                .remove(&node)
                .unwrap_or(NO_COMPUTED_LEVEL);
            if computed_level < current_level {
                callbacks.set_level(node, computed_level);
                for check in callbacks.neighbor_checks_after_update(node, computed_level, true) {
                    self.check_neighbor(
                        callbacks,
                        check.source,
                        check.target,
                        check.level,
                        check.is_decrease,
                    );
                }
            } else if computed_level > current_level {
                self.enqueue(
                    node,
                    computed_level,
                    self.key(self.level_count.saturating_sub(1), computed_level),
                );
                callbacks.set_level(node, self.level_count.saturating_sub(1));
                for check in callbacks.neighbor_checks_after_update(node, current_level, false) {
                    self.check_neighbor(
                        callbacks,
                        check.source,
                        check.target,
                        check.level,
                        check.is_decrease,
                    );
                }
            }
        }

        self.has_work = self.first_queued_level < self.level_count;
        budget
    }

    pub fn has_work(&self) -> bool {
        self.has_work
    }

    pub fn queue_size(&self) -> usize {
        self.computed_levels.len()
    }

    fn check_edge_with_levels(
        &mut self,
        callbacks: &mut impl DynamicGraphCallbacks,
        source: i64,
        target: i64,
        candidate: u8,
        mut current_level: u8,
        mut computed_level: u8,
        is_decrease: bool,
    ) {
        if callbacks.is_source(target) {
            return;
        }

        let candidate = clamp_level(candidate, self.level_count);
        current_level = clamp_level(current_level, self.level_count);
        let was_not_queued = if computed_level == NO_COMPUTED_LEVEL {
            computed_level = current_level;
            true
        } else {
            false
        };

        let target_level = if is_decrease {
            computed_level.min(candidate)
        } else {
            clamp_level(
                callbacks.get_computed_level(target, source, candidate),
                self.level_count,
            )
        };

        let old_queue_key = self.key(current_level, computed_level);
        if current_level != target_level {
            let new_queue_key = self.key(current_level, target_level);
            if old_queue_key != new_queue_key && !was_not_queued {
                self.dequeue(target, old_queue_key, new_queue_key, false);
            }
            self.enqueue(target, target_level, new_queue_key);
        } else if !was_not_queued {
            self.dequeue(target, old_queue_key, self.level_count, true);
        }
    }

    fn computed_level(&self, node: i64) -> u8 {
        self.computed_levels
            .get(&node)
            .copied()
            .unwrap_or(NO_COMPUTED_LEVEL)
    }

    fn key(&self, level: u8, computed_level: u8) -> u8 {
        level
            .min(computed_level)
            .min(self.level_count.saturating_sub(1))
    }

    fn check_first_queued_level(&mut self, limit: u8) {
        let old_first = self.first_queued_level;
        self.first_queued_level = limit;
        for level in old_first.saturating_add(1)..limit {
            if !self.queues[level as usize].is_empty() {
                self.first_queued_level = level;
                break;
            }
        }
    }

    fn dequeue(&mut self, node: i64, queue_key: u8, next_level: u8, remove_computed_level: bool) {
        if remove_computed_level {
            self.computed_levels.remove(&node);
        }
        self.queues[queue_key as usize].remove(node);
        if self.queues[queue_key as usize].is_empty() && self.first_queued_level == queue_key {
            self.check_first_queued_level(next_level);
        }
    }

    fn enqueue(&mut self, node: i64, computed_level: u8, queue_key: u8) {
        self.computed_levels.insert(node, computed_level);
        self.queues[queue_key as usize].insert(node);
        if self.first_queued_level > queue_key {
            self.first_queued_level = queue_key;
        }
    }
}

fn clamp_level(level: u8, level_count: u8) -> u8 {
    level.min(level_count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct LineGraph {
        levels: BTreeMap<i64, u8>,
        blocked: BTreeSet<i64>,
        sources: BTreeSet<i64>,
    }

    impl LineGraph {
        fn level(&self, node: i64) -> u8 {
            *self.levels.get(&node).unwrap_or(&15)
        }
    }

    impl DynamicGraphCallbacks for LineGraph {
        fn is_source(&self, node: i64) -> bool {
            node == i64::MAX
        }

        fn get_computed_level(&self, target: i64, excluded_source: i64, candidate: u8) -> u8 {
            let mut level = candidate;
            if excluded_source != i64::MAX {
                level = level.min(self.compute_level_from_neighbor(i64::MAX, target, 0));
                if level == 0 {
                    return level;
                }
            }
            for source in [target - 1, target + 1] {
                if source != excluded_source {
                    level = level.min(self.compute_level_from_neighbor(
                        source,
                        target,
                        self.level(source),
                    ));
                }
            }
            level
        }

        fn neighbor_checks_after_update(
            &mut self,
            node: i64,
            level: u8,
            is_decrease: bool,
        ) -> Vec<NeighborCheck> {
            [node - 1, node + 1]
                .into_iter()
                .map(|target| NeighborCheck {
                    source: node,
                    target,
                    level,
                    is_decrease,
                })
                .collect()
        }

        fn get_level(&self, node: i64) -> u8 {
            if node == i64::MAX {
                0
            } else {
                self.level(node)
            }
        }

        fn set_level(&mut self, node: i64, level: u8) {
            self.levels.insert(node, level);
        }

        fn compute_level_from_neighbor(&self, source: i64, target: i64, source_level: u8) -> u8 {
            if target == i64::MAX {
                15
            } else if source == i64::MAX {
                if self.sources.contains(&target) {
                    0
                } else {
                    15
                }
            } else if self.blocked.contains(&target) {
                15
            } else {
                source_level.saturating_add(1).min(15)
            }
        }
    }

    #[test]
    fn graph_brightens_from_source_by_level_order() {
        let mut graph = DynamicGraphMinFixedPoint::new(16);
        let mut line = LineGraph::default();
        line.sources.insert(0);

        graph.check_edge(&mut line, i64::MAX, 0, 0, true);
        graph.run_updates(&mut line, 64);

        assert_eq!(line.get_level(0), 0);
        assert_eq!(line.get_level(1), 1);
        assert_eq!(line.get_level(2), 2);
        assert_eq!(line.get_level(10), 10);
        assert!(!graph.has_work());
    }

    #[test]
    fn graph_darkens_and_repairs_from_alternate_source() {
        let mut graph = DynamicGraphMinFixedPoint::new(16);
        let mut line = LineGraph::default();
        line.sources.insert(0);
        line.sources.insert(4);

        graph.check_edge(&mut line, i64::MAX, 0, 0, true);
        graph.check_edge(&mut line, i64::MAX, 4, 0, true);
        graph.run_updates(&mut line, 64);
        line.sources.remove(&0);
        graph.check_node(&mut line, 0);
        graph.run_updates(&mut line, 128);

        assert_eq!(line.get_level(0), 4);
        assert_eq!(line.get_level(1), 3);
        assert_eq!(line.get_level(2), 2);
        assert_eq!(line.get_level(3), 1);
        assert_eq!(line.get_level(4), 0);
    }

    #[test]
    fn remove_if_drops_pending_work() {
        let mut graph = DynamicGraphMinFixedPoint::new(16);
        let mut line = LineGraph::default();

        graph.check_edge(&mut line, i64::MAX, 0, 0, true);
        assert_eq!(graph.queue_size(), 1);
        graph.remove_if(&line, |node| node == 0);

        assert_eq!(graph.queue_size(), 0);
        assert!(!graph.has_work());
    }
}
