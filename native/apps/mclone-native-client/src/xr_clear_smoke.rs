use anyhow::Result;

use crate::cli::XrClearSmokeOptions;

pub(crate) fn run(options: XrClearSmokeOptions) -> Result<()> {
    let _ = std::any::type_name::<openxr::Entry>();
    println!(
        "desktop OpenXR clear-smoke gate ready: frames={} (loader/session/swapchain bring-up is next)",
        options.frames
    );
    Ok(())
}
