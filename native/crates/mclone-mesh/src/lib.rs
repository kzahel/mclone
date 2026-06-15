#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_assets::{
    AssetError, BakedBlockModelFace, BlockModelLibrary, BlockStateAssetIndex, BlockStateRegistry,
    ModelFaceDirection, ResourceLocation, TextureAtlasPlan, TextureMaterial,
};
use mclone_core::{AIR_BLOCK_STATE_ID, BlockStateId};

pub const CHUNK_WIDTH: i32 = 16;
pub const RENDER_SECTION_HEIGHT: i32 = 16;
pub const AIR_BLOCK_ID: u8 = 0;
pub const CAVE_AIR_BLOCK_ID: u8 = 71;
pub const CAVE_AIR_BLOCK_STATE_ID: BlockStateId = BlockStateId(71);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SectionMeshStats {
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisibleChunkMesh {
    pub vertices: Vec<ChunkVertex>,
    pub indices: Vec<u32>,
}

impl VisibleChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn stats(&self) -> SectionMeshStats {
        SectionMeshStats {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedChunkVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedVisibleChunkMesh {
    pub vertices: Vec<TexturedChunkVertex>,
    pub indices: Vec<u32>,
}

impl TexturedVisibleChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn stats(&self) -> SectionMeshStats {
        SectionMeshStats {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RenderSectionKey {
    pub chunk_x: i32,
    pub section_y: i32,
    pub chunk_z: i32,
}

impl RenderSectionKey {
    pub fn new(chunk_x: i32, section_y: i32, chunk_z: i32) -> Self {
        Self {
            chunk_x,
            section_y,
            chunk_z,
        }
    }

    pub fn min_y(self) -> i32 {
        self.section_y * RENDER_SECTION_HEIGHT
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TexturedRenderSectionMesh {
    pub key: RenderSectionKey,
    pub mesh: TexturedVisibleChunkMesh,
}

impl TexturedRenderSectionMesh {
    pub fn is_empty(&self) -> bool {
        self.mesh.is_empty()
    }

    pub fn stats(&self) -> SectionMeshStats {
        self.mesh.stats()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasSpriteUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl AtlasSpriteUv {
    fn map(self, u: f32, v: f32) -> [f32; 2] {
        [
            self.u0 + (self.u1 - self.u0) * (u / 16.0),
            self.v0 + (self.v1 - self.v0) * (v / 16.0),
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TexturedBlockFace {
    pub direction: ModelFaceDirection,
    pub cullface: Option<ModelFaceDirection>,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub uv: [f32; 4],
    pub uv_rotation: i32,
    pub sprite: AtlasSpriteUv,
    pub tintindex: i32,
    pub shade: bool,
}

impl TexturedBlockFace {
    fn from_baked(
        face: &BakedBlockModelFace,
        atlas: &TextureAtlasPlan,
    ) -> Result<Self, TexturedMeshError> {
        let sprite = atlas
            .sprite(&face.texture)
            .ok_or_else(|| TexturedMeshError::MissingSprite(face.texture.clone()))?;
        Ok(Self {
            direction: face.direction,
            cullface: face.cullface,
            from: face.from,
            to: face.to,
            uv: face.uv,
            uv_rotation: face.uv_rotation,
            sprite: AtlasSpriteUv {
                u0: sprite.u0,
                v0: sprite.v0,
                u1: sprite.u1,
                v1: sprite.v1,
            },
            tintindex: face.tintindex,
            shade: face.shade,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedBlockModel {
    pub faces: Vec<TexturedBlockFace>,
    pub occludes: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedMeshCatalog {
    blocks: BTreeMap<BlockStateId, TexturedBlockModel>,
}

impl TexturedMeshCatalog {
    pub fn from_assets(
        registry: &BlockStateRegistry,
        blockstates: &BlockStateAssetIndex,
        models: &BlockModelLibrary,
        atlas: &TextureAtlasPlan,
    ) -> Result<Self, TexturedMeshError> {
        let mut blocks = BTreeMap::new();
        for record in registry.records() {
            let asset = blockstates
                .get(&record.block)
                .ok_or_else(|| TexturedMeshError::MissingBlockStateAsset(record.block.clone()))?;
            let variant_key = record
                .asset_variant_key(asset)
                .unwrap_or_else(|| record.variant_key());
            let model = if let Some(variants) = asset.variants_for_key(&variant_key) {
                variants
                    .first()
                    .map(|variant| &variant.model)
                    .ok_or_else(|| TexturedMeshError::MissingBlockStateVariant {
                        block: record.block.clone(),
                        variant_key: variant_key.clone(),
                    })?
            } else if variant_key.is_empty() {
                asset.model_refs.iter().next().ok_or_else(|| {
                    TexturedMeshError::MissingBlockStateVariant {
                        block: record.block.clone(),
                        variant_key: variant_key.clone(),
                    }
                })?
            } else {
                return Err(TexturedMeshError::MissingBlockStateVariant {
                    block: record.block.clone(),
                    variant_key,
                });
            };
            let baked = models.bake_model(model)?;
            let faces = baked
                .faces
                .iter()
                .map(|face| TexturedBlockFace::from_baked(face, atlas))
                .collect::<Result<Vec<_>, _>>()?;
            let occludes = full_cube_occluder(&faces);
            blocks.insert(record.id, TexturedBlockModel { faces, occludes });
        }

        Ok(Self { blocks })
    }

    pub fn get(&self, state_id: BlockStateId) -> Option<&TexturedBlockModel> {
        self.blocks.get(&state_id)
    }

    pub fn occludes(&self, state_id: BlockStateId) -> bool {
        self.blocks
            .get(&state_id)
            .map(|model| model.occludes)
            .unwrap_or(false)
    }
}

#[derive(Debug)]
pub enum TexturedMeshError {
    Asset(AssetError),
    MissingBlockStateAsset(ResourceLocation),
    MissingBlockStateVariant {
        block: ResourceLocation,
        variant_key: String,
    },
    MissingBlockModel(BlockStateId),
    MissingSprite(TextureMaterial),
}

impl fmt::Display for TexturedMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(f, "{error}"),
            Self::MissingBlockStateAsset(block) => {
                write!(f, "missing blockstate asset for {block}")
            }
            Self::MissingBlockStateVariant { block, variant_key } => {
                write!(f, "missing blockstate variant `{variant_key}` for {block}")
            }
            Self::MissingBlockModel(state_id) => {
                write!(
                    f,
                    "missing textured block model for state id {}",
                    state_id.0
                )
            }
            Self::MissingSprite(material) => {
                write!(
                    f,
                    "missing atlas sprite for material {} in {}",
                    material.texture, material.atlas
                )
            }
        }
    }
}

impl Error for TexturedMeshError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AssetError> for TexturedMeshError {
    fn from(value: AssetError) -> Self {
        Self::Asset(value)
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
        self.blocks[block_index(local_x, local_y, local_z)]
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
}

impl<'a> TexturedChunkMeshInput<'a> {
    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        blocks: &'a [BlockStateId],
    ) -> Self {
        if height <= 0 || height % CHUNK_WIDTH != 0 {
            panic!("chunk height {height} must be a positive multiple of {CHUNK_WIDTH}");
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
        }
    }

    fn block_at_or_air(&self, local_x: i32, local_y: i32, local_z: i32) -> BlockStateId {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(0..self.height).contains(&local_y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            return AIR_BLOCK_STATE_ID;
        }
        self.blocks[block_index(local_x, local_y, local_z)]
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
    let mut sections = Vec::new();
    for input in inputs {
        for local_y_start in (0..input.height).step_by(RENDER_SECTION_HEIGHT as usize) {
            let local_y_end = (local_y_start + RENDER_SECTION_HEIGHT).min(input.height);
            let mut mesh = TexturedVisibleChunkMesh::default();
            add_textured_chunk_range_to_mesh(
                &mut mesh,
                *input,
                inputs,
                catalog,
                local_y_start,
                local_y_end,
            )?;
            if !mesh.is_empty() {
                sections.push(TexturedRenderSectionMesh {
                    key: RenderSectionKey::new(
                        input.chunk_x,
                        (input.min_y + local_y_start).div_euclid(RENDER_SECTION_HEIGHT),
                        input.chunk_z,
                    ),
                    mesh,
                });
            }
        }
    }
    Ok(sections)
}

fn add_chunk_to_mesh(
    mesh: &mut VisibleChunkMesh,
    input: ChunkMeshInput<'_>,
    area: &[ChunkMeshInput<'_>],
) {
    let world_origin_x = chunk_world_origin(input.chunk_x);
    let world_origin_z = chunk_world_origin(input.chunk_z);

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
    let world_origin_x = chunk_world_origin(input.chunk_x);
    let world_origin_z = chunk_world_origin(input.chunk_z);

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
                    add_textured_face(mesh, world_x, world_y, world_z, face);
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
) {
    let base_index = mesh.vertices.len() as u32;
    let color = textured_face_color(face);
    let corners = textured_face_corners(face);
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

fn textured_face_color(face: &TexturedBlockFace) -> [f32; 4] {
    let shade = if face.shade {
        face_shade(face.direction)
    } else {
        1.0
    };
    let tint = block_tint(face.tintindex);
    [tint[0] * shade, tint[1] * shade, tint[2] * shade, 1.0]
}

fn block_tint(tintindex: i32) -> [f32; 3] {
    if tintindex >= 0 {
        [0.46, 0.70, 0.27]
    } else {
        [1.0, 1.0, 1.0]
    }
}

fn face_shade(direction: ModelFaceDirection) -> f32 {
    match direction {
        ModelFaceDirection::Up => 1.0,
        ModelFaceDirection::Down => 0.5,
        ModelFaceDirection::East | ModelFaceDirection::West => 0.86,
        ModelFaceDirection::South => 0.78,
        ModelFaceDirection::North => 0.72,
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

fn block_index(local_x: i32, local_y: i32, local_z: i32) -> usize {
    ((local_y << 8) | (local_z << 4) | local_x) as usize
}

fn block_at_world_or_air(inputs: &[ChunkMeshInput<'_>], world_x: i32, y: i32, world_z: i32) -> u8 {
    let chunk_x = world_x.div_euclid(CHUNK_WIDTH);
    let chunk_z = world_z.div_euclid(CHUNK_WIDTH);
    let local_x = world_x.rem_euclid(CHUNK_WIDTH);
    let local_z = world_z.rem_euclid(CHUNK_WIDTH);
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
    let chunk_x = world_x.div_euclid(CHUNK_WIDTH);
    let chunk_z = world_z.div_euclid(CHUNK_WIDTH);
    let local_x = world_x.rem_euclid(CHUNK_WIDTH);
    let local_z = world_z.rem_euclid(CHUNK_WIDTH);
    inputs
        .iter()
        .find(|input| input.chunk_x == chunk_x && input.chunk_z == chunk_z)
        .map(|input| input.block_at_or_air(local_x, y - input.min_y, local_z))
        .unwrap_or(AIR_BLOCK_STATE_ID)
}

fn chunk_world_origin(chunk_coord: i32) -> i32 {
    chunk_coord * CHUNK_WIDTH
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

fn full_cube_occluder(faces: &[TexturedBlockFace]) -> bool {
    let mut covered = BTreeSet::new();
    for face in faces {
        if face.cullface == Some(face.direction) && face_is_full_cube_side(face) {
            covered.insert(face.direction);
        }
    }
    covered.len() == 6
}

fn face_is_full_cube_side(face: &TexturedBlockFace) -> bool {
    const EPSILON: f32 = 0.0001;
    let full_bounds = face.from.iter().all(|value| value.abs() <= EPSILON)
        && face.to.iter().all(|value| (*value - 16.0).abs() <= EPSILON);
    if !full_bounds {
        return false;
    }
    match face.direction {
        ModelFaceDirection::Down => (face.from[1] - 0.0).abs() <= EPSILON,
        ModelFaceDirection::Up => (face.to[1] - 16.0).abs() <= EPSILON,
        ModelFaceDirection::North => (face.from[2] - 0.0).abs() <= EPSILON,
        ModelFaceDirection::South => (face.to[2] - 16.0).abs() <= EPSILON,
        ModelFaceDirection::West => (face.from[0] - 0.0).abs() <= EPSILON,
        ModelFaceDirection::East => (face.to[0] - 16.0).abs() <= EPSILON,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_assets::{
        AssetPath, BlockModelLibrary, BlockStateAssetIndex, BlockStateRecord, BlockStateRegistry,
        MemoryAssetSource, ResourceLocation, TextureAtlasPlan,
    };

    fn chunk_blocks(height: i32, filled: &[(i32, i32, i32, u8)]) -> Vec<u8> {
        let mut blocks = vec![AIR_BLOCK_ID; height as usize * 16 * 16];
        for &(x, y, z, block_id) in filled {
            blocks[block_index(x, y, z)] = block_id;
        }
        blocks
    }

    #[test]
    fn empty_mesh_stats_are_zero() {
        assert_eq!(SectionMeshStats::default().vertex_count, 0);
        assert_eq!(SectionMeshStats::default().index_count, 0);
    }

    #[test]
    fn single_block_emits_six_faces() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
    }

    #[test]
    fn adjacent_blocks_cull_shared_face() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
    }

    #[test]
    fn cave_air_is_invisible_and_non_occluding_in_debug_mesh() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, CAVE_AIR_BLOCK_ID)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
    }

    #[test]
    fn mesh_positions_use_world_chunk_offset() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(2, -1, 4, 16, &blocks));

        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.position == [32.0, 4.0, -16.0])
        );
    }

    #[test]
    fn area_mesh_culls_faces_across_chunk_boundaries() {
        let left = chunk_blocks(16, &[(15, 0, 0, 1)]);
        let right = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_area_mesh(&[
            ChunkMeshInput::new(0, 0, 0, 16, &left),
            ChunkMeshInput::new(1, 0, 0, 16, &right),
        ]);

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
    }

    #[test]
    fn area_mesh_uses_euclidean_chunk_coordinates_for_negative_world_positions() {
        let chunk = chunk_blocks(16, &[(15, 0, 15, 1)]);
        assert_eq!(
            block_at_world_or_air(&[ChunkMeshInput::new(-1, -1, 0, 16, &chunk)], -1, 0, -1),
            1
        );
    }

    fn textured_chunk_blocks(
        height: i32,
        filled: &[(i32, i32, i32, BlockStateId)],
    ) -> Vec<BlockStateId> {
        let mut blocks = vec![AIR_BLOCK_STATE_ID; height as usize * 16 * 16];
        for &(x, y, z, block_id) in filled {
            blocks[block_index(x, y, z)] = block_id;
        }
        blocks
    }

    fn stone_textured_catalog() -> TexturedMeshCatalog {
        let mut registry = BlockStateRegistry::new();
        registry
            .register(BlockStateRecord::new(
                BlockStateId(1),
                ResourceLocation::parse("minecraft:stone").unwrap(),
                [] as [(&str, &str); 0],
            ))
            .unwrap();
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/stone.json"),
            r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/block.json"),
            "{}",
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/cube.json"),
            r##"{
              "parent":"minecraft:block/block",
              "elements":[{
                "from":[0,0,0],
                "to":[16,16,16],
                "faces":{
                  "down":{"texture":"#down","cullface":"down"},
                  "up":{"texture":"#up","cullface":"up"},
                  "north":{"texture":"#north","cullface":"north"},
                  "south":{"texture":"#south","cullface":"south"},
                  "west":{"texture":"#west","cullface":"west"},
                  "east":{"texture":"#east","cullface":"east"}
                }
              }]
            }"##,
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/cube_all.json"),
            r##"{
              "parent":"minecraft:block/cube",
              "textures":{
                "particle":"#all",
                "down":"#all",
                "up":"#all",
                "north":"#all",
                "east":"#all",
                "south":"#all",
                "west":"#all"
              }
            }"##,
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/stone.json"),
            r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
        );
        source.insert(
            AssetPath::new("assets/minecraft/textures/block/stone.png"),
            png_header(16, 16),
        );

        let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
        let library = BlockModelLibrary::load_for_blockstates(&source, &index).unwrap();
        let materials = library
            .collect_materials_for_models(
                index
                    .assets()
                    .flat_map(|asset| asset.model_refs.iter().cloned()),
            )
            .unwrap();
        let atlas = TextureAtlasPlan::build(&source, materials).unwrap();
        TexturedMeshCatalog::from_assets(&registry, &index, &library, &atlas).unwrap()
    }

    #[test]
    fn textured_single_block_uses_baked_model_faces_and_atlas_uvs() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(1))]);
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
        assert!(catalog.occludes(BlockStateId(1)));
        assert!(!catalog.occludes(AIR_BLOCK_STATE_ID));
        assert!(mesh.vertices.iter().all(
            |vertex| (0.0..=1.0).contains(&vertex.uv[0]) && (0.0..=1.0).contains(&vertex.uv[1])
        ));
    }

    #[test]
    fn cave_air_is_invisible_and_non_occluding_in_textured_mesh() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            16,
            &[
                (0, 0, 0, BlockStateId(1)),
                (1, 0, 0, CAVE_AIR_BLOCK_STATE_ID),
            ],
        );
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
        assert!(!catalog.occludes(CAVE_AIR_BLOCK_STATE_ID));
    }

    #[test]
    fn textured_adjacent_blocks_cull_shared_face() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            16,
            &[(0, 0, 0, BlockStateId(1)), (1, 0, 0, BlockStateId(1))],
        );
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
    }

    #[test]
    fn textured_render_sections_split_by_vertical_section() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            32,
            &[(0, 0, 0, BlockStateId(1)), (0, 16, 0, BlockStateId(1))],
        );
        let sections = build_textured_render_sections(
            &[TexturedChunkMeshInput::new(2, -3, -16, 32, &blocks)],
            &catalog,
        )
        .unwrap();

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].key, RenderSectionKey::new(2, -1, -3));
        assert_eq!(sections[1].key, RenderSectionKey::new(2, 0, -3));
        assert_eq!(sections[0].stats().index_count, 36);
        assert_eq!(sections[1].stats().index_count, 36);
    }

    #[test]
    fn textured_render_sections_cull_across_section_boundary() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            32,
            &[(0, 15, 0, BlockStateId(1)), (0, 16, 0, BlockStateId(1))],
        );
        let sections = build_textured_render_sections(
            &[TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks)],
            &catalog,
        )
        .unwrap();
        let combined = build_textured_visible_chunk_area_mesh(
            &[TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks)],
            &catalog,
        )
        .unwrap();

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].stats().index_count, 30);
        assert_eq!(sections[1].stats().index_count, 30);
        assert_eq!(combined.stats().index_count, 60);
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }
}
