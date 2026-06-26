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
use glam::{Quat, Vec3};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::frame_render::{
    FullFrameGui, FullFrameRenderSummary, RenderStreamStats, record_render_section_update_stats,
    render_full_frame_for_view,
};
#[cfg(not(target_os = "android"))]
use mclone_core::{ChunkPos, Vec3d};
#[cfg(not(target_os = "android"))]
use mclone_mesh::quad_face_count_from_indices;
#[cfg(not(target_os = "android"))]
use mclone_render::chunk::{
    ChunkRenderView, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
#[cfg(not(target_os = "android"))]
use mclone_render::entity::{ActorDrawResources, ActorInstance};
#[cfg(not(target_os = "android"))]
use mclone_render::sky_render::SkyRenderer;
#[cfg(not(target_os = "android"))]
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
#[cfg(all(test, not(target_os = "android")))]
use mclone_render_session::ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND;
#[cfg(not(target_os = "android"))]
use mclone_render_session::{EngineCameraController, EngineCameraSnapshot};
#[cfg(not(target_os = "android"))]
use mclone_ui::GuiDrawList;
#[cfg(not(target_os = "android"))]
use mclone_xr_host::{
    OpenXrControllerActions, OpenXrHostEvent, OpenXrPollStatus, PRIMARY_STEREO_VIEW_TYPE,
    XrControllerSnapshot, XrFrameStats, XrHand, XrStereoConfig,
};
#[cfg(not(target_os = "android"))]
use mclone_xr_scene::{XR_FAR, XR_NEAR, XrViewAlignmentMode, xr_locomotion_input_from_controllers};
#[cfg(not(target_os = "android"))]
use openxr as xr;

#[cfg(not(target_os = "android"))]
use crate::app::actor_instances_from_presentations;
use crate::cli::{XrClearSmokeOptions, XrMcloneSmokeOptions, XrViewPose};
#[cfg(not(target_os = "android"))]
use crate::frame_pacing::elapsed_ms;
#[cfg(not(target_os = "android"))]
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};

#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
mod graphics_metal;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
mod graphics_vulkan;
#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
use graphics_metal as platform_graphics;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
use graphics_vulkan as platform_graphics;

#[cfg(not(target_os = "android"))]
type AcquiredEyeTarget<'a> = mclone_xr_host::XrAcquiredEyeTarget<
    'a,
    platform_graphics::AppGraphics,
    platform_graphics::OpenXrEyeState,
>;

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
const VIEW_TYPE: xr::ViewConfigurationType = PRIMARY_STEREO_VIEW_TYPE;
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
#[derive(Debug)]
struct OpenXrRuntimeManifest {
    runtime_library_path: PathBuf,
}

#[cfg(not(target_os = "android"))]
enum DesktopXrSmoke {
    Clear { frame_limit: Option<u32> },
    Mclone { options: XrMcloneSmokeOptions },
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
    run_desktop_xr_smoke(DesktopXrSmoke::Clear {
        frame_limit: options.frame_limit,
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
    println!(
        "OpenXR blend modes: {}",
        mclone_xr_host::format_debug_list(&blend_modes)
    );
    let environment_blend_mode = mclone_xr_host::selected_environment_blend_mode(&blend_modes);
    println!("OpenXR environment blend mode: {environment_blend_mode:?}");

    let views = instance
        .enumerate_view_configuration_views(system, VIEW_TYPE)
        .context("enumerate OpenXR PRIMARY_STEREO view configuration")?;
    let stereo_config = mclone_xr_host::stereo_config(&views)?;
    println!(
        "OpenXR stereo views: {}",
        mclone_xr_host::format_view_configurations(&views)
    );

    create_graphics_session_probe(
        &instance,
        system,
        stereo_config,
        environment_blend_mode,
        smoke,
    )?;

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
    stereo_config: XrStereoConfig,
    environment_blend_mode: xr::EnvironmentBlendMode,
    smoke: DesktopXrSmoke,
) -> Result<()> {
    let graphics = graphics_metal::create_graphics_session(instance, system)
        .context("create OpenXR Metal graphics session")?;
    let stage = mclone_xr_host::create_stage_reference_space(&graphics.session)?;
    println!(
        "OpenXR Metal session: runtime_device='{}' matched_adapter='{}'",
        graphics.required_device_name, graphics.adapter_name
    );
    println!("OpenXR reference space: STAGE");
    run_smoke_frames(
        graphics,
        stage,
        stereo_config,
        environment_blend_mode,
        smoke,
    )?;
    Ok(())
}

#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
fn create_graphics_session_probe(
    instance: &xr::Instance,
    system: xr::SystemId,
    stereo_config: XrStereoConfig,
    environment_blend_mode: xr::EnvironmentBlendMode,
    smoke: DesktopXrSmoke,
) -> Result<()> {
    let graphics = graphics_vulkan::create_graphics_session(instance, system)
        .context("create OpenXR Vulkan graphics session")?;
    let stage = mclone_xr_host::create_stage_reference_space(&graphics.session)?;
    println!(
        "OpenXR Vulkan session: physical_device='{}' api={} queue_family={}",
        graphics.physical_device_name,
        graphics.physical_device_api_version,
        graphics.queue_family_index
    );
    println!("OpenXR reference space: STAGE");
    run_smoke_frames(
        graphics,
        stage,
        stereo_config,
        environment_blend_mode,
        smoke,
    )?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn log_openxr_host_event(event: OpenXrHostEvent) {
    match event {
        OpenXrHostEvent::SessionStateChanged(state) => {
            println!("OpenXR session state: {state:?}");
        }
        OpenXrHostEvent::InstanceLossPending => {
            println!("OpenXR instance loss pending");
        }
        OpenXrHostEvent::EventsLost(count) => {
            println!("OpenXR events lost: {count}");
        }
    }
}

#[cfg(not(target_os = "android"))]
#[derive(Default)]
struct XrControllerInputSummary {
    frames_polled: u32,
    left_active_frames: u32,
    right_active_frames: u32,
    left_tracked_frames: u32,
    right_tracked_frames: u32,
    max_trigger: f32,
    max_squeeze: f32,
    max_thumbstick: f32,
    select_pressed_frames: u32,
    a_pressed_frames: u32,
    latest_left: Option<XrControllerSnapshot>,
    latest_right: Option<XrControllerSnapshot>,
}

#[cfg(not(target_os = "android"))]
impl XrControllerInputSummary {
    fn record(&mut self, snapshots: &[XrControllerSnapshot]) {
        self.frames_polled += 1;
        for snapshot in snapshots {
            let tracked = snapshot.aim_position.is_some() || snapshot.grip_position.is_some();
            match snapshot.hand {
                XrHand::Left => {
                    self.left_active_frames += 1;
                    self.left_tracked_frames += u32::from(tracked);
                    self.latest_left = Some(*snapshot);
                }
                XrHand::Right => {
                    self.right_active_frames += 1;
                    self.right_tracked_frames += u32::from(tracked);
                    self.latest_right = Some(*snapshot);
                }
            }
            self.max_trigger = self.max_trigger.max(snapshot.trigger);
            self.max_squeeze = self.max_squeeze.max(snapshot.squeeze);
            self.max_thumbstick = self.max_thumbstick.max(snapshot.thumbstick.length());
            self.select_pressed_frames += u32::from(snapshot.select_pressed);
            self.a_pressed_frames += u32::from(snapshot.a_pressed);
        }
    }

    fn print_summary(&self) {
        println!(
            "OpenXR controller input summary: frames_polled={} left_active={} right_active={} left_tracked={} right_tracked={} max_trigger={:.3} max_squeeze={:.3} max_thumbstick={:.3} select_pressed_frames={} a_pressed_frames={}",
            self.frames_polled,
            self.left_active_frames,
            self.right_active_frames,
            self.left_tracked_frames,
            self.right_tracked_frames,
            self.max_trigger,
            self.max_squeeze,
            self.max_thumbstick,
            self.select_pressed_frames,
            self.a_pressed_frames
        );
        if let Some(left) = self.latest_left {
            println!("OpenXR controller latest left: {}", format_snapshot(left));
        }
        if let Some(right) = self.latest_right {
            println!("OpenXR controller latest right: {}", format_snapshot(right));
        }
    }
}

#[cfg(not(target_os = "android"))]
fn format_snapshot(snapshot: XrControllerSnapshot) -> String {
    format!(
        "aim={} grip={} trigger={:.3} squeeze={:.3} select={} a={} thumbstick=({:.3}, {:.3}) thumbstick_pressed={}",
        format_position(snapshot.aim_position),
        format_position(snapshot.grip_position),
        snapshot.trigger,
        snapshot.squeeze,
        snapshot.select_pressed,
        snapshot.a_pressed,
        snapshot.thumbstick.x,
        snapshot.thumbstick.y,
        snapshot.thumbstick_pressed
    )
}

#[cfg(not(target_os = "android"))]
fn format_position(position: Option<Vec3>) -> String {
    position
        .map(|position| format!("({:.3}, {:.3}, {:.3})", position.x, position.y, position.z))
        .unwrap_or_else(|| "untracked".to_owned())
}

#[cfg(not(target_os = "android"))]
fn run_smoke_frames(
    mut graphics: platform_graphics::GraphicsSession,
    stage: xr::Space,
    stereo_config: XrStereoConfig,
    environment_blend_mode: xr::EnvironmentBlendMode,
    smoke: DesktopXrSmoke,
) -> Result<()> {
    let frame_limit = match &smoke {
        DesktopXrSmoke::Clear { frame_limit } => *frame_limit,
        DesktopXrSmoke::Mclone { options } => options.frame_limit,
    };
    let frame_limit_label = frame_limit
        .map(|frames| frames.to_string())
        .unwrap_or_else(|| "unbounded".to_owned());
    let (eye_width, eye_height) = stereo_config.primary_eye_size();
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
        left_eye.texture_count(),
        right_eye.texture_count()
    );
    println!("OpenXR frame limit: {frame_limit_label}");
    if frame_limit.is_none() {
        println!("OpenXR session READY timeout: disabled");
    }
    let mut mclone = match smoke {
        DesktopXrSmoke::Clear { .. } => None,
        DesktopXrSmoke::Mclone { options } => Some(
            XrMcloneWorldState::new(&graphics.device, &graphics.queue, options)
                .context("initialize mclone XR world state")?,
        ),
    };
    let controller_actions = OpenXrControllerActions::create_with_binding_logger(
        graphics.session.instance(),
        &graphics.session,
        |profile, err| println!("OpenXR binding suggestion unavailable for {profile}: {err:?}"),
    )
    .context("initialize OpenXR controller actions")?;
    println!(
        "OpenXR controller actions: requested binding profiles=simple_controller, oculus_touch, valve_index, htc_vive, microsoft_motion_controller"
    );
    let mut controller_summary = XrControllerInputSummary::default();

    let mut event_storage = xr::EventDataBuffer::new();
    let mut session_running = false;
    let session_ready_deadline = frame_limit.map(|_| Instant::now() + SESSION_READY_TIMEOUT);
    let mut frame_stats = XrFrameStats::default();
    let mut runtime_requested_exit = false;

    while frame_limit
        .map(|frames| frame_stats.submitted_frames < u64::from(frames))
        .unwrap_or(true)
    {
        match mclone_xr_host::poll_openxr_events(
            &graphics.session,
            &mut event_storage,
            &mut session_running,
            VIEW_TYPE,
            log_openxr_host_event,
        )
        .context("poll OpenXR events")?
        {
            OpenXrPollStatus::Exit if frame_limit.is_some() => {
                bail!("OpenXR session requested exit before smoke completed")
            }
            OpenXrPollStatus::Exit => {
                runtime_requested_exit = true;
                break;
            }
            OpenXrPollStatus::Idle if !session_running => {
                if session_ready_deadline
                    .map(|deadline| Instant::now() >= deadline)
                    .unwrap_or(false)
                {
                    bail!("timed out waiting for OpenXR session READY state");
                }
                thread::sleep(SESSION_IDLE_POLL_INTERVAL);
                continue;
            }
            OpenXrPollStatus::Idle | OpenXrPollStatus::Running => {}
        }

        let frame_state = mclone_xr_host::wait_begin_frame(
            &mut graphics.frame_wait,
            &mut graphics.frame_stream,
            &mut frame_stats,
        )?;

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
                            &controllers,
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
                frame_stats.record_submitted_frame();
            })
        } else {
            mclone_xr_host::end_skipped_frame(
                &mut graphics.frame_stream,
                frame_state.predicted_display_time,
                environment_blend_mode,
                &mut frame_stats,
            )
        };

        if let Err(err) = frame_result {
            let _ = mclone_xr_host::end_frame_with_layers(
                &mut graphics.frame_stream,
                frame_state.predicted_display_time,
                environment_blend_mode,
                &[],
            );
            return Err(err);
        }
    }

    println!(
        "OpenXR frames submitted: submitted={} runtime_frames={} skipped={}",
        frame_stats.submitted_frames, frame_stats.runtime_frames, frame_stats.skipped_frames
    );
    controller_summary.print_summary();
    if let Some(mclone) = &mclone {
        mclone.print_summary();
    }
    let completed_label = if mclone.is_some() { "mclone" } else { "clear" };
    if !runtime_requested_exit {
        shutdown_openxr_session(&mut graphics, &mut event_storage, &mut session_running)
            .context("shut down OpenXR session after smoke")?;
    }
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
    println!("desktop OpenXR {completed_label} smoke complete: frames={frame_limit_label}");
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
        match mclone_xr_host::poll_openxr_events(
            &graphics.session,
            event_storage,
            session_running,
            VIEW_TYPE,
            log_openxr_host_event,
        )
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
struct XrMcloneWorldState {
    runtime: WindowSceneRuntime,
    camera: EngineCameraController,
    initial_alignment_mode: XrViewAlignmentMode,
    render_options: TexturedSectionRenderOptions,
    draw: TexturedSectionDrawResources,
    sky: SkyRenderer,
    actors: ActorDrawResources,
    render_stats: RenderStreamStats,
    tracking_origin: Option<XrTrackingOrigin>,
    last_locomotion_update: Option<Instant>,
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
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(
            options.scene.chunk_x,
            options.scene.chunk_z,
        ));
        let mut initial_pose_changed = runtime
            .apply_pending_engine_camera_position_updates(&mut camera)
            .context("accept initial XR server player pose")?;
        let initial_alignment_mode = if let Some(view_pose) = options.view_pose {
            apply_xr_startup_view_pose(&mut camera, view_pose)
                .context("apply XR startup view pose")?;
            initial_pose_changed |= runtime
                .commit_engine_camera_player_pose(&mut camera)
                .context("sync initial XR view-pose player pose")?;
            XrViewAlignmentMode::ViewPose
        } else {
            initial_pose_changed |= runtime
                .commit_engine_camera_player_pose(&mut camera)
                .context("sync initial XR player pose")?;
            XrViewAlignmentMode::PlayerSpawn
        };
        if initial_pose_changed {
            let _ = poll_window_runtime_until_idle(&mut runtime)
                .context("wait for chunks after initial XR player pose")?;
        }
        let camera_position = glam_vec3_from_vec3d(camera.snapshot().eye);
        let section_update = runtime
            .sync_all_render_sections(camera_position)
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
            &runtime.traversal_ready_render_section_keys(camera_position),
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
            camera,
            initial_alignment_mode,
            render_options: options.render_options,
            draw,
            sky,
            actors,
            render_stats,
            tracking_origin: None,
            last_locomotion_update: None,
            first_eye_summary: None,
            rendered_frames: 0,
        })
    }

    fn render_views(&mut self, views: &[xr::View]) -> Result<[ChunkRenderView; 2]> {
        if views.len() < 2 {
            bail!("OpenXR runtime returned fewer than two stereo views");
        }
        let tracking_origin = match self.tracking_origin {
            Some(origin) => origin,
            None => {
                let origin =
                    XrTrackingOrigin::from_initial_views(views, self.initial_alignment_mode)?;
                let snapshot = self.camera.snapshot();
                println!(
                    "mclone XR player-root alignment: mode={} root_eye=({:.2}, {:.2}, {:.2}) root_yaw_degrees={:.1} stage_center=({:.3}, {:.3}, {:.3}) stage_yaw_degrees={:.1}",
                    origin.mode_label(),
                    snapshot.eye.x,
                    snapshot.eye.y,
                    snapshot.eye.z,
                    snapshot.yaw_radians.to_degrees(),
                    origin.origin_stage.x,
                    origin.origin_stage.y,
                    origin.origin_stage.z,
                    origin.stage_yaw.to_degrees()
                );
                self.tracking_origin = Some(origin);
                origin
            }
        };
        let transform =
            XrStageToWorld::from_tracking_origin(tracking_origin, self.camera.snapshot())?;
        Ok([
            xr_view_to_chunk_render_view(&views[0], transform, XR_NEAR, XR_FAR)?,
            xr_view_to_chunk_render_view(&views[1], transform, XR_NEAR, XR_FAR)?,
        ])
    }

    fn apply_locomotion_input(&mut self, controllers: &[XrControllerSnapshot]) -> Result<()> {
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        let input = xr_locomotion_input_from_controllers(controllers, dt_seconds);
        self.camera
            .apply_movement_input(self.runtime.client(), input);
        self.runtime
            .commit_engine_camera_player_pose(&mut self.camera)
            .context("sync XR locomotion player pose")?;
        Ok(())
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
fn apply_xr_startup_view_pose(
    camera: &mut EngineCameraController,
    view_pose: XrViewPose,
) -> Result<()> {
    let yaw_radians = view_pose.yaw_degrees.to_radians();
    if !yaw_radians.is_finite() {
        bail!("invalid XR startup view yaw {}", view_pose.yaw_degrees);
    }
    camera.set_eye_pose(
        vec3d_from_glam(Vec3::from_array(view_pose.position)),
        f64::from(yaw_radians),
        0.0,
    );
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

#[cfg(not(target_os = "android"))]
fn vec3d_from_glam(value: Vec3) -> Vec3d {
    Vec3d::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug)]
struct XrTrackingOrigin {
    origin_stage: Vec3,
    stage_yaw: f32,
    mode: XrViewAlignmentMode,
}

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug)]
struct XrStageToWorld {
    origin_stage: Vec3,
    origin_world: Vec3,
    stage_to_world_rotation: Quat,
}

#[cfg(not(target_os = "android"))]
impl XrTrackingOrigin {
    fn from_initial_views(views: &[xr::View], mode: XrViewAlignmentMode) -> Result<Self> {
        let left_stage_pose = mclone_xr_host::view_pose(&views[0])?;
        let right_stage_pose = mclone_xr_host::view_pose(&views[1])?;
        let origin_stage = (left_stage_pose.position + right_stage_pose.position) * 0.5;
        Self::from_stage_view(origin_stage, left_stage_pose.orientation, mode)
    }

    fn from_stage_view(
        origin_stage: Vec3,
        left_stage_orientation: Quat,
        mode: XrViewAlignmentMode,
    ) -> Result<Self> {
        let stage_forward = left_stage_orientation * Vec3::NEG_Z;
        let stage_yaw = yaw_from_forward(stage_forward)
            .ok_or_else(|| anyhow!("OpenXR returned an invalid tracking-origin yaw"))?;
        Ok(Self {
            origin_stage,
            stage_yaw,
            mode,
        })
    }

    fn mode_label(self) -> &'static str {
        self.mode.label()
    }
}

#[cfg(not(target_os = "android"))]
impl XrStageToWorld {
    fn from_tracking_origin(
        origin: XrTrackingOrigin,
        snapshot: EngineCameraSnapshot,
    ) -> Result<Self> {
        let world_yaw = snapshot.yaw_radians as f32;
        if !world_yaw.is_finite() {
            bail!("invalid XR player root yaw {}", snapshot.yaw_radians);
        }
        Ok(Self {
            origin_stage: origin.origin_stage,
            origin_world: glam_vec3_from_vec3d(snapshot.eye),
            stage_to_world_rotation: Quat::from_rotation_y(normalize_angle(
                world_yaw - origin.stage_yaw,
            )),
        })
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
    controllers: &[XrControllerSnapshot],
) -> Result<()> {
    let stereo_views =
        mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)
            .context("locate OpenXR stereo views for mclone frame")?;
    let views = [stereo_views.left, stereo_views.right];
    mclone.apply_locomotion_input(controllers)?;
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

    mclone_xr_host::end_stereo_projection_frame(
        &mut graphics.frame_stream,
        predicted_display_time,
        environment_blend_mode,
        stage,
        stereo_views,
        left_eye,
        right_eye,
    )
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
    let mut left_stats = mclone.render_stats;
    // The shared mclone render resources own per-view uniform buffers. Match
    // Playbox's per-eye submit shape so the right-eye uniform writes cannot
    // overwrite the left-eye pass before the GPU consumes it.
    let left_summary = render_mclone_eye_target(
        graphics,
        left_target,
        &mclone.sky,
        &mut mclone.draw,
        &mut mclone.actors,
        render_views[0],
        actor_instances,
        render_options,
        sky_clear_color,
        time_of_day,
        sun_angle,
        &mut left_stats,
        "left",
    )?;
    let mut right_stats = left_stats;
    render_mclone_eye_target(
        graphics,
        right_target,
        &mclone.sky,
        &mut mclone.draw,
        &mut mclone.actors,
        render_views[1],
        actor_instances,
        render_options,
        sky_clear_color,
        time_of_day,
        sun_angle,
        &mut right_stats,
        "right",
    )?;
    mclone.render_stats = left_stats;
    Ok(left_summary)
}

#[cfg(not(target_os = "android"))]
#[allow(clippy::too_many_arguments)]
fn render_mclone_eye_target(
    graphics: &mut platform_graphics::GraphicsSession,
    target: &AcquiredEyeTarget<'_>,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    actors: &mut ActorDrawResources,
    render_view: ChunkRenderView,
    actor_instances: &[ActorInstance],
    render_options: TexturedSectionRenderOptions,
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_stats: &mut RenderStreamStats,
    label: &'static str,
) -> Result<FullFrameRenderSummary> {
    let mut encoder = graphics
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(match label {
                "left" => "mclone_xr_left_eye_encoder",
                "right" => "mclone_xr_right_eye_encoder",
                _ => "mclone_xr_eye_encoder",
            }),
        });
    let frame = RenderFrameContext::new(
        &graphics.device,
        &graphics.queue,
        &mut encoder,
        RenderFrameTarget::color(
            target.color_view(),
            [target.eye().width, target.eye().height],
        ),
    );
    let summary = render_full_frame_for_view(
        frame,
        &target.eye().depth,
        sky,
        draw,
        Some(actors),
        None,
        None,
        render_view,
        actor_instances,
        None,
        sky_clear_color,
        time_of_day,
        sun_angle,
        render_options,
        FullFrameGui::new(false, false, [1.0, 1.0]),
        |_| GuiDrawList::new(),
        render_stats,
    )
    .with_context(|| format!("render mclone {label} eye"))?;
    let submission = graphics.queue.submit(Some(encoder.finish()));
    graphics
        .device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .with_context(|| format!("wait for OpenXR mclone {label}-eye render submission"))?;
    graphics
        .device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .with_context(|| format!("wait for OpenXR mclone {label}-eye render device idle"))?;
    Ok(summary)
}

#[cfg(not(target_os = "android"))]
fn xr_view_to_chunk_render_view(
    view: &xr::View,
    transform: XrStageToWorld,
    near: f32,
    far: f32,
) -> Result<ChunkRenderView> {
    let stage_pose = mclone_xr_host::view_pose(view)?;
    let (camera_position, camera_orientation) =
        transform.transform_pose(stage_pose.position, stage_pose.orientation);
    let render_view = mclone_xr_host::render_view_from_world_pose(
        mclone_xr_host::XrViewPose {
            position: camera_position,
            orientation: camera_orientation,
        },
        view.fov,
        near,
        far,
    )?;
    Ok(chunk_render_view_from_xr_render_view(render_view))
}

#[cfg(not(target_os = "android"))]
fn chunk_render_view_from_xr_render_view(view: mclone_xr_host::XrRenderView) -> ChunkRenderView {
    ChunkRenderView {
        view: view.view,
        projection: view.projection,
        view_projection: view.view_projection,
        camera_position: view.camera_position,
        camera_forward: view.camera_forward,
        camera_right: view.camera_right,
        camera_up: view.camera_up,
        aspect: view.aspect,
        fov_y_radians: view.fov_y_radians,
        z_near: view.z_near,
        z_far: view.z_far,
    }
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
fn render_clear_frame(
    graphics: &mut platform_graphics::GraphicsSession,
    stage: &xr::Space,
    environment_blend_mode: xr::EnvironmentBlendMode,
    predicted_display_time: xr::Time,
    left_eye: &mut platform_graphics::OpenXrEyeState,
    right_eye: &mut platform_graphics::OpenXrEyeState,
) -> Result<()> {
    let stereo_views =
        mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;

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

    mclone_xr_host::end_stereo_projection_frame(
        &mut graphics.frame_stream,
        predicted_display_time,
        environment_blend_mode,
        stage,
        stereo_views,
        left_eye,
        right_eye,
    )
}

#[cfg(not(target_os = "android"))]
fn acquire_eye_target(
    eye_state: &mut platform_graphics::OpenXrEyeState,
) -> Result<AcquiredEyeTarget<'_>> {
    mclone_xr_host::acquire_eye_target(eye_state)
}

#[cfg(not(target_os = "android"))]
fn clear_stereo_targets(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    left_target: &AcquiredEyeTarget<'_>,
    right_target: &AcquiredEyeTarget<'_>,
) -> Result<()> {
    mclone_xr_host::clear_stereo_targets(
        device,
        queue,
        mclone_xr_host::XrClearTarget {
            color_view: left_target.color_view(),
            depth_view: Some(&left_target.eye().depth.view),
        },
        mclone_xr_host::XrClearTarget {
            color_view: right_target.color_view(),
            depth_view: Some(&right_target.eye().depth.view),
        },
    )
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

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn startup_view_pose_maps_stage_center_to_requested_world_pose() {
        let stage_center = Vec3::new(1.0, 1.6, -0.25);
        let stage_orientation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let view_pose = XrViewPose {
            position: [8.0, 72.0, -12.0],
            yaw_degrees: 0.0,
        };

        let origin = XrTrackingOrigin::from_stage_view(
            stage_center,
            stage_orientation,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let transform = XrStageToWorld::from_tracking_origin(origin, snapshot).unwrap();
        let (world_position, world_orientation) =
            transform.transform_pose(stage_center, stage_orientation);
        let world_forward = world_orientation * Vec3::NEG_Z;

        assert!((world_position - Vec3::from_array(view_pose.position)).length() < 1.0e-5);
        assert!((world_forward - Vec3::NEG_Z).length() < 1.0e-5);
        assert_eq!(origin.mode_label(), "view-pose");
    }

    #[test]
    fn player_root_transform_preserves_physical_hmd_offset() {
        let origin = XrTrackingOrigin::from_stage_view(
            Vec3::new(1.0, 1.6, -0.25),
            Quat::IDENTITY,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let transform = XrStageToWorld::from_tracking_origin(origin, snapshot).unwrap();

        let (world_position, _) = transform.transform_pose(
            origin.origin_stage + Vec3::new(0.35, 0.0, -0.2),
            Quat::IDENTITY,
        );

        assert!((world_position - Vec3::new(8.35, 72.0, -12.2)).length() < 1.0e-5);
    }
}
