use crate::{
    BlockPosKey, DataLayer, Direction, DynamicGraphCallbacks, DynamicGraphMinFixedPoint,
    LayerLightSectionStorage, LightLayer, NeighborCheck, SectionPosKey, block_pos_get_x,
    block_pos_get_y, block_pos_get_z, block_pos_offset, block_to_section_key, section_relative,
};

pub trait BlockLightWorld {
    fn light_emission(&self, pos: BlockPosKey) -> u8;
    fn light_opacity(&self, pos: BlockPosKey) -> Option<u8>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockLightEngine<W> {
    graph: DynamicGraphMinFixedPoint,
    storage: LayerLightSectionStorage,
    world: W,
}

impl<W: BlockLightWorld> BlockLightEngine<W> {
    pub fn new(world: W) -> Self {
        Self {
            graph: DynamicGraphMinFixedPoint::new(16),
            storage: LayerLightSectionStorage::new(LightLayer::Block),
            world,
        }
    }

    pub fn world(&self) -> &W {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }

    pub fn storage(&self) -> &LayerLightSectionStorage {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut LayerLightSectionStorage {
        &mut self.storage
    }

    pub fn activate_section(&mut self, section: SectionPosKey) {
        self.storage.apply_graph_level(section, crate::LIGHT_ONLY);
    }

    pub fn check_block(&mut self, pos: BlockPosKey) {
        let mut delegate = BlockLightGraphDelegate {
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

    pub fn on_block_emission_increase(&mut self, pos: BlockPosKey, emission: u8) {
        if emission == 0
            || !self
                .storage
                .storing_light_for_section(block_to_section_key(pos))
        {
            return;
        }
        let mut delegate = BlockLightGraphDelegate {
            storage: &mut self.storage,
            world: &self.world,
        };
        self.graph.check_edge(
            &mut delegate,
            i64::MAX,
            pos,
            15_u8.saturating_sub(emission.min(15)),
            true,
        );
    }

    pub fn run_updates(&mut self, budget: usize) -> usize {
        let remaining = {
            let mut delegate = BlockLightGraphDelegate {
                storage: &mut self.storage,
                world: &self.world,
            };
            self.graph.run_updates(&mut delegate, budget)
        };
        self.storage.swap_section_map();
        remaining
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

struct BlockLightGraphDelegate<'a, W> {
    storage: &'a mut LayerLightSectionStorage,
    world: &'a W,
}

impl<W: BlockLightWorld> BlockLightGraphDelegate<'_, W> {
    fn light_emission(&self, pos: BlockPosKey) -> u8 {
        self.world.light_emission(pos).min(15)
    }

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

impl<W: BlockLightWorld> DynamicGraphCallbacks for BlockLightGraphDelegate<'_, W> {
    fn is_source(&self, node: i64) -> bool {
        node == i64::MAX
    }

    fn get_computed_level(&self, target: i64, excluded_source: i64, candidate: u8) -> u8 {
        let mut level = candidate;
        if excluded_source != i64::MAX {
            let source_level = self.compute_level_from_neighbor(i64::MAX, target, 0);
            if level > source_level {
                level = source_level;
            }
            if level == 0 {
                return level;
            }
        }

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
            if let Some(source_layer) = source_layer {
                let source_level = self.level_from_layer(source_layer, source);
                let computed = self.compute_level_from_neighbor(source, target, source_level);
                if level > computed {
                    level = computed;
                }
                if level == 0 {
                    return level;
                }
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
        if target == i64::MAX {
            15
        } else if source == i64::MAX {
            source_level.saturating_add(15 - self.light_emission(target))
        } else if source_level >= 15 {
            source_level
        } else if direction_between(source, target).is_none() {
            15
        } else {
            let opacity = self.opacity(target);
            if opacity >= 15 {
                15
            } else {
                // Shape-based face occlusion belongs in the real block-state bridge.
                source_level.saturating_add(opacity.max(1)).min(15)
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
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{block_pos_as_long, section_as_long};

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    struct SyntheticBlockLightWorld {
        emissions: BTreeMap<BlockPosKey, u8>,
        opaque: BTreeSet<BlockPosKey>,
    }

    impl SyntheticBlockLightWorld {
        fn set_emission(&mut self, pos: BlockPosKey, emission: u8) {
            if emission == 0 {
                self.emissions.remove(&pos);
            } else {
                self.emissions.insert(pos, emission);
            }
        }

        fn set_opaque(&mut self, pos: BlockPosKey) {
            self.opaque.insert(pos);
        }
    }

    impl BlockLightWorld for SyntheticBlockLightWorld {
        fn light_emission(&self, pos: BlockPosKey) -> u8 {
            self.emissions.get(&pos).copied().unwrap_or(0)
        }

        fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
            Some(if self.opaque.contains(&pos) { 15 } else { 0 })
        }
    }

    fn active_engine() -> BlockLightEngine<SyntheticBlockLightWorld> {
        let mut engine = BlockLightEngine::new(SyntheticBlockLightWorld::default());
        engine.activate_section(section_as_long(0, 0, 0));
        engine
    }

    #[test]
    fn block_light_brightens_from_emitting_source() {
        let source = block_pos_as_long(1, 1, 1);
        let mut engine = active_engine();
        engine.world_mut().set_emission(source, 15);

        engine.on_block_emission_increase(source, 15);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(source), 15);
        assert_eq!(engine.stored_light(block_pos_as_long(2, 1, 1)), 14);
        assert_eq!(engine.stored_light(block_pos_as_long(3, 1, 1)), 13);
    }

    #[test]
    fn opaque_blocks_stop_block_light() {
        let source = block_pos_as_long(1, 1, 1);
        let opaque = block_pos_as_long(2, 1, 1);
        let mut engine = active_engine();
        engine.world_mut().set_emission(source, 15);
        for y in 0..16 {
            for z in 0..16 {
                engine.world_mut().set_opaque(block_pos_as_long(2, y, z));
            }
        }

        engine.on_block_emission_increase(source, 15);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(source), 15);
        assert_eq!(engine.stored_light(opaque), 0);
        assert_eq!(engine.stored_light(block_pos_as_long(3, 1, 1)), 0);
    }

    #[test]
    fn block_light_darkens_after_source_removal_and_repairs_from_other_source() {
        let first = block_pos_as_long(1, 1, 1);
        let second = block_pos_as_long(5, 1, 1);
        let mut engine = active_engine();
        engine.world_mut().set_emission(first, 15);
        engine.world_mut().set_emission(second, 15);

        engine.on_block_emission_increase(first, 15);
        engine.on_block_emission_increase(second, 15);
        engine.run_all_updates();
        assert_eq!(engine.stored_light(block_pos_as_long(2, 1, 1)), 14);

        engine.world_mut().set_emission(first, 0);
        engine.check_block(first);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(first), 11);
        assert_eq!(engine.stored_light(block_pos_as_long(2, 1, 1)), 12);
        assert_eq!(engine.stored_light(block_pos_as_long(4, 1, 1)), 14);
        assert_eq!(engine.stored_light(second), 15);
    }

    #[test]
    fn block_light_crosses_active_section_boundary() {
        let source = block_pos_as_long(15, 1, 1);
        let across = block_pos_as_long(16, 1, 1);
        let mut engine = BlockLightEngine::new(SyntheticBlockLightWorld::default());
        engine.activate_section(section_as_long(0, 0, 0));
        engine.activate_section(section_as_long(1, 0, 0));
        engine.world_mut().set_emission(source, 15);

        engine.on_block_emission_increase(source, 15);
        engine.run_all_updates();

        assert_eq!(engine.stored_light(source), 15);
        assert_eq!(engine.stored_light(across), 14);
        assert_eq!(engine.stored_light(block_pos_as_long(17, 1, 1)), 13);
    }
}
