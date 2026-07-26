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
    TerrainPreviewProfile, TerrainPreviewReferenceGrid, TerrainPreviewRequest,
    TerrainPreviewSample, TerrainPreviewSurfaceQuality, TerrainPreviewVegetationProduct,
    ValidatedTerrainPreviewRequest, terrain_preview_gpu_compile_work,
};
use mclone_worldgen::terrain_vegetation::{
    TerrainVegetationSourceIdentity, terrain_vegetation_coverage_receipt,
};

use super::{
    ExactPaintedCoverageSnapshot, TERRAIN_EXACT_COVERAGE_MASK_BYTES, TERRAIN_PREVIEW_DEPTH_FORMAT,
    TERRAIN_PREVIEW_SAMPLE_BYTES, TERRAIN_PREVIEW_UNIFORM_BYTES, TERRAIN_PREVIEW_WORKGROUP_AXIS,
    TerrainClipmap, TerrainClipmapConfig, TerrainClipmapDiagnostics, TerrainClipmapTile,
    TerrainCompositionSourceIdentity, TerrainExactCoverageMask, TerrainExactCoverageMode,
    TerrainHorizonPresentation, TerrainPreviewCamera, TerrainPreviewDrawOptions,
    TerrainPreviewLayer, TerrainPreviewSource, TerrainPreviewSplitLayout,
    TerrainVegetationCoordinator, TerrainVegetationCoordinatorState, TerrainVegetationDesiredTile,
    TerrainVegetationExecutor, TerrainVegetationExecutorKind, TerrainVegetationSlotToken,
    TerrainViewportPlan, TerrainViewportTileId,
    horizon_admission::{
        TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL, TerrainHorizonAdmission,
        TerrainHorizonBeginTransition, TerrainHorizonLevelPresentation, TerrainHorizonResourceTile,
    },
    parse_samples, terrain_horizon_orbit_target_y, terrain_preview_compute_wgsl,
    terrain_preview_focus_y_for_profile, terrain_preview_render_wgsl, terrain_preview_tree_wgsl,
    viewport_uniform_bytes_for_request, viewport_uniform_bytes_for_request_with_presentation,
};

pub const TERRAIN_PREVIEW_MATERIAL_UV_COUNT: usize = 256;
const TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES: u64 = 32;

#[derive(Clone, Copy, Debug)]
pub struct TerrainHorizonRenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub color_load: wgpu::LoadOp<wgpu::Color>,
    pub color_store: wgpu::StoreOp,
    pub depth_load: wgpu::LoadOp<f32>,
    pub depth_store: wgpu::StoreOp,
}

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
pub const TERRAIN_VIEWPORT_CPU_COMPILE_BUDGET_MICROS: u64 = 8_000;
const TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS: usize = 12;
const TERRAIN_PREVIEW_TREE_INSTANCE_BYTES: u64 =
    (TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS * size_of::<f32>()) as u64;
const TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE: u32 = 108;
const TERRAIN_HORIZON_NORMAL_HALO_RADIUS: u32 = 2;
const TERRAIN_HORIZON_NORMAL_EDGE_WEST: u32 = 1 << 27;
const TERRAIN_HORIZON_NORMAL_EDGE_EAST: u32 = 1 << 28;
const TERRAIN_HORIZON_NORMAL_EDGE_NORTH: u32 = 1 << 29;
const TERRAIN_HORIZON_NORMAL_EDGE_SOUTH: u32 = 1 << 30;

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
    pub staging_slots: u32,
    pub normal_halo_radius: u32,
    pub normal_halo_samples_per_tile: u32,
    pub normal_halo_fixed_bytes: u64,
    pub normal_height_fixed_bytes: u64,
    pub ready_slots: u32,
    pub requested_levels: u32,
    pub staged_levels: u32,
    pub committed_levels: u32,
    pub vegetation_committed_levels: u32,
    pub atomic_level_commits: u64,
    pub deferred_transition_attempts: u64,
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
    pub exact_coverage_mode: TerrainExactCoverageMode,
    pub exact_coverage_generation: u64,
    pub exact_painted_chunks: u32,
    pub exact_coverage_mask_bytes: u64,
    pub vegetation_service: TerrainHorizonVegetationServiceStats,
    pub finest_sample_spacing: u32,
    pub coarse_ready: bool,
    pub target_ready: bool,
    pub needs_redraw: bool,
    pub residency: TerrainClipmapDiagnostics,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainHorizonVegetationServiceStats {
    pub enabled: bool,
    pub coordinator_state: Option<TerrainVegetationCoordinatorState>,
    pub executor_kind: Option<TerrainVegetationExecutorKind>,
    pub source_fingerprint: u64,
    pub terrain_source_revision: u64,
    pub compiler_source_revision: u64,
    pub vegetation_plan_revision: u64,
    pub product_revision: u32,
    pub record_hash: u64,
    pub family_counts: [u32; 3],
    pub record_count: u32,
    pub product_count: u32,
    pub executor_generation: u32,
    pub source_epoch: u32,
    pub coverage_revision: u64,
    pub desired_tiles: u32,
    pub queued_tiles: u32,
    pub resident_tiles: u32,
    pub in_flight: bool,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub admitted_products: u64,
    pub source_resets: u64,
    pub transport_failures: u64,
    pub executor_restarts: u64,
    pub job_failures: u64,
    pub stale_completions: u64,
    pub superseded_completions: u64,
    pub submit_full_count: u64,
    pub compile_micros: u64,
    pub cache_cell_requests: u64,
    pub cache_cell_hits: u64,
    pub cache_cell_misses: u64,
    pub cache_retained_cells: u64,
    pub cache_retained_preliminary_candidates: u64,
    pub executor_submitted_jobs: u64,
    pub executor_completed_jobs: u64,
    pub executor_transport_failures: u64,
    pub executor_restart_count: u64,
    pub result_capacity_bytes: u64,
    pub result_high_water_bytes: u64,
    pub result_overflow_count: u64,
    pub copied_result_bytes: u64,
    pub main_decode_micros: u64,
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

struct TerrainExactCoverageResources {
    _uniform_buffer: wgpu::Buffer,
    _mask_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mask: TerrainExactCoverageMask,
    mode: TerrainExactCoverageMode,
    uploaded_mode: TerrainExactCoverageMode,
}

impl TerrainExactCoverageResources {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Result<Self, String> {
        let snapshot = ExactPaintedCoverageSnapshot::new(
            TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 0),
            1,
            [],
        )?;
        let mask = snapshot.packed_mask()?;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_exact_coverage_uniform"),
            size: TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mask_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_exact_coverage_mask"),
            size: TERRAIN_EXACT_COVERAGE_MASK_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform_buffer,
            0,
            &mask.uniform_bytes(TerrainExactCoverageMode::Disabled),
        );
        queue.write_buffer(&mask_buffer, 0, &mask.word_bytes());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_exact_coverage_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mask_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            _uniform_buffer: uniform_buffer,
            _mask_buffer: mask_buffer,
            bind_group,
            mask,
            mode: TerrainExactCoverageMode::Disabled,
            uploaded_mode: TerrainExactCoverageMode::Disabled,
        })
    }

    fn set_snapshot(
        &mut self,
        queue: &wgpu::Queue,
        snapshot: &ExactPaintedCoverageSnapshot,
        mode: TerrainExactCoverageMode,
    ) -> Result<(), String> {
        let mask = snapshot.packed_mask()?;
        queue.write_buffer(&self._mask_buffer, 0, &mask.word_bytes());
        queue.write_buffer(&self._uniform_buffer, 0, &mask.uniform_bytes(mode));
        self.mask = mask;
        self.mode = mode;
        self.uploaded_mode = mode;
        Ok(())
    }

    fn disable(&mut self) {
        self.mode = TerrainExactCoverageMode::Disabled;
    }

    fn sync(&mut self, queue: &wgpu::Queue) {
        if self.uploaded_mode != self.mode {
            queue.write_buffer(
                &self._uniform_buffer,
                0,
                &self.mask.uniform_bytes(self.mode),
            );
            self.uploaded_mode = self.mode;
        }
    }
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
    tree_uniform_buffer: wgpu::Buffer,
    gpu_sample_buffer: wgpu::Buffer,
    _normal_height_buffer: wgpu::Buffer,
    reference_sample_buffer: Option<wgpu::Buffer>,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    tree_render_bind_group: wgpu::BindGroup,
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
        normal_height_buffer: wgpu::Buffer,
        tile_id: TerrainViewportTileId,
    ) -> Result<Self, String> {
        Self::new_with_reference_buffer(
            device,
            compute_layout,
            render_layout,
            sample_byte_len,
            normal_height_buffer,
            tile_id,
            true,
        )
    }

    fn new_gpu_only(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        normal_height_byte_len: u64,
        tile_id: TerrainViewportTileId,
    ) -> Result<Self, String> {
        let normal_height_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_horizon_normal_heights"),
            size: normal_height_byte_len,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        Self::new_with_reference_buffer(
            device,
            compute_layout,
            render_layout,
            sample_byte_len,
            normal_height_buffer,
            tile_id,
            false,
        )
    }

    fn new_with_reference_buffer(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        normal_height_buffer: wgpu::Buffer,
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
        let tree_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_tree_uniforms"),
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: normal_height_buffer.as_entire_binding(),
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: normal_height_buffer.as_entire_binding(),
                },
            ],
        });
        let tree_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_viewport_tree_render_bind_group"),
            layout: render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: tree_uniform_buffer.as_entire_binding(),
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: normal_height_buffer.as_entire_binding(),
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
            tree_uniform_buffer,
            gpu_sample_buffer,
            _normal_height_buffer: normal_height_buffer,
            reference_sample_buffer,
            compute_bind_group,
            render_bind_group,
            tree_render_bind_group,
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
    normal_height_scratch_buffer: wgpu::Buffer,
    compute_layout: wgpu::BindGroupLayout,
    render_layout: wgpu::BindGroupLayout,
    _material_resources: TerrainPreviewMaterialResources,
    exact_coverage: TerrainExactCoverageResources,
    compute_pipeline: wgpu::ComputePipeline,
    horizon_compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    horizon_render_pipeline: wgpu::RenderPipeline,
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
        let normal_height_byte_len = u64::from(sample_count_per_tile)
            .checked_mul(size_of::<f32>() as u64)
            .ok_or("terrain viewport normal-height buffer size overflow")?;
        let normal_height_scratch_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_normal_height_scratch"),
            size: normal_height_byte_len,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_compute_layout"),
            entries: &[
                uniform_layout_entry(0, wgpu::ShaderStages::COMPUTE),
                storage_layout_entry(1, wgpu::ShaderStages::COMPUTE, false, sample_byte_len),
                storage_layout_entry(
                    2,
                    wgpu::ShaderStages::COMPUTE,
                    false,
                    normal_height_byte_len,
                ),
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
                storage_layout_entry(
                    3,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    normal_height_byte_len,
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
        let exact_coverage_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_terrain_exact_coverage_layout"),
                entries: &[
                    uniform_layout_entry_with_size(
                        0,
                        wgpu::ShaderStages::FRAGMENT,
                        TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES,
                    ),
                    storage_layout_entry(
                        1,
                        wgpu::ShaderStages::FRAGMENT,
                        true,
                        TERRAIN_EXACT_COVERAGE_MASK_BYTES,
                    ),
                ],
            });
        let material_resources =
            TerrainPreviewMaterialResources::new(device, queue, &material_layout, material_atlas)?;
        let exact_coverage =
            TerrainExactCoverageResources::new(device, queue, &exact_coverage_layout)?;
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
                bind_group_layouts: &[&render_layout, &material_layout, &exact_coverage_layout],
                push_constant_ranges: &[],
            });
        let tree_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_terrain_viewport_tree_pipeline_layout"),
            bind_group_layouts: &[&render_layout, &exact_coverage_layout],
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
        let horizon_pipeline_constants = [(
            "terrain_sample_halo_radius",
            f64::from(TERRAIN_HORIZON_NORMAL_HALO_RADIUS),
        )];
        let horizon_compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("mclone_terrain_horizon_compute_pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &compute_shader,
                entry_point: Some("compute_main"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &horizon_pipeline_constants,
                    ..Default::default()
                },
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
        let horizon_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_horizon_render_pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some("vertex_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &horizon_pipeline_constants,
                        ..Default::default()
                    },
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
            normal_height_scratch_buffer,
            compute_layout,
            render_layout,
            _material_resources: material_resources,
            exact_coverage,
            compute_pipeline,
            horizon_compute_pipeline,
            render_pipeline,
            horizon_render_pipeline,
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
                self.normal_height_scratch_buffer.clone(),
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
                self.normal_height_scratch_buffer.clone(),
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
                        self.normal_height_scratch_buffer.clone(),
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
                        self.normal_height_scratch_buffer.clone(),
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
        pass.set_bind_group(2, &self.exact_coverage.bind_group, &[]);
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
        pass.set_bind_group(1, &self.exact_coverage.bind_group, &[]);
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
    admission: TerrainHorizonAdmission,
    pending: VecDeque<TerrainHorizonResourceTile>,
    vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    vegetation_coordinator: Option<TerrainVegetationCoordinator>,
    vegetation_error: Option<String>,
    seed: i64,
    requested_center_x: i32,
    requested_center_z: i32,
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
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self, String> {
        Self::new_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            config,
            vegetation_executor,
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
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
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
        let admission = TerrainHorizonAdmission::new(config.level_count, config.slots_per_level())?;
        let mut slots = Vec::with_capacity(config.allocation_slots() as usize);
        let horizon_normal_height_byte_len = terrain_horizon_normal_height_byte_len()?;
        let resource_slots_per_level = config
            .slots_per_level()
            .checked_add(TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL)
            .ok_or("terrain horizon resource slots per level overflow")?;
        for level in 0..config.level_count {
            for resource in 0..resource_slots_per_level {
                slots.push(TerrainViewportGpuTile::new_gpu_only(
                    device,
                    &renderer.compute_layout,
                    &renderer.render_layout,
                    renderer.sample_byte_len,
                    horizon_normal_height_byte_len,
                    TerrainViewportTileId {
                        profile: TerrainPreviewProfile::McloneOverworldV1,
                        seed: 0,
                        tile_x: (resource % config.tiles_per_axis) as i32,
                        tile_z: (resource / config.tiles_per_axis) as i32,
                        sample_spacing: config.sample_spacing(level),
                        content_stage: TerrainPreviewContentStage::Cover,
                        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
                    },
                )?);
            }
        }
        debug_assert_eq!(slots.len(), admission.resource_slots() as usize);
        Ok(Self {
            renderer,
            clipmap,
            slots,
            admission,
            pending: VecDeque::with_capacity(config.allocation_slots() as usize),
            vegetation_executor,
            vegetation_coordinator: None,
            vegetation_error: None,
            seed: 0,
            requested_center_x: 0,
            requested_center_z: 0,
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
            self.admission.source_reset();
            self.pending.clear();
            for slot in &mut self.slots {
                slot.clear_vegetation();
            }
            self.renderer.exact_coverage.disable();
        }
        self.seed = seed;
        self.requested_center_x = center_x;
        self.requested_center_z = center_z;
        self.content_stage = content_stage;
        if let Err(error) = self.schedule_requested_transition() {
            self.vegetation_error = Some(error);
        }
    }

    pub fn set_exact_painted_coverage(
        &mut self,
        queue: &wgpu::Queue,
        snapshot: &ExactPaintedCoverageSnapshot,
        mode: TerrainExactCoverageMode,
    ) -> Result<(), String> {
        let expected = TerrainCompositionSourceIdentity::new(
            TerrainPreviewProfile::McloneOverworldV1,
            self.seed,
        );
        if snapshot.source() != expected {
            return Err(format!(
                "exact-painted coverage source {:?} does not match horizon source {:?}",
                snapshot.source(),
                expected
            ));
        }
        self.renderer
            .exact_coverage
            .set_snapshot(queue, snapshot, mode)
    }

    pub fn clear_exact_painted_coverage(&mut self) {
        self.renderer.exact_coverage.disable();
    }

    fn schedule_requested_transition(&mut self) -> Result<(), String> {
        if self.admission.has_staged_levels() {
            return Ok(());
        }
        let mut requested_clipmap = self.clipmap.clone();
        let update =
            requested_clipmap.update_center(self.requested_center_x, self.requested_center_z);
        let (transition, entering) = self
            .admission
            .begin_transition(&requested_clipmap.levels(), &update.rebased_levels)?;
        if transition == TerrainHorizonBeginTransition::Deferred {
            return Ok(());
        }
        self.clipmap = requested_clipmap;
        if transition == TerrainHorizonBeginTransition::Started {
            for resource in entering {
                self.slots[resource.resource_slot as usize].clear_vegetation();
                self.pending.push_back(resource);
            }
        }
        self.refresh_vegetation_desired(self.requested_center_x, self.requested_center_z)
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

    pub fn shutdown_vegetation(&mut self) {
        if let Some(coordinator) = self.vegetation_coordinator.as_mut() {
            coordinator.shutdown();
        }
    }

    pub fn vegetation_shutdown_complete(&self) -> bool {
        self.vegetation_coordinator
            .as_ref()
            .is_none_or(|coordinator| {
                coordinator.state() == TerrainVegetationCoordinatorState::Terminated
            })
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
        self.encode_impl(
            device,
            queue,
            encoder,
            color_view,
            None,
            wgpu::LoadOp::Clear(self.renderer.clear_color),
            wgpu::StoreOp::Store,
            wgpu::LoadOp::Clear(0.0),
            None,
            width,
            height,
            presentation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_to_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: TerrainHorizonRenderTarget<'_>,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.encode_impl(
            device,
            queue,
            encoder,
            target.color_view,
            Some(target.depth_view),
            target.color_load,
            target.color_store,
            target.depth_load,
            Some(target.depth_store),
            width,
            height,
            presentation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_impl(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        external_depth_view: Option<&wgpu::TextureView>,
        color_load: wgpu::LoadOp<wgpu::Color>,
        color_store: wgpu::StoreOp,
        depth_load: wgpu::LoadOp<f32>,
        depth_store: Option<wgpu::StoreOp>,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.resize(device, width, height);
        self.renderer.exact_coverage.sync(queue);
        if !self.admission.has_staged_levels()
            && (self.clipmap.center() != (self.requested_center_x, self.requested_center_z)
                || !self.clipmap.origins_settled())
            && let Err(error) = self.schedule_requested_transition()
        {
            self.vegetation_error = Some(error);
        }
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
            let Some(resource) = self.pending.pop_front() else {
                break;
            };
            let tile = resource.tile;
            let slot_index = resource.resource_slot as usize;
            if self.admission.assignment(resource.resource_slot) != Some(tile)
                || self.admission.slot_generation(resource.resource_slot)
                    != Some(resource.slot_generation)
            {
                continue;
            }
            let tile_id = terrain_horizon_tile_id(self.seed, self.content_stage, tile);
            let slot = &mut self.slots[slot_index];
            slot.request = tile_id.preview_request().validate()?;
            queue.write_buffer(
                &slot.uniform_buffer,
                0,
                &terrain_horizon_uniform_bytes(
                    slot.request,
                    width,
                    height,
                    options,
                    presentation.camera,
                    uniform_presentation,
                    focus_y,
                    None,
                    0,
                ),
            );
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("mclone_terrain_horizon_compute_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.renderer.horizon_compute_pipeline);
                pass.set_bind_group(0, &slot.compute_bind_group, &[]);
                let workgroups =
                    terrain_horizon_samples_per_axis().div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
                pass.dispatch_workgroups(workgroups, workgroups, 1);
            }
            slot.gpu_submitted = true;
            self.admission.mark_ready(resource.resource_slot)?;
            dispatched_refills = dispatched_refills.saturating_add(1);
        }
        self.clipmap.note_refills_completed(dispatched_refills);
        self.dispatched_refills_total = self
            .dispatched_refills_total
            .saturating_add(u64::from(dispatched_refills));
        self.admission.commit_ready_terrain();

        if let Some(error) = self.vegetation_error.take() {
            return Err(error);
        }
        if let Some(coordinator) = self.vegetation_coordinator.as_mut()
            && let Some(admission) = coordinator.pump()
        {
            let slot_index = admission.identity.slot.physical_slot as usize;
            if self
                .admission
                .slot_generation(admission.identity.slot.physical_slot)
                != Some(admission.identity.slot.slot_generation)
                || self
                    .admission
                    .assignment(admission.identity.slot.physical_slot)
                    .is_none_or(|tile| {
                        tile.tile_x != admission.identity.tile.tile_x
                            || tile.tile_z != admission.identity.tile.tile_z
                            || tile.sample_spacing != admission.identity.tile.sample_spacing
                    })
            {
                return Err(
                    "terrain vegetation coordinator admitted a mismatched physical slot".to_owned(),
                );
            }
            self.slots[slot_index].upload_vegetation(device, queue, admission.product)?;
        }
        let slots = &self.slots;
        let seed = self.seed;
        let content_stage = self.content_stage;
        self.admission.commit_ready_vegetation(|resource| {
            slots[resource.resource_slot as usize]
                .vegetation
                .as_ref()
                .is_some_and(|product| {
                    product.request().request()
                        == terrain_horizon_tile_id(seed, content_stage, resource.tile)
                            .preview_request()
                })
        });

        let terrain_levels = self.admission.terrain_presentations();
        for level in &terrain_levels {
            let inner_hole = finer_level_bounds(&terrain_levels, level.snapshot.level);
            for resource in &level.tiles {
                let slot_index = resource.resource_slot as usize;
                let slot = &self.slots[slot_index];
                queue.write_buffer(
                    &slot.uniform_buffer,
                    0,
                    &terrain_horizon_uniform_bytes(
                        slot.request,
                        width,
                        height,
                        options,
                        presentation.camera,
                        uniform_presentation,
                        focus_y,
                        inner_hole,
                        terrain_horizon_outer_edge_flags(
                            level,
                            resource.tile,
                            self.clipmap.config().level_count,
                        ),
                    ),
                );
            }
        }
        let vegetation_levels = self.admission.vegetation_presentations();
        for level in &vegetation_levels {
            if level.snapshot.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING {
                continue;
            }
            let inner_hole = finer_level_bounds(&vegetation_levels, level.snapshot.level);
            for resource in &level.tiles {
                let slot = &self.slots[resource.resource_slot as usize];
                queue.write_buffer(
                    &slot.tree_uniform_buffer,
                    0,
                    &terrain_horizon_uniform_bytes(
                        slot.request,
                        width,
                        height,
                        options,
                        presentation.camera,
                        uniform_presentation,
                        focus_y,
                        inner_hole,
                        0,
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
                        load: color_load,
                        store: color_store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: external_depth_view.unwrap_or(&self.renderer.depth.view),
                    depth_ops: Some(wgpu::Operations {
                        load: depth_load,
                        store: depth_store.unwrap_or(if self.renderer.depth_capture_enabled {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        }),
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.renderer.horizon_render_pipeline);
            pass.set_bind_group(1, &self.renderer._material_resources.bind_group, &[]);
            pass.set_bind_group(2, &self.renderer.exact_coverage.bind_group, &[]);
            for level in terrain_levels.iter().rev() {
                drawn_levels = drawn_levels.saturating_add(1);
                for resource in &level.tiles {
                    let slot_index = resource.resource_slot as usize;
                    pass.set_bind_group(0, &self.slots[slot_index].render_bind_group, &[]);
                    pass.draw(0..TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6, 0..1);
                    drawn_tiles = drawn_tiles.saturating_add(1);
                }
            }
            pass.set_pipeline(&self.renderer.tree_pipeline);
            pass.set_bind_group(1, &self.renderer.exact_coverage.bind_group, &[]);
            for level in &vegetation_levels {
                if level.snapshot.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING {
                    continue;
                }
                for resource in &level.tiles {
                    let slot = &self.slots[resource.resource_slot as usize];
                    let Some(instance_buffer) = slot.tree_instance_buffer.as_ref() else {
                        continue;
                    };
                    pass.set_bind_group(0, &slot.tree_render_bind_group, &[]);
                    pass.set_vertex_buffer(0, instance_buffer.slice(..));
                    pass.draw(
                        0..TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE,
                        0..slot.tree_instance_count,
                    );
                }
            }
        }

        let admission_diagnostics = self.admission.diagnostics();
        let allocation_slots = admission_diagnostics.logical_slots;
        let ready_slots = terrain_levels
            .iter()
            .map(|level| level.tiles.len() as u32)
            .sum();
        let vegetation_resources = vegetation_levels
            .iter()
            .filter(|level| {
                level.snapshot.sample_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
            })
            .flat_map(|level| level.tiles.iter())
            .collect::<Vec<_>>();
        let tree_instance_count = vegetation_resources.iter().fold(0_u32, |count, resource| {
            count.saturating_add(self.slots[resource.resource_slot as usize].tree_instance_count)
        });
        let tree_proxy_vertex_count =
            tree_instance_count.saturating_mul(TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE);
        let vertex_count = drawn_tiles
            .saturating_mul(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6)
            .saturating_add(tree_proxy_vertex_count);
        let normal_halo_samples_per_tile = terrain_horizon_normal_halo_samples_per_tile();
        let normal_halo_fixed_bytes = u64::from(self.admission.resource_slots())
            .saturating_mul(u64::from(normal_halo_samples_per_tile))
            .saturating_mul(size_of::<f32>() as u64);
        let normal_height_fixed_bytes = u64::from(self.admission.resource_slots())
            .saturating_mul(terrain_horizon_normal_height_byte_len()?);
        let fixed_resident_bytes = u64::from(self.admission.resource_slots())
            .saturating_mul(
                self.renderer
                    .sample_byte_len
                    .saturating_add(terrain_horizon_normal_height_byte_len()?)
                    .saturating_add(TERRAIN_PREVIEW_UNIFORM_BYTES.saturating_mul(2)),
            )
            .saturating_add(TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES)
            .saturating_add(TERRAIN_EXACT_COVERAGE_MASK_BYTES);
        let vegetation_bytes = self
            .slots
            .iter()
            .map(|slot| slot.tree_instance_bytes)
            .sum::<u64>();
        let resident_bytes = fixed_resident_bytes.saturating_add(vegetation_bytes);
        let vegetation_ready_tiles = vegetation_resources
            .iter()
            .filter(|resource| {
                self.slots[resource.resource_slot as usize]
                    .vegetation
                    .is_some()
            })
            .count() as u32;
        let (pending_vegetation_tiles, vegetation_settled) = self
            .vegetation_coordinator
            .as_ref()
            .map(|coordinator| {
                let diagnostics = coordinator.diagnostics();
                let pending = diagnostics
                    .queued_tiles
                    .saturating_add(u32::from(diagnostics.in_flight));
                let settled = match diagnostics.state {
                    TerrainVegetationCoordinatorState::Running => {
                        diagnostics.desired_tiles == diagnostics.resident_tiles && pending == 0
                    }
                    TerrainVegetationCoordinatorState::Failed
                    | TerrainVegetationCoordinatorState::Terminated => true,
                    TerrainVegetationCoordinatorState::Starting
                    | TerrainVegetationCoordinatorState::ShuttingDown => false,
                };
                (pending, settled)
            })
            .unwrap_or((0, true));
        let vegetation_service = self.vegetation_service_stats(&vegetation_levels)?;
        if vegetation_service.record_count != tree_instance_count {
            return Err(format!(
                "terrain vegetation receipt counts {} records but GPU slots contain \
                 {tree_instance_count} instances",
                vegetation_service.record_count
            ));
        }
        let target_ready = ready_slots == allocation_slots
            && self.pending.is_empty()
            && !self.admission.has_staged_levels()
            && self.clipmap.center() == (self.requested_center_x, self.requested_center_z)
            && self.clipmap.origins_settled()
            && vegetation_settled;
        Ok(TerrainHorizonFrameStats {
            revision: self.clipmap.diagnostics().revision,
            allocation_slots,
            staging_slots: admission_diagnostics.staging_slots,
            normal_halo_radius: TERRAIN_HORIZON_NORMAL_HALO_RADIUS,
            normal_halo_samples_per_tile,
            normal_halo_fixed_bytes,
            normal_height_fixed_bytes,
            ready_slots,
            requested_levels: admission_diagnostics.requested_levels,
            staged_levels: admission_diagnostics.staged_levels,
            committed_levels: admission_diagnostics.committed_levels,
            vegetation_committed_levels: admission_diagnostics.vegetation_committed_levels,
            atomic_level_commits: admission_diagnostics.atomic_level_commits,
            deferred_transition_attempts: admission_diagnostics.deferred_transition_attempts,
            pending_refills: self.pending.len() as u32,
            dispatched_refills,
            dispatched_refills_total: self.dispatched_refills_total,
            drawn_levels,
            drawn_tiles,
            vertex_count,
            vegetation_ready_tiles,
            pending_vegetation_tiles,
            tree_instance_count,
            tree_proxy_vertex_count,
            fixed_resident_bytes,
            vegetation_bytes,
            resident_bytes,
            exact_coverage_mode: self.renderer.exact_coverage.mode,
            exact_coverage_generation: self.renderer.exact_coverage.mask.generation,
            exact_painted_chunks: self.renderer.exact_coverage.mask.painted_chunks,
            exact_coverage_mask_bytes: TERRAIN_EXACT_COVERAGE_MASK_BYTES,
            vegetation_service,
            finest_sample_spacing: self.clipmap.config().base_sample_spacing,
            coarse_ready: drawn_levels > 0,
            target_ready,
            needs_redraw: !target_ready,
            residency: self.clipmap.diagnostics(),
        })
    }

    fn vegetation_service_stats(
        &self,
        levels: &[TerrainHorizonLevelPresentation],
    ) -> Result<TerrainHorizonVegetationServiceStats, String> {
        let Some(coordinator) = self.vegetation_coordinator.as_ref() else {
            return Ok(TerrainHorizonVegetationServiceStats {
                enabled: self.vegetation_executor.is_some(),
                ..Default::default()
            });
        };
        let diagnostics = coordinator.diagnostics();
        let source = coordinator.source();
        let receipt = terrain_vegetation_coverage_receipt(
            source,
            levels
                .iter()
                .filter(|level| {
                    level.snapshot.sample_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
                })
                .flat_map(|level| level.tiles.iter())
                .filter_map(|resource| {
                    self.slots[resource.resource_slot as usize]
                        .vegetation
                        .as_ref()
                }),
        )?;
        let executor = diagnostics.executor;
        Ok(TerrainHorizonVegetationServiceStats {
            enabled: true,
            coordinator_state: Some(diagnostics.state),
            executor_kind: Some(diagnostics.executor_kind),
            source_fingerprint: receipt.source_fingerprint,
            terrain_source_revision: source.terrain_source_revision,
            compiler_source_revision: source.compiler_source_revision,
            vegetation_plan_revision: source.vegetation_plan_revision,
            product_revision: source.product_revision,
            record_hash: receipt.record_hash,
            family_counts: receipt.family_counts,
            record_count: receipt.record_count,
            product_count: receipt.product_count,
            executor_generation: diagnostics.executor_generation,
            source_epoch: diagnostics.source_epoch,
            coverage_revision: diagnostics.coverage_revision,
            desired_tiles: diagnostics.desired_tiles,
            queued_tiles: diagnostics.queued_tiles,
            resident_tiles: diagnostics.resident_tiles,
            in_flight: diagnostics.in_flight,
            submitted_jobs: diagnostics.submitted_jobs,
            completed_jobs: diagnostics.completed_jobs,
            admitted_products: diagnostics.admitted_products,
            source_resets: diagnostics.source_resets,
            transport_failures: diagnostics.transport_failures,
            executor_restarts: diagnostics.executor_restarts,
            job_failures: diagnostics.job_failures,
            stale_completions: diagnostics
                .stale_generation_completions
                .saturating_add(diagnostics.stale_source_completions)
                .saturating_add(diagnostics.stale_request_completions)
                .saturating_add(diagnostics.stale_slot_completions),
            superseded_completions: diagnostics.superseded_completions,
            submit_full_count: diagnostics.submit_full_count,
            compile_micros: diagnostics.compile_micros,
            cache_cell_requests: diagnostics.cache_cell_requests,
            cache_cell_hits: diagnostics.cache_cell_hits,
            cache_cell_misses: diagnostics.cache_cell_misses,
            cache_retained_cells: diagnostics.cache_retained_cells,
            cache_retained_preliminary_candidates: diagnostics
                .cache_retained_preliminary_candidates,
            executor_submitted_jobs: executor.submitted_jobs,
            executor_completed_jobs: executor.completed_jobs,
            executor_transport_failures: executor.transport_failures,
            executor_restart_count: executor.restarts,
            result_capacity_bytes: executor.result_capacity_bytes,
            result_high_water_bytes: executor.result_high_water_bytes,
            result_overflow_count: executor.result_overflows,
            copied_result_bytes: executor.copied_result_bytes,
            main_decode_micros: executor.main_decode_micros,
        })
    }

    fn refresh_vegetation_desired(&mut self, focus_x: i32, focus_z: i32) -> Result<(), String> {
        if self.vegetation_executor.is_none() && self.vegetation_coordinator.is_none() {
            return Ok(());
        }
        let source = terrain_horizon_vegetation_source(self.seed, self.content_stage)?;
        let desired = self
            .admission
            .current_requested_presentations()
            .into_iter()
            .filter(|level| {
                level.snapshot.sample_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
            })
            .flat_map(|level| level.tiles)
            .map(|resource| TerrainVegetationDesiredTile {
                tile: terrain_horizon_tile_id(self.seed, self.content_stage, resource.tile),
                slot: TerrainVegetationSlotToken {
                    physical_slot: resource.resource_slot,
                    slot_generation: resource.slot_generation,
                },
            })
            .collect::<Vec<_>>();
        if self.vegetation_coordinator.is_none() {
            let executor = self
                .vegetation_executor
                .take()
                .expect("checked terrain vegetation executor");
            if desired.is_empty() {
                return Err(
                    "terrain vegetation was enabled for a clipmap with no record levels".to_owned(),
                );
            }
            self.vegetation_coordinator = Some(TerrainVegetationCoordinator::new(
                executor,
                source,
                desired.len(),
            )?);
        }
        self.vegetation_coordinator
            .as_mut()
            .expect("created terrain vegetation coordinator")
            .update_desired(source, focus_x, focus_z, desired)
    }
}

fn finer_level_bounds(
    levels: &[TerrainHorizonLevelPresentation],
    level: u32,
) -> Option<super::TerrainClipmapBounds> {
    let finer = level.checked_sub(1)?;
    levels
        .iter()
        .find(|candidate| candidate.snapshot.level == finer)
        .map(|candidate| candidate.snapshot.bounds)
}

fn terrain_horizon_samples_per_axis() -> u32 {
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
        .saturating_add(1)
        .saturating_add(TERRAIN_HORIZON_NORMAL_HALO_RADIUS.saturating_mul(2))
}

fn terrain_horizon_normal_halo_samples_per_tile() -> u32 {
    let drawn_samples_per_axis = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.saturating_add(1);
    terrain_horizon_samples_per_axis()
        .saturating_mul(terrain_horizon_samples_per_axis())
        .saturating_sub(drawn_samples_per_axis.saturating_mul(drawn_samples_per_axis))
}

fn terrain_horizon_normal_height_byte_len() -> Result<u64, String> {
    let samples_per_axis = terrain_horizon_samples_per_axis();
    u64::from(
        samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain horizon halo sample count overflow")?,
    )
    .checked_mul(size_of::<f32>() as u64)
    .ok_or_else(|| "terrain horizon normal-height byte size overflow".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn terrain_horizon_uniform_bytes(
    request: ValidatedTerrainPreviewRequest,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
    presentation: super::TerrainPreviewUniformPresentation,
    focus_y: f32,
    inner_hole: Option<super::TerrainClipmapBounds>,
    normal_edge_flags: u32,
) -> Vec<u8> {
    debug_assert_eq!(
        normal_edge_flags
            & !(TERRAIN_HORIZON_NORMAL_EDGE_WEST
                | TERRAIN_HORIZON_NORMAL_EDGE_EAST
                | TERRAIN_HORIZON_NORMAL_EDGE_NORTH
                | TERRAIN_HORIZON_NORMAL_EDGE_SOUTH),
        0
    );
    let mut bytes = viewport_uniform_bytes_for_request_with_presentation(
        request,
        width,
        height,
        options,
        camera,
        presentation,
        focus_y,
        inner_hole,
    );
    const CONTENT_STAGE_FLAGS_W_OFFSET: usize = 8 * 16 + 3 * 4;
    let flag_bytes = bytes
        .get_mut(CONTENT_STAGE_FLAGS_W_OFFSET..CONTENT_STAGE_FLAGS_W_OFFSET + 4)
        .expect("terrain preview uniform content flags remain present");
    let flags = u32::from_le_bytes(
        flag_bytes
            .try_into()
            .expect("terrain preview uniform flag word remains four bytes"),
    ) | normal_edge_flags;
    flag_bytes.copy_from_slice(&flags.to_le_bytes());
    bytes
}

fn terrain_horizon_outer_edge_flags(
    level: &TerrainHorizonLevelPresentation,
    tile: TerrainClipmapTile,
    level_count: u32,
) -> u32 {
    if level.snapshot.level.saturating_add(1) >= level_count {
        return 0;
    }
    let min_tile_x = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_x)
        .min()
        .unwrap_or(tile.tile_x);
    let max_tile_x = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_x)
        .max()
        .unwrap_or(tile.tile_x);
    let min_tile_z = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_z)
        .min()
        .unwrap_or(tile.tile_z);
    let max_tile_z = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_z)
        .max()
        .unwrap_or(tile.tile_z);
    let mut flags = 0;
    if tile.tile_x == min_tile_x {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_WEST;
    }
    if tile.tile_x == max_tile_x {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_EAST;
    }
    if tile.tile_z == min_tile_z {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_NORTH;
    }
    if tile.tile_z == max_tile_z {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_SOUTH;
    }
    flags
}

fn terrain_horizon_tile_id(
    seed: i64,
    content_stage: TerrainPreviewContentStage,
    tile: TerrainClipmapTile,
) -> TerrainViewportTileId {
    TerrainViewportTileId {
        profile: TerrainPreviewProfile::McloneOverworldV1,
        seed,
        tile_x: tile.tile_x,
        tile_z: tile.tile_z,
        sample_spacing: tile.sample_spacing,
        content_stage,
        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
    }
}

fn terrain_horizon_vegetation_source(
    seed: i64,
    content_stage: TerrainPreviewContentStage,
) -> Result<TerrainVegetationSourceIdentity, String> {
    TerrainVegetationSourceIdentity::for_request(TerrainPreviewRequest {
        profile: TerrainPreviewProfile::McloneOverworldV1,
        seed,
        center_x: 0,
        center_z: 0,
        sample_spacing: 1,
        cells_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
        topology: McloneOverworldSamplingTopology::Unbounded,
        content_stage,
        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
    })
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
    fn horizon_halo_keeps_draw_topology_fixed_and_reports_its_cost() {
        let drawn_samples_per_axis = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1;
        assert_eq!(drawn_samples_per_axis, 65);
        assert_eq!(terrain_horizon_samples_per_axis(), 69);
        assert_eq!(terrain_horizon_normal_halo_samples_per_tile(), 536);
        assert_eq!(
            terrain_horizon_normal_height_byte_len().unwrap(),
            u64::from(69_u32.pow(2)) * size_of::<f32>() as u64
        );
    }

    #[test]
    fn horizon_outer_edges_select_the_coarse_normal_footprint() {
        let mut clipmap = TerrainClipmap::new(TerrainClipmapConfig {
            level_count: 2,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        })
        .unwrap();
        clipmap.update_center(0, 0);
        let snapshots = clipmap.levels();
        let presentation =
            |snapshot: super::super::TerrainClipmapLevelSnapshot| TerrainHorizonLevelPresentation {
                tiles: snapshot
                    .tiles
                    .iter()
                    .enumerate()
                    .map(|(slot, tile)| TerrainHorizonResourceTile {
                        tile: *tile,
                        resource_slot: slot as u32,
                        slot_generation: 1,
                    })
                    .collect(),
                snapshot,
            };
        let fine = presentation(snapshots[0].clone());
        let west_north = fine
            .tiles
            .iter()
            .find(|resource| {
                resource.tile.tile_x == fine.snapshot.origin_tile_x
                    && resource.tile.tile_z == fine.snapshot.origin_tile_z
            })
            .unwrap()
            .tile;
        assert_eq!(
            terrain_horizon_outer_edge_flags(&fine, west_north, 2),
            TERRAIN_HORIZON_NORMAL_EDGE_WEST | TERRAIN_HORIZON_NORMAL_EDGE_NORTH
        );
        let interior = fine
            .tiles
            .iter()
            .find(|resource| {
                resource.tile.tile_x == fine.snapshot.origin_tile_x + 1
                    && resource.tile.tile_z == fine.snapshot.origin_tile_z + 1
            })
            .unwrap()
            .tile;
        assert_eq!(terrain_horizon_outer_edge_flags(&fine, interior, 2), 0);

        let coarse = presentation(snapshots[1].clone());
        assert_eq!(
            terrain_horizon_outer_edge_flags(&coarse, coarse.tiles[0].tile, 2),
            0
        );

        let shared_boundary = 256_i32;
        let fine_wide_footprint = (
            shared_boundary - 2 * fine.snapshot.sample_spacing as i32,
            shared_boundary + 2 * fine.snapshot.sample_spacing as i32,
        );
        let coarse_narrow_footprint = (
            shared_boundary - coarse.snapshot.sample_spacing as i32,
            shared_boundary + coarse.snapshot.sample_spacing as i32,
        );
        assert_eq!(fine_wide_footprint, coarse_narrow_footprint);
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
