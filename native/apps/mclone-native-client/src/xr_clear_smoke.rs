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
use glam::Vec3;
#[cfg(not(target_os = "android"))]
use mclone_xr_host::{
    OpenXrControllerActions, OpenXrHostEvent, OpenXrPollStatus, PRIMARY_STEREO_VIEW_TYPE,
    XrControllerSnapshot, XrFrameStats, XrHand, XrStereoConfig,
};
#[cfg(not(target_os = "android"))]
use mclone_xr_scene::{
    XrDebugUiScreen as SceneXrDebugUiScreen, XrFramePipelineHostTiming, XrFramePipelineReporter,
    XrMcloneTerrainState, XrSceneOptions, XrStartupViewPose, XrTerrainEyeTarget,
    XrTerrainFrameSummary, XrUnderwaterDetectionMode,
};
#[cfg(not(target_os = "android"))]
use openxr as xr;

use crate::cli::{
    SceneOptions, XrClearSmokeOptions, XrDebugUiScreen as CliXrDebugUiScreen, XrMcloneSmokeOptions,
};
#[cfg(not(target_os = "android"))]
use crate::remote_session::RemoteServerSession;
#[cfg(not(target_os = "android"))]
use crate::render_cache::load_asset_source;
#[cfg(not(target_os = "android"))]
use crate::scene_runtime::{
    native_window_scene_runtime, native_window_scene_runtime_with_mesh_assets,
};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::elapsed_ms;
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::local_single_view::NativeSingleViewSessionRuntime;
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::render_assets::{
    load_actor_texture_assets_from_asset_source, load_textured_mesh_assets_from_source,
};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::session::{RemoteSessionEndpoint, SessionStartRequest};
#[cfg(not(target_os = "android"))]
use mclone_audio::{AudioEngine, AudioSettings};

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

#[cfg(not(target_os = "android"))]
type DesktopXrMcloneTerrainState = XrMcloneTerrainState<RemoteServerSession>;

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
const XR_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
#[cfg(not(target_os = "android"))]
const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
#[cfg(not(target_os = "android"))]
const XR_SAMPLE_COUNT: u32 = 1;
#[cfg(not(target_os = "android"))]
const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "android"))]
const SUBMITTED_FRAME_PROGRESS_TIMEOUT: Duration = Duration::from_secs(15);
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
    b_pressed_frames: u32,
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
            self.b_pressed_frames += u32::from(snapshot.b_pressed);
        }
    }

    fn print_summary(&self) {
        println!(
            "OpenXR controller input summary: frames_polled={} left_active={} right_active={} left_tracked={} right_tracked={} max_trigger={:.3} max_squeeze={:.3} max_thumbstick={:.3} select_pressed_frames={} a_pressed_frames={} b_pressed_frames={}",
            self.frames_polled,
            self.left_active_frames,
            self.right_active_frames,
            self.left_tracked_frames,
            self.right_tracked_frames,
            self.max_trigger,
            self.max_squeeze,
            self.max_thumbstick,
            self.select_pressed_frames,
            self.a_pressed_frames,
            self.b_pressed_frames
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
        "aim={} aim_dir={} grip={} trigger={:.3} squeeze={:.3} select={} a={} b={} thumbstick=({:.3}, {:.3}) thumbstick_pressed={}",
        format_position(snapshot.aim_position),
        format_direction(snapshot.aim_direction),
        format_position(snapshot.grip_position),
        snapshot.trigger,
        snapshot.squeeze,
        snapshot.select_pressed,
        snapshot.a_pressed,
        snapshot.b_pressed,
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
fn format_direction(direction: Option<Vec3>) -> String {
    direction
        .map(|direction| {
            format!(
                "({:.3}, {:.3}, {:.3})",
                direction.x, direction.y, direction.z
            )
        })
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
    } else {
        println!(
            "OpenXR submitted-frame progress timeout: {:.1}s",
            SUBMITTED_FRAME_PROGRESS_TIMEOUT.as_secs_f64()
        );
    }
    let mut mclone = match smoke {
        DesktopXrSmoke::Clear { .. } => None,
        DesktopXrSmoke::Mclone { options } => Some(
            create_mclone_terrain_state(&graphics.device, &graphics.queue, options)
                .context("initialize mclone XR terrain state")?,
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
    let mut submitted_frame_progress_deadline =
        frame_limit.map(|_| Instant::now() + SUBMITTED_FRAME_PROGRESS_TIMEOUT);
    let mut frame_stats = XrFrameStats::default();
    let mut frame_pipeline_reporter = XrFramePipelineReporter::new(None);
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

        let frame_wall_start = Instant::now();
        let submitted_frames_before = frame_stats.submitted_frames;
        let wait_begin_start = Instant::now();
        let frame_state = mclone_xr_host::wait_begin_frame(
            &mut graphics.frame_wait,
            &mut graphics.frame_stream,
            &mut frame_stats,
        )?;
        let wait_begin_ms = elapsed_ms(wait_begin_start.elapsed());

        let mut controller_poll_ms = 0.0;
        let mut rendered_summary = None;
        let frame_result = if frame_state.should_render {
            let controller_poll_start = Instant::now();
            let result = match controller_actions.poll(
                &graphics.session,
                &stage,
                frame_state.predicted_display_time,
            ) {
                Ok(controllers) => {
                    controller_poll_ms = elapsed_ms(controller_poll_start.elapsed());
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
                        .map(|summary| {
                            rendered_summary = Some(summary);
                        })
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
                }
                Err(err) => {
                    controller_poll_ms = elapsed_ms(controller_poll_start.elapsed());
                    Err(err)
                }
            };
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

        if let Some(mclone) = &mut mclone {
            let (frame_pipeline_report, frame_pipeline_revision) = frame_pipeline_reporter
                .record_frame(
                    XrFramePipelineHostTiming {
                        frame_wall_ms: elapsed_ms(frame_wall_start.elapsed()),
                        wait_frame_ms: wait_begin_ms,
                        controller_poll_ms,
                        rendered: rendered_summary.is_some(),
                        thread_cpu_ms: None,
                    },
                    rendered_summary,
                );
            mclone.set_frame_pipeline_report(frame_pipeline_report, frame_pipeline_revision);
        }

        if let Some(deadline) = submitted_frame_progress_deadline.as_mut() {
            if frame_stats.submitted_frames > submitted_frames_before {
                *deadline = Instant::now() + SUBMITTED_FRAME_PROGRESS_TIMEOUT;
            } else if Instant::now() >= *deadline {
                bail!(
                    "timed out waiting for OpenXR submitted-frame progress: submitted={} runtime_frames={} skipped={}",
                    frame_stats.submitted_frames,
                    frame_stats.runtime_frames,
                    frame_stats.skipped_frames
                );
            }
        }
    }

    println!(
        "OpenXR frames submitted: submitted={} runtime_frames={} skipped={}",
        frame_stats.submitted_frames, frame_stats.runtime_frames, frame_stats.skipped_frames
    );
    controller_summary.print_summary();
    if let Some(mclone) = &mclone {
        print_mclone_summary(mclone);
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
fn create_mclone_terrain_state(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    options: XrMcloneSmokeOptions,
) -> Result<DesktopXrMcloneTerrainState> {
    let asset_source = load_asset_source().context("load mclone XR asset source")?;
    let actor_assets = load_actor_texture_assets_from_asset_source(&asset_source)
        .context("load mclone actor texture assets")?;
    let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
        Ok(audio) => Some(audio),
        Err(err) => {
            println!("desktop OpenXR audio disabled: {err:#}");
            None
        }
    };
    let scene = xr_scene_options_from_desktop_scene(
        &options.scene,
        options.underwater_mode,
        options.debug_ui_screen,
    )?;
    let startup_view_pose = options.view_pose.map(|view_pose| XrStartupViewPose {
        position: view_pose.position,
        yaw_degrees: view_pose.yaw_degrees,
    });
    let mut state = if options.scene.remote_addr.is_some() {
        let request = session_start_request_for_desktop_scene(&options.scene);
        let runtime = NativeSingleViewSessionRuntime::from_active_runtime(
            request,
            native_window_scene_runtime(&options.scene)?,
        )?;
        XrMcloneTerrainState::with_runtime(
            device,
            queue,
            XR_COLOR_FORMAT,
            scene,
            runtime,
            options.render_options,
            actor_assets.atlas.clone(),
            actor_assets.figures.clone(),
            &asset_source,
            startup_view_pose,
        )
    } else {
        XrMcloneTerrainState::start_local_async(
            device,
            queue,
            XR_COLOR_FORMAT,
            scene,
            options.render_options,
            load_textured_mesh_assets_from_source(&asset_source)?,
            actor_assets.atlas.clone(),
            actor_assets.figures.clone(),
            &asset_source,
            startup_view_pose,
        )
    }
    .context("initialize shared mclone XR terrain scene")?;
    state.set_audio_engine(audio);
    state.set_session_runtime_factory(|request, scene, mesh_assets| {
        let desktop_scene = desktop_scene_options_for_xr_request(&request, &scene);
        let runtime = native_window_scene_runtime_with_mesh_assets(&desktop_scene, mesh_assets)?;
        NativeSingleViewSessionRuntime::from_active_runtime(request, runtime)
    });
    Ok(state)
}

#[cfg(not(target_os = "android"))]
fn session_start_request_for_desktop_scene(scene: &SceneOptions) -> SessionStartRequest {
    scene.remote_addr.as_ref().map_or(
        SessionStartRequest::new_seed_local_world(scene.seed),
        |remote_addr| SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
        },
    )
}

#[cfg(not(target_os = "android"))]
fn xr_scene_options_from_desktop_scene(
    scene: &SceneOptions,
    underwater_mode: crate::cli::XrUnderwaterMode,
    debug_ui_screen: Option<CliXrDebugUiScreen>,
) -> Result<XrSceneOptions> {
    XrSceneOptions {
        seed: scene.seed,
        chunk_x: scene.chunk_x,
        chunk_z: scene.chunk_z,
        render_distance: u32::try_from(scene.render_distance)
            .context("desktop XR render distance must fit u32")?,
        render_compile_worker_count: scene.render_compile_worker_count,
        movement_speed_multiplier: scene.movement_speed_multiplier,
        day_time_override: scene.day_time_override,
        freeze_time: scene.freeze_time,
        debug_passive_showcase: scene.debug_passive_showcase,
        lighting_enabled: scene.lighting_enabled,
        adaptive_chunk_publication_budget: scene.adaptive_chunk_publication_budget,
        far_lod: scene.far_lod,
        underwater_detection_mode: xr_underwater_mode_from_desktop(underwater_mode),
        debug_ui_screen: debug_ui_screen.map(xr_debug_ui_screen_from_desktop),
        skip_actors: false,
        world_root: scene.world_root.clone(),
        world_dir: scene.world_dir.clone(),
    }
    .validated()
}

#[cfg(not(target_os = "android"))]
fn xr_debug_ui_screen_from_desktop(screen: CliXrDebugUiScreen) -> SceneXrDebugUiScreen {
    match screen {
        CliXrDebugUiScreen::Pause => SceneXrDebugUiScreen::Pause,
        CliXrDebugUiScreen::Controls => SceneXrDebugUiScreen::Controls,
    }
}

#[cfg(not(target_os = "android"))]
fn xr_underwater_mode_from_desktop(
    mode: crate::cli::XrUnderwaterMode,
) -> XrUnderwaterDetectionMode {
    match mode {
        crate::cli::XrUnderwaterMode::Midpoint => XrUnderwaterDetectionMode::Midpoint,
        crate::cli::XrUnderwaterMode::PerEye => XrUnderwaterDetectionMode::PerEye,
    }
}

#[cfg(not(target_os = "android"))]
fn desktop_scene_options_for_xr_request(
    request: &SessionStartRequest,
    scene: &XrSceneOptions,
) -> SceneOptions {
    let mut seed = scene.seed;
    let mut remote_addr = None;
    let mut world_dir = scene.world_dir.clone();
    match request {
        SessionStartRequest::CreateLocalWorld { options } => {
            seed = options.seed;
            if options.requested_id.is_none() {
                world_dir = None;
            }
        }
        SessionStartRequest::OpenLocalWorld { .. } => {
            remote_addr = None;
        }
        SessionStartRequest::JoinRemote { endpoint } => {
            remote_addr = Some(endpoint.address.clone());
            world_dir = None;
        }
        SessionStartRequest::Unknown => {}
    }
    SceneOptions {
        seed,
        chunk_x: scene.chunk_x,
        chunk_z: scene.chunk_z,
        render_distance: scene.render_distance as i32,
        render_compile_worker_count: scene.render_compile_worker_count,
        movement_speed_multiplier: scene.movement_speed_multiplier,
        simulation_cadence: Default::default(),
        remote_addr,
        day_time_override: scene.day_time_override,
        freeze_time: scene.freeze_time,
        first_person_player_visible: false,
        lighting_enabled: scene.lighting_enabled,
        far_lod: scene.far_lod,
        world_root: scene.world_root.clone(),
        world_dir,
        ..SceneOptions::default()
    }
}

#[cfg(not(target_os = "android"))]
fn print_mclone_summary(mclone: &DesktopXrMcloneTerrainState) {
    let summary = mclone.frame_summary();
    println!(
        "mclone XR frame summary: frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={} ui_draw_rebuilds={} ui_draw_cache_hits={} ui_panel_repaints={} ui_panel_cache_hits={} ui_panel_texture_recreates={} ui_panel_composites={} local_startup_active={}",
        summary.rendered_frames,
        summary.section_count,
        summary.drawn_section_count,
        summary.index_count,
        summary.drawn_index_count,
        summary.actor_count,
        summary.drawn_actor_count,
        summary.ui_draw_cache.rebuild_count,
        summary.ui_draw_cache.cache_hit_count,
        summary.ui_panel.repaint_count,
        summary.ui_panel.cache_hit_count,
        summary.ui_panel.texture_recreate_count,
        summary.ui_panel.composite_count,
        summary.local_startup_active
    );
}

#[cfg(not(target_os = "android"))]
fn render_mclone_frame(
    graphics: &mut platform_graphics::GraphicsSession,
    stage: &xr::Space,
    environment_blend_mode: xr::EnvironmentBlendMode,
    predicted_display_time: xr::Time,
    left_eye: &mut platform_graphics::OpenXrEyeState,
    right_eye: &mut platform_graphics::OpenXrEyeState,
    mclone: &mut DesktopXrMcloneTerrainState,
    controllers: &[XrControllerSnapshot],
) -> Result<XrTerrainFrameSummary> {
    let stereo_views =
        mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)
            .context("locate OpenXR stereo views for mclone frame")?;
    mclone.apply_locomotion_input(controllers, [stereo_views.left, stereo_views.right])?;

    let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
    let right_target = match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image")
    {
        Ok(target) => target,
        Err(err) => {
            let _ = left_target.release();
            return Err(err);
        }
    };

    let render_result = mclone.render_frame(
        &graphics.device,
        &graphics.queue,
        [stereo_views.left, stereo_views.right],
        XrTerrainEyeTarget {
            color_view: left_target.color_view(),
            depth: &left_target.eye().depth,
            size: [left_target.eye().width, left_target.eye().height],
        },
        XrTerrainEyeTarget {
            color_view: right_target.color_view(),
            depth: &right_target.eye().depth,
            size: [right_target.eye().width, right_target.eye().height],
        },
    );
    let left_release_result = left_target.release();
    let right_release_result = right_target.release();
    let frame_summary = render_result?;
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
    )?;
    Ok(frame_summary)
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
