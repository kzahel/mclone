#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod graphics_vulkan;

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CStr, CString, c_char, c_int};
    use std::sync::Once;
    use std::time::{Duration, Instant};

    use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
    use anyhow::{Context, Result, bail};
    use mclone_app_runtime::host_mode::{RemoteDedicatedServerSession, SingleViewHostOptions};
    use mclone_app_runtime::local_single_view::{
        LocalSingleViewSceneOptions, NativeSingleViewSessionRuntime,
    };
    use mclone_app_runtime::render_assets::{
        ActorTextureAssets, TexturedMeshAssets, load_actor_texture_assets_from_asset_source,
        load_asset_source, load_textured_mesh_assets_from_source,
    };
    use mclone_app_runtime::session::{RemoteSessionEndpoint, SessionStartRequest};
    use mclone_assets::AssetSourceChain;
    use mclone_audio::{AudioEngine, AudioSettings};
    use mclone_net::NativeClientSession;
    use mclone_protocol::{ClientCommand, ServerUpdate};
    use mclone_render::chunk::TexturedSectionRenderOptions;
    use mclone_render_session::EngineCameraSnapshot;
    use mclone_xr_host::{
        OpenXrControllerActions, OpenXrHostEvent, OpenXrPollStatus, PRIMARY_STEREO_VIEW_TYPE,
        XrControllerSnapshot, XrFrameStats,
    };
    use mclone_xr_scene::{
        XrMcloneTerrainState, XrSceneOptions, XrStartupViewPose, XrTerrainEyeTarget,
    };
    use openxr as xr;

    use super::graphics_vulkan;

    type AcquiredEyeTarget<'a> = mclone_xr_host::XrAcquiredEyeTarget<
        'a,
        graphics_vulkan::AppGraphics,
        graphics_vulkan::OpenXrEyeState,
    >;
    type AndroidXrSceneRuntime = NativeSingleViewSessionRuntime<AndroidXrRemoteServerSession>;
    type AndroidXrTerrainState = XrMcloneTerrainState<AndroidXrRemoteServerSession>;

    const LOG_TAG: &str = "mclone_android_xr";
    const STARTUP_ARGV_INTENT_EXTRA: &str = "mclone.startup.argv";
    const REMOTE_ADDR_PROPERTY: &str = "debug.mclone.remote_addr";
    const XR_VIEW_POSE_PROPERTY: &str = "debug.mclone.xr_view_pose";
    const ANDROID_ASSET_ROOT_ENV: &str = "MCLONE_ANDROID_ASSET_ROOT";
    const ANDROID_PROPERTY_VALUE_MAX: usize = 92;
    const VIEW_TYPE: xr::ViewConfigurationType = PRIMARY_STEREO_VIEW_TYPE;
    const XR_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    const XR_SAMPLE_COUNT: u32 = 1;
    const SESSION_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(25);
    const ANDROID_XR_SESSION_SMOKE_SEED: i64 = 246_813_579;
    const ANDROID_XR_PERF_FALLBACK_TARGET_HZ: f64 = 72.0;
    const ANDROID_XR_PERF_DEFAULT_FLIGHT_SPEED_BLOCKS_PER_SECOND: f64 = 4.3;

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
        if let Some(existing) = std::env::var_os(ANDROID_ASSET_ROOT_ENV) {
            log::info!(
                "Android XR preserving {ANDROID_ASSET_ROOT_ENV}={}",
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
            std::env::set_var(ANDROID_ASSET_ROOT_ENV, &path);
        }
        log::info!(
            "Android XR {ANDROID_ASSET_ROOT_ENV} configured from app data path: {}",
            path.display()
        );
    }

    struct AndroidXrRuntimeAssets {
        mesh_assets: TexturedMeshAssets,
        actor_assets: ActorTextureAssets,
        asset_source: AssetSourceChain,
    }

    impl AndroidXrRuntimeAssets {
        fn terrain_atlas_size(&self) -> (u32, u32) {
            (self.mesh_assets.atlas.width, self.mesh_assets.atlas.height)
        }

        fn actor_atlas_size(&self) -> (u32, u32) {
            (
                self.actor_assets.atlas.width,
                self.actor_assets.atlas.height,
            )
        }
    }

    fn load_android_xr_runtime_assets() -> Result<AndroidXrRuntimeAssets> {
        let asset_root = std::env::var_os(ANDROID_ASSET_ROOT_ENV)
            .map(std::path::PathBuf::from)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<unset>".to_owned());
        let load_start = Instant::now();
        let source = load_asset_source().with_context(|| {
            format!("load Android XR asset source from {ANDROID_ASSET_ROOT_ENV}={asset_root}")
        })?;
        let mesh_assets = load_textured_mesh_assets_from_source(&source)
            .context("load Android XR textured mesh assets")?;
        let actor_assets = load_actor_texture_assets_from_asset_source(&source)
            .context("load Android XR actor texture assets")?;
        let assets = AndroidXrRuntimeAssets {
            mesh_assets,
            actor_assets,
            asset_source: source,
        };
        let (terrain_atlas_width, terrain_atlas_height) = assets.terrain_atlas_size();
        let (actor_atlas_width, actor_atlas_height) = assets.actor_atlas_size();
        log::info!(
            "Android XR runtime assets: root={} terrain_atlas={}x{} actor_atlas={}x{} load_ms={:.3}",
            asset_root,
            terrain_atlas_width,
            terrain_atlas_height,
            actor_atlas_width,
            actor_atlas_height,
            load_start.elapsed().as_secs_f64() * 1000.0
        );
        log::info!("MCLONE_ANDROID_XR_ASSETS_READY");
        Ok(assets)
    }

    #[derive(Clone, Debug, PartialEq)]
    struct AndroidXrStartupOptions {
        scene: XrSceneOptions,
        remote_addr: Option<String>,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
    }

    impl Default for AndroidXrStartupOptions {
        fn default() -> Self {
            Self {
                scene: XrSceneOptions::default(),
                remote_addr: None,
                session_smoke: None,
                perf_seconds: None,
                perf_flight: None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct AndroidXrPerfFlight {
        speed_blocks_per_second: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum AndroidXrSessionSmoke {
        NewWorld,
    }

    impl AndroidXrSessionSmoke {
        fn label(self) -> &'static str {
            match self {
                Self::NewWorld => "new-world",
            }
        }
    }

    fn parse_android_xr_startup_options(
        startup_argv_json: Option<&str>,
    ) -> Result<AndroidXrStartupOptions> {
        let Some(json) = startup_argv_json.filter(|json| !json.trim().is_empty()) else {
            return Ok(AndroidXrStartupOptions::default());
        };
        let argv = serde_json::from_str::<Vec<String>>(json)
            .context("parse Android XR startup argv JSON")?;
        let mut options = AndroidXrStartupOptions::default();
        let mut index = 0;
        while index < argv.len() {
            match argv[index].as_str() {
                "--seed" => {
                    options.scene.seed = parse_next(&argv, &mut index, "--seed")?;
                }
                "--chunk-x" => {
                    options.scene.chunk_x = parse_next(&argv, &mut index, "--chunk-x")?;
                }
                "--chunk-z" => {
                    options.scene.chunk_z = parse_next(&argv, &mut index, "--chunk-z")?;
                }
                "--render-distance" => {
                    options.scene.render_distance =
                        parse_next(&argv, &mut index, "--render-distance")?;
                }
                "--day-time" => {
                    options.scene.day_time_override =
                        Some(parse_next(&argv, &mut index, "--day-time")?);
                }
                "--remote-addr" => {
                    options.remote_addr = parse_remote_addr_arg(parse_next_string(
                        &argv,
                        &mut index,
                        "--remote-addr",
                    )?);
                }
                "--session-smoke" => {
                    options.session_smoke = Some(parse_session_smoke_arg(parse_next_string(
                        &argv,
                        &mut index,
                        "--session-smoke",
                    )?)?);
                }
                "--perf-seconds" => {
                    let seconds = parse_next::<u64>(&argv, &mut index, "--perf-seconds")?;
                    if seconds == 0 {
                        bail!("--perf-seconds must be greater than zero");
                    }
                    options.perf_seconds = Some(seconds);
                }
                "--perf-flight" => {
                    if options.perf_flight.is_none() {
                        options.perf_flight = Some(AndroidXrPerfFlight {
                            speed_blocks_per_second:
                                ANDROID_XR_PERF_DEFAULT_FLIGHT_SPEED_BLOCKS_PER_SECOND,
                        });
                    }
                }
                "--perf-flight-speed" => {
                    let speed = parse_next::<f64>(&argv, &mut index, "--perf-flight-speed")?;
                    options.perf_flight = Some(AndroidXrPerfFlight {
                        speed_blocks_per_second: validate_perf_flight_speed(speed)?,
                    });
                }
                "--freeze-time" => {}
                unknown => bail!("unsupported Android XR startup argument `{unknown}`"),
            }
            if argv[index] == "--freeze-time" {
                options.scene.freeze_time = true;
            }
            index += 1;
        }
        if options.perf_flight.is_some() && options.perf_seconds.is_none() {
            bail!("--perf-flight requires --perf-seconds");
        }
        options.scene = options.scene.validated()?;
        Ok(options)
    }

    fn validate_perf_flight_speed(speed_blocks_per_second: f64) -> Result<f64> {
        if !speed_blocks_per_second.is_finite() || speed_blocks_per_second <= 0.0 {
            bail!("--perf-flight-speed must be a finite positive number");
        }
        Ok(speed_blocks_per_second)
    }

    fn parse_session_smoke_arg(value: String) -> Result<AndroidXrSessionSmoke> {
        match value.trim() {
            "new-world" => Ok(AndroidXrSessionSmoke::NewWorld),
            value => bail!("unsupported Android XR session smoke `{value}`"),
        }
    }

    fn parse_next_string(argv: &[String], index: &mut usize, flag: &str) -> Result<String> {
        *index += 1;
        argv.get(*index)
            .cloned()
            .with_context(|| format!("{flag} requires a value"))
    }

    fn parse_next<T>(argv: &[String], index: &mut usize, flag: &str) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        *index += 1;
        let value = argv
            .get(*index)
            .with_context(|| format!("{flag} requires a value"))?;
        value
            .parse::<T>()
            .with_context(|| format!("{flag} has invalid value `{value}`"))
    }

    fn parse_remote_addr_arg(value: String) -> Option<String> {
        let value = value.trim();
        if value.is_empty() || matches!(value, "default" | "off" | "none" | "false" | "0") {
            None
        } else {
            Some(value.to_owned())
        }
    }

    fn parse_android_xr_startup_view_pose(
        value: Option<&str>,
    ) -> Result<Option<XrStartupViewPose>> {
        let Some(value) = value.map(str::trim) else {
            return Ok(None);
        };
        if matches!(
            value,
            "" | "default" | "off" | "none" | "false" | "0" | "disabled"
        ) {
            return Ok(None);
        }
        let parts = value
            .split([',', ':', ';', ' '])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() != 4 {
            bail!("{XR_VIEW_POSE_PROPERTY} expects X,Y,Z,YAW_DEGREES");
        }
        let mut numbers = [0.0; 4];
        for (index, part) in parts.into_iter().enumerate() {
            let number = part
                .parse::<f32>()
                .with_context(|| format!("invalid {XR_VIEW_POSE_PROPERTY} component `{part}`"))?;
            if !number.is_finite() {
                bail!("{XR_VIEW_POSE_PROPERTY} component `{part}` must be finite");
            }
            numbers[index] = number;
        }
        Ok(Some(XrStartupViewPose {
            position: [numbers[0], numbers[1], numbers[2]],
            yaw_degrees: numbers[3],
        }))
    }

    fn android_remote_addr() -> Option<String> {
        android_property(REMOTE_ADDR_PROPERTY)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
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

        let startup_view_pose_property = android_property(XR_VIEW_POSE_PROPERTY);
        match startup_view_pose_property.as_deref() {
            Some(value) if !value.trim().is_empty() && value.trim() != "0" => {
                log::info!("Android XR startup view pose from {XR_VIEW_POSE_PROPERTY}: {value}");
            }
            _ => {
                log::info!("Android XR startup view pose from {XR_VIEW_POSE_PROPERTY}: <default>");
            }
        }
        let startup_options = match parse_android_xr_startup_options(startup_argv.as_deref()) {
            Ok(options) => options,
            Err(error) => {
                log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
                return;
            }
        };
        let scene_options = startup_options.scene;
        let startup_view_pose =
            match parse_android_xr_startup_view_pose(startup_view_pose_property.as_deref()) {
                Ok(value) => value,
                Err(error) => {
                    log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
                    return;
                }
            };
        let legacy_remote_addr = android_remote_addr();
        let remote_addr = startup_options
            .remote_addr
            .clone()
            .or_else(|| legacy_remote_addr.clone());
        if let Some(remote_addr) = startup_options.remote_addr.as_deref() {
            log::info!(
                "Android XR remote dedicated address from {STARTUP_ARGV_INTENT_EXTRA}: {remote_addr}"
            );
        } else if let Some(remote_addr) = legacy_remote_addr.as_deref() {
            log::info!(
                "Android XR remote dedicated address from legacy {REMOTE_ADDR_PROPERTY}: {remote_addr}"
            );
        } else {
            log::info!("Android XR remote dedicated address: <none>");
        }
        if let Some(smoke) = startup_options.session_smoke {
            log::info!("Android XR session smoke: {}", smoke.label());
        } else {
            log::info!("Android XR session smoke: <none>");
        }
        if let Some(seconds) = startup_options.perf_seconds {
            log::info!("Android XR performance probe: {seconds}s");
        } else {
            log::info!("Android XR performance probe: <none>");
        }
        if let Some(flight) = startup_options.perf_flight {
            log::info!(
                "Android XR performance flight: speed={:.3} blocks/s",
                flight.speed_blocks_per_second
            );
        } else {
            log::info!("Android XR performance flight: <none>");
        }
        log::info!(
            "Android XR scene options: seed={} center=({}, {}) render_distance={} day_time={:?} freeze_time={} lighting={}",
            scene_options.seed,
            scene_options.chunk_x,
            scene_options.chunk_z,
            scene_options.render_distance,
            scene_options.day_time_override,
            scene_options.freeze_time,
            scene_options.lighting_enabled
        );

        let runtime_assets = match load_android_xr_runtime_assets() {
            Ok(assets) => assets,
            Err(error) => {
                log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
                return;
            }
        };

        log::info!("MCLONE_ANDROID_XR_PACKAGE_READY");
        if let Err(error) = run_android_openxr_mclone(
            &app,
            runtime_assets,
            scene_options,
            startup_view_pose,
            remote_addr,
            startup_options.session_smoke,
            startup_options.perf_seconds,
            startup_options.perf_flight,
        ) {
            log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
        }
    }

    #[allow(unsafe_code)]
    fn run_android_openxr_mclone(
        app: &AndroidApp,
        runtime_assets: AndroidXrRuntimeAssets,
        scene_options: XrSceneOptions,
        startup_view_pose: Option<XrStartupViewPose>,
        remote_addr: Option<String>,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
    ) -> Result<()> {
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
            left_eye.texture_count(),
            right_eye.texture_count()
        );
        let controller_actions = OpenXrControllerActions::create_with_binding_logger(
            graphics.session.instance(),
            &graphics.session,
            |profile, err| {
                log::warn!("OpenXR binding suggestion unavailable for {profile}: {err:?}");
            },
        )
        .context("initialize Android XR controller actions")?;
        log::info!(
            "Android XR controller actions: requested binding profiles=simple_controller, oculus_touch, valve_index, htc_vive, microsoft_motion_controller"
        );
        log::info!("MCLONE_ANDROID_XR_CONTROLLERS_READY");
        log::info!("MCLONE_ANDROID_XR_SESSION_READY");

        let mut terrain = create_android_xr_terrain_state(
            &graphics.device,
            &graphics.queue,
            runtime_assets,
            startup_view_pose,
            scene_options,
            remote_addr,
        )
        .context("initialize Android XR terrain runtime")?;
        let terrain_summary = terrain.frame_summary();
        log::info!(
            "MCLONE_ANDROID_XR_TERRAIN_READY sections={} indices={} actors={}",
            terrain_summary.section_count,
            terrain_summary.index_count,
            terrain_summary.actor_count
        );

        run_mclone_frame_loop(
            app,
            &mut graphics,
            &stage,
            environment_blend_mode,
            &mut left_eye,
            &mut right_eye,
            &mut terrain,
            &controller_actions,
            session_smoke,
            perf_seconds,
            perf_flight,
            scene_options.render_distance,
        )
    }

    fn create_android_xr_terrain_state(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        runtime_assets: AndroidXrRuntimeAssets,
        startup_view_pose: Option<XrStartupViewPose>,
        scene_options: XrSceneOptions,
        remote_addr: Option<String>,
    ) -> Result<AndroidXrTerrainState> {
        let AndroidXrRuntimeAssets {
            mesh_assets,
            actor_assets,
            asset_source,
        } = runtime_assets;
        let runtime = if let Some(remote_addr) = remote_addr {
            let session = AndroidXrRemoteServerSession::connect(remote_addr.as_str())?;
            AndroidXrSceneRuntime::remote_dedicated_with_mesh_assets(
                RemoteSessionEndpoint::new(remote_addr.clone()),
                android_xr_host_options(scene_options),
                session,
                mesh_assets,
            )
            .with_context(|| {
                format!(
                    "failed to initialize Android XR remote dedicated runtime from {remote_addr}"
                )
            })?
        } else {
            AndroidXrSceneRuntime::local_with_mesh_assets(
                android_xr_local_options(scene_options),
                mesh_assets,
            )
            .context("failed to initialize Android XR local integrated runtime")?
        };
        let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
            Ok(audio) => Some(audio),
            Err(error) => {
                log::warn!("Android XR audio disabled: {error:#}");
                None
            }
        };
        let mut terrain = XrMcloneTerrainState::with_runtime(
            device,
            queue,
            XR_COLOR_FORMAT,
            scene_options,
            runtime,
            TexturedSectionRenderOptions::default(),
            actor_assets.atlas,
            startup_view_pose,
        )?;
        terrain.set_audio_engine(audio);
        terrain.set_session_runtime_factory(|request, scene_options, mesh_assets| {
            android_xr_scene_runtime_for_request(request, scene_options, mesh_assets)
        });
        Ok(terrain)
    }

    fn android_xr_scene_runtime_for_request(
        request: SessionStartRequest,
        scene_options: XrSceneOptions,
        mesh_assets: TexturedMeshAssets,
    ) -> Result<AndroidXrSceneRuntime> {
        match request {
            SessionStartRequest::NewLocalWorld { seed } => {
                let mut scene_options = scene_options;
                scene_options.seed = seed;
                AndroidXrSceneRuntime::local_with_mesh_assets(
                    android_xr_local_options(scene_options),
                    mesh_assets,
                )
                .context("failed to initialize Android XR replacement local runtime")
            }
            SessionStartRequest::JoinRemote { endpoint } => {
                let session = AndroidXrRemoteServerSession::connect(endpoint.address.as_str())?;
                AndroidXrSceneRuntime::remote_dedicated_with_mesh_assets(
                    endpoint.clone(),
                    android_xr_host_options(scene_options),
                    session,
                    mesh_assets,
                )
                .with_context(|| {
                    format!(
                        "failed to initialize Android XR replacement remote runtime from {}",
                        endpoint.address
                    )
                })
            }
            SessionStartRequest::Unknown => bail!("unsupported Android XR replacement session"),
        }
    }

    fn android_xr_local_options(scene: XrSceneOptions) -> LocalSingleViewSceneOptions {
        LocalSingleViewSceneOptions::new(scene.seed, scene.center(), scene.render_distance)
            .with_day_time(scene.day_time_override)
            .with_freeze_time(scene.freeze_time)
            .with_lighting_enabled(scene.lighting_enabled)
    }

    fn android_xr_host_options(scene: XrSceneOptions) -> SingleViewHostOptions {
        SingleViewHostOptions::new(scene.center(), scene.render_distance)
    }

    #[derive(Debug)]
    struct AndroidXrRemoteServerSession {
        addr: String,
        session: NativeClientSession,
    }

    impl AndroidXrRemoteServerSession {
        fn connect(addr: impl Into<String>) -> Result<Self> {
            let addr = addr.into();
            let session = NativeClientSession::connect(addr.as_str())
                .with_context(|| format!("failed to connect to Android XR remote server {addr}"))?;
            Ok(Self { addr, session })
        }
    }

    impl RemoteDedicatedServerSession for AndroidXrRemoteServerSession {
        fn send_command(&mut self, command: ClientCommand) -> Result<Vec<ServerUpdate>> {
            self.session.send_command(&command).with_context(|| {
                format!(
                    "failed to exchange command with Android XR remote server {}",
                    self.addr
                )
            })
        }

        fn reconnect(&mut self) -> Result<()> {
            self.session = NativeClientSession::connect(self.addr.as_str()).with_context(|| {
                format!(
                    "failed to reconnect to Android XR remote server {}",
                    self.addr
                )
            })?;
            Ok(())
        }
    }

    fn run_mclone_frame_loop(
        app: &AndroidApp,
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        left_eye: &mut graphics_vulkan::OpenXrEyeState,
        right_eye: &mut graphics_vulkan::OpenXrEyeState,
        terrain: &mut AndroidXrTerrainState,
        controller_actions: &OpenXrControllerActions,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
        render_distance: u32,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();
        let mut perf_probe = AndroidXrPerfProbe::new(perf_seconds, perf_flight, render_distance);
        let mut logged_ready = false;
        let mut logged_controller_activity = false;
        let mut session_smoke_pending_start = false;
        let mut session_smoke_started = false;
        let mut session_smoke_ready = false;

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

            let frame_wall_start = Instant::now();
            let mut frame_timing = AndroidXrFrameTiming::default();
            if matches!(session_smoke, Some(AndroidXrSessionSmoke::NewWorld))
                && session_smoke_pending_start
                && !session_smoke_started
            {
                log::info!(
                    "MCLONE_ANDROID_XR_REPLACEMENT_STARTED new-world seed={}",
                    ANDROID_XR_SESSION_SMOKE_SEED
                );
                terrain
                    .replace_session_for_request(
                        &graphics.device,
                        &graphics.queue,
                        SessionStartRequest::NewLocalWorld {
                            seed: ANDROID_XR_SESSION_SMOKE_SEED,
                        },
                    )
                    .context("run Android XR new-world session smoke replacement")?;
                session_smoke_started = true;
                session_smoke_pending_start = false;
            }

            let wait_begin_start = Instant::now();
            let frame_state = mclone_xr_host::wait_begin_frame(
                &mut graphics.frame_wait,
                &mut graphics.frame_stream,
                &mut frame_stats,
            )?;
            frame_timing.wait_begin_ms = elapsed_ms(wait_begin_start);

            let mut rendered_frame = None;
            let mut perf_started_after_ready = false;
            let frame_result = if frame_state.should_render {
                let controller_poll_start = Instant::now();
                let render_result = match controller_actions.poll(
                    &graphics.session,
                    stage,
                    frame_state.predicted_display_time,
                ) {
                    Ok(controllers) => {
                        frame_timing.controller_poll_ms = elapsed_ms(controller_poll_start);
                        if !logged_controller_activity && !controllers.is_empty() {
                            logged_controller_activity = true;
                            log::info!(
                                "MCLONE_ANDROID_XR_CONTROLLERS_ACTIVE count={}",
                                controllers.len()
                            );
                        }
                        let render_start = Instant::now();
                        let result = render_mclone_frame(
                            graphics,
                            stage,
                            environment_blend_mode,
                            frame_state.predicted_display_time,
                            left_eye,
                            right_eye,
                            terrain,
                            &controllers,
                            perf_probe.automated_flight_speed(),
                        );
                        frame_timing.render_mclone_frame_ms = elapsed_ms(render_start);
                        result
                    }
                    Err(error) => {
                        frame_timing.controller_poll_ms = elapsed_ms(controller_poll_start);
                        Err(error)
                    }
                };
                match render_result {
                    Ok(rendered) => {
                        frame_stats.record_submitted_frame();
                        rendered_frame = Some(rendered);
                        let summary = rendered.summary;
                        if !logged_ready {
                            logged_ready = true;
                            log::info!(
                                "Android XR terrain first-frame summary: frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                                summary.rendered_frames,
                                summary.section_count,
                                summary.drawn_section_count,
                                summary.index_count,
                                summary.drawn_index_count,
                                summary.actor_count,
                                summary.drawn_actor_count
                            );
                            log::info!("MCLONE_ANDROID_XR_READY");
                            perf_started_after_ready =
                                perf_probe.start_after_ready(frame_stats, rendered);
                            if session_smoke.is_some() {
                                session_smoke_pending_start = true;
                            }
                        } else if session_smoke_started
                            && !session_smoke_ready
                            && summary.rendered_frames > 0
                        {
                            session_smoke_ready = true;
                            log::info!(
                                "MCLONE_ANDROID_XR_REPLACEMENT_READY new-world seed={} frames={} sections={} drawn_sections={} indices={} drawn_indices={}",
                                ANDROID_XR_SESSION_SMOKE_SEED,
                                summary.rendered_frames,
                                summary.section_count,
                                summary.drawn_section_count,
                                summary.index_count,
                                summary.drawn_index_count
                            );
                        }
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
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
            frame_timing.frame_wall_ms = elapsed_ms(frame_wall_start);
            if !perf_started_after_ready {
                perf_probe.record_frame(frame_timing, frame_stats, rendered_frame);
            }

            if frame_stats.submitted_frames == 1 {
                log::info!(
                    "OpenXR mclone terrain frame submitted: submitted={} runtime_frames={} skipped={}",
                    frame_stats.submitted_frames,
                    frame_stats.runtime_frames,
                    frame_stats.skipped_frames
                );
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct AndroidXrFrameTiming {
        frame_wall_ms: f64,
        wait_begin_ms: f64,
        controller_poll_ms: f64,
        render_mclone_frame_ms: f64,
    }

    #[derive(Clone, Copy, Debug)]
    struct AndroidXrRenderedFrame {
        summary: mclone_xr_scene::XrTerrainFrameSummary,
        camera: EngineCameraSnapshot,
    }

    struct AndroidXrPerfProbe {
        requested_seconds: Option<u64>,
        flight: Option<AndroidXrPerfFlight>,
        render_distance: u32,
        active: Option<AndroidXrActivePerfProbe>,
        completed: bool,
    }

    impl AndroidXrPerfProbe {
        fn new(
            requested_seconds: Option<u64>,
            flight: Option<AndroidXrPerfFlight>,
            render_distance: u32,
        ) -> Self {
            Self {
                requested_seconds,
                flight,
                render_distance,
                active: None,
                completed: false,
            }
        }

        fn automated_flight_speed(&self) -> Option<f64> {
            if self.active.is_some() {
                self.flight.map(|flight| flight.speed_blocks_per_second)
            } else {
                None
            }
        }

        fn start_after_ready(
            &mut self,
            frame_stats: XrFrameStats,
            rendered: AndroidXrRenderedFrame,
        ) -> bool {
            let Some(seconds) = self.requested_seconds else {
                return false;
            };
            if self.completed || self.active.is_some() {
                return false;
            }
            let mode = android_xr_perf_mode_label(self.flight);
            let flight_speed = self
                .flight
                .map(|flight| flight.speed_blocks_per_second)
                .unwrap_or(0.0);
            log::info!(
                "MCLONE_ANDROID_XR_PERF_START seconds={} mode={} render_distance={} flight_speed_blocks_per_second={:.3} target_hz={:.1} budget_ms={:.3} submitted={} runtime_frames={} skipped={}",
                seconds,
                mode,
                self.render_distance,
                flight_speed,
                ANDROID_XR_PERF_FALLBACK_TARGET_HZ,
                perf_budget_ms(),
                frame_stats.submitted_frames,
                frame_stats.runtime_frames,
                frame_stats.skipped_frames
            );
            self.active = Some(AndroidXrActivePerfProbe {
                requested: Duration::from_secs(seconds),
                started: Instant::now(),
                start_stats: frame_stats,
                frame_wall_ms: Vec::new(),
                max_wait_begin_ms: 0.0,
                max_controller_poll_ms: 0.0,
                max_render_mclone_frame_ms: 0.0,
                over_budget_frames: 0,
                over_2x_budget_frames: 0,
                over_4x_budget_frames: 0,
                render_distance: self.render_distance,
                flight_speed_blocks_per_second: self
                    .flight
                    .map(|flight| flight.speed_blocks_per_second),
                start_camera: rendered.camera,
                latest_camera: rendered.camera,
                latest_summary: rendered.summary,
            });
            true
        }

        fn record_frame(
            &mut self,
            timing: AndroidXrFrameTiming,
            frame_stats: XrFrameStats,
            rendered: Option<AndroidXrRenderedFrame>,
        ) {
            let Some(active) = self.active.as_mut() else {
                return;
            };
            active.record_frame(timing, rendered);
            if active.started.elapsed() >= active.requested {
                active.log_summary(frame_stats);
                self.completed = true;
                self.active = None;
            }
        }
    }

    struct AndroidXrActivePerfProbe {
        requested: Duration,
        started: Instant,
        start_stats: XrFrameStats,
        frame_wall_ms: Vec<f64>,
        max_wait_begin_ms: f64,
        max_controller_poll_ms: f64,
        max_render_mclone_frame_ms: f64,
        over_budget_frames: u64,
        over_2x_budget_frames: u64,
        over_4x_budget_frames: u64,
        render_distance: u32,
        flight_speed_blocks_per_second: Option<f64>,
        start_camera: EngineCameraSnapshot,
        latest_camera: EngineCameraSnapshot,
        latest_summary: mclone_xr_scene::XrTerrainFrameSummary,
    }

    impl AndroidXrActivePerfProbe {
        fn record_frame(
            &mut self,
            timing: AndroidXrFrameTiming,
            rendered: Option<AndroidXrRenderedFrame>,
        ) {
            let budget_ms = perf_budget_ms();
            self.frame_wall_ms.push(timing.frame_wall_ms);
            if timing.frame_wall_ms > budget_ms {
                self.over_budget_frames += 1;
            }
            if timing.frame_wall_ms > budget_ms * 2.0 {
                self.over_2x_budget_frames += 1;
            }
            if timing.frame_wall_ms > budget_ms * 4.0 {
                self.over_4x_budget_frames += 1;
            }
            self.max_wait_begin_ms = self.max_wait_begin_ms.max(timing.wait_begin_ms);
            self.max_controller_poll_ms =
                self.max_controller_poll_ms.max(timing.controller_poll_ms);
            self.max_render_mclone_frame_ms = self
                .max_render_mclone_frame_ms
                .max(timing.render_mclone_frame_ms);
            if let Some(rendered) = rendered {
                self.latest_summary = rendered.summary;
                self.latest_camera = rendered.camera;
            }
        }

        fn log_summary(&mut self, frame_stats: XrFrameStats) {
            let mut sorted = self.frame_wall_ms.clone();
            sorted.sort_by(|a, b| a.total_cmp(b));
            let sample_seconds = self.started.elapsed().as_secs_f64();
            let frame_count = self.frame_wall_ms.len() as u64;
            let submitted_delta = frame_stats.submitted_frames - self.start_stats.submitted_frames;
            let runtime_delta = frame_stats.runtime_frames - self.start_stats.runtime_frames;
            let skipped_delta = frame_stats.skipped_frames - self.start_stats.skipped_frames;
            let average_frame_ms = if self.frame_wall_ms.is_empty() {
                0.0
            } else {
                self.frame_wall_ms.iter().sum::<f64>() / self.frame_wall_ms.len() as f64
            };
            let min_frame_ms = sorted.first().copied().unwrap_or(0.0);
            let max_frame_ms = sorted.last().copied().unwrap_or(0.0);
            let flight_speed = self.flight_speed_blocks_per_second.unwrap_or(0.0);
            let flight_distance_blocks =
                camera_distance_blocks(self.start_camera, self.latest_camera);
            log::info!(
                "MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds={:.3} mode={} render_distance={} flight_speed_blocks_per_second={:.3} flight_distance_blocks={:.3} target_hz={:.1} budget_ms={:.3} frames={} submitted_delta={} runtime_delta={} skipped_delta={} frame_avg_ms={:.3} frame_min_ms={:.3} frame_p50_ms={:.3} frame_p95_ms={:.3} frame_p99_ms={:.3} frame_max_ms={:.3} over_budget={} over_2x_budget={} over_4x_budget={} max_wait_begin_ms={:.3} max_controller_poll_ms={:.3} max_render_mclone_frame_ms={:.3} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                sample_seconds,
                android_xr_perf_mode_label_for_speed(self.flight_speed_blocks_per_second),
                self.render_distance,
                flight_speed,
                flight_distance_blocks,
                ANDROID_XR_PERF_FALLBACK_TARGET_HZ,
                perf_budget_ms(),
                frame_count,
                submitted_delta,
                runtime_delta,
                skipped_delta,
                average_frame_ms,
                min_frame_ms,
                percentile(&sorted, 0.50),
                percentile(&sorted, 0.95),
                percentile(&sorted, 0.99),
                max_frame_ms,
                self.over_budget_frames,
                self.over_2x_budget_frames,
                self.over_4x_budget_frames,
                self.max_wait_begin_ms,
                self.max_controller_poll_ms,
                self.max_render_mclone_frame_ms,
                self.latest_summary.section_count,
                self.latest_summary.drawn_section_count,
                self.latest_summary.index_count,
                self.latest_summary.drawn_index_count,
                self.latest_summary.actor_count,
                self.latest_summary.drawn_actor_count
            );
        }
    }

    fn perf_budget_ms() -> f64 {
        1000.0 / ANDROID_XR_PERF_FALLBACK_TARGET_HZ
    }

    fn android_xr_perf_mode_label(flight: Option<AndroidXrPerfFlight>) -> &'static str {
        android_xr_perf_mode_label_for_speed(flight.map(|flight| flight.speed_blocks_per_second))
    }

    fn android_xr_perf_mode_label_for_speed(speed_blocks_per_second: Option<f64>) -> &'static str {
        if speed_blocks_per_second.is_some() {
            "flight"
        } else {
            "steady"
        }
    }

    fn camera_distance_blocks(start: EngineCameraSnapshot, end: EngineCameraSnapshot) -> f64 {
        let dx = end.eye.x - start.eye.x;
        let dy = end.eye.y - start.eye.y;
        let dz = end.eye.z - start.eye.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    fn elapsed_ms(start: Instant) -> f64 {
        start.elapsed().as_secs_f64() * 1000.0
    }

    fn percentile(sorted: &[f64], percentile: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let index = ((sorted.len() as f64 * percentile).ceil() as usize)
            .saturating_sub(1)
            .min(sorted.len() - 1);
        sorted[index]
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

    fn render_mclone_frame(
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        predicted_display_time: xr::Time,
        left_eye: &mut graphics_vulkan::OpenXrEyeState,
        right_eye: &mut graphics_vulkan::OpenXrEyeState,
        terrain: &mut AndroidXrTerrainState,
        controllers: &[XrControllerSnapshot],
        automated_flight_speed_blocks_per_second: Option<f64>,
    ) -> Result<AndroidXrRenderedFrame> {
        let stereo_views =
            mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;
        if let Some(speed) = automated_flight_speed_blocks_per_second {
            terrain
                .apply_automated_flight_input([stereo_views.left, stereo_views.right], speed)
                .context("apply Android XR automated flight locomotion")?;
        } else {
            terrain
                .apply_locomotion_input(controllers, [stereo_views.left, stereo_views.right])
                .context("apply Android XR controller locomotion")?;
        }

        let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
        let right_target =
            match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image") {
                Ok(target) => target,
                Err(err) => {
                    let _ = left_target.release();
                    return Err(err);
                }
            };
        let frame_summary = terrain.render_frame(
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
        let frame_summary = frame_summary?;
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
        Ok(AndroidXrRenderedFrame {
            summary: frame_summary,
            camera: terrain.camera_snapshot(),
        })
    }

    fn acquire_eye_target(
        eye_state: &mut graphics_vulkan::OpenXrEyeState,
    ) -> Result<AcquiredEyeTarget<'_>> {
        mclone_xr_host::acquire_eye_target(eye_state)
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
