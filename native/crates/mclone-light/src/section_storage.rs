use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BlockPosKey, DataLayer, DataLayerStorageMap, LightLayer, SectionPosKey, block_pos_get_x,
    block_pos_get_y, block_pos_get_z, block_pos_offset, block_to_section_key, section_offset,
    section_relative,
};

pub const LIGHT_AND_DATA: u8 = 0;
pub const LIGHT_ONLY: u8 = 1;
pub const EMPTY: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionEdgeUpdate {
    pub source: SectionPosKey,
    pub target: SectionPosKey,
    pub level: u8,
    pub is_decrease: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SectionLevelChange {
    pub added: bool,
    pub unmarked_for_removal: bool,
    pub marked_for_removal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerLightSectionStorage {
    layer: LightLayer,
    data_section_set: BTreeSet<SectionPosKey>,
    to_mark_no_data: BTreeSet<SectionPosKey>,
    to_mark_data: BTreeSet<SectionPosKey>,
    visible_section_data: DataLayerStorageMap,
    updating_section_data: DataLayerStorageMap,
    changed_sections: BTreeSet<SectionPosKey>,
    sections_affected_by_light_updates: BTreeSet<SectionPosKey>,
    queued_sections: BTreeMap<SectionPosKey, DataLayer>,
    untrusted_sections: BTreeSet<SectionPosKey>,
    columns_to_retain_queued_data_for: BTreeSet<SectionPosKey>,
    to_remove: BTreeSet<SectionPosKey>,
    has_to_remove: bool,
}

impl LayerLightSectionStorage {
    pub fn new(layer: LightLayer) -> Self {
        let updating_section_data = DataLayerStorageMap::new();
        let mut visible_section_data = updating_section_data.copy();
        visible_section_data.disable_cache();
        Self {
            layer,
            data_section_set: BTreeSet::new(),
            to_mark_no_data: BTreeSet::new(),
            to_mark_data: BTreeSet::new(),
            visible_section_data,
            updating_section_data,
            changed_sections: BTreeSet::new(),
            sections_affected_by_light_updates: BTreeSet::new(),
            queued_sections: BTreeMap::new(),
            untrusted_sections: BTreeSet::new(),
            columns_to_retain_queued_data_for: BTreeSet::new(),
            to_remove: BTreeSet::new(),
            has_to_remove: false,
        }
    }

    pub fn layer(&self) -> LightLayer {
        self.layer
    }

    pub fn storing_light_for_section(&self, section: SectionPosKey) -> bool {
        self.updating_section_data.has_layer(section)
    }

    pub fn get_data_layer_data(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.queued_sections
            .get(&section)
            .or_else(|| self.visible_section_data.get_layer(section))
    }

    pub fn get_updating_data_layer(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.updating_section_data.get_layer(section)
    }

    pub fn get_visible_data_layer(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.visible_section_data.get_layer(section)
    }

    pub fn get_stored_level(&self, block: BlockPosKey) -> u8 {
        let section = block_to_section_key(block);
        let data_layer = self
            .updating_section_data
            .get_layer(section)
            .unwrap_or_else(|| panic!("block {block} is in an unlit section"));
        data_layer.get(
            section_relative(block_pos_get_x(block)),
            section_relative(block_pos_get_y(block)),
            section_relative(block_pos_get_z(block)),
        )
    }

    pub fn set_stored_level(&mut self, block: BlockPosKey, level: u8) {
        let section = block_to_section_key(block);
        if self.changed_sections.insert(section) {
            self.updating_section_data.copy_data_layer(section);
        }
        let data_layer = self
            .updating_section_data
            .get_layer_mut(section)
            .unwrap_or_else(|| panic!("block {block} is in an unlit section"));
        data_layer.set(
            section_relative(block_pos_get_x(block)),
            section_relative(block_pos_get_y(block)),
            section_relative(block_pos_get_z(block)),
            level,
        );
        self.mark_block_neighbor_sections_affected(block);
    }

    pub fn section_level(&self, section: SectionPosKey) -> u8 {
        if section == SectionPosKey::MAX {
            EMPTY
        } else if self.data_section_set.contains(&section) {
            LIGHT_AND_DATA
        } else if !self.to_remove.contains(&section)
            && self.updating_section_data.has_layer(section)
        {
            LIGHT_ONLY
        } else {
            EMPTY
        }
    }

    pub fn level_from_source(&self, section: SectionPosKey) -> u8 {
        if self.to_mark_no_data.contains(&section) {
            EMPTY
        } else if !self.data_section_set.contains(&section) && !self.to_mark_data.contains(&section)
        {
            EMPTY
        } else {
            LIGHT_AND_DATA
        }
    }

    pub fn apply_graph_level(&mut self, section: SectionPosKey, level: u8) -> SectionLevelChange {
        assert!(level <= EMPTY, "section graph level {level} out of range");
        let old_level = self.section_level(section);
        let mut change = SectionLevelChange::default();

        if old_level != LIGHT_AND_DATA && level == LIGHT_AND_DATA {
            self.data_section_set.insert(section);
            self.to_mark_data.remove(&section);
        }

        if old_level == LIGHT_AND_DATA && level != LIGHT_AND_DATA {
            self.data_section_set.remove(&section);
            self.to_mark_no_data.remove(&section);
        }

        if old_level >= EMPTY && level != EMPTY {
            if self.to_remove.remove(&section) {
                change.unmarked_for_removal = true;
            } else {
                let data_layer = self.create_data_layer(section);
                self.updating_section_data.set_layer(section, data_layer);
                self.changed_sections.insert(section);
                self.mark_section_and_neighbor_sections_affected(section);
                change.added = true;
            }
        }

        if old_level != EMPTY && level >= EMPTY {
            self.to_remove.insert(section);
            change.marked_for_removal = true;
        }

        self.has_to_remove = !self.to_remove.is_empty();
        change
    }

    pub fn create_data_layer(&self, section: SectionPosKey) -> DataLayer {
        self.queued_sections
            .get(&section)
            .cloned()
            .unwrap_or_else(DataLayer::new)
    }

    pub fn remove_marked_sections(&mut self) -> Vec<SectionPosKey> {
        let to_remove = std::mem::take(&mut self.to_remove);
        let mut removed = Vec::new();

        for section in to_remove {
            let queued = self.queued_sections.remove(&section);
            let removed_layer = self.updating_section_data.remove_layer(section);
            if self
                .columns_to_retain_queued_data_for
                .contains(&crate::section_get_zero_node(section))
            {
                if let Some(queued) = queued {
                    self.queued_sections.insert(section, queued);
                } else if let Some(removed_layer) = removed_layer {
                    self.queued_sections.insert(section, removed_layer);
                }
            }
            removed.push(section);
        }

        self.updating_section_data.clear_cache();
        self.has_to_remove = false;
        removed
    }

    pub fn accept_queued_sections_for_stored_layers(&mut self) {
        let queued = self
            .queued_sections
            .iter()
            .map(|(section, data_layer)| (*section, data_layer.clone()))
            .collect::<Vec<_>>();

        for (section, data_layer) in queued {
            if self.storing_light_for_section(section)
                && self.updating_section_data.get_layer(section) != Some(&data_layer)
            {
                self.updating_section_data.set_layer(section, data_layer);
                self.changed_sections.insert(section);
            }
        }

        let accepted_sections = self
            .queued_sections
            .keys()
            .copied()
            .filter(|section| self.storing_light_for_section(*section))
            .collect::<Vec<_>>();
        for section in accepted_sections {
            self.queued_sections.remove(&section);
        }
        self.updating_section_data.clear_cache();
    }

    pub fn has_to_remove(&self) -> bool {
        self.has_to_remove
    }

    pub fn retain_data(&mut self, column: SectionPosKey, retain: bool) {
        let column = crate::section_get_zero_node(column);
        if retain {
            self.columns_to_retain_queued_data_for.insert(column);
        } else {
            self.columns_to_retain_queued_data_for.remove(&column);
        }
    }

    pub fn queue_section_data(
        &mut self,
        section: SectionPosKey,
        data_layer: Option<DataLayer>,
        trusted: bool,
    ) {
        if let Some(data_layer) = data_layer {
            self.queued_sections.insert(section, data_layer);
            if !trusted {
                self.untrusted_sections.insert(section);
            }
        } else {
            self.queued_sections.remove(&section);
            self.untrusted_sections.remove(&section);
        }
    }

    pub fn update_section_status(
        &mut self,
        section: SectionPosKey,
        is_empty: bool,
    ) -> Option<SectionEdgeUpdate> {
        let has_data = self.data_section_set.contains(&section);
        if !has_data && !is_empty {
            self.to_mark_data.insert(section);
            return Some(SectionEdgeUpdate {
                source: SectionPosKey::MAX,
                target: section,
                level: LIGHT_AND_DATA,
                is_decrease: true,
            });
        }

        if has_data && is_empty {
            self.to_mark_no_data.insert(section);
            return Some(SectionEdgeUpdate {
                source: SectionPosKey::MAX,
                target: section,
                level: EMPTY,
                is_decrease: false,
            });
        }

        None
    }

    pub fn swap_section_map(&mut self) -> Vec<SectionPosKey> {
        if !self.changed_sections.is_empty() {
            let mut visible = self.updating_section_data.copy();
            visible.disable_cache();
            self.visible_section_data = visible;
            self.changed_sections.clear();
        }

        std::mem::take(&mut self.sections_affected_by_light_updates)
            .into_iter()
            .collect()
    }

    pub fn queued_section_count(&self) -> usize {
        self.queued_sections.len()
    }

    pub fn changed_section_count(&self) -> usize {
        self.changed_sections.len()
    }

    pub fn affected_section_count(&self) -> usize {
        self.sections_affected_by_light_updates.len()
    }

    pub fn untrusted_section_count(&self) -> usize {
        self.untrusted_sections.len()
    }

    fn mark_block_neighbor_sections_affected(&mut self, block: BlockPosKey) {
        for dz in -1..=1 {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    self.sections_affected_by_light_updates
                        .insert(block_to_section_key(block_pos_offset(block, dx, dy, dz)));
                }
            }
        }
    }

    fn mark_section_and_neighbor_sections_affected(&mut self, section: SectionPosKey) {
        for dz in -1..=1 {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    self.sections_affected_by_light_updates
                        .insert(section_offset(section, dx, dy, dz));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block_pos_as_long, section_as_long, section_x, section_y, section_z};

    #[test]
    fn queued_section_data_takes_precedence_over_visible_data() {
        let section = section_as_long(0, 0, 0);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);
        storage.apply_graph_level(section, LIGHT_ONLY);
        storage
            .updating_section_data
            .get_layer_mut(section)
            .unwrap()
            .set(1, 1, 1, 3);
        storage.swap_section_map();

        let mut queued = DataLayer::new();
        queued.set(1, 1, 1, 11);
        storage.queue_section_data(section, Some(queued), false);

        assert_eq!(
            storage.get_data_layer_data(section).unwrap().get(1, 1, 1),
            11
        );
        assert_eq!(storage.untrusted_section_count(), 1);
    }

    #[test]
    fn visible_and_updating_maps_are_separate_until_swap() {
        let section = section_as_long(0, 0, 0);
        let block = block_pos_as_long(1, 2, 3);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);

        storage.apply_graph_level(section, LIGHT_ONLY);
        storage
            .updating_section_data
            .get_layer_mut(section)
            .unwrap()
            .set(1, 2, 3, 4);
        storage.swap_section_map();

        storage.set_stored_level(block, 9);

        assert_eq!(
            storage
                .get_updating_data_layer(section)
                .unwrap()
                .get(1, 2, 3),
            9
        );
        assert_eq!(
            storage
                .get_visible_data_layer(section)
                .unwrap()
                .get(1, 2, 3),
            4
        );

        let affected = storage.swap_section_map();
        assert_eq!(
            storage
                .get_visible_data_layer(section)
                .unwrap()
                .get(1, 2, 3),
            9
        );
        assert!(affected.contains(&section));
    }

    #[test]
    fn setting_stored_level_copy_on_writes_only_once_per_changed_section() {
        let section = section_as_long(0, 0, 0);
        let first = block_pos_as_long(1, 2, 3);
        let second = block_pos_as_long(4, 5, 6);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);

        storage.apply_graph_level(section, LIGHT_ONLY);
        storage.swap_section_map();
        storage.set_stored_level(first, 7);
        storage.set_stored_level(second, 8);

        assert_eq!(storage.changed_section_count(), 1);
        assert_eq!(storage.get_stored_level(first), 7);
        assert_eq!(storage.get_stored_level(second), 8);
    }

    #[test]
    fn section_status_records_source_edge_requests() {
        let section = section_as_long(0, 0, 0);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Sky);

        let add = storage.update_section_status(section, false).unwrap();
        assert_eq!(
            add,
            SectionEdgeUpdate {
                source: SectionPosKey::MAX,
                target: section,
                level: LIGHT_AND_DATA,
                is_decrease: true,
            }
        );
        assert_eq!(storage.level_from_source(section), LIGHT_AND_DATA);

        storage.apply_graph_level(section, LIGHT_AND_DATA);
        let remove = storage.update_section_status(section, true).unwrap();
        assert_eq!(
            remove,
            SectionEdgeUpdate {
                source: SectionPosKey::MAX,
                target: section,
                level: EMPTY,
                is_decrease: false,
            }
        );
        assert_eq!(storage.level_from_source(section), EMPTY);
    }

    #[test]
    fn applying_section_levels_tracks_light_and_data_light_only_and_empty() {
        let section = section_as_long(0, 0, 0);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);

        let add = storage.apply_graph_level(section, LIGHT_ONLY);
        assert!(add.added);
        assert_eq!(storage.section_level(section), LIGHT_ONLY);
        assert!(storage.storing_light_for_section(section));
        assert_eq!(storage.changed_section_count(), 1);
        assert_eq!(storage.affected_section_count(), 27);

        storage.apply_graph_level(section, LIGHT_AND_DATA);
        assert_eq!(storage.section_level(section), LIGHT_AND_DATA);

        let remove = storage.apply_graph_level(section, EMPTY);
        assert!(remove.marked_for_removal);
        assert!(storage.has_to_remove());
        assert_eq!(storage.section_level(section), EMPTY);
        assert_eq!(storage.remove_marked_sections(), vec![section]);
        assert!(!storage.storing_light_for_section(section));
        assert!(!storage.has_to_remove());
    }

    #[test]
    fn retained_removed_section_data_returns_to_queue() {
        let section = section_as_long(2, 3, 4);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);
        storage.apply_graph_level(section, LIGHT_ONLY);
        storage
            .updating_section_data
            .get_layer_mut(section)
            .unwrap()
            .set(1, 1, 1, 6);

        storage.retain_data(section, true);
        storage.apply_graph_level(section, EMPTY);
        storage.remove_marked_sections();

        assert_eq!(storage.queued_section_count(), 1);
        assert_eq!(
            storage.get_data_layer_data(section).unwrap().get(1, 1, 1),
            6
        );
    }

    #[test]
    fn queued_sections_are_accepted_for_existing_storage_layers() {
        let section = section_as_long(0, 0, 0);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);
        storage.apply_graph_level(section, LIGHT_ONLY);

        let mut queued = DataLayer::new();
        queued.set(2, 2, 2, 12);
        storage.queue_section_data(section, Some(queued), true);

        storage.accept_queued_sections_for_stored_layers();

        assert_eq!(storage.queued_section_count(), 0);
        assert_eq!(
            storage
                .get_updating_data_layer(section)
                .unwrap()
                .get(2, 2, 2),
            12
        );
        assert_eq!(storage.changed_section_count(), 1);
    }

    #[test]
    fn affected_sections_expand_across_block_boundaries() {
        let section = section_as_long(0, 0, 0);
        let edge_block = block_pos_as_long(15, 15, 15);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Block);
        storage.apply_graph_level(section, LIGHT_ONLY);
        storage.swap_section_map();

        storage.set_stored_level(edge_block, 10);
        let affected = storage.swap_section_map();

        assert!(affected.contains(&section_as_long(0, 0, 0)));
        assert!(affected.contains(&section_as_long(1, 1, 1)));
    }

    #[test]
    fn section_neighbor_marking_uses_section_coordinates() {
        let section = section_as_long(4, -2, 7);
        let mut storage = LayerLightSectionStorage::new(LightLayer::Sky);

        storage.apply_graph_level(section, LIGHT_ONLY);
        let affected = storage.swap_section_map();

        assert_eq!(affected.len(), 27);
        assert!(affected.contains(&section_as_long(3, -3, 6)));
        assert!(affected.contains(&section_as_long(5, -1, 8)));
        assert!(
            affected.iter().any(|key| {
                section_x(*key) == 4 && section_y(*key) == -2 && section_z(*key) == 7
            })
        );
    }
}
