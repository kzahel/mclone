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
use openxr as xr;

use crate::cli::XrClearSmokeOptions;

#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
mod graphics_metal;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
mod graphics_vulkan;
#[cfg(all(not(target_os = "android"), target_vendor = "apple"))]
use graphics_metal as platform_graphics;
#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
use graphics_vulkan as platform_graphics;

#[cfg(target_os = "android")]
pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
    let _ = options;
    bail!("--xr-clear-smoke is a desktop OpenXR smoke; Android XR packaging is a later target")
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
#[derive(Debug)]
struct OpenXrRuntimeManifest {
    runtime_library_path: PathBuf,
}

#[cfg(not(target_os = "android"))]
pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
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

    create_graphics_session_probe(
        &instance,
        system,
        &views,
        environment_blend_mode,
        options.frames,
    )?;

    println!(
        "desktop OpenXR clear smoke complete: frames={}",
        options.frames
    );

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
    frames: u32,
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
    run_clear_frame_smoke(graphics, stage, views, environment_blend_mode, frames)?;
    Ok(())
}

#[cfg(all(not(target_os = "android"), not(target_vendor = "apple")))]
fn create_graphics_session_probe(
    instance: &xr::Instance,
    system: xr::SystemId,
    views: &[xr::ViewConfigurationView],
    environment_blend_mode: xr::EnvironmentBlendMode,
    frames: u32,
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
    run_clear_frame_smoke(graphics, stage, views, environment_blend_mode, frames)?;
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
fn run_clear_frame_smoke(
    mut graphics: platform_graphics::GraphicsSession,
    stage: xr::Space,
    views: &[xr::ViewConfigurationView],
    environment_blend_mode: xr::EnvironmentBlendMode,
    frames: u32,
) -> Result<()> {
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
            render_clear_frame(
                &mut graphics,
                &stage,
                environment_blend_mode,
                frame_state.predicted_display_time,
                &mut left_eye,
                &mut right_eye,
            )
            .map(|()| {
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
        "OpenXR clear frames submitted: submitted={submitted_frames} runtime_frames={runtime_frames} skipped={skipped_frames}"
    );
    shutdown_openxr_session(&mut graphics, &mut event_storage, &mut session_running)
        .context("shut down OpenXR session after clear smoke")?;
    // Mirrors Playbox's desktop OpenXR shutdown shape. Some runtimes fault while
    // destroying session-owned graphics handles after an app-requested EXITING
    // transition; this smoke is process-bounded, so keep the XR object graph
    // alive after the clean shutdown instead of touching a stopped runtime.
    std::mem::forget(left_eye);
    std::mem::forget(right_eye);
    std::mem::forget(stage);
    std::mem::forget(graphics);
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
        view: &target.eye_state.depth_view,
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
