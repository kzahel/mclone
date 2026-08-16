use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};
use mclone_core::Vec3d;
use mclone_input::{FlatInputFrame, KeyboardMouseInputAdapter};
use mclone_render::headless::{HeadlessStereoFrameOptions, write_headless_stereo_frame_png};
use mclone_render_session::{EngineCameraSnapshot, XrFov, XrView, XrViewPose};
use mclone_scene::{
    EmbeddedWorldActivationPhase, EmbeddedWorldActivationReport, EmbeddedWorldPreviewPhase,
    EmbeddedWorldPreviewSnapshot,
};

use crate::cli::{LobbyScenarioSmokeOptions, XrEmulationScreenshotOptions};
use crate::offscreen_scene_host::OffscreenDriver;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::WindowSceneAssets;

const XR_EMULATION_IPD_BLOCKS: f32 = 0.064;
const XR_EMULATION_VERTICAL_FOV_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const XR_EMULATION_FRAME_TIME: Duration = Duration::from_micros(16_667);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct XrEmulationScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) eye_width: u32,
    pub(crate) eye_height: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) eye_pixel_difference_count: usize,
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) gui_command_count: usize,
    pub(crate) ui_panel_composite_count: u64,
    pub(crate) embedded_preview: Option<EmbeddedWorldPreviewSnapshot>,
    pub(crate) embedded_activation_reports: Vec<EmbeddedWorldActivationReport>,
    pub(crate) embedded_activation_switch_reports: Vec<mclone_scene::WarmWorldSwitchReport>,
    pub(crate) seasonal_appearance_receipt_json: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LobbyScenarioStereoSmokeReport {
    pub(crate) path: PathBuf,
    pub(crate) eye_pixel_difference_count: usize,
    pub(crate) switch_count: u64,
}

pub(crate) fn run_lobby_scenario_stereo_smoke(
    options: &LobbyScenarioSmokeOptions,
) -> Result<LobbyScenarioStereoSmokeReport> {
    if options.directory.exists() {
        std::fs::remove_dir_all(&options.directory).with_context(|| {
            format!(
                "clear lobby scenario stereo smoke directory `{}`",
                options.directory.display()
            )
        })?;
    }
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "create lobby scenario stereo smoke directory `{}`",
            options.directory.display()
        )
    })?;
    let path = options.directory.join("returned-lobby-stereo.png");
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let (capture, (summary, switch_count, actor_receipt)) = write_headless_stereo_frame_png(
        HeadlessStereoFrameOptions {
            path: path.clone(),
            eye_width: options.width,
            eye_height: options.height,
        },
        move |device, queue, format, size, left_view, right_view| {
            let mut driver = OffscreenDriver::new_stereo_emulation(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
            )?;
            if let Some(registry) =
                mclone_app_runtime::prepared_assets::AssetPackSourceRegistry::discover_native_with_reference(
                    asset_source.clone(),
                )?
            {
                driver.host_mut().configure_asset_pack_sources(
                    registry,
                    mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
                )?;
            }
            let mut views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            driver.drive_stereo_until_view_settled(device, queue, views)?;
            driver
                .host_mut()
                .set_mono_ui_screen(Some(mclone_ui::GameScreen::Title));
            driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
            driver.render_stereo(device, queue, views, left_view, right_view)?;
            driver.host_mut().apply_xr_ui_action(
                mclone_ui::GameUiAction::EnterScenario(mclone_ui::GameScenarioId::LobbyPreview),
                device,
                queue,
            )?;

            let mut saw_playable_lobby = false;
            let mut saw_visible_preview = false;
            for _ in 0..480 {
                std::thread::sleep(Duration::from_millis(4));
                views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
                driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
                driver.render_stereo(device, queue, views, left_view, right_view)?;
                if driver.host().active_world_seed()
                    == mclone_server::AuthoredWorldFixtureKind::Table.seed()
                    && driver.host().local_startup_complete()
                {
                    saw_playable_lobby = true;
                    if driver.host().active_world_behavior_profile()
                        != mclone_server::WorldBehaviorProfile::ProtectedLobby
                    {
                        bail!("synthetic-stereo lobby did not retain protected authority");
                    }
                }
                if let Some(preview) = driver.host().embedded_world_preview_snapshot() {
                    if preview.phase == EmbeddedWorldPreviewPhase::Failed {
                        bail!("synthetic-stereo lobby preview failed: {preview:?}");
                    }
                    if preview.phase == EmbeddedWorldPreviewPhase::Visible {
                        saw_visible_preview = true;
                        break;
                    }
                }
            }
            if !saw_playable_lobby || !saw_visible_preview {
                bail!(
                    "synthetic-stereo menu launch did not reach playable lobby and visible preview: lobby={saw_playable_lobby} preview={saw_visible_preview}"
                );
            }
            views = synthetic_stereo_preview_views(&mut driver, size)?;
            driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
            driver.render_stereo(device, queue, views, left_view, right_view)?;
            if driver
                .host()
                .embedded_world_preview_snapshot()
                .is_none_or(|preview| preview.last_drawn_section_count == 0)
            {
                bail!("synthetic-stereo lobby preview produced no placed draws");
            }
            let (activation_reports, switch_reports) = drive_embedded_preview_activation_roundtrip(
                &mut driver,
                device,
                queue,
                size,
                left_view,
                right_view,
            )?;
            if activation_reports.len() != 2 || switch_reports.len() != 2 {
                bail!("synthetic-stereo lobby did not complete two activation legs");
            }
            if driver.host().active_world_seed()
                != mclone_server::AuthoredWorldFixtureKind::Table.seed()
                || driver.host().active_world_behavior_profile()
                    != mclone_server::WorldBehaviorProfile::ProtectedLobby
            {
                bail!("synthetic-stereo lobby round trip did not return to protected lobby");
            }
            views = synthetic_stereo_preview_views(&mut driver, size)?;
            driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
            let summary = driver.render_stereo(device, queue, views, left_view, right_view)?;
            let actor_receipt = driver
                .host()
                .embedded_world_preview_snapshot()
                .context("returned synthetic-stereo lobby lost its preview")?
                .render;
            Ok((summary, switch_reports.len() as u64, actor_receipt))
        },
    )?;
    if capture.non_clear_rgb_pixel_count == 0 || summary.drawn_section_count == 0 {
        bail!("synthetic-stereo lobby capture rendered no world pixels");
    }
    if capture.eye_pixel_difference_count == 0 {
        bail!("synthetic-stereo lobby eyes are pixel-identical");
    }
    let expected_actor_count = actor_receipt
        .last_actor_entity_count
        .saturating_add(actor_receipt.last_actor_remote_player_count)
        .saturating_add(actor_receipt.last_actor_source_local_player_count);
    if actor_receipt.last_actor_source_local_player_count != 0
        || actor_receipt.last_submitted_actor_count != expected_actor_count
        || actor_receipt.last_drawn_actor_count != expected_actor_count
        || actor_receipt.last_source_rejected_actor_count != 0
        || actor_receipt.last_clip_rejected_actor_count != 0
        || actor_receipt.last_frustum_rejected_actor_count != 0
    {
        bail!("synthetic-stereo lobby lost or duplicated a preview actor: {actor_receipt:?}");
    }
    let receipt = serde_json::json!({
        "schema": 1,
        "capture": path,
        "eyePixelDifferenceCount": capture.eye_pixel_difference_count,
        "switchCount": switch_count,
        "lobbyBehavior": "protectedLobby",
        "islandBehavior": "mutable",
        "previewActors": {
            "entities": actor_receipt.last_actor_entity_count,
            "remotePlayers": actor_receipt.last_actor_remote_player_count,
            "sourceLocalPlayers": actor_receipt.last_actor_source_local_player_count,
            "submitted": actor_receipt.last_submitted_actor_count,
            "drawn": actor_receipt.last_drawn_actor_count,
            "meshRebuilds": actor_receipt.actor_mesh_rebuild_count,
            "meshUploads": actor_receipt.actor_mesh_upload_count,
            "gpuCapacityBytes": actor_receipt.actor_gpu_capacity_bytes,
        },
    });
    std::fs::write(
        options.directory.join("report.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(LobbyScenarioStereoSmokeReport {
        path,
        eye_pixel_difference_count: capture.eye_pixel_difference_count,
        switch_count,
    })
}

pub(crate) fn run_xr_emulation_screenshot(
    options: &XrEmulationScreenshotOptions,
) -> Result<XrEmulationScreenshotReport> {
    let emulated_input = held_xr_emulation_input(&options.held_keys)?;
    let assets = WindowSceneAssets::load()?;
    let asset_source = mclone_assets::SharedAssetSource::new(load_asset_source()?);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let season_preview = options.season_preview;
    let input_frames = options.input_frames;
    let (
        capture,
        (
            summary,
            embedded_preview,
            embedded_activation_reports,
            embedded_activation_switch_reports,
            seasonal_diagnostics,
        ),
    ) = write_headless_stereo_frame_png(
        HeadlessStereoFrameOptions {
            path: options.path.clone(),
            eye_width: options.eye_width,
            eye_height: options.eye_height,
        },
        move |device, queue, format, size, left_view, right_view| {
            let mut driver = OffscreenDriver::new_stereo_emulation(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
            )?;
            driver
                .host_mut()
                .set_season_preview_settings(season_preview);
            if let Some(registry) = mclone_app_runtime::prepared_assets::AssetPackSourceRegistry::discover_native_with_reference(asset_source.clone())? {
                driver.host_mut().configure_asset_pack_sources(
                    registry,
                    mclone_app_runtime::prepared_assets::reference_asset_pack_selection(),
                )?;
                if let Some(path) = mclone_app_runtime::asset_pack_preferences::native_asset_pack_preference_path(scene.world_root.as_deref()) {
                    driver.host_mut().configure_asset_pack_preference_storage(Box::new(
                        mclone_app_runtime::asset_pack_preferences::FileAssetPackPreferenceStorage::new(path),
                    ))?;
                }
            }
            let mut views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            driver.drive_stereo_until_view_settled(device, queue, views)?;
            if scene.startup.terrain_presentation
                == mclone_app_runtime::startup_args::TerrainPresentationMode::Composed
            {
                drive_terrain_horizon_until_ready(
                    &mut driver,
                    device,
                    queue,
                    size,
                    left_view,
                    right_view,
                )?;
            }
            drive_embedded_preview_until_visible(
                &mut driver,
                device,
                queue,
                size,
                left_view,
                right_view,
            )?;
            let (embedded_activation_reports, embedded_activation_switch_reports) =
                drive_embedded_preview_activation_roundtrip(
                    &mut driver,
                    device,
                    queue,
                    size,
                    left_view,
                    right_view,
                )?;
            drive_warm_world_swap_roundtrip_if_requested(
                &mut driver,
                device,
                queue,
                size,
                left_view,
                right_view,
            )?;
            views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            drive_asset_replacement_roundtrip_if_requested(
                &mut driver,
                device,
                queue,
                views,
                left_view,
                right_view,
            )?;

            if let Some(frame) = emulated_input {
                // If startup leaves an XR screen open, close it through the
                // same controller edge used on headset before exercising held
                // locomotion input. The final block below opens the pause panel
                // so every capture proves world-quad composition.
                if driver.stereo_ui_is_active() {
                    apply_menu_toggle(&mut driver, views)?;
                }
                for _ in 0..input_frames {
                    std::thread::sleep(XR_EMULATION_FRAME_TIME);
                    views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
                    driver.apply_stereo_input_frame(frame, views)?;
                    driver.render_stereo(device, queue, views, left_view, right_view)?;
                }
                views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            }
            if options.pause_panel
                && driver.host().embedded_world_preview_snapshot().is_none()
                && !driver.stereo_ui_is_active()
            {
                apply_menu_toggle(&mut driver, views)?;
            }

            views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
            if driver.host().embedded_world_preview_snapshot().is_some() {
                views = synthetic_stereo_preview_views(&mut driver, size)?;
            } else if driver.host().warm_world_standby_snapshot().is_some() {
                views = synthetic_stereo_gate_views(&mut driver, size, 3.0)?;
            }
            let summary = driver.render_stereo(device, queue, views, left_view, right_view)?;
            Ok((
                summary,
                driver.host().embedded_world_preview_snapshot(),
                embedded_activation_reports,
                embedded_activation_switch_reports,
                driver.host().solar_frame_diagnostics(),
            ))
        },
    )?;

    if capture.non_clear_rgb_pixel_count == 0 || summary.drawn_section_count == 0 {
        bail!("XR emulation capture rendered no world pixels");
    }
    if capture.eye_pixel_difference_count == 0 {
        bail!("XR emulation eyes are pixel-identical; stereo parallax was not preserved");
    }
    if embedded_preview.is_none() {
        let ui_mismatch = if options.pause_panel {
            !summary.ui_active
                || summary.gui_command_count == 0
                || summary.ui_panel.composite_count < 2
        } else {
            summary.ui_active
                || summary.gui_command_count != 0
                || summary.ui_panel.composite_count != 0
        };
        if ui_mismatch {
            bail!(
                "XR emulation capture did not preserve the requested world UI state: pause_panel={} active={} commands={} composites={}",
                options.pause_panel,
                summary.ui_active,
                summary.gui_command_count,
                summary.ui_panel.composite_count
            );
        }
    }
    let seasonal_appearance_receipt_json =
        crate::offscreen_flat_client::seasonal_appearance_receipt_json(
            options.season_preview,
            options.scene.startup.world_generation_profile,
            seasonal_diagnostics,
        )?;

    Ok(XrEmulationScreenshotReport {
        path: capture.path,
        eye_width: capture.eye_width,
        eye_height: capture.eye_height,
        width: capture.width,
        height: capture.height,
        byte_len: capture.byte_len,
        eye_pixel_difference_count: capture.eye_pixel_difference_count,
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        gui_command_count: summary.gui_command_count,
        ui_panel_composite_count: summary.ui_panel.composite_count,
        embedded_preview,
        embedded_activation_reports,
        embedded_activation_switch_reports,
        seasonal_appearance_receipt_json,
    })
}

fn drive_embedded_preview_activation_roundtrip(
    driver: &mut OffscreenDriver,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    left_view: &wgpu::TextureView,
    right_view: &wgpu::TextureView,
) -> Result<(
    Vec<EmbeddedWorldActivationReport>,
    Vec<mclone_scene::WarmWorldSwitchReport>,
)> {
    if driver.host().embedded_world_preview_snapshot().is_none() {
        return Ok((Vec::new(), Vec::new()));
    }
    let source_world = driver.host().active_world_instance_id();
    let mut reports = Vec::with_capacity(2);
    let mut switches = Vec::with_capacity(2);
    let mut destination_world = None;
    for leg in 0..2 {
        let views = synthetic_stereo_preview_views(driver, size)?;
        driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
        driver.apply_stereo_input_frame(
            FlatInputFrame {
                use_item: true,
                ..FlatInputFrame::default()
            },
            views,
        )?;
        let requested = driver.host().embedded_world_activation_snapshot();
        if requested.phase != EmbeddedWorldActivationPhase::Closing {
            bail!("synthetic-stereo diorama Use did not begin a closing blink: {requested:?}");
        }

        let expected_source = driver.host().active_world_instance_id();
        let mut completed = None;
        let mut last_snapshot = driver.host().embedded_world_activation_snapshot();
        for _ in 0..120 {
            std::thread::sleep(XR_EMULATION_FRAME_TIME);
            let views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
            driver.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
            driver.render_stereo(device, queue, views, left_view, right_view)?;
            let snapshot = driver.host().embedded_world_activation_snapshot();
            last_snapshot = snapshot.clone();
            if snapshot.phase == EmbeddedWorldActivationPhase::Failed {
                bail!("synthetic-stereo diorama activation failed: {snapshot:?}");
            }
            if snapshot.phase == EmbeddedWorldActivationPhase::Idle {
                completed = snapshot.last_report;
                break;
            }
        }
        let report = completed.with_context(|| {
            format!(
                "synthetic-stereo diorama activation did not complete within 120 rendered frames: activation={last_snapshot:?} standby={:?} preview={:?}",
                driver.host().warm_world_standby_snapshot(),
                driver.host().embedded_world_preview_snapshot(),
            )
        })?;
        let active_world = driver.host().active_world_instance_id();
        if active_world == expected_source {
            bail!("synthetic-stereo diorama activation retained its source world");
        }
        validate_stereo_embedded_world_activation(&report, expected_source, active_world)?;
        switches.push(
            driver
                .host()
                .last_warm_world_switch_report()
                .context("synthetic-stereo activation completed without switch facts")?,
        );
        if leg == 0 {
            destination_world = Some(active_world);
        }
        drive_embedded_preview_until_visible(driver, device, queue, size, left_view, right_view)?;
        reports.push(report);
    }
    let destination_world = destination_world.context("diorama roundtrip lost destination id")?;
    if driver.host().active_world_instance_id() != source_world
        || reports[0].source_world != source_world
        || reports[0].destination_world != destination_world
        || reports[1].source_world != destination_world
        || reports[1].destination_world != source_world
    {
        bail!("synthetic-stereo diorama activation lost A-to-B-to-A identity");
    }
    eprintln!(
        "live_diorama_activation_stereo A={} B={} first_ms={:.3} return_ms={:.3}",
        source_world.get(),
        destination_world.get(),
        reports[0].switch_elapsed_ms.unwrap_or_default(),
        reports[1].switch_elapsed_ms.unwrap_or_default(),
    );
    Ok((reports, switches))
}

fn validate_stereo_embedded_world_activation(
    report: &EmbeddedWorldActivationReport,
    expected_source: mclone_scene::WorldInstanceId,
    expected_destination: mclone_scene::WorldInstanceId,
) -> Result<()> {
    if report.source_world != expected_source
        || report.destination_world != expected_destination
        || report.switch_elapsed_ms.is_none()
        || report.covered_rendered_frames == 0
        || report.first_uncovered_world != Some(expected_destination)
        || report.first_uncovered_drawn_section_count == 0
        || report.first_uncovered_uploaded_section_count != 0
        || report.first_uncovered_submitted_compile_section_count != 0
        || report.first_uncovered_accepted_compile_result_count != 0
        || report.first_uncovered_eye_count != 2
        || !report
            .post_swap_entry
            .is_some_and(|sample| sample.support.supported())
        || !report
            .first_uncovered_entry
            .is_some_and(|sample| sample.support.supported())
        || !report
            .stability_entry
            .is_some_and(|sample| sample.support.supported())
        || report.stability_activation_frame.is_none()
        || report.completed_activation_frame.is_none()
        || report.failure.is_some()
    {
        bail!(
            "synthetic-stereo embedded-world activation violated cover/conservation facts: {report:?}"
        );
    }
    Ok(())
}

fn drive_terrain_horizon_until_ready(
    driver: &mut OffscreenDriver,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    left_view: &wgpu::TextureView,
    right_view: &wgpu::TextureView,
) -> Result<()> {
    let mut last = None;
    for _ in 0..360 {
        if last.is_some_and(|diagnostics: mclone_scene::SceneTerrainViewDiagnostics| {
            diagnostics.target_ready && diagnostics.drawn_tiles > 0
        }) {
            return Ok(());
        }
        std::thread::sleep(XR_EMULATION_FRAME_TIME);
        let views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
        driver.render_stereo(device, queue, views, left_view, right_view)?;
        last = driver.host().terrain_view_diagnostics();
    }
    bail!(
        "synthetic-stereo composed horizon did not become ready within 360 rendered frames: \
         preference={:?} source_supported={} diagnostics={last:?}",
        driver.host().terrain_presentation_preference(),
        driver.host().terrain_presentation_supported(),
    )
}

fn drive_embedded_preview_until_visible(
    driver: &mut OffscreenDriver,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    left_view: &wgpu::TextureView,
    right_view: &wgpu::TextureView,
) -> Result<()> {
    if driver.host().embedded_world_preview_snapshot().is_none() {
        return Ok(());
    }
    for _ in 0..240 {
        let preview = driver
            .host()
            .embedded_world_preview_snapshot()
            .context("embedded preview disappeared during stereo warmup")?;
        match preview.phase {
            EmbeddedWorldPreviewPhase::Visible => return Ok(()),
            EmbeddedWorldPreviewPhase::Failed => {
                bail!("embedded preview failed during stereo warmup: {preview:?}")
            }
            EmbeddedWorldPreviewPhase::Warming => {}
        }
        let views = synthetic_stereo_views(driver.host().camera_snapshot(), size);
        driver.render_stereo(device, queue, views, left_view, right_view)?;
    }
    bail!("embedded preview did not become visible within 240 stereo frames")
}

fn drive_warm_world_swap_roundtrip_if_requested(
    driver: &mut OffscreenDriver,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    left_view: &wgpu::TextureView,
    right_view: &wgpu::TextureView,
) -> Result<()> {
    if driver.host().warm_world_standby_snapshot().is_none()
        || driver.host().embedded_world_preview_snapshot().is_some()
    {
        return Ok(());
    }
    let source_id = driver.host().active_world_instance_id();
    let initial_crossing_count = driver
        .host()
        .world_gate_snapshot()
        .context("synthetic-stereo warm world has no paired gate")?
        .crossing_count;

    // Exercise the same visual-midpoint rule used on headset. The first live
    // stereo frame approaches and arms the gate; the second places both eyes'
    // midpoint just beyond the plane and must perform the ownership exchange
    // before drawing either eye.
    let views = synthetic_stereo_gate_views(driver, size, 1.0)?;
    driver.render_stereo(device, queue, views, left_view, right_view)?;
    let views = synthetic_stereo_gate_views(driver, size, -0.3)?;
    let destination = driver.render_stereo(device, queue, views, left_view, right_view)?;
    let destination_id = driver.host().active_world_instance_id();
    if destination_id == source_id {
        bail!("synthetic-stereo gate crossing retained the source identity");
    }
    let first = driver
        .host()
        .last_warm_world_switch_report()
        .context("synthetic-stereo gate crossing produced no switch report")?;
    validate_stereo_warm_world_switch(&first, destination.drawn_section_count)?;

    let views = synthetic_stereo_gate_views(driver, size, 1.0)?;
    driver.render_stereo(device, queue, views, left_view, right_view)?;
    let views = synthetic_stereo_gate_views(driver, size, -0.3)?;
    let source = driver.render_stereo(device, queue, views, left_view, right_view)?;
    if driver.host().active_world_instance_id() != source_id {
        bail!("synthetic-stereo gate round trip lost the source identity");
    }
    let second = driver
        .host()
        .last_warm_world_switch_report()
        .context("synthetic-stereo return crossing produced no switch report")?;
    validate_stereo_warm_world_switch(&second, source.drawn_section_count)?;
    let gate = driver
        .host()
        .world_gate_snapshot()
        .context("synthetic-stereo round trip lost its gate")?;
    if gate.crossing_count != initial_crossing_count + 2 || gate.armed {
        bail!("synthetic-stereo gate round trip retained invalid hysteresis state: {gate:?}");
    }
    if first.source_instance_id != source_id
        || first.destination_instance_id != destination_id
        || second.source_instance_id != destination_id
        || second.destination_instance_id != source_id
    {
        bail!("synthetic-stereo warm-world switch reports lost A-to-B-to-A identity");
    }
    eprintln!(
        "warm_world_gate_stereo A={} B={} crossings=2 first_drawn={} return_drawn={} first_ms={:.3} return_ms={:.3}",
        first.source_seed,
        first.destination_seed,
        destination.drawn_section_count,
        source.drawn_section_count,
        first.switch_elapsed_ms,
        second.switch_elapsed_ms,
    );
    Ok(())
}

fn synthetic_stereo_preview_views(
    driver: &mut OffscreenDriver,
    size: [u32; 2],
) -> Result<[XrView; 2]> {
    let preview = driver
        .host()
        .embedded_world_preview_snapshot()
        .context("synthetic-stereo embedded preview placement is unavailable")?;
    let target = preview
        .placement
        .composition_anchor()
        .add(Vec3d::new(0.0, 0.25, 0.0));
    let eye = target.add(Vec3d::new(0.0, 2.0, -6.0));
    let forward = target.subtract(eye);
    let length = (forward.x * forward.x + forward.y * forward.y + forward.z * forward.z).sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        bail!("synthetic-stereo embedded preview camera has no direction");
    }
    let forward = forward.scale(1.0 / length);
    // XR's tracked forward axis is -Z; use the stage-to-world yaw convention
    // rather than the flat spectator camera's +Z convention.
    let yaw_radians = (-forward.x).atan2(-forward.z);
    let pitch_radians = forward.y.clamp(-1.0, 1.0).asin();
    let speed = driver.host().camera_snapshot().speed_blocks_per_second;
    driver
        .host_mut()
        .set_mono_capture_camera(eye, yaw_radians, pitch_radians, speed);
    Ok(synthetic_stereo_views(
        driver.host().camera_snapshot(),
        size,
    ))
}

fn synthetic_stereo_gate_views(
    driver: &mut OffscreenDriver,
    size: [u32; 2],
    oriented_distance: f64,
) -> Result<[XrView; 2]> {
    let gate = driver
        .host()
        .world_gate_snapshot()
        .context("synthetic-stereo gate placement is unavailable")?;
    let approach_sign = match gate.direction {
        mclone_scene::WorldGateSwitchDirection::PositiveToNegative => 1.0,
        mclone_scene::WorldGateSwitchDirection::NegativeToPositive => -1.0,
    };
    let offset = gate
        .active_endpoint
        .normal
        .scale(approach_sign * oriented_distance);
    let eye = gate
        .active_endpoint
        .feet_position
        .add(offset)
        .add(Vec3d::new(
            0.0,
            mclone_client::LOCAL_PLAYER_STANDING_EYE_HEIGHT,
            0.0,
        ));
    let forward = gate.active_endpoint.normal.scale(-approach_sign);
    // XR's tracked forward axis is -Z, so use the same yaw convention as
    // XrTrackingOrigin rather than the flat-camera movement convention.
    let yaw_radians = (-forward.x).atan2(-forward.z);
    let speed = driver.host().camera_snapshot().speed_blocks_per_second;
    driver
        .host_mut()
        .set_mono_player_camera(eye, yaw_radians, 0.0, speed);
    Ok(synthetic_stereo_views(
        driver.host().camera_snapshot(),
        size,
    ))
}

fn validate_stereo_warm_world_switch(
    report: &mclone_scene::WarmWorldSwitchReport,
    drawn_section_count: usize,
) -> Result<()> {
    if report.first_drawable_destination_frame != Some(1)
        || report.first_drawn_section_count == 0
        || drawn_section_count == 0
        || report.switch_uploaded_section_count != 0
        || report.switch_submitted_compile_section_count != 0
        || report.switch_accepted_compile_result_count != 0
        || report.switch_materialized_renderer
    {
        bail!(
            "synthetic-stereo warm-world switch {} violated next-frame/conservation facts: frame={:?} reported_drawn={} rendered_drawn={} uploaded={} submitted={} accepted={} materialized={}",
            report.sequence,
            report.first_drawable_destination_frame,
            report.first_drawn_section_count,
            drawn_section_count,
            report.switch_uploaded_section_count,
            report.switch_submitted_compile_section_count,
            report.switch_accepted_compile_result_count,
            report.switch_materialized_renderer,
        );
    }
    Ok(())
}

fn drive_asset_replacement_roundtrip_if_requested(
    driver: &mut OffscreenDriver,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    views: [XrView; 2],
    left_view: &wgpu::TextureView,
    right_view: &wgpu::TextureView,
) -> Result<()> {
    let authored = std::env::var_os("MCLONE_ASSET_REPLACEMENT_AUTHORED").map(PathBuf::from);
    let fallback = std::env::var_os("MCLONE_ASSET_REPLACEMENT_FALLBACK").map(PathBuf::from);
    let (authored, fallback) = match (authored, fallback) {
        (Some(authored), Some(fallback)) => (authored, fallback),
        (None, None) => return Ok(()),
        _ => {
            bail!(
                "MCLONE_ASSET_REPLACEMENT_AUTHORED and MCLONE_ASSET_REPLACEMENT_FALLBACK must be set together"
            );
        }
    };

    let restore = driver.host().active_asset_snapshot_for_epoch(2);
    let request =
        mclone_app_runtime::prepared_assets::PreparedSceneAssetsRequest::first_party_from_files(
            1, authored, fallback,
        )?;
    driver.host_mut().begin_asset_replacement(request)?;
    let mut restored = false;
    for _ in 0..180 {
        driver.render_stereo_frozen(device, queue, views, left_view, right_view)?;
        match driver.host().asset_replacement_status() {
            mclone_scene::AssetReplacementStatus::Active { epoch: 1 } if !restored => {
                driver
                    .host_mut()
                    .begin_prepared_asset_replacement(restore.clone())?;
                restored = true;
            }
            mclone_scene::AssetReplacementStatus::Active { epoch: 2 } => {
                let report = driver
                    .host_mut()
                    .take_last_asset_replacement_commit()
                    .context("XR asset replacement committed without a report")?;
                if !report.session_preserved
                    || !report.camera_preserved
                    || !report.command_count_unchanged
                    || !report.update_count_unchanged
                {
                    bail!("XR asset replacement mutated session/camera/runtime facts");
                }
                println!(
                    "XR asset replacement smoke completed epochs 0 -> 1 -> 2 with {} sections",
                    report.section_count
                );
                return Ok(());
            }
            mclone_scene::AssetReplacementStatus::Failed {
                active_epoch,
                message,
            } => {
                bail!(
                    "XR asset replacement smoke failed with active epoch {active_epoch} retained: {message}"
                );
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!("XR asset replacement smoke did not complete within 180 frames")
}

fn held_xr_emulation_input(keys: &[mclone_input::KeyboardKey]) -> Result<Option<FlatInputFrame>> {
    if keys.is_empty() {
        return Ok(None);
    }
    let mut keyboard = KeyboardMouseInputAdapter::new();
    for &key in keys {
        let event = keyboard.handle_key(key, true, false);
        if !event.handled || event.frame.is_some() {
            bail!(
                "--xr-emulation-key `{}` is not a held locomotion binding; use WASD, arrows, Space, KeyX, Shift, or Control",
                key.label()
            );
        }
    }
    keyboard
        .held_frame()
        .map(Some)
        .context("--xr-emulation-key values did not produce held locomotion input")
}

fn apply_menu_toggle(driver: &mut OffscreenDriver, views: [XrView; 2]) -> Result<()> {
    driver.apply_stereo_input_frame(
        FlatInputFrame {
            open_menu: true,
            ..FlatInputFrame::default()
        },
        views,
    )?;
    driver.apply_stereo_input_frame(FlatInputFrame::default(), views)
}

fn synthetic_stereo_views(snapshot: EngineCameraSnapshot, size: [u32; 2]) -> [XrView; 2] {
    // Stage-space head translation stays fixed. The shared scene transform maps
    // it onto the engine camera/player root, including locomotion and snap-turn.
    // Pitch is the one head-relative component not carried by that yaw/root
    // transform, so project it into the synthetic pose directly.
    let pitch = if snapshot.pitch_radians.is_finite() {
        snapshot.pitch_radians as f32
    } else {
        0.0
    };
    let orientation = Quat::from_rotation_x(pitch);
    let half_ipd = orientation * Vec3::X * (XR_EMULATION_IPD_BLOCKS * 0.5);
    let fov = symmetric_fov(size);
    [
        XrView {
            pose: XrViewPose {
                position: -half_ipd,
                orientation,
            },
            fov,
        },
        XrView {
            pose: XrViewPose {
                position: half_ipd,
                orientation,
            },
            fov,
        },
    ]
}

fn symmetric_fov(size: [u32; 2]) -> XrFov {
    let aspect = size[0].max(1) as f32 / size[1].max(1) as f32;
    let tan_y = (XR_EMULATION_VERTICAL_FOV_RADIANS * 0.5).tan();
    let angle_y = tan_y.atan();
    let angle_x = (tan_y * aspect).atan();
    XrFov {
        angle_left: -angle_x,
        angle_right: angle_x,
        angle_up: angle_y,
        angle_down: -angle_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{ChunkPos, Vec3d};

    #[test]
    fn synthetic_views_use_fixed_ipd_and_symmetric_fov() {
        let views = synthetic_stereo_views(test_snapshot(), [1200, 1000]);

        assert!(
            ((views[1].pose.position - views[0].pose.position).length() - XR_EMULATION_IPD_BLOCKS)
                .abs()
                < 1.0e-6
        );
        assert_eq!(views[0].fov, views[1].fov);
        assert_eq!(views[0].fov.angle_left, -views[0].fov.angle_right);
        assert_eq!(views[0].fov.angle_down, -views[0].fov.angle_up);
        assert!(views[0].fov.angle_right > views[0].fov.angle_up);
    }

    #[test]
    fn scene_tracking_maps_synthetic_head_to_engine_camera() {
        let snapshot = test_snapshot();
        let views = synthetic_stereo_views(snapshot, [1000, 1000]);
        let origin = mclone_scene::XrTrackingOrigin::from_initial_views(
            &views,
            mclone_scene::XrViewAlignmentMode::PlayerSpawn,
        )
        .unwrap();
        let transform =
            mclone_scene::XrStageToWorld::from_tracking_origin(origin, snapshot).unwrap();
        let left = transform.transform_position(views[0].pose.position);
        let right = transform.transform_position(views[1].pose.position);
        let center = (left + right) * 0.5;
        let expected_center = Vec3::new(
            snapshot.eye.x as f32,
            snapshot.eye.y as f32,
            snapshot.eye.z as f32,
        );
        assert!((center - expected_center).length() < 1.0e-5);
        let expected_eye_axis = Quat::from_rotation_y(snapshot.yaw_radians as f32) * Vec3::X;
        assert!(
            ((right - left).normalize() - expected_eye_axis).length() < 1.0e-5,
            "engine yaw should rotate the synthetic eye axis"
        );
    }

    #[test]
    fn emulation_keys_accept_only_held_locomotion_bindings() {
        let frame = held_xr_emulation_input(&[
            mclone_input::KeyboardKey::KeyW,
            mclone_input::KeyboardKey::ArrowLeft,
        ])
        .unwrap()
        .expect("held frame");
        assert!(frame.forward);
        assert_eq!(frame.keyboard_turn, 1.0);

        let error = held_xr_emulation_input(&[mclone_input::KeyboardKey::Escape]).unwrap_err();
        assert!(error.to_string().contains("not a held locomotion binding"));
    }

    fn test_snapshot() -> EngineCameraSnapshot {
        EngineCameraSnapshot {
            eye: Vec3d::new(12.0, 64.0, -7.0),
            yaw_radians: 1.2,
            pitch_radians: 0.25,
            speed_blocks_per_second: 4.0,
            chunk_pos: ChunkPos::new(0, 0),
        }
    }
}
