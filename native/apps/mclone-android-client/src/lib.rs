#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod startup;
#[cfg(target_os = "android")]
mod surface_driver;

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CString, c_char, c_int};
    use std::sync::Once;

    use mclone_android_platform::{
        ANDROID_ASSET_ROOT_ENV, AndroidAppDataPathPreference, android_app_data_asset_root,
    };
    use mclone_app_runtime::native_remote_session::NativeRemoteServerSession;
    use mclone_scene::McloneSceneHost;
    use winit::event_loop::{ControlFlow, EventLoop};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;

    use crate::startup::prepare_android_startup;
    use crate::surface_driver::AndroidSurfaceDriver;

    pub(crate) type AndroidSceneHost = McloneSceneHost<NativeRemoteServerSession>;

    const LOG_TAG: &str = "mclone_android";

    #[allow(unsafe_code)]
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_print(
            priority: c_int,
            tag: *const c_char,
            format: *const c_char,
            ...
        ) -> c_int;
    }

    #[allow(unsafe_code)]
    fn android_log(priority: c_int, message: impl AsRef<str>) {
        let tag = CString::new(LOG_TAG).expect("static log tag contains no nul");
        let format = c"%s";
        let message = CString::new(message.as_ref().replace('\0', "\\0"))
            .expect("nul bytes were escaped before logging");
        unsafe {
            __android_log_print(priority, tag.as_ptr(), format.as_ptr(), message.as_ptr());
        }
    }

    struct AndroidLogger;

    impl log::Log for AndroidLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let priority = match record.level() {
                log::Level::Error => 6,
                log::Level::Warn => 5,
                log::Level::Info => 4,
                log::Level::Debug => 3,
                log::Level::Trace => 2,
            };
            android_log(priority, format!("{}: {}", record.target(), record.args()));
        }

        fn flush(&self) {}
    }

    static LOGGER: AndroidLogger = AndroidLogger;
    static INIT_LOGGER: Once = Once::new();

    fn init_android_logger() {
        INIT_LOGGER.call_once(|| {
            if log::set_logger(&LOGGER).is_ok() {
                log::set_max_level(log::LevelFilter::Info);
            }
        });
    }

    #[allow(unsafe_code)]
    fn configure_android_asset_root(app: &AndroidApp) {
        if let Some(existing) = std::env::var_os(ANDROID_ASSET_ROOT_ENV) {
            log::info!(
                "preserving {ANDROID_ASSET_ROOT_ENV}={}",
                std::path::PathBuf::from(existing).display()
            );
            return;
        }
        let Some(path) =
            android_app_data_asset_root(app, AndroidAppDataPathPreference::InternalFirst)
        else {
            log::warn!("could not resolve Android app data path for {ANDROID_ASSET_ROOT_ENV}");
            return;
        };
        unsafe {
            std::env::set_var(ANDROID_ASSET_ROOT_ENV, &path);
        }
        log::info!(
            "configured {ANDROID_ASSET_ROOT_ENV} from app data path: {}",
            path.display()
        );
    }

    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        init_android_logger();
        configure_android_asset_root(&app);
        log::info!("Mclone Android starting");
        let startup = match prepare_android_startup(&app) {
            Ok(startup) => startup,
            Err(error) => {
                log::error!("MCLONE_ANDROID_FAILURE: {error:#}");
                return;
            }
        };
        let event_loop = EventLoop::builder()
            .with_android_app(app)
            .build()
            .expect("create Android event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut driver = AndroidSurfaceDriver::new(startup);
        event_loop
            .run_app(&mut driver)
            .expect("run Android event loop");
    }
}

#[cfg(target_os = "android")]
pub(crate) use android::AndroidSceneHost;

#[cfg(not(target_os = "android"))]
pub fn host_placeholder() {}
