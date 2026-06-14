use crate::prng::RandomSource;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecorationContext;

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
pub enum ConfiguredDecorator {
    Nope,
    Square,
    Count(CountConfiguration),
    Chance(ChanceDecoratorConfiguration),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prng::{SimpleRandomSource, WorldgenRandom};

    const CONTEXT: DecorationContext = DecorationContext;
    const POS: BlockPos = BlockPos::new(32, 72, -48);

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
}
