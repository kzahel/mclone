use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_core::BlockStateId;
use mclone_mesh::{
    TexturedChunkMeshInput, TexturedMeshCatalog, TexturedRenderSectionMesh,
    TexturedVisibleChunkMesh, build_textured_render_sections_for_chunk_set,
    pack_textured_render_sections,
};
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

use super::{
    CanonicalTerrainChunk, CanonicalTerrainCompiler, CanonicalTerrainStage,
    CanonicalTerrainVisibility, canonical_terrain_presentation_blocks,
};

const CANONICAL_WORKER_RAW_CACHE_MAX_CHUNKS: usize = 1_024;

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
pub struct CanonicalPackedAdmission {
    pub requested: CanonicalMeshRequestReceipt,
    pub packed_sections: Vec<u8>,
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

pub struct CanonicalMeshSession {
    compiler: CanonicalTerrainCompiler,
    catalog: TexturedMeshCatalog,
    chunks: BTreeMap<(i32, i32), CanonicalTerrainChunk>,
    raw_lru: VecDeque<(i32, i32)>,
    desired: BTreeSet<(i32, i32)>,
    visibility: CanonicalTerrainVisibility,
    frontier: CanonicalMeshFrontier,
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
            chunks: BTreeMap::new(),
            raw_lru: VecDeque::new(),
            desired: BTreeSet::new(),
            visibility: CanonicalTerrainVisibility::default(),
            frontier: CanonicalMeshFrontier::default(),
        }
    }

    pub fn set_frontier(&mut self, frontier: CanonicalMeshFrontier) {
        self.frontier = frontier;
    }

    pub const fn frontier(&self) -> CanonicalMeshFrontier {
        self.frontier
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

        let presentation_started = timing_now();
        let presented = input_positions
            .iter()
            .filter_map(|position| {
                let chunk = self.chunks.get(position)?;
                let blocks = canonical_terrain_presentation_blocks(&chunk.blocks, self.visibility)
                    .into_iter()
                    .map(|block| BlockStateId(u32::from(block)))
                    .collect::<Vec<_>>();
                Some((*position, blocks))
            })
            .collect::<Vec<_>>();
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

        let pack_started = timing_now();
        let mut admissions = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let sections = admission_sections
                .remove(&receipt.coordinate.tuple())
                .unwrap_or_default();
            admissions.push(CanonicalPackedAdmission {
                requested: receipt,
                packed_sections: pack_textured_render_sections(&sections),
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
