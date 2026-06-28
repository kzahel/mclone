use crate::block::{
    ANDESITE, BIRCH_LEAVES, BIRCH_LOG, DEEPSLATE, DIORITE, GRANITE, GRASS_BLOCK, LAVA, OAK_LEAVES,
    OAK_LOG, RawBlockId, SPRUCE_LEAVES, SPRUCE_LOG, STONE, TUFF, WATER,
};
use crate::placement::{ConfiguredDecorator, IntProvider};
use crate::prng::RandomSource;

const WATER_SPRING_VALID_BLOCKS: [RawBlockId; 4] = [STONE, GRANITE, DIORITE, ANDESITE];
const LAVA_SPRING_VALID_BLOCKS: [RawBlockId; 6] =
    [STONE, GRANITE, DIORITE, ANDESITE, DEEPSLATE, TUFF];

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
pub struct LakeConfiguration {
    pub state: RawBlockId,
}

impl LakeConfiguration {
    pub const fn new(state: RawBlockId) -> Self {
        Self { state }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiskConfiguration {
    pub state: RawBlockId,
    pub radius: IntProvider,
    pub half_height: i32,
    pub targets: &'static [RawBlockId],
}

impl DiskConfiguration {
    pub const fn new(
        state: RawBlockId,
        radius: IntProvider,
        half_height: i32,
        targets: &'static [RawBlockId],
    ) -> Self {
        Self {
            state,
            radius,
            half_height,
            targets,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpringConfiguration {
    pub state: RawBlockId,
    pub requires_block_below: bool,
    pub rock_count: i32,
    pub hole_count: i32,
    pub valid_blocks: &'static [RawBlockId],
}

impl SpringConfiguration {
    pub const fn new(
        state: RawBlockId,
        requires_block_below: bool,
        rock_count: i32,
        hole_count: i32,
        valid_blocks: &'static [RawBlockId],
    ) -> Self {
        Self {
            state,
            requires_block_below,
            rock_count,
            hole_count,
            valid_blocks,
        }
    }

    pub const fn water() -> Self {
        Self::new(WATER, true, 4, 1, &WATER_SPRING_VALID_BLOCKS)
    }

    pub const fn lava() -> Self {
        Self::new(LAVA, true, 4, 1, &LAVA_SPRING_VALID_BLOCKS)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeightedBlockState {
    pub state: RawBlockId,
    pub weight: i32,
}

impl WeightedBlockState {
    pub const fn new(state: RawBlockId, weight: i32) -> Self {
        Self { state, weight }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RandomPatchConfiguration {
    pub state: RawBlockId,
    pub weighted_states: &'static [WeightedBlockState],
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
            weighted_states: &[],
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

    pub(super) fn tree_height(self, random: &mut impl RandomSource) -> i32 {
        self.base_height
            + random.next_int_bound(self.height_rand_a + 1)
            + random.next_int_bound(self.height_rand_b + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrunkPlacerConfiguration {
    Straight(StraightTrunkPlacerConfiguration),
    Fancy(StraightTrunkPlacerConfiguration),
}

impl TrunkPlacerConfiguration {
    pub const fn straight(base_height: i32, height_rand_a: i32, height_rand_b: i32) -> Self {
        Self::Straight(StraightTrunkPlacerConfiguration::new(
            base_height,
            height_rand_a,
            height_rand_b,
        ))
    }

    pub const fn fancy(base_height: i32, height_rand_a: i32, height_rand_b: i32) -> Self {
        Self::Fancy(StraightTrunkPlacerConfiguration::new(
            base_height,
            height_rand_a,
            height_rand_b,
        ))
    }

    pub(super) fn tree_height(self, random: &mut impl RandomSource) -> i32 {
        match self {
            Self::Straight(config) | Self::Fancy(config) => config.tree_height(random),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoliagePlacerConfiguration {
    Blob {
        radius: IntProvider,
        offset: IntProvider,
        height: i32,
    },
    Fancy {
        radius: IntProvider,
        offset: IntProvider,
        height: i32,
    },
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
    pub(super) fn foliage_height(
        self,
        random: &mut impl RandomSource,
        tree_height: i32,
        _config: TreeConfiguration,
    ) -> i32 {
        match self {
            Self::Blob { height, .. } => height,
            Self::Fancy { height, .. } => height,
            Self::Spruce { trunk_height, .. } => (tree_height - trunk_height.sample(random)).max(4),
            Self::Pine { height, .. } => height.sample(random),
        }
    }

    pub(super) fn foliage_radius(self, random: &mut impl RandomSource, trunk_height: i32) -> i32 {
        match self {
            Self::Blob { radius, .. } | Self::Fancy { radius, .. } => radius.sample(random),
            Self::Spruce { radius, .. } => radius.sample(random),
            Self::Pine { radius, .. } => {
                radius.sample(random) + random.next_int_bound((trunk_height + 1).max(1))
            }
        }
    }

    pub(super) fn offset(self, random: &mut impl RandomSource) -> i32 {
        match self {
            Self::Blob { offset, .. }
            | Self::Fancy { offset, .. }
            | Self::Spruce { offset, .. }
            | Self::Pine { offset, .. } => offset.sample(random),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TwoLayersFeatureSize {
    pub limit: i32,
    pub lower_size: i32,
    pub upper_size: i32,
    pub min_clipped_height: Option<i32>,
}

impl TwoLayersFeatureSize {
    pub const fn new(limit: i32, lower_size: i32, upper_size: i32) -> Self {
        Self {
            limit,
            lower_size,
            upper_size,
            min_clipped_height: None,
        }
    }

    pub const fn with_min_clipped_height(mut self, min_clipped_height: i32) -> Self {
        self.min_clipped_height = Some(min_clipped_height);
        self
    }

    pub const fn size_at_height(self, height: i32) -> i32 {
        if height < self.limit {
            self.lower_size
        } else {
            self.upper_size
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeConfiguration {
    pub log: RawBlockId,
    pub leaves: RawBlockId,
    pub trunk_placer: TrunkPlacerConfiguration,
    pub foliage_placer: FoliagePlacerConfiguration,
    pub minimum_size: TwoLayersFeatureSize,
    pub beehive_probability: Option<f32>,
}

impl Eq for TreeConfiguration {}

impl TreeConfiguration {
    pub const fn new(
        log: RawBlockId,
        leaves: RawBlockId,
        trunk_placer: TrunkPlacerConfiguration,
        foliage_placer: FoliagePlacerConfiguration,
        minimum_size: TwoLayersFeatureSize,
    ) -> Self {
        Self {
            log,
            leaves,
            trunk_placer,
            foliage_placer,
            minimum_size,
            beehive_probability: None,
        }
    }

    pub const fn with_beehive_probability(mut self, probability: f32) -> Self {
        self.beehive_probability = Some(probability);
        self
    }

    pub const fn oak() -> Self {
        Self::new(
            OAK_LOG,
            OAK_LEAVES,
            TrunkPlacerConfiguration::straight(4, 2, 0),
            FoliagePlacerConfiguration::Blob {
                radius: IntProvider::constant(2),
                offset: IntProvider::constant(0),
                height: 3,
            },
            TwoLayersFeatureSize::new(1, 0, 1),
        )
    }

    pub const fn oak_bees_0002() -> Self {
        Self::oak().with_beehive_probability(0.002)
    }

    pub const fn birch() -> Self {
        Self::new(
            BIRCH_LOG,
            BIRCH_LEAVES,
            TrunkPlacerConfiguration::straight(5, 2, 0),
            FoliagePlacerConfiguration::Blob {
                radius: IntProvider::constant(2),
                offset: IntProvider::constant(0),
                height: 3,
            },
            TwoLayersFeatureSize::new(1, 0, 1),
        )
    }

    pub const fn birch_bees_0002() -> Self {
        Self::birch().with_beehive_probability(0.002)
    }

    pub const fn fancy_oak() -> Self {
        Self::new(
            OAK_LOG,
            OAK_LEAVES,
            TrunkPlacerConfiguration::fancy(3, 11, 0),
            FoliagePlacerConfiguration::Fancy {
                radius: IntProvider::constant(2),
                offset: IntProvider::constant(4),
                height: 4,
            },
            TwoLayersFeatureSize::new(0, 0, 0).with_min_clipped_height(4),
        )
    }

    pub const fn fancy_oak_bees_0002() -> Self {
        Self::fancy_oak().with_beehive_probability(0.002)
    }

    pub const fn spruce() -> Self {
        Self::new(
            SPRUCE_LOG,
            SPRUCE_LEAVES,
            TrunkPlacerConfiguration::straight(5, 2, 1),
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
            TrunkPlacerConfiguration::straight(6, 4, 0),
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
    pub(super) fn matches(self, block_id: RawBlockId) -> bool {
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
    Noop,
    Lake(LakeConfiguration),
    Spring(SpringConfiguration),
    SimpleBlock(SimpleBlockConfiguration),
    RandomPatch(RandomPatchConfiguration),
    Flower(RandomPatchConfiguration),
    Disk(DiskConfiguration),
    GlowLichen(GlowLichenConfiguration),
    BasicTree(BasicTreeConfiguration),
    Tree(TreeConfiguration),
    RandomSelector(RandomFeatureConfiguration),
    Decorated(DecoratedFeatureConfiguration),
    Ore(OreConfiguration),
    FreezeTopLayer,
}

impl ConfiguredFeature {
    pub const fn noop() -> Self {
        Self::Noop
    }

    pub const fn lake(config: LakeConfiguration) -> Self {
        Self::Lake(config)
    }

    pub const fn spring(config: SpringConfiguration) -> Self {
        Self::Spring(config)
    }

    pub const fn simple_block(config: SimpleBlockConfiguration) -> Self {
        Self::SimpleBlock(config)
    }

    pub const fn random_patch(config: RandomPatchConfiguration) -> Self {
        Self::RandomPatch(config)
    }

    pub const fn flower(config: RandomPatchConfiguration) -> Self {
        Self::Flower(config)
    }

    pub const fn disk(config: DiskConfiguration) -> Self {
        Self::Disk(config)
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

    pub const fn freeze_top_layer() -> Self {
        Self::FreezeTopLayer
    }
}
