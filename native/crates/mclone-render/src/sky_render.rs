//! Sky dome, sunrise/sunset glow, and visible sun render pass.
//!
//! Parity port of the flat-shaded portion of `LevelRenderer.renderSky`
//! (`reference/.../client/renderer/LevelRenderer.java:1699`): the sky disc
//! (`buildSkyDisc(+16)`, `:587`) tinted by the day/night sky color, and the
//! sunrise/sunset glow `TRIANGLE_FAN` (`:1717`). Drawn before the chunk pass with
//! depth writes off, into a rotation-only (camera-at-infinity) view-projection.
//!
//! The pass also owns the original square sun/continuous moon, the retained
//! Java sun/moon presentation, and bounded static star catalogs. Triangle fans
//! are expanded to indexed triangle lists since wgpu has no fan topology.

use std::cell::{Cell, RefCell};
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
const STAR_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = 96;
const STAR_MULTIVIEW_UNIFORM_BYTE_SIZE: wgpu::BufferAddress = SKY_MULTIVIEW_UNIFORM_BYTE_SIZE + 32;
const SKY_VERTEX_FLOAT_COUNT: usize = 7; // position(3) + color(4)
const SKY_VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (SKY_VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const SUN_VERTEX_FLOAT_COUNT: usize = 8; // position(3) + uv(2) + opacity + mode + phase
const SUN_VERTEX_BYTE_SIZE: wgpu::BufferAddress =
    (SUN_VERTEX_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const SUN_VERTEX_COUNT: usize = 4;
const SUN_DISTANCE: f32 = 100.0;
const CELESTIAL_QUAD_COUNT: usize = 3;
const SUN_BODY_QUAD: usize = 0;
const SUN_HALO_QUAD: usize = 1;
const MOON_BODY_QUAD: usize = 2;
const TEXTURED_SUN_MODE: f32 = 0.0;
const SQUARE_SUN_MODE: f32 = 1.0;
const SUN_HALO_MODE: f32 = 2.0;
const SQUARE_MOON_MODE: f32 = 3.0;
const REFERENCE_MOON_MODE: f32 = 4.0;
const MCLONE_SUN_HALO_ANGULAR_DIAMETER_DEGREES: f32 = 2.4;
pub const MCLONE_STAR_COUNT: u32 = 1_536;
pub const MCLONE_STAR_CATALOG_MAX_COUNT: u32 = 4_096;
pub const REFERENCE_STAR_CANDIDATE_COUNT: u32 = 1_500;
pub const REFERENCE_STAR_COUNT: u32 = 780;
const STAR_INSTANCE_FLOAT_COUNT: usize = 9;
const STAR_INSTANCE_BYTE_SIZE: wgpu::BufferAddress =
    (STAR_INSTANCE_FLOAT_COUNT * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
const STAR_INDEX_COUNT: u32 = 6;
const STAR_INDEX_BYTE_SIZE: wgpu::BufferAddress =
    STAR_INDEX_COUNT as wgpu::BufferAddress * std::mem::size_of::<u32>() as wgpu::BufferAddress;

pub const MCLONE_SUN_TEXTURE_PATH: &str = "assets/mclone/textures/environment/sun.png";
pub const REFERENCE_SUN_TEXTURE_PATH: &str = "assets/minecraft/textures/environment/sun.png";
pub const REFERENCE_MOON_TEXTURE_PATH: &str =
    "assets/minecraft/textures/environment/moon_phases.png";

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
type StarInstance = [f32; STAR_INSTANCE_FLOAT_COUNT];
type StarUniformParameters = [f32; 8];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CelestialRenderStats {
    pub sun_body_draws: u32,
    pub sun_halo_draws: u32,
    pub horizon_glow_draws: u32,
    pub moon_body_draws: u32,
    pub star_draws: u32,
    pub submitted_star_count: u32,
    pub catalog_star_count: u32,
    pub feature_buffer_writes: u32,
    pub resident_resource_bytes: u64,
}

impl CelestialRenderStats {
    pub const fn optional_draw_count(self) -> u32 {
        self.sun_body_draws
            + self.sun_halo_draws
            + self.horizon_glow_draws
            + self.moon_body_draws
            + self.star_draws
    }
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoonTextureAssets {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub source_path: Option<AssetPath>,
}

impl MoonTextureAssets {
    /// Proprietary-free 4×2 atlas in Java's eight-phase frame order.
    pub fn generated_fallback() -> Self {
        const CELL: u32 = 16;
        const WIDTH: u32 = CELL * 4;
        const HEIGHT: u32 = CELL * 2;
        let mut rgba = vec![0; (WIDTH * HEIGHT * 4) as usize];
        for frame in 0..8_u32 {
            let our_phase = ((frame + 4) % 8) as f32 / 8.0;
            let illumination = (1.0 - (our_phase * std::f32::consts::TAU).cos()) * 0.5;
            let waxing = our_phase > 0.0 && our_phase < 0.5;
            let threshold = 1.0 - illumination * 2.0;
            let cell_x = frame % 4;
            let cell_y = frame / 4;
            for y in 0..CELL {
                for x in 0..CELL {
                    let px = (x as f32 + 0.5) / CELL as f32 * 2.0 - 1.0;
                    let py = (y as f32 + 0.5) / CELL as f32 * 2.0 - 1.0;
                    let radius = px.hypot(py);
                    if radius > 1.0 {
                        continue;
                    }
                    let lit = if waxing {
                        px >= threshold
                    } else {
                        px <= -threshold
                    };
                    let value = if lit { 232 } else { 34 };
                    let atlas_x = cell_x * CELL + x;
                    let atlas_y = cell_y * CELL + y;
                    let offset = ((atlas_y * WIDTH + atlas_x) * 4) as usize;
                    rgba[offset..offset + 4].copy_from_slice(&[value, value, value, 255]);
                }
            }
        }
        Self {
            width: WIDTH,
            height: HEIGHT,
            rgba,
            source_path: None,
        }
    }
}

pub fn load_moon_texture_assets(source: &impl AssetSource) -> Result<MoonTextureAssets> {
    let path = AssetPath::new(REFERENCE_MOON_TEXTURE_PATH);
    let Some(bytes) = source.read(&path)? else {
        return Ok(MoonTextureAssets::generated_fallback());
    };
    let image = image::load_from_memory(&bytes)
        .with_context(|| format!("decode moon texture {}", path.as_str()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || width * 2 != height * 4 {
        bail!(
            "moon texture {} must be a non-empty 4x2 atlas, got {width}x{height}",
            path.as_str()
        );
    }
    Ok(MoonTextureAssets {
        width,
        height,
        rgba: image.into_raw(),
        source_path: Some(path),
    })
}

/// Renders the flat sky geometry (disc + sunrise/sunset glow) into a color
/// attachment, clearing it first. Owns the shared shader/pipelines and the
/// per-frame vertex buffers.
pub struct SkyRenderer {
    disc_pipeline: wgpu::RenderPipeline,
    glow_pipeline: wgpu::RenderPipeline,
    sun_pipeline: wgpu::RenderPipeline,
    moon_pipeline: wgpu::RenderPipeline,
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
    moon_texture_bind_group: wgpu::BindGroup,
    moon_texture_assets: MoonTextureAssets,
    star_pipeline: wgpu::RenderPipeline,
    star_uniforms: PerViewUniformBuffer,
    star_bind_group: wgpu::BindGroup,
    star_index_buffer: wgpu::Buffer,
    star_instance_buffer: wgpu::Buffer,
    star_count: u32,
    reference_star_instance_buffer: wgpu::Buffer,
    reference_star_count: u32,
    celestial_resident_resource_bytes: u64,
    last_celestial_stats: Cell<CelestialRenderStats>,
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
        let sun_assets = load_sun_texture_assets(source)?;
        let moon_assets = load_moon_texture_assets(source)?;
        Ok(Self::new_with_config_and_celestial_assets(
            device,
            queue,
            RenderConfig::for_color_target(color_profile, color_format),
            sun_assets,
            moon_assets,
        ))
    }

    pub fn new_with_config_and_source(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
        source: &impl AssetSource,
    ) -> Result<Self> {
        let sun_assets = load_sun_texture_assets(source)?;
        let moon_assets = load_moon_texture_assets(source)?;
        Ok(Self::new_with_config_and_celestial_assets(
            device,
            queue,
            render_config,
            sun_assets,
            moon_assets,
        ))
    }

    pub fn new_with_color_profile_and_assets(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
        assets: SunTextureAssets,
    ) -> Self {
        Self::new_with_color_profile_and_celestial_assets(
            device,
            queue,
            color_format,
            color_profile,
            assets,
            MoonTextureAssets::generated_fallback(),
        )
    }

    pub fn new_with_color_profile_and_celestial_assets(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        color_profile: RenderColorProfile,
        sun_texture_assets: SunTextureAssets,
        moon_texture_assets: MoonTextureAssets,
    ) -> Self {
        Self::new_with_config_and_celestial_assets(
            device,
            queue,
            RenderConfig::for_color_target(color_profile, color_format),
            sun_texture_assets,
            moon_texture_assets,
        )
    }

    pub fn new_with_config_and_assets(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
        sun_texture_assets: SunTextureAssets,
    ) -> Self {
        Self::new_with_config_and_celestial_assets(
            device,
            queue,
            render_config,
            sun_texture_assets,
            MoonTextureAssets::generated_fallback(),
        )
    }

    pub fn new_with_config_and_celestial_assets(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_config: RenderConfig,
        sun_texture_assets: SunTextureAssets,
        moon_texture_assets: MoonTextureAssets,
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
            celestial_additive_blend(),
        );
        let moon_pipeline = make_sun_pipeline(
            device,
            "mclone_moon_pipeline",
            &sun_pipeline_layout,
            &sun_shader,
            render_config.color_format,
            None,
            celestial_alpha_blend(),
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
            size: celestial_vertex_slot_size() * PER_VIEW_UNIFORM_SLOT_COUNT as wgpu::BufferAddress,
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
        let moon_texture_bind_group = create_celestial_texture_bind_group(
            device,
            queue,
            &sun_texture_bind_group_layout,
            "mclone_moon_texture",
            moon_texture_assets.width,
            moon_texture_assets.height,
            &moon_texture_assets.rgba,
        );
        let star_uniforms = PerViewUniformBuffer::new(
            device,
            "mclone_star_uniforms",
            STAR_UNIFORM_BYTE_SIZE,
            PER_VIEW_UNIFORM_SLOT_COUNT,
        );
        let star_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_star_bind_group_layout"),
                entries: &[star_uniforms.layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
            });
        let star_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_star_bind_group"),
            layout: &star_bind_group_layout,
            entries: &[star_uniforms.bind_group_entry(0)],
        });
        let star_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_star_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky_stars.wgsl").into()),
        });
        let star_pipeline = make_star_pipeline(
            device,
            "mclone_star_pipeline",
            &star_bind_group_layout,
            &star_shader,
            render_config.color_format,
            None,
        );
        let star_indices = [0_u32, 1, 2, 0, 2, 3];
        let star_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_star_indices"),
            contents: &index_bytes(&star_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let star_catalog = mclone_star_catalog();
        debug_assert!(star_catalog.len() as u32 <= MCLONE_STAR_CATALOG_MAX_COUNT);
        let star_count = star_catalog.len() as u32;
        let star_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mclone_star_catalog"),
            contents: &star_instance_bytes(&star_catalog),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let reference_star_catalog = reference_star_catalog();
        debug_assert_eq!(reference_star_catalog.len() as u32, REFERENCE_STAR_COUNT);
        let reference_star_count = reference_star_catalog.len() as u32;
        let reference_star_instance_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mclone_reference_star_catalog"),
                contents: &star_instance_bytes(&reference_star_catalog),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let celestial_resident_resource_bytes = (star_count + reference_star_count) as u64
            * STAR_INSTANCE_BYTE_SIZE
            + STAR_INDEX_BYTE_SIZE
            + celestial_vertex_slot_size() * u64::from(PER_VIEW_UNIFORM_SLOT_COUNT)
            + sun_texture_assets.rgba.len() as u64
            + moon_texture_assets.rgba.len() as u64;

        Self {
            disc_pipeline,
            glow_pipeline,
            sun_pipeline,
            moon_pipeline,
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
            moon_texture_bind_group,
            moon_texture_assets,
            star_pipeline,
            star_uniforms,
            star_bind_group,
            star_index_buffer,
            star_instance_buffer,
            star_count,
            reference_star_instance_buffer,
            reference_star_count,
            celestial_resident_resource_bytes,
            last_celestial_stats: Cell::new(CelestialRenderStats::default()),
            color_transform: render_config.target_color_transform(),
            color_format: render_config.color_format,
            multiview: RefCell::new(None),
        }
    }

    pub fn sun_texture_assets(&self) -> &SunTextureAssets {
        &self.sun_texture_assets
    }

    pub fn moon_texture_assets(&self) -> &MoonTextureAssets {
        &self.moon_texture_assets
    }

    pub fn celestial_stats(&self) -> CelestialRenderStats {
        self.last_celestial_stats.get()
    }

    fn star_catalog(&self, reference_profile: bool) -> (&wgpu::Buffer, u32) {
        if reference_profile {
            (
                &self.reference_star_instance_buffer,
                self.reference_star_count,
            )
        } else {
            (&self.star_instance_buffer, self.star_count)
        }
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
        let glow_range = if celestial_horizon_glow_enabled(sky_state) {
            sky_state.glow()
        } else {
            None
        }
        .map(|glow| {
            let range = sky_vertex_slot_range(view_slot, GLOW_VERTEX_COUNT);
            queue.write_buffer(
                &self.glow_vertex_buffer,
                range.start,
                &vertex_bytes(&sky_glow_vertices(glow, self.color_transform)),
            );
            range
        });
        let sun_range = celestial_sun_body_enabled(sky_state).then(|| {
            let range = celestial_quad_slot_range(view_slot, SUN_BODY_QUAD);
            let mode = if sky_state.is_reference_profile() {
                TEXTURED_SUN_MODE
            } else {
                SQUARE_SUN_MODE
            };
            queue.write_buffer(
                &self.sun_vertex_buffer,
                range.start,
                &sun_vertex_bytes(&celestial_quad_vertices(
                    Vec3::from_array(sky_state.sun_direction()),
                    sky_state.sun_opacity(),
                    sky_state.sun_angular_diameter_degrees(),
                    mode,
                    0.0,
                )),
            );
            range
        });
        let halo_range = celestial_sun_halo_enabled(sky_state).then(|| {
            let range = celestial_quad_slot_range(view_slot, SUN_HALO_QUAD);
            queue.write_buffer(
                &self.sun_vertex_buffer,
                range.start,
                &sun_vertex_bytes(&celestial_quad_vertices(
                    Vec3::from_array(sky_state.sun_direction()),
                    sky_state.sun_opacity(),
                    MCLONE_SUN_HALO_ANGULAR_DIAMETER_DEGREES,
                    SUN_HALO_MODE,
                    0.0,
                )),
            );
            range
        });
        let moon_range = celestial_moon_vertices(sky_state).map(|vertices| {
            let range = celestial_quad_slot_range(view_slot, MOON_BODY_QUAD);
            queue.write_buffer(
                &self.sun_vertex_buffer,
                range.start,
                &sun_vertex_bytes(&vertices),
            );
            range
        });
        let (star_instance_buffer, star_catalog_count) =
            self.star_catalog(sky_state.is_reference_profile());
        let star_draw =
            celestial_star_draw(sky_state, star_catalog_count).map(|(count, parameters)| {
                let bytes = star_uniform_bytes(sky_view_projection, parameters);
                let offset = self.star_uniforms.write_slot(queue, view_slot, &bytes);
                (count, offset)
            });
        let celestial_stats = prepared_celestial_stats(
            sun_range.is_some(),
            halo_range.is_some(),
            glow_range.is_some(),
            moon_range.is_some(),
            star_draw.map_or(0, |(count, _)| count),
            star_catalog_count,
            self.celestial_resident_resource_bytes,
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
        if halo_range.is_some() || sun_range.is_some() {
            pass.set_pipeline(&self.sun_pipeline);
            pass.set_bind_group(1, &self.sun_texture_bind_group, &[]);
            pass.set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            if let Some(halo_range) = halo_range {
                pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(halo_range));
                pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
            }
            if let Some(sun_range) = sun_range {
                pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(sun_range));
                pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
            }
        }
        if let Some(moon_range) = moon_range {
            pass.set_pipeline(&self.moon_pipeline);
            pass.set_bind_group(1, &self.moon_texture_bind_group, &[]);
            pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(moon_range));
            pass.set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
        }
        if let Some((star_count, star_uniform_offset)) = star_draw {
            pass.set_pipeline(&self.star_pipeline);
            pass.set_bind_group(0, &self.star_bind_group, &[star_uniform_offset]);
            pass.set_vertex_buffer(0, star_instance_buffer.slice(..));
            pass.set_index_buffer(self.star_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..STAR_INDEX_COUNT, 0, 0..star_count);
        }
        self.last_celestial_stats.set(celestial_stats);
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
        let (clear_color, disc_range, glow_range, sun_range, halo_range, moon_range) =
            self.prepare_vertices(queue, clear_color, sky_state);
        let renderer = self.multiview_renderer(device)?;
        renderer.write_uniforms(queue, sky_view_projections);
        let (star_instance_buffer, star_catalog_count) =
            self.star_catalog(sky_state.is_reference_profile());
        let star_draw = celestial_star_draw(sky_state, star_catalog_count);
        if let Some((_, parameters)) = star_draw {
            renderer.write_star_uniforms(queue, sky_view_projections, parameters);
        }
        let celestial_stats = prepared_celestial_stats(
            sun_range.is_some(),
            halo_range.is_some(),
            glow_range.is_some(),
            moon_range.is_some(),
            star_draw.map_or(0, |(count, _)| count),
            star_catalog_count,
            self.celestial_resident_resource_bytes,
        );

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
        if halo_range.is_some() || sun_range.is_some() {
            pass.set_pipeline(&renderer.sun_pipeline);
            pass.set_bind_group(1, &self.sun_texture_bind_group, &[]);
            pass.set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            if let Some(halo_range) = halo_range {
                pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(halo_range));
                pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
            }
            if let Some(sun_range) = sun_range {
                pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(sun_range));
                pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
            }
        }
        if let Some(moon_range) = moon_range {
            pass.set_pipeline(&renderer.moon_pipeline);
            pass.set_bind_group(1, &self.moon_texture_bind_group, &[]);
            pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(moon_range));
            pass.set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.sun_index_count, 0, 0..1);
        }
        if let Some((star_count, _)) = star_draw {
            pass.set_pipeline(&renderer.star_pipeline);
            pass.set_bind_group(0, &renderer.star_bind_group, &[]);
            pass.set_vertex_buffer(0, star_instance_buffer.slice(..));
            pass.set_index_buffer(self.star_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..STAR_INDEX_COUNT, 0, 0..star_count);
        }
        self.last_celestial_stats.set(celestial_stats);
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
        Option<Range<wgpu::BufferAddress>>,
        Option<Range<wgpu::BufferAddress>>,
        Option<Range<wgpu::BufferAddress>>,
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
        let glow_range = if celestial_horizon_glow_enabled(sky_state) {
            sky_state.glow()
        } else {
            None
        }
        .map(|glow| {
            let range = sky_vertex_slot_range(SINGLE_VIEW_SLOT, GLOW_VERTEX_COUNT);
            queue.write_buffer(
                &self.glow_vertex_buffer,
                range.start,
                &vertex_bytes(&sky_glow_vertices(glow, self.color_transform)),
            );
            range
        });
        let sun_range = celestial_sun_body_enabled(sky_state).then(|| {
            let range = celestial_quad_slot_range(SINGLE_VIEW_SLOT, SUN_BODY_QUAD);
            let mode = if sky_state.is_reference_profile() {
                TEXTURED_SUN_MODE
            } else {
                SQUARE_SUN_MODE
            };
            queue.write_buffer(
                &self.sun_vertex_buffer,
                range.start,
                &sun_vertex_bytes(&celestial_quad_vertices(
                    Vec3::from_array(sky_state.sun_direction()),
                    sky_state.sun_opacity(),
                    sky_state.sun_angular_diameter_degrees(),
                    mode,
                    0.0,
                )),
            );
            range
        });
        let halo_range = celestial_sun_halo_enabled(sky_state).then(|| {
            let range = celestial_quad_slot_range(SINGLE_VIEW_SLOT, SUN_HALO_QUAD);
            queue.write_buffer(
                &self.sun_vertex_buffer,
                range.start,
                &sun_vertex_bytes(&celestial_quad_vertices(
                    Vec3::from_array(sky_state.sun_direction()),
                    sky_state.sun_opacity(),
                    MCLONE_SUN_HALO_ANGULAR_DIAMETER_DEGREES,
                    SUN_HALO_MODE,
                    0.0,
                )),
            );
            range
        });
        let moon_range = celestial_moon_vertices(sky_state).map(|vertices| {
            let range = celestial_quad_slot_range(SINGLE_VIEW_SLOT, MOON_BODY_QUAD);
            queue.write_buffer(
                &self.sun_vertex_buffer,
                range.start,
                &sun_vertex_bytes(&vertices),
            );
            range
        });
        (
            clear_color,
            disc_range,
            glow_range,
            sun_range,
            halo_range,
            moon_range,
        )
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

fn celestial_vertex_slot_size() -> wgpu::BufferAddress {
    SUN_VERTEX_BYTE_SIZE * (SUN_VERTEX_COUNT * CELESTIAL_QUAD_COUNT) as wgpu::BufferAddress
}

fn celestial_quad_slot_range(
    view_slot: PerViewSlot,
    quad_index: usize,
) -> Range<wgpu::BufferAddress> {
    debug_assert!(quad_index < CELESTIAL_QUAD_COUNT);
    let slot = view_slot.byte_range(celestial_vertex_slot_size());
    let quad_size = SUN_VERTEX_BYTE_SIZE * SUN_VERTEX_COUNT as wgpu::BufferAddress;
    let start = slot.start + quad_size * quad_index as wgpu::BufferAddress;
    start..start + quad_size
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
    moon_pipeline: wgpu::RenderPipeline,
    star_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    star_uniform_buffer: wgpu::Buffer,
    star_bind_group: wgpu::BindGroup,
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
            celestial_additive_blend(),
        );
        let moon_pipeline = make_sun_pipeline(
            device,
            "mclone_moon_multiview_pipeline",
            &sun_pipeline_layout,
            &sun_shader,
            color_format,
            NonZeroU32::new(2),
            celestial_alpha_blend(),
        );
        let star_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_star_multiview_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/sky_stars_multiview.wgsl").into(),
            ),
        });
        let star_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_star_multiview_uniforms"),
            size: STAR_MULTIVIEW_UNIFORM_BYTE_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let star_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_star_multiview_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(STAR_MULTIVIEW_UNIFORM_BYTE_SIZE),
                    },
                    count: None,
                }],
            });
        let star_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_star_multiview_bind_group"),
            layout: &star_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: star_uniform_buffer.as_entire_binding(),
            }],
        });
        let star_pipeline = make_star_pipeline(
            device,
            "mclone_star_multiview_pipeline",
            &star_bind_group_layout,
            &star_shader,
            color_format,
            NonZeroU32::new(2),
        );
        Self {
            disc_pipeline,
            glow_pipeline,
            sun_pipeline,
            moon_pipeline,
            star_pipeline,
            uniform_buffer,
            bind_group,
            star_uniform_buffer,
            star_bind_group,
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

    fn write_star_uniforms(
        &self,
        queue: &wgpu::Queue,
        sky_view_projections: [Mat4; 2],
        parameters: StarUniformParameters,
    ) {
        let bytes = star_multiview_uniform_bytes(sky_view_projections, parameters);
        queue.write_buffer(&self.star_uniform_buffer, 0, &bytes);
    }
}

fn color_transform_rgba(color: [f32; 4], transform: RenderTargetColorTransform) -> [f32; 4] {
    let [r, g, b] = color_transform_rgb([color[0], color[1], color[2]], transform);
    [r, g, b, color[3]]
}

fn celestial_additive_blend() -> wgpu::BlendState {
    wgpu::BlendState {
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
    }
}

fn celestial_alpha_blend() -> wgpu::BlendState {
    wgpu::BlendState::ALPHA_BLENDING
}

fn create_celestial_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::BindGroup {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
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
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mclone_celestial_nearest_sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mclone_celestial_texture_bind_group"),
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
        ],
    })
}

fn make_star_pipeline(
    device: &wgpu::Device,
    label: &str,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    multiview: Option<NonZeroU32>,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mclone_star_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: STAR_INSTANCE_BYTE_SIZE,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: 16,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: 32,
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
                blend: Some(celestial_additive_blend()),
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

fn mclone_star_catalog() -> Vec<StarInstance> {
    let anchors: &[(f32, f32, f32)] = &[
        (0.047, 89.26, 0.0),
        (0.166, 29.0, 1.0),
        (0.214, 7.4, 2.0),
        (0.322, -8.2, 0.0),
        (0.418, 38.8, 1.0),
        (0.546, -60.4, 2.0),
        (0.681, 19.1, 0.0),
        (0.836, -26.4, 1.0),
    ];
    let mut catalog = Vec::with_capacity(MCLONE_STAR_COUNT as usize);
    for &(right_ascension, declination, color_class) in anchors {
        catalog.push(star_instance(
            right_ascension,
            declination.to_radians(),
            0.095,
            1.0,
            color_class,
            0.0,
        ));
    }
    let mut state = 0x6d2b_79f5_u32;
    while catalog.len() < MCLONE_STAR_COUNT as usize {
        let right_ascension = random_unit(&mut state);
        let declination = (random_unit(&mut state) * 2.0 - 1.0).asin();
        let brightness_seed = random_unit(&mut state);
        let brightness = 0.28 + brightness_seed.powi(3) * 0.72;
        let size = 0.025 + brightness * 0.055;
        let color_class = (next_random(&mut state) % 3) as f32;
        let orientation = random_unit(&mut state) * std::f32::consts::TAU;
        catalog.push(star_instance(
            right_ascension,
            declination,
            size,
            brightness,
            color_class,
            orientation,
        ));
    }
    catalog.sort_by(|left, right| right[5].total_cmp(&left[5]));
    catalog
}

fn star_instance(
    right_ascension_turns: f32,
    declination_radians: f32,
    angular_size_degrees: f32,
    brightness: f32,
    color_class: f32,
    orientation_radians: f32,
) -> StarInstance {
    let right_ascension = right_ascension_turns * std::f32::consts::TAU;
    let half_size = SUN_DISTANCE * (angular_size_degrees.to_radians() * 0.5).tan();
    [
        right_ascension.sin(),
        right_ascension.cos(),
        declination_radians.sin(),
        declination_radians.cos(),
        half_size,
        brightness,
        color_class,
        orientation_radians.sin(),
        orientation_radians.cos(),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ReferenceStarSeedRecord {
    right_ascension_turns: f32,
    declination_radians: f32,
    angular_size_degrees: f32,
    orientation_radians: f32,
}

/// Exact accepted candidates from Java 1.17.1 `LevelRenderer.drawStars`.
///
/// Java builds four static vertices per accepted candidate. The shared GPU
/// path stores the equivalent direction, angular half-size, and random roll,
/// then expands the same square from one immutable instance on the GPU.
fn reference_star_catalog() -> Vec<StarInstance> {
    reference_star_seed_records()
        .into_iter()
        .map(|record| {
            star_instance(
                record.right_ascension_turns,
                record.declination_radians,
                record.angular_size_degrees,
                1.0,
                1.0,
                record.orientation_radians,
            )
        })
        .collect()
}

fn reference_star_seed_records() -> Vec<ReferenceStarSeedRecord> {
    let mut random = JavaRandom::new(10_842);
    let mut records = Vec::with_capacity(REFERENCE_STAR_CANDIDATE_COUNT as usize);
    for _ in 0..REFERENCE_STAR_CANDIDATE_COUNT {
        let mut x = f64::from(random.next_float() * 2.0 - 1.0);
        let mut y = f64::from(random.next_float() * 2.0 - 1.0);
        let mut z = f64::from(random.next_float() * 2.0 - 1.0);
        let half_size = f64::from(0.15_f32 + random.next_float() * 0.1);
        let radius_squared = x * x + y * y + z * z;
        if !(0.01..1.0).contains(&radius_squared) {
            continue;
        }
        let inverse_radius = radius_squared.sqrt().recip();
        x *= inverse_radius;
        y *= inverse_radius;
        z *= inverse_radius;
        let orientation = random.next_double() * std::f64::consts::TAU;

        // Map the reference Y-up star sphere through its fixed -90° Y rig
        // into Mclone's +X east, +Y up, +Z south horizon convention. At zero
        // sidereal angle the star shader reconstructs this exact direction.
        let right_ascension =
            (-z).atan2(y).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
        let declination = (-x).asin();
        let angular_size = 2.0 * (half_size / 100.0).atan().to_degrees();
        records.push(ReferenceStarSeedRecord {
            right_ascension_turns: right_ascension as f32,
            declination_radians: declination as f32,
            angular_size_degrees: angular_size as f32,
            orientation_radians: orientation as f32,
        });
    }
    records
}

#[derive(Clone, Copy)]
struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 25_214_903_917;
    const ADDEND: u64 = 11;
    const MASK: u64 = (1_u64 << 48) - 1;

    fn new(seed: i64) -> Self {
        Self {
            seed: (seed as u64 ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    fn next_bits(&mut self, bits: u32) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND)
            & Self::MASK;
        (self.seed >> (48 - bits)) as u32
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 / 16_777_216.0
    }

    fn next_double(&mut self) -> f64 {
        let upper = u64::from(self.next_bits(26));
        let lower = u64::from(self.next_bits(27));
        ((upper << 27) + lower) as f64 / (1_u64 << 53) as f64
    }
}

fn next_random(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

fn random_unit(state: &mut u32) -> f32 {
    (next_random(state) >> 8) as f32 / 16_777_216.0
}

fn celestial_star_draw(
    sky_state: SkyRenderState,
    full_count: u32,
) -> Option<(u32, StarUniformParameters)> {
    let celestial = sky_state.celestial()?;
    let count = celestial.settings.star_density.selected_count(full_count);
    if count == 0 {
        return None;
    }
    let visibility = if let Some(reference_brightness) = sky_state.reference_star_brightness() {
        reference_brightness
    } else {
        let moon_suppression = celestial.lunar_sample.moonlight_factor * 0.22;
        celestial.star_visibility * (1.0 - moon_suppression)
    }
    .clamp(0.0, 1.0);
    if visibility <= 0.001 {
        return None;
    }
    let latitude = celestial.effective_latitude_degrees.to_radians();
    let sidereal = celestial.local_sidereal_angle_turns * std::f32::consts::TAU;
    Some((
        count,
        [
            sidereal.cos(),
            sidereal.sin(),
            latitude.cos(),
            latitude.sin(),
            visibility,
            0.0,
            0.0,
            0.0,
        ],
    ))
}

fn prepared_celestial_stats(
    sun_body: bool,
    sun_halo: bool,
    horizon_glow: bool,
    moon_body: bool,
    submitted_star_count: u32,
    catalog_star_count: u32,
    resident_resource_bytes: u64,
) -> CelestialRenderStats {
    let feature_buffer_writes = u32::from(sun_body)
        + u32::from(sun_halo)
        + u32::from(horizon_glow)
        + u32::from(moon_body)
        + u32::from(submitted_star_count > 0);
    CelestialRenderStats {
        sun_body_draws: u32::from(sun_body),
        sun_halo_draws: u32::from(sun_halo),
        horizon_glow_draws: u32::from(horizon_glow),
        moon_body_draws: u32::from(moon_body),
        star_draws: u32::from(submitted_star_count > 0),
        submitted_star_count,
        catalog_star_count,
        feature_buffer_writes,
        resident_resource_bytes,
    }
}

fn star_instance_bytes(instances: &[StarInstance]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(instances.len() * STAR_INSTANCE_BYTE_SIZE as usize);
    for instance in instances {
        for value in instance {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn star_uniform_bytes(
    view_projection: Mat4,
    parameters: StarUniformParameters,
) -> [u8; STAR_UNIFORM_BYTE_SIZE as usize] {
    let mut bytes = [0; STAR_UNIFORM_BYTE_SIZE as usize];
    bytes[..SKY_UNIFORM_BYTE_SIZE as usize]
        .copy_from_slice(&matrix_bytes(view_projection.to_cols_array_2d()));
    for (index, value) in parameters.into_iter().enumerate() {
        let start = SKY_UNIFORM_BYTE_SIZE as usize + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn star_multiview_uniform_bytes(
    view_projections: [Mat4; 2],
    parameters: StarUniformParameters,
) -> [u8; STAR_MULTIVIEW_UNIFORM_BYTE_SIZE as usize] {
    let mut bytes = [0; STAR_MULTIVIEW_UNIFORM_BYTE_SIZE as usize];
    bytes[..SKY_UNIFORM_BYTE_SIZE as usize]
        .copy_from_slice(&matrix_bytes(view_projections[0].to_cols_array_2d()));
    bytes[SKY_UNIFORM_BYTE_SIZE as usize..SKY_MULTIVIEW_UNIFORM_BYTE_SIZE as usize]
        .copy_from_slice(&matrix_bytes(view_projections[1].to_cols_array_2d()));
    for (index, value) in parameters.into_iter().enumerate() {
        let start = SKY_MULTIVIEW_UNIFORM_BYTE_SIZE as usize + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn make_sun_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    multiview: Option<NonZeroU32>,
    blend: wgpu::BlendState,
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
                    wgpu::VertexAttribute {
                        offset: 24,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Float32,
                    },
                    wgpu::VertexAttribute {
                        offset: 28,
                        shader_location: 4,
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
                blend: Some(blend),
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

fn celestial_quad_vertices(
    direction: Vec3,
    opacity: f32,
    angular_diameter_degrees: f32,
    mode: f32,
    phase: f32,
) -> [SunVertex; SUN_VERTEX_COUNT] {
    oriented_celestial_quad_vertices(
        direction,
        None,
        opacity,
        angular_diameter_degrees,
        mode,
        phase,
    )
}

fn oriented_celestial_quad_vertices(
    direction: Vec3,
    right_hint: Option<Vec3>,
    opacity: f32,
    angular_diameter_degrees: f32,
    mode: f32,
    phase: f32,
) -> [SunVertex; SUN_VERTEX_COUNT] {
    let direction = direction.try_normalize().unwrap_or(Vec3::Y);
    let reference_up = if direction.y.abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let right = right_hint
        .and_then(|right| (right - direction * right.dot(direction)).try_normalize())
        .unwrap_or_else(|| direction.cross(reference_up).normalize());
    let up = right.cross(direction).normalize();
    let center = direction * SUN_DISTANCE;
    let half_size = SUN_DISTANCE * (angular_diameter_degrees.to_radians() * 0.5).tan();
    let right = right * half_size;
    let up = up * half_size;
    [
        to_sun_vertex(center - right + up, [0.0, 0.0], opacity, mode, phase),
        to_sun_vertex(center + right + up, [1.0, 0.0], opacity, mode, phase),
        to_sun_vertex(center + right - up, [1.0, 1.0], opacity, mode, phase),
        to_sun_vertex(center - right - up, [0.0, 1.0], opacity, mode, phase),
    ]
}

fn to_sun_vertex(position: Vec3, uv: [f32; 2], opacity: f32, mode: f32, phase: f32) -> SunVertex {
    [
        position.x, position.y, position.z, uv[0], uv[1], opacity, mode, phase,
    ]
}

fn celestial_sun_body_enabled(sky_state: SkyRenderState) -> bool {
    sky_state
        .celestial()
        .map_or(true, |celestial| celestial.settings.sun_body_enabled)
}

fn celestial_sun_halo_enabled(sky_state: SkyRenderState) -> bool {
    !sky_state.is_reference_profile()
        && sky_state
            .celestial()
            .is_some_and(|celestial| celestial.settings.sun_halo_enabled)
}

fn celestial_horizon_glow_enabled(sky_state: SkyRenderState) -> bool {
    sky_state
        .celestial()
        .map_or(true, |celestial| celestial.settings.horizon_glow_enabled)
}

fn celestial_moon_vertices(sky_state: SkyRenderState) -> Option<[SunVertex; SUN_VERTEX_COUNT]> {
    let celestial = sky_state.celestial()?;
    if !celestial.settings.moon_body_enabled {
        return None;
    }
    let reference = sky_state.is_reference_profile();
    let direction = Vec3::from_array(sky_state.moon_direction()?);
    let opacity = if reference {
        1.0
    } else {
        smoothstep_f32(-2.0, 4.0, celestial.lunar_sample.elevation_degrees)
    };
    if opacity <= 0.001 {
        return None;
    }
    let (mode, phase, right_hint) = if reference {
        let semantic_frame =
            celestial.lunar_phase.steps() as u32 * 8 / u32::from(mclone_season::LUNAR_PHASE_STEPS);
        let java_frame = (semantic_frame + 4) % 8;
        (REFERENCE_MOON_MODE, java_frame as f32, None)
    } else {
        (
            SQUARE_MOON_MODE,
            celestial.lunar_phase.turns() as f32,
            Some(Vec3::from_array(celestial.lunar_sample.lit_limb_tangent)),
        )
    };
    Some(oriented_celestial_quad_vertices(
        direction,
        right_hint,
        opacity,
        sky_state.moon_angular_diameter_degrees(),
        mode,
        phase,
    ))
}

fn smoothstep_f32(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    use crate::sky::CelestialRenderState;
    use mclone_season::{
        CelestialDebugSettings, LunarInput, LunarPhase, LunarSample, MCLONE_AXIAL_TILT_DEGREES,
        MCLONE_LUNAR_ORBIT_INCLINATION_DEGREES, OrbitalPhase,
    };

    fn celestial(settings: CelestialDebugSettings) -> CelestialRenderState {
        let lunar_sample = LunarSample::compute(LunarInput {
            orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
            lunar_phase: LunarPhase::FULL,
            effective_latitude_degrees: 0.0,
            solar_time_fraction: 0.0,
            axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
            orbital_inclination_degrees: MCLONE_LUNAR_ORBIT_INCLINATION_DEGREES,
            node_phase: 0.0,
        })
        .unwrap();
        CelestialRenderState {
            settings,
            orbital_phase: OrbitalPhase::NORTHWARD_EQUINOX,
            solar_time_fraction: 0.0,
            solar_direction: [0.0, 1.0, 0.0],
            solar_elevation_degrees: 90.0,
            lunar_phase: LunarPhase::FULL,
            lunar_sample,
            effective_latitude_degrees: 0.0,
            local_sidereal_angle_turns: 0.0,
            star_visibility: 1.0,
        }
    }

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
    fn generated_moon_fallback_is_a_complete_reference_order_atlas() {
        let assets = MoonTextureAssets::generated_fallback();
        assert_eq!((assets.width, assets.height), (64, 32));
        assert_eq!(assets.rgba.len(), 64 * 32 * 4);
        assert!(assets.source_path.is_none());
        let full_center = ((8 * assets.width + 8) * 4) as usize;
        let new_center = ((24 * assets.width + 8) * 4) as usize;
        assert_eq!(assets.rgba[full_center], 232);
        assert_eq!(assets.rgba[new_center], 34);
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
        let vertices = celestial_quad_vertices(
            Vec3::Y,
            0.625,
            crate::sky::VANILLA_SUN_ANGULAR_DIAMETER_DEGREES,
            TEXTURED_SUN_MODE,
            0.0,
        );
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
    fn mclone_sun_quad_has_earth_like_angular_diameter() {
        let diameter = crate::sky::MCLONE_SUN_ANGULAR_DIAMETER_DEGREES;
        let vertices = celestial_quad_vertices(Vec3::Y, 1.0, diameter, SQUARE_SUN_MODE, 0.0);
        let half_size = (vertices[1][2] - vertices[0][2]).abs() * 0.5;
        let measured_degrees = 2.0 * (half_size / SUN_DISTANCE).atan().to_degrees();
        assert!((measured_degrees - 0.53).abs() < 1.0e-4);
        assert_eq!(vertices[0][6], SQUARE_SUN_MODE);
    }

    #[test]
    fn celestial_controls_gate_sun_halo_and_horizon_work_independently() {
        let baseline = SkyRenderState::mclone_fixed(0.0, 0.0)
            .with_celestial(celestial(CelestialDebugSettings::default()));
        assert!(celestial_sun_body_enabled(baseline));
        assert!(celestial_sun_halo_enabled(baseline));
        assert!(celestial_horizon_glow_enabled(baseline));

        let mut settings = CelestialDebugSettings::default();
        settings.sun_body_enabled = false;
        settings.sun_halo_enabled = false;
        settings.horizon_glow_enabled = false;
        let off = SkyRenderState::mclone_fixed(0.0, 0.0).with_celestial(celestial(settings));
        assert!(!celestial_sun_body_enabled(off));
        assert!(!celestial_sun_halo_enabled(off));
        assert!(!celestial_horizon_glow_enabled(off));

        let retained =
            SkyRenderState::vanilla(0.0, 0.0).with_celestial(celestial(Default::default()));
        assert!(celestial_sun_body_enabled(retained));
        assert!(!celestial_sun_halo_enabled(retained));
    }

    #[test]
    fn original_and_reference_moons_keep_distinct_shape_phase_and_size_laws() {
        let original = SkyRenderState::mclone_fixed(0.5, std::f32::consts::PI)
            .with_celestial(celestial(Default::default()));
        let original_vertices = celestial_moon_vertices(original).unwrap();
        assert_eq!(original_vertices[0][6], SQUARE_MOON_MODE);
        assert_eq!(original_vertices[0][7], 0.5);
        let original_half_size = Vec3::from_array([
            original_vertices[1][0] - original_vertices[0][0],
            original_vertices[1][1] - original_vertices[0][1],
            original_vertices[1][2] - original_vertices[0][2],
        ])
        .length()
            * 0.5;
        let original_degrees = 2.0 * (original_half_size / SUN_DISTANCE).atan().to_degrees();
        assert!((original_degrees - crate::sky::MCLONE_MOON_ANGULAR_DIAMETER_DEGREES).abs() < 1e-4);

        let retained =
            SkyRenderState::vanilla(0.0, 0.0).with_celestial(celestial(Default::default()));
        let retained_vertices = celestial_moon_vertices(retained).unwrap();
        assert_eq!(retained_vertices[0][6], REFERENCE_MOON_MODE);
        assert_eq!(retained_vertices[0][7], 0.0);

        let mut off = CelestialDebugSettings::default();
        off.moon_body_enabled = false;
        assert!(
            celestial_moon_vertices(
                SkyRenderState::mclone_fixed(0.5, std::f32::consts::PI)
                    .with_celestial(celestial(off))
            )
            .is_none()
        );
    }

    #[test]
    fn star_catalog_is_stable_bounded_brightness_sorted_and_nested() {
        let first = mclone_star_catalog();
        let second = mclone_star_catalog();
        assert_eq!(first, second);
        assert_eq!(first.len(), MCLONE_STAR_COUNT as usize);
        assert!(first.len() as u32 <= MCLONE_STAR_CATALOG_MAX_COUNT);
        assert!(first.windows(2).all(|pair| pair[0][5] >= pair[1][5]));
        assert!(star_instance_bytes(&first).len() < 128 * 1_024);
        assert!(first.iter().all(|star| {
            (star[0].hypot(star[1]) - 1.0).abs() < 1.0e-5
                && (star[2].hypot(star[3]) - 1.0).abs() < 1.0e-5
                && (star[7].hypot(star[8]) - 1.0).abs() < 1.0e-5
        }));

        let base = SkyRenderState::mclone_fixed(0.5, std::f32::consts::PI);
        for (density, expected) in [
            (mclone_season::CelestialStarDensity::Quarter, 384),
            (mclone_season::CelestialStarDensity::Half, 768),
            (mclone_season::CelestialStarDensity::Full, 1_536),
        ] {
            let mut settings = CelestialDebugSettings::default();
            settings.star_density = density;
            assert_eq!(
                celestial_star_draw(base.with_celestial(celestial(settings)), MCLONE_STAR_COUNT)
                    .unwrap()
                    .0,
                expected
            );
        }
        let mut off = CelestialDebugSettings::default();
        off.star_density = mclone_season::CelestialStarDensity::Off;
        assert!(
            celestial_star_draw(base.with_celestial(celestial(off)), MCLONE_STAR_COUNT).is_none()
        );
    }

    #[test]
    fn retained_star_catalog_matches_java_10842_candidate_stream() {
        let catalog = reference_star_catalog();
        let records = reference_star_seed_records();
        assert_eq!(catalog.len(), REFERENCE_STAR_COUNT as usize);
        assert_eq!(catalog, reference_star_catalog());
        assert_eq!(records.len(), REFERENCE_STAR_COUNT as usize);
        assert_eq!(records[0].right_ascension_turns.to_bits(), 0x3f67_9c06);
        assert_eq!(records[0].declination_radians.to_bits(), 0x3f0f_bf68);
        assert_eq!(records[0].angular_size_degrees.to_bits(), 0x3e30_3a8d);
        assert_eq!(records[0].orientation_radians.to_bits(), 0x408e_eb9c);
        assert!(
            records
                .iter()
                .all(|star| (0.171..=0.287).contains(&star.angular_size_degrees))
        );
    }

    #[test]
    fn celestial_cost_receipt_reports_exact_optional_work() {
        let resident = 48_000;
        assert_eq!(
            prepared_celestial_stats(false, false, false, false, 0, MCLONE_STAR_COUNT, resident),
            CelestialRenderStats {
                catalog_star_count: MCLONE_STAR_COUNT,
                resident_resource_bytes: resident,
                ..CelestialRenderStats::default()
            }
        );
        assert_eq!(
            prepared_celestial_stats(
                true,
                true,
                true,
                true,
                MCLONE_STAR_COUNT,
                MCLONE_STAR_COUNT,
                resident,
            ),
            CelestialRenderStats {
                sun_body_draws: 1,
                sun_halo_draws: 1,
                horizon_glow_draws: 1,
                moon_body_draws: 1,
                star_draws: 1,
                submitted_star_count: MCLONE_STAR_COUNT,
                catalog_star_count: MCLONE_STAR_COUNT,
                feature_buffer_writes: 5,
                resident_resource_bytes: resident,
            }
        );
    }

    #[test]
    fn sun_shaders_parse_for_mono_and_multiview() {
        for source in [
            include_str!("shaders/sky_sun.wgsl"),
            include_str!("shaders/sky_sun_multiview.wgsl"),
            include_str!("shaders/sky_stars.wgsl"),
            include_str!("shaders/sky_stars_multiview.wgsl"),
        ] {
            naga::front::wgsl::parse_str(source).expect("celestial WGSL parses");
        }
    }
}
