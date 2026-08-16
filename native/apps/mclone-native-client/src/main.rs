#[cfg(test)]
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod actor_assets;
mod app;
mod camera;
mod cli;
mod desktop_gamepad;
mod desktop_scene_host;
#[cfg(feature = "xr")]
mod desktop_xr;
mod frame_pacing;
mod headless;
mod live_diorama_smoke;
mod offscreen_flat_client;
mod offscreen_scene_host;
mod perf;
mod remote_player_visual_smoke;
mod render_cache;
mod render_compile_capacity;
mod scene_runtime;
mod torch_light_probe;
mod winit_frame_driver;
mod worldgen_showcase;
mod xr_emulation;

use crate::app::run_window;
#[cfg(test)]
use crate::camera::SPECTATOR_BASE_SPEED;
use crate::cli::Cli;
#[cfg(test)]
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, HeadlessActorReviewSheetOptions,
    HeadlessActorWalkReviewOptions, HeadlessDualViewOptions, HeadlessScreenshotOptions,
    HeadlessScreenshotUi, LoadingSettlePerfOptions, MovementPerfOptions,
    RemotePlayerVisualSmokeOptions, RendererRebuildSmokeOptions, SceneOptions,
    StartupStreamingPerfOptions, TimedemoOptions, TorchLightProbeOptions, WindowFrameReportOptions,
    WindowStartIntent, XrClearSmokeOptions, XrEmulationScreenshotOptions, XrMcloneSmokeOptions,
    XrUnderwaterMode, XrViewPose, parse_screenshot_ui_arg,
};
use crate::headless::{
    run_headless_screenshot, run_renderer_rebuild_smoke, write_actor_review_sheet,
    write_actor_walk_review, write_headless_dual_view,
};
use crate::live_diorama_smoke::run_live_diorama_smoke;
use crate::offscreen_flat_client::run_lobby_scenario_smoke;
use crate::offscreen_flat_client::run_offscreen_warm_world_swap_smoke;
use crate::perf::{
    run_frame_budget_probe, run_loading_settle_perf, run_movement_perf_smoke,
    run_startup_streaming_perf, run_timedemo,
};
use crate::remote_player_visual_smoke::run_remote_player_visual_smoke;
use crate::torch_light_probe::run_torch_light_probe;
use crate::worldgen_showcase::run_worldgen_showcase;
use crate::xr_emulation::run_lobby_scenario_stereo_smoke;
use crate::xr_emulation::run_xr_emulation_screenshot;
use anyhow::Result;
#[cfg(test)]
use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkPos, ChunkSnapshot};
#[cfg(test)]
use mclone_render::chunk::TexturedSectionRenderOptions;
#[cfg(test)]
use mclone_render::color_profile::RenderColorProfile;
use mclone_render::headless::{HeadlessClearOptions, write_headless_clear_png};
#[cfg(test)]
use mclone_render_session::snapshot_mesh_block_state_ids;

#[cfg(target_os = "macos")]
embed_plist::embed_info_plist!("../Info.plist");

const MIN_RENDER_DISTANCE: i32 = 2;
const MAX_RENDER_DISTANCE: i32 = 32;
const DEFAULT_MOVEMENT_PERF_STEPS: usize = 12;
const DEFAULT_MOVEMENT_PERF_PATH_RADIUS: i32 = 4;
const DEFAULT_TIMEDEMO_FRAMES: usize = 120;
const DEFAULT_TIMEDEMO_PATH_RADIUS: i32 = 4;
const DEFAULT_FRAME_BUDGET_PROBE_FRAMES: usize = 240;
const DEFAULT_STARTUP_STREAMING_PERF_FRAMES: usize = 2400;
const DEFAULT_FRAME_BUDGET_TARGET_HZ: f64 = 120.0;
const MAX_MOVEMENT_PERF_STEPS: usize = 512;
const MAX_TIMEDEMO_FRAMES: usize = 4096;
const MAX_FRAME_BUDGET_PROBE_FRAMES: usize = 4096;
const MAX_STARTUP_STREAMING_PERF_FRAMES: usize = 72000;
const MAX_MOVEMENT_PERF_PATH_RADIUS: i32 = 128;
const MAX_LOADING_SETTLE_DISTANCE_COUNT: usize = 16;

fn main() -> Result<()> {
    env_logger::init();
    match Cli::parse(std::env::args().skip(1))? {
        Cli::HeadlessClear {
            path,
            width,
            height,
        } => {
            let report = write_headless_clear_png(HeadlessClearOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
            })?;
            println!(
                "headless clear saved to {} ({}x{}, {} bytes)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len
            );
            Ok(())
        }
        Cli::HeadlessScreenshot { options } => {
            let report = run_headless_screenshot(&options)?;
            println!(
                "headless full-frame screenshot saved to {} ({}x{}, {} bytes, {} sections, {} drawn sections, {} GUI commands, {} flat HUD retained rebuilds, {} flat HUD retained cache hits, {} remote players, {} entities, {} actors, {} drawn actors, underwater={})",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.section_count,
                report.drawn_section_count,
                report.gui_command_count,
                report.flat_hud_retained_rebuild_count,
                report.flat_hud_retained_cache_hit_count,
                report.remote_player_count,
                report.entity_count,
                report.actor_count,
                report.drawn_actor_count,
                report.underwater
            );
            let terrain_view = report.terrain_view.map_or_else(
                || serde_json::json!({ "enabled": false }),
                |terrain| {
                    serde_json::json!({
                        "enabled": true,
                        "sourceGeneration": terrain.source_generation,
                        "coverageGeneration": terrain.coverage_generation,
                        "exactColumnCount": terrain.exact_column_count,
                        "exactCenterReady": terrain.exact_center_ready,
                        "exactHandoffTopology": terrain.exact_handoff_topology.label(),
                        "lastFrameRevision": terrain.last_frame_revision,
                        "readySlots": terrain.ready_slots,
                        "drawnLevels": terrain.drawn_levels,
                        "drawnTiles": terrain.drawn_tiles,
                        "vertexCount": terrain.vertex_count,
                        "fixedResidentBytes": terrain.fixed_resident_bytes,
                        "residentBytes": terrain.resident_bytes,
                        "exactConnectorSegments": terrain.exact_connector_segments,
                        "exactConnectorVertexCount": terrain.exact_connector_vertex_count,
                        "exactConnectorBytes": terrain.exact_connector_bytes,
                        "exactTransitionPreparationMicros": terrain.exact_transition_preparation_micros,
                        "exactTransitionPayloadBytes": terrain.exact_transition_payload_bytes,
                        "drawnTreeTiles": terrain.drawn_tree_tiles,
                        "drawnTreeInstances": terrain.drawn_tree_instances,
                        "targetReady": terrain.target_ready,
                        "treeInstanceCount": terrain.tree_instance_count,
                        "pendingVegetationTiles": terrain.pending_vegetation_tiles,
                        "vegetationEnabled": terrain.vegetation_enabled,
                        "vegetationResidentTiles": terrain.vegetation_resident_tiles,
                        "vegetationSubmittedJobs": terrain.vegetation_submitted_jobs,
                        "vegetationCompletedJobs": terrain.vegetation_completed_jobs,
                        "vegetationTransportFailures": terrain.vegetation_transport_failures,
                        "vegetationJobFailures": terrain.vegetation_job_failures,
                    })
                },
            );
            println!("MCLONE_TERRAIN_SEAM_STATE {terrain_view}");
            println!(
                "MCLONE_SEASONAL_APPEARANCE_STATE {}",
                report.seasonal_appearance_receipt_json
            );
            println!("MCLONE_CELESTIAL_STATE {}", report.celestial_receipt_json);
            Ok(())
        }
        Cli::WorldgenShowcase { options } => {
            let report = run_worldgen_showcase(&options)?;
            println!(
                "worldgen showcase card saved to {} (profile={}, seed={}, card={}x{}, panels={}x{}, views={}, receipt={})",
                report.card_path.display(),
                report.profile,
                report.seed,
                report.card_width,
                report.card_height,
                report.panel_width,
                report.panel_height,
                report.panels.len(),
                report.receipt_path.display(),
            );
            Ok(())
        }
        Cli::LiveDioramaSmoke { options } => {
            let report = run_live_diorama_smoke(&options)?;
            println!(
                "live world diorama smoke saved to {} (changed pixels={}, B sections={}, B indices={}, stereo eye differences={})",
                report.directory.display(),
                report.active_preview_pixel_difference_count,
                report.preview_drawn_section_count,
                report.preview_drawn_index_count,
                report.stereo_eye_pixel_difference_count,
            );
            Ok(())
        }
        Cli::LobbyScenarioSmoke { options } => {
            let report = run_lobby_scenario_smoke(&options)?;
            println!(
                "lobby scenario smoke saved to {} (captures={}, switches={})",
                report.directory.display(),
                report.capture_count,
                report.switch_count,
            );
            Ok(())
        }
        Cli::LobbyScenarioCatalogSmoke { options } => {
            let report = run_lobby_scenario_smoke(&options)?;
            println!(
                "catalog lobby scenario smoke saved to {} (captures={}, switches={})",
                report.directory.display(),
                report.capture_count,
                report.switch_count,
            );
            Ok(())
        }
        Cli::LobbyScenarioStereoSmoke { options } => {
            let report = run_lobby_scenario_stereo_smoke(&options)?;
            println!(
                "lobby scenario stereo smoke saved to {} (eye differences={}, switches={})",
                report.path.display(),
                report.eye_pixel_difference_count,
                report.switch_count,
            );
            Ok(())
        }
        Cli::WarmWorldSwapSmoke { options } => {
            let report = run_offscreen_warm_world_swap_smoke(&options)?;
            println!(
                "warm-world gate smoke saved to {} ({}x{}, A={} B={}, captures={}, crossings={}, A/B differing pixels={:.3}%, B/gate delta={:.3}%)",
                report.directory.display(),
                report.width,
                report.height,
                report.source_seed,
                report.destination_seed,
                report.frames.len(),
                report.switches.len(),
                report.source_destination_difference_ratio * 100.0,
                report.destination_gate_difference_ratio * 100.0,
            );
            if let Some(cost) = report.process_cost {
                println!(
                    "warm-world process cost sample={}ms one_world_cpu={:.3}% two_world_cpu={:.3}% rss_delta_kb={} thread_delta={}",
                    cost.one_world.duration_ms,
                    cost.one_world.cpu_percent_of_one_core.unwrap_or(f64::NAN),
                    cost.two_worlds.cpu_percent_of_one_core.unwrap_or(f64::NAN),
                    cost.retained_rss_delta_kb.unwrap_or_default(),
                    cost.observed_thread_delta.unwrap_or_default(),
                );
            }
            Ok(())
        }
        Cli::HeadlessDualView { options } => {
            let reports = write_headless_dual_view(&options)?;
            for report in reports {
                println!(
                    "headless dual-view {} saved to {} ({}x{}, {} bytes, {} non-clear RGB pixels, {} sections, {} drawn sections, {} indices, {} drawn indices, {} GUI commands, shared preparations={}, rendered views={}, paired differing pixels={})",
                    report.view_name,
                    report.path.display(),
                    report.width,
                    report.height,
                    report.byte_len,
                    report.non_clear_rgb_pixel_count,
                    report.section_count,
                    report.drawn_section_count,
                    report.index_count,
                    report.drawn_index_count,
                    report.gui_command_count,
                    report.shared_preparation_count,
                    report.rendered_view_count,
                    report.paired_pixel_difference_count,
                );
            }
            Ok(())
        }
        Cli::XrEmulationScreenshot { options } => {
            let report = run_xr_emulation_screenshot(&options)?;
            println!(
                "XR emulation side-by-side screenshot saved to {} ({}x{} total, {}x{} per eye, {} bytes, {} differing eye pixels, {} sections, {} drawn sections, {} GUI commands, {} eye UI composites)",
                report.path.display(),
                report.width,
                report.height,
                report.eye_width,
                report.eye_height,
                report.byte_len,
                report.eye_pixel_difference_count,
                report.section_count,
                report.drawn_section_count,
                report.gui_command_count,
                report.ui_panel_composite_count,
            );
            println!(
                "MCLONE_SEASONAL_APPEARANCE_STATE {}",
                report.seasonal_appearance_receipt_json
            );
            println!("MCLONE_CELESTIAL_STATE {}", report.celestial_receipt_json);
            Ok(())
        }
        Cli::HeadlessActorReviewSheet { options } => {
            let report = write_actor_review_sheet(&options)?;
            println!(
                "actor review sheet saved to {} ({}x{}, {} bytes, {} views, {} non-clear RGB pixels, {} actors, {} drawn actors)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.view_count,
                report.non_clear_rgb_pixel_count,
                report.actor_count,
                report.drawn_actor_count
            );
            Ok(())
        }
        Cli::HeadlessActorWalkReview { options } => {
            let report = write_actor_walk_review(&options)?;
            println!(
                "actor walk review saved to {} (sheet {}x{}, frame {}x{}, {} frames, {} fps, {:.2} cycles, {} bytes, {} non-clear RGB pixels, {} actors, {} drawn actors, video={} video_bytes={:?})",
                report.sheet_path.display(),
                report.sheet_width,
                report.frame_height,
                report.frame_width,
                report.frame_height,
                report.frame_count,
                report.fps,
                report.cycles,
                report.sheet_byte_len,
                report.non_clear_rgb_pixel_count,
                report.actor_count,
                report.drawn_actor_count,
                report
                    .video_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "none".to_owned()),
                report.video_byte_len
            );
            Ok(())
        }
        Cli::RendererRebuildSmoke { options } => {
            let report = run_renderer_rebuild_smoke(&options)?;
            println!(
                "renderer rebuild smoke saved before={} after={} ({}x{}, {} bytes, sections={}, drawn_sections={}, indices={}, drawn_indices={}, mismatch_pixels={}, reuploaded_sections={}, state_preserved={}, config_changed={}, before_render_size={:?}, after_render_size={:?})",
                report.before_path.display(),
                report.after_path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.section_count,
                report.drawn_section_count,
                report.index_count,
                report.drawn_index_count,
                report.pixel_mismatch_count,
                report.reuploaded_section_count,
                report.state_preserved,
                report.config_changed,
                report.before_render_size,
                report.after_render_size
            );
            Ok(())
        }
        Cli::TorchLightProbe { options } => {
            let report = run_torch_light_probe(&options)?;
            let frames = report
                .frames
                .iter()
                .map(|frame| {
                    format!(
                        "{}={} ({} bytes, sections={}, vertices={}, indices={})",
                        frame.label,
                        frame.path.display(),
                        frame.byte_len,
                        frame.section_count,
                        frame.vertex_count,
                        frame.index_count
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            let samples = report
                .samples
                .iter()
                .map(|sample| {
                    format!(
                        "{}@{},{},{}=block:{} uniform_sky:{} opening_sky:{}",
                        sample.label,
                        sample.position[0],
                        sample.position[1],
                        sample.position[2],
                        sample.block_light,
                        sample.uniform_sky_light,
                        sample.skylight_opening_sky_light
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "torch light probe saved {} ({}x{}, samples=[{}])",
                frames, report.width, report.height, samples
            );
            Ok(())
        }
        Cli::RemotePlayerVisualSmoke { options } => {
            let report = run_remote_player_visual_smoke(&options)?;
            println!(
                "remote player visual smoke saved to {} ({}x{}, {} bytes, {} remote players, {} actors, {} drawn actors, remote figures={:?}, walk distances={:?})",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.remote_player_count,
                report.actor_count,
                report.drawn_actor_count,
                report.remote_actor_figures,
                report.remote_actor_walk_animation_distances
            );
            Ok(())
        }
        Cli::MovementPerf { options } => {
            let report = run_movement_perf_smoke(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::Timedemo { options } => {
            let report = run_timedemo(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::FrameBudgetProbe { options } => {
            let report = run_frame_budget_probe(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::StartupStreamingPerf { options } => {
            let report = run_startup_streaming_perf(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::LoadingSettlePerf { options } => {
            let report = run_loading_settle_perf(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::XrClearSmoke { options } => run_xr_clear_smoke(options),
        Cli::XrMcloneSmoke { options } => run_xr_mclone_smoke(options),
        Cli::DesktopXr {
            options,
            window,
            start_intent,
        } => run_desktop_xr(options, window, start_intent),
        Cli::Window {
            scene,
            render_options,
            window,
            start_intent,
            startup_wait,
            frame_report,
        } => run_window(
            scene,
            render_options,
            window,
            start_intent,
            startup_wait,
            frame_report,
        ),
    }
}

fn print_benchmark_metadata(name: &str, indent: &str, trailing_comma: bool) {
    println!("{indent}\"benchmark\": \"{}\",", json_escape(name));
    println!(
        "{indent}\"recorded_unix_seconds\": {},",
        current_unix_seconds()
    );
    println!(
        "{indent}\"git_commit\": \"{}\",",
        json_escape(&git_short_commit())
    );
    println!("{indent}\"git_dirty\": {},", git_dirty());
    let suffix = if trailing_comma { "," } else { "" };
    println!(
        "{indent}\"debug_assertions\": {}{suffix}",
        cfg!(debug_assertions)
    );
}

pub(crate) fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(crate) fn git_short_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

pub(crate) fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true)
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(feature = "xr")]
fn run_xr_clear_smoke(options: crate::cli::XrClearSmokeOptions) -> Result<()> {
    desktop_xr::run(options)
}

#[cfg(not(feature = "xr"))]
fn run_xr_clear_smoke(_options: crate::cli::XrClearSmokeOptions) -> Result<()> {
    anyhow::bail!("rebuild with `--features xr` to use --xr-clear-smoke")
}

#[cfg(feature = "xr")]
fn run_xr_mclone_smoke(options: crate::cli::XrMcloneSmokeOptions) -> Result<()> {
    desktop_xr::run_mclone(options)
}

#[cfg(not(feature = "xr"))]
fn run_xr_mclone_smoke(_options: crate::cli::XrMcloneSmokeOptions) -> Result<()> {
    anyhow::bail!("rebuild with `--features xr` to use --xr-mclone-smoke")
}

#[cfg(feature = "xr")]
fn run_desktop_xr(
    options: crate::cli::XrMcloneSmokeOptions,
    window: bool,
    start_intent: crate::cli::WindowStartIntent,
) -> Result<()> {
    desktop_xr::run_desktop(options, window, start_intent)
}

#[cfg(not(feature = "xr"))]
fn run_desktop_xr(
    _options: crate::cli::XrMcloneSmokeOptions,
    _window: bool,
    _start_intent: crate::cli::WindowStartIntent,
) -> Result<()> {
    anyhow::bail!("rebuild with `--features xr` to use --desktop-xr")
}

#[cfg(test)]
mod tests;
