use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::size_of;
use std::num::NonZeroU64;
use std::sync::mpsc;

use mclone_render_color::{RenderTargetColorTransform, color_transform_wgpu};
use mclone_worldgen::levelgen::{
    McloneOverworldSamplingTopology, McloneOverworldVegetationPlanCache, McloneTreeFamily,
    McloneVegetationSource,
};
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING,
    TerrainPreviewComparison, TerrainPreviewCompileWork, TerrainPreviewContentStage,
    TerrainPreviewProfile, TerrainPreviewReferenceGrid, TerrainPreviewSample,
    TerrainPreviewSurfaceQuality, TerrainPreviewVegetationProduct, ValidatedTerrainPreviewRequest,
    terrain_preview_gpu_compile_work,
};

use super::{
    TERRAIN_PREVIEW_DEPTH_FORMAT, TERRAIN_PREVIEW_SAMPLE_BYTES, TERRAIN_PREVIEW_UNIFORM_BYTES,
    TERRAIN_PREVIEW_WORKGROUP_AXIS, TerrainClipmap, TerrainClipmapConfig,
    TerrainClipmapDiagnostics, TerrainClipmapTile, TerrainHorizonPresentation,
    TerrainPreviewCamera, TerrainPreviewDrawOptions, TerrainPreviewLayer, TerrainPreviewSource,
    TerrainPreviewSplitLayout, TerrainViewportPlan, TerrainViewportTileId, parse_samples,
    terrain_horizon_orbit_target_y, terrain_preview_compute_wgsl,
    terrain_preview_focus_y_for_profile, terrain_preview_render_wgsl, terrain_preview_tree_wgsl,
    viewport_uniform_bytes_for_request, viewport_uniform_bytes_for_request_with_presentation,
};

pub const TERRAIN_PREVIEW_MATERIAL_UV_COUNT: usize = 256;

#[derive(Clone, Copy, Debug)]
pub struct TerrainPreviewMaterialAtlas<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub material_uvs: &'a [[f32; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
}

pub const TERRAIN_VIEWPORT_MAX_RESIDENT_TILES: usize = 192;
pub const TERRAIN_VIEWPORT_TILE_COMPILES_PER_FRAME: usize = 4;
pub const TERRAIN_VIEWPORT_GPU_DISPATCHES_PER_FRAME: usize = 16;
pub const TERRAIN_HORIZON_VEGETATION_COMPILES_PER_FRAME: usize = 1;
pub const TERRAIN_VIEWPORT_CPU_COMPILE_BUDGET_MICROS: u64 = 8_000;
const TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS: usize = 12;
const TERRAIN_PREVIEW_TREE_INSTANCE_BYTES: u64 =
    (TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS * size_of::<f32>()) as u64;
const TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE: u32 = 108;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainViewportFrameStats {
    pub revision: u64,
    pub requested_spacing: u32,
    pub effective_spacing: u32,
    pub published_spacing: u32,
    pub cpu_published_spacing: u32,
    pub gpu_published_spacing: u32,
    pub view_width_blocks: u32,
    pub view_height_blocks: u32,
    pub level_count: u32,
    pub visible_tile_count: u32,
    pub published_tile_count: u32,
    pub cpu_published_tile_count: u32,
    pub gpu_published_tile_count: u32,
    pub resident_tile_count: u32,
    pub queued_tile_count: u32,
    pub cpu_queued_tile_count: u32,
    pub gpu_queued_tile_count: u32,
    pub pending_readback_count: u32,
    pub cpu_compiled_tiles: u32,
    pub cpu_compiled_tiles_total: u64,
    pub gpu_dispatched_tiles: u32,
    pub gpu_dispatched_tiles_total: u64,
    pub macro_compiled_tiles_total: u64,
    pub request_cpu_compiled_tiles: u32,
    pub request_gpu_dispatched_tiles: u32,
    pub request_macro_compiled_tiles: u32,
    pub request_cache_hit_tiles: u32,
    pub request_cpu_compile_work: TerrainPreviewCompileWork,
    pub request_gpu_compile_work: TerrainPreviewCompileWork,
    pub evicted_tiles_total: u64,
    pub stale_result_count: u64,
    pub sample_count: u32,
    pub vertex_count: u32,
    pub vegetation_summary_tile_count: u32,
    pub cpu_vegetation_summary_tile_count: u32,
    pub gpu_vegetation_summary_tile_count: u32,
    pub vegetation_record_tile_count: u32,
    pub vegetation_aggregated_tile_count: u32,
    pub tree_instance_count: u32,
    pub tree_instance_bytes: u64,
    pub tree_proxy_vertex_count: u32,
    pub vegetation_cell_requests: u64,
    pub vegetation_cell_hits: u64,
    pub vegetation_cell_misses: u64,
    pub retained_vegetation_cells: u32,
    pub reference_bytes: u64,
    pub gpu_sample_bytes: u64,
    pub readback_bytes: u64,
    pub request_readback_bytes: u64,
    pub resident_bytes: u64,
    pub cpu_reference_micros: u64,
    pub cpu_vegetation_micros: u64,
    pub cpu_pack_upload_micros: u64,
    pub request_cpu_reference_micros: u64,
    pub request_cpu_vegetation_micros: u64,
    pub request_cpu_pack_upload_micros: u64,
    pub request_macro_compile_micros: u64,
    pub cpu_coarse_ready: bool,
    pub cpu_target_ready: bool,
    pub gpu_coarse_ready: bool,
    pub gpu_target_ready: bool,
    pub coarse_ready: bool,
    pub target_ready: bool,
    pub cache_enabled: bool,
    pub budget_limited: bool,
    pub needs_redraw: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainViewportCompletedComparison {
    pub revision: u64,
    pub sample_spacing: u32,
    pub tile_count: u32,
    pub comparison: TerrainPreviewComparison,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainViewportExternalCpuRequest {
    pub revision: u64,
    pub tile: TerrainViewportTileId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainHorizonFrameStats {
    pub revision: u64,
    pub allocation_slots: u32,
    pub ready_slots: u32,
    pub pending_refills: u32,
    pub dispatched_refills: u32,
    pub dispatched_refills_total: u64,
    pub drawn_levels: u32,
    pub drawn_tiles: u32,
    pub vertex_count: u32,
    pub vegetation_ready_tiles: u32,
    pub pending_vegetation_tiles: u32,
    pub tree_instance_count: u32,
    pub tree_proxy_vertex_count: u32,
    pub fixed_resident_bytes: u64,
    pub vegetation_bytes: u64,
    pub resident_bytes: u64,
    pub finest_sample_spacing: u32,
    pub coarse_ready: bool,
    pub target_ready: bool,
    pub needs_redraw: bool,
    pub residency: TerrainClipmapDiagnostics,
}

struct EncodedTerrainViewportReadbacks {
    readbacks: Vec<EncodedTerrainViewportReadback>,
}

struct EncodedTerrainViewportReadback {
    cache_epoch: u64,
    tile: TerrainViewportTileId,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
}

struct PendingTerrainViewportReadback {
    cache_epoch: u64,
    tile: TerrainViewportTileId,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

struct TerrainViewportDepthTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl TerrainViewportDepthTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_viewport_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TERRAIN_PREVIEW_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }
}

struct TerrainPreviewMaterialResources {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    _uv_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl TerrainPreviewMaterialResources {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: TerrainPreviewMaterialAtlas<'_>,
    ) -> Result<Self, String> {
        let width = atlas.width.max(1);
        let height = atlas.height.max(1);
        let expected_len = width as usize * height as usize * 4;
        if atlas.rgba.len() != expected_len {
            return Err(format!(
                "terrain preview material atlas has {} bytes; expected {expected_len} for \
                 {width}x{height} RGBA",
                atlas.rgba.len()
            ));
        }
        let mip_levels = generate_material_mips(width, height, atlas.rgba, 5);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_preview_material_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (mip_level, mip) in mip_levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip_level as u32,
                    origin: Default::default(),
                    aspect: Default::default(),
                },
                &mip.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.width * 4),
                    rows_per_image: Some(mip.height),
                },
                wgpu::Extent3d {
                    width: mip.width,
                    height: mip.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_terrain_preview_material_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: mip_levels.len().saturating_sub(1) as f32,
            ..Default::default()
        });
        let mut uv_bytes =
            Vec::with_capacity(TERRAIN_PREVIEW_MATERIAL_UV_COUNT * 4 * size_of::<f32>());
        for rect in atlas.material_uvs {
            for value in rect {
                uv_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let uv_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_material_uvs"),
            size: uv_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uv_buffer, 0, &uv_bytes);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_preview_material_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uv_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            _uv_buffer: uv_buffer,
            bind_group,
        })
    }
}

struct TerrainPreviewMaterialMip {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn generate_material_mips(
    width: u32,
    height: u32,
    rgba: &[u8],
    max_level_count: usize,
) -> Vec<TerrainPreviewMaterialMip> {
    let mut levels = vec![TerrainPreviewMaterialMip {
        width,
        height,
        rgba: rgba.to_vec(),
    }];
    while levels.len() < max_level_count
        && levels
            .last()
            .is_some_and(|level| level.width > 1 || level.height > 1)
    {
        let previous = levels.last().expect("base material mip exists");
        let next_width = (previous.width / 2).max(1);
        let next_height = (previous.height / 2).max(1);
        let mut next_rgba = vec![0_u8; next_width as usize * next_height as usize * 4];
        for y in 0..next_height {
            for x in 0..next_width {
                let mut sum = [0_u32; 4];
                let mut count = 0_u32;
                for source_y in (y * 2)..(y * 2 + 2).min(previous.height) {
                    for source_x in (x * 2)..(x * 2 + 2).min(previous.width) {
                        let source = ((source_y * previous.width + source_x) * 4) as usize;
                        for channel in 0..4 {
                            sum[channel] += u32::from(previous.rgba[source + channel]);
                        }
                        count += 1;
                    }
                }
                let destination = ((y * next_width + x) * 4) as usize;
                for channel in 0..4 {
                    next_rgba[destination + channel] = (sum[channel] / count) as u8;
                }
            }
        }
        levels.push(TerrainPreviewMaterialMip {
            width: next_width,
            height: next_height,
            rgba: next_rgba,
        });
    }
    levels
}

struct TerrainViewportGpuTile {
    request: ValidatedTerrainPreviewRequest,
    reference: Option<TerrainPreviewReferenceGrid>,
    vegetation: Option<TerrainPreviewVegetationProduct>,
    gpu_samples: Option<Vec<TerrainPreviewSample>>,
    gpu_submitted: bool,
    uniform_buffer: wgpu::Buffer,
    gpu_sample_buffer: wgpu::Buffer,
    reference_sample_buffer: Option<wgpu::Buffer>,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    tree_instance_buffer: Option<wgpu::Buffer>,
    tree_instance_count: u32,
    tree_instance_bytes: u64,
    last_used: u64,
}

impl TerrainViewportGpuTile {
    fn new(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        tile_id: TerrainViewportTileId,
    ) -> Result<Self, String> {
        Self::new_with_reference_buffer(
            device,
            compute_layout,
            render_layout,
            sample_byte_len,
            tile_id,
            true,
        )
    }

    fn new_gpu_only(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        tile_id: TerrainViewportTileId,
    ) -> Result<Self, String> {
        Self::new_with_reference_buffer(
            device,
            compute_layout,
            render_layout,
            sample_byte_len,
            tile_id,
            false,
        )
    }

    fn new_with_reference_buffer(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        tile_id: TerrainViewportTileId,
        allocate_reference_buffer: bool,
    ) -> Result<Self, String> {
        let request = tile_id.preview_request().validate()?;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_tile_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_gpu_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let reference_sample_buffer = allocate_reference_buffer.then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_terrain_viewport_reference_samples"),
                size: sample_byte_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_viewport_compute_bind_group"),
            layout: compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
            ],
        });
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_viewport_render_bind_group"),
            layout: render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: reference_sample_buffer
                        .as_ref()
                        .unwrap_or(&gpu_sample_buffer)
                        .as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            request,
            reference: None,
            vegetation: None,
            gpu_samples: None,
            gpu_submitted: false,
            uniform_buffer,
            gpu_sample_buffer,
            reference_sample_buffer,
            compute_bind_group,
            render_bind_group,
            tree_instance_buffer: None,
            tree_instance_count: 0,
            tree_instance_bytes: 0,
            last_used: 0,
        })
    }

    fn upload_reference(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sample_byte_len: u64,
        reference: TerrainPreviewReferenceGrid,
        vegetation: TerrainPreviewVegetationProduct,
    ) -> Result<(), String> {
        let reference_bytes = reference.packed_bytes();
        if reference_bytes.len() as u64 != sample_byte_len {
            return Err(format!(
                "terrain viewport reference upload is {} bytes, expected {}",
                reference_bytes.len(),
                sample_byte_len
            ));
        }
        let reference_sample_buffer = self
            .reference_sample_buffer
            .as_ref()
            .ok_or("terrain viewport reference upload requires a reference buffer")?;
        queue.write_buffer(reference_sample_buffer, 0, &reference_bytes);
        self.upload_vegetation(device, queue, vegetation)?;
        self.reference = Some(reference);
        Ok(())
    }

    fn upload_vegetation(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vegetation: TerrainPreviewVegetationProduct,
    ) -> Result<(), String> {
        let (tree_bytes, tree_instance_count) = tree_instance_bytes(&vegetation)?;
        self.tree_instance_buffer = if tree_bytes.is_empty() {
            None
        } else {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_terrain_viewport_tree_instances"),
                size: tree_bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &tree_bytes);
            Some(buffer)
        };
        self.tree_instance_count = tree_instance_count;
        self.tree_instance_bytes = tree_bytes.len() as u64;
        self.vegetation = Some(vegetation);
        Ok(())
    }

    fn clear_vegetation(&mut self) {
        self.vegetation = None;
        self.tree_instance_buffer = None;
        self.tree_instance_count = 0;
        self.tree_instance_bytes = 0;
    }

    fn upload_macro(
        &mut self,
        queue: &wgpu::Queue,
        sample_byte_len: u64,
        macro_grid: TerrainPreviewReferenceGrid,
    ) -> Result<(), String> {
        let bytes = macro_grid.packed_bytes();
        if bytes.len() as u64 != sample_byte_len {
            return Err(format!(
                "terrain viewport macro upload is {} bytes, expected {}",
                bytes.len(),
                sample_byte_len
            ));
        }
        queue.write_buffer(&self.gpu_sample_buffer, 0, &bytes);
        self.gpu_samples = Some(macro_grid.samples().to_vec());
        self.gpu_submitted = true;
        Ok(())
    }
}

pub struct TerrainViewportRenderer {
    sample_count_per_tile: u32,
    sample_byte_len: u64,
    compute_layout: wgpu::BindGroupLayout,
    render_layout: wgpu::BindGroupLayout,
    _material_resources: TerrainPreviewMaterialResources,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    tree_pipeline: wgpu::RenderPipeline,
    clear_color: wgpu::Color,
    depth: TerrainViewportDepthTarget,
    depth_capture_enabled: bool,
    cache: HashMap<TerrainViewportTileId, TerrainViewportGpuTile>,
    vegetation_cache: Option<McloneOverworldVegetationPlanCache>,
    pending: Vec<PendingTerrainViewportReadback>,
    plan: Option<TerrainViewportPlan>,
    cpu_queue: VecDeque<TerrainViewportTileId>,
    external_cpu_inflight: HashMap<TerrainViewportTileId, u64>,
    external_macro_inflight: HashMap<TerrainViewportTileId, u64>,
    gpu_queue: VecDeque<TerrainViewportTileId>,
    cpu_published_level: Option<usize>,
    gpu_published_level: Option<usize>,
    cache_enabled: bool,
    cache_epoch: u64,
    latest_revision: u64,
    last_comparison_revision: Option<u64>,
    use_clock: u64,
    cpu_compiled_tiles_total: u64,
    gpu_dispatched_tiles_total: u64,
    macro_compiled_tiles_total: u64,
    request_cpu_compiled_tiles: u32,
    request_gpu_dispatched_tiles: u32,
    request_macro_compiled_tiles: u32,
    request_cache_hit_tiles: u32,
    request_cpu_compile_work: TerrainPreviewCompileWork,
    request_gpu_compile_work: TerrainPreviewCompileWork,
    request_cpu_reference_micros: u64,
    request_cpu_vegetation_micros: u64,
    request_cpu_pack_upload_micros: u64,
    request_macro_compile_micros: u64,
    request_vegetation_cell_requests: u64,
    request_vegetation_cell_hits: u64,
    request_vegetation_cell_misses: u64,
    evicted_tiles_total: u64,
    stale_result_count: u64,
    focus_request: Option<(TerrainPreviewProfile, i64, i32, i32)>,
    focus_y: f32,
}

impl TerrainViewportRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
    ) -> Result<Self, String> {
        Self::new_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            RenderTargetColorTransform::Identity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_target_color_transform(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self, String> {
        let samples_per_axis = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1;
        let sample_count_per_tile = samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain viewport sample count overflow")?;
        let sample_byte_len = u64::from(sample_count_per_tile)
            .checked_mul(TERRAIN_PREVIEW_SAMPLE_BYTES)
            .ok_or("terrain viewport sample buffer size overflow")?;
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_compute_layout"),
            entries: &[
                uniform_layout_entry(0, wgpu::ShaderStages::COMPUTE),
                storage_layout_entry(1, wgpu::ShaderStages::COMPUTE, false, sample_byte_len),
            ],
        });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_render_layout"),
            entries: &[
                uniform_layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_layout_entry(
                    1,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    sample_byte_len,
                ),
                storage_layout_entry(
                    2,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    sample_byte_len,
                ),
            ],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_material_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                uniform_layout_entry_with_size(
                    2,
                    wgpu::ShaderStages::FRAGMENT,
                    (TERRAIN_PREVIEW_MATERIAL_UV_COUNT * 4 * size_of::<f32>()) as u64,
                ),
            ],
        });
        let material_resources =
            TerrainPreviewMaterialResources::new(device, queue, &material_layout, material_atlas)?;
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(terrain_preview_compute_wgsl().into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_render_shader"),
            source: wgpu::ShaderSource::Wgsl(
                terrain_preview_render_wgsl(target_color_transform).into(),
            ),
        });
        let tree_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_tree_shader"),
            source: wgpu::ShaderSource::Wgsl(
                terrain_preview_tree_wgsl(target_color_transform).into(),
            ),
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_viewport_compute_pipeline_layout"),
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_viewport_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout, &material_layout],
                push_constant_ranges: &[],
            });
        let tree_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_terrain_viewport_tree_pipeline_layout"),
            bind_group_layouts: &[&render_layout],
            push_constant_ranges: &[],
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mclone_terrain_viewport_compute_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("compute_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_terrain_viewport_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
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
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let tree_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_terrain_viewport_tree_pipeline"),
            layout: Some(&tree_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tree_shader,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: TERRAIN_PREVIEW_TREE_INSTANCE_BYTES,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &tree_shader,
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
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Ok(Self {
            sample_count_per_tile,
            sample_byte_len,
            compute_layout,
            render_layout,
            _material_resources: material_resources,
            compute_pipeline,
            render_pipeline,
            tree_pipeline,
            clear_color: color_transform_wgpu(
                wgpu::Color {
                    r: 0.025,
                    g: 0.035,
                    b: 0.055,
                    a: 1.0,
                },
                target_color_transform,
            ),
            depth: TerrainViewportDepthTarget::new(device, width, height),
            depth_capture_enabled: false,
            cache: HashMap::new(),
            vegetation_cache: None,
            pending: Vec::new(),
            plan: None,
            cpu_queue: VecDeque::new(),
            external_cpu_inflight: HashMap::new(),
            external_macro_inflight: HashMap::new(),
            gpu_queue: VecDeque::new(),
            cpu_published_level: None,
            gpu_published_level: None,
            cache_enabled: true,
            cache_epoch: 0,
            latest_revision: 0,
            last_comparison_revision: None,
            use_clock: 0,
            cpu_compiled_tiles_total: 0,
            gpu_dispatched_tiles_total: 0,
            macro_compiled_tiles_total: 0,
            request_cpu_compiled_tiles: 0,
            request_gpu_dispatched_tiles: 0,
            request_macro_compiled_tiles: 0,
            request_cache_hit_tiles: 0,
            request_cpu_compile_work: TerrainPreviewCompileWork::default(),
            request_gpu_compile_work: TerrainPreviewCompileWork::default(),
            request_cpu_reference_micros: 0,
            request_cpu_vegetation_micros: 0,
            request_cpu_pack_upload_micros: 0,
            request_macro_compile_micros: 0,
            request_vegetation_cell_requests: 0,
            request_vegetation_cell_hits: 0,
            request_vegetation_cell_misses: 0,
            evicted_tiles_total: 0,
            stale_result_count: 0,
            focus_request: None,
            focus_y: 64.0,
        })
    }

    pub fn set_cache_enabled(&mut self, enabled: bool) {
        if self.cache_enabled == enabled {
            return;
        }
        self.cache_enabled = enabled;
        if !enabled {
            self.clear_cache();
        }
    }

    pub fn clear_cache(&mut self) {
        self.stale_result_count = self.stale_result_count.saturating_add(
            (self.cpu_queue.len()
                + self.external_cpu_inflight.len()
                + self.gpu_queue.len()
                + self.external_macro_inflight.len()) as u64,
        );
        self.cache.clear();
        self.vegetation_cache = None;
        self.cpu_queue.clear();
        self.external_cpu_inflight.clear();
        self.external_macro_inflight.clear();
        self.gpu_queue.clear();
        self.cpu_published_level = None;
        self.gpu_published_level = None;
        self.cache_epoch = self.cache_epoch.saturating_add(1);
        self.last_comparison_revision = None;
        self.reset_request_counters();
        self.rebuild_queues();
    }

    pub fn set_viewport(&mut self, revision: u64, plan: TerrainViewportPlan) {
        self.latest_revision = revision;
        let focus_request = (
            plan.request.profile,
            plan.request.seed,
            plan.request.center_x,
            plan.request.center_z,
        );
        if self.focus_request != Some(focus_request) {
            self.focus_request = Some(focus_request);
            self.focus_y = terrain_preview_focus_y_for_profile(
                focus_request.0,
                focus_request.1,
                focus_request.2,
                focus_request.3,
            );
        }
        if self.plan.as_ref() == Some(&plan) {
            return;
        }
        if self.cache_enabled {
            self.stale_result_count = self.stale_result_count.saturating_add(
                (self.cpu_queue.len()
                    + self.external_cpu_inflight.len()
                    + self.gpu_queue.len()
                    + self.external_macro_inflight.len()) as u64,
            );
            self.cpu_queue.clear();
            self.external_cpu_inflight.clear();
            self.external_macro_inflight.clear();
            self.gpu_queue.clear();
        } else {
            self.cache.clear();
            self.vegetation_cache = None;
            self.cpu_queue.clear();
            self.external_cpu_inflight.clear();
            self.external_macro_inflight.clear();
            self.gpu_queue.clear();
            self.cache_epoch = self.cache_epoch.saturating_add(1);
        }
        self.cpu_published_level = None;
        self.gpu_published_level = None;
        self.last_comparison_revision = None;
        self.plan = Some(plan);
        self.reset_request_counters();
        self.request_cache_hit_tiles = self.current_plan_cache_hits();
        self.rebuild_queues();
        self.refresh_published_levels();
        self.touch_current_tiles();
        self.evict_unused_tiles();
    }

    pub const fn stale_result_count(&self) -> u64 {
        self.stale_result_count
    }

    pub fn take_external_cpu_request(&mut self) -> Option<TerrainViewportExternalCpuRequest> {
        let plan = self.plan.as_ref()?;
        if plan.request.profile != TerrainPreviewProfile::VanillaOverworld {
            return None;
        }
        let tile = self.take_next_missing_cpu_tile()?;
        self.external_cpu_inflight
            .insert(tile, self.latest_revision);
        Some(TerrainViewportExternalCpuRequest {
            revision: self.latest_revision,
            tile,
        })
    }

    pub fn accept_external_cpu_tile(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: TerrainViewportExternalCpuRequest,
        reference: TerrainPreviewReferenceGrid,
        compile_micros: u64,
    ) -> Result<bool, String> {
        let active = self.external_cpu_inflight.get(&request.tile).copied();
        if active != Some(request.revision) || !self.plan_contains_tile(request.tile) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return Ok(false);
        }
        self.external_cpu_inflight.remove(&request.tile);
        if reference.request().request() != request.tile.preview_request() {
            return Err("external terrain preview grid does not match its tile request".to_owned());
        }
        let compile_work = reference.compile_work();
        let vegetation = TerrainPreviewVegetationProduct::compile(request.tile.preview_request())?;
        if !self.cache.contains_key(&request.tile) {
            let tile = TerrainViewportGpuTile::new(
                device,
                &self.compute_layout,
                &self.render_layout,
                self.sample_byte_len,
                request.tile,
            )?;
            self.cache.insert(request.tile, tile);
        }
        let tile = self
            .cache
            .get_mut(&request.tile)
            .expect("created external terrain viewport tile is resident");
        tile.upload_reference(device, queue, self.sample_byte_len, reference, vegetation)?;
        self.use_clock = self.use_clock.saturating_add(1);
        tile.last_used = self.use_clock;
        self.cpu_compiled_tiles_total = self.cpu_compiled_tiles_total.saturating_add(1);
        self.request_cpu_compiled_tiles = self.request_cpu_compiled_tiles.saturating_add(1);
        self.request_cpu_compile_work = self.request_cpu_compile_work.saturating_add(compile_work);
        self.request_cpu_reference_micros = self
            .request_cpu_reference_micros
            .saturating_add(compile_micros);
        self.refresh_published_levels();
        self.touch_current_tiles();
        self.evict_unused_tiles();
        Ok(true)
    }

    pub fn reject_external_cpu_tile(&mut self, request: TerrainViewportExternalCpuRequest) {
        if self.external_cpu_inflight.get(&request.tile).copied() != Some(request.revision) {
            return;
        }
        self.external_cpu_inflight.remove(&request.tile);
        if self.plan_contains_tile(request.tile) {
            self.cpu_queue.push_front(request.tile);
        }
    }

    pub fn take_external_macro_request(&mut self) -> Option<TerrainViewportExternalCpuRequest> {
        let plan = self.plan.as_ref()?;
        if plan.request.profile != TerrainPreviewProfile::VanillaOverworld {
            return None;
        }
        let tile = self.take_next_missing_gpu_tile()?;
        self.external_macro_inflight
            .insert(tile, self.latest_revision);
        Some(TerrainViewportExternalCpuRequest {
            revision: self.latest_revision,
            tile,
        })
    }

    pub fn accept_external_macro_tile(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: TerrainViewportExternalCpuRequest,
        macro_grid: TerrainPreviewReferenceGrid,
        compile_micros: u64,
    ) -> Result<bool, String> {
        let active = self.external_macro_inflight.get(&request.tile).copied();
        if active != Some(request.revision) || !self.plan_contains_tile(request.tile) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return Ok(false);
        }
        self.external_macro_inflight.remove(&request.tile);
        if macro_grid.request().request() != request.tile.preview_request() {
            return Err("external terrain macro grid does not match its tile request".to_owned());
        }
        if !self.cache.contains_key(&request.tile) {
            let tile = TerrainViewportGpuTile::new(
                device,
                &self.compute_layout,
                &self.render_layout,
                self.sample_byte_len,
                request.tile,
            )?;
            self.cache.insert(request.tile, tile);
        }
        let tile = self
            .cache
            .get_mut(&request.tile)
            .expect("created external terrain macro tile is resident");
        tile.upload_macro(queue, self.sample_byte_len, macro_grid)?;
        self.use_clock = self.use_clock.saturating_add(1);
        tile.last_used = self.use_clock;
        self.macro_compiled_tiles_total = self.macro_compiled_tiles_total.saturating_add(1);
        self.request_macro_compiled_tiles = self.request_macro_compiled_tiles.saturating_add(1);
        self.request_macro_compile_micros = self
            .request_macro_compile_micros
            .saturating_add(compile_micros);
        self.refresh_published_levels();
        self.touch_current_tiles();
        self.evict_unused_tiles();
        Ok(true)
    }

    pub fn reject_external_macro_tile(&mut self, request: TerrainViewportExternalCpuRequest) {
        if self.external_macro_inflight.get(&request.tile).copied() != Some(request.revision) {
            return;
        }
        self.external_macro_inflight.remove(&request.tile);
        if self.plan_contains_tile(request.tile) {
            self.gpu_queue.push_front(request.tile);
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth.width != width || self.depth.height != height {
            self.depth = TerrainViewportDepthTarget::new(device, width, height);
        }
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        let unpadded_row_bytes = self
            .depth
            .width
            .checked_mul(size_of::<f32>() as u32)
            .ok_or("terrain viewport depth row byte length overflow")?;
        if bytes_per_row < unpadded_row_bytes
            || !bytes_per_row.is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        {
            return Err(format!(
                "terrain viewport depth copy row length must be at least \
                 {unpadded_row_bytes} bytes and {}-byte aligned, got {bytes_per_row}",
                wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            ));
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.depth.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: destination,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.depth.height),
                },
            },
            wgpu::Extent3d {
                width: self.depth.width,
                height: self.depth.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.depth_capture_enabled = enabled;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        options: TerrainPreviewDrawOptions,
        camera: TerrainPreviewCamera,
        mut clock_ms: impl FnMut() -> f64,
    ) -> Result<TerrainViewportFrameStats, String> {
        self.resize(device, width, height);
        let plan = self
            .plan
            .clone()
            .ok_or("terrain viewport renderer has no active plan")?;
        let cpu_required =
            source_needs_cpu(options, plan.request.content_stage, plan.effective_spacing);
        let gpu_required = source_needs_gpu(options, plan.request.profile);
        let focus_y = self.focus_y;
        let mut encoded_readbacks = Vec::new();
        let mut gpu_dispatched_tiles = 0_u32;

        if gpu_required && plan.request.profile.supports_gpu_lod() {
            let mut compute_encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_terrain_viewport_compute_encoder"),
                });
            for _ in 0..TERRAIN_VIEWPORT_GPU_DISPATCHES_PER_FRAME {
                let Some(tile_id) = self.take_next_missing_gpu_tile() else {
                    break;
                };
                if !self.cache.contains_key(&tile_id) {
                    let tile = TerrainViewportGpuTile::new(
                        device,
                        &self.compute_layout,
                        &self.render_layout,
                        self.sample_byte_len,
                        tile_id,
                    )?;
                    self.cache.insert(tile_id, tile);
                }
                let tile = self
                    .cache
                    .get_mut(&tile_id)
                    .expect("created terrain viewport GPU tile is resident");
                queue.write_buffer(
                    &tile.uniform_buffer,
                    0,
                    &viewport_uniform_bytes_for_request(
                        tile.request,
                        width,
                        height,
                        options,
                        camera,
                        plan.request.center_x,
                        plan.request.center_z,
                        plan.view_width_blocks,
                        plan.view_height_blocks,
                        focus_y,
                    ),
                );
                {
                    let mut pass =
                        compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("mclone_terrain_viewport_compute_pass"),
                            timestamp_writes: None,
                        });
                    pass.set_pipeline(&self.compute_pipeline);
                    pass.set_bind_group(0, &tile.compute_bind_group, &[]);
                    let workgroups = (TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1)
                        .div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
                    pass.dispatch_workgroups(workgroups, workgroups, 1);
                }
                let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("mclone_terrain_viewport_readback"),
                    size: self.sample_byte_len,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                compute_encoder.copy_buffer_to_buffer(
                    &tile.gpu_sample_buffer,
                    0,
                    &readback_buffer,
                    0,
                    self.sample_byte_len,
                );
                tile.gpu_submitted = true;
                self.use_clock = self.use_clock.saturating_add(1);
                tile.last_used = self.use_clock;
                encoded_readbacks.push(EncodedTerrainViewportReadback {
                    cache_epoch: self.cache_epoch,
                    tile: tile_id,
                    readback_buffer,
                    byte_len: self.sample_byte_len,
                });
                gpu_dispatched_tiles = gpu_dispatched_tiles.saturating_add(1);
                self.gpu_dispatched_tiles_total = self.gpu_dispatched_tiles_total.saturating_add(1);
                self.request_gpu_dispatched_tiles =
                    self.request_gpu_dispatched_tiles.saturating_add(1);
                self.request_gpu_compile_work = self
                    .request_gpu_compile_work
                    .saturating_add(terrain_preview_gpu_compile_work(tile.request));
            }
            if !encoded_readbacks.is_empty() {
                queue.submit(std::iter::once(compute_encoder.finish()));
                self.mark_submitted(EncodedTerrainViewportReadbacks {
                    readbacks: encoded_readbacks,
                });
            }
        }

        let mut cpu_compiled_tiles = 0_u32;
        let mut cpu_reference_micros = 0_u64;
        let mut cpu_vegetation_micros = 0_u64;
        let mut cpu_pack_upload_micros = 0_u64;

        if cpu_required && plan.request.profile != TerrainPreviewProfile::VanillaOverworld {
            for _ in 0..TERRAIN_VIEWPORT_TILE_COMPILES_PER_FRAME {
                let frame_cpu_micros = cpu_reference_micros
                    .saturating_add(cpu_vegetation_micros)
                    .saturating_add(cpu_pack_upload_micros);
                if cpu_compiled_tiles > 0
                    && frame_cpu_micros >= TERRAIN_VIEWPORT_CPU_COMPILE_BUDGET_MICROS
                {
                    break;
                }
                let Some(tile_id) = self.take_next_missing_cpu_tile() else {
                    break;
                };
                let reference_started = clock_ms();
                let reference = TerrainPreviewReferenceGrid::compile(tile_id.preview_request())?;
                let compile_work = reference.compile_work();
                let reference_micros =
                    ((clock_ms() - reference_started).max(0.0) * 1_000.0).round();
                cpu_reference_micros = cpu_reference_micros
                    .saturating_add(reference_micros.min(u64::MAX as f64) as u64);
                let vegetation_started = clock_ms();
                let vegetation = self.compile_vegetation_product(tile_id)?;
                let vegetation_report = vegetation.cache_report();
                let vegetation_micros =
                    ((clock_ms() - vegetation_started).max(0.0) * 1_000.0).round();
                cpu_vegetation_micros = cpu_vegetation_micros
                    .saturating_add(vegetation_micros.min(u64::MAX as f64) as u64);
                if !self.cache.contains_key(&tile_id) {
                    let tile = TerrainViewportGpuTile::new(
                        device,
                        &self.compute_layout,
                        &self.render_layout,
                        self.sample_byte_len,
                        tile_id,
                    )?;
                    self.cache.insert(tile_id, tile);
                }
                let tile = self
                    .cache
                    .get_mut(&tile_id)
                    .expect("created terrain viewport CPU tile is resident");
                let upload_started = clock_ms();
                tile.upload_reference(device, queue, self.sample_byte_len, reference, vegetation)?;
                let upload_micros = ((clock_ms() - upload_started).max(0.0) * 1_000.0).round();
                cpu_pack_upload_micros = cpu_pack_upload_micros
                    .saturating_add(upload_micros.min(u64::MAX as f64) as u64);
                self.use_clock = self.use_clock.saturating_add(1);
                tile.last_used = self.use_clock;
                cpu_compiled_tiles = cpu_compiled_tiles.saturating_add(1);
                self.cpu_compiled_tiles_total = self.cpu_compiled_tiles_total.saturating_add(1);
                self.request_cpu_compiled_tiles = self.request_cpu_compiled_tiles.saturating_add(1);
                self.request_cpu_compile_work =
                    self.request_cpu_compile_work.saturating_add(compile_work);
                self.request_vegetation_cell_requests = self
                    .request_vegetation_cell_requests
                    .saturating_add(vegetation_report.cell_requests);
                self.request_vegetation_cell_hits = self
                    .request_vegetation_cell_hits
                    .saturating_add(vegetation_report.cell_hits);
                self.request_vegetation_cell_misses = self
                    .request_vegetation_cell_misses
                    .saturating_add(vegetation_report.cell_misses);
            }
        }
        self.request_cpu_reference_micros = self
            .request_cpu_reference_micros
            .saturating_add(cpu_reference_micros);
        self.request_cpu_vegetation_micros = self
            .request_cpu_vegetation_micros
            .saturating_add(cpu_vegetation_micros);
        self.request_cpu_pack_upload_micros = self
            .request_cpu_pack_upload_micros
            .saturating_add(cpu_pack_upload_micros);

        self.refresh_published_levels();
        let cpu_published_tiles = self.cpu_published_tiles().to_vec();
        let gpu_published_tiles = self.gpu_published_tiles().to_vec();
        let drawn_tiles = cpu_published_tiles
            .iter()
            .chain(gpu_published_tiles.iter())
            .copied()
            .collect::<HashSet<_>>();
        for tile_id in &drawn_tiles {
            let tile = self
                .cache
                .get_mut(tile_id)
                .expect("published terrain viewport tiles are resident");
            queue.write_buffer(
                &tile.uniform_buffer,
                0,
                &viewport_uniform_bytes_for_request(
                    tile.request,
                    width,
                    height,
                    options,
                    camera,
                    plan.request.center_x,
                    plan.request.center_z,
                    plan.view_width_blocks,
                    plan.view_height_blocks,
                    focus_y,
                ),
            );
            self.use_clock = self.use_clock.saturating_add(1);
            tile.last_used = self.use_clock;
        }
        self.encode_draw(
            encoder,
            color_view,
            width.max(1),
            height.max(1),
            options,
            &cpu_published_tiles,
            &gpu_published_tiles,
        );
        self.evict_unused_tiles();

        let target = plan.target_level();
        let cpu_coarse_ready = self.level_cpu_ready(plan.coarsest_level().visible_tiles.as_slice());
        let cpu_target_ready = self.level_cpu_ready(target.visible_tiles.as_slice());
        let gpu_coarse_ready = self.level_gpu_ready(plan.coarsest_level().visible_tiles.as_slice());
        let gpu_target_ready = self.level_gpu_ready(target.visible_tiles.as_slice());
        let coarse_ready =
            (!cpu_required || cpu_coarse_ready) && (!gpu_required || gpu_coarse_ready);
        let target_ready =
            (!cpu_required || cpu_target_ready) && (!gpu_required || gpu_target_ready);
        let cpu_published_tile_count = u32::try_from(cpu_published_tiles.len()).unwrap_or(u32::MAX);
        let gpu_published_tile_count = u32::try_from(gpu_published_tiles.len()).unwrap_or(u32::MAX);
        let published_tile_count = match options.source {
            TerrainPreviewSource::Reference => cpu_published_tile_count,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => gpu_published_tile_count,
            TerrainPreviewSource::Split => cpu_published_tile_count.min(gpu_published_tile_count),
        };
        let sample_count = cpu_published_tile_count
            .max(gpu_published_tile_count)
            .saturating_mul(self.sample_count_per_tile);
        let terrain_vertex_count = cpu_published_tile_count
            .saturating_add(gpu_published_tile_count)
            .saturating_mul(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6);
        let vegetation_tiles = drawn_tiles
            .iter()
            .filter_map(|tile_id| self.cache.get(tile_id))
            .filter_map(|tile| tile.vegetation.as_ref())
            .collect::<Vec<_>>();
        let cpu_vegetation_summary_tile_count = u32::try_from(
            vegetation_tiles
                .iter()
                .filter(|product| product.summary_available())
                .count(),
        )
        .unwrap_or(u32::MAX);
        let gpu_vegetation_summary_tile_count = u32::try_from(
            gpu_published_tiles
                .iter()
                .filter(|tile_id| tile_id.content_stage == TerrainPreviewContentStage::Cover)
                .count(),
        )
        .unwrap_or(u32::MAX);
        let vegetation_summary_tile_count = match options.source {
            TerrainPreviewSource::Reference => cpu_vegetation_summary_tile_count,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => {
                gpu_vegetation_summary_tile_count
            }
            TerrainPreviewSource::Split => {
                cpu_vegetation_summary_tile_count.max(gpu_vegetation_summary_tile_count)
            }
        };
        let vegetation_record_tile_count = u32::try_from(
            vegetation_tiles
                .iter()
                .filter(|product| product.records_requested())
                .count(),
        )
        .unwrap_or(u32::MAX);
        let cpu_vegetation_aggregated_tile_count = u32::try_from(
            vegetation_tiles
                .iter()
                .filter(|product| product.records_aggregated())
                .count(),
        )
        .unwrap_or(u32::MAX);
        let gpu_vegetation_aggregated_tile_count = u32::try_from(
            gpu_published_tiles
                .iter()
                .filter(|tile_id| {
                    tile_id.content_stage == TerrainPreviewContentStage::Cover
                        && tile_id.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
                })
                .count(),
        )
        .unwrap_or(u32::MAX);
        let vegetation_aggregated_tile_count = match options.source {
            TerrainPreviewSource::Reference => cpu_vegetation_aggregated_tile_count,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => {
                gpu_vegetation_aggregated_tile_count
            }
            TerrainPreviewSource::Split => {
                cpu_vegetation_aggregated_tile_count.max(gpu_vegetation_aggregated_tile_count)
            }
        };
        let tree_instance_count = vegetation_tiles.iter().fold(0_u32, |count, product| {
            count.saturating_add(u32::try_from(product.occurrences().len()).unwrap_or(u32::MAX))
        });
        let tree_instance_bytes = drawn_tiles
            .iter()
            .filter_map(|tile_id| self.cache.get(tile_id))
            .map(|tile| tile.tree_instance_bytes)
            .sum::<u64>();
        let tree_proxy_vertex_count = if options.layer == TerrainPreviewLayer::Terrain
            && plan.request.content_stage == TerrainPreviewContentStage::Cover
        {
            render_panels(
                options.source,
                options.split_layout,
                width.max(1),
                height.max(1),
            )
            .iter()
            .fold(0_u32, |count, panel| {
                let tiles = if options.source == TerrainPreviewSource::Reference
                    || (options.source == TerrainPreviewSource::Split && panel.instance == 0)
                {
                    &cpu_published_tiles
                } else {
                    &gpu_published_tiles
                };
                count.saturating_add(tiles.iter().fold(0_u32, |tile_count, tile_id| {
                    let instances = self
                        .cache
                        .get(tile_id)
                        .map_or(0, |tile| tile.tree_instance_count);
                    tile_count.saturating_add(
                        instances.saturating_mul(TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE),
                    )
                }))
            })
        } else {
            0
        };
        let vertex_count = terrain_vertex_count.saturating_add(tree_proxy_vertex_count);
        let resident_tile_count = u32::try_from(self.cache.len()).unwrap_or(u32::MAX);
        let cpu_queued_tile_count = if cpu_required {
            u32::try_from(self.cpu_queue.len() + self.external_cpu_inflight.len())
                .unwrap_or(u32::MAX)
        } else {
            0
        };
        let gpu_queued_tile_count = if gpu_required {
            u32::try_from(self.gpu_queue.len() + self.external_macro_inflight.len())
                .unwrap_or(u32::MAX)
        } else {
            0
        };
        let pending_readback_count = self
            .pending
            .iter()
            .filter(|pending| pending.cache_epoch == self.cache_epoch)
            .count();
        let cpu_ready_tiles = self
            .cache
            .values()
            .filter(|tile| tile.reference.is_some())
            .count() as u64;
        let gpu_ready_tiles = self
            .cache
            .values()
            .filter(|tile| tile.gpu_submitted)
            .count() as u64;
        let resident_tree_instance_bytes = self
            .cache
            .values()
            .map(|tile| tile.tree_instance_bytes)
            .sum::<u64>();
        let retained_vegetation_cells = self.vegetation_cache.as_ref().map_or(0, |cache| {
            u32::try_from(cache.report().retained_cells).unwrap_or(u32::MAX)
        });
        let cpu_published_spacing = self
            .cpu_published_level
            .map(|index| plan.levels[index].sample_spacing)
            .unwrap_or(0);
        let gpu_published_spacing = self
            .gpu_published_level
            .map(|index| plan.levels[index].sample_spacing)
            .unwrap_or(0);
        let published_spacing = match options.source {
            TerrainPreviewSource::Reference => cpu_published_spacing,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => gpu_published_spacing,
            TerrainPreviewSource::Split => match (cpu_published_spacing, gpu_published_spacing) {
                (0, _) | (_, 0) => 0,
                (cpu, gpu) => cpu.max(gpu),
            },
        };
        let stats = TerrainViewportFrameStats {
            revision: self.latest_revision,
            requested_spacing: plan.requested_spacing,
            effective_spacing: plan.effective_spacing,
            published_spacing,
            cpu_published_spacing,
            gpu_published_spacing,
            view_width_blocks: plan.view_width_blocks,
            view_height_blocks: plan.view_height_blocks,
            level_count: u32::try_from(plan.levels.len()).unwrap_or(u32::MAX),
            visible_tile_count: u32::try_from(target.visible_tiles.len()).unwrap_or(u32::MAX),
            published_tile_count,
            cpu_published_tile_count,
            gpu_published_tile_count,
            resident_tile_count,
            queued_tile_count: cpu_queued_tile_count.saturating_add(gpu_queued_tile_count),
            cpu_queued_tile_count,
            gpu_queued_tile_count,
            pending_readback_count: u32::try_from(pending_readback_count).unwrap_or(u32::MAX),
            cpu_compiled_tiles,
            cpu_compiled_tiles_total: self.cpu_compiled_tiles_total,
            gpu_dispatched_tiles,
            gpu_dispatched_tiles_total: self.gpu_dispatched_tiles_total,
            macro_compiled_tiles_total: self.macro_compiled_tiles_total,
            request_cpu_compiled_tiles: self.request_cpu_compiled_tiles,
            request_gpu_dispatched_tiles: self.request_gpu_dispatched_tiles,
            request_macro_compiled_tiles: self.request_macro_compiled_tiles,
            request_cache_hit_tiles: self.request_cache_hit_tiles,
            request_cpu_compile_work: self.request_cpu_compile_work,
            request_gpu_compile_work: self.request_gpu_compile_work,
            evicted_tiles_total: self.evicted_tiles_total,
            stale_result_count: self.stale_result_count,
            sample_count,
            vertex_count,
            vegetation_summary_tile_count,
            cpu_vegetation_summary_tile_count,
            gpu_vegetation_summary_tile_count,
            vegetation_record_tile_count,
            vegetation_aggregated_tile_count,
            tree_instance_count,
            tree_instance_bytes,
            tree_proxy_vertex_count,
            vegetation_cell_requests: self.request_vegetation_cell_requests,
            vegetation_cell_hits: self.request_vegetation_cell_hits,
            vegetation_cell_misses: self.request_vegetation_cell_misses,
            retained_vegetation_cells,
            reference_bytes: cpu_ready_tiles * self.sample_byte_len,
            gpu_sample_bytes: gpu_ready_tiles * self.sample_byte_len,
            readback_bytes: u64::from(gpu_dispatched_tiles) * self.sample_byte_len,
            request_readback_bytes: u64::from(self.request_gpu_dispatched_tiles)
                * self.sample_byte_len,
            resident_bytes: u64::from(resident_tile_count)
                * (self.sample_byte_len * 2 + TERRAIN_PREVIEW_UNIFORM_BYTES)
                + resident_tree_instance_bytes,
            cpu_reference_micros,
            cpu_vegetation_micros,
            cpu_pack_upload_micros,
            request_cpu_reference_micros: self.request_cpu_reference_micros,
            request_cpu_vegetation_micros: self.request_cpu_vegetation_micros,
            request_cpu_pack_upload_micros: self.request_cpu_pack_upload_micros,
            request_macro_compile_micros: self.request_macro_compile_micros,
            cpu_coarse_ready,
            cpu_target_ready,
            gpu_coarse_ready,
            gpu_target_ready,
            coarse_ready,
            target_ready,
            cache_enabled: self.cache_enabled,
            budget_limited: plan.budget_limited,
            needs_redraw: cpu_queued_tile_count > 0
                || gpu_queued_tile_count > 0
                || pending_readback_count > 0,
        };
        Ok(stats)
    }

    fn mark_submitted(&mut self, encoded: EncodedTerrainViewportReadbacks) {
        for readback in encoded.readbacks {
            let (sender, receiver) = mpsc::channel();
            readback
                .readback_buffer
                .slice(..readback.byte_len)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.pending.push(PendingTerrainViewportReadback {
                cache_epoch: readback.cache_epoch,
                tile: readback.tile,
                readback_buffer: readback.readback_buffer,
                byte_len: readback.byte_len,
                receiver,
            });
        }
    }

    pub fn poll_completed(
        &mut self,
        device: &wgpu::Device,
    ) -> Vec<Result<TerrainViewportCompletedComparison, String>> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut completed = Vec::new();
        let mut index = 0;
        while index < self.pending.len() {
            match self.pending[index].receiver.try_recv() {
                Ok(Ok(())) => {
                    let pending = self.pending.remove(index);
                    let mapped = pending
                        .readback_buffer
                        .slice(..pending.byte_len)
                        .get_mapped_range();
                    let samples = parse_samples(&mapped);
                    drop(mapped);
                    pending.readback_buffer.unmap();
                    if pending.cache_epoch != self.cache_epoch {
                        self.stale_result_count = self.stale_result_count.saturating_add(1);
                        continue;
                    }
                    match (self.cache.get_mut(&pending.tile), samples) {
                        (Some(tile), Ok(samples)) => tile.gpu_samples = Some(samples),
                        (Some(_), Err(error)) => completed.push(Err(error)),
                        (None, _) => {
                            self.stale_result_count = self.stale_result_count.saturating_add(1);
                        }
                    }
                }
                Ok(Err(error)) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain viewport tile {:?} readback failed: {error}",
                        pending.tile
                    )));
                }
                Err(mpsc::TryRecvError::Empty) => index += 1,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain viewport tile {:?} readback callback disconnected",
                        pending.tile
                    )));
                }
            }
        }
        self.refresh_published_levels();
        if self.last_comparison_revision != Some(self.latest_revision)
            && let Some(result) = self.aggregate_target_comparison()
        {
            match result {
                Ok(comparison) => {
                    self.last_comparison_revision = Some(self.latest_revision);
                    completed.push(Ok(comparison));
                }
                Err(error) => completed.push(Err(error)),
            }
        }
        completed
    }

    fn compile_vegetation_product(
        &mut self,
        tile: TerrainViewportTileId,
    ) -> Result<TerrainPreviewVegetationProduct, String> {
        let request = tile.preview_request();
        if request.profile != TerrainPreviewProfile::McloneOverworldV1
            || request.content_stage != TerrainPreviewContentStage::Cover
        {
            return TerrainPreviewVegetationProduct::compile(request);
        }
        let source =
            McloneVegetationSource::new(request.seed, McloneOverworldSamplingTopology::Unbounded);
        let cache = self
            .vegetation_cache
            .get_or_insert_with(|| McloneOverworldVegetationPlanCache::new(source));
        TerrainPreviewVegetationProduct::compile_with_cache(request, cache)
    }

    fn reset_request_counters(&mut self) {
        self.request_cpu_compiled_tiles = 0;
        self.request_gpu_dispatched_tiles = 0;
        self.request_macro_compiled_tiles = 0;
        self.request_cache_hit_tiles = 0;
        self.request_cpu_compile_work = TerrainPreviewCompileWork::default();
        self.request_gpu_compile_work = TerrainPreviewCompileWork::default();
        self.request_cpu_reference_micros = 0;
        self.request_cpu_vegetation_micros = 0;
        self.request_cpu_pack_upload_micros = 0;
        self.request_macro_compile_micros = 0;
        self.request_vegetation_cell_requests = 0;
        self.request_vegetation_cell_hits = 0;
        self.request_vegetation_cell_misses = 0;
    }

    fn current_plan_cache_hits(&self) -> u32 {
        let Some(plan) = &self.plan else {
            return 0;
        };
        let mut seen = HashSet::new();
        u32::try_from(
            plan.levels
                .iter()
                .flat_map(|level| level.visible_tiles.iter())
                .filter(|tile| seen.insert(**tile) && self.cache.contains_key(tile))
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    fn rebuild_queues(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        let mut cpu_scheduled = HashSet::new();
        let mut gpu_scheduled = HashSet::new();
        for level in &plan.levels {
            for tile in &level.visible_tiles {
                let resident = self.cache.get(tile);
                if resident.is_none_or(|resident| resident.reference.is_none())
                    && cpu_scheduled.insert(*tile)
                {
                    self.cpu_queue.push_back(*tile);
                }
                if resident.is_none_or(|resident| !resident.gpu_submitted)
                    && gpu_scheduled.insert(*tile)
                {
                    self.gpu_queue.push_back(*tile);
                }
            }
        }
        if self.cache_enabled {
            for tile in &plan.target_level().preload_tiles {
                let resident = self.cache.get(tile);
                if resident.is_none_or(|resident| resident.reference.is_none())
                    && cpu_scheduled.insert(*tile)
                {
                    self.cpu_queue.push_back(*tile);
                }
                if resident.is_none_or(|resident| !resident.gpu_submitted)
                    && gpu_scheduled.insert(*tile)
                {
                    self.gpu_queue.push_back(*tile);
                }
            }
        }
    }

    fn take_next_missing_cpu_tile(&mut self) -> Option<TerrainViewportTileId> {
        while let Some(tile) = self.cpu_queue.pop_front() {
            if self
                .cache
                .get(&tile)
                .is_none_or(|resident| resident.reference.is_none())
            {
                return Some(tile);
            }
        }
        None
    }

    fn take_next_missing_gpu_tile(&mut self) -> Option<TerrainViewportTileId> {
        while let Some(tile) = self.gpu_queue.pop_front() {
            if self
                .cache
                .get(&tile)
                .is_none_or(|resident| !resident.gpu_submitted)
            {
                return Some(tile);
            }
        }
        None
    }

    fn plan_contains_tile(&self, tile: TerrainViewportTileId) -> bool {
        self.plan.as_ref().is_some_and(|plan| {
            plan.levels
                .iter()
                .any(|level| level.visible_tiles.contains(&tile))
                || plan.target_level().preload_tiles.contains(&tile)
        })
    }

    fn refresh_published_levels(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            self.cpu_published_level = None;
            self.gpu_published_level = None;
            return;
        };
        self.cpu_published_level = plan
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| self.level_cpu_ready(level.visible_tiles.as_slice()))
            .map(|(index, _)| index)
            .next_back();
        self.gpu_published_level = plan
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| self.level_gpu_ready(level.visible_tiles.as_slice()))
            .map(|(index, _)| index)
            .next_back();
    }

    fn level_cpu_ready(&self, tiles: &[TerrainViewportTileId]) -> bool {
        tiles.iter().all(|tile| {
            self.cache
                .get(tile)
                .is_some_and(|resident| resident.reference.is_some())
        })
    }

    fn level_gpu_ready(&self, tiles: &[TerrainViewportTileId]) -> bool {
        tiles.iter().all(|tile| {
            self.cache
                .get(tile)
                .is_some_and(|resident| resident.gpu_samples.is_some())
        })
    }

    fn cpu_published_tiles(&self) -> &[TerrainViewportTileId] {
        self.plan
            .as_ref()
            .zip(self.cpu_published_level)
            .map(|(plan, index)| plan.levels[index].visible_tiles.as_slice())
            .unwrap_or(&[])
    }

    fn gpu_published_tiles(&self) -> &[TerrainViewportTileId] {
        self.plan
            .as_ref()
            .zip(self.gpu_published_level)
            .map(|(plan, index)| plan.levels[index].visible_tiles.as_slice())
            .unwrap_or(&[])
    }

    fn touch_current_tiles(&mut self) {
        let protected = self.protected_tiles();
        for tile_id in protected {
            if let Some(tile) = self.cache.get_mut(&tile_id) {
                self.use_clock = self.use_clock.saturating_add(1);
                tile.last_used = self.use_clock;
            }
        }
    }

    fn protected_tiles(&self) -> HashSet<TerrainViewportTileId> {
        let mut protected = HashSet::new();
        if let Some(plan) = &self.plan {
            for level in &plan.levels {
                protected.extend(level.visible_tiles.iter().copied());
            }
            if self.cache_enabled {
                protected.extend(plan.target_level().preload_tiles.iter().copied());
            }
        }
        protected.extend(
            self.pending
                .iter()
                .filter(|pending| pending.cache_epoch == self.cache_epoch)
                .map(|pending| pending.tile),
        );
        protected
    }

    fn evict_unused_tiles(&mut self) {
        let protected = self.protected_tiles();
        while self.cache.len() > TERRAIN_VIEWPORT_MAX_RESIDENT_TILES {
            let candidate = self
                .cache
                .iter()
                .filter(|(tile, _)| !protected.contains(tile))
                .min_by_key(|(_, resident)| resident.last_used)
                .map(|(tile, _)| *tile);
            let Some(candidate) = candidate else {
                break;
            };
            self.cache.remove(&candidate);
            self.evicted_tiles_total = self.evicted_tiles_total.saturating_add(1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        options: TerrainPreviewDrawOptions,
        cpu_tiles: &[TerrainViewportTileId],
        gpu_tiles: &[TerrainViewportTileId],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_terrain_viewport_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: if self.depth_capture_enabled {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.render_pipeline);
        pass.set_bind_group(1, &self._material_resources.bind_group, &[]);
        let panels = render_panels(options.source, options.split_layout, width, height);
        for panel in &panels {
            pass.set_scissor_rect(panel.x, panel.y, panel.width, panel.height);
            let tiles = if options.source == TerrainPreviewSource::Reference
                || (options.source == TerrainPreviewSource::Split && panel.instance == 0)
            {
                cpu_tiles
            } else {
                gpu_tiles
            };
            for tile_id in tiles {
                let tile = self
                    .cache
                    .get(tile_id)
                    .expect("drawn terrain viewport tiles are resident");
                if options.layer == TerrainPreviewLayer::Error
                    && (tile.reference.is_none() || tile.gpu_samples.is_none())
                {
                    continue;
                }
                pass.set_bind_group(0, &tile.render_bind_group, &[]);
                pass.draw(
                    0..TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6,
                    panel.instance..panel.instance + 1,
                );
            }
        }
        if options.layer != TerrainPreviewLayer::Terrain {
            return;
        }
        pass.set_pipeline(&self.tree_pipeline);
        for panel in &panels {
            pass.set_scissor_rect(panel.x, panel.y, panel.width, panel.height);
            let tiles = if options.source == TerrainPreviewSource::Reference
                || (options.source == TerrainPreviewSource::Split && panel.instance == 0)
            {
                cpu_tiles
            } else {
                gpu_tiles
            };
            for tile_id in tiles {
                let tile = self
                    .cache
                    .get(tile_id)
                    .expect("drawn terrain viewport tiles are resident");
                let Some(instance_buffer) = tile.tree_instance_buffer.as_ref() else {
                    continue;
                };
                pass.set_bind_group(0, &tile.render_bind_group, &[]);
                pass.set_vertex_buffer(0, instance_buffer.slice(..));
                let first_instance = panel.instance.saturating_mul(tile.tree_instance_count);
                pass.draw(
                    0..TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE,
                    first_instance..first_instance.saturating_add(tile.tree_instance_count),
                );
            }
        }
    }

    fn aggregate_target_comparison(
        &self,
    ) -> Option<Result<TerrainViewportCompletedComparison, String>> {
        let plan = self.plan.as_ref()?;
        let target = plan.target_level();
        let sample_capacity = target
            .visible_tiles
            .len()
            .checked_mul(usize::try_from(self.sample_count_per_tile).ok()?)?;
        let mut reference_samples = Vec::with_capacity(sample_capacity);
        let mut gpu_samples = Vec::with_capacity(sample_capacity);
        for tile_id in &target.visible_tiles {
            let tile = self.cache.get(tile_id)?;
            let samples = tile.gpu_samples.as_ref()?;
            let reference = tile.reference.as_ref()?.samples();
            reference_samples.extend_from_slice(reference);
            gpu_samples.extend(samples.iter().zip(reference).map(|(gpu, cpu)| {
                if tile_id.content_stage.includes_structured_hydrology()
                    && cpu.planned_stream_influence > 0.0
                {
                    *cpu
                } else {
                    *gpu
                }
            }));
        }
        Some(
            TerrainPreviewComparison::compare_samples(&reference_samples, &gpu_samples).map(
                |comparison| TerrainViewportCompletedComparison {
                    revision: self.latest_revision,
                    sample_spacing: target.sample_spacing,
                    tile_count: u32::try_from(target.visible_tiles.len()).unwrap_or(u32::MAX),
                    comparison,
                },
            ),
        )
    }
}

pub struct TerrainHorizonRenderer {
    renderer: TerrainViewportRenderer,
    clipmap: TerrainClipmap,
    slots: Vec<TerrainViewportGpuTile>,
    assignments: Vec<Option<TerrainClipmapTile>>,
    ready: Vec<bool>,
    pending: VecDeque<TerrainClipmapTile>,
    pending_vegetation: VecDeque<TerrainClipmapTile>,
    vegetation_cache: Option<McloneOverworldVegetationPlanCache>,
    vegetation_enabled: bool,
    seed: i64,
    content_stage: TerrainPreviewContentStage,
    dispatched_refills_total: u64,
}

impl TerrainHorizonRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        config: TerrainClipmapConfig,
        vegetation_enabled: bool,
    ) -> Result<Self, String> {
        Self::new_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            config,
            vegetation_enabled,
            RenderTargetColorTransform::Identity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_target_color_transform(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        config: TerrainClipmapConfig,
        vegetation_enabled: bool,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self, String> {
        let clipmap = TerrainClipmap::new(config)?;
        let config = clipmap.config();
        let renderer = TerrainViewportRenderer::new_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            target_color_transform,
        )?;
        let mut slots = Vec::with_capacity(config.allocation_slots() as usize);
        for level in 0..config.level_count {
            for physical_z in 0..config.tiles_per_axis {
                for physical_x in 0..config.tiles_per_axis {
                    slots.push(TerrainViewportGpuTile::new_gpu_only(
                        device,
                        &renderer.compute_layout,
                        &renderer.render_layout,
                        renderer.sample_byte_len,
                        TerrainViewportTileId {
                            profile: TerrainPreviewProfile::McloneOverworldV1,
                            seed: 0,
                            tile_x: physical_x as i32,
                            tile_z: physical_z as i32,
                            sample_spacing: config.sample_spacing(level),
                            content_stage: TerrainPreviewContentStage::Cover,
                            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
                        },
                    )?);
                }
            }
        }
        let slot_count = config.allocation_slots() as usize;
        Ok(Self {
            renderer,
            clipmap,
            slots,
            assignments: vec![None; slot_count],
            ready: vec![false; slot_count],
            pending: VecDeque::with_capacity(slot_count),
            pending_vegetation: VecDeque::with_capacity(config.slots_per_level() as usize),
            vegetation_cache: None,
            vegetation_enabled,
            seed: 0,
            content_stage: TerrainPreviewContentStage::Cover,
            dispatched_refills_total: 0,
        })
    }

    pub const fn config(&self) -> TerrainClipmapConfig {
        self.clipmap.config()
    }

    pub const fn diagnostics(&self) -> TerrainClipmapDiagnostics {
        self.clipmap.diagnostics()
    }

    pub fn set_view(
        &mut self,
        seed: i64,
        center_x: i32,
        center_z: i32,
        content_stage: TerrainPreviewContentStage,
    ) {
        let source_changed = self.seed != seed || self.content_stage != content_stage;
        if source_changed {
            self.clipmap = TerrainClipmap::new(self.clipmap.config())
                .expect("an already validated terrain clipmap config remains valid");
            self.assignments.fill(None);
            self.ready.fill(false);
            self.pending.clear();
            self.pending_vegetation.clear();
            self.vegetation_cache = None;
        }
        self.seed = seed;
        self.content_stage = content_stage;

        let update = self.clipmap.update_center(center_x, center_z);
        for tile in &update.refills {
            let slot = tile.physical_slot as usize;
            self.assignments[slot] = Some(*tile);
            self.ready[slot] = false;
            self.slots[slot].clear_vegetation();
        }
        self.pending.retain(|tile| {
            self.assignments
                .get(tile.physical_slot as usize)
                .copied()
                .flatten()
                == Some(*tile)
        });
        self.pending_vegetation.retain(|tile| {
            self.assignments
                .get(tile.physical_slot as usize)
                .copied()
                .flatten()
                == Some(*tile)
        });
        for tile in update.refills {
            if !self.pending.contains(&tile) {
                self.pending.push_back(tile);
            }
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.renderer.resize(device, width, height);
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.renderer.set_depth_capture_enabled(enabled);
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        self.renderer
            .copy_depth_to_buffer(encoder, destination, bytes_per_row)
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.resize(device, width, height);
        let uniform_presentation = presentation.uniform_facts()?;
        let options = TerrainPreviewDrawOptions {
            source: TerrainPreviewSource::Gpu,
            view: presentation.view,
            layer: TerrainPreviewLayer::Terrain,
            split_layout: TerrainPreviewSplitLayout::Columns,
        };
        let focus_y = terrain_horizon_orbit_target_y();
        let mut dispatched_refills = 0_u32;
        for _ in 0..TERRAIN_VIEWPORT_GPU_DISPATCHES_PER_FRAME {
            let Some(tile) = self.pending.pop_front() else {
                break;
            };
            let slot_index = tile.physical_slot as usize;
            if self.assignments[slot_index] != Some(tile) {
                continue;
            }
            let tile_id = TerrainViewportTileId {
                profile: TerrainPreviewProfile::McloneOverworldV1,
                seed: self.seed,
                tile_x: tile.tile_x,
                tile_z: tile.tile_z,
                sample_spacing: tile.sample_spacing,
                content_stage: self.content_stage,
                surface_quality: TerrainPreviewSurfaceQuality::Inferred,
            };
            let slot = &mut self.slots[slot_index];
            slot.request = tile_id.preview_request().validate()?;
            queue.write_buffer(
                &slot.uniform_buffer,
                0,
                &viewport_uniform_bytes_for_request_with_presentation(
                    slot.request,
                    width,
                    height,
                    options,
                    presentation.camera,
                    uniform_presentation,
                    focus_y,
                    None,
                ),
            );
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("mclone_terrain_horizon_compute_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.renderer.compute_pipeline);
                pass.set_bind_group(0, &slot.compute_bind_group, &[]);
                let workgroups = (TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1)
                    .div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
                pass.dispatch_workgroups(workgroups, workgroups, 1);
            }
            slot.gpu_submitted = true;
            self.ready[slot_index] = true;
            if self.vegetation_enabled
                && tile.sample_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
                && !self.pending_vegetation.contains(&tile)
            {
                self.pending_vegetation.push_back(tile);
            }
            dispatched_refills = dispatched_refills.saturating_add(1);
        }
        self.clipmap.note_refills_completed(dispatched_refills);
        self.dispatched_refills_total = self
            .dispatched_refills_total
            .saturating_add(u64::from(dispatched_refills));

        for _ in 0..TERRAIN_HORIZON_VEGETATION_COMPILES_PER_FRAME {
            let Some(tile) = self.pending_vegetation.pop_front() else {
                break;
            };
            let slot_index = tile.physical_slot as usize;
            if self.assignments[slot_index] != Some(tile) || !self.ready[slot_index] {
                continue;
            }
            let tile_id = TerrainViewportTileId {
                profile: TerrainPreviewProfile::McloneOverworldV1,
                seed: self.seed,
                tile_x: tile.tile_x,
                tile_z: tile.tile_z,
                sample_spacing: tile.sample_spacing,
                content_stage: self.content_stage,
                surface_quality: TerrainPreviewSurfaceQuality::Inferred,
            };
            let cache = self.vegetation_cache.get_or_insert_with(|| {
                McloneOverworldVegetationPlanCache::new(McloneVegetationSource::new(
                    self.seed,
                    McloneOverworldSamplingTopology::Unbounded,
                ))
            });
            let vegetation = TerrainPreviewVegetationProduct::compile_with_cache(
                tile_id.preview_request(),
                cache,
            )?;
            self.slots[slot_index].upload_vegetation(device, queue, vegetation)?;
        }

        let levels = self.clipmap.levels();
        let mut level_ready = vec![false; levels.len()];
        for level in &levels {
            level_ready[level.level as usize] = level.tiles.iter().all(|tile| {
                let slot = tile.physical_slot as usize;
                self.assignments[slot] == Some(*tile) && self.ready[slot]
            });
        }
        for level in &levels {
            if !level_ready[level.level as usize] {
                continue;
            }
            let inner_hole = if level.level > 0 && level_ready[level.level as usize - 1] {
                level.inner_hole
            } else {
                None
            };
            for tile in &level.tiles {
                let slot_index = tile.physical_slot as usize;
                let slot = &self.slots[slot_index];
                queue.write_buffer(
                    &slot.uniform_buffer,
                    0,
                    &viewport_uniform_bytes_for_request_with_presentation(
                        slot.request,
                        width,
                        height,
                        options,
                        presentation.camera,
                        uniform_presentation,
                        focus_y,
                        inner_hole,
                    ),
                );
            }
        }

        let mut drawn_levels = 0_u32;
        let mut drawn_tiles = 0_u32;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_terrain_horizon_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.renderer.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.renderer.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: if self.renderer.depth_capture_enabled {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.renderer.render_pipeline);
            pass.set_bind_group(1, &self.renderer._material_resources.bind_group, &[]);
            for level in levels.iter().rev() {
                if !level_ready[level.level as usize] {
                    continue;
                }
                drawn_levels = drawn_levels.saturating_add(1);
                for tile in &level.tiles {
                    let slot_index = tile.physical_slot as usize;
                    pass.set_bind_group(0, &self.slots[slot_index].render_bind_group, &[]);
                    pass.draw(0..TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6, 0..1);
                    drawn_tiles = drawn_tiles.saturating_add(1);
                }
            }
            pass.set_pipeline(&self.renderer.tree_pipeline);
            for level in &levels {
                if !level_ready[level.level as usize]
                    || level.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
                {
                    continue;
                }
                for tile in &level.tiles {
                    let slot = &self.slots[tile.physical_slot as usize];
                    let Some(instance_buffer) = slot.tree_instance_buffer.as_ref() else {
                        continue;
                    };
                    pass.set_bind_group(0, &slot.render_bind_group, &[]);
                    pass.set_vertex_buffer(0, instance_buffer.slice(..));
                    pass.draw(
                        0..TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE,
                        0..slot.tree_instance_count,
                    );
                }
            }
        }

        let ready_slots = self.ready.iter().filter(|ready| **ready).count() as u32;
        let allocation_slots = self.clipmap.config().allocation_slots();
        let tree_instance_count = self.slots.iter().fold(0_u32, |count, slot| {
            count.saturating_add(slot.tree_instance_count)
        });
        let tree_proxy_vertex_count =
            tree_instance_count.saturating_mul(TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE);
        let vertex_count = drawn_tiles
            .saturating_mul(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6)
            .saturating_add(tree_proxy_vertex_count);
        let fixed_resident_bytes = u64::from(allocation_slots).saturating_mul(
            self.renderer
                .sample_byte_len
                .saturating_add(TERRAIN_PREVIEW_UNIFORM_BYTES),
        );
        let vegetation_bytes = self
            .slots
            .iter()
            .map(|slot| slot.tree_instance_bytes)
            .sum::<u64>();
        let resident_bytes = fixed_resident_bytes.saturating_add(vegetation_bytes);
        let vegetation_ready_tiles = self
            .slots
            .iter()
            .filter(|slot| slot.vegetation.is_some())
            .count() as u32;
        let target_ready = ready_slots == allocation_slots
            && self.pending.is_empty()
            && self.pending_vegetation.is_empty();
        Ok(TerrainHorizonFrameStats {
            revision: self.clipmap.diagnostics().revision,
            allocation_slots,
            ready_slots,
            pending_refills: self.pending.len() as u32,
            dispatched_refills,
            dispatched_refills_total: self.dispatched_refills_total,
            drawn_levels,
            drawn_tiles,
            vertex_count,
            vegetation_ready_tiles,
            pending_vegetation_tiles: self.pending_vegetation.len() as u32,
            tree_instance_count,
            tree_proxy_vertex_count,
            fixed_resident_bytes,
            vegetation_bytes,
            resident_bytes,
            finest_sample_spacing: self.clipmap.config().base_sample_spacing,
            coarse_ready: drawn_levels > 0,
            target_ready,
            needs_redraw: !target_ready,
            residency: self.clipmap.diagnostics(),
        })
    }
}

fn tree_instance_bytes(
    vegetation: &TerrainPreviewVegetationProduct,
) -> Result<(Vec<u8>, u32), String> {
    let instance_count = u32::try_from(vegetation.occurrences().len())
        .map_err(|_| "terrain preview tree instance count exceeds u32")?;
    let byte_capacity = vegetation
        .occurrences()
        .len()
        .checked_mul(TERRAIN_PREVIEW_TREE_INSTANCE_BYTES as usize)
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or("terrain preview tree instance byte size overflow")?;
    let mut bytes = Vec::with_capacity(byte_capacity);
    for panel in 0..2 {
        for occurrence in vegetation.occurrences() {
            let base = occurrence
                .working_base()
                .map_err(|error| format!("terrain preview tree base is invalid: {error}"))?;
            let family = match occurrence.record.family {
                McloneTreeFamily::TemperateBroadleaf => 1.0,
                McloneTreeFamily::CoolWetConifer => 2.0,
                McloneTreeFamily::WarmDryAcacia => 3.0,
            };
            let values = [
                base.x as f32,
                base.y as f32,
                base.z as f32,
                f32::from(occurrence.record.trunk_height),
                f32::from(occurrence.record.crown_radius),
                f32::from(occurrence.record.crown_depth),
                family,
                f32::from(occurrence.record.orientation),
                panel as f32,
                f32::from(occurrence.record.landmark_rank),
                0.0,
                0.0,
            ];
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok((bytes, instance_count))
}

#[derive(Clone, Copy)]
struct RenderPanel {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    instance: u32,
}

fn render_panels(
    source: TerrainPreviewSource,
    split_layout: TerrainPreviewSplitLayout,
    width: u32,
    height: u32,
) -> Vec<RenderPanel> {
    if source != TerrainPreviewSource::Split {
        return vec![RenderPanel {
            x: 0,
            y: 0,
            width,
            height,
            instance: 0,
        }];
    }
    if split_layout == TerrainPreviewSplitLayout::Rows {
        let first_height = (height / 2).max(1);
        let second_height = height.saturating_sub(first_height).max(1);
        vec![
            RenderPanel {
                x: 0,
                y: 0,
                width,
                height: first_height,
                instance: 0,
            },
            RenderPanel {
                x: 0,
                y: first_height,
                width,
                height: second_height,
                instance: 1,
            },
        ]
    } else {
        let first_width = (width / 2).max(1);
        let second_width = width.saturating_sub(first_width).max(1);
        vec![
            RenderPanel {
                x: 0,
                y: 0,
                width: first_width,
                height,
                instance: 0,
            },
            RenderPanel {
                x: first_width,
                y: 0,
                width: second_width,
                height,
                instance: 1,
            },
        ]
    }
}

fn source_needs_cpu(
    options: TerrainPreviewDrawOptions,
    content_stage: TerrainPreviewContentStage,
    effective_spacing: u32,
) -> bool {
    !matches!(
        options.source,
        TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro
    ) || options.layer == TerrainPreviewLayer::Error
        || (content_stage.includes_structured_hydrology()
            && effective_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING)
}

fn source_needs_gpu(options: TerrainPreviewDrawOptions, profile: TerrainPreviewProfile) -> bool {
    match profile {
        TerrainPreviewProfile::McloneOverworldV1 => {
            matches!(
                options.source,
                TerrainPreviewSource::Gpu | TerrainPreviewSource::Split
            ) || options.layer == TerrainPreviewLayer::Error
        }
        TerrainPreviewProfile::VanillaOverworld => {
            matches!(
                options.source,
                TerrainPreviewSource::Macro | TerrainPreviewSource::Split
            ) || options.layer == TerrainPreviewLayer::Error
        }
    }
}

fn uniform_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    uniform_layout_entry_with_size(binding, visibility, TERRAIN_PREVIEW_UNIFORM_BYTES)
}

fn uniform_layout_entry_with_size(
    binding: u32,
    visibility: wgpu::ShaderStages,
    byte_len: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(byte_len),
        },
        count: None,
    }
}

fn storage_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
    byte_len: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(byte_len),
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::terrain_preview::TerrainPreviewRequest;

    #[test]
    fn compare_panels_match_shader_layout() {
        assert_eq!(
            render_panels(
                TerrainPreviewSource::Split,
                TerrainPreviewSplitLayout::Columns,
                1200,
                700
            )
            .len(),
            2
        );
        let wide = render_panels(
            TerrainPreviewSource::Split,
            TerrainPreviewSplitLayout::Columns,
            1200,
            900,
        );
        assert_eq!((wide[0].width, wide[1].x), (600, 600));
        let tall = render_panels(
            TerrainPreviewSource::Split,
            TerrainPreviewSplitLayout::Rows,
            1200,
            900,
        );
        assert_eq!((tall[0].height, tall[1].y), (450, 450));
        assert_eq!(
            render_panels(
                TerrainPreviewSource::Gpu,
                TerrainPreviewSplitLayout::Rows,
                400,
                900
            )
            .len(),
            1
        );
    }

    #[test]
    fn compute_template_is_still_the_shared_production_kernel() {
        assert!(
            super::super::TERRAIN_PREVIEW_COMPUTE_WGSL_TEMPLATE.contains("fn land_surface_height")
        );
        assert_eq!(
            mclone_worldgen::terrain_preview::TERRAIN_PREVIEW_SAMPLE_FLOATS,
            usize::try_from(TERRAIN_PREVIEW_SAMPLE_BYTES / 4).unwrap()
        );
    }

    #[test]
    fn tree_instances_preserve_semantics_for_both_compare_panels() {
        let request = TerrainPreviewRequest::new(12_345, -80, 48, 4)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let product = TerrainPreviewVegetationProduct::compile(request).unwrap();
        let (bytes, instance_count) = tree_instance_bytes(&product).unwrap();

        assert!(instance_count > 0);
        assert_eq!(
            bytes.len(),
            usize::try_from(instance_count).unwrap()
                * TERRAIN_PREVIEW_TREE_INSTANCE_BYTES as usize
                * 2
        );
        let values = bytes
            .chunks_exact(size_of::<f32>())
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        let panel_stride =
            usize::try_from(instance_count).unwrap() * TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS;
        assert_eq!(values[8], 0.0);
        assert_eq!(values[panel_stride + 8], 1.0);
        assert!((1.0..=3.0).contains(&values[6]));
        assert_eq!(values[9], 3.0);
    }

    #[test]
    fn source_lanes_are_independent_except_for_error_comparison() {
        let mut options = TerrainPreviewDrawOptions::default();
        assert!(source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(!source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));

        options.source = TerrainPreviewSource::Gpu;
        assert!(!source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));

        options.source = TerrainPreviewSource::Split;
        assert!(source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));

        options.source = TerrainPreviewSource::Reference;
        options.layer = TerrainPreviewLayer::Error;
        assert!(source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::VanillaOverworld,
        ));
        options.layer = TerrainPreviewLayer::Terrain;
        options.source = TerrainPreviewSource::Macro;
        assert!(!source_needs_cpu(
            options,
            TerrainPreviewContentStage::Surface,
            32,
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::VanillaOverworld,
        ));
        assert!(source_needs_cpu(
            TerrainPreviewDrawOptions {
                source: TerrainPreviewSource::Gpu,
                ..TerrainPreviewDrawOptions::default()
            },
            TerrainPreviewContentStage::Structured,
            4,
        ));
        assert!(!source_needs_cpu(
            TerrainPreviewDrawOptions {
                source: TerrainPreviewSource::Gpu,
                ..TerrainPreviewDrawOptions::default()
            },
            TerrainPreviewContentStage::Cover,
            8,
        ));
    }
}
