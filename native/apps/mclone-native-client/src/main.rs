#[cfg(test)]
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod app;
mod camera;
mod cli;
mod frame_pacing;
mod headless;
mod perf;
mod remote_session;
mod render_cache;
mod scene_runtime;
mod ui;

use crate::app::run_window;
#[cfg(test)]
use crate::camera::SPECTATOR_BASE_SPEED;
use crate::cli::Cli;
#[cfg(test)]
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, HeadlessScreenshotOptions, HeadlessScreenshotUi,
    MovementPerfOptions, SceneOptions, TimedemoOptions, parse_screenshot_ui_arg,
};
use crate::headless::{run_headless_screenshot, write_headless_chunk_scenarios};
use crate::perf::{run_frame_budget_probe, run_movement_perf_smoke, run_timedemo};
#[cfg(test)]
use crate::render_cache::snapshot_mesh_block_state_ids;
use crate::scene_runtime::build_scene_textured_sections;
use crate::ui::render_static_title_ui;
use anyhow::Result;
#[cfg(test)]
use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkPos, ChunkSnapshot};
use mclone_render::chunk::ChunkCamera;
#[cfg(test)]
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::headless::{
    HeadlessChunkOptions, HeadlessClearOptions, HeadlessUiOptions, write_headless_clear_png,
    write_headless_textured_sections_png_with_options, write_headless_ui_png,
};

const DEFAULT_SEED: i64 = 12345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const MIN_RENDER_DISTANCE: i32 = 2;
const DEFAULT_RENDER_DISTANCE: i32 = 2;
const MAX_RENDER_DISTANCE: i32 = 16;
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
        Cli::HeadlessChunk {
            path,
            width,
            height,
            scene,
            render_options,
        } => {
            let scene_mesh = build_scene_textured_sections(&scene)?;
            let report = write_headless_textured_sections_png_with_options(
                HeadlessChunkOptions {
                    path,
                    width,
                    height,
                    color: mclone_render::default_clear_color(),
                    camera: ChunkCamera::overview_for_chunk_area(
                        scene.chunk_x,
                        scene.chunk_z,
                        scene.render_distance,
                    ),
                },
                &scene_mesh.sections,
                scene_mesh.atlas.as_upload(),
                render_options,
            )?;
            println!(
                "headless textured sections saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.vertex_count,
                report.index_count
            );
            Ok(())
        }
        Cli::HeadlessChunkScenarios {
            directory,
            width,
            height,
            scene,
            render_options,
        } => {
            let scene_mesh = build_scene_textured_sections(&scene)?;
            let reports = write_headless_chunk_scenarios(
                &directory,
                width,
                height,
                &scene,
                &scene_mesh,
                render_options,
            )?;
            for report in reports {
                println!(
                    "headless textured scenario saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                    report.path.display(),
                    report.width,
                    report.height,
                    report.byte_len,
                    report.vertex_count,
                    report.index_count
                );
            }
            Ok(())
        }
        Cli::HeadlessUi {
            path,
            width,
            height,
        } => {
            let draw = render_static_title_ui(width, height);
            let report = write_headless_ui_png(
                HeadlessUiOptions {
                    path,
                    width,
                    height,
                    color: mclone_render::default_clear_color(),
                },
                &draw,
            )?;
            println!(
                "headless UI saved to {} ({}x{}, {} bytes, {} commands)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.command_count
            );
            Ok(())
        }
        Cli::HeadlessScreenshot { options } => {
            let report = run_headless_screenshot(&options)?;
            println!(
                "headless full-frame screenshot saved to {} ({}x{}, {} bytes, {} sections, {} drawn sections, {} GUI commands)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.section_count,
                report.drawn_section_count,
                report.gui_command_count
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
        Cli::Window {
            scene,
            render_options,
        } => run_window(scene, render_options),
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

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{BlockStateId, ChunkRevision, ChunkStatus};

    #[test]
    fn cli_defaults_to_window() {
        assert_eq!(
            Cli::parse([]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
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
    fn cli_parses_headless_ui_dimensions() {
        let cli = Cli::parse([
            "--headless-ui".to_owned(),
            "/tmp/mclone-ui.png".to_owned(),
            "--width".to_owned(),
            "960".to_owned(),
            "--height".to_owned(),
            "540".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessUi {
                path: PathBuf::from("/tmp/mclone-ui.png"),
                width: 960,
                height: 540,
            }
        );
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
            "--screenshot-scripted-interaction".to_owned(),
            "true".to_owned(),
            "--force-fullbright".to_owned(),
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
                    ui: HeadlessScreenshotUi::Pause,
                    debug_pane: true,
                    scripted_interaction: true,
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
    fn cli_parses_headless_chunk_scene_options() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
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
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    seed: -9,
                    chunk_x: 2,
                    chunk_z: -3,
                    render_distance: DEFAULT_RENDER_DISTANCE,
                    remote_addr: None,
                    day_time_override: None,
                    freeze_time: false,
                    lighting_enabled: true,
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_render_distance() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--render-distance".to_owned(),
            "16".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    render_distance: 16,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_rejects_render_distance_below_java_minimum() {
        let err = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--render-distance".to_owned(),
            "1".to_owned(),
        ])
        .unwrap_err()
        .to_string();

        assert!(err.contains("--render-distance must be between 2 and 16"));
    }

    #[test]
    fn cli_parses_section_occlusion_toggle() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--section-occlusion".to_owned(),
            "false".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    ..TexturedSectionRenderOptions::default()
                },
            }
        );

        let cli = Cli::parse([
            "--disable-section-occlusion".to_owned(),
            "--enable-section-occlusion".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_fullbright_toggle() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--fullbright".to_owned(),
            "true".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
            }
        );

        let cli = Cli::parse([
            "--force-fullbright".to_owned(),
            "--disable-fullbright".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_disable_lighting_as_runtime_bypass() {
        let cli = Cli::parse(["--disable-lighting".to_owned()]).unwrap();

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
            }
        );

        let cli = Cli::parse([
            "--disable-lighting".to_owned(),
            "--disable-fullbright".to_owned(),
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
    fn cli_parses_headless_chunk_scenarios() {
        let cli = Cli::parse([
            "--headless-chunk-scenarios".to_owned(),
            "/tmp/mclone-camera".to_owned(),
            "--width".to_owned(),
            "320".to_owned(),
            "--height".to_owned(),
            "180".to_owned(),
            "--render-distance".to_owned(),
            "2".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunkScenarios {
                directory: PathBuf::from("/tmp/mclone-camera"),
                width: 320,
                height: 180,
                scene: SceneOptions {
                    render_distance: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_remote_addr() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--remote-addr".to_owned(),
            "127.0.0.1:25565".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    remote_addr: Some("127.0.0.1:25565".to_owned()),
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
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
