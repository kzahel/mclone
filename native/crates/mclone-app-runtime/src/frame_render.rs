use anyhow::{Context, Result, ensure};
use mclone_assets::AssetSource;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkRenderView, ChunkTextureAtlas,
    DEPTH_FORMAT, PreparedTexturedSectionRecords, TexturedSectionDrawResources,
    TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::color_profile::{DEFAULT_RENDER_SCALE, RenderConfig};
use mclone_render::entity::{
    ActorDrawResources, ActorInstance, ActorRenderStats, ActorTextureAtlas,
};
use mclone_render::fog::RenderFog;
use mclone_render::gui::{GuiRenderOptions, GuiRenderer};
use mclone_render::screen_effect::{ScreenEffectsRenderer, UnderwaterOverlay};
use mclone_render::selection_outline::{SelectionOutline, SelectionOutlineRenderer};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameContext;
use mclone_render_session::RenderSectionCacheUpdate;
use mclone_ui::GuiDrawList;

pub const MIN_FLAT_RENDER_SCALE: f32 = 0.25;
pub const MAX_FLAT_RENDER_SCALE: f32 = 2.0;
const SCALE_EPSILON: f32 = 0.000_1;

const FLAT_SCALE_PRESENT_WGSL: &str = r#"
@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var source_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );

    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(source_texture, source_sampler, input.uv);
}
"#;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderStreamStats {
    pub section_count: usize,
    pub drawn_section_count: usize,
    pub face_count: u32,
    pub drawn_face_count: u32,
    pub index_count: u32,
    pub drawn_index_count: u32,
    pub last_rebuilt_section_count: usize,
    pub last_removed_section_count: usize,
    pub last_rebuilt_vertex_count: u32,
    pub last_rebuilt_face_count: u32,
    pub last_rebuilt_index_count: u32,
    pub last_neighbor_ready_section_count: usize,
    pub last_near_exception_section_count: usize,
    pub last_deferred_section_count: usize,
    pub last_submitted_compile_section_count: usize,
    pub last_completed_compile_section_count: usize,
    pub last_stale_compile_section_count: usize,
    pub last_pending_compile_jobs: usize,
    pub last_visibility_graph_build_count: usize,
    pub last_visibility_graph_total_ms: f64,
    pub last_visibility_graph_worst_ms: f64,
    pub last_uploaded_section_count: usize,
    pub last_upload_removed_section_count: usize,
    pub last_uploaded_vertex_count: u32,
    pub last_uploaded_face_count: u32,
    pub last_uploaded_index_count: u32,
    pub last_remesh_ms: f64,
    pub last_upload_ms: f64,
    pub last_frame_ms: f32,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
    pub drawn_actor_index_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FullFrameRenderSummary {
    pub section_count: usize,
    pub drawn_section_count: usize,
    pub index_count: u32,
    pub drawn_index_count: u32,
    pub gui_command_count: usize,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FullFrameRenderTiming {
    pub terrain_records_ms: f64,
    pub terrain_cull_ms: f64,
    pub terrain_uniform_write_ms: f64,
    pub terrain_translucent_collect_ms: f64,
    pub terrain_translucent_sort_ms: f64,
    pub terrain_prepare_ms: f64,
    pub terrain_encode_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FullFrameGui {
    pub active: bool,
    pub covers_world: bool,
    pub scale: [f32; 2],
}

impl FullFrameGui {
    pub const fn new(active: bool, covers_world: bool, scale: [f32; 2]) -> Self {
        Self {
            active,
            covers_world,
            scale,
        }
    }
}

pub struct FlatRenderResources {
    render_config: RenderConfig,
    depth: ChunkDepthTarget,
    scaled_color: Option<FlatScaledColorTarget>,
    scale_presenter: Option<FlatScalePresenter>,
    sky: SkyRenderer,
    draw: TexturedSectionDrawResources,
    actors: ActorDrawResources,
    screen_effects: ScreenEffectsRenderer,
    selection_outline: SelectionOutlineRenderer,
    gui: GuiRenderer,
}

pub struct FlatRenderResourcePartsMut<'a> {
    pub depth: &'a ChunkDepthTarget,
    pub sky: &'a SkyRenderer,
    pub draw: &'a mut TexturedSectionDrawResources,
    pub actors: &'a mut ActorDrawResources,
    pub screen_effects: &'a mut ScreenEffectsRenderer,
    pub selection_outline: &'a mut SelectionOutlineRenderer,
    pub gui: &'a mut GuiRenderer,
}

impl FlatRenderResources {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_size: [u32; 2],
        render_config: RenderConfig,
        chunk_atlas: ChunkTextureAtlas<'_>,
        actor_atlas: ActorTextureAtlas<'_>,
        asset_source: &impl AssetSource,
    ) -> Result<Self> {
        let render_config = Self::validate_supported_config(render_config)?;
        let render_size = flat_render_size(target_size, render_config);
        let depth = ChunkDepthTarget::new(device, render_size[0], render_size[1]);
        let scale_presenter = needs_scaled_target(render_config)
            .then(|| FlatScalePresenter::new(device, render_config.color_format));
        let scaled_color = scale_presenter.as_ref().map(|presenter| {
            FlatScaledColorTarget::new(
                device,
                render_size,
                render_config.color_format,
                &presenter.bind_group_layout,
                &presenter.sampler,
            )
        });
        let draw = TexturedSectionDrawResources::new(
            device,
            queue,
            render_config.color_format,
            &[],
            chunk_atlas,
        )
        .context("failed to initialize chunk draw resources")?;
        let sky = SkyRenderer::new_with_config(device, render_config);
        let actors =
            ActorDrawResources::new(device, queue, render_config.color_format, actor_atlas)
                .context("failed to initialize actor draw resources")?;
        let screen_effects =
            ScreenEffectsRenderer::new(device, queue, render_config.color_format, asset_source)
                .context("failed to initialize screen effects renderer")?;
        let selection_outline = SelectionOutlineRenderer::new(device, render_config.color_format);
        let gui = GuiRenderer::new(device, render_config.color_format);
        Ok(Self {
            render_config,
            depth,
            scaled_color,
            scale_presenter,
            sky,
            draw,
            actors,
            screen_effects,
            selection_outline,
            gui,
        })
    }

    pub fn validate_supported_config(render_config: RenderConfig) -> Result<RenderConfig> {
        let render_config = render_config.validate()?;
        ensure!(
            render_config.depth_format == DEPTH_FORMAT,
            "flat render resources require depth format {:?}, got {:?}",
            DEPTH_FORMAT,
            render_config.depth_format
        );
        ensure!(
            render_config.sample_count == 1,
            "flat render resources require sample_count=1, got {}",
            render_config.sample_count
        );
        ensure!(
            (MIN_FLAT_RENDER_SCALE..=MAX_FLAT_RENDER_SCALE).contains(&render_config.render_scale),
            "flat render resources require render_scale between {:.3} and {:.3}, got {:.3}",
            MIN_FLAT_RENDER_SCALE,
            MAX_FLAT_RENDER_SCALE,
            render_config.render_scale
        );
        ensure!(
            !render_config.hdr,
            "flat render resources do not support HDR targets yet"
        );
        Ok(render_config)
    }

    pub const fn render_config(&self) -> RenderConfig {
        self.render_config
    }

    pub fn render_size(&self) -> [u32; 2] {
        [self.depth.width, self.depth.height]
    }

    pub fn resize_frame_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let render_size = flat_render_size(size, self.render_config);
        self.depth.resize(device, render_size[0], render_size[1]);
        match (&mut self.scaled_color, &self.scale_presenter) {
            (Some(scaled_color), Some(presenter)) => scaled_color.resize(
                device,
                render_size,
                self.render_config.color_format,
                &presenter.bind_group_layout,
                &presenter.sampler,
            ),
            (None, _) => {}
            (Some(_), None) => unreachable!("scaled color target requires a presenter"),
        }
    }

    pub fn draw_mut(&mut self) -> &mut TexturedSectionDrawResources {
        &mut self.draw
    }

    pub fn section_count(&self) -> usize {
        self.draw.section_count()
    }

    pub fn index_count(&self) -> u32 {
        self.draw.index_count()
    }

    pub fn parts_mut(&mut self) -> FlatRenderResourcePartsMut<'_> {
        FlatRenderResourcePartsMut {
            depth: &self.depth,
            sky: &self.sky,
            draw: &mut self.draw,
            actors: &mut self.actors,
            screen_effects: &mut self.screen_effects,
            selection_outline: &mut self.selection_outline,
            gui: &mut self.gui,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_full_frame<BuildGuiDraw>(
        &mut self,
        frame: RenderFrameContext<'_>,
        camera: ChunkCamera,
        actor_instances: &[ActorInstance],
        underwater_overlay: Option<UnderwaterOverlay>,
        sky_clear_color: wgpu::Color,
        time_of_day: f32,
        sun_angle: f32,
        render_options: TexturedSectionRenderOptions,
        selection_outline: Option<&SelectionOutline>,
        gui: FullFrameGui,
        build_gui_draw: BuildGuiDraw,
        render_stats: &mut RenderStreamStats,
    ) -> Result<FullFrameRenderSummary>
    where
        BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
    {
        let RenderFrameContext {
            device,
            queue,
            encoder,
            target,
        } = frame;
        let render_target = self
            .scaled_color
            .as_ref()
            .map_or(target, |scaled| scaled.render_target());
        let render_view = camera.render_view(render_target.size[0], render_target.size[1]);
        let render_frame = RenderFrameContext::new(device, queue, encoder, render_target);
        let summary = render_full_frame_for_view(
            render_frame,
            &self.depth,
            &self.sky,
            &mut self.draw,
            Some(&mut self.actors),
            Some(&mut self.screen_effects),
            Some(&mut self.gui),
            render_view,
            actor_instances,
            underwater_overlay,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            gui,
            build_gui_draw,
            render_stats,
        )?;
        self.selection_outline.render(
            device,
            queue,
            encoder,
            render_target,
            &self.depth,
            render_view,
            selection_outline,
        );
        if let (Some(scaled), Some(presenter)) = (&self.scaled_color, &self.scale_presenter) {
            presenter.present(encoder, scaled, target.color_view);
        }
        Ok(summary)
    }
}

struct FlatScaledColorTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

impl FlatScaledColorTarget {
    fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let size = [size[0].max(1), size[1].max(1)];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_flat_scaled_color"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_flat_scaled_color_bind_group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Self {
            _texture: texture,
            view,
            bind_group,
            size,
        }
    }

    fn resize(
        &mut self,
        device: &wgpu::Device,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) {
        let size = [size[0].max(1), size[1].max(1)];
        if self.size == size {
            return;
        }
        *self = Self::new(device, size, format, bind_group_layout, sampler);
    }

    fn render_target(&self) -> mclone_render::target::RenderFrameTarget<'_> {
        mclone_render::target::RenderFrameTarget::color(&self.view, self.size)
    }
}

struct FlatScalePresenter {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl FlatScalePresenter {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_flat_scale_present_shader"),
            source: wgpu::ShaderSource::Wgsl(FLAT_SCALE_PRESENT_WGSL.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_flat_scale_present_bind_group_layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_flat_scale_present_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_flat_scale_present_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_flat_scale_present_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn present(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scaled: &FlatScaledColorTarget,
        output_view: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_flat_scale_present_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &scaled.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub fn scaled_frame_size(target_size: [u32; 2], render_scale: f32) -> [u32; 2] {
    [
        ((target_size[0].max(1) as f32) * render_scale)
            .round()
            .max(1.0) as u32,
        ((target_size[1].max(1) as f32) * render_scale)
            .round()
            .max(1.0) as u32,
    ]
}

fn flat_render_size(target_size: [u32; 2], render_config: RenderConfig) -> [u32; 2] {
    if needs_scaled_target(render_config) {
        scaled_frame_size(target_size, render_config.render_scale)
    } else {
        [target_size[0].max(1), target_size[1].max(1)]
    }
}

fn needs_scaled_target(render_config: RenderConfig) -> bool {
    (render_config.render_scale - DEFAULT_RENDER_SCALE).abs() > SCALE_EPSILON
}

#[allow(clippy::too_many_arguments)]
pub fn render_full_frame<BuildGuiDraw>(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    actors: Option<&mut ActorDrawResources>,
    screen_effects: Option<&mut ScreenEffectsRenderer>,
    gui_renderer: Option<&mut GuiRenderer>,
    camera: ChunkCamera,
    actor_instances: &[ActorInstance],
    underwater_overlay: Option<UnderwaterOverlay>,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    gui: FullFrameGui,
    build_gui_draw: BuildGuiDraw,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary>
where
    BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
{
    let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
    render_full_frame_for_view(
        frame,
        depth,
        sky,
        draw,
        actors,
        screen_effects,
        gui_renderer,
        render_view,
        actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        render_options,
        gui,
        build_gui_draw,
        render_stats,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_full_frame_for_view<BuildGuiDraw>(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    actors: Option<&mut ActorDrawResources>,
    screen_effects: Option<&mut ScreenEffectsRenderer>,
    gui_renderer: Option<&mut GuiRenderer>,
    render_view: ChunkRenderView,
    actor_instances: &[ActorInstance],
    underwater_overlay: Option<UnderwaterOverlay>,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    gui: FullFrameGui,
    build_gui_draw: BuildGuiDraw,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary>
where
    BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
{
    render_full_frame_for_view_inner(
        frame,
        depth,
        sky,
        draw,
        actors,
        screen_effects,
        gui_renderer,
        render_view,
        actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        render_options,
        gui,
        build_gui_draw,
        None,
        None,
        render_stats,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_full_frame_for_view_timed<BuildGuiDraw>(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    actors: Option<&mut ActorDrawResources>,
    screen_effects: Option<&mut ScreenEffectsRenderer>,
    gui_renderer: Option<&mut GuiRenderer>,
    render_view: ChunkRenderView,
    actor_instances: &[ActorInstance],
    underwater_overlay: Option<UnderwaterOverlay>,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    gui: FullFrameGui,
    build_gui_draw: BuildGuiDraw,
    render_stats: &mut RenderStreamStats,
) -> Result<(FullFrameRenderSummary, FullFrameRenderTiming)>
where
    BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
{
    let mut timing = FullFrameRenderTiming::default();
    let summary = render_full_frame_for_view_inner(
        frame,
        depth,
        sky,
        draw,
        actors,
        screen_effects,
        gui_renderer,
        render_view,
        actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        render_options,
        gui,
        build_gui_draw,
        None,
        Some(&mut timing),
        render_stats,
    )?;
    Ok((summary, timing))
}

#[allow(clippy::too_many_arguments)]
pub fn render_full_frame_for_view_with_prepared_records<BuildGuiDraw>(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    prepared_records: &PreparedTexturedSectionRecords,
    actors: Option<&mut ActorDrawResources>,
    screen_effects: Option<&mut ScreenEffectsRenderer>,
    gui_renderer: Option<&mut GuiRenderer>,
    render_view: ChunkRenderView,
    actor_instances: &[ActorInstance],
    underwater_overlay: Option<UnderwaterOverlay>,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    gui: FullFrameGui,
    build_gui_draw: BuildGuiDraw,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary>
where
    BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
{
    render_full_frame_for_view_inner(
        frame,
        depth,
        sky,
        draw,
        actors,
        screen_effects,
        gui_renderer,
        render_view,
        actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        render_options,
        gui,
        build_gui_draw,
        Some(prepared_records),
        None,
        render_stats,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_full_frame_for_view_with_prepared_records_timed<BuildGuiDraw>(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    prepared_records: &PreparedTexturedSectionRecords,
    actors: Option<&mut ActorDrawResources>,
    screen_effects: Option<&mut ScreenEffectsRenderer>,
    gui_renderer: Option<&mut GuiRenderer>,
    render_view: ChunkRenderView,
    actor_instances: &[ActorInstance],
    underwater_overlay: Option<UnderwaterOverlay>,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    gui: FullFrameGui,
    build_gui_draw: BuildGuiDraw,
    render_stats: &mut RenderStreamStats,
) -> Result<(FullFrameRenderSummary, FullFrameRenderTiming)>
where
    BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
{
    let mut timing = FullFrameRenderTiming::default();
    let summary = render_full_frame_for_view_inner(
        frame,
        depth,
        sky,
        draw,
        actors,
        screen_effects,
        gui_renderer,
        render_view,
        actor_instances,
        underwater_overlay,
        sky_clear_color,
        time_of_day,
        sun_angle,
        render_options,
        gui,
        build_gui_draw,
        Some(prepared_records),
        Some(&mut timing),
        render_stats,
    )?;
    Ok((summary, timing))
}

#[allow(clippy::too_many_arguments)]
fn render_full_frame_for_view_inner<BuildGuiDraw>(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    actors: Option<&mut ActorDrawResources>,
    screen_effects: Option<&mut ScreenEffectsRenderer>,
    gui_renderer: Option<&mut GuiRenderer>,
    render_view: ChunkRenderView,
    actor_instances: &[ActorInstance],
    underwater_overlay: Option<UnderwaterOverlay>,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    gui: FullFrameGui,
    build_gui_draw: BuildGuiDraw,
    prepared_records: Option<&PreparedTexturedSectionRecords>,
    mut timing: Option<&mut FullFrameRenderTiming>,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary>
where
    BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
{
    let fog = underwater_overlay
        .is_some()
        .then(RenderFog::underwater)
        .unwrap_or_default();
    let render_options = render_options
        .with_sky_darken(mclone_render::light_texture::sky_darken(time_of_day))
        .with_fog(fog);
    let mut actor_stats = ActorRenderStats::default();

    if !gui.covers_world {
        let background_clear_color = if fog.enabled {
            clear_frame_color(frame.encoder, frame.target.color_view, fog.clear_color());
            fog.clear_color()
        } else {
            sky.render(
                frame.queue,
                frame.encoder,
                frame.target.color_view,
                sky_clear_color,
                render_view.sky_view_projection(),
                time_of_day,
                sun_angle,
            );
            sky_clear_color
        };
        // The sky/clear pass prepared the background; the chunk pass loads it.
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            background_clear_color,
        )?
        .with_loaded_color();
        let frame_stats = match (timing.as_deref_mut(), prepared_records) {
            (Some(timing), Some(records)) => {
                let (frame_stats, terrain_timing) = draw.render_prepared_with_options_timed(
                    records,
                    frame.queue,
                    frame.encoder,
                    render_target,
                    render_view,
                    render_options,
                )?;
                timing.terrain_records_ms += terrain_timing.records_ms;
                timing.terrain_cull_ms += terrain_timing.cull_ms;
                timing.terrain_uniform_write_ms += terrain_timing.uniform_write_ms;
                timing.terrain_translucent_collect_ms += terrain_timing.translucent_collect_ms;
                timing.terrain_translucent_sort_ms += terrain_timing.translucent_sort_ms;
                timing.terrain_prepare_ms += terrain_timing.prepare_ms;
                timing.terrain_encode_ms += terrain_timing.encode_ms;
                frame_stats
            }
            (Some(timing), None) => {
                let (frame_stats, terrain_timing) = draw.render_with_options_timed(
                    frame.queue,
                    frame.encoder,
                    render_target,
                    render_view,
                    render_options,
                )?;
                timing.terrain_records_ms += terrain_timing.records_ms;
                timing.terrain_cull_ms += terrain_timing.cull_ms;
                timing.terrain_uniform_write_ms += terrain_timing.uniform_write_ms;
                timing.terrain_translucent_collect_ms += terrain_timing.translucent_collect_ms;
                timing.terrain_translucent_sort_ms += terrain_timing.translucent_sort_ms;
                timing.terrain_prepare_ms += terrain_timing.prepare_ms;
                timing.terrain_encode_ms += terrain_timing.encode_ms;
                frame_stats
            }
            (None, Some(records)) => draw.render_prepared_with_options(
                records,
                frame.queue,
                frame.encoder,
                render_target,
                render_view,
                render_options,
            )?,
            (None, None) => draw.render_with_options(
                frame.queue,
                frame.encoder,
                render_target,
                render_view,
                render_options,
            )?,
        };
        render_stats.drawn_section_count = frame_stats.drawn_section_count;
        render_stats.drawn_face_count = frame_stats.drawn_face_count();
        render_stats.drawn_index_count = frame_stats.drawn_index_count;
        if !actor_instances.is_empty() {
            actor_stats = actors
                .context("actor instances requested without actor draw resources")?
                .render(
                    frame.device,
                    frame.queue,
                    frame.encoder,
                    frame.target.with_depth(&depth.view),
                    render_view,
                    render_options,
                    actor_instances,
                )?;
        }
        if let Some(overlay) = underwater_overlay {
            screen_effects
                .context("underwater overlay requested without screen effects renderer")?
                .render_underwater(
                    frame.device,
                    frame.queue,
                    frame.encoder,
                    frame.target,
                    overlay,
                );
        }
    } else {
        render_stats.drawn_section_count = 0;
        render_stats.drawn_face_count = 0;
        render_stats.drawn_index_count = 0;
    }
    render_stats.actor_count = actor_instances.len();
    render_stats.drawn_actor_count = actor_stats.drawn_actor_count;
    render_stats.drawn_actor_index_count = actor_stats.index_count;

    let gui_draw = build_gui_draw(render_stats);
    let gui_command_count = gui_draw.commands().len();
    if gui.active {
        gui_renderer
            .context("active GUI requested without GUI renderer")?
            .render(
                frame.device,
                frame.queue,
                frame.encoder,
                frame.target,
                gui.scale,
                &gui_draw,
                if gui.covers_world {
                    GuiRenderOptions::clear(mclone_render::default_clear_color())
                } else {
                    GuiRenderOptions::overlay()
                },
            )?;
    }

    Ok(FullFrameRenderSummary {
        section_count: draw.section_count(),
        drawn_section_count: render_stats.drawn_section_count,
        index_count: draw.index_count(),
        drawn_index_count: render_stats.drawn_index_count,
        gui_command_count,
        actor_count: actor_instances.len(),
        drawn_actor_count: actor_stats.drawn_actor_count,
    })
}

pub fn record_render_section_update_stats(
    render_stats: &mut RenderStreamStats,
    section_update: &RenderSectionCacheUpdate,
    upload_report: TexturedSectionUploadReport,
) {
    render_stats.last_rebuilt_section_count = section_update.rebuilt_section_count();
    render_stats.last_removed_section_count = section_update.removed_section_count();
    render_stats.last_rebuilt_vertex_count = section_update.rebuilt_vertex_count;
    render_stats.last_rebuilt_face_count = section_update.rebuilt_face_count();
    render_stats.last_rebuilt_index_count = section_update.rebuilt_index_count;
    render_stats.last_neighbor_ready_section_count = section_update.neighbor_ready_section_count;
    render_stats.last_near_exception_section_count = section_update.near_exception_section_count;
    render_stats.last_deferred_section_count = section_update.deferred_section_count;
    render_stats.last_submitted_compile_section_count =
        section_update.submitted_compile_section_count;
    render_stats.last_completed_compile_section_count =
        section_update.completed_compile_section_count;
    render_stats.last_stale_compile_section_count = section_update.stale_compile_section_count;
    render_stats.last_pending_compile_jobs = section_update.pending_compile_jobs;
    render_stats.last_visibility_graph_build_count =
        section_update.visibility_graph_stats.build_count;
    render_stats.last_visibility_graph_total_ms = section_update.visibility_graph_stats.total_ms;
    render_stats.last_visibility_graph_worst_ms = section_update.visibility_graph_stats.worst_ms;
    render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
    render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
    render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
    render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
    render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
}

fn clear_frame_color(
    encoder: &mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    color: wgpu::Color,
) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mclone_world_background_clear_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        ..Default::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_render_resources_accept_default_supported_config() {
        assert!(FlatRenderResources::validate_supported_config(RenderConfig::default()).is_ok());
    }

    #[test]
    fn flat_render_resources_reject_unsupported_rebuild_config() {
        let sample_error = FlatRenderResources::validate_supported_config(
            RenderConfig::default().with_sample_count(4),
        )
        .unwrap_err()
        .to_string();
        assert!(sample_error.contains("sample_count=1"));

        let depth_error = FlatRenderResources::validate_supported_config(
            RenderConfig::default().with_depth_format(wgpu::TextureFormat::Depth32Float),
        )
        .unwrap_err()
        .to_string();
        assert!(depth_error.contains("depth format"));

        assert!(
            FlatRenderResources::validate_supported_config(
                RenderConfig::default().with_render_scale(0.75),
            )
            .is_ok()
        );

        let scale_error = FlatRenderResources::validate_supported_config(
            RenderConfig::default().with_render_scale(2.5),
        )
        .unwrap_err()
        .to_string();
        assert!(scale_error.contains("render_scale between"));

        let hdr_error =
            FlatRenderResources::validate_supported_config(RenderConfig::default().with_hdr(true))
                .unwrap_err()
                .to_string();
        assert!(hdr_error.contains("HDR"));
    }

    #[test]
    fn scaled_frame_size_rounds_and_clamps() {
        assert_eq!(scaled_frame_size([320, 180], 0.5), [160, 90]);
        assert_eq!(scaled_frame_size([321, 181], 0.5), [161, 91]);
        assert_eq!(scaled_frame_size([0, 0], 0.25), [1, 1]);
        assert_eq!(
            flat_render_size(
                [16_384, 16_384],
                RenderConfig::default().with_render_scale(1.0)
            ),
            [16_384, 16_384]
        );
    }
}
