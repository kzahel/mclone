use std::ffi::{CStr, CString, c_char};

use anyhow::{Context, Result, bail};
use mclone_android_platform::{android_app_data_world_root, normalize_android_legacy_remote_addr};
use mclone_app_runtime::render_compile_capacity::{
    RenderCompileCapacityHostKind, host_total_memory_bytes,
    preflight_render_compile_capacity_report,
};
use mclone_app_runtime::startup_args::{
    RenderCompileCapacityRequest, RenderDistanceLimits, StartupArgState, StartupCameraOptions,
    StartupSceneOptions, StartupWorldStorageProjection, format_optional_startup_path,
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

#[allow(unsafe_code)]
unsafe extern "C" {
    fn __system_property_get(name: *const c_char, value: *mut c_char) -> i32;
}

#[derive(Clone, Debug)]
pub(crate) struct AndroidStartupOptions {
    pub(crate) scene: McloneSceneHostOptions,
    pub(crate) remote_addr: Option<String>,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) camera: StartupCameraOptions,
}

pub(crate) fn prepare_android_startup(app: &AndroidApp) -> Result<AndroidStartupOptions> {
    let startup_argv = android_startup_argv_json(app)?;
    match startup_argv.as_deref() {
        Some(json) if !json.trim().is_empty() => {
            log::info!("Android startup argv from {STARTUP_ARGV_INTENT_EXTRA}: {json}");
        }
        _ => log::info!("Android startup argv from {STARTUP_ARGV_INTENT_EXTRA}: <none>"),
    }
    let (mut options, default_world_root_enabled) =
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
    Ok(options)
}

fn parse_android_startup_options(
    startup_argv_json: Option<&str>,
) -> Result<(AndroidStartupOptions, bool)> {
    let mut shared_args = StartupArgState::new(
        android_startup_scene_defaults(),
        TexturedSectionRenderOptions::default(),
    );
    if let Some(json) = startup_argv_json.filter(|json| !json.trim().is_empty()) {
        let argv =
            serde_json::from_str::<Vec<String>>(json).context("parse Android startup argv JSON")?;
        let mut argv = argv.into_iter();
        while let Some(arg) = argv.next() {
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
            render_options: parsed.render_options,
            camera: parsed.camera,
        },
        default_world_root_enabled,
    ))
}

fn android_startup_scene_defaults() -> StartupSceneOptions {
    StartupSceneOptions::default().with_initial_time_frozen_at(6000)
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
