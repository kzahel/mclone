#![deny(unsafe_code)]

#[cfg_attr(not(any(test, target_os = "android")), allow(dead_code))]
const ANDROID_REMOTE_ADDR_NONE_SENTINEL: &str = "__mclone_none__";

// Android XR-owned startup flags are OpenXR session/perf harness controls.
// Shared scene/session policy flags must stay in `StartupArgState`.
#[cfg(test)]
const ANDROID_XR_LOCAL_ARG_FLAGS: &[&str] = &[
    "--adaptive-chunk-publication-budget",
    "--frame-accounting",
    "--multiview-proof",
    "--perf-chunk-view-churn",
    "--perf-churn-interval-seconds",
    "--perf-churn-offset-chunks",
    "--perf-flight",
    "--perf-flight-speed",
    "--perf-frozen-render",
    "--perf-metrics",
    "--perf-metrics-periodic",
    "--perf-detail",
    "--perf-orbit-speed",
    "--perf-seconds",
    "--perf-settled-orbit",
    "--perf-settled-stationary",
    "--session-smoke",
    "--sky-terrain-actors-multiview-perf",
    "--sky-terrain-multiview-perf",
    "--terrain-multiview-perf",
    "--terrain-multiview-proof",
    "--xr-debug-ui",
    "--xr-display-refresh-rate",
    "--xr-foveation",
    "--xr-frame-overlap",
    "--xr-frame-serial",
    "--xr-full-frame-multiview",
    "--xr-overlap-eye-submits",
    "--xr-overlap-runtime-prefetch",
    "--xr-render-completed-result-accept-budget",
    "--xr-render-scale",
    "--xr-render-section-accept-budget",
    "--xr-render-section-upload-budget",
    "--xr-skip-actors",
    "--xr-underwater-mode",
];

#[cfg_attr(not(any(test, target_os = "android")), allow(dead_code))]
fn normalize_android_legacy_remote_addr(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == ANDROID_REMOTE_ADDR_NONE_SENTINEL {
        None
    } else {
        Some(value.to_owned())
    }
}

#[cfg(target_os = "android")]
mod graphics_vulkan;

#[cfg(target_os = "android")]
mod perf_metrics;

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CStr, CString, c_char, c_int};
    use std::sync::Once;
    use std::thread;
    use std::time::{Duration, Instant};

    use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
    use anyhow::{Context, Result, bail};
    use mclone_android_platform::android_app_data_world_root;
    use mclone_app_runtime::frame_render::scaled_frame_size;
    use mclone_app_runtime::host_mode::{
        RemoteCommandUpdate, RemoteCommandUpdateBatch, RemoteDedicatedServerSession,
        SingleViewHostOptions,
    };
    use mclone_app_runtime::local_single_view::{
        IntegratedWorldSessionStorage, LocalSingleViewSceneOptions, NativeSingleViewSessionRuntime,
    };
    use mclone_app_runtime::render_assets::{
        ActorTextureAssets, TexturedMeshAssets, load_actor_texture_assets_from_asset_source,
        load_asset_source, load_textured_mesh_assets_from_source,
    };
    use mclone_app_runtime::render_compile_capacity::{
        RenderCompileCapacityHostKind, host_total_memory_bytes,
        preflight_render_compile_capacity_report,
    };
    use mclone_app_runtime::session::{RemoteSessionEndpoint, SessionStartRequest};
    use mclone_app_runtime::startup_args::{
        RenderCompileCapacityRequest, RenderDistanceLimits, StartupArgState, StartupSceneOptions,
        StartupWorldStorageProjection, format_optional_startup_path, parse_bool_arg,
        parse_string_arg,
    };
    use mclone_assets::AssetSourceChain;
    use mclone_audio::{AudioEngine, AudioSettings};
    use mclone_diagnostics::{
        BudgetDecisionFamily, BudgetDecisionPanelReport, BudgetDecisionReason, CriticalPathLabel,
        DiagnosticLaneAvailability, FrameAccountingConfig, FrameAccumulator, FrameHostKind,
        FrameObservation, FramePipelineReport, FrameSummaryReport, PeerThreadActivityReport,
        PeerThreadId, PeerThreadPanelReport, PercentileMethod, QueueAgeReport, QueueAgeTracker,
        QueueId, QueuePanelReport, StageId, StageSpan, WorkWindow, percentile_sorted_ms,
    };
    use mclone_net::{NativeClientIoSession, NativeServerUpdateBatch};
    use mclone_protocol::{ClientCommand, ServerUpdate};
    use mclone_render::chunk::{
        ChunkDepthTarget, ChunkMultiviewDepthTarget, TexturedSectionRecordCacheStats,
        TexturedSectionRecordPrepareStats, TexturedSectionRenderOptions,
    };
    use mclone_render_session::EngineCameraSnapshot;
    use mclone_xr_host::{
        OpenXrControllerActions, OpenXrHostEvent, OpenXrPollStatus, PRIMARY_STEREO_VIEW_TYPE,
        XrControllerSnapshot, XrDisplayRefreshSnapshot, XrFrameStats,
    };
    use mclone_xr_scene::{
        MAX_XR_RENDER_DISTANCE, XrDebugUiScreen, XrFramePipelineHostTiming,
        XrFramePipelineReporter, XrMcloneTerrainState, XrSceneOptions, XrStartupViewPose,
        XrTerrainEyeTarget, XrTerrainMultiviewTarget, XrUnderwaterDetectionMode,
    };
    use openxr as xr;

    use super::graphics_vulkan;
    use super::perf_metrics;

    type AcquiredEyeTarget<'a> = mclone_xr_host::XrAcquiredEyeTarget<
        'a,
        graphics_vulkan::AppGraphics,
        graphics_vulkan::OpenXrEyeState,
    >;
    type AcquiredStereoTarget<'a> = mclone_xr_host::XrAcquiredStereoTarget<
        'a,
        graphics_vulkan::AppGraphics,
        graphics_vulkan::OpenXrStereoState,
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
    const XR_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    const XR_SAMPLE_COUNT: u32 = 1;
    const SESSION_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(25);
    const ANDROID_XR_SESSION_SMOKE_SEED: i64 = 246_813_579;
    const ANDROID_XR_PERF_FALLBACK_TARGET_HZ: f64 = 72.0;
    const ANDROID_XR_PERF_DEFAULT_FLIGHT_SPEED_BLOCKS_PER_SECOND: f64 = 4.3;
    const ANDROID_XR_PERF_DEFAULT_CHURN_INTERVAL_SECONDS: f64 = 3.0;
    const ANDROID_XR_PERF_DEFAULT_CHURN_OFFSET_CHUNKS: i32 = 16;
    const ANDROID_XR_PERF_SETTLE_MIN_SECONDS: f64 = 5.0;
    const ANDROID_XR_PERF_SETTLE_QUIET_FRAMES: u64 = 45;
    const ANDROID_XR_PERF_SETTLE_PROGRESS_FRAMES: u64 = 120;
    const ANDROID_XR_PERF_WORST_FRAME_COUNT: usize = 5;
    const TERRAIN_MULTIVIEW_PERF_WARMUP_FRAMES: usize = 12;
    const TERRAIN_MULTIVIEW_PERF_SAMPLE_FRAMES: usize = 60;
    const ANDROID_XR_DEFAULT_RENDER_SCALE: f32 = 1.0;
    const ANDROID_XR_MIN_RENDER_SCALE: f32 = 0.25;
    const ANDROID_XR_MAX_RENDER_SCALE: f32 = 1.0;

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
        default_world_root_enabled: bool,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
        perf_settled_orbit: Option<AndroidXrPerfOrbit>,
        perf_chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
        perf_settled_stationary: bool,
        perf_frozen_render: bool,
        perf_metrics: bool,
        perf_metrics_periodic: bool,
        perf_detail: AndroidXrPerfDetail,
        frame_accounting_enabled: bool,
        multiview_proof: bool,
        terrain_multiview_proof: bool,
        terrain_multiview_perf: bool,
        sky_terrain_multiview_perf: bool,
        sky_terrain_actors_multiview_perf: bool,
        skip_actors: bool,
        full_frame_multiview: bool,
        frame_overlap: bool,
        frame_overlap_mode: AndroidXrFrameOverlapMode,
        overlap_eye_submits: bool,
        overlap_runtime_prefetch: bool,
        render_section_upload_budget: Option<usize>,
        render_section_accept_budget: Option<usize>,
        render_completed_result_accept_budget: Option<usize>,
        xr_foveation: AndroidXrFoveation,
        xr_render_scale: f32,
        xr_display_refresh_rate: Option<f32>,
    }

    impl Default for AndroidXrStartupOptions {
        fn default() -> Self {
            Self {
                scene: XrSceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                remote_addr: None,
                default_world_root_enabled: true,
                session_smoke: None,
                perf_seconds: None,
                perf_flight: None,
                perf_settled_orbit: None,
                perf_chunk_view_churn: None,
                perf_settled_stationary: false,
                perf_frozen_render: false,
                perf_metrics: false,
                perf_metrics_periodic: false,
                perf_detail: AndroidXrPerfDetail::Full,
                frame_accounting_enabled: true,
                multiview_proof: false,
                terrain_multiview_proof: false,
                terrain_multiview_perf: false,
                sky_terrain_multiview_perf: false,
                sky_terrain_actors_multiview_perf: false,
                skip_actors: false,
                full_frame_multiview: false,
                frame_overlap: false,
                frame_overlap_mode: AndroidXrFrameOverlapMode::default(),
                overlap_eye_submits: false,
                overlap_runtime_prefetch: false,
                render_section_upload_budget: None,
                render_section_accept_budget: None,
                render_completed_result_accept_budget: None,
                xr_foveation: AndroidXrFoveation::Off,
                xr_render_scale: ANDROID_XR_DEFAULT_RENDER_SCALE,
                xr_display_refresh_rate: None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum AndroidXrFrameOverlapMode {
        #[default]
        Default,
        ExplicitOverlap,
        Serial,
    }

    impl AndroidXrFrameOverlapMode {
        const fn label(self) -> &'static str {
            match self {
                Self::Default => "default",
                Self::ExplicitOverlap => "explicit-overlap",
                Self::Serial => "serial",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum AndroidXrFoveation {
        Off,
        Low,
        Medium,
        High,
    }

    impl AndroidXrFoveation {
        fn parse_label(flag: &str, value: &str) -> Result<Self> {
            match value.trim().to_ascii_lowercase().as_str() {
                "off" | "none" | "false" | "0" | "disabled" => Ok(Self::Off),
                "low" => Ok(Self::Low),
                "medium" | "med" => Ok(Self::Medium),
                "high" => Ok(Self::High),
                value => bail!(
                    "{flag} has unsupported value `{value}`; expected off, low, medium, or high"
                ),
            }
        }

        const fn label(self) -> &'static str {
            match self {
                Self::Off => "off",
                Self::Low => "low",
                Self::Medium => "medium",
                Self::High => "high",
            }
        }

        const fn is_enabled(self) -> bool {
            !matches!(self, Self::Off)
        }

        fn level_profile(self) -> Option<xr::FoveationLevelProfile> {
            let level = match self {
                Self::Off => return None,
                Self::Low => xr::FoveationLevelFB::LOW,
                Self::Medium => xr::FoveationLevelFB::MEDIUM,
                Self::High => xr::FoveationLevelFB::HIGH,
            };
            Some(xr::FoveationLevelProfile {
                level,
                vertical_offset: 0.0,
                dynamic: xr::FoveationDynamicFB::DISABLED,
            })
        }
    }

    fn parse_android_xr_render_scale(flag: &str, value: &str) -> Result<f32> {
        let value = value.trim();
        let scale = if let Some(percent) = value.strip_suffix('%') {
            parse_render_scale_number(flag, percent)? / 100.0
        } else if let Some(scale) = value.strip_suffix('x') {
            parse_render_scale_number(flag, scale)?
        } else if let Some(scale) = value.strip_suffix('X') {
            parse_render_scale_number(flag, scale)?
        } else {
            parse_render_scale_number(flag, value)?
        };
        if !(ANDROID_XR_MIN_RENDER_SCALE..=ANDROID_XR_MAX_RENDER_SCALE).contains(&scale) {
            bail!(
                "{flag} must be between {:.2} and {:.2}, got `{value}`",
                ANDROID_XR_MIN_RENDER_SCALE,
                ANDROID_XR_MAX_RENDER_SCALE
            );
        }
        Ok(scale)
    }

    fn parse_render_scale_number(flag: &str, value: &str) -> Result<f32> {
        let scale = value
            .trim()
            .parse::<f32>()
            .with_context(|| format!("parse {flag} value `{value}`"))?;
        if !scale.is_finite() {
            bail!("{flag} must be finite, got `{value}`");
        }
        Ok(scale)
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct AndroidXrPerfFlight {
        speed_blocks_per_second: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct AndroidXrPerfOrbit {
        speed_blocks_per_second: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct AndroidXrPerfChunkViewChurn {
        interval_seconds: f64,
        offset_chunks: i32,
    }

    impl Default for AndroidXrPerfChunkViewChurn {
        fn default() -> Self {
            Self {
                interval_seconds: ANDROID_XR_PERF_DEFAULT_CHURN_INTERVAL_SECONDS,
                offset_chunks: ANDROID_XR_PERF_DEFAULT_CHURN_OFFSET_CHUNKS,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum AndroidXrPerfDetail {
        #[default]
        Full,
        Minimal,
    }

    impl AndroidXrPerfDetail {
        fn parse_label(flag: &str, value: &str) -> Result<Self> {
            match value.trim() {
                "full" => Ok(Self::Full),
                "minimal" | "summary" | "summary-only" => Ok(Self::Minimal),
                value => bail!("{flag} has unsupported value `{value}`; expected full or minimal"),
            }
        }

        const fn label(self) -> &'static str {
            match self {
                Self::Full => "full",
                Self::Minimal => "minimal",
            }
        }

        const fn is_full(self) -> bool {
            matches!(self, Self::Full)
        }
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
        let mut options = AndroidXrStartupOptions::default();
        let Some(json) = startup_argv_json.filter(|json| !json.trim().is_empty()) else {
            resolve_android_xr_frame_overlap(&mut options)?;
            return Ok(options);
        };
        let argv = serde_json::from_str::<Vec<String>>(json)
            .context("parse Android XR startup argv JSON")?;
        let mut shared_args = StartupArgState::new(
            android_xr_startup_scene_defaults(),
            TexturedSectionRenderOptions::default(),
        );
        let mut underwater_detection_mode = XrUnderwaterDetectionMode::default();
        let mut debug_ui_screen = None;
        let mut adaptive_chunk_publication_budget = None;
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
                "--xr-underwater-mode" => {
                    underwater_detection_mode = XrUnderwaterDetectionMode::parse_label(
                        "--xr-underwater-mode",
                        &parse_next_string(&mut argv, "--xr-underwater-mode")?,
                    )?;
                }
                "--xr-debug-ui" => {
                    let value = parse_next_string(&mut argv, "--xr-debug-ui")?;
                    debug_ui_screen = match value.trim() {
                        "none" | "off" | "false" => None,
                        value => Some(XrDebugUiScreen::parse_label("--xr-debug-ui", value)?),
                    };
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
                        speed_blocks_per_second: validate_perf_motion_speed(
                            "--perf-flight-speed",
                            speed,
                        )?,
                    });
                }
                "--perf-settled-orbit" => {
                    if options.perf_settled_orbit.is_none() {
                        options.perf_settled_orbit = Some(AndroidXrPerfOrbit {
                            speed_blocks_per_second:
                                ANDROID_XR_PERF_DEFAULT_FLIGHT_SPEED_BLOCKS_PER_SECOND,
                        });
                    }
                }
                "--perf-orbit-speed" => {
                    let speed = parse_next::<f64>(&mut argv, "--perf-orbit-speed")?;
                    options.perf_settled_orbit = Some(AndroidXrPerfOrbit {
                        speed_blocks_per_second: validate_perf_motion_speed(
                            "--perf-orbit-speed",
                            speed,
                        )?,
                    });
                }
                "--perf-chunk-view-churn" => {
                    options
                        .perf_chunk_view_churn
                        .get_or_insert_with(AndroidXrPerfChunkViewChurn::default);
                }
                "--perf-churn-interval-seconds" => {
                    let interval_seconds =
                        parse_next::<f64>(&mut argv, "--perf-churn-interval-seconds")?;
                    let churn = options
                        .perf_chunk_view_churn
                        .get_or_insert_with(AndroidXrPerfChunkViewChurn::default);
                    churn.interval_seconds = validate_perf_churn_interval_seconds(
                        "--perf-churn-interval-seconds",
                        interval_seconds,
                    )?;
                }
                "--perf-churn-offset-chunks" => {
                    let offset_chunks = parse_next::<i32>(&mut argv, "--perf-churn-offset-chunks")?;
                    let churn = options
                        .perf_chunk_view_churn
                        .get_or_insert_with(AndroidXrPerfChunkViewChurn::default);
                    churn.offset_chunks = validate_perf_churn_offset_chunks(
                        "--perf-churn-offset-chunks",
                        offset_chunks,
                    )?;
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
                "--perf-metrics-periodic" => {
                    options.perf_metrics = true;
                    options.perf_metrics_periodic = true;
                }
                "--perf-detail" => {
                    options.perf_detail = AndroidXrPerfDetail::parse_label(
                        "--perf-detail",
                        &parse_next_string(&mut argv, "--perf-detail")?,
                    )?;
                }
                "--frame-accounting" => {
                    options.frame_accounting_enabled = parse_bool_arg(
                        "--frame-accounting",
                        Some(parse_next_string(&mut argv, "--frame-accounting")?),
                    )?;
                }
                "--adaptive-chunk-publication-budget" => {
                    adaptive_chunk_publication_budget = Some(parse_bool_arg(
                        "--adaptive-chunk-publication-budget",
                        Some(parse_next_string(
                            &mut argv,
                            "--adaptive-chunk-publication-budget",
                        )?),
                    )?);
                }
                "--multiview-proof" => {
                    options.multiview_proof = true;
                }
                "--terrain-multiview-proof" => {
                    options.terrain_multiview_proof = true;
                }
                "--terrain-multiview-perf" => {
                    options.terrain_multiview_perf = true;
                }
                "--sky-terrain-multiview-perf" => {
                    options.sky_terrain_multiview_perf = true;
                }
                "--sky-terrain-actors-multiview-perf" => {
                    options.sky_terrain_actors_multiview_perf = true;
                }
                "--xr-skip-actors" => {
                    options.skip_actors = true;
                }
                "--xr-full-frame-multiview" => {
                    options.full_frame_multiview = true;
                }
                "--xr-frame-overlap" => {
                    if options.frame_overlap_mode == AndroidXrFrameOverlapMode::Serial {
                        bail!("--xr-frame-overlap cannot be combined with --xr-frame-serial");
                    }
                    options.frame_overlap_mode = AndroidXrFrameOverlapMode::ExplicitOverlap;
                }
                "--xr-frame-serial" => {
                    if options.frame_overlap_mode == AndroidXrFrameOverlapMode::ExplicitOverlap {
                        bail!("--xr-frame-serial cannot be combined with --xr-frame-overlap");
                    }
                    options.frame_overlap_mode = AndroidXrFrameOverlapMode::Serial;
                }
                "--xr-overlap-eye-submits" => {
                    options.overlap_eye_submits = true;
                }
                "--xr-overlap-runtime-prefetch" => {
                    options.overlap_runtime_prefetch = true;
                }
                "--xr-render-section-upload-budget" => {
                    let budget =
                        parse_next::<usize>(&mut argv, "--xr-render-section-upload-budget")?;
                    if budget == 0 {
                        bail!("--xr-render-section-upload-budget must be greater than zero");
                    }
                    options.render_section_upload_budget = Some(budget);
                }
                "--xr-render-section-accept-budget" => {
                    let budget =
                        parse_next::<usize>(&mut argv, "--xr-render-section-accept-budget")?;
                    if budget == 0 {
                        bail!("--xr-render-section-accept-budget must be greater than zero");
                    }
                    options.render_section_accept_budget = Some(budget);
                }
                "--xr-render-completed-result-accept-budget" => {
                    let budget = parse_next::<usize>(
                        &mut argv,
                        "--xr-render-completed-result-accept-budget",
                    )?;
                    if budget == 0 {
                        bail!(
                            "--xr-render-completed-result-accept-budget must be greater than zero"
                        );
                    }
                    options.render_completed_result_accept_budget = Some(budget);
                }
                "--xr-foveation" => {
                    options.xr_foveation = AndroidXrFoveation::parse_label(
                        "--xr-foveation",
                        &parse_next_string(&mut argv, "--xr-foveation")?,
                    )?;
                }
                "--xr-render-scale" => {
                    options.xr_render_scale = parse_android_xr_render_scale(
                        "--xr-render-scale",
                        &parse_next_string(&mut argv, "--xr-render-scale")?,
                    )?;
                }
                "--xr-display-refresh-rate" => {
                    options.xr_display_refresh_rate = Some(validate_display_refresh_rate(
                        parse_next::<f32>(&mut argv, "--xr-display-refresh-rate")?,
                    )?);
                }
                unknown => bail!("unsupported Android XR startup argument `{unknown}`"),
            }
        }
        if shared_args.scene().render_compile_capacity_request
            == RenderCompileCapacityRequest::Derived
            && shared_args.scene().remote_addr.is_some()
        {
            bail!("--render-compile-capacity derived applies only to local integrated worlds");
        }
        if shared_args.scene().render_compile_capacity_request
            == RenderCompileCapacityRequest::Derived
        {
            let report = preflight_render_compile_capacity_report(
                RenderCompileCapacityHostKind::Xr,
                host_total_memory_bytes(),
            );
            shared_args.apply_render_compile_capacity_report(&report);
        }
        let shared_options = shared_args.finish();
        options.remote_addr = shared_options.scene.remote_addr.clone();
        let storage = shared_options.storage.project(None);
        options.default_world_root_enabled = storage.default_world_root_enabled;
        let mut scene = android_xr_scene_options_from_startup(shared_options.scene, &storage);
        scene.underwater_detection_mode = underwater_detection_mode;
        scene.debug_ui_screen = debug_ui_screen;
        scene.skip_actors = options.skip_actors;
        scene.adaptive_chunk_publication_budget =
            adaptive_chunk_publication_budget.unwrap_or(options.remote_addr.is_none());
        storage.validate_local_integrated_world(options.remote_addr.as_deref())?;
        if options.remote_addr.is_some() && scene.adaptive_chunk_publication_budget {
            bail!("--adaptive-chunk-publication-budget applies only to local integrated worlds");
        }
        options.scene = scene.validated()?;
        options.render_options = shared_options.render_options;
        if options.perf_flight.is_some() && options.perf_seconds.is_none() {
            bail!("--perf-flight requires --perf-seconds");
        }
        if options.perf_settled_orbit.is_some() && options.perf_seconds.is_none() {
            bail!("--perf-settled-orbit requires --perf-seconds");
        }
        if options.perf_chunk_view_churn.is_some() && options.perf_seconds.is_none() {
            bail!("--perf-chunk-view-churn requires --perf-seconds");
        }
        if options.perf_settled_stationary && options.perf_seconds.is_none() {
            bail!("--perf-settled-stationary requires --perf-seconds");
        }
        if options.perf_settled_stationary && options.perf_flight.is_some() {
            bail!("--perf-settled-stationary cannot be combined with --perf-flight");
        }
        if options.perf_settled_orbit.is_some() && options.perf_flight.is_some() {
            bail!("--perf-settled-orbit cannot be combined with --perf-flight");
        }
        if options.perf_settled_orbit.is_some() && options.perf_settled_stationary {
            bail!("--perf-settled-orbit cannot be combined with --perf-settled-stationary");
        }
        if options.perf_chunk_view_churn.is_some() && options.perf_flight.is_some() {
            bail!("--perf-chunk-view-churn cannot be combined with --perf-flight");
        }
        if options.perf_chunk_view_churn.is_some() && options.perf_settled_orbit.is_some() {
            bail!("--perf-chunk-view-churn cannot be combined with --perf-settled-orbit");
        }
        if options.perf_chunk_view_churn.is_some() && options.perf_settled_stationary {
            bail!("--perf-chunk-view-churn cannot be combined with --perf-settled-stationary");
        }
        if options.perf_frozen_render && options.perf_seconds.is_none() {
            bail!("--perf-frozen-render requires --perf-seconds");
        }
        if options.perf_frozen_render && options.perf_flight.is_some() {
            bail!("--perf-frozen-render cannot be combined with --perf-flight");
        }
        if options.perf_frozen_render && options.perf_chunk_view_churn.is_some() {
            bail!("--perf-frozen-render cannot be combined with --perf-chunk-view-churn");
        }
        if options.multiview_proof && options.terrain_multiview_proof {
            bail!("--multiview-proof cannot be combined with --terrain-multiview-proof");
        }
        if [
            options.terrain_multiview_perf,
            options.sky_terrain_multiview_perf,
            options.sky_terrain_actors_multiview_perf,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
            > 1
        {
            bail!("multiview perf modes cannot be combined");
        }
        if (options.terrain_multiview_perf
            || options.sky_terrain_multiview_perf
            || options.sky_terrain_actors_multiview_perf)
            && (options.multiview_proof || options.terrain_multiview_proof)
        {
            bail!("multiview perf modes cannot be combined with multiview proof modes");
        }
        if options.full_frame_multiview
            && (options.multiview_proof
                || options.terrain_multiview_proof
                || options.terrain_multiview_perf
                || options.sky_terrain_multiview_perf
                || options.sky_terrain_actors_multiview_perf)
        {
            bail!(
                "--xr-full-frame-multiview cannot be combined with multiview proof or microbenchmark modes"
            );
        }
        if options.overlap_eye_submits
            && (options.full_frame_multiview
                || options.multiview_proof
                || options.terrain_multiview_proof
                || options.terrain_multiview_perf
                || options.sky_terrain_multiview_perf
                || options.sky_terrain_actors_multiview_perf)
        {
            bail!("--xr-overlap-eye-submits only applies to the per-eye full-frame path");
        }
        if options.overlap_runtime_prefetch
            && (options.full_frame_multiview
                || options.multiview_proof
                || options.terrain_multiview_proof
                || options.terrain_multiview_perf
                || options.sky_terrain_multiview_perf
                || options.sky_terrain_actors_multiview_perf)
        {
            bail!("--xr-overlap-runtime-prefetch only applies to the per-eye full-frame path");
        }
        if options.overlap_runtime_prefetch && options.perf_frozen_render {
            bail!("--xr-overlap-runtime-prefetch cannot be combined with --perf-frozen-render");
        }
        if matches!(options.perf_detail, AndroidXrPerfDetail::Minimal)
            && !options.frame_accounting_enabled
        {
            bail!("--perf-detail minimal requires --frame-accounting true");
        }
        if options.multiview_proof
            || options.terrain_multiview_proof
            || options.terrain_multiview_perf
            || options.sky_terrain_multiview_perf
            || options.sky_terrain_actors_multiview_perf
        {
            if options.session_smoke.is_some() {
                bail!("multiview proof modes cannot be combined with --session-smoke");
            }
            if options.perf_seconds.is_some()
                || options.perf_flight.is_some()
                || options.perf_settled_orbit.is_some()
                || options.perf_chunk_view_churn.is_some()
                || options.perf_settled_stationary
                || options.perf_frozen_render
                || options.perf_metrics
            {
                bail!("terrain multiview diagnostics cannot be combined with performance probes");
            }
        }
        resolve_android_xr_frame_overlap(&mut options)?;
        Ok(options)
    }

    fn resolve_android_xr_frame_overlap(options: &mut AndroidXrStartupOptions) -> Result<()> {
        let per_eye_full_frame = android_xr_uses_per_eye_full_frame_path(options);
        let older_overlap_probe = options.overlap_eye_submits || options.overlap_runtime_prefetch;
        match options.frame_overlap_mode {
            AndroidXrFrameOverlapMode::Default => {
                options.frame_overlap = per_eye_full_frame && !older_overlap_probe;
            }
            AndroidXrFrameOverlapMode::ExplicitOverlap => {
                if !per_eye_full_frame {
                    bail!("--xr-frame-overlap only applies to the per-eye full-frame path");
                }
                if older_overlap_probe {
                    bail!("--xr-frame-overlap cannot be combined with older overlap probe flags");
                }
                options.frame_overlap = true;
            }
            AndroidXrFrameOverlapMode::Serial => {
                if !per_eye_full_frame {
                    bail!("--xr-frame-serial only applies to the per-eye full-frame path");
                }
                if older_overlap_probe {
                    bail!("--xr-frame-serial cannot be combined with older overlap probe flags");
                }
                options.frame_overlap = false;
            }
        }
        Ok(())
    }

    fn android_xr_uses_per_eye_full_frame_path(options: &AndroidXrStartupOptions) -> bool {
        !(options.full_frame_multiview
            || options.multiview_proof
            || options.terrain_multiview_proof
            || options.terrain_multiview_perf
            || options.sky_terrain_multiview_perf
            || options.sky_terrain_actors_multiview_perf)
    }

    fn android_xr_startup_scene_defaults() -> StartupSceneOptions {
        let scene = XrSceneOptions::default();
        StartupSceneOptions {
            seed: scene.seed,
            chunk_x: scene.chunk_x,
            chunk_z: scene.chunk_z,
            render_distance: scene.render_distance,
            render_compile_worker_count: scene.render_compile_worker_count,
            render_compile_max_pending_jobs: scene.render_compile_max_pending_jobs,
            render_compile_capacity_request: Default::default(),
            movement_speed_multiplier: scene.movement_speed_multiplier,
            remote_addr: None,
            day_time_override: scene.day_time_override,
            freeze_time: scene.freeze_time,
            debug_passive_showcase: scene.debug_passive_showcase,
            lighting_enabled: scene.lighting_enabled,
            light_status_batch_size: scene.light_status_batch_size,
        }
    }

    fn android_xr_scene_options_from_startup(
        scene: StartupSceneOptions,
        storage: &StartupWorldStorageProjection,
    ) -> XrSceneOptions {
        XrSceneOptions {
            seed: scene.seed,
            chunk_x: scene.chunk_x,
            chunk_z: scene.chunk_z,
            render_distance: scene.render_distance,
            render_compile_worker_count: scene.render_compile_worker_count,
            render_compile_max_pending_jobs: scene.render_compile_max_pending_jobs,
            movement_speed_multiplier: scene.movement_speed_multiplier,
            day_time_override: scene.day_time_override,
            freeze_time: scene.freeze_time,
            debug_passive_showcase: scene.debug_passive_showcase,
            lighting_enabled: scene.lighting_enabled,
            light_status_batch_size: scene.light_status_batch_size,
            adaptive_chunk_publication_budget: true,
            far_lod: Default::default(),
            underwater_detection_mode: XrUnderwaterDetectionMode::default(),
            debug_ui_screen: None,
            skip_actors: false,
            world_root: storage.world_root.clone(),
            world_dir: storage.world_dir.clone(),
        }
    }

    fn validate_perf_motion_speed(flag: &str, speed_blocks_per_second: f64) -> Result<f64> {
        if !speed_blocks_per_second.is_finite() || speed_blocks_per_second <= 0.0 {
            bail!("{flag} must be a finite positive number");
        }
        Ok(speed_blocks_per_second)
    }

    fn validate_perf_churn_interval_seconds(flag: &str, interval_seconds: f64) -> Result<f64> {
        if !interval_seconds.is_finite() || interval_seconds <= 0.0 {
            bail!("{flag} must be a finite positive number");
        }
        Ok(interval_seconds)
    }

    fn validate_perf_churn_offset_chunks(flag: &str, offset_chunks: i32) -> Result<i32> {
        if offset_chunks <= 0 {
            bail!("{flag} must be greater than zero");
        }
        Ok(offset_chunks)
    }

    fn validate_display_refresh_rate(rate: f32) -> Result<f32> {
        if !rate.is_finite() || rate <= 0.0 {
            bail!("--xr-display-refresh-rate must be a finite positive number");
        }
        Ok(rate)
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
            .and_then(|value| super::normalize_android_legacy_remote_addr(&value))
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

    fn report_android_xr_failure(app: &AndroidApp, error: &anyhow::Error) {
        let message = format!("{error:#}");
        log::error!("MCLONE_ANDROID_XR_FAILURE: {message}");
        if let Err(report_error) = report_android_xr_failure_to_activity(app, &message) {
            log::warn!(
                "failed to surface Android XR startup failure in activity: {report_error:#}"
            );
        }
    }

    #[allow(unsafe_code)]
    fn report_android_xr_failure_to_activity(app: &AndroidApp, message: &str) -> Result<()> {
        let vm = app.vm_as_ptr() as *mut jni::sys::JavaVM;
        let activity = app.activity_as_ptr() as jni::sys::jobject;
        if vm.is_null() || activity.is_null() {
            bail!("null JVM or Activity");
        }

        let vm = unsafe { jni::JavaVM::from_raw(vm) };
        vm.attach_current_thread(|env| {
            let activity = unsafe { jni::objects::JObject::from_raw(env, activity) };
            let message = env.new_string(message)?;
            let message = jni::objects::JObject::from(message);
            env.call_method(
                &activity,
                jni::jni_str!("reportMcloneNativeFailure"),
                jni::jni_sig!("(Ljava/lang/String;)V"),
                &[jni::objects::JValue::Object(&message)],
            )?;
            Ok::<(), jni::errors::Error>(())
        })
        .context("call McloneXrActivity.reportMcloneNativeFailure")
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
                report_android_xr_failure(&app, &error);
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
        let mut startup_options = match parse_android_xr_startup_options(startup_argv.as_deref()) {
            Ok(options) => options,
            Err(error) => {
                report_android_xr_failure(&app, &error);
                return;
            }
        };
        if startup_options.default_world_root_enabled && startup_options.scene.world_root.is_none()
        {
            let storage = StartupWorldStorageProjection::from_parts(
                startup_options.default_world_root_enabled,
                startup_options.scene.world_root.clone(),
                startup_options.scene.world_dir.clone(),
            )
            .with_default_world_root(android_app_data_world_root(&app, "Android XR"));
            startup_options.scene.world_root = storage.world_root;
        }
        let startup_view_pose =
            match parse_android_xr_startup_view_pose(startup_view_pose_property.as_deref()) {
                Ok(value) => value,
                Err(error) => {
                    report_android_xr_failure(&app, &error);
                    return;
                }
            };
        let legacy_remote_addr = android_remote_addr();
        let remote_addr = startup_options
            .remote_addr
            .clone()
            .or_else(|| legacy_remote_addr.clone());
        let storage = StartupWorldStorageProjection::from_parts(
            startup_options.default_world_root_enabled,
            startup_options.scene.world_root.clone(),
            startup_options.scene.world_dir.clone(),
        );
        if let Err(error) = storage.validate_local_integrated_world(remote_addr.as_deref()) {
            report_android_xr_failure(&app, &error);
            return;
        }
        if remote_addr.is_some() {
            startup_options.scene.adaptive_chunk_publication_budget = false;
        }
        let scene_options = startup_options.scene.clone();
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
        if let Some(orbit) = startup_options.perf_settled_orbit {
            log::info!(
                "Android XR performance settled orbit: speed={:.3} blocks/s",
                orbit.speed_blocks_per_second
            );
        } else {
            log::info!("Android XR performance settled orbit: <none>");
        }
        if let Some(churn) = startup_options.perf_chunk_view_churn {
            log::info!(
                "Android XR performance chunk-view churn: interval={:.3}s offset_chunks={}",
                churn.interval_seconds,
                churn.offset_chunks
            );
        } else {
            log::info!("Android XR performance chunk-view churn: <none>");
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
            "Android XR performance metrics periodic: {}",
            startup_options.perf_metrics_periodic
        );
        log::info!(
            "Android XR performance detail: {}",
            startup_options.perf_detail.label()
        );
        log::info!(
            "Android XR frame accounting: {}",
            startup_options.frame_accounting_enabled
        );
        log::info!(
            "Android XR multiview proof: {}",
            startup_options.multiview_proof
        );
        log::info!(
            "Android XR terrain multiview proof: {}",
            startup_options.terrain_multiview_proof
        );
        log::info!(
            "Android XR terrain multiview perf: {}",
            startup_options.terrain_multiview_perf
        );
        log::info!(
            "Android XR sky terrain multiview perf: {}",
            startup_options.sky_terrain_multiview_perf
        );
        log::info!(
            "Android XR sky terrain actors multiview perf: {}",
            startup_options.sky_terrain_actors_multiview_perf
        );
        log::info!("Android XR skip actors: {}", startup_options.skip_actors);
        log::info!(
            "Android XR full-frame multiview: {}",
            startup_options.full_frame_multiview
        );
        log::info!(
            "Android XR frame overlap mode: {}",
            startup_options.frame_overlap_mode.label()
        );
        log::info!(
            "Android XR frame overlap: {}",
            startup_options.frame_overlap
        );
        log::info!(
            "Android XR overlap eye submits: {}",
            startup_options.overlap_eye_submits
        );
        log::info!(
            "Android XR overlap runtime prefetch: {}",
            startup_options.overlap_runtime_prefetch
        );
        log::info!(
            "Android XR render section upload budget: {}",
            format_optional_usize(startup_options.render_section_upload_budget)
        );
        log::info!(
            "Android XR render section accept budget: {}",
            format_optional_usize(startup_options.render_section_accept_budget)
        );
        log::info!(
            "Android XR render completed-result accept budget: {}",
            format_optional_usize(startup_options.render_completed_result_accept_budget)
        );
        log::info!(
            "Android XR fixed foveation: {}",
            startup_options.xr_foveation.label()
        );
        log::info!(
            "Android XR render scale: {:.3}",
            startup_options.xr_render_scale
        );
        log::info!(
            "Android XR scene options: seed={} center=({}, {}) render_distance={} render_compile_workers={} render_compile_max_pending_jobs={:?} day_time={:?} freeze_time={} lighting={} light_status_batch_size={} adaptive_chunk_publication_budget={} skip_actors={}",
            scene_options.seed,
            scene_options.chunk_x,
            scene_options.chunk_z,
            scene_options.render_distance,
            scene_options.render_compile_worker_count,
            scene_options.render_compile_max_pending_jobs,
            scene_options.day_time_override,
            scene_options.freeze_time,
            scene_options.lighting_enabled,
            scene_options.light_status_batch_size,
            scene_options.adaptive_chunk_publication_budget,
            scene_options.skip_actors
        );
        log::info!(
            "Android XR world storage: default_root_enabled={} world_root={} world_dir={}",
            startup_options.default_world_root_enabled,
            format_optional_startup_path(scene_options.world_root.as_deref(), "none"),
            format_optional_startup_path(scene_options.world_dir.as_deref(), "none")
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
                report_android_xr_failure(&app, &error);
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
            startup_options.perf_settled_orbit,
            startup_options.perf_chunk_view_churn,
            startup_options.perf_settled_stationary,
            startup_options.perf_frozen_render,
            startup_options.perf_metrics,
            startup_options.perf_metrics_periodic,
            startup_options.perf_detail,
            startup_options.frame_accounting_enabled,
            startup_options.multiview_proof,
            startup_options.terrain_multiview_proof,
            startup_options.terrain_multiview_perf,
            startup_options.sky_terrain_multiview_perf,
            startup_options.sky_terrain_actors_multiview_perf,
            startup_options.full_frame_multiview,
            startup_options.frame_overlap,
            startup_options.overlap_eye_submits,
            startup_options.overlap_runtime_prefetch,
            startup_options.render_section_upload_budget,
            startup_options.render_section_accept_budget,
            startup_options.render_completed_result_accept_budget,
            startup_options.xr_foveation,
            startup_options.xr_render_scale,
            startup_options.xr_display_refresh_rate,
        ) {
            report_android_xr_failure(&app, &error);
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
        perf_settled_orbit: Option<AndroidXrPerfOrbit>,
        perf_chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
        perf_settled_stationary: bool,
        perf_frozen_render: bool,
        perf_metrics: bool,
        perf_metrics_periodic: bool,
        perf_detail: AndroidXrPerfDetail,
        frame_accounting_enabled: bool,
        multiview_proof: bool,
        terrain_multiview_proof: bool,
        terrain_multiview_perf: bool,
        sky_terrain_multiview_perf: bool,
        sky_terrain_actors_multiview_perf: bool,
        full_frame_multiview: bool,
        frame_overlap: bool,
        overlap_eye_submits: bool,
        overlap_runtime_prefetch: bool,
        render_section_upload_budget: Option<usize>,
        render_section_accept_budget: Option<usize>,
        render_completed_result_accept_budget: Option<usize>,
        xr_foveation: AndroidXrFoveation,
        xr_render_scale: f32,
        xr_display_refresh_rate: Option<f32>,
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
            "OpenXR extensions: android_create_instance=true vulkan_enable2=true khr_android_thread_settings={} fb_passthrough={} fb_alpha_blend={} fb_display_refresh_rate={} fb_swapchain_update_state={} fb_foveation={} fb_foveation_configuration={} fb_foveation_vulkan={} fb_render_model={} ext_hand_tracking={} fb_hand_tracking_mesh={} fb_hand_tracking_aim={} meta_virtual_keyboard={} fb_spatial_entity={} fb_spatial_entity_query={} fb_scene={} fb_scene_capture={} fb_spatial_entity_container={} meta_spatial_entity_mesh={} fb_body_tracking={} meta_body_tracking_full_body={} meta_performance_metrics={} ext_debug_utils={}",
            available.khr_android_thread_settings,
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
        if available.khr_android_thread_settings {
            enabled_extensions.khr_android_thread_settings = true;
            log::info!("Enabling XR_KHR_android_thread_settings");
        }
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
        if xr_foveation.is_enabled()
            && !(available.fb_swapchain_update_state
                && available.fb_foveation
                && available.fb_foveation_configuration
                && available.fb_foveation_vulkan)
        {
            bail!(
                "--xr-foveation {} requires XR_FB_swapchain_update_state, XR_FB_foveation, XR_FB_foveation_configuration, and XR_FB_foveation_vulkan",
                xr_foveation.label()
            );
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
        register_android_xr_render_thread(&graphics.session);
        log::info!(
            "OpenXR Vulkan session: physical_device='{}' api={} queue_family={}",
            graphics.physical_device_name,
            graphics.physical_device_api_version,
            graphics.queue_family_index
        );
        log::info!(
            "OpenXR wgpu features: multiview={}",
            graphics
                .device
                .features()
                .contains(wgpu::Features::MULTIVIEW)
        );
        let multiview = graphics.multiview_diagnostics;
        log::info!(
            "OpenXR Vulkan multiview diagnostics: instance_properties2_ext={} device_khr_multiview_ext={} raw_feature={} raw_geometry_shader={} raw_tessellation_shader={} max_views={} max_instance_index={} wgpu_adapter={} wgpu_device={}",
            multiview.instance_properties2_extension,
            multiview.device_khr_multiview_extension,
            multiview.raw_feature_multiview,
            multiview.raw_feature_multiview_geometry_shader,
            multiview.raw_feature_multiview_tessellation_shader,
            multiview.raw_max_multiview_view_count,
            multiview.raw_max_multiview_instance_index,
            multiview.wgpu_adapter_multiview,
            multiview.wgpu_device_multiview
        );
        let mut display_refresh = mclone_xr_host::query_display_refresh_snapshot(
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
        if let Some(rate) = xr_display_refresh_rate {
            request_display_refresh_rate(&graphics.session, &mut display_refresh, rate)?;
        }
        let stage = mclone_xr_host::create_stage_reference_space(&graphics.session)?;
        log::info!("OpenXR reference space: STAGE");

        let (recommended_eye_width, recommended_eye_height) = stereo_config.primary_eye_size();
        let [eye_width, eye_height] = scaled_frame_size(
            [recommended_eye_width, recommended_eye_height],
            xr_render_scale,
        );
        log::info!(
            "OpenXR render scale: scale={:.3} recommended_eye={}x{} active_eye={}x{} pixel_fraction={:.3}",
            xr_render_scale,
            recommended_eye_width,
            recommended_eye_height,
            eye_width,
            eye_height,
            xr_render_scale * xr_render_scale
        );
        if multiview_proof || terrain_multiview_proof {
            let mut stereo_target = graphics_vulkan::create_stereo(
                &graphics.device,
                &graphics.session,
                eye_width,
                eye_height,
                XR_COLOR_FORMAT,
                XR_SAMPLE_COUNT,
                xr_foveation.level_profile(),
            )
            .context("create OpenXR stereo array swapchain")?;
            log::info!(
                "OpenXR multiview proof swapchain: color_format={XR_COLOR_FORMAT:?} eye={}x{} images={} layers={}",
                eye_width,
                eye_height,
                stereo_target.texture_count(),
                stereo_target.array_size()
            );
            if multiview_proof {
                log::info!("MCLONE_ANDROID_XR_SESSION_READY");
                return run_multiview_proof_loop(
                    app,
                    &mut graphics,
                    &stage,
                    environment_blend_mode,
                    &mut stereo_target,
                );
            }

            log::info!("MCLONE_ANDROID_XR_SESSION_READY");
            let mut terrain = create_android_xr_terrain_state(
                &graphics.device,
                &graphics.queue,
                runtime_assets,
                startup_view_pose,
                scene_options.clone(),
                render_options,
                remote_addr,
            )
            .context("initialize Android XR terrain multiview proof runtime")?;
            terrain.set_display_refresh_hz(display_refresh.current_rate);
            return run_terrain_multiview_proof_loop(
                app,
                &mut graphics,
                &stage,
                environment_blend_mode,
                &mut stereo_target,
                &mut terrain,
            );
        }

        if terrain_multiview_perf || sky_terrain_multiview_perf || sky_terrain_actors_multiview_perf
        {
            log::info!("MCLONE_ANDROID_XR_SESSION_READY");
            let mut terrain = create_android_xr_terrain_state(
                &graphics.device,
                &graphics.queue,
                runtime_assets,
                startup_view_pose,
                scene_options.clone(),
                render_options,
                remote_addr,
            )
            .context("initialize Android XR terrain multiview perf runtime")?;
            terrain.set_display_refresh_hz(display_refresh.current_rate);
            return run_terrain_multiview_perf_loop(
                app,
                &mut graphics,
                &stage,
                environment_blend_mode,
                eye_width,
                eye_height,
                &mut terrain,
                sky_terrain_multiview_perf,
                sky_terrain_actors_multiview_perf,
            );
        }

        if full_frame_multiview {
            let mut stereo_target = graphics_vulkan::create_stereo(
                &graphics.device,
                &graphics.session,
                eye_width,
                eye_height,
                XR_COLOR_FORMAT,
                XR_SAMPLE_COUNT,
                xr_foveation.level_profile(),
            )
            .context("create OpenXR full-frame multiview stereo array swapchain")?;
            let mut multiview_depth =
                ChunkMultiviewDepthTarget::new(&graphics.device, eye_width, eye_height);
            log::info!(
                "OpenXR full-frame multiview swapchain: color_format={XR_COLOR_FORMAT:?} eye={}x{} images={} layers={}",
                eye_width,
                eye_height,
                stereo_target.texture_count(),
                stereo_target.array_size()
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
                scene_options.clone(),
                render_options,
                remote_addr,
            )
            .context("initialize Android XR full-frame multiview terrain runtime")?;
            let terrain_summary = terrain.frame_summary();
            terrain.set_display_refresh_hz(display_refresh.current_rate);
            terrain.set_render_split_timing_enabled(perf_seconds.is_some());
            terrain.set_render_section_upload_budget(render_section_upload_budget);
            terrain.set_render_section_accept_budget(render_section_accept_budget);
            terrain
                .set_render_completed_result_accept_budget(render_completed_result_accept_budget);
            log::info!(
                "MCLONE_ANDROID_XR_TERRAIN_READY sections={} indices={} actors={}",
                terrain_summary.section_count,
                terrain_summary.index_count,
                terrain_summary.actor_count
            );

            return run_mclone_frame_loop(
                app,
                &mut graphics,
                &stage,
                environment_blend_mode,
                AndroidXrFrameTargets::Multiview {
                    stereo_target: &mut stereo_target,
                    depth: &mut multiview_depth,
                },
                &mut terrain,
                &controller_actions,
                session_smoke,
                perf_seconds,
                perf_flight,
                perf_settled_orbit,
                perf_chunk_view_churn,
                perf_settled_stationary,
                perf_frozen_render,
                perf_metrics,
                perf_metrics_periodic,
                perf_detail,
                frame_accounting_enabled,
                [scene_options.chunk_x, scene_options.chunk_z],
                startup_view_pose,
                scene_options.render_distance,
                scene_options.render_compile_worker_count,
                scene_options.render_compile_max_pending_jobs,
                scene_options.skip_actors,
                scene_options.adaptive_chunk_publication_budget,
                display_refresh,
                render_section_upload_budget,
                render_section_accept_budget,
                render_completed_result_accept_budget,
                xr_foveation,
                xr_render_scale,
            );
        }

        let mut left_eye = graphics_vulkan::create_eye(
            &graphics.device,
            &graphics.session,
            eye_width,
            eye_height,
            XR_COLOR_FORMAT,
            XR_DEPTH_FORMAT,
            XR_SAMPLE_COUNT,
            xr_foveation.level_profile(),
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
            xr_foveation.level_profile(),
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
            scene_options.clone(),
            render_options,
            remote_addr,
        )
        .context("initialize Android XR terrain runtime")?;
        let terrain_summary = terrain.frame_summary();
        terrain.set_display_refresh_hz(display_refresh.current_rate);
        terrain.set_render_split_timing_enabled(perf_seconds.is_some());
        terrain.set_defer_eye_waits_enabled(
            frame_overlap || overlap_eye_submits || overlap_runtime_prefetch,
        );
        terrain.set_overlap_runtime_prefetch_enabled(frame_overlap || overlap_runtime_prefetch);
        terrain.set_render_section_upload_budget(render_section_upload_budget);
        terrain.set_render_section_accept_budget(render_section_accept_budget);
        terrain.set_render_completed_result_accept_budget(render_completed_result_accept_budget);
        log::info!("Android XR frame overlap active: {}", frame_overlap);
        log::info!(
            "Android XR per-eye submit overlap active: {}",
            overlap_eye_submits
        );
        log::info!(
            "Android XR per-eye runtime prefetch active: {}",
            overlap_runtime_prefetch
        );
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
            AndroidXrFrameTargets::PerEye {
                left_eye: &mut left_eye,
                right_eye: &mut right_eye,
                frame_overlap,
                overlap_eye_submits,
                overlap_runtime_prefetch,
            },
            &mut terrain,
            &controller_actions,
            session_smoke,
            perf_seconds,
            perf_flight,
            perf_settled_orbit,
            perf_chunk_view_churn,
            perf_settled_stationary,
            perf_frozen_render,
            perf_metrics,
            perf_metrics_periodic,
            perf_detail,
            frame_accounting_enabled,
            [scene_options.chunk_x, scene_options.chunk_z],
            startup_view_pose,
            scene_options.render_distance,
            scene_options.render_compile_worker_count,
            scene_options.render_compile_max_pending_jobs,
            scene_options.skip_actors,
            scene_options.adaptive_chunk_publication_budget,
            display_refresh,
            render_section_upload_budget,
            render_section_accept_budget,
            render_completed_result_accept_budget,
            xr_foveation,
            xr_render_scale,
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
                android_xr_host_options(&scene_options),
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
                scene_options.clone(),
                runtime,
                render_options,
                actor_assets.atlas.clone(),
                actor_assets.figures.clone(),
                &asset_source,
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
                actor_assets.atlas.clone(),
                actor_assets.figures.clone(),
                &asset_source,
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
            SessionStartRequest::CreateLocalWorld { options } => {
                let mut scene_options = scene_options;
                scene_options.seed = options.seed;
                AndroidXrSceneRuntime::local_with_mesh_assets(
                    android_xr_local_options(&scene_options),
                    mesh_assets,
                )
                .context("failed to initialize Android XR replacement local runtime")
            }
            SessionStartRequest::OpenLocalWorld { .. } => {
                AndroidXrSceneRuntime::local_with_mesh_assets(
                    android_xr_local_options(&scene_options),
                    mesh_assets,
                )
                .context("failed to initialize Android XR replacement persistent local runtime")
            }
            SessionStartRequest::JoinRemote { endpoint } => {
                let session = AndroidXrRemoteServerSession::connect(endpoint.address.as_str())?;
                AndroidXrSceneRuntime::remote_dedicated_with_mesh_assets(
                    endpoint.clone(),
                    android_xr_host_options(&scene_options),
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

    fn android_xr_local_options(scene: &XrSceneOptions) -> LocalSingleViewSceneOptions {
        let storage = IntegratedWorldSessionStorage::from_world_dir(scene.world_dir.as_deref())
            .with_adaptive_chunk_publication_budget(scene.adaptive_chunk_publication_budget);
        LocalSingleViewSceneOptions::new(scene.seed, scene.center(), scene.render_distance)
            .with_initial_spawn_center()
            .with_day_time(scene.day_time_override)
            .with_freeze_time(scene.freeze_time)
            .with_debug_passive_showcase(scene.debug_passive_showcase)
            .with_lighting_enabled(scene.lighting_enabled)
            .with_light_status_batch_size(scene.light_status_batch_size)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
            .with_integrated_world_session_storage(storage)
    }

    fn android_xr_host_options(scene: &XrSceneOptions) -> SingleViewHostOptions {
        SingleViewHostOptions::new(scene.center(), scene.render_distance)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
    }

    fn request_display_refresh_rate(
        session: &xr::Session<graphics_vulkan::AppGraphics>,
        display_refresh: &mut XrDisplayRefreshSnapshot,
        requested_rate: f32,
    ) -> Result<()> {
        if !display_refresh.extension_supported {
            bail!("--xr-display-refresh-rate requires XR_FB_display_refresh_rate");
        }
        if !display_refresh
            .supported_rates
            .iter()
            .any(|rate| mclone_xr_host::refresh_rates_match(*rate, requested_rate))
        {
            bail!(
                "--xr-display-refresh-rate {requested_rate:.1} is not in supported rates: {}",
                mclone_xr_host::display_refresh_rates_label(&display_refresh.supported_rates)
            );
        }
        log::info!("Requesting OpenXR display refresh: {requested_rate:.1} Hz");
        session
            .request_display_refresh_rate(requested_rate)
            .with_context(|| format!("request OpenXR display refresh {requested_rate:.1} Hz"))?;
        for _ in 0..30 {
            let current = mclone_xr_host::query_current_display_refresh_rate(session);
            display_refresh.current_rate = current;
            if current.is_some_and(|rate| mclone_xr_host::refresh_rates_match(rate, requested_rate))
            {
                log::info!("OpenXR display refresh after request: {requested_rate:.1} Hz");
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        log::warn!(
            "OpenXR display refresh query stayed at {} after requesting {requested_rate:.1} Hz; using requested rate for perf budget",
            format_optional_hz(display_refresh.current_rate)
        );
        display_refresh.current_rate = Some(requested_rate);
        Ok(())
    }

    #[allow(unsafe_code)]
    fn register_android_xr_render_thread(session: &xr::Session<graphics_vulkan::AppGraphics>) {
        let Some(thread_settings) = session.instance().exts().khr_android_thread_settings else {
            log::info!(
                "OpenXR Android thread settings unavailable; renderer thread not registered"
            );
            return;
        };

        let raw_thread_id = unsafe { libc::syscall(libc::SYS_gettid) };
        let Ok(thread_id) = u32::try_from(raw_thread_id) else {
            log::warn!("OpenXR Android thread settings skipped: invalid gettid={raw_thread_id}");
            return;
        };
        let result = unsafe {
            (thread_settings.set_android_application_thread)(
                session.as_raw(),
                xr::sys::AndroidThreadTypeKHR::RENDERER_MAIN,
                thread_id,
            )
        };
        if result == xr::sys::Result::SUCCESS {
            log::info!(
                "OpenXR Android thread settings: registered renderer main thread tid={thread_id}"
            );
        } else {
            log::warn!(
                "OpenXR Android thread settings failed for renderer main thread tid={thread_id}: {result:?}"
            );
        }
    }

    #[derive(Debug)]
    struct AndroidXrRemoteServerSession {
        addr: String,
        session: NativeClientIoSession,
    }

    impl AndroidXrRemoteServerSession {
        fn connect(addr: impl Into<String>) -> Result<Self> {
            let addr = addr.into();
            let session = NativeClientIoSession::connect(addr.as_str())
                .with_context(|| format!("failed to connect to Android XR remote server {addr}"))?;
            Ok(Self { addr, session })
        }
    }

    impl RemoteDedicatedServerSession for AndroidXrRemoteServerSession {
        fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
            self.session.send_command_only(command).with_context(|| {
                format!(
                    "failed to send command to Android XR remote server {}",
                    self.addr
                )
            })
        }

        fn drain_command_updates(&mut self) -> Result<Vec<ServerUpdate>> {
            self.session.drain_command_updates().with_context(|| {
                format!(
                    "failed to drain updates from Android XR remote server {}",
                    self.addr
                )
            })
        }

        fn try_drain_command_updates(&mut self) -> Result<Option<Vec<ServerUpdate>>> {
            self.session.try_drain_command_updates().with_context(|| {
                format!(
                    "failed to poll updates from Android XR remote server {}",
                    self.addr
                )
            })
        }

        fn drain_command_update_batch(&mut self) -> Result<RemoteCommandUpdateBatch> {
            self.session
                .drain_update_batch()
                .map(remote_batch_from_native)
                .with_context(|| {
                    format!(
                        "failed to drain updates from Android XR remote server {}",
                        self.addr
                    )
                })
        }

        fn try_drain_command_update_batch(&mut self) -> Result<Option<RemoteCommandUpdateBatch>> {
            self.session
                .try_drain_update_batch()
                .map(|batch| batch.map(remote_batch_from_native))
                .with_context(|| {
                    format!(
                        "failed to poll updates from Android XR remote server {}",
                        self.addr
                    )
                })
        }

        fn reconnect(&mut self) -> Result<()> {
            self.session =
                NativeClientIoSession::connect(self.addr.as_str()).with_context(|| {
                    format!(
                        "failed to reconnect to Android XR remote server {}",
                        self.addr
                    )
                })?;
            Ok(())
        }
    }

    fn remote_batch_from_native(batch: NativeServerUpdateBatch) -> RemoteCommandUpdateBatch {
        let queued_age = batch.queued_age();
        RemoteCommandUpdateBatch {
            response_sequence: Some(batch.response_sequence),
            producer_read_ms: batch.producer_read_ms,
            producer_decode_ms: batch.producer_decode_ms,
            updates: batch
                .updates
                .into_iter()
                .map(|update| RemoteCommandUpdate {
                    update: update.update,
                    encoded_len: Some(update.encoded_len),
                    queued_age,
                    response_sequence: Some(update.response_sequence),
                    producer_read_ms: update.producer_read_ms,
                    producer_decode_ms: update.producer_decode_ms,
                })
                .collect(),
        }
    }

    fn run_multiview_proof_loop(
        app: &AndroidApp,
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        stereo_target: &mut graphics_vulkan::OpenXrStereoState,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();

        loop {
            if !poll_android_events(app, Some(Duration::from_millis(0)))? {
                log::info!("Android activity destroyed; exiting OpenXR multiview proof loop");
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
                        "OpenXR multiview proof requested exit: submitted={} runtime_frames={} skipped={}",
                        frame_stats.submitted_frames,
                        frame_stats.runtime_frames,
                        frame_stats.skipped_frames
                    );
                    return Ok(());
                }
                OpenXrPollStatus::Idle if !session_running => {
                    if !poll_android_events(app, Some(SESSION_IDLE_POLL_INTERVAL))? {
                        log::info!(
                            "Android activity destroyed while waiting for OpenXR READY in multiview proof"
                        );
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
            if frame_state.should_render {
                match render_multiview_proof_frame(
                    graphics,
                    stage,
                    environment_blend_mode,
                    frame_state.predicted_display_time,
                    stereo_target,
                ) {
                    Ok(proof) => {
                        frame_stats.record_submitted_frame();
                        log::info!(
                            "MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY submitted={} runtime_frames={} skipped={} eye={}x{} layers={} multiview={} private_left_red={} private_right_green={} swapchain_left_red={} swapchain_right_green={} expected_disparity_px={:.1} tolerance_px={:.1} left_actual=({:.1},{:.1}) left_expected=({:.1},{:.1}) left_error_px={:.1} left_pixels={} right_actual=({:.1},{:.1}) right_expected=({:.1},{:.1}) right_error_px={:.1} right_pixels={}",
                            frame_stats.submitted_frames,
                            frame_stats.runtime_frames,
                            frame_stats.skipped_frames,
                            stereo_target.width,
                            stereo_target.height,
                            stereo_target.array_size(),
                            graphics
                                .device
                                .features()
                                .contains(wgpu::Features::MULTIVIEW),
                            proof.private_multiview.left_red_pixels,
                            proof.private_multiview.right_green_pixels,
                            proof.swapchain_multiview.left_red_pixels,
                            proof.swapchain_multiview.right_green_pixels,
                            proof.projection.expected_disparity_px,
                            proof.projection.tolerance_px,
                            proof.projection.left.actual_px[0],
                            proof.projection.left.actual_px[1],
                            proof.projection.left.expected_px[0],
                            proof.projection.left.expected_px[1],
                            proof.projection.left.error_px,
                            proof.projection.left.pixel_count,
                            proof.projection.right.actual_px[0],
                            proof.projection.right.actual_px[1],
                            proof.projection.right.expected_px[0],
                            proof.projection.right.expected_px[1],
                            proof.projection.right.error_px,
                            proof.projection.right.pixel_count
                        );
                        return Ok(());
                    }
                    Err(error) => {
                        let _ = mclone_xr_host::end_frame_with_layers(
                            &mut graphics.frame_stream,
                            frame_state.predicted_display_time,
                            environment_blend_mode,
                            &[],
                        );
                        return Err(error);
                    }
                }
            } else {
                if let Err(error) = mclone_xr_host::end_skipped_frame(
                    &mut graphics.frame_stream,
                    frame_state.predicted_display_time,
                    environment_blend_mode,
                    &mut frame_stats,
                ) {
                    return Err(error);
                }
            }
        }
    }

    struct TerrainPerfEyeTarget {
        _color: wgpu::Texture,
        color_view: wgpu::TextureView,
        depth: ChunkDepthTarget,
        size: [u32; 2],
    }

    impl TerrainPerfEyeTarget {
        fn new(device: &wgpu::Device, width: u32, height: u32, label: &'static str) -> Self {
            let width = width.max(1);
            let height = height.max(1);
            let color = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: XR_COLOR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let color_view = color.create_view(&Default::default());
            Self {
                _color: color,
                color_view,
                depth: ChunkDepthTarget::new(device, width, height),
                size: [width, height],
            }
        }

        fn target(&self) -> XrTerrainEyeTarget<'_> {
            XrTerrainEyeTarget {
                color_view: &self.color_view,
                depth: &self.depth,
                size: self.size,
            }
        }
    }

    struct TerrainPerfMultiviewTarget {
        _color: wgpu::Texture,
        color_view: wgpu::TextureView,
        depth: ChunkMultiviewDepthTarget,
        size: [u32; 2],
    }

    impl TerrainPerfMultiviewTarget {
        fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
            let width = width.max(1);
            let height = height.max(1);
            let color = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mclone_xr_terrain_perf_multiview_color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: XR_COLOR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let color_view = color.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mclone_xr_terrain_perf_multiview_color_view"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                base_array_layer: 0,
                array_layer_count: Some(2),
                ..Default::default()
            });
            Self {
                _color: color,
                color_view,
                depth: ChunkMultiviewDepthTarget::new(device, width, height),
                size: [width, height],
            }
        }

        fn target(&self) -> XrTerrainMultiviewTarget<'_> {
            XrTerrainMultiviewTarget {
                color_view: &self.color_view,
                depth: &self.depth,
                size: self.size,
            }
        }
    }

    struct TerrainPerfTargets {
        left: TerrainPerfEyeTarget,
        right: TerrainPerfEyeTarget,
        multiview: TerrainPerfMultiviewTarget,
    }

    impl TerrainPerfTargets {
        fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
            Self {
                left: TerrainPerfEyeTarget::new(
                    device,
                    width,
                    height,
                    "mclone_xr_terrain_perf_left_color",
                ),
                right: TerrainPerfEyeTarget::new(
                    device,
                    width,
                    height,
                    "mclone_xr_terrain_perf_right_color",
                ),
                multiview: TerrainPerfMultiviewTarget::new(device, width, height),
            }
        }
    }

    #[derive(Clone, Copy)]
    struct TerrainMultiviewPerfFrame {
        summary: mclone_xr_scene::XrTerrainMultiviewFrameSummary,
        ready: bool,
    }

    struct TerrainMultiviewPerfSummary {
        terrain: mclone_xr_scene::XrTerrainMultiviewFrameSummary,
        stereo_ms: Vec<f64>,
        multiview_ms: Vec<f64>,
    }

    impl TerrainMultiviewPerfSummary {
        fn stereo_stats(&self) -> SampleStats {
            SampleStats::from_samples(&self.stereo_ms)
        }

        fn multiview_stats(&self) -> SampleStats {
            SampleStats::from_samples(&self.multiview_ms)
        }

        fn speedup(&self) -> f64 {
            let stereo = self.stereo_stats().avg_ms;
            let multiview = self.multiview_stats().avg_ms;
            if multiview > 0.0 {
                stereo / multiview
            } else {
                0.0
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct SampleStats {
        avg_ms: f64,
        min_ms: f64,
        p50_ms: f64,
        p95_ms: f64,
        max_ms: f64,
    }

    impl SampleStats {
        fn from_samples(samples: &[f64]) -> Self {
            if samples.is_empty() {
                return Self::default();
            }
            let mut sorted = samples.to_vec();
            sorted.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
            let avg_ms = sorted.iter().sum::<f64>() / sorted.len() as f64;
            Self {
                avg_ms,
                min_ms: sorted[0],
                p50_ms: percentile_sorted_ms(&sorted, 0.50, PercentileMethod::NearestRank),
                p95_ms: percentile_sorted_ms(&sorted, 0.95, PercentileMethod::NearestRank),
                max_ms: *sorted.last().expect("non-empty samples checked above"),
            }
        }
    }

    fn run_terrain_multiview_perf_loop(
        app: &AndroidApp,
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        eye_width: u32,
        eye_height: u32,
        terrain: &mut AndroidXrTerrainState,
        include_sky: bool,
        include_actors: bool,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();
        let targets = TerrainPerfTargets::new(&graphics.device, eye_width, eye_height);
        let perf_start = Instant::now();

        loop {
            if perf_start.elapsed() > Duration::from_secs(60) {
                bail!(
                    "terrain multiview perf timed out: submitted={} runtime_frames={} skipped={} sections={}",
                    frame_stats.submitted_frames,
                    frame_stats.runtime_frames,
                    frame_stats.skipped_frames,
                    terrain.frame_summary().section_count
                );
            }
            if !poll_android_events(app, Some(Duration::from_millis(0)))? {
                log::info!(
                    "Android activity destroyed; exiting OpenXR terrain multiview perf loop"
                );
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
                        "OpenXR terrain multiview perf requested exit: submitted={} runtime_frames={} skipped={}",
                        frame_stats.submitted_frames,
                        frame_stats.runtime_frames,
                        frame_stats.skipped_frames
                    );
                    return Ok(());
                }
                OpenXrPollStatus::Idle if !session_running => {
                    if !poll_android_events(app, Some(SESSION_IDLE_POLL_INTERVAL))? {
                        log::info!(
                            "Android activity destroyed while waiting for OpenXR READY in terrain multiview perf"
                        );
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
            if frame_state.should_render {
                let frame_result = render_terrain_multiview_perf_frame(
                    graphics,
                    stage,
                    frame_state.predicted_display_time,
                    &targets,
                    terrain,
                    include_sky,
                    include_actors,
                );
                let end_result = mclone_xr_host::end_frame_with_layers(
                    &mut graphics.frame_stream,
                    frame_state.predicted_display_time,
                    environment_blend_mode,
                    &[],
                );
                let maybe_summary = match frame_result {
                    Ok(summary) => summary,
                    Err(error) => {
                        let _ = end_result;
                        return Err(error);
                    }
                };
                end_result.context("end terrain multiview perf OpenXR frame")?;
                frame_stats.record_submitted_frame();
                if let Some(summary) = maybe_summary {
                    let stereo = summary.stereo_stats();
                    let multiview = summary.multiview_stats();
                    let delta_ms = stereo.avg_ms - multiview.avg_ms;
                    let marker = if include_actors {
                        "MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF_SUMMARY"
                    } else if include_sky {
                        "MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF_SUMMARY"
                    } else {
                        "MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY"
                    };
                    log::info!(
                        "{} samples={} warmup={} eye={}x{} sections={} left_drawn_sections={} right_drawn_sections={} left_drawn_indices={} right_drawn_indices={} actors={} drawn_actors={} stereo_avg_ms={:.3} stereo_min_ms={:.3} stereo_p50_ms={:.3} stereo_p95_ms={:.3} stereo_max_ms={:.3} multiview_avg_ms={:.3} multiview_min_ms={:.3} multiview_p50_ms={:.3} multiview_p95_ms={:.3} multiview_max_ms={:.3} delta_avg_ms={:.3} speedup={:.3}",
                        marker,
                        TERRAIN_MULTIVIEW_PERF_SAMPLE_FRAMES,
                        TERRAIN_MULTIVIEW_PERF_WARMUP_FRAMES,
                        eye_width,
                        eye_height,
                        summary.terrain.section_count,
                        summary.terrain.left.drawn_section_count,
                        summary.terrain.right.drawn_section_count,
                        summary.terrain.left.drawn_index_count,
                        summary.terrain.right.drawn_index_count,
                        summary.terrain.actor_count,
                        summary.terrain.drawn_actor_count,
                        stereo.avg_ms,
                        stereo.min_ms,
                        stereo.p50_ms,
                        stereo.p95_ms,
                        stereo.max_ms,
                        multiview.avg_ms,
                        multiview.min_ms,
                        multiview.p50_ms,
                        multiview.p95_ms,
                        multiview.max_ms,
                        delta_ms,
                        summary.speedup()
                    );
                    return Ok(());
                }
            } else if let Err(error) = mclone_xr_host::end_skipped_frame(
                &mut graphics.frame_stream,
                frame_state.predicted_display_time,
                environment_blend_mode,
                &mut frame_stats,
            ) {
                return Err(error);
            }
        }
    }

    fn render_terrain_multiview_perf_frame(
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        predicted_display_time: xr::Time,
        targets: &TerrainPerfTargets,
        terrain: &mut AndroidXrTerrainState,
        include_sky: bool,
        include_actors: bool,
    ) -> Result<Option<TerrainMultiviewPerfSummary>> {
        let stereo_views =
            mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;
        let startup_summary = terrain
            .render_terrain_multiview_frame(
                &graphics.device,
                &graphics.queue,
                [stereo_views.left, stereo_views.right],
                targets.multiview.target(),
            )
            .context("warm up terrain multiview perf scene")?;
        let readiness = TerrainMultiviewPerfFrame {
            summary: startup_summary,
            ready: !terrain.frame_summary().local_startup_active
                && startup_summary.left.drawn_index_count > 0
                && startup_summary.right.drawn_index_count > 0,
        };
        if !readiness.ready {
            return Ok(None);
        }

        for _ in 0..TERRAIN_MULTIVIEW_PERF_WARMUP_FRAMES {
            if include_sky {
                let _ = terrain.render_sky_terrain_stereo_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets.left.target(),
                    targets.right.target(),
                )?;
            } else if include_actors {
                let _ = terrain.render_sky_terrain_actors_stereo_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets.left.target(),
                    targets.right.target(),
                )?;
            } else {
                let _ = terrain.render_terrain_stereo_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets.left.target(),
                    targets.right.target(),
                )?;
            }
            if include_sky {
                let _ = terrain.render_sky_terrain_multiview_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets.multiview.target(),
                )?;
            } else if include_actors {
                let _ = terrain.render_sky_terrain_actors_multiview_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets.multiview.target(),
                )?;
            } else {
                let _ = terrain.render_terrain_multiview_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets.multiview.target(),
                )?;
            }
        }

        let mut stereo_ms = Vec::with_capacity(TERRAIN_MULTIVIEW_PERF_SAMPLE_FRAMES);
        let mut multiview_ms = Vec::with_capacity(TERRAIN_MULTIVIEW_PERF_SAMPLE_FRAMES);
        let mut latest_summary = readiness.summary;
        for index in 0..TERRAIN_MULTIVIEW_PERF_SAMPLE_FRAMES {
            if index % 2 == 0 {
                stereo_ms.push(measure_terrain_stereo_frame(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets,
                    terrain,
                    include_sky,
                    include_actors,
                )?);
                let (elapsed_ms, summary) = measure_terrain_multiview_frame(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets,
                    terrain,
                    include_sky,
                    include_actors,
                )?;
                multiview_ms.push(elapsed_ms);
                latest_summary = summary;
            } else {
                let (elapsed_ms, summary) = measure_terrain_multiview_frame(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets,
                    terrain,
                    include_sky,
                    include_actors,
                )?;
                multiview_ms.push(elapsed_ms);
                latest_summary = summary;
                stereo_ms.push(measure_terrain_stereo_frame(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    targets,
                    terrain,
                    include_sky,
                    include_actors,
                )?);
            }
        }

        Ok(Some(TerrainMultiviewPerfSummary {
            terrain: latest_summary,
            stereo_ms,
            multiview_ms,
        }))
    }

    fn measure_terrain_stereo_frame(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [xr::View; 2],
        targets: &TerrainPerfTargets,
        terrain: &mut AndroidXrTerrainState,
        include_sky: bool,
        include_actors: bool,
    ) -> Result<f64> {
        let start = Instant::now();
        if include_actors {
            let _summary = terrain.render_sky_terrain_actors_stereo_frame_frozen(
                device,
                queue,
                views,
                targets.left.target(),
                targets.right.target(),
            )?;
        } else if include_sky {
            let _summary = terrain.render_sky_terrain_stereo_frame_frozen(
                device,
                queue,
                views,
                targets.left.target(),
                targets.right.target(),
            )?;
        } else {
            let _summary = terrain.render_terrain_stereo_frame_frozen(
                device,
                queue,
                views,
                targets.left.target(),
                targets.right.target(),
            )?;
        }
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    fn measure_terrain_multiview_frame(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [xr::View; 2],
        targets: &TerrainPerfTargets,
        terrain: &mut AndroidXrTerrainState,
        include_sky: bool,
        include_actors: bool,
    ) -> Result<(f64, mclone_xr_scene::XrTerrainMultiviewFrameSummary)> {
        let start = Instant::now();
        let summary = if include_actors {
            terrain.render_sky_terrain_actors_multiview_frame_frozen(
                device,
                queue,
                views,
                targets.multiview.target(),
            )?
        } else if include_sky {
            terrain.render_sky_terrain_multiview_frame_frozen(
                device,
                queue,
                views,
                targets.multiview.target(),
            )?
        } else {
            terrain.render_terrain_multiview_frame_frozen(
                device,
                queue,
                views,
                targets.multiview.target(),
            )?
        };
        Ok((start.elapsed().as_secs_f64() * 1000.0, summary))
    }

    fn run_terrain_multiview_proof_loop(
        app: &AndroidApp,
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        stereo_target: &mut graphics_vulkan::OpenXrStereoState,
        terrain: &mut AndroidXrTerrainState,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();
        let mut depth = ChunkMultiviewDepthTarget::new(
            &graphics.device,
            stereo_target.width,
            stereo_target.height,
        );
        let proof_start = Instant::now();

        loop {
            if proof_start.elapsed() > Duration::from_secs(45) {
                bail!(
                    "terrain multiview proof timed out: submitted={} runtime_frames={} skipped={} sections={}",
                    frame_stats.submitted_frames,
                    frame_stats.runtime_frames,
                    frame_stats.skipped_frames,
                    terrain.frame_summary().section_count
                );
            }
            if !poll_android_events(app, Some(Duration::from_millis(0)))? {
                log::info!(
                    "Android activity destroyed; exiting OpenXR terrain multiview proof loop"
                );
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
                        "OpenXR terrain multiview proof requested exit: submitted={} runtime_frames={} skipped={}",
                        frame_stats.submitted_frames,
                        frame_stats.runtime_frames,
                        frame_stats.skipped_frames
                    );
                    return Ok(());
                }
                OpenXrPollStatus::Idle if !session_running => {
                    if !poll_android_events(app, Some(SESSION_IDLE_POLL_INTERVAL))? {
                        log::info!(
                            "Android activity destroyed while waiting for OpenXR READY in terrain multiview proof"
                        );
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
            if frame_state.should_render {
                match render_terrain_multiview_proof_frame(
                    graphics,
                    stage,
                    environment_blend_mode,
                    frame_state.predicted_display_time,
                    stereo_target,
                    &mut depth,
                    terrain,
                ) {
                    Ok(proof) => {
                        frame_stats.record_submitted_frame();
                        if let Some(difference) = proof.difference {
                            log::info!(
                                "MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY submitted={} runtime_frames={} skipped={} eye={}x{} layers={} sections={} left_drawn_sections={} right_drawn_sections={} left_drawn_indices={} right_drawn_indices={} different_pixels={} minimum_different_pixels={}",
                                frame_stats.submitted_frames,
                                frame_stats.runtime_frames,
                                frame_stats.skipped_frames,
                                stereo_target.width,
                                stereo_target.height,
                                stereo_target.array_size(),
                                proof.summary.section_count,
                                proof.summary.left.drawn_section_count,
                                proof.summary.right.drawn_section_count,
                                proof.summary.left.drawn_index_count,
                                proof.summary.right.drawn_index_count,
                                difference.different_pixels,
                                difference.minimum_expected_different_pixels
                            );
                            return Ok(());
                        }
                        if frame_stats.submitted_frames % 60 == 0 {
                            log::info!(
                                "OpenXR terrain multiview proof waiting for drawable terrain: submitted={} sections={} left_indices={} right_indices={} pending_chunks={} pending_jobs={}",
                                frame_stats.submitted_frames,
                                proof.summary.section_count,
                                proof.summary.left.drawn_index_count,
                                proof.summary.right.drawn_index_count,
                                proof.summary.upload.pending_render_chunks_after,
                                proof.summary.upload.pending_compile_jobs_after
                            );
                        }
                    }
                    Err(error) => {
                        let _ = mclone_xr_host::end_frame_with_layers(
                            &mut graphics.frame_stream,
                            frame_state.predicted_display_time,
                            environment_blend_mode,
                            &[],
                        );
                        return Err(error);
                    }
                }
            } else if let Err(error) = mclone_xr_host::end_skipped_frame(
                &mut graphics.frame_stream,
                frame_state.predicted_display_time,
                environment_blend_mode,
                &mut frame_stats,
            ) {
                return Err(error);
            }
        }
    }

    struct MultiviewProofFrame {
        projection: mclone_xr_host::XrStereoProjectionProof,
        private_multiview: mclone_xr_host::XrMultiviewLayerProof,
        swapchain_multiview: mclone_xr_host::XrMultiviewLayerProof,
    }

    struct TerrainMultiviewProofFrame {
        summary: mclone_xr_scene::XrTerrainMultiviewFrameSummary,
        difference: Option<mclone_xr_host::XrMultiviewLayerDifferenceProof>,
    }

    fn render_multiview_proof_frame(
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        predicted_display_time: xr::Time,
        stereo_target: &mut graphics_vulkan::OpenXrStereoState,
    ) -> Result<MultiviewProofFrame> {
        let stereo_views =
            mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;
        let target_width = stereo_target.width;
        let target_height = stereo_target.height;
        let private_multiview = mclone_xr_host::render_private_multiview_readback_proof(
            &graphics.device,
            &graphics.queue,
            XR_COLOR_FORMAT,
            64,
            64,
        )
        .context("validate private OpenXR multiview readback proof")?;
        let projection = mclone_xr_host::render_stereo_projection_readback_proof(
            &graphics.device,
            &graphics.queue,
            XR_COLOR_FORMAT,
            target_width,
            target_height,
            stereo_views,
        )
        .context("validate OpenXR stereo projection proof")?;
        let target =
            acquire_stereo_target(stereo_target).context("acquire multiview proof target")?;
        mclone_xr_host::render_multiview_layer_proof(
            &graphics.device,
            &graphics.queue,
            target.color_array_view(),
            XR_COLOR_FORMAT,
        )
        .context("render OpenXR multiview proof")?;
        let swapchain_multiview = mclone_xr_host::read_multiview_layer_color_proof(
            &graphics.device,
            &graphics.queue,
            target.color_texture()?,
            target_width,
            target_height,
            "swapchain",
        )
        .context("validate OpenXR swapchain multiview readback proof")?;
        target.release()?;
        mclone_xr_host::end_multiview_projection_frame(
            &mut graphics.frame_stream,
            predicted_display_time,
            environment_blend_mode,
            stage,
            stereo_views,
            stereo_target,
        )?;
        Ok(MultiviewProofFrame {
            projection,
            private_multiview,
            swapchain_multiview,
        })
    }

    fn render_terrain_multiview_proof_frame(
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        predicted_display_time: xr::Time,
        stereo_target: &mut graphics_vulkan::OpenXrStereoState,
        depth: &mut ChunkMultiviewDepthTarget,
        terrain: &mut AndroidXrTerrainState,
    ) -> Result<TerrainMultiviewProofFrame> {
        let stereo_views =
            mclone_xr_host::locate_stereo_views(&graphics.session, stage, predicted_display_time)?;
        let target_width = stereo_target.width;
        let target_height = stereo_target.height;
        if depth.width != target_width || depth.height != target_height {
            *depth = ChunkMultiviewDepthTarget::new(&graphics.device, target_width, target_height);
        }
        let target = acquire_stereo_target(stereo_target)
            .context("acquire terrain multiview proof target")?;
        let render_result: Result<TerrainMultiviewProofFrame> = (|| {
            let summary = terrain
                .render_terrain_multiview_frame(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    XrTerrainMultiviewTarget {
                        color_view: target.color_array_view(),
                        depth,
                        size: [target_width, target_height],
                    },
                )
                .context("render OpenXR terrain multiview proof")?;
            terrain
                .render_overlay_multiview_smoke_frame_frozen(
                    &graphics.device,
                    &graphics.queue,
                    [stereo_views.left, stereo_views.right],
                    XrTerrainMultiviewTarget {
                        color_view: target.color_array_view(),
                        depth,
                        size: [target_width, target_height],
                    },
                )
                .context("render OpenXR overlay multiview smoke")?;
            let difference =
                if summary.left.drawn_index_count > 0 && summary.right.drawn_index_count > 0 {
                    Some(
                        mclone_xr_host::read_multiview_layer_difference_proof(
                            &graphics.device,
                            &graphics.queue,
                            target.color_texture()?,
                            target_width,
                            target_height,
                            "terrain",
                        )
                        .context("validate OpenXR terrain multiview layer difference")?,
                    )
                } else {
                    None
                };
            Ok(TerrainMultiviewProofFrame {
                summary,
                difference,
            })
        })();
        target.release()?;
        let proof = render_result?;
        mclone_xr_host::end_multiview_projection_frame(
            &mut graphics.frame_stream,
            predicted_display_time,
            environment_blend_mode,
            stage,
            stereo_views,
            stereo_target,
        )?;
        Ok(proof)
    }

    fn run_mclone_frame_loop(
        app: &AndroidApp,
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        mut frame_targets: AndroidXrFrameTargets<'_>,
        terrain: &mut AndroidXrTerrainState,
        controller_actions: &OpenXrControllerActions,
        session_smoke: Option<AndroidXrSessionSmoke>,
        perf_seconds: Option<u64>,
        perf_flight: Option<AndroidXrPerfFlight>,
        perf_settled_orbit: Option<AndroidXrPerfOrbit>,
        perf_chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
        perf_settled_stationary: bool,
        perf_frozen_render: bool,
        perf_metrics: bool,
        perf_metrics_periodic: bool,
        perf_detail: AndroidXrPerfDetail,
        frame_accounting_enabled: bool,
        startup_center: [i32; 2],
        fixed_render_view_pose: Option<XrStartupViewPose>,
        render_distance: u32,
        render_compile_worker_count: usize,
        render_compile_max_pending_jobs: Option<usize>,
        skip_actors: bool,
        adaptive_chunk_publication_budget: bool,
        display_refresh: XrDisplayRefreshSnapshot,
        render_section_upload_budget: Option<usize>,
        render_section_accept_budget: Option<usize>,
        render_completed_result_accept_budget: Option<usize>,
        xr_foveation: AndroidXrFoveation,
        xr_render_scale: f32,
    ) -> Result<()> {
        let mut event_storage = xr::EventDataBuffer::new();
        let mut session_running = false;
        let mut frame_stats = XrFrameStats::default();
        let render_path = frame_targets.render_path();
        let xr_eye_size = frame_targets.eye_size();
        let mut frame_pipeline_reporter =
            XrFramePipelineReporter::new(display_refresh.current_rate.map(f64::from));
        let mut perf_probe = AndroidXrPerfProbe::new(
            perf_seconds,
            perf_flight,
            perf_settled_orbit,
            perf_chunk_view_churn,
            perf_settled_stationary,
            perf_frozen_render,
            perf_detail,
            frame_accounting_enabled,
            startup_center,
            render_distance,
            render_compile_worker_count,
            render_compile_max_pending_jobs,
            skip_actors,
            adaptive_chunk_publication_budget,
            display_refresh,
            render_path,
            render_section_upload_budget,
            render_section_accept_budget,
            render_completed_result_accept_budget,
            xr_foveation,
            xr_render_scale,
            xr_eye_size,
        );
        let performance_metrics_wait_for_perf_window = perf_seconds.is_some();
        let mut performance_metrics_probe = if perf_metrics {
            let mode = if perf_metrics_periodic {
                perf_metrics::XrPerformanceMetricsMode::Periodic
            } else {
                perf_metrics::XrPerformanceMetricsMode::OneShot
            };
            let probe = perf_metrics::XrPerformanceMetricsProbe::new(
                graphics.session.instance(),
                &graphics.session,
                mode,
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
                    .start_session_for_request(
                        &graphics.device,
                        &graphics.queue,
                        SessionStartRequest::new_seed_local_world(ANDROID_XR_SESSION_SMOKE_SEED),
                    )
                    .context("run Android XR new-world session smoke replacement")?;
                session_smoke_started = true;
                session_smoke_pending_start = false;
            }

            let wait_frame_start = Instant::now();
            let frame_state = graphics.frame_wait.wait().context("wait OpenXR frame")?;
            frame_timing.wait_frame_ms = elapsed_ms(wait_frame_start);
            let thread_cpu_start_ms = mclone_diagnostics::clock::thread_cpu_time_ms();
            let begin_frame_start = Instant::now();
            graphics
                .frame_stream
                .begin()
                .context("begin OpenXR frame")?;
            frame_timing.begin_frame_ms = elapsed_ms(begin_frame_start);
            frame_timing.wait_begin_ms = frame_timing.wait_frame_ms + frame_timing.begin_frame_ms;
            frame_stats.record_runtime_frame();

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
                        let result = match &mut frame_targets {
                            AndroidXrFrameTargets::PerEye {
                                left_eye,
                                right_eye,
                                ..
                            } => render_mclone_frame(
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
                            ),
                            AndroidXrFrameTargets::Multiview {
                                stereo_target,
                                depth,
                            } => render_mclone_multiview_frame(
                                graphics,
                                stage,
                                environment_blend_mode,
                                frame_state.predicted_display_time,
                                stereo_target,
                                depth,
                                terrain,
                                &controllers,
                                perf_probe.automation(),
                                fixed_render_view_pose,
                            ),
                        };
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
                                "Android XR terrain first-frame summary: render_path={} frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={} local_startup_active={}",
                                render_path.label(),
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
                                "Android XR terrain ready summary: render_path={} frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                                render_path.label(),
                                summary.rendered_frames,
                                summary.section_count,
                                summary.drawn_section_count,
                                summary.index_count,
                                summary.drawn_index_count,
                                summary.actor_count,
                                summary.drawn_actor_count
                            );
                            log::info!("MCLONE_ANDROID_XR_READY");
                            if render_path == AndroidXrRenderPath::Multiview {
                                log::info!(
                                    "MCLONE_ANDROID_XR_FULL_FRAME_MULTIVIEW_READY frames={} sections={} drawn_sections={} indices={} drawn_indices={} actors={} drawn_actors={}",
                                    summary.rendered_frames,
                                    summary.section_count,
                                    summary.drawn_section_count,
                                    summary.index_count,
                                    summary.drawn_index_count,
                                    summary.actor_count,
                                    summary.drawn_actor_count
                                );
                            }
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
            if perf_started_after_ready {
                if let Some(probe) = performance_metrics_probe.as_mut() {
                    probe.start_perf_window();
                }
            }
            frame_timing.frame_wall_ms = elapsed_ms(frame_wall_start);
            if let (Some(start_ms), Some(end_ms)) = (
                thread_cpu_start_ms,
                mclone_diagnostics::clock::thread_cpu_time_ms(),
            ) {
                frame_timing.thread_cpu_ms = (end_ms - start_ms).max(0.0);
                frame_timing.thread_cpu_valid = true;
            }
            // Feed the perf overlay whenever it is open (or perf diagnostics are
            // compiled in / recording). The frame-pipeline report only assembles
            // already-measured host and runtime timings, so running it while the
            // overlay is visible is cheap and keeps the panel populated without
            // requiring the `perf-diagnostics` build feature.
            let frame_pipeline_overlay_open = terrain.frame_metrics_visible();
            let budget_decision_panel = if cfg!(feature = "perf-diagnostics")
                || perf_probe.is_recording()
                || frame_pipeline_overlay_open
            {
                terrain.latest_budget_decision_panel()
            } else {
                BudgetDecisionPanelReport::empty()
            };
            if cfg!(feature = "perf-diagnostics") || frame_pipeline_overlay_open {
                let (frame_pipeline_report, frame_pipeline_revision) = frame_pipeline_reporter
                    .record_frame_with_budget_decision_panel(
                        XrFramePipelineHostTiming {
                            frame_wall_ms: frame_timing.frame_wall_ms,
                            wait_frame_ms: frame_timing.wait_frame_ms,
                            controller_poll_ms: frame_timing.controller_poll_ms,
                            rendered: rendered_frame.is_some(),
                            thread_cpu_ms: frame_timing
                                .thread_cpu_valid
                                .then_some(frame_timing.thread_cpu_ms),
                        },
                        rendered_frame.map(|rendered| rendered.summary),
                        budget_decision_panel.clone(),
                    );
                terrain.set_frame_pipeline_report(frame_pipeline_report, frame_pipeline_revision);
            }
            if !perf_started_after_ready && perf_probe.is_recording() {
                perf_probe.record_frame(
                    frame_timing,
                    frame_stats,
                    rendered_frame,
                    budget_decision_panel,
                );
            }
            if rendered_frame.is_some() {
                if let Some(probe) = performance_metrics_probe.as_mut() {
                    if !performance_metrics_wait_for_perf_window || probe.perf_window_started() {
                        probe.tick();
                    }
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
        wait_frame_ms: f64,
        thread_cpu_ms: f64,
        thread_cpu_valid: bool,
        begin_frame_ms: f64,
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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum AndroidXrRenderPath {
        PerEye,
        PerEyeFrameOverlap,
        PerEyeOverlap,
        PerEyePrefetch,
        Multiview,
    }

    impl AndroidXrRenderPath {
        const fn label(self) -> &'static str {
            match self {
                Self::PerEye => "per-eye",
                Self::PerEyeFrameOverlap => "per-eye-frame-overlap",
                Self::PerEyeOverlap => "per-eye-overlap",
                Self::PerEyePrefetch => "per-eye-prefetch",
                Self::Multiview => "multiview",
            }
        }
    }

    enum AndroidXrFrameTargets<'a> {
        PerEye {
            left_eye: &'a mut graphics_vulkan::OpenXrEyeState,
            right_eye: &'a mut graphics_vulkan::OpenXrEyeState,
            frame_overlap: bool,
            overlap_eye_submits: bool,
            overlap_runtime_prefetch: bool,
        },
        Multiview {
            stereo_target: &'a mut graphics_vulkan::OpenXrStereoState,
            depth: &'a mut ChunkMultiviewDepthTarget,
        },
    }

    impl AndroidXrFrameTargets<'_> {
        fn render_path(&self) -> AndroidXrRenderPath {
            match self {
                Self::PerEye {
                    frame_overlap,
                    overlap_eye_submits,
                    overlap_runtime_prefetch,
                    ..
                } => {
                    if *frame_overlap {
                        AndroidXrRenderPath::PerEyeFrameOverlap
                    } else if *overlap_runtime_prefetch {
                        AndroidXrRenderPath::PerEyePrefetch
                    } else if *overlap_eye_submits {
                        AndroidXrRenderPath::PerEyeOverlap
                    } else {
                        AndroidXrRenderPath::PerEye
                    }
                }
                Self::Multiview { .. } => AndroidXrRenderPath::Multiview,
            }
        }

        fn eye_size(&self) -> [u32; 2] {
            match self {
                Self::PerEye { left_eye, .. } => [left_eye.width, left_eye.height],
                Self::Multiview { stereo_target, .. } => {
                    [stereo_target.width, stereo_target.height]
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum AndroidXrPerfAutomation {
        Flight {
            speed_blocks_per_second: f64,
        },
        Orbit {
            speed_blocks_per_second: f64,
            elapsed_seconds: f64,
        },
        Stationary {
            frozen_render: bool,
        },
        ChunkViewChurn {
            center_x: i32,
            center_z: i32,
        },
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct AndroidXrRenderFrameTiming {
        locate_views_ms: f64,
        locomotion_ms: f64,
        locomotion_input_ms: f64,
        locomotion_camera_apply_ms: f64,
        locomotion_commit_ms: f64,
        locomotion_commit_server_command_ms: f64,
        locomotion_commit_server_command_send_ms: f64,
        locomotion_commit_server_command_drain_updates_ms: f64,
        locomotion_commit_server_command_apply_updates_ms: f64,
        locomotion_commit_server_command_apply_dirty_mark_ms: f64,
        locomotion_commit_server_command_apply_client_updates_ms: f64,
        locomotion_commit_server_command_updates: usize,
        locomotion_commit_server_command_snapshot_updates: usize,
        locomotion_commit_server_command_section_block_updates: usize,
        locomotion_commit_server_command_unload_updates: usize,
        locomotion_commit_position_updates_ms: f64,
        locomotion_commit_interest_ms: f64,
        locomotion_commit_interest_command_send_ms: f64,
        locomotion_commit_interest_command_drain_updates_ms: f64,
        locomotion_commit_interest_command_apply_updates_ms: f64,
        locomotion_commit_interest_command_apply_dirty_mark_ms: f64,
        locomotion_commit_interest_command_apply_client_updates_ms: f64,
        locomotion_commit_interest_command_updates: usize,
        locomotion_commit_interest_command_snapshot_updates: usize,
        locomotion_commit_interest_command_section_block_updates: usize,
        locomotion_commit_interest_command_unload_updates: usize,
        locomotion_gameplay_interaction_ms: f64,
        acquire_left_ms: f64,
        acquire_right_ms: f64,
        terrain_render_frame_ms: f64,
        terrain_render_views_ms: f64,
        terrain_menu_pointer_ms: f64,
        terrain_runtime_upload_ms: f64,
        terrain_runtime_poll_ms: f64,
        terrain_runtime_sync_ms: f64,
        terrain_runtime_sync_unattributed_ms: f64,
        terrain_runtime_result_accept_ms: f64,
        terrain_runtime_dirty_seed_ms: f64,
        terrain_runtime_prepare_ms: f64,
        terrain_runtime_submit_ms: f64,
        terrain_runtime_submit_snapshot_ms: f64,
        terrain_runtime_submit_handoff_ms: f64,
        terrain_runtime_submit_handoff_worst_ms: f64,
        terrain_runtime_submit_request_count: usize,
        terrain_runtime_submit_request_build_ms: f64,
        terrain_runtime_submit_compiler_ms: f64,
        terrain_runtime_submit_compiler_worst_ms: f64,
        terrain_runtime_submit_compiler_capacity_check_ms: f64,
        terrain_runtime_submit_compiler_capacity_check_worst_ms: f64,
        terrain_runtime_submit_compiler_command_send_ms: f64,
        terrain_runtime_submit_compiler_command_send_worst_ms: f64,
        terrain_runtime_submit_compiler_command_lock_wait_ms: f64,
        terrain_runtime_submit_compiler_command_lock_wait_worst_ms: f64,
        terrain_runtime_submit_compiler_command_slot_select_ms: f64,
        terrain_runtime_submit_compiler_command_slot_select_worst_ms: f64,
        terrain_runtime_submit_compiler_command_slot_write_ms: f64,
        terrain_runtime_submit_compiler_command_slot_write_worst_ms: f64,
        terrain_runtime_submit_compiler_command_queue_push_ms: f64,
        terrain_runtime_submit_compiler_command_queue_push_worst_ms: f64,
        terrain_runtime_submit_compiler_command_notify_ms: f64,
        terrain_runtime_submit_compiler_command_notify_worst_ms: f64,
        terrain_runtime_submit_compiler_command_post_enqueue_ms: f64,
        terrain_runtime_submit_compiler_command_post_enqueue_worst_ms: f64,
        terrain_runtime_submit_compiler_pending_mark_ms: f64,
        terrain_runtime_submit_compiler_pending_mark_worst_ms: f64,
        terrain_runtime_submit_mark_inflight_ms: f64,
        terrain_runtime_submit_apply_ready_plan_ms: f64,
        terrain_runtime_submit_ready_update_ms: f64,
        terrain_runtime_submit_ready_section_count: usize,
        terrain_runtime_submit_deferred_section_count: usize,
        terrain_runtime_submit_dirty_chunk_count_before: usize,
        terrain_runtime_submit_dirty_chunk_count_after: usize,
        terrain_runtime_submit_dirty_section_count_before: usize,
        terrain_runtime_submit_dirty_section_count_after: usize,
        terrain_runtime_submit_inflight_section_count_before: usize,
        terrain_runtime_submit_inflight_section_count_after: usize,
        terrain_runtime_submit_request_target_section_count: usize,
        terrain_runtime_submit_request_target_section_count_worst: usize,
        terrain_runtime_submit_request_snapshot_count: usize,
        terrain_runtime_submit_request_snapshot_section_count: usize,
        terrain_runtime_submit_request_snapshot_section_count_worst: usize,
        terrain_runtime_submit_request_light_section_count: usize,
        terrain_runtime_submit_request_light_section_count_worst: usize,
        terrain_runtime_submit_request_revision_count: usize,
        terrain_runtime_submit_request_estimated_payload_bytes: usize,
        terrain_runtime_submit_request_estimated_payload_bytes_worst: usize,
        terrain_runtime_dispatcher_pending_jobs: usize,
        terrain_runtime_dispatcher_max_pending_jobs: usize,
        terrain_runtime_dispatcher_available_job_slots: usize,
        terrain_runtime_dispatcher_queued_compile_tasks: usize,
        terrain_runtime_gpu_upload_ms: f64,
        terrain_runtime_gpu_upload_pre_sync_ms: f64,
        terrain_runtime_gpu_upload_post_sync_ms: f64,
        terrain_runtime_upload_enqueue_ms: f64,
        terrain_runtime_upload_select_ms: f64,
        terrain_runtime_upload_apply_ms: f64,
        terrain_runtime_upload_apply_dirty_mark_ms: f64,
        terrain_runtime_upload_apply_remove_ms: f64,
        terrain_runtime_upload_apply_section_state_ms: f64,
        terrain_runtime_upload_apply_vertex_bytes_ms: f64,
        terrain_runtime_upload_apply_vertex_buffer_ms: f64,
        terrain_runtime_upload_apply_index_bytes_ms: f64,
        terrain_runtime_upload_apply_index_buffer_ms: f64,
        terrain_runtime_upload_apply_mesh_insert_ms: f64,
        terrain_runtime_upload_apply_mesh_upload_worst_ms: f64,
        terrain_runtime_ready_sections_ms: f64,
        terrain_runtime_ready_publish_ms: f64,
        terrain_shared_records_ms: f64,
        terrain_record_cache_prepare: TexturedSectionRecordPrepareStats,
        terrain_left_eye_ms: f64,
        terrain_right_eye_ms: f64,
        terrain_left_eye_prepare_ms: f64,
        terrain_left_eye_cull_ms: f64,
        terrain_left_eye_uniform_write_ms: f64,
        terrain_left_eye_translucent_collect_ms: f64,
        terrain_left_eye_translucent_sort_ms: f64,
        terrain_left_eye_encode_ms: f64,
        terrain_left_eye_section_encode_ms: f64,
        terrain_left_eye_full_frame_ms: f64,
        terrain_left_eye_sky_ms: f64,
        terrain_left_eye_far_lod_ms: f64,
        terrain_left_eye_opaque_ms: f64,
        terrain_left_eye_translucent_ms: f64,
        terrain_left_eye_actor_ms: f64,
        terrain_left_eye_screen_effect_ms: f64,
        terrain_left_eye_gui_ms: f64,
        terrain_left_eye_xr_fade_ms: f64,
        terrain_left_eye_xr_selection_ms: f64,
        terrain_left_eye_xr_world_lines_ms: f64,
        terrain_left_eye_xr_world_panel_ms: f64,
        terrain_left_eye_encoder_finish_ms: f64,
        terrain_left_eye_submit_ms: f64,
        terrain_left_eye_poll_wait_ms: f64,
        terrain_right_eye_prepare_ms: f64,
        terrain_right_eye_cull_ms: f64,
        terrain_right_eye_uniform_write_ms: f64,
        terrain_right_eye_translucent_collect_ms: f64,
        terrain_right_eye_translucent_sort_ms: f64,
        terrain_right_eye_encode_ms: f64,
        terrain_right_eye_section_encode_ms: f64,
        terrain_right_eye_full_frame_ms: f64,
        terrain_right_eye_sky_ms: f64,
        terrain_right_eye_far_lod_ms: f64,
        terrain_right_eye_opaque_ms: f64,
        terrain_right_eye_translucent_ms: f64,
        terrain_right_eye_actor_ms: f64,
        terrain_right_eye_screen_effect_ms: f64,
        terrain_right_eye_gui_ms: f64,
        terrain_right_eye_xr_fade_ms: f64,
        terrain_right_eye_xr_selection_ms: f64,
        terrain_right_eye_xr_world_lines_ms: f64,
        terrain_right_eye_xr_world_panel_ms: f64,
        terrain_right_eye_encoder_finish_ms: f64,
        terrain_right_eye_submit_ms: f64,
        terrain_right_eye_poll_wait_ms: f64,
        terrain_stereo_finish_ms: f64,
        terrain_stereo_submit_ms: f64,
        terrain_stereo_poll_wait_ms: f64,
        terrain_overlap_runtime_prefetch_ms: f64,
        terrain_overlap_runtime_prefetch_poll_ms: f64,
        terrain_overlap_runtime_prefetch_sync_ms: f64,
        terrain_overlap_runtime_prefetch_gpu_upload_ms: f64,
        terrain_overlap_runtime_prefetch_ready_sections_ms: f64,
        terrain_multiview_sky_ms: f64,
        terrain_multiview_terrain_ms: f64,
        terrain_multiview_actor_ms: f64,
        terrain_multiview_screen_effect_ms: f64,
        terrain_multiview_world_overlays_ms: f64,
        terrain_multiview_submit_ms: f64,
        terrain_multiview_poll_wait_ms: f64,
        release_eyes_ms: f64,
        end_frame_ms: f64,
    }

    #[derive(Clone, Copy, Debug)]
    struct AndroidXrWorstFrameSnapshot {
        sample_frame: u64,
        timing: AndroidXrFrameTiming,
        summary: Option<mclone_xr_scene::XrTerrainFrameSummary>,
    }

    struct AndroidXrPerfProbe {
        requested_seconds: Option<u64>,
        flight: Option<AndroidXrPerfFlight>,
        settled_orbit: Option<AndroidXrPerfOrbit>,
        chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
        settled_stationary: bool,
        frozen_render: bool,
        detail: AndroidXrPerfDetail,
        frame_accounting_enabled: bool,
        chunk_view_churn_base_center: [i32; 2],
        render_distance: u32,
        render_compile_worker_count: usize,
        render_compile_max_pending_jobs: Option<usize>,
        skip_actors: bool,
        adaptive_chunk_publication_budget: bool,
        display_refresh: XrDisplayRefreshSnapshot,
        render_path: AndroidXrRenderPath,
        render_section_upload_budget: Option<usize>,
        render_section_accept_budget: Option<usize>,
        render_completed_result_accept_budget: Option<usize>,
        xr_foveation: AndroidXrFoveation,
        xr_render_scale: f32,
        xr_eye_size: [u32; 2],
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
            settled_orbit: Option<AndroidXrPerfOrbit>,
            chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
            settled_stationary: bool,
            frozen_render: bool,
            detail: AndroidXrPerfDetail,
            frame_accounting_enabled: bool,
            chunk_view_churn_base_center: [i32; 2],
            render_distance: u32,
            render_compile_worker_count: usize,
            render_compile_max_pending_jobs: Option<usize>,
            skip_actors: bool,
            adaptive_chunk_publication_budget: bool,
            display_refresh: XrDisplayRefreshSnapshot,
            render_path: AndroidXrRenderPath,
            render_section_upload_budget: Option<usize>,
            render_section_accept_budget: Option<usize>,
            render_completed_result_accept_budget: Option<usize>,
            xr_foveation: AndroidXrFoveation,
            xr_render_scale: f32,
            xr_eye_size: [u32; 2],
        ) -> Self {
            let target_hz = display_refresh
                .current_rate
                .map(f64::from)
                .filter(|hz| hz.is_finite() && *hz > 0.0)
                .unwrap_or(ANDROID_XR_PERF_FALLBACK_TARGET_HZ);
            Self {
                requested_seconds,
                flight,
                settled_orbit,
                chunk_view_churn,
                settled_stationary,
                frozen_render,
                detail,
                frame_accounting_enabled,
                chunk_view_churn_base_center,
                render_distance,
                render_compile_worker_count,
                render_compile_max_pending_jobs,
                skip_actors,
                adaptive_chunk_publication_budget,
                display_refresh,
                render_path,
                render_section_upload_budget,
                render_section_accept_budget,
                render_completed_result_accept_budget,
                xr_foveation,
                xr_render_scale,
                xr_eye_size,
                target_hz,
                settle_started: None,
                settle_frames: 0,
                settle_quiet_frames: 0,
                active: None,
                completed: false,
            }
        }

        fn automation(&self) -> Option<AndroidXrPerfAutomation> {
            let needs_settle = self.settled_stationary
                || self.settled_orbit.is_some()
                || self.chunk_view_churn.is_some();
            if needs_settle && self.requested_seconds.is_some() && !self.completed {
                if let (Some(active), Some(orbit)) = (self.active.as_ref(), self.settled_orbit) {
                    return Some(AndroidXrPerfAutomation::Orbit {
                        speed_blocks_per_second: orbit.speed_blocks_per_second,
                        elapsed_seconds: active.started.elapsed().as_secs_f64(),
                    });
                }
                if let (Some(active), Some(churn)) = (self.active.as_ref(), self.chunk_view_churn) {
                    let [center_x, center_z] =
                        self.chunk_view_churn_center(churn, active.started.elapsed());
                    return Some(AndroidXrPerfAutomation::ChunkViewChurn { center_x, center_z });
                }
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

        fn is_recording(&self) -> bool {
            self.active.is_some()
        }

        fn chunk_view_churn_center(
            &self,
            churn: AndroidXrPerfChunkViewChurn,
            elapsed: Duration,
        ) -> [i32; 2] {
            let step = (elapsed.as_secs_f64() / churn.interval_seconds).floor() as i64;
            let offset_x = if step % 2 == 0 {
                0
            } else {
                churn.offset_chunks
            };
            [
                self.chunk_view_churn_base_center[0].saturating_add(offset_x),
                self.chunk_view_churn_base_center[1],
            ]
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
            let needs_settle = self.settled_stationary
                || self.settled_orbit.is_some()
                || self.chunk_view_churn.is_some();
            let mode = android_xr_perf_mode_label(
                self.flight,
                self.settled_orbit,
                self.chunk_view_churn,
                self.settled_stationary,
                self.frozen_render,
            );
            if needs_settle && !self.record_settle_frame(rendered.summary, mode) {
                return false;
            }
            let flight_speed = self
                .flight
                .map(|flight| flight.speed_blocks_per_second)
                .or_else(|| {
                    self.settled_orbit
                        .map(|orbit| orbit.speed_blocks_per_second)
                })
                .unwrap_or(0.0);
            let chunk_view_churn_interval_seconds = self
                .chunk_view_churn
                .map_or(0.0, |churn| churn.interval_seconds);
            let chunk_view_churn_offset_chunks =
                self.chunk_view_churn.map_or(0, |churn| churn.offset_chunks);
            let settle_seconds = self
                .settle_started
                .map_or(0.0, |started| started.elapsed().as_secs_f64());
            if needs_settle {
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
                "MCLONE_ANDROID_XR_PERF_START seconds={} mode={} detail={} render_path={} frame_accounting_enabled={} render_section_upload_budget={} render_section_accept_budget={} render_completed_result_accept_budget={} skip_actors={} adaptive_chunk_publication_budget={} render_distance={} render_compile_workers={} render_compile_max_pending_jobs={} flight_speed_blocks_per_second={:.3} chunk_view_churn_interval_seconds={:.3} chunk_view_churn_offset_chunks={} settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} refresh_supported={} current_hz={} supported_hz={} target_hz={:.1} budget_ms={:.3} submitted={} runtime_frames={} skipped={}",
                seconds,
                mode,
                self.detail.label(),
                self.render_path.label(),
                self.frame_accounting_enabled,
                format_optional_usize(self.render_section_upload_budget),
                format_optional_usize(self.render_section_accept_budget),
                format_optional_usize(self.render_completed_result_accept_budget),
                self.skip_actors,
                self.adaptive_chunk_publication_budget,
                self.render_distance,
                self.render_compile_worker_count,
                format_optional_usize(self.render_compile_max_pending_jobs),
                flight_speed,
                chunk_view_churn_interval_seconds,
                chunk_view_churn_offset_chunks,
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
            log::info!(
                "MCLONE_ANDROID_XR_PERF_CONFIG xr_foveation={} xr_render_scale={:.3} xr_eye_size={}x{}",
                self.xr_foveation.label(),
                self.xr_render_scale,
                self.xr_eye_size[0],
                self.xr_eye_size[1]
            );
            self.active = Some(AndroidXrActivePerfProbe {
                requested: Duration::from_secs(seconds),
                started: Instant::now(),
                start_stats: frame_stats,
                detail: self.detail,
                frame_accounting: self
                    .frame_accounting_enabled
                    .then(|| android_xr_frame_accounting_accumulator(self.target_hz)),
                sample_frames: 0,
                max_wait_begin_ms: 0.0,
                max_wait_frame_ms: 0.0,
                max_begin_frame_ms: 0.0,
                max_controller_poll_ms: 0.0,
                max_render_mclone_frame_ms: 0.0,
                max_render: AndroidXrRenderFrameTiming::default(),
                max_upload: mclone_xr_scene::XrTerrainUploadSummary::default(),
                upload_work_frames: 0,
                diagnostics_refresh_frames: 0,
                render_distance: self.render_distance,
                render_compile_worker_count: self.render_compile_worker_count,
                render_compile_max_pending_jobs: self.render_compile_max_pending_jobs,
                skip_actors: self.skip_actors,
                adaptive_chunk_publication_budget: self.adaptive_chunk_publication_budget,
                display_refresh: self.display_refresh.clone(),
                target_hz: self.target_hz,
                frame_accounting_enabled: self.frame_accounting_enabled,
                render_path: self.render_path,
                render_section_upload_budget: self.render_section_upload_budget,
                render_section_accept_budget: self.render_section_accept_budget,
                render_completed_result_accept_budget: self.render_completed_result_accept_budget,
                xr_foveation: self.xr_foveation,
                xr_render_scale: self.xr_render_scale,
                xr_eye_size: self.xr_eye_size,
                mode_label: mode,
                flight_speed_blocks_per_second: self
                    .flight
                    .map(|flight| flight.speed_blocks_per_second)
                    .or_else(|| {
                        self.settled_orbit
                            .map(|orbit| orbit.speed_blocks_per_second)
                    }),
                chunk_view_churn: self.chunk_view_churn,
                settle_seconds,
                settle_frames: self.settle_frames,
                settle_quiet_frames: self.settle_quiet_frames,
                start_camera: rendered.camera,
                latest_camera: rendered.camera,
                start_record_cache: rendered.summary.timing.record_cache_prepare.cache,
                latest_record_cache: rendered.summary.timing.record_cache_prepare.cache,
                record_rebuild_frames: 0,
                record_rebuild_total_ms: 0.0,
                record_rebuild_max_ms: 0.0,
                frame_details: Vec::new(),
                start_scheduler_cumulative_feature_chunks_published: rendered
                    .summary
                    .upload
                    .poll_scheduler_cumulative_feature_chunks_published,
                start_scheduler_cumulative_light_statuses_published: rendered
                    .summary
                    .upload
                    .poll_scheduler_cumulative_light_statuses_published,
                latest_summary: rendered.summary,
                latest_budget_decision_panel: BudgetDecisionPanelReport::empty(),
            });
            true
        }

        fn record_settle_frame(
            &mut self,
            summary: mclone_xr_scene::XrTerrainFrameSummary,
            mode: &'static str,
        ) -> bool {
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
                    "MCLONE_ANDROID_XR_PERF_SETTLE_PROGRESS mode={} settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} quiet={} poll_changed={} server_cmd_q={} server_update_q={} pending_jobs_after={} pending_chunks_after={} deferred_sections={} submitted_sections={} deadline_skipped_requests={} completed_sections={} stale_sections={} uploaded_sections={} upload_removed_sections={} ready_sections={} ui_draw_rebuilds={} ui_draw_cache_hits={} ui_panel_repaints={} ui_panel_cache_hits={} ui_panel_texture_recreates={} ui_panel_composites={} sections={} drawn_sections={} drawn_indices={}",
                    mode,
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
                    upload.deadline_skipped_compile_request_count,
                    upload.completed_compile_section_count,
                    upload.stale_compile_section_count,
                    upload.uploaded_section_count,
                    upload.upload_removed_section_count,
                    upload.traversal_ready_section_count,
                    summary.ui_draw_cache.rebuild_count,
                    summary.ui_draw_cache.cache_hit_count,
                    summary.ui_panel.repaint_count,
                    summary.ui_panel.cache_hit_count,
                    summary.ui_panel.texture_recreate_count,
                    summary.ui_panel.composite_count,
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
            budget_decision_panel: BudgetDecisionPanelReport,
        ) {
            let Some(active) = self.active.as_mut() else {
                return;
            };
            active.record_frame(timing, rendered, budget_decision_panel);
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
        detail: AndroidXrPerfDetail,
        frame_accounting: Option<FrameAccumulator>,
        sample_frames: u64,
        max_wait_begin_ms: f64,
        max_wait_frame_ms: f64,
        max_begin_frame_ms: f64,
        max_controller_poll_ms: f64,
        max_render_mclone_frame_ms: f64,
        max_render: AndroidXrRenderFrameTiming,
        max_upload: mclone_xr_scene::XrTerrainUploadSummary,
        upload_work_frames: u64,
        diagnostics_refresh_frames: u64,
        render_distance: u32,
        render_compile_worker_count: usize,
        render_compile_max_pending_jobs: Option<usize>,
        skip_actors: bool,
        adaptive_chunk_publication_budget: bool,
        display_refresh: XrDisplayRefreshSnapshot,
        target_hz: f64,
        frame_accounting_enabled: bool,
        render_path: AndroidXrRenderPath,
        render_section_upload_budget: Option<usize>,
        render_section_accept_budget: Option<usize>,
        render_completed_result_accept_budget: Option<usize>,
        xr_foveation: AndroidXrFoveation,
        xr_render_scale: f32,
        xr_eye_size: [u32; 2],
        mode_label: &'static str,
        flight_speed_blocks_per_second: Option<f64>,
        chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
        settle_seconds: f64,
        settle_frames: u64,
        settle_quiet_frames: u64,
        start_camera: EngineCameraSnapshot,
        latest_camera: EngineCameraSnapshot,
        start_record_cache: TexturedSectionRecordCacheStats,
        latest_record_cache: TexturedSectionRecordCacheStats,
        record_rebuild_frames: u64,
        record_rebuild_total_ms: f64,
        record_rebuild_max_ms: f64,
        frame_details: Vec<AndroidXrWorstFrameSnapshot>,
        start_scheduler_cumulative_feature_chunks_published: u64,
        start_scheduler_cumulative_light_statuses_published: u64,
        latest_summary: mclone_xr_scene::XrTerrainFrameSummary,
        latest_budget_decision_panel: BudgetDecisionPanelReport,
    }

    impl AndroidXrActivePerfProbe {
        fn record_frame(
            &mut self,
            timing: AndroidXrFrameTiming,
            rendered: Option<AndroidXrRenderedFrame>,
            budget_decision_panel: BudgetDecisionPanelReport,
        ) {
            self.latest_budget_decision_panel = budget_decision_panel;
            self.sample_frames = self.sample_frames.saturating_add(1);
            let sample_frame = self.sample_frames;
            if let Some(frame_accounting) = self.frame_accounting.as_mut() {
                frame_accounting.record_frame(android_xr_frame_observation(
                    sample_frame,
                    timing,
                    rendered.is_some(),
                ));
            }
            if self.detail.is_full() {
                self.frame_details.push(AndroidXrWorstFrameSnapshot {
                    sample_frame,
                    timing,
                    summary: rendered.as_ref().map(|rendered| rendered.summary),
                });
            }
            self.max_wait_begin_ms = self.max_wait_begin_ms.max(timing.wait_begin_ms);
            self.max_wait_frame_ms = self.max_wait_frame_ms.max(timing.wait_frame_ms);
            self.max_begin_frame_ms = self.max_begin_frame_ms.max(timing.begin_frame_ms);
            self.max_controller_poll_ms =
                self.max_controller_poll_ms.max(timing.controller_poll_ms);
            self.max_render_mclone_frame_ms = self
                .max_render_mclone_frame_ms
                .max(timing.render_mclone_frame_ms);
            self.max_render = max_render_timing(self.max_render, timing.render);
            if let Some(rendered) = rendered {
                let record_prepare = rendered.summary.timing.record_cache_prepare;
                if record_prepare.rebuilt {
                    self.record_rebuild_frames += 1;
                    self.record_rebuild_total_ms += record_prepare.rebuild_ms;
                    self.record_rebuild_max_ms =
                        self.record_rebuild_max_ms.max(record_prepare.rebuild_ms);
                }
                self.latest_record_cache = record_prepare.cache;
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

        fn frame_accounting_report(&self) -> FrameSummaryReport {
            if let Some(frame_accounting) = self.frame_accounting.as_ref() {
                return frame_accounting.summary_report();
            }
            let mut frame_accounting = android_xr_frame_accounting_accumulator(self.target_hz);
            for detail in &self.frame_details {
                frame_accounting.record_frame(android_xr_frame_observation(
                    detail.sample_frame,
                    detail.timing,
                    detail.summary.is_some(),
                ));
            }
            frame_accounting.summary_report()
        }

        fn log_summary(&mut self, frame_stats: XrFrameStats) {
            let frame_accounting = self.frame_accounting_report();
            let frame_pipeline_report = self.detail.is_full().then(|| {
                let queue_panel = android_xr_queue_panel(&self.frame_details);
                let peer_thread_panel = android_xr_peer_thread_panel(&self.frame_details);
                FramePipelineReport::new(frame_accounting.clone(), queue_panel)
                    .with_peer_thread_panel(peer_thread_panel)
                    .with_budget_decision_panel(self.latest_budget_decision_panel.clone())
            });
            let sample_seconds = self.started.elapsed().as_secs_f64();
            let frame_count = frame_accounting.frames;
            let submitted_delta = frame_stats.submitted_frames - self.start_stats.submitted_frames;
            let runtime_delta = frame_stats.runtime_frames - self.start_stats.runtime_frames;
            let skipped_delta = frame_stats.skipped_frames - self.start_stats.skipped_frames;
            let budget_ms = perf_budget_ms(self.target_hz);
            let frame_wall = frame_accounting.frame_wall;
            let wait_frame = frame_accounting.wait;
            let app_work = frame_accounting.app_work;
            let thread_cpu = frame_accounting.thread_cpu;
            let blocked = frame_accounting.blocked;
            let headroom = frame_accounting.headroom;
            let over_budget = frame_accounting.over_budget;
            let thread_cpu_valid_frames = thread_cpu.count;
            let thread_cpu_valid_pct = if frame_count == 0 {
                0.0
            } else {
                thread_cpu_valid_frames as f64 * 100.0 / frame_count as f64
            };
            let runtime_fps = if sample_seconds > 0.0 {
                runtime_delta as f64 / sample_seconds
            } else {
                0.0
            };
            let submitted_fps = if sample_seconds > 0.0 {
                submitted_delta as f64 / sample_seconds
            } else {
                0.0
            };
            let flight_speed = self.flight_speed_blocks_per_second.unwrap_or(0.0);
            let chunk_view_churn_interval_seconds = self
                .chunk_view_churn
                .map_or(0.0, |churn| churn.interval_seconds);
            let chunk_view_churn_offset_chunks =
                self.chunk_view_churn.map_or(0, |churn| churn.offset_chunks);
            let flight_distance_blocks =
                camera_distance_blocks(self.start_camera, self.latest_camera);
            let latest_upload = self.latest_summary.upload;
            let feature_publications_delta = latest_upload
                .poll_scheduler_cumulative_feature_chunks_published
                .saturating_sub(self.start_scheduler_cumulative_feature_chunks_published);
            let light_publications_delta = latest_upload
                .poll_scheduler_cumulative_light_statuses_published
                .saturating_sub(self.start_scheduler_cumulative_light_statuses_published);
            let publication_units_delta =
                feature_publications_delta.saturating_add(light_publications_delta);
            let feature_publications_per_second =
                per_second_u64(feature_publications_delta, sample_seconds);
            let light_publications_per_second =
                per_second_u64(light_publications_delta, sample_seconds);
            let publication_units_per_second =
                per_second_u64(publication_units_delta, sample_seconds);
            let record_cache_delta = self
                .latest_record_cache
                .sample_delta(self.start_record_cache);
            let record_rebuild_avg_ms = if self.record_rebuild_frames == 0 {
                0.0
            } else {
                self.record_rebuild_total_ms / self.record_rebuild_frames as f64
            };
            log::info!(
                "MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds={:.3} mode={} detail={} render_path={} frame_accounting_enabled={} conservation_violations={} render_section_upload_budget={} render_section_accept_budget={} render_completed_result_accept_budget={} skip_actors={} adaptive_chunk_publication_budget={} xr_foveation={} xr_render_scale={:.3} xr_eye_size={}x{} render_distance={} render_compile_workers={} render_compile_max_pending_jobs={} flight_speed_blocks_per_second={:.3} chunk_view_churn_interval_seconds={:.3} chunk_view_churn_offset_chunks={} flight_distance_blocks={:.3} settle_seconds={:.3} settle_min_seconds={:.3} settle_frames={} settle_quiet_frames={} refresh_supported={} current_hz={} supported_hz={} target_hz={:.1} budget_ms={:.3} frames={} submitted_delta={} runtime_delta={} skipped_delta={} frame_avg_ms={:.3} frame_min_ms={:.3} frame_p50_ms={:.3} frame_p95_ms={:.3} frame_p99_ms={:.3} frame_max_ms={:.3} over_budget={} {}={} {}={} app_work_avg_ms={:.3} app_work_p50_ms={:.3} app_work_p95_ms={:.3} headroom_avg_ms={:.3} app_over_period_frames={} app_over_period_pct={:.1}",
                sample_seconds,
                self.mode_label,
                self.detail.label(),
                self.render_path.label(),
                self.frame_accounting_enabled,
                frame_accounting.conservation_violations.total(),
                format_optional_usize(self.render_section_upload_budget),
                format_optional_usize(self.render_section_accept_budget),
                format_optional_usize(self.render_completed_result_accept_budget),
                self.skip_actors,
                self.adaptive_chunk_publication_budget,
                self.xr_foveation.label(),
                self.xr_render_scale,
                self.xr_eye_size[0],
                self.xr_eye_size[1],
                self.render_distance,
                self.render_compile_worker_count,
                format_optional_usize(self.render_compile_max_pending_jobs),
                flight_speed,
                chunk_view_churn_interval_seconds,
                chunk_view_churn_offset_chunks,
                flight_distance_blocks,
                self.settle_seconds,
                ANDROID_XR_PERF_SETTLE_MIN_SECONDS,
                self.settle_frames,
                self.settle_quiet_frames,
                self.display_refresh.extension_supported,
                format_optional_hz(self.display_refresh.current_rate),
                format_supported_hz(&self.display_refresh.supported_rates),
                self.target_hz,
                budget_ms,
                frame_count,
                submitted_delta,
                runtime_delta,
                skipped_delta,
                frame_wall.average_ms,
                frame_wall.min_ms,
                frame_wall.p50_ms,
                frame_wall.p95_ms,
                frame_wall.p99_ms,
                frame_wall.max_ms,
                over_budget.single_period_frames(),
                mclone_diagnostics::legacy_json_keys::OVER_DOUBLE_BUDGET,
                over_budget.double_period_frames(),
                mclone_diagnostics::legacy_json_keys::OVER_QUAD_BUDGET,
                over_budget.quad_period_frames(),
                app_work.average_ms,
                app_work.p50_ms,
                app_work.p95_ms,
                headroom.average_ms,
                frame_accounting.app_over_period_frames,
                frame_accounting.app_over_period_pct
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_PUBLICATION sample_seconds={:.3} mode={} render_path={} adaptive_chunk_publication_budget={} feature_chunks_delta={} light_statuses_delta={} publication_units_delta={} feature_chunks_per_second={:.3} light_statuses_per_second={:.3} publication_units_per_second={:.3} latest_feature_cumulative={} latest_light_cumulative={}",
                sample_seconds,
                self.mode_label,
                self.render_path.label(),
                self.adaptive_chunk_publication_budget,
                feature_publications_delta,
                light_publications_delta,
                publication_units_delta,
                feature_publications_per_second,
                light_publications_per_second,
                publication_units_per_second,
                latest_upload.poll_scheduler_cumulative_feature_chunks_published,
                latest_upload.poll_scheduler_cumulative_light_statuses_published
            );
            if let Some(frame_pipeline_report) = frame_pipeline_report.as_ref() {
                log_android_xr_frame_pipeline_report(frame_pipeline_report);
            }
            log::info!(
                "MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds={:.3} mode={} render_path={} xr_render_scale={:.3} target_hz={:.1} budget_ms={:.3} frames={} submitted_delta={} runtime_delta={} skipped_delta={} submitted_fps={:.2} runtime_fps={:.2} wait_frame_avg_ms={:.3} wait_frame_p50_ms={:.3} wait_frame_p95_ms={:.3} wait_frame_max_ms={:.3} app_work_avg_ms={:.3} app_work_min_ms={:.3} app_work_p50_ms={:.3} app_work_p95_ms={:.3} app_work_p99_ms={:.3} app_work_max_ms={:.3} headroom_avg_ms={:.3} headroom_p50_ms={:.3} headroom_p05_ms={:.3} headroom_p01_ms={:.3} headroom_min_ms={:.3} app_over_period_frames={} app_over_period_pct={:.1}",
                sample_seconds,
                self.mode_label,
                self.render_path.label(),
                self.xr_render_scale,
                self.target_hz,
                budget_ms,
                frame_count,
                submitted_delta,
                runtime_delta,
                skipped_delta,
                submitted_fps,
                runtime_fps,
                wait_frame.average_ms,
                wait_frame.p50_ms,
                wait_frame.p95_ms,
                wait_frame.max_ms,
                app_work.average_ms,
                app_work.min_ms,
                app_work.p50_ms,
                app_work.p95_ms,
                app_work.p99_ms,
                app_work.max_ms,
                headroom.average_ms,
                headroom.p50_ms,
                headroom.p05_ms,
                headroom.p01_ms,
                headroom.min_ms,
                frame_accounting.app_over_period_frames,
                frame_accounting.app_over_period_pct
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_CPU_BLOCKED sample_seconds={:.3} mode={} render_path={} frames={} valid_frames={} valid_pct={:.1} app_work_avg_ms={:.3} thread_cpu_avg_ms={:.3} thread_cpu_p50_ms={:.3} thread_cpu_p95_ms={:.3} thread_cpu_p99_ms={:.3} thread_cpu_max_ms={:.3} blocked_avg_ms={:.3} blocked_p50_ms={:.3} blocked_p95_ms={:.3} blocked_p99_ms={:.3} blocked_max_ms={:.3}",
                sample_seconds,
                self.mode_label,
                self.render_path.label(),
                frame_count,
                thread_cpu_valid_frames,
                thread_cpu_valid_pct,
                app_work.average_ms,
                thread_cpu.average_ms,
                thread_cpu.p50_ms,
                thread_cpu.p95_ms,
                thread_cpu.p99_ms,
                thread_cpu.max_ms,
                blocked.average_ms,
                blocked.p50_ms,
                blocked.p95_ms,
                blocked.p99_ms,
                blocked.max_ms
            );
            if !self.detail.is_full() {
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_DETAIL detail={} frame_details=disabled retained_frame_details={} max_detail_markers=disabled",
                    self.detail.label(),
                    self.frame_details.len()
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
                return;
            }
            self.log_worst_frames(&frame_accounting);
            log::info!(
                "MCLONE_ANDROID_XR_PERF_STAGES max_wait_begin_ms={:.3} max_wait_frame_ms={:.3} max_begin_frame_ms={:.3} max_controller_poll_ms={:.3} max_render_mclone_frame_ms={:.3} max_locate_views_ms={:.3} max_locomotion_ms={:.3} max_acquire_left_ms={:.3} max_acquire_right_ms={:.3} max_release_eyes_ms={:.3} max_end_frame_ms={:.3}",
                self.max_wait_begin_ms,
                self.max_wait_frame_ms,
                self.max_begin_frame_ms,
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
                "MCLONE_ANDROID_XR_PERF_LOCOMOTION max_input_ms={:.3} max_camera_apply_ms={:.3} max_commit_ms={:.3} max_commit_server_command_ms={:.3} max_commit_position_updates_ms={:.3} max_commit_interest_ms={:.3} max_gameplay_interaction_ms={:.3}",
                self.max_render.locomotion_input_ms,
                self.max_render.locomotion_camera_apply_ms,
                self.max_render.locomotion_commit_ms,
                self.max_render.locomotion_commit_server_command_ms,
                self.max_render.locomotion_commit_position_updates_ms,
                self.max_render.locomotion_commit_interest_ms,
                self.max_render.locomotion_gameplay_interaction_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_LOCOMOTION_COMMAND max_total_ms={:.3} max_send_ms={:.3} max_drain_updates_ms={:.3} max_apply_updates_ms={:.3} max_apply_dirty_mark_ms={:.3} max_apply_client_updates_ms={:.3} max_updates={} max_snapshot_updates={} max_section_block_updates={} max_unload_updates={}",
                self.max_render.locomotion_commit_server_command_ms,
                self.max_render.locomotion_commit_server_command_send_ms,
                self.max_render
                    .locomotion_commit_server_command_drain_updates_ms,
                self.max_render
                    .locomotion_commit_server_command_apply_updates_ms,
                self.max_render
                    .locomotion_commit_server_command_apply_dirty_mark_ms,
                self.max_render
                    .locomotion_commit_server_command_apply_client_updates_ms,
                self.max_render.locomotion_commit_server_command_updates,
                self.max_render
                    .locomotion_commit_server_command_snapshot_updates,
                self.max_render
                    .locomotion_commit_server_command_section_block_updates,
                self.max_render
                    .locomotion_commit_server_command_unload_updates
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND max_total_ms={:.3} max_send_ms={:.3} max_drain_updates_ms={:.3} max_apply_updates_ms={:.3} max_apply_dirty_mark_ms={:.3} max_apply_client_updates_ms={:.3} max_updates={} max_snapshot_updates={} max_section_block_updates={} max_unload_updates={}",
                self.max_render.locomotion_commit_interest_ms,
                self.max_render.locomotion_commit_interest_command_send_ms,
                self.max_render
                    .locomotion_commit_interest_command_drain_updates_ms,
                self.max_render
                    .locomotion_commit_interest_command_apply_updates_ms,
                self.max_render
                    .locomotion_commit_interest_command_apply_dirty_mark_ms,
                self.max_render
                    .locomotion_commit_interest_command_apply_client_updates_ms,
                self.max_render.locomotion_commit_interest_command_updates,
                self.max_render
                    .locomotion_commit_interest_command_snapshot_updates,
                self.max_render
                    .locomotion_commit_interest_command_section_block_updates,
                self.max_render
                    .locomotion_commit_interest_command_unload_updates
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
                "MCLONE_ANDROID_XR_PERF_TERRAIN_RUNTIME max_runtime_poll_ms={:.3} max_runtime_sync_ms={:.3} max_runtime_result_accept_ms={:.3} max_runtime_dirty_seed_ms={:.3} max_runtime_prepare_ms={:.3} max_runtime_submit_ms={:.3} max_runtime_submit_snapshot_ms={:.3} max_runtime_submit_handoff_ms={:.3} max_runtime_gpu_upload_ms={:.3} max_runtime_upload_enqueue_ms={:.3} max_runtime_upload_select_ms={:.3} max_runtime_upload_apply_ms={:.3} max_runtime_ready_sections_ms={:.3} max_runtime_ready_publish_ms={:.3}",
                self.max_render.terrain_runtime_poll_ms,
                self.max_render.terrain_runtime_sync_ms,
                self.max_render.terrain_runtime_result_accept_ms,
                self.max_render.terrain_runtime_dirty_seed_ms,
                self.max_render.terrain_runtime_prepare_ms,
                self.max_render.terrain_runtime_submit_ms,
                self.max_render.terrain_runtime_submit_snapshot_ms,
                self.max_render.terrain_runtime_submit_handoff_ms,
                self.max_render.terrain_runtime_gpu_upload_ms,
                self.max_render.terrain_runtime_upload_enqueue_ms,
                self.max_render.terrain_runtime_upload_select_ms,
                self.max_render.terrain_runtime_upload_apply_ms,
                self.max_render.terrain_runtime_ready_sections_ms,
                self.max_render.terrain_runtime_ready_publish_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_GPU_SYNC_MAX sync_unattributed_ms={:.3} gpu_upload_pre_sync_ms={:.3} gpu_upload_post_sync_ms={:.3}",
                self.max_render.terrain_runtime_sync_unattributed_ms,
                self.max_render.terrain_runtime_gpu_upload_pre_sync_ms,
                self.max_render.terrain_runtime_gpu_upload_post_sync_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_UPLOAD_APPLY_MAX dirty_mark_ms={:.3} remove_ms={:.3} section_state_ms={:.3} vertex_bytes_ms={:.3} vertex_buffer_ms={:.3} index_bytes_ms={:.3} index_buffer_ms={:.3} mesh_insert_ms={:.3} mesh_upload_worst_ms={:.3}",
                self.max_render.terrain_runtime_upload_apply_dirty_mark_ms,
                self.max_render.terrain_runtime_upload_apply_remove_ms,
                self.max_render
                    .terrain_runtime_upload_apply_section_state_ms,
                self.max_render.terrain_runtime_upload_apply_vertex_bytes_ms,
                self.max_render
                    .terrain_runtime_upload_apply_vertex_buffer_ms,
                self.max_render.terrain_runtime_upload_apply_index_bytes_ms,
                self.max_render.terrain_runtime_upload_apply_index_buffer_ms,
                self.max_render.terrain_runtime_upload_apply_mesh_insert_ms,
                self.max_render
                    .terrain_runtime_upload_apply_mesh_upload_worst_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_TERRAIN_SUBMIT_MAX max_handoff_single_ms={:.3} request_count={} max_request_build_ms={:.3} max_compiler_ms={:.3} max_compiler_single_ms={:.3} max_capacity_check_ms={:.3} max_capacity_check_single_ms={:.3} max_command_send_ms={:.3} max_command_send_single_ms={:.3} max_pending_mark_ms={:.3} max_pending_mark_single_ms={:.3} max_mark_inflight_ms={:.3} max_apply_ready_plan_ms={:.3} max_ready_update_ms={:.3} ready_sections={} deferred_sections={} dirty_chunks_before={} dirty_chunks_after={} dirty_sections_before={} dirty_sections_after={} inflight_sections_before={} inflight_sections_after={} request_target_sections={} request_target_sections_single={} request_snapshots={} request_snapshot_sections={} request_snapshot_sections_single={} request_light_sections={} request_light_sections_single={} request_revisions={} request_payload_bytes={} request_payload_bytes_single={}",
                self.max_render.terrain_runtime_submit_handoff_worst_ms,
                self.max_render.terrain_runtime_submit_request_count,
                self.max_render.terrain_runtime_submit_request_build_ms,
                self.max_render.terrain_runtime_submit_compiler_ms,
                self.max_render.terrain_runtime_submit_compiler_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_capacity_check_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_capacity_check_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_send_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_send_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_pending_mark_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_pending_mark_worst_ms,
                self.max_render.terrain_runtime_submit_mark_inflight_ms,
                self.max_render.terrain_runtime_submit_apply_ready_plan_ms,
                self.max_render.terrain_runtime_submit_ready_update_ms,
                self.max_render.terrain_runtime_submit_ready_section_count,
                self.max_render
                    .terrain_runtime_submit_deferred_section_count,
                self.max_render
                    .terrain_runtime_submit_dirty_chunk_count_before,
                self.max_render
                    .terrain_runtime_submit_dirty_chunk_count_after,
                self.max_render
                    .terrain_runtime_submit_dirty_section_count_before,
                self.max_render
                    .terrain_runtime_submit_dirty_section_count_after,
                self.max_render
                    .terrain_runtime_submit_inflight_section_count_before,
                self.max_render
                    .terrain_runtime_submit_inflight_section_count_after,
                self.max_render
                    .terrain_runtime_submit_request_target_section_count,
                self.max_render
                    .terrain_runtime_submit_request_target_section_count_worst,
                self.max_render
                    .terrain_runtime_submit_request_snapshot_count,
                self.max_render
                    .terrain_runtime_submit_request_snapshot_section_count,
                self.max_render
                    .terrain_runtime_submit_request_snapshot_section_count_worst,
                self.max_render
                    .terrain_runtime_submit_request_light_section_count,
                self.max_render
                    .terrain_runtime_submit_request_light_section_count_worst,
                self.max_render
                    .terrain_runtime_submit_request_revision_count,
                self.max_render
                    .terrain_runtime_submit_request_estimated_payload_bytes,
                self.max_render
                    .terrain_runtime_submit_request_estimated_payload_bytes_worst
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_TERRAIN_ENQUEUE_MAX max_lock_wait_ms={:.3} max_lock_wait_single_ms={:.3} max_slot_select_ms={:.3} max_slot_select_single_ms={:.3} max_slot_write_ms={:.3} max_slot_write_single_ms={:.3} max_queue_push_ms={:.3} max_queue_push_single_ms={:.3} max_notify_ms={:.3} max_notify_single_ms={:.3} max_post_enqueue_ms={:.3} max_post_enqueue_single_ms={:.3}",
                self.max_render
                    .terrain_runtime_submit_compiler_command_lock_wait_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_lock_wait_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_slot_select_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_slot_select_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_slot_write_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_slot_write_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_queue_push_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_queue_push_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_notify_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_notify_worst_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_post_enqueue_ms,
                self.max_render
                    .terrain_runtime_submit_compiler_command_post_enqueue_worst_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_TERRAIN_DISPATCHER_MAX pending_jobs={} max_pending_jobs={} available_slots={} queued_compile_tasks={}",
                self.max_render.terrain_runtime_dispatcher_pending_jobs,
                self.max_render.terrain_runtime_dispatcher_max_pending_jobs,
                self.max_render
                    .terrain_runtime_dispatcher_available_job_slots,
                self.max_render
                    .terrain_runtime_dispatcher_queued_compile_tasks
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
                "MCLONE_ANDROID_XR_PERF_TERRAIN_EYE_SPLIT max_left_full_frame_ms={:.3} max_left_sky_ms={:.3} max_left_far_lod_ms={:.3} max_left_opaque_ms={:.3} max_left_translucent_ms={:.3} max_left_actor_ms={:.3} max_left_screen_effect_ms={:.3} max_left_gui_ms={:.3} max_left_xr_fade_ms={:.3} max_left_xr_selection_ms={:.3} max_left_xr_world_lines_ms={:.3} max_left_xr_world_panel_ms={:.3} max_left_encoder_finish_ms={:.3} max_right_full_frame_ms={:.3} max_right_sky_ms={:.3} max_right_far_lod_ms={:.3} max_right_opaque_ms={:.3} max_right_translucent_ms={:.3} max_right_actor_ms={:.3} max_right_screen_effect_ms={:.3} max_right_gui_ms={:.3} max_right_xr_fade_ms={:.3} max_right_xr_selection_ms={:.3} max_right_xr_world_lines_ms={:.3} max_right_xr_world_panel_ms={:.3} max_right_encoder_finish_ms={:.3}",
                self.max_render.terrain_left_eye_full_frame_ms,
                self.max_render.terrain_left_eye_sky_ms,
                self.max_render.terrain_left_eye_far_lod_ms,
                self.max_render.terrain_left_eye_opaque_ms,
                self.max_render.terrain_left_eye_translucent_ms,
                self.max_render.terrain_left_eye_actor_ms,
                self.max_render.terrain_left_eye_screen_effect_ms,
                self.max_render.terrain_left_eye_gui_ms,
                self.max_render.terrain_left_eye_xr_fade_ms,
                self.max_render.terrain_left_eye_xr_selection_ms,
                self.max_render.terrain_left_eye_xr_world_lines_ms,
                self.max_render.terrain_left_eye_xr_world_panel_ms,
                self.max_render.terrain_left_eye_encoder_finish_ms,
                self.max_render.terrain_right_eye_full_frame_ms,
                self.max_render.terrain_right_eye_sky_ms,
                self.max_render.terrain_right_eye_far_lod_ms,
                self.max_render.terrain_right_eye_opaque_ms,
                self.max_render.terrain_right_eye_translucent_ms,
                self.max_render.terrain_right_eye_actor_ms,
                self.max_render.terrain_right_eye_screen_effect_ms,
                self.max_render.terrain_right_eye_gui_ms,
                self.max_render.terrain_right_eye_xr_fade_ms,
                self.max_render.terrain_right_eye_xr_selection_ms,
                self.max_render.terrain_right_eye_xr_world_lines_ms,
                self.max_render.terrain_right_eye_xr_world_panel_ms,
                self.max_render.terrain_right_eye_encoder_finish_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_OVERLAP max_runtime_prefetch_ms={:.3} max_runtime_prefetch_poll_ms={:.3} max_runtime_prefetch_sync_ms={:.3} max_runtime_prefetch_gpu_upload_ms={:.3} max_runtime_prefetch_ready_sections_ms={:.3}",
                self.max_render.terrain_overlap_runtime_prefetch_ms,
                self.max_render.terrain_overlap_runtime_prefetch_poll_ms,
                self.max_render.terrain_overlap_runtime_prefetch_sync_ms,
                self.max_render
                    .terrain_overlap_runtime_prefetch_gpu_upload_ms,
                self.max_render
                    .terrain_overlap_runtime_prefetch_ready_sections_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_MULTIVIEW max_multiview_sky_ms={:.3} max_multiview_terrain_ms={:.3} max_multiview_actor_ms={:.3} max_multiview_screen_effect_ms={:.3} max_multiview_world_overlays_ms={:.3} max_multiview_submit_ms={:.3} max_multiview_poll_wait_ms={:.3}",
                self.max_render.terrain_multiview_sky_ms,
                self.max_render.terrain_multiview_terrain_ms,
                self.max_render.terrain_multiview_actor_ms,
                self.max_render.terrain_multiview_screen_effect_ms,
                self.max_render.terrain_multiview_world_overlays_ms,
                self.max_render.terrain_multiview_submit_ms,
                self.max_render.terrain_multiview_poll_wait_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_UPLOAD_MAX work_frames={} update_pump_stalled={} update_pump_stall_count={} server_update_queue_depth={} server_update_queue_bytes={} server_update_applied_bytes={} server_update_oldest_applied_age_ms={:.3} rebuilt_sections={} removed_sections={} rebuilt_vertices={} rebuilt_indices={} accepted_results={} queued_completed_results={} uploaded_sections={} upload_removed_sections={} uploaded_vertices={} uploaded_indices={} queued_upload_sections={} queued_upload_removed_sections={} ready_sections={}",
                self.upload_work_frames,
                self.max_upload.update_pump_stalled,
                self.max_upload.update_pump_stall_count,
                self.max_upload.server_update_queue_depth,
                self.max_upload.server_update_queue_bytes,
                self.max_upload.server_update_applied_bytes,
                self.max_upload.server_update_oldest_applied_age_ms,
                self.max_upload.rebuilt_section_count,
                self.max_upload.removed_section_count,
                self.max_upload.rebuilt_vertex_count,
                self.max_upload.rebuilt_index_count,
                self.max_upload.accepted_compile_result_count,
                self.max_upload.queued_completed_compile_result_count,
                self.max_upload.uploaded_section_count,
                self.max_upload.upload_removed_section_count,
                self.max_upload.uploaded_vertex_count,
                self.max_upload.uploaded_index_count,
                self.max_upload.queued_upload_section_count,
                self.max_upload.queued_upload_removed_section_count,
                self.max_upload.traversal_ready_section_count
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_UPLOAD_PHASE_MAX phase_events={} enqueued_lifecycle={} superseded_lifecycle={} drained_lifecycle={} released_jobs={} released_on_enqueue={} released_on_apply={} queued_lifecycle={} held_lifecycle={} held_jobs={} upload_limited={} accept_limited={} backpressured={}",
                self.max_upload.upload_phase_event_count,
                self.max_upload.upload_enqueued_lifecycle_item_count,
                self.max_upload.upload_superseded_lifecycle_item_count,
                self.max_upload.upload_drained_lifecycle_item_count,
                self.max_upload.upload_released_compile_job_count,
                self.max_upload.upload_released_compile_jobs_on_enqueue,
                self.max_upload.upload_released_compile_jobs_on_apply,
                self.max_upload.queued_upload_lifecycle_item_count,
                self.max_upload.upload_held_lifecycle_item_count,
                self.max_upload.upload_held_compile_job_count,
                self.max_upload.upload_limited,
                self.max_upload.upload_accept_limited,
                self.max_upload.upload_backpressured
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_RECORD_CACHE ready_set_calls={} ready_set_changed={} ready_set_unchanged={} ready_set_backpressured={} ready_set_backpressured_changed={} ready_set_backpressured_unchanged={} ready_set_skipped={} ready_set_backpressured_skipped={} prepared_rebuilds={} prepared_rebuild_frames={} prepared_rebuild_total_ms={:.3} prepared_rebuild_avg_ms={:.3} prepared_rebuild_max_ms={:.3} cumulative_rebuild_max_ms={:.3} prepared_rebuild_initial_dirty={} prepared_rebuild_upload_dirty={} prepared_rebuild_remove_dirty={} prepared_rebuild_ready_dirty={} prepared_rebuild_backpressured_dirty={} prepared_rebuild_multi_dirty={} cumulative_rebuild_visibility_max={} cumulative_rebuild_loaded_max={} cumulative_rebuild_ready_max={} cumulative_rebuild_index_max={}",
                record_cache_delta.ready_set_calls,
                record_cache_delta.ready_set_changed_calls,
                record_cache_delta.ready_set_unchanged_calls,
                record_cache_delta.ready_set_upload_backpressured_calls,
                record_cache_delta.ready_set_upload_backpressured_changed_calls,
                record_cache_delta.ready_set_upload_backpressured_unchanged_calls,
                record_cache_delta.ready_set_skipped_calls,
                record_cache_delta.ready_set_upload_backpressured_skipped_calls,
                record_cache_delta.prepared_record_rebuilds,
                self.record_rebuild_frames,
                self.record_rebuild_total_ms,
                record_rebuild_avg_ms,
                self.record_rebuild_max_ms,
                self.latest_record_cache.prepared_record_rebuild_max_ms,
                record_cache_delta.prepared_record_rebuild_initial_dirty,
                record_cache_delta.prepared_record_rebuild_section_upload_dirty,
                record_cache_delta.prepared_record_rebuild_section_remove_dirty,
                record_cache_delta.prepared_record_rebuild_ready_set_dirty,
                record_cache_delta.prepared_record_rebuild_upload_backpressured_dirty,
                record_cache_delta.prepared_record_rebuild_multi_dirty,
                self.latest_record_cache
                    .prepared_record_rebuild_visibility_section_max,
                self.latest_record_cache
                    .prepared_record_rebuild_loaded_section_max,
                self.latest_record_cache
                    .prepared_record_rebuild_ready_section_max,
                self.latest_record_cache
                    .prepared_record_rebuild_loaded_index_max
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_RUNTIME_MAX poll_total_ms={:.3} drain_updates_ms={:.3} producer_read_ms={:.3} producer_decode_ms={:.3} producer_response_sequence={} deferred_chunk_drop_ms={:.3} deferred_chunk_drop_items={} deferred_chunk_drop_backlog_items={} apply_updates_ms={:.3} dirty_mark_ms={:.3} client_apply_ms={:.3} poll_diagnostics_ms={:.3} diagnostics_refresh_frames={} diagnostics_refreshed={} diagnostics_cache_age_ms={:.3} server_detail_refreshes={} server_detail_age_ms={:.3} server_tick_ms={:.3} scheduler_tick_ms={:.3} updates={} snapshot_updates={} section_updates={} unload_updates={}",
                self.max_upload.poll_total_ms,
                self.max_upload.poll_drain_updates_ms,
                self.max_upload.poll_producer_read_ms,
                self.max_upload.poll_producer_decode_ms,
                self.max_upload.poll_producer_response_sequence.unwrap_or(0),
                self.max_upload.poll_client_deferred_chunk_drop_ms,
                self.max_upload.poll_client_deferred_chunk_drop_items,
                self.max_upload
                    .poll_client_deferred_chunk_drop_backlog_items,
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
                "MCLONE_ANDROID_XR_PERF_UPDATE_APPLY_MAX snapshot_ms={:.3} snapshot_dirty_ms={:.3} snapshot_client_ms={:.3} section_ms={:.3} section_dirty_ms={:.3} section_client_ms={:.3} unload_ms={:.3} unload_dirty_ms={:.3} unload_client_ms={:.3} other_ms={:.3} other_dirty_ms={:.3} other_client_ms={:.3} mixed_ms={:.3} mixed_dirty_ms={:.3} mixed_client_ms={:.3} snapshot_updates={} section_updates={} unload_updates={} other_updates={} mixed_updates={}",
                self.max_upload.poll_snapshot_update_apply_ms,
                self.max_upload.poll_snapshot_update_dirty_mark_ms,
                self.max_upload.poll_snapshot_update_client_apply_ms,
                self.max_upload.poll_section_block_update_apply_ms,
                self.max_upload.poll_section_block_update_dirty_mark_ms,
                self.max_upload.poll_section_block_update_client_apply_ms,
                self.max_upload.poll_unload_update_apply_ms,
                self.max_upload.poll_unload_update_dirty_mark_ms,
                self.max_upload.poll_unload_update_client_apply_ms,
                self.max_upload.poll_other_update_apply_ms,
                self.max_upload.poll_other_update_dirty_mark_ms,
                self.max_upload.poll_other_update_client_apply_ms,
                self.max_upload.poll_mixed_update_apply_ms,
                self.max_upload.poll_mixed_update_dirty_mark_ms,
                self.max_upload.poll_mixed_update_client_apply_ms,
                self.max_upload.poll_snapshot_updates,
                self.max_upload.poll_section_block_updates,
                self.max_upload.poll_unload_updates,
                self.max_upload.poll_other_updates,
                self.max_upload.poll_mixed_updates
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_QUEUE_MAX server_cmd_q={} server_update_q={} server_pending_jobs={} server_pending_publications={} scheduler_pending_jobs={} scheduler_completed_jobs={} scheduler_dirty_chunks={} scheduler_loaded_chunks={} scheduler_visible_chunks={} scheduler_ticket_chunks={} player_visible_chunks={} player_outbound_q={} scheduler_feature_published={} scheduler_light_published={} scheduler_snapshot_ready={} scheduler_feature_drained={} scheduler_feature_jobs_completed={} scheduler_pending_worldgen_publication_jobs={} scheduler_pending_worldgen_publication_chunks={} scheduler_pending_light_publications={} scheduler_worldgen_mailbox_pending_jobs={} scheduler_light_mailbox_pending_statuses={}",
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
                self.max_upload.player_outbound_queue_depth,
                self.max_upload.poll_scheduler_feature_chunks_published,
                self.max_upload.poll_scheduler_light_statuses_published,
                self.max_upload
                    .poll_scheduler_feature_snapshot_ready_events
                    .saturating_add(self.max_upload.poll_scheduler_light_snapshot_ready_events),
                self.max_upload
                    .poll_scheduler_completed_feature_jobs_drained,
                self.max_upload.poll_scheduler_feature_jobs_completed,
                self.max_upload
                    .poll_scheduler_pending_worldgen_publication_jobs,
                self.max_upload
                    .poll_scheduler_pending_worldgen_publication_chunks,
                self.max_upload.poll_scheduler_pending_light_publications,
                self.max_upload.poll_scheduler_worldgen_mailbox_pending_jobs,
                self.max_upload
                    .poll_scheduler_light_mailbox_pending_statuses
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_COMPILE_MAX pending_chunks_before={} pending_chunks_after={} pending_jobs_before={} pending_jobs_after={} max_pending_jobs={} available_slots_before={} available_slots_after={} neighbor_ready_sections={} near_exception_sections={} deferred_sections={} submitted_sections={} deadline_skipped_requests={} accepted_results={} queued_completed_results={} completed_sections={} stale_sections={} visibility_graph_builds={} visibility_graph_total_ms={:.3} visibility_graph_worst_ms={:.3}",
                self.max_upload.pending_render_chunks_before,
                self.max_upload.pending_render_chunks_after,
                self.max_upload.pending_compile_jobs_before,
                self.max_upload.pending_compile_jobs_after,
                self.max_upload.max_pending_compile_jobs,
                self.max_upload.available_compile_slots_before,
                self.max_upload.available_compile_slots_after,
                self.max_upload.neighbor_ready_section_count,
                self.max_upload.near_exception_section_count,
                self.max_upload.deferred_section_count,
                self.max_upload.submitted_compile_section_count,
                self.max_upload.deadline_skipped_compile_request_count,
                self.max_upload.accepted_compile_result_count,
                self.max_upload.queued_completed_compile_result_count,
                self.max_upload.completed_compile_section_count,
                self.max_upload.stale_compile_section_count,
                self.max_upload.visibility_graph_build_count,
                self.max_upload.visibility_graph_total_ms,
                self.max_upload.visibility_graph_worst_ms
            );
            log::info!(
                "MCLONE_ANDROID_XR_PERF_UPLOAD_LAST poll_changed={} pending_chunks_before={} pending_chunks_after={} pending_jobs_before={} pending_jobs_after={} max_pending_jobs={} available_slots_before={} available_slots_after={} deadline_skipped_requests={} accepted_results={} queued_completed_results={} rebuilt_sections={} removed_sections={} uploaded_sections={} upload_removed_sections={} uploaded_indices={} ready_sections={}",
                latest_upload.poll_changed,
                latest_upload.pending_render_chunks_before,
                latest_upload.pending_render_chunks_after,
                latest_upload.pending_compile_jobs_before,
                latest_upload.pending_compile_jobs_after,
                latest_upload.max_pending_compile_jobs,
                latest_upload.available_compile_slots_before,
                latest_upload.available_compile_slots_after,
                latest_upload.deadline_skipped_compile_request_count,
                latest_upload.accepted_compile_result_count,
                latest_upload.queued_completed_compile_result_count,
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

        fn log_worst_frames(&self, frame_accounting: &FrameSummaryReport) {
            for detail in &frame_accounting.worst_frames {
                let Some(snapshot) = self
                    .frame_details
                    .iter()
                    .find(|snapshot| snapshot.sample_frame == detail.frame_index)
                else {
                    continue;
                };
                let rank = detail.rank;
                let timing = snapshot.timing;
                let render = timing.render;
                let summary = snapshot.summary;
                let upload = summary.map(|summary| summary.upload);
                let known_render_ms = render.locate_views_ms
                    + render.locomotion_ms
                    + render.acquire_left_ms
                    + render.acquire_right_ms
                    + render.terrain_render_frame_ms
                    + render.release_eyes_ms
                    + render.end_frame_ms;
                let render_unattributed_ms =
                    (timing.render_mclone_frame_ms - known_render_ms).max(0.0);
                let terrain_poll_wait_ms = render.terrain_stereo_poll_wait_ms;
                let terrain_before_poll_wait_ms =
                    (render.terrain_render_frame_ms - terrain_poll_wait_ms).max(0.0);
                let eye_poll_wait_ms =
                    render.terrain_left_eye_poll_wait_ms + render.terrain_right_eye_poll_wait_ms;
                let eye_cpu_ms = (render.terrain_left_eye_ms + render.terrain_right_eye_ms
                    - eye_poll_wait_ms)
                    .max(0.0);
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME rank={} sample_frame={} rendered={} frame_wall_ms={:.3} wait_frame_ms={:.3} app_work_ms={:.3} thread_cpu_valid={} thread_cpu_ms={:.3} blocked_ms={:.3} headroom_ms={:.3} over_budget={} {}={} wait_begin_ms={:.3} begin_frame_ms={:.3} controller_poll_ms={:.3} render_mclone_frame_ms={:.3} locate_views_ms={:.3} locomotion_ms={:.3} acquire_left_ms={:.3} acquire_right_ms={:.3} release_eyes_ms={:.3} end_frame_ms={:.3}",
                    rank,
                    detail.frame_index,
                    detail.rendered,
                    detail.frame_wall_ms,
                    detail.wait_ms,
                    detail.app_work_ms,
                    detail.thread_cpu_ms.is_some(),
                    detail.thread_cpu_ms.unwrap_or(0.0),
                    detail.blocked_ms.unwrap_or(0.0),
                    detail.headroom_ms.unwrap_or(0.0),
                    detail.over_single_budget(),
                    mclone_diagnostics::legacy_json_keys::OVER_DOUBLE_BUDGET,
                    detail.over_double_budget(),
                    timing.wait_begin_ms,
                    timing.begin_frame_ms,
                    timing.controller_poll_ms,
                    timing.render_mclone_frame_ms,
                    render.locate_views_ms,
                    render.locomotion_ms,
                    render.acquire_left_ms,
                    render.acquire_right_ms,
                    render.release_eyes_ms,
                    render.end_frame_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_LOCOMOTION rank={} sample_frame={} input_ms={:.3} camera_apply_ms={:.3} commit_ms={:.3} commit_server_command_ms={:.3} commit_position_updates_ms={:.3} commit_interest_ms={:.3} gameplay_interaction_ms={:.3}",
                    rank,
                    detail.frame_index,
                    render.locomotion_input_ms,
                    render.locomotion_camera_apply_ms,
                    render.locomotion_commit_ms,
                    render.locomotion_commit_server_command_ms,
                    render.locomotion_commit_position_updates_ms,
                    render.locomotion_commit_interest_ms,
                    render.locomotion_gameplay_interaction_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_LOCOMOTION_COMMAND rank={} sample_frame={} total_ms={:.3} send_ms={:.3} drain_updates_ms={:.3} apply_updates_ms={:.3} apply_dirty_mark_ms={:.3} apply_client_updates_ms={:.3} updates={} snapshot_updates={} section_block_updates={} unload_updates={}",
                    rank,
                    snapshot.sample_frame,
                    render.locomotion_commit_server_command_ms,
                    render.locomotion_commit_server_command_send_ms,
                    render.locomotion_commit_server_command_drain_updates_ms,
                    render.locomotion_commit_server_command_apply_updates_ms,
                    render.locomotion_commit_server_command_apply_dirty_mark_ms,
                    render.locomotion_commit_server_command_apply_client_updates_ms,
                    render.locomotion_commit_server_command_updates,
                    render.locomotion_commit_server_command_snapshot_updates,
                    render.locomotion_commit_server_command_section_block_updates,
                    render.locomotion_commit_server_command_unload_updates
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_LOCOMOTION_INTEREST_COMMAND rank={} sample_frame={} total_ms={:.3} send_ms={:.3} drain_updates_ms={:.3} apply_updates_ms={:.3} apply_dirty_mark_ms={:.3} apply_client_updates_ms={:.3} updates={} snapshot_updates={} section_block_updates={} unload_updates={}",
                    rank,
                    snapshot.sample_frame,
                    render.locomotion_commit_interest_ms,
                    render.locomotion_commit_interest_command_send_ms,
                    render.locomotion_commit_interest_command_drain_updates_ms,
                    render.locomotion_commit_interest_command_apply_updates_ms,
                    render.locomotion_commit_interest_command_apply_dirty_mark_ms,
                    render.locomotion_commit_interest_command_apply_client_updates_ms,
                    render.locomotion_commit_interest_command_updates,
                    render.locomotion_commit_interest_command_snapshot_updates,
                    render.locomotion_commit_interest_command_section_block_updates,
                    render.locomotion_commit_interest_command_unload_updates
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_BUDGET rank={} sample_frame={} known_render_ms={:.3} render_unattributed_ms={:.3} terrain_before_poll_wait_ms={:.3} terrain_poll_wait_ms={:.3} eye_cpu_ms={:.3} eye_poll_wait_ms={:.3}",
                    rank,
                    snapshot.sample_frame,
                    known_render_ms,
                    render_unattributed_ms,
                    terrain_before_poll_wait_ms,
                    terrain_poll_wait_ms,
                    eye_cpu_ms,
                    eye_poll_wait_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_TERRAIN rank={} sample_frame={} terrain_frame_ms={:.3} render_views_ms={:.3} runtime_upload_ms={:.3} shared_records_ms={:.3} left_eye_ms={:.3} right_eye_ms={:.3} left_prepare_ms={:.3} left_encode_ms={:.3} left_submit_ms={:.3} left_poll_wait_ms={:.3} right_prepare_ms={:.3} right_encode_ms={:.3} right_submit_ms={:.3} right_poll_wait_ms={:.3} stereo_submit_ms={:.3} stereo_poll_wait_ms={:.3}",
                    rank,
                    snapshot.sample_frame,
                    render.terrain_render_frame_ms,
                    render.terrain_render_views_ms,
                    render.terrain_runtime_upload_ms,
                    render.terrain_shared_records_ms,
                    render.terrain_left_eye_ms,
                    render.terrain_right_eye_ms,
                    render.terrain_left_eye_prepare_ms,
                    render.terrain_left_eye_encode_ms,
                    render.terrain_left_eye_submit_ms,
                    render.terrain_left_eye_poll_wait_ms,
                    render.terrain_right_eye_prepare_ms,
                    render.terrain_right_eye_encode_ms,
                    render.terrain_right_eye_submit_ms,
                    render.terrain_right_eye_poll_wait_ms,
                    render.terrain_stereo_submit_ms,
                    render.terrain_stereo_poll_wait_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_EYE_SPLIT rank={} sample_frame={} left_full_frame_ms={:.3} left_sky_ms={:.3} left_far_lod_ms={:.3} left_opaque_ms={:.3} left_translucent_ms={:.3} left_actor_ms={:.3} left_screen_effect_ms={:.3} left_gui_ms={:.3} left_xr_fade_ms={:.3} left_xr_selection_ms={:.3} left_xr_world_lines_ms={:.3} left_xr_world_panel_ms={:.3} left_encoder_finish_ms={:.3} right_full_frame_ms={:.3} right_sky_ms={:.3} right_far_lod_ms={:.3} right_opaque_ms={:.3} right_translucent_ms={:.3} right_actor_ms={:.3} right_screen_effect_ms={:.3} right_gui_ms={:.3} right_xr_fade_ms={:.3} right_xr_selection_ms={:.3} right_xr_world_lines_ms={:.3} right_xr_world_panel_ms={:.3} right_encoder_finish_ms={:.3}",
                    rank,
                    snapshot.sample_frame,
                    render.terrain_left_eye_full_frame_ms,
                    render.terrain_left_eye_sky_ms,
                    render.terrain_left_eye_far_lod_ms,
                    render.terrain_left_eye_opaque_ms,
                    render.terrain_left_eye_translucent_ms,
                    render.terrain_left_eye_actor_ms,
                    render.terrain_left_eye_screen_effect_ms,
                    render.terrain_left_eye_gui_ms,
                    render.terrain_left_eye_xr_fade_ms,
                    render.terrain_left_eye_xr_selection_ms,
                    render.terrain_left_eye_xr_world_lines_ms,
                    render.terrain_left_eye_xr_world_panel_ms,
                    render.terrain_left_eye_encoder_finish_ms,
                    render.terrain_right_eye_full_frame_ms,
                    render.terrain_right_eye_sky_ms,
                    render.terrain_right_eye_far_lod_ms,
                    render.terrain_right_eye_opaque_ms,
                    render.terrain_right_eye_translucent_ms,
                    render.terrain_right_eye_actor_ms,
                    render.terrain_right_eye_screen_effect_ms,
                    render.terrain_right_eye_gui_ms,
                    render.terrain_right_eye_xr_fade_ms,
                    render.terrain_right_eye_xr_selection_ms,
                    render.terrain_right_eye_xr_world_lines_ms,
                    render.terrain_right_eye_xr_world_panel_ms,
                    render.terrain_right_eye_encoder_finish_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_RUNTIME rank={} sample_frame={} poll_ms={:.3} sync_ms={:.3} result_accept_ms={:.3} dirty_seed_ms={:.3} prepare_ms={:.3} submit_ms={:.3} submit_snapshot_ms={:.3} submit_handoff_ms={:.3} gpu_upload_ms={:.3} upload_enqueue_ms={:.3} upload_select_ms={:.3} upload_apply_ms={:.3} ready_sections_ms={:.3} ready_publish_ms={:.3} submit_request_count={} submit_request_build_ms={:.3} compiler_ms={:.3} compiler_single_ms={:.3} command_send_ms={:.3} command_send_single_ms={:.3} notify_ms={:.3} notify_single_ms={:.3} mark_inflight_ms={:.3} apply_ready_plan_ms={:.3} ready_update_ms={:.3}",
                    rank,
                    snapshot.sample_frame,
                    render.terrain_runtime_poll_ms,
                    render.terrain_runtime_sync_ms,
                    render.terrain_runtime_result_accept_ms,
                    render.terrain_runtime_dirty_seed_ms,
                    render.terrain_runtime_prepare_ms,
                    render.terrain_runtime_submit_ms,
                    render.terrain_runtime_submit_snapshot_ms,
                    render.terrain_runtime_submit_handoff_ms,
                    render.terrain_runtime_gpu_upload_ms,
                    render.terrain_runtime_upload_enqueue_ms,
                    render.terrain_runtime_upload_select_ms,
                    render.terrain_runtime_upload_apply_ms,
                    render.terrain_runtime_ready_sections_ms,
                    render.terrain_runtime_ready_publish_ms,
                    render.terrain_runtime_submit_request_count,
                    render.terrain_runtime_submit_request_build_ms,
                    render.terrain_runtime_submit_compiler_ms,
                    render.terrain_runtime_submit_compiler_worst_ms,
                    render.terrain_runtime_submit_compiler_command_send_ms,
                    render.terrain_runtime_submit_compiler_command_send_worst_ms,
                    render.terrain_runtime_submit_compiler_command_notify_ms,
                    render.terrain_runtime_submit_compiler_command_notify_worst_ms,
                    render.terrain_runtime_submit_mark_inflight_ms,
                    render.terrain_runtime_submit_apply_ready_plan_ms,
                    render.terrain_runtime_submit_ready_update_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_GPU_SYNC rank={} sample_frame={} sync_unattributed_ms={:.3} gpu_upload_pre_sync_ms={:.3} gpu_upload_post_sync_ms={:.3}",
                    rank,
                    snapshot.sample_frame,
                    render.terrain_runtime_sync_unattributed_ms,
                    render.terrain_runtime_gpu_upload_pre_sync_ms,
                    render.terrain_runtime_gpu_upload_post_sync_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPLOAD_APPLY rank={} sample_frame={} dirty_mark_ms={:.3} remove_ms={:.3} section_state_ms={:.3} vertex_bytes_ms={:.3} vertex_buffer_ms={:.3} index_bytes_ms={:.3} index_buffer_ms={:.3} mesh_insert_ms={:.3} mesh_upload_worst_ms={:.3}",
                    rank,
                    snapshot.sample_frame,
                    render.terrain_runtime_upload_apply_dirty_mark_ms,
                    render.terrain_runtime_upload_apply_remove_ms,
                    render.terrain_runtime_upload_apply_section_state_ms,
                    render.terrain_runtime_upload_apply_vertex_bytes_ms,
                    render.terrain_runtime_upload_apply_vertex_buffer_ms,
                    render.terrain_runtime_upload_apply_index_bytes_ms,
                    render.terrain_runtime_upload_apply_index_buffer_ms,
                    render.terrain_runtime_upload_apply_mesh_insert_ms,
                    render.terrain_runtime_upload_apply_mesh_upload_worst_ms
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPDATE_APPLY rank={} sample_frame={} snapshot_ms={:.3} snapshot_dirty_ms={:.3} snapshot_client_ms={:.3} section_ms={:.3} section_dirty_ms={:.3} section_client_ms={:.3} unload_ms={:.3} unload_dirty_ms={:.3} unload_client_ms={:.3} other_ms={:.3} other_dirty_ms={:.3} other_client_ms={:.3} mixed_ms={:.3} mixed_dirty_ms={:.3} mixed_client_ms={:.3} snapshot_updates={} section_updates={} unload_updates={} other_updates={} mixed_updates={}",
                    rank,
                    snapshot.sample_frame,
                    upload.map_or(0.0, |upload| upload.poll_snapshot_update_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_snapshot_update_dirty_mark_ms),
                    upload.map_or(0.0, |upload| upload.poll_snapshot_update_client_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_section_block_update_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_section_block_update_dirty_mark_ms),
                    upload.map_or(0.0, |upload| upload
                        .poll_section_block_update_client_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_unload_update_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_unload_update_dirty_mark_ms),
                    upload.map_or(0.0, |upload| upload.poll_unload_update_client_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_other_update_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_other_update_dirty_mark_ms),
                    upload.map_or(0.0, |upload| upload.poll_other_update_client_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_mixed_update_apply_ms),
                    upload.map_or(0.0, |upload| upload.poll_mixed_update_dirty_mark_ms),
                    upload.map_or(0.0, |upload| upload.poll_mixed_update_client_apply_ms),
                    upload.map_or(0, |upload| upload.poll_snapshot_updates),
                    upload.map_or(0, |upload| upload.poll_section_block_updates),
                    upload.map_or(0, |upload| upload.poll_unload_updates),
                    upload.map_or(0, |upload| upload.poll_other_updates),
                    upload.map_or(0, |upload| upload.poll_mixed_updates)
                );
                log::info!(
                    "MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPLOAD rank={} sample_frame={} sections={} drawn_sections={} indices={} drawn_indices={} ready_sections={} poll_changed={} pending_chunks_after={} pending_jobs_after={} submitted_sections={} accepted_results={} queued_completed_results={} completed_sections={} stale_sections={} uploaded_sections={} upload_removed_sections={} upload_limited={} accept_limited={} backpressured={} held_lifecycle={} held_jobs={} server_cmd_q={} server_update_q={} scheduler_pending_jobs={} scheduler_completed_jobs={} scheduler_dirty_chunks={} scheduler_loaded_chunks={} scheduler_ticket_chunks={} scheduler_feature_published={} scheduler_light_published={} scheduler_snapshot_ready={} scheduler_pending_worldgen_publication_chunks={} scheduler_pending_light_publications={} scheduler_worldgen_mailbox_pending_jobs={} scheduler_light_mailbox_pending_statuses={} request_target_sections={} request_target_sections_single={} request_snapshots={} request_snapshot_sections={} request_snapshot_sections_single={} request_payload_bytes={} request_payload_bytes_single={} dispatcher_pending_jobs={} dispatcher_queued_compile_tasks={}",
                    rank,
                    snapshot.sample_frame,
                    summary.map_or(0, |summary| summary.section_count),
                    summary.map_or(0, |summary| summary.drawn_section_count),
                    summary.map_or(0, |summary| summary.index_count),
                    summary.map_or(0, |summary| summary.drawn_index_count),
                    upload.map_or(0, |upload| upload.traversal_ready_section_count),
                    upload.map_or(false, |upload| upload.poll_changed),
                    upload.map_or(0, |upload| upload.pending_render_chunks_after),
                    upload.map_or(0, |upload| upload.pending_compile_jobs_after),
                    upload.map_or(0, |upload| upload.submitted_compile_section_count),
                    upload.map_or(0, |upload| upload.accepted_compile_result_count),
                    upload.map_or(0, |upload| upload.queued_completed_compile_result_count),
                    upload.map_or(0, |upload| upload.completed_compile_section_count),
                    upload.map_or(0, |upload| upload.stale_compile_section_count),
                    upload.map_or(0, |upload| upload.uploaded_section_count),
                    upload.map_or(0, |upload| upload.upload_removed_section_count),
                    upload.map_or(false, |upload| upload.upload_limited),
                    upload.map_or(false, |upload| upload.upload_accept_limited),
                    upload.map_or(false, |upload| upload.upload_backpressured),
                    upload.map_or(0, |upload| upload.upload_held_lifecycle_item_count),
                    upload.map_or(0, |upload| upload.upload_held_compile_job_count),
                    upload.map_or(0, |upload| upload.server_command_queue_depth),
                    upload.map_or(0, |upload| upload.server_update_queue_depth),
                    upload.map_or(0, |upload| upload.scheduler_pending_jobs),
                    upload.map_or(0, |upload| upload.scheduler_completed_jobs),
                    upload.map_or(0, |upload| upload.scheduler_dirty_chunks),
                    upload.map_or(0, |upload| upload.scheduler_loaded_snapshot_chunks),
                    upload.map_or(0, |upload| upload.scheduler_active_ticket_chunks),
                    upload.map_or(0, |upload| upload.poll_scheduler_feature_chunks_published),
                    upload.map_or(0, |upload| upload.poll_scheduler_light_statuses_published),
                    upload.map_or(0, |upload| upload
                        .poll_scheduler_feature_snapshot_ready_events
                        .saturating_add(upload.poll_scheduler_light_snapshot_ready_events)),
                    upload.map_or(0, |upload| upload
                        .poll_scheduler_pending_worldgen_publication_chunks),
                    upload.map_or(0, |upload| upload.poll_scheduler_pending_light_publications),
                    upload.map_or(0, |upload| upload
                        .poll_scheduler_worldgen_mailbox_pending_jobs),
                    upload.map_or(0, |upload| upload
                        .poll_scheduler_light_mailbox_pending_statuses),
                    render.terrain_runtime_submit_request_target_section_count,
                    render.terrain_runtime_submit_request_target_section_count_worst,
                    render.terrain_runtime_submit_request_snapshot_count,
                    render.terrain_runtime_submit_request_snapshot_section_count,
                    render.terrain_runtime_submit_request_snapshot_section_count_worst,
                    render.terrain_runtime_submit_request_estimated_payload_bytes,
                    render.terrain_runtime_submit_request_estimated_payload_bytes_worst,
                    render.terrain_runtime_dispatcher_pending_jobs,
                    render.terrain_runtime_dispatcher_queued_compile_tasks
                );
            }
        }
    }

    fn android_xr_frame_accounting_accumulator(target_hz: f64) -> FrameAccumulator {
        FrameAccumulator::new(
            FrameAccountingConfig::from_target_hz(target_hz)
                .with_percentile_method(PercentileMethod::NearestRank)
                .with_worst_frame_capacity(ANDROID_XR_PERF_WORST_FRAME_COUNT),
        )
    }

    fn android_xr_frame_observation(
        sample_frame: u64,
        timing: AndroidXrFrameTiming,
        rendered: bool,
    ) -> FrameObservation {
        let app_work_ms = (timing.frame_wall_ms - timing.wait_frame_ms).max(0.0);
        let thread_cpu_ms = timing.thread_cpu_valid.then_some(timing.thread_cpu_ms);
        let mut observation = FrameObservation::new(sample_frame, timing.frame_wall_ms)
            .with_wait_ms(timing.wait_frame_ms)
            .with_app_work_ms(app_work_ms)
            .with_thread_cpu_ms(thread_cpu_ms)
            .with_rendered(rendered)
            .with_stage_span(StageSpan::new(
                StageId::GpuExecutionPresentationWait,
                timing.wait_frame_ms,
            ))
            .with_stage_span(StageSpan::new(
                StageId::InputPoseEvents,
                timing.controller_poll_ms,
            ));
        for span in android_xr_render_stage_spans(timing.render) {
            observation = observation.with_stage_span(span);
        }
        observation
    }

    fn android_xr_render_stage_spans(timing: AndroidXrRenderFrameTiming) -> Vec<StageSpan> {
        let dirty_ready_scan_ms =
            timing.terrain_runtime_dirty_seed_ms + timing.terrain_runtime_prepare_ms;
        let request_build_ms = timing.terrain_runtime_submit_snapshot_ms
            + timing.terrain_runtime_submit_request_build_ms;
        let worker_submit_ms = timing.terrain_runtime_submit_compiler_ms;
        let prepared_record_ms = timing.terrain_runtime_submit_mark_inflight_ms
            + timing.terrain_runtime_submit_apply_ready_plan_ms
            + timing.terrain_runtime_submit_ready_update_ms;
        let attributed_sync_ms = timing.terrain_runtime_result_accept_ms
            + dirty_ready_scan_ms
            + request_build_ms
            + worker_submit_ms
            + prepared_record_ms;
        let sync_unattributed_ms = timing
            .terrain_runtime_sync_unattributed_ms
            .max((timing.terrain_runtime_sync_ms - attributed_sync_ms).max(0.0));
        let multiview_draw_ms = timing.terrain_multiview_sky_ms
            + timing.terrain_multiview_terrain_ms
            + timing.terrain_multiview_actor_ms
            + timing.terrain_multiview_screen_effect_ms
            + timing.terrain_multiview_world_overlays_ms
            + timing.terrain_multiview_submit_ms;
        let draw_encode_ms = timing.terrain_render_views_ms.max(multiview_draw_ms);
        vec![
            StageSpan::new(StageId::HostSessionCommands, timing.terrain_runtime_poll_ms),
            StageSpan::new(
                StageId::CompletedResultAcceptance,
                timing.terrain_runtime_result_accept_ms,
            ),
            StageSpan::new(StageId::RenderAdmissionDirtyReadyScan, dirty_ready_scan_ms),
            StageSpan::new(StageId::RenderAdmissionRequestBuild, request_build_ms),
            StageSpan::new(StageId::RenderAdmissionWorkerSubmit, worker_submit_ms),
            StageSpan::new(
                StageId::RenderAdmissionPreparedRecordMaintenance,
                prepared_record_ms,
            ),
            StageSpan::new(StageId::RenderSectionAdmission, sync_unattributed_ms),
            StageSpan::new(StageId::UploadApply, timing.terrain_runtime_gpu_upload_ms),
            StageSpan::new(
                StageId::PreparedDrawRecords,
                timing.terrain_shared_records_ms,
            ),
            StageSpan::new(StageId::DrawEncode, draw_encode_ms),
        ]
    }

    fn android_xr_queue_panel(details: &[AndroidXrWorstFrameSnapshot]) -> QueuePanelReport {
        let mut inbound_updates = QueueAgeTracker::new(QueueId::InboundUpdates);
        let mut completed_results = QueueAgeTracker::new(QueueId::CompletedRenderResults);
        let mut upload_work = QueueAgeTracker::new(QueueId::UploadWork);
        let mut host_publication = QueueAgeTracker::new(QueueId::HostPublication);
        let mut host_publication_runner = QueueAgeTracker::new(QueueId::HostPublicationRunner);
        let mut host_publication_worldgen = QueueAgeTracker::new(QueueId::HostPublicationWorldgen);
        let mut host_publication_light = QueueAgeTracker::new(QueueId::HostPublicationLight);
        let mut render_compile_jobs = QueueAgeTracker::new(QueueId::RenderCompileJobs);
        let mut now_ms = 0.0;

        for detail in details {
            now_ms += detail.timing.frame_wall_ms;
            let Some(summary) = detail.summary else {
                continue;
            };
            let upload = summary.upload;
            inbound_updates.reconcile_depth(usize_to_u64(upload.server_update_queue_depth), now_ms);

            let completed = usize_to_u64(upload.completed_compile_section_count);
            if completed > 0 {
                completed_results.enqueue(completed, now_ms);
                completed_results.dequeue(completed, now_ms);
            }
            completed_results.reconcile_depth(
                usize_to_u64(upload.queued_completed_compile_result_count),
                now_ms,
            );

            upload_work.reconcile_depth(
                usize_to_u64(upload.queued_upload_lifecycle_item_count),
                now_ms,
            );

            if !upload.host_mode.server_owned_lanes_are_remote() {
                let runner_depth = upload.server_pending_publications;
                let worldgen_depth = upload.poll_scheduler_pending_worldgen_publication_chunks;
                let light_depth = upload.poll_scheduler_pending_light_publications;
                let publication_depth = runner_depth
                    .saturating_add(worldgen_depth)
                    .saturating_add(light_depth);
                host_publication.reconcile_depth(usize_to_u64(publication_depth), now_ms);
                host_publication_runner.reconcile_depth(usize_to_u64(runner_depth), now_ms);
                host_publication_worldgen.reconcile_depth(usize_to_u64(worldgen_depth), now_ms);
                host_publication_light.reconcile_depth(usize_to_u64(light_depth), now_ms);
            }
            render_compile_jobs
                .reconcile_depth(usize_to_u64(upload.pending_compile_jobs_after), now_ms);
        }

        let latest_host_mode = details
            .iter()
            .rev()
            .find_map(|detail| detail.summary.map(|summary| summary.upload.host_mode))
            .unwrap_or_default();
        let host_publication = if latest_host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublication)
        } else {
            host_publication.report(now_ms)
        };
        let host_publication_runner = if latest_host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublicationRunner)
        } else {
            host_publication_runner.report(now_ms)
        };
        let host_publication_worldgen = if latest_host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublicationWorldgen)
        } else {
            host_publication_worldgen.report(now_ms)
        };
        let host_publication_light = if latest_host_mode.server_owned_lanes_are_remote() {
            remote_host_queue_report(QueueId::HostPublicationLight)
        } else {
            host_publication_light.report(now_ms)
        };

        QueuePanelReport::new(vec![
            inbound_updates.report(now_ms),
            completed_results.report(now_ms),
            upload_work.report(now_ms),
            host_publication,
            host_publication_runner,
            host_publication_worldgen,
            host_publication_light,
            render_compile_jobs.report(now_ms),
        ])
    }

    fn android_xr_peer_thread_panel(
        details: &[AndroidXrWorstFrameSnapshot],
    ) -> PeerThreadPanelReport {
        let latest_upload = details
            .iter()
            .rev()
            .find_map(|detail| detail.summary.map(|summary| summary.upload))
            .unwrap_or_default();
        let server_busy_ms = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.poll_server_reported_total_ms)
            .sum::<f64>();
        let max_server_pending_jobs = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.server_pending_jobs)
            .max()
            .unwrap_or(0);
        let max_worldgen_pending_jobs = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.poll_scheduler_worldgen_mailbox_pending_jobs)
            .max()
            .unwrap_or(0);
        let max_light_pending_jobs = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.poll_scheduler_light_mailbox_pending_statuses)
            .max()
            .unwrap_or(0);
        let max_render_compile_pending_jobs = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.pending_compile_jobs_after)
            .max()
            .unwrap_or(0);
        let submitted_compile_sections = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.submitted_compile_section_count)
            .sum::<usize>();
        let completed_compile_sections = details
            .iter()
            .filter_map(|detail| detail.summary)
            .map(|summary| summary.upload.completed_compile_section_count)
            .sum::<usize>();
        let server_lanes_remote = latest_upload.host_mode.server_owned_lanes_are_remote();

        PeerThreadPanelReport::new(vec![
            server_runner_peer_report(server_lanes_remote, max_server_pending_jobs, server_busy_ms),
            worker_metrics_peer_report(
                PeerThreadId::Worldgen,
                server_lanes_remote,
                usize_to_u64(latest_upload.worldgen_job_frame_metrics.request_frames),
                usize_to_u64(latest_upload.worldgen_job_frame_metrics.response_frames),
                usize_to_u64(latest_upload.worldgen_job_frame_metrics.request_bytes),
                usize_to_u64(latest_upload.worldgen_job_frame_metrics.response_bytes),
                usize_to_u64(latest_upload.worldgen_job_frame_metrics.max_pending_frames),
                latest_upload.worldgen_job_frame_metrics.last_request_us,
                latest_upload.worldgen_job_frame_metrics.total_request_us,
                latest_upload.worldgen_job_frame_metrics.max_request_us,
                max_worldgen_pending_jobs,
            ),
            worker_metrics_peer_report(
                PeerThreadId::LightStatus,
                server_lanes_remote,
                usize_to_u64(latest_upload.light_status_job_frame_metrics.request_frames),
                usize_to_u64(latest_upload.light_status_job_frame_metrics.response_frames),
                usize_to_u64(latest_upload.light_status_job_frame_metrics.request_bytes),
                usize_to_u64(latest_upload.light_status_job_frame_metrics.response_bytes),
                usize_to_u64(
                    latest_upload
                        .light_status_job_frame_metrics
                        .max_pending_frames,
                ),
                latest_upload.light_status_job_frame_metrics.last_request_us,
                latest_upload
                    .light_status_job_frame_metrics
                    .total_request_us,
                latest_upload.light_status_job_frame_metrics.max_request_us,
                max_light_pending_jobs,
            ),
            PeerThreadActivityReport::new(PeerThreadId::RenderCompileWorkers)
                .with_pending_jobs(usize_to_u64(max_render_compile_pending_jobs))
                .with_frames(
                    usize_to_u64(submitted_compile_sections),
                    usize_to_u64(completed_compile_sections),
                )
                .with_request_timing_ms(
                    None,
                    latest_upload.dispatcher_total_compile_worker_busy_ms,
                    latest_upload.dispatcher_max_compile_worker_task_ms,
                )
                .with_busy_idle_ms(
                    Some(latest_upload.dispatcher_total_compile_worker_busy_ms),
                    None,
                ),
        ])
    }

    fn worker_metrics_peer_report(
        lane: PeerThreadId,
        server_lanes_remote: bool,
        request_frames: u64,
        response_frames: u64,
        request_bytes: u64,
        response_bytes: u64,
        max_pending_frames: u64,
        last_request_us: u128,
        total_request_us: u128,
        max_request_us: u128,
        pending_jobs: usize,
    ) -> PeerThreadActivityReport {
        let report = PeerThreadActivityReport::new(lane)
            .with_pending_jobs(usize_to_u64(pending_jobs))
            .with_frames(request_frames, response_frames)
            .with_bytes(request_bytes, response_bytes)
            .with_max_pending_frames(max_pending_frames)
            .with_request_timing_ms(
                (last_request_us > 0).then_some(micros_to_ms(last_request_us)),
                micros_to_ms(total_request_us),
                micros_to_ms(max_request_us),
            )
            .with_busy_idle_ms(Some(micros_to_ms(total_request_us)), None);
        if server_lanes_remote {
            report.with_availability(DiagnosticLaneAvailability::RemoteHost)
        } else {
            report
        }
    }

    fn remote_host_queue_report(queue: QueueId) -> QueueAgeReport {
        QueueAgeReport::snapshot(queue, 0, 0, 0, None, 0.0, 0)
            .with_availability(DiagnosticLaneAvailability::RemoteHost)
    }

    fn server_runner_peer_report(
        server_lanes_remote: bool,
        pending_jobs: usize,
        busy_ms: f64,
    ) -> PeerThreadActivityReport {
        let report = PeerThreadActivityReport::new(PeerThreadId::ServerRunner)
            .with_pending_jobs(usize_to_u64(pending_jobs))
            .with_busy_idle_ms(Some(busy_ms), None);
        if server_lanes_remote {
            report.with_availability(DiagnosticLaneAvailability::RemoteHost)
        } else {
            report
        }
    }

    fn log_android_xr_frame_pipeline_report(report: &FramePipelineReport) {
        log::info!(
            "MCLONE_ANDROID_XR_PERF_FRAME_PIPELINE schema_version={} frames={} queues={} peers={} stages={} budget_decisions={}",
            report.schema_version,
            report.frame_summary.frames,
            report.queue_panel.queues.len(),
            report.peer_thread_panel.peers.len(),
            report.stage_spans.len(),
            report.budget_decision_panel.decisions.len()
        );
        for queue in &report.queue_panel.queues {
            log::info!(
                "MCLONE_ANDROID_XR_PERF_QUEUE queue={} enqueued={} dequeued={} depth={} oldest_age_ms={} max_oldest_age_ms={:.3} conservation_violations={} availability={}",
                queue_id_label(&queue.queue),
                queue.enqueued_total,
                queue.dequeued_total,
                queue.depth,
                format_optional_f64(queue.oldest_age_ms),
                queue.max_oldest_age_ms,
                queue.conservation_violations,
                queue.availability.label()
            );
        }
        for peer in &report.peer_thread_panel.peers {
            log::info!(
                "MCLONE_ANDROID_XR_PERF_PEER lane={} active={} pending_jobs={} request_frames={} response_frames={} request_bytes={} response_bytes={} max_pending_frames={} busy_ms={} idle_ms={} last_request_ms={} total_request_ms={:.3} max_request_ms={:.3} conservation_violations={} availability={}",
                peer_thread_id_label(&peer.lane),
                peer.active,
                peer.pending_jobs,
                peer.request_frames,
                peer.response_frames,
                peer.request_bytes,
                peer.response_bytes,
                peer.max_pending_frames,
                format_optional_f64(peer.busy_ms),
                format_optional_f64(peer.idle_ms),
                format_optional_f64(peer.last_request_ms),
                peer.total_request_ms,
                peer.max_request_ms,
                peer.conservation_violations,
                peer.availability.label()
            );
        }
        for span in &report.stage_spans {
            log::info!(
                "MCLONE_ANDROID_XR_PERF_STAGE stage={} label={} elapsed_ms={:.3} thread_cpu_ms={}",
                stage_id_label(span.stage),
                critical_path_label(span.label),
                span.elapsed_ms,
                format_optional_f64(span.thread_cpu_ms)
            );
        }
        for decision in &report.budget_decision_panel.decisions {
            log::info!(
                "MCLONE_ANDROID_XR_PERF_BUDGET_DECISION family={} host={} window={} stage={} reason={} grant_ms={:.3} min_units={} max_units={} max_pending_units={} target_period_ms={:.3} queue_depth={} oldest_queue_age_ms={} per_unit_cost_ms={} previous_max_units={} clean_windows={}",
                budget_decision_family_label(decision.family),
                frame_host_kind_label(decision.address.host_kind),
                work_window_label(decision.address.work_window),
                stage_id_label(decision.address.stage),
                budget_decision_reason_label(decision.trace.reason),
                decision.grant.elapsed_ms,
                decision.grant.min_units,
                decision.grant.max_units,
                format_optional_u32(decision.grant.max_pending_units),
                decision.trace.input_snapshot.target_period_ms,
                decision.trace.input_snapshot.queue_depth,
                format_optional_f64(decision.trace.input_snapshot.oldest_queue_age_ms),
                format_optional_f64(decision.trace.input_snapshot.per_unit_cost_ms),
                decision.trace.previous_max_units,
                decision.trace.consecutive_clean_windows
            );
        }
    }

    fn perf_budget_ms(target_hz: f64) -> f64 {
        1000.0 / target_hz.max(1.0)
    }

    fn per_second_u64(count: u64, seconds: f64) -> f64 {
        if seconds > 0.0 {
            count as f64 / seconds
        } else {
            0.0
        }
    }

    fn format_optional_hz(rate: Option<f32>) -> String {
        rate.map(|hz| format!("{hz:.1}"))
            .unwrap_or_else(|| "unknown".to_owned())
    }

    fn format_optional_usize(value: Option<usize>) -> String {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_owned())
    }

    fn format_optional_f64(value: Option<f64>) -> String {
        value
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "none".to_owned())
    }

    fn format_optional_u32(value: Option<u32>) -> String {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_owned())
    }

    fn usize_to_u64(value: usize) -> u64 {
        value.try_into().unwrap_or(u64::MAX)
    }

    fn micros_to_ms(value: u128) -> f64 {
        value as f64 / 1000.0
    }

    fn critical_path_label(label: CriticalPathLabel) -> &'static str {
        match label {
            CriticalPathLabel::CurrentFrameCritical => "current-frame-critical",
            CriticalPathLabel::NextFrameSlack => "next-frame-slack",
            CriticalPathLabel::ParallelCpuPeer => "parallel-cpu-peer",
            CriticalPathLabel::RemoteHostWork => "remote-host-work",
            CriticalPathLabel::QueuedBackpressured => "queued-backpressured",
        }
    }

    fn budget_decision_family_label(family: BudgetDecisionFamily) -> &'static str {
        match family {
            BudgetDecisionFamily::FeaturePublication => "feature-publication",
            BudgetDecisionFamily::LightPublication => "light-publication",
            BudgetDecisionFamily::FeatureJobAdmission => "feature-job-admission",
            BudgetDecisionFamily::RenderAdmission => "render-admission",
            BudgetDecisionFamily::CompletedResultAcceptance => "completed-result-acceptance",
            BudgetDecisionFamily::SectionUpload => "section-upload",
            BudgetDecisionFamily::RenderCompileWorkers => "render-compile-workers",
        }
    }

    fn budget_decision_reason_label(reason: BudgetDecisionReason) -> &'static str {
        match reason {
            BudgetDecisionReason::ColdStartFloor => "cold-start-floor",
            BudgetDecisionReason::MissingInputFloor => "missing-input-floor",
            BudgetDecisionReason::TargetPeriodChangedFloor => "target-period-changed-floor",
            BudgetDecisionReason::CutFrameMiss => "cut-frame-miss",
            BudgetDecisionReason::CutNegativeHeadroom => "cut-negative-headroom",
            BudgetDecisionReason::CutQueueAge => "cut-queue-age",
            BudgetDecisionReason::RaiseSustainedHeadroom => "raise-sustained-headroom",
            BudgetDecisionReason::HoldHysteresis => "hold-hysteresis",
            BudgetDecisionReason::HoldAtCap => "hold-at-cap",
        }
    }

    fn frame_host_kind_label(host: FrameHostKind) -> &'static str {
        match host {
            FrameHostKind::DesktopFlatWinit => "desktop-flat-winit",
            FrameHostKind::FlatAndroidWinit => "flat-android-winit",
            FrameHostKind::WebRafWorkers => "web-raf-workers",
            FrameHostKind::AndroidXrOpenXr => "android-xr-open-xr",
            FrameHostKind::DesktopXrOpenXr => "desktop-xr-open-xr",
            FrameHostKind::HeadlessOffscreenPerf => "headless-offscreen-perf",
            FrameHostKind::IntegratedServerRunner => "integrated-server-runner",
            FrameHostKind::DedicatedServerCommandLoop => "dedicated-server-command-loop",
        }
    }

    fn work_window_label(window: WorkWindow) -> &'static str {
        match window {
            WorkWindow::BeforeRender => "before-render",
            WorkWindow::PostSubmitOverlapSlack => "post-submit-overlap-slack",
            WorkWindow::GameplayTick => "gameplay-tick",
            WorkWindow::TickSlack => "tick-slack",
            WorkWindow::WorkerPoll => "worker-poll",
            WorkWindow::OffscreenStep => "offscreen-step",
            WorkWindow::CommandTick => "command-tick",
        }
    }

    fn stage_id_label(stage: StageId) -> &'static str {
        match stage {
            StageId::InputPoseEvents => "input-pose-events",
            StageId::HostSessionCommands => "host-session-commands",
            StageId::TerrainGeneration => "terrain-generation",
            StageId::LightComputeStatus => "light-compute-status",
            StageId::SchedulerPublication => "scheduler-publication",
            StageId::TransportDecode => "transport-decode",
            StageId::ClientUpdateApply => "client-update-apply",
            StageId::RenderSectionAdmission => "render-section-admission",
            StageId::RenderAdmissionDirtyReadyScan => "render-admission-dirty-ready-scan",
            StageId::RenderAdmissionRequestBuild => "render-admission-request-build",
            StageId::RenderAdmissionWorkerSubmit => "render-admission-worker-submit",
            StageId::RenderAdmissionPreparedRecordMaintenance => {
                "render-admission-prepared-record-maintenance"
            }
            StageId::CpuMeshCompile => "cpu-mesh-compile",
            StageId::CompletedResultAcceptance => "completed-result-acceptance",
            StageId::GpuUpload => "gpu-upload",
            StageId::UploadApply => "upload-apply",
            StageId::PreparedDrawRecords => "prepared-draw-records",
            StageId::DrawEncode => "draw-encode",
            StageId::GpuExecutionPresentationWait => "gpu-execution-presentation-wait",
            StageId::UiDebug => "ui-debug",
        }
    }

    fn queue_id_label(queue: &QueueId) -> &str {
        match queue {
            QueueId::InboundUpdates => "inbound-updates",
            QueueId::CompletedRenderResults => "completed-render-results",
            QueueId::UploadWork => "upload-work",
            QueueId::HostPublication => "host-publication",
            QueueId::HostPublicationRunner => "host-publication-runner",
            QueueId::HostPublicationWorldgen => "host-publication-worldgen",
            QueueId::HostPublicationLight => "host-publication-light",
            QueueId::RenderCompileJobs => "render-compile-jobs",
            QueueId::Custom(label) => label.as_str(),
        }
    }

    fn peer_thread_id_label(peer: &PeerThreadId) -> &str {
        match peer {
            PeerThreadId::ServerRunner => "server-runner",
            PeerThreadId::Worldgen => "worldgen",
            PeerThreadId::LightStatus => "light-status",
            PeerThreadId::RenderCompileWorkers => "render-compile-workers",
            PeerThreadId::Custom(label) => label.as_str(),
        }
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

    fn max_record_prepare_stats(
        a: TexturedSectionRecordPrepareStats,
        b: TexturedSectionRecordPrepareStats,
    ) -> TexturedSectionRecordPrepareStats {
        TexturedSectionRecordPrepareStats {
            rebuilt: a.rebuilt || b.rebuilt,
            rebuild_ms: a.rebuild_ms.max(b.rebuild_ms),
            dirty_causes: a.dirty_causes.with(b.dirty_causes),
            visibility_section_count: a.visibility_section_count.max(b.visibility_section_count),
            loaded_section_count: a.loaded_section_count.max(b.loaded_section_count),
            ready_section_count: a.ready_section_count.max(b.ready_section_count),
            loaded_index_count: a.loaded_index_count.max(b.loaded_index_count),
            cache: max_record_cache_stats(a.cache, b.cache),
        }
    }

    fn max_record_cache_stats(
        a: TexturedSectionRecordCacheStats,
        b: TexturedSectionRecordCacheStats,
    ) -> TexturedSectionRecordCacheStats {
        TexturedSectionRecordCacheStats {
            ready_set_calls: a.ready_set_calls.max(b.ready_set_calls),
            ready_set_changed_calls: a.ready_set_changed_calls.max(b.ready_set_changed_calls),
            ready_set_unchanged_calls: a.ready_set_unchanged_calls.max(b.ready_set_unchanged_calls),
            ready_set_upload_backpressured_calls: a
                .ready_set_upload_backpressured_calls
                .max(b.ready_set_upload_backpressured_calls),
            ready_set_upload_backpressured_changed_calls: a
                .ready_set_upload_backpressured_changed_calls
                .max(b.ready_set_upload_backpressured_changed_calls),
            ready_set_upload_backpressured_unchanged_calls: a
                .ready_set_upload_backpressured_unchanged_calls
                .max(b.ready_set_upload_backpressured_unchanged_calls),
            ready_set_skipped_calls: a.ready_set_skipped_calls.max(b.ready_set_skipped_calls),
            ready_set_upload_backpressured_skipped_calls: a
                .ready_set_upload_backpressured_skipped_calls
                .max(b.ready_set_upload_backpressured_skipped_calls),
            prepared_record_rebuilds: a.prepared_record_rebuilds.max(b.prepared_record_rebuilds),
            prepared_record_rebuild_total_ms: a
                .prepared_record_rebuild_total_ms
                .max(b.prepared_record_rebuild_total_ms),
            prepared_record_rebuild_max_ms: a
                .prepared_record_rebuild_max_ms
                .max(b.prepared_record_rebuild_max_ms),
            prepared_record_rebuild_initial_dirty: a
                .prepared_record_rebuild_initial_dirty
                .max(b.prepared_record_rebuild_initial_dirty),
            prepared_record_rebuild_section_upload_dirty: a
                .prepared_record_rebuild_section_upload_dirty
                .max(b.prepared_record_rebuild_section_upload_dirty),
            prepared_record_rebuild_section_remove_dirty: a
                .prepared_record_rebuild_section_remove_dirty
                .max(b.prepared_record_rebuild_section_remove_dirty),
            prepared_record_rebuild_ready_set_dirty: a
                .prepared_record_rebuild_ready_set_dirty
                .max(b.prepared_record_rebuild_ready_set_dirty),
            prepared_record_rebuild_upload_backpressured_dirty: a
                .prepared_record_rebuild_upload_backpressured_dirty
                .max(b.prepared_record_rebuild_upload_backpressured_dirty),
            prepared_record_rebuild_multi_dirty: a
                .prepared_record_rebuild_multi_dirty
                .max(b.prepared_record_rebuild_multi_dirty),
            prepared_record_rebuild_visibility_section_max: a
                .prepared_record_rebuild_visibility_section_max
                .max(b.prepared_record_rebuild_visibility_section_max),
            prepared_record_rebuild_loaded_section_max: a
                .prepared_record_rebuild_loaded_section_max
                .max(b.prepared_record_rebuild_loaded_section_max),
            prepared_record_rebuild_ready_section_max: a
                .prepared_record_rebuild_ready_section_max
                .max(b.prepared_record_rebuild_ready_section_max),
            prepared_record_rebuild_loaded_index_max: a
                .prepared_record_rebuild_loaded_index_max
                .max(b.prepared_record_rebuild_loaded_index_max),
        }
    }

    fn max_render_timing(
        a: AndroidXrRenderFrameTiming,
        b: AndroidXrRenderFrameTiming,
    ) -> AndroidXrRenderFrameTiming {
        AndroidXrRenderFrameTiming {
            locate_views_ms: a.locate_views_ms.max(b.locate_views_ms),
            locomotion_ms: a.locomotion_ms.max(b.locomotion_ms),
            locomotion_input_ms: a.locomotion_input_ms.max(b.locomotion_input_ms),
            locomotion_camera_apply_ms: a
                .locomotion_camera_apply_ms
                .max(b.locomotion_camera_apply_ms),
            locomotion_commit_ms: a.locomotion_commit_ms.max(b.locomotion_commit_ms),
            locomotion_commit_server_command_ms: a
                .locomotion_commit_server_command_ms
                .max(b.locomotion_commit_server_command_ms),
            locomotion_commit_server_command_send_ms: a
                .locomotion_commit_server_command_send_ms
                .max(b.locomotion_commit_server_command_send_ms),
            locomotion_commit_server_command_drain_updates_ms: a
                .locomotion_commit_server_command_drain_updates_ms
                .max(b.locomotion_commit_server_command_drain_updates_ms),
            locomotion_commit_server_command_apply_updates_ms: a
                .locomotion_commit_server_command_apply_updates_ms
                .max(b.locomotion_commit_server_command_apply_updates_ms),
            locomotion_commit_server_command_apply_dirty_mark_ms: a
                .locomotion_commit_server_command_apply_dirty_mark_ms
                .max(b.locomotion_commit_server_command_apply_dirty_mark_ms),
            locomotion_commit_server_command_apply_client_updates_ms: a
                .locomotion_commit_server_command_apply_client_updates_ms
                .max(b.locomotion_commit_server_command_apply_client_updates_ms),
            locomotion_commit_server_command_updates: a
                .locomotion_commit_server_command_updates
                .max(b.locomotion_commit_server_command_updates),
            locomotion_commit_server_command_snapshot_updates: a
                .locomotion_commit_server_command_snapshot_updates
                .max(b.locomotion_commit_server_command_snapshot_updates),
            locomotion_commit_server_command_section_block_updates: a
                .locomotion_commit_server_command_section_block_updates
                .max(b.locomotion_commit_server_command_section_block_updates),
            locomotion_commit_server_command_unload_updates: a
                .locomotion_commit_server_command_unload_updates
                .max(b.locomotion_commit_server_command_unload_updates),
            locomotion_commit_position_updates_ms: a
                .locomotion_commit_position_updates_ms
                .max(b.locomotion_commit_position_updates_ms),
            locomotion_commit_interest_ms: a
                .locomotion_commit_interest_ms
                .max(b.locomotion_commit_interest_ms),
            locomotion_commit_interest_command_send_ms: a
                .locomotion_commit_interest_command_send_ms
                .max(b.locomotion_commit_interest_command_send_ms),
            locomotion_commit_interest_command_drain_updates_ms: a
                .locomotion_commit_interest_command_drain_updates_ms
                .max(b.locomotion_commit_interest_command_drain_updates_ms),
            locomotion_commit_interest_command_apply_updates_ms: a
                .locomotion_commit_interest_command_apply_updates_ms
                .max(b.locomotion_commit_interest_command_apply_updates_ms),
            locomotion_commit_interest_command_apply_dirty_mark_ms: a
                .locomotion_commit_interest_command_apply_dirty_mark_ms
                .max(b.locomotion_commit_interest_command_apply_dirty_mark_ms),
            locomotion_commit_interest_command_apply_client_updates_ms: a
                .locomotion_commit_interest_command_apply_client_updates_ms
                .max(b.locomotion_commit_interest_command_apply_client_updates_ms),
            locomotion_commit_interest_command_updates: a
                .locomotion_commit_interest_command_updates
                .max(b.locomotion_commit_interest_command_updates),
            locomotion_commit_interest_command_snapshot_updates: a
                .locomotion_commit_interest_command_snapshot_updates
                .max(b.locomotion_commit_interest_command_snapshot_updates),
            locomotion_commit_interest_command_section_block_updates: a
                .locomotion_commit_interest_command_section_block_updates
                .max(b.locomotion_commit_interest_command_section_block_updates),
            locomotion_commit_interest_command_unload_updates: a
                .locomotion_commit_interest_command_unload_updates
                .max(b.locomotion_commit_interest_command_unload_updates),
            locomotion_gameplay_interaction_ms: a
                .locomotion_gameplay_interaction_ms
                .max(b.locomotion_gameplay_interaction_ms),
            acquire_left_ms: a.acquire_left_ms.max(b.acquire_left_ms),
            acquire_right_ms: a.acquire_right_ms.max(b.acquire_right_ms),
            terrain_render_frame_ms: a.terrain_render_frame_ms.max(b.terrain_render_frame_ms),
            terrain_render_views_ms: a.terrain_render_views_ms.max(b.terrain_render_views_ms),
            terrain_menu_pointer_ms: a.terrain_menu_pointer_ms.max(b.terrain_menu_pointer_ms),
            terrain_runtime_upload_ms: a.terrain_runtime_upload_ms.max(b.terrain_runtime_upload_ms),
            terrain_runtime_poll_ms: a.terrain_runtime_poll_ms.max(b.terrain_runtime_poll_ms),
            terrain_runtime_sync_ms: a.terrain_runtime_sync_ms.max(b.terrain_runtime_sync_ms),
            terrain_runtime_sync_unattributed_ms: a
                .terrain_runtime_sync_unattributed_ms
                .max(b.terrain_runtime_sync_unattributed_ms),
            terrain_runtime_result_accept_ms: a
                .terrain_runtime_result_accept_ms
                .max(b.terrain_runtime_result_accept_ms),
            terrain_runtime_dirty_seed_ms: a
                .terrain_runtime_dirty_seed_ms
                .max(b.terrain_runtime_dirty_seed_ms),
            terrain_runtime_prepare_ms: a
                .terrain_runtime_prepare_ms
                .max(b.terrain_runtime_prepare_ms),
            terrain_runtime_submit_ms: a.terrain_runtime_submit_ms.max(b.terrain_runtime_submit_ms),
            terrain_runtime_submit_snapshot_ms: a
                .terrain_runtime_submit_snapshot_ms
                .max(b.terrain_runtime_submit_snapshot_ms),
            terrain_runtime_submit_handoff_ms: a
                .terrain_runtime_submit_handoff_ms
                .max(b.terrain_runtime_submit_handoff_ms),
            terrain_runtime_submit_handoff_worst_ms: a
                .terrain_runtime_submit_handoff_worst_ms
                .max(b.terrain_runtime_submit_handoff_worst_ms),
            terrain_runtime_submit_request_count: a
                .terrain_runtime_submit_request_count
                .max(b.terrain_runtime_submit_request_count),
            terrain_runtime_submit_request_build_ms: a
                .terrain_runtime_submit_request_build_ms
                .max(b.terrain_runtime_submit_request_build_ms),
            terrain_runtime_submit_compiler_ms: a
                .terrain_runtime_submit_compiler_ms
                .max(b.terrain_runtime_submit_compiler_ms),
            terrain_runtime_submit_compiler_worst_ms: a
                .terrain_runtime_submit_compiler_worst_ms
                .max(b.terrain_runtime_submit_compiler_worst_ms),
            terrain_runtime_submit_compiler_capacity_check_ms: a
                .terrain_runtime_submit_compiler_capacity_check_ms
                .max(b.terrain_runtime_submit_compiler_capacity_check_ms),
            terrain_runtime_submit_compiler_capacity_check_worst_ms: a
                .terrain_runtime_submit_compiler_capacity_check_worst_ms
                .max(b.terrain_runtime_submit_compiler_capacity_check_worst_ms),
            terrain_runtime_submit_compiler_command_send_ms: a
                .terrain_runtime_submit_compiler_command_send_ms
                .max(b.terrain_runtime_submit_compiler_command_send_ms),
            terrain_runtime_submit_compiler_command_send_worst_ms: a
                .terrain_runtime_submit_compiler_command_send_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_send_worst_ms),
            terrain_runtime_submit_compiler_command_lock_wait_ms: a
                .terrain_runtime_submit_compiler_command_lock_wait_ms
                .max(b.terrain_runtime_submit_compiler_command_lock_wait_ms),
            terrain_runtime_submit_compiler_command_lock_wait_worst_ms: a
                .terrain_runtime_submit_compiler_command_lock_wait_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_lock_wait_worst_ms),
            terrain_runtime_submit_compiler_command_slot_select_ms: a
                .terrain_runtime_submit_compiler_command_slot_select_ms
                .max(b.terrain_runtime_submit_compiler_command_slot_select_ms),
            terrain_runtime_submit_compiler_command_slot_select_worst_ms: a
                .terrain_runtime_submit_compiler_command_slot_select_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_slot_select_worst_ms),
            terrain_runtime_submit_compiler_command_slot_write_ms: a
                .terrain_runtime_submit_compiler_command_slot_write_ms
                .max(b.terrain_runtime_submit_compiler_command_slot_write_ms),
            terrain_runtime_submit_compiler_command_slot_write_worst_ms: a
                .terrain_runtime_submit_compiler_command_slot_write_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_slot_write_worst_ms),
            terrain_runtime_submit_compiler_command_queue_push_ms: a
                .terrain_runtime_submit_compiler_command_queue_push_ms
                .max(b.terrain_runtime_submit_compiler_command_queue_push_ms),
            terrain_runtime_submit_compiler_command_queue_push_worst_ms: a
                .terrain_runtime_submit_compiler_command_queue_push_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_queue_push_worst_ms),
            terrain_runtime_submit_compiler_command_notify_ms: a
                .terrain_runtime_submit_compiler_command_notify_ms
                .max(b.terrain_runtime_submit_compiler_command_notify_ms),
            terrain_runtime_submit_compiler_command_notify_worst_ms: a
                .terrain_runtime_submit_compiler_command_notify_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_notify_worst_ms),
            terrain_runtime_submit_compiler_command_post_enqueue_ms: a
                .terrain_runtime_submit_compiler_command_post_enqueue_ms
                .max(b.terrain_runtime_submit_compiler_command_post_enqueue_ms),
            terrain_runtime_submit_compiler_command_post_enqueue_worst_ms: a
                .terrain_runtime_submit_compiler_command_post_enqueue_worst_ms
                .max(b.terrain_runtime_submit_compiler_command_post_enqueue_worst_ms),
            terrain_runtime_submit_compiler_pending_mark_ms: a
                .terrain_runtime_submit_compiler_pending_mark_ms
                .max(b.terrain_runtime_submit_compiler_pending_mark_ms),
            terrain_runtime_submit_compiler_pending_mark_worst_ms: a
                .terrain_runtime_submit_compiler_pending_mark_worst_ms
                .max(b.terrain_runtime_submit_compiler_pending_mark_worst_ms),
            terrain_runtime_submit_mark_inflight_ms: a
                .terrain_runtime_submit_mark_inflight_ms
                .max(b.terrain_runtime_submit_mark_inflight_ms),
            terrain_runtime_submit_apply_ready_plan_ms: a
                .terrain_runtime_submit_apply_ready_plan_ms
                .max(b.terrain_runtime_submit_apply_ready_plan_ms),
            terrain_runtime_submit_ready_update_ms: a
                .terrain_runtime_submit_ready_update_ms
                .max(b.terrain_runtime_submit_ready_update_ms),
            terrain_runtime_submit_ready_section_count: a
                .terrain_runtime_submit_ready_section_count
                .max(b.terrain_runtime_submit_ready_section_count),
            terrain_runtime_submit_deferred_section_count: a
                .terrain_runtime_submit_deferred_section_count
                .max(b.terrain_runtime_submit_deferred_section_count),
            terrain_runtime_submit_dirty_chunk_count_before: a
                .terrain_runtime_submit_dirty_chunk_count_before
                .max(b.terrain_runtime_submit_dirty_chunk_count_before),
            terrain_runtime_submit_dirty_chunk_count_after: a
                .terrain_runtime_submit_dirty_chunk_count_after
                .max(b.terrain_runtime_submit_dirty_chunk_count_after),
            terrain_runtime_submit_dirty_section_count_before: a
                .terrain_runtime_submit_dirty_section_count_before
                .max(b.terrain_runtime_submit_dirty_section_count_before),
            terrain_runtime_submit_dirty_section_count_after: a
                .terrain_runtime_submit_dirty_section_count_after
                .max(b.terrain_runtime_submit_dirty_section_count_after),
            terrain_runtime_submit_inflight_section_count_before: a
                .terrain_runtime_submit_inflight_section_count_before
                .max(b.terrain_runtime_submit_inflight_section_count_before),
            terrain_runtime_submit_inflight_section_count_after: a
                .terrain_runtime_submit_inflight_section_count_after
                .max(b.terrain_runtime_submit_inflight_section_count_after),
            terrain_runtime_submit_request_target_section_count: a
                .terrain_runtime_submit_request_target_section_count
                .max(b.terrain_runtime_submit_request_target_section_count),
            terrain_runtime_submit_request_target_section_count_worst: a
                .terrain_runtime_submit_request_target_section_count_worst
                .max(b.terrain_runtime_submit_request_target_section_count_worst),
            terrain_runtime_submit_request_snapshot_count: a
                .terrain_runtime_submit_request_snapshot_count
                .max(b.terrain_runtime_submit_request_snapshot_count),
            terrain_runtime_submit_request_snapshot_section_count: a
                .terrain_runtime_submit_request_snapshot_section_count
                .max(b.terrain_runtime_submit_request_snapshot_section_count),
            terrain_runtime_submit_request_snapshot_section_count_worst: a
                .terrain_runtime_submit_request_snapshot_section_count_worst
                .max(b.terrain_runtime_submit_request_snapshot_section_count_worst),
            terrain_runtime_submit_request_light_section_count: a
                .terrain_runtime_submit_request_light_section_count
                .max(b.terrain_runtime_submit_request_light_section_count),
            terrain_runtime_submit_request_light_section_count_worst: a
                .terrain_runtime_submit_request_light_section_count_worst
                .max(b.terrain_runtime_submit_request_light_section_count_worst),
            terrain_runtime_submit_request_revision_count: a
                .terrain_runtime_submit_request_revision_count
                .max(b.terrain_runtime_submit_request_revision_count),
            terrain_runtime_submit_request_estimated_payload_bytes: a
                .terrain_runtime_submit_request_estimated_payload_bytes
                .max(b.terrain_runtime_submit_request_estimated_payload_bytes),
            terrain_runtime_submit_request_estimated_payload_bytes_worst: a
                .terrain_runtime_submit_request_estimated_payload_bytes_worst
                .max(b.terrain_runtime_submit_request_estimated_payload_bytes_worst),
            terrain_runtime_dispatcher_pending_jobs: a
                .terrain_runtime_dispatcher_pending_jobs
                .max(b.terrain_runtime_dispatcher_pending_jobs),
            terrain_runtime_dispatcher_max_pending_jobs: a
                .terrain_runtime_dispatcher_max_pending_jobs
                .max(b.terrain_runtime_dispatcher_max_pending_jobs),
            terrain_runtime_dispatcher_available_job_slots: a
                .terrain_runtime_dispatcher_available_job_slots
                .max(b.terrain_runtime_dispatcher_available_job_slots),
            terrain_runtime_dispatcher_queued_compile_tasks: a
                .terrain_runtime_dispatcher_queued_compile_tasks
                .max(b.terrain_runtime_dispatcher_queued_compile_tasks),
            terrain_runtime_gpu_upload_ms: a
                .terrain_runtime_gpu_upload_ms
                .max(b.terrain_runtime_gpu_upload_ms),
            terrain_runtime_gpu_upload_pre_sync_ms: a
                .terrain_runtime_gpu_upload_pre_sync_ms
                .max(b.terrain_runtime_gpu_upload_pre_sync_ms),
            terrain_runtime_gpu_upload_post_sync_ms: a
                .terrain_runtime_gpu_upload_post_sync_ms
                .max(b.terrain_runtime_gpu_upload_post_sync_ms),
            terrain_runtime_upload_enqueue_ms: a
                .terrain_runtime_upload_enqueue_ms
                .max(b.terrain_runtime_upload_enqueue_ms),
            terrain_runtime_upload_select_ms: a
                .terrain_runtime_upload_select_ms
                .max(b.terrain_runtime_upload_select_ms),
            terrain_runtime_upload_apply_ms: a
                .terrain_runtime_upload_apply_ms
                .max(b.terrain_runtime_upload_apply_ms),
            terrain_runtime_upload_apply_dirty_mark_ms: a
                .terrain_runtime_upload_apply_dirty_mark_ms
                .max(b.terrain_runtime_upload_apply_dirty_mark_ms),
            terrain_runtime_upload_apply_remove_ms: a
                .terrain_runtime_upload_apply_remove_ms
                .max(b.terrain_runtime_upload_apply_remove_ms),
            terrain_runtime_upload_apply_section_state_ms: a
                .terrain_runtime_upload_apply_section_state_ms
                .max(b.terrain_runtime_upload_apply_section_state_ms),
            terrain_runtime_upload_apply_vertex_bytes_ms: a
                .terrain_runtime_upload_apply_vertex_bytes_ms
                .max(b.terrain_runtime_upload_apply_vertex_bytes_ms),
            terrain_runtime_upload_apply_vertex_buffer_ms: a
                .terrain_runtime_upload_apply_vertex_buffer_ms
                .max(b.terrain_runtime_upload_apply_vertex_buffer_ms),
            terrain_runtime_upload_apply_index_bytes_ms: a
                .terrain_runtime_upload_apply_index_bytes_ms
                .max(b.terrain_runtime_upload_apply_index_bytes_ms),
            terrain_runtime_upload_apply_index_buffer_ms: a
                .terrain_runtime_upload_apply_index_buffer_ms
                .max(b.terrain_runtime_upload_apply_index_buffer_ms),
            terrain_runtime_upload_apply_mesh_insert_ms: a
                .terrain_runtime_upload_apply_mesh_insert_ms
                .max(b.terrain_runtime_upload_apply_mesh_insert_ms),
            terrain_runtime_upload_apply_mesh_upload_worst_ms: a
                .terrain_runtime_upload_apply_mesh_upload_worst_ms
                .max(b.terrain_runtime_upload_apply_mesh_upload_worst_ms),
            terrain_runtime_ready_sections_ms: a
                .terrain_runtime_ready_sections_ms
                .max(b.terrain_runtime_ready_sections_ms),
            terrain_runtime_ready_publish_ms: a
                .terrain_runtime_ready_publish_ms
                .max(b.terrain_runtime_ready_publish_ms),
            terrain_shared_records_ms: a.terrain_shared_records_ms.max(b.terrain_shared_records_ms),
            terrain_record_cache_prepare: max_record_prepare_stats(
                a.terrain_record_cache_prepare,
                b.terrain_record_cache_prepare,
            ),
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
            terrain_left_eye_full_frame_ms: a
                .terrain_left_eye_full_frame_ms
                .max(b.terrain_left_eye_full_frame_ms),
            terrain_left_eye_sky_ms: a.terrain_left_eye_sky_ms.max(b.terrain_left_eye_sky_ms),
            terrain_left_eye_far_lod_ms: a
                .terrain_left_eye_far_lod_ms
                .max(b.terrain_left_eye_far_lod_ms),
            terrain_left_eye_opaque_ms: a
                .terrain_left_eye_opaque_ms
                .max(b.terrain_left_eye_opaque_ms),
            terrain_left_eye_translucent_ms: a
                .terrain_left_eye_translucent_ms
                .max(b.terrain_left_eye_translucent_ms),
            terrain_left_eye_actor_ms: a.terrain_left_eye_actor_ms.max(b.terrain_left_eye_actor_ms),
            terrain_left_eye_screen_effect_ms: a
                .terrain_left_eye_screen_effect_ms
                .max(b.terrain_left_eye_screen_effect_ms),
            terrain_left_eye_gui_ms: a.terrain_left_eye_gui_ms.max(b.terrain_left_eye_gui_ms),
            terrain_left_eye_xr_fade_ms: a
                .terrain_left_eye_xr_fade_ms
                .max(b.terrain_left_eye_xr_fade_ms),
            terrain_left_eye_xr_selection_ms: a
                .terrain_left_eye_xr_selection_ms
                .max(b.terrain_left_eye_xr_selection_ms),
            terrain_left_eye_xr_world_lines_ms: a
                .terrain_left_eye_xr_world_lines_ms
                .max(b.terrain_left_eye_xr_world_lines_ms),
            terrain_left_eye_xr_world_panel_ms: a
                .terrain_left_eye_xr_world_panel_ms
                .max(b.terrain_left_eye_xr_world_panel_ms),
            terrain_left_eye_encoder_finish_ms: a
                .terrain_left_eye_encoder_finish_ms
                .max(b.terrain_left_eye_encoder_finish_ms),
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
            terrain_right_eye_full_frame_ms: a
                .terrain_right_eye_full_frame_ms
                .max(b.terrain_right_eye_full_frame_ms),
            terrain_right_eye_sky_ms: a.terrain_right_eye_sky_ms.max(b.terrain_right_eye_sky_ms),
            terrain_right_eye_far_lod_ms: a
                .terrain_right_eye_far_lod_ms
                .max(b.terrain_right_eye_far_lod_ms),
            terrain_right_eye_opaque_ms: a
                .terrain_right_eye_opaque_ms
                .max(b.terrain_right_eye_opaque_ms),
            terrain_right_eye_translucent_ms: a
                .terrain_right_eye_translucent_ms
                .max(b.terrain_right_eye_translucent_ms),
            terrain_right_eye_actor_ms: a
                .terrain_right_eye_actor_ms
                .max(b.terrain_right_eye_actor_ms),
            terrain_right_eye_screen_effect_ms: a
                .terrain_right_eye_screen_effect_ms
                .max(b.terrain_right_eye_screen_effect_ms),
            terrain_right_eye_gui_ms: a.terrain_right_eye_gui_ms.max(b.terrain_right_eye_gui_ms),
            terrain_right_eye_xr_fade_ms: a
                .terrain_right_eye_xr_fade_ms
                .max(b.terrain_right_eye_xr_fade_ms),
            terrain_right_eye_xr_selection_ms: a
                .terrain_right_eye_xr_selection_ms
                .max(b.terrain_right_eye_xr_selection_ms),
            terrain_right_eye_xr_world_lines_ms: a
                .terrain_right_eye_xr_world_lines_ms
                .max(b.terrain_right_eye_xr_world_lines_ms),
            terrain_right_eye_xr_world_panel_ms: a
                .terrain_right_eye_xr_world_panel_ms
                .max(b.terrain_right_eye_xr_world_panel_ms),
            terrain_right_eye_encoder_finish_ms: a
                .terrain_right_eye_encoder_finish_ms
                .max(b.terrain_right_eye_encoder_finish_ms),
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
            terrain_overlap_runtime_prefetch_ms: a
                .terrain_overlap_runtime_prefetch_ms
                .max(b.terrain_overlap_runtime_prefetch_ms),
            terrain_overlap_runtime_prefetch_poll_ms: a
                .terrain_overlap_runtime_prefetch_poll_ms
                .max(b.terrain_overlap_runtime_prefetch_poll_ms),
            terrain_overlap_runtime_prefetch_sync_ms: a
                .terrain_overlap_runtime_prefetch_sync_ms
                .max(b.terrain_overlap_runtime_prefetch_sync_ms),
            terrain_overlap_runtime_prefetch_gpu_upload_ms: a
                .terrain_overlap_runtime_prefetch_gpu_upload_ms
                .max(b.terrain_overlap_runtime_prefetch_gpu_upload_ms),
            terrain_overlap_runtime_prefetch_ready_sections_ms: a
                .terrain_overlap_runtime_prefetch_ready_sections_ms
                .max(b.terrain_overlap_runtime_prefetch_ready_sections_ms),
            terrain_multiview_sky_ms: a.terrain_multiview_sky_ms.max(b.terrain_multiview_sky_ms),
            terrain_multiview_terrain_ms: a
                .terrain_multiview_terrain_ms
                .max(b.terrain_multiview_terrain_ms),
            terrain_multiview_actor_ms: a
                .terrain_multiview_actor_ms
                .max(b.terrain_multiview_actor_ms),
            terrain_multiview_screen_effect_ms: a
                .terrain_multiview_screen_effect_ms
                .max(b.terrain_multiview_screen_effect_ms),
            terrain_multiview_world_overlays_ms: a
                .terrain_multiview_world_overlays_ms
                .max(b.terrain_multiview_world_overlays_ms),
            terrain_multiview_submit_ms: a
                .terrain_multiview_submit_ms
                .max(b.terrain_multiview_submit_ms),
            terrain_multiview_poll_wait_ms: a
                .terrain_multiview_poll_wait_ms
                .max(b.terrain_multiview_poll_wait_ms),
            release_eyes_ms: a.release_eyes_ms.max(b.release_eyes_ms),
            end_frame_ms: a.end_frame_ms.max(b.end_frame_ms),
        }
    }

    fn terrain_upload_summary_has_work(summary: mclone_xr_scene::XrTerrainUploadSummary) -> bool {
        summary.poll_changed
            || summary.rebuilt_section_count > 0
            || summary.removed_section_count > 0
            || summary.submitted_compile_section_count > 0
            || summary.accepted_compile_result_count > 0
            || summary.queued_completed_compile_result_count > 0
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
            && summary.ui_draw_cache.rebuild_count == 0
            && summary.ui_panel.repaint_count == 0
            && summary.ui_panel.texture_recreate_count == 0
    }

    fn max_upload_summary(
        a: mclone_xr_scene::XrTerrainUploadSummary,
        b: mclone_xr_scene::XrTerrainUploadSummary,
    ) -> mclone_xr_scene::XrTerrainUploadSummary {
        mclone_xr_scene::XrTerrainUploadSummary {
            host_mode: if b.host_mode.server_owned_lanes_are_remote() {
                b.host_mode
            } else {
                a.host_mode
            },
            poll_changed: a.poll_changed || b.poll_changed,
            poll_total_ms: a.poll_total_ms.max(b.poll_total_ms),
            poll_drain_updates_ms: a.poll_drain_updates_ms.max(b.poll_drain_updates_ms),
            poll_producer_read_ms: a.poll_producer_read_ms.max(b.poll_producer_read_ms),
            poll_producer_decode_ms: a.poll_producer_decode_ms.max(b.poll_producer_decode_ms),
            poll_producer_response_sequence: a
                .poll_producer_response_sequence
                .max(b.poll_producer_response_sequence),
            poll_client_deferred_chunk_drop_ms: a
                .poll_client_deferred_chunk_drop_ms
                .max(b.poll_client_deferred_chunk_drop_ms),
            poll_client_deferred_chunk_drop_items: a
                .poll_client_deferred_chunk_drop_items
                .max(b.poll_client_deferred_chunk_drop_items),
            poll_client_deferred_chunk_drop_backlog_items: a
                .poll_client_deferred_chunk_drop_backlog_items
                .max(b.poll_client_deferred_chunk_drop_backlog_items),
            poll_apply_updates_ms: a.poll_apply_updates_ms.max(b.poll_apply_updates_ms),
            poll_dirty_mark_ms: a.poll_dirty_mark_ms.max(b.poll_dirty_mark_ms),
            poll_client_apply_updates_ms: a
                .poll_client_apply_updates_ms
                .max(b.poll_client_apply_updates_ms),
            poll_snapshot_update_apply_ms: a
                .poll_snapshot_update_apply_ms
                .max(b.poll_snapshot_update_apply_ms),
            poll_snapshot_update_dirty_mark_ms: a
                .poll_snapshot_update_dirty_mark_ms
                .max(b.poll_snapshot_update_dirty_mark_ms),
            poll_snapshot_update_client_apply_ms: a
                .poll_snapshot_update_client_apply_ms
                .max(b.poll_snapshot_update_client_apply_ms),
            poll_section_block_update_apply_ms: a
                .poll_section_block_update_apply_ms
                .max(b.poll_section_block_update_apply_ms),
            poll_section_block_update_dirty_mark_ms: a
                .poll_section_block_update_dirty_mark_ms
                .max(b.poll_section_block_update_dirty_mark_ms),
            poll_section_block_update_client_apply_ms: a
                .poll_section_block_update_client_apply_ms
                .max(b.poll_section_block_update_client_apply_ms),
            poll_unload_update_apply_ms: a
                .poll_unload_update_apply_ms
                .max(b.poll_unload_update_apply_ms),
            poll_unload_update_dirty_mark_ms: a
                .poll_unload_update_dirty_mark_ms
                .max(b.poll_unload_update_dirty_mark_ms),
            poll_unload_update_client_apply_ms: a
                .poll_unload_update_client_apply_ms
                .max(b.poll_unload_update_client_apply_ms),
            poll_other_update_apply_ms: a
                .poll_other_update_apply_ms
                .max(b.poll_other_update_apply_ms),
            poll_other_update_dirty_mark_ms: a
                .poll_other_update_dirty_mark_ms
                .max(b.poll_other_update_dirty_mark_ms),
            poll_other_update_client_apply_ms: a
                .poll_other_update_client_apply_ms
                .max(b.poll_other_update_client_apply_ms),
            poll_mixed_update_apply_ms: a
                .poll_mixed_update_apply_ms
                .max(b.poll_mixed_update_apply_ms),
            poll_mixed_update_dirty_mark_ms: a
                .poll_mixed_update_dirty_mark_ms
                .max(b.poll_mixed_update_dirty_mark_ms),
            poll_mixed_update_client_apply_ms: a
                .poll_mixed_update_client_apply_ms
                .max(b.poll_mixed_update_client_apply_ms),
            update_pump_stalled: a.update_pump_stalled || b.update_pump_stalled,
            update_pump_stall_count: a.update_pump_stall_count.max(b.update_pump_stall_count),
            server_update_applied_bytes: a
                .server_update_applied_bytes
                .max(b.server_update_applied_bytes),
            server_update_oldest_applied_age_ms: a
                .server_update_oldest_applied_age_ms
                .max(b.server_update_oldest_applied_age_ms),
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
            poll_scheduler_completed_feature_jobs_drained: a
                .poll_scheduler_completed_feature_jobs_drained
                .max(b.poll_scheduler_completed_feature_jobs_drained),
            poll_scheduler_feature_chunks_published: a
                .poll_scheduler_feature_chunks_published
                .max(b.poll_scheduler_feature_chunks_published),
            poll_scheduler_feature_chunks_skipped: a
                .poll_scheduler_feature_chunks_skipped
                .max(b.poll_scheduler_feature_chunks_skipped),
            poll_scheduler_feature_jobs_completed: a
                .poll_scheduler_feature_jobs_completed
                .max(b.poll_scheduler_feature_jobs_completed),
            poll_scheduler_feature_snapshot_ready_events: a
                .poll_scheduler_feature_snapshot_ready_events
                .max(b.poll_scheduler_feature_snapshot_ready_events),
            poll_scheduler_light_status_batches_enqueued: a
                .poll_scheduler_light_status_batches_enqueued
                .max(b.poll_scheduler_light_status_batches_enqueued),
            poll_scheduler_completed_light_statuses_drained: a
                .poll_scheduler_completed_light_statuses_drained
                .max(b.poll_scheduler_completed_light_statuses_drained),
            poll_scheduler_light_statuses_published: a
                .poll_scheduler_light_statuses_published
                .max(b.poll_scheduler_light_statuses_published),
            poll_scheduler_light_statuses_skipped: a
                .poll_scheduler_light_statuses_skipped
                .max(b.poll_scheduler_light_statuses_skipped),
            poll_scheduler_light_snapshot_ready_events: a
                .poll_scheduler_light_snapshot_ready_events
                .max(b.poll_scheduler_light_snapshot_ready_events),
            poll_scheduler_cumulative_feature_chunks_published: a
                .poll_scheduler_cumulative_feature_chunks_published
                .max(b.poll_scheduler_cumulative_feature_chunks_published),
            poll_scheduler_cumulative_light_statuses_published: a
                .poll_scheduler_cumulative_light_statuses_published
                .max(b.poll_scheduler_cumulative_light_statuses_published),
            poll_scheduler_pending_worldgen_publication_jobs: a
                .poll_scheduler_pending_worldgen_publication_jobs
                .max(b.poll_scheduler_pending_worldgen_publication_jobs),
            poll_scheduler_pending_worldgen_publication_chunks: a
                .poll_scheduler_pending_worldgen_publication_chunks
                .max(b.poll_scheduler_pending_worldgen_publication_chunks),
            poll_scheduler_pending_light_publications: a
                .poll_scheduler_pending_light_publications
                .max(b.poll_scheduler_pending_light_publications),
            poll_scheduler_worldgen_mailbox_pending_jobs: a
                .poll_scheduler_worldgen_mailbox_pending_jobs
                .max(b.poll_scheduler_worldgen_mailbox_pending_jobs),
            poll_scheduler_light_mailbox_pending_statuses: a
                .poll_scheduler_light_mailbox_pending_statuses
                .max(b.poll_scheduler_light_mailbox_pending_statuses),
            poll_updates: a.poll_updates.max(b.poll_updates),
            poll_snapshot_updates: a.poll_snapshot_updates.max(b.poll_snapshot_updates),
            poll_section_block_updates: a
                .poll_section_block_updates
                .max(b.poll_section_block_updates),
            poll_unload_updates: a.poll_unload_updates.max(b.poll_unload_updates),
            poll_other_updates: a.poll_other_updates.max(b.poll_other_updates),
            poll_mixed_updates: a.poll_mixed_updates.max(b.poll_mixed_updates),
            server_command_queue_depth: a
                .server_command_queue_depth
                .max(b.server_command_queue_depth),
            server_update_queue_depth: a.server_update_queue_depth.max(b.server_update_queue_depth),
            server_update_queue_bytes: a.server_update_queue_bytes.max(b.server_update_queue_bytes),
            server_pending_jobs: a.server_pending_jobs.max(b.server_pending_jobs),
            server_pending_publications: a
                .server_pending_publications
                .max(b.server_pending_publications),
            runner_frame_metrics: if a.runner_frame_metrics.total_request_us
                >= b.runner_frame_metrics.total_request_us
            {
                a.runner_frame_metrics
            } else {
                b.runner_frame_metrics
            },
            worldgen_job_frame_metrics: if a.worldgen_job_frame_metrics.total_request_us
                >= b.worldgen_job_frame_metrics.total_request_us
            {
                a.worldgen_job_frame_metrics
            } else {
                b.worldgen_job_frame_metrics
            },
            light_status_job_frame_metrics: if a.light_status_job_frame_metrics.total_request_us
                >= b.light_status_job_frame_metrics.total_request_us
            {
                a.light_status_job_frame_metrics
            } else {
                b.light_status_job_frame_metrics
            },
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
            max_pending_compile_jobs: a.max_pending_compile_jobs.max(b.max_pending_compile_jobs),
            available_compile_slots_before: a
                .available_compile_slots_before
                .max(b.available_compile_slots_before),
            available_compile_slots_after: a
                .available_compile_slots_after
                .max(b.available_compile_slots_after),
            dispatcher_compile_worker_count: a
                .dispatcher_compile_worker_count
                .max(b.dispatcher_compile_worker_count),
            dispatcher_completed_compile_tasks: a
                .dispatcher_completed_compile_tasks
                .max(b.dispatcher_completed_compile_tasks),
            dispatcher_total_compile_worker_busy_ms: a
                .dispatcher_total_compile_worker_busy_ms
                .max(b.dispatcher_total_compile_worker_busy_ms),
            dispatcher_max_compile_worker_task_ms: a
                .dispatcher_max_compile_worker_task_ms
                .max(b.dispatcher_max_compile_worker_task_ms),
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
            deadline_skipped_compile_request_count: a
                .deadline_skipped_compile_request_count
                .max(b.deadline_skipped_compile_request_count),
            accepted_compile_result_count: a
                .accepted_compile_result_count
                .max(b.accepted_compile_result_count),
            queued_completed_compile_result_count: a
                .queued_completed_compile_result_count
                .max(b.queued_completed_compile_result_count),
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
            queued_upload_section_count: a
                .queued_upload_section_count
                .max(b.queued_upload_section_count),
            queued_upload_removed_section_count: a
                .queued_upload_removed_section_count
                .max(b.queued_upload_removed_section_count),
            queued_upload_lifecycle_item_count: a
                .queued_upload_lifecycle_item_count
                .max(b.queued_upload_lifecycle_item_count),
            upload_phase_event_count: a.upload_phase_event_count.max(b.upload_phase_event_count),
            upload_enqueued_lifecycle_item_count: a
                .upload_enqueued_lifecycle_item_count
                .max(b.upload_enqueued_lifecycle_item_count),
            upload_superseded_lifecycle_item_count: a
                .upload_superseded_lifecycle_item_count
                .max(b.upload_superseded_lifecycle_item_count),
            upload_drained_lifecycle_item_count: a
                .upload_drained_lifecycle_item_count
                .max(b.upload_drained_lifecycle_item_count),
            upload_released_compile_job_count: a
                .upload_released_compile_job_count
                .max(b.upload_released_compile_job_count),
            upload_released_compile_jobs_on_enqueue: a
                .upload_released_compile_jobs_on_enqueue
                .max(b.upload_released_compile_jobs_on_enqueue),
            upload_released_compile_jobs_on_apply: a
                .upload_released_compile_jobs_on_apply
                .max(b.upload_released_compile_jobs_on_apply),
            upload_held_lifecycle_item_count: a
                .upload_held_lifecycle_item_count
                .max(b.upload_held_lifecycle_item_count),
            upload_held_compile_job_count: a
                .upload_held_compile_job_count
                .max(b.upload_held_compile_job_count),
            upload_limited: a.upload_limited || b.upload_limited,
            upload_accept_limited: a.upload_accept_limited || b.upload_accept_limited,
            upload_backpressured: a.upload_backpressured || b.upload_backpressured,
            traversal_ready_section_count: a
                .traversal_ready_section_count
                .max(b.traversal_ready_section_count),
            visibility_graph_build_count: a
                .visibility_graph_build_count
                .max(b.visibility_graph_build_count),
            visibility_graph_total_ms: a.visibility_graph_total_ms.max(b.visibility_graph_total_ms),
            visibility_graph_worst_ms: a.visibility_graph_worst_ms.max(b.visibility_graph_worst_ms),
            record_cache: max_record_cache_stats(a.record_cache, b.record_cache),
        }
    }

    fn android_xr_perf_mode_label(
        flight: Option<AndroidXrPerfFlight>,
        settled_orbit: Option<AndroidXrPerfOrbit>,
        chunk_view_churn: Option<AndroidXrPerfChunkViewChurn>,
        settled_stationary: bool,
        frozen_render: bool,
    ) -> &'static str {
        if frozen_render {
            "stationary-frozen-render"
        } else if chunk_view_churn.is_some() {
            "chunk-view-churn"
        } else if settled_orbit.is_some() {
            "settled-orbit"
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

    fn copy_locomotion_timing(
        timing: &mut AndroidXrRenderFrameTiming,
        locomotion: mclone_xr_scene::XrLocomotionTiming,
    ) {
        timing.locomotion_input_ms = locomotion.input_ms;
        timing.locomotion_camera_apply_ms = locomotion.camera_apply_ms;
        timing.locomotion_commit_ms = locomotion.commit_ms;
        timing.locomotion_commit_server_command_ms = locomotion.commit_server_command_ms;
        timing.locomotion_commit_server_command_send_ms = locomotion.commit_server_command_send_ms;
        timing.locomotion_commit_server_command_drain_updates_ms =
            locomotion.commit_server_command_drain_updates_ms;
        timing.locomotion_commit_server_command_apply_updates_ms =
            locomotion.commit_server_command_apply_updates_ms;
        timing.locomotion_commit_server_command_apply_dirty_mark_ms =
            locomotion.commit_server_command_apply_dirty_mark_ms;
        timing.locomotion_commit_server_command_apply_client_updates_ms =
            locomotion.commit_server_command_apply_client_updates_ms;
        timing.locomotion_commit_server_command_updates = locomotion.commit_server_command_updates;
        timing.locomotion_commit_server_command_snapshot_updates =
            locomotion.commit_server_command_snapshot_updates;
        timing.locomotion_commit_server_command_section_block_updates =
            locomotion.commit_server_command_section_block_updates;
        timing.locomotion_commit_server_command_unload_updates =
            locomotion.commit_server_command_unload_updates;
        timing.locomotion_commit_position_updates_ms = locomotion.commit_position_updates_ms;
        timing.locomotion_commit_interest_ms = locomotion.commit_interest_ms;
        timing.locomotion_commit_interest_command_send_ms =
            locomotion.commit_interest_command_send_ms;
        timing.locomotion_commit_interest_command_drain_updates_ms =
            locomotion.commit_interest_command_drain_updates_ms;
        timing.locomotion_commit_interest_command_apply_updates_ms =
            locomotion.commit_interest_command_apply_updates_ms;
        timing.locomotion_commit_interest_command_apply_dirty_mark_ms =
            locomotion.commit_interest_command_apply_dirty_mark_ms;
        timing.locomotion_commit_interest_command_apply_client_updates_ms =
            locomotion.commit_interest_command_apply_client_updates_ms;
        timing.locomotion_commit_interest_command_updates =
            locomotion.commit_interest_command_updates;
        timing.locomotion_commit_interest_command_snapshot_updates =
            locomotion.commit_interest_command_snapshot_updates;
        timing.locomotion_commit_interest_command_section_block_updates =
            locomotion.commit_interest_command_section_block_updates;
        timing.locomotion_commit_interest_command_unload_updates =
            locomotion.commit_interest_command_unload_updates;
        timing.locomotion_gameplay_interaction_ms = locomotion.gameplay_interaction_ms;
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
        let locomotion_timing = match automation {
            Some(AndroidXrPerfAutomation::Flight {
                speed_blocks_per_second,
            }) => terrain
                .apply_automated_flight_input(
                    [stereo_views.left, stereo_views.right],
                    speed_blocks_per_second,
                )
                .context("apply Android XR automated flight locomotion")?,
            Some(AndroidXrPerfAutomation::Orbit {
                speed_blocks_per_second,
                elapsed_seconds,
            }) => terrain
                .apply_automated_orbit_input(speed_blocks_per_second, elapsed_seconds)
                .context("apply Android XR automated orbit locomotion")?,
            Some(AndroidXrPerfAutomation::Stationary {
                frozen_render: freeze_runtime,
            }) => {
                frozen_render = freeze_runtime;
                terrain.apply_automated_stationary_input()
            }
            Some(AndroidXrPerfAutomation::ChunkViewChurn { center_x, center_z }) => terrain
                .apply_automated_chunk_view_churn(center_x, center_z)
                .context("apply Android XR automated chunk-view churn")?,
            None => terrain
                .apply_locomotion_input(controllers, [stereo_views.left, stereo_views.right])
                .context("apply Android XR controller locomotion")?,
        };
        timing.locomotion_ms = elapsed_ms(locomotion_start);
        copy_locomotion_timing(&mut timing, locomotion_timing);

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
        copy_terrain_frame_timing(&mut timing, frame_summary.timing);
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

    fn render_mclone_multiview_frame(
        graphics: &mut graphics_vulkan::VulkanGraphicsSession,
        stage: &xr::Space,
        environment_blend_mode: xr::EnvironmentBlendMode,
        predicted_display_time: xr::Time,
        stereo_target: &mut graphics_vulkan::OpenXrStereoState,
        depth: &mut ChunkMultiviewDepthTarget,
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
        let locomotion_timing = match automation {
            Some(AndroidXrPerfAutomation::Flight {
                speed_blocks_per_second,
            }) => terrain
                .apply_automated_flight_input(
                    [stereo_views.left, stereo_views.right],
                    speed_blocks_per_second,
                )
                .context("apply Android XR automated flight locomotion")?,
            Some(AndroidXrPerfAutomation::Orbit {
                speed_blocks_per_second,
                elapsed_seconds,
            }) => terrain
                .apply_automated_orbit_input(speed_blocks_per_second, elapsed_seconds)
                .context("apply Android XR automated orbit locomotion")?,
            Some(AndroidXrPerfAutomation::Stationary {
                frozen_render: freeze_runtime,
            }) => {
                frozen_render = freeze_runtime;
                terrain.apply_automated_stationary_input()
            }
            Some(AndroidXrPerfAutomation::ChunkViewChurn { center_x, center_z }) => terrain
                .apply_automated_chunk_view_churn(center_x, center_z)
                .context("apply Android XR automated chunk-view churn")?,
            None => terrain
                .apply_locomotion_input(controllers, [stereo_views.left, stereo_views.right])
                .context("apply Android XR controller locomotion")?,
        };
        timing.locomotion_ms = elapsed_ms(locomotion_start);
        copy_locomotion_timing(&mut timing, locomotion_timing);

        let target_width = stereo_target.width;
        let target_height = stereo_target.height;
        if depth.width != target_width || depth.height != target_height {
            *depth = ChunkMultiviewDepthTarget::new(&graphics.device, target_width, target_height);
        }
        let acquire_start = Instant::now();
        let target = acquire_stereo_target(stereo_target)
            .context("acquire full-frame multiview OpenXR image")?;
        timing.acquire_left_ms = elapsed_ms(acquire_start);
        let terrain_render_start = Instant::now();
        let terrain_target = XrTerrainMultiviewTarget {
            color_view: target.color_array_view(),
            depth,
            size: [target_width, target_height],
        };
        let frame_summary = if frozen_render {
            let fixed_render_view_pose = fixed_render_view_pose.with_context(
                || "Android XR multiview frozen render probe requires a fixed startup view pose",
            )?;
            terrain.render_frame_multiview_frozen_runtime_at_view_pose(
                &graphics.device,
                &graphics.queue,
                fixed_render_view_pose,
                [stereo_views.left.fov, stereo_views.right.fov],
                terrain_target,
            )
        } else {
            terrain.render_frame_multiview(
                &graphics.device,
                &graphics.queue,
                [stereo_views.left, stereo_views.right],
                terrain_target,
            )
        };
        timing.terrain_render_frame_ms = elapsed_ms(terrain_render_start);
        let release_start = Instant::now();
        let release_result = target.release();
        timing.release_eyes_ms = elapsed_ms(release_start);
        let frame_summary = frame_summary?;
        copy_terrain_frame_timing(&mut timing, frame_summary.timing);
        release_result?;

        let end_frame_start = Instant::now();
        mclone_xr_host::end_multiview_projection_frame(
            &mut graphics.frame_stream,
            predicted_display_time,
            environment_blend_mode,
            stage,
            stereo_views,
            stereo_target,
        )?;
        timing.end_frame_ms = elapsed_ms(end_frame_start);
        Ok(AndroidXrRenderedFrame {
            summary: frame_summary,
            camera: terrain.camera_snapshot(),
            timing,
        })
    }

    fn copy_terrain_frame_timing(
        timing: &mut AndroidXrRenderFrameTiming,
        scene_timing: mclone_xr_scene::XrTerrainFrameTiming,
    ) {
        timing.terrain_render_views_ms = scene_timing.render_views_ms;
        timing.terrain_menu_pointer_ms = scene_timing.menu_pointer_ms;
        timing.terrain_runtime_upload_ms = scene_timing.runtime_upload_ms;
        timing.terrain_runtime_poll_ms = scene_timing.runtime_poll_ms;
        timing.terrain_runtime_sync_ms = scene_timing.runtime_sync_ms;
        timing.terrain_runtime_sync_unattributed_ms = scene_timing.runtime_sync_unattributed_ms;
        timing.terrain_runtime_result_accept_ms = scene_timing.runtime_result_accept_ms;
        timing.terrain_runtime_dirty_seed_ms = scene_timing.runtime_dirty_seed_ms;
        timing.terrain_runtime_prepare_ms = scene_timing.runtime_prepare_ms;
        timing.terrain_runtime_submit_ms = scene_timing.runtime_submit_ms;
        timing.terrain_runtime_submit_snapshot_ms = scene_timing.runtime_submit_snapshot_ms;
        timing.terrain_runtime_submit_handoff_ms = scene_timing.runtime_submit_handoff_ms;
        timing.terrain_runtime_submit_handoff_worst_ms =
            scene_timing.runtime_submit_handoff_worst_ms;
        timing.terrain_runtime_submit_request_count = scene_timing.runtime_submit_request_count;
        timing.terrain_runtime_submit_request_build_ms =
            scene_timing.runtime_submit_request_build_ms;
        timing.terrain_runtime_submit_compiler_ms = scene_timing.runtime_submit_compiler_ms;
        timing.terrain_runtime_submit_compiler_worst_ms =
            scene_timing.runtime_submit_compiler_worst_ms;
        timing.terrain_runtime_submit_compiler_capacity_check_ms =
            scene_timing.runtime_submit_compiler_capacity_check_ms;
        timing.terrain_runtime_submit_compiler_capacity_check_worst_ms =
            scene_timing.runtime_submit_compiler_capacity_check_worst_ms;
        timing.terrain_runtime_submit_compiler_command_send_ms =
            scene_timing.runtime_submit_compiler_command_send_ms;
        timing.terrain_runtime_submit_compiler_command_send_worst_ms =
            scene_timing.runtime_submit_compiler_command_send_worst_ms;
        timing.terrain_runtime_submit_compiler_command_lock_wait_ms =
            scene_timing.runtime_submit_compiler_command_lock_wait_ms;
        timing.terrain_runtime_submit_compiler_command_lock_wait_worst_ms =
            scene_timing.runtime_submit_compiler_command_lock_wait_worst_ms;
        timing.terrain_runtime_submit_compiler_command_slot_select_ms =
            scene_timing.runtime_submit_compiler_command_slot_select_ms;
        timing.terrain_runtime_submit_compiler_command_slot_select_worst_ms =
            scene_timing.runtime_submit_compiler_command_slot_select_worst_ms;
        timing.terrain_runtime_submit_compiler_command_slot_write_ms =
            scene_timing.runtime_submit_compiler_command_slot_write_ms;
        timing.terrain_runtime_submit_compiler_command_slot_write_worst_ms =
            scene_timing.runtime_submit_compiler_command_slot_write_worst_ms;
        timing.terrain_runtime_submit_compiler_command_queue_push_ms =
            scene_timing.runtime_submit_compiler_command_queue_push_ms;
        timing.terrain_runtime_submit_compiler_command_queue_push_worst_ms =
            scene_timing.runtime_submit_compiler_command_queue_push_worst_ms;
        timing.terrain_runtime_submit_compiler_command_notify_ms =
            scene_timing.runtime_submit_compiler_command_notify_ms;
        timing.terrain_runtime_submit_compiler_command_notify_worst_ms =
            scene_timing.runtime_submit_compiler_command_notify_worst_ms;
        timing.terrain_runtime_submit_compiler_command_post_enqueue_ms =
            scene_timing.runtime_submit_compiler_command_post_enqueue_ms;
        timing.terrain_runtime_submit_compiler_command_post_enqueue_worst_ms =
            scene_timing.runtime_submit_compiler_command_post_enqueue_worst_ms;
        timing.terrain_runtime_submit_compiler_pending_mark_ms =
            scene_timing.runtime_submit_compiler_pending_mark_ms;
        timing.terrain_runtime_submit_compiler_pending_mark_worst_ms =
            scene_timing.runtime_submit_compiler_pending_mark_worst_ms;
        timing.terrain_runtime_submit_mark_inflight_ms =
            scene_timing.runtime_submit_mark_inflight_ms;
        timing.terrain_runtime_submit_apply_ready_plan_ms =
            scene_timing.runtime_submit_apply_ready_plan_ms;
        timing.terrain_runtime_submit_ready_update_ms = scene_timing.runtime_submit_ready_update_ms;
        timing.terrain_runtime_submit_ready_section_count =
            scene_timing.runtime_submit_ready_section_count;
        timing.terrain_runtime_submit_deferred_section_count =
            scene_timing.runtime_submit_deferred_section_count;
        timing.terrain_runtime_submit_dirty_chunk_count_before =
            scene_timing.runtime_submit_dirty_chunk_count_before;
        timing.terrain_runtime_submit_dirty_chunk_count_after =
            scene_timing.runtime_submit_dirty_chunk_count_after;
        timing.terrain_runtime_submit_dirty_section_count_before =
            scene_timing.runtime_submit_dirty_section_count_before;
        timing.terrain_runtime_submit_dirty_section_count_after =
            scene_timing.runtime_submit_dirty_section_count_after;
        timing.terrain_runtime_submit_inflight_section_count_before =
            scene_timing.runtime_submit_inflight_section_count_before;
        timing.terrain_runtime_submit_inflight_section_count_after =
            scene_timing.runtime_submit_inflight_section_count_after;
        timing.terrain_runtime_submit_request_target_section_count =
            scene_timing.runtime_submit_request_target_section_count;
        timing.terrain_runtime_submit_request_target_section_count_worst =
            scene_timing.runtime_submit_request_target_section_count_worst;
        timing.terrain_runtime_submit_request_snapshot_count =
            scene_timing.runtime_submit_request_snapshot_count;
        timing.terrain_runtime_submit_request_snapshot_section_count =
            scene_timing.runtime_submit_request_snapshot_section_count;
        timing.terrain_runtime_submit_request_snapshot_section_count_worst =
            scene_timing.runtime_submit_request_snapshot_section_count_worst;
        timing.terrain_runtime_submit_request_light_section_count =
            scene_timing.runtime_submit_request_light_section_count;
        timing.terrain_runtime_submit_request_light_section_count_worst =
            scene_timing.runtime_submit_request_light_section_count_worst;
        timing.terrain_runtime_submit_request_revision_count =
            scene_timing.runtime_submit_request_revision_count;
        timing.terrain_runtime_submit_request_estimated_payload_bytes =
            scene_timing.runtime_submit_request_estimated_payload_bytes;
        timing.terrain_runtime_submit_request_estimated_payload_bytes_worst =
            scene_timing.runtime_submit_request_estimated_payload_bytes_worst;
        timing.terrain_runtime_dispatcher_pending_jobs =
            scene_timing.runtime_dispatcher_pending_jobs;
        timing.terrain_runtime_dispatcher_max_pending_jobs =
            scene_timing.runtime_dispatcher_max_pending_jobs;
        timing.terrain_runtime_dispatcher_available_job_slots =
            scene_timing.runtime_dispatcher_available_job_slots;
        timing.terrain_runtime_dispatcher_queued_compile_tasks =
            scene_timing.runtime_dispatcher_queued_compile_tasks;
        timing.terrain_runtime_gpu_upload_ms = scene_timing.runtime_gpu_upload_ms;
        timing.terrain_runtime_gpu_upload_pre_sync_ms = scene_timing.runtime_gpu_upload_pre_sync_ms;
        timing.terrain_runtime_gpu_upload_post_sync_ms =
            scene_timing.runtime_gpu_upload_post_sync_ms;
        timing.terrain_runtime_upload_enqueue_ms = scene_timing.runtime_upload_enqueue_ms;
        timing.terrain_runtime_upload_select_ms = scene_timing.runtime_upload_select_ms;
        timing.terrain_runtime_upload_apply_ms = scene_timing.runtime_upload_apply_ms;
        timing.terrain_runtime_upload_apply_dirty_mark_ms =
            scene_timing.runtime_upload_apply_dirty_mark_ms;
        timing.terrain_runtime_upload_apply_remove_ms = scene_timing.runtime_upload_apply_remove_ms;
        timing.terrain_runtime_upload_apply_section_state_ms =
            scene_timing.runtime_upload_apply_section_state_ms;
        timing.terrain_runtime_upload_apply_vertex_bytes_ms =
            scene_timing.runtime_upload_apply_vertex_bytes_ms;
        timing.terrain_runtime_upload_apply_vertex_buffer_ms =
            scene_timing.runtime_upload_apply_vertex_buffer_ms;
        timing.terrain_runtime_upload_apply_index_bytes_ms =
            scene_timing.runtime_upload_apply_index_bytes_ms;
        timing.terrain_runtime_upload_apply_index_buffer_ms =
            scene_timing.runtime_upload_apply_index_buffer_ms;
        timing.terrain_runtime_upload_apply_mesh_insert_ms =
            scene_timing.runtime_upload_apply_mesh_insert_ms;
        timing.terrain_runtime_upload_apply_mesh_upload_worst_ms =
            scene_timing.runtime_upload_apply_mesh_upload_worst_ms;
        timing.terrain_runtime_ready_sections_ms = scene_timing.runtime_ready_sections_ms;
        timing.terrain_runtime_ready_publish_ms = scene_timing.runtime_ready_publish_ms;
        timing.terrain_shared_records_ms = scene_timing.shared_records_ms;
        timing.terrain_record_cache_prepare = scene_timing.record_cache_prepare;
        timing.terrain_left_eye_ms = scene_timing.left_eye_ms;
        timing.terrain_right_eye_ms = scene_timing.right_eye_ms;
        timing.terrain_left_eye_prepare_ms = scene_timing.left_eye_render.prepare_ms;
        timing.terrain_left_eye_cull_ms = scene_timing.left_eye_render.cull_ms;
        timing.terrain_left_eye_uniform_write_ms = scene_timing.left_eye_render.uniform_write_ms;
        timing.terrain_left_eye_translucent_collect_ms =
            scene_timing.left_eye_render.translucent_collect_ms;
        timing.terrain_left_eye_translucent_sort_ms =
            scene_timing.left_eye_render.translucent_sort_ms;
        timing.terrain_left_eye_encode_ms = scene_timing.left_eye_render.encode_ms;
        timing.terrain_left_eye_section_encode_ms = scene_timing.left_eye_render.section_encode_ms;
        timing.terrain_left_eye_full_frame_ms = scene_timing.left_eye_render.full_frame_ms;
        timing.terrain_left_eye_sky_ms = scene_timing.left_eye_render.sky_ms;
        timing.terrain_left_eye_far_lod_ms = scene_timing.left_eye_render.far_lod_ms;
        timing.terrain_left_eye_opaque_ms = scene_timing.left_eye_render.terrain_opaque_ms;
        timing.terrain_left_eye_translucent_ms =
            scene_timing.left_eye_render.terrain_translucent_ms;
        timing.terrain_left_eye_actor_ms = scene_timing.left_eye_render.actor_ms;
        timing.terrain_left_eye_screen_effect_ms = scene_timing.left_eye_render.screen_effect_ms;
        timing.terrain_left_eye_gui_ms = scene_timing.left_eye_render.gui_ms;
        timing.terrain_left_eye_xr_fade_ms = scene_timing.left_eye_render.xr_fade_ms;
        timing.terrain_left_eye_xr_selection_ms = scene_timing.left_eye_render.xr_selection_ms;
        timing.terrain_left_eye_xr_world_lines_ms = scene_timing.left_eye_render.xr_world_lines_ms;
        timing.terrain_left_eye_xr_world_panel_ms = scene_timing.left_eye_render.xr_world_panel_ms;
        timing.terrain_left_eye_encoder_finish_ms = scene_timing.left_eye_render.encoder_finish_ms;
        timing.terrain_left_eye_submit_ms = scene_timing.left_eye_render.submit_ms;
        timing.terrain_left_eye_poll_wait_ms = scene_timing.left_eye_render.poll_wait_ms;
        timing.terrain_right_eye_prepare_ms = scene_timing.right_eye_render.prepare_ms;
        timing.terrain_right_eye_cull_ms = scene_timing.right_eye_render.cull_ms;
        timing.terrain_right_eye_uniform_write_ms = scene_timing.right_eye_render.uniform_write_ms;
        timing.terrain_right_eye_translucent_collect_ms =
            scene_timing.right_eye_render.translucent_collect_ms;
        timing.terrain_right_eye_translucent_sort_ms =
            scene_timing.right_eye_render.translucent_sort_ms;
        timing.terrain_right_eye_encode_ms = scene_timing.right_eye_render.encode_ms;
        timing.terrain_right_eye_section_encode_ms =
            scene_timing.right_eye_render.section_encode_ms;
        timing.terrain_right_eye_full_frame_ms = scene_timing.right_eye_render.full_frame_ms;
        timing.terrain_right_eye_sky_ms = scene_timing.right_eye_render.sky_ms;
        timing.terrain_right_eye_far_lod_ms = scene_timing.right_eye_render.far_lod_ms;
        timing.terrain_right_eye_opaque_ms = scene_timing.right_eye_render.terrain_opaque_ms;
        timing.terrain_right_eye_translucent_ms =
            scene_timing.right_eye_render.terrain_translucent_ms;
        timing.terrain_right_eye_actor_ms = scene_timing.right_eye_render.actor_ms;
        timing.terrain_right_eye_screen_effect_ms = scene_timing.right_eye_render.screen_effect_ms;
        timing.terrain_right_eye_gui_ms = scene_timing.right_eye_render.gui_ms;
        timing.terrain_right_eye_xr_fade_ms = scene_timing.right_eye_render.xr_fade_ms;
        timing.terrain_right_eye_xr_selection_ms = scene_timing.right_eye_render.xr_selection_ms;
        timing.terrain_right_eye_xr_world_lines_ms =
            scene_timing.right_eye_render.xr_world_lines_ms;
        timing.terrain_right_eye_xr_world_panel_ms =
            scene_timing.right_eye_render.xr_world_panel_ms;
        timing.terrain_right_eye_encoder_finish_ms =
            scene_timing.right_eye_render.encoder_finish_ms;
        timing.terrain_right_eye_submit_ms = scene_timing.right_eye_render.submit_ms;
        timing.terrain_right_eye_poll_wait_ms = scene_timing.right_eye_render.poll_wait_ms;
        timing.terrain_stereo_finish_ms = scene_timing.stereo_finish_ms;
        timing.terrain_stereo_submit_ms = scene_timing.stereo_submit_ms;
        timing.terrain_stereo_poll_wait_ms = scene_timing.stereo_poll_wait_ms;
        timing.terrain_overlap_runtime_prefetch_ms = scene_timing.overlap_runtime_prefetch_ms;
        timing.terrain_overlap_runtime_prefetch_poll_ms =
            scene_timing.overlap_runtime_prefetch_poll_ms;
        timing.terrain_overlap_runtime_prefetch_sync_ms =
            scene_timing.overlap_runtime_prefetch_sync_ms;
        timing.terrain_overlap_runtime_prefetch_gpu_upload_ms =
            scene_timing.overlap_runtime_prefetch_gpu_upload_ms;
        timing.terrain_overlap_runtime_prefetch_ready_sections_ms =
            scene_timing.overlap_runtime_prefetch_ready_sections_ms;
        timing.terrain_multiview_sky_ms = scene_timing.multiview_sky_ms;
        timing.terrain_multiview_terrain_ms = scene_timing.multiview_terrain_ms;
        timing.terrain_multiview_actor_ms = scene_timing.multiview_actor_ms;
        timing.terrain_multiview_screen_effect_ms = scene_timing.multiview_screen_effect_ms;
        timing.terrain_multiview_world_overlays_ms = scene_timing.multiview_world_overlays_ms;
        timing.terrain_multiview_submit_ms = scene_timing.multiview_submit_ms;
        timing.terrain_multiview_poll_wait_ms = scene_timing.multiview_poll_wait_ms;
    }

    fn acquire_eye_target(
        eye_state: &mut graphics_vulkan::OpenXrEyeState,
    ) -> Result<AcquiredEyeTarget<'_>> {
        mclone_xr_host::acquire_eye_target(eye_state)
    }

    fn acquire_stereo_target(
        stereo_state: &mut graphics_vulkan::OpenXrStereoState,
    ) -> Result<AcquiredStereoTarget<'_>> {
        mclone_xr_host::acquire_stereo_target(stereo_state)
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        ANDROID_REMOTE_ADDR_NONE_SENTINEL, ANDROID_XR_LOCAL_ARG_FLAGS,
        normalize_android_legacy_remote_addr,
    };

    fn collect_source_flags(source: &str) -> BTreeSet<String> {
        let mut flags = BTreeSet::new();
        let bytes = source.as_bytes();
        let mut index = 0;
        while let Some(offset) = source[index..].find("--") {
            let start = index + offset;
            let mut end = start + 2;
            while end < bytes.len() {
                let byte = bytes[end];
                if byte.is_ascii_alphanumeric() || byte == b'-' {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > start + 2 {
                flags.insert(source[start..end].to_owned());
            }
            index = end.max(start + 2);
        }
        flags
    }

    #[test]
    fn android_xr_startup_flags_are_classified_as_shared_or_xr_local() {
        let shared = mclone_app_runtime::startup_args::STARTUP_ARG_FLAGS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let xr_local = ANDROID_XR_LOCAL_ARG_FLAGS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(
            shared.is_disjoint(&xr_local),
            "Android XR local flags must not duplicate shared startup flags"
        );

        let unclassified = collect_source_flags(include_str!("lib.rs"))
            .into_iter()
            .filter(|flag| !shared.contains(flag.as_str()) && !xr_local.contains(flag.as_str()))
            .collect::<Vec<_>>();
        assert!(
            unclassified.is_empty(),
            "classify Android XR startup flags as shared startup or XR-local: {unclassified:?}"
        );
    }

    #[test]
    fn android_legacy_remote_addr_ignores_empty_values() {
        assert_eq!(normalize_android_legacy_remote_addr(""), None);
        assert_eq!(normalize_android_legacy_remote_addr("   \t"), None);
    }

    #[test]
    fn android_legacy_remote_addr_ignores_none_sentinel() {
        assert_eq!(
            normalize_android_legacy_remote_addr(ANDROID_REMOTE_ADDR_NONE_SENTINEL),
            None
        );
    }

    #[test]
    fn android_legacy_remote_addr_preserves_real_address() {
        assert_eq!(
            normalize_android_legacy_remote_addr(" 192.168.1.10:25565 "),
            Some("192.168.1.10:25565".to_owned())
        );
    }

    #[test]
    fn android_legacy_remote_addr_preserves_sentinel_near_misses() {
        assert_eq!(
            normalize_android_legacy_remote_addr("__mclone_none__:25565"),
            Some("__mclone_none__:25565".to_owned())
        );
    }
}
