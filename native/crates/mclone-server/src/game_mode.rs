use mclone_core::{BlockHitResult, BlockPos, Direction, HitResultType, Vec3d};
use mclone_worldgen::block::{
    AIR, CAVE_AIR, DANDELION, DEAD_BUSH, FERN, GLOW_LICHEN, GRASS, LARGE_FERN_LOWER,
    LARGE_FERN_UPPER, POPPY, RawBlockId, SNOW, has_fluid,
};

pub(crate) const JAVA_OVERWORLD_MAX_BUILD_HEIGHT: i32 = 256;

const JAVA_BLOCK_BREAK_REACH_SQR: f64 = 36.0;
const JAVA_USE_ITEM_ON_REACH_SQR: f64 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerInteractionContext {
    player_feet_position: Vec3d,
    max_build_height: i32,
}

impl ServerInteractionContext {
    pub(crate) const fn new(player_feet_position: Vec3d, max_build_height: i32) -> Self {
        Self {
            player_feet_position,
            max_build_height,
        }
    }

    pub(crate) const fn debug_creative(player_feet_position: Vec3d) -> Self {
        Self::new(player_feet_position, JAVA_OVERWORLD_MAX_BUILD_HEIGHT)
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

    pub(crate) fn debug_place_target(
        self,
        hit: BlockHitResult,
        clicked_block: RawBlockId,
        relative_block: Option<RawBlockId>,
        placing_block: RawBlockId,
    ) -> Option<BlockPos> {
        if !self.may_use_item_on(hit) {
            return None;
        }
        if matches!(placing_block, AIR | CAVE_AIR) {
            return None;
        }

        let replace_clicked =
            can_replace_for_debug_place(clicked_block, placing_block, hit.direction, true);
        let target = if replace_clicked {
            hit.block_pos
        } else {
            hit.block_pos.relative(hit.direction)
        };
        if target.y >= self.max_build_height {
            return None;
        }

        let target_block = if replace_clicked {
            clicked_block
        } else {
            relative_block?
        };
        can_replace_for_debug_place(target_block, placing_block, hit.direction, replace_clicked)
            .then_some(target)
    }

    fn block_break_distance_sqr(self, pos: BlockPos) -> f64 {
        let dx = self.player_feet_position.x - (pos.x as f64 + 0.5);
        let dy = self.player_feet_position.y - (pos.y as f64 + 0.5) + 1.5;
        let dz = self.player_feet_position.z - (pos.z as f64 + 0.5);
        dx * dx + dy * dy + dz * dz
    }

    fn distance_to_block_center_sqr(self, pos: BlockPos) -> f64 {
        let dx = self.player_feet_position.x - (pos.x as f64 + 0.5);
        let dy = self.player_feet_position.y - (pos.y as f64 + 0.5);
        let dz = self.player_feet_position.z - (pos.z as f64 + 0.5);
        dx * dx + dy * dy + dz * dz
    }
}

fn can_replace_for_debug_place(
    existing: RawBlockId,
    placing: RawBlockId,
    clicked_face: Direction,
    replacing_clicked: bool,
) -> bool {
    match existing {
        SNOW if placing == SNOW => !replacing_clicked || clicked_face == Direction::Up,
        _ if existing == placing => false,
        AIR | CAVE_AIR => true,
        SNOW => true,
        GRASS | FERN | DANDELION | POPPY | DEAD_BUSH | LARGE_FERN_LOWER | LARGE_FERN_UPPER
        | GLOW_LICHEN => true,
        id if has_fluid(id) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{DIRT, STONE, WATER};

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
    fn block_place_context_replaces_replaceable_clicked_blocks() {
        let context = ServerInteractionContext::debug_creative(Vec3d::new(0.5, 80.0, 0.5));
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), GRASS, Some(AIR), DIRT),
            Some(clicked)
        );
        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), WATER, Some(AIR), DIRT),
            Some(clicked)
        );
        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::North), SNOW, Some(AIR), DIRT),
            Some(clicked)
        );
        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), SNOW, Some(AIR), SNOW),
            Some(clicked)
        );
        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::North), SNOW, Some(AIR), SNOW),
            Some(BlockPos::new(0, 80, -1))
        );
        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), GRASS, Some(AIR), AIR),
            None
        );
    }

    #[test]
    fn block_place_context_uses_relative_pos_for_solid_clicked_blocks() {
        let context = ServerInteractionContext::debug_creative(Vec3d::new(0.5, 80.0, 0.5));
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), STONE, Some(AIR), DIRT),
            Some(BlockPos::new(0, 81, 0))
        );
        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), STONE, Some(STONE), DIRT),
            None
        );
    }

    #[test]
    fn block_place_context_rejects_too_high_relative_targets() {
        let context = ServerInteractionContext::debug_creative(Vec3d::new(0.5, 254.0, 0.5));
        let clicked = BlockPos::new(0, JAVA_OVERWORLD_MAX_BUILD_HEIGHT - 1, 0);

        assert_eq!(
            context.debug_place_target(block_hit(clicked, Direction::Up), STONE, Some(AIR), DIRT),
            None
        );
    }
}
