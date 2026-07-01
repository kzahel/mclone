use mclone_core::{BlockHitResult, BlockPos, Direction};
use mclone_worldgen::block::{
    AIR, BIRCH_LOG, BIRCH_LOG_X, BIRCH_LOG_Z, CAVE_AIR, DANDELION, DEAD_BUSH, DEEPSLATE,
    DEEPSLATE_X, DEEPSLATE_Z, FERN, GLOW_LICHEN, GRASS, LARGE_FERN_LOWER, LARGE_FERN_UPPER,
    OAK_LOG, OAK_LOG_X, OAK_LOG_Z, POPPY, RawBlockId, SNOW, SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z,
    TORCH, WALL_TORCH_EAST, WALL_TORCH_NORTH, WALL_TORCH_SOUTH, WALL_TORCH_WEST, base_block_id,
    has_fluid, material_blocks_motion,
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
        block_at: impl Fn(BlockPos) -> Option<RawBlockId>,
    ) -> Option<BlockPlacement> {
        self.place(
            BlockPlaceContext::new(UseOnContext::new(hit, self), clicked_block),
            block_at,
        )
    }

    fn place(
        self,
        context: BlockPlaceContext,
        block_at: impl Fn(BlockPos) -> Option<RawBlockId>,
    ) -> Option<BlockPlacement> {
        let pos = context.clicked_pos();
        if !context.can_place(block_at(pos)) {
            return None;
        }
        Some(BlockPlacement {
            pos,
            block: self.placement_block(context, &block_at)?,
        })
    }

    fn placement_block(
        self,
        context: BlockPlaceContext,
        block_at: &impl Fn(BlockPos) -> Option<RawBlockId>,
    ) -> Option<RawBlockId> {
        if self.is_torch_item() {
            torch_placement_block(context, block_at)
        } else {
            Some(rotated_pillar_block_for_axis(
                self.block,
                context.use_on.clicked_face(),
            ))
        }
    }

    const fn is_torch_item(self) -> bool {
        base_block_id(self.block) == TORCH
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

fn torch_placement_block(
    context: BlockPlaceContext,
    block_at: &impl Fn(BlockPos) -> Option<RawBlockId>,
) -> Option<RawBlockId> {
    let pos = context.clicked_pos();
    match context.use_on.clicked_face() {
        Direction::Up => can_support_torch(block_at(pos.below())).then_some(TORCH),
        Direction::North => {
            wall_torch_block(Direction::North, pos, block_at).then_some(WALL_TORCH_NORTH)
        }
        Direction::South => {
            wall_torch_block(Direction::South, pos, block_at).then_some(WALL_TORCH_SOUTH)
        }
        Direction::West => {
            wall_torch_block(Direction::West, pos, block_at).then_some(WALL_TORCH_WEST)
        }
        Direction::East => {
            wall_torch_block(Direction::East, pos, block_at).then_some(WALL_TORCH_EAST)
        }
        Direction::Down => None,
    }
}

fn wall_torch_block(
    facing: Direction,
    pos: BlockPos,
    block_at: &impl Fn(BlockPos) -> Option<RawBlockId>,
) -> bool {
    can_support_torch(block_at(pos.relative(facing.opposite())))
}

fn can_support_torch(block: Option<RawBlockId>) -> bool {
    block.is_some_and(material_blocks_motion)
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
    use mclone_worldgen::block::{
        DIRT, OAK_LOG, OAK_LOG_X, OAK_LOG_Z, STONE, TORCH, WALL_TORCH_EAST, WALL_TORCH_NORTH, WATER,
    };

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

    fn place_item(
        block: RawBlockId,
        hit: BlockHitResult,
        clicked_block: RawBlockId,
        relative_block: Option<RawBlockId>,
    ) -> Option<BlockPlacement> {
        let clicked_pos = hit.block_pos;
        let relative_pos = clicked_pos.relative(hit.direction);
        item(block).use_on(hit, clicked_block, |pos| {
            if pos == clicked_pos {
                Some(clicked_block)
            } else if pos == relative_pos {
                relative_block
            } else {
                Some(AIR)
            }
        })
    }

    fn place_item_with_blocks(
        block: RawBlockId,
        hit: BlockHitResult,
        blocks: &[(BlockPos, RawBlockId)],
    ) -> Option<BlockPlacement> {
        let clicked_block = blocks
            .iter()
            .find_map(|(pos, block)| (*pos == hit.block_pos).then_some(*block))
            .expect("clicked block must be present");
        item(block).use_on(hit, clicked_block, |pos| {
            blocks
                .iter()
                .find_map(|(block_pos, block)| (*block_pos == pos).then_some(*block))
        })
    }

    #[test]
    fn block_place_context_replaces_replaceable_clicked_blocks() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            place_item(DIRT, block_hit(clicked, Direction::Up), GRASS, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: DIRT,
            })
        );
        assert_eq!(
            place_item(DIRT, block_hit(clicked, Direction::Up), WATER, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: DIRT,
            })
        );
        assert_eq!(
            place_item(DIRT, block_hit(clicked, Direction::North), SNOW, Some(AIR)),
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
            place_item(SNOW, block_hit(clicked, Direction::Up), SNOW, Some(AIR)),
            Some(BlockPlacement {
                pos: clicked,
                block: SNOW,
            })
        );
        assert_eq!(
            place_item(SNOW, block_hit(clicked, Direction::North), SNOW, Some(AIR)),
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
            place_item(DIRT, block_hit(clicked, Direction::Up), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 81, 0),
                block: DIRT,
            })
        );
        assert_eq!(
            place_item(DIRT, block_hit(clicked, Direction::Up), STONE, Some(STONE)),
            None
        );
    }

    #[test]
    fn rotated_pillar_placement_uses_clicked_face_axis() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            place_item(OAK_LOG, block_hit(clicked, Direction::Up), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 81, 0),
                block: OAK_LOG,
            })
        );
        assert_eq!(
            place_item(
                OAK_LOG,
                block_hit(clicked, Direction::East),
                STONE,
                Some(AIR)
            ),
            Some(BlockPlacement {
                pos: BlockPos::new(1, 80, 0),
                block: OAK_LOG_X,
            })
        );
        assert_eq!(
            place_item(
                OAK_LOG,
                block_hit(clicked, Direction::North),
                STONE,
                Some(AIR)
            ),
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
            place_item(
                OAK_LOG,
                block_hit(clicked, Direction::Up),
                STONE,
                Some(OAK_LOG_X)
            ),
            None
        );
        assert_eq!(
            place_item(
                OAK_LOG,
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

    #[test]
    fn torch_item_uses_standing_state_on_supported_top_face() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            place_item(TORCH, block_hit(clicked, Direction::Up), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 81, 0),
                block: TORCH,
            })
        );
    }

    #[test]
    fn torch_item_uses_wall_state_matching_clicked_side_face() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            place_item(TORCH, block_hit(clicked, Direction::East), STONE, Some(AIR)),
            Some(BlockPlacement {
                pos: BlockPos::new(1, 80, 0),
                block: WALL_TORCH_EAST,
            })
        );
        assert_eq!(
            place_item(
                TORCH,
                block_hit(clicked, Direction::North),
                STONE,
                Some(AIR)
            ),
            Some(BlockPlacement {
                pos: BlockPos::new(0, 80, -1),
                block: WALL_TORCH_NORTH,
            })
        );
    }

    #[test]
    fn torch_item_rejects_unsupported_faces_and_missing_support() {
        let clicked = BlockPos::new(0, 80, 0);

        assert_eq!(
            place_item(TORCH, block_hit(clicked, Direction::Down), STONE, Some(AIR)),
            None
        );
        assert_eq!(
            place_item_with_blocks(
                TORCH,
                block_hit(clicked, Direction::Up),
                &[(clicked, GRASS), (clicked.below(), AIR)],
            ),
            None
        );
    }
}
