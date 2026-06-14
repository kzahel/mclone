use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use mclone_render::headless::{HeadlessClearOptions, write_headless_clear_png};
use mclone_render::native::{NativeSurfaceContext, SurfaceClearStatus};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn main() -> Result<()> {
    env_logger::init();
    match Cli::parse(std::env::args().skip(1))? {
        Cli::HeadlessClear {
            path,
            width,
            height,
        } => {
            let report = write_headless_clear_png(HeadlessClearOptions {
                path,
                width,
                height,
                color: mclone_render::default_clear_color(),
            })?;
            println!(
                "headless clear saved to {} ({}x{}, {} bytes)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len
            );
            Ok(())
        }
        Cli::Window => run_window(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Cli {
    Window,
    HeadlessClear {
        path: PathBuf,
        width: u32,
        height: u32,
    },
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut width = 96;
        let mut height = 64;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--headless-clear" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-clear requires an output PNG path")?;
                    mode = Some(Self::HeadlessClear {
                        path,
                        width,
                        height,
                    });
                }
                "--width" => {
                    width = parse_u32_arg("--width", args.next())?;
                    if let Some(Self::HeadlessClear { width: w, .. }) = &mut mode {
                        *w = width;
                    }
                }
                "--height" => {
                    height = parse_u32_arg("--height", args.next())?;
                    if let Some(Self::HeadlessClear { height: h, .. }) = &mut mode {
                        *h = height;
                    }
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }
        Ok(mode.unwrap_or(Self::Window))
    }
}

fn parse_u32_arg(flag: &str, value: Option<String>) -> Result<u32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("{flag} requires an unsigned integer, got `{value}`"))?;
    if parsed == 0 {
        bail!("{flag} must be greater than zero");
    }
    Ok(parsed)
}

fn print_help() {
    println!(
        "mclone-native-client\n\n\
         Usage:\n\
           mclone-native-client\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\n\
         Window mode opens a native winit/wgpu clear-color window. Headless mode writes a PNG for GPU validation."
    );
}

fn run_window() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ClearApp::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[derive(Default)]
struct ClearApp {
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
}

impl ClearApp {
    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for ClearApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("mclone native")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 900));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                log::error!("failed to create native window: {err}");
                event_loop.exit();
                return;
            }
        };
        let surface = match NativeSurfaceContext::new(window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
        self.surface = Some(surface);
        self.window = Some(window);
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.resize(size);
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if matches!(event.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(surface) = &mut self.surface else {
                    return;
                };
                match surface.render_clear(mclone_render::default_clear_color()) {
                    Ok(SurfaceClearStatus::Presented | SurfaceClearStatus::Skipped) => {}
                    Ok(SurfaceClearStatus::Reconfigured) => self.request_redraw(),
                    Err(err) => {
                        log::error!("render failed: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
                self.request_redraw();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_defaults_to_window() {
        assert_eq!(Cli::parse([]).unwrap(), Cli::Window);
    }

    #[test]
    fn cli_parses_headless_clear_dimensions_after_path() {
        let cli = Cli::parse([
            "--headless-clear".to_owned(),
            "/tmp/mclone.png".to_owned(),
            "--width".to_owned(),
            "32".to_owned(),
            "--height".to_owned(),
            "16".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessClear {
                path: PathBuf::from("/tmp/mclone.png"),
                width: 32,
                height: 16,
            }
        );
    }

    #[test]
    fn cli_parses_dimensions_before_headless_path() {
        let cli = Cli::parse([
            "--width".to_owned(),
            "32".to_owned(),
            "--height".to_owned(),
            "16".to_owned(),
            "--headless-clear".to_owned(),
            "/tmp/mclone.png".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessClear {
                path: PathBuf::from("/tmp/mclone.png"),
                width: 32,
                height: 16,
            }
        );
    }
}
