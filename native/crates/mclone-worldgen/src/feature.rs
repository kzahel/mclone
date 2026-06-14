use crate::block::{
    AIR, DANDELION, DIRT, GRASS, GRASS_BLOCK, MYCELIUM, OAK_LEAVES, OAK_LOG, PODZOL, POPPY,
    RawBlockId, WATER,
};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, ConfiguredDecorator, DecorationContext};
use crate::prng::{RandomSource, WorldgenRandom};

const CHUNK_WIDTH: i32 = 16;

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
            place_on: &[GRASS_BLOCK],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StarterOakTreeConfiguration {
    pub log: RawBlockId,
    pub leaves: RawBlockId,
    pub min_height: i32,
    pub random_height: i32,
}

impl StarterOakTreeConfiguration {
    pub const fn oak() -> Self {
        Self {
            log: OAK_LOG,
            leaves: OAK_LEAVES,
            min_height: 4,
            random_height: 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfiguredFeature {
    SimpleBlock(SimpleBlockConfiguration),
    RandomPatch(RandomPatchConfiguration),
    StarterOakTree(StarterOakTreeConfiguration),
}

impl ConfiguredFeature {
    pub const fn simple_block(config: SimpleBlockConfiguration) -> Self {
        Self::SimpleBlock(config)
    }

    pub const fn random_patch(config: RandomPatchConfiguration) -> Self {
        Self::RandomPatch(config)
    }

    pub const fn starter_oak_tree(config: StarterOakTreeConfiguration) -> Self {
        Self::StarterOakTree(config)
    }

    pub fn place(
        &self,
        chunk: &mut MutableChunkBlockBuffer,
        random: &mut impl RandomSource,
        origin: BlockPos,
    ) -> bool {
        match *self {
            Self::SimpleBlock(config) => place_simple_block(chunk, random, origin, config),
            Self::RandomPatch(config) => place_random_patch(chunk, random, origin, config),
            Self::StarterOakTree(config) => place_starter_oak_tree(chunk, random, origin, config),
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

    pub fn place(
        &self,
        chunk: &mut MutableChunkBlockBuffer,
        random: &mut impl RandomSource,
        origin: BlockPos,
    ) -> bool {
        let decoration_context = DecorationContext::new(chunk.min_y, chunk.height);
        let mut positions = vec![origin];
        for decorator in &self.decorators {
            positions = positions
                .into_iter()
                .flat_map(|pos| decorator.get_positions(&decoration_context, random, pos))
                .collect();
            if positions.is_empty() {
                return false;
            }
        }

        let mut placed_any = false;
        for pos in positions {
            placed_any |= self.feature.place(chunk, random, pos);
        }
        placed_any
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecorationReport {
    pub attempted_features: usize,
    pub placed_features: usize,
    pub added_non_air_blocks: usize,
}

pub fn apply_starter_overworld_decoration(
    seed: i64,
    chunk: &mut MutableChunkBlockBuffer,
) -> DecorationReport {
    let before = chunk.non_air_block_count();
    let min_block_x = chunk.chunk_x * CHUNK_WIDTH;
    let min_block_z = chunk.chunk_z * CHUNK_WIDTH;
    let origin = BlockPos::new(min_block_x, chunk.min_y, min_block_z);
    let features = starter_overworld_features();
    let mut random = WorldgenRandom::default();
    let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);
    let mut placed_features = 0;

    for (index, feature) in features.iter().enumerate() {
        random.set_feature_seed(decoration_seed, index as i32, feature.step.index());
        if feature.place(chunk, &mut random, origin) {
            placed_features += 1;
        }
    }

    DecorationReport {
        attempted_features: features.len(),
        placed_features,
        added_non_air_blocks: chunk.non_air_block_count().saturating_sub(before),
    }
}

pub fn starter_overworld_features() -> Vec<PlacedFeature> {
    vec![
        PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            ConfiguredFeature::starter_oak_tree(StarterOakTreeConfiguration::oak()),
            vec![ConfiguredDecorator::count(2), ConfiguredDecorator::square()],
        ),
        PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            ConfiguredFeature::random_patch(RandomPatchConfiguration::new(GRASS)),
            vec![ConfiguredDecorator::count(2), ConfiguredDecorator::square()],
        ),
        PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            ConfiguredFeature::random_patch(RandomPatchConfiguration::new(DANDELION)),
            vec![ConfiguredDecorator::count(1), ConfiguredDecorator::square()],
        ),
        PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            ConfiguredFeature::random_patch(RandomPatchConfiguration::new(POPPY)),
            vec![ConfiguredDecorator::count(1), ConfiguredDecorator::square()],
        ),
    ]
}

fn place_simple_block(
    chunk: &mut MutableChunkBlockBuffer,
    _random: &mut impl RandomSource,
    origin: BlockPos,
    config: SimpleBlockConfiguration,
) -> bool {
    let below = BlockPos::new(origin.x, origin.y - 1, origin.z);
    let above = BlockPos::new(origin.x, origin.y + 1, origin.z);
    let Some(block_below) = block_at_world(chunk, below) else {
        return false;
    };
    let Some(current) = block_at_world(chunk, origin) else {
        return false;
    };
    let Some(block_above) = block_at_world(chunk, above) else {
        return false;
    };

    if !matches_allowed(config.place_on, block_below)
        || !matches_allowed(config.place_in, current)
        || !matches_allowed(config.place_under, block_above)
        || !can_survive_simple_plant(config.to_place, current, block_below)
    {
        return false;
    }

    set_block_world(chunk, origin, config.to_place)
}

fn place_random_patch(
    chunk: &mut MutableChunkBlockBuffer,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: RandomPatchConfiguration,
) -> bool {
    let projected = if config.project {
        project_to_surface(chunk, origin).unwrap_or(origin)
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
        let Some(current) = block_at_world(chunk, pos) else {
            continue;
        };
        let Some(block_below) = block_at_world(chunk, below) else {
            continue;
        };
        let can_replace = current == AIR || (config.can_replace && is_replaceable_plant(current));
        if can_replace
            && matches_allowed(config.place_on, block_below)
            && can_survive_simple_plant(config.state, current, block_below)
            && set_block_world(chunk, pos, config.state)
        {
            placed += 1;
        }
    }

    placed > 0
}

fn place_starter_oak_tree(
    chunk: &mut MutableChunkBlockBuffer,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: StarterOakTreeConfiguration,
) -> bool {
    let Some(base) = project_to_surface(chunk, origin) else {
        return false;
    };
    let below = BlockPos::new(base.x, base.y - 1, base.z);
    let Some(block_below) = block_at_world(chunk, below) else {
        return false;
    };
    if !matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM) {
        return false;
    }

    let height = config.min_height + random.next_int_bound(config.random_height.max(1));
    let leaves_center_y = base.y + height;
    if base.y < chunk.min_y + 1 || leaves_center_y + 1 >= chunk.min_y + chunk.height {
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
        .any(|(pos, _)| !can_replace_tree_block(chunk, *pos))
    {
        return false;
    }

    set_block_world(chunk, below, DIRT);
    for (pos, block_id) in targets {
        set_block_world(chunk, pos, block_id);
    }
    true
}

fn project_to_surface(chunk: &MutableChunkBlockBuffer, pos: BlockPos) -> Option<BlockPos> {
    let local_x = pos.x - chunk.chunk_x * CHUNK_WIDTH;
    let local_z = pos.z - chunk.chunk_z * CHUNK_WIDTH;
    if !(0..CHUNK_WIDTH).contains(&local_x) || !(0..CHUNK_WIDTH).contains(&local_z) {
        return None;
    }
    Some(BlockPos::new(
        pos.x,
        chunk.world_surface_height(local_x, local_z),
        pos.z,
    ))
}

fn block_at_world(chunk: &MutableChunkBlockBuffer, pos: BlockPos) -> Option<RawBlockId> {
    let local_x = pos.x - chunk.chunk_x * CHUNK_WIDTH;
    let local_z = pos.z - chunk.chunk_z * CHUNK_WIDTH;
    if !(0..CHUNK_WIDTH).contains(&local_x)
        || !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y)
        || !(0..CHUNK_WIDTH).contains(&local_z)
    {
        return None;
    }
    Some(chunk.get_block_at_y(local_x, pos.y, local_z))
}

fn set_block_world(
    chunk: &mut MutableChunkBlockBuffer,
    pos: BlockPos,
    block_id: RawBlockId,
) -> bool {
    let local_x = pos.x - chunk.chunk_x * CHUNK_WIDTH;
    let local_z = pos.z - chunk.chunk_z * CHUNK_WIDTH;
    if !(0..CHUNK_WIDTH).contains(&local_x)
        || !(chunk.min_y..chunk.min_y + chunk.height).contains(&pos.y)
        || !(0..CHUNK_WIDTH).contains(&local_z)
    {
        return false;
    }
    chunk.set_block_at_y(local_x, pos.y, local_z, block_id);
    true
}

fn matches_allowed(allowed: &[RawBlockId], block_id: RawBlockId) -> bool {
    allowed.is_empty() || allowed.contains(&block_id)
}

fn can_survive_simple_plant(
    block_id: RawBlockId,
    current: RawBlockId,
    block_below: RawBlockId,
) -> bool {
    matches!(block_id, GRASS | DANDELION | POPPY)
        && current == AIR
        && matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
}

fn is_replaceable_plant(block_id: RawBlockId) -> bool {
    matches!(block_id, GRASS | DANDELION | POPPY)
}

fn can_replace_tree_block(chunk: &MutableChunkBlockBuffer, pos: BlockPos) -> bool {
    let Some(block_id) = block_at_world(chunk, pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | WATER | GRASS | DANDELION | POPPY | OAK_LEAVES | OAK_LOG
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::STONE;
    use crate::prng::WorldgenRandom;

    fn flat_grass_chunk() -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 32);
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                chunk.set_block_at_y(x, 0, z, STONE);
                chunk.set_block_at_y(x, 1, z, DIRT);
                chunk.set_block_at_y(x, 2, z, GRASS_BLOCK);
            }
        }
        chunk
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
            place_on: &[GRASS_BLOCK],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(chunk.non_air_block_count() > before);
        assert!(chunk.blocks.iter().any(|block_id| *block_id == GRASS));
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
                place_on: &[GRASS_BLOCK],
            }),
            vec![ConfiguredDecorator::count(2), ConfiguredDecorator::square()],
        );

        assert!(placed.place(&mut chunk, &mut random, BlockPos::new(0, 0, 0)));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == POPPY));
    }

    #[test]
    fn starter_oak_tree_places_log_and_leaf_blocks() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(1);
        let feature = ConfiguredFeature::starter_oak_tree(StarterOakTreeConfiguration::oak());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == OAK_LOG));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == OAK_LEAVES));
    }

    #[test]
    fn starter_overworld_decoration_reports_added_blocks() {
        let mut chunk = flat_grass_chunk();
        let report = apply_starter_overworld_decoration(12_345, &mut chunk);

        assert_eq!(report.attempted_features, 4);
        assert!(report.placed_features > 0);
        assert!(report.added_non_air_blocks > 0);
    }
}
