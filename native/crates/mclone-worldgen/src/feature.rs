use std::{collections::BTreeMap, sync::OnceLock};

use crate::biome::{BiomeDefinition, OverworldBiomeSource};
use crate::block::{
    AIR, ANDESITE, BIRCH_LEAVES, BIRCH_LOG, COAL_ORE, COPPER_ORE, DANDELION, DEAD_BUSH, DEEPSLATE,
    DEEPSLATE_COAL_ORE, DEEPSLATE_COPPER_ORE, DEEPSLATE_DIAMOND_ORE, DEEPSLATE_GOLD_ORE,
    DEEPSLATE_IRON_ORE, DEEPSLATE_LAPIS_ORE, DEEPSLATE_REDSTONE_ORE, DIAMOND_ORE, DIORITE, DIRT,
    FERN, GLOW_LICHEN, GOLD_ORE, GRANITE, GRASS, GRASS_BLOCK, GRAVEL, IRON_ORE, LAPIS_ORE,
    LARGE_FERN_LOWER, LARGE_FERN_UPPER, LAVA, MYCELIUM, OAK_LEAVES, OAK_LOG, PODZOL, POPPY,
    RED_SAND, REDSTONE_ORE, RawBlockId, SAND, SNOW, SPRUCE_LEAVES, SPRUCE_LOG, STONE, TERRACOTTA,
    TUFF, WATER,
};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{
    BlockPos, ConfiguredDecorator, DecorationContext, HeightProvider, HeightmapType, IntProvider,
    VerticalAnchor,
};
use crate::prng::{RandomSource, WorldgenRandom};

const CHUNK_WIDTH: i32 = 16;
pub const FEATURES_CHUNK_DEPENDENCY_RADIUS: i32 = 8;
pub const FEATURES_WRITE_RADIUS_CUTOFF: i32 = 1;
const SIN_TABLE_SIZE: usize = 65_536;
const SIN_TABLE_MASK: i32 = 65_535;
const SIN_SCALE: f32 = 10_430.378_f32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationStep {
    RawGeneration,
    Lakes,
    LocalModifications,
    UndergroundStructures,
    SurfaceStructures,
    Strongholds,
    UndergroundOres,
    UndergroundDecoration,
    VegetalDecoration,
    TopLayerModification,
}

impl DecorationStep {
    pub const fn index(self) -> i32 {
        match self {
            Self::RawGeneration => 0,
            Self::Lakes => 1,
            Self::LocalModifications => 2,
            Self::UndergroundStructures => 3,
            Self::SurfaceStructures => 4,
            Self::Strongholds => 5,
            Self::UndergroundOres => 6,
            Self::UndergroundDecoration => 7,
            Self::VegetalDecoration => 8,
            Self::TopLayerModification => 9,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FeatureRegionMetrics {
    pub block_reads: usize,
    pub height_queries: usize,
    pub block_write_attempts: usize,
    pub block_writes: usize,
    pub blocked_block_writes: usize,
}

pub trait FeatureWorld {
    fn center_chunk_x(&self) -> i32;
    fn center_chunk_z(&self) -> i32;
    fn min_y(&self) -> i32;
    fn height(&self) -> i32;
    fn non_air_block_count(&self) -> usize;
    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId>;
    fn height_at(&mut self, heightmap: HeightmapType, world_x: i32, world_z: i32) -> Option<i32>;
    fn world_surface_height_at(&mut self, world_x: i32, world_z: i32) -> Option<i32>;
    fn set_block_world(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool;
}

impl FeatureWorld for MutableChunkBlockBuffer {
    fn center_chunk_x(&self) -> i32 {
        self.chunk_x
    }

    fn center_chunk_z(&self) -> i32 {
        self.chunk_z
    }

    fn min_y(&self) -> i32 {
        self.min_y
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn non_air_block_count(&self) -> usize {
        self.non_air_block_count()
    }

    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId> {
        let local_x = pos.x - self.chunk_x * CHUNK_WIDTH;
        let local_z = pos.z - self.chunk_z * CHUNK_WIDTH;
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return None;
        }
        Some(self.get_block_at_y(local_x, pos.y, local_z))
    }

    fn height_at(&mut self, heightmap: HeightmapType, world_x: i32, world_z: i32) -> Option<i32> {
        let local_x = world_x - self.chunk_x * CHUNK_WIDTH;
        let local_z = world_z - self.chunk_z * CHUNK_WIDTH;
        if !(0..CHUNK_WIDTH).contains(&local_x) || !(0..CHUNK_WIDTH).contains(&local_z) {
            return None;
        }
        Some(heightmap_height(self, heightmap, local_x, local_z))
    }

    fn world_surface_height_at(&mut self, world_x: i32, world_z: i32) -> Option<i32> {
        let local_x = world_x - self.chunk_x * CHUNK_WIDTH;
        let local_z = world_z - self.chunk_z * CHUNK_WIDTH;
        if !(0..CHUNK_WIDTH).contains(&local_x) || !(0..CHUNK_WIDTH).contains(&local_z) {
            return None;
        }
        Some(self.world_surface_height(local_x, local_z))
    }

    fn set_block_world(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool {
        let local_x = pos.x - self.chunk_x * CHUNK_WIDTH;
        let local_z = pos.z - self.chunk_z * CHUNK_WIDTH;
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&pos.y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return false;
        }
        self.set_block_at_y(local_x, pos.y, local_z, block_id);
        true
    }
}

#[derive(Debug)]
pub struct FeatureRegion {
    center_chunk_x: i32,
    center_chunk_z: i32,
    dependency_radius: i32,
    write_radius_cutoff: i32,
    chunks: BTreeMap<(i32, i32), MutableChunkBlockBuffer>,
    metrics: FeatureRegionMetrics,
}

impl FeatureRegion {
    pub fn new(
        center_chunk_x: i32,
        center_chunk_z: i32,
        chunks: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> Self {
        Self::with_radii(
            center_chunk_x,
            center_chunk_z,
            FEATURES_CHUNK_DEPENDENCY_RADIUS,
            FEATURES_WRITE_RADIUS_CUTOFF,
            chunks,
        )
    }

    pub fn with_radii(
        center_chunk_x: i32,
        center_chunk_z: i32,
        dependency_radius: i32,
        write_radius_cutoff: i32,
        chunks: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> Self {
        if dependency_radius < 0 {
            panic!("feature dependency radius must be non-negative");
        }
        if write_radius_cutoff < 0 {
            panic!("feature write radius cutoff must be non-negative");
        }
        if write_radius_cutoff > dependency_radius {
            panic!(
                "feature write radius cutoff {write_radius_cutoff} exceeds dependency radius {dependency_radius}"
            );
        }

        let chunks = chunks
            .into_iter()
            .map(|chunk| ((chunk.chunk_x, chunk.chunk_z), chunk))
            .collect();
        Self {
            center_chunk_x,
            center_chunk_z,
            dependency_radius,
            write_radius_cutoff,
            chunks,
            metrics: FeatureRegionMetrics::default(),
        }
    }

    pub fn set_center(&mut self, center_chunk_x: i32, center_chunk_z: i32) {
        self.center_chunk_x = center_chunk_x;
        self.center_chunk_z = center_chunk_z;
    }

    pub fn dependency_radius(&self) -> i32 {
        self.dependency_radius
    }

    pub fn write_radius_cutoff(&self) -> i32 {
        self.write_radius_cutoff
    }

    pub fn metrics(&self) -> FeatureRegionMetrics {
        self.metrics
    }

    pub fn chunk(&self, chunk_x: i32, chunk_z: i32) -> Option<&MutableChunkBlockBuffer> {
        self.chunks.get(&(chunk_x, chunk_z))
    }

    pub fn chunk_mut(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Option<&mut MutableChunkBlockBuffer> {
        self.chunks.get_mut(&(chunk_x, chunk_z))
    }

    pub fn into_chunk(mut self, chunk_x: i32, chunk_z: i32) -> Option<MutableChunkBlockBuffer> {
        self.chunks.remove(&(chunk_x, chunk_z))
    }

    pub fn remove_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Option<MutableChunkBlockBuffer> {
        self.chunks.remove(&(chunk_x, chunk_z))
    }

    fn get_chunk(&self, chunk_x: i32, chunk_z: i32) -> &MutableChunkBlockBuffer {
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        self.chunks.get(&(chunk_x, chunk_z)).unwrap_or_else(|| {
            panic!(
                "feature region missing dependency chunk ({chunk_x}, {chunk_z}) for center ({}, {})",
                self.center_chunk_x, self.center_chunk_z
            )
        })
    }

    fn get_chunk_mut(&mut self, chunk_x: i32, chunk_z: i32) -> &mut MutableChunkBlockBuffer {
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        self.chunks.get_mut(&(chunk_x, chunk_z)).unwrap_or_else(|| {
            panic!(
                "feature region missing dependency chunk ({chunk_x}, {chunk_z}) for center ({}, {})",
                self.center_chunk_x, self.center_chunk_z
            )
        })
    }

    fn ensure_within_dependency_window(&self, chunk_x: i32, chunk_z: i32) {
        if (chunk_x - self.center_chunk_x).abs() > self.dependency_radius
            || (chunk_z - self.center_chunk_z).abs() > self.dependency_radius
        {
            panic!(
                "feature region access outside dependency window: center ({}, {}), requested ({chunk_x}, {chunk_z}), radius {}",
                self.center_chunk_x, self.center_chunk_z, self.dependency_radius
            );
        }
    }

    fn can_write_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        (chunk_x - self.center_chunk_x).abs() <= self.write_radius_cutoff
            && (chunk_z - self.center_chunk_z).abs() <= self.write_radius_cutoff
    }
}

impl FeatureWorld for FeatureRegion {
    fn center_chunk_x(&self) -> i32 {
        self.center_chunk_x
    }

    fn center_chunk_z(&self) -> i32 {
        self.center_chunk_z
    }

    fn min_y(&self) -> i32 {
        self.get_chunk(self.center_chunk_x, self.center_chunk_z)
            .min_y
    }

    fn height(&self) -> i32 {
        self.get_chunk(self.center_chunk_x, self.center_chunk_z)
            .height
    }

    fn non_air_block_count(&self) -> usize {
        self.chunks
            .values()
            .map(MutableChunkBlockBuffer::non_air_block_count)
            .sum()
    }

    fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId> {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.metrics.block_reads += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return None;
        }
        Some(chunk.get_block_at_y(
            pos.x - chunk_x * CHUNK_WIDTH,
            pos.y,
            pos.z - chunk_z * CHUNK_WIDTH,
        ))
    }

    fn height_at(&mut self, heightmap: HeightmapType, world_x: i32, world_z: i32) -> Option<i32> {
        let chunk_x = block_to_chunk_coord(world_x);
        let chunk_z = block_to_chunk_coord(world_z);
        self.metrics.height_queries += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        Some(heightmap_height(
            chunk,
            heightmap,
            world_x - chunk_x * CHUNK_WIDTH,
            world_z - chunk_z * CHUNK_WIDTH,
        ))
    }

    fn world_surface_height_at(&mut self, world_x: i32, world_z: i32) -> Option<i32> {
        let chunk_x = block_to_chunk_coord(world_x);
        let chunk_z = block_to_chunk_coord(world_z);
        self.metrics.height_queries += 1;
        let chunk = self.get_chunk(chunk_x, chunk_z);
        Some(chunk.world_surface_height(
            world_x - chunk_x * CHUNK_WIDTH,
            world_z - chunk_z * CHUNK_WIDTH,
        ))
    }

    fn set_block_world(&mut self, pos: BlockPos, block_id: RawBlockId) -> bool {
        let chunk_x = block_to_chunk_coord(pos.x);
        let chunk_z = block_to_chunk_coord(pos.z);
        self.ensure_within_dependency_window(chunk_x, chunk_z);
        self.metrics.block_write_attempts += 1;
        if !self.can_write_chunk(chunk_x, chunk_z) {
            self.metrics.blocked_block_writes += 1;
            return false;
        }

        let chunk = self.get_chunk_mut(chunk_x, chunk_z);
        if !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y) {
            return false;
        }
        chunk.set_block_at_y(
            pos.x - chunk_x * CHUNK_WIDTH,
            pos.y,
            pos.z - chunk_z * CHUNK_WIDTH,
            block_id,
        );
        self.metrics.block_writes += 1;
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimpleBlockConfiguration {
    pub to_place: RawBlockId,
    pub place_on: &'static [RawBlockId],
    pub place_in: &'static [RawBlockId],
    pub place_under: &'static [RawBlockId],
}

impl SimpleBlockConfiguration {
    pub const fn new(to_place: RawBlockId) -> Self {
        Self {
            to_place,
            place_on: &[],
            place_in: &[],
            place_under: &[],
        }
    }

    pub const fn place_on(mut self, place_on: &'static [RawBlockId]) -> Self {
        self.place_on = place_on;
        self
    }

    pub const fn place_in(mut self, place_in: &'static [RawBlockId]) -> Self {
        self.place_in = place_in;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RandomPatchConfiguration {
    pub state: RawBlockId,
    pub tries: i32,
    pub xspread: i32,
    pub yspread: i32,
    pub zspread: i32,
    pub project: bool,
    pub can_replace: bool,
    pub double_plant: bool,
    pub place_on: &'static [RawBlockId],
}

impl RandomPatchConfiguration {
    pub const fn new(state: RawBlockId) -> Self {
        Self {
            state,
            tries: 64,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            place_on: &[GRASS_BLOCK],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    const GLOW_LICHEN_VALID: [Self; 5] =
        [Self::Up, Self::North, Self::East, Self::South, Self::West];

    const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
        }
    }

    const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowLichenConfiguration {
    pub search_range: i32,
    pub chance_of_spreading: f32,
    pub can_be_placed_on: &'static [RawBlockId],
}

impl Eq for GlowLichenConfiguration {}

impl GlowLichenConfiguration {
    pub const fn default_overworld() -> Self {
        Self {
            search_range: 20,
            chance_of_spreading: 0.5,
            can_be_placed_on: &[STONE, ANDESITE, DIORITE, GRANITE, TUFF, DEEPSLATE],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasicTreeConfiguration {
    pub log: RawBlockId,
    pub leaves: RawBlockId,
    pub min_height: i32,
    pub random_height: i32,
}

impl BasicTreeConfiguration {
    pub const fn new(
        log: RawBlockId,
        leaves: RawBlockId,
        min_height: i32,
        random_height: i32,
    ) -> Self {
        Self {
            log,
            leaves,
            min_height,
            random_height,
        }
    }

    pub const fn oak() -> Self {
        Self::new(OAK_LOG, OAK_LEAVES, 4, 3)
    }

    pub const fn birch() -> Self {
        Self::new(BIRCH_LOG, BIRCH_LEAVES, 5, 3)
    }

    pub const fn spruce() -> Self {
        Self::new(SPRUCE_LOG, SPRUCE_LEAVES, 6, 4)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StraightTrunkPlacerConfiguration {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
}

impl StraightTrunkPlacerConfiguration {
    pub const fn new(base_height: i32, height_rand_a: i32, height_rand_b: i32) -> Self {
        Self {
            base_height,
            height_rand_a,
            height_rand_b,
        }
    }

    fn tree_height(self, random: &mut impl RandomSource) -> i32 {
        self.base_height
            + random.next_int_bound(self.height_rand_a + 1)
            + random.next_int_bound(self.height_rand_b + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoliagePlacerConfiguration {
    Spruce {
        radius: IntProvider,
        offset: IntProvider,
        trunk_height: IntProvider,
    },
    Pine {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
    },
}

impl FoliagePlacerConfiguration {
    fn foliage_height(
        self,
        random: &mut impl RandomSource,
        tree_height: i32,
        _config: TreeConfiguration,
    ) -> i32 {
        match self {
            Self::Spruce { trunk_height, .. } => (tree_height - trunk_height.sample(random)).max(4),
            Self::Pine { height, .. } => height.sample(random),
        }
    }

    fn foliage_radius(self, random: &mut impl RandomSource, trunk_height: i32) -> i32 {
        match self {
            Self::Spruce { radius, .. } => radius.sample(random),
            Self::Pine { radius, .. } => {
                radius.sample(random) + random.next_int_bound((trunk_height + 1).max(1))
            }
        }
    }

    fn offset(self, random: &mut impl RandomSource) -> i32 {
        match self {
            Self::Spruce { offset, .. } | Self::Pine { offset, .. } => offset.sample(random),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TwoLayersFeatureSize {
    pub limit: i32,
    pub lower_size: i32,
    pub upper_size: i32,
}

impl TwoLayersFeatureSize {
    pub const fn new(limit: i32, lower_size: i32, upper_size: i32) -> Self {
        Self {
            limit,
            lower_size,
            upper_size,
        }
    }

    pub const fn size_at_height(self, height: i32) -> i32 {
        if height < self.limit {
            self.lower_size
        } else {
            self.upper_size
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeConfiguration {
    pub log: RawBlockId,
    pub leaves: RawBlockId,
    pub trunk_placer: StraightTrunkPlacerConfiguration,
    pub foliage_placer: FoliagePlacerConfiguration,
    pub minimum_size: TwoLayersFeatureSize,
}

impl TreeConfiguration {
    pub const fn new(
        log: RawBlockId,
        leaves: RawBlockId,
        trunk_placer: StraightTrunkPlacerConfiguration,
        foliage_placer: FoliagePlacerConfiguration,
        minimum_size: TwoLayersFeatureSize,
    ) -> Self {
        Self {
            log,
            leaves,
            trunk_placer,
            foliage_placer,
            minimum_size,
        }
    }

    pub const fn spruce() -> Self {
        Self::new(
            SPRUCE_LOG,
            SPRUCE_LEAVES,
            StraightTrunkPlacerConfiguration::new(5, 2, 1),
            FoliagePlacerConfiguration::Spruce {
                radius: IntProvider::uniform(2, 3),
                offset: IntProvider::uniform(0, 2),
                trunk_height: IntProvider::uniform(1, 2),
            },
            TwoLayersFeatureSize::new(2, 0, 2),
        )
    }

    pub const fn pine() -> Self {
        Self::new(
            SPRUCE_LOG,
            SPRUCE_LEAVES,
            StraightTrunkPlacerConfiguration::new(6, 4, 0),
            FoliagePlacerConfiguration::Pine {
                radius: IntProvider::constant(1),
                offset: IntProvider::constant(1),
                height: IntProvider::uniform(3, 4),
            },
            TwoLayersFeatureSize::new(2, 0, 2),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedConfiguredFeature {
    pub feature: Box<ConfiguredFeature>,
    pub chance: f32,
}

impl Eq for WeightedConfiguredFeature {}

impl WeightedConfiguredFeature {
    pub fn new(feature: ConfiguredFeature, chance: f32) -> Self {
        Self {
            feature: Box::new(feature),
            chance,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RandomFeatureConfiguration {
    pub features: Vec<WeightedConfiguredFeature>,
    pub default_feature: Box<ConfiguredFeature>,
}

impl Eq for RandomFeatureConfiguration {}

impl RandomFeatureConfiguration {
    pub fn new(
        features: impl Into<Vec<WeightedConfiguredFeature>>,
        default_feature: ConfiguredFeature,
    ) -> Self {
        Self {
            features: features.into(),
            default_feature: Box::new(default_feature),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecoratedFeatureConfiguration {
    pub feature: Box<ConfiguredFeature>,
    pub decorators: Vec<ConfiguredDecorator>,
}

impl DecoratedFeatureConfiguration {
    pub fn new(
        feature: ConfiguredFeature,
        decorators: impl Into<Vec<ConfiguredDecorator>>,
    ) -> Self {
        Self {
            feature: Box::new(feature),
            decorators: decorators.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OreTarget {
    NaturalStone,
    StoneOreReplaceables,
    DeepslateOreReplaceables,
}

impl OreTarget {
    fn matches(self, block_id: RawBlockId) -> bool {
        match self {
            Self::NaturalStone => {
                matches!(
                    block_id,
                    STONE | GRANITE | DIORITE | ANDESITE | TUFF | DEEPSLATE
                )
            }
            Self::StoneOreReplaceables => matches!(block_id, STONE | GRANITE | DIORITE | ANDESITE),
            Self::DeepslateOreReplaceables => matches!(block_id, TUFF | DEEPSLATE),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OreTargetBlockState {
    pub target: OreTarget,
    pub state: RawBlockId,
}

impl OreTargetBlockState {
    pub const fn new(target: OreTarget, state: RawBlockId) -> Self {
        Self { target, state }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OreConfiguration {
    pub target_states: Vec<OreTargetBlockState>,
    pub size: i32,
    pub discard_chance_on_air_exposure: f32,
}

impl Eq for OreConfiguration {}

impl OreConfiguration {
    pub fn new(
        target_states: impl Into<Vec<OreTargetBlockState>>,
        size: i32,
        discard_chance_on_air_exposure: f32,
    ) -> Self {
        Self {
            target_states: target_states.into(),
            size,
            discard_chance_on_air_exposure,
        }
    }

    pub fn natural_stone(state: RawBlockId, size: i32) -> Self {
        Self::new(
            [OreTargetBlockState::new(OreTarget::NaturalStone, state)],
            size,
            0.0,
        )
    }

    pub fn stone_and_deepslate_ore(
        stone_ore: RawBlockId,
        deepslate_ore: RawBlockId,
        size: i32,
    ) -> Self {
        Self::new(
            [
                OreTargetBlockState::new(OreTarget::StoneOreReplaceables, stone_ore),
                OreTargetBlockState::new(OreTarget::DeepslateOreReplaceables, deepslate_ore),
            ],
            size,
            0.0,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfiguredFeature {
    SimpleBlock(SimpleBlockConfiguration),
    RandomPatch(RandomPatchConfiguration),
    GlowLichen(GlowLichenConfiguration),
    BasicTree(BasicTreeConfiguration),
    Tree(TreeConfiguration),
    RandomSelector(RandomFeatureConfiguration),
    Decorated(DecoratedFeatureConfiguration),
    Ore(OreConfiguration),
}

impl ConfiguredFeature {
    pub const fn simple_block(config: SimpleBlockConfiguration) -> Self {
        Self::SimpleBlock(config)
    }

    pub const fn random_patch(config: RandomPatchConfiguration) -> Self {
        Self::RandomPatch(config)
    }

    pub const fn glow_lichen(config: GlowLichenConfiguration) -> Self {
        Self::GlowLichen(config)
    }

    pub const fn basic_tree(config: BasicTreeConfiguration) -> Self {
        Self::BasicTree(config)
    }

    pub const fn tree(config: TreeConfiguration) -> Self {
        Self::Tree(config)
    }

    pub fn random_selector(config: RandomFeatureConfiguration) -> Self {
        Self::RandomSelector(config)
    }

    pub fn decorated(config: DecoratedFeatureConfiguration) -> Self {
        Self::Decorated(config)
    }

    pub const fn ore(config: OreConfiguration) -> Self {
        Self::Ore(config)
    }

    pub fn place<W: FeatureWorld>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: BlockPos,
    ) -> bool {
        match self {
            Self::SimpleBlock(config) => place_simple_block(world, random, origin, *config),
            Self::RandomPatch(config) => place_random_patch(world, random, origin, *config),
            Self::GlowLichen(config) => place_glow_lichen(world, random, origin, *config),
            Self::BasicTree(config) => place_basic_tree(world, random, origin, *config),
            Self::Tree(config) => place_tree(world, random, origin, *config),
            Self::RandomSelector(config) => place_random_selector(world, random, origin, config),
            Self::Decorated(config) => {
                place_configured_decorated_feature(world, random, origin, config)
            }
            Self::Ore(config) => place_ore(world, random, origin, config),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedFeature {
    pub step: DecorationStep,
    pub feature: ConfiguredFeature,
    pub decorators: Vec<ConfiguredDecorator>,
}

impl PlacedFeature {
    pub fn new(
        step: DecorationStep,
        feature: ConfiguredFeature,
        decorators: impl Into<Vec<ConfiguredDecorator>>,
    ) -> Self {
        Self {
            step,
            feature,
            decorators: decorators.into(),
        }
    }

    pub fn place<W: FeatureWorld>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: BlockPos,
    ) -> bool {
        let decoration_context = DecorationContext::new(world.min_y(), world.height());
        self.place_decorated(world, random, &decoration_context, 0, origin)
    }

    fn place_decorated<W: FeatureWorld>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        decoration_context: &DecorationContext,
        decorator_index: usize,
        pos: BlockPos,
    ) -> bool {
        let Some(decorator) = self.decorators.get(decorator_index) else {
            return self.feature.place(world, random, pos);
        };

        let mut placed_any = false;
        for next_pos in decorator_positions(world, decoration_context, random, *decorator, pos) {
            placed_any |= self.place_decorated(
                world,
                random,
                decoration_context,
                decorator_index + 1,
                next_pos,
            );
        }
        placed_any
    }
}

fn decorator_positions<W: FeatureWorld>(
    world: &mut W,
    context: &DecorationContext,
    random: &mut impl RandomSource,
    decorator: ConfiguredDecorator,
    pos: BlockPos,
) -> Vec<BlockPos> {
    match decorator {
        ConfiguredDecorator::Heightmap(config) => {
            let Some(y) = world.height_at(config.heightmap, pos.x, pos.z) else {
                return Vec::new();
            };
            if y > context.get_min_build_height() {
                vec![BlockPos::new(pos.x, y, pos.z)]
            } else {
                Vec::new()
            }
        }
        ConfiguredDecorator::HeightmapSpreadDouble(config) => {
            let Some(y) = world.height_at(config.heightmap, pos.x, pos.z) else {
                return Vec::new();
            };
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
        ConfiguredDecorator::WaterDepthThreshold(config) => {
            let Some(ocean_floor) = world.height_at(HeightmapType::OceanFloor, pos.x, pos.z) else {
                return Vec::new();
            };
            let Some(world_surface) = world.height_at(HeightmapType::WorldSurface, pos.x, pos.z)
            else {
                return Vec::new();
            };
            if world_surface - ocean_floor > config.max_water_depth {
                Vec::new()
            } else {
                vec![pos]
            }
        }
        _ => decorator.get_positions(context, random, pos),
    }
}

fn place_configured_decorated_feature<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &DecoratedFeatureConfiguration,
) -> bool {
    let decoration_context = DecorationContext::new(world.min_y(), world.height());
    place_configured_decorated_feature_at(
        world,
        random,
        &decoration_context,
        config.feature.as_ref(),
        &config.decorators,
        0,
        origin,
    )
}

fn place_configured_decorated_feature_at<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    decoration_context: &DecorationContext,
    feature: &ConfiguredFeature,
    decorators: &[ConfiguredDecorator],
    decorator_index: usize,
    pos: BlockPos,
) -> bool {
    let Some(decorator) = decorators.get(decorator_index) else {
        return feature.place(world, random, pos);
    };

    let mut placed_any = false;
    for next_pos in decorator_positions(world, decoration_context, random, *decorator, pos) {
        placed_any |= place_configured_decorated_feature_at(
            world,
            random,
            decoration_context,
            feature,
            decorators,
            decorator_index + 1,
            next_pos,
        );
    }
    placed_any
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecorationReport {
    pub biome_key: &'static str,
    pub attempted_features: usize,
    pub placed_features: usize,
    pub added_non_air_blocks: usize,
}

pub fn apply_overworld_biome_decoration(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    chunk: &mut MutableChunkBlockBuffer,
) -> DecorationReport {
    let biome = chunk_center_biome(seed, biome_source, chunk.chunk_x, chunk.chunk_z);
    apply_overworld_biome_features(seed, biome, chunk)
}

pub fn apply_overworld_biome_decoration_to_region(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    region: &mut FeatureRegion,
) -> DecorationReport {
    let biome = chunk_center_biome(
        seed,
        biome_source,
        region.center_chunk_x(),
        region.center_chunk_z(),
    );
    apply_overworld_biome_features(seed, biome, region)
}

pub fn apply_overworld_biome_features<W: FeatureWorld>(
    seed: i64,
    biome: BiomeDefinition,
    world: &mut W,
) -> DecorationReport {
    let before = world.non_air_block_count();
    let min_block_x = world.center_chunk_x() * CHUNK_WIDTH;
    let min_block_z = world.center_chunk_z() * CHUNK_WIDTH;
    let origin = BlockPos::new(min_block_x, world.min_y(), min_block_z);
    let features = overworld_features_for_biome(biome);
    let mut random = WorldgenRandom::default();
    let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);
    let mut placed_features = 0;

    for step_index in 0..=DecorationStep::TopLayerModification.index() {
        let mut feature_index = 0;
        for feature in features
            .iter()
            .filter(|feature| feature.step.index() == step_index)
        {
            random.set_feature_seed(decoration_seed, feature_index, step_index);
            if feature.place(world, &mut random, origin) {
                placed_features += 1;
            }
            feature_index += 1;
        }
    }

    DecorationReport {
        biome_key: biome.key(),
        attempted_features: features.len(),
        placed_features,
        added_non_air_blocks: world.non_air_block_count().saturating_sub(before),
    }
}

pub fn overworld_features_for_biome(biome: BiomeDefinition) -> Vec<PlacedFeature> {
    let mut biome_features = match biome.key() {
        "minecraft:plains" | "minecraft:sunflower_plains" => plains_features(),
        "minecraft:forest" | "minecraft:wooded_hills" | "minecraft:flower_forest" => {
            forest_features()
        }
        "minecraft:birch_forest"
        | "minecraft:birch_forest_hills"
        | "minecraft:tall_birch_forest"
        | "minecraft:tall_birch_hills" => birch_forest_features(),
        "minecraft:taiga"
        | "minecraft:taiga_hills"
        | "minecraft:taiga_mountains"
        | "minecraft:giant_tree_taiga"
        | "minecraft:giant_tree_taiga_hills"
        | "minecraft:giant_spruce_taiga"
        | "minecraft:giant_spruce_taiga_hills" => taiga_features(),
        "minecraft:snowy_taiga"
        | "minecraft:snowy_taiga_hills"
        | "minecraft:snowy_taiga_mountains"
        | "minecraft:snowy_tundra"
        | "minecraft:snowy_mountains" => snowy_features(),
        "minecraft:mountains"
        | "minecraft:wooded_mountains"
        | "minecraft:mountain_edge"
        | "minecraft:gravelly_mountains"
        | "minecraft:modified_gravelly_mountains" => mountain_features(),
        "minecraft:desert" | "minecraft:desert_hills" | "minecraft:desert_lakes" => {
            desert_features()
        }
        "minecraft:badlands"
        | "minecraft:badlands_plateau"
        | "minecraft:wooded_badlands_plateau"
        | "minecraft:modified_badlands_plateau"
        | "minecraft:modified_wooded_badlands_plateau"
        | "minecraft:eroded_badlands" => badlands_features(),
        "minecraft:swamp" | "minecraft:swamp_hills" => swamp_features(),
        "minecraft:mushroom_fields" | "minecraft:mushroom_field_shore" => mushroom_field_features(),
        _ => default_land_features(),
    };
    let mut features = default_underground_variety_features();
    features.extend(default_ore_features());
    features.append(&mut biome_features);
    features
}

fn chunk_center_biome(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    chunk_x: i32,
    chunk_z: i32,
) -> BiomeDefinition {
    biome_source.get_block_position_biome_definition(
        seed,
        chunk_x * CHUNK_WIDTH + CHUNK_WIDTH / 2,
        chunk_z * CHUNK_WIDTH + CHUNK_WIDTH / 2,
    )
}

fn plains_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 0, 0.35, 1),
        grass_patch(GRASS, 4),
        flower_patch(DANDELION, 1),
        flower_patch(POPPY, 1),
    ]
}

fn default_underground_variety_features() -> Vec<PlacedFeature> {
    vec![
        underground_ore_feature(
            DIRT,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::top(),
            10,
        ),
        underground_ore_feature(
            GRAVEL,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::top(),
            8,
        ),
        underground_ore_feature(
            GRANITE,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(79),
            10,
        ),
        underground_ore_feature(
            DIORITE,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(79),
            10,
        ),
        underground_ore_feature(
            ANDESITE,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(79),
            10,
        ),
        underground_ore_feature(
            TUFF,
            33,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(16),
            1,
        ),
        underground_ore_feature(
            DEEPSLATE,
            64,
            VerticalAnchor::absolute(0),
            VerticalAnchor::absolute(16),
            2,
        ),
    ]
}

fn default_ore_features() -> Vec<PlacedFeature> {
    vec![
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(COAL_ORE, DEEPSLATE_COAL_ORE, 17),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(127)),
            20,
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(IRON_ORE, DEEPSLATE_IRON_ORE, 9),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(63)),
            20,
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(GOLD_ORE, DEEPSLATE_GOLD_ORE, 9),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(31)),
            2,
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(REDSTONE_ORE, DEEPSLATE_REDSTONE_ORE, 8),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
            8,
        ),
        single_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(DIAMOND_ORE, DEEPSLATE_DIAMOND_ORE, 8),
            HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
        ),
        single_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(LAPIS_ORE, DEEPSLATE_LAPIS_ORE, 7),
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(30), 0),
        ),
        counted_ore_feature(
            OreConfiguration::stone_and_deepslate_ore(COPPER_ORE, DEEPSLATE_COPPER_ORE, 10),
            HeightProvider::trapezoid(VerticalAnchor::absolute(0), VerticalAnchor::absolute(96), 0),
            6,
        ),
    ]
}

fn underground_ore_feature(
    block_id: RawBlockId,
    size: i32,
    min_inclusive: VerticalAnchor,
    max_inclusive: VerticalAnchor,
    count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::ore(OreConfiguration::natural_stone(block_id, size)),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(HeightProvider::uniform(min_inclusive, max_inclusive)),
        ],
    )
}

fn counted_ore_feature(
    config: OreConfiguration,
    height: HeightProvider,
    count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::ore(config),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(height),
        ],
    )
}

fn single_ore_feature(config: OreConfiguration, height: HeightProvider) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::UndergroundOres,
        ConfiguredFeature::ore(config),
        vec![
            ConfiguredDecorator::square(),
            ConfiguredDecorator::range(height),
        ],
    )
}

fn forest_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 5, 0.35, 1),
        tree_feature(BasicTreeConfiguration::birch(), 2, 0.25, 1),
        grass_patch(GRASS, 3),
        grass_patch(FERN, 1),
        flower_patch(DANDELION, 1),
        flower_patch(POPPY, 1),
    ]
}

fn birch_forest_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::birch(), 7, 0.3, 2),
        grass_patch(GRASS, 3),
        flower_patch(DANDELION, 1),
        flower_patch(POPPY, 1),
    ]
}

fn taiga_features() -> Vec<PlacedFeature> {
    vec![
        large_fern_patch_feature(),
        glow_lichen_feature(),
        taiga_vegetation_feature(),
        grass_patch(FERN, 4),
        grass_patch(GRASS, 2),
    ]
}

fn snowy_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::spruce(), 3, 0.2, 1),
        grass_patch(FERN, 1),
    ]
}

fn mountain_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::spruce(), 1, 0.25, 1),
        tree_feature(BasicTreeConfiguration::oak(), 0, 0.2, 1),
        grass_patch(GRASS, 1),
    ]
}

fn desert_features() -> Vec<PlacedFeature> {
    vec![dead_bush_patch(2)]
}

fn badlands_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 1, 0.1, 1),
        dead_bush_patch(2),
    ]
}

fn swamp_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 2, 0.25, 1),
        grass_patch(GRASS, 2),
        flower_patch(POPPY, 1),
        dead_bush_patch(1),
    ]
}

fn mushroom_field_features() -> Vec<PlacedFeature> {
    vec![grass_patch(GRASS, 1)]
}

fn default_land_features() -> Vec<PlacedFeature> {
    vec![
        tree_feature(BasicTreeConfiguration::oak(), 1, 0.1, 1),
        grass_patch(GRASS, 2),
        flower_patch(DANDELION, 1),
    ]
}

fn tree_feature(
    config: BasicTreeConfiguration,
    count: i32,
    extra_chance: f32,
    extra_count: i32,
) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::basic_tree(config),
        vec![
            ConfiguredDecorator::count_extra(count, extra_chance, extra_count),
            ConfiguredDecorator::square(),
        ],
    )
}

fn taiga_vegetation_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_selector(RandomFeatureConfiguration::new(
            [WeightedConfiguredFeature::new(
                ConfiguredFeature::decorated(DecoratedFeatureConfiguration::new(
                    ConfiguredFeature::tree(TreeConfiguration::pine()),
                    [ConfiguredDecorator::count_extra(6, 0.1, 1)],
                )),
                0.33333334,
            )],
            ConfiguredFeature::tree(TreeConfiguration::spruce()),
        )),
        vec![
            ConfiguredDecorator::count_extra(10, 0.1, 1),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::water_depth_threshold(0),
            ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
        ],
    )
}

fn large_fern_patch_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            tries: 64,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: false,
            can_replace: false,
            double_plant: true,
            place_on: &[GRASS_BLOCK, DIRT, PODZOL, MYCELIUM],
        }),
        vec![
            ConfiguredDecorator::count(7),
            ConfiguredDecorator::square(),
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ConfiguredDecorator::spread_32_above(),
        ],
    )
}

fn glow_lichen_feature() -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld()),
        vec![
            ConfiguredDecorator::Count(crate::placement::CountConfiguration::from_provider(
                IntProvider::uniform(20, 30),
            )),
            ConfiguredDecorator::range(HeightProvider::uniform(
                VerticalAnchor::bottom(),
                VerticalAnchor::absolute(54),
            )),
            ConfiguredDecorator::square(),
        ],
    )
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) const CURRENT_TAIGA_VEGETATION_FEATURE_INDEX: i32 = 2;
    pub(crate) const JAVA_TAIGA_VEGETATION_FEATURE_INDEX: i32 = 2;

    pub(crate) fn place_taiga_vegetation_with_feature_index<W: FeatureWorld>(
        seed: i64,
        world: &mut W,
        feature_index: i32,
    ) -> bool {
        let min_block_x = world.center_chunk_x() * CHUNK_WIDTH;
        let min_block_z = world.center_chunk_z() * CHUNK_WIDTH;
        let origin = BlockPos::new(min_block_x, world.min_y(), min_block_z);
        let mut random = WorldgenRandom::default();
        let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);
        random.set_feature_seed(
            decoration_seed,
            feature_index,
            DecorationStep::VegetalDecoration.index(),
        );
        taiga_vegetation_feature().place(world, &mut random, origin)
    }
}

fn grass_patch(block_id: RawBlockId, count: i32) -> PlacedFeature {
    random_patch_feature(
        RandomPatchConfiguration {
            state: block_id,
            tries: 48,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            place_on: &[GRASS_BLOCK, DIRT, PODZOL, MYCELIUM],
        },
        count,
    )
}

fn flower_patch(block_id: RawBlockId, count: i32) -> PlacedFeature {
    random_patch_feature(RandomPatchConfiguration::new(block_id), count)
}

fn dead_bush_patch(count: i32) -> PlacedFeature {
    random_patch_feature(
        RandomPatchConfiguration {
            state: DEAD_BUSH,
            tries: 16,
            xspread: 7,
            yspread: 3,
            zspread: 7,
            project: true,
            can_replace: false,
            double_plant: false,
            place_on: &[SAND, RED_SAND, TERRACOTTA, DIRT, GRASS_BLOCK, PODZOL],
        },
        count,
    )
}

fn random_patch_feature(config: RandomPatchConfiguration, count: i32) -> PlacedFeature {
    PlacedFeature::new(
        DecorationStep::VegetalDecoration,
        ConfiguredFeature::random_patch(config),
        vec![
            ConfiguredDecorator::count(count),
            ConfiguredDecorator::square(),
        ],
    )
}

fn place_simple_block<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    origin: BlockPos,
    config: SimpleBlockConfiguration,
) -> bool {
    let below = BlockPos::new(origin.x, origin.y - 1, origin.z);
    let above = BlockPos::new(origin.x, origin.y + 1, origin.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    let Some(block_above) = world.block_at_world(above) else {
        return false;
    };

    if !matches_allowed(config.place_on, block_below)
        || !matches_allowed(config.place_in, current)
        || !matches_allowed(config.place_under, block_above)
        || !can_survive_simple_plant(config.to_place, current, block_below)
    {
        return false;
    }

    world.set_block_world(origin, config.to_place)
}

fn place_random_patch<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: RandomPatchConfiguration,
) -> bool {
    let projected = if config.project {
        project_to_surface(world, origin).unwrap_or(origin)
    } else {
        origin
    };
    let mut placed = 0;

    for _ in 0..config.tries {
        let pos = BlockPos::new(
            projected.x + random.next_int_bound(config.xspread + 1)
                - random.next_int_bound(config.xspread + 1),
            projected.y + random.next_int_bound(config.yspread + 1)
                - random.next_int_bound(config.yspread + 1),
            projected.z + random.next_int_bound(config.zspread + 1)
                - random.next_int_bound(config.zspread + 1),
        );
        let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
        let Some(current) = world.block_at_world(pos) else {
            continue;
        };
        let Some(block_below) = world.block_at_world(below) else {
            continue;
        };
        let can_replace = current == AIR || (config.can_replace && is_replaceable_plant(current));
        if can_replace
            && matches_allowed(config.place_on, block_below)
            && can_survive_simple_plant(config.state, current, block_below)
        {
            let did_place = if config.double_plant {
                let lower = world.set_block_world(pos, LARGE_FERN_LOWER);
                let upper =
                    world.set_block_world(BlockPos::new(pos.x, pos.y + 1, pos.z), LARGE_FERN_UPPER);
                lower && upper
            } else {
                world.set_block_world(pos, config.state)
            };
            if did_place {
                placed += 1;
            }
        }
    }

    placed > 0
}

fn place_glow_lichen<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: GlowLichenConfiguration,
) -> bool {
    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    if !is_air_or_water(current) {
        return false;
    }

    let directions = shuffled_directions(&Direction::GLOW_LICHEN_VALID, random);
    if place_glow_lichen_if_possible(world, random, origin, current, config, &directions) {
        return true;
    }

    for direction in &directions {
        let candidate = offset_pos(origin, *direction);
        let side_directions =
            shuffled_directions_except(&Direction::GLOW_LICHEN_VALID, random, direction.opposite());

        for _ in 0..config.search_range {
            let Some(candidate_state) = world.block_at_world(candidate) else {
                break;
            };
            if !is_air_or_water(candidate_state) && candidate_state != GLOW_LICHEN {
                break;
            }
            if place_glow_lichen_if_possible(
                world,
                random,
                candidate,
                candidate_state,
                config,
                &side_directions,
            ) {
                return true;
            }
        }
    }

    false
}

fn place_glow_lichen_if_possible<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    pos: BlockPos,
    current: RawBlockId,
    config: GlowLichenConfiguration,
    directions: &[Direction],
) -> bool {
    if !is_air_or_water(current) && current != GLOW_LICHEN {
        return false;
    }

    for direction in directions {
        let neighbor = offset_pos(pos, *direction);
        let Some(neighbor_state) = world.block_at_world(neighbor) else {
            continue;
        };
        if !config.can_be_placed_on.contains(&neighbor_state) {
            continue;
        }

        world.set_block_world(pos, GLOW_LICHEN);
        if random.next_float() < config.chance_of_spreading {
            consume_glow_lichen_spread_random(random);
        }
        return true;
    }

    false
}

fn shuffled_directions(directions: &[Direction], random: &mut impl RandomSource) -> Vec<Direction> {
    let mut shuffled = directions.to_vec();
    shuffle_java_style(&mut shuffled, random);
    shuffled
}

fn shuffled_directions_except(
    directions: &[Direction],
    random: &mut impl RandomSource,
    excluded: Direction,
) -> Vec<Direction> {
    let mut filtered = directions
        .iter()
        .copied()
        .filter(|direction| *direction != excluded)
        .collect::<Vec<_>>();
    shuffle_java_style(&mut filtered, random);
    filtered
}

fn shuffle_java_style<T>(items: &mut [T], random: &mut impl RandomSource) {
    for i in (2..=items.len()).rev() {
        let swap_with = random.next_int_bound(i as i32) as usize;
        items.swap(i - 1, swap_with);
    }
}

fn consume_glow_lichen_spread_random(random: &mut impl RandomSource) {
    // Native: compact glow_lichen state consumes Java spread RNG without storing extra faces.
    let mut directions = Direction::ALL;
    shuffle_java_style(&mut directions, random);
}

fn offset_pos(pos: BlockPos, direction: Direction) -> BlockPos {
    let (dx, dy, dz) = direction.offset();
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}

fn is_air_or_water(block_id: RawBlockId) -> bool {
    matches!(block_id, AIR | WATER)
}

fn place_basic_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: BasicTreeConfiguration,
) -> bool {
    let Some(base) = project_to_surface(world, origin) else {
        return false;
    };
    let below = BlockPos::new(base.x, base.y - 1, base.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    if !matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM) {
        return false;
    }

    let height = config.min_height + random.next_int_bound(config.random_height.max(1));
    let leaves_center_y = base.y + height;
    if base.y < world.min_y() + 1 || leaves_center_y + 1 >= world.min_y() + world.height() {
        return false;
    }

    let mut targets = Vec::new();
    for y in base.y..base.y + height {
        targets.push((BlockPos::new(base.x, y, base.z), config.log));
    }

    for dy in -2_i32..=1 {
        let radius: i32 = if dy == 1 { 1 } else { 2 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let corner = dx.abs() == radius && dz.abs() == radius;
                if corner && (dy == 1 || random.next_boolean()) {
                    continue;
                }
                targets.push((
                    BlockPos::new(base.x + dx, leaves_center_y + dy, base.z + dz),
                    config.leaves,
                ));
            }
        }
    }

    if targets
        .iter()
        .any(|(pos, _)| !can_replace_tree_block(world, *pos))
    {
        return false;
    }

    world.set_block_world(below, DIRT);
    for (pos, block_id) in targets {
        world.set_block_world(pos, block_id);
    }
    true
}

fn place_random_selector<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &RandomFeatureConfiguration,
) -> bool {
    for weighted in &config.features {
        if random.next_float() < weighted.chance {
            return weighted.feature.place(world, random, origin);
        }
    }

    config.default_feature.place(world, random, origin)
}

fn place_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: TreeConfiguration,
) -> bool {
    let base = origin;

    let tree_height = config.trunk_placer.tree_height(random);
    let foliage_height = config
        .foliage_placer
        .foliage_height(random, tree_height, config);
    let trunk_height = tree_height - foliage_height;
    let foliage_radius = config.foliage_placer.foliage_radius(random, trunk_height);

    if base.y < world.min_y() + 1 || base.y + tree_height + 1 > world.min_y() + world.height() {
        return false;
    }
    if !can_survive_tree_sapling(world, base) {
        return false;
    }
    if get_max_free_tree_height(world, tree_height, base, config) < tree_height {
        return false;
    }

    place_straight_trunk(world, random, base, tree_height, config);
    let foliage_attachment = BlockPos::new(base.x, base.y + tree_height, base.z);
    create_foliage(
        world,
        random,
        config,
        foliage_attachment,
        foliage_height,
        foliage_radius,
    );
    true
}

fn get_max_free_tree_height<W: FeatureWorld>(
    world: &mut W,
    tree_height: i32,
    base: BlockPos,
    config: TreeConfiguration,
) -> i32 {
    for y_offset in 0..=tree_height + 1 {
        let radius = config.minimum_size.size_at_height(y_offset);
        for x_offset in -radius..=radius {
            for z_offset in -radius..=radius {
                let pos = BlockPos::new(base.x + x_offset, base.y + y_offset, base.z + z_offset);
                if !is_free_tree_pos(world, pos) {
                    return y_offset - 2;
                }
            }
        }
    }

    tree_height
}

fn place_straight_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
) {
    set_dirt_at(world, random, BlockPos::new(base.x, base.y - 1, base.z));

    for y_offset in 0..height {
        place_log(
            world,
            random,
            BlockPos::new(base.x, base.y + y_offset, base.z),
            config,
        );
    }
}

fn create_foliage<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    attachment: BlockPos,
    foliage_height: i32,
    foliage_radius: i32,
) {
    let offset = config.foliage_placer.offset(random);
    match config.foliage_placer {
        FoliagePlacerConfiguration::Spruce { .. } => {
            let mut radius = random.next_int_bound(2);
            let mut radius_limit = 1;
            let mut reset_radius = 0;

            for y_offset in (-(foliage_height)..=offset).rev() {
                place_leaves_row(world, random, config, attachment, radius, y_offset);
                if radius >= radius_limit {
                    radius = reset_radius;
                    reset_radius = 1;
                    radius_limit = (radius_limit + 1).min(foliage_radius);
                } else {
                    radius += 1;
                }
            }
        }
        FoliagePlacerConfiguration::Pine { .. } => {
            let mut radius = 0;

            for y_offset in ((offset - foliage_height)..=offset).rev() {
                place_leaves_row(world, random, config, attachment, radius, y_offset);
                if radius >= 1 && y_offset == offset - foliage_height + 1 {
                    radius -= 1;
                } else if radius < foliage_radius {
                    radius += 1;
                }
            }
        }
    }
}

fn place_leaves_row<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    center: BlockPos,
    radius: i32,
    y_offset: i32,
) {
    for x_offset in -radius..=radius {
        for z_offset in -radius..=radius {
            if should_skip_conifer_leaf(x_offset.abs(), z_offset.abs(), radius) {
                continue;
            }
            try_place_leaf(
                world,
                random,
                BlockPos::new(
                    center.x + x_offset,
                    center.y + y_offset,
                    center.z + z_offset,
                ),
                config,
            );
        }
    }
}

fn should_skip_conifer_leaf(abs_x: i32, abs_z: i32, radius: i32) -> bool {
    abs_x == radius && abs_z == radius && radius > 0
}

fn place_log<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
) -> bool {
    if valid_tree_pos(world, pos) {
        world.set_block_world(pos, config.log)
    } else {
        false
    }
}

fn try_place_leaf<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
) -> bool {
    if valid_tree_pos(world, pos) {
        world.set_block_world(pos, config.leaves)
    } else {
        false
    }
}

fn set_dirt_at<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
) -> bool {
    let Some(current) = world.block_at_world(pos) else {
        return false;
    };
    if matches!(current, DIRT | PODZOL) {
        true
    } else {
        world.set_block_world(pos, DIRT)
    }
}

fn place_ore<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &OreConfiguration,
) -> bool {
    if config.size <= 0 {
        return false;
    }

    let angle = random.next_float() * std::f32::consts::PI;
    let radius = config.size as f32 / 8.0;
    let padding = ceil_f32((config.size as f32 / 16.0 * 2.0 + 1.0) / 2.0);
    let start_x = origin.x as f64 + (angle as f64).sin() * radius as f64;
    let end_x = origin.x as f64 - (angle as f64).sin() * radius as f64;
    let start_z = origin.z as f64 + (angle as f64).cos() * radius as f64;
    let end_z = origin.z as f64 - (angle as f64).cos() * radius as f64;
    let start_y = origin.y as f64 + random.next_int_bound(3) as f64 - 2.0;
    let end_y = origin.y as f64 + random.next_int_bound(3) as f64 - 2.0;
    let min_x = origin.x - ceil_f32(radius) - padding;
    let min_y = origin.y - 2 - padding;
    let min_z = origin.z - ceil_f32(radius) - padding;
    let width_xz = 2 * (ceil_f32(radius) + padding);
    let height_y = 2 * (2 + padding);

    for x in min_x..=min_x + width_xz {
        for z in min_z..=min_z + width_xz {
            if min_y
                <= world
                    .world_surface_height_at(x, z)
                    .unwrap_or(world.min_y() - 1)
            {
                return do_place_ore(
                    world, random, config, start_x, end_x, start_z, end_z, start_y, end_y, min_x,
                    min_y, min_z, width_xz, height_y,
                );
            }
        }
    }

    false
}

#[allow(clippy::too_many_arguments)]
fn do_place_ore<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: &OreConfiguration,
    start_x: f64,
    end_x: f64,
    start_z: f64,
    end_z: f64,
    start_y: f64,
    end_y: f64,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    width_xz: i32,
    height_y: i32,
) -> bool {
    if width_xz <= 0 || height_y <= 0 {
        return false;
    }

    let mut placed = 0;
    let mut visited = vec![false; (width_xz * height_y * width_xz) as usize];
    let size = config.size as usize;
    let mut spheres = vec![0.0; size * 4];

    for index in 0..size {
        let progress = index as f32 / config.size as f32;
        let center_x = lerp(progress as f64, start_x, end_x);
        let center_y = lerp(progress as f64, start_y, end_y);
        let center_z = lerp(progress as f64, start_z, end_z);
        let scale = random.next_double() * config.size as f64 / 16.0;
        let radius = ((mth_sin(std::f32::consts::PI * progress) as f64 + 1.0) * scale + 1.0) / 2.0;
        spheres[index * 4] = center_x;
        spheres[index * 4 + 1] = center_y;
        spheres[index * 4 + 2] = center_z;
        spheres[index * 4 + 3] = radius;
    }

    for left in 0..size.saturating_sub(1) {
        if spheres[left * 4 + 3] <= 0.0 {
            continue;
        }
        for right in left + 1..size {
            if spheres[right * 4 + 3] <= 0.0 {
                continue;
            }

            let dx = spheres[left * 4] - spheres[right * 4];
            let dy = spheres[left * 4 + 1] - spheres[right * 4 + 1];
            let dz = spheres[left * 4 + 2] - spheres[right * 4 + 2];
            let dr = spheres[left * 4 + 3] - spheres[right * 4 + 3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[right * 4 + 3] = -1.0;
                } else {
                    spheres[left * 4 + 3] = -1.0;
                }
            }
        }
    }

    for index in 0..size {
        let radius = spheres[index * 4 + 3];
        if radius < 0.0 {
            continue;
        }

        let center_x = spheres[index * 4];
        let center_y = spheres[index * 4 + 1];
        let center_z = spheres[index * 4 + 2];
        let block_min_x = floor_f64(center_x - radius).max(min_x);
        let block_min_y = floor_f64(center_y - radius).max(min_y);
        let block_min_z = floor_f64(center_z - radius).max(min_z);
        let block_max_x = floor_f64(center_x + radius).max(block_min_x);
        let block_max_y = floor_f64(center_y + radius).max(block_min_y);
        let block_max_z = floor_f64(center_z + radius).max(block_min_z);

        for x in block_min_x..=block_max_x {
            let normalized_x = (x as f64 + 0.5 - center_x) / radius;
            if normalized_x * normalized_x >= 1.0 {
                continue;
            }
            for y in block_min_y..=block_max_y {
                let normalized_y = (y as f64 + 0.5 - center_y) / radius;
                if normalized_x * normalized_x + normalized_y * normalized_y >= 1.0 {
                    continue;
                }
                for z in block_min_z..=block_max_z {
                    let normalized_z = (z as f64 + 0.5 - center_z) / radius;
                    if normalized_x * normalized_x
                        + normalized_y * normalized_y
                        + normalized_z * normalized_z
                        >= 1.0
                        || !(world.min_y()..world.min_y() + world.height()).contains(&y)
                    {
                        continue;
                    }

                    let visited_index =
                        x - min_x + (y - min_y) * width_xz + (z - min_z) * width_xz * height_y;
                    if visited_index < 0 || visited_index as usize >= visited.len() {
                        continue;
                    }
                    let visited_index = visited_index as usize;
                    if visited[visited_index] {
                        continue;
                    }
                    visited[visited_index] = true;

                    let pos = BlockPos::new(x, y, z);
                    let Some(current) = world.block_at_world(pos) else {
                        continue;
                    };
                    for target in &config.target_states {
                        if can_place_ore(current, world, random, config, *target, pos)
                            && world.set_block_world(pos, target.state)
                        {
                            placed += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    placed > 0
}

fn can_place_ore<W: FeatureWorld>(
    current: RawBlockId,
    world: &mut W,
    random: &mut impl RandomSource,
    config: &OreConfiguration,
    target: OreTargetBlockState,
    pos: BlockPos,
) -> bool {
    if !target.target.matches(current) {
        return false;
    }
    if should_skip_air_check(random, config.discard_chance_on_air_exposure) {
        true
    } else {
        !is_adjacent_to_air(world, pos)
    }
}

fn should_skip_air_check(random: &mut impl RandomSource, chance: f32) -> bool {
    if chance <= 0.0 {
        true
    } else if chance >= 1.0 {
        false
    } else {
        random.next_float() >= chance
    }
}

fn is_adjacent_to_air<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    const NEIGHBORS: [(i32, i32, i32); 6] = [
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ];

    NEIGHBORS.iter().any(|(dx, dy, dz)| {
        world.block_at_world(BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)) == Some(AIR)
    })
}

fn project_to_surface<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> Option<BlockPos> {
    Some(BlockPos::new(
        pos.x,
        world.world_surface_height_at(pos.x, pos.z)?,
        pos.z,
    ))
}

fn heightmap_height(
    chunk: &MutableChunkBlockBuffer,
    heightmap: HeightmapType,
    local_x: i32,
    local_z: i32,
) -> i32 {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        if heightmap_is_opaque(heightmap, chunk.get_block_at_y(local_x, y, local_z)) {
            return y + 1;
        }
    }
    chunk.min_y
}

fn heightmap_is_opaque(heightmap: HeightmapType, block_id: RawBlockId) -> bool {
    match heightmap {
        HeightmapType::WorldSurfaceWg | HeightmapType::WorldSurface => block_id != AIR,
        HeightmapType::OceanFloorWg | HeightmapType::OceanFloor => material_blocks_motion(block_id),
        HeightmapType::MotionBlocking => material_blocks_motion(block_id) || has_fluid(block_id),
        HeightmapType::MotionBlockingNoLeaves => {
            (material_blocks_motion(block_id) || has_fluid(block_id)) && !is_leaves(block_id)
        }
    }
}

fn material_blocks_motion(block_id: RawBlockId) -> bool {
    !matches!(
        block_id,
        AIR | WATER
            | LAVA
            | SNOW
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
    )
}

fn has_fluid(block_id: RawBlockId) -> bool {
    matches!(block_id, WATER | LAVA)
}

fn is_leaves(block_id: RawBlockId) -> bool {
    matches!(block_id, OAK_LEAVES | BIRCH_LEAVES | SPRUCE_LEAVES)
}

fn matches_allowed(allowed: &[RawBlockId], block_id: RawBlockId) -> bool {
    allowed.is_empty() || allowed.contains(&block_id)
}

fn can_survive_simple_plant(
    block_id: RawBlockId,
    current: RawBlockId,
    block_below: RawBlockId,
) -> bool {
    current == AIR
        && match block_id {
            GRASS | FERN | DANDELION | POPPY => {
                matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
            }
            LARGE_FERN_LOWER | LARGE_FERN_UPPER => {
                matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
            }
            DEAD_BUSH => matches!(
                block_below,
                SAND | RED_SAND | TERRACOTTA | DIRT | GRASS_BLOCK | PODZOL
            ),
            _ => false,
        }
}

fn is_replaceable_plant(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
    )
}

fn can_survive_tree_sapling<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
}

fn valid_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | WATER
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | BIRCH_LEAVES
            | SPRUCE_LEAVES
    )
}

fn is_free_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    if valid_tree_pos(world, pos) {
        return true;
    }
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(block_id, OAK_LOG | BIRCH_LOG | SPRUCE_LOG)
}

fn can_replace_tree_block<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | WATER
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | OAK_LOG
            | BIRCH_LEAVES
            | BIRCH_LOG
            | SPRUCE_LEAVES
            | SPRUCE_LOG
    )
}

fn block_to_chunk_coord(block: i32) -> i32 {
    block.div_euclid(CHUNK_WIDTH)
}

fn ceil_f32(value: f32) -> i32 {
    let truncated = value as i32;
    if value > truncated as f32 {
        truncated + 1
    } else {
        truncated
    }
}

fn floor_f64(value: f64) -> i32 {
    let truncated = value as i32;
    if value < truncated as f64 {
        truncated - 1
    } else {
        truncated
    }
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

fn mth_sin(value: f32) -> f32 {
    let index = ((value * SIN_SCALE).trunc() as i32 & SIN_TABLE_MASK) as usize;
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
    use crate::biome::get_layered_biome_by_id;
    use crate::block::STONE;
    use crate::prng::WorldgenRandom;

    fn flat_grass_chunk() -> MutableChunkBlockBuffer {
        flat_grass_chunk_at(0, 0)
    }

    fn flat_grass_chunk_at(chunk_x: i32, chunk_z: i32) -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, 32);
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                chunk.set_block_at_y(x, 0, z, STONE);
                chunk.set_block_at_y(x, 1, z, DIRT);
                chunk.set_block_at_y(x, 2, z, GRASS_BLOCK);
            }
        }
        chunk
    }

    fn solid_stone_chunk() -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 64);
        for x in 0..CHUNK_WIDTH {
            for y in 0..64 {
                for z in 0..CHUNK_WIDTH {
                    chunk.set_block_at_y(x, y, z, STONE);
                }
            }
        }
        chunk
    }

    fn count_blocks(chunk: &MutableChunkBlockBuffer, block_id: RawBlockId) -> usize {
        chunk
            .blocks
            .iter()
            .filter(|current| **current == block_id)
            .count()
    }

    #[test]
    fn simple_block_feature_places_on_grass_surface() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(0);
        let feature = ConfiguredFeature::simple_block(
            SimpleBlockConfiguration::new(DANDELION)
                .place_on(&[GRASS_BLOCK])
                .place_in(&[AIR]),
        );

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(4, 3, 5)));
        assert_eq!(chunk.get_block_at_y(4, 3, 5), DANDELION);
    }

    #[test]
    fn random_patch_projects_to_surface_and_places_multiple_blocks() {
        let mut chunk = flat_grass_chunk();
        let before = chunk.non_air_block_count();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: GRASS,
            tries: 16,
            xspread: 3,
            yspread: 1,
            zspread: 3,
            project: true,
            can_replace: false,
            double_plant: false,
            place_on: &[GRASS_BLOCK],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(chunk.non_air_block_count() > before);
        assert!(chunk.blocks.iter().any(|block_id| *block_id == GRASS));
    }

    #[test]
    fn random_patch_double_plant_writes_large_fern_halves() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(3);
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: true,
            place_on: &[],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), LARGE_FERN_LOWER);
        assert_eq!(chunk.get_block_at_y(8, 4, 8), LARGE_FERN_UPPER);
    }

    #[test]
    fn glow_lichen_feature_places_against_stone_face() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_block_at_y(8, 9, 8, STONE);
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 8, 8)));
        assert_eq!(chunk.get_block_at_y(8, 8, 8), GLOW_LICHEN);
    }

    #[test]
    fn placed_feature_applies_count_then_square_decorators() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let placed = PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            ConfiguredFeature::random_patch(RandomPatchConfiguration {
                state: POPPY,
                tries: 8,
                xspread: 1,
                yspread: 1,
                zspread: 1,
                project: true,
                can_replace: false,
                double_plant: false,
                place_on: &[GRASS_BLOCK],
            }),
            vec![ConfiguredDecorator::count(2), ConfiguredDecorator::square()],
        );

        assert!(placed.place(&mut chunk, &mut random, BlockPos::new(0, 0, 0)));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == POPPY));
    }

    #[test]
    fn feature_world_heightmaps_follow_reduced_java_predicates() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_block_at_y(8, 2, 8, DIRT);
        chunk.set_block_at_y(8, 3, 8, WATER);
        chunk.set_block_at_y(8, 4, 8, GRASS);
        chunk.set_block_at_y(8, 5, 8, SPRUCE_LEAVES);
        chunk.set_block_at_y(8, 6, 8, SNOW);

        assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 8, 8), Some(7));
        assert_eq!(chunk.height_at(HeightmapType::OceanFloor, 8, 8), Some(6));
        assert_eq!(
            chunk.height_at(HeightmapType::MotionBlocking, 8, 8),
            Some(6)
        );
        assert_eq!(
            chunk.height_at(HeightmapType::MotionBlockingNoLeaves, 8, 8),
            Some(4)
        );
    }

    #[test]
    fn placed_feature_uses_world_heightmap_and_water_depth_decorators() {
        let feature = ConfiguredFeature::simple_block(
            SimpleBlockConfiguration::new(POPPY)
                .place_on(&[GRASS_BLOCK])
                .place_in(&[AIR]),
        );
        let placed = PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            feature,
            vec![
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ],
        );
        let origin = BlockPos::new(8, 0, 8);

        let mut dry_chunk = flat_grass_chunk();
        let mut dry_random = WorldgenRandom::new(12_345);
        assert!(placed.place(&mut dry_chunk, &mut dry_random, origin));
        assert_eq!(dry_chunk.get_block_at_y(8, 3, 8), POPPY);

        let mut wet_chunk = flat_grass_chunk();
        wet_chunk.set_block_at_y(8, 3, 8, WATER);
        let mut wet_random = WorldgenRandom::new(12_345);
        assert!(!placed.place(&mut wet_chunk, &mut wet_random, origin));
        assert_eq!(wet_chunk.get_block_at_y(8, 3, 8), WATER);
        assert_eq!(dry_random.get_count(), wet_random.get_count());
    }

    #[test]
    fn decorated_configured_feature_applies_nested_decorators() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::decorated(DecoratedFeatureConfiguration::new(
            ConfiguredFeature::simple_block(
                SimpleBlockConfiguration::new(DANDELION)
                    .place_on(&[GRASS_BLOCK])
                    .place_in(&[AIR]),
            ),
            [ConfiguredDecorator::heightmap(HeightmapType::OceanFloor)],
        ));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), DANDELION);
        assert_eq!(random.get_count(), 0);
    }

    #[test]
    fn ore_feature_replaces_natural_stone_blob() {
        let mut chunk = solid_stone_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 32, 8)));
        assert!(count_blocks(&chunk, DIORITE) > 0);
        assert!(count_blocks(&chunk, STONE) < CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 64);
    }

    #[test]
    fn placed_feature_interleaves_decorator_branches_with_feature_random() {
        let origin = BlockPos::new(0, 0, 0);
        let range =
            HeightProvider::uniform(VerticalAnchor::absolute(8), VerticalAnchor::absolute(48));
        let ore = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));
        let placed = PlacedFeature::new(
            DecorationStep::UndergroundOres,
            ore.clone(),
            vec![
                ConfiguredDecorator::count(2),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(range),
            ],
        );
        let mut placed_chunk = solid_stone_chunk();
        let mut placed_random = WorldgenRandom::new(12_345);

        placed.place(&mut placed_chunk, &mut placed_random, origin);

        let mut manual_chunk = solid_stone_chunk();
        let mut manual_random = WorldgenRandom::new(12_345);
        let context = DecorationContext::new(manual_chunk.min_y, manual_chunk.height);
        for _ in 0..2 {
            for square_pos in
                ConfiguredDecorator::square().get_positions(&context, &mut manual_random, origin)
            {
                for range_pos in ConfiguredDecorator::range(range).get_positions(
                    &context,
                    &mut manual_random,
                    square_pos,
                ) {
                    ore.place(&mut manual_chunk, &mut manual_random, range_pos);
                }
            }
        }

        assert_eq!(placed_chunk.blocks, manual_chunk.blocks);
        assert_eq!(placed_random.get_count(), manual_random.get_count());
    }

    #[test]
    fn basic_tree_places_configured_log_and_leaf_blocks() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(1);
        let feature = ConfiguredFeature::basic_tree(BasicTreeConfiguration::birch());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == BIRCH_LOG));
        assert!(
            chunk
                .blocks
                .iter()
                .any(|block_id| *block_id == BIRCH_LEAVES)
        );
    }

    #[test]
    fn feature_region_allows_neighbor_tree_to_spill_into_center_chunk() {
        let mut region = FeatureRegion::with_radii(
            -1,
            0,
            1,
            1,
            vec![flat_grass_chunk_at(-1, 0), flat_grass_chunk_at(0, 0)],
        );
        let mut random = WorldgenRandom::new(1);
        let feature = ConfiguredFeature::basic_tree(BasicTreeConfiguration::oak());

        assert!(feature.place(&mut region, &mut random, BlockPos::new(-1, 0, 8)));

        let center = region.chunk(0, 0).expect("center chunk exists");
        assert!(center.blocks.iter().any(|block_id| *block_id == OAK_LEAVES));
        assert_eq!(region.metrics().blocked_block_writes, 0);
    }

    #[test]
    fn feature_region_blocks_writes_beyond_cutoff() {
        let mut region = FeatureRegion::with_radii(
            0,
            0,
            2,
            1,
            vec![flat_grass_chunk_at(0, 0), flat_grass_chunk_at(2, 0)],
        );

        assert!(!region.set_block_world(BlockPos::new(32, 3, 0), POPPY));
        assert_eq!(
            region.metrics(),
            FeatureRegionMetrics {
                block_write_attempts: 1,
                blocked_block_writes: 1,
                ..FeatureRegionMetrics::default()
            }
        );
        assert_eq!(region.chunk(2, 0).unwrap().get_block_at_y(0, 3, 0), AIR);
    }

    #[test]
    #[should_panic(expected = "outside dependency window")]
    fn feature_region_rejects_reads_outside_dependency_window() {
        let mut region = FeatureRegion::with_radii(0, 0, 1, 1, vec![flat_grass_chunk()]);

        let _ = region.block_at_world(BlockPos::new(32, 3, 0));
    }

    #[test]
    fn biome_feature_tables_select_distinct_visible_families() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let birch = overworld_features_for_biome(get_layered_biome_by_id(27));
        let taiga = overworld_features_for_biome(get_layered_biome_by_id(5));
        let desert = overworld_features_for_biome(get_layered_biome_by_id(2));

        assert!(plains.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::BasicTree(BasicTreeConfiguration { log: OAK_LOG, .. })
            )
        }));
        assert!(birch.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::BasicTree(BasicTreeConfiguration { log: BIRCH_LOG, .. })
            )
        }));
        assert!(
            taiga
                .iter()
                .any(|feature| matches!(feature.feature, ConfiguredFeature::RandomSelector(_)))
        );
        assert!(desert.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: DEAD_BUSH,
                    ..
                })
            )
        }));
    }

    #[test]
    fn taiga_feature_table_uses_vanilla_taiga_vegetation_selector() {
        let taiga = overworld_features_for_biome(get_layered_biome_by_id(133));
        let vegetal_features = taiga
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();

        assert!(matches!(
            vegetal_features[0].feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                state: LARGE_FERN_LOWER,
                double_plant: true,
                ..
            })
        ));
        assert_eq!(
            vegetal_features[1].feature,
            ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld())
        );
        let feature = vegetal_features[2];

        assert_eq!(feature.step, DecorationStep::VegetalDecoration);
        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::count_extra(10, 0.1, 1),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );
        match &feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![WeightedConfiguredFeature::new(
                        ConfiguredFeature::decorated(DecoratedFeatureConfiguration::new(
                            ConfiguredFeature::tree(TreeConfiguration::pine()),
                            [ConfiguredDecorator::count_extra(6, 0.1, 1)],
                        )),
                        0.33333334,
                    )]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::spruce())
                );
            }
            other => panic!("expected random selector, got {other:?}"),
        }
    }

    #[test]
    fn biome_feature_tables_start_with_default_underground_variety() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let expected = [
            (DIRT, 33, 10, VerticalAnchor::top()),
            (GRAVEL, 33, 8, VerticalAnchor::top()),
            (GRANITE, 33, 10, VerticalAnchor::absolute(79)),
            (DIORITE, 33, 10, VerticalAnchor::absolute(79)),
            (ANDESITE, 33, 10, VerticalAnchor::absolute(79)),
            (TUFF, 33, 1, VerticalAnchor::absolute(16)),
            (DEEPSLATE, 64, 2, VerticalAnchor::absolute(16)),
        ];

        for (feature, (expected_block, expected_size, expected_count, max_y)) in
            plains.iter().zip(expected)
        {
            assert_eq!(feature.step, DecorationStep::UndergroundOres);
            assert_eq!(
                feature.decorators,
                vec![
                    ConfiguredDecorator::count(expected_count),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::range(HeightProvider::uniform(
                        VerticalAnchor::absolute(0),
                        max_y,
                    )),
                ]
            );
            match &feature.feature {
                ConfiguredFeature::Ore(config) => {
                    assert_eq!(config.size, expected_size);
                    assert_eq!(
                        config.target_states,
                        vec![OreTargetBlockState::new(
                            OreTarget::NaturalStone,
                            expected_block,
                        )]
                    );
                }
                other => panic!("expected ore feature, got {other:?}"),
            }
        }
    }

    #[test]
    fn biome_feature_tables_include_default_ores_after_variety() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let expected = [
            (
                COAL_ORE,
                DEEPSLATE_COAL_ORE,
                17,
                Some(20),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(127)),
            ),
            (
                IRON_ORE,
                DEEPSLATE_IRON_ORE,
                9,
                Some(20),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(63)),
            ),
            (
                GOLD_ORE,
                DEEPSLATE_GOLD_ORE,
                9,
                Some(2),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(31)),
            ),
            (
                REDSTONE_ORE,
                DEEPSLATE_REDSTONE_ORE,
                8,
                Some(8),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
            ),
            (
                DIAMOND_ORE,
                DEEPSLATE_DIAMOND_ORE,
                8,
                None,
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
            ),
            (
                LAPIS_ORE,
                DEEPSLATE_LAPIS_ORE,
                7,
                None,
                HeightProvider::trapezoid(
                    VerticalAnchor::absolute(0),
                    VerticalAnchor::absolute(30),
                    0,
                ),
            ),
            (
                COPPER_ORE,
                DEEPSLATE_COPPER_ORE,
                10,
                Some(6),
                HeightProvider::trapezoid(
                    VerticalAnchor::absolute(0),
                    VerticalAnchor::absolute(96),
                    0,
                ),
            ),
        ];

        for (feature, (stone_ore, deepslate_ore, size, count, height)) in
            plains.iter().skip(7).zip(expected)
        {
            assert_eq!(feature.step, DecorationStep::UndergroundOres);
            let mut expected_decorators = Vec::new();
            if let Some(count) = count {
                expected_decorators.push(ConfiguredDecorator::count(count));
            }
            expected_decorators.push(ConfiguredDecorator::square());
            expected_decorators.push(ConfiguredDecorator::range(height));
            assert_eq!(feature.decorators, expected_decorators);

            match &feature.feature {
                ConfiguredFeature::Ore(config) => {
                    assert_eq!(config.size, size);
                    assert_eq!(
                        config.target_states,
                        vec![
                            OreTargetBlockState::new(OreTarget::StoneOreReplaceables, stone_ore),
                            OreTargetBlockState::new(
                                OreTarget::DeepslateOreReplaceables,
                                deepslate_ore,
                            ),
                        ]
                    );
                }
                other => panic!("expected ore feature, got {other:?}"),
            }
        }
    }

    #[test]
    fn biome_overworld_decoration_reports_added_blocks() {
        let mut chunk = flat_grass_chunk();
        let report = apply_overworld_biome_features(12_345, get_layered_biome_by_id(4), &mut chunk);

        assert_eq!(report.biome_key, "minecraft:forest");
        assert_eq!(report.attempted_features, 20);
        assert!(report.placed_features > 0);
        assert!(report.added_non_air_blocks > 0);
    }
}
