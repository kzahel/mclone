//! Desktop-native construction and option projection for the shared scene
//! host. Surface drivers select topology and remain responsible for targets
//! and cadence.

use anyhow::{Context, Result};
use mclone_app_runtime::native_service_assembly::NativeSessionServices;
use mclone_app_runtime::prepared_assets::{
    AssetPackSourceRegistry, reference_asset_pack_selection,
};
use mclone_app_runtime::session::{RemoteSessionEndpoint, SessionStartRequest};
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_scene::{McloneSceneHost, McloneSceneHostOptions, XrStartupViewPose};

use crate::cli::SceneOptions;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneAssets, native_window_scene_runtime_with_mesh_assets};

pub(crate) type DesktopSceneHost = McloneSceneHost;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DesktopSceneHostOverrides {
    pub(crate) freeze_scheduled_fluid_ticks: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_desktop_scene_host(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    scene: &SceneOptions,
    render_options: TexturedSectionRenderOptions,
    assets: &WindowSceneAssets,
    asset_source: &impl mclone_assets::AssetSource,
    startup_view_pose: Option<XrStartupViewPose>,
) -> Result<DesktopSceneHost> {
    create_desktop_scene_host_with_overrides(
        device,
        queue,
        color_format,
        scene,
        render_options,
        assets,
        asset_source,
        startup_view_pose,
        DesktopSceneHostOverrides::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_desktop_scene_host_with_overrides(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    scene: &SceneOptions,
    render_options: TexturedSectionRenderOptions,
    assets: &WindowSceneAssets,
    asset_source: &impl mclone_assets::AssetSource,
    startup_view_pose: Option<XrStartupViewPose>,
    overrides: DesktopSceneHostOverrides,
) -> Result<DesktopSceneHost> {
    let mut host_scene = scene_host_options_from_desktop(scene)?;
    host_scene.freeze_scheduled_fluid_ticks = overrides.freeze_scheduled_fluid_ticks;
    let mut host = if scene.remote_addr.is_some() {
        let endpoint = RemoteSessionEndpoint::new(
            scene
                .remote_addr
                .clone()
                .expect("remote address presence checked"),
        );
        let runtime = NativeSessionServices::from_active_runtime(
            SessionStartRequest::JoinRemote {
                endpoint: endpoint.clone(),
            },
            native_window_scene_runtime_with_mesh_assets(scene, assets.mesh_assets.clone())?,
        )?;
        McloneSceneHost::with_runtime(
            device,
            queue,
            color_format,
            mclone_app_runtime::monotonic::system_monotonic_clock(),
            host_scene,
            runtime,
            render_options,
            assets.actor_textures.atlas.clone(),
            assets.actor_textures.figures.clone(),
            asset_source,
            startup_view_pose,
        )
    } else {
        McloneSceneHost::start_local_async(
            device,
            queue,
            color_format,
            mclone_app_runtime::monotonic::system_monotonic_clock(),
            host_scene,
            render_options,
            assets.mesh_assets.clone(),
            assets.actor_textures.atlas.clone(),
            assets.actor_textures.figures.clone(),
            asset_source,
            startup_view_pose,
        )
    }
    .context("initialize desktop Mono scene host")?;

    host.set_teleport_preview_capability(mclone_client::native_teleport_preview_capability());

    host.set_session_runtime_factory(|endpoint, scene, mesh_assets| {
        let desktop_scene = desktop_scene_options_for_remote(&endpoint, &scene);
        let runtime = native_window_scene_runtime_with_mesh_assets(&desktop_scene, mesh_assets)?;
        NativeSessionServices::from_active_runtime(
            SessionStartRequest::JoinRemote { endpoint },
            runtime,
        )
    });
    configure_desktop_asset_pack_sources(&mut host, scene.world_root.as_deref())?;
    Ok(host)
}

pub(crate) fn configure_desktop_asset_pack_sources(
    host: &mut McloneSceneHost,
    world_root: Option<&std::path::Path>,
) -> Result<()> {
    let reference = mclone_assets::SharedAssetSource::new(
        load_asset_source().context("reload native reference source for asset-pack discovery")?,
    );
    let Some(registry) = AssetPackSourceRegistry::discover_native_with_reference(reference)? else {
        return Ok(());
    };
    host.configure_asset_pack_sources(registry, reference_asset_pack_selection())?;
    if let Some(path) =
        mclone_app_runtime::asset_pack_preferences::native_asset_pack_preference_path(world_root)
    {
        host.configure_asset_pack_preference_storage(Box::new(
            mclone_app_runtime::asset_pack_preferences::FileAssetPackPreferenceStorage::new(path),
        ))?;
    }
    Ok(())
}

pub(crate) fn scene_host_options_from_desktop(
    scene: &SceneOptions,
) -> Result<McloneSceneHostOptions> {
    let startup = scene.to_startup_scene();
    McloneSceneHostOptions {
        startup,
        render_compile_worker_timing_enabled: scene.render_compile_worker_timing_enabled,
        simulation_cadence: scene.simulation_cadence,
        first_person_player_visible: scene.first_person_player_visible,
        use_initial_spawn_center: false,
        freeze_scheduled_fluid_ticks: false,
        adaptive_chunk_publication_budget: scene.adaptive_chunk_publication_budget,
        adaptive_render_admission_budget: scene.adaptive_render_admission_budget,
        startup_lod_prewarm: scene.startup_lod_prewarm,
        underwater_detection_mode: mclone_scene::XrUnderwaterDetectionMode::Midpoint,
        debug_ui_screen: None,
        skip_actors: false,
        world_root: scene.world_root.clone(),
        world_dir: scene.world_dir.clone(),
    }
    .validated()
}

fn desktop_scene_options_for_remote(
    endpoint: &RemoteSessionEndpoint,
    scene: &McloneSceneHostOptions,
) -> SceneOptions {
    SceneOptions {
        seed: scene.seed,
        chunk_x: scene.chunk_x,
        chunk_z: scene.chunk_z,
        render_distance: scene.render_distance as i32,
        render_compile_worker_count: scene.render_compile_worker_count,
        render_compile_max_pending_jobs: scene.render_compile_max_pending_jobs,
        render_compile_worker_timing_enabled: scene.render_compile_worker_timing_enabled,
        movement_speed_multiplier: scene.movement_speed_multiplier,
        simulation_cadence: scene.simulation_cadence,
        remote_addr: Some(endpoint.address.clone()),
        day_time_override: scene.day_time_override,
        freeze_time: scene.freeze_time,
        debug_passive_showcase: scene.debug_passive_showcase,
        first_person_player_visible: scene.first_person_player_visible,
        lighting_enabled: scene.lighting_enabled,
        light_status_batch_size: scene.light_status_batch_size,
        adaptive_chunk_publication_budget: scene.adaptive_chunk_publication_budget,
        adaptive_render_admission_budget: scene.adaptive_render_admission_budget,
        far_lod: scene.far_lod,
        startup_lod_prewarm: scene.startup_lod_prewarm,
        world_root: scene.world_root.clone(),
        world_dir: None,
        ..SceneOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mclone_app_runtime::far_lod::FarTerrainLodConfig;
    use mclone_server::SimulationCadenceConfig;

    use super::*;

    fn customized_scene() -> SceneOptions {
        SceneOptions {
            seed: -42,
            chunk_x: 7,
            chunk_z: -9,
            render_distance: 6,
            render_compile_worker_count: 3,
            render_compile_max_pending_jobs: Some(11),
            render_compile_worker_timing_enabled: false,
            movement_speed_multiplier: 1.25,
            simulation_cadence: SimulationCadenceConfig::new(30, 30, 60)
                .with_max_catch_up_host_frames(7),
            day_time_override: Some(13_000),
            freeze_time: true,
            first_person_player_visible: true,
            debug_passive_showcase: false,
            lighting_enabled: false,
            light_status_batch_size: 5,
            adaptive_chunk_publication_budget: false,
            adaptive_render_admission_budget: true,
            far_lod: FarTerrainLodConfig::enabled().with_extra_radius_chunks(8),
            startup_lod_prewarm: false,
            world_root: Some(PathBuf::from("world-root")),
            world_dir: Some(PathBuf::from("world-dir")),
            ..SceneOptions::default()
        }
    }

    #[test]
    fn desktop_scene_options_survive_mono_host_conversion() {
        let scene = customized_scene();
        let mono = scene_host_options_from_desktop(&scene).expect("valid Mono scene");

        assert_eq!(mono.seed, scene.seed);
        assert_eq!(mono.chunk_x, scene.chunk_x);
        assert_eq!(mono.chunk_z, scene.chunk_z);
        assert_eq!(mono.render_distance, scene.render_distance as u32);
        assert_eq!(
            mono.render_compile_worker_count,
            scene.render_compile_worker_count
        );
        assert_eq!(
            mono.render_compile_max_pending_jobs,
            scene.render_compile_max_pending_jobs
        );
        assert_eq!(
            mono.render_compile_worker_timing_enabled,
            scene.render_compile_worker_timing_enabled
        );
        assert_eq!(
            mono.movement_speed_multiplier,
            scene.movement_speed_multiplier
        );
        assert_eq!(mono.simulation_cadence, scene.simulation_cadence);
        assert_eq!(
            mono.first_person_player_visible,
            scene.first_person_player_visible
        );
        assert_eq!(mono.day_time_override, scene.day_time_override);
        assert_eq!(mono.freeze_time, scene.freeze_time);
        assert_eq!(mono.debug_passive_showcase, scene.debug_passive_showcase);
        assert!(!mono.use_initial_spawn_center);
        assert!(!mono.freeze_scheduled_fluid_ticks);
        assert_eq!(mono.lighting_enabled, scene.lighting_enabled);
        assert_eq!(mono.light_status_batch_size, scene.light_status_batch_size);
        assert_eq!(
            mono.adaptive_chunk_publication_budget,
            scene.adaptive_chunk_publication_budget
        );
        assert_eq!(
            mono.adaptive_render_admission_budget,
            scene.adaptive_render_admission_budget
        );
        assert_eq!(mono.far_lod, scene.far_lod);
        assert_eq!(mono.startup_lod_prewarm, scene.startup_lod_prewarm);
        assert_eq!(mono.world_root, scene.world_root);
        assert_eq!(mono.world_dir, scene.world_dir);
    }

    #[test]
    fn remote_runtime_scene_preserves_shared_host_options() {
        let scene = customized_scene();
        let mono = scene_host_options_from_desktop(&scene).expect("valid Mono scene");
        let endpoint = RemoteSessionEndpoint::new("127.0.0.1:25565");
        let remote = desktop_scene_options_for_remote(&endpoint, &mono);

        assert_eq!(remote.remote_addr.as_deref(), Some("127.0.0.1:25565"));
        assert_eq!(remote.simulation_cadence, scene.simulation_cadence);
        assert_eq!(
            remote.render_compile_worker_timing_enabled,
            scene.render_compile_worker_timing_enabled
        );
        assert_eq!(
            remote.first_person_player_visible,
            scene.first_person_player_visible
        );
        assert_eq!(remote.debug_passive_showcase, scene.debug_passive_showcase);
        assert_eq!(
            remote.adaptive_render_admission_budget,
            scene.adaptive_render_admission_budget
        );
        assert_eq!(remote.far_lod, scene.far_lod);
        assert_eq!(remote.startup_lod_prewarm, scene.startup_lod_prewarm);
        assert_eq!(remote.world_root, scene.world_root);
        assert_eq!(remote.world_dir, None);
    }
}
