use mclone_protocol::{ItemKind, ItemStackSnapshot};

pub(crate) const EGG_MAX_STACK_SIZE: u8 = 16;
pub(crate) const FEATHER_MAX_STACK_SIZE: u8 = 64;

pub(crate) const fn item_max_stack_size(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Egg | ItemKind::MallardEgg => EGG_MAX_STACK_SIZE,
        ItemKind::MallardFeather => FEATHER_MAX_STACK_SIZE,
        ItemKind::HuntingSpear => 1,
        ItemKind::BeeHotel => 1,
        ItemKind::WoodenHoe => 1,
        ItemKind::Venison
        | ItemKind::DeerHide
        | ItemKind::ShedAntler
        | ItemKind::Beeswax
        | ItemKind::WheatSeeds
        | ItemKind::Wheat => 64,
    }
}

pub(crate) fn item_stack_has_room(stack: ItemStackSnapshot) -> bool {
    stack.count < item_max_stack_size(stack.kind)
}

pub(crate) fn item_stacks_can_merge(left: ItemStackSnapshot, right: ItemStackSnapshot) -> bool {
    left.kind == right.kind
        && left.count != 0
        && right.count != 0
        && u16::from(left.count) + u16::from(right.count)
            <= u16::from(item_max_stack_size(left.kind))
}

pub(crate) fn merged_item_stack(
    left: ItemStackSnapshot,
    right: ItemStackSnapshot,
) -> Option<ItemStackSnapshot> {
    item_stacks_can_merge(left, right).then_some(ItemStackSnapshot {
        kind: left.kind,
        count: left.count + right.count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egg_stack_size_matches_java_1_17_1() {
        assert_eq!(item_max_stack_size(ItemKind::Egg), 16);
        assert_eq!(item_max_stack_size(ItemKind::MallardEgg), 16);
    }

    #[test]
    fn merge_requires_same_item_and_capacity() {
        let one = ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 1,
        };
        let full = ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: EGG_MAX_STACK_SIZE,
        };

        assert_eq!(
            merged_item_stack(one, one),
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2
            })
        );
        assert_eq!(merged_item_stack(full, one), None);
        assert_eq!(
            merged_item_stack(
                one,
                ItemStackSnapshot {
                    kind: ItemKind::MallardEgg,
                    count: 1,
                },
            ),
            None
        );
    }
}
