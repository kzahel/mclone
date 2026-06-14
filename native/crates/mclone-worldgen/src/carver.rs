use crate::biome::OverworldBiomeSource;
use crate::levelgen::MutableChunkBlockBuffer;
use crate::prng::{RandomSource, SimpleRandomSource, WorldgenRandom};
use std::sync::OnceLock;

const CHUNK_WIDTH: i32 = 16;
const CARVER_RANGE: i32 = 4;
const SIN_TABLE_SIZE: usize = 65_536;
const SIN_TABLE_MASK: i32 = 65_535;
const SIN_SCALE: f32 = 10_430.378_f32;
const COS_OFFSET: f32 = 16_384.0_f32;
const PI: f32 = std::f32::consts::PI;
const TWO_PI: f32 = PI * 2.0;

const AIR: u8 = 0;
const STONE: u8 = 1;
const WATER: u8 = 2;
const GRASS_BLOCK: u8 = 4;
const DIRT: u8 = 5;
const SAND: u8 = 6;
const GRAVEL: u8 = 7;
const SNOW: u8 = 8;
const LAVA: u8 = 9;
const GRANITE: u8 = 10;
const DIORITE: u8 = 11;
const ANDESITE: u8 = 12;
const COARSE_DIRT: u8 = 13;
const PODZOL: u8 = 14;
const MYCELIUM: u8 = 15;
const TERRACOTTA: u8 = 16;
const WHITE_TERRACOTTA: u8 = 17;
const ORANGE_TERRACOTTA: u8 = 18;
const MAGENTA_TERRACOTTA: u8 = 19;
const LIGHT_BLUE_TERRACOTTA: u8 = 20;
const YELLOW_TERRACOTTA: u8 = 21;
const LIME_TERRACOTTA: u8 = 22;
const PINK_TERRACOTTA: u8 = 23;
const GRAY_TERRACOTTA: u8 = 24;
const LIGHT_GRAY_TERRACOTTA: u8 = 25;
const CYAN_TERRACOTTA: u8 = 26;
const PURPLE_TERRACOTTA: u8 = 27;
const BLUE_TERRACOTTA: u8 = 28;
const BROWN_TERRACOTTA: u8 = 29;
const GREEN_TERRACOTTA: u8 = 30;
const RED_TERRACOTTA: u8 = 31;
const BLACK_TERRACOTTA: u8 = 32;
const SANDSTONE: u8 = 33;
const RED_SANDSTONE: u8 = 34;
const PACKED_ICE: u8 = 35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationStepCarving {
    Air,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarvingMask {
    min_y: i32,
    height: i32,
    bits: Vec<u8>,
}

impl CarvingMask {
    pub fn new(min_y: i32, height: i32) -> Self {
        Self {
            min_y,
            height,
            bits: vec![0; height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
        }
    }

    pub fn is_set(&self, local_x: i32, y: i32, local_z: i32) -> bool {
        self.bits[self.index(local_x, y, local_z)] != 0
    }

    pub fn set(&mut self, local_x: i32, y: i32, local_z: i32) {
        let index = self.index(local_x, y, local_z);
        self.bits[index] = 1;
    }

    pub fn carved_count(&self) -> usize {
        self.bits.iter().filter(|&&bit| bit != 0).count()
    }

    pub fn bits(&self) -> &[u8] {
        &self.bits
    }

    fn index(&self, local_x: i32, y: i32, local_z: i32) -> usize {
        if !(0..CHUNK_WIDTH).contains(&local_x) || !(0..CHUNK_WIDTH).contains(&local_z) {
            panic!("local position out of chunk: ({local_x}, {y}, {local_z})");
        }
        if !(self.min_y..self.min_y + self.height).contains(&y) {
            panic!(
                "y {y} outside carving mask range {}..{}",
                self.min_y,
                self.min_y + self.height
            );
        }
        (local_x | (local_z << 4) | ((y - self.min_y) << 8)) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CarvingContext {
    min_gen_y: i32,
    gen_depth: i32,
}

impl CarvingContext {
    fn new(chunk: &MutableChunkBlockBuffer) -> Self {
        Self {
            min_gen_y: chunk.min_y,
            gen_depth: chunk.height,
        }
    }

    fn min_y(&self) -> i32 {
        self.min_gen_y
    }

    fn gen_depth(&self) -> i32 {
        self.gen_depth
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
}

impl VerticalAnchor {
    fn resolve_y(&self, context: &CarvingContext) -> i32 {
        match *self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => context.min_y() + offset,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum HeightProvider {
    BiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    },
}

impl HeightProvider {
    fn biased_to_bottom(
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

    fn sample(&self, random: &mut impl RandomSource, context: &CarvingContext) -> i32 {
        match *self {
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
                    let biased_bound = random.next_int_bound(outer) + inner;
                    min + random.next_int_bound(biased_bound)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FloatProvider {
    Constant(f32),
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    Trapezoid {
        min_inclusive: f32,
        max_exclusive: f32,
        plateau: f32,
    },
}

impl FloatProvider {
    fn sample(&self, random: &mut impl RandomSource) -> f32 {
        match *self {
            Self::Constant(value) => value,
            Self::Uniform {
                min_inclusive,
                max_exclusive,
            } => min_inclusive + random.next_float() * (max_exclusive - min_inclusive),
            Self::Trapezoid {
                min_inclusive,
                max_exclusive,
                plateau,
            } => {
                let span = max_exclusive - min_inclusive;
                let slope_width = (span - plateau) / 2.0;
                let upper_width = span - slope_width;
                min_inclusive
                    + random.next_float() * upper_width
                    + random.next_float() * slope_width
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CarverDebugSettings {
    debug_mode: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CarverConfiguration {
    probability: f32,
    y: HeightProvider,
    y_scale: FloatProvider,
    lava_level: VerticalAnchor,
    aquifers_enabled: bool,
    debug_settings: CarverDebugSettings,
}

impl CarverConfiguration {
    fn lava_level(&self, context: &CarvingContext) -> i32 {
        self.lava_level.resolve_y(context)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CaveCarverConfiguration {
    base: CarverConfiguration,
    horizontal_radius_multiplier: FloatProvider,
    vertical_radius_multiplier: FloatProvider,
    floor_level: FloatProvider,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanyonShapeConfiguration {
    distance_factor: FloatProvider,
    thickness: FloatProvider,
    width_smoothness: i32,
    horizontal_radius_factor: FloatProvider,
    vertical_radius_default_factor: f32,
    vertical_radius_center_factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanyonCarverConfiguration {
    base: CarverConfiguration,
    vertical_rotation: FloatProvider,
    shape: CanyonShapeConfiguration,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ConfiguredAirCarver {
    Cave(CaveCarverConfiguration),
    Canyon(CanyonCarverConfiguration),
}

impl ConfiguredAirCarver {
    fn is_start_chunk(&self, random: &mut WorldgenRandom) -> bool {
        random.next_float() <= self.base().probability
    }

    fn carve(
        &self,
        context: &CarvingContext,
        chunk: &mut MutableChunkBlockBuffer,
        seed: i64,
        random: &mut WorldgenRandom,
        source_chunk_x: i32,
        source_chunk_z: i32,
        mask: &mut CarvingMask,
    ) -> bool {
        match *self {
            Self::Cave(config) => cave_carve(
                context,
                config,
                chunk,
                seed,
                random,
                source_chunk_x,
                source_chunk_z,
                mask,
            ),
            Self::Canyon(config) => canyon_carve(
                context,
                config,
                chunk,
                seed,
                random,
                source_chunk_x,
                source_chunk_z,
                mask,
            ),
        }
    }

    fn base(&self) -> &CarverConfiguration {
        match self {
            Self::Cave(config) => &config.base,
            Self::Canyon(config) => &config.base,
        }
    }
}

pub fn apply_overworld_air_carvers(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    chunk: &mut MutableChunkBlockBuffer,
) -> CarvingMask {
    apply_overworld_carvers(seed, biome_source, chunk, GenerationStepCarving::Air)
}

pub fn apply_overworld_carvers(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    chunk: &mut MutableChunkBlockBuffer,
    step: GenerationStepCarving,
) -> CarvingMask {
    match step {
        GenerationStepCarving::Air => apply_air_carvers(seed, biome_source, chunk),
    }
}

fn apply_air_carvers(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    chunk: &mut MutableChunkBlockBuffer,
) -> CarvingMask {
    let context = CarvingContext::new(chunk);
    let mut random = WorldgenRandom::default();
    let mut mask = CarvingMask::new(chunk.min_y, chunk.height);

    for source_chunk_x in chunk.chunk_x - CARVER_RANGE..=chunk.chunk_x + CARVER_RANGE {
        for source_chunk_z in chunk.chunk_z - CARVER_RANGE..=chunk.chunk_z + CARVER_RANGE {
            let biome = biome_source.get_noise_biome_definition(
                source_chunk_x << 2,
                0,
                source_chunk_z << 2,
            );
            let carvers = overworld_air_carvers_for_biome(biome.key());
            for (carver_index, carver) in carvers.iter().enumerate() {
                random.set_large_feature_seed(
                    seed.wrapping_add(carver_index as i64),
                    source_chunk_x,
                    source_chunk_z,
                );
                if carver.is_start_chunk(&mut random) {
                    carver.carve(
                        &context,
                        chunk,
                        seed,
                        &mut random,
                        source_chunk_x,
                        source_chunk_z,
                        &mut mask,
                    );
                }
            }
        }
    }

    mask
}

fn overworld_air_carvers_for_biome(biome_key: &str) -> [ConfiguredAirCarver; 2] {
    if is_ocean_biome(biome_key) {
        [ocean_cave_carver(), canyon_carver()]
    } else {
        [cave_carver(), canyon_carver()]
    }
}

fn is_ocean_biome(biome_key: &str) -> bool {
    matches!(
        biome_key,
        "minecraft:ocean"
            | "minecraft:deep_ocean"
            | "minecraft:warm_ocean"
            | "minecraft:deep_warm_ocean"
            | "minecraft:lukewarm_ocean"
            | "minecraft:deep_lukewarm_ocean"
            | "minecraft:cold_ocean"
            | "minecraft:deep_cold_ocean"
            | "minecraft:frozen_ocean"
            | "minecraft:deep_frozen_ocean"
    )
}

fn cave_carver() -> ConfiguredAirCarver {
    ConfiguredAirCarver::Cave(CaveCarverConfiguration {
        base: overworld_cave_base_config(0.142_857_15),
        horizontal_radius_multiplier: FloatProvider::Constant(1.0),
        vertical_radius_multiplier: FloatProvider::Constant(1.0),
        floor_level: FloatProvider::Constant(-0.7),
    })
}

fn ocean_cave_carver() -> ConfiguredAirCarver {
    ConfiguredAirCarver::Cave(CaveCarverConfiguration {
        base: overworld_cave_base_config(0.066_666_67),
        horizontal_radius_multiplier: FloatProvider::Constant(1.0),
        vertical_radius_multiplier: FloatProvider::Constant(1.0),
        floor_level: FloatProvider::Constant(-0.7),
    })
}

fn canyon_carver() -> ConfiguredAirCarver {
    ConfiguredAirCarver::Canyon(CanyonCarverConfiguration {
        base: CarverConfiguration {
            probability: 0.02,
            y: HeightProvider::biased_to_bottom(
                VerticalAnchor::Absolute(20),
                VerticalAnchor::Absolute(67),
                8,
            ),
            y_scale: FloatProvider::Constant(3.0),
            lava_level: VerticalAnchor::AboveBottom(10),
            aquifers_enabled: false,
            debug_settings: CarverDebugSettings { debug_mode: false },
        },
        vertical_rotation: FloatProvider::Uniform {
            min_inclusive: -0.125,
            max_exclusive: 0.125,
        },
        shape: CanyonShapeConfiguration {
            distance_factor: FloatProvider::Uniform {
                min_inclusive: 0.75,
                max_exclusive: 1.0,
            },
            thickness: FloatProvider::Trapezoid {
                min_inclusive: 0.0,
                max_exclusive: 6.0,
                plateau: 2.0,
            },
            width_smoothness: 3,
            horizontal_radius_factor: FloatProvider::Uniform {
                min_inclusive: 0.75,
                max_exclusive: 1.0,
            },
            vertical_radius_default_factor: 1.0,
            vertical_radius_center_factor: 0.0,
        },
    })
}

fn overworld_cave_base_config(probability: f32) -> CarverConfiguration {
    CarverConfiguration {
        probability,
        y: HeightProvider::biased_to_bottom(
            VerticalAnchor::Absolute(0),
            VerticalAnchor::Absolute(127),
            8,
        ),
        y_scale: FloatProvider::Constant(0.5),
        lava_level: VerticalAnchor::AboveBottom(10),
        aquifers_enabled: false,
        debug_settings: CarverDebugSettings { debug_mode: false },
    }
}

#[allow(clippy::too_many_arguments)]
fn cave_carve(
    context: &CarvingContext,
    config: CaveCarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    _seed: i64,
    random: &mut WorldgenRandom,
    source_chunk_x: i32,
    source_chunk_z: i32,
    mask: &mut CarvingMask,
) -> bool {
    let range = (get_range() * 2 - 1) * CHUNK_WIDTH;
    let first_bound = random.next_int_bound(get_cave_bound()) + 1;
    let second_bound = random.next_int_bound(first_bound) + 1;
    let cave_count = random.next_int_bound(second_bound);
    let mut carved = false;

    for _ in 0..cave_count {
        let x = chunk_block_x(source_chunk_x, random.next_int_bound(CHUNK_WIDTH)) as f64;
        let y = config.base.y.sample(random, context) as f64;
        let z = chunk_block_z(source_chunk_z, random.next_int_bound(CHUNK_WIDTH)) as f64;
        let horizontal_radius_multiplier = config.horizontal_radius_multiplier.sample(random);
        let vertical_radius_multiplier = config.vertical_radius_multiplier.sample(random);
        let floor_level = config.floor_level.sample(random);

        let mut tunnel_count = 1;
        if random.next_int_bound(4) == 0 {
            let y_scale = config.base.y_scale.sample(random);
            let radius = 1.0 + random.next_float() * 6.0;
            carved |= create_room(
                context,
                &config.base,
                chunk,
                random.next_long(),
                x,
                y,
                z,
                radius,
                y_scale,
                floor_level,
                mask,
            );
            tunnel_count += random.next_int_bound(4);
        }

        for _ in 0..tunnel_count {
            let yaw = random.next_float() * TWO_PI;
            let pitch = (random.next_float() - 0.5) / 4.0;
            let thickness = get_thickness(random);
            let branch_count = range - random.next_int_bound(range / 4);
            carved |= create_tunnel(
                context,
                &config.base,
                chunk,
                random.next_long(),
                x,
                y,
                z,
                horizontal_radius_multiplier,
                vertical_radius_multiplier,
                thickness,
                yaw,
                pitch,
                0,
                branch_count,
                floor_level,
                mask,
            );
        }
    }

    carved
}

fn get_cave_bound() -> i32 {
    15
}

fn get_thickness(random: &mut WorldgenRandom) -> f32 {
    let mut thickness = random.next_float() * 2.0 + random.next_float();
    if random.next_int_bound(10) == 0 {
        thickness *= random.next_float() * random.next_float() * 3.0 + 1.0;
    }
    thickness
}

#[allow(clippy::too_many_arguments)]
fn create_room(
    context: &CarvingContext,
    config: &CarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    seed: i64,
    x: f64,
    y: f64,
    z: f64,
    radius: f32,
    vertical_radius_multiplier: f32,
    floor_level: f32,
    mask: &mut CarvingMask,
) -> bool {
    let horizontal_radius = 1.5 + (sin(PI / 2.0) * radius) as f64;
    carve_ellipsoid(
        context,
        config,
        chunk,
        seed,
        x + 1.0,
        y,
        z,
        horizontal_radius,
        horizontal_radius * vertical_radius_multiplier as f64,
        mask,
        |relative_x, relative_y, relative_z, _world_y| {
            cave_should_skip(relative_x, relative_y, relative_z, floor_level as f64)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn create_tunnel(
    context: &CarvingContext,
    config: &CarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    seed: i64,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    horizontal_radius_multiplier: f32,
    vertical_radius_multiplier: f32,
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    start_branch: i32,
    branch_count: i32,
    floor_level: f32,
    mask: &mut CarvingMask,
) -> bool {
    let mut random = SimpleRandomSource::new(seed);
    let split_branch_index = random.next_int_bound(branch_count / 2) + branch_count / 4;
    let wide_pitch = random.next_int_bound(6) == 0;
    let mut carved = false;
    let mut pitch_velocity = 0.0_f32;
    let mut yaw_velocity = 0.0_f32;

    for current_branch in start_branch..branch_count {
        let radius_wave = sin(PI * current_branch as f32 / branch_count as f32);
        let horizontal_radius = 1.5 + (radius_wave * thickness) as f64;
        let vertical_radius = horizontal_radius * vertical_radius_multiplier as f64;
        let cos_pitch = cos(pitch);
        x += (cos(yaw) * cos_pitch) as f64;
        y += sin(pitch) as f64;
        z += (sin(yaw) * cos_pitch) as f64;
        pitch *= if wide_pitch { 0.92 } else { 0.7 };
        pitch += pitch_velocity * 0.1;
        yaw += yaw_velocity * 0.1;
        pitch_velocity *= 0.9;
        yaw_velocity *= 0.75;
        pitch_velocity += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
        yaw_velocity += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

        if current_branch == split_branch_index && thickness > 1.0 {
            carved |= create_tunnel(
                context,
                config,
                chunk,
                random.next_long(),
                x,
                y,
                z,
                horizontal_radius_multiplier,
                vertical_radius_multiplier,
                random.next_float() * 0.5 + 0.5,
                yaw - PI / 2.0,
                pitch / 3.0,
                current_branch,
                branch_count,
                floor_level,
                mask,
            );
            carved |= create_tunnel(
                context,
                config,
                chunk,
                random.next_long(),
                x,
                y,
                z,
                horizontal_radius_multiplier,
                vertical_radius_multiplier,
                random.next_float() * 0.5 + 0.5,
                yaw + PI / 2.0,
                pitch / 3.0,
                current_branch,
                branch_count,
                floor_level,
                mask,
            );
            return carved;
        }

        if random.next_int_bound(4) != 0 {
            if !can_reach(chunk, x, z, current_branch, branch_count, thickness as f64) {
                return carved;
            }

            carved |= carve_ellipsoid(
                context,
                config,
                chunk,
                seed,
                x,
                y,
                z,
                horizontal_radius * horizontal_radius_multiplier as f64,
                vertical_radius,
                mask,
                |relative_x, relative_y, relative_z, _world_y| {
                    cave_should_skip(relative_x, relative_y, relative_z, floor_level as f64)
                },
            );
        }
    }

    carved
}

fn cave_should_skip(relative_x: f64, relative_y: f64, relative_z: f64, floor_level: f64) -> bool {
    relative_y <= floor_level
        || relative_x * relative_x + relative_y * relative_y + relative_z * relative_z >= 1.0
}

#[allow(clippy::too_many_arguments)]
fn canyon_carve(
    context: &CarvingContext,
    config: CanyonCarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    _seed: i64,
    random: &mut WorldgenRandom,
    source_chunk_x: i32,
    source_chunk_z: i32,
    mask: &mut CarvingMask,
) -> bool {
    let x = chunk_block_x(source_chunk_x, random.next_int_bound(CHUNK_WIDTH)) as f64;
    let y = config.base.y.sample(random, context) as f64;
    let z = chunk_block_z(source_chunk_z, random.next_int_bound(CHUNK_WIDTH)) as f64;
    let yaw = random.next_float() * TWO_PI;
    let pitch = config.vertical_rotation.sample(random);
    let y_scale = config.base.y_scale.sample(random);
    let thickness = config.shape.thickness.sample(random);
    let branch_count =
        ((get_range() * 2 - 1) * CHUNK_WIDTH) as f32 * config.shape.distance_factor.sample(random);

    canyon_do_carve(
        context,
        config,
        chunk,
        random.next_long(),
        x,
        y,
        z,
        thickness,
        yaw,
        pitch,
        0,
        branch_count as i32,
        y_scale,
        mask,
    )
}

#[allow(clippy::too_many_arguments)]
fn canyon_do_carve(
    context: &CarvingContext,
    config: CanyonCarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    seed: i64,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    start_branch: i32,
    branch_count: i32,
    y_scale: f32,
    mask: &mut CarvingMask,
) -> bool {
    let mut random = SimpleRandomSource::new(seed);
    let width_factors = init_width_factors(context, config, &mut random);
    let mut carved = false;
    let mut yaw_velocity = 0.0_f32;
    let mut pitch_velocity = 0.0_f32;

    for current_branch in start_branch..branch_count {
        let radius_wave = sin(current_branch as f32 * PI / branch_count as f32);
        let mut horizontal_radius = 1.5 + (radius_wave * thickness) as f64;
        let mut vertical_radius = horizontal_radius * y_scale as f64;
        horizontal_radius *= config.shape.horizontal_radius_factor.sample(&mut random) as f64;
        vertical_radius = update_vertical_radius(
            config,
            &mut random,
            vertical_radius,
            branch_count,
            current_branch,
        );
        let cos_pitch = cos(pitch);
        x += (cos(yaw) * cos_pitch) as f64;
        y += sin(pitch) as f64;
        z += (sin(yaw) * cos_pitch) as f64;
        pitch *= 0.7;
        pitch += pitch_velocity * 0.05;
        yaw += yaw_velocity * 0.05;
        pitch_velocity *= 0.8;
        yaw_velocity *= 0.5;
        pitch_velocity += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
        yaw_velocity += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

        if random.next_int_bound(4) != 0 {
            if !can_reach(chunk, x, z, current_branch, branch_count, thickness as f64) {
                return carved;
            }

            carved |= carve_ellipsoid(
                context,
                &config.base,
                chunk,
                seed,
                x,
                y,
                z,
                horizontal_radius,
                vertical_radius,
                mask,
                |relative_x, relative_y, relative_z, world_y| {
                    canyon_should_skip(
                        relative_x,
                        relative_y,
                        relative_z,
                        world_y,
                        context.min_y(),
                        &width_factors,
                    )
                },
            );
        }
    }

    carved
}

fn init_width_factors(
    context: &CarvingContext,
    config: CanyonCarverConfiguration,
    random: &mut SimpleRandomSource,
) -> Vec<f32> {
    let mut width_factors = vec![0.0; context.gen_depth() as usize];
    let mut width = 1.0_f32;
    for index in 0..context.gen_depth() {
        if index == 0 || random.next_int_bound(config.shape.width_smoothness) == 0 {
            width = 1.0 + random.next_float() * random.next_float();
        }
        width_factors[index as usize] = width * width;
    }
    width_factors
}

fn update_vertical_radius(
    config: CanyonCarverConfiguration,
    random: &mut SimpleRandomSource,
    vertical_radius: f64,
    branch_count: i32,
    current_branch: i32,
) -> f64 {
    let centered_progress = 1.0 - (0.5 - current_branch as f32 / branch_count as f32).abs() * 2.0;
    let factor = config.shape.vertical_radius_default_factor
        + config.shape.vertical_radius_center_factor * centered_progress;
    factor as f64 * vertical_radius * random_between(random, 0.75, 1.0) as f64
}

fn random_between(random: &mut SimpleRandomSource, min_inclusive: f32, max_exclusive: f32) -> f32 {
    min_inclusive + random.next_float() * (max_exclusive - min_inclusive)
}

fn canyon_should_skip(
    relative_x: f64,
    relative_y: f64,
    relative_z: f64,
    world_y: i32,
    min_y: i32,
    width_factors: &[f32],
) -> bool {
    let relative_index = world_y - min_y;
    if relative_index <= 0 || relative_index as usize > width_factors.len() {
        return true;
    }
    (relative_x * relative_x + relative_z * relative_z)
        * width_factors[(relative_index - 1) as usize] as f64
        + relative_y * relative_y / 6.0
        >= 1.0
}

#[allow(clippy::too_many_arguments)]
fn carve_ellipsoid(
    context: &CarvingContext,
    config: &CarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    seed: i64,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    mask: &mut CarvingMask,
    should_skip: impl Fn(f64, f64, f64, i32) -> bool,
) -> bool {
    let mut carve_random = SimpleRandomSource::new(
        seed.wrapping_add(chunk.chunk_x as i64)
            .wrapping_add(chunk.chunk_z as i64),
    );

    if !can_carve_chunk(chunk, x, z, horizontal_radius) {
        return false;
    }

    let min_block_x = chunk_min_block_x(chunk.chunk_x);
    let min_block_z = chunk_min_block_z(chunk.chunk_z);
    let min_x = ((floor(x - horizontal_radius) - min_block_x - 1).max(0)).min(15);
    let max_x = ((floor(x + horizontal_radius) - min_block_x).max(0)).min(15);
    let min_y = (floor(y - vertical_radius) - 1).max(context.min_y() + 1);
    let max_y = (floor(y + vertical_radius) + 1).min(context.min_y() + context.gen_depth() - 8);
    let min_z = ((floor(z - horizontal_radius) - min_block_z - 1).max(0)).min(15);
    let max_z = ((floor(z + horizontal_radius) - min_block_z).max(0)).min(15);

    if !config.aquifers_enabled
        && has_disallowed_liquid(chunk, min_x, max_x, min_y, max_y, min_z, max_z)
    {
        return false;
    }

    let mut carved = false;
    for local_x in min_x..=max_x {
        let relative_x = ((min_block_x + local_x) as f64 + 0.5 - x) / horizontal_radius;
        for local_z in min_z..=max_z {
            let relative_z = ((min_block_z + local_z) as f64 + 0.5 - z) / horizontal_radius;
            if relative_x * relative_x + relative_z * relative_z >= 1.0 {
                continue;
            }

            let mut reached_surface = false;
            let mut world_y = max_y;
            while world_y > min_y {
                let relative_y = (world_y as f64 - 0.5 - y) / vertical_radius;
                if !should_skip(relative_x, relative_y, relative_z, world_y)
                    && !mask.is_set(local_x, world_y, local_z)
                {
                    mask.set(local_x, world_y, local_z);
                    carved |= carve_block(
                        context,
                        config,
                        chunk,
                        &mut carve_random,
                        local_x,
                        world_y,
                        local_z,
                        &mut reached_surface,
                    );
                }
                world_y -= 1;
            }
        }
    }

    carved
}

fn can_carve_chunk(
    chunk: &MutableChunkBlockBuffer,
    x: f64,
    z: f64,
    horizontal_radius: f64,
) -> bool {
    let chunk_middle_x = chunk_middle_block_x(chunk.chunk_x) as f64;
    let chunk_middle_z = chunk_middle_block_z(chunk.chunk_z) as f64;
    (x - chunk_middle_x).abs() <= 16.0 + horizontal_radius * 2.0
        && (z - chunk_middle_z).abs() <= 16.0 + horizontal_radius * 2.0
}

fn has_disallowed_liquid(
    chunk: &MutableChunkBlockBuffer,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
) -> bool {
    for local_x in min_x..=max_x {
        for local_z in min_z..=max_z {
            for world_y in min_y - 1..=max_y + 1 {
                let on_x_edge = local_x == min_x || local_x == max_x;
                let on_z_edge = local_z == min_z || local_z == max_z;
                if (!on_x_edge && !on_z_edge) || (world_y == max_y + 1 && !on_x_edge && !on_z_edge)
                {
                    continue;
                }

                if is_water(chunk.get_block_at_y(local_x, world_y, local_z)) {
                    return true;
                }

                if world_y != min_y - 1 && !on_x_edge && !on_z_edge {
                    break;
                }
            }
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn carve_block(
    context: &CarvingContext,
    config: &CarverConfiguration,
    chunk: &mut MutableChunkBlockBuffer,
    random: &mut SimpleRandomSource,
    local_x: i32,
    y: i32,
    local_z: i32,
    reached_surface: &mut bool,
) -> bool {
    let current = chunk.get_block_at_y(local_x, y, local_z);
    let above = if y + 1 >= chunk.min_y + chunk.height {
        AIR
    } else {
        chunk.get_block_at_y(local_x, y + 1, local_z)
    };

    if is_grass_or_mycelium(current) {
        *reached_surface = true;
    }

    if !can_replace_block(current, above) {
        return false;
    }

    let carve_state = get_carve_state(context, config, y, random);
    chunk.set_block_at_y(local_x, y, local_z, carve_state);

    if *reached_surface
        && y - 1 >= chunk.min_y
        && chunk.get_block_at_y(local_x, y - 1, local_z) == DIRT
    {
        chunk.set_block_at_y(local_x, y - 1, local_z, GRASS_BLOCK);
    }

    true
}

fn get_carve_state(
    context: &CarvingContext,
    config: &CarverConfiguration,
    y: i32,
    _random: &mut SimpleRandomSource,
) -> u8 {
    if y <= config.lava_level(context) {
        LAVA
    } else if !config.aquifers_enabled {
        if config.debug_settings.debug_mode {
            AIR
        } else {
            AIR
        }
    } else {
        panic!("aquifer-backed carve states are outside the 1.17.1 overworld MVP target");
    }
}

fn can_replace_block(current: u8, above: u8) -> bool {
    match current {
        SAND | GRAVEL => !is_water(above),
        _ => can_replace_block_without_above(current),
    }
}

fn can_replace_block_without_above(block: u8) -> bool {
    matches!(
        block,
        STONE
            | GRANITE
            | DIORITE
            | ANDESITE
            | DIRT
            | COARSE_DIRT
            | PODZOL
            | GRASS_BLOCK
            | TERRACOTTA
            | WHITE_TERRACOTTA
            | ORANGE_TERRACOTTA
            | MAGENTA_TERRACOTTA
            | LIGHT_BLUE_TERRACOTTA
            | YELLOW_TERRACOTTA
            | LIME_TERRACOTTA
            | PINK_TERRACOTTA
            | GRAY_TERRACOTTA
            | LIGHT_GRAY_TERRACOTTA
            | CYAN_TERRACOTTA
            | PURPLE_TERRACOTTA
            | BLUE_TERRACOTTA
            | BROWN_TERRACOTTA
            | GREEN_TERRACOTTA
            | RED_TERRACOTTA
            | BLACK_TERRACOTTA
            | SANDSTONE
            | RED_SANDSTONE
            | MYCELIUM
            | SNOW
            | PACKED_ICE
    )
}

fn is_grass_or_mycelium(block: u8) -> bool {
    block == GRASS_BLOCK || block == MYCELIUM
}

fn is_water(block: u8) -> bool {
    block == WATER
}

fn can_reach(
    chunk: &MutableChunkBlockBuffer,
    x: f64,
    z: f64,
    current_branch: i32,
    branch_count: i32,
    width: f64,
) -> bool {
    let chunk_middle_x = chunk_middle_block_x(chunk.chunk_x) as f64;
    let chunk_middle_z = chunk_middle_block_z(chunk.chunk_z) as f64;
    let delta_x = x - chunk_middle_x;
    let delta_z = z - chunk_middle_z;
    let remaining_branch_count = branch_count - current_branch;
    let reach = width + 18.0;
    delta_x * delta_x + delta_z * delta_z - remaining_branch_count.pow(2) as f64 <= reach * reach
}

fn get_range() -> i32 {
    CARVER_RANGE
}

fn chunk_block_x(chunk_x: i32, local_x: i32) -> i32 {
    chunk_x * CHUNK_WIDTH + local_x
}

fn chunk_block_z(chunk_z: i32, local_z: i32) -> i32 {
    chunk_z * CHUNK_WIDTH + local_z
}

fn chunk_min_block_x(chunk_x: i32) -> i32 {
    chunk_x * CHUNK_WIDTH
}

fn chunk_min_block_z(chunk_z: i32) -> i32 {
    chunk_z * CHUNK_WIDTH
}

fn chunk_middle_block_x(chunk_x: i32) -> i32 {
    chunk_min_block_x(chunk_x) + 8
}

fn chunk_middle_block_z(chunk_z: i32) -> i32 {
    chunk_min_block_z(chunk_z) + 8
}

fn floor(value: f64) -> i32 {
    value.floor() as i32
}

fn sin(value: f32) -> f32 {
    let index = ((value * SIN_SCALE).trunc() as i32 & SIN_TABLE_MASK) as usize;
    sin_table()[index]
}

fn cos(value: f32) -> f32 {
    let index = ((value * SIN_SCALE + COS_OFFSET).trunc() as i32 & SIN_TABLE_MASK) as usize;
    sin_table()[index]
}

fn sin_table() -> &'static [f32] {
    static SIN_TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    SIN_TABLE
        .get_or_init(|| {
            (0..SIN_TABLE_SIZE)
                .map(|index| {
                    ((index as f64) * std::f64::consts::TAU / SIN_TABLE_SIZE as f64).sin() as f32
                })
                .collect()
        })
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::{NoiseBasedChunkGenerator, NoiseGeneratorSettings};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ChunkFixture {
        module: String,
        minecraft_version: String,
        seed: String,
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        block_order: String,
        palette: Vec<String>,
        blocks: Vec<u8>,
    }

    fn carved_fixture() -> ChunkFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json"
        ))
        .expect("valid carved fixture")
    }

    #[test]
    fn overworld_air_carvers_match_java_fixture_from_native_surface_stage() {
        let carved = carved_fixture();
        assert_eq!(carved.module, "carved-chunk");
        assert_eq!(carved.minecraft_version, "1.17.1");
        assert_eq!(carved.block_order, "y-major,z-major,x-minor");
        assert_eq!(carved.palette[0], "minecraft:air");

        let seed = carved.seed.parse::<i64>().expect("i64 seed");
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let generator = NoiseBasedChunkGenerator::new(
            biome_source.clone(),
            seed,
            NoiseGeneratorSettings::overworld(),
        );
        let mut chunk = generator.fill_from_noise(carved.chunk_x, carved.chunk_z);
        generator.build_surface_and_bedrock(&mut chunk);

        let mask = apply_overworld_air_carvers(seed, &biome_source, &mut chunk);
        assert_eq!(mask.bits().len(), (carved.height * 16 * 16) as usize);
        assert!(mask.carved_count() > 0);
        assert_blocks_match(&chunk.blocks, &carved.blocks, carved.min_y, carved.height);
    }

    #[test]
    fn ocean_biomes_use_ocean_cave_probability() {
        let [first, second] = overworld_air_carvers_for_biome("minecraft:deep_ocean");
        assert_eq!(
            first.base().probability.to_bits(),
            0.066_666_67_f32.to_bits()
        );
        assert!(matches!(first, ConfiguredAirCarver::Cave(_)));
        assert!(matches!(second, ConfiguredAirCarver::Canyon(_)));
    }

    fn assert_blocks_match(actual: &[u8], expected: &[u8], min_y: i32, height: i32) {
        assert_eq!(actual.len(), expected.len());
        let mismatches: Vec<_> = actual
            .iter()
            .zip(expected.iter())
            .enumerate()
            .filter_map(|(index, (&actual, &expected))| {
                (actual != expected).then(|| {
                    let local_y = index as i32 / 256;
                    let within_layer = index as i32 % 256;
                    let local_z = within_layer / 16;
                    let local_x = within_layer % 16;
                    (local_x, min_y + local_y, local_z, actual, expected)
                })
            })
            .collect();

        if !mismatches.is_empty() {
            let sample: Vec<_> = mismatches.iter().take(16).copied().collect();
            let mut actual_counts = [0_usize; 10];
            let mut expected_counts = [0_usize; 10];
            for &block in actual {
                if (block as usize) < actual_counts.len() {
                    actual_counts[block as usize] += 1;
                }
            }
            for &block in expected {
                if (block as usize) < expected_counts.len() {
                    expected_counts[block as usize] += 1;
                }
            }
            panic!(
                "block mismatch count={} height={} sample={sample:?} actual_counts={actual_counts:?} expected_counts={expected_counts:?}",
                mismatches.len(),
                height
            );
        }
    }
}
