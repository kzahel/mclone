use std::sync::OnceLock;

use crate::prng::SimpleRandomSource;

use super::noise::BetaPerlinSimplexNoise;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum BetaBiome {
    Rainforest = 0,
    Swampland = 1,
    SeasonalForest = 2,
    Forest = 3,
    Savanna = 4,
    Shrubland = 5,
    Taiga = 6,
    Desert = 7,
    Plains = 8,
    Tundra = 9,
}

impl BetaBiome {
    pub const ALL: [Self; 10] = [
        Self::Rainforest,
        Self::Swampland,
        Self::SeasonalForest,
        Self::Forest,
        Self::Savanna,
        Self::Shrubland,
        Self::Taiga,
        Self::Desert,
        Self::Plains,
        Self::Tundra,
    ];

    pub const fn semantic_id(self) -> u8 {
        self as u8
    }

    pub const fn native_biome_id(self) -> i32 {
        match self {
            Self::Rainforest => 21,
            Self::Swampland => 6,
            Self::SeasonalForest => 27,
            Self::Forest => 4,
            Self::Savanna => 35,
            Self::Shrubland => 1,
            Self::Taiga => 5,
            Self::Desert => 2,
            Self::Plains => 1,
            Self::Tundra => 12,
        }
    }

    pub const fn is_desert(self) -> bool {
        matches!(self, Self::Desert)
    }

    pub const fn is_snowy(self) -> bool {
        matches!(self, Self::Taiga | Self::Tundra)
    }
}

#[derive(Clone, Debug)]
pub struct BetaClimateRegion {
    pub temperatures: Vec<f64>,
    pub downfalls: Vec<f64>,
    pub biomes: Vec<BetaBiome>,
    pub size_x: usize,
    pub size_z: usize,
}

impl BetaClimateRegion {
    pub fn biome(&self, local_x: usize, local_z: usize) -> BetaBiome {
        self.biomes[local_x * self.size_z + local_z]
    }

    pub fn temperature(&self, local_x: usize, local_z: usize) -> f64 {
        self.temperatures[local_x * self.size_z + local_z]
    }
}

#[derive(Clone, Debug)]
pub(super) struct BetaClimateSource {
    temperature_noise: BetaPerlinSimplexNoise,
    downfall_noise: BetaPerlinSimplexNoise,
    detail_noise: BetaPerlinSimplexNoise,
}

impl BetaClimateSource {
    pub(super) fn new(seed: i64) -> Self {
        let mut temperature_random = SimpleRandomSource::new(seed.wrapping_mul(9_871));
        let mut downfall_random = SimpleRandomSource::new(seed.wrapping_mul(39_811));
        let mut detail_random = SimpleRandomSource::new(seed.wrapping_mul(543_321));
        Self {
            temperature_noise: BetaPerlinSimplexNoise::new(&mut temperature_random, 4),
            downfall_noise: BetaPerlinSimplexNoise::new(&mut downfall_random, 4),
            detail_noise: BetaPerlinSimplexNoise::new(&mut detail_random, 2),
        }
    }

    pub(super) fn region(&self, x: i32, z: i32, size_x: usize, size_z: usize) -> BetaClimateRegion {
        let float_0025 = f64::from(0.025_f32);
        let float_005 = f64::from(0.05_f32);
        let mut temperatures = self.temperature_noise.region(
            f64::from(x),
            f64::from(z),
            size_x,
            size_z,
            float_0025,
            float_0025,
            0.25,
            0.5,
        );
        let mut downfalls = self.downfall_noise.region(
            f64::from(x),
            f64::from(z),
            size_x,
            size_z,
            float_005,
            float_005,
            1.0 / 3.0,
            0.5,
        );
        let detail = self.detail_noise.region(
            f64::from(x),
            f64::from(z),
            size_x,
            size_z,
            0.25,
            0.25,
            1.0 / 1.7,
            0.5,
        );
        let mut biomes = Vec::with_capacity(size_x * size_z);
        for index in 0..size_x * size_z {
            let detail_value = detail[index] * 1.1 + 0.5;
            let temperature_blend = 0.01;
            let mut temperature = (temperatures[index] * 0.15 + 0.7) * (1.0 - temperature_blend)
                + detail_value * temperature_blend;
            let downfall_blend = 0.002;
            let mut downfall = (downfalls[index] * 0.15 + 0.5) * (1.0 - downfall_blend)
                + detail_value * downfall_blend;
            temperature = 1.0 - (1.0 - temperature) * (1.0 - temperature);
            temperature = temperature.clamp(0.0, 1.0);
            downfall = downfall.clamp(0.0, 1.0);
            temperatures[index] = temperature;
            downfalls[index] = downfall;
            biomes.push(beta_biome_from_climate(temperature, downfall));
        }
        BetaClimateRegion {
            temperatures,
            downfalls,
            biomes,
            size_x,
            size_z,
        }
    }

    #[allow(dead_code)]
    pub(super) fn temperatures(&self, x: i32, z: i32, size_x: usize, size_z: usize) -> Vec<f64> {
        let float_0025 = f64::from(0.025_f32);
        let mut temperatures = self.temperature_noise.region(
            f64::from(x),
            f64::from(z),
            size_x,
            size_z,
            float_0025,
            float_0025,
            0.25,
            0.5,
        );
        let detail = self.detail_noise.region(
            f64::from(x),
            f64::from(z),
            size_x,
            size_z,
            0.25,
            0.25,
            1.0 / 1.7,
            0.5,
        );
        for index in 0..size_x * size_z {
            let detail_value = detail[index] * 1.1 + 0.5;
            let blend = 0.01;
            let mut temperature =
                (temperatures[index] * 0.15 + 0.7) * (1.0 - blend) + detail_value * blend;
            temperature = 1.0 - (1.0 - temperature) * (1.0 - temperature);
            temperatures[index] = temperature.clamp(0.0, 1.0);
        }
        temperatures
    }
}

pub fn beta_biome_from_climate(temperature: f64, downfall: f64) -> BetaBiome {
    let temperature_index = (temperature * 63.0) as usize;
    let downfall_index = (downfall * 63.0) as usize;
    beta_biome_table()[temperature_index + downfall_index * 64]
}

fn beta_biome_table() -> &'static [BetaBiome; 4096] {
    static TABLE: OnceLock<[BetaBiome; 4096]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut values = [BetaBiome::Plains; 4096];
        for temperature in 0..64 {
            for downfall in 0..64 {
                values[temperature + downfall * 64] =
                    beta_compute_biome(temperature as f32 / 63.0_f32, downfall as f32 / 63.0_f32);
            }
        }
        values
    })
}

fn beta_compute_biome(temperature: f32, mut downfall: f32) -> BetaBiome {
    downfall *= temperature;
    if temperature < 0.1 {
        BetaBiome::Tundra
    } else if downfall < 0.2 {
        if temperature < 0.5 {
            BetaBiome::Tundra
        } else if temperature < 0.95 {
            BetaBiome::Savanna
        } else {
            BetaBiome::Desert
        }
    } else if downfall > 0.5 && temperature < 0.7 {
        BetaBiome::Swampland
    } else if temperature < 0.5 {
        BetaBiome::Taiga
    } else if temperature < 0.97 {
        if downfall < 0.35 {
            BetaBiome::Shrubland
        } else {
            BetaBiome::Forest
        }
    } else if downfall < 0.45 {
        BetaBiome::Plains
    } else if downfall < 0.9 {
        BetaBiome::SeasonalForest
    } else {
        BetaBiome::Rainforest
    }
}
