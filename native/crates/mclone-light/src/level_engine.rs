#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::{
    BlockLightEngine, BlockLightWorld, BlockPosKey, DataLayer, DynamicGraphRunReport, LightLayer,
    SectionPosKey, SkyLightEngine, SkyLightWorld,
};

pub const MAX_SOURCE_LEVEL: u8 = 15;
pub const LIGHT_SECTION_PADDING: i32 = 1;

#[cfg(not(target_arch = "wasm32"))]
type TimingSample = Instant;

#[cfg(target_arch = "wasm32")]
type TimingSample = ();

#[cfg(not(target_arch = "wasm32"))]
fn timing_start() -> Option<TimingSample> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn timing_start() -> Option<TimingSample> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn timing_elapsed_us(start: Option<TimingSample>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
fn timing_elapsed_us(_start: Option<TimingSample>) -> u128 {
    0
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LightLayerRunReport {
    pub calls: usize,
    pub processed_nodes: usize,
    pub queue_before: usize,
    pub queue_after: usize,
    pub run_updates_us: u128,
}

impl LightLayerRunReport {
    fn record_call(&mut self, graph: DynamicGraphRunReport, run_updates_us: u128) {
        self.queue_before = self.queue_before.max(graph.queue_before);
        self.calls += 1;
        self.processed_nodes += graph.processed_nodes;
        self.queue_after = graph.queue_after;
        self.run_updates_us += run_updates_us;
    }

    fn add_assign(&mut self, other: Self) {
        self.queue_before = self.queue_before.max(other.queue_before);
        self.calls += other.calls;
        self.processed_nodes += other.processed_nodes;
        self.queue_after = other.queue_after;
        self.run_updates_us += other.run_updates_us;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LevelLightRunReport {
    pub iterations: usize,
    pub block: LightLayerRunReport,
    pub sky: LightLayerRunReport,
}

impl LevelLightRunReport {
    fn add_assign(&mut self, other: Self) {
        self.iterations += other.iterations;
        self.block.add_assign(other.block);
        self.sky.add_assign(other.sky);
    }
}

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

    pub fn queue_section_data(
        &mut self,
        layer: LightLayer,
        section: SectionPosKey,
        data_layer: Option<DataLayer>,
        trusted: bool,
    ) {
        match layer {
            LightLayer::Block => self
                .block_engine
                .queue_section_data(section, data_layer, trusted),
            LightLayer::Sky => self
                .sky_engine
                .queue_section_data(section, data_layer, trusted),
        }
    }

    pub fn retain_data(&mut self, column: SectionPosKey, retain: bool) {
        self.block_engine.retain_data(column, retain);
        self.sky_engine.retain_data(column, retain);
    }

    pub fn accept_queued_section_data(&mut self) {
        self.block_engine.accept_queued_section_data();
        self.sky_engine.accept_queued_section_data();
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
        self.run_updates_report(budget).0
    }

    pub fn run_updates_report(&mut self, budget: usize) -> (usize, LevelLightRunReport) {
        let mut report = LevelLightRunReport::default();
        if budget == 0 {
            return (0, report);
        }
        report.iterations = 1;
        let block_budget = budget / 2;
        let start = timing_start();
        let (remaining_block_budget, block_report) =
            self.block_engine.run_updates_report(block_budget);
        report
            .block
            .record_call(block_report, timing_elapsed_us(start));
        let sky_budget = budget - block_budget + remaining_block_budget;
        let start = timing_start();
        let (remaining_sky_budget, sky_report) = self.sky_engine.run_updates_report(sky_budget);
        report.sky.record_call(sky_report, timing_elapsed_us(start));
        if remaining_block_budget == 0 && remaining_sky_budget > 0 {
            let start = timing_start();
            let (remaining, block_report) =
                self.block_engine.run_updates_report(remaining_sky_budget);
            report
                .block
                .record_call(block_report, timing_elapsed_us(start));
            (remaining, report)
        } else {
            (remaining_sky_budget, report)
        }
    }

    pub fn run_all_updates(&mut self) {
        self.run_all_updates_report();
    }

    pub fn run_all_updates_report(&mut self) -> LevelLightRunReport {
        let mut report = LevelLightRunReport::default();
        while self.has_light_work() {
            let (remaining, step_report) = self.run_updates_report(16_384);
            report.add_assign(step_report);
            if remaining > 0 {
                break;
            }
        }
        report
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

    #[test]
    fn level_light_engine_accepts_queued_section_data() {
        let section = section_as_long(0, 0, 0);
        let block_pos = block_pos_as_long(1, 2, 3);
        let mut engine = LevelLightEngine::new(
            SyntheticLightWorld::default(),
            SyntheticLightWorld::default(),
        );
        let mut sky = DataLayer::new();
        sky.set(1, 2, 3, 9);
        let mut block = DataLayer::new();
        block.set(1, 2, 3, 12);

        engine.queue_section_data(LightLayer::Sky, section, Some(sky), true);
        engine.queue_section_data(LightLayer::Block, section, Some(block), true);
        engine.update_section_status(section, false);
        engine.accept_queued_section_data();

        assert_eq!(engine.sky_engine().stored_light(block_pos), 9);
        assert_eq!(engine.block_engine().stored_light(block_pos), 12);
        assert_eq!(engine.get_raw_brightness(block_pos, 0), 12);
    }
}
