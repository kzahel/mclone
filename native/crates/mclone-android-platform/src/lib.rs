#![forbid(unsafe_code)]

use std::path::PathBuf;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

pub const ANDROID_WORLD_ROOT_DIR_NAME: &str = "worlds";

pub fn android_world_root_from_app_data_paths(
    internal_data_path: Option<PathBuf>,
    external_data_path: Option<PathBuf>,
) -> Option<PathBuf> {
    internal_data_path
        .or(external_data_path)
        .map(|path| path.join(ANDROID_WORLD_ROOT_DIR_NAME))
}

#[cfg(target_os = "android")]
pub fn android_app_data_world_root(app: &AndroidApp, log_label: &str) -> Option<PathBuf> {
    let root =
        android_world_root_from_app_data_paths(app.internal_data_path(), app.external_data_path());
    if let Some(root) = &root {
        log::info!("{log_label} world catalog root: {}", root.display());
    } else {
        log::warn!("{log_label} could not resolve an app data path for persistent worlds");
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_root_prefers_internal_app_data_path() {
        let root = android_world_root_from_app_data_paths(
            Some(PathBuf::from("/data/user/0/com.example/files")),
            Some(PathBuf::from("/sdcard/Android/data/com.example/files")),
        );

        assert_eq!(
            root,
            Some(PathBuf::from("/data/user/0/com.example/files/worlds"))
        );
    }

    #[test]
    fn world_root_falls_back_to_external_app_data_path() {
        let root = android_world_root_from_app_data_paths(
            None,
            Some(PathBuf::from("/sdcard/Android/data/com.example/files")),
        );

        assert_eq!(
            root,
            Some(PathBuf::from(
                "/sdcard/Android/data/com.example/files/worlds"
            ))
        );
    }

    #[test]
    fn world_root_is_absent_without_app_data_path() {
        assert_eq!(android_world_root_from_app_data_paths(None, None), None);
    }
}
