use crate::{
    BlockPosKey, DataLayer, Direction, DynamicGraphCallbacks, DynamicGraphMinFixedPoint,
    LIGHT_ONLY, LightEngineRunReport, NeighborCheck, SectionPosKey, SkyLightSectionStorage,
    SkySourceUpdateKind, block_pos_as_long, block_pos_flat_index, block_pos_get_x, block_pos_get_y,
    block_pos_get_z, block_pos_offset, block_to_section_key, section_offset, section_relative,
    section_x, section_y, section_z, timing_elapsed_us, timing_start,
};

pub trait SkyLightWorld {
    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyLightEngine<W> {
    graph: DynamicGraphMinFixedPoint,
    storage: SkyLightSectionStorage,
    world: W,
}

impl<W: SkyLightWorld> SkyLightEngine<W> {
    pub fn new(world: W) -> Self {
        Self {
            graph: DynamicGraphMinFixedPoint::new(16),
            storage: SkyLightSectionStorage::new(),
            world,
        }
    }

    pub fn world(&self) -> &W {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }

    pub fn storage(&self) -> &SkyLightSectionStorage {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut SkyLightSectionStorage {
        &mut self.storage
    }

    pub fn activate_section(&mut self, section: SectionPosKey) {
        self.storage.activate_section(section);
    }

    pub fn update_section_status(&mut self, section: SectionPosKey, empty: bool) {
        if !empty {
            self.storage.activate_data_section(section);
        }
    }

    pub fn enable_light_sources(&mut self, column: SectionPosKey, enabled: bool) {
        self.storage.enable_light_sources(column, enabled);
    }

    pub fn queue_section_data(
        &mut self,
        section: SectionPosKey,
        data_layer: Option<DataLayer>,
        trusted: bool,
    ) {
        self.storage
            .queue_section_data(section, data_layer, trusted);
    }

    pub fn retain_data(&mut self, column: SectionPosKey, retain: bool) {
        self.storage.retain_data(column, retain);
    }

    pub fn accept_queued_section_data(&mut self) {
        self.storage.accept_queued_sections_for_stored_layers();
        self.storage.swap_section_map();
    }

    /// Free a section's sky-light storage (+ its sky-source bookkeeping) on chunk
    /// unload (155 P0 eviction).
    pub fn remove_section(&mut self, section: SectionPosKey) -> bool {
        self.storage.remove_section(section)
    }

    /// Forget an unloaded chunk column's sky-source and top-section state.
    pub fn forget_column(&mut self, column: SectionPosKey) {
        self.storage.forget_column(column);
    }

    pub fn stored_section_count(&self) -> usize {
        self.storage.stored_section_count()
    }

    pub fn check_sky_source(&mut self, pos: BlockPosKey) {
        if !self
            .storage
            .storing_light_for_section(block_to_section_key(pos))
        {
            return;
        }
        let mut delegate = SkyLightGraphDelegate {
            storage: &mut self.storage,
            world: &self.world,
        };
        self.graph.check_edge(&mut delegate, i64::MAX, pos, 0, true);
    }

    pub fn check_block(&mut self, pos: BlockPosKey) {
        let mut delegate = SkyLightGraphDelegate {
            storage: &mut self.storage,
            world: &self.world,
        };
        for target in std::iter::once(pos).chain(Direction::ALL.into_iter().map(|direction| {
            let (dx, dy, dz) = direction.step();
            block_pos_offset(pos, dx, dy, dz)
        })) {
            check_node(&mut self.graph, &mut delegate, target);
        }
    }

    pub fn run_updates(&mut self, budget: usize) -> usize {
        self.run_updates_report(budget).0
    }

    pub fn run_updates_report(&mut self, budget: usize) -> (usize, LightEngineRunReport) {
        let source_start = timing_start();
        let source_update_count = self.apply_source_updates();
        let source_updates_us = timing_elapsed_us(source_start);
        let graph_start = timing_start();
        let remaining = {
            let mut delegate = SkyLightGraphDelegate {
                storage: &mut self.storage,
                world: &self.world,
            };
            self.graph.run_updates_report(&mut delegate, budget)
        };
        let graph_us = timing_elapsed_us(graph_start);
        let swap_start = timing_start();
        let affected_sections = self.storage.swap_section_map().len();
        let storage_swap_us = timing_elapsed_us(swap_start);
        (
            remaining.0,
            LightEngineRunReport {
                graph: remaining.1,
                source_update_count,
                source_updates_us,
                graph_us,
                storage_swap_us,
                affected_sections,
            },
        )
    }

    pub fn has_work(&self) -> bool {
        self.graph.has_work() || self.storage.has_source_inconsistencies()
    }

    pub fn queue_size(&self) -> usize {
        self.graph.queue_size()
    }

    pub fn run_all_updates(&mut self) {
        while self.has_work() {
            let remaining = self.run_updates(16_384);
            if remaining > 0 {
                break;
            }
        }
    }

    pub fn stored_light(&self, pos: BlockPosKey) -> u8 {
        self.storage
            .get_visible_data_layer(block_to_section_key(pos))
            .map_or(0, |layer| {
                layer.get(
                    section_relative(block_pos_get_x(pos)),
                    section_relative(block_pos_get_y(pos)),
                    section_relative(block_pos_get_z(pos)),
                )
            })
    }

    fn apply_source_updates(&mut self) -> usize {
        let updates = self.storage.drain_source_updates();
        if updates.is_empty() {
            return 0;
        }
        let update_count = updates.len();

        let mut delegate = SkyLightGraphDelegate {
            storage: &mut self.storage,
            world: &self.world,
        };
        for update in updates {
            match update.kind {
                SkySourceUpdateKind::Add
                    if delegate.storage.section_level(update.section) == LIGHT_ONLY =>
                {
                    apply_light_only_source_add(&mut self.graph, &mut delegate, update.section);
                }
                SkySourceUpdateKind::Add => {
                    apply_data_source_add(&mut self.graph, &mut delegate, update.section);
                }
                SkySourceUpdateKind::Remove => {
                    apply_source_remove(&mut self.graph, &mut delegate, update.section);
                }
            }
        }
        update_count
    }
}

fn apply_data_source_add<W: SkyLightWorld>(
    graph: &mut DynamicGraphMinFixedPoint,
    delegate: &mut SkyLightGraphDelegate<'_, W>,
    section: SectionPosKey,
) {
    let min_x = section_x(section) * 16;
    let y = section_y(section) * 16 + 15;
    let min_z = section_z(section) * 16;
    for local_z in 0..16 {
        for local_x in 0..16 {
            let pos = block_pos_as_long(min_x + local_x, y, min_z + local_z);
            graph.check_edge(delegate, i64::MAX, pos, 0, true);
        }
    }
}

fn apply_light_only_source_add<W: SkyLightWorld>(
    graph: &mut DynamicGraphMinFixedPoint,
    delegate: &mut SkyLightGraphDelegate<'_, W>,
    section: SectionPosKey,
) {
    graph.remove_if(&*delegate, |node| {
        node != i64::MAX && block_to_section_key(node) == section
    });
    delegate.storage.fill_stored_section(section, 15);

    let min_x = section_x(section) * 16;
    let min_y = section_y(section) * 16;
    let min_z = section_z(section) * 16;

    for direction in Direction::HORIZONTALS {
        let (dx, _, dz) = direction.step();
        let neighbor_section = section_offset(section, dx, 0, dz);
        if !delegate.storage.storing_light_for_section(neighbor_section)
            || delegate.storage.section_has_source(neighbor_section)
        {
            continue;
        }

        for a in 0..16 {
            for b in 0..16 {
                let (source, target) = match direction {
                    Direction::North => (
                        block_pos_as_long(min_x + a, min_y + b, min_z),
                        block_pos_as_long(min_x + a, min_y + b, min_z - 1),
                    ),
                    Direction::South => (
                        block_pos_as_long(min_x + a, min_y + b, min_z + 15),
                        block_pos_as_long(min_x + a, min_y + b, min_z + 16),
                    ),
                    Direction::West => (
                        block_pos_as_long(min_x, min_y + a, min_z + b),
                        block_pos_as_long(min_x - 1, min_y + a, min_z + b),
                    ),
                    Direction::East => (
                        block_pos_as_long(min_x + 15, min_y + a, min_z + b),
                        block_pos_as_long(min_x + 16, min_y + a, min_z + b),
                    ),
                    Direction::Down | Direction::Up => unreachable!("horizontal direction"),
                };
                let level = delegate.compute_level_from_neighbor(source, target, 0);
                graph.check_edge(delegate, source, target, level, true);
            }
        }
    }

    for local_z in 0..16 {
        for local_x in 0..16 {
            let source = block_pos_as_long(min_x + local_x, min_y, min_z + local_z);
            let target = block_pos_as_long(min_x + local_x, min_y - 1, min_z + local_z);
            let level = delegate.compute_level_from_neighbor(source, target, 0);
            graph.check_edge(delegate, source, target, level, true);
        }
    }
}

fn apply_source_remove<W: SkyLightWorld>(
    graph: &mut DynamicGraphMinFixedPoint,
    delegate: &mut SkyLightGraphDelegate<'_, W>,
    section: SectionPosKey,
) {
    let min_x = section_x(section) * 16;
    let y = section_y(section) * 16 + 15;
    let min_z = section_z(section) * 16;
    for local_z in 0..16 {
        for local_x in 0..16 {
            let pos = block_pos_as_long(min_x + local_x, y, min_z + local_z);
            graph.check_edge(delegate, i64::MAX, pos, 15, false);
        }
    }
}

fn check_node<W: SkyLightWorld>(
    graph: &mut DynamicGraphMinFixedPoint,
    delegate: &mut SkyLightGraphDelegate<W>,
    mut pos: BlockPosKey,
) {
    let mut section = block_to_section_key(pos);
    if delegate.storage.storing_light_for_section(section) {
        graph.check_node(delegate, pos);
        return;
    }

    pos = block_pos_flat_index(pos);
    while !delegate.storage.storing_light_for_section(section)
        && !delegate.storage.is_above_data(section)
    {
        pos = block_pos_offset(pos, 0, 16, 0);
        section = section_offset(section, 0, 1, 0);
    }

    if delegate.storage.storing_light_for_section(section) {
        graph.check_node(delegate, pos);
    }
}

struct SkyLightGraphDelegate<'a, W> {
    storage: &'a mut SkyLightSectionStorage,
    world: &'a W,
}

impl<W: SkyLightWorld> SkyLightGraphDelegate<'_, W> {
    fn opacity(&self, pos: BlockPosKey) -> u8 {
        self.world.light_opacity(pos).unwrap_or(16)
    }

    fn level_from_layer(&self, layer: &DataLayer, pos: BlockPosKey) -> u8 {
        15 - layer.get(
            section_relative(block_pos_get_x(pos)),
            section_relative(block_pos_get_y(pos)),
            section_relative(block_pos_get_z(pos)),
        )
    }
}

impl<W: SkyLightWorld> DynamicGraphCallbacks for SkyLightGraphDelegate<'_, W> {
    fn is_source(&self, node: i64) -> bool {
        node == i64::MAX
    }

    fn get_computed_level(&self, target: i64, excluded_source: i64, candidate: u8) -> u8 {
        let mut level = candidate;
        let target_section = block_to_section_key(target);
        let target_layer = self.storage.get_updating_data_layer(target_section);

        for direction in Direction::ALL {
            let (dx, dy, dz) = direction.step();
            let source = block_pos_offset(target, dx, dy, dz);
            if source == excluded_source {
                continue;
            }
            let source_section = block_to_section_key(source);
            let source_layer = if target_section == source_section {
                target_layer
            } else {
                self.storage.get_updating_data_layer(source_section)
            };
            let source_level = if let Some(source_layer) = source_layer {
                self.level_from_layer(source_layer, source)
            } else {
                if direction == Direction::Down {
                    continue;
                }
                15 - self.storage.get_light_value(source, true)
            };

            let computed = self.compute_level_from_neighbor(source, target, source_level);
            if level > computed {
                level = computed;
            }
            if level == 0 {
                return level;
            }
        }

        level
    }

    fn neighbor_checks_after_update(
        &mut self,
        node: i64,
        level: u8,
        is_decrease: bool,
    ) -> Vec<NeighborCheck> {
        let source_section = block_to_section_key(node);
        let y = block_pos_get_y(node);
        let section_y = section_y(source_section);
        let skip_sections = if section_relative(y) == 0 {
            let mut skipped = 0;
            while !self.storage.storing_light_for_section(section_offset(
                source_section,
                0,
                -skipped - 1,
                0,
            )) && self.storage.has_sections_below(section_y - skipped - 1)
            {
                skipped += 1;
            }
            skipped
        } else {
            0
        };

        let mut checks = Vec::new();

        let down = block_pos_offset(node, 0, -1 - skip_sections * 16, 0);
        self.push_neighbor_check(&mut checks, node, node, down, level, is_decrease);

        let up = block_pos_offset(node, 0, 1, 0);
        self.push_neighbor_check(&mut checks, node, node, up, level, is_decrease);

        for direction in Direction::HORIZONTALS {
            let (dx, _, dz) = direction.step();
            for vertical_skip in 0..=skip_sections * 16 {
                let target = block_pos_offset(node, dx, -vertical_skip, dz);
                let target_section = block_to_section_key(target);
                if source_section == target_section {
                    checks.push(NeighborCheck {
                        source: node,
                        target,
                        level,
                        is_decrease,
                    });
                    break;
                }

                if self.storage.storing_light_for_section(target_section) {
                    checks.push(NeighborCheck {
                        source: block_pos_offset(node, 0, -vertical_skip, 0),
                        target,
                        level,
                        is_decrease,
                    });
                }
            }
        }

        checks
    }

    fn get_level(&self, node: i64) -> u8 {
        if node == i64::MAX {
            0
        } else if self
            .storage
            .storing_light_for_section(block_to_section_key(node))
        {
            15 - self.storage.get_stored_level(node)
        } else {
            15
        }
    }

    fn set_level(&mut self, node: i64, level: u8) {
        if node != i64::MAX
            && self
                .storage
                .storing_light_for_section(block_to_section_key(node))
        {
            self.storage.set_stored_level(node, 15 - level.min(15));
        }
    }

    fn compute_level_from_neighbor(&self, source: i64, target: i64, source_level: u8) -> u8 {
        if target == i64::MAX || source == i64::MAX {
            15
        } else if source_level >= 15 {
            source_level
        } else {
            let Some(direction) = direction_between(source, target) else {
                return 15;
            };
            let opacity = self.opacity(target);
            if opacity >= 15 {
                15
            } else if self
                .world
                .light_opacity(source)
                .is_some_and(|source_opacity| source_opacity >= 15)
            {
                15
            } else {
                let direct_down = direction == Direction::Down
                    && block_pos_get_x(source) == block_pos_get_x(target)
                    && block_pos_get_z(source) == block_pos_get_z(target);
                if direct_down && source_level == 0 && opacity == 0 {
                    0
                } else {
                    // Shape-based face occlusion belongs in the real block-state bridge.
                    source_level.saturating_add(opacity.max(1)).min(15)
                }
            }
        }
    }
}

impl<W: SkyLightWorld> SkyLightGraphDelegate<'_, W> {
    fn push_neighbor_check(
        &self,
        checks: &mut Vec<NeighborCheck>,
        source: BlockPosKey,
        source_section_node: BlockPosKey,
        target: BlockPosKey,
        level: u8,
        is_decrease: bool,
    ) {
        let source_section = block_to_section_key(source_section_node);
        let target_section = block_to_section_key(target);
        if source_section == target_section
            || self.storage.storing_light_for_section(target_section)
        {
            checks.push(NeighborCheck {
                source,
                target,
                level,
                is_decrease,
            });
        }
    }
}

fn direction_between(source: BlockPosKey, target: BlockPosKey) -> Option<Direction> {
    let dx = (block_pos_get_x(target) - block_pos_get_x(source)).signum();
    let dy = (block_pos_get_y(target) - block_pos_get_y(source)).signum();
    let dz = (block_pos_get_z(target) - block_pos_get_z(source)).signum();
    match (dx, dy, dz) {
        (0, -1, 0) => Some(Direction::Down),
        (0, 1, 0) => Some(Direction::Up),
        (0, 0, -1) => Some(Direction::North),
        (0, 0, 1) => Some(Direction::South),
        (-1, 0, 0) => Some(Direction::West),
        (1, 0, 0) => Some(Direction::East),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{block_pos_as_long, section_as_long};

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    struct SyntheticSkyLightWorld {
        opacity: BTreeMap<BlockPosKey, u8>,
    }

    impl SyntheticSkyLightWorld {
        fn set_opaque(&mut self, pos: BlockPosKey) {
            self.opacity.insert(pos, 15);
        }

        fn set_opacity(&mut self, pos: BlockPosKey, opacity: u8) {
            self.opacity.insert(pos, opacity);
        }
    }

    impl SkyLightWorld for SyntheticSkyLightWorld {
        fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
            Some(self.opacity.get(&pos).copied().unwrap_or(0))
        }
    }

    fn active_engine() -> SkyLightEngine<SyntheticSkyLightWorld> {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.activate_section(section_as_long(0, 0, 0));
        engine
    }

    #[test]
    fn sky_light_falls_straight_down_without_decay() {
        let mut engine = active_engine();

        engine.check_sky_source(block_pos_as_long(1, 15, 1));
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(1, 15, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 8, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 0, 1)), 15);
    }

    #[test]
    fn sky_light_sideways_spread_decays_under_overhang() {
        let mut engine = active_engine();
        for y in 0..16 {
            for z in 0..16 {
                engine.world_mut().set_opaque(block_pos_as_long(2, y, z));
            }
        }

        engine.check_sky_source(block_pos_as_long(1, 15, 1));
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(1, 1, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(3, 1, 1)), 0);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 1, 2)), 14);
    }

    #[test]
    fn sky_light_decays_through_opacity_one_blocks() {
        let mut engine = active_engine();
        engine
            .world_mut()
            .set_opacity(block_pos_as_long(1, 12, 1), 1);

        engine.check_sky_source(block_pos_as_long(1, 15, 1));
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(1, 13, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 12, 1)), 14);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 11, 1)), 13);
    }

    #[test]
    fn sky_light_data_source_section_rechecks_opacity_one_blocks() {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.update_section_status(section_as_long(0, 0, 0), false);
        for z in 1..=3 {
            for x in 1..=3 {
                engine
                    .world_mut()
                    .set_opacity(block_pos_as_long(x, 12, z), 1);
            }
        }
        engine.world_mut().set_opaque(block_pos_as_long(4, 12, 2));

        engine.enable_light_sources(section_as_long(0, 0, 0), true);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(2, 13, 2)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(2, 12, 2)), 14);
        assert_eq!(engine.stored_light(block_pos_as_long(2, 11, 2)), 13);
        assert_eq!(engine.stored_light(block_pos_as_long(4, 12, 2)), 0);
    }

    #[test]
    fn sky_light_light_only_source_section_fills_full_bright() {
        let mut engine = active_engine();
        engine.world_mut().set_opaque(block_pos_as_long(2, 12, 1));

        engine.enable_light_sources(section_as_long(0, 0, 0), true);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(2, 12, 1)), 15);
    }

    #[test]
    fn sky_light_data_source_above_lower_section_rechecks_opacity_one_blocks() {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.update_section_status(section_as_long(0, 1, 0), false);
        engine.update_section_status(section_as_long(0, 0, 0), false);
        for z in 1..=3 {
            for x in 1..=3 {
                engine
                    .world_mut()
                    .set_opacity(block_pos_as_long(x, 12, z), 1);
            }
        }

        engine.enable_light_sources(section_as_long(0, 0, 0), true);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(2, 13, 2)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(2, 12, 2)), 14);
        assert_eq!(engine.stored_light(block_pos_as_long(2, 11, 2)), 13);
    }

    #[test]
    fn sky_light_data_source_does_not_leak_through_opaque_source_face() {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.update_section_status(section_as_long(0, 0, 0), false);
        for z in 0..16 {
            for x in 0..16 {
                engine.world_mut().set_opaque(block_pos_as_long(x, 15, z));
            }
        }

        engine.enable_light_sources(section_as_long(0, 0, 0), true);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(8, 15, 8)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(8, 14, 8)), 0);
    }

    #[test]
    fn sky_light_crosses_active_section_boundary() {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.activate_section(section_as_long(0, 1, 0));
        engine.activate_section(section_as_long(0, 0, 0));

        engine.check_sky_source(block_pos_as_long(1, 31, 1));
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(1, 31, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 1, 1)), 15);
    }

    #[test]
    fn sky_light_skips_missing_vertical_sections_between_active_sections() {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.activate_section(section_as_long(0, 2, 0));
        engine.activate_section(section_as_long(0, 0, 0));

        engine.check_sky_source(block_pos_as_long(1, 47, 1));
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(1, 32, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 15, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 1, 1)), 15);
    }
}
