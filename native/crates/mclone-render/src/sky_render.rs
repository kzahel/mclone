//! Sky dome, sunrise/sunset glow, and visible sun render pass.
//!
//! Parity port of the flat-shaded portion of `LevelRenderer.renderSky`
//! (`reference/.../client/renderer/LevelRenderer.java:1699`): the sky disc
//! (`buildSkyDisc(+16)`, `:587`) tinted by the day/night sky color, and the
//! sunrise/sunset glow `TRIANGLE_FAN` (`:1717`). Drawn before the chunk pass with
//! depth writes off, into a rotation-only (camera-at-infinity) view-projection.
//!
//! The textured sun is the first Phase 3 celestial body. Moon phases and the
//! star field remain follow-up work. Triangle fans are expanded to indexed
//! triangle lists since wgpu has no fan topology.

use std::cell::RefCell;
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};
use mclone_assets::{AssetPath, AssetSource};
use mclone_diagnostics::GpuPassId;
use wgpu::util::DeviceExt;

use crate::color_profile::{
    RenderColorProfile, RenderConfig, RenderTargetColorTransform, color_transform_rgb,
    color_transform_wgpu,
};
use crate::gpu_timestamps::GpuTimestampFrameEncoder;
use crate::sky::{SkyGlow, SkyRenderState};
use crate::uniform::{
    PER_VIEW_UNIFORM_SLOT_COUNT, PerViewSlot, PerViewUniformBuffer, SINGLE_VIEW_SLOT,
};

const SKY_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = 64;
const SKY_MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = SKY_UNIFORM_BYTE_SIZE * 2;
const SKY_VERTEX_FLOAT_COUNT: usize = 7; // position(3) + color(4)
const SKY_VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (SKY_VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const SUN_VERTEX_FLOAT_COUNT: usize = 6; // position(3) + uv(2) + opacity(1)
const SUN_VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (SUN_VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const SUN_VERTEX_COUNT: usize = 4;
const SUN_HALF_SIZE: f32 = 30.0;
const SUN_DISTANCE: f32 = 100.0;

pub const MCLONE_SUN_TEXTURE_PATH: &str = "assets/mclone/textures/environment/sun.png";
pub const REFERENCE_SUN_TEXTURE_PATH: &str = "assets/minecraft/textures/environment/sun.png";

// Sky disc: a center vertex plus a ring sampled every 45° from -180..=180.
const DISC_HEIGHT: f32 = 16.0;
const DISC_RADIUS: f32 = 512.0;
const DISC_RING_COUNT: usize = 9; // -180, -135, ..., 180
const DISC_VERTEX_COUNT: usize = DISC_RING_COUNT + 1;

// Glow fan: a center vertex plus 17 ring points (0..=16), per the reference loop.
const GLOW_RING_COUNT: usize = 17;
const GLOW_VERTEX_COUNT: usize = GLOW_RING_COUNT + 1;

type SkyVertex = [f32; SKY_VERTEX_FLOAT_COUNT];
type SunVertex = [f32; SUN_VERTEX_FLOAT_COUNT];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SunTextureAssets {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub source_path: Option<AssetPath>,
}

impl SunTextureAssets {
    pub fn generated_fallback() -> Self {
        const SIZE: u32 = 32;
        let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = (f64::from(x) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
                let dy = (f64::from(y) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
                let radius = dx.hypot(dy);
                let alpha = ((1.0 - radius) * 12.0).clamp(0.0, 1.0);
                rgba.extend_from_slice(&[255, 246, 190, (alpha * 255.0).round() as u8]);
            }
        }
        Self {
            width: SIZE,
            height: SIZE,
            rgba,
            source_path: None,
        }
    }
}

pub fn load_sun_texture_assets(source: &impl AssetSource) -> Result<SunTextureAssets> {
    for path in [MCLONE_SUN_TEXTURE_PATH, REFERENCE_SUN_TEXTURE_PATH] {
        let path = AssetPath::new(path);
        let Some(bytes) = source.read(&path)? else {
            continue;
        };
        let image = image::load_from_memory(&bytes)
            .with_context(|| format!("decode sun texture {}", path.as_str()))?
            .to_rgba8();
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            bail!("sun texture {} has zero dimensions", path.as_str());
        }
        return Ok(SunTextureAssets {
            width,
            height,
            rgba: image.into_raw(),
            source_path: Some(path),
        });
    }
    Ok(SunTextureAssets::generated_fallback())
}

/// Renders the flat sky geometry (disc + sunrise/sunset glow) into a color
/// attachment, clearing it first. Owns the shared shader/pipelines and the
/// per-frame vertex buffers.
pub struct SkyRenderer {
    disc_pipeline: wgpu::RenderPipeline,
    glow_pipeline: wgpu::RenderPipeline,
    sun_pipeline: wgpu::RenderPipeline,
    uniforms: PerViewUniformBuffer,
    bind_group: wgpu::BindGroup,
    disc_vertex_buffer: wgpu::Buffer,
    disc_index_buffer: wgpu::Buffer,
    disc_index_count: u32,
    glow_vertex_buffer: wgpu::Buffer,
    glow_index_buffer: wgpu::Buffer,
    glow_index_count: u32,
    sun_vertex_buffer: wgpu::Buffer,
    sun_index_buffer: wgpu::Buffer,
    sun_index_count: u32,
    sun_texture_bind_group_layout: wgpu::BindGroupLayout,
    sun_texture_bind_group: wgpu::BindGroup,
    sun_texture_assets: SunTextureAssets,
    color_transform: RenderTargetColorTransform,
    color_format: wgpu::TextureFormat,
    multiview: RefCell<Option<SkyMultiviewRenderer>>,
}

impl SkyRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        Self::new_with_color_profile(device, queue, color_format, RenderColorProfile::default())
    }

    pub fn new_with_color_profile(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
    ) -> Self {
        Self::new_with_config(
            device,
            queue,
            RenderConfig::for_color_target(color_profile, color_format),
        )
    }

    pub fn new_with_config(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
    ) -> Self {
        Self::new_with_config_and_assets(
            device,
            queue,
            render_config,
            SunTextureAssets::generated_fallback(),
        )
    }

    pub fn new_with_color_profile_and_source(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
        source: &impl AssetSource,
    ) -> Result<Self> {
        let assets = load_sun_texture_assets(source)?;
        Ok(Self::new_with_config_and_assets(
            device,
            queue,
            RenderConfig::for_color_target(color_profile, color_format),
            assets,
        ))
    }

    pub fn new_with_config_and_source(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
        source: &impl AssetSource,
    ) -> Result<Self> {
        let assets = load_sun_texture_assets(source)?;
        Ok(Self::new_with_config_and_assets(
            device,
            queue,
            render_config,
            assets,
        ))
    }

    pub fn new_with_color_profile_and_assets(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
        assets: SunTextureAssets,
    ) -> Self {
        Self::new_with_config_and_assets(
            device,
            queue,
            RenderConfig::for_color_target(color_profile, color_format),
            assets,
        )
    }

    pub fn new_with_config_and_assets(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
        sun_texture_assets: SunTextureAssets,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_sky_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_sky_uniforms",
            SKY_UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_sky_bind_group_layout"),
            entries: &[uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_sky_bind_group"),
            layout: &bind_group_layout,
            entries: &[uniforms.bind_group_entry(0)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_sky_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let sun_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_sun_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky_sun.wgsl").into()),
        });
        let sun_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_sun_texture_bind_group_layout"),
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
                ],
            });
        let sun_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_sun_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout, &sun_texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: SKY_VERTEX_BYTE_SIZE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let make_pipeline = |label: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[vertex_layout.clone()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: render_config.color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                // The sky pass owns no depth attachment; it is drawn before the
                // chunk pass clears depth, with depth writes implicitly off.
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };

        let disc_pipeline = make_pipeline("mclone_sky_disc_pipeline", None);
        // Glow blends additively over the disc, matching vanilla's
        // SRC_ALPHA, ONE color factors (alpha left untouched).
        let glow_pipeline = make_pipeline(
            "mclone_sky_glow_pipeline",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        );
        let sun_pipeline = make_sun_pipeline(
            device,
            "mclone_sun_pipeline",
            &sun_pipeline_layout,
            &sun_shader,
            render_config.color_format,
            None,
        );

        let disc_indices = fan_indices(DISC_VERTEX_COUNT);
        let glow_indices = fan_indices(GLOW_VERTEX_COUNT);
        let disc_index_count = disc_indices.len() as u32;
        let glow_index_count = glow_indices.len() as u32;
        let disc_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_sky_disc_indices"),
            contents: &index_bytes(&disc_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let glow_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_sky_glow_indices"),
            contents: &index_bytes(&glow_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let disc_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sky_disc_vertices"),
            size: sky_vertex_slot_size(DISC_VERTEX_COUNT)
                * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let glow_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sky_glow_vertices"),
            size: sky_vertex_slot_size(GLOW_VERTEX_COUNT)
                * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sun_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sun_vertices"),
            size: sun_vertex_slot_size() * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sun_indices = [0_u32, 1, 2, 0, 2, 3];
        let sun_index_count = sun_indices.len() as u32;
        let sun_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_sun_indices"),
            contents: &index_bytes(&sun_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let sun_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_sun_texture"),
            size: wgpu::Extent3d {
                width: sun_texture_assets.width,
                height: sun_texture_assets.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &sun_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &sun_texture_assets.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(sun_texture_assets.width * 4),
                rows_per_image: Some(sun_texture_assets.height),
            },
            wgpu::Extent3d {
                width: sun_texture_assets.width,
                height: sun_texture_assets.height,
                depth_or_array_layers: 1,
            },
        );
        let sun_texture_view = sun_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sun_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_sun_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sun_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_sun_texture_bind_group"),
            layout: &sun_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sun_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sun_sampler),
                },
            ],
        });

        Self {
            disc_pipeline,
            glow_pipeline,
            sun_pipeline,
            uniforms,
            bind_group,
            disc_vertex_buffer,
            disc_index_buffer,
            disc_index_count,
            glow_vertex_buffer,
            glow_index_buffer,
            glow_index_count,
            sun_vertex_buffer,
            sun_index_buffer,
            sun_index_count,
            sun_texture_bind_group_layout,
            sun_texture_bind_group,
            sun_texture_assets,
            color_transform: render_config.target_color_transform(),
            color_format: render_config.color_format,
            multiview: RefCell::new(None),
        }
    }

    pub fn sun_texture_assets(&self) -> &SunTextureAssets {
        &self.sun_texture_assets
    }

    /// Clears the color attachment to `clear_color` and draws the sky disc (tinted
    /// by the same color) plus the sunrise/sunset glow when within the dawn/dusk
    /// band. `sky_view_projection` must be rotation-only (see
    /// [`crate::chunk::ChunkRenderView::sky_view_projection`]).
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projection: Mat4,
        sky_state: SkyRenderState,
    ) {
        self.render_in_slot(
            queue,
            encoder,
            color_view,
            clear_color,
            sky_view_projection,
            sky_state,
            SINGLE_VIEW_SLOT,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_with_gpu_timestamps(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projection: Mat4,
        sky_state: SkyRenderState,
        gpu_timestamps: &GpuTimestampFrameEncoder,
    ) {
        self.render_in_slot_inner(
            queue,
            encoder,
            color_view,
            clear_color,
            sky_view_projection,
            sky_state,
            SINGLE_VIEW_SLOT,
            Some(gpu_timestamps),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_in_slot(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projection: Mat4,
        sky_state: SkyRenderState,
        view_slot: PerViewSlot,
    ) {
        self.render_in_slot_inner(
            queue,
            encoder,
            color_view,
            clear_color,
            sky_view_projection,
            sky_state,
            view_slot,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_in_slot_with_gpu_timestamps(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projection: Mat4,
        sky_state: SkyRenderState,
        view_slot: PerViewSlot,
        gpu_timestamps: &GpuTimestampFrameEncoder,
    ) {
        self.render_in_slot_inner(
            queue,
            encoder,
            color_view,
            clear_color,
            sky_view_projection,
            sky_state,
            view_slot,
            Some(gpu_timestamps),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_in_slot_inner(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projection: Mat4,
        sky_state: SkyRenderState,
        view_slot: PerViewSlot,
        gpu_timestamps: Option<&GpuTimestampFrameEncoder>,
    ) {
        let clear_color = color_transform_wgpu(clear_color, self.color_transform);
        let sky_color = [
            clear_color.r as f32,
            clear_color.g as f32,
            clear_color.b as f32,
        ];
        let uniform_offset = self.uniforms.write_slot(
            queue,
            view_slot,
            &matrix_bytes(sky_view_projection.to_cols_array_2d()),
        );
        let disc_range = sky_vertex_slot_range(view_slot, DISC_VERTEX_COUNT);
        queue.write_buffer(
            &self.disc_vertex_buffer,
            disc_range.start,
            &vertex_bytes(&disc_vertices(sky_color)),
        );
        let glow_range = if let Some(glow) = sky_state.glow() {
            let range = sky_vertex_slot_range(view_slot, GLOW_VERTEX_COUNT);
            queue.write_buffer(
                &self.glow_vertex_buffer,
                range.start,
                &vertex_bytes(&sky_glow_vertices(glow, self.color_transform)),
            );
            Some(range)
        } else {
            None
        };
        let sun_range = sun_vertex_slot_range(view_slot);
        queue.write_buffer(
            &self.sun_vertex_buffer,
            sun_range.start,
            &sun_vertex_bytes(&sun_vertices(
                Vec3::from_array(sky_state.sun_direction()),
                sky_state.sun_opacity(),
            )),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_sky_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: gpu_timestamps
                .and_then(|timestamps| timestamps.render_pass_timestamp_writes(GpuPassId::Sky)),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.bind_group, &[uniform_offset]);
        pass.set_pipeline(&self.disc_pipeline);
        pass.set_vertex_buffer(0, self.disc_vertex_buffer.slice(disc_range));
        pass.set_index_buffer(self.disc_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.disc_index_count, 0, 0..1);
        if let Some(glow_range) = glow_range {
            pass.set_pipeline(&self.glow_pipeline);
            pass.set_vertex_buffer(0, self.glow_vertex_buffer.slice(glow_range));
            pass.set_index_buffer(self.glow_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.glow_index_count, 0, 0..1);
        }
        pass.set_pipeline(&self.sun_pipeline);
        pass.set_bind_group(1, &self.sun_texture_bind_group, &[]);
        pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(sun_range));
        pass.set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_multiview(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projections: [Mat4; 2],
        sky_state: SkyRenderState,
    ) -> Result<()> {
        self.render_multiview_inner(
            device,
            queue,
            encoder,
            color_view,
            clear_color,
            sky_view_projections,
            sky_state,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_multiview_with_gpu_timestamps(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projections: [Mat4; 2],
        sky_state: SkyRenderState,
        gpu_timestamps: &GpuTimestampFrameEncoder,
    ) -> Result<()> {
        self.render_multiview_inner(
            device,
            queue,
            encoder,
            color_view,
            clear_color,
            sky_view_projections,
            sky_state,
            Some(gpu_timestamps),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_multiview_inner(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        sky_view_projections: [Mat4; 2],
        sky_state: SkyRenderState,
        gpu_timestamps: Option<&GpuTimestampFrameEncoder>,
    ) -> Result<()> {
        let (clear_color, disc_range, glow_range, sun_range) =
            self.prepare_vertices(queue, clear_color, sky_state);
        let renderer = self.multiview_renderer(device)?;
        renderer.write_uniforms(queue, sky_view_projections);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_sky_multiview_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: gpu_timestamps
                .and_then(|timestamps| timestamps.render_pass_timestamp_writes(GpuPassId::Sky)),
            ..Default::default()
        });
        pass.set_bind_group(0, &renderer.bind_group, &[]);
        pass.set_pipeline(&renderer.disc_pipeline);
        pass.set_vertex_buffer(0, self.disc_vertex_buffer.slice(disc_range));
        pass.set_index_buffer(self.disc_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.disc_index_count, 0, 0..1);
        if let Some(glow_range) = glow_range {
            pass.set_pipeline(&renderer.glow_pipeline);
            pass.set_vertex_buffer(0, self.glow_vertex_buffer.slice(glow_range));
            pass.set_index_buffer(self.glow_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.glow_index_count, 0, 0..1);
        }
        pass.set_pipeline(&renderer.sun_pipeline);
        pass.set_bind_group(1, &self.sun_texture_bind_group, &[]);
        pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(sun_range));
        pass.set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
        Ok(())
    }

    fn prepare_vertices(
        &self,
        queue: &wgpu::Queue,
        clear_color: wgpu::Color,
        sky_state: SkyRenderState,
    ) -> (
        wgpu::Color,
        Range<wgpu::BufferAddress>,
        Option<Range<wgpu::BufferAddress>>,
        Range<wgpu::BufferAddress>,
    ) {
        let clear_color = color_transform_wgpu(clear_color, self.color_transform);
        let sky_color = [
            clear_color.r as f32,
            clear_color.g as f32,
            clear_color.b as f32,
        ];
        let disc_range = sky_vertex_slot_range(SINGLE_VIEW_SLOT, DISC_VERTEX_COUNT);
        queue.write_buffer(
            &self.disc_vertex_buffer,
            disc_range.start,
            &vertex_bytes(&disc_vertices(sky_color)),
        );
        let glow_range = if let Some(glow) = sky_state.glow() {
            let range = sky_vertex_slot_range(SINGLE_VIEW_SLOT, GLOW_VERTEX_COUNT);
            queue.write_buffer(
                &self.glow_vertex_buffer,
                range.start,
                &vertex_bytes(&sky_glow_vertices(glow, self.color_transform)),
            );
            Some(range)
        } else {
            None
        };
        let sun_range = sun_vertex_slot_range(SINGLE_VIEW_SLOT);
        queue.write_buffer(
            &self.sun_vertex_buffer,
            sun_range.start,
            &sun_vertex_bytes(&sun_vertices(
                Vec3::from_array(sky_state.sun_direction()),
                sky_state.sun_opacity(),
            )),
        );
        (clear_color, disc_range, glow_range, sun_range)
    }

    fn multiview_renderer(
        &self,
        device: &wgpu::Device,
    ) -> Result<std::cell::Ref<'_, SkyMultiviewRenderer>> {
        if !device.features().contains(wgpu::Features::MULTIVIEW) {
            bail!("sky multiview render requires wgpu MULTIVIEW");
        }
        if self.multiview.borrow().is_none() {
            let renderer = SkyMultiviewRenderer::new(
                device,
                self.color_format,
                &self.sun_texture_bind_group_layout,
            );
            *self.multiview.borrow_mut() = Some(renderer);
        }
        Ok(std::cell::Ref::map(self.multiview.borrow(), |renderer| {
            renderer
                .as_ref()
                .expect("sky multiview renderer initialized above")
        }))
    }
}

fn sky_vertex_slot_size(vertex_count: usize) -> wgpu::BufferAddress {
    SKY_VERTEX_BYTE_SIZE * vertex_count as wgpu::BufferAddress
}

fn sun_vertex_slot_size() -> wgpu::BufferAddress {
    SUN_VERTEX_BYTE_SIZE * SUN_VERTEX_COUNT as wgpu::BufferAddress
}

fn sun_vertex_slot_range(view_slot: PerViewSlot) -> Range<wgpu::BufferAddress> {
    view_slot.byte_range(sun_vertex_slot_size())
}

fn sky_vertex_slot_range(
    view_slot: PerViewSlot,
    vertex_count: usize,
) -> Range<wgpu::BufferAddress> {
    view_slot.byte_range(sky_vertex_slot_size(vertex_count))
}

struct SkyMultiviewRenderer {
    disc_pipeline: wgpu::RenderPipeline,
    glow_pipeline: wgpu::RenderPipeline,
    sun_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl SkyMultiviewRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sun_texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_sky_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky_multiview.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_sky_multiview_uniforms"),
            size: SKY_MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_sky_multiview_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(SKY_MULTIVIEW_UNIFORM_BYTE_SIZE),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_sky_multiview_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_sky_multiview_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let sun_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_sun_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky_sun_multiview.wgsl").into()),
        });
        let sun_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_sun_multiview_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout, sun_texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: SKY_VERTEX_BYTE_SIZE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };
        let make_pipeline = |label: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[vertex_layout.clone()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                multiview: NonZeroU32::new(2),
                cache: None,
            })
        };
        let disc_pipeline = make_pipeline("mclone_sky_multiview_disc_pipeline", None);
        let glow_pipeline = make_pipeline(
            "mclone_sky_multiview_glow_pipeline",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        );
        let sun_pipeline = make_sun_pipeline(
            device,
            "mclone_sun_multiview_pipeline",
            &sun_pipeline_layout,
            &sun_shader,
            color_format,
            NonZeroU32::new(2),
        );
        Self {
            disc_pipeline,
            glow_pipeline,
            sun_pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    fn write_uniforms(&self, queue: &wgpu::Queue, sky_view_projections: [Mat4; 2]) {
        let mut bytes = [0u8; SKY_MULTIVIEW_UNIFORM_BYTE_SIZE as usize];
        bytes[..SKY_UNIFORM_BYTE_SIZE as usize]
            .copy_from_slice(&matrix_bytes(sky_view_projections[0].to_cols_array_2d()));
        bytes[SKY_UNIFORM_BYTE_SIZE as usize..]
            .copy_from_slice(&matrix_bytes(sky_view_projections[1].to_cols_array_2d()));
        queue.write_buffer(&self.uniform_buffer, 0, &bytes);
    }
}

fn color_transform_rgba(color: [f32; 4], transform: RenderTargetColorTransform) -> [f32; 4] {
    let [r, g, b] = color_transform_rgb([color[0], color[1], color[2]], transform);
    [r, g, b, color[3]]
}

fn make_sun_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: SUN_VERTEX_BYTE_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                    wgpu::VertexAttribute {
                        offset: 12,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 20,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview,
        cache: None,
    })
}

/// Triangle-list indices for a fan with vertex 0 at the center and the remainder
/// forming the ring: `(0, i, i+1)` for each adjacent ring pair.
fn fan_indices(vertex_count: usize) -> Vec<u32> {
    let mut indices = Vec::new();
    for i in 1..vertex_count.saturating_sub(1) {
        indices.push(0);
        indices.push(i as u32);
        indices.push(i as u32 + 1);
    }
    indices
}

/// Port of `buildSkyDisc(16.0)`: a flat disc at y=+16, radius 512, tinted flat by
/// the day/night sky color.
fn disc_vertices(color: [f32; 3]) -> Vec<SkyVertex> {
    let [r, g, b] = color;
    let mut vertices = Vec::with_capacity(DISC_VERTEX_COUNT);
    vertices.push([0.0, DISC_HEIGHT, 0.0, r, g, b, 1.0]);
    let mut degrees = -180_i32;
    while degrees <= 180 {
        let radians = (degrees as f32).to_radians();
        vertices.push([
            DISC_RADIUS * radians.cos(),
            DISC_HEIGHT,
            DISC_RADIUS * radians.sin(),
            r,
            g,
            b,
            1.0,
        ]);
        degrees += 45;
    }
    vertices
}

/// Port of the `getSunriseColor` glow `TRIANGLE_FAN` (`LevelRenderer:1717`),
/// baking the celestial rig rotation into camera-relative vertex positions.
fn glow_vertices(color: [f32; 4], sun_angle: f32) -> Vec<SkyVertex> {
    let flip_degrees = if sun_angle.sin() < 0.0 { 180.0 } else { 0.0 };
    // Rig: XP(90) * ZP(flip) * ZP(90), applied right-to-left to each vertex.
    let rig = Quat::from_rotation_x(90.0_f32.to_radians())
        * Quat::from_rotation_z((flip_degrees as f32).to_radians())
        * Quat::from_rotation_z(90.0_f32.to_radians());
    let [r, g, b, alpha] = color;
    let mut vertices = Vec::with_capacity(GLOW_VERTEX_COUNT);
    let center = rig * Vec3::new(0.0, 100.0, 0.0);
    vertices.push([center.x, center.y, center.z, r, g, b, alpha]);
    for i in 0..GLOW_RING_COUNT {
        let angle = i as f32 * std::f32::consts::TAU / 16.0;
        let (sin_a, cos_a) = angle.sin_cos();
        let point = rig * Vec3::new(sin_a * 120.0, cos_a * 120.0, -cos_a * 40.0 * alpha);
        // Ring vertices fade to zero alpha so the glow blends out radially.
        vertices.push([point.x, point.y, point.z, r, g, b, 0.0]);
    }
    vertices
}

fn sky_glow_vertices(glow: SkyGlow, color_transform: RenderTargetColorTransform) -> Vec<SkyVertex> {
    match glow {
        SkyGlow::Vanilla { color, sun_angle } => {
            glow_vertices(color_transform_rgba(color, color_transform), sun_angle)
        }
        SkyGlow::Directional {
            color,
            sun_direction,
        } => directional_glow_vertices(
            color_transform_rgba(color, color_transform),
            Vec3::from_array(sun_direction),
        ),
    }
}

fn directional_glow_vertices(color: [f32; 4], sun_direction: Vec3) -> Vec<SkyVertex> {
    let horizontal = Vec3::new(sun_direction.x, 0.0, sun_direction.z)
        .try_normalize()
        .unwrap_or(Vec3::X);
    let tangent = Vec3::Y.cross(horizontal).normalize();
    let [r, g, b, alpha] = color;
    let center = horizontal * 100.0;
    let mut vertices = Vec::with_capacity(GLOW_VERTEX_COUNT);
    vertices.push([center.x, center.y, center.z, r, g, b, alpha]);
    for i in 0..GLOW_RING_COUNT {
        let angle = i as f32 * std::f32::consts::TAU / 16.0;
        let (sin_a, cos_a) = angle.sin_cos();
        let point = center + tangent * (sin_a * 120.0) + Vec3::Y * (cos_a * 120.0)
            - horizontal * (cos_a * 40.0 * alpha);
        vertices.push([point.x, point.y, point.z, r, g, b, 0.0]);
    }
    vertices
}

fn sun_vertices(direction: Vec3, opacity: f32) -> [SunVertex; SUN_VERTEX_COUNT] {
    let direction = direction.try_normalize().unwrap_or(Vec3::Y);
    let reference_up = if direction.y.abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let right = direction.cross(reference_up).normalize();
    let up = right.cross(direction).normalize();
    let center = direction * SUN_DISTANCE;
    let right = right * SUN_HALF_SIZE;
    let up = up * SUN_HALF_SIZE;
    [
        to_sun_vertex(center - right + up, [0.0, 0.0], opacity),
        to_sun_vertex(center + right + up, [1.0, 0.0], opacity),
        to_sun_vertex(center + right - up, [1.0, 1.0], opacity),
        to_sun_vertex(center - right - up, [0.0, 1.0], opacity),
    ]
}

fn to_sun_vertex(position: Vec3, uv: [f32; 2], opacity: f32) -> SunVertex {
    [position.x, position.y, position.z, uv[0], uv[1], opacity]
}

fn vertex_bytes(vertices: &[SkyVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * SKY_VERTEX_BYTE_SIZE as usize);
    for vertex in vertices {
        for value in vertex {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn sun_vertex_bytes(vertices: &[SunVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * SUN_VERTEX_BYTE_SIZE as usize);
    for vertex in vertices {
        for value in vertex {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn index_bytes(indices: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(indices));
    for index in indices {
        bytes.extend_from_slice(&index.to_ne_bytes());
    }
    bytes
}

fn matrix_bytes(matrix: [[f32; 4]; 4]) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, value) in matrix.into_iter().flatten().enumerate() {
        let start = index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_has_expected_vertex_and_index_counts() {
        let vertices = disc_vertices([0.1, 0.2, 0.3]);
        assert_eq!(vertices.len(), DISC_VERTEX_COUNT);
        // Center is at y = +16, ring radius reaches 512.
        assert_eq!(vertices[0][1], DISC_HEIGHT);
        assert!((vertices[1][0].hypot(vertices[1][2]) - DISC_RADIUS).abs() < 1e-2);
        assert_eq!(
            fan_indices(DISC_VERTEX_COUNT).len(),
            (DISC_RING_COUNT - 1) * 3
        );
    }

    #[test]
    fn glow_center_sits_on_the_horizon_and_flips_with_sun_angle() {
        let color = [0.85, 0.4, 0.2, 0.9];
        // sun_angle with positive sine -> no flip; center hugs the horizon (y≈0).
        let dawn = glow_vertices(color, 1.0);
        assert_eq!(dawn.len(), GLOW_VERTEX_COUNT);
        assert!(dawn[0][1].abs() < 1e-3, "center y = {}", dawn[0][1]);
        // Flipping the sun below the horizon mirrors the glow to the opposite side.
        let dusk = glow_vertices(color, -1.0);
        assert!(
            (dawn[0][0] + dusk[0][0]).abs() < 1e-3,
            "glow should mirror across origin"
        );
        // Center keeps full alpha; ring fades to zero.
        assert_eq!(dawn[0][6], color[3]);
        assert_eq!(dawn[1][6], 0.0);
    }

    #[test]
    fn generated_sun_fallback_is_a_bounded_rgba_disc() {
        let assets = SunTextureAssets::generated_fallback();
        assert_eq!((assets.width, assets.height), (32, 32));
        assert_eq!(assets.rgba.len(), 32 * 32 * 4);
        assert_eq!(assets.rgba[3], 0);
        let center = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(assets.rgba[center..center + 3], [255, 246, 190]);
        assert_eq!(assets.rgba[center + 3], 255);
        assert!(assets.source_path.is_none());
    }

    #[test]
    fn vanilla_sun_direction_and_quad_follow_clock_anchors() {
        assert!(
            Vec3::from_array(SkyRenderState::vanilla(0.0, 0.0).sun_direction())
                .abs_diff_eq(Vec3::Y, 1.0e-6)
        );
        assert!(
            Vec3::from_array(
                SkyRenderState::vanilla(0.0, std::f32::consts::FRAC_PI_2).sun_direction(),
            )
            .abs_diff_eq(Vec3::NEG_X, 1.0e-6)
        );
        let vertices = sun_vertices(Vec3::Y, 0.625);
        let center = vertices.iter().fold(Vec3::ZERO, |sum, vertex| {
            sum + Vec3::new(vertex[0], vertex[1], vertex[2])
        }) / vertices.len() as f32;
        assert!(center.abs_diff_eq(Vec3::Y * SUN_DISTANCE, 1.0e-5));
        assert!(vertices.iter().all(|vertex| vertex[5] == 0.625));
        assert!(
            vertices
                .iter()
                .all(|vertex| vertex.iter().all(|value| value.is_finite()))
        );
    }

    #[test]
    fn sun_shaders_parse_for_mono_and_multiview() {
        for source in [
            include_str!("shaders/sky_sun.wgsl"),
            include_str!("shaders/sky_sun_multiview.wgsl"),
        ] {
            naga::front::wgsl::parse_str(source).expect("sun WGSL parses");
        }
    }
}
