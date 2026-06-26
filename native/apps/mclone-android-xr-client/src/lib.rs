#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod graphics_vulkan;

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CStr, CString, c_char, c_int};
    use std::sync::Once;
    use std::time::Duration;

    use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
    use anyhow::{Context, Result, bail};
    use mclone_xr_host::{
        OpenXrHostEvent, OpenXrPollStatus, PRIMARY_STEREO_VIEW_TYPE, XrFrameStats,
    };
    use openxr as xr;

    use super::graphics_vulkan;

    type AcquiredEyeTarget<'a> = mclone_xr_host::XrAcquiredEyeTarget<
        'a,
        graphics_vulkan::AppGraphics,
        graphics_vulkan::OpenXrEyeState,
    >;

    const LOG_TAG: &str = "mclone_android_xr";
    const STARTUP_ARGV_INTENT_EXTRA: &str = "mclone.startup.argv";
    const XR_VIEW_POSE_PROPERTY: &str = "debug.mclone.xr_view_pose";
    const ANDROID_PROPERTY_VALUE_MAX: usize = 92;
    const VIEW_TYPE: xr::ViewConfigurationType = PRIMARY_STEREO_VIEW_TYPE;
    const XR_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    const XR_SAMPLE_COUNT: u32 = 1;
    const SESSION_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(25);

    #[allow(unsafe_code)]
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_print(
            priority: c_int,
            tag: *const c_char,
            format: *const c_char,
            ...
        ) -> c_int;

        fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
    }

    #[allow(unsafe_code)]
    fn android_log(priority: c_int, message: impl AsRef<str>) {
        let tag = CString::new(LOG_TAG).expect("static log tag contains no nul");
        let format = c"%s";
        let message = CString::new(message.as_ref().replace('\0', "\\0"))
            .expect("nul bytes were escaped before logging");
        unsafe {
            __android_log_print(priority, tag.as_ptr(), format.as_ptr(), message.as_ptr());
        }
    }

    struct AndroidLogger;

    impl log::Log for AndroidLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let priority = match record.level() {
                log::Level::Error => 6,
                log::Level::Warn => 5,
                log::Level::Info => 4,
                log::Level::Debug => 3,
                log::Level::Trace => 2,
            };
            android_log(priority, format!("{}: {}", record.target(), record.args()));
        }

        fn flush(&self) {}
    }

    static LOGGER: AndroidLogger = AndroidLogger;
    static INIT_LOGGER: Once = Once::new();

    fn init_android_logger() {
        INIT_LOGGER.call_once(|| {
            if log::set_logger(&LOGGER).is_ok() {
                log::set_max_level(log::LevelFilter::Info);
            }
        });
    }

    #[allow(unsafe_code)]
    fn configure_android_asset_root(app: &AndroidApp) {
        if let Some(existing) = std::env::var_os("MCLONE_ANDROID_ASSET_ROOT") {
            log::info!(
                "Android XR preserving MCLONE_ANDROID_ASSET_ROOT={}",
                std::path::PathBuf::from(existing).display()
            );
            return;
        }

        let Some(path) = app
            .external_data_path()
            .or_else(|| app.internal_data_path())
        else {
            log::warn!(
                "Android XR could not resolve an app data path for MCLONE_ANDROID_ASSET_ROOT"
            );
            return;
        };

        unsafe {
            std::env::set_var("MCLONE_ANDROID_ASSET_ROOT", &path);
        }
        log::info!(
            "Android XR MCLONE_ANDROID_ASSET_ROOT configured from app data path: {}",
            path.display()
        );
    }

    #[allow(unsafe_code)]
    fn android_property(name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let mut value = [0 as c_char; ANDROID_PROPERTY_VALUE_MAX];
        let len = unsafe { __system_property_get(name.as_ptr(), value.as_mut_ptr()) };
        if len <= 0 {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(value.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    #[allow(unsafe_code)]
    fn android_startup_argv_json(app: &AndroidApp) -> Result<Option<String>> {
        let vm = app.vm_as_ptr() as *mut jni::sys::JavaVM;
        let activity = app.activity_as_ptr() as jni::sys::jobject;
        if vm.is_null() || activity.is_null() {
            log::warn!("Android XR startup argv intent extra unavailable: null JVM or Activity");
            return Ok(None);
        }

        let vm = unsafe { jni::JavaVM::from_raw(vm) };
        vm.attach_current_thread(|env| {
            let activity = unsafe { jni::objects::JObject::from_raw(env, activity) };
            let object = env
                .call_method(
                    &activity,
                    jni::jni_str!("getMcloneStartupArgvJson"),
                    jni::jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .into_object()?;
            if object.is_null() {
                return Ok(None);
            }
            let string = jni::objects::JString::cast_local(env, object)?;
            string.try_to_string(env).map(Some)
        })
        .with_context(|| {
            format!("failed to read Android intent extra `{STARTUP_ARGV_INTENT_EXTRA}`")
        })
    }

    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        init_android_logger();
        configure_android_asset_root(&app);
        log::info!("Mclone Android XR package starting");

        let startup_argv = match android_startup_argv_json(&app) {
            Ok(value) => value,
            Err(error) => {
                log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
                return;
            }
        };
        match startup_argv.as_deref() {
            Some(json) if !json.trim().is_empty() => {
                log::info!("Android XR startup argv from {STARTUP_ARGV_INTENT_EXTRA}: {json}");
            }
            _ => {
                log::info!("Android XR startup argv from {STARTUP_ARGV_INTENT_EXTRA}: <none>");
            }
        }

        match android_property(XR_VIEW_POSE_PROPERTY) {
            Some(value) if !value.trim().is_empty() && value.trim() != "0" => {
                log::info!("Android XR startup view pose from {XR_VIEW_POSE_PROPERTY}: {value}");
            }
            _ => {
                log::info!("Android XR startup view pose from {XR_VIEW_POSE_PROPERTY}: <default>");
            }
        }

        log::info!("MCLONE_ANDROID_XR_PACKAGE_READY");
        if let Err(error) = run_android_openxr_clear(&app) {
            log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
        }
    }

    #[allow(unsafe_code)]
    fn run_android_openxr_clear(app: &AndroidApp) -> Result<()> {
        wait_for_android_resume(app)?;
        let entry = unsafe { xr::Entry::load().context("load OpenXR loader")? };
        entry
            .initialize_android_loader()
            .context("initialize Android OpenXR loader")?;
        log::info!("Android OpenXR loader initialized");

        let available = entry
            .enumerate_extensions()
            .context("enumerate OpenXR instance extensions")?;
        let mut enabled_extensions = xr::ExtensionSet::default();
        if !available.khr_android_create_instance {
            bail!("OpenXR runtime does not support XR_KHR_android_create_instance");
        }
        if !available.khr_vulkan_enable2 {
            bail!("OpenXR runtime does not support XR_KHR_vulkan_enable2");
        }
        enabled_extensions.khr_android_create_instance = true;
        enabled_extensions.khr_vulkan_enable2 = true;
        log::info!(
            "OpenXR extensions: android_create_instance=true vulkan_enable2=true fb_passthrough={} fb_alpha_blend={} fb_display_refresh_rate={} fb_swapchain_update_state={} fb_foveation={} fb_foveation_configuration={} fb_foveation_vulkan={} fb_render_model={} ext_hand_tracking={} fb_hand_tracking_mesh={} fb_hand_tracking_aim={} meta_virtual_keyboard={} fb_spatial_entity={} fb_spatial_entity_query={} fb_scene={} fb_scene_capture={} fb_spatial_entity_container={} meta_spatial_entity_mesh={} fb_body_tracking={} meta_body_tracking_full_body={} ext_debug_utils={}",
            available.fb_passthrough,
            available.fb_composition_layer_alpha_blend,
            available.fb_display_refresh_rate,
            available.fb_swapchain_update_state,
            available.fb_foveation,
            available.fb_foveation_configuration,
            available.fb_foveation_vulkan,
            available.fb_render_model,
            available.ext_hand_tracking,
            available.fb_hand_tracking_mesh,
            available.fb_hand_tracking_aim,
            available.meta_virtual_keyboard,
            available.fb_spatial_entity,
            available.fb_spatial_entity_query,
            available.fb_scene,
            available.fb_scene_capture,
            available.fb_spatial_entity_container,
            available.meta_spatial_entity_mesh,
            available.fb_body_tracking,
            available.meta_body_tracking_full_body,
            available.ext_debug_utils,
        );
        if available.fb_passthrough {
            enabled_extensions.fb_passthrough = true;
            log::info!("Enabling XR_FB_passthrough");
        }
        if available.fb_composition_layer_alpha_blend {
            enabled_extensions.fb_composition_layer_alpha_blend = true;
            log::info!("Enabling XR_FB_composition_layer_alpha_blend");
        }
        if available.fb_display_refresh_rate {
            enabled_extensions.fb_display_refresh_rate = true;
            log::info!("Enabling XR_FB_display_refresh_rate");
        }
        if available.fb_swapchain_update_state
            && available.fb_foveation
            && available.fb_foveation_configuration
            && available.fb_foveation_vulkan
        {
            enabled_extensions.fb_swapchain_update_state = true;
            enabled_extensions.fb_foveation = true;
            enabled_extensions.fb_foveation_configuration = true;
            enabled_extensions.fb_foveation_vulkan = true;
            log::info!("Enabling XR_FB_swapchain_update_state");
            log::info!("Enabling XR_FB_foveation");
            log::info!("Enabling XR_FB_foveation_configuration");
            log::info!("Enabling XR_FB_foveation_vulkan");
        }
        if available.fb_render_model {
            enabled_extensions.fb_render_model = true;
            log::info!("Enabling XR_FB_render_model");
        }
        if available.ext_hand_tracking {
            enabled_extensions.ext_hand_tracking = true;
            log::info!("Enabling XR_EXT_hand_tracking");
        }
        if available.fb_hand_tracking_mesh && available.ext_hand_tracking {
            enabled_extensions.fb_hand_tracking_mesh = true;
            log::info!("Enabling XR_FB_hand_tracking_mesh");
        }
        if available.fb_hand_tracking_aim && available.ext_hand_tracking {
            enabled_extensions.fb_hand_tracking_aim = true;
            log::info!("Enabling XR_FB_hand_tracking_aim");
        }
        if available.meta_virtual_keyboard {
            enabled_extensions.meta_virtual_keyboard = true;
            log::info!("Enabling XR_META_virtual_keyboard");
        }
        if available.fb_spatial_entity {
            enabled_extensions.fb_spatial_entity = true;
            log::info!("Enabling XR_FB_spatial_entity");
        }
        if available.fb_spatial_entity_query && available.fb_spatial_entity {
            enabled_extensions.fb_spatial_entity_query = true;
            log::info!("Enabling XR_FB_spatial_entity_query");
        }
        if available.fb_spatial_entity_container && available.fb_spatial_entity {
            enabled_extensions.fb_spatial_entity_container = true;
            log::info!("Enabling XR_FB_spatial_entity_container");
        }
        if available.fb_scene && available.fb_spatial_entity {
            enabled_extensions.fb_scene = true;
            log::info!("Enabling XR_FB_scene");
        }
        if available.fb_scene_capture {
            enabled_extensions.fb_scene_capture = true;
            log::info!("Enabling XR_FB_scene_capture");
        }
        if available.meta_spatial_entity_mesh && available.fb_spatial_entity {
            enabled_extensions.meta_spatial_entity_mesh = true;
            log::info!("Enabling XR_META_spatial_entity_mesh");
        }
        if available.fb_body_tracking {
            enabled_extensions.fb_body_tracking = true;
            log::info!("Enabling XR_FB_body_tracking");
        }
        if available.meta_body_tracking_full_body && available.fb_body_tracking {
            enabled_extensions.meta_body_tracking_full_body = true;
            log::info!("Enabling XR_META_body_tracking_full_body");
        }

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
        log::info!(
            "OpenXR runtime: {} v{}",
            properties.runtime_name,
            properties.runtime_version
        );

        let system = instance
            .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .context("locate OpenXR head-mounted display system")?;
        let system_properties = instance
            .system_properties(system)
            .context("query OpenXR system properties")?;
        log::info!(
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
        let environment_blend_mode = mclone_xr_host::selected_environment_blend_mode(&blend_modes);
        log::info!(
            "OpenXR environment blend modes: {}; selected={environment_blend_mode:?}",
            mclone_xr_host::format_debug_list(&blend_modes)
        );

        let view_configs = instance
            .enumerate_view_configuration_views(system, VIEW_TYPE)
            .context("enumerate OpenXR PRIMARY_STEREO view configuration")?;
        let stereo_config = mclone_xr_host::stereo_config(&view_configs)?;
        log::info!(
            "OpenXR stereo views: {}",
            mclone_xr_host::format_view_configurations(&view_configs)
        );

        let mut graphics = graphics_vulkan::create_graphics_session(&instance, system)
            .context("create OpenXR Vulkan graphics session")?;
        log::info!(
            "OpenXR Vulkan session: physical_device='{}' api={} queue_family={}",
            graphics.physical_device_name,
            graphics.physical_device_api_version,
            graphics.queue_family_index
        );
        let stage = mclone_xr_host::create_stage_reference_space(&graphics.session)?;
        log::info!("OpenXR reference space: STAGE");

        let (eye_width, eye_height) = stereo_config.primary_eye_size();
        let mut left_eye = graphics_vulkan::create_eye(
            &graphics.device,
            &graphics.session,
            eye_width,
            eye_height,
            XR_COLOR_FORMAT,
            XR_DEPTH_FORMAT,
            XR_SAMPLE_COUNT,
        )
        .context("create OpenXR left-eye color swapchain")?;
        let mut right_eye = graphics_vulkan::create_eye(
            &graphics.device,
            &graphics.session,
            eye_width,
            eye_height,
            XR_COLOR_FORMAT,
            XR_DEPTH_FORMAT,
            XR_SAMPLE_COUNT,
        )
        .context("create OpenXR right-eye color swapchain")?;
        log::info!(
            "OpenXR swapchains: color_format={XR_COLOR_FORMAT:?} depth_format={XR_DEPTH_FORMAT:?} eye={}x{} images={}/{}",
            eye_width,
            eye_height,
            left_eye.textures.len(),
            right_eye.textures.len()
        );
        log::info!("MCLONE_ANDROID_XR_SESSION_READY");

        run_clear_frame_loop(
            app,
            &mut graphics,
            &stage,
            environment_blend_mode,
            &mut left_eye,
            &mut right_eye,
        )
    }

    fn run_clear_frame_loop(
        app: &AndroidApp,
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        left_eye: &mut graphics_vulkan::OpenXrEyeState,
        right_eye: &mut graphics_vulkan::OpenXrEyeState,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();
        let mut logged_ready = false;

        loop {
            if !poll_android_events(app, Some(Duration::from_millis(0)))? {
                log::info!("Android activity destroyed; exiting OpenXR loop");
                return Ok(());
            }

            match mclone_xr_host::poll_openxr_events(
                &graphics.session,
                &mut event_storage,
                &mut session_running,
                VIEW_TYPE,
                log_openxr_host_event,
            )
            .context("poll OpenXR events")?
            {
                OpenXrPollStatus::Exit => {
                    log::info!(
                        "OpenXR session requested exit: submitted={} runtime_frames={} skipped={}",
                        frame_stats.submitted_frames,
                        frame_stats.runtime_frames,
                        frame_stats.skipped_frames
                    );
                    return Ok(());
                }
                OpenXrPollStatus::Idle if !session_running => {
                    if !poll_android_events(app, Some(SESSION_IDLE_POLL_INTERVAL))? {
                        log::info!("Android activity destroyed while waiting for OpenXR READY");
                        return Ok(());
                    }
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
                render_clear_frame(
                    graphics,
                    stage,
                    environment_blend_mode,
                    frame_state.predicted_display_time,
                    left_eye,
                    right_eye,
                )
                .map(|()| {
                    frame_stats.record_submitted_frame();
                    if !logged_ready {
                        logged_ready = true;
                        log::info!("MCLONE_ANDROID_XR_READY");
                    }
                })
            } else {
                mclone_xr_host::end_skipped_frame(
                    &mut graphics.frame_stream,
                    frame_state.predicted_display_time,
                    environment_blend_mode,
                    &mut frame_stats,
                )
            };

            if let Err(error) = frame_result {
                let _ = mclone_xr_host::end_frame_with_layers(
                    &mut graphics.frame_stream,
                    frame_state.predicted_display_time,
                    environment_blend_mode,
                    &[],
                );
                return Err(error);
            }

            if frame_stats.submitted_frames == 1 {
                log::info!(
                    "OpenXR clear frame submitted: submitted={} runtime_frames={} skipped={}",
                    frame_stats.submitted_frames,
                    frame_stats.runtime_frames,
                    frame_stats.skipped_frames
                );
            }
        }
    }

    fn log_openxr_host_event(event: OpenXrHostEvent) {
        match event {
            OpenXrHostEvent::SessionStateChanged(state) => {
                log::info!("OpenXR session state: {state:?}");
            }
            OpenXrHostEvent::InstanceLossPending => {
                log::info!("OpenXR instance loss pending");
            }
            OpenXrHostEvent::EventsLost(count) => {
                log::warn!("OpenXR events lost: {count}");
            }
        }
    }

    fn render_clear_frame(
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        predicted_display_time: xr::Time,
        left_eye: &mut graphics_vulkan::OpenXrEyeState,
        right_eye: &mut graphics_vulkan::OpenXrEyeState,
    ) -> Result<()> {
        let stereo_views =
            mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;

        let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
        let right_target =
            match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image") {
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

    fn acquire_eye_target(
        eye_state: &mut graphics_vulkan::OpenXrEyeState,
    ) -> Result<AcquiredEyeTarget<'_>> {
        mclone_xr_host::acquire_eye_target(eye_state)
    }

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
                depth_view: Some(&left_target.eye().depth_view),
            },
            mclone_xr_host::XrClearTarget {
                color_view: right_target.color_view(),
                depth_view: Some(&right_target.eye().depth_view),
            },
        )
    }

    fn wait_for_android_resume(app: &AndroidApp) -> Result<()> {
        log::info!("Waiting for Android activity resume and focus");
        let mut resumed = false;
        let mut focused = false;
        let mut has_window = app.native_window().is_some();
        for _ in 0..160 {
            let mut destroyed = false;
            app.poll_events(Some(Duration::from_millis(50)), |event| {
                if let PollEvent::Main(main) = event {
                    match main {
                        MainEvent::Start => log::info!("Android activity started"),
                        MainEvent::Resume { .. } => {
                            log::info!("Android activity resumed");
                            resumed = true;
                        }
                        MainEvent::InitWindow { .. } => {
                            log::info!("Android activity window initialized");
                            has_window = true;
                        }
                        MainEvent::TerminateWindow { .. } => {
                            log::info!("Android activity window terminated");
                            has_window = false;
                        }
                        MainEvent::GainedFocus => {
                            log::info!("Android activity gained focus");
                            focused = true;
                        }
                        MainEvent::LostFocus => {
                            log::info!("Android activity lost focus");
                            focused = false;
                        }
                        MainEvent::Pause => {
                            log::info!("Android activity paused");
                            resumed = false;
                        }
                        MainEvent::Stop => {
                            log::info!("Android activity stopped");
                            resumed = false;
                            focused = false;
                        }
                        MainEvent::Destroy => destroyed = true,
                        MainEvent::InputAvailable => drain_android_input_events(app),
                        _ => {}
                    }
                }
            });
            if destroyed {
                bail!("Android activity was destroyed before XR startup");
            }
            has_window |= app.native_window().is_some();
            if resumed && focused && has_window {
                log::info!("Android activity is resumed, focused, and windowed for OpenXR startup");
                return Ok(());
            }
        }
        bail!(
            "timed out waiting for Android activity readiness before XR startup (resumed={resumed}, focused={focused}, has_window={has_window})"
        );
    }

    fn poll_android_events(app: &AndroidApp, timeout: Option<Duration>) -> Result<bool> {
        let mut keep_running = true;
        app.poll_events(timeout, |event| {
            if let PollEvent::Main(main) = event {
                match main {
                    MainEvent::Resume { .. } => log::info!("Android activity resumed"),
                    MainEvent::InitWindow { .. } => {
                        log::info!("Android activity window initialized");
                    }
                    MainEvent::TerminateWindow { .. } => {
                        log::info!("Android activity window terminated");
                    }
                    MainEvent::GainedFocus => log::info!("Android activity gained focus"),
                    MainEvent::LostFocus => log::info!("Android activity lost focus"),
                    MainEvent::Pause => log::info!("Android activity paused"),
                    MainEvent::Stop => log::info!("Android activity stopped"),
                    MainEvent::Destroy => keep_running = false,
                    MainEvent::InputAvailable => drain_android_input_events(app),
                    _ => {}
                }
            }
        });
        Ok(keep_running)
    }

    fn drain_android_input_events(app: &AndroidApp) {
        if let Ok(mut iter) = app.input_events_iter() {
            while iter.next(|_| InputStatus::Handled) {}
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn host_placeholder() {}
