use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use mclone_mesh::{ChunkMeshInput, VisibleChunkMesh, build_visible_chunk_mesh};
use mclone_render::chunk::{ChunkCamera, ChunkDrawResources};
use mclone_render::headless::{
    HeadlessChunkOptions, HeadlessClearOptions, write_headless_chunk_png, write_headless_clear_png,
};
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_worldgen::levelgen::generate_overworld_surface_chunk;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const DEFAULT_SEED: i64 = 12345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;

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
        Cli::HeadlessChunk {
            path,
            width,
            height,
            scene,
        } => {
            let mesh = build_scene_mesh(scene)?;
            let report = write_headless_chunk_png(
                HeadlessChunkOptions {
                    path,
                    width,
                    height,
                    color: mclone_render::default_clear_color(),
                    camera: ChunkCamera::overview_for_chunk(scene.chunk_x, scene.chunk_z),
                },
                &mesh,
            )?;
            println!(
                "headless chunk saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.vertex_count,
                report.index_count
            );
            Ok(())
        }
        Cli::Window { scene } => run_window(scene),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneOptions {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Cli {
    Window {
        scene: SceneOptions,
    },
    HeadlessClear {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    HeadlessChunk {
        path: PathBuf,
        width: u32,
        height: u32,
        scene: SceneOptions,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessMode {
    Clear(PathBuf),
    Chunk(PathBuf),
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut width = None;
        let mut height = None;
        let mut scene = SceneOptions::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--headless-clear" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-clear requires an output PNG path")?;
                    set_headless_mode(&mut mode, HeadlessMode::Clear(path))?;
                }
                "--headless-chunk" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .context("--headless-chunk requires an output PNG path")?;
                    set_headless_mode(&mut mode, HeadlessMode::Chunk(path))?;
                }
                "--width" => width = Some(parse_u32_arg("--width", args.next())?),
                "--height" => height = Some(parse_u32_arg("--height", args.next())?),
                "--seed" => scene.seed = parse_i64_arg("--seed", args.next())?,
                "--chunk-x" => scene.chunk_x = parse_i32_arg("--chunk-x", args.next())?,
                "--chunk-z" => scene.chunk_z = parse_i32_arg("--chunk-z", args.next())?,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }

        match mode {
            Some(HeadlessMode::Clear(path)) => Ok(Self::HeadlessClear {
                path,
                width: width.unwrap_or(96),
                height: height.unwrap_or(64),
            }),
            Some(HeadlessMode::Chunk(path)) => Ok(Self::HeadlessChunk {
                path,
                width: width.unwrap_or(640),
                height: height.unwrap_or(480),
                scene,
            }),
            None => Ok(Self::Window { scene }),
        }
    }
}

fn set_headless_mode(mode: &mut Option<HeadlessMode>, next: HeadlessMode) -> Result<()> {
    if mode.is_some() {
        bail!("only one headless output mode can be selected");
    }
    *mode = Some(next);
    Ok(())
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

fn parse_i32_arg(flag: &str, value: Option<String>) -> Result<i32> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i32>()
        .with_context(|| format!("{flag} requires a signed integer, got `{value}`"))
}

fn parse_i64_arg(flag: &str, value: Option<String>) -> Result<i64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i64>()
        .with_context(|| format!("{flag} requires a signed 64-bit integer, got `{value}`"))
}

fn print_help() {
    println!(
        "mclone-native-client\n\n\
         Usage:\n\
           mclone-native-client [--seed 12345] [--chunk-x 0] [--chunk-z 0]\n\
           mclone-native-client --headless-clear /tmp/mclone-native-clear.png [--width 96] [--height 64]\n\
           mclone-native-client --headless-chunk /tmp/mclone-native-chunk.png [--width 640] [--height 480] [--seed 12345] [--chunk-x 0] [--chunk-z 0]\n\n\
         Window mode renders one generated chunk with a fixed overview camera. Headless modes write PNGs for GPU validation."
    );
}

fn run_window(scene: SceneOptions) -> Result<()> {
    let mesh = build_scene_mesh(scene)?;
    let stats = mesh.stats();
    log::info!(
        "generated chunk seed={} chunk=({}, {}) mesh={} vertices {} indices",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        stats.vertex_count,
        stats.index_count
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(
        mesh,
        ChunkCamera::overview_for_chunk(scene.chunk_x, scene.chunk_z),
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn build_scene_mesh(scene: SceneOptions) -> Result<VisibleChunkMesh> {
    let chunk = generate_overworld_surface_chunk(scene.seed, scene.chunk_x, scene.chunk_z);
    let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(
        chunk.chunk_x,
        chunk.chunk_z,
        chunk.min_y,
        chunk.height,
        chunk.blocks(),
    ));
    if mesh.is_empty() {
        bail!(
            "generated chunk seed={} chunk=({}, {}) produced an empty mesh",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z
        );
    }
    Ok(mesh)
}

struct ChunkApp {
    mesh: VisibleChunkMesh,
    camera: ChunkCamera,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    draw: Option<ChunkDrawResources>,
}

impl ChunkApp {
    fn new(mesh: VisibleChunkMesh, camera: ChunkCamera) -> Self {
        Self {
            mesh,
            camera,
            window: None,
            surface: None,
            draw: None,
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for ChunkApp {
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
        let draw = match ChunkDrawResources::new(
            &surface.device,
            surface.config.format,
            surface.config.width,
            surface.config.height,
            &self.mesh,
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        log::info!("uploaded chunk mesh with {} indices", draw.index_count());
        self.draw = Some(draw);
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
                if let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) {
                    draw.resize(&surface.device, surface.config.width, surface.config.height);
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if matches!(event.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let result = {
                    let (Some(surface), Some(draw)) = (&mut self.surface, &mut self.draw) else {
                        return;
                    };
                    surface.render_with(|_device, queue, encoder, view, size| {
                        draw.render(
                            queue,
                            encoder,
                            view,
                            size,
                            self.camera,
                            mclone_render::default_clear_color(),
                        )
                    })
                };
                match result {
                    Ok(SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped) => {}
                    Ok(SurfaceFrameStatus::Reconfigured) => self.request_redraw(),
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
        assert_eq!(
            Cli::parse([]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default()
            }
        );
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

    #[test]
    fn cli_parses_headless_chunk_scene_options() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--seed".to_owned(),
            "-9".to_owned(),
            "--chunk-x".to_owned(),
            "2".to_owned(),
            "--chunk-z".to_owned(),
            "-3".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    seed: -9,
                    chunk_x: 2,
                    chunk_z: -3,
                },
            }
        );
    }
}
