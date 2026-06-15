use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_assets::{
    AssetSource, BlockModelLibrary, BlockStateAssetIndex, BlockStateRegistry,
    FilesystemAssetSource, TextureAtlasPlan,
};
use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot,
    PackedLightSection, SECTION_HEIGHT,
};
use mclone_mesh::{
    RenderSectionKey, TexturedChunkMeshInput, TexturedMeshCatalog,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, VisibilityGraphBuildStats,
    build_textured_render_sections_for_chunk_set_with_stats,
    build_textured_render_sections_with_stats, quad_face_count_from_indices,
};
use mclone_net::{LocalTransport, request_server_updates};
use mclone_protocol::{ChunkInterest, ServerUpdate};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedSectionDrawResources, TexturedSectionRenderOptions, TexturedSectionUploadReport,
    textured_section_visibility_stats_with_options,
};
use mclone_render::gui::{GuiRenderOptions, GuiRenderer};
use mclone_render::headless::{
    HEADLESS_FORMAT, HeadlessChunkOptions, HeadlessClearOptions, HeadlessFrameLoopOptions,
    HeadlessFrameOptions, HeadlessTimedemoOptions, HeadlessUiOptions, run_headless_frame_loop,
    run_headless_textured_sections_timedemo, write_headless_clear_png, write_headless_frame_png,
    write_headless_textured_sections_png_with_options, write_headless_ui_png,
};
use mclone_render::native::{
    NativeSurfaceContext, SurfaceFrameStatus, SurfacePresentModePreference,
    surface_present_mode_label,
};
use mclone_render::target::RenderFrameContext;
use mclone_server::IntegratedServer;
use mclone_ui::{
    Button, Checkbox, Color, CycleButton, Font, GuiDrawList, GuiScale, Interaction, Point, Rect,
    WidgetId,
};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const DEFAULT_SEED: i64 = 12345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_CHUNK_RADIUS: i32 = 1;
const MAX_CHUNK_RADIUS: i32 = 4;
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
const SPECTATOR_BASE_SPEED: f32 = 32.0;
const SPECTATOR_MIN_SPEED: f32 = 2.0;
const SPECTATOR_MAX_SPEED: f32 = 256.0;
const SPECTATOR_MOUSE_SENSITIVITY: f32 = 0.0035;
const SPECTATOR_PITCH_LIMIT: f32 = 1.52;
const DEFAULT_FPS_CAP: u32 = 120;
const FPS_CAPS: [u32; 6] = [60, 90, 120, 144, 165, 240];
const DEFAULT_RENDER_CHUNK_MESH_BUDGET: usize = 1;

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
                        scene.chunk_radius,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SceneOptions {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    chunk_radius: i32,
    remote_addr: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MovementPerfOptions {
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    width: u32,
    height: u32,
    steps: usize,
    path_radius_chunks: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TimedemoOptions {
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    width: u32,
    height: u32,
    frames: usize,
    path_radius_chunks: i32,
}

#[derive(Clone, Debug, PartialEq)]
struct FrameBudgetProbeOptions {
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    width: u32,
    height: u32,
    frames: usize,
    path_radius_chunks: i32,
    target_hz: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HeadlessScreenshotOptions {
    path: PathBuf,
    width: u32,
    height: u32,
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    ui: HeadlessScreenshotUi,
    debug_pane: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum HeadlessScreenshotUi {
    #[default]
    None,
    Title,
    Pause,
    OptionsTitle,
    OptionsPause,
}

impl Default for MovementPerfOptions {
    fn default() -> Self {
        Self {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            width: 1280,
            height: 720,
            steps: DEFAULT_MOVEMENT_PERF_STEPS,
            path_radius_chunks: DEFAULT_MOVEMENT_PERF_PATH_RADIUS,
        }
    }
}

impl Default for TimedemoOptions {
    fn default() -> Self {
        Self {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            width: 1280,
            height: 720,
            frames: DEFAULT_TIMEDEMO_FRAMES,
            path_radius_chunks: DEFAULT_TIMEDEMO_PATH_RADIUS,
        }
    }
}

impl Default for FrameBudgetProbeOptions {
    fn default() -> Self {
        Self {
            scene: SceneOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            width: 1280,
            height: 720,
            frames: DEFAULT_FRAME_BUDGET_PROBE_FRAMES,
            path_radius_chunks: DEFAULT_MOVEMENT_PERF_PATH_RADIUS,
            target_hz: DEFAULT_FRAME_BUDGET_TARGET_HZ,
        }
    }
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            chunk_radius: DEFAULT_CHUNK_RADIUS,
            remote_addr: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Cli {
    Window {
        scene: SceneOptions,
        render_options: TexturedSectionRenderOptions,
    },
    HeadlessClear {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    HeadlessChunk {
        path: PathBuf,
        width: u32,
        height: u32,
        scene: SceneOptions,
        render_options: TexturedSectionRenderOptions,
    },
    HeadlessChunkScenarios {
        directory: PathBuf,
        width: u32,
        height: u32,
        scene: SceneOptions,
        render_options: TexturedSectionRenderOptions,
    },
    HeadlessUi {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    HeadlessScreenshot {
        options: HeadlessScreenshotOptions,
    },
    MovementPerf {
        options: MovementPerfOptions,
    },
    Timedemo {
        options: TimedemoOptions,
    },
    FrameBudgetProbe {
        options: FrameBudgetProbeOptions,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessMode {
    Clear(PathBuf),
    Chunk(PathBuf),
    ChunkScenarios(PathBuf),
    Ui(PathBuf),
    Screenshot(PathBuf),
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut width = None;
        let mut height = None;
        let mut scene = SceneOptions::default();
        let mut render_options = TexturedSectionRenderOptions::default();
        let mut screenshot_ui = HeadlessScreenshotUi::None;
        let mut screenshot_debug_pane = false;
        let mut movement_perf = false;
        let mut timedemo = false;
        let mut frame_budget_probe = false;
        let mut movement_steps = DEFAULT_MOVEMENT_PERF_STEPS;
        let mut timedemo_frames = DEFAULT_TIMEDEMO_FRAMES;
        let mut frame_budget_frames = DEFAULT_FRAME_BUDGET_PROBE_FRAMES;
        let mut target_hz = DEFAULT_FRAME_BUDGET_TARGET_HZ;
        let mut path_radius = DEFAULT_MOVEMENT_PERF_PATH_RADIUS;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--movement-perf" => {
                    if mode.is_some() {
                        bail!("--movement-perf cannot be combined with a headless output mode");
                    }
                    if timedemo {
                        bail!("--movement-perf cannot be combined with --timedemo");
                    }
                    movement_perf = true;
                }
                "--timedemo" => {
                    if mode.is_some() {
                        bail!("--timedemo cannot be combined with a headless output mode");
                    }
                    if movement_perf {
                        bail!("--timedemo cannot be combined with --movement-perf");
                    }
                    timedemo = true;
                }
                "--frame-budget-probe" => {
                    if mode.is_some() {
                        bail!(
                            "--frame-budget-probe cannot be combined with a headless output mode"
                        );
                    }
                    frame_budget_probe = true;
                }
                "--headless-clear" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-clear requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Clear(path))?;
                }
                "--headless-chunk" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Chunk(path))?;
                }
                "--headless-chunk-scenarios" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk-scenarios requires an output directory")?;
                    if movement_perf || timedemo || frame_budget_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::ChunkScenarios(path))?;
                }
                "--headless-ui" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-ui requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Ui(path))?;
                }
                "--screenshot" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--screenshot requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Screenshot(path))?;
                }
                "--screenshot-ui" => {
                    screenshot_ui = parse_screenshot_ui_arg("--screenshot-ui", args.next())?;
                }
                "--screenshot-debug-pane" => {
                    screenshot_debug_pane = parse_bool_arg("--screenshot-debug-pane", args.next())?;
                }
                "--width" => width = Some(parse_u32_arg("--width", args.next())?),
                "--height" => height = Some(parse_u32_arg("--height", args.next())?),
                "--seed" => scene.seed = parse_i64_arg("--seed", args.next())?,
                "--chunk-x" => scene.chunk_x = parse_i32_arg("--chunk-x", args.next())?,
                "--chunk-z" => scene.chunk_z = parse_i32_arg("--chunk-z", args.next())?,
                "--chunk-radius" => {
                    scene.chunk_radius = parse_chunk_radius_arg("--chunk-radius", args.next())?
                }
                "--remote-addr" => {
                    scene.remote_addr =
                        Some(args.next().context("--remote-addr requires HOST:PORT")?);
                }
                "--section-occlusion" => {
                    render_options.section_occlusion_culling =
                        parse_bool_arg("--section-occlusion", args.next())?;
                }
                "--fullbright" => {
                    render_options.force_fullbright = parse_bool_arg("--fullbright", args.next())?;
                }
                "--disable-section-occlusion" => {
                    render_options.section_occlusion_culling = false;
                }
                "--enable-section-occlusion" => {
                    render_options.section_occlusion_culling = true;
                }
                "--force-fullbright" => {
                    render_options.force_fullbright = true;
                }
                "--disable-fullbright" => {
                    render_options.force_fullbright = false;
                }
                "--movement-steps" => {
                    movement_perf = true;
                    movement_steps = parse_movement_steps_arg("--movement-steps", args.next())?;
                }
                "--timedemo-frames" => {
                    timedemo = true;
                    timedemo_frames = parse_timedemo_frames_arg("--timedemo-frames", args.next())?;
                }
                "--frame-budget-frames" => {
                    frame_budget_probe = true;
                    frame_budget_frames =
                        parse_frame_budget_frames_arg("--frame-budget-frames", args.next())?;
                }
                "--target-hz" => {
                    frame_budget_probe = true;
                    target_hz = parse_target_hz_arg("--target-hz", args.next())?;
                }
                "--movement-path-radius" => {
                    movement_perf = true;
                    path_radius = parse_path_radius_arg(&arg, args.next())?;
                }
                "--path-radius" => {
                    path_radius = parse_path_radius_arg(&arg, args.next())?;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }

        let perf_mode_count = movement_perf as u8 + timedemo as u8 + frame_budget_probe as u8;
        if perf_mode_count > 1 {
            bail!("--movement-perf, --timedemo, and --frame-budget-probe are mutually exclusive");
        }
        match mode {
            Some(HeadlessMode::Clear(path)) => Ok(Self::HeadlessClear {
                path,
                width: width.unwrap_or(96),
                height: height.unwrap_or(64),
            }),
            Some(HeadlessMode::Chunk(path)) => Ok(Self::HeadlessChunk {
                path,
                width: width.unwrap_or(640),
                height: height.unwrap_or(480),
                scene,
                render_options,
            }),
            Some(HeadlessMode::ChunkScenarios(directory)) => Ok(Self::HeadlessChunkScenarios {
                directory,
                width: width.unwrap_or(960),
                height: height.unwrap_or(640),
                scene,
                render_options,
            }),
            Some(HeadlessMode::Ui(path)) => Ok(Self::HeadlessUi {
                path,
                width: width.unwrap_or(960),
                height: height.unwrap_or(540),
            }),
            Some(HeadlessMode::Screenshot(path)) => Ok(Self::HeadlessScreenshot {
                options: HeadlessScreenshotOptions {
                    path,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    scene,
                    render_options,
                    ui: screenshot_ui,
                    debug_pane: screenshot_debug_pane,
                },
            }),
            None if movement_perf => Ok(Self::MovementPerf {
                options: MovementPerfOptions {
                    scene,
                    render_options,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    steps: movement_steps,
                    path_radius_chunks: path_radius,
                },
            }),
            None if timedemo => Ok(Self::Timedemo {
                options: TimedemoOptions {
                    scene,
                    render_options,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    frames: timedemo_frames,
                    path_radius_chunks: path_radius,
                },
            }),
            None if frame_budget_probe => Ok(Self::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    scene,
                    render_options,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    frames: frame_budget_frames,
                    path_radius_chunks: path_radius,
                    target_hz,
                },
            }),
            None => Ok(Self::Window {
                scene,
                render_options,
            }),
        }
    }
}

fn set_headless_mode(mode: &mut Option<HeadlessMode>, next: HeadlessMode) -> Result<()> {
    if mode.is_some() {
        bail!("only one headless output mode can be selected");
    }
    *mode = Some(next);
    Ok(())
}

fn parse_u32_arg(flag: &str, value: Option<String>) -> Result<u32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if parsed == 0 {
        bail!("{flag} must be greater than zero");
    }
    Ok(parsed)
}

fn parse_i32_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i32>()
        .with_context(|| format!("{flag} requires a signed integer, got `{value}`"))
}

fn parse_i64_arg(flag: &str, value: Option<String>) -> Result<i64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i64>()
        .with_context(|| format!("{flag} requires a signed 64-bit integer, got `{value}`"))
}

fn parse_chunk_radius_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let parsed = parse_i32_arg(flag, value)?;
    if !(0..=MAX_CHUNK_RADIUS).contains(&parsed) {
        bail!("{flag} must be between 0 and {MAX_CHUNK_RADIUS}");
    }
    Ok(parsed)
}

fn parse_movement_steps_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if !(1..=MAX_MOVEMENT_PERF_STEPS).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_MOVEMENT_PERF_STEPS}");
    }
    Ok(parsed)
}

fn parse_timedemo_frames_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if !(1..=MAX_TIMEDEMO_FRAMES).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_TIMEDEMO_FRAMES}");
    }
    Ok(parsed)
}

fn parse_frame_budget_frames_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if !(1..=MAX_FRAME_BUDGET_PROBE_FRAMES).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_FRAME_BUDGET_PROBE_FRAMES}");
    }
    Ok(parsed)
}

fn parse_target_hz_arg(flag: &str, value: Option<String>) -> Result<f64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<f64>()
        .with_context(|| format!("{flag} requires a positive number, got `{value}`"))?;
    if !parsed.is_finite() || !(1.0..=1000.0).contains(&parsed) {
        bail!("{flag} must be between 1 and 1000");
    }
    Ok(parsed)
}

fn parse_path_radius_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let parsed = parse_i32_arg(flag, value)?;
    if !(1..=MAX_MOVEMENT_PERF_PATH_RADIUS).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_MOVEMENT_PERF_PATH_RADIUS}");
    }
    Ok(parsed)
}

fn parse_bool_arg(flag: &str, value: Option<String>) -> Result<bool> {
    let value = value.with_context(|| format!("{flag} requires true or false"))?;
    match value.as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("{flag} must be true or false, got `{value}`"),
    }
}

fn parse_screenshot_ui_arg(flag: &str, value: Option<String>) -> Result<HeadlessScreenshotUi> {
    let value = value.with_context(|| {
        format!("{flag} requires none, title, pause, options-title, or options-pause")
    })?;
    match value.as_str() {
        "none" | "off" | "false" | "0" => Ok(HeadlessScreenshotUi::None),
        "title" => Ok(HeadlessScreenshotUi::Title),
        "pause" => Ok(HeadlessScreenshotUi::Pause),
        "options-title" | "options_title" => Ok(HeadlessScreenshotUi::OptionsTitle),
        "options-pause" | "options_pause" | "options" => Ok(HeadlessScreenshotUi::OptionsPause),
        _ => bail!(
            "{flag} must be none, title, pause, options-title, or options-pause, got `{value}`"
        ),
    }
}

fn print_help() {
    println!(
        "mclone-native-client\n\n\
         Usage:\n\
           mclone-native-client [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--remote-addr 127.0.0.1:25565] [--section-occlusion true|false]\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\
           mclone-native-client --headless-ui /tmp/mclone-ui-title.png [--width 960] [--height 540]\n\
           mclone-native-client --screenshot /tmp/mclone-frame.png [--width 1280] [--height 720] [--screenshot-ui none|title|pause|options-title|options-pause] [--screenshot-debug-pane true|false] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --headless-chunk /tmp/mclone-native-chunk.png [--width 640] [--height 480] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--remote-addr 127.0.0.1:25565] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --headless-chunk-scenarios /tmp/mclone-native-camera [--width 960] [--height 640] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --movement-perf [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--movement-steps 12] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --timedemo [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--timedemo-frames 120] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --frame-budget-probe [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--chunk-radius 1] [--frame-budget-frames 240] [--target-hz 120] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
         Window mode streams chunks around a free-fly spectator camera with WASD, Space/X vertical movement, mouse-lock look, Shift boost, tilde debug pane toggle, O section-occlusion toggle, L fullbright toggle, and wheel speed controls. Use --disable-section-occlusion/--enable-section-occlusion and --force-fullbright/--disable-fullbright as shortcuts. Headless modes write PNGs for GPU validation. Perf modes write JSON. Timedemo loads a static chunk radius large enough to contain its camera path. Frame-budget probe runs a deterministic offscreen camera/chunk-interest script and counts work frames over an explicit target Hz budget."
    );
}

fn run_window(scene: SceneOptions, render_options: TexturedSectionRenderOptions) -> Result<()> {
    let runtime = WindowSceneRuntime::new(&scene)?;
    log::info!(
        "native window runtime seed={} initial_center=({}, {}) radius={} remote={:?} atlas={}x{}",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.chunk_radius,
        scene.remote_addr,
        runtime.mesh_assets.atlas.width,
        runtime.mesh_assets.atlas.height
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(
        runtime,
        SpectatorCamera::spawn_for_scene(&scene),
        render_options,
        scene.chunk_radius,
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[derive(Clone, Debug)]
struct MovementPerfReport {
    options: MovementPerfOptions,
    total_elapsed_ms: f64,
    steps: Vec<MovementPerfStepReport>,
}

#[derive(Clone, Copy, Debug)]
struct MovementPerfStepReport {
    index: usize,
    center: ChunkPos,
    elapsed_ms: f64,
    set_interest_ms: f64,
    poll_ms: f64,
    remesh_ms: f64,
    poll_count: usize,
    loaded_chunks: usize,
    client_visible_chunks: usize,
    active_ticket_chunks: usize,
    pending_unload_chunks: usize,
    block_ticking_chunks: usize,
    entity_ticking_chunks: usize,
    simulation_tick: u64,
    simulation_scheduler_tick_ms: f64,
    simulation_block_tick_ms: f64,
    simulation_fluid_tick_ms: f64,
    simulation_entity_tick_ms: f64,
    fluid_ticks_executed: usize,
    deferred_fluid_ticks: usize,
    fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
    rebuilt_sections: usize,
    removed_sections: usize,
    rebuilt_vertices: u32,
    rebuilt_faces: u32,
    rebuilt_indices: u32,
    visibility_graph_build_count: usize,
    visibility_graph_total_ms: f64,
    visibility_graph_average_ms: f64,
    visibility_graph_worst_ms: f64,
    loaded_sections: usize,
    visible_sections: usize,
    frustum_sections: usize,
    graph_cull_enabled: bool,
    graph_culled_sections: usize,
    loaded_faces: u32,
    visible_faces: u32,
    frustum_faces: u32,
    graph_culled_faces: u32,
    loaded_indices: u32,
    visible_indices: u32,
    frustum_indices: u32,
    graph_culled_indices: u32,
}

impl MovementPerfReport {
    fn validate(&self) -> Result<()> {
        let expected_visible_chunks = square_count(self.options.scene.chunk_radius)?;
        for step in &self.steps {
            if step.loaded_chunks != expected_visible_chunks {
                bail!(
                    "movement step {} loaded_chunks={} expected {expected_visible_chunks}",
                    step.index,
                    step.loaded_chunks
                );
            }
            if step.client_visible_chunks != expected_visible_chunks {
                bail!(
                    "movement step {} client_visible_chunks={} expected {expected_visible_chunks}",
                    step.index,
                    step.client_visible_chunks
                );
            }
            if step.loaded_sections == 0 {
                bail!("movement step {} produced no render sections", step.index);
            }
            if step.visible_sections == 0 {
                bail!(
                    "movement step {} frustum culled every render section",
                    step.index
                );
            }
            if step.visible_sections > step.loaded_sections {
                bail!(
                    "movement step {} visible_sections={} exceeds loaded_sections={}",
                    step.index,
                    step.visible_sections,
                    step.loaded_sections
                );
            }
            if step.visible_faces == 0 {
                bail!(
                    "movement step {} frustum culled every render face",
                    step.index
                );
            }
            if step.visible_faces > step.loaded_faces {
                bail!(
                    "movement step {} visible_faces={} exceeds loaded_faces={}",
                    step.index,
                    step.visible_faces,
                    step.loaded_faces
                );
            }
        }
        if self
            .steps
            .iter()
            .all(|step| step.visible_sections == step.loaded_sections)
        {
            bail!("movement perf smoke did not cull any loaded render section");
        }
        Ok(())
    }

    fn print_json(&self) {
        println!("{{");
        print_benchmark_metadata("native_runtime_movement", "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.options.scene.chunk_x, self.options.scene.chunk_z
        );
        println!("  \"chunk_radius\": {},", self.options.scene.chunk_radius);
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"path_radius_chunks\": {},",
            self.options.path_radius_chunks
        );
        println!("  \"steps\": {},", self.options.steps);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"total_elapsed_ms\": {:.3},", self.total_elapsed_ms);
        println!("  \"step_reports\": [");
        for (index, step) in self.steps.iter().enumerate() {
            let suffix = if index + 1 == self.steps.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!("      \"index\": {},", step.index);
            println!(
                "      \"center\": {{ \"x\": {}, \"z\": {} }},",
                step.center.x, step.center.z
            );
            println!("      \"elapsed_ms\": {:.3},", step.elapsed_ms);
            println!("      \"set_interest_ms\": {:.3},", step.set_interest_ms);
            println!("      \"poll_ms\": {:.3},", step.poll_ms);
            println!("      \"remesh_ms\": {:.3},", step.remesh_ms);
            println!("      \"poll_count\": {},", step.poll_count);
            println!("      \"loaded_chunks\": {},", step.loaded_chunks);
            println!(
                "      \"client_visible_chunks\": {},",
                step.client_visible_chunks
            );
            println!(
                "      \"active_ticket_chunks\": {},",
                step.active_ticket_chunks
            );
            println!(
                "      \"pending_unload_chunks\": {},",
                step.pending_unload_chunks
            );
            println!(
                "      \"block_ticking_chunks\": {},",
                step.block_ticking_chunks
            );
            println!(
                "      \"entity_ticking_chunks\": {},",
                step.entity_ticking_chunks
            );
            println!("      \"simulation_tick\": {},", step.simulation_tick);
            println!(
                "      \"simulation_scheduler_tick_ms\": {:.3},",
                step.simulation_scheduler_tick_ms
            );
            println!(
                "      \"simulation_block_tick_ms\": {:.3},",
                step.simulation_block_tick_ms
            );
            println!(
                "      \"simulation_fluid_tick_ms\": {:.3},",
                step.simulation_fluid_tick_ms
            );
            println!(
                "      \"simulation_entity_tick_ms\": {:.3},",
                step.simulation_entity_tick_ms
            );
            println!(
                "      \"fluid_ticks_executed\": {},",
                step.fluid_ticks_executed
            );
            println!(
                "      \"deferred_fluid_ticks\": {},",
                step.deferred_fluid_ticks
            );
            println!(
                "      \"fluid_mutated_blocks\": {},",
                step.fluid_mutated_blocks
            );
            println!(
                "      \"scheduled_fluid_ticks\": {},",
                step.scheduled_fluid_ticks
            );
            println!("      \"rebuilt_sections\": {},", step.rebuilt_sections);
            println!("      \"removed_sections\": {},", step.removed_sections);
            println!("      \"rebuilt_vertices\": {},", step.rebuilt_vertices);
            println!("      \"rebuilt_faces\": {},", step.rebuilt_faces);
            println!("      \"rebuilt_indices\": {},", step.rebuilt_indices);
            println!(
                "      \"visibility_graph_build_count\": {},",
                step.visibility_graph_build_count
            );
            println!(
                "      \"visibility_graph_total_ms\": {:.3},",
                step.visibility_graph_total_ms
            );
            println!(
                "      \"visibility_graph_average_ms\": {:.6},",
                step.visibility_graph_average_ms
            );
            println!(
                "      \"visibility_graph_worst_ms\": {:.6},",
                step.visibility_graph_worst_ms
            );
            println!("      \"loaded_sections\": {},", step.loaded_sections);
            println!("      \"visible_sections\": {},", step.visible_sections);
            println!("      \"frustum_sections\": {},", step.frustum_sections);
            println!("      \"graph_cull_enabled\": {},", step.graph_cull_enabled);
            println!(
                "      \"graph_culled_sections\": {},",
                step.graph_culled_sections
            );
            println!("      \"loaded_faces\": {},", step.loaded_faces);
            println!("      \"visible_faces\": {},", step.visible_faces);
            println!("      \"frustum_faces\": {},", step.frustum_faces);
            println!("      \"graph_culled_faces\": {},", step.graph_culled_faces);
            println!("      \"loaded_indices\": {},", step.loaded_indices);
            println!("      \"visible_indices\": {},", step.visible_indices);
            println!("      \"frustum_indices\": {},", step.frustum_indices);
            println!(
                "      \"graph_culled_indices\": {}",
                step.graph_culled_indices
            );
            println!("    }}{suffix}");
        }
        println!("  ]");
        println!("}}");
    }
}

#[derive(Clone, Debug)]
struct TimedemoReport {
    options: TimedemoOptions,
    loaded_chunk_radius: i32,
    visibility_graph_stats: VisibilityGraphBuildStats,
    scene_build_ms: f64,
    section_count: usize,
    vertex_count: u32,
    face_count: u32,
    index_count: u32,
    render: mclone_render::headless::HeadlessTimedemoReport,
}

#[derive(Clone, Debug)]
struct FrameBudgetProbeReport {
    options: FrameBudgetProbeOptions,
    runtime_setup_ms: f64,
    initial_poll_count: usize,
    initial_poll_ms: f64,
    initial_remesh_ms: f64,
    initial_section_count: usize,
    initial_face_count: u32,
    initial_index_count: u32,
    headless: mclone_render::headless::HeadlessFrameLoopReport,
    frames: Vec<FrameBudgetProbeFrameReport>,
}

#[derive(Clone, Copy, Debug)]
struct FrameBudgetProbeFrameReport {
    index: usize,
    center: ChunkPos,
    interest_center_changed: bool,
    interest_updates_changed: bool,
    runtime_changed: bool,
    set_interest_ms: f64,
    poll_ms: f64,
    remesh_ms: f64,
    upload_ms: f64,
    render_ms: f64,
    rebuilt_sections: usize,
    removed_sections: usize,
    uploaded_sections: usize,
    upload_removed_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
    loaded_chunks: usize,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    drawn_sections: usize,
    drawn_indices: u32,
}

impl Default for FrameBudgetProbeFrameReport {
    fn default() -> Self {
        Self {
            index: 0,
            center: ChunkPos::new(0, 0),
            interest_center_changed: false,
            interest_updates_changed: false,
            runtime_changed: false,
            set_interest_ms: 0.0,
            poll_ms: 0.0,
            remesh_ms: 0.0,
            upload_ms: 0.0,
            render_ms: 0.0,
            rebuilt_sections: 0,
            removed_sections: 0,
            uploaded_sections: 0,
            upload_removed_sections: 0,
            uploaded_vertices: 0,
            uploaded_indices: 0,
            loaded_chunks: 0,
            pending_jobs: 0,
            pending_publications: 0,
            pending_render_chunks: 0,
            drawn_sections: 0,
            drawn_indices: 0,
        }
    }
}

impl FrameBudgetProbeFrameReport {
    fn add_section_timing(&mut self, timing: FrameBudgetProbeSectionTiming) {
        self.remesh_ms += timing.remesh_ms;
        self.upload_ms += timing.upload_ms;
        self.rebuilt_sections += timing.rebuilt_sections;
        self.removed_sections += timing.removed_sections;
        self.uploaded_sections += timing.uploaded_sections;
        self.upload_removed_sections += timing.upload_removed_sections;
        self.uploaded_vertices += timing.uploaded_vertices;
        self.uploaded_indices += timing.uploaded_indices;
    }
}

struct FrameBudgetProbeState {
    runtime: WindowSceneRuntime,
    depth: ChunkDepthTarget,
    draw: TexturedSectionDrawResources,
    gui: GuiRenderer,
    ui: NativeUi,
    render_stats: RenderStreamStats,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameBudgetProbeSectionTiming {
    remesh_ms: f64,
    upload_ms: f64,
    rebuilt_sections: usize,
    removed_sections: usize,
    uploaded_sections: usize,
    upload_removed_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
}

impl TimedemoReport {
    fn validate(&self) -> Result<()> {
        if self.render.frame_count != self.options.frames {
            bail!(
                "timedemo rendered {} frames, expected {}",
                self.render.frame_count,
                self.options.frames
            );
        }
        if self.section_count == 0 || self.index_count == 0 {
            bail!("timedemo scene produced no render sections");
        }
        if self.render.max_drawn_section_count == 0 {
            bail!("timedemo drew no render sections");
        }
        Ok(())
    }

    fn print_json(&self) {
        println!("{{");
        print_benchmark_metadata("native_render_timedemo", "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.options.scene.chunk_x, self.options.scene.chunk_z
        );
        println!("  \"chunk_radius\": {},", self.options.scene.chunk_radius);
        println!("  \"loaded_chunk_radius\": {},", self.loaded_chunk_radius);
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"path_radius_chunks\": {},",
            self.options.path_radius_chunks
        );
        println!("  \"frames\": {},", self.options.frames);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"scene_build_ms\": {:.3},", self.scene_build_ms);
        println!(
            "  \"visibility_graph_build_count\": {},",
            self.visibility_graph_stats.build_count
        );
        println!(
            "  \"visibility_graph_total_ms\": {:.3},",
            self.visibility_graph_stats.total_ms
        );
        println!(
            "  \"visibility_graph_average_ms\": {:.6},",
            self.visibility_graph_stats.average_ms()
        );
        println!(
            "  \"visibility_graph_worst_ms\": {:.6},",
            self.visibility_graph_stats.worst_ms
        );
        println!("  \"section_count\": {},", self.section_count);
        println!("  \"vertex_count\": {},", self.vertex_count);
        println!("  \"face_count\": {},", self.face_count);
        println!("  \"index_count\": {},", self.index_count);
        println!("  \"render\": {{");
        println!("    \"frame_count\": {},", self.render.frame_count);
        println!("    \"setup_ms\": {:.3},", self.render.setup_ms);
        println!("    \"total_frame_ms\": {:.3},", self.render.total_frame_ms);
        println!(
            "    \"average_frame_ms\": {:.3},",
            self.render.average_frame_ms
        );
        println!("    \"min_frame_ms\": {:.3},", self.render.min_frame_ms);
        println!("    \"max_frame_ms\": {:.3},", self.render.max_frame_ms);
        println!(
            "    \"loaded_section_count\": {},",
            self.render.loaded_section_count
        );
        println!(
            "    \"average_drawn_section_count\": {:.3},",
            self.render.average_drawn_section_count
        );
        println!(
            "    \"max_drawn_section_count\": {},",
            self.render.max_drawn_section_count
        );
        println!(
            "    \"average_frustum_section_count\": {:.3},",
            self.render.average_frustum_section_count
        );
        println!(
            "    \"max_frustum_section_count\": {},",
            self.render.max_frustum_section_count
        );
        println!(
            "    \"graph_cull_enabled_frame_count\": {},",
            self.render.graph_cull_enabled_frame_count
        );
        println!(
            "    \"average_graph_culled_section_count\": {:.3},",
            self.render.average_graph_culled_section_count
        );
        println!(
            "    \"max_graph_culled_section_count\": {},",
            self.render.max_graph_culled_section_count
        );
        println!(
            "    \"loaded_index_count\": {},",
            self.render.loaded_index_count
        );
        println!(
            "    \"average_drawn_index_count\": {:.3},",
            self.render.average_drawn_index_count
        );
        println!(
            "    \"max_drawn_index_count\": {},",
            self.render.max_drawn_index_count
        );
        println!(
            "    \"average_frustum_index_count\": {:.3},",
            self.render.average_frustum_index_count
        );
        println!(
            "    \"max_frustum_index_count\": {},",
            self.render.max_frustum_index_count
        );
        println!(
            "    \"average_graph_culled_index_count\": {:.3},",
            self.render.average_graph_culled_index_count
        );
        println!(
            "    \"max_graph_culled_index_count\": {}",
            self.render.max_graph_culled_index_count
        );
        println!("  }}");
        println!("}}");
    }
}

impl FrameBudgetProbeReport {
    fn target_frame_ms(&self) -> f64 {
        target_frame_ms(self.options.target_hz)
    }

    fn validate(&self) -> Result<()> {
        if self.headless.frame_count != self.options.frames {
            bail!(
                "frame-budget probe rendered {} frames, expected {}",
                self.headless.frame_count,
                self.options.frames
            );
        }
        if self.frames.len() != self.options.frames {
            bail!(
                "frame-budget probe recorded {} frame reports, expected {}",
                self.frames.len(),
                self.options.frames
            );
        }
        if self.initial_section_count == 0 || self.initial_index_count == 0 {
            bail!("frame-budget probe initial scene produced no render sections");
        }
        Ok(())
    }

    fn print_json(&self) {
        let target_frame_ms = self.target_frame_ms();
        let over_budget = frame_budget_over_count(&self.headless.frames, target_frame_ms, 1.0);
        let over_2x = frame_budget_over_count(&self.headless.frames, target_frame_ms, 2.0);
        let over_4x = frame_budget_over_count(&self.headless.frames, target_frame_ms, 4.0);
        let p95 = frame_budget_percentile_ms(&self.headless.frames, 0.95);
        let p99 = frame_budget_percentile_ms(&self.headless.frames, 0.99);
        println!("{{");
        print_benchmark_metadata("native_frame_budget_probe", "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.options.scene.chunk_x, self.options.scene.chunk_z
        );
        println!("  \"chunk_radius\": {},", self.options.scene.chunk_radius);
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"path_radius_chunks\": {},",
            self.options.path_radius_chunks
        );
        println!("  \"frames\": {},", self.options.frames);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"target_hz\": {:.3},", self.options.target_hz);
        println!("  \"target_frame_ms\": {:.3},", target_frame_ms);
        println!("  \"over_budget_frames\": {},", over_budget);
        println!("  \"over_2x_budget_frames\": {},", over_2x);
        println!("  \"over_4x_budget_frames\": {},", over_4x);
        println!("  \"p95_frame_ms\": {:.3},", p95);
        println!("  \"p99_frame_ms\": {:.3},", p99);
        println!("  \"max_frame_ms\": {:.3},", self.headless.max_frame_ms);
        println!("  \"runtime_setup_ms\": {:.3},", self.runtime_setup_ms);
        println!("  \"initial_poll_count\": {},", self.initial_poll_count);
        println!("  \"initial_poll_ms\": {:.3},", self.initial_poll_ms);
        println!("  \"initial_remesh_ms\": {:.3},", self.initial_remesh_ms);
        println!(
            "  \"initial_section_count\": {},",
            self.initial_section_count
        );
        println!("  \"initial_face_count\": {},", self.initial_face_count);
        println!("  \"initial_index_count\": {},", self.initial_index_count);
        println!("  \"headless\": {{");
        println!("    \"setup_ms\": {:.3},", self.headless.setup_ms);
        println!(
            "    \"total_frame_ms\": {:.3},",
            self.headless.total_frame_ms
        );
        println!(
            "    \"average_frame_ms\": {:.3},",
            self.headless.average_frame_ms
        );
        println!("    \"min_frame_ms\": {:.3},", self.headless.min_frame_ms);
        println!("    \"max_frame_ms\": {:.3}", self.headless.max_frame_ms);
        println!("  }},");
        println!("  \"frame_reports\": [");
        for (index, frame) in self.frames.iter().enumerate() {
            let timing = self.headless.frames.get(index).copied().unwrap_or_default();
            let suffix = if index + 1 == self.frames.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!("      \"index\": {},", frame.index);
            println!(
                "      \"center\": {{ \"x\": {}, \"z\": {} }},",
                frame.center.x, frame.center.z
            );
            println!("      \"frame_ms\": {:.3},", timing.frame_ms);
            println!(
                "      \"budget_multiple\": {:.3},",
                timing.frame_ms / target_frame_ms
            );
            println!(
                "      \"over_budget\": {},",
                timing.frame_ms > target_frame_ms
            );
            println!(
                "      \"interest_center_changed\": {},",
                frame.interest_center_changed
            );
            println!(
                "      \"interest_updates_changed\": {},",
                frame.interest_updates_changed
            );
            println!("      \"runtime_changed\": {},", frame.runtime_changed);
            println!("      \"set_interest_ms\": {:.3},", frame.set_interest_ms);
            println!("      \"poll_ms\": {:.3},", frame.poll_ms);
            println!("      \"remesh_ms\": {:.3},", frame.remesh_ms);
            println!("      \"upload_ms\": {:.3},", frame.upload_ms);
            println!("      \"render_ms\": {:.3},", frame.render_ms);
            println!("      \"encode_ms\": {:.3},", timing.encode_ms);
            println!("      \"submit_ms\": {:.3},", timing.submit_ms);
            println!("      \"device_poll_ms\": {:.3},", timing.device_poll_ms);
            println!("      \"rebuilt_sections\": {},", frame.rebuilt_sections);
            println!("      \"removed_sections\": {},", frame.removed_sections);
            println!("      \"uploaded_sections\": {},", frame.uploaded_sections);
            println!(
                "      \"upload_removed_sections\": {},",
                frame.upload_removed_sections
            );
            println!("      \"uploaded_vertices\": {},", frame.uploaded_vertices);
            println!("      \"uploaded_indices\": {},", frame.uploaded_indices);
            println!("      \"loaded_chunks\": {},", frame.loaded_chunks);
            println!("      \"pending_jobs\": {},", frame.pending_jobs);
            println!(
                "      \"pending_publications\": {},",
                frame.pending_publications
            );
            println!(
                "      \"pending_render_chunks\": {},",
                frame.pending_render_chunks
            );
            println!("      \"drawn_sections\": {},", frame.drawn_sections);
            println!("      \"drawn_indices\": {}", frame.drawn_indices);
            println!("    }}{suffix}");
        }
        println!("  ]");
        println!("}}");
    }
}

fn target_frame_ms(target_hz: f64) -> f64 {
    1000.0 / target_hz.max(1.0)
}

fn frame_budget_over_count(
    frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    target_frame_ms: f64,
    multiplier: f64,
) -> usize {
    let threshold = target_frame_ms * multiplier;
    frames
        .iter()
        .filter(|frame| frame.frame_ms > threshold)
        .count()
}

fn frame_budget_percentile_ms(
    frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    percentile: f64,
) -> f64 {
    if frames.is_empty() {
        return 0.0;
    }
    let mut values = frames
        .iter()
        .map(|frame| frame.frame_ms)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let clamped = percentile.clamp(0.0, 1.0);
    let index = ((values.len() - 1) as f64 * clamped).ceil() as usize;
    values[index.min(values.len() - 1)]
}

fn run_movement_perf_smoke(options: &MovementPerfOptions) -> Result<MovementPerfReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--movement-perf currently requires the local integrated server path");
    }
    let total_start = Instant::now();
    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
    let mut steps = Vec::with_capacity(options.steps);

    for index in 0..options.steps {
        let step_start = Instant::now();
        let spectator = circular_movement_spectator(
            &options.scene,
            options.path_radius_chunks,
            index,
            options.steps,
        );
        let center = spectator.chunk_pos();
        let set_interest_start = Instant::now();
        runtime.set_interest_center(center)?;
        let set_interest_ms = elapsed_ms(set_interest_start.elapsed());
        let (poll_count, poll_ms) = poll_window_runtime_until_idle(&mut runtime)?;

        let remesh_start = Instant::now();
        let section_update = runtime.sync_all_render_sections()?;
        let sections = runtime.cached_sections();
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let camera = spectator.camera(runtime.radius_chunks);
        let render_view = camera.render_view(options.width, options.height);
        let visibility = textured_section_visibility_stats_with_options(
            &sections,
            render_view,
            options.render_options,
        );
        let stats = runtime.stats();

        steps.push(MovementPerfStepReport {
            index,
            center,
            elapsed_ms: elapsed_ms(step_start.elapsed()),
            set_interest_ms,
            poll_ms,
            remesh_ms,
            poll_count,
            loaded_chunks: stats.loaded_chunks,
            client_visible_chunks: stats.client_visible_chunks,
            active_ticket_chunks: stats.active_ticket_chunks,
            pending_unload_chunks: stats.pending_unload_chunks,
            block_ticking_chunks: stats.block_ticking_chunks,
            entity_ticking_chunks: stats.entity_ticking_chunks,
            simulation_tick: stats.last_simulation_tick,
            simulation_scheduler_tick_ms: stats.last_simulation_scheduler_tick_ms,
            simulation_block_tick_ms: stats.last_simulation_block_tick_ms,
            simulation_fluid_tick_ms: stats.last_simulation_fluid_tick_ms,
            simulation_entity_tick_ms: stats.last_simulation_entity_tick_ms,
            fluid_ticks_executed: stats.last_simulation_fluid_ticks_executed,
            deferred_fluid_ticks: stats.last_simulation_deferred_fluid_ticks,
            fluid_mutated_blocks: stats.last_simulation_fluid_mutated_blocks,
            scheduled_fluid_ticks: stats.scheduled_fluid_ticks,
            rebuilt_sections: section_update.rebuilt_section_count(),
            removed_sections: section_update.removed_section_count(),
            rebuilt_vertices: section_update.rebuilt_vertex_count,
            rebuilt_faces: section_update.rebuilt_face_count(),
            rebuilt_indices: section_update.rebuilt_index_count,
            visibility_graph_build_count: section_update.visibility_graph_stats.build_count,
            visibility_graph_total_ms: section_update.visibility_graph_stats.total_ms,
            visibility_graph_average_ms: section_update.visibility_graph_stats.average_ms(),
            visibility_graph_worst_ms: section_update.visibility_graph_stats.worst_ms,
            loaded_sections: visibility.loaded_section_count,
            visible_sections: visibility.drawn_section_count,
            frustum_sections: visibility.frustum_section_count,
            graph_cull_enabled: visibility.graph_cull_enabled,
            graph_culled_sections: visibility.graph_culled_section_count,
            loaded_faces: visibility.loaded_face_count(),
            visible_faces: visibility.drawn_face_count(),
            frustum_faces: visibility.frustum_face_count(),
            graph_culled_faces: visibility.graph_culled_face_count(),
            loaded_indices: visibility.loaded_index_count,
            visible_indices: visibility.drawn_index_count,
            frustum_indices: visibility.frustum_index_count,
            graph_culled_indices: visibility.graph_culled_index_count,
        });
    }

    Ok(MovementPerfReport {
        options: options.clone(),
        total_elapsed_ms: elapsed_ms(total_start.elapsed()),
        steps,
    })
}

fn run_timedemo(options: &TimedemoOptions) -> Result<TimedemoReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--timedemo currently requires the local integrated server path");
    }
    let loaded_scene = timedemo_loaded_scene(options)?;
    let scene_start = Instant::now();
    let scene_mesh = build_scene_textured_sections(&loaded_scene)?;
    let scene_build_ms = elapsed_ms(scene_start.elapsed());
    let cameras = timedemo_cameras(&options.scene, options.path_radius_chunks, options.frames);
    let render = run_headless_textured_sections_timedemo(
        HeadlessTimedemoOptions {
            width: options.width,
            height: options.height,
            color: mclone_render::default_clear_color(),
            cameras,
            render_options: options.render_options,
        },
        &scene_mesh.sections,
        scene_mesh.atlas.as_upload(),
    )?;

    let vertex_count = scene_mesh
        .sections
        .iter()
        .map(|section| section.stats().vertex_count)
        .sum();
    let index_count = scene_mesh
        .sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum();
    Ok(TimedemoReport {
        options: options.clone(),
        loaded_chunk_radius: loaded_scene.chunk_radius,
        visibility_graph_stats: scene_mesh.visibility_graph_stats,
        scene_build_ms,
        section_count: scene_mesh.sections.len(),
        vertex_count,
        face_count: quad_face_count_from_indices(index_count),
        index_count,
        render,
    })
}

fn probe_sync_upload_sections(
    frame: &RenderFrameContext<'_>,
    state: &mut FrameBudgetProbeState,
) -> Result<FrameBudgetProbeSectionTiming> {
    let remesh_start = Instant::now();
    let section_update = state.runtime.sync_render_sections()?;
    let remesh_ms = elapsed_ms(remesh_start.elapsed());
    let upload_start = Instant::now();
    let upload_report = state
        .draw
        .apply_section_updates(
            frame.device,
            &section_update.rebuilt_sections,
            &section_update.removed_section_keys,
        )
        .context("failed to upload frame-budget probe section updates")?;
    let upload_ms = elapsed_ms(upload_start.elapsed());

    state.render_stats.section_count = state.draw.section_count();
    state.render_stats.index_count = state.draw.index_count();
    state.render_stats.face_count = quad_face_count_from_indices(state.render_stats.index_count);
    state.render_stats.drawn_section_count = 0;
    state.render_stats.drawn_face_count = 0;
    state.render_stats.drawn_index_count = 0;
    record_render_section_update_stats(&mut state.render_stats, &section_update, upload_report);
    state.render_stats.last_remesh_ms = remesh_ms;
    state.render_stats.last_upload_ms = upload_ms;

    Ok(FrameBudgetProbeSectionTiming {
        remesh_ms,
        upload_ms,
        rebuilt_sections: section_update.rebuilt_section_count(),
        removed_sections: section_update.removed_section_count(),
        uploaded_sections: upload_report.uploaded_section_count,
        upload_removed_sections: upload_report.removed_section_count,
        uploaded_vertices: upload_report.uploaded_vertex_count,
        uploaded_indices: upload_report.uploaded_index_count,
    })
}

fn run_frame_budget_probe(options: &FrameBudgetProbeOptions) -> Result<FrameBudgetProbeReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--frame-budget-probe currently requires the local integrated server path");
    }

    let runtime_setup_start = Instant::now();
    let initial_spectator = circular_movement_spectator(
        &options.scene,
        options.path_radius_chunks,
        0,
        options.frames,
    );
    let initial_center = initial_spectator.chunk_pos();
    let mut runtime_scene = options.scene.clone();
    runtime_scene.chunk_x = initial_center.x;
    runtime_scene.chunk_z = initial_center.z;
    let mut runtime = WindowSceneRuntime::new(&runtime_scene)?;
    let (initial_poll_count, initial_poll_ms) = poll_window_runtime_until_idle(&mut runtime)?;
    let initial_remesh_start = Instant::now();
    let initial_update = runtime.sync_all_render_sections()?;
    let initial_sections = runtime.cached_sections();
    let initial_remesh_ms = elapsed_ms(initial_remesh_start.elapsed());
    if initial_sections.is_empty() {
        bail!(
            "frame-budget probe seed={} center=({}, {}) radius={} produced no initial render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.chunk_radius
        );
    }
    let initial_index_count = initial_sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum::<u32>();
    let initial_section_count = initial_sections.len();
    let initial_face_count = quad_face_count_from_indices(initial_index_count);
    let runtime_setup_ms = elapsed_ms(runtime_setup_start.elapsed());

    let mut frame_reports = Vec::with_capacity(options.frames);
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let path_radius_chunks = options.path_radius_chunks;
    let frame_count = options.frames;
    let chunk_radius = options.scene.chunk_radius;
    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: initial_update.rebuilt_section_count(),
        removed_section_count: initial_update.removed_section_count(),
        uploaded_vertex_count: initial_update.rebuilt_vertex_count,
        uploaded_index_count: initial_update.rebuilt_index_count,
    };

    let (headless, _state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: options.frames,
        },
        move |device, queue, format, size| {
            let depth = ChunkDepthTarget::new(device, size[0], size[1]);
            let draw = TexturedSectionDrawResources::new(
                device,
                queue,
                format,
                &initial_sections,
                runtime.mesh_assets.atlas.as_upload(),
            )?;
            let gui = GuiRenderer::new(device, format);
            let mut render_stats = RenderStreamStats {
                section_count: draw.section_count(),
                index_count: draw.index_count(),
                face_count: quad_face_count_from_indices(draw.index_count()),
                ..RenderStreamStats::default()
            };
            record_render_section_update_stats(&mut render_stats, &initial_update, initial_upload);
            let mut ui = NativeUi::new(chunk_radius);
            ui.set_screen(None);
            ui.set_scale(GuiScale::from_pixels(size[0], size[1]));
            Ok(FrameBudgetProbeState {
                runtime,
                depth,
                draw,
                gui,
                ui,
                render_stats,
            })
        },
        |index, frame, state| {
            let spectator =
                circular_movement_spectator(&scene, path_radius_chunks, index, frame_count);
            let center = spectator.chunk_pos();
            let mut report = FrameBudgetProbeFrameReport {
                index,
                center,
                ..FrameBudgetProbeFrameReport::default()
            };

            let set_interest_start = Instant::now();
            report.interest_center_changed = state.runtime.interest_center != center;
            report.interest_updates_changed = state.runtime.set_interest_center(center)?;
            report.set_interest_ms = elapsed_ms(set_interest_start.elapsed());
            let poll_start = Instant::now();
            report.runtime_changed = state.runtime.poll()?;
            report.poll_ms = elapsed_ms(poll_start.elapsed());
            if state.runtime.has_pending_render_work() {
                let update = probe_sync_upload_sections(&frame, state)?;
                report.add_section_timing(update);
            }

            let camera = spectator.camera(state.runtime.radius_chunks);
            let render_start = Instant::now();
            render_full_frame(
                frame,
                &state.depth,
                &mut state.draw,
                &mut state.gui,
                camera,
                render_options,
                FramePacingUiState::default(),
                &state.ui,
                None,
                &mut state.render_stats,
            )?;
            report.render_ms = elapsed_ms(render_start.elapsed());

            let stats = state.runtime.stats();
            report.loaded_chunks = stats.loaded_chunks;
            report.pending_jobs = stats.pending_jobs;
            report.pending_publications = stats.pending_publications;
            report.pending_render_chunks = stats.pending_render_chunks;
            report.drawn_sections = state.render_stats.drawn_section_count;
            report.drawn_indices = state.render_stats.drawn_index_count;
            frame_reports.push(report);
            Ok(())
        },
    )?;

    Ok(FrameBudgetProbeReport {
        options: options.clone(),
        runtime_setup_ms,
        initial_poll_count,
        initial_poll_ms,
        initial_remesh_ms,
        initial_section_count,
        initial_face_count,
        initial_index_count,
        headless,
        frames: frame_reports,
    })
}

fn timedemo_loaded_scene(options: &TimedemoOptions) -> Result<SceneOptions> {
    let loaded_radius = options.scene.chunk_radius.max(options.path_radius_chunks);
    if loaded_radius > MAX_CHUNK_RADIUS {
        bail!(
            "--timedemo requires static loaded radius {loaded_radius}, but the current max is {MAX_CHUNK_RADIUS}; lower --path-radius"
        );
    }
    let mut scene = options.scene.clone();
    scene.chunk_radius = loaded_radius;
    Ok(scene)
}

fn circular_movement_spectator(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    index: usize,
    steps: usize,
) -> SpectatorCamera {
    let origin_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let origin_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let radius_blocks = path_radius_chunks.max(1) as f32 * CHUNK_WIDTH as f32;
    let angle = std::f32::consts::TAU * index as f32 / steps.max(1) as f32;
    let position = Vec3::new(
        origin_x + radius_blocks * angle.cos(),
        88.0,
        origin_z + radius_blocks * angle.sin(),
    );
    let target = Vec3::new(origin_x, 56.0, origin_z);
    let direction = (target - position).normalize_or_zero();
    let horizontal = Vec3::new(direction.x, 0.0, direction.z).length();
    SpectatorCamera {
        position,
        yaw: direction.x.atan2(direction.z),
        pitch: direction.y.atan2(horizontal),
        speed: SPECTATOR_BASE_SPEED,
    }
}

fn timedemo_cameras(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    frames: usize,
) -> Vec<ChunkCamera> {
    (0..frames)
        .map(|index| timedemo_camera(scene, path_radius_chunks, index, frames))
        .collect()
}

fn timedemo_camera(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    index: usize,
    frames: usize,
) -> ChunkCamera {
    let center_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let center_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let radius_blocks = path_radius_chunks.max(1) as f32 * CHUNK_WIDTH as f32;
    let angle = std::f32::consts::TAU * index as f32 / frames.max(1) as f32;
    let bob = (angle * 2.0).sin() * 8.0;
    ChunkCamera {
        eye: [
            center_x + radius_blocks * angle.cos(),
            92.0 + bob,
            center_z + radius_blocks * angle.sin(),
        ],
        target: [center_x, 52.0, center_z],
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 65.0_f32.to_radians(),
        z_near: 0.1,
        z_far: 900.0,
    }
}

fn square_count(radius: i32) -> Result<usize> {
    if radius < 0 {
        bail!("radius must be non-negative");
    }
    let side = usize::try_from(radius)
        .context("radius exceeds usize")?
        .saturating_mul(2)
        .saturating_add(1);
    Ok(side * side)
}

#[derive(Clone, Debug)]
struct SceneTexturedSections {
    sections: Vec<TexturedRenderSectionMesh>,
    visibility_graph_stats: VisibilityGraphBuildStats,
    atlas: TextureAtlasImage,
}

impl SceneTexturedSections {
    #[cfg(test)]
    fn section_count(&self) -> usize {
        self.sections.len()
    }

    #[cfg(test)]
    fn index_count(&self) -> u32 {
        self.sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}

#[derive(Clone, Debug)]
struct TextureAtlasImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl TextureAtlasImage {
    fn as_upload(&self) -> ChunkTextureAtlas<'_> {
        ChunkTextureAtlas {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
        }
    }
}

fn build_scene_textured_sections(scene: &SceneOptions) -> Result<SceneTexturedSections> {
    let client = build_scene_client_runtime(scene)?;
    let mesh_assets = load_textured_mesh_assets()?;
    let build = build_client_textured_sections(&client, &mesh_assets.catalog)?;
    if build.sections.is_empty() {
        bail!(
            "generated chunk area seed={} center=({}, {}) radius={} produced no textured render sections",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.chunk_radius
        );
    }
    Ok(SceneTexturedSections {
        sections: build.sections,
        visibility_graph_stats: build.visibility_graph,
        atlas: mesh_assets.atlas,
    })
}

fn build_client_textured_sections(
    client: &ClientRuntime,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = mesh_chunks_from_client(client)?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_with_stats(&inputs, catalog)
        .context("failed to build textured sections")
}

fn mesh_chunks_from_client(client: &ClientRuntime) -> Result<Vec<MeshChunkBlocks>> {
    client
        .chunk_snapshots()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()
}

fn textured_mesh_inputs(chunks: &[MeshChunkBlocks]) -> Vec<TexturedChunkMeshInput<'_>> {
    chunks
        .iter()
        .map(|chunk| {
            TexturedChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
            .with_light_sections(&chunk.light_sections)
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
struct CachedTexturedRenderSections {
    sections: BTreeMap<RenderSectionKey, TexturedRenderSectionMesh>,
}

#[derive(Clone, Debug, Default)]
struct RenderSectionCacheUpdate {
    rebuilt_sections: Vec<TexturedRenderSectionMesh>,
    removed_section_keys: BTreeSet<RenderSectionKey>,
    rebuilt_vertex_count: u32,
    rebuilt_index_count: u32,
    visibility_graph_stats: VisibilityGraphBuildStats,
}

impl RenderSectionCacheUpdate {
    fn rebuilt_section_count(&self) -> usize {
        self.rebuilt_sections.len()
    }

    fn removed_section_count(&self) -> usize {
        self.removed_section_keys.len()
    }

    fn rebuilt_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.rebuilt_index_count)
    }
}

impl CachedTexturedRenderSections {
    fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.sections
            .keys()
            .any(|key| key.chunk_x == pos.x && key.chunk_z == pos.z)
    }

    fn sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.sections.values().cloned().collect()
    }

    fn rebuild_dirty(
        &mut self,
        client: &ClientRuntime,
        catalog: &TexturedMeshCatalog,
        dirty_chunks: &BTreeSet<ChunkPos>,
    ) -> Result<RenderSectionCacheUpdate> {
        if dirty_chunks.is_empty() {
            return Ok(RenderSectionCacheUpdate::default());
        }

        let chunks = mesh_chunks_from_client(client)?;
        let inputs = textured_mesh_inputs(&chunks);
        let loaded_targets = chunks
            .iter()
            .map(|chunk| (chunk.chunk_x, chunk.chunk_z))
            .collect::<BTreeSet<_>>();
        let target_chunks = dirty_chunks
            .iter()
            .map(|pos| (pos.x, pos.z))
            .filter(|coord| loaded_targets.contains(coord))
            .collect::<BTreeSet<_>>();
        let rebuilt_report = build_textured_render_sections_for_chunk_set_with_stats(
            &inputs,
            catalog,
            &target_chunks,
        )
        .context("failed to build dirty textured sections")?;
        let rebuilt_keys = rebuilt_report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let dirty_coords = dirty_chunks
            .iter()
            .map(|pos| (pos.x, pos.z))
            .collect::<BTreeSet<_>>();
        let old_dirty_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| dirty_coords.contains(&(key.chunk_x, key.chunk_z)))
            .collect::<BTreeSet<_>>();
        let removed_section_keys = old_dirty_keys
            .difference(&rebuilt_keys)
            .copied()
            .collect::<BTreeSet<_>>();

        for key in &removed_section_keys {
            self.sections.remove(key);
        }

        let mut report = RenderSectionCacheUpdate {
            rebuilt_sections: rebuilt_report.sections,
            removed_section_keys,
            rebuilt_vertex_count: 0,
            rebuilt_index_count: 0,
            visibility_graph_stats: rebuilt_report.visibility_graph,
        };
        for section in &report.rebuilt_sections {
            let stats = section.stats();
            report.rebuilt_vertex_count += stats.vertex_count;
            report.rebuilt_index_count += stats.index_count;
            self.sections.insert(section.key, section.clone());
        }
        Ok(report)
    }
}

fn write_headless_chunk_scenarios(
    directory: &Path,
    width: u32,
    height: u32,
    scene: &SceneOptions,
    scene_mesh: &SceneTexturedSections,
    render_options: TexturedSectionRenderOptions,
) -> Result<Vec<mclone_render::headless::HeadlessChunkReport>> {
    let mut reports = Vec::new();
    for (name, camera) in chunk_capture_scenarios(scene) {
        let path = directory.join(format!("{name}.png"));
        reports.push(write_headless_textured_sections_png_with_options(
            HeadlessChunkOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
                camera,
            },
            &scene_mesh.sections,
            scene_mesh.atlas.as_upload(),
            render_options,
        )?);
    }
    Ok(reports)
}

fn run_headless_screenshot(
    options: &HeadlessScreenshotOptions,
) -> Result<HeadlessScreenshotReport> {
    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
    poll_window_runtime_until_idle(&mut runtime)?;
    let section_update = runtime.sync_all_render_sections()?;
    let sections = runtime.cached_sections();
    if sections.is_empty() {
        bail!(
            "headless screenshot seed={} center=({}, {}) radius={} produced no render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.chunk_radius
        );
    }

    let spectator = SpectatorCamera::spawn_for_scene(&options.scene);
    let mut ui = NativeUi::new(options.scene.chunk_radius);
    ui.set_screen(options.ui.native_screen());
    ui.set_scale(GuiScale::from_pixels(options.width, options.height));

    let mut render_stats = RenderStreamStats::default();
    let debug_pane = options.debug_pane;
    let render_options = options.render_options;
    let camera = spectator.camera(runtime.radius_chunks);
    let runtime_stats = runtime.stats();
    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: section_update.rebuilt_section_count(),
        removed_section_count: section_update.removed_section_count(),
        uploaded_vertex_count: section_update.rebuilt_vertex_count,
        uploaded_index_count: section_update.rebuilt_index_count,
    };

    let (frame_report, summary) = write_headless_frame_png(
        HeadlessFrameOptions {
            path: options.path.clone(),
            width: options.width,
            height: options.height,
        },
        |frame| {
            let depth =
                ChunkDepthTarget::new(frame.device, frame.target.size[0], frame.target.size[1]);
            let mut draw = TexturedSectionDrawResources::new(
                frame.device,
                frame.queue,
                HEADLESS_FORMAT,
                &sections,
                runtime.mesh_assets.atlas.as_upload(),
            )?;
            let mut gui = GuiRenderer::new(frame.device, HEADLESS_FORMAT);

            render_stats.section_count = draw.section_count();
            render_stats.index_count = draw.index_count();
            render_stats.face_count = quad_face_count_from_indices(render_stats.index_count);
            record_render_section_update_stats(&mut render_stats, &section_update, initial_upload);

            let debug_stats = debug_pane.then_some(DebugPaneStats {
                position: spectator.position,
                speed: spectator.speed,
                runtime: runtime_stats,
                render: render_stats,
                frame: FrameTimingStats::default(),
                pacing: FramePacingDebugStats::default(),
                section_occlusion: render_options.section_occlusion_culling,
                force_fullbright: render_options.force_fullbright,
            });

            render_full_frame(
                frame,
                &depth,
                &mut draw,
                &mut gui,
                camera,
                render_options,
                FramePacingUiState::default(),
                &ui,
                debug_stats,
                &mut render_stats,
            )
        },
    )?;

    Ok(HeadlessScreenshotReport {
        path: frame_report.path,
        width: frame_report.width,
        height: frame_report.height,
        byte_len: frame_report.byte_len,
        section_count: summary.section_count,
        drawn_section_count: summary.drawn_section_count,
        index_count: summary.index_count,
        drawn_index_count: summary.drawn_index_count,
        gui_command_count: summary.gui_command_count,
    })
}

fn chunk_capture_scenarios(scene: &SceneOptions) -> [(&'static str, ChunkCamera); 3] {
    let overview =
        ChunkCamera::overview_for_chunk_area(scene.chunk_x, scene.chunk_z, scene.chunk_radius);
    let mut orbit = overview;
    orbit.orbit(0.7, -0.16);
    let mut close = overview;
    close.zoom(0.45);
    close.orbit(-0.32, 0.08);
    [
        ("overview", overview),
        ("orbit-east", orbit),
        ("close", close),
    ]
}

fn build_scene_client_runtime(scene: &SceneOptions) -> Result<ClientRuntime> {
    let mut client = if scene.remote_addr.is_some() {
        ClientRuntime::new(ClientHost::RemoteDedicated)
    } else {
        ClientRuntime::local_integrated()
    };
    let radius_chunks =
        u32::try_from(scene.chunk_radius).context("chunk radius must be positive")?;

    let command = client.set_chunk_interest(ChunkInterest {
        center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
        radius_chunks,
    });

    if let Some(remote_addr) = &scene.remote_addr {
        let updates = request_server_updates(remote_addr.as_str(), &command)
            .with_context(|| format!("failed to request chunk updates from {remote_addr}"))?;
        client.apply_updates(updates);
    } else {
        let mut server = IntegratedServer::new(scene.seed);
        let mut transport = LocalTransport::new();
        transport.send_client_command(command);

        for command in transport.drain_client_commands() {
            for update in server.handle_command(command) {
                transport.send_server_update(update);
            }
        }
        for update in poll_integrated_server_until_idle(&mut server)? {
            transport.send_server_update(update);
        }
        client.apply_updates(transport.drain_server_updates());
    }
    Ok(client)
}

#[derive(Debug)]
struct WindowSceneRuntime {
    client: ClientRuntime,
    server: Option<IntegratedServer>,
    transport: LocalTransport,
    remote_addr: Option<String>,
    radius_chunks: u32,
    interest_center: ChunkPos,
    mesh_assets: TexturedMeshAssets,
    render_sections: CachedTexturedRenderSections,
    dirty_render_chunks: BTreeSet<ChunkPos>,
    last_tick: u64,
    last_simulation_tick: u64,
    last_tick_unloads_processed: usize,
    last_simulation_block_tick_chunks: usize,
    last_simulation_entity_tick_chunks: usize,
    last_simulation_scheduler_tick_ms: f64,
    last_simulation_block_tick_ms: f64,
    last_simulation_fluid_tick_ms: f64,
    last_simulation_entity_tick_ms: f64,
    last_simulation_fluid_ticks_executed: usize,
    last_simulation_deferred_fluid_ticks: usize,
    last_simulation_fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowRuntimeStats {
    interest_center: ChunkPos,
    loaded_chunks: usize,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    client_visible_chunks: usize,
    active_ticket_chunks: usize,
    pending_unload_chunks: usize,
    block_ticking_chunks: usize,
    entity_ticking_chunks: usize,
    last_tick: u64,
    last_simulation_tick: u64,
    last_tick_unloads_processed: usize,
    last_simulation_block_tick_chunks: usize,
    last_simulation_entity_tick_chunks: usize,
    last_simulation_scheduler_tick_ms: f64,
    last_simulation_block_tick_ms: f64,
    last_simulation_fluid_tick_ms: f64,
    last_simulation_entity_tick_ms: f64,
    last_simulation_fluid_ticks_executed: usize,
    last_simulation_deferred_fluid_ticks: usize,
    last_simulation_fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
}

impl WindowSceneRuntime {
    fn new(scene: &SceneOptions) -> Result<Self> {
        let client = if scene.remote_addr.is_some() {
            ClientRuntime::new(ClientHost::RemoteDedicated)
        } else {
            ClientRuntime::local_integrated()
        };
        let radius_chunks =
            u32::try_from(scene.chunk_radius).context("chunk radius must be positive")?;
        let mut runtime = Self {
            client,
            server: scene
                .remote_addr
                .is_none()
                .then(|| IntegratedServer::new(scene.seed)),
            transport: LocalTransport::new(),
            remote_addr: scene.remote_addr.clone(),
            radius_chunks,
            interest_center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
            mesh_assets: load_textured_mesh_assets()?,
            render_sections: CachedTexturedRenderSections::default(),
            dirty_render_chunks: BTreeSet::new(),
            last_tick: 0,
            last_simulation_tick: 0,
            last_tick_unloads_processed: 0,
            last_simulation_block_tick_chunks: 0,
            last_simulation_entity_tick_chunks: 0,
            last_simulation_scheduler_tick_ms: 0.0,
            last_simulation_block_tick_ms: 0.0,
            last_simulation_fluid_tick_ms: 0.0,
            last_simulation_entity_tick_ms: 0.0,
            last_simulation_fluid_ticks_executed: 0,
            last_simulation_deferred_fluid_ticks: 0,
            last_simulation_fluid_mutated_blocks: 0,
            scheduled_fluid_ticks: 0,
        };
        runtime.set_interest_center(runtime.interest_center)?;
        Ok(runtime)
    }

    fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        if self.client.chunk_interest().is_some_and(|interest| {
            interest.center == center && interest.radius_chunks == self.radius_chunks
        }) {
            return Ok(false);
        }

        self.interest_center = center;
        let command = self.client.set_chunk_interest(ChunkInterest {
            center,
            radius_chunks: self.radius_chunks,
        });

        if let Some(remote_addr) = &self.remote_addr {
            let updates = request_server_updates(remote_addr.as_str(), &command)
                .with_context(|| format!("failed to request chunk updates from {remote_addr}"))?;
            let changed = !updates.is_empty();
            self.apply_server_updates(updates);
            return Ok(changed);
        }

        self.transport.send_client_command(command);
        self.flush_local_commands()
    }

    fn poll(&mut self) -> Result<bool> {
        let mut changed = self.flush_local_commands()?;
        if let Some(server) = &mut self.server {
            let report = server
                .try_simulation_tick_report()
                .context("failed to tick integrated server")?;
            self.last_tick = report.chunk_tick;
            self.last_simulation_tick = report.simulation_tick;
            self.last_tick_unloads_processed = report.pending_unloads_processed;
            self.last_simulation_block_tick_chunks = report.block_tick_chunks;
            self.last_simulation_entity_tick_chunks = report.entity_tick_chunks;
            self.last_simulation_scheduler_tick_ms = micros_to_ms(report.timing.scheduler_tick_us);
            self.last_simulation_block_tick_ms = micros_to_ms(report.timing.block_tick_us);
            self.last_simulation_fluid_tick_ms = micros_to_ms(report.timing.fluid_tick_us);
            self.last_simulation_entity_tick_ms = micros_to_ms(report.timing.entity_tick_us);
            self.last_simulation_fluid_ticks_executed = report.fluid_ticks_executed;
            self.last_simulation_deferred_fluid_ticks = report.deferred_fluid_ticks;
            self.last_simulation_fluid_mutated_blocks = report.fluid_mutated_blocks;
            self.scheduled_fluid_ticks = report.scheduled_fluid_ticks;
            changed |= self.apply_server_updates(report.updates);
        }
        Ok(changed)
    }

    fn flush_local_commands(&mut self) -> Result<bool> {
        let Some(server) = &mut self.server else {
            return Ok(false);
        };
        let mut changed = false;
        for command in self.transport.drain_client_commands() {
            for update in server.handle_command(command) {
                self.transport.send_server_update(update);
            }
        }
        let updates = self.transport.drain_server_updates();
        changed |= self.apply_server_updates(updates);
        Ok(changed)
    }

    fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> bool {
        let changed = !updates.is_empty();
        for update in &updates {
            match update {
                ServerUpdate::ChunkSnapshot(snapshot) => {
                    self.mark_render_chunk_dirty(snapshot.pos);
                }
                ServerUpdate::ChunkUnload { pos } => {
                    self.mark_render_chunk_dirty(*pos);
                }
            }
        }
        self.client.apply_updates(updates);
        changed
    }

    fn mark_render_chunk_dirty(&mut self, pos: ChunkPos) {
        self.dirty_render_chunks
            .extend(render_dirty_chunk_neighborhood(pos));
    }

    fn sync_render_sections(&mut self) -> Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_with_budget(DEFAULT_RENDER_CHUNK_MESH_BUDGET)
    }

    fn sync_all_render_sections(&mut self) -> Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_with_budget(usize::MAX)
    }

    fn sync_render_sections_with_budget(
        &mut self,
        chunk_budget: usize,
    ) -> Result<RenderSectionCacheUpdate> {
        if self.render_sections.is_empty()
            && self.client.loaded_chunk_count() > 0
            && self.dirty_render_chunks.is_empty()
        {
            let loaded = self
                .client
                .chunk_snapshots()
                .map(|snapshot| snapshot.pos)
                .collect::<Vec<_>>();
            for pos in loaded {
                self.mark_render_chunk_dirty(pos);
            }
        }

        if chunk_budget == 0 || self.dirty_render_chunks.is_empty() {
            return Ok(RenderSectionCacheUpdate::default());
        }

        let mut stale_dirty_chunks = Vec::new();
        let mut loaded_dirty_chunks = Vec::new();
        let mut removal_dirty_chunks = Vec::new();
        for pos in self.dirty_render_chunks.iter().copied() {
            if self.client.chunk_snapshot(pos).is_some() {
                loaded_dirty_chunks.push(pos);
            } else if self.render_sections.contains_chunk(pos) {
                removal_dirty_chunks.push(pos);
            } else {
                stale_dirty_chunks.push(pos);
            }
        }
        let dirty_chunks = loaded_dirty_chunks
            .into_iter()
            .take(chunk_budget)
            .chain(removal_dirty_chunks)
            .collect::<BTreeSet<_>>();
        for pos in stale_dirty_chunks {
            self.dirty_render_chunks.remove(&pos);
        }
        if dirty_chunks.is_empty() {
            return Ok(RenderSectionCacheUpdate::default());
        }

        let report = self.render_sections.rebuild_dirty(
            &self.client,
            &self.mesh_assets.catalog,
            &dirty_chunks,
        )?;
        for pos in dirty_chunks {
            self.dirty_render_chunks.remove(&pos);
        }
        Ok(report)
    }

    fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.render_sections.sections()
    }

    fn has_pending_render_work(&self) -> bool {
        self.pending_render_chunk_count() > 0
    }

    fn pending_render_chunk_count(&self) -> usize {
        self.dirty_render_chunks
            .iter()
            .filter(|pos| {
                self.client.chunk_snapshot(**pos).is_some()
                    || self.render_sections.contains_chunk(**pos)
            })
            .count()
    }

    fn pending_job_count(&self) -> usize {
        self.server
            .as_ref()
            .map_or(0, IntegratedServer::pending_job_count)
    }

    fn pending_publication_count(&self) -> usize {
        self.server
            .as_ref()
            .map_or(0, IntegratedServer::pending_publication_count)
    }

    fn stats(&self) -> WindowRuntimeStats {
        let scheduler_metrics = self
            .server
            .as_ref()
            .map(|server| server.scheduler().metrics());
        WindowRuntimeStats {
            interest_center: self.interest_center,
            loaded_chunks: self.client.loaded_chunk_count(),
            pending_jobs: self.pending_job_count(),
            pending_publications: self.pending_publication_count(),
            pending_render_chunks: self.pending_render_chunk_count(),
            client_visible_chunks: scheduler_metrics
                .map_or(self.client.loaded_chunk_count(), |metrics| {
                    metrics.client_visible_chunks
                }),
            active_ticket_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.active_ticket_chunks),
            pending_unload_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.pending_unload_chunks),
            block_ticking_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.block_ticking_chunks),
            entity_ticking_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.entity_ticking_status_chunks),
            last_tick: self.last_tick,
            last_simulation_tick: self.last_simulation_tick,
            last_tick_unloads_processed: self.last_tick_unloads_processed,
            last_simulation_block_tick_chunks: self.last_simulation_block_tick_chunks,
            last_simulation_entity_tick_chunks: self.last_simulation_entity_tick_chunks,
            last_simulation_scheduler_tick_ms: self.last_simulation_scheduler_tick_ms,
            last_simulation_block_tick_ms: self.last_simulation_block_tick_ms,
            last_simulation_fluid_tick_ms: self.last_simulation_fluid_tick_ms,
            last_simulation_entity_tick_ms: self.last_simulation_entity_tick_ms,
            last_simulation_fluid_ticks_executed: self.last_simulation_fluid_ticks_executed,
            last_simulation_deferred_fluid_ticks: self.last_simulation_deferred_fluid_ticks,
            last_simulation_fluid_mutated_blocks: self.last_simulation_fluid_mutated_blocks,
            scheduled_fluid_ticks: self.scheduled_fluid_ticks,
        }
    }
}

fn poll_integrated_server_until_idle(server: &mut IntegratedServer) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            server
                .try_poll()
                .context("failed to poll integrated server worldgen jobs")?,
        );
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for integrated server worldgen jobs");
        }
        if server.pending_publication_count() == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn poll_window_runtime_until_idle(runtime: &mut WindowSceneRuntime) -> Result<(usize, f64)> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut polls = 0_usize;
    let mut poll_ms = 0.0_f64;
    loop {
        let poll_start = Instant::now();
        runtime.poll()?;
        poll_ms += elapsed_ms(poll_start.elapsed());
        polls += 1;
        if runtime.pending_job_count() == 0 {
            return Ok((polls, poll_ms));
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for window runtime worldgen jobs");
        }
        if runtime.pending_publication_count() == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[derive(Clone, Debug)]
struct TexturedMeshAssets {
    catalog: TexturedMeshCatalog,
    atlas: TextureAtlasImage,
}

fn load_textured_mesh_assets() -> Result<TexturedMeshAssets> {
    let root = extracted_asset_root();
    if !root.exists() {
        bail!(
            "missing extracted Minecraft assets at {}; run ./scripts/decompile-mc.sh from the repository root",
            root.display()
        );
    }

    let source = FilesystemAssetSource::new(root);
    let registry = BlockStateRegistry::terrain_mvp();
    let blockstates = BlockStateAssetIndex::load_namespace(&source, "minecraft")
        .context("failed to load vanilla blockstate assets")?;
    registry
        .validate_blockstate_assets(&blockstates)
        .context("terrain MVP registry is not covered by vanilla blockstates")?;
    let selected_model_refs = selected_model_refs(&registry, &blockstates)?;
    let models = BlockModelLibrary::load_model_tree(&source, selected_model_refs.iter().cloned())
        .context("failed to load vanilla block model tree")?;
    let materials = models
        .collect_materials_for_models(selected_model_refs.iter().cloned())
        .context("failed to collect selected block model materials")?;
    let atlas_plan = TextureAtlasPlan::build(&source, materials)
        .context("failed to plan block texture atlas")?;
    let atlas = stitch_texture_atlas(&source, &atlas_plan)?;
    let catalog = TexturedMeshCatalog::from_assets(&registry, &blockstates, &models, &atlas_plan)
        .context("failed to build textured mesh catalog")?;

    Ok(TexturedMeshAssets { catalog, atlas })
}

fn selected_model_refs(
    registry: &BlockStateRegistry,
    blockstates: &BlockStateAssetIndex,
) -> Result<BTreeSet<mclone_assets::ResourceLocation>> {
    let mut refs = BTreeSet::new();
    for record in registry.records() {
        let asset = blockstates
            .get(&record.block)
            .with_context(|| format!("missing blockstate asset for {}", record.block))?;
        if let Some(variant_key) = record.asset_variant_key(asset) {
            let Some(variants) = asset.variants_for_key(&variant_key) else {
                bail!(
                    "missing blockstate variant `{variant_key}` for {}",
                    record.block
                );
            };
            if let Some(variant) = variants.first() {
                refs.insert(variant.model.clone());
            }
        } else if record.variant_key().is_empty() && !asset.model_refs.is_empty() {
            refs.extend(asset.model_refs.iter().cloned());
        } else {
            bail!(
                "missing blockstate variant `{}` for {}",
                record.variant_key(),
                record.block
            );
        }
    }
    Ok(refs)
}

fn stitch_texture_atlas(
    source: &impl AssetSource,
    plan: &TextureAtlasPlan,
) -> Result<TextureAtlasImage> {
    let width = plan.width();
    let height = plan.height();
    if width == 0 || height == 0 {
        bail!("cannot stitch an empty texture atlas");
    }
    let mut atlas = vec![0; width as usize * height as usize * 4];

    for sprite in plan.sprites() {
        let bytes = source
            .read(&sprite.info.path)?
            .with_context(|| format!("missing texture {}", sprite.info.path))?;
        let image = image::load_from_memory(&bytes)
            .with_context(|| format!("failed to decode texture {}", sprite.info.path))?
            .to_rgba8();
        let (sprite_width, sprite_height) = image.dimensions();
        if sprite_width != sprite.info.width || sprite_height != sprite.info.height {
            bail!(
                "texture {} decoded as {}x{} but atlas plan expected {}x{}",
                sprite.info.path,
                sprite_width,
                sprite_height,
                sprite.info.width,
                sprite.info.height
            );
        }

        let image = image.as_raw();
        for row in 0..sprite.info.height {
            let source_start = (row * sprite.info.width * 4) as usize;
            let source_end = source_start + (sprite.info.width * 4) as usize;
            let dest_start = (((sprite.y + row) * width + sprite.x) * 4) as usize;
            let dest_end = dest_start + (sprite.info.width * 4) as usize;
            atlas[dest_start..dest_end].copy_from_slice(&image[source_start..source_end]);
        }
    }

    Ok(TextureAtlasImage {
        width,
        height,
        rgba: atlas,
    })
}

fn extracted_asset_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../reference/minecraft-1.17.1/extracted")
}

#[derive(Clone, Debug)]
struct MeshChunkBlocks {
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    blocks: Vec<mclone_core::BlockStateId>,
    light_sections: Vec<PackedLightSection>,
}

fn snapshot_mesh_block_state_ids(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        bail!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos,
            snapshot.height
        );
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; expected_len];
    let min_section_y = snapshot.min_y / SECTION_HEIGHT;
    let section_count = snapshot.height / SECTION_HEIGHT;

    for section in &snapshot.sections {
        let section_offset = section.section_y - min_section_y;
        if !(0..section_count).contains(&section_offset) {
            bail!(
                "chunk snapshot {:?} contains section {} outside {}..{}",
                snapshot.pos,
                section.section_y,
                min_section_y,
                min_section_y + section_count - 1
            );
        }
        let unpacked = section.unpack_block_state_ids();
        if unpacked.len() != CHUNK_SECTION_VOLUME {
            bail!(
                "chunk snapshot {:?} section {} unpacked to {} blocks",
                snapshot.pos,
                section.section_y,
                unpacked.len()
            );
        }
        let start = section_offset as usize * CHUNK_SECTION_VOLUME;
        for (index, state_id) in unpacked.into_iter().enumerate() {
            blocks[start + index] = state_id;
        }
    }

    Ok(MeshChunkBlocks {
        chunk_x: snapshot.pos.x,
        chunk_z: snapshot.pos.z,
        min_y: snapshot.min_y,
        height: snapshot.height,
        blocks,
        light_sections: snapshot.light_sections.clone(),
    })
}

impl SceneOptions {
    #[cfg(test)]
    fn chunk_positions(&self) -> impl Iterator<Item = (i32, i32)> {
        let min_x = self.chunk_x - self.chunk_radius;
        let max_x = self.chunk_x + self.chunk_radius;
        let min_z = self.chunk_z - self.chunk_radius;
        let max_z = self.chunk_z + self.chunk_radius;
        (min_x..=max_x)
            .flat_map(move |chunk_x| (min_z..=max_z).map(move |chunk_z| (chunk_x, chunk_z)))
    }
}

#[derive(Clone, Debug)]
struct SpectatorCamera {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    speed: f32,
}

impl SpectatorCamera {
    fn spawn_for_scene(scene: &SceneOptions) -> Self {
        let center_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
        let center_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
        Self {
            position: Vec3::new(center_x, 88.0, center_z),
            yaw: 0.55,
            pitch: -0.35,
            speed: SPECTATOR_BASE_SPEED,
        }
    }

    fn camera(&self, chunk_radius: u32) -> ChunkCamera {
        let forward = self.forward();
        ChunkCamera {
            eye: self.position.to_array(),
            target: (self.position + forward).to_array(),
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 64.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0 + chunk_radius as f32 * 128.0,
        }
    }

    fn chunk_pos(&self) -> ChunkPos {
        ChunkPos::new(
            world_block_to_chunk_coord(self.position.x),
            world_block_to_chunk_coord(self.position.z),
        )
    }

    fn look(&mut self, yaw_delta: f32, pitch_delta: f32) {
        if yaw_delta.is_finite() {
            self.yaw += yaw_delta;
        }
        if pitch_delta.is_finite() {
            self.pitch =
                (self.pitch + pitch_delta).clamp(-SPECTATOR_PITCH_LIMIT, SPECTATOR_PITCH_LIMIT);
        }
    }

    fn adjust_speed(&mut self, wheel_amount: f32) {
        if !wheel_amount.is_finite() {
            return;
        }
        let multiplier = (1.0 + wheel_amount * 0.18).clamp(0.5, 1.8);
        self.speed = (self.speed * multiplier).clamp(SPECTATOR_MIN_SPEED, SPECTATOR_MAX_SPEED);
    }

    fn move_local(
        &mut self,
        right_axis: f32,
        up_axis: f32,
        forward_axis: f32,
        boosted: bool,
        dt: f32,
    ) -> bool {
        if dt <= 0.0 {
            return false;
        }

        let forward = self.forward();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let direction = right * right_axis + Vec3::Y * up_axis + forward * forward_axis;
        let Some(direction) = direction.try_normalize() else {
            return false;
        };
        let boost = if boosted { 3.0 } else { 1.0 };
        self.position += direction * self.speed * boost * dt;
        true
    }

    fn forward(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos).normalize()
    }
}

fn world_block_to_chunk_coord(value: f32) -> i32 {
    (value / CHUNK_WIDTH as f32).floor() as i32
}

fn render_dirty_chunk_neighborhood(pos: ChunkPos) -> [ChunkPos; 5] {
    [
        pos,
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RenderStreamStats {
    section_count: usize,
    drawn_section_count: usize,
    face_count: u32,
    drawn_face_count: u32,
    index_count: u32,
    drawn_index_count: u32,
    last_rebuilt_section_count: usize,
    last_removed_section_count: usize,
    last_rebuilt_vertex_count: u32,
    last_rebuilt_face_count: u32,
    last_rebuilt_index_count: u32,
    last_visibility_graph_build_count: usize,
    last_visibility_graph_total_ms: f64,
    last_visibility_graph_worst_ms: f64,
    last_uploaded_section_count: usize,
    last_upload_removed_section_count: usize,
    last_uploaded_vertex_count: u32,
    last_uploaded_face_count: u32,
    last_uploaded_index_count: u32,
    last_remesh_ms: f64,
    last_upload_ms: f64,
    last_frame_ms: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum FramePacingMode {
    #[default]
    Vsync,
    Capped,
    Uncapped,
}

impl FramePacingMode {
    fn label(self) -> &'static str {
        match self {
            Self::Vsync => "VSync",
            Self::Capped => "Max FPS",
            Self::Uncapped => "Uncapped",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Vsync => Self::Capped,
            Self::Capped => Self::Uncapped,
            Self::Uncapped => Self::Vsync,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FramePacing {
    mode: FramePacingMode,
    fps_cap: u32,
    monitor_name: Option<String>,
    monitor_refresh_hz: Option<f32>,
    active_present_mode_label: &'static str,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
            monitor_name: None,
            monitor_refresh_hz: None,
            active_present_mode_label: "fifo",
        }
    }
}

impl FramePacing {
    fn present_mode_preference(&self) -> SurfacePresentModePreference {
        match self.mode {
            FramePacingMode::Vsync => SurfacePresentModePreference::Vsync,
            FramePacingMode::Capped | FramePacingMode::Uncapped => {
                SurfacePresentModePreference::NoVsync
            }
        }
    }

    fn target_frame_duration(&self) -> Option<Duration> {
        if self.mode != FramePacingMode::Capped {
            return None;
        }
        Some(Duration::from_secs_f64(1.0 / self.fps_cap.max(1) as f64))
    }

    fn target_frame_ms(&self) -> Option<f64> {
        match self.mode {
            FramePacingMode::Vsync => self
                .monitor_refresh_hz
                .filter(|refresh_hz| *refresh_hz > 1.0)
                .map(|refresh_hz| 1000.0 / refresh_hz as f64),
            FramePacingMode::Capped => Some(1000.0 / self.fps_cap.max(1) as f64),
            FramePacingMode::Uncapped => None,
        }
    }

    fn update_monitor(&mut self, window: &Window) {
        if let Some(monitor) = window.current_monitor() {
            self.monitor_name = monitor.name();
            self.monitor_refresh_hz = monitor
                .refresh_rate_millihertz()
                .map(|millihertz| millihertz as f32 / 1000.0);
        } else {
            self.monitor_name = None;
            self.monitor_refresh_hz = None;
        }
    }

    fn apply_to_surface(&mut self, surface: &mut NativeSurfaceContext) {
        let active = surface.set_present_mode_preference(self.present_mode_preference());
        self.active_present_mode_label = surface_present_mode_label(active);
    }

    fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
    }

    fn cycle_fps_cap(&mut self) {
        let current = FPS_CAPS
            .iter()
            .position(|cap| *cap == self.fps_cap)
            .unwrap_or_else(|| {
                FPS_CAPS
                    .iter()
                    .position(|cap| *cap >= self.fps_cap)
                    .unwrap_or(FPS_CAPS.len() - 1)
            });
        self.fps_cap = FPS_CAPS[(current + 1) % FPS_CAPS.len()];
    }

    fn ui_state(&self) -> FramePacingUiState {
        FramePacingUiState {
            mode: self.mode,
            fps_cap: self.fps_cap,
        }
    }

    fn debug_stats(&self) -> FramePacingDebugStats {
        FramePacingDebugStats {
            mode: self.mode,
            fps_cap: self.fps_cap,
            monitor_refresh_hz: self.monitor_refresh_hz,
            target_frame_ms: self.target_frame_ms(),
            active_present_mode_label: self.active_present_mode_label,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FramePacingUiState {
    mode: FramePacingMode,
    fps_cap: u32,
}

impl Default for FramePacingUiState {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FramePacingDebugStats {
    mode: FramePacingMode,
    fps_cap: u32,
    monitor_refresh_hz: Option<f32>,
    target_frame_ms: Option<f64>,
    active_present_mode_label: &'static str,
}

impl Default for FramePacingDebugStats {
    fn default() -> Self {
        FramePacing::default().debug_stats()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FrameTimingStats {
    frame_count: u64,
    over_budget_count: u64,
    over_2x_budget_count: u64,
    over_4x_budget_count: u64,
    last_frame_ms: f64,
    worst_frame_ms: f64,
    budget_ms: Option<f64>,
    last_runtime_poll_ms: f64,
    last_remesh_ms: f64,
    last_upload_ms: f64,
    last_render_ms: f64,
    last_surface_acquire_ms: f64,
    last_surface_encode_ms: f64,
    last_surface_submit_ms: f64,
    last_surface_present_ms: f64,
}

impl FrameTimingStats {
    fn begin_frame(&mut self, frame_ms: f64, budget_ms: Option<f64>) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.last_frame_ms = frame_ms;
        self.worst_frame_ms = self.worst_frame_ms.max(frame_ms);
        self.budget_ms = budget_ms;
        self.last_runtime_poll_ms = 0.0;
        self.last_remesh_ms = 0.0;
        self.last_upload_ms = 0.0;
        self.last_render_ms = 0.0;
        self.last_surface_acquire_ms = 0.0;
        self.last_surface_encode_ms = 0.0;
        self.last_surface_submit_ms = 0.0;
        self.last_surface_present_ms = 0.0;

        if let Some(budget_ms) = budget_ms.filter(|budget_ms| *budget_ms > 0.0) {
            if frame_ms > budget_ms {
                self.over_budget_count = self.over_budget_count.saturating_add(1);
            }
            if frame_ms > budget_ms * 2.0 {
                self.over_2x_budget_count = self.over_2x_budget_count.saturating_add(1);
            }
            if frame_ms > budget_ms * 4.0 {
                self.over_4x_budget_count = self.over_4x_budget_count.saturating_add(1);
            }
        }
    }

    fn record_runtime_poll(&mut self, ms: f64) {
        self.last_runtime_poll_ms = ms;
    }

    fn record_remesh_upload(&mut self, remesh_ms: f64, upload_ms: f64) {
        self.last_remesh_ms = remesh_ms;
        self.last_upload_ms = upload_ms;
    }

    fn record_surface_frame(
        &mut self,
        render_ms: f64,
        acquire_ms: f64,
        encode_ms: f64,
        submit_ms: f64,
        present_ms: f64,
    ) {
        self.last_render_ms = render_ms;
        self.last_surface_acquire_ms = acquire_ms;
        self.last_surface_encode_ms = encode_ms;
        self.last_surface_submit_ms = submit_ms;
        self.last_surface_present_ms = present_ms;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FullFrameRenderSummary {
    section_count: usize,
    drawn_section_count: usize,
    index_count: u32,
    drawn_index_count: u32,
    gui_command_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HeadlessScreenshotReport {
    path: PathBuf,
    width: u32,
    height: u32,
    byte_len: usize,
    section_count: usize,
    drawn_section_count: usize,
    index_count: u32,
    drawn_index_count: u32,
    gui_command_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DebugPaneStats {
    position: Vec3,
    speed: f32,
    runtime: WindowRuntimeStats,
    render: RenderStreamStats,
    frame: FrameTimingStats,
    pacing: FramePacingDebugStats,
    section_occlusion: bool,
    force_fullbright: bool,
}

impl DebugPaneStats {
    fn lines(self) -> Vec<String> {
        let occlusion = if self.section_occlusion { "ON" } else { "OFF" };
        let lighting = if self.force_fullbright {
            "FULL"
        } else {
            "LIGHT"
        };
        let budget = self
            .pacing
            .target_frame_ms
            .map(|ms| format!("{ms:.1}MS"))
            .unwrap_or_else(|| "UNCAPPED".to_owned());
        let refresh = self
            .pacing
            .monitor_refresh_hz
            .map(|hz| format!("{hz:.1}HZ"))
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let pacing_target = match self.pacing.mode {
            FramePacingMode::Capped => format!("{}FPS", self.pacing.fps_cap),
            FramePacingMode::Vsync | FramePacingMode::Uncapped => refresh,
        };
        vec![
            "DEBUG".to_string(),
            format!(
                "POS {:.1} {:.1} {:.1}",
                self.position.x, self.position.y, self.position.z
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                self.runtime.interest_center.x, self.runtime.interest_center.z, self.speed
            ),
            format!("OCC {}  {}", occlusion, lighting),
            format!(
                "TICK {} SIM {}",
                self.runtime.last_tick, self.runtime.last_simulation_tick
            ),
            format!(
                "CHUNKS L{} V{} P{}",
                self.runtime.loaded_chunks,
                self.runtime.client_visible_chunks,
                self.runtime.pending_jobs
            ),
            format!("STREAM PUB{}", self.runtime.pending_publications),
            format!("MESH Q{}", self.runtime.pending_render_chunks),
            format!(
                "TICKING B{}:{} E{}:{}",
                self.runtime.block_ticking_chunks,
                self.runtime.last_simulation_block_tick_chunks,
                self.runtime.entity_ticking_chunks,
                self.runtime.last_simulation_entity_tick_chunks
            ),
            format!(
                "FLUID {}/{}/{}/{}",
                self.runtime.last_simulation_fluid_ticks_executed,
                self.runtime.last_simulation_deferred_fluid_ticks,
                self.runtime.last_simulation_fluid_mutated_blocks,
                self.runtime.scheduled_fluid_ticks
            ),
            format!(
                "DRAW S {}/{} F {}/{}",
                self.render.drawn_section_count,
                self.render.section_count,
                self.render.drawn_face_count,
                self.render.face_count
            ),
            format!(
                "MESH R{} U{} F {:.1}MS",
                self.render.last_rebuilt_section_count,
                self.render.last_uploaded_section_count,
                self.render.last_frame_ms
            ),
            format!("BUDGET {} FRAME {:.1}MS", budget, self.frame.last_frame_ms),
            format!(
                "OVER {}/{}/{} WORST {:.1}",
                self.frame.over_budget_count,
                self.frame.over_2x_budget_count,
                self.frame.over_4x_budget_count,
                self.frame.worst_frame_ms
            ),
            format!(
                "STAGE POLL {:.1} MESH {:.1} UP {:.1}",
                self.frame.last_runtime_poll_ms,
                self.frame.last_remesh_ms,
                self.frame.last_upload_ms
            ),
            format!(
                "GPU ACQ {:.1} ENC {:.1} SUB {:.1} PRS {:.1}",
                self.frame.last_surface_acquire_ms,
                self.frame.last_surface_encode_ms,
                self.frame.last_surface_submit_ms,
                self.frame.last_surface_present_ms
            ),
            format!(
                "PACE {} {} {}",
                self.pacing.mode.label(),
                pacing_target,
                self.pacing.active_present_mode_label.to_ascii_uppercase()
            ),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeScreen {
    Title,
    Pause,
    Options { parent: OptionsParent },
}

impl HeadlessScreenshotUi {
    fn native_screen(self) -> Option<NativeScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(NativeScreen::Title),
            Self::Pause => Some(NativeScreen::Pause),
            Self::OptionsTitle => Some(NativeScreen::Options {
                parent: OptionsParent::Title,
            }),
            Self::OptionsPause => Some(NativeScreen::Options {
                parent: OptionsParent::Pause,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OptionsParent {
    Title,
    Pause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeUiAction {
    StartWorld,
    Resume,
    OpenOptions(OptionsParent),
    BackToTitle,
    BackToPause,
    ToggleSectionOcclusion,
    ToggleFullbright,
    CycleFramePacing,
    CycleFpsCap,
    Quit,
}

struct NativeUi {
    screen: Option<NativeScreen>,
    pointer: Option<Point>,
    pressed: Option<WidgetId>,
    font: Font,
    chunk_radius: i32,
    scale: GuiScale,
}

const ID_TITLE_START: WidgetId = WidgetId(1);
const ID_TITLE_OPTIONS: WidgetId = WidgetId(2);
const ID_TITLE_QUIT: WidgetId = WidgetId(3);
const ID_PAUSE_RESUME: WidgetId = WidgetId(4);
const ID_PAUSE_OPTIONS: WidgetId = WidgetId(5);
const ID_PAUSE_TITLE: WidgetId = WidgetId(6);
const ID_OPTIONS_OCCLUSION: WidgetId = WidgetId(7);
const ID_OPTIONS_FULLBRIGHT: WidgetId = WidgetId(8);
const ID_OPTIONS_RADIUS: WidgetId = WidgetId(9);
const ID_OPTIONS_BACK: WidgetId = WidgetId(10);
const ID_OPTIONS_FRAME_PACING: WidgetId = WidgetId(11);
const ID_OPTIONS_FPS_CAP: WidgetId = WidgetId(12);

impl NativeUi {
    fn new(chunk_radius: i32) -> Self {
        Self {
            screen: Some(NativeScreen::Title),
            pointer: None,
            pressed: None,
            font: Font::default(),
            chunk_radius,
            scale: GuiScale::from_pixels(1280, 900),
        }
    }

    fn new_ingame(chunk_radius: i32) -> Self {
        let mut ui = Self::new(chunk_radius);
        ui.set_screen(None);
        ui
    }

    fn set_scale(&mut self, scale: GuiScale) {
        self.scale = scale;
        self.pointer = self.pointer.map(|point| Point {
            x: point.x.clamp(0.0, scale.width),
            y: point.y.clamp(0.0, scale.height),
        });
    }

    fn is_active(&self) -> bool {
        self.screen.is_some()
    }

    fn covers_world(&self) -> bool {
        self.screen == Some(NativeScreen::Title)
    }

    fn open_pause(&mut self) {
        self.screen = Some(NativeScreen::Pause);
        self.pressed = None;
    }

    fn close(&mut self) {
        self.screen = None;
        self.pressed = None;
    }

    fn set_screen(&mut self, screen: Option<NativeScreen>) {
        self.screen = screen;
        self.pressed = None;
    }

    fn clear_input(&mut self) {
        self.pointer = None;
        self.pressed = None;
    }

    fn pointer_move(&mut self, point: Point) -> bool {
        if !self.is_active() {
            return false;
        }
        self.pointer = Some(point);
        true
    }

    fn pointer_down(&mut self, point: Point) -> bool {
        if !self.is_active() {
            return false;
        }
        self.pointer = Some(point);
        self.pressed = self.widget_at(point);
        true
    }

    fn pointer_up(&mut self, point: Point) -> (bool, Option<NativeUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let pressed = self.pressed.take();
        let released = self.widget_at(point);
        let action = match (pressed, released) {
            (Some(id), Some(released)) if id == released => self.action_for(id),
            _ => None,
        };
        (true, action)
    }

    fn key_pressed(&mut self, key: KeyCode) -> (bool, Option<NativeUiAction>) {
        let Some(screen) = self.screen else {
            return (false, None);
        };
        match (screen, key) {
            (NativeScreen::Pause, KeyCode::Escape) => (true, Some(NativeUiAction::Resume)),
            (NativeScreen::Options { parent }, KeyCode::Escape) => match parent {
                OptionsParent::Title => (true, Some(NativeUiAction::BackToTitle)),
                OptionsParent::Pause => (true, Some(NativeUiAction::BackToPause)),
            },
            (NativeScreen::Title, KeyCode::Escape) => (true, None),
            _ => (false, None),
        }
    }

    fn apply_action(&mut self, action: NativeUiAction) {
        match action {
            NativeUiAction::StartWorld | NativeUiAction::Resume => self.close(),
            NativeUiAction::OpenOptions(parent) => {
                self.screen = Some(NativeScreen::Options { parent });
                self.pressed = None;
            }
            NativeUiAction::BackToTitle => {
                self.screen = Some(NativeScreen::Title);
                self.pressed = None;
            }
            NativeUiAction::BackToPause => {
                self.screen = Some(NativeScreen::Pause);
                self.pressed = None;
            }
            NativeUiAction::ToggleSectionOcclusion
            | NativeUiAction::ToggleFullbright
            | NativeUiAction::CycleFramePacing
            | NativeUiAction::CycleFpsCap
            | NativeUiAction::Quit => {}
        }
    }

    fn render_draw_list(
        &self,
        render_options: TexturedSectionRenderOptions,
        frame_pacing: FramePacingUiState,
    ) -> GuiDrawList {
        let mut draw = GuiDrawList::new();
        match self.screen {
            Some(NativeScreen::Title) => self.render_title(&mut draw),
            Some(NativeScreen::Pause) => self.render_pause(&mut draw),
            Some(NativeScreen::Options { parent }) => {
                self.render_options_screen(&mut draw, render_options, frame_pacing, parent)
            }
            None => {}
        }
        draw
    }

    fn render_debug_pane(&self, draw: &mut GuiDrawList, stats: &DebugPaneStats) {
        let line_height = self.font.line_height();
        let lines = stats.lines();
        let panel_width = 236.0_f32.min(self.scale.width - 8.0).max(120.0);
        let panel_height = 8.0 + line_height * lines.len() as f32;
        let panel = Rect::new(
            4.0,
            4.0,
            panel_width,
            panel_height.min((self.scale.height - 8.0).max(0.0)),
        );
        draw.fill(panel, Color::rgba(6, 9, 10, 185));
        draw.outline(panel, Color::rgba(110, 140, 136, 230));
        draw.push_clip(panel.inset(4.0));
        let text = Color::rgba(220, 238, 220, 255);
        let muted = Color::rgba(165, 186, 176, 255);
        let mut y = panel.y + 5.0;
        for (index, line) in lines.iter().enumerate() {
            self.font.draw_shadow(
                draw,
                line,
                panel.x + 6.0,
                y,
                if index == 0 { text } else { muted },
            );
            y += line_height;
        }
        draw.pop_clip();
    }

    fn widget_at(&self, point: Point) -> Option<WidgetId> {
        match self.screen? {
            NativeScreen::Title => title_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            NativeScreen::Pause => pause_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            NativeScreen::Options { .. } => {
                let rects = option_widgets(self.scale);
                if rects.occlusion.contains(point) {
                    Some(ID_OPTIONS_OCCLUSION)
                } else if rects.fullbright.contains(point) {
                    Some(ID_OPTIONS_FULLBRIGHT)
                } else if rects.frame_pacing.contains(point) {
                    Some(ID_OPTIONS_FRAME_PACING)
                } else if rects.fps_cap.contains(point) {
                    Some(ID_OPTIONS_FPS_CAP)
                } else if rects.back.contains(point) {
                    Some(ID_OPTIONS_BACK)
                } else {
                    None
                }
            }
        }
    }

    fn action_for(&self, id: WidgetId) -> Option<NativeUiAction> {
        match id {
            ID_TITLE_START => Some(NativeUiAction::StartWorld),
            ID_TITLE_OPTIONS => Some(NativeUiAction::OpenOptions(OptionsParent::Title)),
            ID_TITLE_QUIT => Some(NativeUiAction::Quit),
            ID_PAUSE_RESUME => Some(NativeUiAction::Resume),
            ID_PAUSE_OPTIONS => Some(NativeUiAction::OpenOptions(OptionsParent::Pause)),
            ID_PAUSE_TITLE => Some(NativeUiAction::BackToTitle),
            ID_OPTIONS_OCCLUSION => Some(NativeUiAction::ToggleSectionOcclusion),
            ID_OPTIONS_FULLBRIGHT => Some(NativeUiAction::ToggleFullbright),
            ID_OPTIONS_FRAME_PACING => Some(NativeUiAction::CycleFramePacing),
            ID_OPTIONS_FPS_CAP => Some(NativeUiAction::CycleFpsCap),
            ID_OPTIONS_BACK => match self.screen {
                Some(NativeScreen::Options {
                    parent: OptionsParent::Title,
                }) => Some(NativeUiAction::BackToTitle),
                Some(NativeScreen::Options {
                    parent: OptionsParent::Pause,
                }) => Some(NativeUiAction::BackToPause),
                _ => None,
            },
            _ => None,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction {
            pointer: self.pointer,
            pressed: self.pressed,
            focused: None,
        }
    }

    fn render_title(&self, draw: &mut GuiDrawList) {
        draw.fill_gradient(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(24, 44, 51, 255),
            Color::rgba(7, 10, 12, 255),
        );
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 55),
        );
        self.font.draw_centered(
            draw,
            "MCLONE",
            self.scale.width * 0.5,
            34.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered(
            draw,
            "NATIVE RUST CLIENT",
            self.scale.width * 0.5,
            48.0,
            Color::rgba(185, 212, 198, 255),
        );
        for button in title_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
        self.font.draw_shadow(
            draw,
            "MINECRAFT 1.17.1 TARGET",
            4.0,
            self.scale.height - 12.0,
            Color::rgba(160, 176, 170, 255),
        );
    }

    fn render_pause(&self, draw: &mut GuiDrawList) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 135),
        );
        self.font.draw_centered(
            draw,
            "PAUSED",
            self.scale.width * 0.5,
            self.scale.height * 0.25,
            Color::rgba(245, 252, 234, 255),
        );
        for button in pause_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
    }

    fn render_options_screen(
        &self,
        draw: &mut GuiDrawList,
        render_options: TexturedSectionRenderOptions,
        frame_pacing: FramePacingUiState,
        parent: OptionsParent,
    ) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = centered_panel(self.scale, 242.0, 190.0);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 245),
            Color::rgba(15, 20, 22, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered(
            draw,
            "OPTIONS",
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(245, 252, 234, 255),
        );
        let widgets = option_widgets(self.scale);
        Checkbox::new(
            ID_OPTIONS_OCCLUSION,
            widgets.occlusion,
            "Section Occlusion",
            render_options.section_occlusion_culling,
        )
        .render(draw, &self.font, self.interaction());
        Checkbox::new(
            ID_OPTIONS_FULLBRIGHT,
            widgets.fullbright,
            "Force Fullbright",
            render_options.force_fullbright,
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FRAME_PACING,
            widgets.frame_pacing,
            "Frame Pacing",
            frame_pacing.mode.label(),
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FPS_CAP,
            widgets.fps_cap,
            "FPS Cap",
            frame_pacing.fps_cap.to_string(),
        )
        .render(draw, &self.font, self.interaction());
        let mut radius = CycleButton::new(
            ID_OPTIONS_RADIUS,
            widgets.radius,
            "Chunk Radius",
            format!("{} restart", self.chunk_radius),
        );
        radius.enabled = false;
        radius.render(draw, &self.font, self.interaction());
        Button::new(
            ID_OPTIONS_BACK,
            widgets.back,
            match parent {
                OptionsParent::Title => "Back",
                OptionsParent::Pause => "Done",
            },
        )
        .render(draw, &self.font, self.interaction());
    }
}

#[derive(Clone, Copy, Debug)]
struct OptionWidgetRects {
    occlusion: Rect,
    fullbright: Rect,
    frame_pacing: Rect,
    fps_cap: Rect,
    radius: Rect,
    back: Rect,
}

fn title_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 22.0;
    [
        Button::new(
            ID_TITLE_START,
            menu_button_rect(scale, y),
            "Start Local World",
        ),
        Button::new(
            ID_TITLE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        ),
        Button::new(ID_TITLE_QUIT, menu_button_rect(scale, y + 48.0), "Quit"),
    ]
}

fn pause_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 22.0;
    [
        Button::new(ID_PAUSE_RESUME, menu_button_rect(scale, y), "Back To Game"),
        Button::new(
            ID_PAUSE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        ),
        Button::new(
            ID_PAUSE_TITLE,
            menu_button_rect(scale, y + 48.0),
            "Quit To Title",
        ),
    ]
}

fn option_widgets(scale: GuiScale) -> OptionWidgetRects {
    let panel = centered_panel(scale, 242.0, 190.0);
    OptionWidgetRects {
        occlusion: Rect::new(panel.x + 26.0, panel.y + 38.0, 190.0, 18.0),
        fullbright: Rect::new(panel.x + 26.0, panel.y + 60.0, 190.0, 18.0),
        frame_pacing: Rect::new(panel.x + 25.0, panel.y + 84.0, 192.0, 20.0),
        fps_cap: Rect::new(panel.x + 25.0, panel.y + 108.0, 192.0, 20.0),
        radius: Rect::new(panel.x + 25.0, panel.y + 132.0, 192.0, 20.0),
        back: Rect::new(panel.center_x() - 55.0, panel.y + 160.0, 110.0, 20.0),
    }
}

fn centered_panel(scale: GuiScale, width: f32, height: f32) -> Rect {
    Rect::new(
        (scale.width - width).max(0.0) * 0.5,
        (scale.height - height).max(0.0) * 0.5,
        width.min(scale.width),
        height.min(scale.height),
    )
}

fn menu_button_rect(scale: GuiScale, y: f32) -> Rect {
    Rect::new(scale.width * 0.5 - 90.0, y, 180.0, 20.0)
}

fn render_static_title_ui(width: u32, height: u32) -> GuiDrawList {
    let scale = GuiScale::from_pixels(width, height);
    let mut ui = NativeUi::new(DEFAULT_CHUNK_RADIUS);
    ui.set_scale(scale);
    ui.render_draw_list(
        TexturedSectionRenderOptions::default(),
        FramePacingUiState::default(),
    )
}

fn render_full_frame(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    draw: &mut TexturedSectionDrawResources,
    gui: &mut GuiRenderer,
    camera: ChunkCamera,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacingUiState,
    ui: &NativeUi,
    debug_stats: Option<DebugPaneStats>,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary> {
    let ui_active = ui.is_active();
    let ui_covers_world = ui.covers_world();
    let debug_stats = (!ui_active).then_some(debug_stats).flatten();
    let gui_active = ui_active || debug_stats.is_some();

    if !ui_covers_world {
        let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            mclone_render::default_clear_color(),
        )?;
        let frame_stats = draw.render_with_options(
            frame.queue,
            frame.encoder,
            render_target,
            render_view,
            render_options,
        )?;
        render_stats.drawn_section_count = frame_stats.drawn_section_count;
        render_stats.drawn_face_count = frame_stats.drawn_face_count();
        render_stats.drawn_index_count = frame_stats.drawn_index_count;
    } else {
        render_stats.drawn_section_count = 0;
        render_stats.drawn_face_count = 0;
        render_stats.drawn_index_count = 0;
    }

    let mut ui_draw = ui.render_draw_list(render_options, frame_pacing);
    if let Some(mut stats) = debug_stats {
        stats.render = *render_stats;
        ui.render_debug_pane(&mut ui_draw, &stats);
    }

    let gui_command_count = ui_draw.commands().len();
    if gui_active {
        gui.render(
            frame.device,
            frame.queue,
            frame.encoder,
            frame.target,
            [ui.scale.width, ui.scale.height],
            &ui_draw,
            if ui_covers_world {
                GuiRenderOptions::clear(mclone_render::default_clear_color())
            } else {
                GuiRenderOptions::overlay()
            },
        )?;
    }

    Ok(FullFrameRenderSummary {
        section_count: draw.section_count(),
        drawn_section_count: render_stats.drawn_section_count,
        index_count: draw.index_count(),
        drawn_index_count: render_stats.drawn_index_count,
        gui_command_count,
    })
}

struct ChunkApp {
    runtime: WindowSceneRuntime,
    spectator: SpectatorCamera,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacing,
    ui: NativeUi,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    depth: Option<ChunkDepthTarget>,
    draw: Option<TexturedSectionDrawResources>,
    gui: Option<GuiRenderer>,
    pressed_keys: std::collections::HashSet<KeyCode>,
    mouse_locked: bool,
    mouse_lock_requested: bool,
    last_cursor: Option<(f64, f64)>,
    debug_visible: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    render_stats: RenderStreamStats,
    frame_timing: FrameTimingStats,
}

impl ChunkApp {
    fn new(
        runtime: WindowSceneRuntime,
        spectator: SpectatorCamera,
        render_options: TexturedSectionRenderOptions,
        chunk_radius: i32,
    ) -> Self {
        Self {
            runtime,
            spectator,
            render_options,
            frame_pacing: FramePacing::default(),
            ui: NativeUi::new_ingame(chunk_radius),
            window: None,
            surface: None,
            depth: None,
            draw: None,
            gui: None,
            pressed_keys: std::collections::HashSet::new(),
            mouse_locked: false,
            mouse_lock_requested: false,
            last_cursor: None,
            debug_visible: false,
            last_frame: Instant::now(),
            next_redraw_at: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn finish_redraw(&mut self, event_loop: &ActiveEventLoop, frame_start: Instant) {
        let Some(window) = &self.window else {
            return;
        };
        if let Some(frame_duration) = self.frame_pacing.target_frame_duration() {
            let next_redraw_at = next_capped_redraw_deadline(
                frame_start,
                Instant::now(),
                self.next_redraw_at,
                frame_duration,
            );
            self.next_redraw_at = Some(next_redraw_at);
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
        } else {
            self.next_redraw_at = None;
            event_loop.set_control_flow(ControlFlow::Poll);
            window.request_redraw();
        }
    }

    fn update_camera_from_keys(&mut self, now: Instant) -> Result<()> {
        let frame_dt = now.duration_since(self.last_frame);
        let movement_dt = frame_dt.as_secs_f32().min(0.05);
        self.last_frame = now;
        self.render_stats.last_frame_ms = (frame_dt.as_secs_f64() * 1000.0) as f32;
        self.frame_timing.begin_frame(
            frame_dt.as_secs_f64() * 1000.0,
            self.frame_pacing.target_frame_ms(),
        );

        if self.ui.is_active() {
            return Ok(());
        }

        let right = key_axis(&self.pressed_keys, KeyCode::KeyD, KeyCode::KeyA);
        let up = vertical_axis(&self.pressed_keys);
        let forward = key_axis(&self.pressed_keys, KeyCode::KeyW, KeyCode::KeyS);
        let boosted = self.pressed_keys.contains(&KeyCode::ShiftLeft)
            || self.pressed_keys.contains(&KeyCode::ShiftRight);
        if self
            .spectator
            .move_local(right, up, forward, boosted, movement_dt)
        {
            self.update_interest_from_spectator()?;
        }
        Ok(())
    }

    fn gui_scale(&self) -> Option<GuiScale> {
        self.surface.as_ref().map(|surface| {
            GuiScale::from_pixels(surface.config.width.max(1), surface.config.height.max(1))
        })
    }

    fn gui_point(&self, x: f64, y: f64) -> Option<Point> {
        self.gui_scale().map(|scale| scale.client_to_gui(x, y))
    }

    fn apply_ui_action(
        &mut self,
        action: NativeUiAction,
        event_loop: &ActiveEventLoop,
        from_pointer_click: bool,
    ) {
        let should_arm_mouse_lock = from_pointer_click
            && matches!(action, NativeUiAction::StartWorld | NativeUiAction::Resume);
        match action {
            NativeUiAction::ToggleSectionOcclusion => {
                self.render_options.section_occlusion_culling =
                    !self.render_options.section_occlusion_culling;
                log::info!(
                    "section occlusion culling {}",
                    if self.render_options.section_occlusion_culling {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            NativeUiAction::ToggleFullbright => {
                self.render_options.force_fullbright = !self.render_options.force_fullbright;
                log::info!(
                    "fullbright {}",
                    if self.render_options.force_fullbright {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            NativeUiAction::CycleFramePacing => {
                self.frame_pacing.cycle_mode();
                self.next_redraw_at = None;
                if let Some(surface) = &mut self.surface {
                    self.frame_pacing.apply_to_surface(surface);
                }
                log::info!(
                    "frame pacing set to {} at cap {} fps",
                    self.frame_pacing.mode.label(),
                    self.frame_pacing.fps_cap
                );
            }
            NativeUiAction::CycleFpsCap => {
                self.frame_pacing.cycle_fps_cap();
                self.next_redraw_at = None;
                log::info!("fps cap set to {}", self.frame_pacing.fps_cap);
            }
            NativeUiAction::Quit => {
                event_loop.exit();
                return;
            }
            NativeUiAction::StartWorld
            | NativeUiAction::Resume
            | NativeUiAction::OpenOptions(_)
            | NativeUiAction::BackToTitle
            | NativeUiAction::BackToPause => {}
        }
        self.ui.apply_action(action);
        if should_arm_mouse_lock {
            self.mouse_lock_requested = true;
        }
        self.pressed_keys.clear();
        self.last_cursor = None;
        self.sync_mouse_lock();
        self.request_redraw();
    }

    fn sync_mouse_lock(&mut self) {
        self.set_mouse_lock(self.mouse_lock_requested && !self.ui.is_active());
    }

    fn set_mouse_lock(&mut self, should_lock: bool) {
        let Some(window) = &self.window else {
            self.mouse_locked = false;
            return;
        };
        if should_lock == self.mouse_locked {
            return;
        }

        if should_lock {
            let grab_result =
                window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|locked_err| {
                        log::warn!(
                            "cursor lock unavailable ({locked_err}); trying confined cursor grab"
                        );
                        window.set_cursor_grab(CursorGrabMode::Confined)
                    });
            match grab_result {
                Ok(()) => {
                    window.set_cursor_visible(false);
                    self.mouse_locked = true;
                    self.last_cursor = None;
                }
                Err(err) => {
                    log::warn!("failed to grab cursor for mouse look: {err}");
                    window.set_cursor_visible(true);
                    self.mouse_locked = false;
                }
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                log::warn!("failed to release cursor grab: {err}");
            }
            window.set_cursor_visible(true);
            self.mouse_locked = false;
            self.last_cursor = None;
        }
    }

    fn update_interest_from_spectator(&mut self) -> Result<()> {
        let center = self.spectator.chunk_pos();
        if self.runtime.set_interest_center(center)? {
            log::info!(
                "chunk interest moved to ({}, {}) at spectator position ({:.1}, {:.1}, {:.1})",
                center.x,
                center.z,
                self.spectator.position.x,
                self.spectator.position.y,
                self.spectator.position.z
            );
        }
        Ok(())
    }

    fn poll_runtime_and_upload(&mut self) -> Result<()> {
        let poll_start = Instant::now();
        let changed = self.runtime.poll()?;
        self.frame_timing
            .record_runtime_poll(elapsed_ms(poll_start.elapsed()));
        if !changed && !self.runtime.has_pending_render_work() {
            return Ok(());
        }
        self.upload_runtime_sections()?;
        Ok(())
    }

    fn upload_runtime_sections(&mut self) -> Result<()> {
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let remesh_start = Instant::now();
        let section_update = self.runtime.sync_render_sections()?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let upload_report = draw
            .apply_section_updates(
                &surface.device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("failed to upload streamed chunk section updates")?;
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(&section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(remesh_ms, upload_ms);
        log::info!(
            "streamed chunks loaded={} sections={} faces={} indices={} rebuilt={} visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} uploaded={} uploaded_vertices={} uploaded_faces={} uploaded_indices={} removed={} remesh_ms={:.3} upload_ms={:.3}",
            self.runtime.client.loaded_chunk_count(),
            section_count,
            face_count,
            index_count,
            section_update.rebuilt_section_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            upload_report.uploaded_section_count,
            upload_report.uploaded_vertex_count,
            upload_report.uploaded_face_count(),
            upload_report.uploaded_index_count,
            upload_report.removed_section_count,
            remesh_ms,
            upload_ms
        );
        Ok(())
    }

    fn record_section_update_stats(
        &mut self,
        section_update: &RenderSectionCacheUpdate,
        upload_report: TexturedSectionUploadReport,
    ) {
        record_render_section_update_stats(&mut self.render_stats, section_update, upload_report);
    }

    fn debug_pane_stats(&self) -> DebugPaneStats {
        DebugPaneStats {
            position: self.spectator.position,
            speed: self.spectator.speed,
            runtime: self.runtime.stats(),
            render: self.render_stats,
            frame: self.frame_timing,
            pacing: self.frame_pacing.debug_stats(),
            section_occlusion: self.render_options.section_occlusion_culling,
            force_fullbright: self.render_options.force_fullbright,
        }
    }
}

fn record_render_section_update_stats(
    render_stats: &mut RenderStreamStats,
    section_update: &RenderSectionCacheUpdate,
    upload_report: TexturedSectionUploadReport,
) {
    render_stats.last_rebuilt_section_count = section_update.rebuilt_section_count();
    render_stats.last_removed_section_count = section_update.removed_section_count();
    render_stats.last_rebuilt_vertex_count = section_update.rebuilt_vertex_count;
    render_stats.last_rebuilt_face_count = section_update.rebuilt_face_count();
    render_stats.last_rebuilt_index_count = section_update.rebuilt_index_count;
    render_stats.last_visibility_graph_build_count =
        section_update.visibility_graph_stats.build_count;
    render_stats.last_visibility_graph_total_ms = section_update.visibility_graph_stats.total_ms;
    render_stats.last_visibility_graph_worst_ms = section_update.visibility_graph_stats.worst_ms;
    render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
    render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
    render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
    render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
    render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
}

impl ApplicationHandler for ChunkApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("mclone native")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 900));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                log::error!("failed to create native window: {err}");
                event_loop.exit();
                return;
            }
        };
        let mut surface = match NativeSurfaceContext::new(window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
        self.frame_pacing.update_monitor(&window);
        self.frame_pacing.apply_to_surface(&mut surface);
        let remesh_start = Instant::now();
        let section_update = match self.runtime.sync_all_render_sections() {
            Ok(update) => update,
            Err(err) => {
                log::error!("failed to build initial chunk sections: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let sections = self.runtime.cached_sections();
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let depth =
            ChunkDepthTarget::new(&surface.device, surface.config.width, surface.config.height);
        let draw = match TexturedSectionDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            &sections,
            self.runtime.mesh_assets.atlas.as_upload(),
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let gui = GuiRenderer::new(&surface.device, surface.config.format);
        self.ui.set_scale(GuiScale::from_pixels(
            surface.config.width,
            surface.config.height,
        ));
        self.render_stats.section_count = draw.section_count();
        self.render_stats.index_count = draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(
            &section_update,
            TexturedSectionUploadReport {
                uploaded_section_count: section_update.rebuilt_section_count(),
                removed_section_count: section_update.removed_section_count(),
                uploaded_vertex_count: section_update.rebuilt_vertex_count,
                uploaded_index_count: section_update.rebuilt_index_count,
            },
        );
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        log::info!(
            "uploaded {} initial render sections with {} vertices / {} faces / {} indices visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} remesh_ms={:.3} upload_ms={:.3}",
            draw.section_count(),
            section_update.rebuilt_vertex_count,
            self.render_stats.face_count,
            draw.index_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            remesh_ms,
            upload_ms
        );
        self.depth = Some(depth);
        self.draw = Some(draw);
        self.gui = Some(gui);
        self.surface = Some(surface);
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.frame_timing = FrameTimingStats::default();
        self.next_redraw_at = None;
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        self.sync_mouse_lock();
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.resize(size);
                }
                if let (Some(surface), Some(depth)) = (&self.surface, &mut self.depth) {
                    depth.resize(&surface.device, surface.config.width, surface.config.height);
                    self.ui.set_scale(GuiScale::from_pixels(
                        surface.config.width,
                        surface.config.height,
                    ));
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if let Some(scale) = self.gui_scale() {
                        self.ui.set_scale(scale);
                    }
                    if key_code == KeyCode::Backquote
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.debug_visible = !self.debug_visible;
                        self.request_redraw();
                        return;
                    }
                    if event.state == ElementState::Pressed && self.ui.is_active() {
                        let (handled, action) = self.ui.key_pressed(key_code);
                        if let Some(action) = action {
                            self.apply_ui_action(action, event_loop, false);
                        } else if handled {
                            self.request_redraw();
                        }
                        return;
                    }
                    if key_code == KeyCode::Escape
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.mouse_lock_requested = false;
                        self.ui.open_pause();
                        self.pressed_keys.clear();
                        self.last_cursor = None;
                        self.sync_mouse_lock();
                        self.request_redraw();
                        return;
                    }
                    if key_code == KeyCode::KeyO
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.render_options.section_occlusion_culling =
                            !self.render_options.section_occlusion_culling;
                        log::info!(
                            "section occlusion culling {}",
                            if self.render_options.section_occlusion_culling {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.request_redraw();
                        return;
                    }
                    if key_code == KeyCode::KeyL
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.render_options.force_fullbright =
                            !self.render_options.force_fullbright;
                        log::info!(
                            "fullbright {}",
                            if self.render_options.force_fullbright {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.request_redraw();
                        return;
                    }
                    match event.state {
                        ElementState::Pressed => {
                            self.pressed_keys.insert(key_code);
                        }
                        ElementState::Released => {
                            self.pressed_keys.remove(&key_code);
                        }
                    }
                    self.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.ui.is_active() {
                    if button == MouseButton::Left {
                        if let Some((x, y)) = self.last_cursor
                            && let Some(point) = self.gui_point(x, y)
                        {
                            match state {
                                ElementState::Pressed => {
                                    self.ui.pointer_down(point);
                                }
                                ElementState::Released => {
                                    let (_handled, action) = self.ui.pointer_up(point);
                                    if let Some(action) = action {
                                        self.apply_ui_action(action, event_loop, true);
                                        return;
                                    }
                                }
                            }
                        }
                        self.request_redraw();
                    }
                    return;
                }
                if state == ElementState::Pressed {
                    self.mouse_lock_requested = true;
                    self.last_cursor = None;
                    self.sync_mouse_lock();
                    self.request_redraw();
                } else if button == MouseButton::Left || button == MouseButton::Right {
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let cursor = (position.x, position.y);
                if self.ui.is_active() {
                    self.last_cursor = Some(cursor);
                    if let Some(point) = self.gui_point(cursor.0, cursor.1) {
                        self.ui.pointer_move(point);
                    }
                    self.request_redraw();
                    return;
                }
                if self.mouse_lock_requested && !self.mouse_locked {
                    if let Some(previous) = self.last_cursor {
                        let dx = (cursor.0 - previous.0) as f32;
                        let dy = (cursor.1 - previous.1) as f32;
                        self.spectator.look(
                            -dx * SPECTATOR_MOUSE_SENSITIVITY,
                            -dy * SPECTATOR_MOUSE_SENSITIVITY,
                        );
                        self.request_redraw();
                    }
                }
                self.last_cursor = Some(cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.ui.is_active() {
                    return;
                }
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                self.spectator.adjust_speed(amount);
                log::info!("spectator speed {:.1} blocks/s", self.spectator.speed);
                self.request_redraw();
            }
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.pressed_keys.clear();
                self.last_cursor = None;
                self.ui.clear_input();
                self.set_mouse_lock(false);
            }
            WindowEvent::Focused(true) => {
                self.sync_mouse_lock();
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                if let Some(window) = &self.window {
                    self.frame_pacing.update_monitor(window);
                }
                if let Err(err) = self.update_camera_from_keys(frame_start) {
                    log::error!("failed to update spectator camera: {err:#}");
                    event_loop.exit();
                    return;
                }
                if let Err(err) = self.poll_runtime_and_upload() {
                    log::error!("failed to poll chunk runtime: {err:#}");
                    event_loop.exit();
                    return;
                }
                let Some(gui_scale) = self.gui_scale() else {
                    return;
                };
                self.ui.set_scale(gui_scale);
                let camera = self.spectator.camera(self.runtime.radius_chunks);
                let render_options = self.render_options;
                let frame_pacing = self.frame_pacing.ui_state();
                let debug_stats = self.debug_visible.then(|| self.debug_pane_stats());
                let mut render_stats = self.render_stats;
                let render_start = Instant::now();
                let result = {
                    let (Some(surface), Some(depth), Some(draw), Some(gui)) = (
                        &mut self.surface,
                        &self.depth,
                        &mut self.draw,
                        &mut self.gui,
                    ) else {
                        return;
                    };
                    self.frame_pacing.apply_to_surface(surface);
                    surface.render_with_report(|frame| {
                        render_full_frame(
                            frame,
                            depth,
                            draw,
                            gui,
                            camera,
                            render_options,
                            frame_pacing,
                            &self.ui,
                            debug_stats,
                            &mut render_stats,
                        )?;
                        Ok(())
                    })
                };
                match result {
                    Ok(report) => {
                        self.frame_timing.record_surface_frame(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                self.render_stats = render_stats;
                                self.finish_redraw(event_loop, frame_start);
                            }
                            SurfaceFrameStatus::Reconfigured => self.request_redraw(),
                        }
                    }
                    Err(err) => {
                        log::error!("render failed: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.ui.is_active() || !self.mouse_locked {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event {
            let dx = delta.0 as f32;
            let dy = delta.1 as f32;
            self.spectator.look(
                -dx * SPECTATOR_MOUSE_SENSITIVITY,
                -dy * SPECTATOR_MOUSE_SENSITIVITY,
            );
            self.request_redraw();
        }
    }
}

fn key_axis(
    keys: &std::collections::HashSet<KeyCode>,
    positive: KeyCode,
    negative: KeyCode,
) -> f32 {
    let positive = keys.contains(&positive) as i32;
    let negative = keys.contains(&negative) as i32;
    (positive - negative) as f32
}

fn vertical_axis(keys: &std::collections::HashSet<KeyCode>) -> f32 {
    key_axis(keys, KeyCode::Space, KeyCode::KeyX)
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn next_capped_redraw_deadline(
    frame_start: Instant,
    finish: Instant,
    previous_deadline: Option<Instant>,
    frame_duration: Duration,
) -> Instant {
    let mut next = previous_deadline.unwrap_or(frame_start) + frame_duration;
    while next <= finish {
        next += frame_duration;
    }
    next
}

fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1000.0
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
    fn world_block_to_chunk_coord_floors_negative_positions() {
        assert_eq!(world_block_to_chunk_coord(0.0), 0);
        assert_eq!(world_block_to_chunk_coord(15.99), 0);
        assert_eq!(world_block_to_chunk_coord(16.0), 1);
        assert_eq!(world_block_to_chunk_coord(-0.01), -1);
        assert_eq!(world_block_to_chunk_coord(-16.0), -1);
        assert_eq!(world_block_to_chunk_coord(-16.01), -2);
    }

    #[test]
    fn spectator_spawn_starts_interest_in_scene_center_chunk() {
        let scene = SceneOptions {
            chunk_x: 3,
            chunk_z: -2,
            ..SceneOptions::default()
        };
        let spectator = SpectatorCamera::spawn_for_scene(&scene);

        assert_eq!(spectator.chunk_pos(), ChunkPos::new(3, -2));
    }

    #[test]
    fn spectator_crossing_chunk_boundary_changes_interest_center() {
        let scene = SceneOptions::default();
        let mut spectator = SpectatorCamera::spawn_for_scene(&scene);
        spectator.position.x = 16.25;
        spectator.position.z = -0.25;

        assert_eq!(spectator.chunk_pos(), ChunkPos::new(1, -1));
    }

    #[test]
    fn spectator_pitch_is_clamped() {
        let mut spectator = SpectatorCamera::spawn_for_scene(&SceneOptions::default());
        spectator.look(0.0, 100.0);
        assert_eq!(spectator.pitch, SPECTATOR_PITCH_LIMIT);
        spectator.look(0.0, -200.0);
        assert_eq!(spectator.pitch, -SPECTATOR_PITCH_LIMIT);
    }

    #[test]
    fn vertical_axis_uses_space_for_up_and_x_for_down() {
        let mut keys = std::collections::HashSet::new();

        keys.insert(KeyCode::Space);
        assert_eq!(vertical_axis(&keys), 1.0);

        keys.insert(KeyCode::KeyX);
        assert_eq!(vertical_axis(&keys), 0.0);

        keys.remove(&KeyCode::Space);
        assert_eq!(vertical_axis(&keys), -1.0);
    }

    #[test]
    fn capped_redraw_deadline_skips_missed_frames() {
        let frame_duration = Duration::from_millis(8);
        let start = Instant::now();
        let previous_deadline = start + frame_duration;
        let finish = start + Duration::from_millis(27);

        let next =
            next_capped_redraw_deadline(start, finish, Some(previous_deadline), frame_duration);

        assert!(next > finish);
        assert_eq!(next.duration_since(start), frame_duration * 4);
    }

    #[test]
    fn frame_timing_counts_budget_overruns() {
        let mut stats = FrameTimingStats::default();

        stats.begin_frame(9.0, Some(8.0));
        stats.begin_frame(17.0, Some(8.0));
        stats.begin_frame(33.0, Some(8.0));

        assert_eq!(stats.frame_count, 3);
        assert_eq!(stats.over_budget_count, 3);
        assert_eq!(stats.over_2x_budget_count, 2);
        assert_eq!(stats.over_4x_budget_count, 1);
        assert_eq!(stats.worst_frame_ms, 33.0);
    }

    #[test]
    fn frame_budget_helpers_count_and_percentile_frame_times() {
        let frames = [
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 4.0,
                ..Default::default()
            },
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 9.0,
                ..Default::default()
            },
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 17.0,
                ..Default::default()
            },
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 35.0,
                ..Default::default()
            },
        ];

        assert_eq!(target_frame_ms(125.0), 8.0);
        assert_eq!(frame_budget_over_count(&frames, 8.0, 1.0), 3);
        assert_eq!(frame_budget_over_count(&frames, 8.0, 2.0), 2);
        assert_eq!(frame_budget_over_count(&frames, 8.0, 4.0), 1);
        assert_eq!(frame_budget_percentile_ms(&frames, 0.95), 35.0);
    }

    #[test]
    fn frame_pacing_cycles_mode_and_fps_cap() {
        let mut pacing = FramePacing::default();

        pacing.cycle_mode();
        assert_eq!(pacing.mode, FramePacingMode::Capped);
        pacing.cycle_fps_cap();
        assert_eq!(pacing.fps_cap, 144);
    }

    #[test]
    fn debug_pane_formats_and_draws_runtime_stats() {
        let stats = DebugPaneStats {
            position: Vec3::new(1.25, 64.0, -2.5),
            speed: 32.0,
            runtime: WindowRuntimeStats {
                interest_center: ChunkPos::new(3, -4),
                loaded_chunks: 9,
                pending_jobs: 1,
                pending_publications: 2,
                pending_render_chunks: 3,
                client_visible_chunks: 8,
                active_ticket_chunks: 9,
                pending_unload_chunks: 0,
                block_ticking_chunks: 4,
                entity_ticking_chunks: 2,
                last_tick: 12,
                last_simulation_tick: 11,
                last_tick_unloads_processed: 0,
                last_simulation_block_tick_chunks: 4,
                last_simulation_entity_tick_chunks: 2,
                last_simulation_scheduler_tick_ms: 0.1,
                last_simulation_block_tick_ms: 0.2,
                last_simulation_fluid_tick_ms: 0.3,
                last_simulation_entity_tick_ms: 0.4,
                last_simulation_fluid_ticks_executed: 5,
                last_simulation_deferred_fluid_ticks: 6,
                last_simulation_fluid_mutated_blocks: 7,
                scheduled_fluid_ticks: 8,
            },
            render: RenderStreamStats {
                section_count: 16,
                drawn_section_count: 10,
                face_count: 200,
                drawn_face_count: 120,
                last_rebuilt_section_count: 2,
                last_uploaded_section_count: 2,
                last_frame_ms: 16.7,
                ..RenderStreamStats::default()
            },
            frame: FrameTimingStats {
                frame_count: 12,
                over_budget_count: 3,
                over_2x_budget_count: 1,
                over_4x_budget_count: 0,
                last_frame_ms: 16.7,
                worst_frame_ms: 33.4,
                budget_ms: Some(8.3),
                last_runtime_poll_ms: 5.0,
                last_remesh_ms: 2.0,
                last_upload_ms: 1.0,
                last_surface_acquire_ms: 0.2,
                last_surface_encode_ms: 1.4,
                last_surface_submit_ms: 0.1,
                last_surface_present_ms: 0.0,
                ..FrameTimingStats::default()
            },
            pacing: FramePacingDebugStats {
                mode: FramePacingMode::Vsync,
                fps_cap: 120,
                monitor_refresh_hz: Some(120.0),
                target_frame_ms: Some(8.3),
                active_present_mode_label: "fifo",
            },
            section_occlusion: true,
            force_fullbright: false,
        };

        let lines = stats.lines();
        assert_eq!(lines[0], "DEBUG");
        assert_eq!(lines[1], "POS 1.2 64.0 -2.5");
        assert_eq!(lines[2], "CHUNK 3 -4 SPEED 32.0");
        assert_eq!(lines[3], "OCC ON  LIGHT");
        assert!(lines.iter().any(|line| line == "BUDGET 8.3MS FRAME 16.7MS"));
        assert!(lines.iter().any(|line| line == "OVER 3/1/0 WORST 33.4"));

        let mut ui = NativeUi::new(1);
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let mut draw = GuiDrawList::new();
        ui.render_debug_pane(&mut draw, &stats);
        assert!(!draw.commands().is_empty());
    }

    #[test]
    fn window_runtime_streams_chunks_when_spectator_crosses_boundary() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let initial_center = ChunkPos::new(0, 0);
        let initial_stats = runtime.stats();
        assert_eq!(initial_stats.interest_center, initial_center);
        assert_eq!(initial_stats.loaded_chunks, 1);
        assert_eq!(initial_stats.pending_jobs, 0);
        assert!(runtime.client.chunk_snapshot(initial_center).is_some());

        let initial_update = runtime.sync_render_sections().unwrap();
        assert!(initial_update.rebuilt_section_count() > 0);
        assert_eq!(initial_update.removed_section_count(), 0);
        let initial_sections = runtime.cached_sections();
        assert!(!initial_sections.is_empty());
        assert!(section_index_count(&initial_sections) > 0);
        assert!(
            initial_sections
                .iter()
                .all(|section| section.key.chunk_x == 0 && section.key.chunk_z == 0)
        );

        let mut spectator = SpectatorCamera::spawn_for_scene(&scene);
        spectator.position.x = 16.25;
        let next_center = spectator.chunk_pos();
        assert_eq!(next_center, ChunkPos::new(1, 0));
        assert!(runtime.set_interest_center(next_center).unwrap());
        assert_eq!(runtime.stats().interest_center, next_center);
        assert!(runtime.client.chunk_snapshot(initial_center).is_none());

        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let moved_stats = runtime.stats();
        assert_eq!(moved_stats.interest_center, next_center);
        assert_eq!(moved_stats.loaded_chunks, 1);
        assert_eq!(moved_stats.pending_jobs, 0);
        assert!(runtime.client.chunk_snapshot(initial_center).is_none());
        assert!(runtime.client.chunk_snapshot(next_center).is_some());

        let moved_update = runtime.sync_render_sections().unwrap();
        assert!(moved_update.rebuilt_section_count() > 0);
        assert!(moved_update.removed_section_count() > 0);
        let moved_sections = runtime.cached_sections();
        assert!(!moved_sections.is_empty());
        assert!(section_index_count(&moved_sections) > 0);
        assert!(
            moved_sections
                .iter()
                .all(|section| section.key.chunk_x == 1 && section.key.chunk_z == 0)
        );
    }

    #[test]
    fn window_runtime_mesh_queue_drains_by_chunk_budget() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 1,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.stats().loaded_chunks, 9);
        let first_update = runtime.sync_render_sections().unwrap();
        assert!(first_update.rebuilt_section_count() > 0);
        assert!(runtime.pending_render_chunk_count() > 0);

        let first_sections = runtime.cached_sections();
        assert!(!first_sections.is_empty());
        assert!(section_index_count(&first_sections) > 0);

        let remaining_update = runtime.sync_all_render_sections().unwrap();
        assert!(remaining_update.rebuilt_section_count() > 0);
        assert_eq!(runtime.pending_render_chunk_count(), 0);
        assert!(runtime.cached_sections().len() > first_sections.len());
    }

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
            "--force-fullbright".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessScreenshot {
                options: HeadlessScreenshotOptions {
                    path: PathBuf::from("/tmp/mclone-frame.png"),
                    width: 960,
                    height: 540,
                    scene: SceneOptions::default(),
                    render_options: TexturedSectionRenderOptions {
                        force_fullbright: true,
                        ..TexturedSectionRenderOptions::default()
                    },
                    ui: HeadlessScreenshotUi::Pause,
                    debug_pane: true,
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
    fn native_ui_has_title_and_ingame_start_modes() {
        let title_ui = NativeUi::new(1);
        assert!(title_ui.is_active());
        assert!(title_ui.covers_world());

        let ingame_ui = NativeUi::new_ingame(1);
        assert!(!ingame_ui.is_active());
        assert!(!ingame_ui.covers_world());
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
                    chunk_radius: DEFAULT_CHUNK_RADIUS,
                    remote_addr: None,
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_chunk_radius() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    chunk_radius: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
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
    fn cli_parses_movement_perf_options() {
        let cli = Cli::parse([
            "--movement-perf".to_owned(),
            "--movement-steps".to_owned(),
            "6".to_owned(),
            "--path-radius".to_owned(),
            "3".to_owned(),
            "--chunk-radius".to_owned(),
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
                        chunk_radius: 2,
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
            "--chunk-radius".to_owned(),
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
                        chunk_radius: 2,
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
            "--chunk-radius".to_owned(),
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
                        chunk_radius: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    frames: 36,
                    path_radius_chunks: 5,
                    target_hz: 120.0,
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
    fn timedemo_loaded_scene_covers_camera_path_radius() {
        let options = TimedemoOptions {
            scene: SceneOptions {
                chunk_radius: 1,
                ..SceneOptions::default()
            },
            path_radius_chunks: 4,
            ..TimedemoOptions::default()
        };

        let loaded_scene = timedemo_loaded_scene(&options).unwrap();

        assert_eq!(loaded_scene.chunk_radius, 4);
        assert_eq!(options.scene.chunk_radius, 1);
    }

    #[test]
    fn timedemo_loaded_scene_rejects_oversized_static_radius() {
        let options = TimedemoOptions {
            scene: SceneOptions {
                chunk_radius: 1,
                ..SceneOptions::default()
            },
            path_radius_chunks: MAX_CHUNK_RADIUS + 1,
            ..TimedemoOptions::default()
        };

        assert!(timedemo_loaded_scene(&options).is_err());
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
            "--chunk-radius".to_owned(),
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
                    chunk_radius: 2,
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
    fn scene_chunk_positions_cover_square_radius() {
        let positions = SceneOptions {
            seed: 0,
            chunk_x: -2,
            chunk_z: 3,
            chunk_radius: 1,
            remote_addr: None,
        }
        .chunk_positions()
        .collect::<Vec<_>>();

        assert_eq!(positions.len(), 9);
        assert!(positions.contains(&(-3, 2)));
        assert!(positions.contains(&(-2, 3)));
        assert!(positions.contains(&(-1, 4)));
    }

    #[test]
    fn circular_movement_spectator_faces_origin_from_ring() {
        let scene = SceneOptions::default();
        let spectator = circular_movement_spectator(&scene, 4, 0, 8);

        assert_eq!(spectator.chunk_pos(), ChunkPos::new(4, 0));
        assert!(spectator.forward().x < -0.5);
        assert!(spectator.forward().z.abs() < 0.2);
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_integrated_server() {
        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let client = build_scene_client_runtime(&scene).unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_remote_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let command = mclone_net::read_client_command_frame(&mut stream).unwrap();
            let mut server = IntegratedServer::new(DEFAULT_SEED);
            let mut updates = server.try_handle_command(command).unwrap();
            updates.extend(poll_integrated_server_until_idle(&mut server).unwrap());
            mclone_net::write_server_update_batch(&mut stream, &updates).unwrap();
        });
        let scene = SceneOptions {
            chunk_radius: 0,
            remote_addr: Some(addr.to_string()),
            ..SceneOptions::default()
        };

        let client = build_scene_client_runtime(&scene).unwrap();
        server.join().unwrap();

        assert_eq!(client.host(), ClientHost::RemoteDedicated);
        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn build_scene_textured_sections_uses_client_runtime_snapshots() {
        if !extracted_asset_root().exists() {
            return;
        }
        let scene_mesh = build_scene_textured_sections(&SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        })
        .unwrap();

        assert!(scene_mesh.section_count() > 0);
        assert!(scene_mesh.index_count() > 0);
        assert!(scene_mesh.atlas.width > 0);
        assert!(scene_mesh.atlas.height > 0);
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

    fn poll_window_runtime_until_idle(runtime: &mut WindowSceneRuntime) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            runtime.poll()?;
            if runtime.pending_job_count() == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for window runtime worldgen jobs");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn section_index_count(sections: &[TexturedRenderSectionMesh]) -> u32 {
        sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}
