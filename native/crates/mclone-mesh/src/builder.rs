use std::collections::BTreeSet;

use crate::ambient_occlusion::{
    AmbientOcclusionFace, AmbientOcclusionSampler, AmbientOcclusionShape, BlockPos,
    calculate_ambient_occlusion_face, calculate_ambient_occlusion_shape,
};
use crate::catalog::{
    LeafDetail, TexturedBlockFace, TexturedFluidKind, TexturedFluidModel, TexturedLeafCardModel,
    TexturedMeshCatalog, TexturedMeshError, TexturedTerrainRenderLayer,
};
use crate::data::{
    ChunkVertex, GrassPatch, RenderSectionKey, TexturedChunkVertex,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    VisibilityGraphBuildStats, VisibleChunkMesh,
};
use crate::tint::{blended_liquid_color, block_tint};
use crate::visibility::{VisGraph, VisibilityGraphTimer, VisibilitySet};
use crate::{AIR_BLOCK_ID, CAVE_AIR_BLOCK_ID, CAVE_AIR_BLOCK_STATE_ID};
use mclone_assets::ModelFaceDirection;
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockPos as CoreBlockPos, BlockStateId, CHUNK_WIDTH, DEFAULT_BIOME_ID,
    HorizontalTopology, PackedLightSection, SECTION_HEIGHT as RENDER_SECTION_HEIGHT,
    block_to_chunk_coord, block_to_section_coord, chunk_block_index, chunk_min_block_coord,
    local_block_coord, obfuscate_biome_zoom_seed,
};
use mclone_light::{
    FULL_BRIGHT, pack_light, packed_block_light, packed_light_at_local_block_or_fullbright,
    packed_sky_light,
};

const LCG_MULTIPLIER: i64 = 6364136223846793005;
const LCG_INCREMENT: i64 = 1442695040888963407;
pub const BUSHY_LEAF_CARD_OVERHANG: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedRenderSectionBuildOptions {
    pub grass_patches: bool,
    pub topology: HorizontalTopology,
}

impl TexturedRenderSectionBuildOptions {
    pub const OFF: Self = Self {
        grass_patches: false,
        topology: HorizontalTopology::UNBOUNDED,
    };

    pub const fn with_grass_patches(mut self, grass_patches: bool) -> Self {
        self.grass_patches = grass_patches;
        self
    }

    pub const fn with_topology(mut self, topology: HorizontalTopology) -> Self {
        self.topology = topology;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChunkMeshInput<'a> {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: &'a [u8],
}

impl<'a> ChunkMeshInput<'a> {
    pub fn new(chunk_x: i32, chunk_z: i32, min_y: i32, height: i32, blocks: &'a [u8]) -> Self {
        if height <= 0 || height % RENDER_SECTION_HEIGHT != 0 {
            panic!("chunk height {height} must be a positive multiple of {RENDER_SECTION_HEIGHT}");
        }
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        if blocks.len() != expected_len {
            panic!(
                "chunk mesh input has {} blocks; expected {expected_len}",
                blocks.len()
            );
        }
        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
        }
    }

    fn block_at_or_air(&self, local_x: i32, local_y: i32, local_z: i32) -> u8 {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(0..self.height).contains(&local_y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return AIR_BLOCK_ID;
        }
        self.blocks[chunk_block_index(local_x, local_y, local_z)]
    }
}

pub fn build_visible_chunk_mesh(input: ChunkMeshInput<'_>) -> VisibleChunkMesh {
    build_visible_chunk_area_mesh(&[input])
}

pub fn build_visible_chunk_area_mesh(inputs: &[ChunkMeshInput<'_>]) -> VisibleChunkMesh {
    let mut mesh = VisibleChunkMesh::default();
    for input in inputs {
        add_chunk_to_mesh(&mut mesh, *input, inputs);
    }
    mesh
}

#[derive(Clone, Copy, Debug)]
pub struct TexturedChunkMeshInput<'a> {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: &'a [BlockStateId],
    pub biomes: &'a [i32],
    pub light_sections: &'a [PackedLightSection],
    pub biome_zoom_seed: Option<i64>,
    pub fluids_visible: bool,
}

impl<'a> TexturedChunkMeshInput<'a> {
    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        blocks: &'a [BlockStateId],
    ) -> Self {
        if height <= 0 || height % RENDER_SECTION_HEIGHT != 0 {
            panic!("chunk height {height} must be a positive multiple of {RENDER_SECTION_HEIGHT}");
        }
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        if blocks.len() != expected_len {
            panic!(
                "textured chunk mesh input has {} blocks; expected {expected_len}",
                blocks.len()
            );
        }
        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            biomes: &[],
            light_sections: &[],
            biome_zoom_seed: None,
            fluids_visible: true,
        }
    }

    pub fn with_biomes(mut self, biomes: &'a [i32]) -> Self {
        self.biomes = biomes;
        self
    }

    pub fn with_world_seed(mut self, seed: i64) -> Self {
        self.biome_zoom_seed = Some(obfuscate_biome_zoom_seed(seed));
        self
    }

    pub fn with_biome_zoom_seed(mut self, biome_zoom_seed: i64) -> Self {
        self.biome_zoom_seed = Some(biome_zoom_seed);
        self
    }

    pub fn with_light_sections(mut self, light_sections: &'a [PackedLightSection]) -> Self {
        self.light_sections = light_sections;
        self
    }

    /// Include or suppress liquid geometry without changing the canonical
    /// block-state input. Diagnostic and preview clients use this to inspect
    /// the same generated chunk with water presentation hidden.
    pub fn with_fluids_visible(mut self, fluids_visible: bool) -> Self {
        self.fluids_visible = fluids_visible;
        self
    }

    fn block_at_or_air(&self, local_x: i32, local_y: i32, local_z: i32) -> BlockStateId {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(0..self.height).contains(&local_y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return AIR_BLOCK_STATE_ID;
        }
        self.blocks[chunk_block_index(local_x, local_y, local_z)]
    }

    fn packed_light_at_or_fullbright(&self, local_x: i32, y: i32, local_z: i32) -> u32 {
        packed_light_at_local_block_or_fullbright(
            self.light_sections,
            self.min_y,
            self.height,
            local_x,
            y,
            local_z,
        )
    }

    fn biome_id_at_or_default(&self, local_x: i32, world_y: i32, local_z: i32) -> i32 {
        let local_quart_x = local_x.div_euclid(4).clamp(0, 3);
        let local_quart_z = local_z.div_euclid(4).clamp(0, 3);
        let quart_y = world_y.div_euclid(4);
        self.biome_id_at_quart_or_default(local_quart_x, quart_y, local_quart_z)
    }

    fn biome_id_at_quart_or_default(
        &self,
        local_quart_x: i32,
        quart_y: i32,
        local_quart_z: i32,
    ) -> i32 {
        if self.biomes.is_empty() {
            return DEFAULT_BIOME_ID;
        }
        let quart_height = self.height / 4;
        let expected_len = (quart_height * 4 * 4) as usize;
        if quart_height <= 0 || self.biomes.len() != expected_len {
            return DEFAULT_BIOME_ID;
        }

        let local_quart_x = local_quart_x.clamp(0, 3);
        let local_quart_z = local_quart_z.clamp(0, 3);
        let local_quart_y = (quart_y - self.min_y.div_euclid(4)).clamp(0, quart_height - 1);
        let index = ((local_quart_y << 4) | (local_quart_z << 2) | local_quart_x) as usize;
        self.biomes.get(index).copied().unwrap_or(DEFAULT_BIOME_ID)
    }
}

pub fn build_textured_visible_chunk_mesh(
    input: TexturedChunkMeshInput<'_>,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedVisibleChunkMesh, TexturedMeshError> {
    build_textured_visible_chunk_area_mesh(&[input], catalog)
}

pub fn build_textured_visible_chunk_area_mesh(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedVisibleChunkMesh, TexturedMeshError> {
    let mut mesh = TexturedVisibleChunkMesh::default();
    for section in build_textured_render_sections(inputs, catalog)? {
        append_textured_mesh(&mut mesh, &section.mesh);
    }
    Ok(mesh)
}

pub fn build_textured_render_sections(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
) -> Result<Vec<TexturedRenderSectionMesh>, TexturedMeshError> {
    Ok(build_textured_render_sections_with_stats(inputs, catalog)?.sections)
}

pub fn build_textured_render_sections_for_chunk_set(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_chunks: &BTreeSet<(i32, i32)>,
) -> Result<Vec<TexturedRenderSectionMesh>, TexturedMeshError> {
    Ok(
        build_textured_render_sections_for_chunk_set_with_stats(inputs, catalog, target_chunks)?
            .sections,
    )
}

pub fn build_textured_render_sections_with_stats(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    build_textured_render_sections_for_chunks(
        inputs,
        catalog,
        None,
        None,
        TexturedRenderSectionBuildOptions::OFF,
    )
}

pub fn build_textured_render_sections_for_chunk_set_with_stats(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_chunks: &BTreeSet<(i32, i32)>,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    build_textured_render_sections_for_chunks(
        inputs,
        catalog,
        Some(target_chunks),
        None,
        TexturedRenderSectionBuildOptions::OFF,
    )
}

pub fn build_textured_render_sections_for_section_set_with_stats(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    build_textured_render_sections_for_section_set_with_stats_and_options(
        inputs,
        catalog,
        target_sections,
        TexturedRenderSectionBuildOptions::OFF,
    )
}

pub fn build_textured_render_sections_for_section_set_with_stats_and_options(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
    options: TexturedRenderSectionBuildOptions,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    build_textured_render_sections_for_chunks(inputs, catalog, None, Some(target_sections), options)
}

fn build_textured_render_sections_for_chunks(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_chunks: Option<&BTreeSet<(i32, i32)>>,
    target_sections: Option<&BTreeSet<RenderSectionKey>>,
    options: TexturedRenderSectionBuildOptions,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    let mut sections = Vec::new();
    let mut visibility_graph = VisibilityGraphBuildStats::default();
    for input in inputs {
        if target_chunks.is_some_and(|targets| !targets.contains(&(input.chunk_x, input.chunk_z))) {
            continue;
        }
        for local_y_start in (0..input.height).step_by(RENDER_SECTION_HEIGHT as usize) {
            let local_y_end = (local_y_start + RENDER_SECTION_HEIGHT).min(input.height);
            let key = RenderSectionKey::new(
                input.chunk_x,
                block_to_section_coord(input.min_y + local_y_start),
                input.chunk_z,
            );
            if target_sections.is_some_and(|targets| !targets.contains(&key)) {
                continue;
            }
            if textured_section_is_air_like(*input, local_y_start, local_y_end) {
                visibility_graph.record_ms(0.0);
                sections.push(TexturedRenderSectionMesh {
                    key,
                    mesh: TexturedVisibleChunkMesh::default(),
                    grass_patches: Vec::new(),
                    visibility: VisibilitySet::all_visible(),
                });
                continue;
            }
            let mut mesh = TexturedVisibleChunkMesh::default();
            let visibility_start = VisibilityGraphTimer::start();
            let visibility =
                build_textured_section_visibility(*input, catalog, local_y_start, local_y_end);
            visibility_graph.record_ms(visibility_start.elapsed_ms());
            add_textured_chunk_range_to_mesh(
                &mut mesh,
                *input,
                inputs,
                catalog,
                local_y_start,
                local_y_end,
            )?;
            let grass_patches = if options.grass_patches {
                discover_grass_patches(
                    *input,
                    inputs,
                    catalog,
                    local_y_start,
                    local_y_end,
                    options.topology,
                )
            } else {
                Vec::new()
            };
            sections.push(TexturedRenderSectionMesh {
                key,
                mesh,
                grass_patches,
                visibility,
            });
        }
    }
    Ok(TexturedRenderSectionBuildReport {
        sections,
        visibility_graph,
    })
}

fn discover_grass_patches(
    input: TexturedChunkMeshInput<'_>,
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    local_y_start: i32,
    local_y_end: i32,
    topology: HorizontalTopology,
) -> Vec<GrassPatch> {
    let world_origin_x = chunk_min_block_coord(input.chunk_x);
    let world_origin_z = chunk_min_block_coord(input.chunk_z);
    let mut patches = Vec::new();

    for local_y in local_y_start..local_y_end {
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let state_id = input.block_at_or_air(local_x, local_y, local_z);
                if !catalog.is_grass_patch_surface(state_id) {
                    continue;
                }
                let world_x = world_origin_x + local_x;
                let world_y = input.min_y + local_y;
                let world_z = world_origin_z + local_z;
                let above = block_state_at_world_or_air(area, world_x, world_y + 1, world_z);
                if catalog.occludes(above) {
                    continue;
                }
                let canonical = topology
                    .canonicalize_block(CoreBlockPos::new(world_x, world_y, world_z))
                    .unwrap_or(CoreBlockPos::new(world_x, world_y, world_z));
                let tint = block_tint(
                    catalog,
                    crate::catalog::TexturedBlockTint::Grass,
                    world_x,
                    world_y,
                    world_z,
                    |x, y, z| biome_id_at_world_or_default(area, x, y, z),
                );
                patches.push(GrassPatch {
                    root: [world_x, world_y + 1, world_z],
                    packed_tint: pack_rgb8(tint),
                    packed_light: packed_light_at_world_or_fullbright(
                        area,
                        world_x,
                        world_y + 1,
                        world_z,
                    ),
                    seed: fold_position_hash(stable_position_hash(
                        canonical.x,
                        canonical.y,
                        canonical.z,
                    )),
                    flags: 0,
                    reserved: 0,
                });
            }
        }
    }

    patches
}

fn pack_rgb8(color: [f32; 3]) -> u32 {
    color
        .into_iter()
        .enumerate()
        .fold(0, |packed, (shift, value)| {
            let channel = (value.clamp(0.0, 1.0) * 255.0).round() as u32;
            packed | channel << (shift * 8)
        })
}

fn fold_position_hash(hash: u64) -> u32 {
    (hash as u32) ^ (hash >> 32) as u32
}

fn build_textured_section_visibility(
    input: TexturedChunkMeshInput<'_>,
    catalog: &TexturedMeshCatalog,
    local_y_start: i32,
    local_y_end: i32,
) -> VisibilitySet {
    let mut graph = VisGraph::new();
    for local_y in local_y_start..local_y_end {
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let state_id = input.block_at_or_air(local_x, local_y, local_z);
                if catalog.occludes(state_id) {
                    graph.set_opaque_local(
                        local_x as usize,
                        (local_y - local_y_start) as usize,
                        local_z as usize,
                    );
                }
            }
        }
    }
    graph.resolve()
}

fn add_chunk_to_mesh(
    mesh: &mut VisibleChunkMesh,
    input: ChunkMeshInput<'_>,
    area: &[ChunkMeshInput<'_>],
) {
    let world_origin_x = chunk_min_block_coord(input.chunk_x);
    let world_origin_z = chunk_min_block_coord(input.chunk_z);

    for local_y in 0..input.height {
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let block_id = input.block_at_or_air(local_x, local_y, local_z);
                if is_air_like_block_id(block_id) {
                    continue;
                }

                let world_x = world_origin_x + local_x;
                let world_y = input.min_y + local_y;
                let world_z = world_origin_z + local_z;
                for face in FACES {
                    let neighbor = block_at_world_or_air(
                        area,
                        world_x + face.neighbor[0],
                        world_y + face.neighbor[1],
                        world_z + face.neighbor[2],
                    );
                    if is_air_like_block_id(neighbor) {
                        add_face(mesh, world_x, world_y, world_z, block_id, face);
                    }
                }
            }
        }
    }
}

fn add_textured_chunk_range_to_mesh(
    mesh: &mut TexturedVisibleChunkMesh,
    input: TexturedChunkMeshInput<'_>,
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    local_y_start: i32,
    local_y_end: i32,
) -> Result<(), TexturedMeshError> {
    let world_origin_x = chunk_min_block_coord(input.chunk_x);
    let world_origin_z = chunk_min_block_coord(input.chunk_z);
    let ao_sampler = TexturedAmbientOcclusionSampler { area, catalog };
    let mut cutout_mesh = TexturedVisibleChunkMesh::default();
    let mut translucent_mesh = TexturedVisibleChunkMesh::default();

    for local_y in local_y_start..local_y_end {
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let state_id = input.block_at_or_air(local_x, local_y, local_z);
                if is_air_like_block_state(state_id) {
                    continue;
                }

                let Some(block_model) = catalog.get(state_id) else {
                    return Err(TexturedMeshError::MissingBlockModel(state_id));
                };
                let world_x = world_origin_x + local_x;
                let world_y = input.min_y + local_y;
                let world_z = world_origin_z + local_z;
                if input.fluids_visible
                    && let Some(fluid) = block_model.fluid
                {
                    add_textured_liquid_block_to_mesh(
                        &mut translucent_mesh,
                        area,
                        catalog,
                        world_x,
                        world_y,
                        world_z,
                        fluid,
                    );
                    // Waterlogged blocks (seagrass, kelp, coral, sea pickle, ...)
                    // still render their own model on top of the liquid, matching
                    // vanilla where LiquidBlockRenderer and the block renderer are
                    // separate passes. Pure water/lava have no faces and fall through
                    // to the empty-faces `continue` below.
                }
                if block_model.faces.is_empty() {
                    continue;
                }

                for face in &block_model.faces {
                    if let Some(cullface) = face.cullface {
                        let offset = direction_offset(cullface);
                        let neighbor = block_state_at_world_or_air(
                            area,
                            world_x + offset[0],
                            world_y + offset[1],
                            world_z + offset[2],
                        );
                        if catalog.occludes(neighbor) {
                            continue;
                        }
                    }
                    let corners = textured_face_corners(face);
                    let ao_shape = calculate_ambient_occlusion_shape(
                        corners,
                        face.direction,
                        block_model.collision_shape_full_block,
                    );
                    let lighting =
                        if block_model.ambient_occlusion && block_model.light_emission == 0 {
                            calculate_ambient_occlusion_face(
                                &ao_sampler,
                                BlockPos::new(world_x, world_y, world_z),
                                face.direction,
                                ao_shape,
                                face.shade,
                            )
                        } else {
                            let packed_light = flat_packed_light_for_face(
                                area, world_x, world_y, world_z, face, ao_shape,
                            );
                            AmbientOcclusionFace::flat(face.direction, face.shade, packed_light)
                        };
                    match block_model.render_layer {
                        TexturedTerrainRenderLayer::Solid => {
                            add_textured_face(
                                mesh, area, catalog, world_x, world_y, world_z, face, corners,
                                lighting,
                            );
                        }
                        TexturedTerrainRenderLayer::Cutout => {
                            add_textured_face(
                                &mut cutout_mesh,
                                area,
                                catalog,
                                world_x,
                                world_y,
                                world_z,
                                face,
                                corners,
                                lighting,
                            );
                        }
                        TexturedTerrainRenderLayer::Translucent => {
                            add_textured_face(
                                &mut translucent_mesh,
                                area,
                                catalog,
                                world_x,
                                world_y,
                                world_z,
                                face,
                                corners,
                                lighting,
                            );
                        }
                    }
                }
                if catalog.leaf_detail() == LeafDetail::Bushy
                    && let Some(cards) = block_model.leaf_cards
                    && !leaf_is_fully_enclosed(area, catalog, world_x, world_y, world_z)
                {
                    add_bushy_leaf_cards(
                        &mut cutout_mesh,
                        area,
                        catalog,
                        world_x,
                        world_y,
                        world_z,
                        cards,
                    );
                }
            }
        }
    }

    mesh.mark_all_indices_solid();
    cutout_mesh.mark_all_indices_cutout();
    translucent_mesh.opaque_index_count = 0;
    translucent_mesh.solid_index_count = 0;
    append_textured_mesh(mesh, &cutout_mesh);
    append_textured_mesh(mesh, &translucent_mesh);

    Ok(())
}

fn append_textured_mesh(mesh: &mut TexturedVisibleChunkMesh, source: &TexturedVisibleChunkMesh) {
    let base_index = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&source.vertices);
    let target_solid_end = mesh.solid_index_range().end as usize;
    let target_opaque_end = mesh.opaque_index_range().end as usize;
    let source_solid_end = source.solid_index_range().end as usize;
    let source_opaque_end = source.opaque_index_range().end as usize;

    let target_indices = std::mem::take(&mut mesh.indices);
    let adjusted_source = source
        .indices
        .iter()
        .map(|index| base_index + *index)
        .collect::<Vec<_>>();
    mesh.indices
        .reserve(target_indices.len() + adjusted_source.len());
    mesh.indices
        .extend_from_slice(&target_indices[..target_solid_end]);
    mesh.indices
        .extend_from_slice(&adjusted_source[..source_solid_end]);
    mesh.solid_index_count = mesh.indices.len() as u32;
    mesh.indices
        .extend_from_slice(&target_indices[target_solid_end..target_opaque_end]);
    mesh.indices
        .extend_from_slice(&adjusted_source[source_solid_end..source_opaque_end]);
    mesh.opaque_index_count = mesh.indices.len() as u32;
    mesh.indices
        .extend_from_slice(&target_indices[target_opaque_end..]);
    mesh.indices
        .extend_from_slice(&adjusted_source[source_opaque_end..]);
}

#[derive(Clone, Copy, Debug)]
struct Face {
    neighbor: [i32; 3],
    corners: [[f32; 3]; 4],
    shade: f32,
}

const FACES: [Face; 6] = [
    Face {
        neighbor: [0, 1, 0],
        corners: [
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        shade: 1.0,
    },
    Face {
        neighbor: [0, -1, 0],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        shade: 0.5,
    },
    Face {
        neighbor: [1, 0, 0],
        corners: [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ],
        shade: 0.86,
    },
    Face {
        neighbor: [-1, 0, 0],
        corners: [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        shade: 0.86,
    },
    Face {
        neighbor: [0, 0, 1],
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        shade: 0.78,
    },
    Face {
        neighbor: [0, 0, -1],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
        shade: 0.72,
    },
];

fn add_face(
    mesh: &mut VisibleChunkMesh,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block_id: u8,
    face: Face,
) {
    let base_index = mesh.vertices.len() as u32;
    let color = shaded_color(block_id, face.shade);
    for corner in face.corners {
        mesh.vertices.push(ChunkVertex {
            position: [
                world_x as f32 + corner[0],
                world_y as f32 + corner[1],
                world_z as f32 + corner[2],
            ],
            color,
        });
    }
    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn add_textured_face(
    mesh: &mut TexturedVisibleChunkMesh,
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    face: &TexturedBlockFace,
    corners: [[f32; 3]; 4],
    lighting: AmbientOcclusionFace,
) {
    let base_index = mesh.vertices.len() as u32;
    let uvs = textured_face_uvs(face);
    let tint = block_tint(catalog, face.tint, world_x, world_y, world_z, |x, y, z| {
        biome_id_at_world_or_default(area, x, y, z)
    });
    for index in 0..4 {
        let corner = corners[index];
        let color = [
            tint[0] * lighting.brightness[index],
            tint[1] * lighting.brightness[index],
            tint[2] * lighting.brightness[index],
            1.0,
        ];
        mesh.vertices.push(TexturedChunkVertex {
            position: [
                world_x as f32 + corner[0],
                world_y as f32 + corner[1],
                world_z as f32 + corner[2],
            ],
            uv: uvs[index],
            color,
            packed_light: lighting.lightmap[index],
        });
    }
    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn leaf_is_fully_enclosed(
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> bool {
    [
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
        [-1, 0, 0],
        [1, 0, 0],
    ]
    .into_iter()
    .all(|offset| {
        catalog.is_leaf(block_state_at_world_or_air(
            area,
            world_x + offset[0],
            world_y + offset[1],
            world_z + offset[2],
        ))
    })
}

fn add_bushy_leaf_cards(
    mesh: &mut TexturedVisibleChunkMesh,
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    cards: TexturedLeafCardModel,
) {
    let layout = bushy_leaf_layout(world_x, world_y, world_z);
    let tint = block_tint(catalog, cards.tint, world_x, world_y, world_z, |x, y, z| {
        biome_id_at_world_or_default(area, x, y, z)
    });
    let color = [tint[0], tint[1], tint[2], 1.0];
    let packed_light = liquid_packed_light(area, world_x, world_y, world_z);
    let uvs = [
        cards.sprite.map(0.0, 16.0),
        cards.sprite.map(0.0, 0.0),
        cards.sprite.map(16.0, 0.0),
        cards.sprite.map(16.0, 16.0),
    ];

    for angle in [layout.angle, layout.angle + std::f32::consts::FRAC_PI_2] {
        let direction = [angle.cos(), angle.sin()];
        let corners = [
            [
                0.5 - direction[0] * layout.radius,
                layout.bottom,
                0.5 - direction[1] * layout.radius,
            ],
            [
                0.5 - direction[0] * layout.radius,
                layout.top,
                0.5 - direction[1] * layout.radius,
            ],
            [
                0.5 + direction[0] * layout.radius,
                layout.top,
                0.5 + direction[1] * layout.radius,
            ],
            [
                0.5 + direction[0] * layout.radius,
                layout.bottom,
                0.5 + direction[1] * layout.radius,
            ],
        ];
        add_textured_leaf_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            corners,
            uvs,
            color,
            packed_light,
            false,
        );
        add_textured_leaf_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            corners,
            uvs,
            color,
            packed_light,
            true,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BushyLeafLayout {
    angle: f32,
    radius: f32,
    bottom: f32,
    top: f32,
}

fn bushy_leaf_layout(world_x: i32, world_y: i32, world_z: i32) -> BushyLeafLayout {
    let hash = stable_position_hash(world_x, world_y, world_z);
    match hash & 3 {
        0 => BushyLeafLayout {
            angle: std::f32::consts::PI / 12.0,
            radius: 0.70,
            bottom: -0.12,
            top: 1.15,
        },
        1 => BushyLeafLayout {
            angle: std::f32::consts::PI * 5.0 / 24.0,
            radius: 0.74,
            bottom: -0.15,
            top: 1.12,
        },
        2 => BushyLeafLayout {
            angle: std::f32::consts::PI / 3.0,
            radius: 0.68,
            bottom: -0.10,
            top: 1.16,
        },
        _ => BushyLeafLayout {
            angle: std::f32::consts::PI * 11.0 / 24.0,
            radius: 0.72,
            bottom: -0.14,
            top: 1.10,
        },
    }
}

fn stable_position_hash(world_x: i32, world_y: i32, world_z: i32) -> u64 {
    let mut value = (world_x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (world_y as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
        ^ (world_z as i64 as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[allow(clippy::too_many_arguments)]
fn add_textured_leaf_quad(
    mesh: &mut TexturedVisibleChunkMesh,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    corners: [[f32; 3]; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
    packed_light: u32,
    reversed: bool,
) {
    let base_index = mesh.vertices.len() as u32;
    for index in 0..4 {
        let corner = corners[index];
        mesh.vertices.push(TexturedChunkVertex {
            position: [
                world_x as f32 + corner[0],
                world_y as f32 + corner[1],
                world_z as f32 + corner[2],
            ],
            uv: uvs[index],
            color,
            packed_light,
        });
    }
    if reversed {
        mesh.indices.extend_from_slice(&[
            base_index,
            base_index + 3,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 1,
        ]);
    } else {
        mesh.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }
}

const MAX_FLUID_HEIGHT: f32 = 8.0 / 9.0;
const LIQUID_EPSILON: f32 = 0.001;

fn add_textured_liquid_block_to_mesh(
    mesh: &mut TexturedVisibleChunkMesh,
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    fluid: TexturedFluidModel,
) {
    let mut h00 = fluid_height_at(area, catalog, world_x, world_y, world_z, fluid.kind);
    let mut h01 = fluid_height_at(area, catalog, world_x, world_y, world_z + 1, fluid.kind);
    let mut h11 = fluid_height_at(area, catalog, world_x + 1, world_y, world_z + 1, fluid.kind);
    let mut h10 = fluid_height_at(area, catalog, world_x + 1, world_y, world_z, fluid.kind);
    let render_top = !same_fluid_at(area, catalog, world_x, world_y + 1, world_z, fluid.kind)
        && !liquid_face_occluded_by_neighbor(
            area,
            catalog,
            world_x,
            world_y + 1,
            world_z,
            ModelFaceDirection::Up,
            h00.min(h01).min(h11).min(h10),
        );
    let render_bottom = !same_fluid_at(area, catalog, world_x, world_y - 1, world_z, fluid.kind)
        && !liquid_face_occluded_by_neighbor(
            area,
            catalog,
            world_x,
            world_y - 1,
            world_z,
            ModelFaceDirection::Down,
            MAX_FLUID_HEIGHT,
        );
    let render_north = !same_fluid_at(area, catalog, world_x, world_y, world_z - 1, fluid.kind)
        && !liquid_face_occluded_by_neighbor(
            area,
            catalog,
            world_x,
            world_y,
            world_z - 1,
            ModelFaceDirection::North,
            h00.max(h10),
        );
    let render_south = !same_fluid_at(area, catalog, world_x, world_y, world_z + 1, fluid.kind)
        && !liquid_face_occluded_by_neighbor(
            area,
            catalog,
            world_x,
            world_y,
            world_z + 1,
            ModelFaceDirection::South,
            h01.max(h11),
        );
    let render_west = !same_fluid_at(area, catalog, world_x - 1, world_y, world_z, fluid.kind)
        && !liquid_face_occluded_by_neighbor(
            area,
            catalog,
            world_x - 1,
            world_y,
            world_z,
            ModelFaceDirection::West,
            h00.max(h01),
        );
    let render_east = !same_fluid_at(area, catalog, world_x + 1, world_y, world_z, fluid.kind)
        && !liquid_face_occluded_by_neighbor(
            area,
            catalog,
            world_x + 1,
            world_y,
            world_z,
            ModelFaceDirection::East,
            h10.max(h11),
        );

    if !render_top
        && !render_bottom
        && !render_north
        && !render_south
        && !render_west
        && !render_east
    {
        return;
    }

    let bottom_y = if render_bottom { LIQUID_EPSILON } else { 0.0 };

    if render_top {
        h00 = (h00 - LIQUID_EPSILON).max(0.0);
        h01 = (h01 - LIQUID_EPSILON).max(0.0);
        h11 = (h11 - LIQUID_EPSILON).max(0.0);
        h10 = (h10 - LIQUID_EPSILON).max(0.0);
        let top_corners = [
            [0.0, h00, 0.0],
            [0.0, h01, 1.0],
            [1.0, h11, 1.0],
            [1.0, h10, 0.0],
        ];
        let top_uvs = [
            fluid.still.map(0.0, 0.0),
            fluid.still.map(0.0, 16.0),
            fluid.still.map(16.0, 16.0),
            fluid.still.map(16.0, 0.0),
        ];
        let top_color = liquid_color_at(area, fluid.kind, world_x, world_y, world_z, 1.0);
        let top_light = liquid_packed_light(area, world_x, world_y, world_z);
        add_textured_liquid_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            top_corners,
            top_uvs,
            top_color,
            top_light,
        );
        if should_render_backward_up_face(area, catalog, world_x, world_y + 1, world_z, fluid.kind)
        {
            add_textured_liquid_quad_reversed(
                mesh,
                world_x,
                world_y,
                world_z,
                top_corners,
                top_uvs,
                top_color,
                top_light,
            );
        }
    }

    if render_bottom {
        add_textured_liquid_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            [
                [0.0, bottom_y, 1.0],
                [0.0, bottom_y, 0.0],
                [1.0, bottom_y, 0.0],
                [1.0, bottom_y, 1.0],
            ],
            [
                fluid.still.map(0.0, 16.0),
                fluid.still.map(0.0, 0.0),
                fluid.still.map(16.0, 0.0),
                fluid.still.map(16.0, 16.0),
            ],
            liquid_color_at(area, fluid.kind, world_x, world_y, world_z, 0.5),
            liquid_packed_light(area, world_x, world_y - 1, world_z),
        );
    }

    let side_light = liquid_packed_light(area, world_x, world_y, world_z);
    if render_north {
        add_textured_liquid_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            [
                [1.0, h10, LIQUID_EPSILON],
                [1.0, bottom_y, LIQUID_EPSILON],
                [0.0, bottom_y, LIQUID_EPSILON],
                [0.0, h00, LIQUID_EPSILON],
            ],
            [
                fluid.flow.map(8.0, liquid_side_v(h10)),
                fluid.flow.map(8.0, 8.0),
                fluid.flow.map(0.0, 8.0),
                fluid.flow.map(0.0, liquid_side_v(h00)),
            ],
            liquid_color_at(area, fluid.kind, world_x, world_y, world_z, 0.72),
            side_light,
        );
    }
    if render_south {
        add_textured_liquid_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            [
                [0.0, h01, 1.0 - LIQUID_EPSILON],
                [0.0, bottom_y, 1.0 - LIQUID_EPSILON],
                [1.0, bottom_y, 1.0 - LIQUID_EPSILON],
                [1.0, h11, 1.0 - LIQUID_EPSILON],
            ],
            [
                fluid.flow.map(0.0, liquid_side_v(h01)),
                fluid.flow.map(0.0, 8.0),
                fluid.flow.map(8.0, 8.0),
                fluid.flow.map(8.0, liquid_side_v(h11)),
            ],
            liquid_color_at(area, fluid.kind, world_x, world_y, world_z, 0.78),
            side_light,
        );
    }
    if render_west {
        add_textured_liquid_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            [
                [LIQUID_EPSILON, h00, 0.0],
                [LIQUID_EPSILON, bottom_y, 0.0],
                [LIQUID_EPSILON, bottom_y, 1.0],
                [LIQUID_EPSILON, h01, 1.0],
            ],
            [
                fluid.flow.map(8.0, liquid_side_v(h00)),
                fluid.flow.map(8.0, 8.0),
                fluid.flow.map(0.0, 8.0),
                fluid.flow.map(0.0, liquid_side_v(h01)),
            ],
            liquid_color_at(area, fluid.kind, world_x, world_y, world_z, 0.86),
            side_light,
        );
    }
    if render_east {
        add_textured_liquid_quad(
            mesh,
            world_x,
            world_y,
            world_z,
            [
                [1.0 - LIQUID_EPSILON, h11, 1.0],
                [1.0 - LIQUID_EPSILON, bottom_y, 1.0],
                [1.0 - LIQUID_EPSILON, bottom_y, 0.0],
                [1.0 - LIQUID_EPSILON, h10, 0.0],
            ],
            [
                fluid.flow.map(8.0, liquid_side_v(h11)),
                fluid.flow.map(8.0, 8.0),
                fluid.flow.map(0.0, 8.0),
                fluid.flow.map(0.0, liquid_side_v(h10)),
            ],
            liquid_color_at(area, fluid.kind, world_x, world_y, world_z, 0.86),
            side_light,
        );
    }
}

fn add_textured_liquid_quad(
    mesh: &mut TexturedVisibleChunkMesh,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    corners: [[f32; 3]; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
    packed_light: u32,
) {
    add_textured_liquid_quad_with_winding(
        mesh,
        world_x,
        world_y,
        world_z,
        corners,
        uvs,
        color,
        packed_light,
        false,
    );
}

fn add_textured_liquid_quad_reversed(
    mesh: &mut TexturedVisibleChunkMesh,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    corners: [[f32; 3]; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
    packed_light: u32,
) {
    add_textured_liquid_quad_with_winding(
        mesh,
        world_x,
        world_y,
        world_z,
        corners,
        uvs,
        color,
        packed_light,
        true,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_textured_liquid_quad_with_winding(
    mesh: &mut TexturedVisibleChunkMesh,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    corners: [[f32; 3]; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
    packed_light: u32,
    reversed: bool,
) {
    let base_index = mesh.vertices.len() as u32;
    for index in 0..4 {
        let corner = corners[index];
        mesh.vertices.push(TexturedChunkVertex {
            position: [
                world_x as f32 + corner[0],
                world_y as f32 + corner[1],
                world_z as f32 + corner[2],
            ],
            uv: uvs[index],
            color,
            packed_light,
        });
    }
    if reversed {
        mesh.indices.extend_from_slice(&[
            base_index,
            base_index + 3,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 1,
        ]);
    } else {
        mesh.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }
}

fn same_fluid_at(
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    kind: TexturedFluidKind,
) -> bool {
    fluid_at_world(area, catalog, world_x, world_y, world_z).is_some_and(|fluid| fluid.kind == kind)
}

fn fluid_at_world(
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<TexturedFluidModel> {
    let state_id = block_state_at_world_or_air(area, world_x, world_y, world_z);
    catalog.fluid(state_id)
}

fn liquid_face_occluded_by_neighbor(
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    direction: ModelFaceDirection,
    face_height: f32,
) -> bool {
    let state_id = block_state_at_world_or_air(area, world_x, world_y, world_z);
    if !catalog.occludes(state_id) {
        return false;
    }
    // Java LiquidBlockRenderer occludes against the liquid box height, not the
    // whole block cell. A full block above only hides a liquid top at height 1.0.
    match direction {
        ModelFaceDirection::Up => face_height >= 1.0,
        _ => face_height > 0.0,
    }
}

fn should_render_backward_up_face(
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    kind: TexturedFluidKind,
) -> bool {
    for dz in -1..=1 {
        for dx in -1..=1 {
            let sample_x = world_x + dx;
            let sample_z = world_z + dz;
            let state_id = block_state_at_world_or_air(area, sample_x, world_y, sample_z);
            let same_fluid = catalog
                .fluid(state_id)
                .is_some_and(|fluid| fluid.kind == kind);
            let solid_render = catalog
                .get(state_id)
                .is_some_and(|model| model.solid_render);
            if !same_fluid && !solid_render {
                return true;
            }
        }
    }
    false
}

fn fluid_height_at(
    area: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    kind: TexturedFluidKind,
) -> f32 {
    let mut count = 0;
    let mut height = 0.0;
    for sample in 0..4 {
        let sample_x = world_x - (sample & 1);
        let sample_z = world_z - ((sample >> 1) & 1);
        if same_fluid_at(area, catalog, sample_x, world_y + 1, sample_z, kind) {
            return 1.0;
        }

        let state_id = block_state_at_world_or_air(area, sample_x, world_y, sample_z);
        if let Some(sample_fluid) = catalog.fluid(state_id).filter(|fluid| fluid.kind == kind) {
            let sample_height = fluid_own_height(sample_fluid);
            if sample_height >= 0.8 {
                height += sample_height * 10.0;
                count += 10;
            } else {
                height += sample_height;
                count += 1;
            }
        } else if !liquid_height_sample_is_solid(catalog, state_id) {
            count += 1;
        }
    }

    if count == 0 {
        0.0
    } else {
        height / count as f32
    }
}

fn fluid_own_height(fluid: TexturedFluidModel) -> f32 {
    if fluid.level == 0 {
        MAX_FLUID_HEIGHT
    } else {
        fluid.level.min(8) as f32 / 9.0
    }
}

fn liquid_height_sample_is_solid(catalog: &TexturedMeshCatalog, state_id: BlockStateId) -> bool {
    catalog
        .get(state_id)
        .map(|model| model.collision_shape_full_block)
        .unwrap_or(false)
}

fn liquid_side_v(height: f32) -> f32 {
    (1.0 - height.clamp(0.0, 1.0)) * 8.0
}

fn liquid_packed_light(
    area: &[TexturedChunkMeshInput<'_>],
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> u32 {
    let current = packed_light_at_world_or_fullbright(area, world_x, world_y, world_z);
    let above = packed_light_at_world_or_fullbright(area, world_x, world_y + 1, world_z);
    pack_light(
        packed_block_light(current).max(packed_block_light(above)),
        packed_sky_light(current).max(packed_sky_light(above)),
    )
}

fn textured_face_corners(face: &TexturedBlockFace) -> [[f32; 3]; 4] {
    let min_x = face.from[0] / 16.0;
    let min_y = face.from[1] / 16.0;
    let min_z = face.from[2] / 16.0;
    let max_x = face.to[0] / 16.0;
    let max_y = face.to[1] / 16.0;
    let max_z = face.to[2] / 16.0;

    match face.direction {
        ModelFaceDirection::Down => [
            [min_x, min_y, max_z],
            [min_x, min_y, min_z],
            [max_x, min_y, min_z],
            [max_x, min_y, max_z],
        ],
        ModelFaceDirection::Up => [
            [min_x, max_y, min_z],
            [min_x, max_y, max_z],
            [max_x, max_y, max_z],
            [max_x, max_y, min_z],
        ],
        ModelFaceDirection::North => [
            [max_x, max_y, min_z],
            [max_x, min_y, min_z],
            [min_x, min_y, min_z],
            [min_x, max_y, min_z],
        ],
        ModelFaceDirection::South => [
            [min_x, max_y, max_z],
            [min_x, min_y, max_z],
            [max_x, min_y, max_z],
            [max_x, max_y, max_z],
        ],
        ModelFaceDirection::West => [
            [min_x, max_y, min_z],
            [min_x, min_y, min_z],
            [min_x, min_y, max_z],
            [min_x, max_y, max_z],
        ],
        ModelFaceDirection::East => [
            [max_x, max_y, max_z],
            [max_x, min_y, max_z],
            [max_x, min_y, min_z],
            [max_x, max_y, min_z],
        ],
    }
}

fn textured_face_uvs(face: &TexturedBlockFace) -> [[f32; 2]; 4] {
    let rotation_steps = (face.uv_rotation / 90).rem_euclid(4);
    std::array::from_fn(|index| {
        let shifted = (index as i32 + rotation_steps).rem_euclid(4);
        let u = if shifted == 0 || shifted == 1 {
            face.uv[0]
        } else {
            face.uv[2]
        };
        let v = if shifted == 0 || shifted == 3 {
            face.uv[1]
        } else {
            face.uv[3]
        };
        face.sprite.map(u, v)
    })
}

fn liquid_color_at(
    area: &[TexturedChunkMeshInput<'_>],
    kind: TexturedFluidKind,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    shade: f32,
) -> [f32; 4] {
    blended_liquid_color(kind, world_x, world_y, world_z, shade, |x, y, z| {
        biome_id_at_world_or_default(area, x, y, z)
    })
}

fn shaded_color(block_id: u8, shade: f32) -> [f32; 4] {
    let [r, g, b] = base_color(block_id);
    [r * shade, g * shade, b * shade, 1.0]
}

fn base_color(block_id: u8) -> [f32; 3] {
    match block_id {
        1 => [0.48, 0.48, 0.48],
        2 => [0.18, 0.34, 0.78],
        3 => [0.13, 0.12, 0.13],
        4 => [0.22, 0.56, 0.19],
        5 => [0.42, 0.28, 0.14],
        6 => [0.76, 0.68, 0.45],
        7 => [0.43, 0.42, 0.40],
        13 => [0.36, 0.24, 0.12],
        14 => [0.31, 0.24, 0.10],
        15 => [0.38, 0.28, 0.42],
        16 => [0.58, 0.32, 0.22],
        17 => [0.78, 0.70, 0.62],
        18 => [0.70, 0.36, 0.13],
        19 => [0.58, 0.28, 0.46],
        20 => [0.43, 0.52, 0.68],
        21 => [0.78, 0.66, 0.22],
        22 => [0.47, 0.60, 0.20],
        23 => [0.70, 0.42, 0.50],
        24 => [0.28, 0.25, 0.24],
        25 => [0.53, 0.49, 0.46],
        26 => [0.32, 0.45, 0.45],
        27 => [0.43, 0.28, 0.48],
        28 => [0.26, 0.32, 0.54],
        29 => [0.34, 0.22, 0.14],
        30 => [0.28, 0.36, 0.18],
        31 => [0.55, 0.25, 0.20],
        32 => [0.12, 0.10, 0.09],
        33 => [0.70, 0.62, 0.40],
        34 => [0.65, 0.35, 0.18],
        35 => [0.62, 0.78, 0.86],
        38 => [0.74, 0.35, 0.15],
        39 => [0.55, 0.76, 0.88],
        40 => [0.86, 0.90, 0.92],
        52 => [0.36, 0.36, 0.35],
        53 => [0.28, 0.28, 0.30],
        54 => [0.18, 0.18, 0.18],
        55 => [0.12, 0.12, 0.13],
        56 => [0.60, 0.42, 0.30],
        57 => [0.42, 0.31, 0.29],
        58 => [0.70, 0.58, 0.48],
        59 => [0.50, 0.44, 0.42],
        60 => [0.86, 0.70, 0.25],
        61 => [0.62, 0.52, 0.26],
        62 => [0.62, 0.08, 0.08],
        63 => [0.42, 0.07, 0.08],
        64 => [0.34, 0.78, 0.82],
        65 => [0.25, 0.56, 0.60],
        66 => [0.18, 0.24, 0.70],
        67 => [0.16, 0.20, 0.48],
        70 => [0.42, 0.72, 0.42],
        88 => [0.50, 0.55, 0.58],
        _ => [0.82, 0.22, 0.70],
    }
}

pub(crate) fn block_at_world_or_air(
    inputs: &[ChunkMeshInput<'_>],
    world_x: i32,
    y: i32,
    world_z: i32,
) -> u8 {
    let chunk_x = block_to_chunk_coord(world_x);
    let chunk_z = block_to_chunk_coord(world_z);
    let local_x = local_block_coord(world_x);
    let local_z = local_block_coord(world_z);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.block_at_or_air(local_x, y - input.min_y, local_z))
        .unwrap_or(AIR_BLOCK_ID)
}

fn is_air_like_block_id(block_id: u8) -> bool {
    matches!(block_id, AIR_BLOCK_ID | CAVE_AIR_BLOCK_ID)
}

fn textured_section_is_air_like(
    input: TexturedChunkMeshInput<'_>,
    local_y_start: i32,
    local_y_end: i32,
) -> bool {
    for local_y in local_y_start..local_y_end {
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                if !is_air_like_block_state(input.block_at_or_air(local_x, local_y, local_z)) {
                    return false;
                }
            }
        }
    }
    true
}

fn is_air_like_block_state(state_id: BlockStateId) -> bool {
    matches!(state_id, AIR_BLOCK_STATE_ID | CAVE_AIR_BLOCK_STATE_ID)
}

fn block_state_at_world_or_air(
    inputs: &[TexturedChunkMeshInput<'_>],
    world_x: i32,
    y: i32,
    world_z: i32,
) -> BlockStateId {
    let chunk_x = block_to_chunk_coord(world_x);
    let chunk_z = block_to_chunk_coord(world_z);
    let local_x = local_block_coord(world_x);
    let local_z = local_block_coord(world_z);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.block_at_or_air(local_x, y - input.min_y, local_z))
        .unwrap_or(AIR_BLOCK_STATE_ID)
}

fn biome_id_at_world_or_default(
    inputs: &[TexturedChunkMeshInput<'_>],
    world_x: i32,
    y: i32,
    world_z: i32,
) -> i32 {
    if let Some(zoom_seed) = inputs.iter().find_map(|input| input.biome_zoom_seed) {
        return fuzzy_offset_constant_column_biome_id(zoom_seed, world_x, world_z, |x, y, z| {
            biome_id_at_quart_or_default(inputs, x, y, z)
        });
    }

    let chunk_x = block_to_chunk_coord(world_x);
    let chunk_z = block_to_chunk_coord(world_z);
    let local_x = local_block_coord(world_x);
    let local_z = local_block_coord(world_z);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.biome_id_at_or_default(local_x, y, local_z))
        .unwrap_or(DEFAULT_BIOME_ID)
}

fn biome_id_at_quart_or_default(
    inputs: &[TexturedChunkMeshInput<'_>],
    quart_x: i32,
    quart_y: i32,
    quart_z: i32,
) -> i32 {
    let chunk_x = quart_x.div_euclid(4);
    let chunk_z = quart_z.div_euclid(4);
    let local_quart_x = quart_x.rem_euclid(4);
    let local_quart_z = quart_z.rem_euclid(4);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.biome_id_at_quart_or_default(local_quart_x, quart_y, local_quart_z))
        .unwrap_or(DEFAULT_BIOME_ID)
}

fn fuzzy_offset_constant_column_biome_id(
    zoom_seed: i64,
    block_x: i32,
    block_z: i32,
    noise_biome_source: impl Fn(i32, i32, i32) -> i32,
) -> i32 {
    let (quart_x, quart_y, quart_z) =
        fuzzy_offset_constant_column_quart(zoom_seed, block_x, block_z);
    noise_biome_source(quart_x, quart_y, quart_z)
}

fn fuzzy_offset_constant_column_quart(
    zoom_seed: i64,
    block_x: i32,
    block_z: i32,
) -> (i32, i32, i32) {
    fuzzy_offset_quart(zoom_seed, block_x, 0, block_z)
}

fn fuzzy_offset_quart(zoom_seed: i64, block_x: i32, block_y: i32, block_z: i32) -> (i32, i32, i32) {
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
    (quart_x, quart_y, quart_z)
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

fn linear_congruential_generator_next(left: i64, right: i64) -> i64 {
    let transformed = left
        .wrapping_mul(LCG_MULTIPLIER)
        .wrapping_add(LCG_INCREMENT);
    left.wrapping_mul(transformed).wrapping_add(right)
}

fn packed_light_at_world_or_fullbright(
    inputs: &[TexturedChunkMeshInput<'_>],
    world_x: i32,
    y: i32,
    world_z: i32,
) -> u32 {
    let chunk_x = block_to_chunk_coord(world_x);
    let chunk_z = block_to_chunk_coord(world_z);
    let local_x = local_block_coord(world_x);
    let local_z = local_block_coord(world_z);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.packed_light_at_or_fullbright(local_x, y, local_z))
        .unwrap_or(FULL_BRIGHT)
}

fn flat_packed_light_for_face(
    area: &[TexturedChunkMeshInput<'_>],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    face: &TexturedBlockFace,
    ao_shape: AmbientOcclusionShape,
) -> u32 {
    let Some(cullface) = face.cullface else {
        if ao_shape.use_face_neighbor {
            let offset = direction_offset(face.direction);
            return packed_light_at_world_or_fullbright(
                area,
                world_x + offset[0],
                world_y + offset[1],
                world_z + offset[2],
            );
        }
        return packed_light_at_world_or_fullbright(area, world_x, world_y, world_z);
    };

    let offset = direction_offset(cullface);
    packed_light_at_world_or_fullbright(
        area,
        world_x + offset[0],
        world_y + offset[1],
        world_z + offset[2],
    )
}

struct TexturedAmbientOcclusionSampler<'a> {
    area: &'a [TexturedChunkMeshInput<'a>],
    catalog: &'a TexturedMeshCatalog,
}

impl AmbientOcclusionSampler for TexturedAmbientOcclusionSampler<'_> {
    fn light_color(&self, pos: BlockPos) -> u32 {
        packed_light_at_world_or_fullbright(self.area, pos.x, pos.y, pos.z)
    }

    fn shade_brightness(&self, pos: BlockPos) -> f32 {
        self.catalog
            .shade_brightness(self.block_state_at_or_air(pos))
    }

    fn light_block(&self, pos: BlockPos) -> u8 {
        self.catalog.light_block(self.block_state_at_or_air(pos))
    }

    fn view_blocking(&self, pos: BlockPos) -> bool {
        self.catalog.view_blocking(self.block_state_at_or_air(pos))
    }

    fn solid_render(&self, pos: BlockPos) -> bool {
        self.catalog.solid_render(self.block_state_at_or_air(pos))
    }
}

impl TexturedAmbientOcclusionSampler<'_> {
    fn block_state_at_or_air(&self, pos: BlockPos) -> BlockStateId {
        block_state_at_world_or_air(self.area, pos.x, pos.y, pos.z)
    }
}

fn direction_offset(direction: ModelFaceDirection) -> [i32; 3] {
    match direction {
        ModelFaceDirection::Down => [0, -1, 0],
        ModelFaceDirection::Up => [0, 1, 0],
        ModelFaceDirection::North => [0, 0, -1],
        ModelFaceDirection::South => [0, 0, 1],
        ModelFaceDirection::West => [-1, 0, 0],
        ModelFaceDirection::East => [1, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_textured_blocks(height: i32) -> Vec<BlockStateId> {
        vec![AIR_BLOCK_STATE_ID; CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * height as usize]
    }

    #[test]
    fn bushy_leaf_layout_is_stable_and_varies_by_world_position() {
        let origin = bushy_leaf_layout(0, 64, 0);
        assert_eq!(origin, bushy_leaf_layout(0, 64, 0));
        assert!(
            (1..32).any(|x| bushy_leaf_layout(x, 64, 0) != origin),
            "nearby leaves should not all select the same card layout"
        );
    }

    fn biome_id_for_world_quart(quart_x: i32, quart_z: i32) -> i32 {
        10_000 + quart_x * 100 + quart_z
    }

    fn test_biomes_for_chunk(chunk_x: i32, chunk_z: i32, height: i32) -> Vec<i32> {
        let quart_height = height / 4;
        let mut biomes = Vec::with_capacity((quart_height * 4 * 4) as usize);
        for _quart_y in 0..quart_height {
            for local_quart_z in 0..4 {
                for local_quart_x in 0..4 {
                    biomes.push(biome_id_for_world_quart(
                        chunk_x * 4 + local_quart_x,
                        chunk_z * 4 + local_quart_z,
                    ));
                }
            }
        }
        biomes
    }

    #[test]
    fn obfuscates_biome_zoom_seed_like_biome_manager() {
        assert_eq!(obfuscate_biome_zoom_seed(1124), 2_991_024_998_819_753_860);
    }

    #[test]
    fn fuzzy_constant_column_quart_matches_reference_cases() {
        let zoom_seed = obfuscate_biome_zoom_seed(1124);

        assert_eq!(
            fuzzy_offset_constant_column_quart(zoom_seed, 0, 0),
            (-1, -1, 0)
        );
        assert_eq!(
            fuzzy_offset_constant_column_quart(zoom_seed, 4, 0),
            (1, 0, -1)
        );
        assert_eq!(
            fuzzy_offset_constant_column_quart(zoom_seed, 15, 15),
            (3, 0, 3)
        );
        assert_eq!(
            fuzzy_offset_constant_column_quart(zoom_seed, 32, 0),
            (8, 0, 0)
        );
    }

    #[test]
    fn unseeded_biome_lookup_uses_direct_quart_cell() {
        let blocks = empty_textured_blocks(16);
        let center_biomes = test_biomes_for_chunk(0, 0, 16);
        let input = TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks).with_biomes(&center_biomes);

        assert_eq!(
            biome_id_at_world_or_default(&[input], 4, 64, 0),
            biome_id_for_world_quart(1, 0)
        );
    }

    #[test]
    fn seeded_biome_lookup_uses_fuzzy_offset_constant_column() {
        let blocks = empty_textured_blocks(16);
        let center_biomes = test_biomes_for_chunk(0, 0, 16);
        let north_biomes = test_biomes_for_chunk(0, -1, 16);
        let west_biomes = test_biomes_for_chunk(-1, 0, 16);
        let inputs = [
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks)
                .with_biomes(&center_biomes)
                .with_world_seed(1124),
            TexturedChunkMeshInput::new(0, -1, 0, 16, &blocks)
                .with_biomes(&north_biomes)
                .with_world_seed(1124),
            TexturedChunkMeshInput::new(-1, 0, 0, 16, &blocks)
                .with_biomes(&west_biomes)
                .with_world_seed(1124),
        ];

        assert_eq!(
            biome_id_at_world_or_default(&inputs, 4, 64, 0),
            biome_id_for_world_quart(1, -1)
        );
        assert_eq!(
            biome_id_at_world_or_default(&inputs, 0, 64, 0),
            biome_id_for_world_quart(-1, 0)
        );
    }

    #[test]
    fn all_air_textured_sections_keep_visibility_without_mesh_work() {
        let blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 16];
        let input = TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks);
        let report =
            build_textured_render_sections_with_stats(&[input], &TexturedMeshCatalog::default())
                .expect("all-air section should build");

        assert_eq!(report.sections.len(), 1);
        assert_eq!(report.sections[0].key, RenderSectionKey::new(0, 0, 0));
        assert!(report.sections[0].is_empty());
        assert_eq!(report.sections[0].visibility, VisibilitySet::all_visible());
        assert_eq!(report.visibility_graph.build_count, 1);
        assert_eq!(report.visibility_graph.total_ms, 0.0);
    }
}
