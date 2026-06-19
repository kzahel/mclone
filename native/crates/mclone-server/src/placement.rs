use mclone_core::{BlockHitResult, BlockPos, Direction};
use mclone_worldgen::block::{
    AIR, CAVE_AIR, DANDELION, DEAD_BUSH, FERN, GLOW_LICHEN, GRASS, LARGE_FERN_LOWER,
    LARGE_FERN_UPPER, POPPY, RawBlockId, SNOW, has_fluid,
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
            block: self.block,
        })
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
        _ if existing == item_in_hand.block() => false,
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
    use mclone_core::Vec3d;
    use mclone_worldgen::block::{DIRT, STONE, WATER};

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
    fn debug_block_item_rejects_air_items() {
        assert_eq!(DebugBlockItem::new(AIR), None);
        assert_eq!(DebugBlockItem::new(CAVE_AIR), None);
    }
}
