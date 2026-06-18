use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BlockPosKey, DataLayer, LayerLightSectionStorage, LightLayer, SectionPosKey, block_pos_get_x,
    block_pos_get_y, block_pos_get_z, block_to_section_key, section_as_long, section_get_zero_node,
    section_relative, section_y,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyLightSectionStorage {
    inner: LayerLightSectionStorage,
    columns_with_sky_sources: BTreeSet<SectionPosKey>,
    top_sections: BTreeMap<SectionPosKey, i32>,
    current_lowest_y: i32,
}

impl SkyLightSectionStorage {
    pub fn new() -> Self {
        Self {
            inner: LayerLightSectionStorage::new(LightLayer::Sky),
            columns_with_sky_sources: BTreeSet::new(),
            top_sections: BTreeMap::new(),
            current_lowest_y: i32::MAX,
        }
    }

    pub fn as_layer_storage(&self) -> &LayerLightSectionStorage {
        &self.inner
    }

    pub fn as_layer_storage_mut(&mut self) -> &mut LayerLightSectionStorage {
        &mut self.inner
    }

    pub fn storing_light_for_section(&self, section: SectionPosKey) -> bool {
        self.inner.storing_light_for_section(section)
    }

    pub fn activate_section(&mut self, section: SectionPosKey) -> crate::SectionLevelChange {
        let change = self.inner.apply_graph_level(section, crate::LIGHT_ONLY);
        if change.added || change.unmarked_for_removal {
            self.on_node_added(section);
        }
        change
    }

    pub fn get_data_layer_data(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.inner.get_data_layer_data(section)
    }

    pub fn get_updating_data_layer(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.inner.get_updating_data_layer(section)
    }

    pub fn get_visible_data_layer(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.inner.get_visible_data_layer(section)
    }

    pub fn get_stored_level(&self, block: BlockPosKey) -> u8 {
        self.inner.get_stored_level(block)
    }

    pub fn set_stored_level(&mut self, block: BlockPosKey, level: u8) {
        self.inner.set_stored_level(block, level);
    }

    pub fn swap_section_map(&mut self) -> Vec<SectionPosKey> {
        self.inner.swap_section_map()
    }

    pub fn enable_light_sources(&mut self, column: SectionPosKey, enabled: bool) {
        let column = section_get_zero_node(column);
        if enabled {
            self.columns_with_sky_sources.insert(column);
        } else {
            self.columns_with_sky_sources.remove(&column);
        }
    }

    pub fn light_on_in_section(&self, section: SectionPosKey) -> bool {
        self.columns_with_sky_sources
            .contains(&section_get_zero_node(section))
    }

    pub fn top_section(&self, column: SectionPosKey) -> i32 {
        *self
            .top_sections
            .get(&section_get_zero_node(column))
            .unwrap_or(&self.current_lowest_y)
    }

    pub fn get_light_value(&self, block: BlockPosKey, updating: bool) -> u8 {
        let section = block_to_section_key(block);
        let mut section_y = section_y(section);
        let column = section_get_zero_node(section);
        let top_section = self.top_section(column);

        if top_section != self.current_lowest_y && section_y < top_section {
            let local_x = section_relative(block_pos_get_x(block));
            let local_y = section_relative(block_pos_get_y(block));
            let local_z = section_relative(block_pos_get_z(block));
            loop {
                let probe_section = section_as_long(
                    crate::section_x(section),
                    section_y,
                    crate::section_z(section),
                );
                if let Some(layer) = self.data_layer_for_light_value(probe_section, updating) {
                    return layer.get(local_x, local_y, local_z);
                }
                section_y += 1;
                if section_y >= top_section {
                    return 15;
                }
            }
        }

        if updating && !self.light_on_in_section(section) {
            0
        } else {
            15
        }
    }

    pub fn has_sections_below(&self, section_y: i32) -> bool {
        section_y >= self.current_lowest_y
    }

    pub fn is_above_data(&self, section: SectionPosKey) -> bool {
        let top_section = self.top_section(section);
        top_section == self.current_lowest_y || section_y(section) >= top_section
    }

    fn data_layer_for_light_value(
        &self,
        section: SectionPosKey,
        updating: bool,
    ) -> Option<&DataLayer> {
        if updating {
            self.inner.get_updating_data_layer(section)
        } else {
            self.inner.get_visible_data_layer(section)
        }
    }

    fn on_node_added(&mut self, section: SectionPosKey) {
        let y = section_y(section);
        if self.current_lowest_y > y {
            self.current_lowest_y = y;
        }

        let column = section_get_zero_node(section);
        let top_section = self.top_section(column);
        if top_section < y + 1 {
            self.top_sections.insert(column, y + 1);
        }
    }
}

impl Default for SkyLightSectionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block_pos_as_long, section_as_long};

    #[test]
    fn sky_storage_missing_above_data_reads_full_light_when_column_enabled() {
        let mut storage = SkyLightSectionStorage::new();
        let section = section_as_long(0, 0, 0);
        storage.activate_section(section);
        storage.enable_light_sources(section, true);
        storage.swap_section_map();

        assert_eq!(
            storage.get_light_value(block_pos_as_long(1, 32, 1), false),
            15
        );
    }

    #[test]
    fn sky_storage_updating_missing_column_without_source_reads_dark() {
        let storage = SkyLightSectionStorage::new();

        assert_eq!(storage.get_light_value(block_pos_as_long(1, 2, 1), true), 0);
        assert_eq!(
            storage.get_light_value(block_pos_as_long(1, 2, 1), false),
            15
        );
    }

    #[test]
    fn sky_storage_reads_visible_layer_below_top_section() {
        let section = section_as_long(0, 0, 0);
        let block = block_pos_as_long(1, 2, 3);
        let mut storage = SkyLightSectionStorage::new();

        storage.activate_section(section);
        storage.set_stored_level(block, 12);
        storage.swap_section_map();

        assert_eq!(storage.get_light_value(block, false), 12);
    }
}
