use std::collections::BTreeSet;

use mclone_assets::ModelFaceDirection;
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_WIDTH, PackedLightSection,
    SECTION_HEIGHT as RENDER_SECTION_HEIGHT, block_to_chunk_coord, block_to_section_coord,
    chunk_block_index, chunk_min_block_coord, chunk_section_index, local_block_coord,
    local_section_block_coord,
};
use mclone_light::{FULL_BRIGHT, pack_light};

use crate::ambient_occlusion::{
    AmbientOcclusionFace, AmbientOcclusionSampler, AmbientOcclusionShape, BlockPos,
    calculate_ambient_occlusion_face, calculate_ambient_occlusion_shape,
};
use crate::catalog::{TexturedBlockFace, TexturedMeshCatalog, TexturedMeshError};
use crate::data::{
    ChunkVertex, RenderSectionKey, TexturedChunkVertex, TexturedRenderSectionBuildReport,
    TexturedRenderSectionMesh, TexturedVisibleChunkMesh, VisibilityGraphBuildStats,
    VisibleChunkMesh,
};
use crate::visibility::{VisGraph, VisibilityGraphTimer, VisibilitySet};
use crate::{AIR_BLOCK_ID, CAVE_AIR_BLOCK_ID, CAVE_AIR_BLOCK_STATE_ID};

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
    pub light_sections: &'a [PackedLightSection],
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
            light_sections: &[],
        }
    }

    pub fn with_light_sections(mut self, light_sections: &'a [PackedLightSection]) -> Self {
        self.light_sections = light_sections;
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

    fn has_light_data(&self) -> bool {
        !self.light_sections.is_empty()
    }

    fn packed_light_at_or_fullbright(&self, local_x: i32, y: i32, local_z: i32) -> u32 {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(self.min_y..self.min_y + self.height).contains(&y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return FULL_BRIGHT;
        }
        if !self.has_light_data() {
            return FULL_BRIGHT;
        }

        let index = chunk_section_index(local_x, local_section_block_coord(y), local_z);
        let section_y = block_to_section_coord(y);
        let sky = self.sky_light_at(section_y, index);
        let block = self.block_light_at(section_y, index);
        pack_light(block, sky)
    }

    fn block_light_at(&self, section_y: i32, index: usize) -> u8 {
        self.light_sections
            .iter()
            .find(|section| section.section_y == section_y)
            .and_then(|section| section.block.as_deref())
            .map_or(0, |layer| data_layer_value(layer, index))
    }

    fn sky_light_at(&self, section_y: i32, index: usize) -> u8 {
        let mut next_sky_layer = None;
        for section in self.light_sections {
            if section.section_y < section_y {
                continue;
            }
            let Some(layer) = section.sky.as_deref() else {
                continue;
            };
            if section.section_y == section_y {
                return data_layer_value(layer, index);
            }
            if next_sky_layer.is_none_or(|(next_section_y, _)| section.section_y < next_section_y) {
                next_sky_layer = Some((section.section_y, layer));
            }
        }

        if self
            .light_sections
            .iter()
            .any(|section| section.sky.is_some())
        {
            next_sky_layer
                .map(|(_, layer)| data_layer_value(layer, index))
                .unwrap_or(15)
        } else {
            0
        }
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
    build_textured_render_sections_for_chunks(inputs, catalog, None, None)
}

pub fn build_textured_render_sections_for_chunk_set_with_stats(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_chunks: &BTreeSet<(i32, i32)>,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    build_textured_render_sections_for_chunks(inputs, catalog, Some(target_chunks), None)
}

pub fn build_textured_render_sections_for_section_set_with_stats(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport, TexturedMeshError> {
    build_textured_render_sections_for_chunks(inputs, catalog, None, Some(target_sections))
}

fn build_textured_render_sections_for_chunks(
    inputs: &[TexturedChunkMeshInput<'_>],
    catalog: &TexturedMeshCatalog,
    target_chunks: Option<&BTreeSet<(i32, i32)>>,
    target_sections: Option<&BTreeSet<RenderSectionKey>>,
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
            sections.push(TexturedRenderSectionMesh {
                key,
                mesh,
                visibility,
            });
        }
    }
    Ok(TexturedRenderSectionBuildReport {
        sections,
        visibility_graph,
    })
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
                if block_model.faces.is_empty() {
                    continue;
                }

                let world_x = world_origin_x + local_x;
                let world_y = input.min_y + local_y;
                let world_z = world_origin_z + local_z;

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
                    add_textured_face(mesh, world_x, world_y, world_z, face, corners, lighting);
                }
            }
        }
    }

    Ok(())
}

fn append_textured_mesh(mesh: &mut TexturedVisibleChunkMesh, source: &TexturedVisibleChunkMesh) {
    let base_index = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&source.vertices);
    mesh.indices
        .extend(source.indices.iter().map(|index| base_index + *index));
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
    world_x: i32,
    world_y: i32,
    world_z: i32,
    face: &TexturedBlockFace,
    corners: [[f32; 3]; 4],
    lighting: AmbientOcclusionFace,
) {
    let base_index = mesh.vertices.len() as u32;
    let uvs = textured_face_uvs(face);
    for index in 0..4 {
        let corner = corners[index];
        mesh.vertices.push(TexturedChunkVertex {
            position: [
                world_x as f32 + corner[0],
                world_y as f32 + corner[1],
                world_z as f32 + corner[2],
            ],
            uv: uvs[index],
            color: textured_face_color(face, lighting.brightness[index]),
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

fn textured_face_color(face: &TexturedBlockFace, brightness: f32) -> [f32; 4] {
    let tint = block_tint(face.tintindex);
    [
        tint[0] * brightness,
        tint[1] * brightness,
        tint[2] * brightness,
        1.0,
    ]
}

fn block_tint(tintindex: i32) -> [f32; 3] {
    if tintindex >= 0 {
        [0.46, 0.70, 0.27]
    } else {
        [1.0, 1.0, 1.0]
    }
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

pub(crate) fn data_layer_value(layer: &[u8], index: usize) -> u8 {
    debug_assert_eq!(layer.len(), mclone_light::DATA_LAYER_SIZE);
    let byte = layer[index >> 1];
    let shift = 4 * (index & 1);
    (byte >> shift) & 15
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
