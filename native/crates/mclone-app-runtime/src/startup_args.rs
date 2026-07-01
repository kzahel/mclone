use anyhow::{Context, Result, bail};
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::RenderColorProfile;
use mclone_render_session::{
    ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
};

use crate::render_assets::DEFAULT_RENDER_SECTION_COMPILE_WORKERS;

pub const ARG_SEED: &str = "--seed";
pub const ARG_CHUNK_X: &str = "--chunk-x";
pub const ARG_CHUNK_Z: &str = "--chunk-z";
pub const ARG_RENDER_DISTANCE: &str = "--render-distance";
pub const ARG_RENDER_COMPILE_WORKERS: &str = "--render-compile-workers";
pub const ARG_REMOTE_ADDR: &str = "--remote-addr";
pub const ARG_DAY_TIME: &str = "--day-time";
pub const ARG_FREEZE_TIME: &str = "--freeze-time";
pub const ARG_MOVEMENT_SPEED_MULTIPLIER: &str = "--movement-speed-multiplier";
pub const ARG_DEBUG_PASSIVE_SHOWCASE: &str = "--debug-passive-showcase";
pub const ARG_LIGHTING: &str = "--lighting";
pub const ARG_SECTION_OCCLUSION: &str = "--section-occlusion";
pub const ARG_FULLBRIGHT: &str = "--fullbright";
pub const ARG_RENDER_COLOR_PROFILE: &str = "--render-color-profile";

pub const QUERY_SEED: &str = "seed";
pub const QUERY_CHUNK_X: &str = "chunkX";
pub const QUERY_CHUNK_Z: &str = "chunkZ";
pub const QUERY_RENDER_DISTANCE: &str = "renderDistance";
pub const QUERY_RENDER_COMPILE_WORKERS: &str = "renderCompileWorkers";
pub const QUERY_REMOTE_WS_URL: &str = "remoteWsUrl";
pub const QUERY_DAY_TIME: &str = "dayTime";
pub const QUERY_FREEZE_TIME: &str = "freezeTime";
pub const QUERY_MOVEMENT_SPEED_MULTIPLIER: &str = "movementSpeedMultiplier";
pub const QUERY_DEBUG_PASSIVE_SHOWCASE: &str = "debugPassiveShowcase";
pub const QUERY_LIGHTING: &str = "lighting";
pub const QUERY_SECTION_OCCLUSION: &str = "sectionOcclusion";
pub const QUERY_FULLBRIGHT: &str = "fullbright";
pub const QUERY_RENDER_COLOR_PROFILE: &str = "renderColorProfile";

pub const STARTUP_QUERY_KEYS: &[&str] = &[
    QUERY_SEED,
    QUERY_CHUNK_X,
    QUERY_CHUNK_Z,
    QUERY_RENDER_DISTANCE,
    QUERY_RENDER_COMPILE_WORKERS,
    QUERY_REMOTE_WS_URL,
    QUERY_DAY_TIME,
    QUERY_FREEZE_TIME,
    QUERY_MOVEMENT_SPEED_MULTIPLIER,
    QUERY_DEBUG_PASSIVE_SHOWCASE,
    QUERY_LIGHTING,
    QUERY_SECTION_OCCLUSION,
    QUERY_FULLBRIGHT,
    QUERY_RENDER_COLOR_PROFILE,
];

pub const DEFAULT_STARTUP_SEED: i64 = 12_345;
pub const DEFAULT_STARTUP_CHUNK_X: i32 = 0;
pub const DEFAULT_STARTUP_CHUNK_Z: i32 = 0;
pub const DEFAULT_STARTUP_RENDER_DISTANCE: u32 = 5;

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
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub render_distance: u32,
    pub render_compile_worker_count: usize,
    pub remote_addr: Option<String>,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub movement_speed_multiplier: f32,
    pub debug_passive_showcase: bool,
    pub lighting_enabled: bool,
}

impl Default for StartupSceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_STARTUP_SEED,
            chunk_x: DEFAULT_STARTUP_CHUNK_X,
            chunk_z: DEFAULT_STARTUP_CHUNK_Z,
            render_distance: DEFAULT_STARTUP_RENDER_DISTANCE,
            render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            remote_addr: None,
            day_time_override: None,
            freeze_time: false,
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            debug_passive_showcase: true,
            lighting_enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartupOptions {
    pub scene: StartupSceneOptions,
    pub render_options: TexturedSectionRenderOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartupArgState {
    scene: StartupSceneOptions,
    render_options: TexturedSectionRenderOptions,
    fullbright_explicit: bool,
}

impl StartupArgState {
    pub fn new(scene: StartupSceneOptions, render_options: TexturedSectionRenderOptions) -> Self {
        Self {
            scene,
            render_options,
            fullbright_explicit: false,
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
            }
            ARG_REMOTE_ADDR => {
                self.scene.remote_addr = parse_remote_addr_arg(args.next())?;
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
            ARG_DEBUG_PASSIVE_SHOWCASE => {
                self.scene.debug_passive_showcase =
                    parse_bool_arg(ARG_DEBUG_PASSIVE_SHOWCASE, args.next())?;
            }
            ARG_LIGHTING => {
                self.scene.lighting_enabled = parse_bool_arg(ARG_LIGHTING, args.next())?;
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
            QUERY_DEBUG_PASSIVE_SHOWCASE => {
                self.scene.debug_passive_showcase =
                    parse_bool_arg(QUERY_DEBUG_PASSIVE_SHOWCASE, value)?;
            }
            QUERY_LIGHTING => {
                self.scene.lighting_enabled = parse_bool_arg(QUERY_LIGHTING, value)?;
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
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn finish(mut self) -> StartupOptions {
        if !self.scene.lighting_enabled && !self.fullbright_explicit {
            self.render_options.force_fullbright = true;
        }
        StartupOptions {
            scene: self.scene,
            render_options: self.render_options,
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
    let parsed = parse_u32_arg(flag, value)?;
    usize::try_from(parsed).with_context(|| format!("{flag} value does not fit usize"))
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
                chunk_x: 0,
                chunk_z: 0,
                render_distance: 5,
                render_compile_worker_count: DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
                remote_addr: None,
                day_time_override: None,
                freeze_time: false,
                movement_speed_multiplier: 1.0,
                debug_passive_showcase: true,
                lighting_enabled: true,
            }
        );
        assert_eq!(
            StartupArgState::default().finish().render_options,
            TexturedSectionRenderOptions::default()
        );
    }

    #[test]
    fn parses_scene_tokens() {
        let options = parse(&[
            ARG_SEED,
            "-77",
            ARG_CHUNK_X,
            "4",
            ARG_CHUNK_Z,
            "-3",
            ARG_RENDER_DISTANCE,
            "5",
            ARG_RENDER_COMPILE_WORKERS,
            "2",
            ARG_DAY_TIME,
            "6000",
            ARG_FREEZE_TIME,
            ARG_MOVEMENT_SPEED_MULTIPLIER,
            "2.5",
            ARG_DEBUG_PASSIVE_SHOWCASE,
            "false",
            ARG_REMOTE_ADDR,
            "127.0.0.1:25565",
        ]);
        assert_eq!(
            options.scene,
            StartupSceneOptions {
                seed: -77,
                chunk_x: 4,
                chunk_z: -3,
                render_distance: 5,
                render_compile_worker_count: 2,
                remote_addr: Some("127.0.0.1:25565".to_owned()),
                day_time_override: Some(6000),
                freeze_time: true,
                movement_speed_multiplier: 2.5,
                debug_passive_showcase: false,
                lighting_enabled: true,
            }
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
    fn parses_query_params() {
        let mut state = StartupArgState::default();
        for (key, value) in [
            (QUERY_SEED, "-77"),
            (QUERY_CHUNK_X, "4"),
            (QUERY_CHUNK_Z, "-3"),
            (QUERY_RENDER_DISTANCE, "6"),
            (QUERY_RENDER_COMPILE_WORKERS, "3"),
            (QUERY_REMOTE_WS_URL, "ws://127.0.0.1:25565"),
            (QUERY_DAY_TIME, "6000"),
            (QUERY_FREEZE_TIME, ""),
            (QUERY_MOVEMENT_SPEED_MULTIPLIER, "0.5"),
            (QUERY_DEBUG_PASSIVE_SHOWCASE, "false"),
            (QUERY_LIGHTING, "false"),
            (QUERY_SECTION_OCCLUSION, "false"),
            (QUERY_FULLBRIGHT, "true"),
            (QUERY_RENDER_COLOR_PROFILE, "stylized-bright"),
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
                chunk_x: 4,
                chunk_z: -3,
                render_distance: 6,
                render_compile_worker_count: 3,
                remote_addr: Some("ws://127.0.0.1:25565".to_owned()),
                day_time_override: Some(6000),
                freeze_time: true,
                movement_speed_multiplier: 0.5,
                debug_passive_showcase: false,
                lighting_enabled: false,
            }
        );
        assert!(!options.render_options.section_occlusion_culling);
        assert!(options.render_options.force_fullbright);
        assert_eq!(
            options.render_options.color_profile,
            RenderColorProfile::StylizedBright
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
