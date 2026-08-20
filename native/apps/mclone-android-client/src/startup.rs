use std::ffi::{CStr, CString, c_char};
use std::fmt::Display;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use mclone_android_platform::{android_app_data_world_root, normalize_android_legacy_remote_addr};
use mclone_app_runtime::client_entry::{
    ClientEntryIntent, ClientEntryResolution, ClientEntrySource,
};
use mclone_app_runtime::render_compile_capacity::{
    RenderCompileCapacityHostKind, host_total_memory_bytes,
    preflight_render_compile_capacity_report,
};
use mclone_app_runtime::session::{RemoteSessionEndpoint, SessionStartRequest};
use mclone_app_runtime::startup_args::{
    RenderCompileCapacityRequest, RenderDistanceLimits, StartupArgState, StartupCameraOptions,
    StartupSceneOptions, StartupWorldStorageProjection, format_optional_startup_path,
    parse_bool_arg,
};
use mclone_core::Vec3d;
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_scene::{McloneSceneHost, McloneSceneHostOptions};
use winit::platform::android::activity::AndroidApp;

const STARTUP_ARGV_INTENT_EXTRA: &str = "mclone.startup.argv";
const REMOTE_ADDR_PROPERTY: &str = "debug.mclone.remote_addr";
const ANDROID_PROPERTY_VALUE_MAX: usize = 92;
const ANDROID_MIN_RENDER_DISTANCE: u32 = 1;
const ANDROID_MAX_RENDER_DISTANCE: u32 = 16;
const DEFAULT_PACING_PERF_WARMUP_SECONDS: u64 = 5;
const DEFAULT_PACING_PERF_CHURN_INTERVAL_SECONDS: f64 = 3.0;
const DEFAULT_PACING_PERF_CHURN_OFFSET_CHUNKS: i32 = 16;
const MAX_PACING_PERF_SECONDS: u64 = 300;

#[allow(unsafe_code)]
unsafe extern "C" {
    fn __system_property_get(name: *const c_char, value: *mut c_char) -> i32;
}

#[derive(Clone, Debug)]
pub(crate) struct AndroidStartupOptions {
    pub(crate) scene: McloneSceneHostOptions,
    pub(crate) remote_addr: Option<String>,
    pub(crate) entry: ClientEntryResolution,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) camera: StartupCameraOptions,
    pub(crate) pacing_perf: Option<AndroidPacingPerfOptions>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AndroidPacingPerfOptions {
    pub(crate) label: String,
    pub(crate) warmup_seconds: u64,
    pub(crate) sample_seconds: u64,
    pub(crate) churn_interval_seconds: f64,
    pub(crate) churn_offset_chunks: i32,
    pub(crate) base_chunk_x: i32,
    pub(crate) base_chunk_z: i32,
}

pub(crate) fn prepare_android_startup(app: &AndroidApp) -> Result<AndroidStartupOptions> {
    let startup_argv = android_startup_argv_json(app)?;
    match startup_argv.as_deref() {
        Some(json) if !json.trim().is_empty() => {
            log::info!("Android startup argv from {STARTUP_ARGV_INTENT_EXTRA}: {json}");
        }
        _ => log::info!("Android startup argv from {STARTUP_ARGV_INTENT_EXTRA}: <none>"),
    }
    let (mut options, default_world_root_enabled, start_in_world) =
        parse_android_startup_options(startup_argv.as_deref())?;
    let argv_remote_addr = options.remote_addr.clone();
    let legacy_remote_addr = android_remote_addr();
    if options.remote_addr.is_none() {
        options.remote_addr = legacy_remote_addr.clone();
    }
    if default_world_root_enabled && options.scene.world_root.is_none() {
        let storage = StartupWorldStorageProjection::from_parts(
            true,
            options.scene.world_root.clone(),
            options.scene.world_dir.clone(),
        )
        .with_default_world_root(android_app_data_world_root(app, "Android"));
        options.scene.world_root = storage.world_root;
    }
    StartupWorldStorageProjection::from_parts(
        default_world_root_enabled,
        options.scene.world_root.clone(),
        options.scene.world_dir.clone(),
    )
    .validate_local_integrated_world(options.remote_addr.as_deref())?;
    options.entry = resolve_android_entry(
        start_in_world,
        &options,
        argv_remote_addr.is_some() || startup_argv.is_some(),
        legacy_remote_addr.is_some(),
    );

    if let Some(remote_addr) = argv_remote_addr.as_deref() {
        log::info!(
            "Mclone Android remote dedicated address from {STARTUP_ARGV_INTENT_EXTRA}: {remote_addr}"
        );
    } else if let Some(remote_addr) = legacy_remote_addr.as_deref() {
        log::info!(
            "Mclone Android remote dedicated address from legacy {REMOTE_ADDR_PROPERTY}: {remote_addr}"
        );
    } else {
        log::info!("Mclone Android remote dedicated address: <none>");
    }
    log::info!(
        "Mclone Android scene options: seed={} center=({}, {}) render_distance={} render_compile_workers={} render_compile_max_pending_jobs={:?} day_time={:?} freeze_time={} movement_speed={:.2}x lighting={}",
        options.scene.seed,
        options.scene.chunk_x,
        options.scene.chunk_z,
        options.scene.render_distance,
        options.scene.render_compile_worker_count,
        options.scene.render_compile_max_pending_jobs,
        options.scene.day_time_override,
        options.scene.freeze_time,
        options.scene.movement_speed_multiplier,
        options.scene.lighting_enabled
    );
    log::info!(
        "Mclone Android world storage: default_root_enabled={} world_root={} world_dir={}",
        default_world_root_enabled,
        format_optional_startup_path(options.scene.world_root.as_deref(), "<none>"),
        format_optional_startup_path(options.scene.world_dir.as_deref(), "<none>")
    );
    log::info!(
        "Mclone Android render options: section_occlusion={} fullbright={} color_profile={}",
        options.render_options.section_occlusion_culling,
        options.render_options.force_fullbright,
        options.render_options.color_profile.as_str()
    );
    log::info!(
        "Mclone Android client entry source={} intent={}",
        options.entry.source.label(),
        options.entry.intent.label(),
    );
    if let Some(perf) = &options.pacing_perf {
        log::info!(
            "Mclone Android pacing perf: label={} warmup_seconds={} sample_seconds={} churn_interval_seconds={:.3} churn_offset_chunks={} base_center=({}, {})",
            perf.label,
            perf.warmup_seconds,
            perf.sample_seconds,
            perf.churn_interval_seconds,
            perf.churn_offset_chunks,
            perf.base_chunk_x,
            perf.base_chunk_z
        );
    }
    Ok(options)
}

fn parse_android_startup_options(
    startup_argv_json: Option<&str>,
) -> Result<(AndroidStartupOptions, bool, Option<bool>)> {
    let mut shared_args = StartupArgState::new(
        android_startup_scene_defaults(),
        TexturedSectionRenderOptions::default(),
    );
    let mut pacing_perf_label = None;
    let mut pacing_perf_warmup_seconds = None;
    let mut pacing_perf_sample_seconds = None;
    let mut pacing_perf_churn_interval_seconds = None;
    let mut pacing_perf_churn_offset_chunks = None;
    let mut start_in_world = None;
    if let Some(json) = startup_argv_json.filter(|json| !json.trim().is_empty()) {
        let argv =
            serde_json::from_str::<Vec<String>>(json).context("parse Android startup argv JSON")?;
        let mut argv = argv.into_iter();
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--menu" => {
                    start_in_world = Some(false);
                    continue;
                }
                "--start-in-world" => {
                    start_in_world = Some(parse_bool_arg(&arg, argv.next())?);
                    continue;
                }
                "--pacing-perf-label" => {
                    pacing_perf_label = Some(parse_android_arg(&mut argv, &arg)?);
                    continue;
                }
                "--pacing-perf-warmup-seconds" => {
                    pacing_perf_warmup_seconds = Some(parse_android_arg(&mut argv, &arg)?);
                    continue;
                }
                "--pacing-perf-seconds" => {
                    pacing_perf_sample_seconds = Some(parse_android_arg(&mut argv, &arg)?);
                    continue;
                }
                "--pacing-perf-churn-interval-seconds" => {
                    pacing_perf_churn_interval_seconds = Some(parse_android_arg(&mut argv, &arg)?);
                    continue;
                }
                "--pacing-perf-churn-offset-chunks" => {
                    pacing_perf_churn_offset_chunks = Some(parse_android_arg(&mut argv, &arg)?);
                    continue;
                }
                _ => {}
            }
            if shared_args.parse_next_arg(
                &arg,
                &mut argv,
                RenderDistanceLimits::new(ANDROID_MIN_RENDER_DISTANCE, ANDROID_MAX_RENDER_DISTANCE),
            )? {
                continue;
            }
            bail!("unsupported Android startup argument `{arg}`");
        }
    }
    if shared_args.scene().render_compile_capacity_request == RenderCompileCapacityRequest::Derived
        && shared_args.scene().remote_addr.is_some()
    {
        bail!("--render-compile-capacity derived applies only to local integrated worlds");
    }
    if shared_args.scene().render_compile_capacity_request == RenderCompileCapacityRequest::Derived
    {
        let report = preflight_render_compile_capacity_report(
            RenderCompileCapacityHostKind::Flat,
            host_total_memory_bytes(),
        );
        shared_args.apply_render_compile_capacity_report(&report);
    }
    let parsed = shared_args.finish();
    let pacing_perf = android_pacing_perf_options(
        pacing_perf_label,
        pacing_perf_warmup_seconds,
        pacing_perf_sample_seconds,
        pacing_perf_churn_interval_seconds,
        pacing_perf_churn_offset_chunks,
        parsed.scene.chunk_x,
        parsed.scene.chunk_z,
    )?;
    let storage = parsed.storage.project(None);
    let default_world_root_enabled = storage.default_world_root_enabled;
    let remote_addr = parsed.scene.remote_addr.clone();
    let scene = McloneSceneHostOptions::from_startup_scene(
        parsed.scene,
        storage.world_root.clone(),
        storage.world_dir.clone(),
    )
    .validated()?;
    Ok((
        AndroidStartupOptions {
            scene,
            remote_addr,
            entry: ClientEntryResolution::ordinary(),
            render_options: parsed.render_options,
            camera: parsed.camera,
            pacing_perf,
        },
        default_world_root_enabled,
        start_in_world,
    ))
}

fn resolve_android_entry(
    start_in_world: Option<bool>,
    options: &AndroidStartupOptions,
    activity_intent_present: bool,
    managed_remote_present: bool,
) -> ClientEntryResolution {
    let implied_session = options.remote_addr.is_some()
        || options.scene.world_dir.is_some()
        || options.pacing_perf.is_some();
    let starts_session = start_in_world.unwrap_or(implied_session);
    let source = if start_in_world.is_some() || activity_intent_present {
        ClientEntrySource::ActivityIntent
    } else if managed_remote_present {
        ClientEntrySource::ManagedLaunch
    } else {
        ClientEntrySource::ProductDefault
    };
    if !starts_session {
        return if source == ClientEntrySource::ProductDefault {
            ClientEntryResolution::ordinary()
        } else {
            ClientEntryResolution::explicit(ClientEntryIntent::Title, source)
        };
    }
    let request = options.remote_addr.as_ref().map_or_else(
        || {
            SessionStartRequest::new_seed_local_world_with_generation_profile(
                options.scene.seed,
                options.scene.world_generation_profile,
            )
        },
        |address| SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new(address.clone()),
        },
    );
    ClientEntryResolution::explicit(ClientEntryIntent::StartSession(request), source)
}

fn parse_android_arg<T>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    let value = args
        .next()
        .with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<T>()
        .map_err(|error| anyhow::anyhow!("invalid {flag} value `{value}`: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn android_pacing_perf_options(
    label: Option<String>,
    warmup_seconds: Option<u64>,
    sample_seconds: Option<u64>,
    churn_interval_seconds: Option<f64>,
    churn_offset_chunks: Option<i32>,
    base_chunk_x: i32,
    base_chunk_z: i32,
) -> Result<Option<AndroidPacingPerfOptions>> {
    let any_perf_arg = label.is_some()
        || warmup_seconds.is_some()
        || churn_interval_seconds.is_some()
        || churn_offset_chunks.is_some();
    let Some(sample_seconds) = sample_seconds else {
        if any_perf_arg {
            bail!("Android pacing perf options require --pacing-perf-seconds");
        }
        return Ok(None);
    };
    if sample_seconds == 0 || sample_seconds > MAX_PACING_PERF_SECONDS {
        bail!(
            "--pacing-perf-seconds must be between 1 and {MAX_PACING_PERF_SECONDS}, got {sample_seconds}"
        );
    }
    let warmup_seconds = warmup_seconds.unwrap_or(DEFAULT_PACING_PERF_WARMUP_SECONDS);
    if warmup_seconds > MAX_PACING_PERF_SECONDS {
        bail!(
            "--pacing-perf-warmup-seconds must be <= {MAX_PACING_PERF_SECONDS}, got {warmup_seconds}"
        );
    }
    let churn_interval_seconds =
        churn_interval_seconds.unwrap_or(DEFAULT_PACING_PERF_CHURN_INTERVAL_SECONDS);
    if !churn_interval_seconds.is_finite() || churn_interval_seconds <= 0.0 {
        bail!(
            "--pacing-perf-churn-interval-seconds must be finite and positive, got {churn_interval_seconds}"
        );
    }
    let churn_offset_chunks =
        churn_offset_chunks.unwrap_or(DEFAULT_PACING_PERF_CHURN_OFFSET_CHUNKS);
    if churn_offset_chunks <= 0 {
        bail!("--pacing-perf-churn-offset-chunks must be positive, got {churn_offset_chunks}");
    }
    let label = label.unwrap_or_else(|| "android-flat".to_owned());
    if label.is_empty()
        || label.len() > 48
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!(
            "--pacing-perf-label must be 1..=48 ASCII letters, digits, '.', '-', or '_', got `{label}`"
        );
    }
    Ok(Some(AndroidPacingPerfOptions {
        label,
        warmup_seconds,
        sample_seconds,
        churn_interval_seconds,
        churn_offset_chunks,
        base_chunk_x,
        base_chunk_z,
    }))
}

fn android_startup_scene_defaults() -> StartupSceneOptions {
    StartupSceneOptions::default()
        .with_graphics_platform_profile(
            mclone_app_runtime::graphics_preferences::ClientGraphicsPlatformProfile::FlatAndroid,
        )
        .with_initial_time_frozen_at(6000)
}

pub(crate) fn apply_startup_camera_options(
    host: &mut McloneSceneHost,
    options: StartupCameraOptions,
) {
    if options.eye.is_none() && options.target.is_none() {
        return;
    }
    let snapshot = host.camera_snapshot();
    let eye = options
        .eye
        .map(vec3d_from_f32_array)
        .unwrap_or(snapshot.eye);
    let (yaw_radians, pitch_radians) = options
        .target
        .and_then(|target| look_at_yaw_pitch(eye, vec3d_from_f32_array(target)))
        .unwrap_or((snapshot.yaw_radians, snapshot.pitch_radians));
    host.set_mono_capture_camera(
        eye,
        yaw_radians,
        pitch_radians,
        snapshot.speed_blocks_per_second,
    );
    log::info!(
        "Mclone Android startup camera pose: eye=({:.2}, {:.2}, {:.2}) yaw={:.3} pitch={:.3}",
        eye.x,
        eye.y,
        eye.z,
        yaw_radians,
        pitch_radians
    );
}

fn look_at_yaw_pitch(eye: Vec3d, target: Vec3d) -> Option<(f64, f64)> {
    let direction = target.subtract(eye);
    let length_sqr = direction.length_sqr();
    if length_sqr <= 1.0e-12 {
        return None;
    }
    let direction = direction.scale(1.0 / length_sqr.sqrt());
    Some((
        direction.x.atan2(direction.z),
        direction.y.clamp(-1.0, 1.0).asin(),
    ))
}

fn vec3d_from_f32_array(value: [f32; 3]) -> Vec3d {
    Vec3d::new(
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
    )
}

fn android_remote_addr() -> Option<String> {
    android_property(REMOTE_ADDR_PROPERTY)
        .and_then(|value| normalize_android_legacy_remote_addr(&value))
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
        log::warn!("Android startup argv intent extra unavailable: null JVM or Activity");
        return Ok(None);
    }

    let vm = unsafe { jni::JavaVM::from_raw(vm) };
    vm.attach_current_thread(|env| {
        let activity = unsafe { jni::objects::JObject::from_raw(env, activity) };
        let intent = env
            .call_method(
                &activity,
                jni::jni_str!("getIntent"),
                jni::jni_sig!("()Landroid/content/Intent;"),
                &[],
            )?
            .into_object()?;
        if intent.is_null() {
            return Ok(None);
        }
        let key = jni::objects::JObject::from(env.new_string(STARTUP_ARGV_INTENT_EXTRA)?);
        let has_extra = env
            .call_method(
                &intent,
                jni::jni_str!("hasExtra"),
                jni::jni_sig!("(Ljava/lang/String;)Z"),
                &[jni::objects::JValue::Object(&key)],
            )?
            .z()?;
        if !has_extra {
            return Ok(None);
        }
        let object = env
            .call_method(
                &intent,
                jni::jni_str!("getStringExtra"),
                jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                &[jni::objects::JValue::Object(&key)],
            )?
            .into_object()?;
        if object.is_null() {
            return Ok(None);
        }
        let string = jni::objects::JString::cast_local(env, object)?;
        string.try_to_string(env).map(Some)
    })
    .with_context(|| format!("failed to read Android intent extra `{STARTUP_ARGV_INTENT_EXTRA}`"))
}
