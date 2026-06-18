use crate::{
    BlockPosKey, DataLayer, Direction, DynamicGraphCallbacks, DynamicGraphMinFixedPoint,
    NeighborCheck, SectionPosKey, SkyLightSectionStorage, block_pos_get_x, block_pos_get_y,
    block_pos_get_z, block_pos_offset, block_to_section_key, section_relative,
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
            self.activate_section(section);
        }
    }

    pub fn enable_light_sources(&mut self, column: SectionPosKey, enabled: bool) {
        self.storage.enable_light_sources(column, enabled);
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
            if delegate
                .storage
                .storing_light_for_section(block_to_section_key(target))
            {
                self.graph.check_node(&mut delegate, target);
            }
        }
    }

    pub fn run_updates(&mut self, budget: usize) -> usize {
        let remaining = {
            let mut delegate = SkyLightGraphDelegate {
                storage: &mut self.storage,
                world: &self.world,
            };
            self.graph.run_updates(&mut delegate, budget)
        };
        self.storage.swap_section_map();
        remaining
    }

    pub fn has_work(&self) -> bool {
        self.graph.has_work()
    }

    pub fn run_all_updates(&mut self) {
        while self.graph.has_work() {
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
        Direction::ALL
            .into_iter()
            .filter_map(|direction| {
                let (dx, dy, dz) = direction.step();
                let target = block_pos_offset(node, dx, dy, dz);
                let target_section = block_to_section_key(target);
                (source_section == target_section
                    || self.storage.storing_light_for_section(target_section))
                .then_some(NeighborCheck {
                    source: node,
                    target,
                    level,
                    is_decrease,
                })
            })
            .collect()
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

fn direction_between(source: BlockPosKey, target: BlockPosKey) -> Option<Direction> {
    let dx = block_pos_get_x(target) - block_pos_get_x(source);
    let dy = block_pos_get_y(target) - block_pos_get_y(source);
    let dz = block_pos_get_z(target) - block_pos_get_z(source);
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
    use std::collections::BTreeSet;

    use super::*;
    use crate::{block_pos_as_long, section_as_long};

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    struct SyntheticSkyLightWorld {
        opaque: BTreeSet<BlockPosKey>,
    }

    impl SyntheticSkyLightWorld {
        fn set_opaque(&mut self, pos: BlockPosKey) {
            self.opaque.insert(pos);
        }
    }

    impl SkyLightWorld for SyntheticSkyLightWorld {
        fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
            Some(if self.opaque.contains(&pos) { 15 } else { 0 })
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
    fn sky_light_crosses_active_section_boundary() {
        let mut engine = SkyLightEngine::new(SyntheticSkyLightWorld::default());
        engine.activate_section(section_as_long(0, 1, 0));
        engine.activate_section(section_as_long(0, 0, 0));

        engine.check_sky_source(block_pos_as_long(1, 31, 1));
        engine.run_all_updates();

        assert_eq!(engine.stored_light(block_pos_as_long(1, 31, 1)), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(1, 1, 1)), 15);
    }
}
