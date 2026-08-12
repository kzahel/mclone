use mclone_core::BlockStateId;
#[cfg(test)]
use mclone_protocol::ItemKind;
use mclone_protocol::ItemStackSnapshot;
use mclone_protocol::{
    DEFAULT_DEBUG_HOTBAR, DebugHotbarItem, HOTBAR_SLOT_COUNT, HOTBAR_SLOT_COUNT_USIZE,
    SetCarriedItemCommand, SetDebugHotbarSlotCommand,
};

use crate::item_stack::item_max_stack_size;

const PLAYER_MAIN_INVENTORY_SLOT_COUNT: usize = 36;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ServerInventory {
    items: [Option<DebugHotbarItem>; HOTBAR_SLOT_COUNT_USIZE],
    item_stacks: [Option<ItemStackSnapshot>; PLAYER_MAIN_INVENTORY_SLOT_COUNT],
    selected: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ItemStackAddResult {
    pub(crate) accepted_count: u8,
    pub(crate) remaining: Option<ItemStackSnapshot>,
}

impl Default for ServerInventory {
    fn default() -> Self {
        let mut item_stacks = [None; PLAYER_MAIN_INVENTORY_SLOT_COUNT];
        item_stacks[0] = Some(ItemStackSnapshot {
            kind: mclone_protocol::ItemKind::HuntingSpear,
            count: 1,
        });
        Self {
            items: DEFAULT_DEBUG_HOTBAR,
            item_stacks,
            selected: 0,
        }
    }
}

impl ServerInventory {
    pub(crate) fn apply_set_carried_item(&mut self, command: SetCarriedItemCommand) -> bool {
        if command.slot >= HOTBAR_SLOT_COUNT {
            return false;
        }
        self.selected = command.slot;
        true
    }

    pub(crate) fn apply_set_debug_hotbar_slot(
        &mut self,
        command: SetDebugHotbarSlotCommand,
    ) -> bool {
        if command.slot >= HOTBAR_SLOT_COUNT {
            return false;
        }
        self.items[command.slot as usize] = command.item;
        true
    }

    pub(crate) fn selected_debug_item(&self) -> Option<DebugHotbarItem> {
        self.items[self.selected as usize]
    }

    pub(crate) fn selected_block_state(&self) -> Option<BlockStateId> {
        match self.selected_debug_item() {
            Some(DebugHotbarItem::Block(block_state)) => Some(block_state),
            Some(DebugHotbarItem::SpawnActor(_)) | None => None,
        }
    }

    pub(crate) fn add_item_stack(&mut self, stack: ItemStackSnapshot) -> ItemStackAddResult {
        let original_count = stack.count;
        let mut remaining_count = stack.count;
        while remaining_count > 0 {
            let Some(slot) = self.slot_with_remaining_space(ItemStackSnapshot {
                count: remaining_count,
                ..stack
            }) else {
                break;
            };
            let accepted = self.add_item_stack_to_slot(
                slot,
                ItemStackSnapshot {
                    count: remaining_count,
                    ..stack
                },
            );
            if accepted == 0 {
                break;
            }
            remaining_count -= accepted;
        }

        ItemStackAddResult {
            accepted_count: original_count - remaining_count,
            remaining: (remaining_count > 0).then_some(ItemStackSnapshot {
                count: remaining_count,
                ..stack
            }),
        }
    }

    pub(crate) fn hotbar_item_stacks(
        &self,
    ) -> [Option<ItemStackSnapshot>; mclone_protocol::HOTBAR_SLOT_COUNT_USIZE] {
        std::array::from_fn(|slot| self.item_stacks[slot])
    }

    pub(crate) fn restore_item_stacks(
        &mut self,
        stacks: [Option<ItemStackSnapshot>; PLAYER_MAIN_INVENTORY_SLOT_COUNT],
    ) {
        self.item_stacks = stacks;
    }

    pub(crate) fn item_stacks(
        &self,
    ) -> [Option<ItemStackSnapshot>; PLAYER_MAIN_INVENTORY_SLOT_COUNT] {
        self.item_stacks
    }

    pub(crate) fn selected_item_stack(&self) -> Option<ItemStackSnapshot> {
        self.item_stacks[usize::from(self.selected)]
    }

    pub(crate) fn consume_selected_item(&mut self, kind: mclone_protocol::ItemKind) -> bool {
        let slot = usize::from(self.selected);
        let Some(mut stack) = self.item_stacks[slot] else {
            return false;
        };
        if stack.kind != kind || stack.count == 0 {
            return false;
        }
        stack.count -= 1;
        self.item_stacks[slot] = (stack.count > 0).then_some(stack);
        true
    }

    #[cfg(test)]
    pub(crate) fn item_count(&self, kind: ItemKind) -> u32 {
        self.item_stacks
            .iter()
            .filter_map(|stack| *stack)
            .filter(|stack| stack.kind == kind)
            .map(|stack| u32::from(stack.count))
            .sum()
    }

    pub(crate) const fn selected_hotbar_slot(&self) -> u8 {
        self.selected
    }

    pub(crate) fn restore_selected_hotbar_slot(&mut self, slot: u8) {
        if slot < HOTBAR_SLOT_COUNT {
            self.selected = slot;
        }
    }

    #[cfg(test)]
    pub(crate) const fn item_stack_in_slot(&self, slot: usize) -> Option<ItemStackSnapshot> {
        self.item_stacks[slot]
    }

    #[cfg(test)]
    pub(crate) fn set_item_stack_for_test(
        &mut self,
        slot: usize,
        stack: Option<ItemStackSnapshot>,
    ) {
        self.item_stacks[slot] = stack;
    }

    fn slot_with_remaining_space(&self, stack: ItemStackSnapshot) -> Option<usize> {
        let selected = usize::from(self.selected);
        if self.slot_has_remaining_space(selected, stack) {
            return Some(selected);
        }
        self.item_stacks
            .iter()
            .enumerate()
            .find_map(|(slot, _)| self.slot_has_remaining_space(slot, stack).then_some(slot))
            .or_else(|| self.free_item_slot())
    }

    fn slot_has_remaining_space(&self, slot: usize, stack: ItemStackSnapshot) -> bool {
        let Some(stored) = self.item_stacks.get(slot).copied().flatten() else {
            return false;
        };
        stored.kind == stack.kind && stored.count < item_max_stack_size(stored.kind)
    }

    fn free_item_slot(&self) -> Option<usize> {
        self.item_stacks.iter().position(|stack| stack.is_none())
    }

    fn add_item_stack_to_slot(&mut self, slot: usize, stack: ItemStackSnapshot) -> u8 {
        let max_count = item_max_stack_size(stack.kind);
        let Some(stored) = self.item_stacks.get_mut(slot) else {
            return 0;
        };
        match stored {
            Some(existing) if existing.kind == stack.kind => {
                let accepted = stack.count.min(max_count.saturating_sub(existing.count));
                existing.count += accepted;
                accepted
            }
            Some(_) => 0,
            empty @ None => {
                let accepted = stack.count.min(max_count);
                *empty = Some(ItemStackSnapshot {
                    count: accepted,
                    ..stack
                });
                accepted
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{DIRT, SAND, STONE, generated_block_state_id};

    #[test]
    fn carried_item_packet_updates_only_valid_hotbar_slots() {
        let mut inventory = ServerInventory::default();

        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 6 }));
        assert_eq!(inventory.selected_hotbar_slot(), 6);
        assert!(!inventory.apply_set_carried_item(SetCarriedItemCommand {
            slot: HOTBAR_SLOT_COUNT
        }));
        assert_eq!(inventory.selected_hotbar_slot(), 6);
    }

    #[test]
    fn selected_block_state_reads_debug_hotbar_slot() {
        let mut inventory = ServerInventory::default();

        assert_eq!(
            inventory.selected_block_state(),
            Some(generated_block_state_id(STONE))
        );
        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 1 }));
        assert_eq!(
            inventory.selected_block_state(),
            Some(generated_block_state_id(DIRT))
        );
        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 6 }));
        assert_eq!(inventory.selected_block_state(), Some(BlockStateId(8)));
    }

    #[test]
    fn debug_hotbar_exposes_actor_tools_in_final_slots() {
        let mut inventory = ServerInventory::default();

        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 7 }));
        assert_eq!(
            inventory.selected_debug_item(),
            Some(DebugHotbarItem::SpawnActor(
                mclone_protocol::DebugActorKind::Chicken
            ))
        );
        assert_eq!(inventory.selected_block_state(), None);
        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 8 }));
        assert_eq!(
            inventory.selected_debug_item(),
            Some(DebugHotbarItem::SpawnActor(
                mclone_protocol::DebugActorKind::Mannequin
            ))
        );
    }

    #[test]
    fn debug_hotbar_slot_assignment_changes_server_held_block() {
        let mut inventory = ServerInventory::default();

        assert!(
            inventory.apply_set_debug_hotbar_slot(SetDebugHotbarSlotCommand {
                slot: 0,
                item: Some(DebugHotbarItem::Block(generated_block_state_id(SAND))),
            })
        );
        assert_eq!(
            inventory.selected_block_state(),
            Some(generated_block_state_id(SAND))
        );
        assert!(
            !inventory.apply_set_debug_hotbar_slot(SetDebugHotbarSlotCommand {
                slot: HOTBAR_SLOT_COUNT,
                item: Some(DebugHotbarItem::Block(generated_block_state_id(STONE))),
            })
        );
        assert_eq!(
            inventory.selected_block_state(),
            Some(generated_block_state_id(SAND))
        );
    }

    #[test]
    fn item_pickup_prefers_selected_stack_then_free_slot() {
        let mut inventory = ServerInventory::default();
        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 3 }));
        inventory.set_item_stack_for_test(
            3,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 15,
            }),
        );

        let result = inventory.add_item_stack(ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 2,
        });

        assert_eq!(
            result,
            ItemStackAddResult {
                accepted_count: 2,
                remaining: None,
            }
        );
        assert_eq!(
            inventory.item_stack_in_slot(3),
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 16,
            })
        );
        assert_eq!(
            inventory.item_stack_in_slot(0),
            Some(ItemStackSnapshot {
                kind: ItemKind::HuntingSpear,
                count: 1,
            })
        );
        assert_eq!(
            inventory.item_stack_in_slot(1),
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            })
        );
        assert_eq!(inventory.item_count(ItemKind::Egg), 17);
    }

    #[test]
    fn item_pickup_reports_remaining_when_inventory_is_full() {
        let mut inventory = ServerInventory::default();
        for slot in 0..PLAYER_MAIN_INVENTORY_SLOT_COUNT {
            inventory.set_item_stack_for_test(
                slot,
                Some(ItemStackSnapshot {
                    kind: ItemKind::Egg,
                    count: 16,
                }),
            );
        }

        let result = inventory.add_item_stack(ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 1,
        });

        assert_eq!(
            result,
            ItemStackAddResult {
                accepted_count: 0,
                remaining: Some(ItemStackSnapshot {
                    kind: ItemKind::Egg,
                    count: 1,
                }),
            }
        );
        assert_eq!(
            inventory.item_count(ItemKind::Egg),
            PLAYER_MAIN_INVENTORY_SLOT_COUNT as u32 * 16
        );
    }
}
