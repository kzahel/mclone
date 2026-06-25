#![deny(unsafe_code)]

#[cfg(target_os = "android")]
mod android {
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;
    use winit::window::{Window, WindowAttributes, WindowId};

    #[derive(Default)]
    struct McloneAndroidApp {
        window: Option<Window>,
    }

    impl ApplicationHandler for McloneAndroidApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let attributes = WindowAttributes::default().with_title("Mclone");
                match event_loop.create_window(attributes) {
                    Ok(window) => {
                        log::info!("Mclone Android window created");
                        self.window = Some(window);
                    }
                    Err(error) => {
                        log::error!("failed to create Mclone Android window: {error}");
                        event_loop.exit();
                        return;
                    }
                }
            }

            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
            log::info!("Mclone Android suspended");
            self.window = None;
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            let Some(window) = self.window.as_ref() else {
                return;
            };
            if window.id() != window_id {
                return;
            }

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => window.request_redraw(),
                WindowEvent::Resized(size) => {
                    log::info!(
                        "Mclone Android window resized to {}x{}",
                        size.width,
                        size.height
                    );
                }
                _ => {}
            }
        }
    }

    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        log::info!("Mclone Android starting");
        let event_loop = EventLoop::builder()
            .with_android_app(app)
            .build()
            .expect("create Android event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut state = McloneAndroidApp::default();
        event_loop
            .run_app(&mut state)
            .expect("run Android event loop");
    }
}

#[cfg(not(target_os = "android"))]
pub fn host_placeholder() {}
