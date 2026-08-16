const INITIAL_STATE_MULTIPLIER: u64 = 0x9E37_79B9_7F4A_7C15;
const INITIAL_STATE_INCREMENT: u64 = 0xD1B5_4A32_D192_ED03;
const NEXT_SEED_MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const NEXT_SEED_INCREMENT: u64 = 1_442_695_040_888_963_407;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NewWorldSeedReroll {
    state: u64,
}

impl NewWorldSeedReroll {
    pub fn new(seed: i64) -> Self {
        Self {
            state: initial_new_world_seed_reroll_state(seed),
        }
    }

    pub fn next_seed(&mut self) -> i64 {
        self.state = self
            .state
            .wrapping_mul(NEXT_SEED_MULTIPLIER)
            .wrapping_add(NEXT_SEED_INCREMENT);
        self.state as i64
    }

    pub const fn state(&self) -> u64 {
        self.state
    }
}

pub fn initial_new_world_seed_reroll_state(seed: i64) -> u64 {
    (seed as u64)
        .wrapping_mul(INITIAL_STATE_MULTIPLIER)
        .wrapping_add(INITIAL_STATE_INCREMENT)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_matches_existing_platform_sequences() {
        assert_eq!(initial_new_world_seed_reroll_state(0), 15111065706836454659);
        assert_eq!(initial_new_world_seed_reroll_state(1), 8065036452450101528);
        assert_eq!(
            initial_new_world_seed_reroll_state(12345),
            8278227847842921904
        );
        assert_eq!(initial_new_world_seed_reroll_state(-1), 3710350887513256174);
    }

    #[test]
    fn next_seed_sequence_matches_existing_lcg() {
        let mut reroll = NewWorldSeedReroll::new(12345);

        assert_eq!(reroll.state(), 8278227847842921904);
        assert_eq!(reroll.next_seed(), -7079405151491724993);
        assert_eq!(reroll.next_seed(), -4887674302230904222);
        assert_eq!(reroll.next_seed(), 4677552153845975689);
    }

    #[test]
    fn browser_catalog_canary_predecessor_rerolls_to_the_reproduced_seed() {
        let mut reroll = NewWorldSeedReroll::new(-8_711_654_666_216_244_498);

        assert_eq!(reroll.next_seed(), 553_534_047_293_117_028);
    }
}
