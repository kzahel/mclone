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
use mclone_scene::{
    McloneSceneHost, McloneSceneHostOptions, XrControllerInputRouter,
    XrDebugUiScreen as SceneXrDebugUiScreen, XrFramePipelineHostTiming, XrSceneFrameTarget,
    XrStartupViewPose, XrTerrainEyeTarget, XrTerrainFrameSummary, XrUnderwaterDetectionMode,
    record_xr_frame_pipeline, xr_frame_pipeline_accounting_config,
};
#[cfg(not(target_os = "android"))]
use mclone_xr_host::{
    OpenXrControllerActions, OpenXrHostEvent, PRIMARY_STEREO_VIEW_TYPE, TrackedControllerState,
    XrHand, XrInputFrame, XrStereoConfig,
};
#[cfg(not(target_os = "android"))]
use openxr as xr;

use crate::cli::{
    SceneOptions, WindowStartIntent, XrClearSmokeOptions, XrDebugUiScreen as CliXrDebugUiScreen,
    XrMcloneSmokeOptions,
};
#[cfg(not(target_os = "android"))]
use crate::render_cache::load_asset_source;
#[cfg(not(target_os = "android"))]
use crate::scene_runtime::native_window_scene_runtime_with_mesh_assets;
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::client_entry::{
    ClientEntryController, ClientEntryEffect, ClientHostAvailability,
};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::client_experience::xr_native_client_experience_profile;
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::native_service_assembly::NativeSessionServices;
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::render_assets::{
    load_actor_texture_assets_from_asset_source, load_textured_mesh_assets_from_source,
};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::session::{RemoteSessionEndpoint, SessionStartRequest};
#[cfg(not(target_os = "android"))]
use mclone_app_runtime::{elapsed_ms, frame_pipeline_accounting::FramePipelineAccountant};
#[cfg(not(target_os = "android"))]
use mclone_audio::{AudioEngine, AudioSettings};
#[cfg(not(target_os = "android"))]
use mclone_diagnostics::FrameHostKind;

#[cfg(not(target_os = "android"))]
mod companion_window;
#[cfg(not(target_os = "android"))]
use companion_window::CompanionWindow;

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
type DesktopXrSceneHost = McloneSceneHost;

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
const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
#[cfg(not(target_os = "android"))]
const XR_SAMPLE_COUNT: u32 = 1;
#[cfg(not(target_os = "android"))]
const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "android"))]
const SUBMITTED_FRAME_PROGRESS_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(not(target_os = "android"))]
#[derive(Debug)]
struct OpenXrRuntimeManifest {
    runtime_library_path: PathBuf,
}

#[cfg(not(target_os = "android"))]
enum DesktopXrMode {
    /// Per-eye diagnostic clear pattern; no world. Frame-bounded liveness smoke.
    Clear { frame_limit: Option<u32> },
    /// The mclone world, frame-bounded. CI/validation smoke gate.
    Mclone { options: XrMcloneSmokeOptions },
    /// The real desktop XR run verb: the same mclone world interior, persistent
    /// by default, with a desktop companion window unless suppressed. `window`
    /// is threaded through here for Slice 3 (companion window); Slice 2 renders
    /// the identical world path and does not yet spawn a window.
    Real {
        options: XrMcloneSmokeOptions,
        window: bool,
        start_intent: WindowStartIntent,
    },
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
    run_desktop_xr(DesktopXrMode::Clear {
        frame_limit: options.frame_limit,
    })
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run_mclone(options: XrMcloneSmokeOptions) -> Result<()> {
    run_desktop_xr(DesktopXrMode::Mclone { options })
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run_desktop(
    options: XrMcloneSmokeOptions,
    window: bool,
    start_intent: WindowStartIntent,
) -> Result<()> {
    run_desktop_xr(DesktopXrMode::Real {
        options,
        window,
        start_intent,
    })
}

#[cfg(not(target_os = "android"))]
fn run_desktop_xr(mode: DesktopXrMode) -> Result<()> {
    let (entry, entry_source) = load_openxr_entry()?;
    let available = entry
        .enumerate_extensions()
        .context("enumerate OpenXR instance extensions")?;
    let mut enabled_extensions = xr::ExtensionSet::default();
    let graphics_extension =
        enable_platform_graphics_extension(&available, &mut enabled_extensions)?;
    let display_refresh_supported = available.fb_display_refresh_rate;
    if display_refresh_supported {
        enabled_extensions.fb_display_refresh_rate = true;
    }

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
        display_refresh_supported,
        mode,
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
    display_refresh_supported: bool,
    mode: DesktopXrMode,
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
        display_refresh_supported,
        mode,
    )?;
    Ok(())
}

#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
fn create_graphics_session_probe(
    instance: &xr::Instance,
    system: xr::SystemId,
    stereo_config: XrStereoConfig,
    environment_blend_mode: xr::EnvironmentBlendMode,
    display_refresh_supported: bool,
    mode: DesktopXrMode,
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
        display_refresh_supported,
        mode,
    )?;
    Ok(())
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
    latest_left: Option<TrackedControllerState>,
    latest_right: Option<TrackedControllerState>,
}

#[cfg(not(target_os = "android"))]
impl XrControllerInputSummary {
    fn record(&mut self, input: &XrInputFrame) {
        self.frames_polled += 1;
        for snapshot in &input.tracked {
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
            if let Some(specific) = input.xr_specific.controller(snapshot.hand) {
                self.max_trigger = self.max_trigger.max(specific.pointer_select_value);
                self.max_squeeze = self.max_squeeze.max(specific.squeeze_value);
                self.max_thumbstick = self
                    .max_thumbstick
                    .max(specific.locomotion_axis.length())
                    .max(specific.turn_axis.length());
            }
        }
        self.select_pressed_frames += u32::from(
            input
                .actions
                .held
                .contains(&mclone_input::PlayerAction::OpenMenu),
        );
        self.a_pressed_frames += u32::from(
            input
                .actions
                .held
                .contains(&mclone_input::PlayerAction::Jump),
        );
        self.b_pressed_frames += u32::from(
            input
                .actions
                .held
                .contains(&mclone_input::PlayerAction::Descend),
        );
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
fn format_snapshot(snapshot: TrackedControllerState) -> String {
    format!(
        "aim={} aim_dir={} grip={}",
        format_position(snapshot.aim_position),
        format_direction(snapshot.aim_direction),
        format_position(snapshot.grip_position)
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
struct DesktopXrFrameOutput {
    summary: Option<XrTerrainFrameSummary>,
    controller_poll_ms: f64,
}

#[cfg(not(target_os = "android"))]
struct DesktopXrFrameLoop<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    stage: &'a xr::Space,
    left_eye: &'a mut platform_graphics::OpenXrEyeState,
    right_eye: &'a mut platform_graphics::OpenXrEyeState,
    mclone: &'a mut Option<DesktopXrSceneHost>,
    controller_actions: &'a mut OpenXrControllerActions,
    ordinary_gamepad: Option<crate::desktop_gamepad::DesktopGamepadCollector>,
    ordinary_gamepad_input: XrControllerInputRouter,
    controller_summary: XrControllerInputSummary,
    frame_pipeline_accountant: FramePipelineAccountant,
    companion: &'a mut Option<CompanionWindow>,
    companion_running_announced: bool,
    window_requested_exit: bool,
}

#[cfg(not(target_os = "android"))]
impl mclone_xr_host::OpenXrFrameLoopHandler<platform_graphics::AppGraphics>
    for DesktopXrFrameLoop<'_>
{
    type RenderOutput = DesktopXrFrameOutput;

    fn pump_platform_events(
        &mut self,
        timeout: Duration,
    ) -> Result<mclone_xr_host::OpenXrFrameLoopControl> {
        if let Some(companion) = self.companion.as_mut() {
            if companion.pump() {
                println!("desktop XR companion window close requested");
                companion.set_status("Desktop XR — shutting down…");
                self.window_requested_exit = true;
                return Ok(mclone_xr_host::OpenXrFrameLoopControl::Complete);
            }
        }
        if !timeout.is_zero() {
            thread::sleep(timeout);
        }
        Ok(mclone_xr_host::OpenXrFrameLoopControl::Continue)
    }

    fn on_openxr_event(&mut self, event: OpenXrHostEvent) {
        println!("{event}");
        if matches!(
            event,
            OpenXrHostEvent::SessionStateChanged(
                xr::SessionState::STOPPING
                    | xr::SessionState::LOSS_PENDING
                    | xr::SessionState::EXITING
            )
        ) {
            self.controller_actions.clear_transient_input();
            self.ordinary_gamepad_input.clear_transient_input();
        }
        if matches!(
            event,
            OpenXrHostEvent::SessionStateChanged(xr::SessionState::READY)
        ) && !self.companion_running_announced
        {
            if let Some(companion) = self.companion.as_mut() {
                companion.set_status("Desktop XR — running · close this window to quit");
            }
            self.companion_running_announced = true;
        }
    }

    fn render_frame(
        &mut self,
        frame: &mut mclone_xr_host::OpenXrRenderFrame<'_, platform_graphics::AppGraphics>,
    ) -> Result<Self::RenderOutput> {
        let controller_poll_start = Instant::now();
        let mut input = self.controller_actions.poll(
            frame.session(),
            self.stage,
            frame.predicted_display_time(),
        )?;
        let controller_poll_ms = elapsed_ms(controller_poll_start.elapsed());
        self.controller_summary.record(&input);
        let summary = if let Some(mclone) = self.mclone.as_mut() {
            if let Some(collector) = self.ordinary_gamepad.as_mut() {
                let poll = collector.poll()?;
                for source_id in poll.disconnected {
                    self.ordinary_gamepad_input.disconnect_source(source_id)?;
                }
                for (source_id, descriptor) in poll.connected {
                    self.ordinary_gamepad_input
                        .connect_source(source_id, descriptor);
                }
                self.ordinary_gamepad_input.route_batch(
                    mclone,
                    &poll.input,
                    self.device,
                    self.queue,
                )?;
            }
            self.ordinary_gamepad_input.merge_into_frame(&mut input);
            Some(render_desktop_xr_frame(
                self.device,
                self.queue,
                frame,
                self.stage,
                self.left_eye,
                self.right_eye,
                mclone,
                &input,
            )?)
        } else {
            render_clear_frame(
                self.device,
                self.queue,
                frame,
                self.stage,
                self.left_eye,
                self.right_eye,
            )?;
            None
        };
        Ok(DesktopXrFrameOutput {
            summary,
            controller_poll_ms,
        })
    }

    fn after_frame(
        &mut self,
        outcome: mclone_xr_host::OpenXrFrameOutcome<Self::RenderOutput>,
    ) -> Result<mclone_xr_host::OpenXrFrameLoopControl> {
        if let Some(mclone) = self.mclone.as_mut() {
            let (rendered_summary, controller_poll_ms) =
                outcome.render_output.map_or((None, 0.0), |output| {
                    (output.summary, output.controller_poll_ms)
                });
            let budget_decision_panel = mclone.latest_budget_decision_panel();
            let update = record_xr_frame_pipeline(
                &mut self.frame_pipeline_accountant,
                XrFramePipelineHostTiming {
                    frame_wall_ms: elapsed_ms(outcome.timing.frame_wall),
                    wait_frame_ms: elapsed_ms(
                        outcome.timing.wait_frame + outcome.timing.begin_frame,
                    ),
                    controller_poll_ms,
                    rendered: rendered_summary.is_some(),
                    thread_cpu_ms: None,
                },
                rendered_summary,
                budget_decision_panel,
            );
            mclone.set_frame_pipeline_budget_signal(update.budget_signal);
            if let Some((report, revision)) = update.published_report {
                mclone.set_frame_pipeline_report(report, revision);
            }
        }
        Ok(mclone_xr_host::OpenXrFrameLoopControl::Continue)
    }
}

#[cfg(not(target_os = "android"))]
fn run_smoke_frames(
    mut graphics: platform_graphics::GraphicsSession,
    stage: xr::Space,
    stereo_config: XrStereoConfig,
    environment_blend_mode: xr::EnvironmentBlendMode,
    display_refresh_supported: bool,
    mode: DesktopXrMode,
) -> Result<()> {
    let display_refresh = mclone_xr_host::query_display_refresh_snapshot(
        &graphics.session,
        display_refresh_supported,
    );
    println!(
        "OpenXR display refresh: current={} supported={}",
        display_refresh
            .current_rate
            .map(|hz| format!("{hz:.1} Hz"))
            .unwrap_or_else(|| "unknown".to_owned()),
        mclone_xr_host::display_refresh_rates_label(&display_refresh.supported_rates)
    );
    let frame_limit = match &mode {
        DesktopXrMode::Clear { frame_limit } => *frame_limit,
        DesktopXrMode::Mclone { options } | DesktopXrMode::Real { options, .. } => {
            options.frame_limit
        }
    };
    // Borrow-only classification captured before the `mode` match below moves
    // `options` out: whether to spawn the desktop companion window, and the
    // human label for the completion line.
    let want_window = matches!(mode, DesktopXrMode::Real { window: true, .. });
    let run_label = match &mode {
        DesktopXrMode::Clear { .. } => "clear smoke",
        DesktopXrMode::Mclone { .. } => "mclone smoke",
        DesktopXrMode::Real { .. } => "run",
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
    let controller_preferences = match &mode {
        DesktopXrMode::Clear { .. } => mclone_input::ControllerInputPreferences::default(),
        DesktopXrMode::Mclone { options } | DesktopXrMode::Real { options, .. } => {
            match mclone_app_runtime::input_preferences::load_native_input_preferences(
                options.scene.world_root.as_deref(),
            ) {
                Ok(preferences) => preferences.controller,
                Err(error) => {
                    println!("desktop XR input preferences unavailable: {error:#}");
                    mclone_input::ControllerInputPreferences::default()
                }
            }
        }
    };
    let mut mclone = match mode {
        DesktopXrMode::Clear { .. } => None,
        DesktopXrMode::Mclone { options } => Some(
            create_mclone_terrain_state(
                &graphics.device,
                &graphics.queue,
                options,
                WindowStartIntent::InWorld,
            )
            .context("initialize mclone XR terrain state")?,
        ),
        DesktopXrMode::Real {
            options,
            start_intent,
            ..
        } => Some(
            create_mclone_terrain_state(&graphics.device, &graphics.queue, options, start_intent)
                .context("initialize mclone XR terrain state")?,
        ),
    };
    if let Some(mclone) = mclone.as_mut() {
        mclone.set_display_refresh_hz(display_refresh.current_rate);
    }

    // Real `--desktop-xr` run: give the operator a desktop presence and a
    // non-headset quit. App-local winit glue, status surface only (no mirror /
    // per-view path — see tactical 164 Slice 3 and the XR render-path guardrail).
    let mut companion = if want_window {
        match CompanionWindow::spawn("Desktop XR — connecting to headset…") {
            Ok(window) => Some(window),
            Err(err) => {
                println!("desktop XR companion window disabled: {err:#}");
                None
            }
        }
    } else {
        None
    };
    let mut controller_actions = OpenXrControllerActions::create_with_binding_logger(
        graphics.session.instance(),
        &graphics.session,
        |profile, err| println!("OpenXR binding suggestion unavailable for {profile}: {err:?}"),
    )
    .context("initialize OpenXR controller actions")?;
    controller_actions.apply_controller_preferences(&controller_preferences);
    println!(
        "OpenXR controller actions: requested binding profiles=simple_controller, oculus_touch, valve_index, htc_vive, microsoft_motion_controller"
    );
    let ordinary_gamepad = match crate::desktop_gamepad::DesktopGamepadCollector::new() {
        Ok(collector) => Some(collector),
        Err(error) => {
            println!("desktop XR ordinary gamepad collector unavailable: {error:#}");
            None
        }
    };
    let policy = mclone_xr_host::OpenXrFrameLoopPolicy {
        view_type: VIEW_TYPE,
        environment_blend_mode,
        frame_limit: frame_limit.map(u64::from),
        session_ready_timeout: frame_limit.map(|_| SESSION_READY_TIMEOUT),
        submitted_frame_progress_timeout: frame_limit.map(|_| SUBMITTED_FRAME_PROGRESS_TIMEOUT),
        runtime_exit_is_error: frame_limit.is_some(),
        idle_poll_interval: mclone_xr_host::SESSION_IDLE_POLL_INTERVAL,
    };
    let mut handler = DesktopXrFrameLoop {
        device: &graphics.device,
        queue: &graphics.queue,
        stage: &stage,
        left_eye: &mut left_eye,
        right_eye: &mut right_eye,
        mclone: &mut mclone,
        controller_actions: &mut controller_actions,
        ordinary_gamepad,
        ordinary_gamepad_input: XrControllerInputRouter::with_controller_preferences(
            &controller_preferences,
        ),
        controller_summary: XrControllerInputSummary::default(),
        frame_pipeline_accountant: FramePipelineAccountant::new_live(
            xr_frame_pipeline_accounting_config(display_refresh.current_rate.map(f64::from)),
        ),
        companion: &mut companion,
        companion_running_announced: false,
        window_requested_exit: false,
    };
    let mut driver = mclone_xr_host::OpenXrFrameDriver::new(
        &graphics.session,
        &mut graphics.frame_wait,
        &mut graphics.frame_stream,
        policy,
    );
    let outcome = driver.run(&mut handler)?;

    println!(
        "OpenXR frames submitted: submitted={} runtime_frames={} skipped={}",
        outcome.stats.submitted_frames, outcome.stats.runtime_frames, outcome.stats.skipped_frames
    );
    handler.controller_summary.print_summary();
    if let Some(mclone) = handler.mclone.as_ref() {
        print_mclone_summary(mclone);
    }
    // Headset-menu EXITING already left the runtime stopped; a window close or a
    // completed frame budget still needs an explicit app-driven EXITING handshake.
    if outcome.exit != mclone_xr_host::OpenXrRunExit::Runtime {
        if handler.window_requested_exit {
            println!("desktop XR shutting down: companion window closed");
        }
        driver
            .request_exit_and_drain(Duration::from_secs(2), |event| println!("{event}"))
            .context("shut down OpenXR session after desktop XR run")?;
    }
    drop(driver);
    drop(handler);
    // Real desktop XR runs and the bounded smokes both exit the process right
    // after this returns, so a graceful shutdown followed by process teardown is
    // the defined quit path. We still `mem::forget` the XR object graph rather
    // than dropping it: on the runtimes this path drives — the macOS WiVRn/Monado
    // self-port and Windows VDXR — destroying session-owned graphics handles
    // after an app-requested EXITING transition faults inside the runtime. Real
    // per-handle teardown stays deferred until a runtime is validated safe to
    // drop post-EXITING; process exit reclaims the memory regardless. The
    // companion window is NOT forgotten — it drops normally below.
    std::mem::forget(left_eye);
    std::mem::forget(right_eye);
    std::mem::forget(stage);
    std::mem::forget(controller_actions);
    std::mem::forget(graphics);
    if let Some(mclone) = mclone {
        std::mem::forget(mclone);
    }
    drop(companion);
    println!("desktop OpenXR {run_label} complete: frames={frame_limit_label}");
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn create_mclone_terrain_state(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    options: XrMcloneSmokeOptions,
    start_intent: WindowStartIntent,
) -> Result<DesktopXrSceneHost> {
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
    let asset_pack_world_root = scene.world_root.clone();
    let startup_view_pose = options.view_pose.map(|view_pose| XrStartupViewPose {
        position: view_pose.position,
        yaw_degrees: view_pose.yaw_degrees,
    });
    let entry = start_intent.resolve(&options.scene);
    let mut state = McloneSceneHost::start_native_without_session(
        device,
        queue,
        XR_COLOR_FORMAT,
        mclone_app_runtime::monotonic::system_monotonic_clock(),
        scene,
        options.render_options,
        load_textured_mesh_assets_from_source(&asset_source)?,
        actor_assets.atlas.clone(),
        actor_assets.figures.clone(),
        &asset_source,
        xr_native_client_experience_profile(),
        startup_view_pose,
    )
    .context("initialize shared session-free mclone XR scene")?;
    state.set_audio_output(audio.map_or_else(
        || mclone_audio::AudioOutputCapability::Unavailable,
        mclone_audio::AudioOutputCapability::available,
    ));
    state.set_teleport_preview_capability(mclone_client::native_teleport_preview_capability());
    state.set_frame_host_kind(FrameHostKind::DesktopXrOpenXr);
    state.set_session_runtime_factory(|endpoint, scene, mesh_assets| {
        let desktop_scene = desktop_scene_options_for_xr_remote(&endpoint, &scene);
        let runtime = native_window_scene_runtime_with_mesh_assets(&desktop_scene, mesh_assets)?;
        NativeSessionServices::from_active_runtime(
            SessionStartRequest::JoinRemote { endpoint },
            runtime,
        )
    });
    crate::desktop_scene_host::configure_desktop_asset_pack_sources(
        &mut state,
        asset_pack_world_root.as_deref(),
    )?;
    let mut entry_controller = ClientEntryController::new(entry);
    let entry_effect = entry_controller
        .update_host(ClientHostAvailability {
            bootstrapped: true,
            foreground: true,
            presentation_available: true,
        })
        .expect("ready desktop XR host dispatches its entry exactly once");
    println!(
        "desktop XR client entry source={} intent={}",
        entry_controller.resolution().source.label(),
        entry_controller.resolution().intent.label(),
    );
    match entry_effect {
        ClientEntryEffect::EnterTitle { status } => {
            state.set_client_entry_status(status);
        }
        ClientEntryEffect::StartSession(request) => {
            state.start_session_for_request(device, queue, request)?;
        }
        ClientEntryEffect::LaunchScenario(intent) => {
            state.begin_lobby_launch(intent)?;
        }
    }
    Ok(state)
}

#[cfg(not(target_os = "android"))]
fn xr_scene_options_from_desktop_scene(
    scene: &SceneOptions,
    underwater_mode: crate::cli::XrUnderwaterMode,
    debug_ui_screen: Option<CliXrDebugUiScreen>,
) -> Result<McloneSceneHostOptions> {
    let mut options = McloneSceneHostOptions::from_startup_scene(
        scene.startup_for_host(),
        scene.world_root.clone(),
        scene.world_dir.clone(),
    );
    options.adaptive_chunk_publication_budget = scene.adaptive_chunk_publication_budget;
    options.underwater_detection_mode = xr_underwater_mode_from_desktop(underwater_mode);
    options.debug_ui_screen = debug_ui_screen.map(xr_debug_ui_screen_from_desktop);
    options.validated()
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
fn desktop_scene_options_for_xr_remote(
    endpoint: &RemoteSessionEndpoint,
    scene: &McloneSceneHostOptions,
) -> SceneOptions {
    let startup = mclone_app_runtime::startup_args::StartupSceneOptions {
        remote_addr: Some(endpoint.address.clone()),
        ..scene.startup.clone()
    };
    SceneOptions {
        startup,
        simulation_cadence: Default::default(),
        first_person_player_visible: false,
        world_root: scene.world_root.clone(),
        world_dir: None,
        ..SceneOptions::default()
    }
}

#[cfg(not(target_os = "android"))]
fn print_mclone_summary(mclone: &DesktopXrSceneHost) {
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
fn render_desktop_xr_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &mut mclone_xr_host::OpenXrRenderFrame<'_, platform_graphics::AppGraphics>,
    stage: &xr::Space,
    left_eye: &mut platform_graphics::OpenXrEyeState,
    right_eye: &mut platform_graphics::OpenXrEyeState,
    mclone: &mut DesktopXrSceneHost,
    input: &XrInputFrame,
) -> Result<XrTerrainFrameSummary> {
    let stereo_views =
        mclone_xr_host::locate_stereo_views(frame.session(), stage, frame.predicted_display_time())
            .context("locate OpenXR stereo views for mclone frame")?;
    let scene_views = [
        mclone_xr_host::xr_view_from_openxr(&stereo_views.left)?,
        mclone_xr_host::xr_view_from_openxr(&stereo_views.right)?,
    ];
    mclone.apply_frame_locomotion(input, scene_views, None)?;

    let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
    let right_target = match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image")
    {
        Ok(target) => target,
        Err(err) => {
            let _ = left_target.release();
            return Err(err);
        }
    };

    let render_result = mclone.render_xr_scene_frame(
        device,
        queue,
        scene_views,
        [scene_views[0].fov, scene_views[1].fov],
        false,
        None,
        XrSceneFrameTarget::PerEye {
            left: XrTerrainEyeTarget {
                color_view: left_target.color_view(),
                depth: &left_target.eye().depth,
                size: [left_target.eye().width, left_target.eye().height],
            },
            right: XrTerrainEyeTarget {
                color_view: right_target.color_view(),
                depth: &right_target.eye().depth,
                size: [right_target.eye().width, right_target.eye().height],
            },
        },
    );
    let left_release_result = left_target.release();
    let right_release_result = right_target.release();
    let frame_summary = render_result?;
    left_release_result?;
    right_release_result?;

    frame.submit_stereo_projection(stage, stereo_views, left_eye, right_eye)?;
    Ok(frame_summary)
}

#[cfg(not(target_os = "android"))]
fn render_clear_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &mut mclone_xr_host::OpenXrRenderFrame<'_, platform_graphics::AppGraphics>,
    stage: &xr::Space,
    left_eye: &mut platform_graphics::OpenXrEyeState,
    right_eye: &mut platform_graphics::OpenXrEyeState,
) -> Result<()> {
    let stereo_views = mclone_xr_host::locate_stereo_views(
        frame.session(),
        stage,
        frame.predicted_display_time(),
    )?;

    let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
    let right_target = match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image")
    {
        Ok(target) => target,
        Err(err) => {
            let _ = left_target.release();
            return Err(err);
        }
    };

    let clear_result = clear_stereo_targets(device, queue, &left_target, &right_target);
    let left_release_result = left_target.release();
    let right_release_result = right_target.release();
    clear_result?;
    left_release_result?;
    right_release_result?;

    frame.submit_stereo_projection(stage, stereo_views, left_eye, right_eye)
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
