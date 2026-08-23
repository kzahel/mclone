use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_core::{BlockStateId, CHUNK_WIDTH, chunk_block_index};
use mclone_mesh::{
    TexturedChunkMeshInput, TexturedMeshCatalog, TexturedRenderSectionMesh,
    TexturedVisibleChunkMesh, build_textured_render_sections_for_chunk_set,
    pack_textured_render_sections,
};
use mclone_worldgen::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, BIRCH_LOG, BIRCH_LOG_X, BIRCH_LOG_Z, DARK_OAK_LOG,
    JUNGLE_LEAVES, JUNGLE_LOG, MUSHROOM_STEM, OAK_LEAVES, OAK_LOG, OAK_LOG_X, OAK_LOG_Z,
    RawBlockId, SPRUCE_LEAVES, SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z, WATER, WATER_LEVEL_1,
    WATER_LEVEL_8,
};
use mclone_worldgen::levelgen::{
    McloneOverworldSamplingTopology, McloneOverworldVegetationPlanCache, McloneTreeFamily,
    McloneTreeOccurrence, McloneVegetationBounds, McloneVegetationSource,
};
use mclone_worldgen::terrain_preview::{
    TerrainPreviewProfile, continental_candidate_tree_records_intersecting,
};

use super::{
    CanonicalTerrainChunk, CanonicalTerrainCompiler, CanonicalTerrainStage,
    CanonicalTerrainVisibility, McloneTreeOccurrenceId, canonical_terrain_presentation_blocks,
};

const CANONICAL_WORKER_RAW_CACHE_MAX_CHUNKS: usize = 1_024;

fn canonical_exact_surface_columns(
    chunk: &CanonicalTerrainChunk,
    catalog: &TexturedMeshCatalog,
) -> Vec<CanonicalExactSurfaceColumn> {
    canonical_exact_surface_columns_with_solid(chunk, catalog, |state| catalog.occludes(state))
}

fn canonical_exact_surface_columns_with_solid(
    chunk: &CanonicalTerrainChunk,
    catalog: &TexturedMeshCatalog,
    solid: impl Fn(BlockStateId) -> bool,
) -> Vec<CanonicalExactSurfaceColumn> {
    let mut columns = Vec::with_capacity((CHUNK_WIDTH * CHUNK_WIDTH) as usize);
    for local_z in 0..CHUNK_WIDTH {
        for local_x in 0..CHUNK_WIDTH {
            let mut water = false;
            let mut column = None;
            for local_y in (0..chunk.height).rev() {
                let block = chunk.blocks[chunk_block_index(local_x, local_y, local_z)];
                if block == WATER || (WATER_LEVEL_1..=WATER_LEVEL_8).contains(&block) {
                    water = true;
                    continue;
                }
                let state = BlockStateId(u32::from(block));
                if !solid(state) || canonical_boundary_natural_feature_block(block) {
                    continue;
                }
                let side_material = u8::try_from(block)
                    .ok()
                    .filter(|_| catalog.terrain_surface_material(state).is_some());
                column = Some(CanonicalExactSurfaceColumn {
                    solid_top_y: i16::try_from(chunk.min_y + local_y + 1)
                        .expect("canonical exact surface height fits i16"),
                    side_material,
                    water,
                });
                break;
            }
            columns.push(column.unwrap_or(CanonicalExactSurfaceColumn {
                solid_top_y: i16::try_from(chunk.min_y).unwrap_or(i16::MIN),
                side_material: None,
                water,
            }));
        }
    }
    columns
}

fn canonical_boundary_natural_feature_block(block: RawBlockId) -> bool {
    matches!(
        block,
        OAK_LOG
            | BIRCH_LOG
            | SPRUCE_LOG
            | OAK_LOG_X
            | OAK_LOG_Z
            | BIRCH_LOG_X
            | BIRCH_LOG_Z
            | SPRUCE_LOG_X
            | SPRUCE_LOG_Z
            | DARK_OAK_LOG
            | MUSHROOM_STEM
            | ACACIA_LOG
            | JUNGLE_LOG
    )
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalMeshCoordinate {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl CanonicalMeshCoordinate {
    pub const fn new(chunk_x: i32, chunk_z: i32) -> Self {
        Self { chunk_x, chunk_z }
    }

    fn tuple(self) -> (i32, i32) {
        (self.chunk_x, self.chunk_z)
    }
}

#[derive(Clone, Debug)]
pub struct CanonicalMeshRequestReceipt {
    pub coordinate: CanonicalMeshCoordinate,
    pub raw_cache_hit: bool,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug)]
pub struct CanonicalPackedNaturalTree {
    pub occurrence: McloneTreeOccurrence,
    pub packed_sections: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct CanonicalPackedAdmission {
    pub requested: CanonicalMeshRequestReceipt,
    pub packed_sections: Vec<u8>,
    pub natural_trees: Vec<CanonicalPackedNaturalTree>,
    pub surface_columns: Vec<CanonicalExactSurfaceColumn>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalExactSurfaceColumn {
    pub solid_top_y: i16,
    pub side_material: Option<u8>,
    pub water: bool,
}

#[derive(Clone, Debug)]
pub struct CanonicalMeshBatch {
    pub admissions: Vec<CanonicalPackedAdmission>,
    pub generation_ms: f64,
    pub presentation_ms: f64,
    pub mesh_ms: f64,
    pub pack_ms: f64,
    pub deduplicated_target_chunks: usize,
    pub raw_cache_chunks: usize,
    pub raw_cache_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CanonicalMeshFrontier {
    #[default]
    SuppressFootprintWalls,
    RetainFootprintWalls,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CanonicalNaturalTreePresentation {
    #[default]
    Integrated,
    Separated,
}

pub struct CanonicalMeshSession {
    compiler: CanonicalTerrainCompiler,
    catalog: TexturedMeshCatalog,
    profile: TerrainPreviewProfile,
    seed: i64,
    stage: CanonicalTerrainStage,
    chunks: BTreeMap<(i32, i32), CanonicalTerrainChunk>,
    raw_lru: VecDeque<(i32, i32)>,
    desired: BTreeSet<(i32, i32)>,
    visibility: CanonicalTerrainVisibility,
    frontier: CanonicalMeshFrontier,
    natural_tree_presentation: CanonicalNaturalTreePresentation,
    vegetation_cache: Option<McloneOverworldVegetationPlanCache>,
}

impl CanonicalMeshSession {
    pub fn new(
        profile: TerrainPreviewProfile,
        seed: i64,
        stage: CanonicalTerrainStage,
        catalog: TexturedMeshCatalog,
    ) -> Self {
        Self {
            compiler: CanonicalTerrainCompiler::new_with_profile(profile, seed, stage),
            catalog,
            profile,
            seed,
            stage,
            chunks: BTreeMap::new(),
            raw_lru: VecDeque::new(),
            desired: BTreeSet::new(),
            visibility: CanonicalTerrainVisibility::default(),
            frontier: CanonicalMeshFrontier::default(),
            natural_tree_presentation: CanonicalNaturalTreePresentation::default(),
            vegetation_cache: None,
        }
    }

    pub fn set_frontier(&mut self, frontier: CanonicalMeshFrontier) {
        self.frontier = frontier;
    }

    pub const fn frontier(&self) -> CanonicalMeshFrontier {
        self.frontier
    }

    pub fn set_natural_tree_presentation(
        &mut self,
        presentation: CanonicalNaturalTreePresentation,
    ) {
        self.natural_tree_presentation = presentation;
    }

    pub const fn natural_tree_presentation(&self) -> CanonicalNaturalTreePresentation {
        self.natural_tree_presentation
    }

    pub fn begin(
        &mut self,
        desired: impl IntoIterator<Item = CanonicalMeshCoordinate>,
        visibility: CanonicalTerrainVisibility,
        cache_enabled: bool,
    ) {
        self.desired = desired
            .into_iter()
            .map(CanonicalMeshCoordinate::tuple)
            .collect();
        self.visibility = visibility;
        if !cache_enabled {
            self.chunks
                .retain(|position, _| self.desired.contains(position));
            self.raw_lru
                .retain(|position| self.desired.contains(position));
        }
        let retained = self
            .raw_lru
            .iter()
            .copied()
            .filter(|position| self.desired.contains(position))
            .collect::<Vec<_>>();
        for position in retained {
            self.touch_raw(position);
        }
        self.evict_raw_cache();
    }

    pub fn compile_batch(
        &mut self,
        requested: &[CanonicalMeshCoordinate],
    ) -> Result<CanonicalMeshBatch, String> {
        if requested.is_empty() {
            return Err("canonical mesh batch is empty".to_owned());
        }
        if requested
            .iter()
            .any(|coordinate| !self.desired.contains(&coordinate.tuple()))
        {
            return Err(
                "canonical mesh batch contains a coordinate outside the desired set".into(),
            );
        }

        let generation_started = timing_now();
        let mut receipts = Vec::with_capacity(requested.len());
        for coordinate in requested {
            let position = coordinate.tuple();
            let raw_cache_hit = self.chunks.contains_key(&position);
            if !raw_cache_hit {
                let chunk = self
                    .compiler
                    .compile(coordinate.chunk_x, coordinate.chunk_z);
                self.chunks.insert(position, chunk);
            }
            self.touch_raw(position);
            let chunk = self
                .chunks
                .get(&position)
                .expect("the requested canonical raw chunk is resident");
            receipts.push(CanonicalMeshRequestReceipt {
                coordinate: *coordinate,
                raw_cache_hit,
                retained_dependency_chunks: chunk.dependency_cache.retained_dependency_chunks,
            });
        }
        self.evict_raw_cache();
        let generation_ms = timing_elapsed_ms(generation_started);

        let requested_positions = requested
            .iter()
            .copied()
            .map(CanonicalMeshCoordinate::tuple)
            .collect::<BTreeSet<_>>();
        let targets = requested_positions
            .iter()
            .flat_map(|(chunk_x, chunk_z)| {
                [
                    (*chunk_x, *chunk_z),
                    (*chunk_x - 1, *chunk_z),
                    (*chunk_x + 1, *chunk_z),
                    (*chunk_x, *chunk_z - 1),
                    (*chunk_x, *chunk_z + 1),
                ]
            })
            .filter(|position| {
                self.desired.contains(position) && self.chunks.contains_key(position)
            })
            .collect::<BTreeSet<_>>();
        let input_positions = targets
            .iter()
            .flat_map(|(chunk_x, chunk_z)| {
                [
                    (*chunk_x, *chunk_z),
                    (*chunk_x - 1, *chunk_z),
                    (*chunk_x + 1, *chunk_z),
                    (*chunk_x, *chunk_z - 1),
                    (*chunk_x, *chunk_z + 1),
                ]
            })
            .filter(|position| {
                self.desired.contains(position) && self.chunks.contains_key(position)
            })
            .collect::<BTreeSet<_>>();

        let separated_occurrences = self.separated_natural_tree_occurrences(&input_positions)?;
        let presentation_started = timing_now();
        let mut presented = input_positions
            .iter()
            .filter_map(|position| {
                let chunk = self.chunks.get(position)?;
                let blocks = canonical_terrain_presentation_blocks(&chunk.blocks, self.visibility)
                    .into_iter()
                    .map(|block| BlockStateId(u32::from(block)))
                    .collect::<Vec<_>>();
                Some((*position, blocks))
            })
            .collect::<BTreeMap<_, _>>();
        let natural_tree_blocks =
            separate_natural_tree_blocks(&self.chunks, &mut presented, &separated_occurrences);
        let inputs = presented
            .iter()
            .filter_map(|((chunk_x, chunk_z), blocks)| {
                let chunk = self.chunks.get(&(*chunk_x, *chunk_z))?;
                Some(
                    TexturedChunkMeshInput::new(
                        *chunk_x,
                        *chunk_z,
                        chunk.min_y,
                        chunk.height,
                        blocks,
                    )
                    .with_biomes(&chunk.biomes)
                    .with_world_seed(chunk.seed)
                    .with_fluids_visible(self.visibility.water),
                )
            })
            .collect::<Vec<_>>();
        let presentation_ms = timing_elapsed_ms(presentation_started);

        let mesh_started = timing_now();
        let mut sections =
            build_textured_render_sections_for_chunk_set(&inputs, &self.catalog, &targets)
                .map_err(|error| format!("failed to mesh canonical terrain in Worker: {error}"))?;
        let natural_tree_meshes = mesh_separated_natural_trees(
            &self.chunks,
            &input_positions,
            &targets,
            &natural_tree_blocks,
            &self.catalog,
        )?;
        let active = self
            .desired
            .iter()
            .copied()
            .filter(|position| self.chunks.contains_key(position))
            .collect::<BTreeSet<_>>();
        if self.frontier == CanonicalMeshFrontier::SuppressFootprintWalls {
            suppress_missing_footprint_walls(&mut sections, &active);
        }
        let mesh_ms = timing_elapsed_ms(mesh_started);

        let mut sections_by_chunk = BTreeMap::<(i32, i32), Vec<_>>::new();
        for section in sections {
            sections_by_chunk
                .entry((section.key.chunk_x, section.key.chunk_z))
                .or_default()
                .push(section);
        }
        let mut admission_sections = requested_positions
            .iter()
            .copied()
            .map(|position| {
                (
                    position,
                    sections_by_chunk.remove(&position).unwrap_or_default(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (correction, correction_sections) in sections_by_chunk {
            let owner = requested_positions
                .iter()
                .copied()
                .find(|requested| {
                    (requested.0 - correction.0).abs() + (requested.1 - correction.1).abs() == 1
                })
                .or_else(|| requested_positions.iter().next().copied())
                .expect("a non-empty batch has one requested admission");
            admission_sections
                .entry(owner)
                .or_default()
                .extend(correction_sections);
        }
        let mut admission_trees = requested_positions
            .iter()
            .copied()
            .map(|position| (position, Vec::new()))
            .collect::<BTreeMap<_, Vec<CanonicalNaturalTreeMesh>>>();
        for natural_tree in natural_tree_meshes {
            let base = natural_tree
                .occurrence
                .working_base()
                .map_err(|error| format!("canonical natural-tree base is invalid: {error}"))?;
            let base_chunk = (base.x.div_euclid(16), base.z.div_euclid(16));
            let owner = requested_positions
                .iter()
                .copied()
                .min_by_key(|requested| {
                    (
                        (requested.0 - base_chunk.0)
                            .abs()
                            .saturating_add((requested.1 - base_chunk.1).abs()),
                        *requested,
                    )
                })
                .expect("a non-empty batch has one requested tree admission");
            admission_trees.entry(owner).or_default().push(natural_tree);
        }

        let pack_started = timing_now();
        let mut admissions = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let coordinate = receipt.coordinate.tuple();
            let sections = admission_sections.remove(&coordinate).unwrap_or_default();
            admissions.push(CanonicalPackedAdmission {
                requested: receipt,
                packed_sections: pack_textured_render_sections(&sections),
                natural_trees: admission_trees
                    .remove(&coordinate)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|tree| CanonicalPackedNaturalTree {
                        occurrence: tree.occurrence,
                        packed_sections: pack_textured_render_sections(&tree.sections),
                    })
                    .collect(),
                surface_columns: canonical_exact_surface_columns(
                    self.chunks
                        .get(&coordinate)
                        .expect("requested canonical surface chunk remains resident"),
                    &self.catalog,
                ),
            });
        }
        let pack_ms = timing_elapsed_ms(pack_started);

        Ok(CanonicalMeshBatch {
            admissions,
            generation_ms,
            presentation_ms,
            mesh_ms,
            pack_ms,
            deduplicated_target_chunks: targets.len(),
            raw_cache_chunks: self.chunks.len(),
            raw_cache_bytes: self.raw_cache_bytes(),
        })
    }

    pub fn raw_cache_chunks(&self) -> usize {
        self.chunks.len()
    }

    pub fn raw_cache_bytes(&self) -> u64 {
        self.chunks.values().fold(0_u64, |bytes, chunk| {
            bytes
                .saturating_add(chunk.blocks.len() as u64)
                .saturating_add(
                    (chunk.biomes.len() as u64).saturating_mul(std::mem::size_of::<i32>() as u64),
                )
        })
    }

    fn separated_natural_tree_occurrences(
        &mut self,
        input_positions: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<McloneTreeOccurrence>, String> {
        if self.natural_tree_presentation != CanonicalNaturalTreePresentation::Separated
            || self.stage != CanonicalTerrainStage::FinalFeatures
            || !self.visibility.vegetation
            || input_positions.is_empty()
        {
            return Ok(Vec::new());
        }
        let min_chunk_x = input_positions
            .iter()
            .map(|(chunk_x, _)| *chunk_x)
            .min()
            .expect("non-empty input positions have a minimum X");
        let max_chunk_x = input_positions
            .iter()
            .map(|(chunk_x, _)| *chunk_x)
            .max()
            .expect("non-empty input positions have a maximum X");
        let min_chunk_z = input_positions
            .iter()
            .map(|(_, chunk_z)| *chunk_z)
            .min()
            .expect("non-empty input positions have a minimum Z");
        let max_chunk_z = input_positions
            .iter()
            .map(|(_, chunk_z)| *chunk_z)
            .max()
            .expect("non-empty input positions have a maximum Z");
        let min_x = min_chunk_x
            .checked_mul(CHUNK_WIDTH)
            .ok_or("canonical natural-tree minimum X overflow")?;
        let min_z = min_chunk_z
            .checked_mul(CHUNK_WIDTH)
            .ok_or("canonical natural-tree minimum Z overflow")?;
        let max_x = max_chunk_x
            .checked_add(1)
            .and_then(|value| value.checked_mul(CHUNK_WIDTH))
            .and_then(|value| value.checked_sub(1))
            .ok_or("canonical natural-tree maximum X overflow")?;
        let max_z = max_chunk_z
            .checked_add(1)
            .and_then(|value| value.checked_mul(CHUNK_WIDTH))
            .and_then(|value| value.checked_sub(1))
            .ok_or("canonical natural-tree maximum Z overflow")?;
        let bounds = McloneVegetationBounds::new(min_x, min_z, max_x, max_z)
            .map_err(|error| error.to_string())?;
        match self.profile {
            TerrainPreviewProfile::McloneOverworldV1 => {
                let source = McloneVegetationSource::new(
                    self.seed,
                    McloneOverworldSamplingTopology::Unbounded,
                );
                let cache = self
                    .vegetation_cache
                    .get_or_insert_with(|| McloneOverworldVegetationPlanCache::new(source));
                if !cache.matches(source) {
                    *cache = McloneOverworldVegetationPlanCache::new(source);
                }
                cache
                    .tree_records_intersecting(bounds)
                    .map_err(|error| error.to_string())
            }
            TerrainPreviewProfile::ContinentalEcoregionCandidate
            | TerrainPreviewProfile::McloneOverworldV2 => {
                continental_candidate_tree_records_intersecting(self.seed, bounds)
            }
            TerrainPreviewProfile::VanillaOverworld | TerrainPreviewProfile::McloneOverworldV3 => {
                Ok(Vec::new())
            }
        }
    }

    fn touch_raw(&mut self, position: (i32, i32)) {
        self.raw_lru.retain(|candidate| *candidate != position);
        self.raw_lru.push_back(position);
    }

    fn evict_raw_cache(&mut self) {
        let mut attempts = self.raw_lru.len();
        while self.chunks.len() > CANONICAL_WORKER_RAW_CACHE_MAX_CHUNKS && attempts > 0 {
            attempts -= 1;
            let Some(position) = self.raw_lru.pop_front() else {
                break;
            };
            if self.desired.contains(&position) {
                self.raw_lru.push_back(position);
            } else {
                self.chunks.remove(&position);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct CanonicalNaturalTreeBlocks {
    occurrence: McloneTreeOccurrence,
    chunks: BTreeMap<(i32, i32), Vec<BlockStateId>>,
}

#[derive(Clone, Debug)]
struct CanonicalNaturalTreeMesh {
    occurrence: McloneTreeOccurrence,
    sections: Vec<TexturedRenderSectionMesh>,
}

fn separate_natural_tree_blocks(
    chunks: &BTreeMap<(i32, i32), CanonicalTerrainChunk>,
    presented: &mut BTreeMap<(i32, i32), Vec<BlockStateId>>,
    occurrences: &[McloneTreeOccurrence],
) -> BTreeMap<McloneTreeOccurrenceId, CanonicalNaturalTreeBlocks> {
    let mut assignments = BTreeMap::<[i32; 3], (McloneTreeOccurrenceId, RawBlockId)>::new();
    let mut occurrence_by_id = BTreeMap::new();
    for occurrence in occurrences {
        let id = McloneTreeOccurrenceId::from(*occurrence);
        occurrence_by_id.insert(id, *occurrence);
        let bounds = occurrence.working_bounds;
        for world_y in bounds.min_y..=bounds.max_y {
            for world_z in bounds.min_z..=bounds.max_z {
                for world_x in bounds.min_x..=bounds.max_x {
                    let chunk_position = (world_x.div_euclid(16), world_z.div_euclid(16));
                    let Some(chunk) = chunks.get(&chunk_position) else {
                        continue;
                    };
                    let local_y = world_y - chunk.min_y;
                    if !(0..chunk.height).contains(&local_y) {
                        continue;
                    }
                    let index =
                        chunk_block_index(world_x.rem_euclid(16), local_y, world_z.rem_euclid(16));
                    let block = chunk.blocks[index];
                    if natural_tree_block_matches(occurrence.record.family, block) {
                        assignments.insert([world_x, world_y, world_z], (id, block));
                    }
                }
            }
        }
    }

    let mut separated = BTreeMap::<McloneTreeOccurrenceId, CanonicalNaturalTreeBlocks>::new();
    for ([world_x, world_y, world_z], (id, block)) in assignments {
        let chunk_position = (world_x.div_euclid(16), world_z.div_euclid(16));
        let Some(chunk) = chunks.get(&chunk_position) else {
            continue;
        };
        let local_y = world_y - chunk.min_y;
        let index = chunk_block_index(world_x.rem_euclid(16), local_y, world_z.rem_euclid(16));
        if let Some(blocks) = presented.get_mut(&chunk_position) {
            blocks[index] = BlockStateId(u32::from(AIR));
        }
        let tree = separated
            .entry(id)
            .or_insert_with(|| CanonicalNaturalTreeBlocks {
                occurrence: occurrence_by_id[&id],
                chunks: BTreeMap::new(),
            });
        let blocks = tree
            .chunks
            .entry(chunk_position)
            .or_insert_with(|| vec![BlockStateId(u32::from(AIR)); chunk.blocks.len()]);
        blocks[index] = BlockStateId(u32::from(block));
    }
    separated
}

fn natural_tree_block_matches(family: McloneTreeFamily, block: RawBlockId) -> bool {
    match family {
        McloneTreeFamily::TemperateBroadleaf => matches!(block, OAK_LOG | OAK_LEAVES),
        McloneTreeFamily::CoolWetConifer => matches!(block, SPRUCE_LOG | SPRUCE_LEAVES),
        McloneTreeFamily::WarmDryAcacia => matches!(block, ACACIA_LOG | ACACIA_LEAVES),
        McloneTreeFamily::HumidJungleBroadleaf => matches!(block, JUNGLE_LOG | JUNGLE_LEAVES),
    }
}

fn mesh_separated_natural_trees(
    chunks: &BTreeMap<(i32, i32), CanonicalTerrainChunk>,
    input_positions: &BTreeSet<(i32, i32)>,
    targets: &BTreeSet<(i32, i32)>,
    natural_trees: &BTreeMap<McloneTreeOccurrenceId, CanonicalNaturalTreeBlocks>,
    catalog: &TexturedMeshCatalog,
) -> Result<Vec<CanonicalNaturalTreeMesh>, String> {
    let mut meshes = Vec::new();
    for tree in natural_trees.values() {
        let tree_targets = tree
            .chunks
            .keys()
            .copied()
            .filter(|position| targets.contains(position))
            .collect::<BTreeSet<_>>();
        if tree_targets.is_empty() {
            continue;
        }
        let presented = input_positions
            .iter()
            .filter_map(|position| {
                let chunk = chunks.get(position)?;
                let blocks = tree
                    .chunks
                    .get(position)
                    .cloned()
                    .unwrap_or_else(|| vec![BlockStateId(u32::from(AIR)); chunk.blocks.len()]);
                Some((*position, blocks))
            })
            .collect::<Vec<_>>();
        let inputs = presented
            .iter()
            .filter_map(|((chunk_x, chunk_z), blocks)| {
                let chunk = chunks.get(&(*chunk_x, *chunk_z))?;
                Some(
                    TexturedChunkMeshInput::new(
                        *chunk_x,
                        *chunk_z,
                        chunk.min_y,
                        chunk.height,
                        blocks,
                    )
                    .with_biomes(&chunk.biomes)
                    .with_world_seed(chunk.seed)
                    .with_fluids_visible(false),
                )
            })
            .collect::<Vec<_>>();
        let sections =
            build_textured_render_sections_for_chunk_set(&inputs, catalog, &tree_targets)
                .map_err(|error| format!("failed to mesh separated canonical tree: {error}"))?;
        if !sections.is_empty() {
            meshes.push(CanonicalNaturalTreeMesh {
                occurrence: tree.occurrence,
                sections,
            });
        }
    }
    Ok(meshes)
}

pub fn suppress_missing_footprint_walls(
    sections: &mut [TexturedRenderSectionMesh],
    chunks: &BTreeSet<(i32, i32)>,
) {
    let Some(min_chunk_x) = chunks.iter().map(|(chunk_x, _)| *chunk_x).min() else {
        return;
    };
    let Some(max_chunk_x) = chunks.iter().map(|(chunk_x, _)| *chunk_x).max() else {
        return;
    };
    let Some(min_chunk_z) = chunks.iter().map(|(_, chunk_z)| *chunk_z).min() else {
        return;
    };
    let Some(max_chunk_z) = chunks.iter().map(|(_, chunk_z)| *chunk_z).max() else {
        return;
    };
    let bounds = [
        min_chunk_x as f32 * 16.0,
        (max_chunk_x + 1) as f32 * 16.0,
        min_chunk_z as f32 * 16.0,
        (max_chunk_z + 1) as f32 * 16.0,
    ];
    for section in sections {
        suppress_mesh_boundary_quads(&mut section.mesh, bounds);
    }
}

fn suppress_mesh_boundary_quads(mesh: &mut TexturedVisibleChunkMesh, bounds: [f32; 4]) {
    let old_indices = std::mem::take(&mut mesh.indices);
    let solid_end = mesh.solid_index_count.min(old_indices.len() as u32) as usize;
    let opaque_end = mesh.opaque_index_count.min(old_indices.len() as u32) as usize;
    append_non_boundary_quads(mesh, &old_indices[..solid_end], bounds);
    mesh.solid_index_count = mesh.indices.len() as u32;
    append_non_boundary_quads(mesh, &old_indices[solid_end..opaque_end], bounds);
    mesh.opaque_index_count = mesh.indices.len() as u32;
    append_non_boundary_quads(mesh, &old_indices[opaque_end..], bounds);
}

fn append_non_boundary_quads(
    mesh: &mut TexturedVisibleChunkMesh,
    indices: &[u32],
    [min_x, max_x, min_z, max_z]: [f32; 4],
) {
    for quad in indices.chunks_exact(6) {
        let positions = quad.iter().filter_map(|index| {
            mesh.vertices
                .get(*index as usize)
                .map(|vertex| vertex.position)
        });
        let mut count = 0;
        let mut on_min_x = true;
        let mut on_max_x = true;
        let mut on_min_z = true;
        let mut on_max_z = true;
        for position in positions {
            count += 1;
            on_min_x &= position[0] == min_x;
            on_max_x &= position[0] == max_x;
            on_min_z &= position[2] == min_z;
            on_max_z &= position[2] == max_z;
        }
        if count == 6 && (on_min_x || on_max_x || on_min_z || on_max_z) {
            continue;
        }
        mesh.indices.extend_from_slice(quad);
    }
}

#[cfg(test)]
mod tests {
    use mclone_worldgen::levelgen::{
        McloneTreeArchetype, McloneTreeBounds, McloneTreeId, McloneTreeRecord,
    };
    use mclone_worldgen::placement::BlockPos;

    use super::*;
    use crate::CanonicalTerrainDependencyCacheReport;

    fn occurrence(family: McloneTreeFamily) -> McloneTreeOccurrence {
        let bounds = McloneTreeBounds::new(1, 1, 1, 3, 8, 3).unwrap();
        McloneTreeOccurrence {
            record: McloneTreeRecord {
                id: McloneTreeId {
                    planning_cell_x: 0,
                    planning_cell_z: 0,
                    candidate_slot: 1,
                    vegetation_revision: 2,
                },
                canonical_base: BlockPos::new(2, 1, 2),
                family,
                archetype: match family {
                    McloneTreeFamily::TemperateBroadleaf => McloneTreeArchetype::RoundedBroadleaf,
                    McloneTreeFamily::CoolWetConifer => McloneTreeArchetype::LayeredConifer,
                    McloneTreeFamily::WarmDryAcacia => McloneTreeArchetype::ForkedAcacia,
                    McloneTreeFamily::HumidJungleBroadleaf => McloneTreeArchetype::LayeredJungle,
                },
                trunk_height: 6,
                crown_radius: 2,
                crown_depth: 4,
                orientation: 0,
                landmark_rank: 0,
                variant_seed: 1,
                bounds,
            },
            x_lift: 0,
            working_bounds: bounds,
        }
    }

    #[test]
    fn separated_tree_blocks_leave_other_feature_families_in_terrain() {
        let mut raw = vec![AIR; 16 * 16 * 16];
        let oak_log = chunk_block_index(2, 1, 2);
        let oak_leaves = chunk_block_index(2, 6, 2);
        let spruce_log = chunk_block_index(3, 1, 3);
        raw[oak_log] = OAK_LOG;
        raw[oak_leaves] = OAK_LEAVES;
        raw[spruce_log] = SPRUCE_LOG;
        let chunk = CanonicalTerrainChunk {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed: 7,
            stage: CanonicalTerrainStage::FinalFeatures,
            chunk_x: 0,
            chunk_z: 0,
            min_y: 0,
            height: 16,
            blocks: raw,
            biomes: Vec::new(),
            fingerprint: 1,
            dependency_cache: CanonicalTerrainDependencyCacheReport::default(),
        };
        let chunks = BTreeMap::from([((0, 0), chunk)]);
        let mut presented = BTreeMap::from([(
            (0, 0),
            chunks[&(0, 0)]
                .blocks
                .iter()
                .map(|block| BlockStateId(u32::from(*block)))
                .collect::<Vec<_>>(),
        )]);
        let occurrence = occurrence(McloneTreeFamily::TemperateBroadleaf);
        let separated = separate_natural_tree_blocks(&chunks, &mut presented, &[occurrence]);
        let tree = &separated[&McloneTreeOccurrenceId::from(occurrence)];

        assert_eq!(presented[&(0, 0)][oak_log], BlockStateId(u32::from(AIR)));
        assert_eq!(presented[&(0, 0)][oak_leaves], BlockStateId(u32::from(AIR)));
        assert_eq!(
            presented[&(0, 0)][spruce_log],
            BlockStateId(u32::from(SPRUCE_LOG))
        );
        assert_eq!(
            tree.chunks[&(0, 0)][oak_log],
            BlockStateId(u32::from(OAK_LOG))
        );
        assert_eq!(
            tree.chunks[&(0, 0)][oak_leaves],
            BlockStateId(u32::from(OAK_LEAVES))
        );
        assert_eq!(
            tree.chunks[&(0, 0)][spruce_log],
            BlockStateId(u32::from(AIR))
        );
    }

    #[test]
    fn exact_surface_columns_skip_trees_and_classify_water() {
        let mut blocks = vec![AIR; 16 * 16 * 16];
        blocks[chunk_block_index(0, 1, 0)] = mclone_worldgen::block::STONE;
        blocks[chunk_block_index(0, 2, 0)] = WATER;
        blocks[chunk_block_index(0, 3, 0)] = OAK_LOG;
        let chunk = CanonicalTerrainChunk {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed: 7,
            stage: CanonicalTerrainStage::FinalFeatures,
            chunk_x: 0,
            chunk_z: 0,
            min_y: 0,
            height: 16,
            blocks,
            biomes: Vec::new(),
            fingerprint: 1,
            dependency_cache: CanonicalTerrainDependencyCacheReport::default(),
        };
        let columns = canonical_exact_surface_columns_with_solid(
            &chunk,
            &TexturedMeshCatalog::default(),
            |state| state == BlockStateId(u32::from(mclone_worldgen::block::STONE)),
        );
        assert_eq!(columns.len(), 16 * 16);
        assert_eq!(
            columns[0],
            CanonicalExactSurfaceColumn {
                solid_top_y: 2,
                side_material: None,
                water: true,
            }
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn timing_now() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn timing_now() -> std::time::Instant {
    std::time::Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn timing_elapsed_ms(started: f64) -> f64 {
    js_sys::Date::now() - started
}

#[cfg(not(target_arch = "wasm32"))]
fn timing_elapsed_ms(started: std::time::Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}
