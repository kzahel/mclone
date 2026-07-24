#![forbid(unsafe_code)]

mod viewport;

use std::fmt::Write;
use std::num::NonZeroU64;
use std::sync::mpsc;

use mclone_worldgen::levelgen::{MCLONE_OVERWORLD_LARGE_FIELD_SPEC, McloneOverworldLargeFieldBand};
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_SAMPLE_FLOATS, TerrainPreviewComparison, TerrainPreviewReferenceGrid,
    TerrainPreviewSample,
};

pub use viewport::{
    TERRAIN_VIEWPORT_AUTO_PIXELS_PER_CELL, TERRAIN_VIEWPORT_MAX_BLOCKS_ACROSS,
    TERRAIN_VIEWPORT_MAX_VISIBLE_TILES_PER_AXIS, TERRAIN_VIEWPORT_MIN_BLOCKS_ACROSS,
    TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES, TerrainViewportDetail, TerrainViewportLevel,
    TerrainViewportPlan, TerrainViewportRequest, TerrainViewportTileId, plan_terrain_viewport,
};

pub const TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION: &str = "mclone-overworld-v1-gpu-preview-a2";
pub const TERRAIN_PREVIEW_COMPUTE_WGSL_TEMPLATE: &str =
    include_str!("shaders/terrain_preview_compute.wgsl");
pub const TERRAIN_PREVIEW_RENDER_WGSL: &str = include_str!("shaders/terrain_preview_render.wgsl");

const TERRAIN_PREVIEW_UNIFORM_BYTES: u64 = 64;
const TERRAIN_PREVIEW_SAMPLE_BYTES: u64 =
    (TERRAIN_PREVIEW_SAMPLE_FLOATS * std::mem::size_of::<f32>()) as u64;
const TERRAIN_PREVIEW_WORKGROUP_AXIS: u32 = 8;
const TERRAIN_PREVIEW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub fn terrain_preview_compute_wgsl() -> String {
    let spec = MCLONE_OVERWORLD_LARGE_FIELD_SPEC;
    let mut constants = String::new();
    for (name, band) in [
        ("CONTINENT_LARGE", spec.continent[0]),
        ("CONTINENT_MEDIUM", spec.continent[1]),
        ("CONTINENT_DETAIL", spec.continent[2]),
        ("RELIEF_LARGE", spec.relief[0]),
        ("RELIEF_DETAIL", spec.relief[1]),
        ("RELIEF_FINE", spec.relief[2]),
        ("RUGGEDNESS_LARGE", spec.ruggedness[0]),
        ("RUGGEDNESS_DETAIL", spec.ruggedness[1]),
        ("RIDGE_LARGE", spec.ridge[0]),
        ("RIDGE_DETAIL", spec.ridge[1]),
        ("MOUNTAIN_DETAIL_LARGE", spec.mountain_detail[0]),
        ("MOUNTAIN_DETAIL_FINE", spec.mountain_detail[1]),
        ("OCEAN_BASIN", spec.ocean_basin),
        ("SEABED_LARGE", spec.seabed[0]),
        ("SEABED_DETAIL", spec.seabed[1]),
        ("TEMPERATURE_LARGE", spec.temperature[0]),
        ("TEMPERATURE_DETAIL", spec.temperature[1]),
        ("MOISTURE_LARGE", spec.moisture[0]),
        ("MOISTURE_DETAIL", spec.moisture[1]),
    ] {
        write_field_constants(&mut constants, name, band);
    }
    TERRAIN_PREVIEW_COMPUTE_WGSL_TEMPLATE
        .replace("// __MCLONE_PRODUCTION_FIELD_CONSTANTS__", &constants)
}

fn write_field_constants(
    destination: &mut String,
    name: &str,
    band: McloneOverworldLargeFieldBand,
) {
    let low = band.domain as u32;
    let high = (band.domain >> 32) as u32;
    writeln!(
        destination,
        "const {name}_DOMAIN: U64 = U64(0x{low:08x}u, 0x{high:08x}u);"
    )
    .expect("writing terrain preview WGSL constants to String cannot fail");
    writeln!(destination, "const {name}_SCALE: i32 = {};", band.scale)
        .expect("writing terrain preview WGSL constants to String cannot fail");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewSource {
    Gpu = 0,
    Reference = 1,
    Split = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewView {
    Map = 0,
    ThreeDimensional = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainPreviewLayer {
    Terrain = 0,
    Height = 1,
    Error = 2,
    Continentalness = 3,
    Climate = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainPreviewDrawOptions {
    pub source: TerrainPreviewSource,
    pub view: TerrainPreviewView,
    pub layer: TerrainPreviewLayer,
}

impl Default for TerrainPreviewDrawOptions {
    fn default() -> Self {
        Self {
            source: TerrainPreviewSource::Reference,
            view: TerrainPreviewView::ThreeDimensional,
            layer: TerrainPreviewLayer::Terrain,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewCamera {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

impl TerrainPreviewCamera {
    pub fn new(yaw_radians: f32, pitch_radians: f32) -> Result<Self, String> {
        if !yaw_radians.is_finite() || !pitch_radians.is_finite() {
            return Err("terrain preview camera angles must be finite".to_owned());
        }
        Ok(Self {
            yaw_radians,
            pitch_radians: pitch_radians.clamp(0.12, 1.25),
        })
    }
}

impl Default for TerrainPreviewCamera {
    fn default() -> Self {
        Self {
            yaw_radians: std::f32::consts::FRAC_PI_4,
            pitch_radians: 0.48,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainPreviewFrameStats {
    pub revision: u64,
    pub cells_per_axis: u32,
    pub samples_per_axis: u32,
    pub sample_count: u32,
    pub vertex_count: u32,
    pub footprint_blocks: u32,
    pub reference_bytes: u64,
    pub gpu_sample_bytes: u64,
    pub readback_bytes: u64,
    pub resident_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPreviewCompletedComparison {
    pub revision: u64,
    pub comparison: TerrainPreviewComparison,
}

pub struct EncodedTerrainPreviewReadback {
    revision: u64,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    reference: TerrainPreviewReferenceGrid,
}

struct PendingTerrainPreviewReadback {
    revision: u64,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    reference: TerrainPreviewReferenceGrid,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

struct TerrainPreviewDepthTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl TerrainPreviewDepthTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_preview_depth"),
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

pub struct TerrainPreviewRenderer {
    cells_per_axis: u32,
    samples_per_axis: u32,
    sample_count: u32,
    sample_byte_len: u64,
    uniform_buffer: wgpu::Buffer,
    gpu_sample_buffer: wgpu::Buffer,
    reference_sample_buffer: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    depth: TerrainPreviewDepthTarget,
    pending: Vec<PendingTerrainPreviewReadback>,
    latest_requested_revision: u64,
    stale_result_count: u64,
}

impl TerrainPreviewRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        cells_per_axis: u32,
    ) -> Result<Self, String> {
        if cells_per_axis == 0 || !cells_per_axis.is_power_of_two() {
            return Err(format!(
                "terrain preview renderer cells per axis must be a non-zero power of two, got \
                 {cells_per_axis}"
            ));
        }
        let samples_per_axis = cells_per_axis
            .checked_add(1)
            .ok_or("terrain preview renderer sample axis overflow")?;
        let sample_count = samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain preview renderer sample count overflow")?;
        let sample_byte_len = u64::from(sample_count)
            .checked_mul(TERRAIN_PREVIEW_SAMPLE_BYTES)
            .ok_or("terrain preview renderer buffer size overflow")?;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_gpu_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let reference_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_reference_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_preview_compute_layout"),
            entries: &[
                uniform_layout_entry(
                    0,
                    wgpu::ShaderStages::COMPUTE,
                    TERRAIN_PREVIEW_UNIFORM_BYTES,
                ),
                storage_layout_entry(1, wgpu::ShaderStages::COMPUTE, false, sample_byte_len),
            ],
        });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_preview_render_layout"),
            entries: &[
                uniform_layout_entry(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    TERRAIN_PREVIEW_UNIFORM_BYTES,
                ),
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
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_preview_compute_bind_group"),
            layout: &compute_layout,
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
            label: Some("mclone_terrain_preview_render_bind_group"),
            layout: &render_layout,
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

        let compute_shader_source = terrain_preview_compute_wgsl();
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_preview_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(compute_shader_source.into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_preview_render_shader"),
            source: wgpu::ShaderSource::Wgsl(TERRAIN_PREVIEW_RENDER_WGSL.into()),
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_preview_compute_pipeline_layout"),
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_preview_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mclone_terrain_preview_compute_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("compute_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_terrain_preview_render_pipeline"),
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
            cells_per_axis,
            samples_per_axis,
            sample_count,
            sample_byte_len,
            uniform_buffer,
            gpu_sample_buffer,
            reference_sample_buffer,
            compute_bind_group,
            render_bind_group,
            compute_pipeline,
            render_pipeline,
            depth: TerrainPreviewDepthTarget::new(device, width, height),
            pending: Vec::new(),
            latest_requested_revision: 0,
            stale_result_count: 0,
        })
    }

    pub const fn stale_result_count(&self) -> u64 {
        self.stale_result_count
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth.width == width && self.depth.height == height {
            return;
        }
        self.depth = TerrainPreviewDepthTarget::new(device, width, height);
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
        revision: u64,
        reference: &TerrainPreviewReferenceGrid,
        options: TerrainPreviewDrawOptions,
        camera: TerrainPreviewCamera,
    ) -> Result<(TerrainPreviewFrameStats, EncodedTerrainPreviewReadback), String> {
        let request = reference.request();
        if request.request().cells_per_axis != self.cells_per_axis
            || request.samples_per_axis() != self.samples_per_axis
            || request.sample_count() != self.sample_count
        {
            return Err(format!(
                "terrain preview grid is {} cells/{} samples but renderer is {} cells/{} samples",
                request.request().cells_per_axis,
                request.sample_count(),
                self.cells_per_axis,
                self.sample_count
            ));
        }
        self.resize(device, width, height);
        self.latest_requested_revision = revision;

        let reference_bytes = reference.packed_bytes();
        if reference_bytes.len() as u64 != self.sample_byte_len {
            return Err(format!(
                "terrain preview reference upload is {} bytes, expected {}",
                reference_bytes.len(),
                self.sample_byte_len
            ));
        }
        queue.write_buffer(&self.reference_sample_buffer, 0, &reference_bytes);
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &uniform_bytes(reference, width, height, options, camera),
        );

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mclone_terrain_preview_compute_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_bind_group, &[]);
            let workgroups = self
                .samples_per_axis
                .div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
            pass.dispatch_workgroups(workgroups, workgroups, 1);
        }

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_readback"),
            size: self.sample_byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(
            &self.gpu_sample_buffer,
            0,
            &readback_buffer,
            0,
            self.sample_byte_len,
        );

        let instance_count = preview_instance_count(options.source);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_terrain_preview_render_pass"),
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
            pass.set_bind_group(0, &self.render_bind_group, &[]);
            pass.draw(
                0..self.cells_per_axis * self.cells_per_axis * 6,
                0..instance_count,
            );
        }

        let stats = TerrainPreviewFrameStats {
            revision,
            cells_per_axis: self.cells_per_axis,
            samples_per_axis: self.samples_per_axis,
            sample_count: self.sample_count,
            vertex_count: self.cells_per_axis * self.cells_per_axis * 6 * instance_count,
            footprint_blocks: request.footprint_blocks(),
            reference_bytes: self.sample_byte_len,
            gpu_sample_bytes: self.sample_byte_len,
            readback_bytes: self.sample_byte_len,
            resident_bytes: self.sample_byte_len * 2 + TERRAIN_PREVIEW_UNIFORM_BYTES,
        };
        Ok((
            stats,
            EncodedTerrainPreviewReadback {
                revision,
                readback_buffer,
                byte_len: self.sample_byte_len,
                reference: reference.clone(),
            },
        ))
    }

    pub fn mark_submitted(&mut self, encoded: EncodedTerrainPreviewReadback) {
        let (sender, receiver) = mpsc::channel();
        encoded.readback_buffer.slice(..encoded.byte_len).map_async(
            wgpu::MapMode::Read,
            move |result| {
                let _ = sender.send(result);
            },
        );
        self.pending.push(PendingTerrainPreviewReadback {
            revision: encoded.revision,
            readback_buffer: encoded.readback_buffer,
            byte_len: encoded.byte_len,
            reference: encoded.reference,
            receiver,
        });
    }

    pub fn poll_completed(
        &mut self,
        device: &wgpu::Device,
    ) -> Vec<Result<TerrainPreviewCompletedComparison, String>> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut completed = Vec::new();
        let mut index = 0;
        while index < self.pending.len() {
            match self.pending[index].receiver.try_recv() {
                Ok(Ok(())) => {
                    let pending = self.pending.remove(index);
                    let result = self.read_completed(&pending);
                    pending.readback_buffer.unmap();
                    if pending.revision == self.latest_requested_revision {
                        completed.push(result.map(|comparison| {
                            TerrainPreviewCompletedComparison {
                                revision: pending.revision,
                                comparison,
                            }
                        }));
                    } else {
                        self.stale_result_count = self.stale_result_count.saturating_add(1);
                    }
                }
                Ok(Err(error)) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain preview revision {} readback failed: {error}",
                        pending.revision
                    )));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    index += 1;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain preview revision {} readback callback disconnected",
                        pending.revision
                    )));
                }
            }
        }
        completed
    }

    fn read_completed(
        &self,
        pending: &PendingTerrainPreviewReadback,
    ) -> Result<TerrainPreviewComparison, String> {
        let mapped = pending
            .readback_buffer
            .slice(..pending.byte_len)
            .get_mapped_range();
        let result = parse_samples(&mapped)
            .and_then(|samples| TerrainPreviewComparison::compare(&pending.reference, &samples));
        drop(mapped);
        result
    }
}

const fn preview_instance_count(source: TerrainPreviewSource) -> u32 {
    match source {
        TerrainPreviewSource::Split => 2,
        TerrainPreviewSource::Gpu | TerrainPreviewSource::Reference => 1,
    }
}

fn uniform_layout_entry(
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

fn uniform_bytes(
    reference: &TerrainPreviewReferenceGrid,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
) -> Vec<u8> {
    let request = reference.request();
    let source = request.request();
    let seed = source.seed as u64;
    let words = [
        request.min_x() as u32,
        request.min_z() as u32,
        source.sample_spacing,
        source.cells_per_axis,
        seed as u32,
        (seed >> 32) as u32,
        options.source as u32,
        options.view as u32,
        options.layer as u32,
        request.samples_per_axis(),
        width.max(1),
        height.max(1),
    ];
    let mut bytes = Vec::with_capacity(TERRAIN_PREVIEW_UNIFORM_BYTES as usize);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for value in [camera.yaw_radians, camera.pitch_radians, 0.0, 0.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn parse_samples(bytes: &[u8]) -> Result<Vec<TerrainPreviewSample>, String> {
    let sample_bytes = TERRAIN_PREVIEW_SAMPLE_BYTES as usize;
    if bytes.is_empty() || bytes.len() % sample_bytes != 0 {
        return Err(format!(
            "terrain preview readback byte length {} is not a positive multiple of {}",
            bytes.len(),
            sample_bytes
        ));
    }
    let mut samples = Vec::with_capacity(bytes.len() / sample_bytes);
    for packed_sample in bytes.chunks_exact(sample_bytes) {
        let mut values = [0.0_f32; TERRAIN_PREVIEW_SAMPLE_FLOATS];
        for (index, value) in values.iter_mut().enumerate() {
            let offset = index * std::mem::size_of::<f32>();
            let value_bytes: [u8; 4] = packed_sample[offset..offset + 4]
                .try_into()
                .expect("terrain preview readback chunks are exact");
            *value = f32::from_le_bytes(value_bytes);
        }
        samples.push(TerrainPreviewSample::from_packed(values));
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::terrain_preview::TerrainPreviewRequest;

    fn validate_shader(source: &str, entry_point: &str) {
        let module = naga::front::wgsl::parse_str(source).expect("terrain preview WGSL parses");
        assert!(
            module
                .entry_points
                .iter()
                .any(|entry| entry.name == entry_point)
        );
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("terrain preview WGSL validates");
    }

    #[test]
    fn compute_and_render_shaders_validate() {
        validate_shader(&terrain_preview_compute_wgsl(), "compute_main");
        validate_shader(TERRAIN_PREVIEW_RENDER_WGSL, "vertex_main");
        validate_shader(TERRAIN_PREVIEW_RENDER_WGSL, "fragment_main");
    }

    #[test]
    fn uniform_packing_matches_wgsl_layout() {
        let reference =
            TerrainPreviewReferenceGrid::compile(TerrainPreviewRequest::new(12_345, -64, 96, 16))
                .unwrap();
        let camera = TerrainPreviewCamera::new(-0.75, 0.65).unwrap();
        let bytes = uniform_bytes(
            &reference,
            1280,
            720,
            TerrainPreviewDrawOptions::default(),
            camera,
        );
        assert_eq!(bytes.len(), TERRAIN_PREVIEW_UNIFORM_BYTES as usize);
        assert_eq!(
            i32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            reference.request().min_x()
        );
        assert_eq!(
            i32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            reference.request().min_z()
        );
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 16);
        assert_eq!(u32::from_le_bytes(bytes[36..40].try_into().unwrap()), 65);
        assert_eq!(f32::from_le_bytes(bytes[48..52].try_into().unwrap()), -0.75);
        assert_eq!(f32::from_le_bytes(bytes[52..56].try_into().unwrap()), 0.65);
    }

    #[test]
    fn parses_packed_samples_and_rejects_misalignment() {
        let sample = TerrainPreviewSample {
            surface_y: 64.0,
            display_y: 64.0,
            continentalness: 0.2,
            relief: -0.1,
            temperature: 0.4,
            moisture: 0.6,
            water: 0.0,
            ruggedness: 0.8,
            base_surface_y: 63.0,
            base_display_y: 64.0,
            ocean_water: 0.0,
        };
        let mut bytes = Vec::new();
        for value in sample.packed() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(parse_samples(&bytes).unwrap(), vec![sample]);
        bytes.push(0);
        assert!(parse_samples(&bytes).is_err());
    }

    #[test]
    fn evaluator_revision_and_production_spec_are_explicit() {
        assert_eq!(
            TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
            "mclone-overworld-v1-gpu-preview-a2"
        );
        let shader = terrain_preview_compute_wgsl();
        assert!(!shader.contains("__MCLONE_PRODUCTION_FIELD_CONSTANTS__"));
        assert!(
            shader.contains("const CONTINENT_LARGE_DOMAIN: U64 = U64(0x636f6e31u, 0x6d636f76u);")
        );
        assert!(shader.contains("const MOUNTAIN_DETAIL_FINE_SCALE: i32 = 8;"));
        assert!(shader.contains("fn splitmix64"));
        assert!(shader.contains("fn gradient_noise"));
        assert!(!shader.contains("band_weight"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("error_color"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("@builtin(instance_index)"));
        assert!(TERRAIN_PREVIEW_RENDER_WGSL.contains("fn reference_base_sample"));
        assert!(
            TERRAIN_PREVIEW_RENDER_WGSL
                .contains("clip_y = (world_height * cos(pitch) - camera_depth * sin(pitch))")
        );
        assert!(
            TERRAIN_PREVIEW_RENDER_WGSL
                .contains("let stacked_compare = compare && width <= height")
        );
        assert_eq!(preview_instance_count(TerrainPreviewSource::Split), 2);
        assert_eq!(preview_instance_count(TerrainPreviewSource::Reference), 1);
    }
}
