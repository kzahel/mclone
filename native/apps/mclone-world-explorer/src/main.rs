mod capture;
mod options;
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
    let process_started = Instant::now();
    if let Some(path) = options.capture.clone() {
        capture::run_capture(&options, &path, process_started)
    } else {
        window::run_window(options, process_started)
    }
}
