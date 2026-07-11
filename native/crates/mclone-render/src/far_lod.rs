use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU32, NonZeroU64};

use glam::Mat4;
use mclone_core::LodTileKey;

use crate::chunk::{ChunkDepthTarget, ChunkRenderView, DEPTH_FORMAT, REVERSED_Z_DEPTH_CLEAR};
use crate::target::RenderFrameTarget;
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

const FAR_TERRAIN_LOD_WGSL: &str = r#"
struct Uniforms {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

const FAR_TERRAIN_LOD_MULTIVIEW_WGSL: &str = r#"
struct Uniforms {
    view_projections: array<mat4x4<f32>, 2>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vertex_main(
    input: VertexInput,
    @builtin(view_index) view_index: i32,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.view_projections[u32(view_index)]
        * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

const FAR_TERRAIN_LOD_VERTEX_FLOATS: usize = 7;
const FAR_TERRAIN_LOD_VERTEX_SIZE: wgpu::BufferAddress =
    (FAR_TERRAIN_LOD_VERTEX_FLOATS * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const FAR_TERRAIN_LOD_MULTIVIEW_UNIFORM_SIZE: wgpu::BufferAddress = 64 * 2;

#[derive(Clone, Debug, PartialEq)]
pub struct FarTerrainLodTileMesh {
    key: LodTileKey,
    vertices: Vec<f32>,
    indices: Vec<u32>,
}

impl FarTerrainLodTileMesh {
    pub fn new(key: LodTileKey, vertices: Vec<f32>, indices: Vec<u32>) -> Self {
        assert!(
            vertices.len() % FAR_TERRAIN_LOD_VERTEX_FLOATS == 0,
            "far terrain LOD tile vertices must use position.xyz + color.rgba"
        );
        Self {
            key,
            vertices,
            indices,
        }
    }

    pub const fn key(&self) -> LodTileKey {
        self.key
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / FAR_TERRAIN_LOD_VERTEX_FLOATS
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    pub fn estimated_owned_bytes(&self) -> usize {
        std::mem::size_of_val(self.vertices.as_slice())
            .saturating_add(std::mem::size_of_val(self.indices.as_slice()))
    }

    pub fn vertices(&self) -> &[f32] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FarTerrainLodFrameUpdate {
    pub revision: u64,
    pub uploads: Vec<FarTerrainLodTileMesh>,
    pub removals: BTreeSet<LodTileKey>,
    pub visible_tiles: BTreeSet<LodTileKey>,
}

impl FarTerrainLodFrameUpdate {
    pub fn is_empty(&self) -> bool {
        self.visible_tiles.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FarTerrainLodRenderStats {
    pub vertex_count: usize,
    pub index_count: usize,
    pub triangle_count: usize,
    pub region_draw_count: usize,
    pub uploaded_bytes: usize,
}

const FAR_TERRAIN_LOD_REGION_CHUNKS: i32 = 16;
const FAR_TERRAIN_LOD_REGION_TILE_COUNT: usize = 16 * 16;
const FAR_TERRAIN_LOD_TILE_VERTEX_CAPACITY: usize = 512;
const FAR_TERRAIN_LOD_TILE_INDEX_CAPACITY: usize = 768;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FarTerrainLodRegionKey {
    x: i32,
    z: i32,
    level: u8,
}

impl FarTerrainLodRegionKey {
    fn for_tile(tile: LodTileKey) -> Self {
        Self {
            x: tile.chunk.x.div_euclid(FAR_TERRAIN_LOD_REGION_CHUNKS),
            z: tile.chunk.z.div_euclid(FAR_TERRAIN_LOD_REGION_CHUNKS),
            level: tile.level,
        }
    }

    fn origin(self) -> [f32; 3] {
        [
            (self.x * FAR_TERRAIN_LOD_REGION_CHUNKS * 16) as f32,
            0.0,
            (self.z * FAR_TERRAIN_LOD_REGION_CHUNKS * 16) as f32,
        ]
    }

    fn tile_slot(self, tile: LodTileKey) -> usize {
        let local_x = tile.chunk.x.rem_euclid(FAR_TERRAIN_LOD_REGION_CHUNKS) as usize;
        let local_z = tile.chunk.z.rem_euclid(FAR_TERRAIN_LOD_REGION_CHUNKS) as usize;
        local_z * FAR_TERRAIN_LOD_REGION_CHUNKS as usize + local_x
    }
}

struct FarTerrainLodRegionArena {
    key: FarTerrainLodRegionKey,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    multiview_uniform_buffer: wgpu::Buffer,
    multiview_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    tile_indices: BTreeMap<LodTileKey, Vec<u32>>,
    tile_vertex_counts: BTreeMap<LodTileKey, usize>,
    index_count: u32,
    vertex_count: usize,
}

impl FarTerrainLodRegionArena {
    fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        multiview_bind_group_layout: &wgpu::BindGroupLayout,
        key: FarTerrainLodRegionKey,
    ) -> Self {
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_far_terrain_lod_region_uniforms",
            64,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_far_terrain_lod_region_bind_group"),
            layout: bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let multiview_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_far_terrain_lod_region_multiview_uniforms"),
            size: FAR_TERRAIN_LOD_MULTIVIEW_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let multiview_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_far_terrain_lod_region_multiview_bind_group"),
            layout: multiview_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: multiview_uniform_buffer.as_entire_binding(),
            }],
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_far_terrain_lod_region_vertices"),
            size: (FAR_TERRAIN_LOD_REGION_TILE_COUNT * FAR_TERRAIN_LOD_TILE_VERTEX_CAPACITY)
                as wgpu::BufferAddress
                * FAR_TERRAIN_LOD_VERTEX_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_far_terrain_lod_region_indices"),
            size: (FAR_TERRAIN_LOD_REGION_TILE_COUNT
                * FAR_TERRAIN_LOD_TILE_INDEX_CAPACITY
                * std::mem::size_of::<u32>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            key,
            uniforms,
            bind_group,
            multiview_uniform_buffer,
            multiview_bind_group,
            vertex_buffer,
            index_buffer,
            tile_indices: BTreeMap::new(),
            tile_vertex_counts: BTreeMap::new(),
            index_count: 0,
            vertex_count: 0,
        }
    }

    fn upload_tile(&mut self, queue: &wgpu::Queue, mesh: &FarTerrainLodTileMesh) -> usize {
        assert!(
            mesh.vertex_count() <= FAR_TERRAIN_LOD_TILE_VERTEX_CAPACITY,
            "far LOD tile {:?} exceeded region vertex slot capacity: {} > {}",
            mesh.key(),
            mesh.vertex_count(),
            FAR_TERRAIN_LOD_TILE_VERTEX_CAPACITY
        );
        assert!(
            mesh.index_count() <= FAR_TERRAIN_LOD_TILE_INDEX_CAPACITY,
            "far LOD tile {:?} exceeded region index slot capacity: {} > {}",
            mesh.key(),
            mesh.index_count(),
            FAR_TERRAIN_LOD_TILE_INDEX_CAPACITY
        );
        let slot = self.key.tile_slot(mesh.key());
        let vertex_base = slot * FAR_TERRAIN_LOD_TILE_VERTEX_CAPACITY;
        let origin = self.key.origin();
        let mut local_vertices = mesh.vertices().to_vec();
        for vertex in local_vertices.chunks_exact_mut(FAR_TERRAIN_LOD_VERTEX_FLOATS) {
            vertex[0] -= origin[0];
            vertex[2] -= origin[2];
        }
        let vertex_bytes = f32_bytes_vec(&local_vertices);
        queue.write_buffer(
            &self.vertex_buffer,
            vertex_base as wgpu::BufferAddress * FAR_TERRAIN_LOD_VERTEX_SIZE,
            &vertex_bytes,
        );
        self.tile_indices.insert(
            mesh.key(),
            mesh.indices()
                .iter()
                .map(|index| index.saturating_add(vertex_base as u32))
                .collect(),
        );
        self.tile_vertex_counts
            .insert(mesh.key(), mesh.vertex_count());
        vertex_bytes.len()
    }

    fn remove_tile(&mut self, tile: LodTileKey) {
        self.tile_indices.remove(&tile);
        self.tile_vertex_counts.remove(&tile);
    }

    fn repack_visible_indices(
        &mut self,
        queue: &wgpu::Queue,
        visible_tiles: &BTreeSet<LodTileKey>,
    ) -> usize {
        let mut indices = Vec::new();
        self.vertex_count = 0;
        for (tile, tile_indices) in &self.tile_indices {
            if visible_tiles.contains(tile) {
                indices.extend_from_slice(tile_indices);
                self.vertex_count += self
                    .tile_vertex_counts
                    .get(tile)
                    .copied()
                    .unwrap_or_default();
            }
        }
        self.index_count = u32::try_from(indices.len()).expect("far LOD region indices fit u32");
        if indices.is_empty() {
            return 0;
        }
        let bytes = u32_bytes_vec(&indices);
        queue.write_buffer(&self.index_buffer, 0, &bytes);
        bytes.len()
    }
}

pub struct FarTerrainLodRenderer {
    pipeline: wgpu::RenderPipeline,
    multiview_pipeline: Option<wgpu::RenderPipeline>,
    bind_group_layout: wgpu::BindGroupLayout,
    multiview_bind_group_layout: wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    regions: BTreeMap<FarTerrainLodRegionKey, FarTerrainLodRegionArena>,
    visible_tiles: BTreeSet<LodTileKey>,
    applied_revision: Option<u64>,
    uploaded_stats: FarTerrainLodRenderStats,
}

impl FarTerrainLodRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_far_terrain_lod_shader"),
            source: wgpu::ShaderSource::Wgsl(FAR_TERRAIN_LOD_WGSL.into()),
        });
        let layout_uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_far_terrain_lod_layout_uniforms",
            64,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_far_terrain_lod_bind_group_layout"),
            entries: &[layout_uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_far_terrain_lod_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let multiview_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_far_terrain_lod_multiview_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(FAR_TERRAIN_LOD_MULTIVIEW_UNIFORM_SIZE),
                    },
                    count: None,
                }],
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_far_terrain_lod_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: FAR_TERRAIN_LOD_VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            shader_location: 1,
                            offset: 12,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            multiview_pipeline: None,
            bind_group_layout,
            multiview_bind_group_layout,
            color_format,
            regions: BTreeMap::new(),
            visible_tiles: BTreeSet::new(),
            applied_revision: None,
            uploaded_stats: FarTerrainLodRenderStats::default(),
        }
    }

    pub fn render_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        render_views: [ChunkRenderView; 2],
        frame: Option<&FarTerrainLodFrameUpdate>,
    ) -> FarTerrainLodRenderStats {
        if self.multiview_pipeline.is_none() {
            self.multiview_pipeline = Some(create_far_lod_multiview_pipeline(
                device,
                &self.multiview_bind_group_layout,
                self.color_format,
            ));
        }
        self.sync_frame(device, queue, frame);
        let region_draw_count = self
            .regions
            .values()
            .filter(|region| region.index_count > 0)
            .count();
        if region_draw_count == 0 {
            return FarTerrainLodRenderStats::default();
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_far_terrain_lod_multiview_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(REVERSED_Z_DEPTH_CLEAR),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(
            self.multiview_pipeline
                .as_ref()
                .expect("far LOD multiview pipeline initialized"),
        );
        for region in self.regions.values() {
            if region.index_count == 0 {
                continue;
            }
            let translation = Mat4::from_translation(glam::Vec3::from_array(region.key.origin()));
            let matrices = [
                render_views[0].view_projection * translation,
                render_views[1].view_projection * translation,
            ];
            let mut bytes = [0; FAR_TERRAIN_LOD_MULTIVIEW_UNIFORM_SIZE as usize];
            bytes[..64].copy_from_slice(&matrix_bytes(matrices[0]));
            bytes[64..].copy_from_slice(&matrix_bytes(matrices[1]));
            queue.write_buffer(&region.multiview_uniform_buffer, 0, &bytes);
            pass.set_bind_group(0, &region.multiview_bind_group, &[]);
            pass.set_vertex_buffer(0, region.vertex_buffer.slice(..));
            pass.set_index_buffer(region.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..region.index_count, 0, 0..1);
        }
        self.uploaded_stats.region_draw_count = region_draw_count;
        self.uploaded_stats
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        frame: Option<&FarTerrainLodFrameUpdate>,
    ) -> FarTerrainLodRenderStats {
        self.render_in_slot(
            device,
            queue,
            encoder,
            target,
            depth,
            render_view,
            frame,
            SINGLE_VIEW_SLOT,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_in_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        depth: &ChunkDepthTarget,
        render_view: ChunkRenderView,
        frame: Option<&FarTerrainLodFrameUpdate>,
        view_slot: PerViewSlot,
    ) -> FarTerrainLodRenderStats {
        self.sync_frame(device, queue, frame);
        let region_draw_count = self
            .regions
            .values()
            .filter(|region| region.index_count > 0)
            .count();
        if region_draw_count == 0 {
            return FarTerrainLodRenderStats::default();
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_far_terrain_lod_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(REVERSED_Z_DEPTH_CLEAR),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        for region in self.regions.values_mut() {
            if region.index_count == 0 {
                continue;
            }
            let origin = region.key.origin();
            let matrix = render_view.view_projection
                * Mat4::from_translation(glam::Vec3::from_array(origin));
            let uniform_offset =
                region
                    .uniforms
                    .write_slot(queue, view_slot, &matrix_bytes(matrix));
            pass.set_bind_group(0, &region.bind_group, &[uniform_offset]);
            pass.set_vertex_buffer(0, region.vertex_buffer.slice(..));
            pass.set_index_buffer(region.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..region.index_count, 0, 0..1);
        }
        self.uploaded_stats.region_draw_count = region_draw_count;
        self.uploaded_stats
    }

    fn sync_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: Option<&FarTerrainLodFrameUpdate>,
    ) {
        let Some(frame) = frame else {
            self.regions.clear();
            self.visible_tiles.clear();
            self.applied_revision = None;
            self.uploaded_stats = FarTerrainLodRenderStats::default();
            return;
        };
        if self.applied_revision == Some(frame.revision) {
            self.uploaded_stats.uploaded_bytes = 0;
            return;
        }
        let mut dirty_regions = BTreeSet::new();
        for tile in &frame.removals {
            let region_key = FarTerrainLodRegionKey::for_tile(*tile);
            if let Some(region) = self.regions.get_mut(&region_key) {
                region.remove_tile(*tile);
                dirty_regions.insert(region_key);
            }
        }
        let mut uploaded_bytes = 0;
        for mesh in &frame.uploads {
            let region_key = FarTerrainLodRegionKey::for_tile(mesh.key());
            let region = self.regions.entry(region_key).or_insert_with(|| {
                FarTerrainLodRegionArena::new(
                    device,
                    &self.bind_group_layout,
                    &self.multiview_bind_group_layout,
                    region_key,
                )
            });
            uploaded_bytes += region.upload_tile(queue, mesh);
            dirty_regions.insert(region_key);
        }
        for tile in self
            .visible_tiles
            .symmetric_difference(&frame.visible_tiles)
        {
            dirty_regions.insert(FarTerrainLodRegionKey::for_tile(*tile));
        }
        self.visible_tiles = frame.visible_tiles.clone();
        for region_key in dirty_regions {
            if let Some(region) = self.regions.get_mut(&region_key) {
                uploaded_bytes += region.repack_visible_indices(queue, &self.visible_tiles);
            }
        }
        self.regions
            .retain(|_, region| !region.tile_indices.is_empty());
        let vertex_count = self
            .regions
            .values()
            .map(|region| region.vertex_count)
            .sum();
        let index_count = self
            .regions
            .values()
            .map(|region| region.index_count as usize)
            .sum();
        self.uploaded_stats = FarTerrainLodRenderStats {
            vertex_count,
            index_count,
            triangle_count: index_count / 3,
            region_draw_count: self
                .regions
                .values()
                .filter(|region| region.index_count > 0)
                .count(),
            uploaded_bytes,
        };
        self.applied_revision = Some(frame.revision);
    }
}

fn far_lod_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: FAR_TERRAIN_LOD_VERTEX_SIZE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                shader_location: 0,
                offset: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                shader_location: 1,
                offset: 12,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    }
}

fn create_far_lod_multiview_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mclone_far_terrain_lod_multiview_shader"),
        source: wgpu::ShaderSource::Wgsl(FAR_TERRAIN_LOD_MULTIVIEW_WGSL.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mclone_far_terrain_lod_multiview_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mclone_far_terrain_lod_multiview_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            buffers: &[far_lod_vertex_buffer_layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview: NonZeroU32::new(2),
        cache: None,
    })
}

fn f32_bytes_vec(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn u32_bytes_vec(values: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn matrix_bytes(matrix: Mat4) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, value) in matrix.to_cols_array().into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn far_terrain_lod_tile_mesh_counts_owned_payload() {
        let mesh = FarTerrainLodTileMesh::new(
            LodTileKey::synthetic(mclone_core::ChunkPos::new(1, -1)),
            vec![
                0.0, 64.0, 0.0, 0.2, 0.8, 0.2, 1.0, 1.0, 64.0, 0.0, 0.2, 0.8, 0.2, 1.0, 0.0, 64.0,
                1.0, 0.2, 0.8, 0.2, 1.0,
            ],
            vec![0, 1, 2],
        );
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.index_count(), 3);
        assert_eq!(mesh.estimated_owned_bytes(), 3 * 7 * 4 + 3 * 4);
        assert_eq!(FarTerrainLodRegionKey::for_tile(mesh.key()).x, 0);
        assert_eq!(FarTerrainLodRegionKey::for_tile(mesh.key()).z, -1);
    }
}
