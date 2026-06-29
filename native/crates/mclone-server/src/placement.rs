use mclone_core::{BlockHitResult, BlockPos, Direction};
use mclone_worldgen::block::{
    AIR, BIRCH_LOG, BIRCH_LOG_X, BIRCH_LOG_Z, CAVE_AIR, DANDELION, DEAD_BUSH, DEEPSLATE,
    DEEPSLATE_X, DEEPSLATE_Z, FERN, GLOW_LICHEN, GRASS, LARGE_FERN_LOWER, LARGE_FERN_UPPER,
    OAK_LOG, OAK_LOG_X, OAK_LOG_Z, POPPY, RawBlockId, SNOW, SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z,
    base_block_id, has_fluid,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UseOnContext {
    hit: BlockHitResult,
    item_in_hand: DebugBlockItem,
}

impl UseOnContext {
    pub(crate) const fn new(hit: BlockHitResult, item_in_hand: DebugBlockItem) -> Self {
        Self { hit, item_in_hand }
    }

    pub(crate) const fn clicked_pos(self) -> BlockPos {
        self.hit.block_pos
    }

    pub(crate) const fn clicked_face(self) -> Direction {
        self.hit.direction
    }

    pub(crate) const fn item_in_hand(self) -> DebugBlockItem {
        self.item_in_hand
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BlockPlaceContext {
    use_on: UseOnContext,
    relative_pos: BlockPos,
    replace_clicked: bool,
}

impl BlockPlaceContext {
    pub(crate) fn new(use_on: UseOnContext, clicked_block: RawBlockId) -> Self {
        let relative_pos = use_on.clicked_pos().relative(use_on.clicked_face());
        let replace_clicked = can_be_replaced(clicked_block, use_on.item_in_hand(), true, use_on);
        Self {
            use_on,
            relative_pos,
            replace_clicked,
        }
    }

    pub(crate) const fn clicked_pos(self) -> BlockPos {
        if self.replace_clicked {
            self.use_on.clicked_pos()
        } else {
            self.relative_pos
        }
    }

    pub(crate) fn can_place(self, target_block: Option<RawBlockId>) -> bool {
        self.replace_clicked
            || target_block.is_some_and(|block| {
                can_be_replaced(
                    block,
                    self.use_on.item_in_hand(),
                    self.replace_clicked,
                    self.use_on,
                )
            })
    }

    pub(crate) const fn replacing_clicked_on_block(self) -> bool {
        self.replace_clicked
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DebugBlockItem {
    block: RawBlockId,
}

impl DebugBlockItem {
    pub(crate) const fn new(block: RawBlockId) -> Option<Self> {
        if matches!(block, AIR | CAVE_AIR) {
            None
        } else {
            Some(Self { block })
        }
    }

    pub(crate) const fn block(self) -> RawBlockId {
        self.block
    }

    pub(crate) fn use_on(
        self,
        hit: BlockHitResult,
        clicked_block: RawBlockId,
        relative_block: Option<RawBlockId>,
    ) -> Option<BlockPlacement> {
        self.place(
            BlockPlaceContext::new(UseOnContext::new(hit, self), clicked_block),
            relative_block,
        )
    }

    fn place(
        self,
        context: BlockPlaceContext,
        relative_block: Option<RawBlockId>,
    ) -> Option<BlockPlacement> {
        let target_block = if context.replacing_clicked_on_block() {
            Some(self.block)
        } else {
            relative_block
        };
        context.can_place(target_block).then_some(BlockPlacement {
            pos: context.clicked_pos(),
            block: self.placement_block(context),
        })
    }

    fn placement_block(self, context: BlockPlaceContext) -> RawBlockId {
        rotated_pillar_block_for_axis(self.block, context.use_on.clicked_face())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BlockPlacement {
    pub(crate) pos: BlockPos,
    pub(crate) block: RawBlockId,
}

fn can_be_replaced(
    existing: RawBlockId,
    item_in_hand: DebugBlockItem,
    replacing_clicked: bool,
    use_on: UseOnContext,
) -> bool {
    match existing {
        SNOW if item_in_hand.block() == SNOW => {
            !replacing_clicked || use_on.clicked_face() == Direction::Up
        }
        _ if base_block_id(existing) == base_block_id(item_in_hand.block()) => false,
        AIR | CAVE_AIR => true,
        SNOW => true,
        GRASS | FERN | DANDELION | POPPY | DEAD_BUSH | LARGE_FERN_LOWER | LARGE_FERN_UPPER
        | GLOW_LICHEN => true,
        id if has_fluid(id) => true,
        _ => false,
    }
}

fn rotated_pillar_block_for_axis(block: RawBlockId, clicked_face: Direction) -> RawBlockId {
    let axis = clicked_face_axis(clicked_face);
    match (block, axis) {
        (OAK_LOG, PlacementAxis::X) => OAK_LOG_X,
        (OAK_LOG, PlacementAxis::Z) => OAK_LOG_Z,
        (BIRCH_LOG, PlacementAxis::X) => BIRCH_LOG_X,
        (BIRCH_LOG, PlacementAxis::Z) => BIRCH_LOG_Z,
        (SPRUCE_LOG, PlacementAxis::X) => SPRUCE_LOG_X,
        (SPRUCE_LOG, PlacementAxis::Z) => SPRUCE_LOG_Z,
        (DEEPSLATE, PlacementAxis::X) => DEEPSLATE_X,
        (DEEPSLATE, PlacementAxis::Z) => DEEPSLATE_Z,
        _ => block,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlacementAxis {
    X,
    Y,
    Z,
}

fn clicked_face_axis(direction: Direction) -> PlacementAxis {
    match direction {
        Direction::East | Direction::West => PlacementAxis::X,
        Direction::Up | Direction::Down => PlacementAxis::Y,
        Direction::North | Direction::South => PlacementAxis::Z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Vec3d;
    use mclone_worldgen::block::{DIRT, OAK_LOG, OAK_LOG_X, OAK_LOG_Z, STONE, WATER};

    fn block_hit(pos: BlockPos, direction: Direction) -> BlockHitResult {
        BlockHitResult::new(
            Vec3d::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5),
            direction,
            pos,
            false,
        )
    }

    fn item(block: RawBlockId) -> DebugBlockItem {
        DebugBlockItem::new(block).expect("debug block item")
    }

    #[test]
    fn block_place_context_replaces_replaceable_clicked_blocks() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            item(DIRT).use_on(block_hit(clicked, Direction::Up), GRASS, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: DIRT,
            })
        );
        assert_eq!(
            item(DIRT).use_on(block_hit(clicked, Direction::Up), WATER, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: DIRT,
            })
        );
        assert_eq!(
            item(DIRT).use_on(block_hit(clicked, Direction::North), SNOW, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: DIRT,
            })
        );
    }

    #[test]
    fn snow_placement_follows_snow_layer_replacement_shape() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            item(SNOW).use_on(block_hit(clicked, Direction::Up), SNOW, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: SNOW,
            })
        );
        assert_eq!(
            item(SNOW).use_on(block_hit(clicked, Direction::North), SNOW, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 80, -1),
                block: SNOW,
            })
        );
    }

    #[test]
    fn block_item_place_uses_relative_pos_for_solid_clicked_blocks() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            item(DIRT).use_on(block_hit(clicked, Direction::Up), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 81, 0),
                block: DIRT,
            })
        );
        assert_eq!(
            item(DIRT).use_on(block_hit(clicked, Direction::Up), STONE, Some(STONE)),
            None
        );
    }

    #[test]
    fn rotated_pillar_placement_uses_clicked_face_axis() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            item(OAK_LOG).use_on(block_hit(clicked, Direction::Up), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 81, 0),
                block: OAK_LOG,
            })
        );
        assert_eq!(
            item(OAK_LOG).use_on(block_hit(clicked, Direction::East), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(1, 80, 0),
                block: OAK_LOG_X,
            })
        );
        assert_eq!(
            item(OAK_LOG).use_on(block_hit(clicked, Direction::North), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 80, -1),
                block: OAK_LOG_Z,
            })
        );
    }

    #[test]
    fn rotated_pillar_variants_do_not_replace_same_base_block() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            item(OAK_LOG).use_on(block_hit(clicked, Direction::Up), STONE, Some(OAK_LOG_X)),
            None
        );
        assert_eq!(
            item(OAK_LOG).use_on(
                block_hit(clicked, Direction::East),
                OAK_LOG_Z,
                Some(OAK_LOG_X)
            ),
            None
        );
    }

    #[test]
    fn debug_block_item_rejects_air_items() {
        assert_eq!(DebugBlockItem::new(AIR), None);
        assert_eq!(DebugBlockItem::new(CAVE_AIR), None);
    }
}
