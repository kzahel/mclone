use anyhow::{Context, Result};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkRenderView,
    PreparedTexturedSectionRecords, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::entity::{ActorDrawResources, ActorInstance, ActorRenderStats};
use mclone_render::fog::RenderFog;
use mclone_render::gui::{GuiRenderOptions, GuiRenderer};
use mclone_render::screen_effect::{ScreenEffectsRenderer, UnderwaterOverlay};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameContext;
use mclone_render_session::RenderSectionCacheUpdate;
use mclone_ui::GuiDrawList;

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
