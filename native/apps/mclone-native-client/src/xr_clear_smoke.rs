#[cfg(not(target_os = "android"))]
use std::{
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

#[cfg(not(target_os = "android"))]
use anyhow::{Context, Result, anyhow, bail};
#[cfg(target_os = "android")]
use anyhow::{Result, bail};
#[cfg(not(target_os = "android"))]
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::frame_render::{
    FullFrameGui, FullFrameRenderSummary, RenderStreamStats, record_render_section_update_stats,
    render_full_frame_for_view,
};
#[cfg(not(target_os = "android"))]
use mclone_mesh::quad_face_count_from_indices;
#[cfg(not(target_os = "android"))]
use mclone_render::chunk::{
    ChunkCamera, ChunkRenderView, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
#[cfg(not(target_os = "android"))]
use mclone_render::entity::{ActorDrawResources, ActorInstance};
#[cfg(not(target_os = "android"))]
use mclone_render::sky_render::SkyRenderer;
#[cfg(not(target_os = "android"))]
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
#[cfg(not(target_os = "android"))]
use mclone_ui::GuiDrawList;
#[cfg(not(target_os = "android"))]
use openxr as xr;

#[cfg(not(target_os = "android"))]
use crate::app::actor_instances_from_presentations;
use crate::cli::{XrClearSmokeOptions, XrMcloneSmokeOptions, XrViewPose};
#[cfg(not(target_os = "android"))]
use crate::frame_pacing::elapsed_ms;
#[cfg(not(target_os = "android"))]
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};

#[cfg(not(target_os = "android"))]
mod actions;
#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
mod graphics_metal;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
mod graphics_vulkan;
#[cfg(not(target_os = "android"))]
use actions::{OpenXrControllerActions, XrControllerInputSummary};
#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
use graphics_metal as platform_graphics;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
use graphics_vulkan as platform_graphics;

#[cfg(target_os = "android")]
pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
    let _ = options;
    bail!("--xr-clear-smoke is a desktop OpenXR smoke; Android XR packaging is a later target")
}

#[cfg(target_os = "android")]
pub(crate) fn run_mclone(options: XrMcloneSmokeOptions) -> Result<()> {
    let _ = options;
    bail!("--xr-mclone-smoke is a desktop OpenXR smoke; Android XR packaging is a later target")
}

#[cfg(not(target_os = "android"))]
const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;
#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
const XR_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
const XR_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
#[cfg(not(target_os = "android"))]
const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
#[cfg(not(target_os = "android"))]
const XR_SAMPLE_COUNT: u32 = 1;
#[cfg(not(target_os = "android"))]
const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "android"))]
const SESSION_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(25);
#[cfg(not(target_os = "android"))]
const XR_NEAR: f32 = 0.05;
#[cfg(not(target_os = "android"))]
const XR_FAR: f32 = 700.0;

#[cfg(not(target_os = "android"))]
#[derive(Debug)]
struct OpenXrRuntimeManifest {
    runtime_library_path: PathBuf,
}

#[cfg(not(target_os = "android"))]
enum DesktopXrSmoke {
    Clear { frames: u32 },
    Mclone { options: XrMcloneSmokeOptions },
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
    run_desktop_xr_smoke(DesktopXrSmoke::Clear {
        frames: options.frames,
    })
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run_mclone(options: XrMcloneSmokeOptions) -> Result<()> {
    run_desktop_xr_smoke(DesktopXrSmoke::Mclone { options })
}

#[cfg(not(target_os = "android"))]
fn run_desktop_xr_smoke(smoke: DesktopXrSmoke) -> Result<()> {
    let (entry, entry_source) = load_openxr_entry()?;
    let available = entry
        .enumerate_extensions()
        .context("enumerate OpenXR instance extensions")?;
    let mut enabled_extensions = xr::ExtensionSet::default();
    let graphics_extension =
        enable_platform_graphics_extension(&available, &mut enabled_extensions)?;

    println!("OpenXR entry: {entry_source}");
    println!(
        "OpenXR extension probe: {graphics_extension}=true, ext_debug_utils={}, ext_hand_tracking={}, fb_display_refresh_rate={}",
        available.ext_debug_utils, available.ext_hand_tracking, available.fb_display_refresh_rate
    );

    let instance = entry
        .create_instance(
            &xr::ApplicationInfo {
                application_name: "mclone",
                application_version: 1,
                engine_name: "mclone",
                engine_version: 1,
                api_version: xr::Version::new(1, 0, 0),
            },
            &enabled_extensions,
            &[],
        )
        .context("create OpenXR instance")?;
    let properties = instance
        .properties()
        .context("query OpenXR runtime properties")?;
    println!(
        "OpenXR runtime: {} v{}",
        properties.runtime_name, properties.runtime_version
    );

    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .context("locate OpenXR head-mounted display system")?;
    let system_properties = instance
        .system_properties(system)
        .context("query OpenXR system properties")?;
    println!(
        "OpenXR system: {} vendor={} orientation_tracking={} position_tracking={}",
        system_properties.system_name,
        system_properties.vendor_id,
        system_properties.tracking_properties.orientation_tracking,
        system_properties.tracking_properties.position_tracking
    );

    let blend_modes = instance
        .enumerate_environment_blend_modes(system, VIEW_TYPE)
        .context("enumerate OpenXR environment blend modes")?;
    if blend_modes.is_empty() {
        bail!("OpenXR runtime reported no PRIMARY_STEREO environment blend modes");
    }
    println!("OpenXR blend modes: {}", format_debug_list(&blend_modes));
    let environment_blend_mode = selected_environment_blend_mode(&blend_modes);
    println!("OpenXR environment blend mode: {environment_blend_mode:?}");

    let views = instance
        .enumerate_view_configuration_views(system, VIEW_TYPE)
        .context("enumerate OpenXR PRIMARY_STEREO view configuration")?;
    if views.len() < 2 {
        bail!(
            "OpenXR PRIMARY_STEREO reported {} view(s); mclone requires at least two",
            views.len()
        );
    }
    println!(
        "OpenXR stereo views: {}",
        format_view_configurations(&views)
    );

    create_graphics_session_probe(&instance, system, &views, environment_blend_mode, smoke)?;

    Ok(())
}

#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
fn enable_platform_graphics_extension(
    available: &xr::ExtensionSet,
    enabled: &mut xr::ExtensionSet,
) -> Result<&'static str> {
    if !available.khr_metal_enable {
        bail!("OpenXR runtime does not support XR_KHR_metal_enable");
    }
    enabled.khr_metal_enable = true;
    Ok("XR_KHR_metal_enable")
}

#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
fn enable_platform_graphics_extension(
    available: &xr::ExtensionSet,
    enabled: &mut xr::ExtensionSet,
) -> Result<&'static str> {
    if !available.khr_vulkan_enable2 {
        bail!("OpenXR runtime does not support XR_KHR_vulkan_enable2");
    }
    enabled.khr_vulkan_enable2 = true;
    Ok("XR_KHR_vulkan_enable2")
}

#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
fn create_graphics_session_probe(
    instance: &xr::Instance,
    system: xr::SystemId,
    views: &[xr::ViewConfigurationView],
    environment_blend_mode: xr::EnvironmentBlendMode,
    smoke: DesktopXrSmoke,
) -> Result<()> {
    let graphics = graphics_metal::create_graphics_session(instance, system)
        .context("create OpenXR Metal graphics session")?;
    let stage = graphics
        .session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .context("create OpenXR STAGE reference space")?;
    println!(
        "OpenXR Metal session: runtime_device='{}' matched_adapter='{}'",
        graphics.required_device_name, graphics.adapter_name
    );
    println!("OpenXR reference space: STAGE");
    run_smoke_frames(graphics, stage, views, environment_blend_mode, smoke)?;
    Ok(())
}

#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
fn create_graphics_session_probe(
    instance: &xr::Instance,
    system: xr::SystemId,
    views: &[xr::ViewConfigurationView],
    environment_blend_mode: xr::EnvironmentBlendMode,
    smoke: DesktopXrSmoke,
) -> Result<()> {
    let graphics = graphics_vulkan::create_graphics_session(instance, system)
        .context("create OpenXR Vulkan graphics session")?;
    let stage = graphics
        .session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .context("create OpenXR STAGE reference space")?;
    println!(
        "OpenXR Vulkan session: physical_device='{}' api={} queue_family={}",
        graphics.physical_device_name,
        graphics.physical_device_api_version,
        graphics.queue_family_index
    );
    println!("OpenXR reference space: STAGE");
    run_smoke_frames(graphics, stage, views, environment_blend_mode, smoke)?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
enum OpenXrPollStatus {
    Idle,
    Running,
    Exit,
}

#[cfg(not(target_os = "android"))]
fn selected_environment_blend_mode(
    blend_modes: &[xr::EnvironmentBlendMode],
) -> xr::EnvironmentBlendMode {
    blend_modes
        .iter()
        .copied()
        .find(|mode| *mode == xr::EnvironmentBlendMode::OPAQUE)
        .unwrap_or(blend_modes[0])
}

#[cfg(not(target_os = "android"))]
fn run_smoke_frames(
    mut graphics: platform_graphics::GraphicsSession,
    stage: xr::Space,
    views: &[xr::ViewConfigurationView],
    environment_blend_mode: xr::EnvironmentBlendMode,
    smoke: DesktopXrSmoke,
) -> Result<()> {
    let frames = match &smoke {
        DesktopXrSmoke::Clear { frames } => *frames,
        DesktopXrSmoke::Mclone { options } => options.frames,
    };
    let eye_width = views[0].recommended_image_rect_width.max(1);
    let eye_height = views[0].recommended_image_rect_height.max(1);
    let mut left_eye = platform_graphics::create_eye(
        &graphics.device,
        &graphics.session,
        eye_width,
        eye_height,
        XR_COLOR_FORMAT,
        XR_DEPTH_FORMAT,
        XR_SAMPLE_COUNT,
    )
    .context("create OpenXR left-eye color swapchain")?;
    let mut right_eye = platform_graphics::create_eye(
        &graphics.device,
        &graphics.session,
        eye_width,
        eye_height,
        XR_COLOR_FORMAT,
        XR_DEPTH_FORMAT,
        XR_SAMPLE_COUNT,
    )
    .context("create OpenXR right-eye color swapchain")?;
    println!(
        "OpenXR swapchains: format={XR_COLOR_FORMAT:?} eye={}x{} images={}/{}",
        eye_width,
        eye_height,
        left_eye.textures.len(),
        right_eye.textures.len()
    );
    let mut mclone = match smoke {
        DesktopXrSmoke::Clear { .. } => None,
        DesktopXrSmoke::Mclone { options } => Some(
            XrMcloneWorldState::new(&graphics.device, &graphics.queue, options)
                .context("initialize mclone XR world state")?,
        ),
    };
    let controller_actions =
        OpenXrControllerActions::create(graphics.session.instance(), &graphics.session)
            .context("initialize OpenXR controller actions")?;
    let mut controller_summary = XrControllerInputSummary::default();

    let mut event_storage = xr::EventDataBuffer::new();
    let mut session_running = false;
    let session_ready_deadline = Instant::now() + SESSION_READY_TIMEOUT;
    let mut submitted_frames = 0;
    let mut skipped_frames = 0;
    let mut runtime_frames = 0;

    while submitted_frames < frames {
        match poll_openxr_events(&graphics.session, &mut event_storage, &mut session_running)
            .context("poll OpenXR events")?
        {
            OpenXrPollStatus::Exit => bail!("OpenXR session requested exit before smoke completed"),
            OpenXrPollStatus::Idle if !session_running => {
                if Instant::now() >= session_ready_deadline {
                    bail!("timed out waiting for OpenXR session READY state");
                }
                thread::sleep(SESSION_IDLE_POLL_INTERVAL);
                continue;
            }
            OpenXrPollStatus::Idle | OpenXrPollStatus::Running => {}
        }

        let frame_state = graphics.frame_wait.wait().context("wait OpenXR frame")?;
        graphics
            .frame_stream
            .begin()
            .context("begin OpenXR frame")?;
        runtime_frames += 1;

        let frame_result = if frame_state.should_render {
            let result = controller_actions
                .poll(
                    &graphics.session,
                    &stage,
                    frame_state.predicted_display_time,
                )
                .and_then(|controllers| {
                    controller_summary.record(&controllers);
                    if let Some(mclone) = &mut mclone {
                        render_mclone_frame(
                            &mut graphics,
                            &stage,
                            environment_blend_mode,
                            frame_state.predicted_display_time,
                            &mut left_eye,
                            &mut right_eye,
                            mclone,
                        )
                    } else {
                        render_clear_frame(
                            &mut graphics,
                            &stage,
                            environment_blend_mode,
                            frame_state.predicted_display_time,
                            &mut left_eye,
                            &mut right_eye,
                        )
                    }
                });
            result.map(|()| {
                submitted_frames += 1;
            })
        } else {
            skipped_frames += 1;
            graphics
                .frame_stream
                .end(
                    frame_state.predicted_display_time,
                    environment_blend_mode,
                    &[],
                )
                .context("end skipped OpenXR frame")
        };

        if let Err(err) = frame_result {
            let _ = graphics.frame_stream.end(
                frame_state.predicted_display_time,
                environment_blend_mode,
                &[],
            );
            return Err(err);
        }
    }

    println!(
        "OpenXR frames submitted: submitted={submitted_frames} runtime_frames={runtime_frames} skipped={skipped_frames}"
    );
    controller_summary.print_summary();
    if let Some(mclone) = &mclone {
        mclone.print_summary();
    }
    let completed_label = if mclone.is_some() { "mclone" } else { "clear" };
    shutdown_openxr_session(&mut graphics, &mut event_storage, &mut session_running)
        .context("shut down OpenXR session after smoke")?;
    // Mirrors Playbox's desktop OpenXR shutdown shape. Some runtimes fault while
    // destroying session-owned graphics handles after an app-requested EXITING
    // transition; this smoke is process-bounded, so keep the XR object graph
    // alive after the clean shutdown instead of touching a stopped runtime.
    std::mem::forget(left_eye);
    std::mem::forget(right_eye);
    std::mem::forget(stage);
    std::mem::forget(controller_actions);
    std::mem::forget(graphics);
    if let Some(mclone) = mclone {
        std::mem::forget(mclone);
    }
    println!("desktop OpenXR {completed_label} smoke complete: frames={frames}");
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn shutdown_openxr_session(
    graphics: &mut platform_graphics::GraphicsSession,
    event_storage: &mut xr::EventDataBuffer,
    session_running: &mut bool,
) -> Result<()> {
    if !*session_running {
        return Ok(());
    }

    match graphics.session.request_exit() {
        Ok(()) => {}
        Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => {
            *session_running = false;
            return Ok(());
        }
        Err(err) => {
            println!("OpenXR session exit request failed: {err:?}");
        }
    }

    let deadline = Instant::now() + Duration::from_secs(2);
    while *session_running && Instant::now() < deadline {
        match poll_openxr_events(&graphics.session, event_storage, session_running)
            .context("poll OpenXR events during shutdown")?
        {
            OpenXrPollStatus::Exit => break,
            OpenXrPollStatus::Idle | OpenXrPollStatus::Running => {}
        }
        thread::sleep(SESSION_IDLE_POLL_INTERVAL);
    }

    if *session_running {
        match graphics.session.end() {
            Ok(_) => {
                *session_running = false;
            }
            Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => {
                *session_running = false;
            }
            Err(err) => {
                println!("OpenXR session end during shutdown failed: {err:?}");
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "android"))]
fn poll_openxr_events(
    session: &xr::Session<platform_graphics::AppGraphics>,
    event_storage: &mut xr::EventDataBuffer,
    session_running: &mut bool,
) -> Result<OpenXrPollStatus> {
    while let Some(event) = session
        .instance()
        .poll_event(event_storage)
        .context("poll OpenXR event")?
    {
        match event {
            xr::Event::SessionStateChanged(event) => {
                let state = event.state();
                println!("OpenXR session state: {state:?}");
                match state {
                    xr::SessionState::READY if !*session_running => {
                        session.begin(VIEW_TYPE).context("begin OpenXR session")?;
                        *session_running = true;
                    }
                    xr::SessionState::STOPPING if *session_running => {
                        session.end().context("end OpenXR session")?;
                        *session_running = false;
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        return Ok(OpenXrPollStatus::Exit);
                    }
                    _ => {}
                }
            }
            xr::Event::InstanceLossPending(_) => return Ok(OpenXrPollStatus::Exit),
            xr::Event::EventsLost(event) => {
                println!("OpenXR events lost: {}", event.lost_event_count());
            }
            _ => {}
        }
    }

    if *session_running {
        Ok(OpenXrPollStatus::Running)
    } else {
        Ok(OpenXrPollStatus::Idle)
    }
}

#[cfg(not(target_os = "android"))]
struct XrMcloneWorldState {
    runtime: WindowSceneRuntime,
    base_camera: ChunkCamera,
    view_pose: Option<XrViewPose>,
    render_options: TexturedSectionRenderOptions,
    draw: TexturedSectionDrawResources,
    sky: SkyRenderer,
    actors: ActorDrawResources,
    render_stats: RenderStreamStats,
    stage_to_world: Option<XrStageToWorld>,
    first_eye_summary: Option<FullFrameRenderSummary>,
    rendered_frames: u32,
}

#[cfg(not(target_os = "android"))]
impl XrMcloneWorldState {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        options: XrMcloneSmokeOptions,
    ) -> Result<Self> {
        let mut runtime = WindowSceneRuntime::new(&options.scene)?;
        let initial_poll_start = Instant::now();
        let (initial_poll_count, initial_poll_ms) = poll_window_runtime_until_idle(&mut runtime)
            .context("wait for initial mclone runtime chunks")?;
        let base_camera = ChunkCamera::overview_for_chunk_area(
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.render_distance,
        );
        let base_camera_position = Vec3::from_array(base_camera.eye);
        let section_update = runtime
            .sync_all_render_sections(base_camera_position)
            .context("compile initial mclone render sections for XR")?;
        let sections = runtime.cached_sections();
        if sections.is_empty() {
            bail!(
                "mclone XR smoke seed={} center=({}, {}) render_distance={} produced no render sections",
                options.scene.seed,
                options.scene.chunk_x,
                options.scene.chunk_z,
                options.scene.render_distance
            );
        }
        let mut draw = TexturedSectionDrawResources::new(
            device,
            queue,
            XR_COLOR_FORMAT,
            &sections,
            runtime.mesh_assets.atlas.as_upload(),
        )
        .context("upload initial mclone render sections for XR")?;
        draw.set_traversal_ready_sections(
            &runtime.traversal_ready_render_section_keys(base_camera_position),
        );
        let sky = SkyRenderer::new(device, XR_COLOR_FORMAT);
        let actors = ActorDrawResources::new(
            device,
            queue,
            XR_COLOR_FORMAT,
            runtime.actor_textures.atlas.as_upload(),
        )
        .context("initialize mclone actor resources for XR")?;
        let initial_upload = TexturedSectionUploadReport {
            uploaded_section_count: section_update.rebuilt_section_count(),
            removed_section_count: section_update.removed_section_count(),
            uploaded_vertex_count: section_update.rebuilt_vertex_count,
            uploaded_index_count: section_update.rebuilt_index_count,
        };
        let mut render_stats = RenderStreamStats {
            section_count: draw.section_count(),
            index_count: draw.index_count(),
            face_count: quad_face_count_from_indices(draw.index_count()),
            ..RenderStreamStats::default()
        };
        record_render_section_update_stats(&mut render_stats, &section_update, initial_upload);
        println!(
            "mclone XR runtime: seed={} center=({}, {}) render_distance={} chunks={} sections={} faces={} indices={} initial_polls={} poll_ms={:.3} elapsed_ms={:.3}",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.render_distance,
            runtime.client().loaded_chunk_count(),
            render_stats.section_count,
            render_stats.face_count,
            render_stats.index_count,
            initial_poll_count,
            initial_poll_ms,
            elapsed_ms(initial_poll_start.elapsed())
        );
        Ok(Self {
            runtime,
            base_camera,
            view_pose: options.view_pose,
            render_options: options.render_options,
            draw,
            sky,
            actors,
            render_stats,
            stage_to_world: None,
            first_eye_summary: None,
            rendered_frames: 0,
        })
    }

    fn render_views(&mut self, views: &[xr::View]) -> Result<[ChunkRenderView; 2]> {
        if views.len() < 2 {
            bail!("OpenXR runtime returned fewer than two stereo views");
        }
        let transform = match self.stage_to_world {
            Some(transform) => transform,
            None => {
                let transform =
                    XrStageToWorld::from_initial_views(views, self.base_camera, self.view_pose)?;
                println!(
                    "mclone XR view alignment: mode={} world_eye=({:.2}, {:.2}, {:.2}) world_yaw_degrees={:.1} stage_center=({:.3}, {:.3}, {:.3})",
                    transform.mode_label(),
                    transform.origin_world.x,
                    transform.origin_world.y,
                    transform.origin_world.z,
                    transform.world_yaw_degrees,
                    transform.origin_stage.x,
                    transform.origin_stage.y,
                    transform.origin_stage.z
                );
                self.stage_to_world = Some(transform);
                transform
            }
        };
        Ok([
            xr_view_to_chunk_render_view(&views[0], transform, XR_NEAR, XR_FAR)?,
            xr_view_to_chunk_render_view(&views[1], transform, XR_NEAR, XR_FAR)?,
        ])
    }

    fn poll_runtime_and_upload(
        &mut self,
        device: &wgpu::Device,
        camera_position: Vec3,
    ) -> Result<()> {
        let changed = self.runtime.poll().context("poll mclone XR runtime")?;
        if !changed && !self.runtime.has_pending_render_work(camera_position) {
            self.draw.set_traversal_ready_sections(
                &self
                    .runtime
                    .traversal_ready_render_section_keys(camera_position),
            );
            return Ok(());
        }
        let section_update = self
            .runtime
            .sync_render_sections(camera_position)
            .context("sync mclone XR render sections")?;
        let upload_report = self
            .draw
            .apply_section_updates(
                device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("upload mclone XR render section updates")?;
        self.draw.set_traversal_ready_sections(
            &self
                .runtime
                .traversal_ready_render_section_keys(camera_position),
        );
        self.render_stats.section_count = self.draw.section_count();
        self.render_stats.index_count = self.draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        record_render_section_update_stats(&mut self.render_stats, &section_update, upload_report);
        Ok(())
    }

    fn actor_instances(&self) -> Vec<ActorInstance> {
        actor_instances_from_presentations(
            &self.runtime.client().actor_presentations(),
            self.runtime.client(),
        )
    }

    fn effective_render_options(&self, camera_position: Vec3) -> TexturedSectionRenderOptions {
        let mut options = self.render_options;
        if self.runtime.camera_inside_occluding_block(camera_position) {
            options.section_occlusion_culling = false;
        }
        options
    }

    fn record_eye0_summary(&mut self, summary: FullFrameRenderSummary) {
        self.first_eye_summary.get_or_insert(summary);
        self.rendered_frames += 1;
    }

    fn print_summary(&self) {
        if let Some(summary) = self.first_eye_summary {
            println!(
                "mclone XR frame summary: frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                self.rendered_frames,
                summary.section_count,
                summary.drawn_section_count,
                summary.index_count,
                summary.drawn_index_count,
                summary.actor_count,
                summary.drawn_actor_count
            );
        }
    }
}

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug)]
struct XrStageToWorld {
    origin_stage: Vec3,
    origin_world: Vec3,
    stage_to_world_rotation: Quat,
    world_yaw_degrees: f32,
    mode: XrViewAlignmentMode,
}

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XrViewAlignmentMode {
    Overview,
    ViewPose,
}

#[cfg(not(target_os = "android"))]
impl XrStageToWorld {
    fn from_initial_views(
        views: &[xr::View],
        base_camera: ChunkCamera,
        view_pose: Option<XrViewPose>,
    ) -> Result<Self> {
        let (left_stage_position, left_stage_orientation) = eye_pose(&views[0])?;
        let (right_stage_position, _) = eye_pose(&views[1])?;
        let origin_stage = (left_stage_position + right_stage_position) * 0.5;
        if let Some(view_pose) = view_pose {
            return Self::from_view_pose(origin_stage, left_stage_orientation, view_pose);
        }
        let origin_world = Vec3::from_array(base_camera.eye);
        let desired_world_orientation = chunk_camera_orientation(base_camera)?;
        let world_yaw_degrees = chunk_camera_yaw_degrees(base_camera)?;
        Ok(Self {
            origin_stage,
            origin_world,
            stage_to_world_rotation: (desired_world_orientation * left_stage_orientation.inverse())
                .normalize(),
            world_yaw_degrees,
            mode: XrViewAlignmentMode::Overview,
        })
    }

    fn from_view_pose(
        origin_stage: Vec3,
        left_stage_orientation: Quat,
        view_pose: XrViewPose,
    ) -> Result<Self> {
        let stage_forward = left_stage_orientation * Vec3::NEG_Z;
        let stage_yaw = yaw_from_forward(stage_forward)
            .ok_or_else(|| anyhow!("OpenXR returned an invalid startup view yaw"))?;
        let world_yaw = view_pose.yaw_degrees.to_radians();
        if !world_yaw.is_finite() {
            bail!("invalid XR startup view yaw {}", view_pose.yaw_degrees);
        }
        Ok(Self {
            origin_stage,
            origin_world: Vec3::from_array(view_pose.position),
            stage_to_world_rotation: Quat::from_rotation_y(normalize_angle(world_yaw - stage_yaw)),
            world_yaw_degrees: view_pose.yaw_degrees,
            mode: XrViewAlignmentMode::ViewPose,
        })
    }

    fn mode_label(self) -> &'static str {
        match self.mode {
            XrViewAlignmentMode::Overview => "overview",
            XrViewAlignmentMode::ViewPose => "view-pose",
        }
    }

    fn transform_pose(self, stage_position: Vec3, stage_orientation: Quat) -> (Vec3, Quat) {
        (
            self.origin_world
                + self
                    .stage_to_world_rotation
                    .mul_vec3(stage_position - self.origin_stage),
            (self.stage_to_world_rotation * stage_orientation).normalize(),
        )
    }
}

#[cfg(not(target_os = "android"))]
fn render_mclone_frame(
    graphics: &mut platform_graphics::GraphicsSession,
    stage: &xr::Space,
    environment_blend_mode: xr::EnvironmentBlendMode,
    predicted_display_time: xr::Time,
    left_eye: &mut platform_graphics::OpenXrEyeState,
    right_eye: &mut platform_graphics::OpenXrEyeState,
    mclone: &mut XrMcloneWorldState,
) -> Result<()> {
    let (_, views) = graphics
        .session
        .locate_views(VIEW_TYPE, predicted_display_time, stage)
        .context("locate OpenXR stereo views for mclone frame")?;
    let render_views = mclone.render_views(&views)?;
    let center_position = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    mclone.poll_runtime_and_upload(&graphics.device, center_position)?;
    let render_options = mclone.effective_render_options(center_position);
    let sky_clear_color = mclone.runtime.sky_clear_color();
    let time_of_day = mclone.runtime.time_of_day();
    let sun_angle = mclone.runtime.sun_angle();
    let actor_instances = mclone.actor_instances();

    let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
    let right_target = match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image")
    {
        Ok(target) => target,
        Err(err) => {
            let _ = left_target.release();
            return Err(err);
        }
    };

    let render_result = render_mclone_eye_targets(
        graphics,
        &left_target,
        &right_target,
        mclone,
        render_views,
        &actor_instances,
        render_options,
        sky_clear_color,
        time_of_day,
        sun_angle,
    );
    let left_release_result = left_target.release();
    let right_release_result = right_target.release();
    let eye0_summary = render_result?;
    left_release_result?;
    right_release_result?;
    mclone.record_eye0_summary(eye0_summary);

    let rect = xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: left_eye.width as _,
            height: left_eye.height as _,
        },
    };
    let projection_views = [
        xr::CompositionLayerProjectionView::new()
            .pose(views[0].pose)
            .fov(views[0].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&left_eye.swapchain)
                    .image_array_index(0)
                    .image_rect(rect),
            ),
        xr::CompositionLayerProjectionView::new()
            .pose(views[1].pose)
            .fov(views[1].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&right_eye.swapchain)
                    .image_array_index(0)
                    .image_rect(rect),
            ),
    ];
    let projection = xr::CompositionLayerProjection::new()
        .space(stage)
        .views(&projection_views);
    let mut layers: Vec<&xr::CompositionLayerBase<'_, platform_graphics::AppGraphics>> =
        Vec::with_capacity(1);
    layers.push(&projection);
    graphics
        .frame_stream
        .end(predicted_display_time, environment_blend_mode, &layers)
        .context("end OpenXR frame with mclone projection layer")
}

#[cfg(not(target_os = "android"))]
#[allow(clippy::too_many_arguments)]
fn render_mclone_eye_targets(
    graphics: &mut platform_graphics::GraphicsSession,
    left_target: &AcquiredEyeTarget<'_>,
    right_target: &AcquiredEyeTarget<'_>,
    mclone: &mut XrMcloneWorldState,
    render_views: [ChunkRenderView; 2],
    actor_instances: &[ActorInstance],
    render_options: TexturedSectionRenderOptions,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
) -> Result<FullFrameRenderSummary> {
    let mut encoder = graphics
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_xr_world_encoder"),
        });
    let mut left_stats = mclone.render_stats;
    let left_summary = {
        let frame = RenderFrameContext::new(
            &graphics.device,
            &graphics.queue,
            &mut encoder,
            RenderFrameTarget::color(
                &left_target.color_view,
                [left_target.eye_state.width, left_target.eye_state.height],
            ),
        );
        render_full_frame_for_view(
            frame,
            &left_target.eye_state.depth,
            &mclone.sky,
            &mut mclone.draw,
            Some(&mut mclone.actors),
            None,
            None,
            render_views[0],
            actor_instances,
            None,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            FullFrameGui::new(false, false, [1.0, 1.0]),
            |_| GuiDrawList::new(),
            &mut left_stats,
        )
        .context("render mclone left eye")?
    };
    let mut right_stats = left_stats;
    {
        let frame = RenderFrameContext::new(
            &graphics.device,
            &graphics.queue,
            &mut encoder,
            RenderFrameTarget::color(
                &right_target.color_view,
                [right_target.eye_state.width, right_target.eye_state.height],
            ),
        );
        render_full_frame_for_view(
            frame,
            &right_target.eye_state.depth,
            &mclone.sky,
            &mut mclone.draw,
            Some(&mut mclone.actors),
            None,
            None,
            render_views[1],
            actor_instances,
            None,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            FullFrameGui::new(false, false, [1.0, 1.0]),
            |_| GuiDrawList::new(),
            &mut right_stats,
        )
        .context("render mclone right eye")?;
    }
    let submission = graphics.queue.submit(Some(encoder.finish()));
    graphics
        .device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .context("wait for OpenXR mclone render submission")?;
    graphics
        .device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .context("wait for OpenXR mclone render device idle")?;
    mclone.render_stats = left_stats;
    Ok(left_summary)
}

#[cfg(not(target_os = "android"))]
fn xr_view_to_chunk_render_view(
    view: &xr::View,
    transform: XrStageToWorld,
    near: f32,
    far: f32,
) -> Result<ChunkRenderView> {
    let (stage_position, stage_orientation) = eye_pose(view)?;
    let (camera_position, camera_orientation) =
        transform.transform_pose(stage_position, stage_orientation);
    let camera_forward = (camera_orientation * Vec3::NEG_Z).normalize_or_zero();
    let camera_right = (camera_orientation * Vec3::X).normalize_or_zero();
    let camera_up = (camera_orientation * Vec3::Y).normalize_or_zero();
    if camera_forward.length_squared() < 1.0e-6 || camera_up.length_squared() < 1.0e-6 {
        bail!("OpenXR returned an invalid view orientation");
    }
    let view_matrix =
        Mat4::from_rotation_translation(camera_orientation, camera_position).inverse();
    let projection = xr_fov_to_projection_rh(view.fov, near, far)?;
    Ok(ChunkRenderView {
        view: view_matrix,
        projection,
        view_projection: projection * view_matrix,
        camera_position,
        camera_forward,
        camera_right,
        camera_up,
        aspect: xr_fov_aspect(view.fov),
        fov_y_radians: (view.fov.angle_up - view.fov.angle_down).abs(),
        z_near: near,
        z_far: far,
    })
}

#[cfg(not(target_os = "android"))]
fn xr_fov_to_projection_rh(fov: xr::Fovf, near: f32, far: f32) -> Result<Mat4> {
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
        bail!("invalid XR projection clipping planes near={near} far={far}");
    }
    let tan_left = fov.angle_left.tan();
    let tan_right = fov.angle_right.tan();
    let tan_up = fov.angle_up.tan();
    let tan_down = fov.angle_down.tan();
    let tan_width = tan_right - tan_left;
    let tan_height = tan_up - tan_down;
    if !tan_width.is_finite()
        || !tan_height.is_finite()
        || tan_width.abs() <= f32::EPSILON
        || tan_height.abs() <= f32::EPSILON
    {
        bail!("invalid OpenXR FOV {:?}", fov);
    }
    Ok(Mat4::from_cols(
        Vec4::new(2.0 / tan_width, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / tan_height, 0.0, 0.0),
        Vec4::new(
            (tan_right + tan_left) / tan_width,
            (tan_up + tan_down) / tan_height,
            -far / (far - near),
            -1.0,
        ),
        Vec4::new(0.0, 0.0, -(near * far) / (far - near), 0.0),
    ))
}

#[cfg(not(target_os = "android"))]
fn xr_fov_aspect(fov: xr::Fovf) -> f32 {
    let width = fov.angle_right.tan() - fov.angle_left.tan();
    let height = fov.angle_up.tan() - fov.angle_down.tan();
    if width.is_finite() && height.is_finite() && height.abs() > f32::EPSILON {
        (width / height).abs().max(0.01)
    } else {
        1.0
    }
}

#[cfg(not(target_os = "android"))]
fn eye_pose(view: &xr::View) -> Result<(Vec3, Quat)> {
    let position = Vec3::new(
        view.pose.position.x,
        view.pose.position.y,
        view.pose.position.z,
    );
    let orientation = Quat::from_xyzw(
        view.pose.orientation.x,
        view.pose.orientation.y,
        view.pose.orientation.z,
        view.pose.orientation.w,
    );
    if !position.is_finite() || !finite_quat(orientation) || orientation.length_squared() < 0.5 {
        bail!("OpenXR returned an invalid view pose");
    }
    Ok((position, orientation.normalize()))
}

#[cfg(not(target_os = "android"))]
fn chunk_camera_orientation(camera: ChunkCamera) -> Result<Quat> {
    let eye = Vec3::from_array(camera.eye);
    let target = Vec3::from_array(camera.target);
    let world_up = Vec3::from_array(camera.up).normalize_or_zero();
    let forward = (target - eye).normalize_or_zero();
    let right = forward.cross(world_up).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    if forward.length_squared() < 1.0e-6
        || right.length_squared() < 1.0e-6
        || up.length_squared() < 1.0e-6
    {
        bail!("invalid base mclone XR camera orientation");
    }
    Ok(Quat::from_mat3(&Mat3::from_cols(right, up, -forward)).normalize())
}

#[cfg(not(target_os = "android"))]
fn chunk_camera_yaw_degrees(camera: ChunkCamera) -> Result<f32> {
    let eye = Vec3::from_array(camera.eye);
    let target = Vec3::from_array(camera.target);
    let yaw = yaw_from_forward(target - eye)
        .ok_or_else(|| anyhow!("invalid base mclone XR camera yaw"))?;
    Ok(yaw.to_degrees())
}

#[cfg(not(target_os = "android"))]
fn yaw_from_forward(forward: Vec3) -> Option<f32> {
    if !forward.is_finite() {
        return None;
    }
    let horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() <= f32::EPSILON {
        return None;
    }
    Some((-horizontal.x).atan2(-horizontal.z))
}

#[cfg(not(target_os = "android"))]
fn normalize_angle(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[cfg(not(target_os = "android"))]
fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}

#[cfg(not(target_os = "android"))]
fn render_clear_frame(
    graphics: &mut platform_graphics::GraphicsSession,
    stage: &xr::Space,
    environment_blend_mode: xr::EnvironmentBlendMode,
    predicted_display_time: xr::Time,
    left_eye: &mut platform_graphics::OpenXrEyeState,
    right_eye: &mut platform_graphics::OpenXrEyeState,
) -> Result<()> {
    let (_, views) = graphics
        .session
        .locate_views(VIEW_TYPE, predicted_display_time, stage)
        .context("locate OpenXR stereo views")?;
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }

    let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
    let right_target = match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image")
    {
        Ok(target) => target,
        Err(err) => {
            let _ = left_target.release();
            return Err(err);
        }
    };

    let clear_result = clear_stereo_targets(
        &graphics.device,
        &graphics.queue,
        &left_target,
        &right_target,
    );
    let left_release_result = left_target.release();
    let right_release_result = right_target.release();
    clear_result?;
    left_release_result?;
    right_release_result?;

    let rect = xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: left_eye.width as _,
            height: left_eye.height as _,
        },
    };
    let projection_views = [
        xr::CompositionLayerProjectionView::new()
            .pose(views[0].pose)
            .fov(views[0].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&left_eye.swapchain)
                    .image_array_index(0)
                    .image_rect(rect),
            ),
        xr::CompositionLayerProjectionView::new()
            .pose(views[1].pose)
            .fov(views[1].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&right_eye.swapchain)
                    .image_array_index(0)
                    .image_rect(rect),
            ),
    ];
    let projection = xr::CompositionLayerProjection::new()
        .space(stage)
        .views(&projection_views);
    let mut layers: Vec<&xr::CompositionLayerBase<'_, platform_graphics::AppGraphics>> =
        Vec::with_capacity(1);
    layers.push(&projection);
    graphics
        .frame_stream
        .end(predicted_display_time, environment_blend_mode, &layers)
        .context("end OpenXR frame with projection layer")
}

#[cfg(not(target_os = "android"))]
struct AcquiredEyeTarget<'a> {
    eye_state: &'a mut platform_graphics::OpenXrEyeState,
    color_view: wgpu::TextureView,
}

#[cfg(not(target_os = "android"))]
impl<'a> AcquiredEyeTarget<'a> {
    fn release(self) -> Result<()> {
        self.eye_state
            .swapchain
            .release_image()
            .context("release OpenXR swapchain image")
    }
}

#[cfg(not(target_os = "android"))]
fn acquire_eye_target(
    eye_state: &mut platform_graphics::OpenXrEyeState,
) -> Result<AcquiredEyeTarget<'_>> {
    let image_index = eye_state
        .swapchain
        .acquire_image()
        .context("acquire OpenXR swapchain image")?;
    if let Err(err) = eye_state.swapchain.wait_image(xr::Duration::INFINITE) {
        let _ = eye_state.swapchain.release_image();
        return Err(err).context("wait for OpenXR swapchain image");
    }
    let color_view = {
        let texture = eye_state
            .textures
            .get(image_index as usize)
            .ok_or_else(|| anyhow!("OpenXR returned out-of-range image index {image_index}"))?;
        texture.create_view(&Default::default())
    };
    Ok(AcquiredEyeTarget {
        eye_state,
        color_view,
    })
}

#[cfg(not(target_os = "android"))]
fn clear_stereo_targets(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    left_target: &AcquiredEyeTarget<'_>,
    right_target: &AcquiredEyeTarget<'_>,
) -> Result<()> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_xr_clear_encoder"),
    });
    clear_eye_target(&mut encoder, left_target, diagnostic_eye_clear_color(0));
    clear_eye_target(&mut encoder, right_target, diagnostic_eye_clear_color(1));
    let submission = queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .context("wait for OpenXR clear submission")?;
    device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .context("wait for OpenXR clear device idle")
}

#[cfg(not(target_os = "android"))]
fn clear_eye_target(
    encoder: &mut wgpu::CommandEncoder,
    target: &AcquiredEyeTarget<'_>,
    color: wgpu::Color,
) {
    let color_attachments = [Some(wgpu::RenderPassColorAttachment {
        view: &target.color_view,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(color),
            store: wgpu::StoreOp::Store,
        },
    })];
    let depth_stencil_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
        view: &target.eye_state.depth.view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    });
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mclone_xr_clear_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
}

#[cfg(not(target_os = "android"))]
fn diagnostic_eye_clear_color(eye: usize) -> wgpu::Color {
    match eye {
        0 => wgpu::Color {
            r: 0.04,
            g: 0.10,
            b: 0.35,
            a: 1.0,
        },
        _ => wgpu::Color {
            r: 0.05,
            g: 0.28,
            b: 0.12,
            a: 1.0,
        },
    }
}

#[cfg(not(target_os = "android"))]
fn load_openxr_entry() -> Result<(xr::Entry, String)> {
    unsafe {
        match xr::Entry::load() {
            Ok(entry) => Ok((entry, "registered OpenXR loader".to_owned())),
            Err(loader_error) => {
                let mut attempted = Vec::new();
                let mut errors = Vec::new();
                for path in openxr_entry_fallback_candidates() {
                    if !path.exists() {
                        continue;
                    }
                    let label = path.display().to_string();
                    attempted.push(label.clone());
                    match load_runtime_entry_from_negotiation(&path) {
                        Ok(entry) => {
                            return Ok((
                                entry,
                                format!(
                                    "runtime negotiation through fallback path {}",
                                    path.display()
                                ),
                            ));
                        }
                        Err(err) => {
                            errors.push(format!("{} negotiation: {err:#}", path.display()));
                        }
                    }
                    match xr::Entry::load_from(&path) {
                        Ok(entry) => {
                            return Ok((entry, format!("fallback loader {}", path.display())));
                        }
                        Err(err) => {
                            errors.push(format!("{} load_from: {err}", path.display()));
                        }
                    }
                }

                if attempted.is_empty() {
                    bail!(
                        "failed to load OpenXR loader ({loader_error}); no fallback paths were found; set XR_RUNTIME_JSON or MONADO_OPENXR_RUNTIME_PATH if a runtime is installed outside the system loader registry"
                    );
                }
                bail!(
                    "failed to load OpenXR loader ({loader_error}); attempted fallbacks: {}; errors: {}",
                    attempted.join(", "),
                    errors.join(" | ")
                )
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn openxr_entry_fallback_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(runtime_path) = std::env::var_os("MONADO_OPENXR_RUNTIME_PATH") {
        candidates.push(PathBuf::from(runtime_path));
    }
    if let Some(manifest_path) = std::env::var_os("XR_RUNTIME_JSON") {
        let manifest_path = PathBuf::from(manifest_path);
        if let Ok(manifest) = runtime_manifest_from_path(&manifest_path) {
            candidates.push(manifest.runtime_library_path);
        }
        candidates.push(manifest_path);
    }

    #[cfg(target_os = "windows")]
    {
        for manifest_path in [
            PathBuf::from(
                r"C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json",
            ),
            PathBuf::from(
                r"C:\Program Files\Meta Horizon\Support\oculus-runtime\oculus_openxr_64.json",
            ),
        ] {
            if let Ok(manifest) = runtime_manifest_from_path(&manifest_path) {
                candidates.push(manifest.runtime_library_path);
            }
            candidates.push(manifest_path);
        }
        candidates.extend([
            PathBuf::from(
                r"C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64\openxr_loader.dll",
            ),
            PathBuf::from(
                r"C:\Program Files\Steam\steamapps\common\SteamVR\bin\win64\openxr_loader.dll",
            ),
        ]);
    }

    candidates
}

#[cfg(not(target_os = "android"))]
fn runtime_manifest_from_path(manifest_path: &Path) -> Result<OpenXrRuntimeManifest> {
    let manifest_text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("read OpenXR runtime manifest {}", manifest_path.display()))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .with_context(|| format!("parse OpenXR runtime manifest {}", manifest_path.display()))?;
    let library_path = manifest
        .get("runtime")
        .and_then(|runtime| runtime.get("library_path"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            anyhow!(
                "manifest {} has no runtime.library_path",
                manifest_path.display()
            )
        })?;
    let library_path = PathBuf::from(library_path);
    let runtime_library_path = if library_path.is_absolute() {
        library_path
    } else {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(library_path)
    };
    Ok(OpenXrRuntimeManifest {
        runtime_library_path,
    })
}

#[cfg(not(target_os = "android"))]
unsafe fn load_runtime_entry_from_negotiation(path: &Path) -> Result<xr::Entry> {
    type NegotiateLoaderRuntimeInterface = unsafe extern "system" fn(
        *const xr::sys::NegotiateLoaderInfo,
        *mut xr::sys::NegotiateRuntimeRequest,
    ) -> xr::sys::Result;

    let library = Box::leak(Box::new(unsafe { libloading::Library::new(path) }?));
    let negotiate: libloading::Symbol<'_, NegotiateLoaderRuntimeInterface> = unsafe {
        library
            .get(b"xrNegotiateLoaderRuntimeInterface\0")
            .context("runtime does not export xrNegotiateLoaderRuntimeInterface")?
    };
    let loader_info = xr::sys::NegotiateLoaderInfo {
        struct_type: xr::sys::LoaderInterfaceStructs::LOADER_INFO,
        struct_version: xr::sys::LOADER_INFO_STRUCT_VERSION as u32,
        struct_size: std::mem::size_of::<xr::sys::NegotiateLoaderInfo>(),
        min_interface_version: xr::sys::CURRENT_LOADER_RUNTIME_VERSION as u32,
        max_interface_version: xr::sys::CURRENT_LOADER_RUNTIME_VERSION as u32,
        min_api_version: xr::CURRENT_API_VERSION,
        max_api_version: xr::CURRENT_API_VERSION,
    };
    let mut runtime_request = xr::sys::NegotiateRuntimeRequest {
        struct_type: xr::sys::LoaderInterfaceStructs::RUNTIME_REQUEST,
        struct_version: xr::sys::RUNTIME_INFO_STRUCT_VERSION as u32,
        struct_size: std::mem::size_of::<xr::sys::NegotiateRuntimeRequest>(),
        runtime_interface_version: 0,
        runtime_api_version: xr::Version::new(0, 0, 0),
        get_instance_proc_addr: None,
    };

    let result = unsafe { negotiate(&loader_info, &mut runtime_request) };
    if result != xr::sys::Result::SUCCESS {
        bail!("xrNegotiateLoaderRuntimeInterface failed with {result:?}");
    }
    let get_instance_proc_addr = runtime_request
        .get_instance_proc_addr
        .ok_or_else(|| anyhow!("runtime negotiation returned null xrGetInstanceProcAddr"))?;
    unsafe { xr::Entry::from_get_instance_proc_addr(get_instance_proc_addr) }
        .context("create OpenXR entry from negotiated xrGetInstanceProcAddr")
}

#[cfg(not(target_os = "android"))]
fn format_debug_list<T: std::fmt::Debug>(values: &[T]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(not(target_os = "android"))]
fn format_view_configurations(views: &[xr::ViewConfigurationView]) -> String {
    views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            format!(
                "#{index} recommended={}x{} max={}x{} samples={}/{}",
                view.recommended_image_rect_width,
                view.recommended_image_rect_height,
                view.max_image_rect_width,
                view.max_image_rect_height,
                view.recommended_swapchain_sample_count,
                view.max_swapchain_sample_count
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;

    fn assert_mat4_close(actual: Mat4, expected: Mat4) {
        for (actual, expected) in actual
            .to_cols_array()
            .into_iter()
            .zip(expected.to_cols_array())
        {
            assert!(
                (actual - expected).abs() < 1.0e-5,
                "matrix mismatch: actual={actual} expected={expected}"
            );
        }
    }

    #[test]
    fn symmetric_xr_fov_matches_chunk_projection_convention() {
        let fov_y = 58.0_f32.to_radians();
        let aspect = 1.25;
        let near = 0.05;
        let far = 700.0;
        let tan_y = (fov_y * 0.5).tan();
        let tan_x = tan_y * aspect;
        let fov = xr::Fovf {
            angle_left: -tan_x.atan(),
            angle_right: tan_x.atan(),
            angle_up: tan_y.atan(),
            angle_down: -tan_y.atan(),
        };

        let actual = xr_fov_to_projection_rh(fov, near, far).unwrap();
        let expected = Mat4::perspective_rh(fov_y, aspect, near, far);

        assert_mat4_close(actual, expected);
    }

    #[test]
    fn startup_view_pose_maps_stage_center_to_requested_world_pose() {
        let stage_center = Vec3::new(1.0, 1.6, -0.25);
        let stage_orientation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let view_pose = XrViewPose {
            position: [8.0, 72.0, -12.0],
            yaw_degrees: 0.0,
        };

        let transform =
            XrStageToWorld::from_view_pose(stage_center, stage_orientation, view_pose).unwrap();
        let (world_position, world_orientation) =
            transform.transform_pose(stage_center, stage_orientation);
        let world_forward = world_orientation * Vec3::NEG_Z;

        assert!((world_position - Vec3::from_array(view_pose.position)).length() < 1.0e-5);
        assert!((world_forward - Vec3::NEG_Z).length() < 1.0e-5);
        assert_eq!(transform.mode_label(), "view-pose");
    }
}
