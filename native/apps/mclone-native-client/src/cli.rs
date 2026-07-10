use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use mclone_app_runtime::far_lod::FarTerrainLodConfig;
use mclone_app_runtime::frame_render::{MAX_FLAT_RENDER_SCALE, MIN_FLAT_RENDER_SCALE};
use mclone_app_runtime::render_assets::{
    DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS, DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
};
use mclone_app_runtime::startup_args::{
    RenderCompileCapacityRequest, RenderDistanceLimits, StartupArgState, StartupSceneOptions,
    parse_bool_arg, parse_i32_arg, parse_u32_arg, parse_u64_arg,
};
use mclone_input::KeyboardKey;
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render_session::{ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER, EngineCameraViewMode};
use mclone_server::{DEFAULT_LIGHT_STATUS_BATCH_SIZE, SimulationCadenceConfig};
use mclone_ui::{GameHelpParent, GameOptionsCategory, GameOptionsParent, GameScreen};

use crate::camera::{SPECTATOR_BASE_SPEED, SPECTATOR_MAX_SPEED, SPECTATOR_MIN_SPEED};
use crate::render_compile_capacity::{
    RenderCompileCapacityHostKind, preflight_render_compile_capacity_report,
};
use crate::{
    DEFAULT_CHUNK_X, DEFAULT_CHUNK_Z, DEFAULT_FRAME_BUDGET_PROBE_FRAMES,
    DEFAULT_FRAME_BUDGET_TARGET_HZ, DEFAULT_MOVEMENT_PERF_PATH_RADIUS, DEFAULT_MOVEMENT_PERF_STEPS,
    DEFAULT_RENDER_DISTANCE, DEFAULT_SEED, DEFAULT_STARTUP_STREAMING_PERF_FRAMES,
    DEFAULT_TIMEDEMO_FRAMES, DEFAULT_TIMEDEMO_PATH_RADIUS, MAX_FRAME_BUDGET_PROBE_FRAMES,
    MAX_LOADING_SETTLE_DISTANCE_COUNT, MAX_MOVEMENT_PERF_PATH_RADIUS, MAX_MOVEMENT_PERF_STEPS,
    MAX_RENDER_DISTANCE, MAX_STARTUP_STREAMING_PERF_FRAMES, MAX_TIMEDEMO_FRAMES,
    MIN_RENDER_DISTANCE,
};

const MAX_SCREENSHOT_REMOTE_SETTLE_MS: u64 = 10_000;
const MAX_STARTUP_WAIT_FRAMES: u32 = 4096;
const DEFAULT_ACTOR_WALK_REVIEW_FRAMES: usize = 24;
const MAX_ACTOR_WALK_REVIEW_FRAMES: usize = 240;
const DEFAULT_ACTOR_WALK_REVIEW_FPS: u32 = 12;
const MAX_ACTOR_WALK_REVIEW_FPS: u32 = 120;
const DEFAULT_ACTOR_WALK_REVIEW_CYCLES: f32 = 2.0;
const MAX_ACTOR_WALK_REVIEW_CYCLES: f32 = 16.0;
const DEFAULT_XR_CLEAR_SMOKE_FRAMES: u32 = 120;
const MAX_XR_SMOKE_FRAMES: u32 = 4096;
const MAX_SIMULATION_CADENCE_RATE_HZ: u32 = 240;
const DEFAULT_XR_EMULATION_INPUT_FRAMES: usize = 8;
const MAX_XR_EMULATION_INPUT_FRAMES: usize = 600;
const DEFAULT_WINDOW_FRAME_REPORT_FRAMES: usize = 3600;
const MAX_WINDOW_FRAME_REPORT_FRAMES: usize = 72000;

// Desktop-owned flags must be mode selection, file paths, window/headless
// harness options, or desktop/XR validation glue. Engine/session startup
// policy belongs in `mclone-app-runtime::startup_args::STARTUP_ARG_FLAGS`.
// The source-scan test fails when a new double-dash flag appears in this file without
// being classified in either shared startup or this desktop-local registry.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const DESKTOP_LOCAL_ARG_FLAGS: &[&str] = &[
    "--actor-review-sheet",
    "--actor-walk-review",
    "--actor-walk-review-video",
    "--adaptive-chunk-publication-budget",
    "--adaptive-render-admission-budget",
    "--cadence",
    "--desktop-xr",
    "--far-lod",
    "--first-person-player",
    "--frame-accounting",
    "--frame-budget-frames",
    "--frame-budget-probe",
    "--frames",
    "--freeze-scheduled-fluid-ticks",
    "--headless-clear",
    "--headless-dual-view",
    "--headless-dual-view-hud",
    "--height",
    "--help",
    "--loading-settle-distances",
    "--loading-settle-perf",
    "--menu",
    "--movement-frame-probe",
    "--movement-frame-speed",
    "--movement-perf",
    "--movement-steps",
    "--no-window",
    "--path-radius",
    "--rebuild-render-scale",
    "--remote-player-visual-smoke",
    "--render-compile-worker-timing",
    "--renderer-rebuild-smoke",
    "--screenshot",
    "--screenshot-blink-debug",
    "--screenshot-camera-view",
    "--screenshot-debug-pane",
    "--screenshot-frame-pipeline-overlay",
    "--screenshot-hud",
    "--screenshot-player-box",
    "--screenshot-remote-settle-ms",
    "--screenshot-scripted-interaction",
    "--screenshot-ui",
    "--settle-distances",
    "--simulation-cadence",
    "--start-in-world",
    "--startup-lod-prewarm",
    "--startup-streaming-frames",
    "--startup-streaming-perf",
    "--startup-streaming-persisted-world",
    "--startup-wait",
    "--target-hz",
    "--timedemo",
    "--timedemo-frames",
    "--torch-light-probe",
    "--view-pose",
    "--walk-review-cycles",
    "--walk-review-fps",
    "--walk-review-frames",
    "--width",
    "--window-frame-report",
    "--window-frame-report-frames",
    "--xr-clear-smoke",
    "--xr-debug-ui",
    "--xr-emulation-input-frames",
    "--xr-emulation-key",
    "--xr-emulation-screenshot",
    "--xr-forever",
    "--xr-mclone-smoke",
    "--xr-underwater-mode",
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SceneOptions {
    pub(crate) seed: i64,
    pub(crate) chunk_x: i32,
    pub(crate) chunk_z: i32,
    pub(crate) render_distance: i32,
    pub(crate) render_compile_worker_count: usize,
    pub(crate) render_compile_max_pending_jobs: Option<usize>,
    pub(crate) render_compile_capacity_mode: RenderCompileCapacityMode,
    pub(crate) render_compile_worker_timing_enabled: bool,
    pub(crate) remote_addr: Option<String>,
    pub(crate) world_root: Option<PathBuf>,
    pub(crate) world_dir: Option<PathBuf>,
    /// Debug override: force the day/night clock to this `dayTime` (ticks) for
    /// captures, instead of using whatever the simulation has advanced to.
    pub(crate) day_time_override: Option<u64>,
    /// Debug: stop the integrated server from advancing the day/night clock, so a
    /// forced (or initial) `dayTime` stays put for inspection.
    pub(crate) freeze_time: bool,
    pub(crate) movement_speed_multiplier: f32,
    pub(crate) simulation_cadence: SimulationCadenceConfig,
    pub(crate) first_person_player_visible: bool,
    pub(crate) debug_passive_showcase: bool,
    /// Debug/perf switch: bypass native `ChunkStatus::Light` promotion and let
    /// generated `Features` snapshots stream directly to the client.
    pub(crate) lighting_enabled: bool,
    /// Debug/perf controller for how many feature publications are coalesced
    /// into one initial light-status worker batch.
    pub(crate) light_status_batch_size: usize,
    /// Experimental shared scheduler controller for feature/light publication.
    pub(crate) adaptive_chunk_publication_budget: bool,
    /// Experimental shared controller for live desktop render admission.
    pub(crate) adaptive_render_admission_budget: bool,
    /// Experimental, opt-in far surface LOD shell. Disabled by default.
    pub(crate) far_lod: FarTerrainLodConfig,
    /// Startup LOD prewarm (tactical 162 Slice 1): build cheap retained far-LOD
    /// coverage before the first playable frame. Only active when `far_lod` is
    /// enabled. Defaults on so `--far-lod true` prewarms; disable with
    /// `--startup-lod-prewarm false`.
    pub(crate) startup_lod_prewarm: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum RenderCompileCapacityMode {
    #[default]
    Default,
    Manual,
    DerivedApplied,
}

impl RenderCompileCapacityMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Manual => "manual",
            Self::DerivedApplied => "derivedApplied",
        }
    }
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
    pub(crate) frame_accounting_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LoadingSettlePerfOptions {
    pub(crate) scene: SceneOptions,
    pub(crate) distances: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StartupStreamingPerfOptions {
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: usize,
    pub(crate) target_hz: f64,
    pub(crate) persisted_world: bool,
    pub(crate) freeze_scheduled_fluid_ticks: bool,
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
    pub(crate) startup_wait: StartupWaitPolicy,
    pub(crate) camera_view: EngineCameraViewMode,
    pub(crate) ui: HeadlessScreenshotUi,
    pub(crate) hud: bool,
    pub(crate) frame_pipeline_overlay: bool,
    pub(crate) debug_pane: bool,
    pub(crate) player_collision_box: bool,
    pub(crate) blink_debug: bool,
    pub(crate) scripted_interaction: bool,
    pub(crate) remote_settle_ms: u64,
    pub(crate) eye: Option<[f32; 3]>,
    pub(crate) target: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessDualViewOptions {
    pub(crate) directory: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) hud: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct XrEmulationScreenshotOptions {
    pub(crate) path: PathBuf,
    /// Per-eye dimensions; the saved image is side-by-side at twice the width.
    pub(crate) eye_width: u32,
    pub(crate) eye_height: u32,
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) held_keys: Vec<KeyboardKey>,
    pub(crate) input_frames: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessActorReviewSheetOptions {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) render_options: TexturedSectionRenderOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessActorWalkReviewOptions {
    pub(crate) sheet_path: PathBuf,
    pub(crate) video_path: Option<PathBuf>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: usize,
    pub(crate) fps: u32,
    pub(crate) cycles: f32,
    pub(crate) render_options: TexturedSectionRenderOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RendererRebuildSmokeOptions {
    pub(crate) directory: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scene: SceneOptions,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) rebuild_render_scale: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TorchLightProbeOptions {
    pub(crate) directory: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) render_options: TexturedSectionRenderOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RemotePlayerVisualSmokeOptions {
    pub(crate) path: PathBuf,
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
    pub(crate) underwater_mode: XrUnderwaterMode,
    pub(crate) debug_ui_screen: Option<XrDebugUiScreen>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct XrViewPose {
    pub(crate) position: [f32; 3],
    pub(crate) yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum XrDebugUiScreen {
    Pause,
    Controls,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum XrUnderwaterMode {
    #[default]
    Midpoint,
    PerEye,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum HeadlessScreenshotUi {
    #[default]
    None,
    Title,
    WorldList,
    WorldCreate,
    WorldDeleteConfirm,
    NewWorld,
    JoinRemote,
    Pause,
    Help,
    BlockPalette,
    OptionsTitle,
    OptionsPause,
    OptionsGraphicsPause,
    OptionsMovementPause,
    OptionsDisplayPause,
    OptionsDebugPause,
    ServerSettingsPause,
}

impl HeadlessScreenshotUi {
    pub(crate) fn game_screen(self) -> Option<GameScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(GameScreen::Title),
            Self::WorldList => Some(GameScreen::WorldList),
            Self::WorldCreate => Some(GameScreen::WorldCreate),
            Self::WorldDeleteConfirm => Some(GameScreen::WorldDeleteConfirm {
                id: mclone_ui::WorldCatalogUiWorldId(0),
            }),
            Self::NewWorld => Some(GameScreen::NewWorld),
            Self::JoinRemote => Some(GameScreen::JoinRemote),
            Self::Pause => Some(GameScreen::Pause),
            Self::Help => Some(GameScreen::Help {
                parent: GameHelpParent::Game,
            }),
            Self::BlockPalette => Some(GameScreen::BlockPalette),
            Self::OptionsTitle => Some(GameScreen::Options {
                parent: GameOptionsParent::Title,
            }),
            Self::OptionsPause => Some(GameScreen::Options {
                parent: GameOptionsParent::Pause,
            }),
            Self::OptionsGraphicsPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Graphics,
            }),
            Self::OptionsMovementPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Movement,
            }),
            Self::OptionsDisplayPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Display,
            }),
            Self::OptionsDebugPause => Some(GameScreen::OptionsCategory {
                parent: GameOptionsParent::Pause,
                category: GameOptionsCategory::Debug,
            }),
            Self::ServerSettingsPause => Some(GameScreen::ServerSettings {
                parent: GameOptionsParent::Pause,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WindowStartIntent {
    #[default]
    InWorld,
    Menu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupWaitPolicy {
    None,
    Progress,
    Playable,
    Idle,
    Frames(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowFrameReportOptions {
    pub(crate) path: PathBuf,
    pub(crate) frames: usize,
}

impl StartupWaitPolicy {
    pub(crate) const DESKTOP_DEFAULT: Self = Self::Playable;
    pub(crate) const OFFSCREEN_SCREENSHOT_DEFAULT: Self = Self::Idle;

    pub(crate) fn offscreen_capture_frame_count(self) -> usize {
        match self {
            Self::Frames(frames) => (frames as usize).saturating_add(1),
            Self::None | Self::Progress | Self::Playable | Self::Idle => 1,
        }
    }
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
            frame_accounting_enabled: true,
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
            render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            render_compile_max_pending_jobs: Some(DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS),
            render_compile_capacity_mode: RenderCompileCapacityMode::Default,
            render_compile_worker_timing_enabled: true,
            remote_addr: None,
            world_root: Some(default_native_world_root()),
            world_dir: None,
            day_time_override: None,
            freeze_time: false,
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            simulation_cadence: SimulationCadenceConfig::default(),
            first_person_player_visible: false,
            debug_passive_showcase: true,
            lighting_enabled: true,
            light_status_batch_size: DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            adaptive_chunk_publication_budget: true,
            adaptive_render_admission_budget: false,
            far_lod: FarTerrainLodConfig::default(),
            startup_lod_prewarm: true,
        }
    }
}

impl SceneOptions {
    pub(crate) fn to_startup_scene(&self) -> StartupSceneOptions {
        StartupSceneOptions {
            seed: self.seed,
            chunk_x: self.chunk_x,
            chunk_z: self.chunk_z,
            render_distance: u32::try_from(self.render_distance)
                .expect("desktop default render distance is non-negative"),
            render_compile_worker_count: self.render_compile_worker_count,
            render_compile_max_pending_jobs: self.render_compile_max_pending_jobs,
            render_compile_capacity_request: if self.render_compile_capacity_mode
                == RenderCompileCapacityMode::DerivedApplied
            {
                RenderCompileCapacityRequest::Derived
            } else {
                RenderCompileCapacityRequest::Default
            },
            remote_addr: self.remote_addr.clone(),
            day_time_override: self.day_time_override,
            freeze_time: self.freeze_time,
            movement_speed_multiplier: self.movement_speed_multiplier,
            debug_passive_showcase: self.debug_passive_showcase,
            lighting_enabled: self.lighting_enabled,
            light_status_batch_size: self.light_status_batch_size,
            far_lod: self.far_lod,
        }
    }

    fn from_startup_scene(scene: StartupSceneOptions) -> Result<Self> {
        Ok(Self {
            seed: scene.seed,
            chunk_x: scene.chunk_x,
            chunk_z: scene.chunk_z,
            render_distance: i32::try_from(scene.render_distance)
                .context("desktop render distance does not fit i32")?,
            render_compile_worker_count: scene.render_compile_worker_count,
            render_compile_max_pending_jobs: scene.render_compile_max_pending_jobs,
            render_compile_capacity_mode: RenderCompileCapacityMode::Default,
            render_compile_worker_timing_enabled: true,
            remote_addr: scene.remote_addr,
            world_root: Some(default_native_world_root()),
            world_dir: None,
            day_time_override: scene.day_time_override,
            freeze_time: scene.freeze_time,
            movement_speed_multiplier: scene.movement_speed_multiplier,
            simulation_cadence: SimulationCadenceConfig::default(),
            first_person_player_visible: false,
            debug_passive_showcase: scene.debug_passive_showcase,
            lighting_enabled: scene.lighting_enabled,
            light_status_batch_size: scene.light_status_batch_size,
            adaptive_chunk_publication_budget: true,
            adaptive_render_admission_budget: false,
            far_lod: scene.far_lod,
            startup_lod_prewarm: true,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cli {
    Window {
        scene: SceneOptions,
        render_options: TexturedSectionRenderOptions,
        start_intent: WindowStartIntent,
        startup_wait: StartupWaitPolicy,
        frame_report: Option<WindowFrameReportOptions>,
    },
    HeadlessClear {
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
    XrEmulationScreenshot {
        options: XrEmulationScreenshotOptions,
    },
    HeadlessActorReviewSheet {
        options: HeadlessActorReviewSheetOptions,
    },
    HeadlessActorWalkReview {
        options: HeadlessActorWalkReviewOptions,
    },
    RendererRebuildSmoke {
        options: RendererRebuildSmokeOptions,
    },
    TorchLightProbe {
        options: TorchLightProbeOptions,
    },
    RemotePlayerVisualSmoke {
        options: RemotePlayerVisualSmokeOptions,
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
    StartupStreamingPerf {
        options: StartupStreamingPerfOptions,
    },
    LoadingSettlePerf {
        options: LoadingSettlePerfOptions,
    },
    XrClearSmoke {
        options: XrClearSmokeOptions,
    },
    XrMcloneSmoke {
        options: XrMcloneSmokeOptions,
    },
    /// The real desktop XR run verb (`--desktop-xr`): the mclone world, persistent
    /// by default (unbounded frames), with a desktop companion window unless
    /// `--no-window` is passed. Reuses `XrMcloneSmokeOptions` for the shared world
    /// content; `window` toggles the Slice 3 companion window.
    DesktopXr {
        options: XrMcloneSmokeOptions,
        window: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessMode {
    ActorReviewSheet(PathBuf),
    ActorWalkReview(PathBuf),
    Clear(PathBuf),
    DualView(PathBuf),
    Screenshot(PathBuf),
    XrEmulationScreenshot(PathBuf),
    RendererRebuildSmoke(PathBuf),
    TorchLightProbe(PathBuf),
    RemotePlayerVisualSmoke(PathBuf),
}

impl Cli {
    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut width = None;
        let mut height = None;
        let mut startup_args = StartupArgState::new(
            SceneOptions::default().to_startup_scene(),
            TexturedSectionRenderOptions::default(),
        );
        let mut screenshot_ui = HeadlessScreenshotUi::None;
        let mut screenshot_hud = false;
        let mut headless_dual_view_hud = false;
        let mut xr_emulation_keys = Vec::new();
        let mut xr_emulation_input_frames = DEFAULT_XR_EMULATION_INPUT_FRAMES;
        let mut xr_emulation_input_options_explicit = false;
        let mut screenshot_frame_pipeline_overlay = false;
        let mut screenshot_debug_pane = false;
        let mut screenshot_player_collision_box = false;
        let mut screenshot_blink_debug = false;
        let mut screenshot_scripted_interaction = false;
        let mut screenshot_remote_settle_ms = 0;
        let mut screenshot_camera_view = EngineCameraViewMode::FirstPerson;
        let mut actor_walk_review_video = None;
        let mut actor_walk_review_options_explicit = false;
        let mut actor_walk_review_frames = DEFAULT_ACTOR_WALK_REVIEW_FRAMES;
        let mut actor_walk_review_fps = DEFAULT_ACTOR_WALK_REVIEW_FPS;
        let mut actor_walk_review_cycles = DEFAULT_ACTOR_WALK_REVIEW_CYCLES;
        let mut first_person_player_visible = false;
        let mut simulation_cadence = SimulationCadenceConfig::default();
        let mut simulation_cadence_explicit = false;
        let mut window_start_intent = WindowStartIntent::InWorld;
        let mut startup_wait = None;
        let mut window_frame_report_path = None;
        let mut window_frame_report_frames = DEFAULT_WINDOW_FRAME_REPORT_FRAMES;
        let mut window_frame_report_frames_explicit = false;
        let mut movement_perf = false;
        let mut timedemo = false;
        let mut frame_budget_probe = false;
        let mut explicit_frame_budget_probe = false;
        let mut frame_budget_frames_explicit = false;
        let mut movement_frame_probe = false;
        let mut startup_streaming_perf = false;
        let mut startup_streaming_persisted_world = false;
        let mut loading_settle_perf = false;
        let mut movement_steps = DEFAULT_MOVEMENT_PERF_STEPS;
        let mut timedemo_frames = DEFAULT_TIMEDEMO_FRAMES;
        let mut frame_budget_frames = DEFAULT_FRAME_BUDGET_PROBE_FRAMES;
        let mut startup_streaming_frames = DEFAULT_STARTUP_STREAMING_PERF_FRAMES;
        let mut freeze_scheduled_fluid_ticks = false;
        let mut loading_settle_distances = default_loading_settle_distances();
        let mut target_hz = DEFAULT_FRAME_BUDGET_TARGET_HZ;
        let mut movement_speed = SPECTATOR_BASE_SPEED;
        let mut frame_accounting_enabled = true;
        let mut path_radius = DEFAULT_MOVEMENT_PERF_PATH_RADIUS;
        let mut xr_clear_smoke = false;
        let mut xr_mclone_smoke = false;
        let mut desktop_xr = false;
        let mut no_window_explicit = false;
        let mut xr_frames_explicit = false;
        let mut xr_forever_explicit = false;
        let mut xr_frame_limit = Some(DEFAULT_XR_CLEAR_SMOKE_FRAMES);
        let mut xr_view_pose_explicit = false;
        let mut xr_view_pose = None;
        let mut xr_underwater_mode_explicit = false;
        let mut xr_underwater_mode = XrUnderwaterMode::default();
        let mut xr_debug_ui_screen_explicit = false;
        let mut xr_debug_ui_screen = None;
        let mut rebuild_render_scale = None;
        let mut far_lod = FarTerrainLodConfig::default();
        let mut startup_lod_prewarm = true;
        let mut adaptive_chunk_publication_budget = None;
        let mut adaptive_render_admission_budget = None;
        let mut render_compile_worker_timing_enabled = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--xr-clear-smoke" => {
                    if mode.is_some()
                        || movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                        || xr_mclone_smoke
                        || desktop_xr
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
                        || startup_streaming_perf
                        || loading_settle_perf
                        || xr_clear_smoke
                        || desktop_xr
                    {
                        bail!("--xr-mclone-smoke cannot be combined with other run modes");
                    }
                    xr_mclone_smoke = true;
                }
                "--desktop-xr" => {
                    if mode.is_some()
                        || movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                        || xr_clear_smoke
                        || xr_mclone_smoke
                    {
                        bail!("--desktop-xr cannot be combined with other run modes");
                    }
                    desktop_xr = true;
                }
                "--no-window" => {
                    no_window_explicit = true;
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
                "--loading-settle-perf" => {
                    if mode.is_some() {
                        bail!(
                            "--loading-settle-perf cannot be combined with a headless output mode"
                        );
                    }
                    loading_settle_perf = true;
                }
                "--startup-streaming-perf" => {
                    if mode.is_some() {
                        bail!(
                            "--startup-streaming-perf cannot be combined with a headless output mode"
                        );
                    }
                    if frame_budget_probe
                        && !explicit_frame_budget_probe
                        && !frame_budget_frames_explicit
                    {
                        frame_budget_probe = false;
                    }
                    startup_streaming_perf = true;
                }
                "--startup-streaming-persisted-world" => {
                    if mode.is_some() {
                        bail!(
                            "--startup-streaming-persisted-world cannot be combined with a headless output mode"
                        );
                    }
                    if frame_budget_probe
                        && !explicit_frame_budget_probe
                        && !frame_budget_frames_explicit
                    {
                        frame_budget_probe = false;
                    }
                    startup_streaming_perf = true;
                    startup_streaming_persisted_world = true;
                }
                "--freeze-scheduled-fluid-ticks" => {
                    if frame_budget_probe
                        && !explicit_frame_budget_probe
                        && !frame_budget_frames_explicit
                    {
                        frame_budget_probe = false;
                    }
                    startup_streaming_perf = true;
                    freeze_scheduled_fluid_ticks = true;
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
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Clear(path))?;
                }
                "--actor-review-sheet" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--actor-review-sheet requires an output PNG path")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::ActorReviewSheet(path))?;
                }
                "--actor-walk-review" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--actor-walk-review requires an output PNG path")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::ActorWalkReview(path))?;
                }
                "--actor-walk-review-video" => {
                    actor_walk_review_video = Some(
                        args.next()
                            .map(PathBuf::from)
                            .context("--actor-walk-review-video requires an output MP4 path")?,
                    );
                }
                "--walk-review-frames" => {
                    actor_walk_review_options_explicit = true;
                    actor_walk_review_frames =
                        parse_actor_walk_review_frames_arg(&arg, args.next())?;
                }
                "--walk-review-fps" => {
                    actor_walk_review_options_explicit = true;
                    actor_walk_review_fps = parse_actor_walk_review_fps_arg(&arg, args.next())?;
                }
                "--walk-review-cycles" => {
                    actor_walk_review_options_explicit = true;
                    actor_walk_review_cycles =
                        parse_actor_walk_review_cycles_arg(&arg, args.next())?;
                }
                "--headless-dual-view" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-dual-view requires an output directory")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::DualView(path))?;
                }
                "--headless-dual-view-hud" => {
                    headless_dual_view_hud =
                        parse_bool_arg("--headless-dual-view-hud", args.next())?;
                }
                "--screenshot" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--screenshot requires an output PNG path")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::Screenshot(path))?;
                }
                "--xr-emulation-screenshot" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--xr-emulation-screenshot requires an output PNG path")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::XrEmulationScreenshot(path))?;
                }
                "--xr-emulation-key" => {
                    xr_emulation_input_options_explicit = true;
                    let value = args
                        .next()
                        .context("--xr-emulation-key requires a physical key code")?;
                    let key = KeyboardKey::from_code_name(&value).with_context(|| {
                        format!(
                            "--xr-emulation-key expected a physical key code such as KeyW or ArrowLeft, got `{value}`"
                        )
                    })?;
                    xr_emulation_keys.push(key);
                }
                "--xr-emulation-input-frames" => {
                    xr_emulation_input_options_explicit = true;
                    let value = args
                        .next()
                        .context("--xr-emulation-input-frames requires a frame count")?;
                    xr_emulation_input_frames = value.parse::<usize>().with_context(|| {
                        format!("--xr-emulation-input-frames expected an integer, got `{value}`")
                    })?;
                    if !(1..=MAX_XR_EMULATION_INPUT_FRAMES).contains(&xr_emulation_input_frames) {
                        bail!(
                            "--xr-emulation-input-frames must be between 1 and {MAX_XR_EMULATION_INPUT_FRAMES}"
                        );
                    }
                }
                "--renderer-rebuild-smoke" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--renderer-rebuild-smoke requires an output directory")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::RendererRebuildSmoke(path))?;
                }
                "--torch-light-probe" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--torch-light-probe requires an output directory")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::TorchLightProbe(path))?;
                }
                "--remote-player-visual-smoke" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--remote-player-visual-smoke requires an output PNG path")?;
                    if movement_perf
                        || timedemo
                        || frame_budget_probe
                        || movement_frame_probe
                        || startup_streaming_perf
                        || loading_settle_perf
                    {
                        bail!("headless output modes cannot be combined with perf modes");
                    }
                    set_headless_mode(&mut mode, HeadlessMode::RemotePlayerVisualSmoke(path))?;
                }
                "--rebuild-render-scale" => {
                    rebuild_render_scale = Some(parse_rebuild_render_scale_arg(&arg, args.next())?);
                }
                "--screenshot-ui" => {
                    screenshot_ui = parse_screenshot_ui_arg("--screenshot-ui", args.next())?;
                }
                "--screenshot-hud" => {
                    screenshot_hud = parse_bool_arg("--screenshot-hud", args.next())?;
                }
                "--screenshot-frame-pipeline-overlay" => {
                    screenshot_frame_pipeline_overlay = parse_bool_arg(&arg, args.next())?;
                }
                "--screenshot-debug-pane" => {
                    screenshot_debug_pane = parse_bool_arg("--screenshot-debug-pane", args.next())?;
                }
                "--screenshot-player-box" => {
                    screenshot_player_collision_box =
                        parse_bool_arg("--screenshot-player-box", args.next())?;
                }
                "--screenshot-blink-debug" => {
                    screenshot_blink_debug =
                        parse_bool_arg("--screenshot-blink-debug", args.next())?;
                }
                "--screenshot-scripted-interaction" => {
                    screenshot_scripted_interaction =
                        parse_bool_arg("--screenshot-scripted-interaction", args.next())?;
                }
                "--screenshot-remote-settle-ms" => {
                    screenshot_remote_settle_ms =
                        parse_screenshot_remote_settle_ms_arg(&arg, args.next())?;
                }
                "--screenshot-camera-view" => {
                    screenshot_camera_view = parse_camera_view_arg(&arg, args.next())?;
                }
                "--first-person-player" => {
                    first_person_player_visible =
                        parse_bool_arg("--first-person-player", args.next())?;
                }
                "--simulation-cadence" | "--cadence" => {
                    simulation_cadence_explicit = true;
                    simulation_cadence = parse_simulation_cadence_arg(&arg, args.next())?;
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
                "--startup-wait" => {
                    startup_wait = Some(parse_startup_wait_arg("--startup-wait", args.next())?);
                }
                "--window-frame-report" => {
                    window_frame_report_path = Some(
                        args.next()
                            .map(PathBuf::from)
                            .context("--window-frame-report requires an output JSON path")?,
                    );
                }
                "--window-frame-report-frames" => {
                    window_frame_report_frames_explicit = true;
                    window_frame_report_frames =
                        parse_window_frame_report_frames_arg(&arg, args.next())?;
                }
                "--far-lod" => {
                    far_lod = if parse_bool_arg("--far-lod", args.next())? {
                        FarTerrainLodConfig::enabled()
                    } else {
                        FarTerrainLodConfig::default()
                    };
                }
                "--startup-lod-prewarm" => {
                    startup_lod_prewarm = parse_bool_arg("--startup-lod-prewarm", args.next())?;
                }
                "--adaptive-chunk-publication-budget" => {
                    adaptive_chunk_publication_budget = Some(parse_bool_arg(
                        "--adaptive-chunk-publication-budget",
                        args.next(),
                    )?);
                }
                "--adaptive-render-admission-budget" => {
                    adaptive_render_admission_budget = Some(parse_bool_arg(
                        "--adaptive-render-admission-budget",
                        args.next(),
                    )?);
                }
                "--render-compile-worker-timing" => {
                    render_compile_worker_timing_enabled = Some(parse_bool_arg(
                        "--render-compile-worker-timing",
                        args.next(),
                    )?);
                }
                "--width" => width = Some(parse_u32_arg("--width", args.next())?),
                "--height" => height = Some(parse_u32_arg("--height", args.next())?),
                "--movement-steps" => {
                    movement_perf = true;
                    movement_steps = parse_movement_steps_arg("--movement-steps", args.next())?;
                }
                "--timedemo-frames" => {
                    timedemo = true;
                    timedemo_frames = parse_timedemo_frames_arg("--timedemo-frames", args.next())?;
                }
                "--frame-budget-frames" => {
                    frame_budget_frames_explicit = true;
                    if !movement_frame_probe {
                        frame_budget_probe = true;
                    }
                    frame_budget_frames =
                        parse_frame_budget_frames_arg("--frame-budget-frames", args.next())?;
                }
                "--startup-streaming-frames" => {
                    if frame_budget_probe
                        && !explicit_frame_budget_probe
                        && !frame_budget_frames_explicit
                    {
                        frame_budget_probe = false;
                    }
                    startup_streaming_perf = true;
                    startup_streaming_frames =
                        parse_startup_streaming_frames_arg(&arg, args.next())?;
                }
                "--target-hz" => {
                    if !movement_frame_probe && !startup_streaming_perf {
                        frame_budget_probe = true;
                    }
                    target_hz = parse_target_hz_arg("--target-hz", args.next())?;
                }
                "--frame-accounting" => {
                    frame_accounting_enabled = parse_bool_arg("--frame-accounting", args.next())?;
                }
                "--loading-settle-distances" | "--settle-distances" => {
                    loading_settle_perf = true;
                    loading_settle_distances =
                        parse_loading_settle_distances_arg(&arg, args.next())?;
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
                "--path-radius" => {
                    path_radius = parse_path_radius_arg(&arg, args.next())?;
                }
                "--frames" => {
                    xr_frames_explicit = true;
                    xr_frame_limit = Some(parse_xr_smoke_frames_arg("--frames", args.next())?);
                }
                "--xr-forever" => {
                    xr_forever_explicit = true;
                    xr_frame_limit = None;
                }
                "--view-pose" => {
                    xr_view_pose_explicit = true;
                    xr_view_pose = parse_xr_view_pose_arg(&arg, args.next())?;
                }
                _ if arg.starts_with("--view-pose=") => {
                    xr_view_pose_explicit = true;
                    xr_view_pose =
                        parse_xr_view_pose_value("--view-pose", &arg["--view-pose=".len()..])?;
                }
                "--xr-underwater-mode" => {
                    xr_underwater_mode_explicit = true;
                    xr_underwater_mode = parse_xr_underwater_mode_arg(&arg, args.next())?;
                }
                "--xr-debug-ui" => {
                    xr_debug_ui_screen_explicit = true;
                    xr_debug_ui_screen = parse_xr_debug_ui_arg(&arg, args.next())?;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {
                    if !startup_args.parse_next_arg(
                        &arg,
                        &mut args,
                        desktop_render_distance_limits(),
                    )? {
                        bail!("unknown argument `{arg}`; pass --help for usage");
                    }
                }
            }
        }

        let perf_mode_count = movement_perf as u8
            + timedemo as u8
            + frame_budget_probe as u8
            + movement_frame_probe as u8
            + startup_streaming_perf as u8
            + loading_settle_perf as u8;
        if perf_mode_count > 1 {
            bail!(
                "--movement-perf, --timedemo, --frame-budget-probe, --movement-frame-probe, --startup-streaming-perf, and --loading-settle-perf are mutually exclusive"
            );
        }
        if (xr_clear_smoke || xr_mclone_smoke || desktop_xr)
            && (mode.is_some() || perf_mode_count > 0)
        {
            bail!("XR modes cannot be combined with headless or perf modes");
        }
        if window_frame_report_path.is_some()
            && (mode.is_some()
                || perf_mode_count > 0
                || xr_clear_smoke
                || xr_mclone_smoke
                || desktop_xr)
        {
            bail!("--window-frame-report applies only to window mode");
        }
        if window_frame_report_frames_explicit && window_frame_report_path.is_none() {
            bail!("--window-frame-report-frames requires --window-frame-report");
        }
        if window_start_intent == WindowStartIntent::Menu
            && (mode.is_some()
                || perf_mode_count > 0
                || xr_clear_smoke
                || xr_mclone_smoke
                || desktop_xr)
        {
            bail!("--menu/--start-in-world false only apply to window mode");
        }
        if no_window_explicit && !desktop_xr {
            bail!("--no-window requires --desktop-xr");
        }
        if xr_frames_explicit && !xr_clear_smoke && !xr_mclone_smoke && !desktop_xr {
            bail!("--frames requires --xr-clear-smoke, --xr-mclone-smoke, or --desktop-xr");
        }
        if xr_forever_explicit && !xr_clear_smoke && !xr_mclone_smoke && !desktop_xr {
            bail!("--xr-forever requires --xr-clear-smoke, --xr-mclone-smoke, or --desktop-xr");
        }
        if xr_frames_explicit && xr_forever_explicit {
            bail!("--xr-forever cannot be combined with --frames");
        }
        if xr_view_pose_explicit && !xr_mclone_smoke && !desktop_xr {
            bail!("--view-pose requires --xr-mclone-smoke or --desktop-xr");
        }
        if xr_underwater_mode_explicit && !xr_mclone_smoke && !desktop_xr {
            bail!("--xr-underwater-mode requires --xr-mclone-smoke or --desktop-xr");
        }
        if xr_debug_ui_screen_explicit && !xr_mclone_smoke && !desktop_xr {
            bail!("--xr-debug-ui requires --xr-mclone-smoke or --desktop-xr");
        }
        if rebuild_render_scale.is_some()
            && !matches!(mode, Some(HeadlessMode::RendererRebuildSmoke(_)))
        {
            bail!("--rebuild-render-scale requires --renderer-rebuild-smoke");
        }
        if headless_dual_view_hud && !matches!(mode, Some(HeadlessMode::DualView(_))) {
            bail!("--headless-dual-view-hud requires --headless-dual-view");
        }
        if (actor_walk_review_video.is_some() || actor_walk_review_options_explicit)
            && !matches!(mode, Some(HeadlessMode::ActorWalkReview(_)))
        {
            bail!("actor walk review options require --actor-walk-review");
        }
        if xr_emulation_input_options_explicit
            && !matches!(mode, Some(HeadlessMode::XrEmulationScreenshot(_)))
        {
            bail!("XR emulation input options require --xr-emulation-screenshot");
        }
        if xr_emulation_input_options_explicit
            && xr_emulation_keys.is_empty()
            && matches!(mode, Some(HeadlessMode::XrEmulationScreenshot(_)))
        {
            bail!("--xr-emulation-input-frames requires --xr-emulation-key");
        }
        if startup_wait.is_some()
            && (perf_mode_count > 0
                || xr_clear_smoke
                || xr_mclone_smoke
                || desktop_xr
                || matches!(
                    mode,
                    Some(
                        HeadlessMode::Clear(_)
                            | HeadlessMode::ActorReviewSheet(_)
                            | HeadlessMode::ActorWalkReview(_)
                            | HeadlessMode::DualView(_)
                            | HeadlessMode::XrEmulationScreenshot(_)
                            | HeadlessMode::RendererRebuildSmoke(_)
                            | HeadlessMode::TorchLightProbe(_)
                            | HeadlessMode::RemotePlayerVisualSmoke(_)
                    )
                ))
        {
            bail!("--startup-wait applies to window mode and --screenshot");
        }
        let render_compile_capacity_request = startup_args.scene().render_compile_capacity_request;
        let render_compile_capacity_manual_fields =
            startup_args.has_manual_render_compile_capacity_fields();
        if matches!(
            render_compile_capacity_request,
            RenderCompileCapacityRequest::Derived
        ) && startup_args.scene().remote_addr.is_some()
        {
            bail!("--render-compile-capacity derived applies only to local integrated worlds");
        }
        if matches!(
            render_compile_capacity_request,
            RenderCompileCapacityRequest::Derived
        ) {
            let host_kind = if xr_mclone_smoke
                || matches!(mode, Some(HeadlessMode::XrEmulationScreenshot(_)))
            {
                RenderCompileCapacityHostKind::Xr
            } else {
                RenderCompileCapacityHostKind::Flat
            };
            let report = preflight_render_compile_capacity_report(host_kind);
            startup_args.apply_render_compile_capacity_report(&report);
        }
        let startup_options = startup_args.finish();
        let startup_camera = startup_options.camera;
        let startup_storage = startup_options.storage;
        let mut scene = SceneOptions::from_startup_scene(startup_options.scene)?;
        scene.first_person_player_visible = first_person_player_visible;
        scene.simulation_cadence = simulation_cadence;
        scene.far_lod = far_lod;
        scene.startup_lod_prewarm = startup_lod_prewarm;
        scene.adaptive_chunk_publication_budget =
            adaptive_chunk_publication_budget.unwrap_or(scene.remote_addr.is_none());
        scene.adaptive_render_admission_budget = adaptive_render_admission_budget.unwrap_or(false);
        if matches!(
            render_compile_capacity_request,
            RenderCompileCapacityRequest::Derived
        ) {
            scene.render_compile_capacity_mode = RenderCompileCapacityMode::DerivedApplied;
        } else if render_compile_capacity_manual_fields {
            scene.render_compile_capacity_mode = RenderCompileCapacityMode::Manual;
        }
        if let Some(enabled) = render_compile_worker_timing_enabled {
            scene.render_compile_worker_timing_enabled = enabled;
        }
        let startup_storage = startup_storage.project(Some(default_native_world_root()));
        scene.world_root = startup_storage.world_root.clone();
        scene.world_dir = startup_storage.world_dir.clone();
        if let Some(max_pending_jobs) = scene.render_compile_max_pending_jobs {
            if max_pending_jobs < scene.render_compile_worker_count {
                bail!(
                    "--render-compile-max-pending-jobs must be at least --render-compile-workers"
                );
            }
        }
        if simulation_cadence_explicit && scene.remote_addr.is_some() {
            bail!("--simulation-cadence applies only to local integrated worlds");
        }
        if scene.adaptive_chunk_publication_budget && scene.remote_addr.is_some() {
            bail!("--adaptive-chunk-publication-budget applies only to local integrated worlds");
        }
        if scene.adaptive_render_admission_budget && scene.remote_addr.is_some() {
            bail!("--adaptive-render-admission-budget applies only to local integrated worlds");
        }
        startup_storage.validate_local_integrated_world(scene.remote_addr.as_deref())?;
        let render_options = startup_options.render_options;
        match mode {
            Some(HeadlessMode::Clear(path)) => Ok(Self::HeadlessClear {
                path,
                width: width.unwrap_or(96),
                height: height.unwrap_or(64),
            }),
            Some(HeadlessMode::ActorReviewSheet(path)) => Ok(Self::HeadlessActorReviewSheet {
                options: HeadlessActorReviewSheetOptions {
                    path,
                    width: width.unwrap_or(1152),
                    height: height.unwrap_or(512),
                    render_options,
                },
            }),
            Some(HeadlessMode::ActorWalkReview(sheet_path)) => Ok(Self::HeadlessActorWalkReview {
                options: HeadlessActorWalkReviewOptions {
                    sheet_path,
                    video_path: actor_walk_review_video,
                    width: width.unwrap_or(360),
                    height: height.unwrap_or(360),
                    frames: actor_walk_review_frames,
                    fps: actor_walk_review_fps,
                    cycles: actor_walk_review_cycles,
                    render_options,
                },
            }),
            Some(HeadlessMode::DualView(directory)) => Ok(Self::HeadlessDualView {
                options: HeadlessDualViewOptions {
                    directory,
                    width: width.unwrap_or(960),
                    height: height.unwrap_or(640),
                    scene,
                    render_options,
                    hud: headless_dual_view_hud,
                },
            }),
            Some(HeadlessMode::Screenshot(path)) => Ok(Self::HeadlessScreenshot {
                options: HeadlessScreenshotOptions {
                    path,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    scene,
                    render_options,
                    startup_wait: startup_wait
                        .unwrap_or(StartupWaitPolicy::OFFSCREEN_SCREENSHOT_DEFAULT),
                    camera_view: screenshot_camera_view,
                    ui: screenshot_ui,
                    hud: screenshot_hud || screenshot_frame_pipeline_overlay,
                    frame_pipeline_overlay: screenshot_frame_pipeline_overlay,
                    debug_pane: screenshot_debug_pane,
                    player_collision_box: screenshot_player_collision_box,
                    blink_debug: screenshot_blink_debug,
                    scripted_interaction: screenshot_scripted_interaction,
                    remote_settle_ms: screenshot_remote_settle_ms,
                    eye: startup_camera.eye,
                    target: startup_camera.target,
                },
            }),
            Some(HeadlessMode::XrEmulationScreenshot(path)) => Ok(Self::XrEmulationScreenshot {
                options: XrEmulationScreenshotOptions {
                    path,
                    eye_width: width.unwrap_or(960),
                    eye_height: height.unwrap_or(960),
                    scene,
                    render_options,
                    held_keys: xr_emulation_keys,
                    input_frames: xr_emulation_input_frames,
                },
            }),
            Some(HeadlessMode::RendererRebuildSmoke(directory)) => Ok(Self::RendererRebuildSmoke {
                options: RendererRebuildSmokeOptions {
                    directory,
                    width: width.unwrap_or(960),
                    height: height.unwrap_or(540),
                    scene,
                    render_options,
                    rebuild_render_scale,
                },
            }),
            Some(HeadlessMode::TorchLightProbe(directory)) => Ok(Self::TorchLightProbe {
                options: TorchLightProbeOptions {
                    directory,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    render_options,
                },
            }),
            Some(HeadlessMode::RemotePlayerVisualSmoke(path)) => {
                Ok(Self::RemotePlayerVisualSmoke {
                    options: RemotePlayerVisualSmokeOptions {
                        path,
                        width: width.unwrap_or(960),
                        height: height.unwrap_or(540),
                        scene,
                        render_options,
                    },
                })
            }
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
                    frame_accounting_enabled,
                },
            }),
            None if startup_streaming_perf => Ok(Self::StartupStreamingPerf {
                options: StartupStreamingPerfOptions {
                    scene,
                    render_options,
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(720),
                    frames: startup_streaming_frames,
                    target_hz,
                    persisted_world: startup_streaming_persisted_world,
                    freeze_scheduled_fluid_ticks,
                },
            }),
            None if loading_settle_perf => Ok(Self::LoadingSettlePerf {
                options: LoadingSettlePerfOptions {
                    scene,
                    distances: loading_settle_distances,
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
                    underwater_mode: xr_underwater_mode,
                    debug_ui_screen: xr_debug_ui_screen,
                },
            }),
            None if desktop_xr => Ok(Self::DesktopXr {
                options: XrMcloneSmokeOptions {
                    scene,
                    render_options,
                    // The real verb is persistent by default: only an explicit
                    // `--frames N` bounds it. `--xr-forever` leaves it unbounded.
                    frame_limit: if xr_frames_explicit {
                        xr_frame_limit
                    } else {
                        None
                    },
                    view_pose: xr_view_pose,
                    underwater_mode: xr_underwater_mode,
                    debug_ui_screen: xr_debug_ui_screen,
                },
                window: !no_window_explicit,
            }),
            None => Ok(Self::Window {
                scene,
                render_options,
                start_intent: window_start_intent,
                startup_wait: startup_wait.unwrap_or(StartupWaitPolicy::DESKTOP_DEFAULT),
                frame_report: window_frame_report_path.map(|path| WindowFrameReportOptions {
                    path,
                    frames: window_frame_report_frames,
                }),
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

pub(crate) fn default_native_world_root() -> PathBuf {
    if let Some(path) = std::env::var_os("MCLONE_WORLD_ROOT") {
        return PathBuf::from(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA"))
        {
            return PathBuf::from(path).join("mclone").join("worlds");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("mclone")
                .join("worlds");
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(path).join("mclone").join("worlds");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("mclone")
                .join("worlds");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".mclone")
        .join("worlds")
}

fn default_loading_settle_distances() -> Vec<i32> {
    vec![5, 10, 15, 20]
}

fn parse_loading_settle_distances_arg(flag: &str, value: Option<String>) -> Result<Vec<i32>> {
    let value = value.with_context(|| format!("{flag} requires a comma-separated list"))?;
    let mut distances = Vec::new();
    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let distance = part.parse::<i32>().with_context(|| {
            format!("{flag} expects render distances such as 5,10,15, got `{part}`")
        })?;
        if !(MIN_RENDER_DISTANCE..=MAX_RENDER_DISTANCE).contains(&distance) {
            bail!(
                "{flag} distances must be between {MIN_RENDER_DISTANCE} and {MAX_RENDER_DISTANCE}"
            );
        }
        if distances.contains(&distance) {
            bail!("{flag} contains duplicate distance {distance}");
        }
        distances.push(distance);
    }
    if distances.is_empty() {
        bail!("{flag} requires at least one distance");
    }
    if distances.len() > MAX_LOADING_SETTLE_DISTANCE_COUNT {
        bail!("{flag} accepts at most {MAX_LOADING_SETTLE_DISTANCE_COUNT} distances");
    }
    Ok(distances)
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

fn parse_window_frame_report_frames_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if !(1..=MAX_WINDOW_FRAME_REPORT_FRAMES).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_WINDOW_FRAME_REPORT_FRAMES}");
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

fn parse_startup_streaming_frames_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if !(1..=MAX_STARTUP_STREAMING_PERF_FRAMES).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_STARTUP_STREAMING_PERF_FRAMES}");
    }
    Ok(parsed)
}

fn parse_actor_walk_review_frames_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if !(2..=MAX_ACTOR_WALK_REVIEW_FRAMES).contains(&parsed) {
        bail!("{flag} must be between 2 and {MAX_ACTOR_WALK_REVIEW_FRAMES}");
    }
    Ok(parsed)
}

fn parse_actor_walk_review_fps_arg(flag: &str, value: Option<String>) -> Result<u32> {
    let parsed = parse_u32_arg(flag, value)?;
    if !(1..=MAX_ACTOR_WALK_REVIEW_FPS).contains(&parsed) {
        bail!("{flag} must be between 1 and {MAX_ACTOR_WALK_REVIEW_FPS}");
    }
    Ok(parsed)
}

fn parse_actor_walk_review_cycles_arg(flag: &str, value: Option<String>) -> Result<f32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<f32>()
        .with_context(|| format!("{flag} requires a positive number, got `{value}`"))?;
    if !parsed.is_finite() || !(0.25..=MAX_ACTOR_WALK_REVIEW_CYCLES).contains(&parsed) {
        bail!("{flag} must be between 0.25 and {MAX_ACTOR_WALK_REVIEW_CYCLES}");
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

fn parse_xr_underwater_mode_arg(flag: &str, value: Option<String>) -> Result<XrUnderwaterMode> {
    let value = value.with_context(|| format!("{flag} requires midpoint or per-eye"))?;
    match value.trim() {
        "midpoint" | "middle" | "center" | "both" => Ok(XrUnderwaterMode::Midpoint),
        "per-eye" | "per_eye" | "eye" | "eyes" => Ok(XrUnderwaterMode::PerEye),
        value => bail!("{flag} must be midpoint or per-eye, got `{value}`"),
    }
}

fn parse_xr_debug_ui_arg(flag: &str, value: Option<String>) -> Result<Option<XrDebugUiScreen>> {
    let value = value.with_context(|| format!("{flag} requires none, pause, or controls"))?;
    match value.trim() {
        "none" | "off" | "false" => Ok(None),
        "pause" => Ok(Some(XrDebugUiScreen::Pause)),
        "controls" | "help" => Ok(Some(XrDebugUiScreen::Controls)),
        value => bail!("{flag} must be none, pause, or controls, got `{value}`"),
    }
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

fn parse_rebuild_render_scale_arg(flag: &str, value: Option<String>) -> Result<f32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<f32>()
        .with_context(|| format!("{flag} requires a positive number, got `{value}`"))?;
    if !parsed.is_finite() || !(MIN_FLAT_RENDER_SCALE..=MAX_FLAT_RENDER_SCALE).contains(&parsed) {
        bail!("{flag} must be between {MIN_FLAT_RENDER_SCALE:.2} and {MAX_FLAT_RENDER_SCALE:.2}");
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

fn parse_simulation_cadence_arg(
    flag: &str,
    value: Option<String>,
) -> Result<SimulationCadenceConfig> {
    let value = value.with_context(|| format!("{flag} requires HOST/GAMEPLAY/PHYSICS"))?;
    let parts = value
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        bail!("{flag} must be HOST/GAMEPLAY/PHYSICS, got `{value}`");
    }
    let host_rate_hz = parse_simulation_cadence_rate(flag, "host", parts[0])?;
    let gameplay_rate_hz = parse_simulation_cadence_rate(flag, "gameplay", parts[1])?;
    let physics_rate_hz = parse_simulation_cadence_rate(flag, "physics", parts[2])?;
    let cadence = SimulationCadenceConfig::new(host_rate_hz, gameplay_rate_hz, physics_rate_hz);
    if !cadence.is_valid() {
        bail!(
            "{flag} rates must use clean integer relationships: lower-rate lanes divide host rate and higher-rate lanes are integer substeps of host rate"
        );
    }
    Ok(cadence)
}

fn parse_simulation_cadence_rate(flag: &str, label: &str, value: &str) -> Result<u32> {
    let rate = value
        .parse::<u32>()
        .with_context(|| format!("{flag} {label} rate must be an unsigned integer"))?;
    if !(1..=MAX_SIMULATION_CADENCE_RATE_HZ).contains(&rate) {
        bail!("{flag} {label} rate must be between 1 and {MAX_SIMULATION_CADENCE_RATE_HZ}");
    }
    Ok(rate)
}

fn parse_screenshot_remote_settle_ms_arg(flag: &str, value: Option<String>) -> Result<u64> {
    let parsed = parse_u64_arg(flag, value)?;
    if parsed > MAX_SCREENSHOT_REMOTE_SETTLE_MS {
        bail!("{flag} must be at most {MAX_SCREENSHOT_REMOTE_SETTLE_MS}");
    }
    Ok(parsed)
}

fn parse_camera_view_arg(flag: &str, value: Option<String>) -> Result<EngineCameraViewMode> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    EngineCameraViewMode::parse(&value)
        .with_context(|| format!("{flag} must be first-person or third-person, got `{value}`"))
}

fn parse_startup_wait_arg(flag: &str, value: Option<String>) -> Result<StartupWaitPolicy> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    match value.as_str() {
        "none" => Ok(StartupWaitPolicy::None),
        "progress" => Ok(StartupWaitPolicy::Progress),
        "playable" => Ok(StartupWaitPolicy::Playable),
        "idle" => Ok(StartupWaitPolicy::Idle),
        _ => {
            let frames = value
                .strip_prefix("frames:")
                .or_else(|| value.strip_prefix("frames="))
                .with_context(|| {
                    format!(
                        "{flag} must be none, progress, playable, idle, or frames:N, got `{value}`"
                    )
                })?;
            let frames = frames
                .parse::<u32>()
                .with_context(|| format!("{flag} frames:N requires an unsigned integer"))?;
            if frames > MAX_STARTUP_WAIT_FRAMES {
                bail!("{flag} frames must be between 0 and {MAX_STARTUP_WAIT_FRAMES}");
            }
            Ok(StartupWaitPolicy::Frames(frames))
        }
    }
}

pub(crate) fn parse_screenshot_ui_arg(
    flag: &str,
    value: Option<String>,
) -> Result<HeadlessScreenshotUi> {
    let value = value.with_context(|| {
        format!(
            "{flag} requires none, title, world-list, world-create, world-delete-confirm, new-world, join-remote, pause, help/controls, block-palette, options-title, options-pause, or server-settings-pause"
        )
    })?;
    match value.as_str() {
        "none" | "off" | "false" | "0" => Ok(HeadlessScreenshotUi::None),
        "title" => Ok(HeadlessScreenshotUi::Title),
        "world-list" | "world_list" | "singleplayer" => Ok(HeadlessScreenshotUi::WorldList),
        "world-create" | "world_create" => Ok(HeadlessScreenshotUi::WorldCreate),
        "world-delete-confirm" | "world_delete_confirm" | "world-delete" | "world_delete" => {
            Ok(HeadlessScreenshotUi::WorldDeleteConfirm)
        }
        "new-world" | "new_world" => Ok(HeadlessScreenshotUi::NewWorld),
        "join-remote" | "join_remote" => Ok(HeadlessScreenshotUi::JoinRemote),
        "pause" => Ok(HeadlessScreenshotUi::Pause),
        "help" | "controls" => Ok(HeadlessScreenshotUi::Help),
        "block-palette" | "block_palette" | "palette" => Ok(HeadlessScreenshotUi::BlockPalette),
        "options-title" | "options_title" => Ok(HeadlessScreenshotUi::OptionsTitle),
        "options-pause" | "options_pause" | "options" => Ok(HeadlessScreenshotUi::OptionsPause),
        "options-graphics" | "options_graphics" => Ok(HeadlessScreenshotUi::OptionsGraphicsPause),
        "options-movement" | "options_movement" => Ok(HeadlessScreenshotUi::OptionsMovementPause),
        "options-display" | "options_display" => Ok(HeadlessScreenshotUi::OptionsDisplayPause),
        "options-debug" | "options_debug" => Ok(HeadlessScreenshotUi::OptionsDebugPause),
        "server-settings-pause" | "server_settings_pause" | "server-settings" => {
            Ok(HeadlessScreenshotUi::ServerSettingsPause)
        }
        _ => bail!(
            "{flag} must be none, title, world-list, world-create, world-delete-confirm, new-world, join-remote, pause, help/controls, block-palette, options-title, options-pause, options-graphics, options-movement, options-display, options-debug, or server-settings-pause, got `{value}`"
        ),
    }
}

fn desktop_render_distance_limits() -> RenderDistanceLimits {
    RenderDistanceLimits::new(
        u32::try_from(MIN_RENDER_DISTANCE)
            .expect("desktop minimum render distance is non-negative"),
        u32::try_from(MAX_RENDER_DISTANCE)
            .expect("desktop maximum render distance is non-negative"),
    )
}

fn print_help() {
    println!(
        "mclone-native-client\n\n\
         Usage:\n\
           mclone-native-client [--menu|--start-in-world true|false] [--startup-wait none|progress|playable|idle|frames:N] [--window-frame-report /tmp/mclone-window.json] [--window-frame-report-frames 3600] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--render-compile-capacity default|derived] [--render-compile-workers 1] [--render-compile-max-pending-jobs 4] [--world-root ./worlds] [--world-dir ./world|--transient] [--far-lod true|false] [--startup-lod-prewarm true|false] [--movement-speed-multiplier 1.0] [--simulation-cadence 20/20/60] [--adaptive-render-admission-budget true|false] [--debug-passive-showcase true|false] [--remote-addr 127.0.0.1:25565] [--section-occlusion true|false] [--lighting true|false] [--render-color-profile vanilla|stylized-bright|linear-experimental]\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\
           mclone-native-client --actor-review-sheet /tmp/mclone-actor-review.png [--width 1152] [--height 512] [--fullbright true|false]\n\
           mclone-native-client --actor-walk-review /tmp/mclone-actor-walk-review.png [--actor-walk-review-video /tmp/mclone-actor-walk-review.mp4] [--width 360] [--height 360] [--walk-review-frames 24] [--walk-review-fps 12] [--walk-review-cycles 2] [--fullbright true|false]\n\
          mclone-native-client --screenshot /tmp/mclone-frame.png [--width 1280] [--height 720] [--startup-wait none|progress|playable|idle|frames:N] [--screenshot-ui none|title|world-list|world-create|world-delete-confirm|new-world|join-remote|pause|help|controls|block-palette|options-title|options-pause|server-settings-pause] [--screenshot-hud true|false] [--screenshot-frame-pipeline-overlay true|false] [--screenshot-debug-pane true|false] [--screenshot-player-box true|false] [--screenshot-blink-debug true|false] [--screenshot-scripted-interaction true|false] [--screenshot-remote-settle-ms 0] [--screenshot-eye x,y,z] [--screenshot-target x,y,z] [--screenshot-camera-view first-person|third-person] [--first-person-player true|false] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--far-lod true|false] [--startup-lod-prewarm true|false] [--movement-speed-multiplier 1.0] [--simulation-cadence 20/20/60] [--debug-passive-showcase true|false] [--section-occlusion true|false] [--lighting true|false] [--fullbright true|false]\n\
           mclone-native-client --xr-emulation-screenshot /tmp/mclone-xr-emulation.png [--width 960] [--height 960] [--xr-emulation-key KeyW] [--xr-emulation-key ArrowLeft] [--xr-emulation-input-frames 8] [scene/render options as --screenshot]\n\
           mclone-native-client --torch-light-probe /tmp/mclone-torch-light-probe [--width 1280] [--height 720] [--render-color-profile vanilla|stylized-bright|linear-experimental]\n\
           mclone-native-client --headless-dual-view /tmp/mclone-dual-view [--headless-dual-view-hud true|false] [--width 960] [--height 640] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --renderer-rebuild-smoke /tmp/mclone-render-rebuild [--width 960] [--height 540] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--section-occlusion true|false] [--fullbright true|false] [--rebuild-render-scale 0.5]\n\
           mclone-native-client --remote-player-visual-smoke /tmp/mclone-remote-player-visual-smoke.png [--width 960] [--height 540] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--day-time 6000] [--freeze-time] [--lighting true|false] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --movement-perf [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--render-compile-workers 1] [--movement-steps 12] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --timedemo [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--timedemo-frames 120] [--path-radius 4] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --frame-budget-probe [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--render-compile-workers 1] [--render-compile-max-pending-jobs 4] [--frame-budget-frames 240] [--target-hz 120] [--path-radius 4] [--frame-accounting true|false] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --movement-frame-probe [--width 1280] [--height 720] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--render-compile-workers 1] [--frame-budget-frames 240] [--target-hz 120] [--path-radius 4] [--movement-frame-speed 32] [--frame-accounting true|false] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --startup-streaming-perf [--startup-streaming-persisted-world] [--freeze-scheduled-fluid-ticks] [--width 1280] [--height 720] [--seed 12345] [--render-distance 20] [--render-compile-capacity default|derived] [--render-compile-workers 1] [--render-compile-max-pending-jobs 4] [--render-compile-worker-timing true|false] [--startup-streaming-frames 2400] [--target-hz 120] [--simulation-cadence 20/20/60] [--adaptive-chunk-publication-budget true|false] [--debug-passive-showcase true|false] [--lighting true|false] [--section-occlusion true|false] [--fullbright true|false]\n\n\
           mclone-native-client --loading-settle-perf [--seed 12345] [--loading-settle-distances 5,10,15,20] [--render-compile-workers 1] [--simulation-cadence 20/20/60] [--debug-passive-showcase true|false] [--lighting true|false]\n\n\
           mclone-native-client --xr-clear-smoke [--frames 120|--xr-forever]\n\
           mclone-native-client --xr-mclone-smoke [--frames 120|--xr-forever] [--view-pose X,Y,Z,YAW_DEGREES] [--xr-underwater-mode midpoint|per-eye] [--xr-debug-ui none|pause|controls] [--seed 12345] [--chunk-x 0] [--chunk-z 0] [--render-distance 5] [--movement-speed-multiplier 1.0] [--day-time 6000] [--freeze-time] [--adaptive-chunk-publication-budget true|false] [--debug-passive-showcase true|false] [--section-occlusion true|false] [--fullbright true|false]\n\
           mclone-native-client --desktop-xr [--no-window] [--frames N|--xr-forever] [--view-pose X,Y,Z,YAW_DEGREES] [--xr-underwater-mode midpoint|per-eye] [--xr-debug-ui none|pause|controls] [scene/render options as --xr-mclone-smoke]\n\n\
        --desktop-xr is the real desktop OpenXR run verb: it renders the same mclone world as --xr-mclone-smoke but is persistent by default (unbounded frames; pass --frames N only to bound a run) and spawns a desktop companion window unless --no-window is passed. Quit the run by closing the companion window (a non-headset quit) or from the headset system menu; both shut the OpenXR session down gracefully. --xr-clear-smoke and --xr-mclone-smoke remain the frame-bounded CI/liveness gates.\n\
        Window mode streams chunks around a collision-backed local player with F1 controls, WASD walking, Space jump, Ctrl sprint, Shift crouch/sneak input, mouse-lock look, F5 camera view toggle, N no-clip debug toggle, X no-clip descend, mouse wheel no-clip speed, tilde debug pane and loading-progress toggle, O section-occlusion toggle, L fullbright toggle, F7 debug physics cube shot, F8 developer renderer-resource rebuild, and F9 developer render-scale rebuild cycle. Use --world-root to choose the menu-managed local world catalog directory. Use --world-dir to open a persistent SQLite-backed local world directory directly; with --transient, local worlds and the menu catalog use transient storage. Use --movement-speed-multiplier to scale local-player walking speed; no-clip fly speed remains a separate menu control. Use --first-person-player true to render the local player body in first-person while hiding head-authored figure parts. Use --startup-wait to select host startup readiness; desktop defaults to playable, screenshots default to idle, and frames:N adds offscreen warmup frames before saving the last capture. --xr-emulation-screenshot renders the shared stereo scene without initializing OpenXR; --width and --height are per-eye, and repeatable --xr-emulation-key physical codes feed the shared keyboard adapter into XR locomotion before capture. Use --window-frame-report to run the live winit/swapchain path for N rendered frames, write surface acquire/encode/submit/present timing JSON, then exit. Use --simulation-cadence HOST/GAMEPLAY/PHYSICS (alias --cadence) to pick a local integrated-server developer cadence such as 60/20/60; lower-rate lanes must divide the host rate, and higher-rate lanes must be integer substeps. The shared scheduler publication controller is the local-integrated default; use --adaptive-chunk-publication-budget false to force the fixed floor for comparison, and remote dedicated sessions keep it off. Use --adaptive-render-admission-budget true to test the shared render admission controller on the live local-integrated desktop path. Render compile in-flight capacity defaults to 4 jobs; use --render-compile-capacity derived to apply the shared host-derived worker/max-pending capacity before startup, or use --render-compile-workers and --render-compile-max-pending-jobs as manual overrides. Use --render-compile-worker-timing false only for meter-tax A/B perf captures; it disables render compile worker busy counters without changing queueing or compile work. Use --freeze-scheduled-fluid-ticks only in startup-streaming perf to isolate initial render-streaming from water/lava scheduled tick mutation. Use --debug-passive-showcase false to disable the default nearby passive-mob showcase for spawn-parity testing. Use --lighting false to bypass server-side ChunkStatus::Light promotion; lighting=false defaults to fullbright unless --fullbright false is also passed. Use --render-color-profile to select vanilla parity, stylized bright, or the reserved linear experimental lane. Headless modes write PNGs for GPU validation. Perf modes write JSON. Timedemo loads a static render distance large enough to contain its camera path. Frame-budget probe runs a deterministic offscreen streaming stress script. Movement-frame probe runs a speed-based offscreen walking script and counts work frames over an explicit target Hz budget. Startup-streaming perf runs the local startup pump to playable, then advances a paced desktop-shaped frame loop while the requested view streams in. With --startup-streaming-persisted-world it first prewarms a temp SQLite world, reopens it through the same startup pump, and measures already-generated persisted startup/streaming."
    );
}
