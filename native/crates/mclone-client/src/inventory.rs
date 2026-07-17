use mclone_core::BlockStateId;
use mclone_protocol::{
    ClientCommand, DEFAULT_DEBUG_HOTBAR, DebugHotbarItem, HOTBAR_SLOT_COUNT,
    HOTBAR_SLOT_COUNT_USIZE, SetCarriedItemCommand, SetDebugHotbarSlotCommand,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientInventory {
    items: [Option<DebugHotbarItem>; HOTBAR_SLOT_COUNT_USIZE],
    selected: u8,
    sent_carried: u8,
}

impl Default for ClientInventory {
    fn default() -> Self {
        Self {
            items: DEFAULT_DEBUG_HOTBAR,
            selected: 0,
            sent_carried: 0,
        }
    }
}

impl ClientInventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn selected_hotbar_slot(&self) -> u8 {
        self.selected
    }

    pub const fn hotbar_items(&self) -> [Option<DebugHotbarItem>; HOTBAR_SLOT_COUNT_USIZE] {
        self.items
    }

    pub fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        if slot >= HOTBAR_SLOT_COUNT {
            return false;
        }
        self.selected = slot;
        true
    }

    pub fn set_debug_hotbar_slot(
        &mut self,
        slot: u8,
        block_state: Option<BlockStateId>,
    ) -> Option<ClientCommand> {
        self.set_debug_hotbar_item(slot, block_state.map(DebugHotbarItem::Block))
    }

    pub fn set_debug_hotbar_item(
        &mut self,
        slot: u8,
        item: Option<DebugHotbarItem>,
    ) -> Option<ClientCommand> {
        if slot >= HOTBAR_SLOT_COUNT {
            return None;
        }
        self.items[slot as usize] = item;
        Some(ClientCommand::SetDebugHotbarSlot(
            SetDebugHotbarSlotCommand { slot, item },
        ))
    }

    pub fn ensure_has_sent_carried_item(&mut self) -> Option<ClientCommand> {
        if self.selected == self.sent_carried {
            return None;
        }
        self.sent_carried = self.selected;
        Some(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
            slot: self.selected,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_selection_uses_java_hotbar_slot_bounds() {
        let mut inventory = ClientInventory::new();

        assert!(inventory.select_hotbar_slot(HOTBAR_SLOT_COUNT - 1));
        assert_eq!(inventory.selected_hotbar_slot(), HOTBAR_SLOT_COUNT - 1);
        assert!(!inventory.select_hotbar_slot(HOTBAR_SLOT_COUNT));
        assert_eq!(inventory.selected_hotbar_slot(), HOTBAR_SLOT_COUNT - 1);
    }

    #[test]
    fn carried_item_sync_only_emits_when_selection_changed() {
        let mut inventory = ClientInventory::new();

        assert_eq!(inventory.ensure_has_sent_carried_item(), None);
        assert!(inventory.select_hotbar_slot(4));
        assert_eq!(
            inventory.ensure_has_sent_carried_item(),
            Some(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
                slot: 4
            }))
        );
        assert_eq!(inventory.ensure_has_sent_carried_item(), None);
    }

    #[test]
    fn debug_hotbar_slot_assignment_updates_client_items_and_emits_command() {
        let mut inventory = ClientInventory::new();

        assert_eq!(
            inventory.hotbar_items()[0],
            Some(DebugHotbarItem::Block(BlockStateId(1)))
        );
        assert_eq!(
            inventory.set_debug_hotbar_slot(0, Some(BlockStateId(91))),
            Some(ClientCommand::SetDebugHotbarSlot(
                SetDebugHotbarSlotCommand {
                    slot: 0,
                    item: Some(DebugHotbarItem::Block(BlockStateId(91))),
                }
            ))
        );
        assert_eq!(
            inventory.hotbar_items()[0],
            Some(DebugHotbarItem::Block(BlockStateId(91)))
        );
        assert_eq!(
            inventory.set_debug_hotbar_slot(HOTBAR_SLOT_COUNT, Some(BlockStateId(5))),
            None
        );
    }
}
