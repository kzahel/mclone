use crate::{BlockPosKey, DataLayer, LayerLightSectionStorage, LightLayer, SectionPosKey};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockLightSectionStorage {
    inner: LayerLightSectionStorage,
}

impl BlockLightSectionStorage {
    pub fn new() -> Self {
        Self {
            inner: LayerLightSectionStorage::new(LightLayer::Block),
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

    pub fn apply_graph_level(
        &mut self,
        section: SectionPosKey,
        level: u8,
    ) -> crate::SectionLevelChange {
        self.inner.apply_graph_level(section, level)
    }

    pub fn swap_section_map(&mut self) -> Vec<SectionPosKey> {
        self.inner.swap_section_map()
    }

    pub fn queue_section_data(
        &mut self,
        section: SectionPosKey,
        data_layer: Option<DataLayer>,
        trusted: bool,
    ) {
        self.inner.queue_section_data(section, data_layer, trusted);
    }

    pub fn retain_data(&mut self, column: SectionPosKey, retain: bool) {
        self.inner.retain_data(column, retain);
    }

    pub fn accept_queued_sections_for_stored_layers(&mut self) {
        self.inner.accept_queued_sections_for_stored_layers();
    }

    pub fn get_light_value(&self, block: BlockPosKey) -> u8 {
        self.inner
            .get_data_layer_data(crate::block_to_section_key(block))
            .map_or(0, |layer| {
                layer.get(
                    crate::section_relative(crate::block_pos_get_x(block)),
                    crate::section_relative(crate::block_pos_get_y(block)),
                    crate::section_relative(crate::block_pos_get_z(block)),
                )
            })
    }
}

impl Default for BlockLightSectionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LIGHT_ONLY, block_pos_as_long, section_as_long};

    #[test]
    fn block_storage_defaults_missing_light_to_zero() {
        let storage = BlockLightSectionStorage::new();

        assert_eq!(storage.get_light_value(block_pos_as_long(1, 2, 3)), 0);
    }

    #[test]
    fn block_storage_reads_visible_data_layer_values() {
        let section = section_as_long(0, 0, 0);
        let block = block_pos_as_long(1, 2, 3);
        let mut storage = BlockLightSectionStorage::new();

        storage.apply_graph_level(section, LIGHT_ONLY);
        storage.set_stored_level(block, 12);
        storage.swap_section_map();

        assert_eq!(storage.get_light_value(block), 12);
    }
}
