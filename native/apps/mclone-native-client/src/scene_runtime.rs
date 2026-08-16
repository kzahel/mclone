use std::time::Duration;

use anyhow::{Context, Result};
use mclone_app_runtime::DEFAULT_STARTUP_READINESS_TIMEOUT;
use mclone_app_runtime::host_mode::SingleViewHostOptions;
use mclone_app_runtime::local_profile::load_or_create_native_local_player_profile;
use mclone_app_runtime::native_remote_session::NativeRemoteServerSession;
use mclone_app_runtime::native_service_assembly::{
    LocalIntegratedSceneOptions, NativeSceneServices,
};
pub(crate) use mclone_app_runtime::{chunk_tracking_radius_for_render_distance, square_count};
use mclone_core::ChunkPos;

use crate::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use crate::cli::SceneOptions;
use crate::render_cache::{TexturedMeshAssets, load_textured_mesh_assets};

pub(crate) type WindowRuntimeStats = mclone_app_runtime::SingleViewRuntimeStats;

#[derive(Clone, Debug)]
pub(crate) struct WindowSceneAssets {
    pub(crate) mesh_assets: TexturedMeshAssets,
    pub(crate) actor_textures: ActorTextureAssets,
}

impl WindowSceneAssets {
    pub(crate) fn load() -> Result<Self> {
        Ok(Self {
            mesh_assets: load_textured_mesh_assets()?,
            actor_textures: load_actor_texture_assets()?,
        })
    }
}

fn scene_render_distance(scene: &SceneOptions) -> Result<u32> {
    u32::try_from(scene.render_distance).context("render distance must be non-negative")
}

pub(crate) fn local_integrated_scene_options(
    scene: &SceneOptions,
) -> Result<LocalIntegratedSceneOptions> {
    let host_scene = crate::desktop_scene_host::scene_host_options_from_desktop(scene)?;
    Ok(mclone_scene::local_integrated_scene_options(&host_scene))
}

#[cfg_attr(not(feature = "xr"), allow(dead_code))]
pub(crate) fn native_window_scene_runtime(
    scene: &SceneOptions,
) -> Result<NativeSceneServices<NativeRemoteServerSession>> {
    native_window_scene_runtime_with_mesh_assets(scene, load_textured_mesh_assets()?)
}

pub(crate) fn native_window_scene_runtime_with_mesh_assets(
    scene: &SceneOptions,
    mesh_assets: TexturedMeshAssets,
) -> Result<NativeSceneServices<NativeRemoteServerSession>> {
    let render_distance = scene_render_distance(scene)?;
    let center = ChunkPos::new(scene.chunk_x, scene.chunk_z);
    let Some(remote_addr) = &scene.remote_addr else {
        return NativeSceneServices::local_with_mesh_assets(
            local_integrated_scene_options(scene)?,
            mesh_assets,
        );
    };

    let profile = load_or_create_native_local_player_profile(scene.world_root.as_deref())?;
    let session = NativeRemoteServerSession::connect_with_identity(
        remote_addr.as_str(),
        "desktop",
        profile.client_identity(),
    )?;
    NativeSceneServices::remote_dedicated_with_mesh_assets(
        SingleViewHostOptions::new(center, render_distance)
            .with_render_compile_worker_count(scene.render_compile_worker_count)
            .with_render_compile_max_pending_jobs(scene.render_compile_max_pending_jobs)
            .with_render_compile_worker_timing_enabled(scene.render_compile_worker_timing_enabled),
        session,
        mesh_assets,
    )
    .with_context(|| {
        format!("failed to initialize desktop remote dedicated runtime from {remote_addr}")
    })
}

pub(crate) fn poll_window_runtime_until_idle(
    runtime: &mut NativeSceneServices<NativeRemoteServerSession>,
) -> Result<(usize, f64)> {
    runtime.poll_until_idle_with_timeout(DEFAULT_STARTUP_READINESS_TIMEOUT)
}

pub(crate) fn poll_window_runtime_until_idle_with_timeout(
    runtime: &mut NativeSceneServices<NativeRemoteServerSession>,
    timeout: Duration,
) -> Result<(usize, f64)> {
    runtime.poll_until_idle_with_timeout(timeout)
}
