use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use mclone_render::chunk::TexturedSectionRenderOptions;

use crate::camera::{SPECTATOR_BASE_SPEED, SPECTATOR_MAX_SPEED, SPECTATOR_MIN_SPEED};
use crate::{
    DEFAULT_CHUNK_X, DEFAULT_CHUNK_Z, DEFAULT_FRAME_BUDGET_PROBE_FRAMES,
    DEFAULT_FRAME_BUDGET_TARGET_HZ, DEFAULT_MOVEMENT_PERF_PATH_RADIUS, DEFAULT_MOVEMENT_PERF_STEPS,
    DEFAULT_RENDER_DISTANCE, DEFAULT_SEED, DEFAULT_TIMEDEMO_FRAMES, DEFAULT_TIMEDEMO_PATH_RADIUS,
    MAX_FRAME_BUDGET_PROBE_FRAMES, MAX_MOVEMENT_PERF_PATH_RADIUS, MAX_MOVEMENT_PERF_STEPS,
    MAX_RENDER_DISTANCE, MAX_TIMEDEMO_FRAMES, MIN_RENDER_DISTANCE,
};

const MAX_SCREENSHOT_REMOTE_SETTLE_MS: u64 = 10_000;
const DEFAULT_XR_CLEAR_SMOKE_FRAMES: u32 = 120;
const MAX_XR_SMOKE_FRAMES: u32 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SceneOptions {
    pub(crate) seed: i64,
    pub(crate) chunk_x: i32,
    pub(crate) chunk_z: i32,
    pub(crate) render_distance: i32,
    pub(crate) remote_addr: Option<String>,
    /// Debug override: force the day/night clock to this `dayTime` (ticks) for
    /// captures, instead of using whatever the simulation has advanced to.
    pub(crate) day_time_override: Option<u64>,
    /// Debug: stop the integrated server from advancing the day/night clock, so a
    /// forced (or initial) `dayTime` stays put for inspection.
    pub(crate) freeze_time: bool,
    /// Debug/perf switch: bypass native `ChunkStatus::Light` promotion and let
    /// generated `Features` snapshots stream directly to the client.
    pub(crate) lighting_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MovementPerfOptions {
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) steps: usize,
    pub(crate) path_radius_chunks: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TimedemoOptions {
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: usize,
    pub(crate) path_radius_chunks: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FrameBudgetProbeOptions {
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) mode: FrameBudgetProbeMode,
    pub(crate) frames: usize,
    pub(crate) path_radius_chunks: i32,
    pub(crate) target_hz: f64,
    pub(crate) movement_speed: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameBudgetProbeMode {
    StressOrbit,
    MovementWalk,
}

impl FrameBudgetProbeMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::StressOrbit => "stress_orbit",
            Self::MovementWalk => "movement_walk",
        }
    }

    pub(crate) fn benchmark_name(self) -> &'static str {
        match self {
            Self::StressOrbit => "native_frame_budget_probe",
            Self::MovementWalk => "native_movement_frame_probe",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessScreenshotOptions {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) ui: HeadlessScreenshotUi,
    pub(crate) debug_pane: bool,
    pub(crate) scripted_interaction: bool,
    pub(crate) remote_settle_ms: u64,
    pub(crate) eye: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessDualViewOptions {
    pub(crate) directory: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct XrClearSmokeOptions {
    pub(crate) frame_limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct XrMcloneSmokeOptions {
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) frame_limit: Option<u32>,
    pub(crate) view_pose: Option<XrViewPose>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct XrViewPose {
    pub(crate) position: [f32; 3],
    pub(crate) yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum HeadlessScreenshotUi {
    #[default]
    None,
    Title,
    NewWorld,
    Pause,
    OptionsTitle,
    OptionsPause,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WindowStartIntent {
    #[default]
    InWorld,
    Menu,
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
            mode: FrameBudgetProbeMode::StressOrbit,
            frames: DEFAULT_FRAME_BUDGET_PROBE_FRAMES,
            path_radius_chunks: DEFAULT_MOVEMENT_PERF_PATH_RADIUS,
            target_hz: DEFAULT_FRAME_BUDGET_TARGET_HZ,
            movement_speed: SPECTATOR_BASE_SPEED,
        }
    }
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            render_distance: DEFAULT_RENDER_DISTANCE,
            remote_addr: None,
            day_time_override: None,
            freeze_time: false,
            lighting_enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cli {
    Window {
        scene: SceneOptions,
        render_options: TexturedSectionRenderOptions,
        start_intent: WindowStartIntent,
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
    HeadlessDualView {
        options: HeadlessDualViewOptions,
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
    XrClearSmoke {
        options: XrClearSmokeOptions,
    },
    XrMcloneSmoke {
        options: XrMcloneSmokeOptions,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessMode {
    Clear(PathBuf),
    Chunk(PathBuf),
    ChunkScenarios(PathBuf),
    DualView(PathBuf),
    Ui(PathBuf),
    Screenshot(PathBuf),
}

impl Cli {
    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut width = None;
        let mut height = None;
        let mut scene = SceneOptions::default();
        let mut render_options = TexturedSectionRenderOptions::default();
        let mut fullbright_explicit = false;
        let mut screenshot_ui = HeadlessScreenshotUi::None;
        let mut screenshot_debug_pane = false;
        let mut screenshot_scripted_interaction = false;
        let mut screenshot_remote_settle_ms = 0;
        let mut screenshot_eye = None;
        let mut window_start_intent = WindowStartIntent::InWorld;
        let mut movement_perf = false;
        let mut timedemo = false;
        let mut frame_budget_probe = false;
        let mut explicit_frame_budget_probe = false;
        let mut movement_frame_probe = false;
        let mut movement_steps = DEFAULT_MOVEMENT_PERF_STEPS;
        let mut timedemo_frames = DEFAULT_TIMEDEMO_FRAMES;
        let mut frame_budget_frames = DEFAULT_FRAME_BUDGET_PROBE_FRAMES;
        let mut target_hz = DEFAULT_FRAME_BUDGET_TARGET_HZ;
        let mut movement_speed = SPECTATOR_BASE_SPEED;
        let mut path_radius = DEFAULT_MOVEMENT_PERF_PATH_RADIUS;
        let mut xr_clear_smoke = false;
        let mut xr_mclone_smoke = false;
        let mut xr_frames_explicit = false;
        let mut xr_forever_explicit = false;
        let mut xr_frame_limit = Some(DEFAULT_XR_CLEAR_SMOKE_FRAMES);
        let mut xr_view_pose_explicit = false;
        let mut xr_view_pose = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--xr-clear-smoke" => {
                    if mode.is_some()
                        || movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || xr_mclone_smoke
                    {
                        bail!("--xr-clear-smoke cannot be combined with other run modes");
                    }
                    xr_clear_smoke = true;
                }
                "--xr-mclone-smoke" => {
                    if mode.is_some()
                        || movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || xr_clear_smoke
                    {
                        bail!("--xr-mclone-smoke cannot be combined with other run modes");
                    }
                    xr_mclone_smoke = true;
                }
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
                    if movement_frame_probe {
                        bail!(
                            "--frame-budget-probe cannot be combined with --movement-frame-probe"
                        );
                    }
                    frame_budget_probe = true;
                    explicit_frame_budget_probe = true;
                }
                "--movement-frame-probe" => {
                    if mode.is_some() {
                        bail!(
                            "--movement-frame-probe cannot be combined with a headless output mode"
                        );
                    }
                    if explicit_frame_budget_probe {
                        bail!(
                            "--movement-frame-probe cannot be combined with --frame-budget-probe"
                        );
                    }
                    frame_budget_probe = false;
                    movement_frame_probe = true;
                }
                "--headless-clear" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-clear requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe || movement_frame_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Clear(path))?;
                }
                "--headless-chunk" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe || movement_frame_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Chunk(path))?;
                }
                "--headless-chunk-scenarios" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk-scenarios requires an output directory")?;
                    if movement_perf || timedemo || frame_budget_probe || movement_frame_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::ChunkScenarios(path))?;
                }
                "--headless-dual-view" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-dual-view requires an output directory")?;
                    if movement_perf || timedemo || frame_budget_probe || movement_frame_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::DualView(path))?;
                }
                "--headless-ui" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-ui requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe || movement_frame_probe {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Ui(path))?;
                }
                "--screenshot" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--screenshot requires an output PNG path")?;
                    if movement_perf || timedemo || frame_budget_probe || movement_frame_probe {
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
                "--screenshot-scripted-interaction" => {
                    screenshot_scripted_interaction =
                        parse_bool_arg("--screenshot-scripted-interaction", args.next())?;
                }
                "--screenshot-remote-settle-ms" => {
                    screenshot_remote_settle_ms =
                        parse_screenshot_remote_settle_ms_arg(&arg, args.next())?;
                }
                "--screenshot-eye" => {
                    screenshot_eye = Some(parse_f32_vec3_arg("--screenshot-eye", args.next())?);
                }
                "--menu" => {
                    window_start_intent = WindowStartIntent::Menu;
                }
                "--start-in-world" => {
                    window_start_intent = if parse_bool_arg("--start-in-world", args.next())? {
                        WindowStartIntent::InWorld
                    } else {
                        WindowStartIntent::Menu
                    };
                }
                "--width" => width = Some(parse_u32_arg("--width", args.next())?),
                "--height" => height = Some(parse_u32_arg("--height", args.next())?),
                "--seed" => scene.seed = parse_i64_arg("--seed", args.next())?,
                "--day-time" => {
                    scene.day_time_override = Some(parse_u64_arg("--day-time", args.next())?);
                }
                "--freeze-time" => {
                    scene.freeze_time = true;
                }
                "--lighting" => {
                    scene.lighting_enabled = parse_bool_arg("--lighting", args.next())?;
                }
                "--disable-lighting" => {
                    scene.lighting_enabled = false;
                }
                "--enable-lighting" => {
                    scene.lighting_enabled = true;
                }
                "--chunk-x" => scene.chunk_x = parse_i32_arg("--chunk-x", args.next())?,
                "--chunk-z" => scene.chunk_z = parse_i32_arg("--chunk-z", args.next())?,
                "--render-distance" => {
                    scene.render_distance =
                        parse_render_distance_arg("--render-distance", args.next())?
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
                    fullbright_explicit = true;
                }
                "--disable-section-occlusion" => {
                    render_options.section_occlusion_culling = false;
                }
                "--enable-section-occlusion" => {
                    render_options.section_occlusion_culling = true;
                }
                "--force-fullbright" => {
                    render_options.force_fullbright = true;
                    fullbright_explicit = true;
                }
                "--disable-fullbright" => {
                    render_options.force_fullbright = false;
                    fullbright_explicit = true;
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
                    if !movement_frame_probe {
                        frame_budget_probe = true;
                    }
                    frame_budget_frames =
                        parse_frame_budget_frames_arg("--frame-budget-frames", args.next())?;
                }
                "--target-hz" => {
                    if !movement_frame_probe {
                        frame_budget_probe = true;
                    }
                    target_hz = parse_target_hz_arg("--target-hz", args.next())?;
                }
                "--movement-frame-speed" => {
                    if explicit_frame_budget_probe {
                        bail!(
                            "--movement-frame-speed cannot be combined with --frame-budget-probe"
                        );
                    }
                    frame_budget_probe = false;
                    movement_frame_probe = true;
                    movement_speed =
                        parse_movement_speed_arg("--movement-frame-speed", args.next())?;
                }
                "--movement-path-radius" => {
                    movement_perf = true;
                    path_radius = parse_path_radius_arg(&arg, args.next())?;
                }
                "--path-radius" => {
                    path_radius = parse_path_radius_arg(&arg, args.next())?;
                }
                "--frames" => {
                    xr_frames_explicit = true;
                    xr_frame_limit = Some(parse_xr_smoke_frames_arg("--frames", args.next())?);
                }
                "--xr-frames" => {
                    xr_frames_explicit = true;
                    xr_frame_limit = Some(parse_xr_smoke_frames_arg("--xr-frames", args.next())?);
                }
                "--xr-forever" => {
                    xr_forever_explicit = true;
                    xr_frame_limit = None;
                }
                "--xr-view-pose" | "--view-pose" => {
                    xr_view_pose_explicit = true;
                    xr_view_pose = parse_xr_view_pose_arg(&arg, args.next())?;
                }
                _ if arg.starts_with("--xr-view-pose=") => {
                    xr_view_pose_explicit = true;
                    xr_view_pose = parse_xr_view_pose_value(
                        "--xr-view-pose",
                        &arg["--xr-view-pose=".len()..],
                    )?;
                }
                _ if arg.starts_with("--view-pose=") => {
                    xr_view_pose_explicit = true;
                    xr_view_pose =
                        parse_xr_view_pose_value("--view-pose", &arg["--view-pose=".len()..])?;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }

        let perf_mode_count = movement_perf as u8
            + timedemo as u8
            + frame_budget_probe as u8
            + movement_frame_probe as u8;
        if perf_mode_count > 1 {
            bail!(
                "--movement-perf, --timedemo, --frame-budget-probe, and --movement-frame-probe are mutually exclusive"
            );
        }
        if (xr_clear_smoke || xr_mclone_smoke) && (mode.is_some() || perf_mode_count > 0) {
            bail!("XR smoke modes cannot be combined with headless or perf modes");
        }
        if window_start_intent == WindowStartIntent::Menu
            && (mode.is_some() || perf_mode_count > 0 || xr_clear_smoke || xr_mclone_smoke)
        {
            bail!("--menu/--start-in-world false only apply to window mode");
        }
        if xr_frames_explicit && !xr_clear_smoke && !xr_mclone_smoke {
            bail!("--frames requires --xr-clear-smoke or --xr-mclone-smoke");
        }
        if xr_forever_explicit && !xr_clear_smoke && !xr_mclone_smoke {
            bail!("--xr-forever requires --xr-clear-smoke or --xr-mclone-smoke");
        }
        if xr_frames_explicit && xr_forever_explicit {
            bail!("--xr-forever cannot be combined with --frames or --xr-frames");
        }
        if xr_view_pose_explicit && !xr_mclone_smoke {
            bail!("--xr-view-pose requires --xr-mclone-smoke");
        }
        if !scene.lighting_enabled && !fullbright_explicit {
            render_options.force_fullbright = true;
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
            Some(HeadlessMode::DualView(directory)) => Ok(Self::HeadlessDualView {
                options: HeadlessDualViewOptions {
                    directory,
                    width: width.unwrap_or(960),
                    height: height.unwrap_or(640),
                    scene,
                    render_options,
                },
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
                    scripted_interaction: screenshot_scripted_interaction,
                    remote_settle_ms: screenshot_remote_settle_ms,
                    eye: screenshot_eye,
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
            None if frame_budget_probe || movement_frame_probe => Ok(Self::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    scene,
                    render_options,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    mode: if movement_frame_probe {
                        FrameBudgetProbeMode::MovementWalk
                    } else {
                        FrameBudgetProbeMode::StressOrbit
                    },
                    frames: frame_budget_frames,
                    path_radius_chunks: path_radius,
                    target_hz,
                    movement_speed,
                },
            }),
            None if xr_clear_smoke => Ok(Self::XrClearSmoke {
                options: XrClearSmokeOptions {
                    frame_limit: xr_frame_limit,
                },
            }),
            None if xr_mclone_smoke => Ok(Self::XrMcloneSmoke {
                options: XrMcloneSmokeOptions {
                    scene,
                    render_options,
                    frame_limit: xr_frame_limit,
                    view_pose: xr_view_pose,
                },
            }),
            None => Ok(Self::Window {
                scene,
                render_options,
                start_intent: window_start_intent,
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

fn parse_u64_arg(flag: &str, value: Option<String>) -> Result<u64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<u64>()
        .with_context(|| format!("{flag} requires an unsigned 64-bit integer, got `{value}`"))
}

fn parse_render_distance_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let parsed = parse_i32_arg(flag, value)?;
    if !(MIN_RENDER_DISTANCE..=MAX_RENDER_DISTANCE).contains(&parsed) {
        bail!("{flag} must be between {MIN_RENDER_DISTANCE} and {MAX_RENDER_DISTANCE}");
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

fn parse_xr_smoke_frames_arg(flag: &str, value: Option<String>) -> Result<u32> {
    let parsed = parse_u32_arg(flag, value)?;
    if !(1..=MAX_XR_SMOKE_FRAMES).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_XR_SMOKE_FRAMES}");
    }
    Ok(parsed)
}

fn parse_xr_view_pose_arg(flag: &str, value: Option<String>) -> Result<Option<XrViewPose>> {
    let value = value.with_context(|| format!("{flag} requires X,Y,Z,YAW_DEGREES"))?;
    parse_xr_view_pose_value(flag, &value)
}

fn parse_xr_view_pose_value(flag: &str, value: &str) -> Result<Option<XrViewPose>> {
    let value = value.trim();
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
        bail!("{flag} expects X,Y,Z,YAW_DEGREES");
    }
    let mut numbers = [0.0; 4];
    for (index, part) in parts.into_iter().enumerate() {
        let number = part
            .parse::<f32>()
            .with_context(|| format!("invalid {flag} component `{part}`"))?;
        if !number.is_finite() {
            bail!("{flag} component `{part}` must be finite");
        }
        numbers[index] = number;
    }
    Ok(Some(XrViewPose {
        position: [numbers[0], numbers[1], numbers[2]],
        yaw_degrees: numbers[3],
    }))
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

fn parse_movement_speed_arg(flag: &str, value: Option<String>) -> Result<f32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<f32>()
        .with_context(|| format!("{flag} requires a positive number, got `{value}`"))?;
    if !parsed.is_finite() || !(SPECTATOR_MIN_SPEED..=SPECTATOR_MAX_SPEED).contains(&parsed) {
        bail!("{flag} must be between {SPECTATOR_MIN_SPEED} and {SPECTATOR_MAX_SPEED}");
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

fn parse_screenshot_remote_settle_ms_arg(flag: &str, value: Option<String>) -> Result<u64> {
    let parsed = parse_u64_arg(flag, value)?;
    if parsed > MAX_SCREENSHOT_REMOTE_SETTLE_MS {
        bail!("{flag} must be at most {MAX_SCREENSHOT_REMOTE_SETTLE_MS}");
    }
    Ok(parsed)
}

fn parse_f32_vec3_arg(flag: &str, value: Option<String>) -> Result<[f32; 3]> {
    let raw = value.with_context(|| format!("{flag} requires x,y,z"))?;
    let parts = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        bail!("{flag} expects x,y,z");
    }
    let mut values = [0.0; 3];
    for (index, part) in parts.into_iter().enumerate() {
        let value: f32 = part
            .parse()
            .with_context(|| format!("invalid {flag} component `{part}`"))?;
        if !value.is_finite() {
            bail!("{flag} component `{part}` must be finite");
        }
        values[index] = value;
    }
    Ok(values)
}

pub(crate) fn parse_screenshot_ui_arg(
    flag: &str,
    value: Option<String>,
) -> Result<HeadlessScreenshotUi> {
    let value = value.with_context(|| {
        format!("{flag} requires none, title, new-world, pause, options-title, or options-pause")
    })?;
    match value.as_str() {
        "none" | "off" | "false" | "0" => Ok(HeadlessScreenshotUi::None),
        "title" => Ok(HeadlessScreenshotUi::Title),
        "new-world" | "new_world" => Ok(HeadlessScreenshotUi::NewWorld),
        "pause" => Ok(HeadlessScreenshotUi::Pause),
        "options-title" | "options_title" => Ok(HeadlessScreenshotUi::OptionsTitle),
        "options-pause" | "options_pause" | "options" => Ok(HeadlessScreenshotUi::OptionsPause),
        _ => bail!(
            "{flag} must be none, title, new-world, pause, options-title, or options-pause, got `{value}`"
        ),
    }
}

fn print_help() {
    println!(
        "mclone-native-client\n\n\
         Usage:\n\
          mclone-native-client [--menu|--start-in-world true|false] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--remote-addr 127.0.0.1:25565] [--section-occlusion true|false] [--lighting true|false]\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\
           mclone-native-client --headless-ui /tmp/mclone-ui-title.png [--width 960] [--height 540]\n\
          mclone-native-client --screenshot /tmp/mclone-frame.png [--width 1280] [--height 720] [--screenshot-ui none|title|new-world|pause|options-title|options-pause] [--screenshot-debug-pane true|false] [--screenshot-scripted-interaction true|false] [--screenshot-remote-settle-ms 0] [--screenshot-eye x,y,z] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--section-occlusion true|false] [--lighting true|false] [--fullbright true|false]\n\
           mclone-native-client --headless-chunk /tmp/mclone-native-chunk.png [--width 640] [--height 480] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--remote-addr 127.0.0.1:25565] [--section-occlusion true|false] [--lighting true|false] [--fullbright true|false]\n\
           mclone-native-client --headless-chunk-scenarios /tmp/mclone-native-camera [--width 960] [--height 640] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --headless-dual-view /tmp/mclone-dual-view [--width 960] [--height 640] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --movement-perf [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--movement-steps 12] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --timedemo [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--timedemo-frames 120] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --frame-budget-probe [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--frame-budget-frames 240] [--target-hz 120] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --movement-frame-probe [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--frame-budget-frames 240] [--target-hz 120] [--path-radius 4] [--movement-frame-speed 32] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --xr-clear-smoke [--frames 120|--xr-forever]\n\
           mclone-native-client --xr-mclone-smoke [--frames 120|--xr-forever] [--xr-view-pose X,Y,Z,YAW_DEGREES] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 2] [--day-time 6000] [--freeze-time] [--section-occlusion true|false] [--fullbright true|false]\n\n\
         Window mode streams chunks around a collision-backed local player with WASD walking, Space jump, Ctrl sprint, Shift crouch/sneak input, mouse-lock look, N no-clip debug toggle, X no-clip descend, mouse wheel no-clip speed, tilde debug pane toggle, O section-occlusion toggle, and L fullbright toggle. Use --disable-lighting/--enable-lighting to bypass or restore server-side ChunkStatus::Light promotion; --disable-lighting defaults to fullbright unless --disable-fullbright is also passed. Use --disable-section-occlusion/--enable-section-occlusion and --force-fullbright/--disable-fullbright as shortcuts. Headless modes write PNGs for GPU validation. Perf modes write JSON. Timedemo loads a static render distance large enough to contain its camera path. Frame-budget probe runs a deterministic offscreen streaming stress script. Movement-frame probe runs a speed-based offscreen walking script and counts work frames over an explicit target Hz budget."
    );
}
