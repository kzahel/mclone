#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod graphics_vulkan;

#[cfg(target_os = "android")]
mod perf_metrics;

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
    use mclone_app_runtime::startup_args::{
        RenderDistanceLimits, StartupArgState, StartupSceneOptions, parse_string_arg,
    };
    use mclone_assets::AssetSourceChain;
    use mclone_audio::{AudioEngine, AudioSettings};
    use mclone_net::NativeClientSession;
    use mclone_protocol::{ClientCommand, ServerUpdate};
    use mclone_render::chunk::TexturedSectionRenderOptions;
    use mclone_render_session::EngineCameraSnapshot;
    use mclone_xr_host::{
        OpenXrControllerActions, OpenXrHostEvent, OpenXrPollStatus, PRIMARY_STEREO_VIEW_TYPE,
        XrControllerSnapshot, XrDisplayRefreshSnapshot, XrFrameStats,
    };
    use mclone_xr_scene::{
        MAX_XR_RENDER_DISTANCE, XrMcloneTerrainState, XrSceneOptions, XrStartupViewPose,
        XrTerrainEyeTarget,
    };
    use openxr as xr;

    use super::graphics_vulkan;
    use super::perf_metrics;

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
    const XR_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    const XR_SAMPLE_COUNT: u32 = 1;
    const SESSION_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(25);
    const ANDROID_XR_SESSION_SMOKE_SEED: i64 = 246_813_579;
    const ANDROID_XR_PERF_FALLBACK_TARGET_HZ: f64 = 72.0;
    const ANDROID_XR_PERF_DEFAULT_FLIGHT_SPEED_BLOCKS_PER_SECOND: f64 = 4.3;
    const ANDROID_XR_PERF_SETTLE_MIN_SECONDS: f64 = 5.0;
    const ANDROID_XR_PERF_SETTLE_QUIET_FRAMES: u64 = 45;
    const ANDROID_XR_PERF_SETTLE_PROGRESS_FRAMES: u64 = 120;

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
        render_options: TexturedSectionRenderOptions,
        remote_addr: Option<String>,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
        perf_settled_stationary: bool,
        perf_frozen_render: bool,
        perf_metrics: bool,
    }

    impl Default for AndroidXrStartupOptions {
        fn default() -> Self {
            Self {
                scene: XrSceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                remote_addr: None,
                session_smoke: None,
                perf_seconds: None,
                perf_flight: None,
                perf_settled_stationary: false,
                perf_frozen_render: false,
                perf_metrics: false,
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
        let mut shared_args = StartupArgState::new(
            android_xr_startup_scene_defaults(),
            TexturedSectionRenderOptions::default(),
        );
        let mut argv = argv.into_iter();
        while let Some(arg) = argv.next() {
            if shared_args.parse_next_arg(
                &arg,
                &mut argv,
                RenderDistanceLimits::new(1, MAX_XR_RENDER_DISTANCE),
            )? {
                continue;
            }
            match arg.as_str() {
                "--session-smoke" => {
                    options.session_smoke = Some(parse_session_smoke_arg(parse_next_string(
                        &mut argv,
                        "--session-smoke",
                    )?)?);
                }
                "--perf-seconds" => {
                    let seconds = parse_next::<u64>(&mut argv, "--perf-seconds")?;
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
                    let speed = parse_next::<f64>(&mut argv, "--perf-flight-speed")?;
                    options.perf_flight = Some(AndroidXrPerfFlight {
                        speed_blocks_per_second: validate_perf_flight_speed(speed)?,
                    });
                }
                "--perf-settled-stationary" => {
                    options.perf_settled_stationary = true;
                }
                "--perf-frozen-render" => {
                    options.perf_settled_stationary = true;
                    options.perf_frozen_render = true;
                }
                "--perf-metrics" => {
                    options.perf_metrics = true;
                }
                unknown => bail!("unsupported Android XR startup argument `{unknown}`"),
            }
        }
        let shared_options = shared_args.finish();
        options.remote_addr = shared_options.scene.remote_addr.clone();
        options.scene = android_xr_scene_options_from_startup(shared_options.scene).validated()?;
        options.render_options = shared_options.render_options;
        if options.perf_flight.is_some() && options.perf_seconds.is_none() {
            bail!("--perf-flight requires --perf-seconds");
        }
        if options.perf_settled_stationary && options.perf_seconds.is_none() {
            bail!("--perf-settled-stationary requires --perf-seconds");
        }
        if options.perf_settled_stationary && options.perf_flight.is_some() {
            bail!("--perf-settled-stationary cannot be combined with --perf-flight");
        }
        if options.perf_frozen_render && options.perf_seconds.is_none() {
            bail!("--perf-frozen-render requires --perf-seconds");
        }
        if options.perf_frozen_render && options.perf_flight.is_some() {
            bail!("--perf-frozen-render cannot be combined with --perf-flight");
        }
        Ok(options)
    }

    fn android_xr_startup_scene_defaults() -> StartupSceneOptions {
        let scene = XrSceneOptions::default();
        StartupSceneOptions {
            seed: scene.seed,
            chunk_x: scene.chunk_x,
            chunk_z: scene.chunk_z,
            render_distance: scene.render_distance,
            movement_speed_multiplier: scene.movement_speed_multiplier,
            remote_addr: None,
            day_time_override: scene.day_time_override,
            freeze_time: scene.freeze_time,
            lighting_enabled: scene.lighting_enabled,
        }
    }

    fn android_xr_scene_options_from_startup(scene: StartupSceneOptions) -> XrSceneOptions {
        XrSceneOptions {
            seed: scene.seed,
            chunk_x: scene.chunk_x,
            chunk_z: scene.chunk_z,
            render_distance: scene.render_distance,
            movement_speed_multiplier: scene.movement_speed_multiplier,
            day_time_override: scene.day_time_override,
            freeze_time: scene.freeze_time,
            lighting_enabled: scene.lighting_enabled,
        }
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

    fn parse_next_string(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
        parse_string_arg(flag, args.next())
    }

    fn parse_next<T>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        value
            .parse::<T>()
            .with_context(|| format!("{flag} has invalid value `{value}`"))
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
            "Android XR performance settled stationary: {}",
            startup_options.perf_settled_stationary
        );
        log::info!(
            "Android XR performance frozen render: {}",
            startup_options.perf_frozen_render
        );
        log::info!(
            "Android XR performance metrics probe: {}",
            startup_options.perf_metrics
        );
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
        log::info!(
            "Android XR render options: section_occlusion={} fullbright={} color_profile={}",
            startup_options.render_options.section_occlusion_culling,
            startup_options.render_options.force_fullbright,
            startup_options.render_options.color_profile.as_str()
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
            startup_options.render_options,
            startup_view_pose,
            remote_addr,
            startup_options.session_smoke,
            startup_options.perf_seconds,
            startup_options.perf_flight,
            startup_options.perf_settled_stationary,
            startup_options.perf_frozen_render,
            startup_options.perf_metrics,
        ) {
            log::error!("MCLONE_ANDROID_XR_FAILURE: {error:#}");
        }
    }

    #[allow(unsafe_code)]
    fn run_android_openxr_mclone(
        app: &AndroidApp,
        runtime_assets: AndroidXrRuntimeAssets,
        scene_options: XrSceneOptions,
        render_options: TexturedSectionRenderOptions,
        startup_view_pose: Option<XrStartupViewPose>,
        remote_addr: Option<String>,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
        perf_settled_stationary: bool,
        perf_frozen_render: bool,
        perf_metrics: bool,
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
            "OpenXR extensions: android_create_instance=true vulkan_enable2=true fb_passthrough={} fb_alpha_blend={} fb_display_refresh_rate={} fb_swapchain_update_state={} fb_foveation={} fb_foveation_configuration={} fb_foveation_vulkan={} fb_render_model={} ext_hand_tracking={} fb_hand_tracking_mesh={} fb_hand_tracking_aim={} meta_virtual_keyboard={} fb_spatial_entity={} fb_spatial_entity_query={} fb_scene={} fb_scene_capture={} fb_spatial_entity_container={} meta_spatial_entity_mesh={} fb_body_tracking={} meta_body_tracking_full_body={} meta_performance_metrics={} ext_debug_utils={}",
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
            available.meta_performance_metrics,
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
        if available.meta_performance_metrics {
            enabled_extensions.meta_performance_metrics = true;
            log::info!("Enabling XR_META_performance_metrics");
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
        let display_refresh = mclone_xr_host::query_display_refresh_snapshot(
            &graphics.session,
            available.fb_display_refresh_rate,
        );
        if display_refresh.extension_supported {
            log::info!(
                "OpenXR display refresh rates: {}",
                mclone_xr_host::display_refresh_rates_label(&display_refresh.supported_rates)
            );
            match display_refresh.current_rate {
                Some(rate) => log::info!("OpenXR current display refresh: {rate:.1} Hz"),
                None => log::info!("OpenXR current display refresh: unknown"),
            }
        } else {
            log::info!("OpenXR display refresh extension unavailable");
        }
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
            render_options,
            remote_addr,
        )
        .context("initialize Android XR terrain runtime")?;
        let terrain_summary = terrain.frame_summary();
        terrain.set_display_refresh_hz(display_refresh.current_rate);
        terrain.set_render_split_timing_enabled(perf_seconds.is_some());
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
            perf_settled_stationary,
            perf_frozen_render,
            perf_metrics,
            startup_view_pose,
            scene_options.render_distance,
            display_refresh,
        )
    }

    fn create_android_xr_terrain_state(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        runtime_assets: AndroidXrRuntimeAssets,
        startup_view_pose: Option<XrStartupViewPose>,
        scene_options: XrSceneOptions,
        render_options: TexturedSectionRenderOptions,
        remote_addr: Option<String>,
    ) -> Result<AndroidXrTerrainState> {
        let AndroidXrRuntimeAssets {
            mesh_assets,
            actor_assets,
            asset_source,
        } = runtime_assets;
        let mut terrain = if let Some(remote_addr) = remote_addr {
            let session = AndroidXrRemoteServerSession::connect(remote_addr.as_str())?;
            let runtime = AndroidXrSceneRuntime::remote_dedicated_with_mesh_assets(
                RemoteSessionEndpoint::new(remote_addr.clone()),
                android_xr_host_options(scene_options),
                session,
                mesh_assets,
            )
            .with_context(|| {
                format!(
                    "failed to initialize Android XR remote dedicated runtime from {remote_addr}"
                )
            })?;
            XrMcloneTerrainState::with_runtime(
                device,
                queue,
                XR_COLOR_FORMAT,
                scene_options,
                runtime,
                render_options,
                actor_assets.atlas,
                startup_view_pose,
            )?
        } else {
            XrMcloneTerrainState::start_local_async(
                device,
                queue,
                XR_COLOR_FORMAT,
                scene_options,
                render_options,
                mesh_assets,
                actor_assets.atlas,
                startup_view_pose,
            )?
        };
        let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
            Ok(audio) => Some(audio),
            Err(error) => {
                log::warn!("Android XR audio disabled: {error:#}");
                None
            }
        };
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
            .with_initial_spawn_center()
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
        perf_settled_stationary: bool,
        perf_frozen_render: bool,
        perf_metrics: bool,
        fixed_render_view_pose: Option<XrStartupViewPose>,
        render_distance: u32,
        display_refresh: XrDisplayRefreshSnapshot,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();
        let mut perf_probe = AndroidXrPerfProbe::new(
            perf_seconds,
            perf_flight,
            perf_settled_stationary,
            perf_frozen_render,
            render_distance,
            display_refresh,
        );
        let mut performance_metrics_probe = if perf_metrics {
            let probe = perf_metrics::XrPerformanceMetricsProbe::new(
                graphics.session.instance(),
                &graphics.session,
            );
            if probe.is_none() {
                log::warn!(
                    "XR_META_performance_metrics requested via --perf-metrics but unavailable; continuing without GPU/compositor attribution"
                );
            }
            probe
        } else {
            None
        };
        let mut logged_first_frame = false;
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
                            perf_probe.automation(),
                            fixed_render_view_pose,
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
                        frame_timing.render = rendered.timing;
                        rendered_frame = Some(rendered);
                        let summary = rendered.summary;
                        if !logged_first_frame {
                            logged_first_frame = true;
                            log::info!(
                                "Android XR terrain first-frame summary: frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={} local_startup_active={}",
                                summary.rendered_frames,
                                summary.section_count,
                                summary.drawn_section_count,
                                summary.index_count,
                                summary.drawn_index_count,
                                summary.actor_count,
                                summary.drawn_actor_count,
                                summary.local_startup_active
                            );
                        }
                        if !logged_ready && !summary.local_startup_active {
                            logged_ready = true;
                            log::info!(
                                "Android XR terrain ready summary: frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                                summary.rendered_frames,
                                summary.section_count,
                                summary.drawn_section_count,
                                summary.index_count,
                                summary.drawn_index_count,
                                summary.actor_count,
                                summary.drawn_actor_count
                            );
                            log::info!("MCLONE_ANDROID_XR_READY");
                            if session_smoke.is_some() {
                                session_smoke_pending_start = true;
                            }
                        } else if session_smoke_started
                            && !session_smoke_ready
                            && !summary.local_startup_active
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
                        perf_started_after_ready = logged_ready
                            && perf_probe.maybe_start_after_rendered_frame(frame_stats, rendered);
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
            if rendered_frame.is_some() {
                if let Some(probe) = performance_metrics_probe.as_mut() {
                    probe.tick();
                }
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
        render: AndroidXrRenderFrameTiming,
    }

    #[derive(Clone, Copy, Debug)]
    struct AndroidXrRenderedFrame {
        summary: mclone_xr_scene::XrTerrainFrameSummary,
        camera: EngineCameraSnapshot,
        timing: AndroidXrRenderFrameTiming,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum AndroidXrPerfAutomation {
        Flight { speed_blocks_per_second: f64 },
        Stationary { frozen_render: bool },
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct AndroidXrRenderFrameTiming {
        locate_views_ms: f64,
        locomotion_ms: f64,
        acquire_left_ms: f64,
        acquire_right_ms: f64,
        terrain_render_frame_ms: f64,
        terrain_render_views_ms: f64,
        terrain_menu_pointer_ms: f64,
        terrain_runtime_upload_ms: f64,
        terrain_runtime_poll_ms: f64,
        terrain_runtime_sync_ms: f64,
        terrain_runtime_gpu_upload_ms: f64,
        terrain_runtime_ready_sections_ms: f64,
        terrain_shared_records_ms: f64,
        terrain_left_eye_ms: f64,
        terrain_right_eye_ms: f64,
        terrain_left_eye_prepare_ms: f64,
        terrain_left_eye_cull_ms: f64,
        terrain_left_eye_uniform_write_ms: f64,
        terrain_left_eye_translucent_collect_ms: f64,
        terrain_left_eye_translucent_sort_ms: f64,
        terrain_left_eye_encode_ms: f64,
        terrain_left_eye_section_encode_ms: f64,
        terrain_left_eye_submit_ms: f64,
        terrain_left_eye_poll_wait_ms: f64,
        terrain_right_eye_prepare_ms: f64,
        terrain_right_eye_cull_ms: f64,
        terrain_right_eye_uniform_write_ms: f64,
        terrain_right_eye_translucent_collect_ms: f64,
        terrain_right_eye_translucent_sort_ms: f64,
        terrain_right_eye_encode_ms: f64,
        terrain_right_eye_section_encode_ms: f64,
        terrain_right_eye_submit_ms: f64,
        terrain_right_eye_poll_wait_ms: f64,
        terrain_stereo_finish_ms: f64,
        terrain_stereo_submit_ms: f64,
        terrain_stereo_poll_wait_ms: f64,
        release_eyes_ms: f64,
        end_frame_ms: f64,
    }

    struct AndroidXrPerfProbe {
        requested_seconds: Option<u64>,
        flight: Option<AndroidXrPerfFlight>,
        settled_stationary: bool,
        frozen_render: bool,
        render_distance: u32,
        display_refresh: XrDisplayRefreshSnapshot,
        target_hz: f64,
        settle_started: Option<Instant>,
        settle_frames: u64,
        settle_quiet_frames: u64,
        active: Option<AndroidXrActivePerfProbe>,
        completed: bool,
    }

    impl AndroidXrPerfProbe {
        fn new(
            requested_seconds: Option<u64>,
            flight: Option<AndroidXrPerfFlight>,
            settled_stationary: bool,
            frozen_render: bool,
            render_distance: u32,
            display_refresh: XrDisplayRefreshSnapshot,
        ) -> Self {
            let target_hz = display_refresh
                .current_rate
                .map(f64::from)
                .filter(|hz| hz.is_finite() && *hz > 0.0)
                .unwrap_or(ANDROID_XR_PERF_FALLBACK_TARGET_HZ);
            Self {
                requested_seconds,
                flight,
                settled_stationary,
                frozen_render,
                render_distance,
                display_refresh,
                target_hz,
                settle_started: None,
                settle_frames: 0,
                settle_quiet_frames: 0,
                active: None,
                completed: false,
            }
        }

        fn automation(&self) -> Option<AndroidXrPerfAutomation> {
            if self.settled_stationary && self.requested_seconds.is_some() && !self.completed {
                return Some(AndroidXrPerfAutomation::Stationary {
                    frozen_render: self.frozen_render && self.active.is_some(),
                });
            }
            if self.active.is_some() {
                return self.flight.map(|flight| AndroidXrPerfAutomation::Flight {
                    speed_blocks_per_second: flight.speed_blocks_per_second,
                });
            } else {
                None
            }
        }

        fn maybe_start_after_rendered_frame(
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
            if self.settled_stationary && !self.record_settle_frame(rendered.summary) {
                return false;
            }
            let mode = android_xr_perf_mode_label(
                self.flight,
                self.settled_stationary,
                self.frozen_render,
            );
            let flight_speed = self
                .flight
                .map(|flight| flight.speed_blocks_per_second)
                .unwrap_or(0.0);
            let settle_seconds = self
                .settle_started
                .map_or(0.0, |started| started.elapsed().as_secs_f64());
            if self.settled_stationary {
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_SETTLED mode={} settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} sections={} drawn_sections={} indices={} drawn_indices={} ready_sections={}",
                    mode,
                    settle_seconds,
                    ANDROID_XR_PERF_SETTLE_MIN_SECONDS,
                    self.settle_frames,
                    self.settle_quiet_frames,
                    rendered.summary.section_count,
                    rendered.summary.drawn_section_count,
                    rendered.summary.index_count,
                    rendered.summary.drawn_index_count,
                    rendered.summary.upload.traversal_ready_section_count
                );
            }
            log::info!(
                "MCLONE_ANDROID_XR_PERF_START seconds={} mode={} render_distance={} flight_speed_blocks_per_second={:.3} settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} refresh_supported={} current_hz={} supported_hz={} target_hz={:.1} budget_ms={:.3} submitted={} runtime_frames={} skipped={}",
                seconds,
                mode,
                self.render_distance,
                flight_speed,
                settle_seconds,
                ANDROID_XR_PERF_SETTLE_MIN_SECONDS,
                self.settle_frames,
                self.settle_quiet_frames,
                self.display_refresh.extension_supported,
                format_optional_hz(self.display_refresh.current_rate),
                format_supported_hz(&self.display_refresh.supported_rates),
                self.target_hz,
                perf_budget_ms(self.target_hz),
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
                max_render: AndroidXrRenderFrameTiming::default(),
                max_upload: mclone_xr_scene::XrTerrainUploadSummary::default(),
                upload_work_frames: 0,
                diagnostics_refresh_frames: 0,
                over_budget_frames: 0,
                over_2x_budget_frames: 0,
                over_4x_budget_frames: 0,
                render_distance: self.render_distance,
                display_refresh: self.display_refresh.clone(),
                target_hz: self.target_hz,
                mode_label: mode,
                flight_speed_blocks_per_second: self
                    .flight
                    .map(|flight| flight.speed_blocks_per_second),
                settle_seconds,
                settle_frames: self.settle_frames,
                settle_quiet_frames: self.settle_quiet_frames,
                start_camera: rendered.camera,
                latest_camera: rendered.camera,
                latest_summary: rendered.summary,
            });
            true
        }

        fn record_settle_frame(&mut self, summary: mclone_xr_scene::XrTerrainFrameSummary) -> bool {
            let _ = self.settle_started.get_or_insert_with(Instant::now);
            self.settle_frames += 1;
            let quiet = android_xr_perf_settle_frame_is_quiet(summary);
            if quiet {
                self.settle_quiet_frames += 1;
            } else {
                self.settle_quiet_frames = 0;
            }
            let settle_seconds = self
                .settle_started
                .map_or(0.0, |started| started.elapsed().as_secs_f64());
            if self.settle_frames == 1
                || self.settle_frames % ANDROID_XR_PERF_SETTLE_PROGRESS_FRAMES == 0
            {
                let upload = summary.upload;
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_SETTLE_PROGRESS mode=stationary-settled settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} quiet={} poll_changed={} server_cmd_q={} server_update_q={} pending_jobs_after={} pending_chunks_after={} deferred_sections={} submitted_sections={} completed_sections={} stale_sections={} uploaded_sections={} upload_removed_sections={} ready_sections={} sections={} drawn_sections={} drawn_indices={}",
                    settle_seconds,
                    ANDROID_XR_PERF_SETTLE_MIN_SECONDS,
                    self.settle_frames,
                    self.settle_quiet_frames,
                    quiet,
                    upload.poll_changed,
                    upload.server_command_queue_depth,
                    upload.server_update_queue_depth,
                    upload.pending_compile_jobs_after,
                    upload.pending_render_chunks_after,
                    upload.deferred_section_count,
                    upload.submitted_compile_section_count,
                    upload.completed_compile_section_count,
                    upload.stale_compile_section_count,
                    upload.uploaded_section_count,
                    upload.upload_removed_section_count,
                    upload.traversal_ready_section_count,
                    summary.section_count,
                    summary.drawn_section_count,
                    summary.drawn_index_count
                );
            }
            self.settle_quiet_frames >= ANDROID_XR_PERF_SETTLE_QUIET_FRAMES
                && settle_seconds >= ANDROID_XR_PERF_SETTLE_MIN_SECONDS
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
        max_render: AndroidXrRenderFrameTiming,
        max_upload: mclone_xr_scene::XrTerrainUploadSummary,
        upload_work_frames: u64,
        diagnostics_refresh_frames: u64,
        over_budget_frames: u64,
        over_2x_budget_frames: u64,
        over_4x_budget_frames: u64,
        render_distance: u32,
        display_refresh: XrDisplayRefreshSnapshot,
        target_hz: f64,
        mode_label: &'static str,
        flight_speed_blocks_per_second: Option<f64>,
        settle_seconds: f64,
        settle_frames: u64,
        settle_quiet_frames: u64,
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
            let budget_ms = perf_budget_ms(self.target_hz);
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
            self.max_render = max_render_timing(self.max_render, timing.render);
            if let Some(rendered) = rendered {
                if terrain_upload_summary_has_work(rendered.summary.upload) {
                    self.upload_work_frames += 1;
                }
                if rendered.summary.upload.poll_diagnostics_refreshed {
                    self.diagnostics_refresh_frames += 1;
                }
                self.max_upload = max_upload_summary(self.max_upload, rendered.summary.upload);
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
            let latest_upload = self.latest_summary.upload;
            log::info!(
                "MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds={:.3} mode={} render_distance={} flight_speed_blocks_per_second={:.3} flight_distance_blocks={:.3} settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} refresh_supported={} current_hz={} supported_hz={} target_hz={:.1} budget_ms={:.3} frames={} submitted_delta={} runtime_delta={} skipped_delta={} frame_avg_ms={:.3} frame_min_ms={:.3} frame_p50_ms={:.3} frame_p95_ms={:.3} frame_p99_ms={:.3} frame_max_ms={:.3} over_budget={} over_2x_budget={} over_4x_budget={}",
                sample_seconds,
                self.mode_label,
                self.render_distance,
                flight_speed,
                flight_distance_blocks,
                self.settle_seconds,
                ANDROID_XR_PERF_SETTLE_MIN_SECONDS,
                self.settle_frames,
                self.settle_quiet_frames,
                self.display_refresh.extension_supported,
                format_optional_hz(self.display_refresh.current_rate),
                format_supported_hz(&self.display_refresh.supported_rates),
                self.target_hz,
                perf_budget_ms(self.target_hz),
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
                self.over_4x_budget_frames
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_STAGES max_wait_begin_ms={:.3} max_controller_poll_ms={:.3} max_render_mclone_frame_ms={:.3} max_locate_views_ms={:.3} max_locomotion_ms={:.3} max_acquire_left_ms={:.3} max_acquire_right_ms={:.3} max_release_eyes_ms={:.3} max_end_frame_ms={:.3}",
                self.max_wait_begin_ms,
                self.max_controller_poll_ms,
                self.max_render_mclone_frame_ms,
                self.max_render.locate_views_ms,
                self.max_render.locomotion_ms,
                self.max_render.acquire_left_ms,
                self.max_render.acquire_right_ms,
                self.max_render.release_eyes_ms,
                self.max_render.end_frame_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms={:.3} max_terrain_render_views_ms={:.3} max_terrain_menu_pointer_ms={:.3} max_terrain_runtime_upload_ms={:.3} max_runtime_poll_ms={:.3} max_runtime_sync_ms={:.3} max_runtime_gpu_upload_ms={:.3} max_runtime_ready_sections_ms={:.3} max_terrain_shared_records_ms={:.3} max_terrain_left_eye_ms={:.3} max_terrain_right_eye_ms={:.3} max_terrain_left_eye_prepare_ms={:.3} max_terrain_left_eye_encode_ms={:.3} max_terrain_left_eye_section_encode_ms={:.3} max_terrain_left_eye_submit_ms={:.3} max_terrain_left_eye_poll_wait_ms={:.3} max_terrain_right_eye_prepare_ms={:.3} max_terrain_right_eye_encode_ms={:.3} max_terrain_right_eye_section_encode_ms={:.3} max_terrain_right_eye_submit_ms={:.3} max_terrain_right_eye_poll_wait_ms={:.3} max_terrain_stereo_finish_ms={:.3} max_terrain_stereo_submit_ms={:.3} max_terrain_stereo_poll_wait_ms={:.3}",
                self.max_render.terrain_render_frame_ms,
                self.max_render.terrain_render_views_ms,
                self.max_render.terrain_menu_pointer_ms,
                self.max_render.terrain_runtime_upload_ms,
                self.max_render.terrain_runtime_poll_ms,
                self.max_render.terrain_runtime_sync_ms,
                self.max_render.terrain_runtime_gpu_upload_ms,
                self.max_render.terrain_runtime_ready_sections_ms,
                self.max_render.terrain_shared_records_ms,
                self.max_render.terrain_left_eye_ms,
                self.max_render.terrain_right_eye_ms,
                self.max_render.terrain_left_eye_prepare_ms,
                self.max_render.terrain_left_eye_encode_ms,
                self.max_render.terrain_left_eye_section_encode_ms,
                self.max_render.terrain_left_eye_submit_ms,
                self.max_render.terrain_left_eye_poll_wait_ms,
                self.max_render.terrain_right_eye_prepare_ms,
                self.max_render.terrain_right_eye_encode_ms,
                self.max_render.terrain_right_eye_section_encode_ms,
                self.max_render.terrain_right_eye_submit_ms,
                self.max_render.terrain_right_eye_poll_wait_ms,
                self.max_render.terrain_stereo_finish_ms,
                self.max_render.terrain_stereo_submit_ms,
                self.max_render.terrain_stereo_poll_wait_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_TERRAIN_PREP max_terrain_left_eye_cull_ms={:.3} max_terrain_left_eye_uniform_write_ms={:.3} max_terrain_left_eye_translucent_collect_ms={:.3} max_terrain_left_eye_translucent_sort_ms={:.3} max_terrain_right_eye_cull_ms={:.3} max_terrain_right_eye_uniform_write_ms={:.3} max_terrain_right_eye_translucent_collect_ms={:.3} max_terrain_right_eye_translucent_sort_ms={:.3}",
                self.max_render.terrain_left_eye_cull_ms,
                self.max_render.terrain_left_eye_uniform_write_ms,
                self.max_render.terrain_left_eye_translucent_collect_ms,
                self.max_render.terrain_left_eye_translucent_sort_ms,
                self.max_render.terrain_right_eye_cull_ms,
                self.max_render.terrain_right_eye_uniform_write_ms,
                self.max_render.terrain_right_eye_translucent_collect_ms,
                self.max_render.terrain_right_eye_translucent_sort_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_UPLOAD_MAX work_frames={} rebuilt_sections={} removed_sections={} rebuilt_vertices={} rebuilt_indices={} uploaded_sections={} upload_removed_sections={} uploaded_vertices={} uploaded_indices={} ready_sections={}",
                self.upload_work_frames,
                self.max_upload.rebuilt_section_count,
                self.max_upload.removed_section_count,
                self.max_upload.rebuilt_vertex_count,
                self.max_upload.rebuilt_index_count,
                self.max_upload.uploaded_section_count,
                self.max_upload.upload_removed_section_count,
                self.max_upload.uploaded_vertex_count,
                self.max_upload.uploaded_index_count,
                self.max_upload.traversal_ready_section_count
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_RUNTIME_MAX poll_total_ms={:.3} drain_updates_ms={:.3} apply_updates_ms={:.3} dirty_mark_ms={:.3} client_apply_ms={:.3} poll_diagnostics_ms={:.3} diagnostics_refresh_frames={} diagnostics_refreshed={} diagnostics_cache_age_ms={:.3} server_detail_refreshes={} server_detail_age_ms={:.3} server_tick_ms={:.3} scheduler_tick_ms={:.3} updates={} snapshot_updates={} section_updates={} unload_updates={}",
                self.max_upload.poll_total_ms,
                self.max_upload.poll_drain_updates_ms,
                self.max_upload.poll_apply_updates_ms,
                self.max_upload.poll_dirty_mark_ms,
                self.max_upload.poll_client_apply_updates_ms,
                self.max_upload.poll_diagnostics_ms,
                self.diagnostics_refresh_frames,
                self.max_upload.poll_diagnostics_refreshed,
                self.max_upload.poll_diagnostics_cache_age_ms,
                self.max_upload.server_diagnostics_detail_refreshes,
                self.max_upload.server_diagnostics_detail_age_ms,
                self.max_upload.poll_server_tick_ms,
                self.max_upload.poll_scheduler_tick_ms,
                self.max_upload.poll_updates,
                self.max_upload.poll_snapshot_updates,
                self.max_upload.poll_section_block_updates,
                self.max_upload.poll_unload_updates
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_QUEUE_MAX server_cmd_q={} server_update_q={} server_pending_jobs={} server_pending_publications={} scheduler_pending_jobs={} scheduler_completed_jobs={} scheduler_dirty_chunks={} scheduler_loaded_chunks={} scheduler_visible_chunks={} scheduler_ticket_chunks={} player_visible_chunks={} player_outbound_q={}",
                self.max_upload.server_command_queue_depth,
                self.max_upload.server_update_queue_depth,
                self.max_upload.server_pending_jobs,
                self.max_upload.server_pending_publications,
                self.max_upload.scheduler_pending_jobs,
                self.max_upload.scheduler_completed_jobs,
                self.max_upload.scheduler_dirty_chunks,
                self.max_upload.scheduler_loaded_snapshot_chunks,
                self.max_upload.scheduler_client_visible_chunks,
                self.max_upload.scheduler_active_ticket_chunks,
                self.max_upload.player_visible_chunks,
                self.max_upload.player_outbound_queue_depth
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_COMPILE_MAX pending_chunks_before={} pending_chunks_after={} pending_jobs_before={} pending_jobs_after={} neighbor_ready_sections={} near_exception_sections={} deferred_sections={} submitted_sections={} completed_sections={} stale_sections={} visibility_graph_builds={} visibility_graph_total_ms={:.3} visibility_graph_worst_ms={:.3}",
                self.max_upload.pending_render_chunks_before,
                self.max_upload.pending_render_chunks_after,
                self.max_upload.pending_compile_jobs_before,
                self.max_upload.pending_compile_jobs_after,
                self.max_upload.neighbor_ready_section_count,
                self.max_upload.near_exception_section_count,
                self.max_upload.deferred_section_count,
                self.max_upload.submitted_compile_section_count,
                self.max_upload.completed_compile_section_count,
                self.max_upload.stale_compile_section_count,
                self.max_upload.visibility_graph_build_count,
                self.max_upload.visibility_graph_total_ms,
                self.max_upload.visibility_graph_worst_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_UPLOAD_LAST poll_changed={} pending_chunks_before={} pending_chunks_after={} pending_jobs_before={} pending_jobs_after={} rebuilt_sections={} removed_sections={} uploaded_sections={} upload_removed_sections={} uploaded_indices={} ready_sections={}",
                latest_upload.poll_changed,
                latest_upload.pending_render_chunks_before,
                latest_upload.pending_render_chunks_after,
                latest_upload.pending_compile_jobs_before,
                latest_upload.pending_compile_jobs_after,
                latest_upload.rebuilt_section_count,
                latest_upload.removed_section_count,
                latest_upload.uploaded_section_count,
                latest_upload.upload_removed_section_count,
                latest_upload.uploaded_index_count,
                latest_upload.traversal_ready_section_count
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_DRAW sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                self.latest_summary.section_count,
                self.latest_summary.drawn_section_count,
                self.latest_summary.index_count,
                self.latest_summary.drawn_index_count,
                self.latest_summary.actor_count,
                self.latest_summary.drawn_actor_count
            );
        }
    }

    fn perf_budget_ms(target_hz: f64) -> f64 {
        1000.0 / target_hz.max(1.0)
    }

    fn format_optional_hz(rate: Option<f32>) -> String {
        rate.map(|hz| format!("{hz:.1}"))
            .unwrap_or_else(|| "unknown".to_owned())
    }

    fn format_supported_hz(rates: &[f32]) -> String {
        if rates.is_empty() {
            return "none".to_owned();
        }
        rates
            .iter()
            .map(|hz| format!("{hz:.1}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn max_render_timing(
        a: AndroidXrRenderFrameTiming,
        b: AndroidXrRenderFrameTiming,
    ) -> AndroidXrRenderFrameTiming {
        AndroidXrRenderFrameTiming {
            locate_views_ms: a.locate_views_ms.max(b.locate_views_ms),
            locomotion_ms: a.locomotion_ms.max(b.locomotion_ms),
            acquire_left_ms: a.acquire_left_ms.max(b.acquire_left_ms),
            acquire_right_ms: a.acquire_right_ms.max(b.acquire_right_ms),
            terrain_render_frame_ms: a.terrain_render_frame_ms.max(b.terrain_render_frame_ms),
            terrain_render_views_ms: a.terrain_render_views_ms.max(b.terrain_render_views_ms),
            terrain_menu_pointer_ms: a.terrain_menu_pointer_ms.max(b.terrain_menu_pointer_ms),
            terrain_runtime_upload_ms: a.terrain_runtime_upload_ms.max(b.terrain_runtime_upload_ms),
            terrain_runtime_poll_ms: a.terrain_runtime_poll_ms.max(b.terrain_runtime_poll_ms),
            terrain_runtime_sync_ms: a.terrain_runtime_sync_ms.max(b.terrain_runtime_sync_ms),
            terrain_runtime_gpu_upload_ms: a
                .terrain_runtime_gpu_upload_ms
                .max(b.terrain_runtime_gpu_upload_ms),
            terrain_runtime_ready_sections_ms: a
                .terrain_runtime_ready_sections_ms
                .max(b.terrain_runtime_ready_sections_ms),
            terrain_shared_records_ms: a.terrain_shared_records_ms.max(b.terrain_shared_records_ms),
            terrain_left_eye_ms: a.terrain_left_eye_ms.max(b.terrain_left_eye_ms),
            terrain_right_eye_ms: a.terrain_right_eye_ms.max(b.terrain_right_eye_ms),
            terrain_left_eye_prepare_ms: a
                .terrain_left_eye_prepare_ms
                .max(b.terrain_left_eye_prepare_ms),
            terrain_left_eye_cull_ms: a.terrain_left_eye_cull_ms.max(b.terrain_left_eye_cull_ms),
            terrain_left_eye_uniform_write_ms: a
                .terrain_left_eye_uniform_write_ms
                .max(b.terrain_left_eye_uniform_write_ms),
            terrain_left_eye_translucent_collect_ms: a
                .terrain_left_eye_translucent_collect_ms
                .max(b.terrain_left_eye_translucent_collect_ms),
            terrain_left_eye_translucent_sort_ms: a
                .terrain_left_eye_translucent_sort_ms
                .max(b.terrain_left_eye_translucent_sort_ms),
            terrain_left_eye_encode_ms: a
                .terrain_left_eye_encode_ms
                .max(b.terrain_left_eye_encode_ms),
            terrain_left_eye_section_encode_ms: a
                .terrain_left_eye_section_encode_ms
                .max(b.terrain_left_eye_section_encode_ms),
            terrain_left_eye_submit_ms: a
                .terrain_left_eye_submit_ms
                .max(b.terrain_left_eye_submit_ms),
            terrain_left_eye_poll_wait_ms: a
                .terrain_left_eye_poll_wait_ms
                .max(b.terrain_left_eye_poll_wait_ms),
            terrain_right_eye_prepare_ms: a
                .terrain_right_eye_prepare_ms
                .max(b.terrain_right_eye_prepare_ms),
            terrain_right_eye_cull_ms: a.terrain_right_eye_cull_ms.max(b.terrain_right_eye_cull_ms),
            terrain_right_eye_uniform_write_ms: a
                .terrain_right_eye_uniform_write_ms
                .max(b.terrain_right_eye_uniform_write_ms),
            terrain_right_eye_translucent_collect_ms: a
                .terrain_right_eye_translucent_collect_ms
                .max(b.terrain_right_eye_translucent_collect_ms),
            terrain_right_eye_translucent_sort_ms: a
                .terrain_right_eye_translucent_sort_ms
                .max(b.terrain_right_eye_translucent_sort_ms),
            terrain_right_eye_encode_ms: a
                .terrain_right_eye_encode_ms
                .max(b.terrain_right_eye_encode_ms),
            terrain_right_eye_section_encode_ms: a
                .terrain_right_eye_section_encode_ms
                .max(b.terrain_right_eye_section_encode_ms),
            terrain_right_eye_submit_ms: a
                .terrain_right_eye_submit_ms
                .max(b.terrain_right_eye_submit_ms),
            terrain_right_eye_poll_wait_ms: a
                .terrain_right_eye_poll_wait_ms
                .max(b.terrain_right_eye_poll_wait_ms),
            terrain_stereo_finish_ms: a.terrain_stereo_finish_ms.max(b.terrain_stereo_finish_ms),
            terrain_stereo_submit_ms: a.terrain_stereo_submit_ms.max(b.terrain_stereo_submit_ms),
            terrain_stereo_poll_wait_ms: a
                .terrain_stereo_poll_wait_ms
                .max(b.terrain_stereo_poll_wait_ms),
            release_eyes_ms: a.release_eyes_ms.max(b.release_eyes_ms),
            end_frame_ms: a.end_frame_ms.max(b.end_frame_ms),
        }
    }

    fn terrain_upload_summary_has_work(summary: mclone_xr_scene::XrTerrainUploadSummary) -> bool {
        summary.poll_changed
            || summary.rebuilt_section_count > 0
            || summary.removed_section_count > 0
            || summary.submitted_compile_section_count > 0
            || summary.completed_compile_section_count > 0
            || summary.stale_compile_section_count > 0
            || summary.uploaded_section_count > 0
            || summary.upload_removed_section_count > 0
    }

    fn android_xr_perf_settle_frame_is_quiet(
        summary: mclone_xr_scene::XrTerrainFrameSummary,
    ) -> bool {
        let upload = summary.upload;
        !terrain_upload_summary_has_work(upload)
            && upload.server_command_queue_depth == 0
            && upload.server_update_queue_depth == 0
            && upload.pending_compile_jobs_after == 0
    }

    fn max_upload_summary(
        a: mclone_xr_scene::XrTerrainUploadSummary,
        b: mclone_xr_scene::XrTerrainUploadSummary,
    ) -> mclone_xr_scene::XrTerrainUploadSummary {
        mclone_xr_scene::XrTerrainUploadSummary {
            poll_changed: a.poll_changed || b.poll_changed,
            poll_total_ms: a.poll_total_ms.max(b.poll_total_ms),
            poll_drain_updates_ms: a.poll_drain_updates_ms.max(b.poll_drain_updates_ms),
            poll_apply_updates_ms: a.poll_apply_updates_ms.max(b.poll_apply_updates_ms),
            poll_dirty_mark_ms: a.poll_dirty_mark_ms.max(b.poll_dirty_mark_ms),
            poll_client_apply_updates_ms: a
                .poll_client_apply_updates_ms
                .max(b.poll_client_apply_updates_ms),
            poll_diagnostics_ms: a.poll_diagnostics_ms.max(b.poll_diagnostics_ms),
            poll_diagnostics_refreshed: a.poll_diagnostics_refreshed
                || b.poll_diagnostics_refreshed,
            poll_diagnostics_cache_age_ms: a
                .poll_diagnostics_cache_age_ms
                .max(b.poll_diagnostics_cache_age_ms),
            server_diagnostics_detail_refreshes: a
                .server_diagnostics_detail_refreshes
                .max(b.server_diagnostics_detail_refreshes),
            server_diagnostics_detail_age_ms: a
                .server_diagnostics_detail_age_ms
                .max(b.server_diagnostics_detail_age_ms),
            poll_server_tick_ms: a.poll_server_tick_ms.max(b.poll_server_tick_ms),
            poll_server_reported_total_ms: a
                .poll_server_reported_total_ms
                .max(b.poll_server_reported_total_ms),
            poll_scheduler_tick_ms: a.poll_scheduler_tick_ms.max(b.poll_scheduler_tick_ms),
            poll_updates: a.poll_updates.max(b.poll_updates),
            poll_snapshot_updates: a.poll_snapshot_updates.max(b.poll_snapshot_updates),
            poll_section_block_updates: a
                .poll_section_block_updates
                .max(b.poll_section_block_updates),
            poll_unload_updates: a.poll_unload_updates.max(b.poll_unload_updates),
            server_command_queue_depth: a
                .server_command_queue_depth
                .max(b.server_command_queue_depth),
            server_update_queue_depth: a.server_update_queue_depth.max(b.server_update_queue_depth),
            server_pending_jobs: a.server_pending_jobs.max(b.server_pending_jobs),
            server_pending_publications: a
                .server_pending_publications
                .max(b.server_pending_publications),
            scheduler_pending_jobs: a.scheduler_pending_jobs.max(b.scheduler_pending_jobs),
            scheduler_completed_jobs: a.scheduler_completed_jobs.max(b.scheduler_completed_jobs),
            scheduler_dirty_chunks: a.scheduler_dirty_chunks.max(b.scheduler_dirty_chunks),
            scheduler_loaded_snapshot_chunks: a
                .scheduler_loaded_snapshot_chunks
                .max(b.scheduler_loaded_snapshot_chunks),
            scheduler_client_visible_chunks: a
                .scheduler_client_visible_chunks
                .max(b.scheduler_client_visible_chunks),
            scheduler_active_ticket_chunks: a
                .scheduler_active_ticket_chunks
                .max(b.scheduler_active_ticket_chunks),
            player_visible_chunks: a.player_visible_chunks.max(b.player_visible_chunks),
            player_outbound_queue_depth: a
                .player_outbound_queue_depth
                .max(b.player_outbound_queue_depth),
            pending_render_chunks_before: a
                .pending_render_chunks_before
                .max(b.pending_render_chunks_before),
            pending_render_chunks_after: a
                .pending_render_chunks_after
                .max(b.pending_render_chunks_after),
            pending_compile_jobs_before: a
                .pending_compile_jobs_before
                .max(b.pending_compile_jobs_before),
            pending_compile_jobs_after: a
                .pending_compile_jobs_after
                .max(b.pending_compile_jobs_after),
            rebuilt_section_count: a.rebuilt_section_count.max(b.rebuilt_section_count),
            removed_section_count: a.removed_section_count.max(b.removed_section_count),
            rebuilt_vertex_count: a.rebuilt_vertex_count.max(b.rebuilt_vertex_count),
            rebuilt_index_count: a.rebuilt_index_count.max(b.rebuilt_index_count),
            neighbor_ready_section_count: a
                .neighbor_ready_section_count
                .max(b.neighbor_ready_section_count),
            near_exception_section_count: a
                .near_exception_section_count
                .max(b.near_exception_section_count),
            deferred_section_count: a.deferred_section_count.max(b.deferred_section_count),
            submitted_compile_section_count: a
                .submitted_compile_section_count
                .max(b.submitted_compile_section_count),
            completed_compile_section_count: a
                .completed_compile_section_count
                .max(b.completed_compile_section_count),
            stale_compile_section_count: a
                .stale_compile_section_count
                .max(b.stale_compile_section_count),
            uploaded_section_count: a.uploaded_section_count.max(b.uploaded_section_count),
            upload_removed_section_count: a
                .upload_removed_section_count
                .max(b.upload_removed_section_count),
            uploaded_vertex_count: a.uploaded_vertex_count.max(b.uploaded_vertex_count),
            uploaded_index_count: a.uploaded_index_count.max(b.uploaded_index_count),
            traversal_ready_section_count: a
                .traversal_ready_section_count
                .max(b.traversal_ready_section_count),
            visibility_graph_build_count: a
                .visibility_graph_build_count
                .max(b.visibility_graph_build_count),
            visibility_graph_total_ms: a.visibility_graph_total_ms.max(b.visibility_graph_total_ms),
            visibility_graph_worst_ms: a.visibility_graph_worst_ms.max(b.visibility_graph_worst_ms),
        }
    }

    fn android_xr_perf_mode_label(
        flight: Option<AndroidXrPerfFlight>,
        settled_stationary: bool,
        frozen_render: bool,
    ) -> &'static str {
        if frozen_render {
            "stationary-frozen-render"
        } else if settled_stationary {
            "stationary-settled"
        } else if flight.is_some() {
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
        automation: Option<AndroidXrPerfAutomation>,
        fixed_render_view_pose: Option<XrStartupViewPose>,
    ) -> Result<AndroidXrRenderedFrame> {
        let mut timing = AndroidXrRenderFrameTiming::default();
        let locate_views_start = Instant::now();
        let stereo_views =
            mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;
        timing.locate_views_ms = elapsed_ms(locate_views_start);
        let locomotion_start = Instant::now();
        let mut frozen_render = false;
        match automation {
            Some(AndroidXrPerfAutomation::Flight {
                speed_blocks_per_second,
            }) => {
                terrain
                    .apply_automated_flight_input(
                        [stereo_views.left, stereo_views.right],
                        speed_blocks_per_second,
                    )
                    .context("apply Android XR automated flight locomotion")?;
            }
            Some(AndroidXrPerfAutomation::Stationary {
                frozen_render: freeze_runtime,
            }) => {
                frozen_render = freeze_runtime;
                terrain.apply_automated_stationary_input();
            }
            None => {
                terrain
                    .apply_locomotion_input(controllers, [stereo_views.left, stereo_views.right])
                    .context("apply Android XR controller locomotion")?;
            }
        }
        timing.locomotion_ms = elapsed_ms(locomotion_start);

        let acquire_left_start = Instant::now();
        let left_target = acquire_eye_target(left_eye).context("acquire left-eye OpenXR image")?;
        timing.acquire_left_ms = elapsed_ms(acquire_left_start);
        let acquire_right_start = Instant::now();
        let right_target =
            match acquire_eye_target(right_eye).context("acquire right-eye OpenXR image") {
                Ok(target) => {
                    timing.acquire_right_ms = elapsed_ms(acquire_right_start);
                    target
                }
                Err(err) => {
                    let _ = left_target.release();
                    return Err(err);
                }
            };
        let terrain_render_start = Instant::now();
        let left_terrain_target = XrTerrainEyeTarget {
            color_view: left_target.color_view(),
            depth: &left_target.eye().depth,
            size: [left_target.eye().width, left_target.eye().height],
        };
        let right_terrain_target = XrTerrainEyeTarget {
            color_view: right_target.color_view(),
            depth: &right_target.eye().depth,
            size: [right_target.eye().width, right_target.eye().height],
        };
        let frame_summary = if frozen_render {
            let fixed_render_view_pose = fixed_render_view_pose.with_context(
                || "Android XR frozen render probe requires a fixed startup view pose",
            )?;
            terrain.render_frame_frozen_runtime_at_view_pose(
                &graphics.device,
                &graphics.queue,
                fixed_render_view_pose,
                [stereo_views.left.fov, stereo_views.right.fov],
                left_terrain_target,
                right_terrain_target,
            )
        } else {
            terrain.render_frame(
                &graphics.device,
                &graphics.queue,
                [stereo_views.left, stereo_views.right],
                left_terrain_target,
                right_terrain_target,
            )
        };
        timing.terrain_render_frame_ms = elapsed_ms(terrain_render_start);
        let release_start = Instant::now();
        let left_release_result = left_target.release();
        let right_release_result = right_target.release();
        timing.release_eyes_ms = elapsed_ms(release_start);
        let frame_summary = frame_summary?;
        timing.terrain_render_views_ms = frame_summary.timing.render_views_ms;
        timing.terrain_menu_pointer_ms = frame_summary.timing.menu_pointer_ms;
        timing.terrain_runtime_upload_ms = frame_summary.timing.runtime_upload_ms;
        timing.terrain_runtime_poll_ms = frame_summary.timing.runtime_poll_ms;
        timing.terrain_runtime_sync_ms = frame_summary.timing.runtime_sync_ms;
        timing.terrain_runtime_gpu_upload_ms = frame_summary.timing.runtime_gpu_upload_ms;
        timing.terrain_runtime_ready_sections_ms = frame_summary.timing.runtime_ready_sections_ms;
        timing.terrain_shared_records_ms = frame_summary.timing.shared_records_ms;
        timing.terrain_left_eye_ms = frame_summary.timing.left_eye_ms;
        timing.terrain_right_eye_ms = frame_summary.timing.right_eye_ms;
        timing.terrain_left_eye_prepare_ms = frame_summary.timing.left_eye_render.prepare_ms;
        timing.terrain_left_eye_cull_ms = frame_summary.timing.left_eye_render.cull_ms;
        timing.terrain_left_eye_uniform_write_ms =
            frame_summary.timing.left_eye_render.uniform_write_ms;
        timing.terrain_left_eye_translucent_collect_ms =
            frame_summary.timing.left_eye_render.translucent_collect_ms;
        timing.terrain_left_eye_translucent_sort_ms =
            frame_summary.timing.left_eye_render.translucent_sort_ms;
        timing.terrain_left_eye_encode_ms = frame_summary.timing.left_eye_render.encode_ms;
        timing.terrain_left_eye_section_encode_ms =
            frame_summary.timing.left_eye_render.section_encode_ms;
        timing.terrain_left_eye_submit_ms = frame_summary.timing.left_eye_render.submit_ms;
        timing.terrain_left_eye_poll_wait_ms = frame_summary.timing.left_eye_render.poll_wait_ms;
        timing.terrain_right_eye_prepare_ms = frame_summary.timing.right_eye_render.prepare_ms;
        timing.terrain_right_eye_cull_ms = frame_summary.timing.right_eye_render.cull_ms;
        timing.terrain_right_eye_uniform_write_ms =
            frame_summary.timing.right_eye_render.uniform_write_ms;
        timing.terrain_right_eye_translucent_collect_ms =
            frame_summary.timing.right_eye_render.translucent_collect_ms;
        timing.terrain_right_eye_translucent_sort_ms =
            frame_summary.timing.right_eye_render.translucent_sort_ms;
        timing.terrain_right_eye_encode_ms = frame_summary.timing.right_eye_render.encode_ms;
        timing.terrain_right_eye_section_encode_ms =
            frame_summary.timing.right_eye_render.section_encode_ms;
        timing.terrain_right_eye_submit_ms = frame_summary.timing.right_eye_render.submit_ms;
        timing.terrain_right_eye_poll_wait_ms = frame_summary.timing.right_eye_render.poll_wait_ms;
        timing.terrain_stereo_finish_ms = frame_summary.timing.stereo_finish_ms;
        timing.terrain_stereo_submit_ms = frame_summary.timing.stereo_submit_ms;
        timing.terrain_stereo_poll_wait_ms = frame_summary.timing.stereo_poll_wait_ms;
        left_release_result?;
        right_release_result?;

        let end_frame_start = Instant::now();
        mclone_xr_host::end_stereo_projection_frame(
            &mut graphics.frame_stream,
            predicted_display_time,
            environment_blend_mode,
            stage,
            stereo_views,
            left_eye,
            right_eye,
        )?;
        timing.end_frame_ms = elapsed_ms(end_frame_start);
        Ok(AndroidXrRenderedFrame {
            summary: frame_summary,
            camera: terrain.camera_snapshot(),
            timing,
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
