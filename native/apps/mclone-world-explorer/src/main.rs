mod capture;
mod exact;
mod input;
mod options;
mod smoke;
mod terrain;
mod window;

use std::time::Instant;

use anyhow::Result;

use crate::options::ExplorerOptions;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let Some(options) = ExplorerOptions::parse()? else {
        return Ok(());
    };
    if let Some(path) = options.smoke_dir.clone() {
        let mut window_options = options.clone();
        window_options.smoke_dir = None;
        window_options.window_smoke_dir = Some(path.join("window"));
        window::run_window(window_options, Instant::now())?;
        return capture::run_smoke(&options, &path.join("offscreen"), Instant::now());
    }
    if let Some(path) = options.capture.clone() {
        capture::run_capture(&options, &path, Instant::now())
    } else {
        window::run_window(options, Instant::now())
    }
}
