use mclone_core::{BlockPos, Vec3d};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct GroundPath {
    nodes: Vec<BlockPos>,
    next_node_index: usize,
    target: BlockPos,
    reached: bool,
}

impl GroundPath {
    pub(super) fn from_single_target(target: BlockPos) -> Self {
        Self::from_nodes(vec![target], target, true)
    }

    pub(super) fn from_nodes(nodes: Vec<BlockPos>, target: BlockPos, reached: bool) -> Self {
        Self {
            nodes,
            next_node_index: 0,
            target,
            reached,
        }
    }

    pub(super) fn is_done(&self) -> bool {
        self.next_node_index >= self.nodes.len()
    }

    pub(super) fn advance(&mut self) {
        self.next_node_index = self.next_node_index.saturating_add(1);
    }

    pub(super) fn target(&self) -> BlockPos {
        self.target
    }

    pub(super) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn can_reach(&self) -> bool {
        self.reached
    }

    pub(super) fn nodes(&self) -> &[BlockPos] {
        &self.nodes
    }

    pub(super) fn next_node_pos(&self) -> Option<BlockPos> {
        self.nodes.get(self.next_node_index).copied()
    }

    pub(super) fn next_entity_pos(&self, entity_width: f32) -> Vec3d {
        let node = self.nodes[self.next_node_index];
        let center_offset = f64::from((entity_width + 1.0) as i32) * 0.5;
        Vec3d::new(
            node.x as f64 + center_offset,
            node.y as f64,
            node.z as f64 + center_offset,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_entity_pos_matches_java_width_centering_for_passive_mobs() {
        let cow_path = GroundPath::from_single_target(BlockPos::new(1, 64, 2));
        let chicken_path = GroundPath::from_single_target(BlockPos::new(1, 64, 2));

        assert_eq!(cow_path.next_entity_pos(0.9), Vec3d::new(1.5, 64.0, 2.5));
        assert_eq!(
            chicken_path.next_entity_pos(0.4),
            Vec3d::new(1.5, 64.0, 2.5)
        );
    }

    #[test]
    fn path_tracks_reachability_and_nodes() {
        let path = GroundPath::from_nodes(
            vec![BlockPos::new(0, 64, 0), BlockPos::new(1, 64, 0)],
            BlockPos::new(1, 64, 0),
            false,
        );

        assert_eq!(path.node_count(), 2);
        assert!(!path.can_reach());
        assert_eq!(
            path.nodes(),
            &[BlockPos::new(0, 64, 0), BlockPos::new(1, 64, 0)]
        );
    }
}
