use mclone_core::BlockStateId;
use mclone_protocol::{
    DEFAULT_DEBUG_HOTBAR, HOTBAR_SLOT_COUNT, HOTBAR_SLOT_COUNT_USIZE, SetCarriedItemCommand,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ServerInventory {
    items: [Option<BlockStateId>; HOTBAR_SLOT_COUNT_USIZE],
    selected: u8,
}

impl Default for ServerInventory {
    fn default() -> Self {
        Self {
            items: DEFAULT_DEBUG_HOTBAR,
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

    pub(crate) fn selected_block_state(&self) -> Option<BlockStateId> {
        self.items[self.selected as usize]
    }

    #[cfg(test)]
    pub(crate) const fn selected_hotbar_slot(&self) -> u8 {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{BRICKS, DIRT, STONE, generated_block_state_id};

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
        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 8 }));
        assert_eq!(inventory.selected_block_state(), None);
    }

    #[test]
    fn debug_hotbar_exposes_bricks_as_a_placeable_block() {
        let mut inventory = ServerInventory::default();

        assert!(inventory.apply_set_carried_item(SetCarriedItemCommand { slot: 7 }));
        assert_eq!(
            inventory.selected_block_state(),
            Some(generated_block_state_id(BRICKS))
        );
    }
}
