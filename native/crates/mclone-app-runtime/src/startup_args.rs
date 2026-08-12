use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_core::{AxisTopology, HorizontalTopology};
use mclone_frame_budget::RenderCompileCapacityReport;
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::RenderColorProfile;
use mclone_render_session::{
    ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
};
use mclone_server::{
    DEFAULT_LIGHT_STATUS_BATCH_SIZE, StarterContentDescriptor, WorldGenerationProfile,
};
use mclone_ui::GameMovementMode;

use crate::{
    DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS, DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
};

pub const ARG_SEED: &str = "--seed";
pub const ARG_GENERATION_PROFILE: &str = "--generation-profile";
pub const ARG_STARTER_CONTENT: &str = "--starter-content";
pub const ARG_WORLD_TOPOLOGY: &str = "--world-topology";
pub const ARG_ALPHA_WINTER: &str = "--alpha-winter";
pub const ARG_CHUNK_X: &str = "--chunk-x";
pub const ARG_CHUNK_Z: &str = "--chunk-z";
pub const ARG_RENDER_DISTANCE: &str = "--render-distance";
pub const ARG_RENDER_COMPILE_WORKERS: &str = "--render-compile-workers";
pub const ARG_RENDER_COMPILE_MAX_PENDING_JOBS: &str = "--render-compile-max-pending-jobs";
pub const ARG_RENDER_COMPILE_CAPACITY: &str = "--render-compile-capacity";
pub const ARG_REMOTE_ADDR: &str = "--remote-addr";
pub const ARG_WORLD_DIR: &str = "--world-dir";
pub const ARG_WORLD_ROOT: &str = "--world-root";
pub const ARG_TRANSIENT: &str = "--transient";
pub const ARG_DAY_TIME: &str = "--day-time";
pub const ARG_FREEZE_TIME: &str = "--freeze-time";
pub const ARG_MOVEMENT_SPEED_MULTIPLIER: &str = "--movement-speed-multiplier";
pub const ARG_MOVEMENT_MODE: &str = "--movement-mode";
pub const ARG_DEBUG_PASSIVE_SHOWCASE: &str = "--debug-passive-showcase";
pub const ARG_LIGHTING: &str = "--lighting";
pub const ARG_LIGHT_STATUS_BATCH_SIZE: &str = "--light-status-batch-size";
pub const ARG_SECTION_OCCLUSION: &str = "--section-occlusion";
pub const ARG_FULLBRIGHT: &str = "--fullbright";
pub const ARG_RENDER_COLOR_PROFILE: &str = "--render-color-profile";
pub const ARG_TERRAIN_PRESENTATION: &str = "--terrain-presentation";
pub const ARG_SCREENSHOT_EYE: &str = "--screenshot-eye";
pub const ARG_SCREENSHOT_TARGET: &str = "--screenshot-target";

// Shared startup flags are engine/session policy or scene-description inputs.
// App parsers should delegate these to `StartupArgState`; platform-local flags
// need an explicit local registry entry in the app crate.
pub const STARTUP_ARG_FLAGS: &[&str] = &[
    ARG_SEED,
    ARG_GENERATION_PROFILE,
    ARG_STARTER_CONTENT,
    ARG_WORLD_TOPOLOGY,
    ARG_ALPHA_WINTER,
    ARG_CHUNK_X,
    ARG_CHUNK_Z,
    ARG_RENDER_DISTANCE,
    ARG_RENDER_COMPILE_WORKERS,
    ARG_RENDER_COMPILE_MAX_PENDING_JOBS,
    ARG_RENDER_COMPILE_CAPACITY,
    ARG_REMOTE_ADDR,
    ARG_WORLD_DIR,
    ARG_WORLD_ROOT,
    ARG_TRANSIENT,
    ARG_DAY_TIME,
    ARG_FREEZE_TIME,
    ARG_MOVEMENT_SPEED_MULTIPLIER,
    ARG_MOVEMENT_MODE,
    ARG_DEBUG_PASSIVE_SHOWCASE,
    ARG_LIGHTING,
    ARG_LIGHT_STATUS_BATCH_SIZE,
    ARG_SECTION_OCCLUSION,
    ARG_FULLBRIGHT,
    ARG_RENDER_COLOR_PROFILE,
    ARG_TERRAIN_PRESENTATION,
    ARG_SCREENSHOT_EYE,
    ARG_SCREENSHOT_TARGET,
];

pub const QUERY_SEED: &str = "seed";
pub const QUERY_GENERATION_PROFILE: &str = "generationProfile";
pub const QUERY_STARTER_CONTENT: &str = "starterContent";
pub const QUERY_WORLD_TOPOLOGY: &str = "worldTopology";
pub const QUERY_ALPHA_WINTER: &str = "alphaWinter";
pub const QUERY_CHUNK_X: &str = "chunkX";
pub const QUERY_CHUNK_Z: &str = "chunkZ";
pub const QUERY_RENDER_DISTANCE: &str = "renderDistance";
pub const QUERY_RENDER_COMPILE_WORKERS: &str = "renderCompileWorkers";
pub const QUERY_RENDER_COMPILE_MAX_PENDING_JOBS: &str = "renderCompileMaxPendingJobs";
pub const QUERY_RENDER_COMPILE_CAPACITY: &str = "renderCompileCapacity";
pub const QUERY_REMOTE_WS_URL: &str = "remoteWsUrl";
pub const QUERY_DAY_TIME: &str = "dayTime";
pub const QUERY_FREEZE_TIME: &str = "freezeTime";
pub const QUERY_MOVEMENT_SPEED_MULTIPLIER: &str = "movementSpeedMultiplier";
pub const QUERY_MOVEMENT_MODE: &str = "movementMode";
pub const QUERY_DEBUG_PASSIVE_SHOWCASE: &str = "debugPassiveShowcase";
pub const QUERY_DEBUG_AUXILIARY_PLAYER_SCRIPT: &str = "debugAuxiliaryPlayerScript";
pub const QUERY_LIGHTING: &str = "lighting";
pub const QUERY_LIGHT_STATUS_BATCH_SIZE: &str = "lightStatusBatchSize";
pub const QUERY_SECTION_OCCLUSION: &str = "sectionOcclusion";
pub const QUERY_FULLBRIGHT: &str = "fullbright";
pub const QUERY_RENDER_COLOR_PROFILE: &str = "renderColorProfile";
pub const QUERY_TERRAIN_PRESENTATION: &str = "terrainPresentation";
pub const QUERY_SCREENSHOT_EYE: &str = "screenshotEye";
pub const QUERY_SCREENSHOT_TARGET: &str = "screenshotTarget";

pub const STARTUP_QUERY_KEYS: &[&str] = &[
    QUERY_SEED,
    QUERY_GENERATION_PROFILE,
    QUERY_STARTER_CONTENT,
    QUERY_WORLD_TOPOLOGY,
    QUERY_ALPHA_WINTER,
    QUERY_CHUNK_X,
    QUERY_CHUNK_Z,
    QUERY_RENDER_DISTANCE,
    QUERY_RENDER_COMPILE_WORKERS,
    QUERY_RENDER_COMPILE_MAX_PENDING_JOBS,
    QUERY_RENDER_COMPILE_CAPACITY,
    QUERY_REMOTE_WS_URL,
    QUERY_DAY_TIME,
    QUERY_FREEZE_TIME,
    QUERY_MOVEMENT_SPEED_MULTIPLIER,
    QUERY_MOVEMENT_MODE,
    QUERY_DEBUG_PASSIVE_SHOWCASE,
    QUERY_DEBUG_AUXILIARY_PLAYER_SCRIPT,
    QUERY_LIGHTING,
    QUERY_LIGHT_STATUS_BATCH_SIZE,
    QUERY_SECTION_OCCLUSION,
    QUERY_FULLBRIGHT,
    QUERY_RENDER_COLOR_PROFILE,
    QUERY_TERRAIN_PRESENTATION,
    QUERY_SCREENSHOT_EYE,
    QUERY_SCREENSHOT_TARGET,
];

pub const DEFAULT_STARTUP_SEED: i64 = 12_345;
pub const DEFAULT_STARTUP_CHUNK_X: i32 = 0;
pub const DEFAULT_STARTUP_CHUNK_Z: i32 = 0;
pub const DEFAULT_STARTUP_RENDER_DISTANCE: u32 = 5;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerrainPresentationMode {
    #[default]
    ExactOnly,
    Composed,
}

impl TerrainPresentationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExactOnly => "exact-only",
            Self::Composed => "composed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderDistanceLimits {
    pub min: u32,
    pub max: u32,
}

impl RenderDistanceLimits {
    pub const fn new(min: u32, max: u32) -> Self {
        Self { min, max }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartupSceneOptions {
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub starter_content: StarterContentDescriptor,
    pub world_topology: HorizontalTopology,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub render_distance: u32,
    pub render_compile_worker_count: usize,
    pub render_compile_max_pending_jobs: Option<usize>,
    pub render_compile_capacity_request: RenderCompileCapacityRequest,
    pub remote_addr: Option<String>,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub movement_speed_multiplier: f32,
    pub movement_mode: GameMovementMode,
    pub debug_passive_showcase: bool,
    pub debug_auxiliary_player_script: bool,
    pub lighting_enabled: bool,
    pub light_status_batch_size: usize,
    pub terrain_presentation: TerrainPresentationMode,
    pub terrain_presentation_explicit: bool,
}

impl Default for StartupSceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_STARTUP_SEED,
            world_generation_profile: WorldGenerationProfile::default(),
            starter_content: StarterContentDescriptor::Wild,
            world_topology: HorizontalTopology::UNBOUNDED,
            chunk_x: DEFAULT_STARTUP_CHUNK_X,
            chunk_z: DEFAULT_STARTUP_CHUNK_Z,
            render_distance: DEFAULT_STARTUP_RENDER_DISTANCE,
            render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            render_compile_max_pending_jobs: Some(DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS),
            render_compile_capacity_request: RenderCompileCapacityRequest::Default,
            remote_addr: None,
            day_time_override: None,
            freeze_time: false,
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            movement_mode: GameMovementMode::Walk,
            debug_passive_showcase: true,
            debug_auxiliary_player_script: false,
            lighting_enabled: true,
            light_status_batch_size: DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            terrain_presentation: TerrainPresentationMode::ExactOnly,
            terrain_presentation_explicit: false,
        }
    }
}

impl StartupSceneOptions {
    /// Applies the flat-client product default of starting at a fixed day time.
    ///
    /// Platform adapters should use named overlays like this instead of
    /// restating the complete shared startup schema.
    pub fn with_initial_time_frozen_at(mut self, day_time: u64) -> Self {
        self.day_time_override = Some(day_time);
        self.freeze_time = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartupOptions {
    pub scene: StartupSceneOptions,
    pub storage: StartupWorldStorageOptions,
    pub render_options: TexturedSectionRenderOptions,
    pub camera: StartupCameraOptions,
}

impl Default for StartupOptions {
    fn default() -> Self {
        Self {
            scene: StartupSceneOptions::default(),
            storage: StartupWorldStorageOptions::default(),
            render_options: TexturedSectionRenderOptions::default(),
            camera: StartupCameraOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StartupWorldStorageOptions {
    pub transient: bool,
    pub world_root: Option<PathBuf>,
    pub world_dir: Option<PathBuf>,
}

impl StartupWorldStorageOptions {
    pub fn default_world_root_enabled(&self) -> bool {
        !self.transient && self.world_root.is_none()
    }

    pub fn world_root_or_default(&self, default_root: Option<PathBuf>) -> Option<PathBuf> {
        if self.transient {
            None
        } else {
            self.world_root.clone().or(default_root)
        }
    }

    pub fn project(&self, default_world_root: Option<PathBuf>) -> StartupWorldStorageProjection {
        StartupWorldStorageProjection {
            default_world_root_enabled: self.default_world_root_enabled(),
            world_root: self.world_root_or_default(default_world_root),
            world_dir: self.world_dir.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupWorldStorageProjection {
    pub default_world_root_enabled: bool,
    pub world_root: Option<PathBuf>,
    pub world_dir: Option<PathBuf>,
}

impl StartupWorldStorageProjection {
    pub fn from_parts(
        default_world_root_enabled: bool,
        world_root: Option<PathBuf>,
        world_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            default_world_root_enabled,
            world_root,
            world_dir,
        }
    }

    pub fn with_default_world_root(mut self, default_world_root: Option<PathBuf>) -> Self {
        if self.default_world_root_enabled && self.world_root.is_none() {
            self.world_root = default_world_root;
        }
        self
    }

    pub fn validate_local_integrated_world(&self, remote_addr: Option<&str>) -> Result<()> {
        if self.world_dir.is_some() && remote_addr.is_some() {
            bail!("--world-dir applies only to local integrated worlds");
        }
        Ok(())
    }
}

pub fn format_optional_startup_path(value: Option<&Path>, none_label: &str) -> String {
    value
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| none_label.to_owned())
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StartupCameraOptions {
    pub eye: Option<[f32; 3]>,
    pub target: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartupArgState {
    scene: StartupSceneOptions,
    storage: StartupWorldStorageOptions,
    render_options: TexturedSectionRenderOptions,
    camera: StartupCameraOptions,
    fullbright_explicit: bool,
    render_compile_worker_count_explicit: bool,
    render_compile_max_pending_jobs_explicit: bool,
    alpha_winter_override: Option<bool>,
}

impl StartupArgState {
    pub fn new(scene: StartupSceneOptions, render_options: TexturedSectionRenderOptions) -> Self {
        Self {
            scene,
            storage: StartupWorldStorageOptions::default(),
            render_options,
            camera: StartupCameraOptions::default(),
            fullbright_explicit: false,
            render_compile_worker_count_explicit: false,
            render_compile_max_pending_jobs_explicit: false,
            alpha_winter_override: None,
        }
    }

    pub const fn scene(&self) -> &StartupSceneOptions {
        &self.scene
    }

    pub const fn render_compile_worker_count_explicit(&self) -> bool {
        self.render_compile_worker_count_explicit
    }

    pub const fn render_compile_max_pending_jobs_explicit(&self) -> bool {
        self.render_compile_max_pending_jobs_explicit
    }

    pub const fn has_manual_render_compile_capacity_fields(&self) -> bool {
        self.render_compile_worker_count_explicit || self.render_compile_max_pending_jobs_explicit
    }

    pub fn apply_render_compile_capacity_report(&mut self, report: &RenderCompileCapacityReport) {
        if self.scene.render_compile_capacity_request != RenderCompileCapacityRequest::Derived {
            return;
        }
        if !self.render_compile_worker_count_explicit {
            self.scene.render_compile_worker_count = report.derived_worker_count;
        }
        if !self.render_compile_max_pending_jobs_explicit {
            self.scene.render_compile_max_pending_jobs = Some(report.derived_max_pending_jobs);
        }
    }

    pub fn parse_next_arg(
        &mut self,
        arg: &str,
        args: &mut impl Iterator<Item = String>,
        render_distance_limits: RenderDistanceLimits,
    ) -> Result<bool> {
        match arg {
            ARG_SEED => {
                self.scene.seed = parse_i64_arg(ARG_SEED, args.next())?;
            }
            ARG_GENERATION_PROFILE => {
                self.scene.world_generation_profile = WorldGenerationProfile::parse_label(
                    &parse_string_arg(ARG_GENERATION_PROFILE, args.next())?,
                )
                .map_err(anyhow::Error::msg)?;
                self.apply_alpha_winter_override();
            }
            ARG_STARTER_CONTENT => {
                self.scene.starter_content = StarterContentDescriptor::parse_label(
                    &parse_string_arg(ARG_STARTER_CONTENT, args.next())?,
                )
                .map_err(anyhow::Error::msg)?;
            }
            ARG_WORLD_TOPOLOGY => {
                self.scene.world_topology = parse_world_topology_arg(
                    ARG_WORLD_TOPOLOGY,
                    &parse_string_arg(ARG_WORLD_TOPOLOGY, args.next())?,
                )?;
            }
            ARG_ALPHA_WINTER => {
                self.alpha_winter_override = Some(parse_bool_arg(ARG_ALPHA_WINTER, args.next())?);
                self.apply_alpha_winter_override();
            }
            ARG_CHUNK_X => {
                self.scene.chunk_x = parse_i32_arg(ARG_CHUNK_X, args.next())?;
            }
            ARG_CHUNK_Z => {
                self.scene.chunk_z = parse_i32_arg(ARG_CHUNK_Z, args.next())?;
            }
            ARG_RENDER_DISTANCE => {
                self.scene.render_distance = parse_render_distance_arg(
                    ARG_RENDER_DISTANCE,
                    args.next(),
                    render_distance_limits,
                )?;
            }
            ARG_RENDER_COMPILE_WORKERS => {
                self.scene.render_compile_worker_count =
                    parse_render_compile_worker_count_arg(ARG_RENDER_COMPILE_WORKERS, args.next())?;
                self.render_compile_worker_count_explicit = true;
            }
            ARG_RENDER_COMPILE_MAX_PENDING_JOBS => {
                self.scene.render_compile_max_pending_jobs =
                    Some(parse_render_compile_worker_count_arg(
                        ARG_RENDER_COMPILE_MAX_PENDING_JOBS,
                        args.next(),
                    )?);
                self.render_compile_max_pending_jobs_explicit = true;
            }
            ARG_RENDER_COMPILE_CAPACITY => {
                self.scene.render_compile_capacity_request =
                    parse_render_compile_capacity_arg(ARG_RENDER_COMPILE_CAPACITY, args.next())?;
            }
            ARG_REMOTE_ADDR => {
                self.scene.remote_addr = parse_remote_addr_arg(args.next())?;
            }
            ARG_WORLD_DIR => {
                if self.storage.transient {
                    bail!("{ARG_TRANSIENT} cannot be combined with {ARG_WORLD_DIR}");
                }
                if self.storage.world_root.is_some() {
                    bail!("{ARG_WORLD_DIR} cannot be combined with {ARG_WORLD_ROOT}");
                }
                self.storage.world_dir =
                    Some(PathBuf::from(parse_string_arg(ARG_WORLD_DIR, args.next())?));
            }
            ARG_WORLD_ROOT => {
                if self.storage.transient {
                    bail!("{ARG_TRANSIENT} cannot be combined with {ARG_WORLD_ROOT}");
                }
                if self.storage.world_dir.is_some() {
                    bail!("{ARG_WORLD_DIR} cannot be combined with {ARG_WORLD_ROOT}");
                }
                self.storage.world_root = Some(PathBuf::from(parse_string_arg(
                    ARG_WORLD_ROOT,
                    args.next(),
                )?));
            }
            ARG_TRANSIENT => {
                if self.storage.world_dir.is_some() {
                    bail!("{ARG_TRANSIENT} cannot be combined with {ARG_WORLD_DIR}");
                }
                if self.storage.world_root.is_some() {
                    bail!("{ARG_TRANSIENT} cannot be combined with {ARG_WORLD_ROOT}");
                }
                self.storage.transient = true;
            }
            ARG_DAY_TIME => {
                self.scene.day_time_override = Some(parse_u64_arg(ARG_DAY_TIME, args.next())?);
            }
            ARG_FREEZE_TIME => {
                self.scene.freeze_time = true;
            }
            ARG_MOVEMENT_SPEED_MULTIPLIER => {
                self.scene.movement_speed_multiplier = parse_movement_speed_multiplier_arg(
                    ARG_MOVEMENT_SPEED_MULTIPLIER,
                    args.next(),
                )?;
            }
            ARG_MOVEMENT_MODE => {
                self.scene.movement_mode = parse_movement_mode_arg(ARG_MOVEMENT_MODE, args.next())?;
            }
            ARG_DEBUG_PASSIVE_SHOWCASE => {
                self.scene.debug_passive_showcase =
                    parse_bool_arg(ARG_DEBUG_PASSIVE_SHOWCASE, args.next())?;
            }
            ARG_LIGHTING => {
                self.scene.lighting_enabled = parse_bool_arg(ARG_LIGHTING, args.next())?;
            }
            ARG_LIGHT_STATUS_BATCH_SIZE => {
                self.scene.light_status_batch_size =
                    parse_usize_arg(ARG_LIGHT_STATUS_BATCH_SIZE, args.next())?;
            }
            ARG_SECTION_OCCLUSION => {
                self.render_options.section_occlusion_culling =
                    parse_bool_arg(ARG_SECTION_OCCLUSION, args.next())?;
            }
            ARG_FULLBRIGHT => {
                self.render_options.force_fullbright = parse_bool_arg(ARG_FULLBRIGHT, args.next())?;
                self.fullbright_explicit = true;
            }
            ARG_RENDER_COLOR_PROFILE => {
                self.render_options.color_profile =
                    parse_render_color_profile_arg(ARG_RENDER_COLOR_PROFILE, args.next())?;
            }
            ARG_TERRAIN_PRESENTATION => {
                self.scene.terrain_presentation =
                    parse_terrain_presentation_mode(ARG_TERRAIN_PRESENTATION, args.next())?;
                self.scene.terrain_presentation_explicit = true;
            }
            ARG_SCREENSHOT_EYE => {
                self.camera.eye = Some(parse_f32_vec3_arg(ARG_SCREENSHOT_EYE, args.next())?);
            }
            ARG_SCREENSHOT_TARGET => {
                self.camera.target = Some(parse_f32_vec3_arg(ARG_SCREENSHOT_TARGET, args.next())?);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn parse_query_param(
        &mut self,
        key: &str,
        value: Option<String>,
        render_distance_limits: RenderDistanceLimits,
    ) -> Result<bool> {
        match key {
            QUERY_SEED => {
                self.scene.seed = parse_i64_arg(QUERY_SEED, value)?;
            }
            QUERY_GENERATION_PROFILE => {
                self.scene.world_generation_profile = WorldGenerationProfile::parse_label(
                    &parse_string_arg(QUERY_GENERATION_PROFILE, value)?,
                )
                .map_err(anyhow::Error::msg)?;
                self.apply_alpha_winter_override();
            }
            QUERY_STARTER_CONTENT => {
                self.scene.starter_content = StarterContentDescriptor::parse_label(
                    &parse_string_arg(QUERY_STARTER_CONTENT, value)?,
                )
                .map_err(anyhow::Error::msg)?;
            }
            QUERY_WORLD_TOPOLOGY => {
                self.scene.world_topology = parse_world_topology_arg(
                    QUERY_WORLD_TOPOLOGY,
                    &parse_string_arg(QUERY_WORLD_TOPOLOGY, value)?,
                )?;
            }
            QUERY_ALPHA_WINTER => {
                self.alpha_winter_override =
                    Some(parse_query_presence_bool(QUERY_ALPHA_WINTER, value)?);
                self.apply_alpha_winter_override();
            }
            QUERY_CHUNK_X => {
                self.scene.chunk_x = parse_i32_arg(QUERY_CHUNK_X, value)?;
            }
            QUERY_CHUNK_Z => {
                self.scene.chunk_z = parse_i32_arg(QUERY_CHUNK_Z, value)?;
            }
            QUERY_RENDER_DISTANCE => {
                self.scene.render_distance = parse_render_distance_arg(
                    QUERY_RENDER_DISTANCE,
                    value,
                    render_distance_limits,
                )?;
            }
            QUERY_RENDER_COMPILE_WORKERS => {
                self.scene.render_compile_worker_count =
                    parse_render_compile_worker_count_arg(QUERY_RENDER_COMPILE_WORKERS, value)?;
                self.render_compile_worker_count_explicit = true;
            }
            QUERY_RENDER_COMPILE_MAX_PENDING_JOBS => {
                self.scene.render_compile_max_pending_jobs =
                    Some(parse_render_compile_worker_count_arg(
                        QUERY_RENDER_COMPILE_MAX_PENDING_JOBS,
                        value,
                    )?);
                self.render_compile_max_pending_jobs_explicit = true;
            }
            QUERY_RENDER_COMPILE_CAPACITY => {
                self.scene.render_compile_capacity_request =
                    parse_render_compile_capacity_arg(QUERY_RENDER_COMPILE_CAPACITY, value)?;
            }
            QUERY_REMOTE_WS_URL => {
                self.scene.remote_addr = parse_remote_addr_value(QUERY_REMOTE_WS_URL, value)?;
            }
            QUERY_DAY_TIME => {
                self.scene.day_time_override = Some(parse_u64_arg(QUERY_DAY_TIME, value)?);
            }
            QUERY_FREEZE_TIME => {
                self.scene.freeze_time = parse_query_presence_bool(QUERY_FREEZE_TIME, value)?;
            }
            QUERY_MOVEMENT_SPEED_MULTIPLIER => {
                self.scene.movement_speed_multiplier =
                    parse_movement_speed_multiplier_arg(QUERY_MOVEMENT_SPEED_MULTIPLIER, value)?;
            }
            QUERY_MOVEMENT_MODE => {
                self.scene.movement_mode = parse_movement_mode_arg(QUERY_MOVEMENT_MODE, value)?;
            }
            QUERY_DEBUG_PASSIVE_SHOWCASE => {
                self.scene.debug_passive_showcase =
                    parse_bool_arg(QUERY_DEBUG_PASSIVE_SHOWCASE, value)?;
            }
            QUERY_DEBUG_AUXILIARY_PLAYER_SCRIPT => {
                self.scene.debug_auxiliary_player_script =
                    parse_bool_arg(QUERY_DEBUG_AUXILIARY_PLAYER_SCRIPT, value)?;
            }
            QUERY_LIGHTING => {
                self.scene.lighting_enabled = parse_bool_arg(QUERY_LIGHTING, value)?;
            }
            QUERY_LIGHT_STATUS_BATCH_SIZE => {
                self.scene.light_status_batch_size =
                    parse_usize_arg(QUERY_LIGHT_STATUS_BATCH_SIZE, value)?;
            }
            QUERY_SECTION_OCCLUSION => {
                self.render_options.section_occlusion_culling =
                    parse_bool_arg(QUERY_SECTION_OCCLUSION, value)?;
            }
            QUERY_FULLBRIGHT => {
                self.render_options.force_fullbright = parse_bool_arg(QUERY_FULLBRIGHT, value)?;
                self.fullbright_explicit = true;
            }
            QUERY_RENDER_COLOR_PROFILE => {
                self.render_options.color_profile =
                    parse_render_color_profile_arg(QUERY_RENDER_COLOR_PROFILE, value)?;
            }
            QUERY_TERRAIN_PRESENTATION => {
                self.scene.terrain_presentation =
                    parse_terrain_presentation_mode(QUERY_TERRAIN_PRESENTATION, value)?;
                self.scene.terrain_presentation_explicit = true;
            }
            QUERY_SCREENSHOT_EYE => {
                self.camera.eye = Some(parse_f32_vec3_arg(QUERY_SCREENSHOT_EYE, value)?);
            }
            QUERY_SCREENSHOT_TARGET => {
                self.camera.target = Some(parse_f32_vec3_arg(QUERY_SCREENSHOT_TARGET, value)?);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn finish(mut self) -> StartupOptions {
        self.apply_alpha_winter_override();
        if !self.scene.lighting_enabled && !self.fullbright_explicit {
            self.render_options.force_fullbright = true;
        }
        StartupOptions {
            scene: self.scene,
            storage: self.storage,
            render_options: self.render_options,
            camera: self.camera,
        }
    }

    fn apply_alpha_winter_override(&mut self) {
        if let (Some(winter), WorldGenerationProfile::AlphaV1 { .. }) = (
            self.alpha_winter_override,
            self.scene.world_generation_profile,
        ) {
            self.scene.world_generation_profile = WorldGenerationProfile::alpha_v1(winter);
        }
    }
}

impl Default for StartupArgState {
    fn default() -> Self {
        Self::new(
            StartupSceneOptions::default(),
            TexturedSectionRenderOptions::default(),
        )
    }
}

pub fn parse_string_arg(flag: &str, value: Option<String>) -> Result<String> {
    value.with_context(|| format!("{flag} requires a value"))
}

pub fn parse_world_topology_arg(flag: &str, value: &str) -> Result<HorizontalTopology> {
    let value = value.trim();
    if matches!(value, "plane" | "unbounded" | "euclidean") {
        return Ok(HorizontalTopology::UNBOUNDED);
    }
    let period = if value == "cylinder-x" {
        32
    } else if let Some(period) = value.strip_prefix("cylinder-x:") {
        period
            .parse::<u32>()
            .with_context(|| format!("invalid X-cylinder period `{period}` for {flag}"))?
    } else {
        bail!("{flag} must be plane, cylinder-x, or cylinder-x:<period-chunks>, got `{value}`");
    };
    let topology =
        HorizontalTopology::new(AxisTopology::periodic(0, period), AxisTopology::Unbounded);
    topology
        .validate()
        .map_err(|error| anyhow::anyhow!("invalid {flag}: {error}"))?;
    Ok(topology)
}

pub fn parse_u32_arg(flag: &str, value: Option<String>) -> Result<u32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if parsed == 0 {
        bail!("{flag} must be greater than zero");
    }
    Ok(parsed)
}

pub fn parse_i32_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i32>()
        .with_context(|| format!("{flag} requires a signed integer, got `{value}`"))
}

pub fn parse_i64_arg(flag: &str, value: Option<String>) -> Result<i64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i64>()
        .with_context(|| format!("{flag} requires a signed 64-bit integer, got `{value}`"))
}

pub fn parse_u64_arg(flag: &str, value: Option<String>) -> Result<u64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<u64>()
        .with_context(|| format!("{flag} requires an unsigned 64-bit integer, got `{value}`"))
}

pub fn parse_f32_arg(flag: &str, value: Option<String>) -> Result<f32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<f32>()
        .with_context(|| format!("{flag} requires a number, got `{value}`"))
}

pub fn parse_bool_arg(flag: &str, value: Option<String>) -> Result<bool> {
    let value = value.with_context(|| format!("{flag} requires true or false"))?;
    match value.as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("{flag} must be true or false, got `{value}`"),
    }
}

pub fn parse_render_color_profile_arg(
    flag: &str,
    value: Option<String>,
) -> Result<RenderColorProfile> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<RenderColorProfile>()
        .map_err(|message| anyhow::anyhow!("{flag} {message}"))
}

pub fn parse_terrain_presentation_mode(
    flag: &str,
    value: Option<String>,
) -> Result<TerrainPresentationMode> {
    let value = value.with_context(|| format!("{flag} requires exact-only or composed"))?;
    match value.trim().to_ascii_lowercase().as_str() {
        "exact-only" | "exact" | "off" | "false" => Ok(TerrainPresentationMode::ExactOnly),
        "composed" | "horizon" | "on" | "true" => Ok(TerrainPresentationMode::Composed),
        _ => bail!("{flag} must be exact-only or composed, got `{value}`"),
    }
}

pub fn parse_f32_vec3_arg(flag: &str, value: Option<String>) -> Result<[f32; 3]> {
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

pub fn parse_render_distance_arg(
    flag: &str,
    value: Option<String>,
    limits: RenderDistanceLimits,
) -> Result<u32> {
    let parsed = parse_u32_arg(flag, value)?;
    if parsed < limits.min || parsed > limits.max {
        bail!("{flag} must be between {} and {}", limits.min, limits.max);
    }
    Ok(parsed)
}

pub fn parse_render_compile_worker_count_arg(flag: &str, value: Option<String>) -> Result<usize> {
    parse_usize_arg(flag, value)
}

pub fn parse_usize_arg(flag: &str, value: Option<String>) -> Result<usize> {
    let parsed = parse_u32_arg(flag, value)?;
    usize::try_from(parsed).with_context(|| format!("{flag} value does not fit usize"))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderCompileCapacityRequest {
    #[default]
    Default,
    Derived,
}

pub fn parse_render_compile_capacity_arg(
    flag: &str,
    value: Option<String>,
) -> Result<RenderCompileCapacityRequest> {
    let value = value.with_context(|| format!("{flag} requires default or derived"))?;
    match value.as_str() {
        "default" | "off" | "false" => Ok(RenderCompileCapacityRequest::Default),
        "derived" | "auto" | "on" | "true" => Ok(RenderCompileCapacityRequest::Derived),
        _ => bail!("{flag} must be default or derived, got `{value}`"),
    }
}

pub fn parse_movement_speed_multiplier_arg(flag: &str, value: Option<String>) -> Result<f32> {
    let parsed = parse_f32_arg(flag, value)?;
    let min = ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32;
    let max = ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32;
    if !parsed.is_finite() || !(min..=max).contains(&parsed) {
        bail!("{flag} must be between {min} and {max}");
    }
    Ok(parsed)
}

pub fn parse_movement_mode_arg(flag: &str, value: Option<String>) -> Result<GameMovementMode> {
    let value = parse_string_arg(flag, value)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "walk" | "walking" | "player" => Ok(GameMovementMode::Walk),
        "fly" | "flying" => Ok(GameMovementMode::Fly),
        "hand-push" | "hand_push" | "handpush" | "gorilla" => Ok(GameMovementMode::HandPush),
        "thruster" | "thrust" => Ok(GameMovementMode::Thruster),
        _ => bail!("{flag} must be walk, fly, hand-push, or thruster, got `{value}`"),
    }
}

fn parse_remote_addr_arg(value: Option<String>) -> Result<Option<String>> {
    parse_remote_addr_value(ARG_REMOTE_ADDR, value)
}

fn parse_remote_addr_value(label: &str, value: Option<String>) -> Result<Option<String>> {
    let value = parse_string_arg(label, value)?;
    let value = value.trim();
    if value.is_empty() || matches!(value, "default" | "off" | "none" | "false" | "0") {
        Ok(None)
    } else {
        Ok(Some(value.to_owned()))
    }
}

fn parse_query_presence_bool(key: &str, value: Option<String>) -> Result<bool> {
    let value = value.unwrap_or_default();
    if value.trim().is_empty() {
        Ok(true)
    } else {
        parse_bool_arg(key, Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> StartupOptions {
        let mut state = StartupArgState::default();
        let mut args = args.iter().map(|arg| (*arg).to_owned());
        while let Some(arg) = args.next() {
            assert!(
                state
                    .parse_next_arg(&arg, &mut args, RenderDistanceLimits::new(1, 16))
                    .unwrap(),
                "unexpected arg `{arg}`"
            );
        }
        state.finish()
    }

    #[test]
    fn defaults_are_host_neutral() {
        assert_eq!(
            StartupSceneOptions::default(),
            StartupSceneOptions {
                seed: 12_345,
                world_generation_profile: WorldGenerationProfile::Overworld,
                starter_content: StarterContentDescriptor::Wild,
                world_topology: HorizontalTopology::UNBOUNDED,
                chunk_x: 0,
                chunk_z: 0,
                render_distance: 5,
                render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
                render_compile_max_pending_jobs: Some(
                    DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS,
                ),
                render_compile_capacity_request: RenderCompileCapacityRequest::Default,
                remote_addr: None,
                day_time_override: None,
                freeze_time: false,
                movement_speed_multiplier: 1.0,
                movement_mode: GameMovementMode::Walk,
                debug_passive_showcase: true,
                debug_auxiliary_player_script: false,
                lighting_enabled: true,
                light_status_batch_size: DEFAULT_LIGHT_STATUS_BATCH_SIZE,
                terrain_presentation: TerrainPresentationMode::ExactOnly,
                terrain_presentation_explicit: false,
            }
        );
        assert_eq!(
            StartupArgState::default().finish().storage,
            StartupWorldStorageOptions::default()
        );
        assert_eq!(
            StartupArgState::default().finish().render_options,
            TexturedSectionRenderOptions::default()
        );
        assert_eq!(
            StartupArgState::default().finish().camera,
            StartupCameraOptions::default()
        );
    }

    #[test]
    fn named_time_overlay_changes_only_time_policy() {
        let baseline = StartupSceneOptions::default();
        let configured = baseline.clone().with_initial_time_frozen_at(6000);

        assert_eq!(configured.day_time_override, Some(6000));
        assert!(configured.freeze_time);
        assert_eq!(
            StartupSceneOptions {
                day_time_override: None,
                freeze_time: false,
                ..configured
            },
            baseline
        );
    }

    #[test]
    fn argv_and_query_sources_produce_equivalent_shared_configuration() {
        let argv = parse(&[
            ARG_SEED,
            "-77",
            ARG_GENERATION_PROFILE,
            "authored-only",
            ARG_STARTER_CONTENT,
            "intro-homestead-v1",
            ARG_WORLD_TOPOLOGY,
            "cylinder-x:32",
            ARG_RENDER_DISTANCE,
            "6",
            ARG_REMOTE_ADDR,
            "example.test:25565",
            ARG_DAY_TIME,
            "6000",
            ARG_FREEZE_TIME,
            ARG_MOVEMENT_SPEED_MULTIPLIER,
            "1.75",
            ARG_MOVEMENT_MODE,
            "thruster",
            ARG_LIGHTING,
            "false",
        ]);
        let mut query = StartupArgState::default();
        for (key, value) in [
            (QUERY_SEED, "-77"),
            (QUERY_GENERATION_PROFILE, "authored-only"),
            (QUERY_STARTER_CONTENT, "intro-homestead-v1"),
            (QUERY_WORLD_TOPOLOGY, "cylinder-x:32"),
            (QUERY_RENDER_DISTANCE, "6"),
            (QUERY_REMOTE_WS_URL, "example.test:25565"),
            (QUERY_DAY_TIME, "6000"),
            (QUERY_FREEZE_TIME, ""),
            (QUERY_MOVEMENT_SPEED_MULTIPLIER, "1.75"),
            (QUERY_MOVEMENT_MODE, "thruster"),
            (QUERY_LIGHTING, "false"),
        ] {
            assert!(
                query
                    .parse_query_param(
                        key,
                        Some(value.to_owned()),
                        RenderDistanceLimits::new(1, 16),
                    )
                    .unwrap()
            );
        }

        assert_eq!(query.finish(), argv);
    }

    #[test]
    fn parses_scene_tokens() {
        let options = parse(&[
            ARG_SEED,
            "-77",
            ARG_GENERATION_PROFILE,
            "authored-only",
            ARG_STARTER_CONTENT,
            "intro-homestead-v1",
            ARG_WORLD_TOPOLOGY,
            "cylinder-x:32",
            ARG_CHUNK_X,
            "4",
            ARG_CHUNK_Z,
            "-3",
            ARG_RENDER_DISTANCE,
            "5",
            ARG_RENDER_COMPILE_WORKERS,
            "2",
            ARG_RENDER_COMPILE_MAX_PENDING_JOBS,
            "6",
            ARG_RENDER_COMPILE_CAPACITY,
            "derived",
            ARG_DAY_TIME,
            "6000",
            ARG_FREEZE_TIME,
            ARG_MOVEMENT_SPEED_MULTIPLIER,
            "2.5",
            ARG_MOVEMENT_MODE,
            "fly",
            ARG_DEBUG_PASSIVE_SHOWCASE,
            "false",
            ARG_LIGHT_STATUS_BATCH_SIZE,
            "5",
            ARG_REMOTE_ADDR,
            "127.0.0.1:25565",
            ARG_SCREENSHOT_EYE,
            "1.5,62.25,-3",
            ARG_SCREENSHOT_TARGET,
            "8,64,8",
        ]);
        assert_eq!(
            options.scene,
            StartupSceneOptions {
                seed: -77,
                world_generation_profile: WorldGenerationProfile::authored_only(),
                starter_content: StarterContentDescriptor::IntroHomesteadV1,
                world_topology: HorizontalTopology::new(
                    AxisTopology::periodic(0, 32),
                    AxisTopology::Unbounded,
                ),
                chunk_x: 4,
                chunk_z: -3,
                render_distance: 5,
                render_compile_worker_count: 2,
                render_compile_max_pending_jobs: Some(6),
                render_compile_capacity_request: RenderCompileCapacityRequest::Derived,
                remote_addr: Some("127.0.0.1:25565".to_owned()),
                day_time_override: Some(6000),
                freeze_time: true,
                movement_speed_multiplier: 2.5,
                movement_mode: GameMovementMode::Fly,
                debug_passive_showcase: false,
                debug_auxiliary_player_script: false,
                lighting_enabled: true,
                light_status_batch_size: 5,
                terrain_presentation: TerrainPresentationMode::ExactOnly,
                terrain_presentation_explicit: false,
            }
        );
        assert_eq!(
            options.camera,
            StartupCameraOptions {
                eye: Some([1.5, 62.25, -3.0]),
                target: Some([8.0, 64.0, 8.0]),
            }
        );
    }

    #[test]
    fn alpha_winter_is_an_order_independent_explicit_override() {
        for args in [
            [ARG_GENERATION_PROFILE, "alpha-v1", ARG_ALPHA_WINTER, "true"],
            [ARG_ALPHA_WINTER, "true", ARG_GENERATION_PROFILE, "alpha-v1"],
        ] {
            assert_eq!(
                parse(&args).scene.world_generation_profile,
                WorldGenerationProfile::alpha_v1(true)
            );
        }

        let mut query = StartupArgState::default();
        query
            .parse_query_param(QUERY_ALPHA_WINTER, None, RenderDistanceLimits::new(1, 16))
            .unwrap();
        query
            .parse_query_param(
                QUERY_GENERATION_PROFILE,
                Some("alpha-v1".to_owned()),
                RenderDistanceLimits::new(1, 16),
            )
            .unwrap();
        assert_eq!(
            query.finish().scene.world_generation_profile,
            WorldGenerationProfile::alpha_v1(true)
        );
    }

    #[test]
    fn beta_profile_parses_from_native_and_browser_startup_inputs() {
        assert_eq!(
            parse(&[ARG_GENERATION_PROFILE, "beta-v1"])
                .scene
                .world_generation_profile,
            WorldGenerationProfile::BetaV1
        );

        let mut query = StartupArgState::default();
        query
            .parse_query_param(
                QUERY_GENERATION_PROFILE,
                Some("beta-v1".to_owned()),
                RenderDistanceLimits::new(1, 16),
            )
            .unwrap();
        assert_eq!(
            query.finish().scene.world_generation_profile,
            WorldGenerationProfile::BetaV1
        );
    }

    #[test]
    fn parses_render_tokens_and_lighting_fullbright_default() {
        let options = parse(&[
            ARG_LIGHTING,
            "false",
            ARG_SECTION_OCCLUSION,
            "false",
            ARG_FULLBRIGHT,
            "true",
            ARG_RENDER_COLOR_PROFILE,
            "stylized-bright",
        ]);
        assert!(!options.scene.lighting_enabled);
        assert!(!options.render_options.section_occlusion_culling);
        assert!(options.render_options.force_fullbright);
        assert_eq!(
            options.render_options.color_profile,
            RenderColorProfile::StylizedBright
        );

        let options = parse(&[ARG_LIGHTING, "false"]);
        assert!(options.render_options.force_fullbright);

        let options = parse(&[ARG_LIGHTING, "false", ARG_FULLBRIGHT, "false"]);
        assert!(!options.render_options.force_fullbright);
    }

    #[test]
    fn movement_mode_accepts_product_labels_and_aliases() {
        for (value, expected) in [
            ("walk", GameMovementMode::Walk),
            ("fly", GameMovementMode::Fly),
            ("hand-push", GameMovementMode::HandPush),
            ("gorilla", GameMovementMode::HandPush),
            ("thruster", GameMovementMode::Thruster),
        ] {
            assert_eq!(
                parse_movement_mode_arg(ARG_MOVEMENT_MODE, Some(value.to_owned())).unwrap(),
                expected
            );
        }
        assert!(
            parse_movement_mode_arg(ARG_MOVEMENT_MODE, Some("spectator".to_owned()))
                .unwrap_err()
                .to_string()
                .contains("walk, fly, hand-push, or thruster")
        );
    }

    #[test]
    fn parses_world_storage_tokens() {
        let options = parse(&[ARG_WORLD_DIR, "/tmp/mclone-world"]);
        assert_eq!(
            options.storage,
            StartupWorldStorageOptions {
                transient: false,
                world_root: None,
                world_dir: Some(PathBuf::from("/tmp/mclone-world")),
            }
        );

        let options = parse(&[ARG_WORLD_ROOT, "/tmp/mclone-worlds"]);
        assert_eq!(
            options.storage,
            StartupWorldStorageOptions {
                transient: false,
                world_root: Some(PathBuf::from("/tmp/mclone-worlds")),
                world_dir: None,
            }
        );

        let options = parse(&[ARG_TRANSIENT]);
        assert_eq!(
            options.storage,
            StartupWorldStorageOptions {
                transient: true,
                world_root: None,
                world_dir: None,
            }
        );
        assert_eq!(options.storage.world_root_or_default(None), None);
        assert_eq!(
            StartupWorldStorageOptions::default()
                .world_root_or_default(Some(PathBuf::from("/tmp/default-worlds"))),
            Some(PathBuf::from("/tmp/default-worlds"))
        );
    }

    #[test]
    fn startup_world_storage_projection_applies_default_root_policy() {
        let projection = StartupWorldStorageOptions::default()
            .project(Some(PathBuf::from("/tmp/default-worlds")));
        assert!(projection.default_world_root_enabled);
        assert_eq!(
            projection.world_root,
            Some(PathBuf::from("/tmp/default-worlds"))
        );
        assert_eq!(projection.world_dir, None);

        let transient = StartupWorldStorageOptions {
            transient: true,
            world_root: None,
            world_dir: None,
        }
        .project(Some(PathBuf::from("/tmp/default-worlds")));
        assert!(!transient.default_world_root_enabled);
        assert_eq!(transient.world_root, None);

        let explicit = StartupWorldStorageOptions {
            transient: false,
            world_root: Some(PathBuf::from("/tmp/explicit-worlds")),
            world_dir: None,
        }
        .project(Some(PathBuf::from("/tmp/default-worlds")));
        assert!(!explicit.default_world_root_enabled);
        assert_eq!(
            explicit.world_root,
            Some(PathBuf::from("/tmp/explicit-worlds"))
        );
    }

    #[test]
    fn startup_world_storage_projection_can_apply_late_platform_default_root() {
        let projection = StartupWorldStorageOptions::default()
            .project(None)
            .with_default_world_root(Some(PathBuf::from("/tmp/android-worlds")));

        assert!(projection.default_world_root_enabled);
        assert_eq!(
            projection.world_root,
            Some(PathBuf::from("/tmp/android-worlds"))
        );
    }

    #[test]
    fn startup_world_storage_projection_rejects_world_dir_for_remote_sessions() {
        let projection = StartupWorldStorageProjection::from_parts(
            true,
            None,
            Some(PathBuf::from("/tmp/mclone-world")),
        );

        assert!(projection.validate_local_integrated_world(None).is_ok());
        let error = projection
            .validate_local_integrated_world(Some("127.0.0.1:25565"))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "--world-dir applies only to local integrated worlds"
        );
    }

    #[test]
    fn format_optional_startup_path_uses_caller_none_label() {
        assert_eq!(
            format_optional_startup_path(Some(Path::new("/tmp/mclone-world")), "<none>"),
            "/tmp/mclone-world"
        );
        assert_eq!(format_optional_startup_path(None, "<none>"), "<none>");
        assert_eq!(format_optional_startup_path(None, "none"), "none");
    }

    #[test]
    fn rejects_conflicting_world_storage_tokens() {
        for args in [
            vec![ARG_TRANSIENT, ARG_WORLD_DIR, "/tmp/mclone-world"],
            vec![ARG_WORLD_DIR, "/tmp/mclone-world", ARG_TRANSIENT],
            vec![ARG_TRANSIENT, ARG_WORLD_ROOT, "/tmp/mclone-worlds"],
            vec![ARG_WORLD_ROOT, "/tmp/mclone-worlds", ARG_TRANSIENT],
            vec![
                ARG_WORLD_DIR,
                "/tmp/mclone-world",
                ARG_WORLD_ROOT,
                "/tmp/mclone-worlds",
            ],
            vec![
                ARG_WORLD_ROOT,
                "/tmp/mclone-worlds",
                ARG_WORLD_DIR,
                "/tmp/mclone-world",
            ],
        ] {
            let mut state = StartupArgState::default();
            let mut args = args.into_iter().map(str::to_owned);
            let err = loop {
                let Some(arg) = args.next() else {
                    panic!("expected conflicting storage args to fail");
                };
                match state.parse_next_arg(&arg, &mut args, RenderDistanceLimits::new(1, 16)) {
                    Ok(true) => continue,
                    Ok(false) => panic!("unexpected arg `{arg}`"),
                    Err(error) => break error,
                }
            };
            assert!(
                err.to_string().contains("cannot be combined"),
                "unexpected error: {err:#}"
            );
        }
    }

    #[test]
    fn parses_query_params() {
        let mut state = StartupArgState::default();
        for (key, value) in [
            (QUERY_SEED, "-77"),
            (QUERY_GENERATION_PROFILE, "authored-only"),
            (QUERY_STARTER_CONTENT, "intro-homestead-v1"),
            (QUERY_CHUNK_X, "4"),
            (QUERY_CHUNK_Z, "-3"),
            (QUERY_RENDER_DISTANCE, "6"),
            (QUERY_RENDER_COMPILE_WORKERS, "3"),
            (QUERY_RENDER_COMPILE_MAX_PENDING_JOBS, "5"),
            (QUERY_RENDER_COMPILE_CAPACITY, "derived"),
            (QUERY_WORLD_TOPOLOGY, "cylinder-x"),
            (QUERY_REMOTE_WS_URL, "ws://127.0.0.1:25565"),
            (QUERY_DAY_TIME, "6000"),
            (QUERY_FREEZE_TIME, ""),
            (QUERY_MOVEMENT_SPEED_MULTIPLIER, "0.5"),
            (QUERY_DEBUG_PASSIVE_SHOWCASE, "false"),
            (QUERY_DEBUG_AUXILIARY_PLAYER_SCRIPT, "true"),
            (QUERY_LIGHTING, "false"),
            (QUERY_LIGHT_STATUS_BATCH_SIZE, "5"),
            (QUERY_SECTION_OCCLUSION, "false"),
            (QUERY_FULLBRIGHT, "true"),
            (QUERY_RENDER_COLOR_PROFILE, "stylized-bright"),
            (QUERY_SCREENSHOT_EYE, "1.5,62.25,-3"),
            (QUERY_SCREENSHOT_TARGET, "8,64,8"),
        ] {
            assert!(
                state
                    .parse_query_param(
                        key,
                        Some(value.to_owned()),
                        RenderDistanceLimits::new(1, 16)
                    )
                    .unwrap(),
                "unexpected query key `{key}`"
            );
        }

        let options = state.finish();
        assert_eq!(
            options.scene,
            StartupSceneOptions {
                seed: -77,
                world_generation_profile: WorldGenerationProfile::authored_only(),
                starter_content: StarterContentDescriptor::IntroHomesteadV1,
                world_topology: HorizontalTopology::new(
                    AxisTopology::periodic(0, 32),
                    AxisTopology::Unbounded,
                ),
                chunk_x: 4,
                chunk_z: -3,
                render_distance: 6,
                render_compile_worker_count: 3,
                render_compile_max_pending_jobs: Some(5),
                render_compile_capacity_request: RenderCompileCapacityRequest::Derived,
                remote_addr: Some("ws://127.0.0.1:25565".to_owned()),
                day_time_override: Some(6000),
                freeze_time: true,
                movement_speed_multiplier: 0.5,
                movement_mode: GameMovementMode::Walk,
                debug_passive_showcase: false,
                debug_auxiliary_player_script: true,
                lighting_enabled: false,
                light_status_batch_size: 5,
                terrain_presentation: TerrainPresentationMode::ExactOnly,
                terrain_presentation_explicit: false,
            }
        );
        assert!(!options.render_options.section_occlusion_culling);
        assert!(options.render_options.force_fullbright);
        assert_eq!(
            options.render_options.color_profile,
            RenderColorProfile::StylizedBright
        );
        assert_eq!(
            options.camera,
            StartupCameraOptions {
                eye: Some([1.5, 62.25, -3.0]),
                target: Some([8.0, 64.0, 8.0]),
            }
        );
    }

    #[test]
    fn parses_query_freeze_time_false() {
        let mut state = StartupArgState::default();
        state
            .parse_query_param(
                QUERY_FREEZE_TIME,
                Some("false".to_owned()),
                RenderDistanceLimits::new(1, 16),
            )
            .unwrap();
        assert!(!state.finish().scene.freeze_time);
    }

    #[test]
    fn terrain_presentation_is_explicit_and_host_neutral() {
        assert_eq!(
            StartupSceneOptions::default().terrain_presentation,
            TerrainPresentationMode::ExactOnly
        );
        let argv = parse(&[ARG_TERRAIN_PRESENTATION, "composed"]).scene;
        assert_eq!(argv.terrain_presentation, TerrainPresentationMode::Composed);
        assert!(argv.terrain_presentation_explicit);
        let mut query = StartupArgState::default();
        assert!(
            query
                .parse_query_param(
                    QUERY_TERRAIN_PRESENTATION,
                    Some("horizon".to_owned()),
                    RenderDistanceLimits::new(1, 16),
                )
                .unwrap()
        );
        let query = query.finish().scene;
        assert_eq!(
            query.terrain_presentation,
            TerrainPresentationMode::Composed
        );
        assert!(query.terrain_presentation_explicit);
        assert_eq!(
            parse_terrain_presentation_mode(ARG_TERRAIN_PRESENTATION, Some("sideways".to_owned()))
                .unwrap_err()
                .to_string(),
            "--terrain-presentation must be exact-only or composed, got `sideways`"
        );
    }

    #[test]
    fn rejects_zero_render_compile_workers() {
        let mut state = StartupArgState::default();
        let mut args = ["0".to_owned()].into_iter();
        let err = state
            .parse_next_arg(
                ARG_RENDER_COMPILE_WORKERS,
                &mut args,
                RenderDistanceLimits::new(1, 16),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("must be greater than zero"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn rejects_zero_light_status_batch_size() {
        let mut state = StartupArgState::default();
        let mut args = ["0".to_owned()].into_iter();
        let err = state
            .parse_next_arg(
                ARG_LIGHT_STATUS_BATCH_SIZE,
                &mut args,
                RenderDistanceLimits::new(1, 16),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("must be greater than zero"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn applies_derived_render_compile_capacity_without_clobbering_manual_fields() {
        let report = RenderCompileCapacityReport {
            available_parallelism: Some(16),
            reserved_parallelism: 4,
            usable_parallelism: Some(12),
            cpu_worker_cap: 7,
            cpu_pack_cap: Some(16),
            total_memory_bytes: Some(64 * 1024 * 1024 * 1024),
            memory_budget_fraction: 0.3,
            memory_budget_bytes: Some(19 * 1024 * 1024 * 1024),
            mesh_footprint: Default::default(),
            mesh_buffer_pack_bytes: Some(16 * 1024 * 1024),
            memory_pack_cap: Some(300),
            worker_floor: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            max_pending_floor: DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS,
            derived_worker_count: 7,
            derived_max_pending_jobs: 14,
            fallback_reason: None,
        };

        let mut state = StartupArgState::default();
        let mut args = [
            ARG_RENDER_COMPILE_CAPACITY,
            "derived",
            ARG_RENDER_COMPILE_WORKERS,
            "2",
        ]
        .into_iter()
        .map(str::to_owned);
        while let Some(arg) = args.next() {
            assert!(
                state
                    .parse_next_arg(&arg, &mut args, RenderDistanceLimits::new(1, 16))
                    .unwrap()
            );
        }

        assert!(state.render_compile_worker_count_explicit());
        assert!(!state.render_compile_max_pending_jobs_explicit());
        state.apply_render_compile_capacity_report(&report);
        let scene = state.finish().scene;
        assert_eq!(scene.render_compile_worker_count, 2);
        assert_eq!(scene.render_compile_max_pending_jobs, Some(14));
    }

    #[test]
    fn rejects_render_distance_outside_configured_limits() {
        let mut state = StartupArgState::default();
        let mut args = ["1".to_owned()].into_iter();
        let err = state
            .parse_next_arg(
                ARG_RENDER_DISTANCE,
                &mut args,
                RenderDistanceLimits::new(2, 16),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("between 2 and 16"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn unknown_tokens_and_query_params_are_left_to_platform_parsers() {
        let mut state = StartupArgState::default();
        let mut args = std::iter::empty();
        assert!(
            !state
                .parse_next_arg(
                    "--platform-only",
                    &mut args,
                    RenderDistanceLimits::new(1, 16)
                )
                .unwrap()
        );
        assert!(
            !state
                .parse_query_param(
                    "platformOnly",
                    Some("1".to_owned()),
                    RenderDistanceLimits::new(1, 16)
                )
                .unwrap()
        );
    }
}
