use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BlockPosKey, DataLayer, LayerLightSectionStorage, LightLayer, SectionPosKey, block_pos_get_x,
    block_pos_get_y, block_pos_get_z, block_to_section_key, section_as_long, section_get_zero_node,
    section_relative, section_x, section_y, section_z,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkySourceUpdateKind {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkySourceUpdate {
    pub section: SectionPosKey,
    pub kind: SkySourceUpdateKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyLightSectionStorage {
    inner: LayerLightSectionStorage,
    sections_with_sources: BTreeSet<SectionPosKey>,
    sections_to_add_sources_to: BTreeSet<SectionPosKey>,
    sections_to_remove_sources_from: BTreeSet<SectionPosKey>,
    columns_with_sky_sources: BTreeSet<SectionPosKey>,
    top_sections: BTreeMap<SectionPosKey, i32>,
    current_lowest_y: i32,
}

impl SkyLightSectionStorage {
    pub fn new() -> Self {
        Self {
            inner: LayerLightSectionStorage::new(LightLayer::Sky),
            sections_with_sources: BTreeSet::new(),
            sections_to_add_sources_to: BTreeSet::new(),
            sections_to_remove_sources_from: BTreeSet::new(),
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

    pub fn activate_data_section(&mut self, section: SectionPosKey) -> crate::SectionLevelChange {
        let change = self.inner.apply_graph_level(section, crate::LIGHT_AND_DATA);
        if change.added || change.unmarked_for_removal {
            self.on_node_added(section);
        }
        change
    }

    pub fn section_level(&self, section: SectionPosKey) -> u8 {
        self.inner.section_level(section)
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

    pub fn fill_stored_section(&mut self, section: SectionPosKey, level: u8) {
        self.inner.fill_stored_section(section, level);
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

    /// Free a sky section's storage on chunk unload. Delegates the `DataLayer`
    /// removal to the shared layer storage and additionally forgets this
    /// section's sky-source bookkeeping (`sections_with_sources` and the pending
    /// add/remove queues), which have no block-light analogue. Column-wide state
    /// (`columns_with_sky_sources`, `top_sections`) is cleared once per column by
    /// [`forget_column`](Self::forget_column).
    pub fn remove_section(&mut self, section: SectionPosKey) -> bool {
        let was_storing = self.inner.remove_section(section);
        self.sections_with_sources.remove(&section);
        self.sections_to_add_sources_to.remove(&section);
        self.sections_to_remove_sources_from.remove(&section);
        was_storing
    }

    /// Forget a whole column's sky-source and top-section state on unload,
    /// mirroring vanilla `updateChunkStatus`'s `enableLightSources(pos, false)` +
    /// `retainData(pos, false)`. `current_lowest_y` is intentionally left at its
    /// historical minimum: it is a floor sentinel (`top_section` default,
    /// `has_sections_below`), and a value at-or-below the true loaded minimum is
    /// conservative — it can only cost a few extra vertical-skip probe iterations
    /// for a still-loaded chunk that is later relit, never a wrong light value,
    /// and recomputing it would require scanning every remaining section.
    pub fn forget_column(&mut self, column: SectionPosKey) {
        let column = section_get_zero_node(column);
        self.columns_with_sky_sources.remove(&column);
        self.top_sections.remove(&column);
        self.inner.forget_retained_column(column);
    }

    pub fn stored_section_count(&self) -> usize {
        self.inner.stored_section_count()
    }

    pub fn enable_light_sources(&mut self, column: SectionPosKey, enabled: bool) {
        let column = section_get_zero_node(column);
        if enabled && self.columns_with_sky_sources.insert(column) {
            let top_section = self.top_section(column);
            if top_section != self.current_lowest_y {
                self.queue_add_source(section_as_long(
                    section_x(column),
                    top_section - 1,
                    section_z(column),
                ));
            }
        } else if !enabled && self.columns_with_sky_sources.remove(&column) {
            let sections = self
                .sections_with_sources
                .iter()
                .copied()
                .chain(self.sections_to_add_sources_to.iter().copied())
                .filter(|section| section_get_zero_node(*section) == column)
                .collect::<Vec<_>>();
            for section in sections {
                self.queue_remove_source(section);
            }
        }
    }

    pub fn has_source_inconsistencies(&self) -> bool {
        !self.sections_to_add_sources_to.is_empty()
            || !self.sections_to_remove_sources_from.is_empty()
    }

    pub fn drain_source_updates(&mut self) -> Vec<SkySourceUpdate> {
        let mut updates = Vec::new();

        let sections_to_add = std::mem::take(&mut self.sections_to_add_sources_to);
        for section in sections_to_add {
            if self.storing_light_for_section(section)
                && !self.sections_to_remove_sources_from.contains(&section)
                && self.sections_with_sources.insert(section)
            {
                updates.push(SkySourceUpdate {
                    section,
                    kind: SkySourceUpdateKind::Add,
                });
            }
        }

        let sections_to_remove = std::mem::take(&mut self.sections_to_remove_sources_from);
        for section in sections_to_remove {
            if self.sections_with_sources.remove(&section)
                && self.storing_light_for_section(section)
            {
                updates.push(SkySourceUpdate {
                    section,
                    kind: SkySourceUpdateKind::Remove,
                });
            }
        }

        updates
    }

    pub fn light_on_in_section(&self, section: SectionPosKey) -> bool {
        self.columns_with_sky_sources
            .contains(&section_get_zero_node(section))
    }

    pub fn section_has_source(&self, section: SectionPosKey) -> bool {
        self.sections_with_sources.contains(&section)
            || self.sections_to_add_sources_to.contains(&section)
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
            if self.columns_with_sky_sources.contains(&column) {
                self.queue_add_source(section);
                if top_section > self.current_lowest_y {
                    self.queue_remove_source(section_as_long(
                        section_x(section),
                        top_section - 1,
                        section_z(section),
                    ));
                }
            }
        }
    }

    fn queue_add_source(&mut self, section: SectionPosKey) {
        self.sections_to_add_sources_to.insert(section);
        self.sections_to_remove_sources_from.remove(&section);
    }

    fn queue_remove_source(&mut self, section: SectionPosKey) {
        self.sections_to_remove_sources_from.insert(section);
        self.sections_to_add_sources_to.remove(&section);
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

    #[test]
    fn sky_storage_queues_source_section_when_column_is_enabled() {
        let section = section_as_long(0, 2, 0);
        let mut storage = SkyLightSectionStorage::new();

        storage.activate_section(section);
        storage.enable_light_sources(section, true);

        assert_eq!(
            storage.drain_source_updates(),
            vec![SkySourceUpdate {
                section,
                kind: SkySourceUpdateKind::Add,
            }]
        );
        assert!(!storage.has_source_inconsistencies());
    }

    #[test]
    fn sky_storage_moves_source_to_higher_top_section() {
        let lower = section_as_long(0, 0, 0);
        let higher = section_as_long(0, 1, 0);
        let mut storage = SkyLightSectionStorage::new();

        storage.activate_section(lower);
        storage.enable_light_sources(lower, true);
        assert_eq!(
            storage.drain_source_updates(),
            vec![SkySourceUpdate {
                section: lower,
                kind: SkySourceUpdateKind::Add,
            }]
        );

        storage.activate_section(higher);

        assert_eq!(
            storage.drain_source_updates(),
            vec![
                SkySourceUpdate {
                    section: higher,
                    kind: SkySourceUpdateKind::Add,
                },
                SkySourceUpdate {
                    section: lower,
                    kind: SkySourceUpdateKind::Remove,
                },
            ]
        );
    }
}
