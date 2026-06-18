use std::collections::BTreeMap;

use crate::{DataLayer, SectionPosKey, section_get_zero_node};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataLayerStorageMap {
    map: BTreeMap<SectionPosKey, DataLayer>,
    last_section_keys: [SectionPosKey; 2],
    cache_enabled: bool,
}

pub type BlockDataLayerStorageMap = DataLayerStorageMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyDataLayerStorageMap {
    data: DataLayerStorageMap,
    top_sections: BTreeMap<SectionPosKey, i32>,
    current_lowest_y: i32,
}

impl Default for DataLayerStorageMap {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLayerStorageMap {
    pub fn new() -> Self {
        Self::from_layers(BTreeMap::new())
    }

    pub fn from_layers(map: BTreeMap<SectionPosKey, DataLayer>) -> Self {
        let mut storage = Self {
            map,
            last_section_keys: [SectionPosKey::MAX; 2],
            cache_enabled: true,
        };
        storage.clear_cache();
        storage
    }

    pub fn copy(&self) -> Self {
        Self::from_layers(self.map.clone())
    }

    pub fn copy_data_layer(&mut self, section: SectionPosKey) {
        let copied = self
            .map
            .get(&section)
            .unwrap_or_else(|| panic!("light section {section} has no DataLayer to copy"))
            .clone();
        self.map.insert(section, copied);
        self.clear_cache();
    }

    pub fn has_layer(&self, section: SectionPosKey) -> bool {
        self.map.contains_key(&section)
    }

    pub fn get_layer(&self, section: SectionPosKey) -> Option<&DataLayer> {
        self.map.get(&section)
    }

    pub fn get_layer_cached(&mut self, section: SectionPosKey) -> Option<&DataLayer> {
        if self.cache_enabled && self.last_section_keys.contains(&section) {
            return self.map.get(&section);
        }
        if !self.map.contains_key(&section) {
            return None;
        }
        if self.cache_enabled {
            self.last_section_keys[1] = self.last_section_keys[0];
            self.last_section_keys[0] = section;
        }
        self.map.get(&section)
    }

    pub fn get_layer_mut(&mut self, section: SectionPosKey) -> Option<&mut DataLayer> {
        self.map.get_mut(&section)
    }

    pub fn remove_layer(&mut self, section: SectionPosKey) -> Option<DataLayer> {
        self.map.remove(&section)
    }

    pub fn set_layer(&mut self, section: SectionPosKey, data_layer: DataLayer) {
        self.map.insert(section, data_layer);
    }

    pub fn clear_cache(&mut self) {
        self.last_section_keys = [SectionPosKey::MAX; 2];
    }

    pub fn disable_cache(&mut self) {
        self.cache_enabled = false;
        self.clear_cache();
    }

    pub fn cache_enabled(&self) -> bool {
        self.cache_enabled
    }

    pub fn cached_section_keys(&self) -> [SectionPosKey; 2] {
        self.last_section_keys
    }

    pub fn layer_count(&self) -> usize {
        self.map.len()
    }
}

impl SkyDataLayerStorageMap {
    pub fn new(current_lowest_y: i32) -> Self {
        Self {
            data: DataLayerStorageMap::new(),
            top_sections: BTreeMap::new(),
            current_lowest_y,
        }
    }

    pub fn copy(&self) -> Self {
        Self {
            data: self.data.copy(),
            top_sections: self.top_sections.clone(),
            current_lowest_y: self.current_lowest_y,
        }
    }

    pub fn current_lowest_y(&self) -> i32 {
        self.current_lowest_y
    }

    pub fn set_current_lowest_y(&mut self, current_lowest_y: i32) {
        self.current_lowest_y = current_lowest_y;
    }

    pub fn top_section(&self, column: SectionPosKey) -> i32 {
        *self
            .top_sections
            .get(&section_get_zero_node(column))
            .unwrap_or(&self.current_lowest_y)
    }

    pub fn set_top_section(&mut self, column: SectionPosKey, section_y: i32) {
        self.top_sections
            .insert(section_get_zero_node(column), section_y);
    }

    pub fn remove_top_section(&mut self, column: SectionPosKey) {
        self.top_sections.remove(&section_get_zero_node(column));
    }

    pub fn top_section_count(&self) -> usize {
        self.top_sections.len()
    }

    pub fn data(&self) -> &DataLayerStorageMap {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut DataLayerStorageMap {
        &mut self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_as_long;

    #[test]
    fn storage_map_copy_is_independent_and_preserves_layers() {
        let section = section_as_long(0, 0, 0);
        let mut layer = DataLayer::new();
        layer.set(1, 2, 3, 9);
        let mut storage = DataLayerStorageMap::new();
        storage.set_layer(section, layer);

        let mut copied = storage.copy();
        copied.get_layer_mut(section).unwrap().set(1, 2, 3, 4);

        assert_eq!(storage.get_layer(section).unwrap().get(1, 2, 3), 9);
        assert_eq!(copied.get_layer(section).unwrap().get(1, 2, 3), 4);
    }

    #[test]
    fn storage_map_cache_tracks_recent_successful_gets() {
        let first = section_as_long(1, 0, 0);
        let second = section_as_long(2, 0, 0);
        let missing = section_as_long(3, 0, 0);
        let mut storage = DataLayerStorageMap::new();
        storage.set_layer(first, DataLayer::new());
        storage.set_layer(second, DataLayer::new());

        assert!(storage.get_layer_cached(first).is_some());
        assert_eq!(storage.cached_section_keys(), [first, SectionPosKey::MAX]);
        assert!(storage.get_layer_cached(second).is_some());
        assert_eq!(storage.cached_section_keys(), [second, first]);
        assert!(storage.get_layer_cached(missing).is_none());
        assert_eq!(storage.cached_section_keys(), [second, first]);

        storage.disable_cache();
        assert!(!storage.cache_enabled());
        assert!(storage.get_layer_cached(first).is_some());
        assert_eq!(
            storage.cached_section_keys(),
            [SectionPosKey::MAX, SectionPosKey::MAX]
        );
    }

    #[test]
    fn sky_storage_map_defaults_top_sections_to_current_lowest_y() {
        let column = section_as_long(4, 7, -2);
        let mut sky = SkyDataLayerStorageMap::new(-5);

        assert_eq!(sky.top_section(column), -5);
        sky.set_top_section(column, 12);
        assert_eq!(sky.top_section(section_as_long(4, -3, -2)), 12);

        let copied = sky.copy();
        sky.remove_top_section(column);

        assert_eq!(sky.top_section(column), -5);
        assert_eq!(copied.top_section(column), 12);
    }
}
