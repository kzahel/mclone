use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrUnderwaterDetectionMode {
    #[default]
    Midpoint,
    PerEye,
}

impl XrUnderwaterDetectionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Midpoint => "midpoint",
            Self::PerEye => "per-eye",
        }
    }

    pub fn parse_label(flag: &str, value: &str) -> Result<Self> {
        match value.trim() {
            "midpoint" | "middle" | "center" | "both" => Ok(Self::Midpoint),
            "per-eye" | "per_eye" | "eye" | "eyes" => Ok(Self::PerEye),
            value => bail!("{flag} must be midpoint or per-eye, got `{value}`"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrDebugUiScreen {
    Pause,
    Controls,
}

impl XrDebugUiScreen {
    pub fn parse_label(flag: &str, value: &str) -> Result<Self> {
        match value.trim() {
            "pause" => Ok(Self::Pause),
            "controls" | "help" => Ok(Self::Controls),
            value => bail!("{flag} must be pause or controls, got `{value}`"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct XrSceneOptions {
    pub seed: i64,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub render_distance: u32,
    pub render_compile_worker_count: usize,
    pub render_compile_max_pending_jobs: Option<usize>,
    pub movement_speed_multiplier: f32,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub debug_passive_showcase: bool,
    pub lighting_enabled: bool,
    pub light_status_batch_size: usize,
    pub adaptive_chunk_publication_budget: bool,
    pub far_lod: FarTerrainLodConfig,
    pub underwater_detection_mode: XrUnderwaterDetectionMode,
    pub debug_ui_screen: Option<XrDebugUiScreen>,
    pub skip_actors: bool,
    pub world_root: Option<PathBuf>,
    pub world_dir: Option<PathBuf>,
}

impl Default for XrSceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_XR_SEED,
            chunk_x: DEFAULT_XR_CHUNK_X,
            chunk_z: DEFAULT_XR_CHUNK_Z,
            render_distance: DEFAULT_XR_RENDER_DISTANCE,
            render_compile_worker_count:
                mclone_app_runtime::render_assets::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            render_compile_max_pending_jobs: Some(
                mclone_app_runtime::render_assets::DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS,
            ),
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            day_time_override: None,
            freeze_time: false,
            debug_passive_showcase: true,
            lighting_enabled: true,
            light_status_batch_size:
                mclone_app_runtime::startup_args::StartupSceneOptions::default()
                    .light_status_batch_size,
            adaptive_chunk_publication_budget: true,
            far_lod: FarTerrainLodConfig::default(),
            underwater_detection_mode: XrUnderwaterDetectionMode::default(),
            debug_ui_screen: None,
            skip_actors: false,
            world_root: None,
            world_dir: None,
        }
    }
}

impl XrSceneOptions {
    pub fn from_startup_scene(
        scene: mclone_app_runtime::startup_args::StartupSceneOptions,
        world_root: Option<PathBuf>,
        world_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            seed: scene.seed,
            chunk_x: scene.chunk_x,
            chunk_z: scene.chunk_z,
            render_distance: scene.render_distance,
            render_compile_worker_count: scene.render_compile_worker_count,
            render_compile_max_pending_jobs: scene.render_compile_max_pending_jobs,
            movement_speed_multiplier: scene.movement_speed_multiplier,
            day_time_override: scene.day_time_override,
            freeze_time: scene.freeze_time,
            debug_passive_showcase: scene.debug_passive_showcase,
            lighting_enabled: scene.lighting_enabled,
            light_status_batch_size: scene.light_status_batch_size,
            adaptive_chunk_publication_budget: true,
            far_lod: scene.far_lod,
            underwater_detection_mode: XrUnderwaterDetectionMode::default(),
            debug_ui_screen: None,
            skip_actors: false,
            world_root,
            world_dir,
        }
    }

    pub fn to_startup_scene(&self) -> mclone_app_runtime::startup_args::StartupSceneOptions {
        mclone_app_runtime::startup_args::StartupSceneOptions {
            seed: self.seed,
            chunk_x: self.chunk_x,
            chunk_z: self.chunk_z,
            render_distance: self.render_distance,
            render_compile_worker_count: self.render_compile_worker_count,
            render_compile_max_pending_jobs: self.render_compile_max_pending_jobs,
            render_compile_capacity_request: Default::default(),
            movement_speed_multiplier: self.movement_speed_multiplier,
            remote_addr: None,
            day_time_override: self.day_time_override,
            freeze_time: self.freeze_time,
            debug_passive_showcase: self.debug_passive_showcase,
            lighting_enabled: self.lighting_enabled,
            light_status_batch_size: self.light_status_batch_size,
            far_lod: self.far_lod,
        }
    }

    pub fn center(&self) -> ChunkPos {
        ChunkPos::new(self.chunk_x, self.chunk_z)
    }

    pub fn validated(self) -> Result<Self> {
        if self.render_distance == 0 || self.render_distance > MAX_XR_RENDER_DISTANCE {
            bail!(
                "XR render distance must be between 1 and {MAX_XR_RENDER_DISTANCE}, got {}",
                self.render_distance
            );
        }
        if self.render_compile_worker_count == 0 {
            bail!("XR render compile worker count must be greater than zero");
        }
        if let Some(max_pending_jobs) = self.render_compile_max_pending_jobs {
            if max_pending_jobs < self.render_compile_worker_count {
                bail!("XR render compile max pending jobs must be at least the worker count");
            }
        }
        if self.light_status_batch_size == 0 {
            bail!("XR light status batch size must be greater than zero");
        }
        let min = ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32;
        let max = ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32;
        if !self.movement_speed_multiplier.is_finite()
            || !(min..=max).contains(&self.movement_speed_multiplier)
        {
            bail!(
                "XR movement speed multiplier must be between {min} and {max}, got {}",
                self.movement_speed_multiplier
            );
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrStartupViewPose {
    pub position: [f32; 3],
    pub yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrViewAlignmentMode {
    PlayerSpawn,
    ViewPose,
}

impl XrViewAlignmentMode {
    pub const fn label(self) -> &'static str {
        match self {
            XrViewAlignmentMode::PlayerSpawn => "player-spawn",
            XrViewAlignmentMode::ViewPose => "view-pose",
        }
    }
}

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn set_locomotion_mode(&mut self, locomotion_mode: XrLocomotionMode) {
        self.locomotion_mode = locomotion_mode;
    }

    pub fn locomotion_mode(&self) -> XrLocomotionMode {
        self.locomotion_mode
    }

    pub fn set_turn_policy(&mut self, turn_policy: XrTurnPolicy) {
        self.turn_policy = turn_policy;
        self.snap_turn_state.reset();
    }

    pub fn turn_policy(&self) -> XrTurnPolicy {
        self.turn_policy
    }

    pub fn set_display_refresh_hz(&mut self, display_refresh_hz: Option<f32>) {
        self.display_refresh_hz = display_refresh_hz.filter(|hz| hz.is_finite() && *hz > 0.0);
    }

    pub fn set_render_split_timing_enabled(&mut self, enabled: bool) {
        self.render_split_timing_enabled = enabled;
    }

    pub fn set_defer_eye_waits_enabled(&mut self, enabled: bool) {
        self.defer_eye_waits_enabled = enabled;
    }

    pub fn set_overlap_runtime_prefetch_enabled(&mut self, enabled: bool) {
        self.overlap_runtime_prefetch_enabled = enabled;
        if !enabled {
            self.prefetched_live_upload = None;
        }
    }

    pub fn set_render_section_upload_budget(&mut self, budget: Option<usize>) {
        self.render_section_upload_budget = budget.filter(|budget| *budget > 0);
    }

    pub fn render_section_upload_budget(&self) -> Option<usize> {
        self.render_section_upload_budget
    }

    pub fn set_render_section_accept_budget(&mut self, budget: Option<usize>) {
        self.render_section_accept_budget = budget.filter(|budget| *budget > 0);
    }

    pub fn render_section_accept_budget(&self) -> Option<usize> {
        self.render_section_accept_budget
    }

    pub fn set_render_completed_result_accept_budget(&mut self, budget: Option<usize>) {
        self.render_completed_result_accept_budget = budget.filter(|budget| *budget > 0);
    }

    pub fn render_completed_result_accept_budget(&self) -> Option<usize> {
        self.render_completed_result_accept_budget
    }

    pub fn camera_snapshot(&self) -> EngineCameraSnapshot {
        self.camera.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xr_scene_startup_projection_round_trips_shared_fields() {
        let mut scene = XrSceneOptions::default();
        scene.seed = 98_765;
        scene.chunk_x = -4;
        scene.chunk_z = 7;
        scene.render_distance = 3;
        scene.freeze_time = true;
        scene.adaptive_chunk_publication_budget = false;
        let startup = scene.to_startup_scene();
        let projected = XrSceneOptions::from_startup_scene(
            startup,
            Some(PathBuf::from("/tmp/worlds")),
            Some(PathBuf::from("/tmp/worlds/demo")),
        );

        assert_eq!(projected.seed, scene.seed);
        assert_eq!(projected.chunk_x, scene.chunk_x);
        assert_eq!(projected.chunk_z, scene.chunk_z);
        assert_eq!(projected.render_distance, scene.render_distance);
        assert_eq!(projected.freeze_time, scene.freeze_time);
        assert_eq!(projected.world_root, Some(PathBuf::from("/tmp/worlds")));
        assert_eq!(projected.world_dir, Some(PathBuf::from("/tmp/worlds/demo")));
        // Platform policy is deliberately layered after the shared projection.
        assert!(projected.adaptive_chunk_publication_budget);
    }
}
