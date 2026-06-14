use crate::levelgen::{NoiseBiome, NoiseBiomeSource};
use crate::noise::ImprovedNoise;
use crate::prng::SimpleRandomSource;
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

const MAX_CACHE: usize = 1024;
const WARM_ID: i32 = 1;
const MEDIUM_ID: i32 = 2;
const COLD_ID: i32 = 3;
const ICE_ID: i32 = 4;
const SPECIAL_MASK: i32 = 3840;
const SPECIAL_SHIFT: i32 = 8;
const WIDTH_BITS: i32 = 2;
const HORIZONTAL_MASK: i32 = (1 << WIDTH_BITS) - 1;
const HORIZONTAL_AREA: usize = 1 << (WIDTH_BITS + WIDTH_BITS);
const LCG_MULTIPLIER: i64 = 6_364_136_223_846_793_005;
const LCG_INCREMENT: i64 = 1_442_695_040_888_963_407;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeDefinition {
    id: i32,
    key: &'static str,
    depth: f32,
    scale: f32,
}

impl BiomeDefinition {
    pub const fn new(id: i32, key: &'static str, depth: f32, scale: f32) -> Self {
        Self {
            id,
            key,
            depth,
            scale,
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn key(&self) -> &'static str {
        self.key
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn noise_biome(&self) -> NoiseBiome {
        NoiseBiome::new(self.depth, self.scale)
    }
}

pub const ALL_OVERWORLD_LAYERED_BIOMES: &[BiomeDefinition] = &[
    BiomeDefinition::new(0, "minecraft:ocean", -1.0, 0.10000000149011612),
    BiomeDefinition::new(1, "minecraft:plains", 0.125, 0.05000000074505806),
    BiomeDefinition::new(2, "minecraft:desert", 0.125, 0.05000000074505806),
    BiomeDefinition::new(3, "minecraft:mountains", 1.0, 0.5),
    BiomeDefinition::new(
        4,
        "minecraft:forest",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        5,
        "minecraft:taiga",
        0.20000000298023224,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        6,
        "minecraft:swamp",
        -0.20000000298023224,
        0.10000000149011612,
    ),
    BiomeDefinition::new(7, "minecraft:river", -0.5, 0.0),
    BiomeDefinition::new(10, "minecraft:frozen_ocean", -1.0, 0.10000000149011612),
    BiomeDefinition::new(11, "minecraft:frozen_river", -0.5, 0.0),
    BiomeDefinition::new(12, "minecraft:snowy_tundra", 0.125, 0.05000000074505806),
    BiomeDefinition::new(
        13,
        "minecraft:snowy_mountains",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        14,
        "minecraft:mushroom_fields",
        0.20000000298023224,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        15,
        "minecraft:mushroom_field_shore",
        0.0,
        0.02500000037252903,
    ),
    BiomeDefinition::new(16, "minecraft:beach", 0.0, 0.02500000037252903),
    BiomeDefinition::new(
        17,
        "minecraft:desert_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        18,
        "minecraft:wooded_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        19,
        "minecraft:taiga_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        20,
        "minecraft:mountain_edge",
        0.800000011920929,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        21,
        "minecraft:jungle",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        22,
        "minecraft:jungle_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        23,
        "minecraft:jungle_edge",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        24,
        "minecraft:deep_ocean",
        -1.7999999523162842,
        0.10000000149011612,
    ),
    BiomeDefinition::new(
        25,
        "minecraft:stone_shore",
        0.10000000149011612,
        0.800000011920929,
    ),
    BiomeDefinition::new(26, "minecraft:snowy_beach", 0.0, 0.02500000037252903),
    BiomeDefinition::new(
        27,
        "minecraft:birch_forest",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        28,
        "minecraft:birch_forest_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        29,
        "minecraft:dark_forest",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        30,
        "minecraft:snowy_taiga",
        0.20000000298023224,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        31,
        "minecraft:snowy_taiga_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        32,
        "minecraft:giant_tree_taiga",
        0.20000000298023224,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        33,
        "minecraft:giant_tree_taiga_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(34, "minecraft:wooded_mountains", 1.0, 0.5),
    BiomeDefinition::new(35, "minecraft:savanna", 0.125, 0.05000000074505806),
    BiomeDefinition::new(36, "minecraft:savanna_plateau", 1.5, 0.02500000037252903),
    BiomeDefinition::new(
        37,
        "minecraft:badlands",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        38,
        "minecraft:wooded_badlands_plateau",
        1.5,
        0.02500000037252903,
    ),
    BiomeDefinition::new(39, "minecraft:badlands_plateau", 1.5, 0.02500000037252903),
    BiomeDefinition::new(44, "minecraft:warm_ocean", -1.0, 0.10000000149011612),
    BiomeDefinition::new(45, "minecraft:lukewarm_ocean", -1.0, 0.10000000149011612),
    BiomeDefinition::new(46, "minecraft:cold_ocean", -1.0, 0.10000000149011612),
    BiomeDefinition::new(
        47,
        "minecraft:deep_warm_ocean",
        -1.7999999523162842,
        0.10000000149011612,
    ),
    BiomeDefinition::new(
        48,
        "minecraft:deep_lukewarm_ocean",
        -1.7999999523162842,
        0.10000000149011612,
    ),
    BiomeDefinition::new(
        49,
        "minecraft:deep_cold_ocean",
        -1.7999999523162842,
        0.10000000149011612,
    ),
    BiomeDefinition::new(
        50,
        "minecraft:deep_frozen_ocean",
        -1.7999999523162842,
        0.10000000149011612,
    ),
    BiomeDefinition::new(
        129,
        "minecraft:sunflower_plains",
        0.125,
        0.05000000074505806,
    ),
    BiomeDefinition::new(130, "minecraft:desert_lakes", 0.22499999403953552, 0.25),
    BiomeDefinition::new(131, "minecraft:gravelly_mountains", 1.0, 0.5),
    BiomeDefinition::new(
        132,
        "minecraft:flower_forest",
        0.10000000149011612,
        0.4000000059604645,
    ),
    BiomeDefinition::new(
        133,
        "minecraft:taiga_mountains",
        0.30000001192092896,
        0.4000000059604645,
    ),
    BiomeDefinition::new(
        134,
        "minecraft:swamp_hills",
        -0.10000000149011612,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        140,
        "minecraft:ice_spikes",
        0.42500001192092896,
        0.45000001788139343,
    ),
    BiomeDefinition::new(
        149,
        "minecraft:modified_jungle",
        0.20000000298023224,
        0.4000000059604645,
    ),
    BiomeDefinition::new(
        151,
        "minecraft:modified_jungle_edge",
        0.20000000298023224,
        0.4000000059604645,
    ),
    BiomeDefinition::new(
        155,
        "minecraft:tall_birch_forest",
        0.20000000298023224,
        0.4000000059604645,
    ),
    BiomeDefinition::new(156, "minecraft:tall_birch_hills", 0.550000011920929, 0.5),
    BiomeDefinition::new(
        157,
        "minecraft:dark_forest_hills",
        0.20000000298023224,
        0.4000000059604645,
    ),
    BiomeDefinition::new(
        158,
        "minecraft:snowy_taiga_mountains",
        0.30000001192092896,
        0.4000000059604645,
    ),
    BiomeDefinition::new(
        160,
        "minecraft:giant_spruce_taiga",
        0.20000000298023224,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        161,
        "minecraft:giant_spruce_taiga_hills",
        0.20000000298023224,
        0.20000000298023224,
    ),
    BiomeDefinition::new(162, "minecraft:modified_gravelly_mountains", 1.0, 0.5),
    BiomeDefinition::new(
        163,
        "minecraft:shattered_savanna",
        0.36250001192092896,
        1.225000023841858,
    ),
    BiomeDefinition::new(
        164,
        "minecraft:shattered_savanna_plateau",
        1.0499999523162842,
        1.2125000953674316,
    ),
    BiomeDefinition::new(
        165,
        "minecraft:eroded_badlands",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        166,
        "minecraft:modified_wooded_badlands_plateau",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        167,
        "minecraft:modified_badlands_plateau",
        0.44999998807907104,
        0.30000001192092896,
    ),
    BiomeDefinition::new(
        168,
        "minecraft:bamboo_jungle",
        0.10000000149011612,
        0.20000000298023224,
    ),
    BiomeDefinition::new(
        169,
        "minecraft:bamboo_jungle_hills",
        0.44999998807907104,
        0.30000001192092896,
    ),
];

pub fn possible_overworld_biomes() -> impl Iterator<Item = BiomeDefinition> {
    ALL_OVERWORLD_LAYERED_BIOMES
        .iter()
        .copied()
        .filter(|biome| biome.id != 168 && biome.id != 169)
}

pub fn get_layered_biome_by_id(id: i32) -> BiomeDefinition {
    ALL_OVERWORLD_LAYERED_BIOMES
        .iter()
        .copied()
        .find(|biome| biome.id == id)
        .unwrap_or_else(|| panic!("unknown layered biome id {id}"))
}

#[derive(Clone)]
pub struct OverworldBiomeSource {
    seed: i64,
    legacy_biome_init_layer: bool,
    large_biomes: bool,
    noise_biome_area: AreaRef,
}

impl OverworldBiomeSource {
    pub fn new(seed: i64, legacy_biome_init_layer: bool, large_biomes: bool) -> Self {
        let biome_zooms = if large_biomes { 6 } else { 4 };
        Self {
            seed,
            legacy_biome_init_layer,
            large_biomes,
            noise_biome_area: build_overworld_biome_area(
                seed,
                legacy_biome_init_layer,
                biome_zooms,
                4,
            ),
        }
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn legacy_biome_init_layer(&self) -> bool {
        self.legacy_biome_init_layer
    }

    pub fn large_biomes(&self) -> bool {
        self.large_biomes
    }

    pub fn get_noise_biome_id(&self, x: i32, _y: i32, z: i32) -> i32 {
        self.noise_biome_area.get(x, z)
    }

    pub fn get_noise_biome_definition(&self, x: i32, y: i32, z: i32) -> BiomeDefinition {
        get_layered_biome_by_id(self.get_noise_biome_id(x, y, z))
    }

    pub fn get_block_position_biome_definition(
        &self,
        seed: i64,
        block_x: i32,
        block_z: i32,
    ) -> BiomeDefinition {
        get_layered_biome_by_id(get_fuzzy_zoomed_biome_id(
            obfuscate_biome_zoom_seed(seed),
            block_x,
            0,
            block_z,
            |quart_x, quart_y, quart_z| self.get_noise_biome_id(quart_x, quart_y, quart_z),
        ))
    }
}

impl NoiseBiomeSource for OverworldBiomeSource {
    fn get_noise_biome(&self, x: i32, y: i32, z: i32) -> NoiseBiome {
        self.get_noise_biome_definition(x, y, z).noise_biome()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkBiomeContainer {
    biomes: Vec<i32>,
    quart_min_y: i32,
    quart_height: i32,
}

impl ChunkBiomeContainer {
    pub fn new(
        min_build_height: i32,
        height: i32,
        chunk_x: i32,
        chunk_z: i32,
        biome_source: &OverworldBiomeSource,
    ) -> Self {
        let quart_min_y = quart_from_block(min_build_height);
        let quart_height = quart_from_block(height) - 1;
        let mut biomes = vec![0; HORIZONTAL_AREA * ceil_div(height, 4) as usize];
        let min_quart_x = quart_from_block(chunk_x * 16);
        let min_quart_z = quart_from_block(chunk_z * 16);

        for index in 0..biomes.len() {
            biomes[index] = Self::generate_biome_for_index(
                biome_source,
                min_quart_x,
                quart_min_y,
                min_quart_z,
                index,
            );
        }

        Self {
            biomes,
            quart_min_y,
            quart_height,
        }
    }

    pub fn from_biome_ids(min_build_height: i32, height: i32, biome_ids: Vec<i32>) -> Self {
        Self {
            biomes: biome_ids,
            quart_min_y: quart_from_block(min_build_height),
            quart_height: quart_from_block(height) - 1,
        }
    }

    fn generate_biome_for_index(
        biome_source: &OverworldBiomeSource,
        min_quart_x: i32,
        quart_min_y: i32,
        min_quart_z: i32,
        index: usize,
    ) -> i32 {
        let index = index as i32;
        let local_x = index & HORIZONTAL_MASK;
        let local_y = index >> (WIDTH_BITS + WIDTH_BITS);
        let local_z = (index >> WIDTH_BITS) & HORIZONTAL_MASK;
        biome_source.get_noise_biome_id(
            min_quart_x + local_x,
            quart_min_y + local_y,
            min_quart_z + local_z,
        )
    }

    pub fn write_biomes(&self) -> Vec<i32> {
        self.biomes.clone()
    }

    pub fn get_noise_biome_id(&self, x: i32, y: i32, z: i32) -> i32 {
        let local_x = x & HORIZONTAL_MASK;
        let local_y = clamp(y - self.quart_min_y, 0, self.quart_height);
        let local_z = z & HORIZONTAL_MASK;
        self.biomes
            [((local_y << (WIDTH_BITS + WIDTH_BITS)) | (local_z << WIDTH_BITS) | local_x) as usize]
    }

    pub fn get_noise_biome(&self, x: i32, y: i32, z: i32) -> NoiseBiome {
        get_layered_biome_by_id(self.get_noise_biome_id(x, y, z)).noise_biome()
    }
}

type AreaRef = Rc<LazyArea>;
type AreaFactory = Rc<dyn Fn() -> AreaRef>;
type ContextRef = Rc<LazyAreaContext>;

struct LazyAreaCache {
    values: HashMap<i64, i32>,
    order: VecDeque<i64>,
}

impl LazyAreaCache {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
            order: VecDeque::new(),
        }
    }
}

struct LazyArea {
    cache: Rc<RefCell<LazyAreaCache>>,
    max_cache: usize,
    transformer: Box<dyn Fn(i32, i32) -> i32>,
}

impl LazyArea {
    fn get(&self, x: i32, z: i32) -> i32 {
        let key = chunk_pos_as_long(x, z);
        if let Some(value) = self.cache.borrow().values.get(&key).copied() {
            return value;
        }

        let value = (self.transformer)(x, z);
        let mut cache = self.cache.borrow_mut();
        if cache.values.insert(key, value).is_none() {
            cache.order.push_back(key);
        }

        if cache.values.len() > self.max_cache {
            let remove_count = self.max_cache / 16;
            for _ in 0..remove_count {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.values.remove(&oldest);
                } else {
                    break;
                }
            }
        }

        value
    }

    fn get_max_cache(&self) -> usize {
        self.max_cache
    }
}

struct LazyAreaContext {
    max_cache: usize,
    cache: Rc<RefCell<LazyAreaCache>>,
    biome_noise: ImprovedNoise,
    seed: i64,
    rval: Cell<i64>,
}

impl LazyAreaContext {
    fn new(max_cache: usize, world_seed: i64, salt: i64) -> Self {
        let mut random = SimpleRandomSource::new(world_seed);
        Self {
            max_cache,
            cache: Rc::new(RefCell::new(LazyAreaCache::new())),
            biome_noise: ImprovedNoise::new(&mut random),
            seed: mix_seed(world_seed, salt),
            rval: Cell::new(0),
        }
    }

    fn create_result<F>(
        &self,
        transformer: F,
        first_area: Option<&AreaRef>,
        second_area: Option<&AreaRef>,
    ) -> AreaRef
    where
        F: Fn(i32, i32) -> i32 + 'static,
    {
        let max_cache = match (first_area, second_area) {
            (Some(first), Some(second)) => {
                MAX_CACHE.min(first.get_max_cache().max(second.get_max_cache()) * 4)
            }
            (Some(first), None) => MAX_CACHE.min(first.get_max_cache() * 4),
            _ => self.max_cache,
        };

        Rc::new(LazyArea {
            cache: self.cache.clone(),
            max_cache,
            transformer: Box::new(transformer),
        })
    }

    fn init_random(&self, x: i32, z: i32) {
        let mut value = self.seed;
        value = linear_congruential_generator_next(value, x as i64);
        value = linear_congruential_generator_next(value, z as i64);
        value = linear_congruential_generator_next(value, x as i64);
        value = linear_congruential_generator_next(value, z as i64);
        self.rval.set(value);
    }

    fn next_random(&self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be a positive integer");
        let value = ((self.rval.get() >> 24).rem_euclid(bound as i64)) as i32;
        self.rval.set(linear_congruential_generator_next(
            self.rval.get(),
            self.seed,
        ));
        value
    }

    fn get_biome_noise(&self) -> &ImprovedNoise {
        &self.biome_noise
    }

    fn random2(&self, first: i32, second: i32) -> i32 {
        if self.next_random(2) == 0 {
            first
        } else {
            second
        }
    }

    fn random4(&self, first: i32, second: i32, third: i32, fourth: i32) -> i32 {
        match self.next_random(4) {
            0 => first,
            1 => second,
            2 => third,
            _ => fourth,
        }
    }
}

fn run_area_transformer0<F>(context: ContextRef, apply_pixel: F) -> AreaFactory
where
    F: Fn(&LazyAreaContext, i32, i32) -> i32 + 'static,
{
    let apply_pixel = Rc::new(apply_pixel);
    Rc::new(move || {
        let apply_pixel = apply_pixel.clone();
        let transformer_context = context.clone();
        context.create_result(
            move |x, z| {
                transformer_context.init_random(x, z);
                apply_pixel(&transformer_context, x, z)
            },
            None,
            None,
        )
    })
}

fn run_area_transformer1<F>(
    context: ContextRef,
    area_factory: AreaFactory,
    apply_pixel: F,
) -> AreaFactory
where
    F: Fn(&LazyAreaContext, &LazyArea, i32, i32) -> i32 + 'static,
{
    let apply_pixel = Rc::new(apply_pixel);
    Rc::new(move || {
        let area = area_factory();
        let apply_pixel = apply_pixel.clone();
        let transformer_context = context.clone();
        let transformer_area = area.clone();
        context.create_result(
            move |x, z| {
                transformer_context.init_random(x, z);
                apply_pixel(&transformer_context, transformer_area.as_ref(), x, z)
            },
            Some(&area),
            None,
        )
    })
}

fn run_area_transformer2<F>(
    context: ContextRef,
    first_area_factory: AreaFactory,
    second_area_factory: AreaFactory,
    apply_pixel: F,
) -> AreaFactory
where
    F: Fn(&LazyAreaContext, &LazyArea, &LazyArea, i32, i32) -> i32 + 'static,
{
    let apply_pixel = Rc::new(apply_pixel);
    Rc::new(move || {
        let first_area = first_area_factory();
        let second_area = second_area_factory();
        let apply_pixel = apply_pixel.clone();
        let transformer_context = context.clone();
        let transformer_first_area = first_area.clone();
        let transformer_second_area = second_area.clone();
        context.create_result(
            move |x, z| {
                transformer_context.init_random(x, z);
                apply_pixel(
                    &transformer_context,
                    transformer_first_area.as_ref(),
                    transformer_second_area.as_ref(),
                    x,
                    z,
                )
            },
            Some(&first_area),
            Some(&second_area),
        )
    })
}

fn run_c0_transformer<F>(context: ContextRef, area_factory: AreaFactory, apply: F) -> AreaFactory
where
    F: Fn(&LazyAreaContext, i32) -> i32 + 'static,
{
    run_area_transformer1(context, area_factory, move |big_context, area, x, z| {
        apply(big_context, area.get(x, z))
    })
}

fn run_c1_transformer<F>(context: ContextRef, area_factory: AreaFactory, apply: F) -> AreaFactory
where
    F: Fn(&LazyAreaContext, i32) -> i32 + 'static,
{
    run_area_transformer1(context, area_factory, move |big_context, area, x, z| {
        apply(big_context, area.get(x, z))
    })
}

fn run_castle_transformer<F>(
    context: ContextRef,
    area_factory: AreaFactory,
    apply: F,
) -> AreaFactory
where
    F: Fn(&LazyAreaContext, i32, i32, i32, i32, i32) -> i32 + 'static,
{
    run_area_transformer1(context, area_factory, move |big_context, area, x, z| {
        apply(
            big_context,
            area.get(x, z - 1),
            area.get(x + 1, z),
            area.get(x, z + 1),
            area.get(x - 1, z),
            area.get(x, z),
        )
    })
}

fn run_bishop_transformer<F>(
    context: ContextRef,
    area_factory: AreaFactory,
    apply: F,
) -> AreaFactory
where
    F: Fn(&LazyAreaContext, i32, i32, i32, i32, i32) -> i32 + 'static,
{
    run_area_transformer1(context, area_factory, move |big_context, area, x, z| {
        apply(
            big_context,
            area.get(x - 1, z + 1),
            area.get(x + 1, z + 1),
            area.get(x + 1, z - 1),
            area.get(x - 1, z - 1),
            area.get(x, z),
        )
    })
}

fn island_layer(context: ContextRef) -> AreaFactory {
    run_area_transformer0(context, |random_context, x, z| {
        if x == 0 && z == 0 {
            1
        } else if random_context.next_random(10) == 0 {
            1
        } else {
            0
        }
    })
}

fn add_island_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_bishop_transformer(
        context,
        area_factory,
        |random_context, south_west, south_east, north_east, north_west, center| {
            if !is_shallow_ocean(center)
                || (is_shallow_ocean(north_west)
                    && is_shallow_ocean(north_east)
                    && is_shallow_ocean(south_west)
                    && is_shallow_ocean(south_east))
            {
                if !is_shallow_ocean(center)
                    && (is_shallow_ocean(north_west)
                        || is_shallow_ocean(south_west)
                        || is_shallow_ocean(north_east)
                        || is_shallow_ocean(south_east))
                    && random_context.next_random(5) == 0
                {
                    if is_shallow_ocean(north_west) {
                        return if center == ICE_ID { ICE_ID } else { north_west };
                    }
                    if is_shallow_ocean(south_west) {
                        return if center == ICE_ID { ICE_ID } else { south_west };
                    }
                    if is_shallow_ocean(north_east) {
                        return if center == ICE_ID { ICE_ID } else { north_east };
                    }
                    if is_shallow_ocean(south_east) {
                        return if center == ICE_ID { ICE_ID } else { south_east };
                    }
                }

                return center;
            }

            let mut candidate_count = 1;
            let mut candidate = 1;
            if !is_shallow_ocean(north_west) {
                if random_context.next_random(candidate_count) == 0 {
                    candidate = north_west;
                }
                candidate_count += 1;
            }
            if !is_shallow_ocean(north_east) {
                if random_context.next_random(candidate_count) == 0 {
                    candidate = north_east;
                }
                candidate_count += 1;
            }
            if !is_shallow_ocean(south_west) {
                if random_context.next_random(candidate_count) == 0 {
                    candidate = south_west;
                }
                candidate_count += 1;
            }
            if !is_shallow_ocean(south_east) && random_context.next_random(candidate_count) == 0 {
                candidate = south_east;
            }

            if random_context.next_random(3) == 0 {
                candidate
            } else if candidate == ICE_ID {
                ICE_ID
            } else {
                center
            }
        },
    )
}

fn remove_too_much_ocean_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |random_context, north, east, south, west, center| {
            if is_shallow_ocean(center)
                && is_shallow_ocean(north)
                && is_shallow_ocean(east)
                && is_shallow_ocean(south)
                && is_shallow_ocean(west)
                && random_context.next_random(2) == 0
            {
                1
            } else {
                center
            }
        },
    )
}

fn add_snow_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_c1_transformer(context, area_factory, |random_context, value| {
        if is_shallow_ocean(value) {
            return value;
        }

        let roll = random_context.next_random(6);
        if roll == 0 {
            ICE_ID
        } else if roll == 1 {
            COLD_ID
        } else {
            WARM_ID
        }
    })
}

fn add_edge_layer_cool_warm(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |_random_context, north, east, south, west, center| {
            if center != WARM_ID
                || ((north != COLD_ID && east != COLD_ID && south != COLD_ID && west != COLD_ID)
                    && (north != ICE_ID && east != ICE_ID && south != ICE_ID && west != ICE_ID))
            {
                center
            } else {
                MEDIUM_ID
            }
        },
    )
}

fn add_edge_layer_heat_ice(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |_random_context, north, east, south, west, center| {
            if center != ICE_ID
                || ((north != WARM_ID && east != WARM_ID && south != WARM_ID && west != WARM_ID)
                    && (north != MEDIUM_ID
                        && east != MEDIUM_ID
                        && south != MEDIUM_ID
                        && west != MEDIUM_ID))
            {
                center
            } else {
                COLD_ID
            }
        },
    )
}

fn add_edge_layer_introduce_special(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_c0_transformer(context, area_factory, |random_context, mut value| {
        if !is_shallow_ocean(value) && random_context.next_random(13) == 0 {
            value |= ((1 + random_context.next_random(15)) << SPECIAL_SHIFT) & SPECIAL_MASK;
        }

        value
    })
}

fn add_mushroom_island_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_bishop_transformer(
        context,
        area_factory,
        |random_context, south_west, south_east, north_east, north_west, center| {
            if is_shallow_ocean(center)
                && is_shallow_ocean(north_west)
                && is_shallow_ocean(south_west)
                && is_shallow_ocean(north_east)
                && is_shallow_ocean(south_east)
                && random_context.next_random(100) == 0
            {
                14
            } else {
                center
            }
        },
    )
}

fn add_deep_ocean_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |_random_context, north, east, south, west, center| {
            if !is_shallow_ocean(center) {
                return center;
            }

            let mut shallow_ocean_neighbors = 0;
            if is_shallow_ocean(north) {
                shallow_ocean_neighbors += 1;
            }
            if is_shallow_ocean(west) {
                shallow_ocean_neighbors += 1;
            }
            if is_shallow_ocean(east) {
                shallow_ocean_neighbors += 1;
            }
            if is_shallow_ocean(south) {
                shallow_ocean_neighbors += 1;
            }

            if shallow_ocean_neighbors <= 3 {
                return center;
            }

            match center {
                44 => 47,
                45 => 48,
                0 => 24,
                46 => 49,
                10 => 50,
                _ => 24,
            }
        },
    )
}

fn river_init_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_c0_transformer(context, area_factory, |random_context, value| {
        if is_shallow_ocean(value) {
            value
        } else {
            random_context.next_random(299_999) + 2
        }
    })
}

fn biome_init_layer(
    context: ContextRef,
    area_factory: AreaFactory,
    legacy_biome_init_layer: bool,
) -> AreaFactory {
    const LEGACY_WARM_BIOMES: [i32; 6] = [2, 4, 3, 6, 1, 5];
    const WARM_BIOMES: [i32; 6] = [2, 2, 2, 35, 35, 1];
    const MEDIUM_BIOMES: [i32; 6] = [4, 29, 3, 1, 27, 6];
    const COLD_BIOMES: [i32; 4] = [4, 3, 5, 1];
    const ICE_BIOMES: [i32; 4] = [12, 12, 12, 30];

    run_c0_transformer(context, area_factory, move |random_context, mut value| {
        let special_bits = (value & SPECIAL_MASK) >> SPECIAL_SHIFT;
        value &= !SPECIAL_MASK;
        if is_ocean(value) || value == 14 {
            return value;
        }

        match value {
            WARM_ID => {
                if special_bits > 0 {
                    if random_context.next_random(3) == 0 {
                        39
                    } else {
                        38
                    }
                } else {
                    let warm_biomes = if legacy_biome_init_layer {
                        &LEGACY_WARM_BIOMES
                    } else {
                        &WARM_BIOMES
                    };
                    warm_biomes[random_context.next_random(warm_biomes.len() as i32) as usize]
                }
            }
            MEDIUM_ID => {
                if special_bits > 0 {
                    21
                } else {
                    MEDIUM_BIOMES[random_context.next_random(MEDIUM_BIOMES.len() as i32) as usize]
                }
            }
            COLD_ID => {
                if special_bits > 0 {
                    32
                } else {
                    COLD_BIOMES[random_context.next_random(COLD_BIOMES.len() as i32) as usize]
                }
            }
            ICE_ID => ICE_BIOMES[random_context.next_random(ICE_BIOMES.len() as i32) as usize],
            _ => 14,
        }
    })
}

fn rare_biome_large_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_c1_transformer(context, area_factory, |random_context, value| {
        if random_context.next_random(10) == 0 && value == 21 {
            168
        } else {
            value
        }
    })
}

fn zoom_layer(context: ContextRef, area_factory: AreaFactory, fuzzy: bool) -> AreaFactory {
    run_area_transformer1(context, area_factory, move |big_context, area, x, z| {
        let parent_x = x >> 1;
        let parent_z = z >> 1;
        let center = area.get(parent_x, parent_z);
        big_context.init_random((x >> 1) << 1, (z >> 1) << 1);
        let odd_x = x & 1;
        let odd_z = z & 1;
        if odd_x == 0 && odd_z == 0 {
            return center;
        }

        let south = area.get(parent_x, (z + 1) >> 1);
        let center_south = big_context.random2(center, south);
        if odd_x == 0 && odd_z == 1 {
            return center_south;
        }

        let east = area.get((x + 1) >> 1, parent_z);
        let center_east = big_context.random2(center, east);
        if odd_x == 1 && odd_z == 0 {
            return center_east;
        }

        let south_east = area.get((x + 1) >> 1, (z + 1) >> 1);
        mode_or_random(big_context, fuzzy, center, east, south, south_east)
    })
}

fn mode_or_random(
    context: &LazyAreaContext,
    fuzzy: bool,
    first: i32,
    second: i32,
    third: i32,
    fourth: i32,
) -> i32 {
    if fuzzy {
        return context.random4(first, second, third, fourth);
    }

    if second == third && third == fourth {
        return second;
    }
    if first == second && first == third {
        return first;
    }
    if first == second && first == fourth {
        return first;
    }
    if first == third && first == fourth {
        return first;
    }
    if first == second && third != fourth {
        return first;
    }
    if first == third && second != fourth {
        return first;
    }
    if first == fourth && second != third {
        return first;
    }
    if second == third && first != fourth {
        return second;
    }
    if second == fourth && first != third {
        return second;
    }
    if third == fourth && first != second {
        return third;
    }

    context.random4(first, second, third, fourth)
}

fn biome_edge_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |_random_context, north, east, south, west, center| {
            let mut edge = 0;
            if check_edge(&mut edge, center)
                || check_edge_strict(&mut edge, north, east, south, west, center, 38, 37)
                || check_edge_strict(&mut edge, north, east, south, west, center, 39, 37)
                || check_edge_strict(&mut edge, north, east, south, west, center, 32, 5)
            {
                return edge;
            }

            if center == 2 && (north == 12 || east == 12 || south == 12 || west == 12) {
                return 34;
            }

            if center == 6 {
                if north == 2
                    || east == 2
                    || south == 2
                    || west == 2
                    || north == 30
                    || east == 30
                    || south == 30
                    || west == 30
                    || north == 12
                    || east == 12
                    || south == 12
                    || west == 12
                {
                    return 1;
                }

                if north == 21
                    || east == 21
                    || south == 21
                    || west == 21
                    || north == 168
                    || east == 168
                    || south == 168
                    || west == 168
                {
                    return 23;
                }
            }

            center
        },
    )
}

fn check_edge(result: &mut i32, edge: i32) -> bool {
    if !is_same(edge, 3) {
        return false;
    }

    *result = edge;
    true
}

fn check_edge_strict(
    result: &mut i32,
    north: i32,
    east: i32,
    south: i32,
    west: i32,
    center: i32,
    check: i32,
    edge: i32,
) -> bool {
    if center != check {
        return false;
    }

    *result = if is_same(north, check)
        && is_same(east, check)
        && is_same(south, check)
        && is_same(west, check)
    {
        center
    } else {
        edge
    };
    true
}

fn region_hills_layer(
    context: ContextRef,
    first_area_factory: AreaFactory,
    second_area_factory: AreaFactory,
) -> AreaFactory {
    run_area_transformer2(
        context,
        first_area_factory,
        second_area_factory,
        |random_context, first_area, second_area, x, z| {
            let center = first_area.get(x, z);
            let river_value = second_area.get(x, z);
            let mutation_roll = (river_value - 2) % 29;
            if !is_shallow_ocean(center) && river_value >= 2 && mutation_roll == 1 {
                return region_hills_mutation(center).unwrap_or(center);
            }

            if random_context.next_random(3) != 0 && mutation_roll != 0 {
                return center;
            }

            let mut mutated = center;
            if center == 2 {
                mutated = 17;
            } else if center == 4 {
                mutated = 18;
            } else if center == 27 {
                mutated = 28;
            } else if center == 29 {
                mutated = 1;
            } else if center == 5 {
                mutated = 19;
            } else if center == 32 {
                mutated = 33;
            } else if center == 30 {
                mutated = 31;
            } else if center == 1 {
                mutated = if random_context.next_random(3) == 0 {
                    18
                } else {
                    4
                };
            } else if center == 12 {
                mutated = 13;
            } else if center == 21 {
                mutated = 22;
            } else if center == 168 {
                mutated = 169;
            } else if center == 0 {
                mutated = 24;
            } else if center == 45 {
                mutated = 48;
            } else if center == 46 {
                mutated = 49;
            } else if center == 10 {
                mutated = 50;
            } else if center == 3 {
                mutated = 34;
            } else if center == 35 {
                mutated = 36;
            } else if is_same(center, 38) {
                mutated = 37;
            } else if (center == 24 || center == 48 || center == 49 || center == 50)
                && random_context.next_random(3) == 0
            {
                mutated = if random_context.next_random(2) == 0 {
                    1
                } else {
                    4
                };
            }

            if mutation_roll == 0 && mutated != center {
                mutated = region_hills_mutation(mutated).unwrap_or(center);
            }

            if mutated == center {
                return center;
            }

            let mut matching_neighbors = 0;
            if is_same(first_area.get(x, z - 1), center) {
                matching_neighbors += 1;
            }
            if is_same(first_area.get(x + 1, z), center) {
                matching_neighbors += 1;
            }
            if is_same(first_area.get(x - 1, z), center) {
                matching_neighbors += 1;
            }
            if is_same(first_area.get(x, z + 1), center) {
                matching_neighbors += 1;
            }

            if matching_neighbors >= 3 {
                mutated
            } else {
                center
            }
        },
    )
}

fn river_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |_random_context, north, east, south, west, center| {
            let filtered_center = river_filter(center);
            if filtered_center == river_filter(east)
                && filtered_center == river_filter(north)
                && filtered_center == river_filter(west)
                && filtered_center == river_filter(south)
            {
                -1
            } else {
                7
            }
        },
    )
}

fn river_filter(value: i32) -> i32 {
    if value >= 2 { 2 + (value & 1) } else { value }
}

fn smooth_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |random_context, north, east, south, west, center| {
            let horizontal_match = west == east;
            let vertical_match = north == south;
            if horizontal_match == vertical_match {
                if horizontal_match {
                    if random_context.next_random(2) == 0 {
                        east
                    } else {
                        north
                    }
                } else {
                    center
                }
            } else if horizontal_match {
                east
            } else {
                north
            }
        },
    )
}

fn rare_biome_spot_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_c1_transformer(context, area_factory, |random_context, value| {
        if random_context.next_random(57) == 0 && value == 1 {
            129
        } else {
            value
        }
    })
}

fn shore_layer(context: ContextRef, area_factory: AreaFactory) -> AreaFactory {
    run_castle_transformer(
        context,
        area_factory,
        |_random_context, north, east, south, west, center| {
            if center == 14 {
                return if is_shallow_ocean(north)
                    || is_shallow_ocean(east)
                    || is_shallow_ocean(south)
                    || is_shallow_ocean(west)
                {
                    15
                } else {
                    center
                };
            }

            if is_jungle_biome(center) {
                if !is_jungle_compatible(north)
                    || !is_jungle_compatible(east)
                    || !is_jungle_compatible(south)
                    || !is_jungle_compatible(west)
                {
                    return 23;
                }

                if is_ocean(north) || is_ocean(east) || is_ocean(south) || is_ocean(west) {
                    return 16;
                }

                return center;
            }

            if center == 3 || center == 34 || center == 20 {
                return if !is_ocean(center)
                    && (is_ocean(north) || is_ocean(east) || is_ocean(south) || is_ocean(west))
                {
                    25
                } else {
                    center
                };
            }

            if is_snowy_biome(center) {
                return if !is_ocean(center)
                    && (is_ocean(north) || is_ocean(east) || is_ocean(south) || is_ocean(west))
                {
                    26
                } else {
                    center
                };
            }

            if center == 37 || center == 38 {
                if !is_ocean(north)
                    && !is_ocean(east)
                    && !is_ocean(south)
                    && !is_ocean(west)
                    && (!is_mesa(north) || !is_mesa(east) || !is_mesa(south) || !is_mesa(west))
                {
                    return 2;
                }

                return center;
            }

            if !is_ocean(center)
                && center != 7
                && center != 6
                && (is_ocean(north) || is_ocean(east) || is_ocean(south) || is_ocean(west))
            {
                16
            } else {
                center
            }
        },
    )
}

fn ocean_layer(context: ContextRef) -> AreaFactory {
    run_area_transformer0(context, |random_context, x, z| {
        let value = random_context
            .get_biome_noise()
            .noise(x as f64 / 8.0, z as f64 / 8.0, 0.0);
        if value > 0.4 {
            44
        } else if value > 0.2 {
            45
        } else if value < -0.4 {
            10
        } else if value < -0.2 {
            46
        } else {
            0
        }
    })
}

fn river_mixer_layer(
    context: ContextRef,
    first_area_factory: AreaFactory,
    second_area_factory: AreaFactory,
) -> AreaFactory {
    run_area_transformer2(
        context,
        first_area_factory,
        second_area_factory,
        |_random_context, first_area, second_area, x, z| {
            let land = first_area.get(x, z);
            let river = second_area.get(x, z);
            if is_ocean(land) {
                return land;
            }

            if river != 7 {
                return land;
            }

            if land == 12 {
                return 11;
            }

            if land != 14 && land != 15 {
                river & 0xff
            } else {
                15
            }
        },
    )
}

fn ocean_mixer_layer(
    context: ContextRef,
    first_area_factory: AreaFactory,
    second_area_factory: AreaFactory,
) -> AreaFactory {
    run_area_transformer2(
        context,
        first_area_factory,
        second_area_factory,
        |_random_context, first_area, second_area, x, z| {
            let land = first_area.get(x, z);
            let ocean = second_area.get(x, z);
            if !is_ocean(land) {
                return land;
            }

            for offset_z in (-8..=8).step_by(4) {
                for offset_x in (-8..=8).step_by(4) {
                    if !is_ocean(first_area.get(x + offset_x, z + offset_z)) {
                        if ocean == 44 {
                            return 45;
                        }
                        if ocean == 10 {
                            return 46;
                        }
                    }
                }
            }

            if land == 24 {
                if ocean == 45 {
                    return 48;
                }
                if ocean == 0 {
                    return 24;
                }
                if ocean == 46 {
                    return 49;
                }
                if ocean == 10 {
                    return 50;
                }
            }

            ocean
        },
    )
}

fn zoom(
    seed: i64,
    fuzzy: bool,
    area_factory: AreaFactory,
    count: i32,
    context_factory: &impl Fn(i64) -> ContextRef,
) -> AreaFactory {
    let mut result = area_factory;
    for index in 0..count {
        result = zoom_layer(context_factory(seed + index as i64), result, fuzzy);
    }

    result
}

fn build_default_layer(
    legacy_biome_init_layer: bool,
    biome_zooms: i32,
    river_zooms: i32,
    context_factory: &impl Fn(i64) -> ContextRef,
) -> AreaFactory {
    let mut land = island_layer(context_factory(1));
    land = zoom_layer(context_factory(2000), land, true);
    land = add_island_layer(context_factory(1), land);
    land = zoom_layer(context_factory(2001), land, false);
    land = add_island_layer(context_factory(2), land);
    land = add_island_layer(context_factory(50), land);
    land = add_island_layer(context_factory(70), land);
    land = remove_too_much_ocean_layer(context_factory(2), land);

    let mut ocean = ocean_layer(context_factory(2));
    ocean = zoom(2001, false, ocean, 6, context_factory);

    land = add_snow_layer(context_factory(2), land);
    land = add_island_layer(context_factory(3), land);
    land = add_edge_layer_cool_warm(context_factory(2), land);
    land = add_edge_layer_heat_ice(context_factory(2), land);
    land = add_edge_layer_introduce_special(context_factory(3), land);
    land = zoom_layer(context_factory(2002), land, false);
    land = zoom_layer(context_factory(2003), land, false);
    land = add_island_layer(context_factory(4), land);
    land = add_mushroom_island_layer(context_factory(5), land);
    land = add_deep_ocean_layer(context_factory(4), land);
    land = zoom(1000, false, land, 0, context_factory);

    let mut rivers = zoom(1000, false, land.clone(), 0, context_factory);
    rivers = river_init_layer(context_factory(100), rivers);

    let mut biomes = biome_init_layer(context_factory(200), land, legacy_biome_init_layer);
    biomes = rare_biome_large_layer(context_factory(1001), biomes);
    biomes = zoom(1000, false, biomes, 2, context_factory);
    biomes = biome_edge_layer(context_factory(1000), biomes);

    let river_mix_source = zoom(1000, false, rivers.clone(), 2, context_factory);
    biomes = region_hills_layer(context_factory(1000), biomes, river_mix_source);

    rivers = zoom(1000, false, rivers, 2, context_factory);
    rivers = zoom(1000, false, rivers, river_zooms, context_factory);
    rivers = river_layer(context_factory(1), rivers);
    rivers = smooth_layer(context_factory(1000), rivers);

    biomes = rare_biome_spot_layer(context_factory(1001), biomes);

    for index in 0..biome_zooms {
        biomes = zoom_layer(context_factory(1000 + index as i64), biomes, false);
        if index == 0 {
            biomes = add_island_layer(context_factory(3), biomes);
        }

        if index == 1 || biome_zooms == 1 {
            biomes = shore_layer(context_factory(1000), biomes);
        }
    }

    biomes = smooth_layer(context_factory(1000), biomes);
    biomes = river_mixer_layer(context_factory(100), biomes, rivers);
    ocean_mixer_layer(context_factory(100), biomes, ocean)
}

fn build_overworld_biome_area(
    seed: i64,
    legacy_biome_init_layer: bool,
    biome_zooms: i32,
    river_zooms: i32,
) -> AreaRef {
    let context_factory = |salt| Rc::new(LazyAreaContext::new(25, seed, salt));
    let area_factory = build_default_layer(
        legacy_biome_init_layer,
        biome_zooms,
        river_zooms,
        &context_factory,
    );
    area_factory()
}

fn category(biome_id: i32) -> Option<i32> {
    match biome_id {
        16 | 26 => Some(9),
        2 | 17 | 130 => Some(12),
        131 | 162 | 20 | 3 | 34 => Some(2),
        27 | 28 | 29 | 157 | 132 | 4 | 155 | 156 | 18 => Some(10),
        140 | 13 | 12 => Some(8),
        168 | 169 | 21 | 23 | 22 | 149 | 151 => Some(3),
        37 | 165 | 167 | 166 => Some(4),
        39 | 38 => Some(5),
        14 | 15 => Some(15),
        25 => Some(0),
        46 | 49 | 50 | 48 | 24 | 47 | 10 | 45 | 0 | 44 => Some(11),
        1 | 129 => Some(6),
        11 | 7 => Some(13),
        35 | 36 | 163 | 164 => Some(7),
        6 | 134 => Some(14),
        160 | 161 | 32 | 33 | 30 | 31 | 158 | 5 | 19 | 133 => Some(1),
        _ => None,
    }
}

fn is_same(left: i32, right: i32) -> bool {
    left == right || category(left).is_some() && category(left) == category(right)
}

fn is_ocean(biome: i32) -> bool {
    matches!(biome, 44 | 45 | 0 | 46 | 10 | 47 | 48 | 24 | 49 | 50)
}

fn is_shallow_ocean(biome: i32) -> bool {
    matches!(biome, 44 | 45 | 0 | 46 | 10)
}

fn region_hills_mutation(biome: i32) -> Option<i32> {
    match biome {
        1 => Some(129),
        2 => Some(130),
        3 => Some(131),
        4 => Some(132),
        5 => Some(133),
        6 => Some(134),
        12 => Some(140),
        21 => Some(149),
        23 => Some(151),
        27 => Some(155),
        28 => Some(156),
        29 => Some(157),
        30 => Some(158),
        32 => Some(160),
        33 => Some(161),
        34 => Some(162),
        35 => Some(163),
        36 => Some(164),
        37 => Some(165),
        38 => Some(166),
        39 => Some(167),
        _ => None,
    }
}

fn is_snowy_biome(value: i32) -> bool {
    matches!(value, 26 | 11 | 12 | 13 | 140 | 30 | 31 | 158 | 10)
}

fn is_jungle_biome(value: i32) -> bool {
    matches!(value, 168 | 169 | 21 | 22 | 23 | 149 | 151)
}

fn is_jungle_compatible(value: i32) -> bool {
    is_jungle_biome(value) || value == 4 || value == 5 || is_ocean(value)
}

fn is_mesa(value: i32) -> bool {
    matches!(value, 37 | 38 | 39 | 165 | 166 | 167)
}

fn obfuscate_biome_zoom_seed(seed: i64) -> i64 {
    let digest = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("sha256 digest has at least eight bytes"),
    )
}

fn get_fuzzy_zoomed_biome_id(
    zoom_seed: i64,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    noise_biome_source: impl Fn(i32, i32, i32) -> i32,
) -> i32 {
    let shifted_x = block_x - 2;
    let shifted_y = block_y - 2;
    let shifted_z = block_z - 2;
    let base_quart_x = shifted_x >> 2;
    let base_quart_y = shifted_y >> 2;
    let base_quart_z = shifted_z >> 2;
    let offset_x = (shifted_x & 3) as f64 / 4.0;
    let offset_y = (shifted_y & 3) as f64 / 4.0;
    let offset_z = (shifted_z & 3) as f64 / 4.0;

    let mut best_corner = 0;
    let mut best_distance = f64::INFINITY;
    for corner in 0..8 {
        let use_base_x = (corner & 4) == 0;
        let use_base_y = (corner & 2) == 0;
        let use_base_z = (corner & 1) == 0;
        let quart_x = if use_base_x {
            base_quart_x
        } else {
            base_quart_x + 1
        };
        let quart_y = if use_base_y {
            base_quart_y
        } else {
            base_quart_y + 1
        };
        let quart_z = if use_base_z {
            base_quart_z
        } else {
            base_quart_z + 1
        };
        let scale_x = if use_base_x { offset_x } else { offset_x - 1.0 };
        let scale_y = if use_base_y { offset_y } else { offset_y - 1.0 };
        let scale_z = if use_base_z { offset_z } else { offset_z - 1.0 };
        let distance = get_fiddled_distance(
            zoom_seed, quart_x, quart_y, quart_z, scale_x, scale_y, scale_z,
        );
        if distance < best_distance {
            best_corner = corner;
            best_distance = distance;
        }
    }

    let quart_x = if (best_corner & 4) == 0 {
        base_quart_x
    } else {
        base_quart_x + 1
    };
    let quart_y = if (best_corner & 2) == 0 {
        base_quart_y
    } else {
        base_quart_y + 1
    };
    let quart_z = if (best_corner & 1) == 0 {
        base_quart_z
    } else {
        base_quart_z + 1
    };
    noise_biome_source(quart_x, quart_y, quart_z)
}

fn get_fiddled_distance(
    zoom_seed: i64,
    quart_x: i32,
    quart_y: i32,
    quart_z: i32,
    scale_x: f64,
    scale_y: f64,
    scale_z: f64,
) -> f64 {
    let mut seed = linear_congruential_generator_next(zoom_seed, quart_x as i64);
    seed = linear_congruential_generator_next(seed, quart_y as i64);
    seed = linear_congruential_generator_next(seed, quart_z as i64);
    seed = linear_congruential_generator_next(seed, quart_x as i64);
    seed = linear_congruential_generator_next(seed, quart_y as i64);
    seed = linear_congruential_generator_next(seed, quart_z as i64);
    let fiddle_x = get_fiddle(seed);
    seed = linear_congruential_generator_next(seed, zoom_seed);
    let fiddle_y = get_fiddle(seed);
    seed = linear_congruential_generator_next(seed, zoom_seed);
    let fiddle_z = get_fiddle(seed);
    square(scale_z + fiddle_z) + square(scale_y + fiddle_y) + square(scale_x + fiddle_x)
}

fn get_fiddle(seed: i64) -> f64 {
    let scaled = (seed >> 24).rem_euclid(1024) as f64;
    (scaled / 1024.0 - 0.5) * 0.9
}

fn square(value: f64) -> f64 {
    value * value
}

fn mix_seed(left: i64, right: i64) -> i64 {
    let mut mixed = linear_congruential_generator_next(right, right);
    mixed = linear_congruential_generator_next(mixed, right);
    mixed = linear_congruential_generator_next(mixed, right);
    let mut seed = linear_congruential_generator_next(left, mixed);
    seed = linear_congruential_generator_next(seed, mixed);
    linear_congruential_generator_next(seed, mixed)
}

fn linear_congruential_generator_next(left: i64, right: i64) -> i64 {
    let transformed = left
        .wrapping_mul(LCG_MULTIPLIER)
        .wrapping_add(LCG_INCREMENT);
    left.wrapping_mul(transformed).wrapping_add(right)
}

fn chunk_pos_as_long(x: i32, z: i32) -> i64 {
    ((x as u32 as u64) | ((z as u32 as u64) << 32)) as i64
}

fn quart_from_block(value: i32) -> i32 {
    value.div_euclid(4)
}

fn ceil_div(value: i32, divisor: i32) -> i32 {
    (value + divisor - 1).div_euclid(divisor)
}

fn clamp(value: i32, min_value: i32, max_value: i32) -> i32 {
    value.max(min_value).min(max_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OverworldBiomeSourceFixture {
        module: String,
        minecraft_version: String,
        biome_source_class: String,
        seed: String,
        legacy_biome_init_layer: bool,
        large_biomes: bool,
        wire_format: OverworldBiomeSourceWireFormatFixture,
        possible_biomes: Vec<BiomeFixture>,
        samples: OverworldBiomeSamplesFixture,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OverworldBiomeSourceWireFormatFixture {
        coordinates: String,
        biome_ids: String,
        biome_keys: String,
        biome_factors: String,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct BiomeFixture {
        id: i32,
        key: String,
        depth: f32,
        scale: f32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OverworldBiomeSamplesFixture {
        biome_method: String,
        grid_order: String,
        sample_y: i32,
        sample_count: usize,
        x: Vec<i32>,
        z: Vec<i32>,
        ids: Vec<i32>,
        keys: Vec<String>,
        depths: Vec<f32>,
        scales: Vec<f32>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct IntegrationFixture {
        module: String,
        minecraft_version: String,
        seed: String,
        wire_format: IntegrationWireFormatFixture,
        chunks: Vec<IntegrationChunkFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct IntegrationWireFormatFixture {
        biome_order: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct IntegrationChunkFixture {
        chunk_x: i32,
        chunk_z: i32,
        biomes: Vec<i32>,
    }

    fn fixtures() -> Vec<OverworldBiomeSourceFixture> {
        [
            include_str!("../../../../test/fixtures/biome/overworld-seed-0.json"),
            include_str!("../../../../test/fixtures/biome/overworld-seed-1.json"),
            include_str!("../../../../test/fixtures/biome/overworld-seed-12345.json"),
            include_str!("../../../../test/fixtures/biome/overworld-seed-2151901553968352745.json"),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid OverworldBiomeSource fixture"))
        .collect()
    }

    fn integration_fixture() -> IntegrationFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0.json"
        ))
        .expect("valid integration fixture")
    }

    #[test]
    fn fixture_metadata_stays_consistent() {
        for fixture in fixtures() {
            assert_eq!(fixture.module, "biome");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.biome_source_class,
                "net.minecraft.world.level.biome.OverworldBiomeSource"
            );
            assert!(!fixture.legacy_biome_init_layer);
            assert!(!fixture.large_biomes);
            assert_eq!(fixture.wire_format.coordinates, "integer");
            assert_eq!(fixture.wire_format.biome_ids, "integer");
            assert_eq!(fixture.wire_format.biome_keys, "string");
            assert_eq!(fixture.wire_format.biome_factors, "number");
            assert_eq!(fixture.samples.biome_method, "getNoiseBiome(x,0,z)");
            assert_eq!(fixture.samples.grid_order, "x-major,z-minor");
            assert_eq!(fixture.samples.sample_y, 0);
            assert_eq!(
                fixture.samples.sample_count,
                fixture.samples.x.len() * fixture.samples.z.len()
            );
            assert_eq!(fixture.samples.ids.len(), fixture.samples.sample_count);
            assert_eq!(fixture.samples.keys.len(), fixture.samples.sample_count);
            assert_eq!(fixture.samples.depths.len(), fixture.samples.sample_count);
            assert_eq!(fixture.samples.scales.len(), fixture.samples.sample_count);
        }
    }

    #[test]
    fn runtime_biome_registry_matches_oracle_possible_biome_list() {
        let fixture = fixtures().remove(0);
        let possible_biomes: Vec<_> = possible_overworld_biomes().collect();

        assert_eq!(possible_biomes.len(), fixture.possible_biomes.len());
        for (actual, expected) in possible_biomes.iter().zip(&fixture.possible_biomes) {
            assert_eq!(actual.id(), expected.id);
            assert_eq!(actual.key(), expected.key);
            assert_eq!(actual.depth().to_bits(), expected.depth.to_bits());
            assert_eq!(actual.scale().to_bits(), expected.scale.to_bits());
        }
    }

    #[test]
    fn matches_java_oracle_across_sampled_quart_grid() {
        for fixture in fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let source = OverworldBiomeSource::new(
                seed,
                fixture.legacy_biome_init_layer,
                fixture.large_biomes,
            );
            let mut index = 0;

            for x in &fixture.samples.x {
                for z in &fixture.samples.z {
                    let biome = source.get_noise_biome_definition(*x, fixture.samples.sample_y, *z);
                    assert_eq!(
                        biome.id(),
                        fixture.samples.ids[index],
                        "biome id mismatch for seed {} at quart ({x}, {z})",
                        fixture.seed
                    );
                    assert_eq!(biome.key(), fixture.samples.keys[index]);
                    assert_eq!(
                        biome.depth().to_bits(),
                        fixture.samples.depths[index].to_bits()
                    );
                    assert_eq!(
                        biome.scale().to_bits(),
                        fixture.samples.scales[index].to_bits()
                    );
                    index += 1;
                }
            }
        }
    }

    #[test]
    fn chunk_biome_container_writes_committed_integration_fixture() {
        let fixture = integration_fixture();
        assert_eq!(fixture.module, "integration");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.wire_format.biome_order, "y-major,z-major,x-minor");
        let chunk = &fixture.chunks[0];
        assert_eq!(chunk.chunk_x, 0);
        assert_eq!(chunk.chunk_z, 0);
        assert_eq!(chunk.biomes.len(), 1024);

        let source = OverworldBiomeSource::new(
            fixture.seed.parse::<i64>().expect("i64 fixture seed"),
            false,
            false,
        );
        let container = ChunkBiomeContainer::new(0, 256, chunk.chunk_x, chunk.chunk_z, &source);

        assert_eq!(container.write_biomes(), chunk.biomes);
    }
}
