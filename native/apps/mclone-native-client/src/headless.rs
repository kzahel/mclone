use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::gui::GuiRenderer;
use mclone_render::headless::{
    HEADLESS_FORMAT, HeadlessChunkOptions, HeadlessFrameOptions, write_headless_frame_png,
    write_headless_textured_sections_png_with_options,
};
use mclone_ui::GuiScale;

use crate::camera::SpectatorCamera;
use crate::cli::{HeadlessScreenshotOptions, SceneOptions};
use crate::{
    DebugPaneStats, FramePacingDebugStats, FramePacingUiState, FrameTimingStats, NativeUi,
    RenderStreamStats, SceneTexturedSections, WindowSceneRuntime, poll_window_runtime_until_idle,
    record_render_section_update_stats, render_full_frame,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeadlessScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) gui_command_count: usize,
}
pub(crate) fn write_headless_chunk_scenarios(
    directory: &Path,
    width: u32,
    height: u32,
    scene: &SceneOptions,
    scene_mesh: &SceneTexturedSections,
    render_options: TexturedSectionRenderOptions,
) -> Result<Vec<mclone_render::headless::HeadlessChunkReport>> {
    let mut reports = Vec::new();
    for (name, camera) in chunk_capture_scenarios(scene) {
        let path = directory.join(format!("{name}.png"));
        reports.push(write_headless_textured_sections_png_with_options(
            HeadlessChunkOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
                camera,
            },
            &scene_mesh.sections,
            scene_mesh.atlas.as_upload(),
            render_options,
        )?);
    }
    Ok(reports)
}

pub(crate) fn run_headless_screenshot(
    options: &HeadlessScreenshotOptions,
) -> Result<HeadlessScreenshotReport> {
    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
    poll_window_runtime_until_idle(&mut runtime)?;
    let spectator = SpectatorCamera::spawn_for_scene(&options.scene);
    let section_update = runtime.sync_all_render_sections(spectator.position)?;
    let sections = runtime.cached_sections();
    if sections.is_empty() {
        bail!(
            "headless screenshot seed={} center=({}, {}) radius={} produced no render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.chunk_radius
        );
    }

    let mut ui = NativeUi::new(options.scene.chunk_radius);
    ui.set_screen(options.ui.native_screen());
    ui.set_scale(GuiScale::from_pixels(options.width, options.height));

    let mut render_stats = RenderStreamStats::default();
    let debug_pane = options.debug_pane;
    let render_options = options.render_options;
    let camera = spectator.camera(runtime.radius_chunks);
    let runtime_stats = runtime.stats();
    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: section_update.rebuilt_section_count(),
        removed_section_count: section_update.removed_section_count(),
        uploaded_vertex_count: section_update.rebuilt_vertex_count,
        uploaded_index_count: section_update.rebuilt_index_count,
    };

    let (frame_report, summary) = write_headless_frame_png(
        HeadlessFrameOptions {
            path: options.path.clone(),
            width: options.width,
            height: options.height,
        },
        |frame| {
            let depth =
                ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
            let mut draw = TexturedSectionDrawResources::new(
                frame.device,
                frame.queue,
                HEADLESS_FORMAT,
                &sections,
                runtime.mesh_assets.atlas.as_upload(),
            )?;
            draw.set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(spectator.position),
            );
            let mut gui = GuiRenderer::new(frame.device, HEADLESS_FORMAT);

            render_stats.section_count = draw.section_count();
            render_stats.index_count = draw.index_count();
            render_stats.face_count = quad_face_count_from_indices(render_stats.index_count);
            record_render_section_update_stats(&mut render_stats, &section_update, initial_upload);

            let debug_stats = debug_pane.then_some(DebugPaneStats {
                position: spectator.position,
                speed: spectator.speed,
                runtime: runtime_stats,
                render: render_stats,
                frame: FrameTimingStats::default(),
                pacing: FramePacingDebugStats::default(),
                section_occlusion: render_options.section_occlusion_culling,
                force_fullbright: render_options.force_fullbright,
            });

            render_full_frame(
                frame,
                &depth,
                &mut draw,
                &mut gui,
                camera,
                render_options,
                FramePacingUiState::default(),
                &ui,
                debug_stats,
                &mut render_stats,
            )
        },
    )?;

    Ok(HeadlessScreenshotReport {
        path: frame_report.path,
        width: frame_report.width,
        height: frame_report.height,
        byte_len: frame_report.byte_len,
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        index_count: summary.index_count,
        drawn_index_count: summary.drawn_index_count,
        gui_command_count: summary.gui_command_count,
    })
}

fn chunk_capture_scenarios(scene: &SceneOptions) -> [(&'static str, ChunkCamera); 3] {
    let overview =
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.chunk_radius);
    let mut orbit = overview;
    orbit.orbit(0.7, -0.16);
    let mut close = overview;
    close.zoom(0.45);
    close.orbit(-0.32, 0.08);
    [
        ("overview", overview),
        ("orbit-east", orbit),
        ("close", close),
    ]
}
