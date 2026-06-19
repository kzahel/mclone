use mclone_protocol::{ClientCommand, HOTBAR_SLOT_COUNT, SetCarriedItemCommand};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientInventory {
    selected: u8,
    sent_carried: u8,
}

impl Default for ClientInventory {
    fn default() -> Self {
        Self {
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

    pub fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        if slot >= HOTBAR_SLOT_COUNT {
            return false;
        }
        self.selected = slot;
        true
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
}
