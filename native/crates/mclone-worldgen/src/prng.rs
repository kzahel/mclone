const MODULUS_BITS: u32 = 48;
const MODULUS_MASK: u64 = (1_u64 << MODULUS_BITS) - 1;
const MULTIPLIER: u64 = 25_214_903_917;
const INCREMENT: u64 = 11;
const FLOAT_MULTIPLIER: f32 = 1.0 / 16_777_216.0;
const DOUBLE_MULTIPLIER: f64 = 1.110223E-16_f32 as f64;

const BASE_CHUNK_MULTIPLIER_X: i64 = 341_873_128_712;
const BASE_CHUNK_MULTIPLIER_Z: i64 = 132_897_987_541;

pub trait RandomSource {
    fn set_seed(&mut self, seed: i64);
    fn next_int(&mut self) -> i32;
    fn next_int_bound(&mut self, bound: i32) -> i32;
    fn next_long(&mut self) -> i64;
    fn next_boolean(&mut self) -> bool;
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;
    fn next_gaussian(&mut self) -> f64;

    fn consume_count(&mut self, count: usize) {
        for _ in 0..count {
            self.next_int();
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimpleRandomSource {
    seed: u64,
    next_next_gaussian: f64,
    have_next_next_gaussian: bool,
}

impl SimpleRandomSource {
    pub fn new(seed: i64) -> Self {
        let mut random = Self {
            seed: 0,
            next_next_gaussian: 0.0,
            have_next_next_gaussian: false,
        };
        random.set_seed(seed);
        random
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = ((seed as u64) ^ MULTIPLIER) & MODULUS_MASK;
    }

    pub fn next_int(&mut self) -> i32 {
        self.next_bits_signed(32)
    }

    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            panic!("Bound must be positive");
        }

        if (bound & (bound - 1)) == 0 {
            return (((bound as i64) * (self.next_bits_raw(31) as i64)) >> 31) as i32;
        }

        loop {
            let bits = self.next_bits_raw(31) as i32;
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        next_long_from_parts(self.next_bits_signed(32), self.next_bits_signed(32))
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next_bits_raw(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        (self.next_bits_raw(24) as f32) * FLOAT_MULTIPLIER
    }

    pub fn next_double(&mut self) -> f64 {
        let upper = self.next_bits_raw(26) as u64;
        let lower = self.next_bits_raw(27) as u64;
        (((upper << 27) + lower) as f64) * DOUBLE_MULTIPLIER
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            return self.next_next_gaussian;
        }

        let mut first;
        let mut second;
        let mut radius_squared;
        loop {
            first = 2.0 * self.next_double() - 1.0;
            second = 2.0 * self.next_double() - 1.0;
            radius_squared = first * first + second * second;
            if radius_squared < 1.0 && radius_squared != 0.0 {
                break;
            }
        }

        let scale = (-2.0 * radius_squared.ln() / radius_squared).sqrt();
        self.next_next_gaussian = second * scale;
        self.have_next_next_gaussian = true;
        first * scale
    }

    pub fn consume_count(&mut self, count: usize) {
        for _ in 0..count {
            self.next_int();
        }
    }

    fn next_bits_signed(&mut self, bits: u32) -> i32 {
        self.next_bits_raw(bits) as i32
    }

    fn next_bits_raw(&mut self, bits: u32) -> u32 {
        assert!((1..=32).contains(&bits), "bits must be between 1 and 32");
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT) & MODULUS_MASK;
        (self.seed >> (MODULUS_BITS - bits)) as u32
    }
}

impl RandomSource for SimpleRandomSource {
    fn set_seed(&mut self, seed: i64) {
        Self::set_seed(self, seed);
    }

    fn next_int(&mut self) -> i32 {
        Self::next_int(self)
    }

    fn next_int_bound(&mut self, bound: i32) -> i32 {
        Self::next_int_bound(self, bound)
    }

    fn next_long(&mut self) -> i64 {
        Self::next_long(self)
    }

    fn next_boolean(&mut self) -> bool {
        Self::next_boolean(self)
    }

    fn next_float(&mut self) -> f32 {
        Self::next_float(self)
    }

    fn next_double(&mut self) -> f64 {
        Self::next_double(self)
    }

    fn next_gaussian(&mut self) -> f64 {
        Self::next_gaussian(self)
    }

    fn consume_count(&mut self, count: usize) {
        Self::consume_count(self, count);
    }
}

#[derive(Clone, Debug)]
pub struct WorldgenRandom {
    source: SimpleRandomSource,
    count: usize,
    next_next_gaussian: f64,
    have_next_next_gaussian: bool,
}

impl Default for WorldgenRandom {
    fn default() -> Self {
        Self::new(0)
    }
}

impl WorldgenRandom {
    pub fn new(seed: i64) -> Self {
        Self {
            source: SimpleRandomSource::new(seed),
            count: 0,
            next_next_gaussian: 0.0,
            have_next_next_gaussian: false,
        }
    }

    pub fn get_count(&self) -> usize {
        self.count
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed);
        self.have_next_next_gaussian = false;
    }

    pub fn next_int(&mut self) -> i32 {
        self.next_bits_signed(32)
    }

    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            panic!("Bound must be positive");
        }

        if (bound & (bound - 1)) == 0 {
            return (((bound as i64) * (self.next_bits_raw(31) as i64)) >> 31) as i32;
        }

        loop {
            let bits = self.next_bits_raw(31) as i32;
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        next_long_from_parts(self.next_bits_signed(32), self.next_bits_signed(32))
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next_bits_raw(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        (self.next_bits_raw(24) as f32) * FLOAT_MULTIPLIER
    }

    pub fn next_double(&mut self) -> f64 {
        let upper = self.next_bits_raw(26) as u64;
        let lower = self.next_bits_raw(27) as u64;
        (((upper << 27) + lower) as f64) * DOUBLE_MULTIPLIER
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            return self.next_next_gaussian;
        }

        let mut first;
        let mut second;
        let mut radius_squared;
        loop {
            first = 2.0 * self.next_double() - 1.0;
            second = 2.0 * self.next_double() - 1.0;
            radius_squared = first * first + second * second;
            if radius_squared < 1.0 && radius_squared != 0.0 {
                break;
            }
        }

        let scale = (-2.0 * radius_squared.ln() / radius_squared).sqrt();
        self.next_next_gaussian = second * scale;
        self.have_next_next_gaussian = true;
        first * scale
    }

    pub fn consume_count(&mut self, count: usize) {
        for _ in 0..count {
            self.next_int();
        }
    }

    pub fn set_base_chunk_seed(&mut self, chunk_x: i32, chunk_z: i32) -> i64 {
        let seed = (chunk_x as i64)
            .wrapping_mul(BASE_CHUNK_MULTIPLIER_X)
            .wrapping_add((chunk_z as i64).wrapping_mul(BASE_CHUNK_MULTIPLIER_Z));
        self.set_seed(seed);
        seed
    }

    pub fn set_decoration_seed(
        &mut self,
        level_seed: i64,
        min_chunk_block_x: i32,
        min_chunk_block_z: i32,
    ) -> i64 {
        self.set_seed(level_seed);
        let x_multiplier = self.next_long() | 1;
        let z_multiplier = self.next_long() | 1;
        let seed = (min_chunk_block_x as i64)
            .wrapping_mul(x_multiplier)
            .wrapping_add((min_chunk_block_z as i64).wrapping_mul(z_multiplier))
            ^ level_seed;
        self.set_seed(seed);
        seed
    }

    pub fn set_feature_seed(
        &mut self,
        decoration_seed: i64,
        index: i32,
        decoration_step: i32,
    ) -> i64 {
        let seed = decoration_seed
            .wrapping_add(index as i64)
            .wrapping_add((10_000_i64).wrapping_mul(decoration_step as i64));
        self.set_seed(seed);
        seed
    }

    pub fn set_large_feature_seed(&mut self, base_seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
        self.set_seed(base_seed);
        let x_multiplier = self.next_long();
        let z_multiplier = self.next_long();
        let seed = (chunk_x as i64).wrapping_mul(x_multiplier)
            ^ (chunk_z as i64).wrapping_mul(z_multiplier)
            ^ base_seed;
        self.set_seed(seed);
        seed
    }

    pub fn set_base_stone_seed(&mut self, level_seed: i64, x: i32, y: i32, z: i32) -> i64 {
        self.set_seed(level_seed);
        let x_multiplier = self.next_long();
        let y_multiplier = self.next_long();
        let z_multiplier = self.next_long();
        let seed = (x as i64).wrapping_mul(x_multiplier)
            ^ (y as i64).wrapping_mul(y_multiplier)
            ^ (z as i64).wrapping_mul(z_multiplier)
            ^ level_seed;
        self.set_seed(seed);
        seed
    }

    pub fn set_large_feature_with_salt(
        &mut self,
        level_seed: i64,
        region_x: i32,
        region_z: i32,
        salt: i32,
    ) -> i64 {
        let seed = (region_x as i64)
            .wrapping_mul(BASE_CHUNK_MULTIPLIER_X)
            .wrapping_add((region_z as i64).wrapping_mul(BASE_CHUNK_MULTIPLIER_Z))
            .wrapping_add(level_seed)
            .wrapping_add(salt as i64);
        self.set_seed(seed);
        seed
    }

    pub fn seed_slime_chunk(chunk_x: i32, chunk_z: i32, level_seed: i64, salt: i64) -> Self {
        let x_square_term = chunk_x.wrapping_mul(chunk_x).wrapping_mul(4_987_142) as i64;
        let x_term = chunk_x.wrapping_mul(5_947_611) as i64;
        let z_square_term = (chunk_z.wrapping_mul(chunk_z) as i64).wrapping_mul(4_392_871);
        let z_term = chunk_z.wrapping_mul(389_711) as i64;
        let seed = level_seed
            .wrapping_add(x_square_term)
            .wrapping_add(x_term)
            .wrapping_add(z_square_term)
            .wrapping_add(z_term)
            ^ salt;
        Self::new(seed)
    }

    fn next_bits_signed(&mut self, bits: u32) -> i32 {
        self.next_bits_raw(bits) as i32
    }

    fn next_bits_raw(&mut self, bits: u32) -> u32 {
        self.count += 1;
        self.source.next_bits_raw(bits)
    }
}

impl RandomSource for WorldgenRandom {
    fn set_seed(&mut self, seed: i64) {
        Self::set_seed(self, seed);
    }

    fn next_int(&mut self) -> i32 {
        Self::next_int(self)
    }

    fn next_int_bound(&mut self, bound: i32) -> i32 {
        Self::next_int_bound(self, bound)
    }

    fn next_long(&mut self) -> i64 {
        Self::next_long(self)
    }

    fn next_boolean(&mut self) -> bool {
        Self::next_boolean(self)
    }

    fn next_float(&mut self) -> f32 {
        Self::next_float(self)
    }

    fn next_double(&mut self) -> f64 {
        Self::next_double(self)
    }

    fn next_gaussian(&mut self) -> f64 {
        Self::next_gaussian(self)
    }

    fn consume_count(&mut self, count: usize) {
        Self::consume_count(self, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PrngFixture {
        module: String,
        minecraft_version: String,
        random_source_class: String,
        random_source_alias: String,
        seed: String,
        count: usize,
        sequence_mode: String,
        wire_format: WireFormat,
        next_int: Vec<i32>,
        next_long: Vec<String>,
        next_double: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    struct WireFormat {
        #[serde(rename = "nextInt")]
        next_int: String,
        #[serde(rename = "nextLong")]
        next_long: String,
        #[serde(rename = "nextDouble")]
        next_double: String,
    }

    fn fixtures() -> Vec<PrngFixture> {
        [
            include_str!("../../../../test/fixtures/prng/seed-0.json"),
            include_str!("../../../../test/fixtures/prng/seed-1.json"),
            include_str!("../../../../test/fixtures/prng/seed-12345.json"),
            include_str!("../../../../test/fixtures/prng/seed-2151901553968352745.json"),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid PRNG fixture"))
        .collect()
    }

    #[test]
    fn fixture_metadata_stays_consistent() {
        for fixture in fixtures() {
            assert_eq!(fixture.module, "prng");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.random_source_class,
                "net.minecraft.world.level.levelgen.SimpleRandomSource"
            );
            assert_eq!(fixture.random_source_alias, "LegacyRandomSource");
            assert_eq!(fixture.sequence_mode, "fresh-instance-per-method");
            assert_eq!(fixture.count, fixture.next_int.len());
            assert_eq!(fixture.count, fixture.next_long.len());
            assert_eq!(fixture.count, fixture.next_double.len());
            assert_eq!(fixture.wire_format.next_int, "number");
            assert_eq!(fixture.wire_format.next_long, "signed-decimal-string");
            assert_eq!(fixture.wire_format.next_double, "number");
        }
    }

    #[test]
    fn next_int_matches_java_oracle() {
        for fixture in fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            for (index, expected) in fixture.next_int.iter().enumerate() {
                let actual = random.next_int();
                assert_eq!(
                    actual, *expected,
                    "nextInt(seed={}) mismatch at index {index}",
                    fixture.seed
                );
            }
        }
    }

    #[test]
    fn next_long_matches_java_oracle() {
        for fixture in fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            for (index, expected) in fixture.next_long.iter().enumerate() {
                let actual = random.next_long().to_string();
                assert_eq!(
                    actual, *expected,
                    "nextLong(seed={}) mismatch at index {index}",
                    fixture.seed
                );
            }
        }
    }

    #[test]
    fn next_double_matches_java_oracle() {
        for fixture in fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            for (index, expected) in fixture.next_double.iter().enumerate() {
                let actual = random.next_double();
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "nextDouble(seed={}) mismatch at index {index}: expected {expected}, got {actual}",
                    fixture.seed
                );
            }
        }
    }

    #[test]
    fn set_seed_rewinds_the_sequence() {
        let mut random = SimpleRandomSource::new(12_345);
        let first = random.next_int();
        random.next_int();
        random.set_seed(12_345);
        assert_eq!(random.next_int(), first);
    }

    #[test]
    #[should_panic(expected = "Bound must be positive")]
    fn rejects_zero_bound() {
        let mut random = SimpleRandomSource::new(0);
        random.next_int_bound(0);
    }

    #[test]
    fn worldgen_random_matches_simple_random_for_base_sequence() {
        let mut worldgen = WorldgenRandom::new(12_345);
        let mut simple = SimpleRandomSource::new(12_345);

        assert_eq!(worldgen.next_int(), simple.next_int());
        assert_eq!(worldgen.next_long(), simple.next_long());
        assert_eq!(worldgen.next_boolean(), simple.next_boolean());
        assert_eq!(
            worldgen.next_float().to_bits(),
            simple.next_float().to_bits()
        );
        assert_eq!(
            worldgen.next_double().to_bits(),
            simple.next_double().to_bits()
        );
    }

    #[test]
    fn worldgen_random_tracks_java_style_next_call_counts() {
        let mut random = WorldgenRandom::new(0);

        assert_eq!(random.get_count(), 0);
        random.next_int();
        assert_eq!(random.get_count(), 1);
        random.next_long();
        assert_eq!(random.get_count(), 3);
        random.next_double();
        assert_eq!(random.get_count(), 5);
        random.consume_count(4);
        assert_eq!(random.get_count(), 9);
    }

    #[test]
    fn worldgen_random_seed_derivation_helpers_mirror_java_formulas() {
        let mut random = WorldgenRandom::new(0);
        assert_eq!(random.set_base_chunk_seed(2, -3), 285_052_294_801);
        assert_eq!(
            random.set_large_feature_with_salt(12_345, 4, -7, 20_003),
            437_206_634_409
        );
    }

    #[test]
    fn decoration_feature_seed_matches_java_oracle_for_negative_chunk_z() {
        let mut random = WorldgenRandom::default();
        let decoration_seed = random.set_decoration_seed(12_345, 0, -16);
        assert_eq!(
            random.set_feature_seed(decoration_seed, 2, 8),
            1_330_090_079_493_877_963
        );
    }

    #[test]
    fn slime_chunk_seeding_returns_a_random_source_initialized_to_the_java_formula() {
        let mut seeded = WorldgenRandom::seed_slime_chunk(4, -7, 12_345, 987_234_911);
        let formula_seed = (12_345_i64
            + (16_i64 * 4_987_142)
            + (4_i64 * 5_947_611)
            + (49_i64 * 4_392_871)
            + (-7_i64 * 389_711))
            ^ 987_234_911;
        let mut direct = WorldgenRandom::new(formula_seed);

        assert_eq!(seeded.next_int(), direct.next_int());
        assert_eq!(
            seeded.next_double().to_bits(),
            direct.next_double().to_bits()
        );
    }
}

fn next_long_from_parts(hi: i32, lo: i32) -> i64 {
    (hi as i64).wrapping_shl(32).wrapping_add(lo as i64)
}
