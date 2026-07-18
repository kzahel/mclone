use mclone_core::{BlockHitResult, BlockPos, HitResultType, HorizontalTopology, Vec3d};

pub(crate) const JAVA_OVERWORLD_MAX_BUILD_HEIGHT: i32 = 256;

const JAVA_BLOCK_BREAK_REACH_SQR: f64 = 36.0;
const JAVA_USE_ITEM_ON_REACH_SQR: f64 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerInteractionContext {
    player_feet_position: Vec3d,
    max_build_height: i32,
    topology: HorizontalTopology,
}

impl ServerInteractionContext {
    #[cfg(test)]
    pub(crate) const fn new(player_feet_position: Vec3d, max_build_height: i32) -> Self {
        Self {
            player_feet_position,
            max_build_height,
            topology: HorizontalTopology::UNBOUNDED,
        }
    }

    #[cfg(test)]
    pub(crate) const fn debug_creative(player_feet_position: Vec3d) -> Self {
        Self::new(player_feet_position, JAVA_OVERWORLD_MAX_BUILD_HEIGHT)
    }

    pub(crate) const fn debug_creative_in(
        player_feet_position: Vec3d,
        topology: HorizontalTopology,
    ) -> Self {
        Self {
            player_feet_position,
            max_build_height: JAVA_OVERWORLD_MAX_BUILD_HEIGHT,
            topology,
        }
    }

    pub(crate) fn may_break_block(self, pos: BlockPos) -> bool {
        self.player_feet_position.is_finite()
            && pos.y < self.max_build_height
            && self.block_break_distance_sqr(pos) <= JAVA_BLOCK_BREAK_REACH_SQR
    }

    pub(crate) fn may_use_item_on(self, hit: BlockHitResult) -> bool {
        self.player_feet_position.is_finite()
            && hit.hit_type() == HitResultType::Block
            && hit.block_pos.y < self.max_build_height
            && self.distance_to_block_center_sqr(hit.block_pos) < JAVA_USE_ITEM_ON_REACH_SQR
    }

    pub(crate) fn may_place_at(self, pos: BlockPos) -> bool {
        pos.y < self.max_build_height
    }

    fn block_break_distance_sqr(self, pos: BlockPos) -> f64 {
        let target = Vec3d::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
        let displacement = self
            .topology
            .shortest_position_displacement(self.player_feet_position, target);
        let dx = displacement.x;
        let dy = self.player_feet_position.y - (pos.y as f64 + 0.5) + 1.5;
        let dz = displacement.z;
        dx * dx + dy * dy + dz * dz
    }

    fn distance_to_block_center_sqr(self, pos: BlockPos) -> f64 {
        let target = Vec3d::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
        let displacement = self
            .topology
            .shortest_position_displacement(self.player_feet_position, target);
        let dx = displacement.x;
        let dy = self.player_feet_position.y - (pos.y as f64 + 0.5);
        let dz = displacement.z;
        dx * dx + dy * dy + dz * dz
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Direction;

    fn block_hit(pos: BlockPos, direction: Direction) -> BlockHitResult {
        BlockHitResult::new(
            Vec3d::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5),
            direction,
            pos,
            false,
        )
    }

    #[test]
    fn break_validation_matches_java_reach_and_height_shape() {
        let context = ServerInteractionContext::debug_creative(Vec3d::new(0.5, 80.0, 0.5));

        assert!(context.may_break_block(BlockPos::new(0, 80, 0)));
        assert!(!context.may_break_block(BlockPos::new(8, 80, 0)));
        assert!(!context.may_break_block(BlockPos::new(0, JAVA_OVERWORLD_MAX_BUILD_HEIGHT, 0)));
    }

    #[test]
    fn use_item_on_validation_uses_java_packet_handler_shape() {
        let context = ServerInteractionContext::debug_creative(Vec3d::new(0.5, 80.0, 0.5));

        assert!(context.may_use_item_on(block_hit(BlockPos::new(0, 80, 0), Direction::Up)));
        assert!(!context.may_use_item_on(block_hit(BlockPos::new(8, 80, 0), Direction::Up)));
        assert!(!context.may_use_item_on(block_hit(
            BlockPos::new(0, JAVA_OVERWORLD_MAX_BUILD_HEIGHT, 0),
            Direction::Up,
        )));
    }

    #[test]
    fn place_target_validation_uses_max_build_height() {
        let context = ServerInteractionContext::debug_creative(Vec3d::new(0.5, 80.0, 0.5));

        assert!(context.may_place_at(BlockPos::new(0, JAVA_OVERWORLD_MAX_BUILD_HEIGHT - 1, 0)));
        assert!(!context.may_place_at(BlockPos::new(0, JAVA_OVERWORLD_MAX_BUILD_HEIGHT, 0)));
    }

    #[test]
    fn periodic_reach_validation_treats_the_seam_as_one_block() {
        let context = ServerInteractionContext::debug_creative_in(
            Vec3d::new(511.5, 80.0, 0.5),
            HorizontalTopology::cylinder_x(0, 32),
        );
        let canonical = BlockPos::new(0, 80, 0);

        assert!(context.may_break_block(canonical));
        assert!(context.may_use_item_on(block_hit(canonical, Direction::Up)));
    }
}
