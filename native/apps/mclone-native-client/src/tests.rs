use super::*;
use crate::cli::{NativeWindowOptions, StartupWaitPolicy, WindowPlatformProfile, XrDebugUiScreen};
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
            hud: false,
            frame_pipeline_overlay: false,
            debug_pane: false,
            worldgen_lens: None,
            player_collision_box: false,
            blink_debug: false,
            controller_focus: false,
            scripted_interaction: false,
            remote_settle_ms: 0,
            settle_frames: 0,
            terrain_horizon_diagnostic: Default::default(),
            eye: None,
            target: None,
            season_preview: Default::default(),
        },
    }
}

mod cli_actor;
mod cli_core;
mod cli_headless;
mod cli_perf;
mod cli_render_options;
mod cli_xr;
