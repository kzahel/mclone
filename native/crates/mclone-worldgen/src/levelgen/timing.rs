#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::feature::FeatureDecorationTiming;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverworldFeatureBatchTiming {
    pub seed_dependency_insert_us: u128,
    pub plan_us: u128,
    pub dependency_cache_hit_clone_us: u128,
    pub dependency_generate_us: u128,
    pub dependency_generation: OverworldDependencyGenerationTiming,
    pub dependency_insert_clone_us: u128,
    pub dependency_retain_us: u128,
    pub retained_dependency_clone_us: u128,
    pub feature_region_init_us: u128,
    pub feature_decoration_us: u128,
    pub feature_decoration_steps: FeatureDecorationTiming,
    pub target_extract_us: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SurfaceFillTiming {
    pub chunk_alloc_us: u128,
    pub noise_columns_us: u128,
    pub terrain_fill_us: u128,
    pub non_air_blocks_written: usize,
}

impl SurfaceFillTiming {
    pub fn total_us(self) -> u128 {
        self.chunk_alloc_us + self.noise_columns_us + self.terrain_fill_us
    }

    pub fn add_assign(&mut self, other: Self) {
        self.chunk_alloc_us += other.chunk_alloc_us;
        self.noise_columns_us += other.noise_columns_us;
        self.terrain_fill_us += other.terrain_fill_us;
        self.non_air_blocks_written += other.non_air_blocks_written;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverworldDependencyGenerationTiming {
    pub generator_setup_us: u128,
    pub surface_fill_us: u128,
    pub surface_fill: SurfaceFillTiming,
    pub surface_bedrock_us: u128,
    pub air_carvers_us: u128,
    pub liquid_carvers_us: u128,
    pub heightmap_prime_us: u128,
}

impl OverworldDependencyGenerationTiming {
    pub fn total_us(self) -> u128 {
        self.generator_setup_us
            + self.surface_fill_us
            + self.surface_bedrock_us
            + self.air_carvers_us
            + self.liquid_carvers_us
            + self.heightmap_prime_us
    }

    pub fn add_assign(&mut self, other: Self) {
        self.generator_setup_us += other.generator_setup_us;
        self.surface_fill_us += other.surface_fill_us;
        self.surface_fill.add_assign(other.surface_fill);
        self.surface_bedrock_us += other.surface_bedrock_us;
        self.air_carvers_us += other.air_carvers_us;
        self.liquid_carvers_us += other.liquid_carvers_us;
        self.heightmap_prime_us += other.heightmap_prime_us;
    }
}

impl OverworldFeatureBatchTiming {
    pub fn total_us(self) -> u128 {
        self.seed_dependency_insert_us
            + self.plan_us
            + self.dependency_cache_hit_clone_us
            + self.dependency_generate_us
            + self.dependency_insert_clone_us
            + self.dependency_retain_us
            + self.retained_dependency_clone_us
            + self.feature_region_init_us
            + self.feature_decoration_us
            + self.target_extract_us
    }

    pub fn add_assign(&mut self, other: Self) {
        self.seed_dependency_insert_us += other.seed_dependency_insert_us;
        self.plan_us += other.plan_us;
        self.dependency_cache_hit_clone_us += other.dependency_cache_hit_clone_us;
        self.dependency_generate_us += other.dependency_generate_us;
        self.dependency_generation
            .add_assign(other.dependency_generation);
        self.dependency_insert_clone_us += other.dependency_insert_clone_us;
        self.dependency_retain_us += other.dependency_retain_us;
        self.retained_dependency_clone_us += other.retained_dependency_clone_us;
        self.feature_region_init_us += other.feature_region_init_us;
        self.feature_decoration_us += other.feature_decoration_us;
        self.feature_decoration_steps
            .add_assign(other.feature_decoration_steps);
        self.target_extract_us += other.target_extract_us;
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn timing_start() -> Option<Instant> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn timing_start() -> Option<()> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn timing_elapsed_us(start: Option<Instant>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn timing_elapsed_us(_start: Option<()>) -> u128 {
    0
}
