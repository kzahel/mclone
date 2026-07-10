#![forbid(unsafe_code)]

use std::path::PathBuf;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

pub const ANDROID_ASSET_ROOT_ENV: &str = "MCLONE_ANDROID_ASSET_ROOT";
pub const ANDROID_WORLD_ROOT_DIR_NAME: &str = "worlds";
pub const ANDROID_REMOTE_ADDR_NONE_SENTINEL: &str = "__mclone_none__";

pub fn normalize_android_legacy_remote_addr(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == ANDROID_REMOTE_ADDR_NONE_SENTINEL {
        None
    } else {
        Some(value.to_owned())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AndroidAppDataPathPreference {
    InternalFirst,
    ExternalFirst,
}

pub fn android_preferred_app_data_path(
    internal_data_path: Option<PathBuf>,
    external_data_path: Option<PathBuf>,
    preference: AndroidAppDataPathPreference,
) -> Option<PathBuf> {
    match preference {
        AndroidAppDataPathPreference::InternalFirst => internal_data_path.or(external_data_path),
        AndroidAppDataPathPreference::ExternalFirst => external_data_path.or(internal_data_path),
    }
}

pub fn android_asset_root_from_app_data_paths(
    internal_data_path: Option<PathBuf>,
    external_data_path: Option<PathBuf>,
    preference: AndroidAppDataPathPreference,
) -> Option<PathBuf> {
    android_preferred_app_data_path(internal_data_path, external_data_path, preference)
}

#[cfg(target_os = "android")]
pub fn android_app_data_asset_root(
    app: &AndroidApp,
    preference: AndroidAppDataPathPreference,
) -> Option<PathBuf> {
    android_asset_root_from_app_data_paths(
        app.internal_data_path(),
        app.external_data_path(),
        preference,
    )
}

pub fn android_world_root_from_app_data_paths(
    internal_data_path: Option<PathBuf>,
    external_data_path: Option<PathBuf>,
) -> Option<PathBuf> {
    android_preferred_app_data_path(
        internal_data_path,
        external_data_path,
        AndroidAppDataPathPreference::InternalFirst,
    )
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
    fn preferred_app_data_path_uses_requested_order() {
        let internal = Some(PathBuf::from("/data/user/0/com.example/files"));
        let external = Some(PathBuf::from("/sdcard/Android/data/com.example/files"));

        assert_eq!(
            android_preferred_app_data_path(
                internal.clone(),
                external.clone(),
                AndroidAppDataPathPreference::InternalFirst,
            ),
            internal
        );
        assert_eq!(
            android_preferred_app_data_path(
                Some(PathBuf::from("/data/user/0/com.example/files")),
                external.clone(),
                AndroidAppDataPathPreference::ExternalFirst,
            ),
            external
        );
    }

    #[test]
    fn asset_root_prefers_requested_app_data_path() {
        let root = android_asset_root_from_app_data_paths(
            Some(PathBuf::from("/data/user/0/com.example/files")),
            Some(PathBuf::from("/sdcard/Android/data/com.example/files")),
            AndroidAppDataPathPreference::ExternalFirst,
        );

        assert_eq!(
            root,
            Some(PathBuf::from("/sdcard/Android/data/com.example/files"))
        );
    }

    #[test]
    fn asset_root_is_absent_without_app_data_path() {
        assert_eq!(
            android_asset_root_from_app_data_paths(
                None,
                None,
                AndroidAppDataPathPreference::InternalFirst,
            ),
            None
        );
    }

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

    #[test]
    fn legacy_remote_addr_ignores_empty_and_none_sentinel() {
        assert_eq!(normalize_android_legacy_remote_addr("   \t"), None);
        assert_eq!(
            normalize_android_legacy_remote_addr(ANDROID_REMOTE_ADDR_NONE_SENTINEL),
            None
        );
    }

    #[test]
    fn legacy_remote_addr_preserves_real_addresses_and_near_misses() {
        assert_eq!(
            normalize_android_legacy_remote_addr(" 192.168.1.10:25565 "),
            Some("192.168.1.10:25565".to_owned())
        );
        assert_eq!(
            normalize_android_legacy_remote_addr("__mclone_none__:25565"),
            Some("__mclone_none__:25565".to_owned())
        );
    }
}
