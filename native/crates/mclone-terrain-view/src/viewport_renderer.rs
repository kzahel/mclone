use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU64;
use std::sync::mpsc;
use std::time::Instant;

use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TerrainPreviewComparison, TerrainPreviewReferenceGrid,
    TerrainPreviewSample,
};

use super::{
    TERRAIN_PREVIEW_DEPTH_FORMAT, TERRAIN_PREVIEW_RENDER_WGSL, TERRAIN_PREVIEW_SAMPLE_BYTES,
    TERRAIN_PREVIEW_UNIFORM_BYTES, TERRAIN_PREVIEW_WORKGROUP_AXIS, TerrainPreviewCamera,
    TerrainPreviewDrawOptions, TerrainPreviewSource, TerrainViewportPlan, TerrainViewportTileId,
    parse_samples, preview_instance_count, terrain_preview_compute_wgsl, viewport_uniform_bytes,
};

pub const TERRAIN_VIEWPORT_MAX_RESIDENT_TILES: usize = 192;
pub const TERRAIN_VIEWPORT_TILE_COMPILES_PER_FRAME: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainViewportFrameStats {
    pub revision: u64,
    pub requested_spacing: u32,
    pub effective_spacing: u32,
    pub published_spacing: u32,
    pub view_width_blocks: u32,
    pub view_height_blocks: u32,
    pub level_count: u32,
    pub visible_tile_count: u32,
    pub published_tile_count: u32,
    pub resident_tile_count: u32,
    pub queued_tile_count: u32,
    pub pending_readback_count: u32,
    pub compiled_tiles: u32,
    pub compiled_tiles_total: u64,
    pub evicted_tiles_total: u64,
    pub stale_result_count: u64,
    pub sample_count: u32,
    pub vertex_count: u32,
    pub reference_bytes: u64,
    pub gpu_sample_bytes: u64,
    pub readback_bytes: u64,
    pub resident_bytes: u64,
    pub cpu_reference_micros: u64,
    pub coarse_ready: bool,
    pub target_ready: bool,
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

pub struct EncodedTerrainViewportReadbacks {
    readbacks: Vec<EncodedTerrainViewportReadback>,
}

struct EncodedTerrainViewportReadback {
    tile: TerrainViewportTileId,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
}

struct PendingTerrainViewportReadback {
    tile: TerrainViewportTileId,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

struct TerrainViewportDepthTarget {
    _texture: wgpu::Texture,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
            width,
            height,
        }
    }
}

struct TerrainViewportGpuTile {
    reference: TerrainPreviewReferenceGrid,
    gpu_samples: Option<Vec<TerrainPreviewSample>>,
    uniform_buffer: wgpu::Buffer,
    _gpu_sample_buffer: wgpu::Buffer,
    _reference_sample_buffer: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    last_used: u64,
}

impl TerrainViewportGpuTile {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        reference: TerrainPreviewReferenceGrid,
    ) -> Result<Self, String> {
        let reference_bytes = reference.packed_bytes();
        if reference_bytes.len() as u64 != sample_byte_len {
            return Err(format!(
                "terrain viewport reference upload is {} bytes, expected {}",
                reference_bytes.len(),
                sample_byte_len
            ));
        }
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_tile_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_gpu_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let reference_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_reference_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&reference_sample_buffer, 0, &reference_bytes);
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
                    resource: reference_sample_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            reference,
            gpu_samples: None,
            uniform_buffer,
            _gpu_sample_buffer: gpu_sample_buffer,
            _reference_sample_buffer: reference_sample_buffer,
            compute_bind_group,
            render_bind_group,
            last_used: 0,
        })
    }
}

pub struct TerrainViewportRenderer {
    sample_count_per_tile: u32,
    sample_byte_len: u64,
    compute_layout: wgpu::BindGroupLayout,
    render_layout: wgpu::BindGroupLayout,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    depth: TerrainViewportDepthTarget,
    cache: HashMap<TerrainViewportTileId, TerrainViewportGpuTile>,
    pending: Vec<PendingTerrainViewportReadback>,
    plan: Option<TerrainViewportPlan>,
    queue: VecDeque<TerrainViewportTileId>,
    published_level: Option<usize>,
    latest_revision: u64,
    last_comparison_revision: Option<u64>,
    use_clock: u64,
    compiled_tiles_total: u64,
    evicted_tiles_total: u64,
    stale_result_count: u64,
}

impl TerrainViewportRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
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
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(terrain_preview_compute_wgsl().into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_render_shader"),
            source: wgpu::ShaderSource::Wgsl(TERRAIN_PREVIEW_RENDER_WGSL.into()),
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
                depth_compare: wgpu::CompareFunction::LessEqual,
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
            compute_pipeline,
            render_pipeline,
            depth: TerrainViewportDepthTarget::new(device, width, height),
            cache: HashMap::new(),
            pending: Vec::new(),
            plan: None,
            queue: VecDeque::new(),
            published_level: None,
            latest_revision: 0,
            last_comparison_revision: None,
            use_clock: 0,
            compiled_tiles_total: 0,
            evicted_tiles_total: 0,
            stale_result_count: 0,
        })
    }

    pub fn set_viewport(&mut self, revision: u64, plan: TerrainViewportPlan) {
        self.latest_revision = revision;
        if self.plan.as_ref() == Some(&plan) {
            return;
        }
        self.stale_result_count = self
            .stale_result_count
            .saturating_add(self.queue.len() as u64);
        self.queue.clear();
        self.published_level = None;
        self.plan = Some(plan);
        self.rebuild_queue();
        self.refresh_published_level();
        self.touch_current_tiles();
        self.evict_unused_tiles();
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth.width != width || self.depth.height != height {
            self.depth = TerrainViewportDepthTarget::new(device, width, height);
        }
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
    ) -> Result<(TerrainViewportFrameStats, EncodedTerrainViewportReadbacks), String> {
        self.resize(device, width, height);
        let plan = self
            .plan
            .clone()
            .ok_or("terrain viewport renderer has no active plan")?;
        let mut encoded_readbacks = Vec::new();
        let mut compiled_tiles = 0_u32;
        let mut cpu_reference_micros = 0_u64;

        for _ in 0..TERRAIN_VIEWPORT_TILE_COMPILES_PER_FRAME {
            let Some(tile_id) = self.take_next_missing_tile() else {
                break;
            };
            let compile_started = Instant::now();
            let reference = TerrainPreviewReferenceGrid::compile(tile_id.preview_request())?;
            cpu_reference_micros = cpu_reference_micros.saturating_add(
                u64::try_from(compile_started.elapsed().as_micros()).unwrap_or(u64::MAX),
            );
            let mut tile = TerrainViewportGpuTile::new(
                device,
                queue,
                &self.compute_layout,
                &self.render_layout,
                self.sample_byte_len,
                reference,
            )?;
            queue.write_buffer(
                &tile.uniform_buffer,
                0,
                &viewport_uniform_bytes(
                    &tile.reference,
                    width,
                    height,
                    options,
                    camera,
                    plan.request.center_x,
                    plan.request.center_z,
                    plan.view_width_blocks,
                    plan.view_height_blocks,
                ),
            );
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
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
            encoder.copy_buffer_to_buffer(
                &tile._gpu_sample_buffer,
                0,
                &readback_buffer,
                0,
                self.sample_byte_len,
            );
            self.use_clock = self.use_clock.saturating_add(1);
            tile.last_used = self.use_clock;
            self.cache.insert(tile_id, tile);
            encoded_readbacks.push(EncodedTerrainViewportReadback {
                tile: tile_id,
                readback_buffer,
                byte_len: self.sample_byte_len,
            });
            compiled_tiles = compiled_tiles.saturating_add(1);
            self.compiled_tiles_total = self.compiled_tiles_total.saturating_add(1);
        }

        self.refresh_published_level();
        let published_tiles = self.published_tiles().to_vec();
        for tile_id in &published_tiles {
            let tile = self
                .cache
                .get_mut(tile_id)
                .expect("published terrain viewport tiles are resident");
            queue.write_buffer(
                &tile.uniform_buffer,
                0,
                &viewport_uniform_bytes(
                    &tile.reference,
                    width,
                    height,
                    options,
                    camera,
                    plan.request.center_x,
                    plan.request.center_z,
                    plan.view_width_blocks,
                    plan.view_height_blocks,
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
            options.source,
            &published_tiles,
        );
        self.evict_unused_tiles();

        let target = plan.target_level();
        let coarse_ready = plan
            .coarsest_level()
            .visible_tiles
            .iter()
            .all(|tile| self.cache.contains_key(tile));
        let target_ready = target
            .visible_tiles
            .iter()
            .all(|tile| self.cache.contains_key(tile));
        let instance_count = preview_instance_count(options.source);
        let published_tile_count = u32::try_from(published_tiles.len()).unwrap_or(u32::MAX);
        let sample_count = published_tile_count.saturating_mul(self.sample_count_per_tile);
        let vertex_count = published_tile_count
            .saturating_mul(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6)
            .saturating_mul(instance_count);
        let resident_tile_count = u32::try_from(self.cache.len()).unwrap_or(u32::MAX);
        let stats = TerrainViewportFrameStats {
            revision: self.latest_revision,
            requested_spacing: plan.requested_spacing,
            effective_spacing: plan.effective_spacing,
            published_spacing: self
                .published_level
                .map(|index| plan.levels[index].sample_spacing)
                .unwrap_or(0),
            view_width_blocks: plan.view_width_blocks,
            view_height_blocks: plan.view_height_blocks,
            level_count: u32::try_from(plan.levels.len()).unwrap_or(u32::MAX),
            visible_tile_count: u32::try_from(target.visible_tiles.len()).unwrap_or(u32::MAX),
            published_tile_count,
            resident_tile_count,
            queued_tile_count: u32::try_from(self.queue.len()).unwrap_or(u32::MAX),
            pending_readback_count: u32::try_from(self.pending.len() + encoded_readbacks.len())
                .unwrap_or(u32::MAX),
            compiled_tiles,
            compiled_tiles_total: self.compiled_tiles_total,
            evicted_tiles_total: self.evicted_tiles_total,
            stale_result_count: self.stale_result_count,
            sample_count,
            vertex_count,
            reference_bytes: u64::from(resident_tile_count) * self.sample_byte_len,
            gpu_sample_bytes: u64::from(resident_tile_count) * self.sample_byte_len,
            readback_bytes: u64::try_from(encoded_readbacks.len()).unwrap_or(u64::MAX)
                * self.sample_byte_len,
            resident_bytes: u64::from(resident_tile_count)
                * (self.sample_byte_len * 2 + TERRAIN_PREVIEW_UNIFORM_BYTES),
            cpu_reference_micros,
            coarse_ready,
            target_ready,
            budget_limited: plan.budget_limited,
            needs_redraw: !self.queue.is_empty(),
        };
        Ok((
            stats,
            EncodedTerrainViewportReadbacks {
                readbacks: encoded_readbacks,
            },
        ))
    }

    pub fn mark_submitted(&mut self, encoded: EncodedTerrainViewportReadbacks) {
        for readback in encoded.readbacks {
            let (sender, receiver) = mpsc::channel();
            readback
                .readback_buffer
                .slice(..readback.byte_len)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.pending.push(PendingTerrainViewportReadback {
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

    fn rebuild_queue(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        let mut scheduled = HashSet::new();
        for level in &plan.levels {
            for tile in &level.visible_tiles {
                if !self.cache.contains_key(tile) && scheduled.insert(*tile) {
                    self.queue.push_back(*tile);
                }
            }
        }
        for tile in &plan.target_level().preload_tiles {
            if !self.cache.contains_key(tile) && scheduled.insert(*tile) {
                self.queue.push_back(*tile);
            }
        }
    }

    fn take_next_missing_tile(&mut self) -> Option<TerrainViewportTileId> {
        while let Some(tile) = self.queue.pop_front() {
            if !self.cache.contains_key(&tile) {
                return Some(tile);
            }
        }
        None
    }

    fn refresh_published_level(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            self.published_level = None;
            return;
        };
        self.published_level = plan
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| {
                level
                    .visible_tiles
                    .iter()
                    .all(|tile| self.cache.contains_key(tile))
            })
            .map(|(index, _)| index)
            .next_back();
    }

    fn published_tiles(&self) -> &[TerrainViewportTileId] {
        self.plan
            .as_ref()
            .zip(self.published_level)
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
            protected.extend(plan.target_level().preload_tiles.iter().copied());
        }
        protected.extend(self.pending.iter().map(|pending| pending.tile));
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

    fn encode_draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        source: TerrainPreviewSource,
        tiles: &[TerrainViewportTileId],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_terrain_viewport_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.025,
                        g: 0.035,
                        b: 0.055,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.render_pipeline);
        let panels = render_panels(source, width, height);
        for panel in panels {
            pass.set_scissor_rect(panel.x, panel.y, panel.width, panel.height);
            for tile_id in tiles {
                let tile = self
                    .cache
                    .get(tile_id)
                    .expect("drawn terrain viewport tiles are resident");
                pass.set_bind_group(0, &tile.render_bind_group, &[]);
                pass.draw(
                    0..TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6,
                    panel.instance..panel.instance + 1,
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
            reference_samples.extend_from_slice(tile.reference.samples());
            gpu_samples.extend_from_slice(samples);
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

#[derive(Clone, Copy)]
struct RenderPanel {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    instance: u32,
}

fn render_panels(source: TerrainPreviewSource, width: u32, height: u32) -> Vec<RenderPanel> {
    if source != TerrainPreviewSource::Split {
        return vec![RenderPanel {
            x: 0,
            y: 0,
            width,
            height,
            instance: 0,
        }];
    }
    if width <= height {
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

fn uniform_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(TERRAIN_PREVIEW_UNIFORM_BYTES),
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

    #[test]
    fn compare_panels_match_shader_layout() {
        assert_eq!(
            render_panels(TerrainPreviewSource::Split, 1200, 700).len(),
            2
        );
        let wide = render_panels(TerrainPreviewSource::Split, 1200, 700);
        assert_eq!((wide[0].width, wide[1].x), (600, 600));
        let tall = render_panels(TerrainPreviewSource::Split, 400, 900);
        assert_eq!((tall[0].height, tall[1].y), (450, 450));
        assert_eq!(render_panels(TerrainPreviewSource::Gpu, 400, 900).len(), 1);
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
}
