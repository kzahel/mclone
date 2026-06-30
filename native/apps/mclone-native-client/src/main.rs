#[cfg(test)]
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod actor_assets;
mod app;
mod camera;
mod cli;
mod flat_client_driver;
mod frame_pacing;
mod headless;
mod offscreen_flat_client;
mod perf;
mod remote_player_visual_smoke;
mod remote_session;
mod render_cache;
mod scene_runtime;
mod ui;
#[cfg(feature = "xr")]
mod xr_clear_smoke;

use crate::app::run_window;
#[cfg(test)]
use crate::camera::SPECTATOR_BASE_SPEED;
use crate::cli::Cli;
#[cfg(test)]
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, HeadlessActorReviewSheetOptions,
    HeadlessDualViewOptions, HeadlessScreenshotOptions, HeadlessScreenshotUi, MovementPerfOptions,
    RemotePlayerVisualSmokeOptions, RendererRebuildSmokeOptions, SceneOptions, TimedemoOptions,
    WindowStartIntent, XrClearSmokeOptions, XrMcloneSmokeOptions, XrUnderwaterMode, XrViewPose,
    parse_screenshot_ui_arg,
};
use crate::headless::{
    run_headless_screenshot, run_renderer_rebuild_smoke, write_actor_review_sheet,
    write_headless_dual_view,
};
use crate::perf::{run_frame_budget_probe, run_movement_perf_smoke, run_timedemo};
use crate::remote_player_visual_smoke::run_remote_player_visual_smoke;
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

const DEFAULT_SEED: i64 = 12345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const MIN_RENDER_DISTANCE: i32 = 2;
const DEFAULT_RENDER_DISTANCE: i32 = 2;
const MAX_RENDER_DISTANCE: i32 = 32;
const DEFAULT_MOVEMENT_PERF_STEPS: usize = 12;
const DEFAULT_MOVEMENT_PERF_PATH_RADIUS: i32 = 4;
const DEFAULT_TIMEDEMO_FRAMES: usize = 120;
const DEFAULT_TIMEDEMO_PATH_RADIUS: i32 = 4;
const DEFAULT_FRAME_BUDGET_PROBE_FRAMES: usize = 240;
const DEFAULT_FRAME_BUDGET_TARGET_HZ: f64 = 120.0;
const MAX_MOVEMENT_PERF_STEPS: usize = 512;
const MAX_TIMEDEMO_FRAMES: usize = 4096;
const MAX_FRAME_BUDGET_PROBE_FRAMES: usize = 4096;
const MAX_MOVEMENT_PERF_PATH_RADIUS: i32 = 128;

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
                "headless full-frame screenshot saved to {} ({}x{}, {} bytes, {} sections, {} drawn sections, {} GUI commands, {} remote players, {} entities, {} actors, {} drawn actors, underwater={})",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.section_count,
                report.drawn_section_count,
                report.gui_command_count,
                report.remote_player_count,
                report.entity_count,
                report.actor_count,
                report.drawn_actor_count,
                report.underwater
            );
            Ok(())
        }
        Cli::HeadlessDualView { options } => {
            let reports = write_headless_dual_view(&options)?;
            for report in reports {
                println!(
                    "headless dual-view {} saved to {} ({}x{}, {} bytes, {} non-clear RGB pixels, {} sections, {} drawn sections, {} indices, {} drawn indices)",
                    report.view_name,
                    report.path.display(),
                    report.width,
                    report.height,
                    report.byte_len,
                    report.non_clear_rgb_pixel_count,
                    report.section_count,
                    report.drawn_section_count,
                    report.index_count,
                    report.drawn_index_count
                );
            }
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
        Cli::XrClearSmoke { options } => run_xr_clear_smoke(options),
        Cli::XrMcloneSmoke { options } => run_xr_mclone_smoke(options),
        Cli::Window {
            scene,
            render_options,
            start_intent,
            startup_wait,
        } => run_window(scene, render_options, start_intent, startup_wait),
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

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn git_short_commit() -> String {
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

fn git_dirty() -> bool {
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
    xr_clear_smoke::run(options)
}

#[cfg(not(feature = "xr"))]
fn run_xr_clear_smoke(_options: crate::cli::XrClearSmokeOptions) -> Result<()> {
    anyhow::bail!("rebuild with `--features xr` to use --xr-clear-smoke")
}

#[cfg(feature = "xr")]
fn run_xr_mclone_smoke(options: crate::cli::XrMcloneSmokeOptions) -> Result<()> {
    xr_clear_smoke::run_mclone(options)
}

#[cfg(not(feature = "xr"))]
fn run_xr_mclone_smoke(_options: crate::cli::XrMcloneSmokeOptions) -> Result<()> {
    anyhow::bail!("rebuild with `--features xr` to use --xr-mclone-smoke")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::StartupWaitPolicy;
    use mclone_core::{BlockStateId, ChunkRevision, ChunkStatus};

    fn screenshot_cli(
        path: &str,
        scene: SceneOptions,
        render_options: TexturedSectionRenderOptions,
    ) -> Cli {
        Cli::HeadlessScreenshot {
            options: HeadlessScreenshotOptions {
                path: PathBuf::from(path),
                width: 1280,
                height: 720,
                scene,
                render_options,
                startup_wait: StartupWaitPolicy::OFFSCREEN_SCREENSHOT_DEFAULT,
                camera_view: mclone_render_session::EngineCameraViewMode::FirstPerson,
                ui: HeadlessScreenshotUi::None,
                debug_pane: false,
                player_collision_box: false,
                scripted_interaction: false,
                remote_settle_ms: 0,
                eye: None,
            },
        }
    }

    #[test]
    fn cli_defaults_to_window() {
        assert_eq!(
            Cli::parse([]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                start_intent: WindowStartIntent::InWorld,
                startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            }
        );
    }

    #[test]
    fn cli_parses_window_menu_start_intent() {
        assert_eq!(
            Cli::parse(["--menu".to_owned()]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                start_intent: WindowStartIntent::Menu,
                startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            }
        );

        assert_eq!(
            Cli::parse(["--start-in-world".to_owned(), "false".to_owned()]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                start_intent: WindowStartIntent::Menu,
                startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            }
        );
    }

    #[test]
    fn cli_parses_startup_wait_policy() {
        assert_eq!(
            Cli::parse(["--startup-wait".to_owned(), "idle".to_owned()]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                start_intent: WindowStartIntent::InWorld,
                startup_wait: StartupWaitPolicy::Idle,
            }
        );

        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--startup-wait".to_owned(),
            "frames:2".to_owned(),
        ])
        .unwrap();
        let Cli::HeadlessScreenshot { options } = cli else {
            panic!("expected screenshot cli");
        };
        assert_eq!(options.startup_wait, StartupWaitPolicy::Frames(2));
    }

    #[test]
    fn cli_rejects_startup_wait_for_non_host_modes() {
        let err = Cli::parse([
            "--headless-dual-view".to_owned(),
            "/tmp/mclone-dual-view".to_owned(),
            "--startup-wait".to_owned(),
            "idle".to_owned(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("applies to window mode and --screenshot"));

        let err = Cli::parse(["--startup-wait".to_owned(), "frames:4097".to_owned()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("frames must be between 0 and 4096"));
    }

    #[test]
    fn cli_parses_headless_dual_view_scene_options() {
        let cli = Cli::parse([
            "--headless-dual-view".to_owned(),
            "/tmp/mclone-dual-view".to_owned(),
            "--width".to_owned(),
            "960".to_owned(),
            "--height".to_owned(),
            "640".to_owned(),
            "--seed".to_owned(),
            "54321".to_owned(),
            "--chunk-x".to_owned(),
            "2".to_owned(),
            "--chunk-z".to_owned(),
            "-1".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessDualView {
                options: HeadlessDualViewOptions {
                    directory: PathBuf::from("/tmp/mclone-dual-view"),
                    width: 960,
                    height: 640,
                    scene: SceneOptions {
                        seed: 54321,
                        chunk_x: 2,
                        chunk_z: -1,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                },
            }
        );
    }

    #[test]
    fn cli_parses_renderer_rebuild_smoke_options() {
        let cli = Cli::parse([
            "--renderer-rebuild-smoke".to_owned(),
            "/tmp/mclone-render-rebuild".to_owned(),
            "--width".to_owned(),
            "640".to_owned(),
            "--height".to_owned(),
            "360".to_owned(),
            "--seed".to_owned(),
            "99".to_owned(),
            "--render-color-profile".to_owned(),
            "stylized-bright".to_owned(),
            "--rebuild-render-scale".to_owned(),
            "0.5".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::RendererRebuildSmoke {
                options: RendererRebuildSmokeOptions {
                    directory: PathBuf::from("/tmp/mclone-render-rebuild"),
                    width: 640,
                    height: 360,
                    scene: SceneOptions {
                        seed: 99,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions {
                        color_profile: RenderColorProfile::StylizedBright,
                        ..TexturedSectionRenderOptions::default()
                    },
                    rebuild_render_scale: Some(0.5),
                },
            }
        );
    }

    #[test]
    fn cli_parses_remote_player_visual_smoke_options() {
        let cli = Cli::parse([
            "--remote-player-visual-smoke".to_owned(),
            "/tmp/mclone-remote-player-visual-smoke.png".to_owned(),
            "--width".to_owned(),
            "800".to_owned(),
            "--height".to_owned(),
            "450".to_owned(),
            "--seed".to_owned(),
            "77".to_owned(),
            "--lighting".to_owned(),
            "false".to_owned(),
            "--fullbright".to_owned(),
            "true".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::RemotePlayerVisualSmoke {
                options: RemotePlayerVisualSmokeOptions {
                    path: PathBuf::from("/tmp/mclone-remote-player-visual-smoke.png"),
                    width: 800,
                    height: 450,
                    scene: SceneOptions {
                        seed: 77,
                        lighting_enabled: false,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions {
                        force_fullbright: true,
                        ..TexturedSectionRenderOptions::default()
                    },
                },
            }
        );
    }

    #[test]
    fn cli_parses_xr_clear_smoke_options() {
        let cli = Cli::parse([
            "--xr-clear-smoke".to_owned(),
            "--frames".to_owned(),
            "12".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::XrClearSmoke {
                options: XrClearSmokeOptions {
                    frame_limit: Some(12),
                },
            }
        );
    }

    #[test]
    fn cli_parses_xr_mclone_smoke_options() {
        let cli = Cli::parse([
            "--xr-mclone-smoke".to_owned(),
            "--frames".to_owned(),
            "24".to_owned(),
            "--seed".to_owned(),
            "54321".to_owned(),
            "--chunk-x".to_owned(),
            "2".to_owned(),
            "--chunk-z".to_owned(),
            "-1".to_owned(),
            "--render-distance".to_owned(),
            "3".to_owned(),
            "--day-time".to_owned(),
            "6000".to_owned(),
            "--freeze-time".to_owned(),
            "--fullbright".to_owned(),
            "true".to_owned(),
            "--view-pose".to_owned(),
            "8,72,-12,180".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::XrMcloneSmoke {
                options: XrMcloneSmokeOptions {
                    scene: SceneOptions {
                        seed: 54321,
                        chunk_x: 2,
                        chunk_z: -1,
                        render_distance: 3,
                        day_time_override: Some(6000),
                        freeze_time: true,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions {
                        force_fullbright: true,
                        ..TexturedSectionRenderOptions::default()
                    },
                    frame_limit: Some(24),
                    view_pose: Some(XrViewPose {
                        position: [8.0, 72.0, -12.0],
                        yaw_degrees: 180.0,
                    }),
                    underwater_mode: XrUnderwaterMode::Midpoint,
                },
            }
        );
    }

    #[test]
    fn cli_parses_xr_view_pose_default() {
        let cli = Cli::parse([
            "--xr-mclone-smoke".to_owned(),
            "--view-pose=default".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::XrMcloneSmoke {
                options: XrMcloneSmokeOptions {
                    scene: SceneOptions::default(),
                    render_options: TexturedSectionRenderOptions::default(),
                    frame_limit: Some(120),
                    view_pose: None,
                    underwater_mode: XrUnderwaterMode::Midpoint,
                },
            }
        );
    }

    #[test]
    fn cli_parses_xr_underwater_mode() {
        let cli = Cli::parse([
            "--xr-mclone-smoke".to_owned(),
            "--xr-underwater-mode".to_owned(),
            "per-eye".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::XrMcloneSmoke {
                options: XrMcloneSmokeOptions {
                    scene: SceneOptions::default(),
                    render_options: TexturedSectionRenderOptions::default(),
                    frame_limit: Some(120),
                    view_pose: None,
                    underwater_mode: XrUnderwaterMode::PerEye,
                },
            }
        );
    }

    #[test]
    fn cli_parses_xr_forever_smoke_options() {
        let cli = Cli::parse([
            "--xr-mclone-smoke".to_owned(),
            "--xr-forever".to_owned(),
            "--view-pose=0,78,-96,180".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::XrMcloneSmoke {
                options: XrMcloneSmokeOptions {
                    scene: SceneOptions::default(),
                    render_options: TexturedSectionRenderOptions::default(),
                    frame_limit: None,
                    view_pose: Some(XrViewPose {
                        position: [0.0, 78.0, -96.0],
                        yaw_degrees: 180.0,
                    }),
                    underwater_mode: XrUnderwaterMode::Midpoint,
                },
            }
        );
    }

    #[test]
    fn cli_rejects_xr_clear_smoke_with_headless_mode() {
        let err = Cli::parse([
            "--xr-clear-smoke".to_owned(),
            "--headless-clear".to_owned(),
            "/tmp/mclone-clear.png".to_owned(),
        ])
        .unwrap_err()
        .to_string();

        assert!(err.contains("XR smoke modes cannot be combined"));
    }

    #[test]
    fn cli_rejects_xr_frames_without_xr_clear_smoke() {
        let err = Cli::parse(["--frames".to_owned(), "12".to_owned()])
            .unwrap_err()
            .to_string();

        assert!(err.contains("--frames requires --xr-clear-smoke or --xr-mclone-smoke"));
    }

    #[test]
    fn cli_rejects_xr_forever_without_xr_smoke() {
        let err = Cli::parse(["--xr-forever".to_owned()])
            .unwrap_err()
            .to_string();

        assert!(err.contains("--xr-forever requires --xr-clear-smoke or --xr-mclone-smoke"));
    }

    #[test]
    fn cli_rejects_xr_forever_with_frame_count() {
        let err = Cli::parse([
            "--xr-clear-smoke".to_owned(),
            "--frames".to_owned(),
            "12".to_owned(),
            "--xr-forever".to_owned(),
        ])
        .unwrap_err()
        .to_string();

        assert!(err.contains("--xr-forever cannot be combined"));
    }

    #[test]
    fn cli_rejects_xr_view_pose_without_mclone_smoke() {
        let err = Cli::parse(["--view-pose".to_owned(), "0,64,0,0".to_owned()])
            .unwrap_err()
            .to_string();

        assert!(err.contains("--view-pose requires --xr-mclone-smoke"));
    }

    #[test]
    fn cli_rejects_xr_underwater_mode_without_mclone_smoke() {
        let err = Cli::parse(["--xr-underwater-mode".to_owned(), "per-eye".to_owned()])
            .unwrap_err()
            .to_string();

        assert!(err.contains("--xr-underwater-mode requires --xr-mclone-smoke"));
    }

    #[test]
    fn cli_rejects_invalid_xr_view_pose() {
        let err = Cli::parse([
            "--xr-mclone-smoke".to_owned(),
            "--view-pose".to_owned(),
            "0,64,0".to_owned(),
        ])
        .unwrap_err()
        .to_string();

        assert!(err.contains("--view-pose expects X,Y,Z,YAW_DEGREES"));
    }

    #[test]
    fn cli_rejects_combined_xr_smoke_modes() {
        let err = Cli::parse([
            "--xr-clear-smoke".to_owned(),
            "--xr-mclone-smoke".to_owned(),
        ])
        .unwrap_err()
        .to_string();

        assert!(err.contains("--xr-mclone-smoke cannot be combined"));
    }

    #[test]
    fn cli_parses_headless_clear_dimensions_after_path() {
        let cli = Cli::parse([
            "--headless-clear".to_owned(),
            "/tmp/mclone.png".to_owned(),
            "--width".to_owned(),
            "32".to_owned(),
            "--height".to_owned(),
            "16".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessClear {
                path: PathBuf::from("/tmp/mclone.png"),
                width: 32,
                height: 16,
            }
        );
    }

    #[test]
    fn cli_parses_actor_review_sheet_options() {
        let cli = Cli::parse([
            "--actor-review-sheet".to_owned(),
            "/tmp/mclone-actor-review.png".to_owned(),
            "--width".to_owned(),
            "900".to_owned(),
            "--height".to_owned(),
            "420".to_owned(),
            "--fullbright".to_owned(),
            "false".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessActorReviewSheet {
                options: HeadlessActorReviewSheetOptions {
                    path: PathBuf::from("/tmp/mclone-actor-review.png"),
                    width: 900,
                    height: 420,
                    render_options: TexturedSectionRenderOptions {
                        force_fullbright: false,
                        ..TexturedSectionRenderOptions::default()
                    },
                },
            }
        );
    }

    #[test]
    fn cli_parses_dimensions_before_headless_path() {
        let cli = Cli::parse([
            "--width".to_owned(),
            "32".to_owned(),
            "--height".to_owned(),
            "16".to_owned(),
            "--headless-clear".to_owned(),
            "/tmp/mclone.png".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessClear {
                path: PathBuf::from("/tmp/mclone.png"),
                width: 32,
                height: 16,
            }
        );
    }

    #[test]
    fn cli_rejects_retired_narrow_headless_modes() {
        for flag in [
            "--headless-ui",
            "--headless-chunk",
            "--headless-chunk-scenarios",
        ] {
            let err = Cli::parse([flag.to_owned(), "/tmp/retired.png".to_owned()])
                .unwrap_err()
                .to_string();
            assert!(err.contains("unknown argument"), "{flag}: {err}");
        }
    }

    #[test]
    fn cli_parses_full_frame_screenshot_options() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--width".to_owned(),
            "960".to_owned(),
            "--height".to_owned(),
            "540".to_owned(),
            "--screenshot-ui".to_owned(),
            "pause".to_owned(),
            "--screenshot-debug-pane".to_owned(),
            "true".to_owned(),
            "--screenshot-player-box".to_owned(),
            "true".to_owned(),
            "--screenshot-scripted-interaction".to_owned(),
            "true".to_owned(),
            "--screenshot-remote-settle-ms".to_owned(),
            "750".to_owned(),
            "--screenshot-eye".to_owned(),
            "1.5,62.25,-3".to_owned(),
            "--screenshot-camera-view".to_owned(),
            "third-person".to_owned(),
            "--fullbright".to_owned(),
            "true".to_owned(),
            "--remote-addr".to_owned(),
            "127.0.0.1:25565".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessScreenshot {
                options: HeadlessScreenshotOptions {
                    path: PathBuf::from("/tmp/mclone-frame.png"),
                    width: 960,
                    height: 540,
                    scene: SceneOptions {
                        remote_addr: Some("127.0.0.1:25565".to_owned()),
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions {
                        force_fullbright: true,
                        ..TexturedSectionRenderOptions::default()
                    },
                    startup_wait: StartupWaitPolicy::OFFSCREEN_SCREENSHOT_DEFAULT,
                    camera_view: mclone_render_session::EngineCameraViewMode::ThirdPersonBack,
                    ui: HeadlessScreenshotUi::Pause,
                    debug_pane: true,
                    player_collision_box: true,
                    scripted_interaction: true,
                    remote_settle_ms: 750,
                    eye: Some([1.5, 62.25, -3.0]),
                },
            }
        );
    }

    #[test]
    fn parse_screenshot_ui_accepts_named_screens() {
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("none".to_owned())).unwrap(),
            HeadlessScreenshotUi::None
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("title".to_owned())).unwrap(),
            HeadlessScreenshotUi::Title
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("new-world".to_owned())).unwrap(),
            HeadlessScreenshotUi::NewWorld
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("join-remote".to_owned())).unwrap(),
            HeadlessScreenshotUi::JoinRemote
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("help".to_owned())).unwrap(),
            HeadlessScreenshotUi::Help
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("controls".to_owned())).unwrap(),
            HeadlessScreenshotUi::Help
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("block-palette".to_owned())).unwrap(),
            HeadlessScreenshotUi::BlockPalette
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("options-title".to_owned())).unwrap(),
            HeadlessScreenshotUi::OptionsTitle
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("options".to_owned())).unwrap(),
            HeadlessScreenshotUi::OptionsPause
        );
        assert!(parse_screenshot_ui_arg("--screenshot-ui", Some("bad".to_owned())).is_err());
    }

    #[test]
    fn cli_parses_screenshot_scene_options() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--seed".to_owned(),
            "-9".to_owned(),
            "--chunk-x".to_owned(),
            "2".to_owned(),
            "--chunk-z".to_owned(),
            "-3".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions {
                    seed: -9,
                    chunk_x: 2,
                    chunk_z: -3,
                    render_distance: DEFAULT_RENDER_DISTANCE,
                    remote_addr: None,
                    day_time_override: None,
                    freeze_time: false,
                    movement_speed_multiplier: 1.0,
                    first_person_player_visible: false,
                    lighting_enabled: true,
                },
                TexturedSectionRenderOptions::default(),
            )
        );
    }

    #[test]
    fn cli_parses_render_distance() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--render-distance".to_owned(),
            "32".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions {
                    render_distance: 32,
                    ..SceneOptions::default()
                },
                TexturedSectionRenderOptions::default(),
            )
        );
    }

    #[test]
    fn cli_parses_movement_speed_multiplier() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--movement-speed-multiplier".to_owned(),
            "2.25".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions {
                    movement_speed_multiplier: 2.25,
                    ..SceneOptions::default()
                },
                TexturedSectionRenderOptions::default(),
            )
        );
    }

    #[test]
    fn cli_parses_first_person_player_visibility() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--first-person-player".to_owned(),
            "true".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions {
                    first_person_player_visible: true,
                    ..SceneOptions::default()
                },
                TexturedSectionRenderOptions::default(),
            )
        );
    }

    #[test]
    fn cli_rejects_render_distance_below_java_minimum() {
        let err = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--render-distance".to_owned(),
            "1".to_owned(),
        ])
        .unwrap_err()
        .to_string();

        assert!(err.contains("--render-distance must be between 2 and 32"));
    }

    #[test]
    fn cli_parses_section_occlusion_toggle() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--section-occlusion".to_owned(),
            "false".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions::default(),
                TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    ..TexturedSectionRenderOptions::default()
                },
            )
        );

        let err = Cli::parse(["--disable-section-occlusion".to_owned()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown argument"));
    }

    #[test]
    fn cli_parses_fullbright_toggle() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--fullbright".to_owned(),
            "true".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions::default(),
                TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
            )
        );

        let cli = Cli::parse(["--fullbright".to_owned(), "false".to_owned()]).unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
                start_intent: WindowStartIntent::InWorld,
                startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            }
        );
    }

    #[test]
    fn cli_parses_render_color_profile() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--render-color-profile".to_owned(),
            "stylized-bright".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions::default(),
                TexturedSectionRenderOptions {
                    color_profile: RenderColorProfile::StylizedBright,
                    ..TexturedSectionRenderOptions::default()
                },
            )
        );

        let err = Cli::parse(["--render-color-profile".to_owned(), "neon".to_owned()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("expected vanilla"));
    }

    #[test]
    fn cli_parses_lighting_false_as_runtime_bypass() {
        let cli = Cli::parse(["--lighting".to_owned(), "false".to_owned()]).unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions {
                    lighting_enabled: false,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
                start_intent: WindowStartIntent::InWorld,
                startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            }
        );

        let cli = Cli::parse([
            "--lighting".to_owned(),
            "false".to_owned(),
            "--fullbright".to_owned(),
            "false".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions {
                    lighting_enabled: false,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
                start_intent: WindowStartIntent::InWorld,
                startup_wait: StartupWaitPolicy::DESKTOP_DEFAULT,
            }
        );
    }

    #[test]
    fn cli_parses_movement_perf_options() {
        let cli = Cli::parse([
            "--movement-perf".to_owned(),
            "--movement-steps".to_owned(),
            "6".to_owned(),
            "--path-radius".to_owned(),
            "3".to_owned(),
            "--render-distance".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "800".to_owned(),
            "--height".to_owned(),
            "600".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::MovementPerf {
                options: MovementPerfOptions {
                    scene: SceneOptions {
                        render_distance: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 800,
                    height: 600,
                    steps: 6,
                    path_radius_chunks: 3,
                },
            }
        );
    }

    #[test]
    fn cli_parses_timedemo_options() {
        let cli = Cli::parse([
            "--timedemo".to_owned(),
            "--timedemo-frames".to_owned(),
            "48".to_owned(),
            "--path-radius".to_owned(),
            "5".to_owned(),
            "--render-distance".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "1024".to_owned(),
            "--height".to_owned(),
            "768".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Timedemo {
                options: TimedemoOptions {
                    scene: SceneOptions {
                        render_distance: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    frames: 48,
                    path_radius_chunks: 5,
                },
            }
        );
    }

    #[test]
    fn cli_parses_frame_budget_probe_options() {
        let cli = Cli::parse([
            "--frame-budget-probe".to_owned(),
            "--frame-budget-frames".to_owned(),
            "36".to_owned(),
            "--target-hz".to_owned(),
            "120".to_owned(),
            "--path-radius".to_owned(),
            "5".to_owned(),
            "--render-distance".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "1024".to_owned(),
            "--height".to_owned(),
            "768".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    scene: SceneOptions {
                        render_distance: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    mode: FrameBudgetProbeMode::StressOrbit,
                    frames: 36,
                    path_radius_chunks: 5,
                    target_hz: 120.0,
                    movement_speed: SPECTATOR_BASE_SPEED,
                },
            }
        );
    }

    #[test]
    fn cli_target_hz_selects_frame_budget_probe() {
        let cli = Cli::parse(["--target-hz".to_owned(), "90".to_owned()]).unwrap();

        assert_eq!(
            cli,
            Cli::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    target_hz: 90.0,
                    ..FrameBudgetProbeOptions::default()
                },
            }
        );
    }

    #[test]
    fn cli_parses_movement_frame_probe_options() {
        let cli = Cli::parse([
            "--target-hz".to_owned(),
            "120".to_owned(),
            "--movement-frame-probe".to_owned(),
            "--frame-budget-frames".to_owned(),
            "36".to_owned(),
            "--movement-frame-speed".to_owned(),
            "48".to_owned(),
            "--path-radius".to_owned(),
            "5".to_owned(),
            "--render-distance".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "1024".to_owned(),
            "--height".to_owned(),
            "768".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    scene: SceneOptions {
                        render_distance: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    mode: FrameBudgetProbeMode::MovementWalk,
                    frames: 36,
                    path_radius_chunks: 5,
                    target_hz: 120.0,
                    movement_speed: 48.0,
                },
            }
        );
    }

    #[test]
    fn cli_parses_remote_addr() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--remote-addr".to_owned(),
            "127.0.0.1:25565".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            screenshot_cli(
                "/tmp/mclone-frame.png",
                SceneOptions {
                    remote_addr: Some("127.0.0.1:25565".to_owned()),
                    ..SceneOptions::default()
                },
                TexturedSectionRenderOptions::default(),
            )
        );
    }

    #[test]
    fn snapshot_mesh_block_state_ids_rehydrates_omitted_air_sections() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        block_state_ids[CHUNK_SECTION_VOLUME] = BlockStateId(1);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            32,
            &block_state_ids,
        );

        let mesh_blocks = snapshot_mesh_block_state_ids(&snapshot).unwrap();

        assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
        assert_eq!(mesh_blocks.blocks[0], AIR_BLOCK_STATE_ID);
        assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], BlockStateId(1));
    }
}
