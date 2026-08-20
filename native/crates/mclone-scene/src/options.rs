use super::*;
use std::ops::{Deref, DerefMut};

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
    Graphics,
    SeasonalDebug,
    CelestialDebug,
}

impl XrDebugUiScreen {
    pub fn parse_label(flag: &str, value: &str) -> Result<Self> {
        match value.trim() {
            "pause" => Ok(Self::Pause),
            "controls" | "help" => Ok(Self::Controls),
            "graphics" | "video" => Ok(Self::Graphics),
            "seasonal" | "seasonal-debug" | "seasons" => Ok(Self::SeasonalDebug),
            "celestial" | "celestial-debug" | "sky" => Ok(Self::CelestialDebug),
            value => {
                bail!(
                    "{flag} must be pause, controls, graphics, seasonal-debug, or celestial-debug, got `{value}`"
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct McloneSceneHostOptions {
    pub startup: mclone_app_runtime::startup_args::StartupSceneOptions,
    pub render_compile_worker_timing_enabled: bool,
    pub simulation_cadence: SimulationCadenceConfig,
    pub player_movement_cadence: PlayerMovementCadenceConfig,
    pub world_behavior_profile: mclone_server::WorldBehaviorProfile,
    pub starter_content: mclone_server::StarterContentDescriptor,
    pub first_person_player_visible: bool,
    pub freeze_scheduled_fluid_ticks: bool,
    pub adaptive_chunk_publication_budget: bool,
    pub adaptive_render_admission_budget: bool,
    pub underwater_detection_mode: XrUnderwaterDetectionMode,
    pub debug_ui_screen: Option<XrDebugUiScreen>,
    pub skip_actors: bool,
    pub world_root: Option<PathBuf>,
    pub world_dir: Option<PathBuf>,
}

impl Default for McloneSceneHostOptions {
    fn default() -> Self {
        Self {
            startup: mclone_app_runtime::startup_args::StartupSceneOptions::default(),
            render_compile_worker_timing_enabled: true,
            simulation_cadence: SimulationCadenceConfig::default(),
            player_movement_cadence: PlayerMovementCadenceConfig::default(),
            world_behavior_profile: mclone_server::WorldBehaviorProfile::Mutable,
            starter_content: mclone_server::StarterContentDescriptor::Wild,
            first_person_player_visible: false,
            freeze_scheduled_fluid_ticks: false,
            adaptive_chunk_publication_budget: true,
            adaptive_render_admission_budget: true,
            underwater_detection_mode: XrUnderwaterDetectionMode::default(),
            debug_ui_screen: None,
            skip_actors: false,
            world_root: None,
            world_dir: None,
        }
    }
}

impl McloneSceneHostOptions {
    pub fn from_startup_scene(
        scene: mclone_app_runtime::startup_args::StartupSceneOptions,
        world_root: Option<PathBuf>,
        world_dir: Option<PathBuf>,
    ) -> Self {
        let starter_content = scene.starter_content;
        Self {
            startup: scene,
            render_compile_worker_timing_enabled: true,
            simulation_cadence: SimulationCadenceConfig::default(),
            player_movement_cadence: PlayerMovementCadenceConfig::default(),
            world_behavior_profile: mclone_server::WorldBehaviorProfile::Mutable,
            starter_content,
            first_person_player_visible: false,
            freeze_scheduled_fluid_ticks: false,
            adaptive_chunk_publication_budget: true,
            adaptive_render_admission_budget: true,
            underwater_detection_mode: XrUnderwaterDetectionMode::default(),
            debug_ui_screen: None,
            skip_actors: false,
            world_root,
            world_dir,
        }
    }

    pub fn center(&self) -> ChunkPos {
        ChunkPos::new(self.chunk_x, self.chunk_z)
    }

    pub(crate) fn project_terrain_presentation_for_source(
        &mut self,
        _local_authoritative_source: bool,
    ) {
        // Preserve the desired preset. `McloneSceneHost` projects unsupported
        // sources to an effective Off state while retaining the preference.
    }

    pub fn validated(self) -> Result<Self> {
        if !self.player_movement_cadence.is_valid() {
            bail!(
                "player movement cadence must have a rate in 1..=1000 Hz and at least one catch-up step, got {}/{}",
                self.player_movement_cadence.rate_hz,
                self.player_movement_cadence.max_catch_up_steps
            );
        }
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

impl Deref for McloneSceneHostOptions {
    type Target = mclone_app_runtime::startup_args::StartupSceneOptions;

    fn deref(&self) -> &Self::Target {
        &self.startup
    }
}

impl DerefMut for McloneSceneHostOptions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.startup
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

impl McloneSceneHost {
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

    pub fn set_frame_host_kind(&mut self, host_kind: FrameHostKind) {
        self.active_world
            .render_admission_policy
            .set_host_kind(host_kind);
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
        self.active_world.camera.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xr_debug_ui_parses_the_shared_seasonal_screen() {
        assert_eq!(
            XrDebugUiScreen::parse_label("--xr-debug-ui", "seasonal-debug").unwrap(),
            XrDebugUiScreen::SeasonalDebug,
        );
        assert_eq!(
            XrDebugUiScreen::parse_label("--xr-debug-ui", "celestial-debug").unwrap(),
            XrDebugUiScreen::CelestialDebug,
        );
    }

    #[test]
    fn xr_scene_startup_projection_round_trips_shared_fields() {
        let mut scene = McloneSceneHostOptions::default();
        scene.seed = 98_765;
        scene.chunk_x = -4;
        scene.chunk_z = 7;
        scene.render_distance = 3;
        scene.freeze_time = true;
        scene.remote_addr = Some("example.test:25565".to_owned());
        scene.adaptive_chunk_publication_budget = false;
        let startup = scene.startup.clone();
        let projected = McloneSceneHostOptions::from_startup_scene(
            startup,
            Some(PathBuf::from("/tmp/worlds")),
            Some(PathBuf::from("/tmp/worlds/demo")),
        );

        assert_eq!(projected.seed, scene.seed);
        assert_eq!(projected.chunk_x, scene.chunk_x);
        assert_eq!(projected.chunk_z, scene.chunk_z);
        assert_eq!(projected.render_distance, scene.render_distance);
        assert_eq!(projected.freeze_time, scene.freeze_time);
        assert_eq!(projected.remote_addr, scene.remote_addr);
        assert_eq!(projected.world_root, Some(PathBuf::from("/tmp/worlds")));
        assert_eq!(projected.world_dir, Some(PathBuf::from("/tmp/worlds/demo")));
        // Platform policy is deliberately layered after the shared projection.
        assert!(projected.adaptive_chunk_publication_budget);
    }

    #[test]
    fn scene_defaults_to_a_sixty_hz_player_clock_independent_of_world_ticks() {
        let scene = McloneSceneHostOptions::default();
        assert_eq!(scene.player_movement_cadence.rate_hz, 60);
        assert_eq!(scene.simulation_cadence.gameplay_rate_hz, 20);
        assert!(scene.validated().is_ok());
    }

    #[test]
    fn scene_rejects_an_invalid_player_movement_cadence() {
        let mut scene = McloneSceneHostOptions::default();
        scene.player_movement_cadence = PlayerMovementCadenceConfig::new(0, 0);
        let error = scene.validated().expect_err("invalid cadence");
        assert!(error.to_string().contains("player movement cadence"));
    }

    #[test]
    fn session_source_projection_retains_the_desired_lod_for_unsupported_sources() {
        let mut compatible = McloneSceneHostOptions::default();
        compatible.world_generation_profile =
            mclone_server::WorldGenerationProfile::McloneOverworldV1;
        compatible.terrain_lod_preset = mclone_core::TerrainLodPreset::High;
        compatible.project_terrain_presentation_for_source(true);
        assert_eq!(
            compatible.terrain_lod_preset,
            mclone_core::TerrainLodPreset::High
        );

        let mut incompatible_profile = compatible.clone();
        incompatible_profile.world_generation_profile =
            mclone_server::WorldGenerationProfile::TopologyProbeV1;
        incompatible_profile.project_terrain_presentation_for_source(true);
        assert_eq!(
            incompatible_profile.terrain_lod_preset,
            mclone_core::TerrainLodPreset::High
        );

        let mut remote = compatible;
        remote.project_terrain_presentation_for_source(false);
        assert_eq!(
            remote.terrain_lod_preset,
            mclone_core::TerrainLodPreset::High
        );
    }
}
