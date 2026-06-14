use crate::prng::RandomSource;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeightmapType {
    WorldSurfaceWg,
    WorldSurface,
    OceanFloorWg,
    OceanFloor,
    MotionBlocking,
    MotionBlockingNoLeaves,
}

pub type HeightSampler = fn(HeightmapType, i32, i32) -> i32;

#[derive(Clone, Copy)]
pub struct DecorationContext {
    min_build_height: i32,
    min_gen_y: i32,
    gen_depth: i32,
    height_sampler: Option<HeightSampler>,
}

impl DecorationContext {
    pub const fn new(min_build_height: i32, gen_depth: i32) -> Self {
        Self::with_generation(min_build_height, min_build_height, gen_depth)
    }

    pub const fn with_generation(min_build_height: i32, min_gen_y: i32, gen_depth: i32) -> Self {
        Self {
            min_build_height,
            min_gen_y,
            gen_depth,
            height_sampler: None,
        }
    }

    pub const fn with_height_sampler(self, height_sampler: HeightSampler) -> Self {
        Self {
            min_build_height: self.min_build_height,
            min_gen_y: self.min_gen_y,
            gen_depth: self.gen_depth,
            height_sampler: Some(height_sampler),
        }
    }

    pub const fn get_min_build_height(&self) -> i32 {
        self.min_build_height
    }

    pub const fn get_min_gen_y(&self) -> i32 {
        self.min_gen_y
    }

    pub const fn get_gen_depth(&self) -> i32 {
        self.gen_depth
    }

    pub fn get_height(&self, heightmap: HeightmapType, x: i32, z: i32) -> i32 {
        let height_sampler = self
            .height_sampler
            .expect("DecorationContext height sampler is required for heightmap placement");
        height_sampler(heightmap, x, z)
    }
}

impl Default for DecorationContext {
    fn default() -> Self {
        Self::new(0, 256)
    }
}

impl fmt::Debug for DecorationContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecorationContext")
            .field("min_build_height", &self.min_build_height)
            .field("min_gen_y", &self.min_gen_y)
            .field("gen_depth", &self.gen_depth)
            .field("has_height_sampler", &self.height_sampler.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl VerticalAnchor {
    pub const fn absolute(value: i32) -> Self {
        Self::Absolute(value)
    }

    pub const fn above_bottom(value: i32) -> Self {
        Self::AboveBottom(value)
    }

    pub const fn below_top(value: i32) -> Self {
        Self::BelowTop(value)
    }

    pub const fn bottom() -> Self {
        Self::above_bottom(0)
    }

    pub const fn top() -> Self {
        Self::below_top(0)
    }

    pub const fn resolve_y(&self, context: &DecorationContext) -> i32 {
        match *self {
            Self::Absolute(value) => value,
            Self::AboveBottom(value) => context.get_min_gen_y() + value,
            Self::BelowTop(value) => context.get_min_gen_y() + context.get_gen_depth() - 1 - value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightProvider {
    Constant {
        value: VerticalAnchor,
    },
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    BiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    },
    VeryBiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    },
    Trapezoid {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        plateau: i32,
    },
}

impl HeightProvider {
    pub const fn constant(value: VerticalAnchor) -> Self {
        Self::Constant { value }
    }

    pub const fn uniform(min_inclusive: VerticalAnchor, max_inclusive: VerticalAnchor) -> Self {
        Self::Uniform {
            min_inclusive,
            max_inclusive,
        }
    }

    pub const fn biased_to_bottom(
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    ) -> Self {
        Self::BiasedToBottom {
            min_inclusive,
            max_inclusive,
            inner,
        }
    }

    pub const fn very_biased_to_bottom(
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    ) -> Self {
        Self::VeryBiasedToBottom {
            min_inclusive,
            max_inclusive,
            inner,
        }
    }

    pub const fn trapezoid(
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        plateau: i32,
    ) -> Self {
        Self::Trapezoid {
            min_inclusive,
            max_inclusive,
            plateau,
        }
    }

    pub fn sample(&self, random: &mut impl RandomSource, context: &DecorationContext) -> i32 {
        match *self {
            Self::Constant { value } => value.resolve_y(context),
            Self::Uniform {
                min_inclusive,
                max_inclusive,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    min
                } else {
                    random_between_inclusive(random, min, max)
                }
            }
            Self::BiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                let outer = max - min - inner + 1;
                if outer <= 0 {
                    min
                } else {
                    let value = random.next_int_bound(outer);
                    random.next_int_bound(value + inner) + min
                }
            }
            Self::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if max - min - inner + 1 <= 0 {
                    min
                } else {
                    let top = next_int(random, min + inner, max);
                    let middle = next_int(random, min, top - 1);
                    next_int(random, min, middle - 1 + inner)
                }
            }
            Self::Trapezoid {
                min_inclusive,
                max_inclusive,
                plateau,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    min
                } else {
                    let span = max - min;
                    if plateau >= span {
                        random_between_inclusive(random, min, max)
                    } else {
                        let lower_span = (span - plateau) / 2;
                        let upper_span = span - lower_span;
                        min + random_between_inclusive(random, 0, upper_span)
                            + random_between_inclusive(random, 0, lower_span)
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntProvider {
    Constant(i32),
}

impl IntProvider {
    pub const fn constant(value: i32) -> Self {
        Self::Constant(value)
    }

    pub fn sample(&self, _random: &mut impl RandomSource) -> i32 {
        match *self {
            Self::Constant(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountConfiguration {
    count: IntProvider,
}

impl CountConfiguration {
    pub const fn new(count: i32) -> Self {
        Self {
            count: IntProvider::constant(count),
        }
    }

    pub const fn from_provider(count: IntProvider) -> Self {
        Self { count }
    }

    pub const fn count(&self) -> IntProvider {
        self.count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChanceDecoratorConfiguration {
    pub chance: i32,
}

impl ChanceDecoratorConfiguration {
    pub const fn new(chance: i32) -> Self {
        Self { chance }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeDecoratorConfiguration {
    pub height: HeightProvider,
}

impl RangeDecoratorConfiguration {
    pub const fn new(height: HeightProvider) -> Self {
        Self { height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeightmapConfiguration {
    pub heightmap: HeightmapType,
}

impl HeightmapConfiguration {
    pub const fn new(heightmap: HeightmapType) -> Self {
        Self { heightmap }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfiguredDecorator {
    Nope,
    Square,
    Count(CountConfiguration),
    Chance(ChanceDecoratorConfiguration),
    Range(RangeDecoratorConfiguration),
    Spread32Above,
    Heightmap(HeightmapConfiguration),
    HeightmapSpreadDouble(HeightmapConfiguration),
}

impl ConfiguredDecorator {
    pub const fn nope() -> Self {
        Self::Nope
    }

    pub const fn square() -> Self {
        Self::Square
    }

    pub const fn count(count: i32) -> Self {
        Self::Count(CountConfiguration::new(count))
    }

    pub const fn chance(chance: i32) -> Self {
        Self::Chance(ChanceDecoratorConfiguration::new(chance))
    }

    pub const fn range(height: HeightProvider) -> Self {
        Self::Range(RangeDecoratorConfiguration::new(height))
    }

    pub const fn spread_32_above() -> Self {
        Self::Spread32Above
    }

    pub const fn heightmap(heightmap: HeightmapType) -> Self {
        Self::Heightmap(HeightmapConfiguration::new(heightmap))
    }

    pub const fn heightmap_spread_double(heightmap: HeightmapType) -> Self {
        Self::HeightmapSpreadDouble(HeightmapConfiguration::new(heightmap))
    }

    pub fn get_positions(
        &self,
        context: &DecorationContext,
        random: &mut impl RandomSource,
        pos: BlockPos,
    ) -> Vec<BlockPos> {
        match *self {
            Self::Nope => nope_positions(context, random, pos),
            Self::Square => square_positions(context, random, pos),
            Self::Count(config) => count_positions(context, random, config, pos),
            Self::Chance(config) => chance_positions(context, random, config, pos),
            Self::Range(config) => range_positions(context, random, config, pos),
            Self::Spread32Above => spread_32_above_positions(context, random, pos),
            Self::Heightmap(config) => heightmap_positions(context, random, config, pos),
            Self::HeightmapSpreadDouble(config) => {
                heightmap_spread_double_positions(context, random, config, pos)
            }
        }
    }
}

pub fn nope_positions(
    _context: &DecorationContext,
    _random: &mut impl RandomSource,
    pos: BlockPos,
) -> Vec<BlockPos> {
    vec![pos]
}

pub fn square_positions(
    _context: &DecorationContext,
    random: &mut impl RandomSource,
    pos: BlockPos,
) -> Vec<BlockPos> {
    vec![BlockPos::new(
        random.next_int_bound(16) + pos.x,
        pos.y,
        random.next_int_bound(16) + pos.z,
    )]
}

pub fn count_positions(
    _context: &DecorationContext,
    random: &mut impl RandomSource,
    config: CountConfiguration,
    pos: BlockPos,
) -> Vec<BlockPos> {
    let count = config.count().sample(random).max(0) as usize;
    vec![pos; count]
}

pub fn chance_positions(
    _context: &DecorationContext,
    random: &mut impl RandomSource,
    config: ChanceDecoratorConfiguration,
    pos: BlockPos,
) -> Vec<BlockPos> {
    if random.next_float() < 1.0 / config.chance as f32 {
        vec![pos]
    } else {
        Vec::new()
    }
}

pub fn range_positions(
    context: &DecorationContext,
    random: &mut impl RandomSource,
    config: RangeDecoratorConfiguration,
    pos: BlockPos,
) -> Vec<BlockPos> {
    vertical_positions(config.height.sample(random, context), pos)
}

pub fn spread_32_above_positions(
    _context: &DecorationContext,
    random: &mut impl RandomSource,
    pos: BlockPos,
) -> Vec<BlockPos> {
    vertical_positions(random.next_int_bound(pos.y.max(0) + 32), pos)
}

pub fn heightmap_positions(
    context: &DecorationContext,
    _random: &mut impl RandomSource,
    config: HeightmapConfiguration,
    pos: BlockPos,
) -> Vec<BlockPos> {
    let y = context.get_height(config.heightmap, pos.x, pos.z);
    if y > context.get_min_build_height() {
        vec![BlockPos::new(pos.x, y, pos.z)]
    } else {
        Vec::new()
    }
}

pub fn heightmap_spread_double_positions(
    context: &DecorationContext,
    random: &mut impl RandomSource,
    config: HeightmapConfiguration,
    pos: BlockPos,
) -> Vec<BlockPos> {
    let y = context.get_height(config.heightmap, pos.x, pos.z);
    if y == context.get_min_build_height() {
        Vec::new()
    } else {
        vec![BlockPos::new(
            pos.x,
            context.get_min_build_height()
                + random.next_int_bound((y - context.get_min_build_height()) * 2),
            pos.z,
        )]
    }
}

fn vertical_positions(y: i32, pos: BlockPos) -> Vec<BlockPos> {
    vec![BlockPos::new(pos.x, y, pos.z)]
}

fn random_between_inclusive(random: &mut impl RandomSource, min: i32, max: i32) -> i32 {
    random.next_int_bound(max - min + 1) + min
}

fn next_int(random: &mut impl RandomSource, min: i32, max: i32) -> i32 {
    if min >= max {
        min
    } else {
        random_between_inclusive(random, min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prng::{SimpleRandomSource, WorldgenRandom};

    const CONTEXT: DecorationContext = DecorationContext::new(0, 256);
    const POS: BlockPos = BlockPos::new(32, 72, -48);

    fn fixed_height(heightmap: HeightmapType, x: i32, z: i32) -> i32 {
        assert_eq!(heightmap, HeightmapType::MotionBlocking);
        assert_eq!(x, POS.x);
        assert_eq!(z, POS.z);
        64
    }

    fn min_build_height(heightmap: HeightmapType, _x: i32, _z: i32) -> i32 {
        assert_eq!(heightmap, HeightmapType::MotionBlocking);
        0
    }

    #[test]
    fn nope_returns_input_position_without_random_draws() {
        let mut random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::nope().get_positions(&CONTEXT, &mut random, POS),
            vec![POS]
        );
        assert_eq!(random.get_count(), 0);
    }

    #[test]
    fn square_offsets_x_and_z_with_two_next_int_16_draws() {
        let mut random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::square().get_positions(&CONTEXT, &mut random, POS),
            vec![BlockPos::new(37, 72, -40)]
        );
        assert_eq!(random.get_count(), 2);
    }

    #[test]
    fn count_repeats_input_position_without_random_draws_for_constant_provider() {
        let mut random = WorldgenRandom::new(99);

        assert_eq!(
            ConfiguredDecorator::count(3).get_positions(&CONTEXT, &mut random, POS),
            vec![POS, POS, POS]
        );
        assert_eq!(random.get_count(), 0);
    }

    #[test]
    fn count_zero_and_negative_are_empty_ranges() {
        let mut zero_random = SimpleRandomSource::new(0);
        let mut negative_random = SimpleRandomSource::new(0);

        assert!(
            ConfiguredDecorator::count(0)
                .get_positions(&CONTEXT, &mut zero_random, POS)
                .is_empty()
        );
        assert!(
            ConfiguredDecorator::count(-2)
                .get_positions(&CONTEXT, &mut negative_random, POS)
                .is_empty()
        );
    }

    #[test]
    fn chance_uses_inverse_chance_threshold() {
        let mut accepted_random = WorldgenRandom::new(5045);
        let mut rejected_random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::chance(4).get_positions(&CONTEXT, &mut accepted_random, POS),
            vec![POS]
        );
        assert!(
            ConfiguredDecorator::chance(4)
                .get_positions(&CONTEXT, &mut rejected_random, POS)
                .is_empty()
        );
        assert_eq!(accepted_random.get_count(), 1);
        assert_eq!(rejected_random.get_count(), 1);
    }

    #[test]
    fn vertical_anchors_resolve_against_generation_context() {
        let context = DecorationContext::with_generation(-80, -64, 384);

        assert_eq!(VerticalAnchor::absolute(12).resolve_y(&context), 12);
        assert_eq!(VerticalAnchor::above_bottom(5).resolve_y(&context), -59);
        assert_eq!(VerticalAnchor::below_top(8).resolve_y(&context), 311);
    }

    #[test]
    fn range_constant_height_resolves_anchor_without_random_draws() {
        let mut random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::range(HeightProvider::constant(VerticalAnchor::below_top(8)))
                .get_positions(&CONTEXT, &mut random, POS),
            vec![BlockPos::new(32, 247, -48)]
        );
        assert_eq!(random.get_count(), 0);
    }

    #[test]
    fn height_providers_match_java_randomized_samples() {
        let mut uniform_random = WorldgenRandom::new(12345);
        let mut biased_random = WorldgenRandom::new(12345);
        let mut very_biased_random = WorldgenRandom::new(12345);
        let mut trapezoid_random = WorldgenRandom::new(12345);

        assert_eq!(
            HeightProvider::uniform(VerticalAnchor::absolute(10), VerticalAnchor::absolute(20))
                .sample(&mut uniform_random, &CONTEXT),
            16
        );
        assert_eq!(
            HeightProvider::biased_to_bottom(
                VerticalAnchor::absolute(0),
                VerticalAnchor::absolute(127),
                8,
            )
            .sample(&mut biased_random, &CONTEXT),
            94
        );
        assert_eq!(
            HeightProvider::very_biased_to_bottom(
                VerticalAnchor::absolute(0),
                VerticalAnchor::absolute(127),
                8,
            )
            .sample(&mut very_biased_random, &CONTEXT),
            99
        );
        assert_eq!(
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(10), 0)
                .sample(&mut trapezoid_random, &CONTEXT),
            5
        );
        assert_eq!(uniform_random.get_count(), 1);
        assert_eq!(biased_random.get_count(), 2);
        assert_eq!(very_biased_random.get_count(), 3);
        assert_eq!(trapezoid_random.get_count(), 2);
    }

    #[test]
    fn empty_height_provider_ranges_return_min_without_random_draws() {
        let mut uniform_random = WorldgenRandom::new(12345);
        let mut biased_random = WorldgenRandom::new(12345);

        assert_eq!(
            HeightProvider::uniform(VerticalAnchor::absolute(20), VerticalAnchor::absolute(10))
                .sample(&mut uniform_random, &CONTEXT),
            20
        );
        assert_eq!(
            HeightProvider::biased_to_bottom(
                VerticalAnchor::absolute(0),
                VerticalAnchor::absolute(6),
                8,
            )
            .sample(&mut biased_random, &CONTEXT),
            0
        );
        assert_eq!(uniform_random.get_count(), 0);
        assert_eq!(biased_random.get_count(), 0);
    }

    #[test]
    fn spread_32_above_samples_from_max_input_y_plus_32() {
        let mut random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::spread_32_above().get_positions(&CONTEXT, &mut random, POS),
            vec![BlockPos::new(32, 35, -48)]
        );
        assert_eq!(random.get_count(), 1);
    }

    #[test]
    fn heightmap_decorator_uses_context_height_and_min_build_height_guard() {
        let context = CONTEXT.with_height_sampler(fixed_height);
        let min_context = CONTEXT.with_height_sampler(min_build_height);
        let mut random = WorldgenRandom::new(12345);
        let mut min_random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking).get_positions(
                &context,
                &mut random,
                POS
            ),
            vec![BlockPos::new(32, 64, -48)]
        );
        assert!(
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking)
                .get_positions(&min_context, &mut min_random, POS)
                .is_empty()
        );
        assert_eq!(random.get_count(), 0);
        assert_eq!(min_random.get_count(), 0);
    }

    #[test]
    fn heightmap_spread_double_samples_between_min_and_double_height_delta() {
        let context = CONTEXT.with_height_sampler(fixed_height);
        let mut random = WorldgenRandom::new(12345);

        assert_eq!(
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking)
                .get_positions(&context, &mut random, POS),
            vec![BlockPos::new(32, 46, -48)]
        );
        assert_eq!(random.get_count(), 1);
    }
}
