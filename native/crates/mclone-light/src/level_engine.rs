use crate::{
    BlockLightEngine, BlockLightWorld, BlockPosKey, SectionPosKey, SkyLightEngine, SkyLightWorld,
};

pub const MAX_SOURCE_LEVEL: u8 = 15;
pub const LIGHT_SECTION_PADDING: i32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelLightEngine<B, S> {
    block_engine: BlockLightEngine<B>,
    sky_engine: SkyLightEngine<S>,
}

impl<B: BlockLightWorld, S: SkyLightWorld> LevelLightEngine<B, S> {
    pub fn new(block_world: B, sky_world: S) -> Self {
        Self {
            block_engine: BlockLightEngine::new(block_world),
            sky_engine: SkyLightEngine::new(sky_world),
        }
    }

    pub fn block_engine(&self) -> &BlockLightEngine<B> {
        &self.block_engine
    }

    pub fn block_engine_mut(&mut self) -> &mut BlockLightEngine<B> {
        &mut self.block_engine
    }

    pub fn sky_engine(&self) -> &SkyLightEngine<S> {
        &self.sky_engine
    }

    pub fn sky_engine_mut(&mut self) -> &mut SkyLightEngine<S> {
        &mut self.sky_engine
    }

    pub fn update_section_status(&mut self, section: SectionPosKey, empty: bool) {
        self.block_engine.update_section_status(section, empty);
        self.sky_engine.update_section_status(section, empty);
    }

    pub fn enable_light_sources(&mut self, section: SectionPosKey, enabled: bool) {
        self.block_engine.enable_light_sources(section, enabled);
        self.sky_engine.enable_light_sources(section, enabled);
    }

    pub fn on_block_emission_increase(&mut self, pos: BlockPosKey, emission: u8) {
        self.block_engine
            .on_block_emission_increase(pos, emission.min(MAX_SOURCE_LEVEL));
    }

    pub fn check_block(&mut self, pos: BlockPosKey) {
        self.block_engine.check_block(pos);
        self.sky_engine.check_block(pos);
    }

    pub fn check_sky_source(&mut self, pos: BlockPosKey) {
        self.sky_engine.check_sky_source(pos);
    }

    pub fn has_light_work(&self) -> bool {
        self.block_engine.has_work() || self.sky_engine.has_work()
    }

    pub fn run_updates(&mut self, budget: usize) -> usize {
        if budget == 0 {
            return 0;
        }
        let block_budget = budget / 2;
        let remaining_block_budget = self.block_engine.run_updates(block_budget);
        let sky_budget = budget - block_budget + remaining_block_budget;
        let remaining_sky_budget = self.sky_engine.run_updates(sky_budget);
        if remaining_block_budget == 0 && remaining_sky_budget > 0 {
            self.block_engine.run_updates(remaining_sky_budget)
        } else {
            remaining_sky_budget
        }
    }

    pub fn run_all_updates(&mut self) {
        while self.has_light_work() {
            let remaining = self.run_updates(16_384);
            if remaining > 0 {
                break;
            }
        }
    }

    pub fn get_raw_brightness(&self, pos: BlockPosKey, sky_darken: u8) -> u8 {
        let sky = self.sky_engine.stored_light(pos).saturating_sub(sky_darken);
        let block = self.block_engine.stored_light(pos);
        sky.max(block)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{block_pos_as_long, section_as_long};

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    struct SyntheticLightWorld {
        opaque: BTreeSet<BlockPosKey>,
        emission: BTreeMap<BlockPosKey, u8>,
    }

    impl SyntheticLightWorld {
        fn set_opaque(&mut self, pos: BlockPosKey) {
            self.opaque.insert(pos);
        }

        fn set_emission(&mut self, pos: BlockPosKey, emission: u8) {
            self.emission.insert(pos, emission);
        }
    }

    impl BlockLightWorld for SyntheticLightWorld {
        fn light_emission(&self, pos: BlockPosKey) -> u8 {
            self.emission.get(&pos).copied().unwrap_or(0)
        }

        fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
            Some(if self.opaque.contains(&pos) { 15 } else { 0 })
        }
    }

    impl SkyLightWorld for SyntheticLightWorld {
        fn light_opacity(&self, pos: BlockPosKey) -> Option<u8> {
            Some(if self.opaque.contains(&pos) { 15 } else { 0 })
        }
    }

    #[test]
    fn level_light_engine_combines_sky_and_block_brightness() {
        let source = block_pos_as_long(1, 1, 1);
        let mut world = SyntheticLightWorld::default();
        world.set_emission(source, 8);
        let mut engine = LevelLightEngine::new(world.clone(), world);
        engine.update_section_status(section_as_long(0, 0, 0), false);
        engine.enable_light_sources(section_as_long(0, 0, 0), true);
        engine.check_sky_source(block_pos_as_long(1, 15, 1));
        engine.on_block_emission_increase(source, 8);

        engine.run_all_updates();

        assert_eq!(engine.get_raw_brightness(block_pos_as_long(1, 1, 1), 0), 15);
        assert_eq!(engine.get_raw_brightness(block_pos_as_long(1, 1, 1), 8), 8);
    }

    #[test]
    fn level_light_engine_check_block_updates_both_layers() {
        let pos = block_pos_as_long(1, 15, 1);
        let mut world = SyntheticLightWorld::default();
        let mut engine = LevelLightEngine::new(world.clone(), world.clone());
        engine.update_section_status(section_as_long(0, 0, 0), false);
        engine.enable_light_sources(section_as_long(0, 0, 0), true);
        engine.check_sky_source(pos);
        engine.run_all_updates();
        assert_eq!(engine.sky_engine().stored_light(pos), 15);

        world.set_opaque(pos);
        *engine.block_engine_mut().world_mut() = world.clone();
        *engine.sky_engine_mut().world_mut() = world;
        engine.check_block(pos);
        engine.run_all_updates();

        assert_eq!(engine.sky_engine().stored_light(pos), 0);
    }
}
