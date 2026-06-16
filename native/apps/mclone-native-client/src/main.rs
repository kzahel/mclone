use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod camera;
mod cli;
mod headless;
mod perf;
mod render_cache;

#[cfg(test)]
use crate::camera::SPECTATOR_BASE_SPEED;
use crate::camera::{SPECTATOR_MOUSE_SENSITIVITY, SpectatorCamera};
use crate::cli::{Cli, HeadlessScreenshotUi, SceneOptions};
#[cfg(test)]
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, HeadlessScreenshotOptions, MovementPerfOptions,
    TimedemoOptions, parse_screenshot_ui_arg,
};
use crate::headless::{run_headless_screenshot, write_headless_chunk_scenarios};
use crate::perf::{run_frame_budget_probe, run_movement_perf_smoke, run_timedemo};
use crate::render_cache::{
    CachedTexturedRenderSections, RenderSectionCacheUpdate, RenderSectionCompileRequest,
    RenderSectionCompileWorker, SceneTexturedSections, TexturedMeshAssets,
    build_client_textured_sections, load_textured_mesh_assets,
};
#[cfg(test)]
use crate::render_cache::{extracted_asset_root, snapshot_mesh_block_state_ids};
use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::{ClientHost, ClientRuntime};
#[cfg(test)]
use mclone_core::CHUNK_SECTION_VOLUME;
use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, SECTION_HEIGHT};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh, quad_face_count_from_indices};
use mclone_net::{LocalTransport, request_server_updates};
use mclone_protocol::{ChunkInterest, SectionBlockUpdate, ServerUpdate};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, TexturedSectionDrawResources,
    TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::gui::{GuiRenderOptions, GuiRenderer};
use mclone_render::headless::{
    HeadlessChunkOptions, HeadlessClearOptions, HeadlessUiOptions, write_headless_clear_png,
    write_headless_textured_sections_png_with_options, write_headless_ui_png,
};
use mclone_render::native::{
    NativeSurfaceContext, SurfaceFrameStatus, SurfacePresentModePreference,
    surface_present_mode_label,
};
use mclone_render::target::RenderFrameContext;
use mclone_server::IntegratedServer;
use mclone_ui::{
    Button, Checkbox, Color, CycleButton, Font, GuiDrawList, GuiScale, Interaction, Point, Rect,
    Slider, WidgetId,
};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const DEFAULT_SEED: i64 = 12345;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_CHUNK_RADIUS: i32 = 1;
const MIN_UI_CHUNK_RADIUS: i32 = 1;
const MAX_CHUNK_RADIUS: i32 = 16;
const DEFAULT_MOVEMENT_PERF_STEPS: usize = 12;
const DEFAULT_MOVEMENT_PERF_PATH_RADIUS: i32 = 4;
const DEFAULT_TIMEDEMO_FRAMES: usize = 120;
const DEFAULT_TIMEDEMO_PATH_RADIUS: i32 = 4;
const DEFAULT_FRAME_BUDGET_PROBE_FRAMES: usize = 240;
const DEFAULT_FRAME_BUDGET_TARGET_HZ: f64 = 120.0;
const MAX_MOVEMENT_PERF_STEPS: usize = 512;
const MAX_TIMEDEMO_FRAMES: usize = 4096;
const MAX_FRAME_BUDGET_PROBE_FRAMES: usize = 4096;
const MAX_MOVEMENT_PERF_PATH_RADIUS: i32 = 128;
const DEFAULT_FPS_CAP: u32 = 120;
const FPS_CAPS: [u32; 6] = [60, 90, 120, 144, 165, 240];
const DEFAULT_RENDER_CHUNK_MESH_BUDGET: usize = 1;
const RENDER_NEIGHBOR_READY_DISTANCE_SQ: f32 = 24.0 * 24.0;

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
            render_options,
        } => {
            let scene_mesh = build_scene_textured_sections(&scene)?;
            let report = write_headless_textured_sections_png_with_options(
                HeadlessChunkOptions {
                    path,
                    width,
                    height,
                    color: mclone_render::default_clear_color(),
                    camera: ChunkCamera::overview_for_chunk_area(
                        scene.chunk_x,
                        scene.chunk_z,
                        scene.chunk_radius,
                    ),
                },
                &scene_mesh.sections,
                scene_mesh.atlas.as_upload(),
                render_options,
            )?;
            println!(
                "headless textured sections saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.vertex_count,
                report.index_count
            );
            Ok(())
        }
        Cli::HeadlessChunkScenarios {
            directory,
            width,
            height,
            scene,
            render_options,
        } => {
            let scene_mesh = build_scene_textured_sections(&scene)?;
            let reports = write_headless_chunk_scenarios(
                &directory,
                width,
                height,
                &scene,
                &scene_mesh,
                render_options,
            )?;
            for report in reports {
                println!(
                    "headless textured scenario saved to {} ({}x{}, {} bytes, {} vertices, {} indices)",
                    report.path.display(),
                    report.width,
                    report.height,
                    report.byte_len,
                    report.vertex_count,
                    report.index_count
                );
            }
            Ok(())
        }
        Cli::HeadlessUi {
            path,
            width,
            height,
        } => {
            let draw = render_static_title_ui(width, height);
            let report = write_headless_ui_png(
                HeadlessUiOptions {
                    path,
                    width,
                    height,
                    color: mclone_render::default_clear_color(),
                },
                &draw,
            )?;
            println!(
                "headless UI saved to {} ({}x{}, {} bytes, {} commands)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.command_count
            );
            Ok(())
        }
        Cli::HeadlessScreenshot { options } => {
            let report = run_headless_screenshot(&options)?;
            println!(
                "headless full-frame screenshot saved to {} ({}x{}, {} bytes, {} sections, {} drawn sections, {} GUI commands)",
                report.path.display(),
                report.width,
                report.height,
                report.byte_len,
                report.section_count,
                report.drawn_section_count,
                report.gui_command_count
            );
            Ok(())
        }
        Cli::MovementPerf { options } => {
            let report = run_movement_perf_smoke(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::Timedemo { options } => {
            let report = run_timedemo(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::FrameBudgetProbe { options } => {
            let report = run_frame_budget_probe(&options)?;
            report.validate()?;
            report.print_json();
            Ok(())
        }
        Cli::Window {
            scene,
            render_options,
        } => run_window(scene, render_options),
    }
}

fn run_window(scene: SceneOptions, render_options: TexturedSectionRenderOptions) -> Result<()> {
    let runtime = WindowSceneRuntime::new(&scene)?;
    log::info!(
        "native window runtime seed={} initial_center=({}, {}) radius={} remote={:?} atlas={}x{}",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.chunk_radius,
        scene.remote_addr,
        runtime.mesh_assets.atlas.width,
        runtime.mesh_assets.atlas.height
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(
        runtime,
        SpectatorCamera::spawn_for_scene(&scene),
        render_options,
        scene.chunk_radius,
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn square_count(radius: i32) -> Result<usize> {
    if radius < 0 {
        bail!("radius must be non-negative");
    }
    let side = usize::try_from(radius)
        .context("radius exceeds usize")?
        .saturating_mul(2)
        .saturating_add(1);
    Ok(side * side)
}

fn build_scene_textured_sections(scene: &SceneOptions) -> Result<SceneTexturedSections> {
    let client = build_scene_client_runtime(scene)?;
    let mesh_assets = load_textured_mesh_assets()?;
    let build = build_client_textured_sections(&client, &mesh_assets.catalog)?;
    if build.sections.is_empty() {
        bail!(
            "generated chunk area seed={} center=({}, {}) radius={} produced no textured render sections",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.chunk_radius
        );
    }
    Ok(SceneTexturedSections {
        sections: build.sections,
        visibility_graph_stats: build.visibility_graph,
        atlas: mesh_assets.atlas,
    })
}

fn build_scene_client_runtime(scene: &SceneOptions) -> Result<ClientRuntime> {
    let mut client = if scene.remote_addr.is_some() {
        ClientRuntime::new(ClientHost::RemoteDedicated)
    } else {
        ClientRuntime::local_integrated()
    };
    let radius_chunks =
        u32::try_from(scene.chunk_radius).context("chunk radius must be positive")?;

    let command = client.set_chunk_interest(ChunkInterest {
        center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
        radius_chunks,
    });

    if let Some(remote_addr) = &scene.remote_addr {
        let updates = request_server_updates(remote_addr.as_str(), &command)
            .with_context(|| format!("failed to request chunk updates from {remote_addr}"))?;
        client.apply_updates(updates);
    } else {
        let mut server = IntegratedServer::new(scene.seed);
        let mut transport = LocalTransport::new();
        transport.send_client_command(command);

        for command in transport.drain_client_commands() {
            for update in server.handle_command(command) {
                transport.send_server_update(update);
            }
        }
        for update in poll_integrated_server_until_idle(&mut server)? {
            transport.send_server_update(update);
        }
        client.apply_updates(transport.drain_server_updates());
    }
    Ok(client)
}

#[derive(Debug)]
struct WindowSceneRuntime {
    client: ClientRuntime,
    server: Option<IntegratedServer>,
    transport: LocalTransport,
    remote_addr: Option<String>,
    radius_chunks: u32,
    interest_center: ChunkPos,
    mesh_assets: TexturedMeshAssets,
    render_sections: CachedTexturedRenderSections,
    dirty_render_chunks: BTreeSet<ChunkPos>,
    dirty_render_sections: BTreeSet<RenderSectionKey>,
    inflight_render_sections: BTreeSet<RenderSectionKey>,
    render_section_revisions: BTreeMap<RenderSectionKey, u64>,
    render_compile_worker: RenderSectionCompileWorker,
    last_tick: u64,
    last_simulation_tick: u64,
    last_tick_unloads_processed: usize,
    last_simulation_block_tick_chunks: usize,
    last_simulation_entity_tick_chunks: usize,
    last_simulation_scheduler_tick_ms: f64,
    last_simulation_block_tick_ms: f64,
    last_simulation_fluid_tick_ms: f64,
    last_simulation_entity_tick_ms: f64,
    last_simulation_fluid_ticks_executed: usize,
    last_simulation_deferred_fluid_ticks: usize,
    last_simulation_fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
    last_poll_diagnostics: RuntimePollDiagnostics,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RuntimePollDiagnostics {
    flush_commands_ms: f64,
    server_tick_ms: f64,
    server_reported_total_ms: f64,
    scheduler_tick_ms: f64,
    scheduler_report_ms: f64,
    scheduler_purge_stale_tickets_ms: f64,
    scheduler_reconcile_holders_ms: f64,
    scheduler_publish_completed_ms: f64,
    scheduler_pending_unload_ms: f64,
    scheduler_apply_events_ms: f64,
    block_tick_ms: f64,
    fluid_tick_ms: f64,
    fluid_event_apply_ms: f64,
    fluid_due_scan_ms: f64,
    fluid_remove_due_ms: f64,
    fluid_tick_fluid_ms: f64,
    fluid_set_block_ms: f64,
    entity_tick_ms: f64,
    apply_updates_ms: f64,
    dirty_mark_ms: f64,
    client_apply_updates_ms: f64,
    scheduler_events: usize,
    updates: usize,
    snapshot_updates: usize,
    section_block_updates: usize,
    unload_updates: usize,
    pending_unloads_processed: usize,
    fluid_due_ticks: usize,
    fluid_executed_ticks: usize,
    fluid_deferred_ticks: usize,
    fluid_mutated_blocks: usize,
    fluid_snapshot_events: usize,
    fluid_event_count: usize,
    scheduled_fluid_ticks: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RuntimeUpdateApplyReport {
    changed: bool,
    total_ms: f64,
    dirty_mark_ms: f64,
    client_apply_updates_ms: f64,
    updates: usize,
    snapshot_updates: usize,
    section_block_updates: usize,
    unload_updates: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowRuntimeStats {
    interest_center: ChunkPos,
    loaded_chunks: usize,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    pending_render_compile_jobs: usize,
    inflight_render_sections: usize,
    client_visible_chunks: usize,
    active_ticket_chunks: usize,
    pending_unload_chunks: usize,
    block_ticking_chunks: usize,
    entity_ticking_chunks: usize,
    last_tick: u64,
    last_simulation_tick: u64,
    last_tick_unloads_processed: usize,
    last_simulation_block_tick_chunks: usize,
    last_simulation_entity_tick_chunks: usize,
    last_simulation_scheduler_tick_ms: f64,
    last_simulation_block_tick_ms: f64,
    last_simulation_fluid_tick_ms: f64,
    last_simulation_entity_tick_ms: f64,
    last_simulation_fluid_ticks_executed: usize,
    last_simulation_deferred_fluid_ticks: usize,
    last_simulation_fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
}

impl WindowSceneRuntime {
    fn new(scene: &SceneOptions) -> Result<Self> {
        let client = if scene.remote_addr.is_some() {
            ClientRuntime::new(ClientHost::RemoteDedicated)
        } else {
            ClientRuntime::local_integrated()
        };
        let radius_chunks =
            u32::try_from(scene.chunk_radius).context("chunk radius must be positive")?;
        let mesh_assets = load_textured_mesh_assets()?;
        let render_compile_worker = RenderSectionCompileWorker::new(mesh_assets.catalog.clone())?;
        let mut runtime = Self {
            client,
            server: scene
                .remote_addr
                .is_none()
                .then(|| IntegratedServer::new(scene.seed)),
            transport: LocalTransport::new(),
            remote_addr: scene.remote_addr.clone(),
            radius_chunks,
            interest_center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
            mesh_assets,
            render_sections: CachedTexturedRenderSections::default(),
            dirty_render_chunks: BTreeSet::new(),
            dirty_render_sections: BTreeSet::new(),
            inflight_render_sections: BTreeSet::new(),
            render_section_revisions: BTreeMap::new(),
            render_compile_worker,
            last_tick: 0,
            last_simulation_tick: 0,
            last_tick_unloads_processed: 0,
            last_simulation_block_tick_chunks: 0,
            last_simulation_entity_tick_chunks: 0,
            last_simulation_scheduler_tick_ms: 0.0,
            last_simulation_block_tick_ms: 0.0,
            last_simulation_fluid_tick_ms: 0.0,
            last_simulation_entity_tick_ms: 0.0,
            last_simulation_fluid_ticks_executed: 0,
            last_simulation_deferred_fluid_ticks: 0,
            last_simulation_fluid_mutated_blocks: 0,
            scheduled_fluid_ticks: 0,
            last_poll_diagnostics: RuntimePollDiagnostics::default(),
        };
        runtime.set_chunk_interest(runtime.interest_center, runtime.radius_chunks)?;
        Ok(runtime)
    }

    fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        self.set_chunk_interest(center, self.radius_chunks)
    }

    fn set_radius_chunks(&mut self, radius_chunks: u32) -> Result<bool> {
        self.set_chunk_interest(self.interest_center, radius_chunks)
    }

    fn set_chunk_interest(&mut self, center: ChunkPos, radius_chunks: u32) -> Result<bool> {
        if self.client.chunk_interest().is_some_and(|interest| {
            interest.center == center && interest.radius_chunks == radius_chunks
        }) {
            self.interest_center = center;
            self.radius_chunks = radius_chunks;
            return Ok(false);
        }

        self.interest_center = center;
        self.radius_chunks = radius_chunks;
        let command = self.client.set_chunk_interest(ChunkInterest {
            center,
            radius_chunks,
        });

        if let Some(remote_addr) = &self.remote_addr {
            let updates = request_server_updates(remote_addr.as_str(), &command)
                .with_context(|| format!("failed to request chunk updates from {remote_addr}"))?;
            let changed = !updates.is_empty();
            self.apply_server_updates(updates);
            return Ok(changed);
        }

        self.transport.send_client_command(command);
        self.flush_local_commands()
    }

    fn poll(&mut self) -> Result<bool> {
        let mut diagnostics = RuntimePollDiagnostics::default();
        let flush_start = Instant::now();
        let mut changed = self.flush_local_commands()?;
        diagnostics.flush_commands_ms = elapsed_ms(flush_start.elapsed());
        if let Some(server) = &mut self.server {
            let server_tick_start = Instant::now();
            let report = server
                .try_simulation_tick_report()
                .context("failed to tick integrated server")?;
            diagnostics.server_tick_ms = elapsed_ms(server_tick_start.elapsed());
            diagnostics.server_reported_total_ms = micros_to_ms(report.timing.total_us);
            diagnostics.scheduler_tick_ms = micros_to_ms(report.timing.scheduler_tick_us);
            diagnostics.scheduler_report_ms = micros_to_ms(report.timing.scheduler_report_us);
            diagnostics.scheduler_purge_stale_tickets_ms =
                micros_to_ms(report.timing.scheduler_purge_stale_tickets_us);
            diagnostics.scheduler_reconcile_holders_ms =
                micros_to_ms(report.timing.scheduler_reconcile_holders_us);
            diagnostics.scheduler_publish_completed_ms =
                micros_to_ms(report.timing.scheduler_publish_completed_us);
            diagnostics.scheduler_pending_unload_ms =
                micros_to_ms(report.timing.scheduler_pending_unload_us);
            diagnostics.scheduler_apply_events_ms =
                micros_to_ms(report.timing.scheduler_apply_events_us);
            diagnostics.block_tick_ms = micros_to_ms(report.timing.block_tick_us);
            diagnostics.fluid_tick_ms = micros_to_ms(report.timing.fluid_tick_us);
            diagnostics.fluid_event_apply_ms = micros_to_ms(report.timing.fluid_event_apply_us);
            diagnostics.fluid_due_scan_ms = micros_to_ms(report.timing.fluid_due_scan_us);
            diagnostics.fluid_remove_due_ms = micros_to_ms(report.timing.fluid_remove_due_us);
            diagnostics.fluid_tick_fluid_ms = micros_to_ms(report.timing.fluid_tick_fluid_us);
            diagnostics.fluid_set_block_ms = micros_to_ms(report.timing.fluid_set_block_us);
            diagnostics.entity_tick_ms = micros_to_ms(report.timing.entity_tick_us);
            diagnostics.scheduler_events = report.scheduler_event_count;
            diagnostics.pending_unloads_processed = report.pending_unloads_processed;
            diagnostics.fluid_due_ticks = report.fluid_due_ticks;
            diagnostics.fluid_executed_ticks = report.fluid_ticks_executed;
            diagnostics.fluid_deferred_ticks = report.deferred_fluid_ticks;
            diagnostics.fluid_mutated_blocks = report.fluid_mutated_blocks;
            diagnostics.fluid_snapshot_events = report.fluid_snapshot_events;
            diagnostics.fluid_event_count = report.fluid_event_count;
            diagnostics.scheduled_fluid_ticks = report.scheduled_fluid_ticks;
            self.last_tick = report.chunk_tick;
            self.last_simulation_tick = report.simulation_tick;
            self.last_tick_unloads_processed = report.pending_unloads_processed;
            self.last_simulation_block_tick_chunks = report.block_tick_chunks;
            self.last_simulation_entity_tick_chunks = report.entity_tick_chunks;
            self.last_simulation_scheduler_tick_ms = micros_to_ms(report.timing.scheduler_tick_us);
            self.last_simulation_block_tick_ms = micros_to_ms(report.timing.block_tick_us);
            self.last_simulation_fluid_tick_ms = micros_to_ms(report.timing.fluid_tick_us);
            self.last_simulation_entity_tick_ms = micros_to_ms(report.timing.entity_tick_us);
            self.last_simulation_fluid_ticks_executed = report.fluid_ticks_executed;
            self.last_simulation_deferred_fluid_ticks = report.deferred_fluid_ticks;
            self.last_simulation_fluid_mutated_blocks = report.fluid_mutated_blocks;
            self.scheduled_fluid_ticks = report.scheduled_fluid_ticks;
            let apply_report = self.apply_server_updates_report(report.updates);
            diagnostics.apply_updates_ms = apply_report.total_ms;
            diagnostics.dirty_mark_ms = apply_report.dirty_mark_ms;
            diagnostics.client_apply_updates_ms = apply_report.client_apply_updates_ms;
            diagnostics.updates = apply_report.updates;
            diagnostics.snapshot_updates = apply_report.snapshot_updates;
            diagnostics.section_block_updates = apply_report.section_block_updates;
            diagnostics.unload_updates = apply_report.unload_updates;
            changed |= apply_report.changed;
        }
        self.last_poll_diagnostics = diagnostics;
        Ok(changed)
    }

    fn flush_local_commands(&mut self) -> Result<bool> {
        let Some(server) = &mut self.server else {
            return Ok(false);
        };
        let mut changed = false;
        for command in self.transport.drain_client_commands() {
            for update in server.handle_command(command) {
                self.transport.send_server_update(update);
            }
        }
        let updates = self.transport.drain_server_updates();
        changed |= self.apply_server_updates(updates);
        Ok(changed)
    }

    fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> bool {
        self.apply_server_updates_report(updates).changed
    }

    fn apply_server_updates_report(
        &mut self,
        updates: Vec<ServerUpdate>,
    ) -> RuntimeUpdateApplyReport {
        let total_start = Instant::now();
        let changed = !updates.is_empty();
        let update_count = updates.len();
        let mut snapshot_updates = 0;
        let mut section_block_updates = 0;
        let mut unload_updates = 0;
        let dirty_mark_start = Instant::now();
        for update in &updates {
            match update {
                ServerUpdate::ChunkSnapshot(snapshot) => {
                    snapshot_updates += 1;
                    self.mark_render_chunk_dirty(snapshot.pos);
                }
                ServerUpdate::ChunkUnload { pos } => {
                    unload_updates += 1;
                    self.mark_render_chunk_dirty(*pos);
                }
                ServerUpdate::SectionBlockUpdates {
                    pos,
                    section_y,
                    updates,
                } => {
                    section_block_updates += 1;
                    self.mark_render_section_updates_dirty(*pos, *section_y, updates);
                }
            }
        }
        let dirty_mark_ms = elapsed_ms(dirty_mark_start.elapsed());
        let client_apply_start = Instant::now();
        self.client.apply_updates(updates);
        let client_apply_updates_ms = elapsed_ms(client_apply_start.elapsed());
        RuntimeUpdateApplyReport {
            changed,
            total_ms: elapsed_ms(total_start.elapsed()),
            dirty_mark_ms,
            client_apply_updates_ms,
            updates: update_count,
            snapshot_updates,
            section_block_updates,
            unload_updates,
        }
    }

    fn mark_render_chunk_dirty(&mut self, pos: ChunkPos) {
        for dirty_pos in render_dirty_chunk_neighborhood(pos) {
            self.bump_known_render_chunk_section_revisions(dirty_pos);
            self.dirty_render_chunks.insert(dirty_pos);
        }
    }

    fn mark_render_section_updates_dirty(
        &mut self,
        pos: ChunkPos,
        section_y: i32,
        updates: &[SectionBlockUpdate],
    ) {
        for update in updates {
            for key in render_dirty_section_keys_for_block_update(pos, section_y, update) {
                self.bump_render_section_revision(key);
                self.dirty_render_sections.insert(key);
            }
        }
    }

    fn bump_known_render_chunk_section_revisions(&mut self, pos: ChunkPos) {
        let keys = self.known_render_section_keys_for_chunk(pos);
        for key in keys {
            self.bump_render_section_revision(key);
        }
    }

    fn known_render_section_keys_for_chunk(&self, pos: ChunkPos) -> BTreeSet<RenderSectionKey> {
        let mut keys = BTreeSet::new();
        if let Some(snapshot) = self.client.chunk_snapshot(pos) {
            keys.extend(render_section_keys_for_snapshot(snapshot));
        }
        keys.extend(
            self.render_sections
                .section_keys()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.dirty_render_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.inflight_render_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys
    }

    fn bump_render_section_revision(&mut self, key: RenderSectionKey) {
        let revision = self.render_section_revisions.entry(key).or_default();
        *revision = revision.wrapping_add(1);
    }

    fn render_section_revision(&self, key: RenderSectionKey) -> u64 {
        self.render_section_revisions
            .get(&key)
            .copied()
            .unwrap_or_default()
    }

    fn drain_completed_render_compile_jobs(&mut self) -> Result<RenderSectionCacheUpdate> {
        let mut update = RenderSectionCacheUpdate::default();
        for completed in self.render_compile_worker.try_recv_completed()? {
            for key in &completed.target_sections {
                self.inflight_render_sections.remove(key);
            }

            let mut accepted_sections = BTreeSet::new();
            let mut stale_sections = BTreeSet::new();
            for key in &completed.target_sections {
                let submitted_revision = completed
                    .section_revisions
                    .get(key)
                    .copied()
                    .unwrap_or_default();
                if self.render_section_revision(*key) == submitted_revision {
                    accepted_sections.insert(*key);
                } else {
                    stale_sections.insert(*key);
                }
            }

            if !stale_sections.is_empty() {
                update.stale_compile_section_count += stale_sections.len();
                self.requeue_stale_render_sections(stale_sections);
            }
            if accepted_sections.is_empty() {
                continue;
            }

            let build_report = completed.result.map_err(anyhow::Error::msg)?;
            let completed_update = self.render_sections.apply_build_report(
                &accepted_sections,
                build_report,
                &BTreeSet::new(),
                &BTreeSet::new(),
            );
            update.merge(completed_update);
        }
        update.pending_compile_jobs = self.render_compile_worker.pending_job_count();
        Ok(update)
    }

    fn requeue_stale_render_sections(&mut self, target_sections: BTreeSet<RenderSectionKey>) {
        for key in target_sections {
            let pos = render_section_chunk_pos(key);
            let snapshot_contains = self
                .client
                .chunk_snapshot(pos)
                .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key));
            if snapshot_contains || self.render_sections.contains_section(key) {
                self.dirty_render_sections.insert(key);
            }
        }
    }

    fn submit_render_compile_job(
        &mut self,
        ready_section_keys: BTreeSet<RenderSectionKey>,
    ) -> Result<usize> {
        if ready_section_keys.is_empty() {
            return Ok(0);
        }
        let submitted_count = ready_section_keys.len();
        let snapshots = self.client.chunk_snapshots().cloned().collect();
        let section_revisions = ready_section_keys
            .iter()
            .map(|key| (*key, self.render_section_revision(*key)))
            .collect();
        self.render_compile_worker
            .submit(RenderSectionCompileRequest {
                target_sections: ready_section_keys.clone(),
                section_revisions,
                snapshots,
            })?;
        self.inflight_render_sections.extend(ready_section_keys);
        Ok(submitted_count)
    }

    fn sync_render_sections(&mut self, camera_position: Vec3) -> Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_with_budget(camera_position, DEFAULT_RENDER_CHUNK_MESH_BUDGET)
    }

    fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut combined = RenderSectionCacheUpdate::default();
        loop {
            let update = self.sync_render_sections_with_budget(camera_position, usize::MAX)?;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || update.submitted_compile_section_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            combined.merge(update);
            if self.render_compile_worker.pending_job_count() == 0
                && !self.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(combined);
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for render section compile queue");
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn sync_render_sections_with_budget(
        &mut self,
        camera_position: Vec3,
        chunk_budget: usize,
    ) -> Result<RenderSectionCacheUpdate> {
        let mut report = self.drain_completed_render_compile_jobs()?;

        if self.render_sections.is_empty()
            && self.client.loaded_chunk_count() > 0
            && self.dirty_render_chunks.is_empty()
            && self.dirty_render_sections.is_empty()
            && self.inflight_render_sections.is_empty()
        {
            let loaded = self
                .client
                .chunk_snapshots()
                .map(|snapshot| snapshot.pos)
                .collect::<Vec<_>>();
            for pos in loaded {
                self.mark_render_chunk_dirty(pos);
            }
        }

        if self.dirty_render_chunks.is_empty() && self.dirty_render_sections.is_empty() {
            report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
            return Ok(report);
        }

        let mut stale_dirty_chunks = BTreeSet::new();
        let mut loaded_dirty_chunks = BTreeSet::new();
        let mut removal_dirty_chunks = BTreeSet::new();
        for pos in self.dirty_render_chunks.iter().copied() {
            if self.client.chunk_snapshot(pos).is_some() {
                loaded_dirty_chunks.insert(pos);
            } else if self.render_sections.contains_chunk(pos) {
                removal_dirty_chunks.insert(pos);
            } else {
                stale_dirty_chunks.insert(pos);
            }
        }

        let mut stale_dirty_sections = BTreeSet::new();
        let mut loaded_dirty_sections_by_chunk: BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>> =
            BTreeMap::new();
        let mut removal_dirty_sections = BTreeSet::new();
        for key in self.dirty_render_sections.iter().copied() {
            let pos = render_section_chunk_pos(key);
            if self
                .client
                .chunk_snapshot(pos)
                .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            {
                loaded_dirty_sections_by_chunk
                    .entry(pos)
                    .or_default()
                    .insert(key);
            } else if self.render_sections.contains_section(key) {
                removal_dirty_sections.insert(key);
            } else {
                stale_dirty_sections.insert(key);
            }
        }

        for pos in stale_dirty_chunks {
            self.dirty_render_chunks.remove(&pos);
        }
        for key in stale_dirty_sections {
            self.dirty_render_sections.remove(&key);
        }

        if !removal_dirty_chunks.is_empty() || !removal_dirty_sections.is_empty() {
            let removal_update = self
                .render_sections
                .remove_sections(&removal_dirty_chunks, &removal_dirty_sections);
            report.merge(removal_update);
            for pos in &removal_dirty_chunks {
                self.dirty_render_chunks.remove(pos);
                self.dirty_render_sections
                    .retain(|key| render_section_chunk_pos(*key) != *pos);
                self.inflight_render_sections
                    .retain(|key| render_section_chunk_pos(*key) != *pos);
            }
            for key in &removal_dirty_sections {
                self.dirty_render_sections.remove(key);
                self.inflight_render_sections.remove(key);
            }
        }

        if chunk_budget == 0 || self.render_compile_worker.pending_job_count() > 0 {
            report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
            return Ok(report);
        }

        let budgeted_loaded_chunks =
            sort_chunk_positions_by_distance(loaded_dirty_chunks.iter().copied(), camera_position)
                .into_iter()
                .take(chunk_budget)
                .collect::<BTreeSet<_>>();
        let remaining_chunk_budget = chunk_budget.saturating_sub(budgeted_loaded_chunks.len());
        let budgeted_dirty_section_chunks =
            sort_dirty_section_chunks_by_distance(&loaded_dirty_sections_by_chunk, camera_position)
                .into_iter()
                .filter(|pos| !budgeted_loaded_chunks.contains(pos))
                .take(remaining_chunk_budget)
                .collect::<BTreeSet<_>>();
        let mut ready_section_keys = BTreeSet::new();
        let mut deferred_section_keys = BTreeSet::new();
        let mut near_exception_section_count = 0;
        let mut deferred_section_count = 0;
        for pos in &budgeted_loaded_chunks {
            let Some(snapshot) = self.client.chunk_snapshot(*pos) else {
                continue;
            };
            for key in render_section_keys_for_snapshot(snapshot) {
                if self.inflight_render_sections.contains(&key) {
                    deferred_section_keys.insert(key);
                    continue;
                }
                let readiness =
                    render_section_neighbor_readiness(&self.client, key, camera_position);
                if readiness.is_ready() {
                    if readiness.is_near_exception() {
                        near_exception_section_count += 1;
                    }
                    ready_section_keys.insert(key);
                } else {
                    deferred_section_count += 1;
                    deferred_section_keys.insert(key);
                }
            }
        }
        for pos in &budgeted_dirty_section_chunks {
            if let Some(keys) = loaded_dirty_sections_by_chunk.get(pos) {
                for key in keys {
                    if self.inflight_render_sections.contains(key) {
                        deferred_section_keys.insert(*key);
                        continue;
                    }
                    let readiness =
                        render_section_neighbor_readiness(&self.client, *key, camera_position);
                    if readiness.is_ready() {
                        if readiness.is_near_exception() {
                            near_exception_section_count += 1;
                        }
                        ready_section_keys.insert(*key);
                    } else {
                        deferred_section_count += 1;
                        deferred_section_keys.insert(*key);
                    }
                }
            }
        }
        if ready_section_keys.is_empty() {
            for pos in budgeted_loaded_chunks {
                self.dirty_render_chunks.remove(&pos);
            }
            self.dirty_render_sections.extend(deferred_section_keys);
            report.merge(RenderSectionCacheUpdate {
                deferred_section_count,
                near_exception_section_count,
                ..RenderSectionCacheUpdate::default()
            });
            report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
            return Ok(report);
        }

        let submitted_compile_section_count =
            self.submit_render_compile_job(ready_section_keys.clone())?;
        report.merge(RenderSectionCacheUpdate {
            deferred_section_count,
            near_exception_section_count,
            submitted_compile_section_count,
            ..RenderSectionCacheUpdate::default()
        });

        for pos in budgeted_loaded_chunks {
            self.dirty_render_chunks.remove(&pos);
        }
        for key in &ready_section_keys {
            self.dirty_render_sections.remove(key);
        }
        self.dirty_render_sections.extend(deferred_section_keys);
        report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
        Ok(report)
    }

    fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.render_sections.sections()
    }

    fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.render_sections
            .section_keys()
            .filter(|key| {
                render_section_neighbor_readiness(&self.client, *key, camera_position).is_ready()
            })
            .collect()
    }

    fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.render_compile_worker.pending_job_count() > 0
            || self.has_ready_pending_render_work(camera_position)
    }

    fn pending_render_chunk_count(&self) -> usize {
        let mut pending = self
            .dirty_render_chunks
            .iter()
            .filter(|pos| {
                self.client.chunk_snapshot(**pos).is_some()
                    || self.render_sections.contains_chunk(**pos)
            })
            .copied()
            .collect::<BTreeSet<_>>();
        pending.extend(
            self.dirty_render_sections
                .iter()
                .copied()
                .filter(|key| {
                    let pos = render_section_chunk_pos(*key);
                    self.client.chunk_snapshot(pos).is_some()
                        || self.render_sections.contains_section(*key)
                })
                .map(render_section_chunk_pos),
        );
        pending.extend(
            self.inflight_render_sections
                .iter()
                .copied()
                .map(render_section_chunk_pos),
        );
        pending.len()
    }

    fn has_ready_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.dirty_render_chunks.iter().copied().any(|pos| {
            if self.render_sections.contains_chunk(pos) && self.client.chunk_snapshot(pos).is_none()
            {
                return true;
            }
            let Some(snapshot) = self.client.chunk_snapshot(pos) else {
                return false;
            };
            render_section_keys_for_snapshot(snapshot)
                .into_iter()
                .any(|key| {
                    !self.inflight_render_sections.contains(&key)
                        && render_section_neighbor_readiness(&self.client, key, camera_position)
                            .is_ready()
                })
        }) || self.dirty_render_sections.iter().copied().any(|key| {
            if self.inflight_render_sections.contains(&key) {
                return false;
            }
            if self.render_sections.contains_section(key)
                && self
                    .client
                    .chunk_snapshot(render_section_chunk_pos(key))
                    .is_none()
            {
                return true;
            }
            self.client
                .chunk_snapshot(render_section_chunk_pos(key))
                .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
                && render_section_neighbor_readiness(&self.client, key, camera_position).is_ready()
        })
    }

    fn pending_job_count(&self) -> usize {
        self.server
            .as_ref()
            .map_or(0, IntegratedServer::pending_job_count)
    }

    fn pending_publication_count(&self) -> usize {
        self.server
            .as_ref()
            .map_or(0, IntegratedServer::pending_publication_count)
    }

    fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.last_poll_diagnostics
    }

    fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(state_id) = self.block_state_at_position(position) else {
            return false;
        };
        self.mesh_assets.catalog.occludes(state_id)
    }

    fn block_state_at_position(&self, position: Vec3) -> Option<mclone_core::BlockStateId> {
        if !position.is_finite() {
            return None;
        }
        self.block_state_at_world(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        )
    }

    fn block_state_at_world(
        &self,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> Option<mclone_core::BlockStateId> {
        let pos = ChunkPos::new(
            world_x.div_euclid(CHUNK_WIDTH),
            world_z.div_euclid(CHUNK_WIDTH),
        );
        self.client
            .chunk_snapshot(pos)
            .and_then(|snapshot| snapshot_block_state_at_world(snapshot, world_x, world_y, world_z))
    }

    fn stats(&self) -> WindowRuntimeStats {
        let scheduler_metrics = self
            .server
            .as_ref()
            .map(|server| server.scheduler().metrics());
        WindowRuntimeStats {
            interest_center: self.interest_center,
            loaded_chunks: self.client.loaded_chunk_count(),
            pending_jobs: self.pending_job_count(),
            pending_publications: self.pending_publication_count(),
            pending_render_chunks: self.pending_render_chunk_count(),
            pending_render_compile_jobs: self.render_compile_worker.pending_job_count(),
            inflight_render_sections: self.inflight_render_sections.len(),
            client_visible_chunks: scheduler_metrics
                .map_or(self.client.loaded_chunk_count(), |metrics| {
                    metrics.client_visible_chunks
                }),
            active_ticket_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.active_ticket_chunks),
            pending_unload_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.pending_unload_chunks),
            block_ticking_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.block_ticking_chunks),
            entity_ticking_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.entity_ticking_status_chunks),
            last_tick: self.last_tick,
            last_simulation_tick: self.last_simulation_tick,
            last_tick_unloads_processed: self.last_tick_unloads_processed,
            last_simulation_block_tick_chunks: self.last_simulation_block_tick_chunks,
            last_simulation_entity_tick_chunks: self.last_simulation_entity_tick_chunks,
            last_simulation_scheduler_tick_ms: self.last_simulation_scheduler_tick_ms,
            last_simulation_block_tick_ms: self.last_simulation_block_tick_ms,
            last_simulation_fluid_tick_ms: self.last_simulation_fluid_tick_ms,
            last_simulation_entity_tick_ms: self.last_simulation_entity_tick_ms,
            last_simulation_fluid_ticks_executed: self.last_simulation_fluid_ticks_executed,
            last_simulation_deferred_fluid_ticks: self.last_simulation_deferred_fluid_ticks,
            last_simulation_fluid_mutated_blocks: self.last_simulation_fluid_mutated_blocks,
            scheduled_fluid_ticks: self.scheduled_fluid_ticks,
        }
    }
}

fn poll_integrated_server_until_idle(server: &mut IntegratedServer) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            server
                .try_poll()
                .context("failed to poll integrated server worldgen jobs")?,
        );
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for integrated server worldgen jobs");
        }
        if server.pending_publication_count() == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn poll_window_runtime_until_idle(runtime: &mut WindowSceneRuntime) -> Result<(usize, f64)> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut polls = 0_usize;
    let mut poll_ms = 0.0_f64;
    loop {
        let poll_start = Instant::now();
        runtime.poll()?;
        poll_ms += elapsed_ms(poll_start.elapsed());
        polls += 1;
        if runtime.pending_job_count() == 0 {
            return Ok((polls, poll_ms));
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for window runtime worldgen jobs");
        }
        if runtime.pending_publication_count() == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl SceneOptions {
    #[cfg(test)]
    fn chunk_positions(&self) -> impl Iterator<Item = (i32, i32)> {
        let min_x = self.chunk_x - self.chunk_radius;
        let max_x = self.chunk_x + self.chunk_radius;
        let min_z = self.chunk_z - self.chunk_radius;
        let max_z = self.chunk_z + self.chunk_radius;
        (min_x..=max_x)
            .flat_map(move |chunk_x| (min_z..=max_z).map(move |chunk_z| (chunk_x, chunk_z)))
    }
}

fn snapshot_block_state_at_world(
    snapshot: &ChunkSnapshot,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<mclone_core::BlockStateId> {
    let chunk_x = world_x.div_euclid(CHUNK_WIDTH);
    let chunk_z = world_z.div_euclid(CHUNK_WIDTH);
    if snapshot.pos != ChunkPos::new(chunk_x, chunk_z) {
        return None;
    }

    let local_y = world_y - snapshot.min_y;
    if !(0..snapshot.height).contains(&local_y) {
        return None;
    }

    let section_y = world_y.div_euclid(SECTION_HEIGHT);
    let local_x = world_x.rem_euclid(CHUNK_WIDTH);
    let local_z = world_z.rem_euclid(CHUNK_WIDTH);
    let local_section_y = world_y.rem_euclid(SECTION_HEIGHT);
    let index = mclone_core::chunk_section_index(local_x, local_section_y, local_z);
    Some(
        snapshot
            .sections
            .iter()
            .find(|section| section.section_y == section_y)
            .map(|section| section.unpack_block_state_ids()[index])
            .unwrap_or(AIR_BLOCK_STATE_ID),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderNeighborReadiness {
    ReadyWithNeighbors,
    ReadyNearCamera,
    DeferredMissingNeighbors,
}

impl RenderNeighborReadiness {
    const fn is_ready(self) -> bool {
        matches!(self, Self::ReadyWithNeighbors | Self::ReadyNearCamera)
    }

    const fn is_near_exception(self) -> bool {
        matches!(self, Self::ReadyNearCamera)
    }
}

fn render_section_keys_for_snapshot(snapshot: &ChunkSnapshot) -> Vec<RenderSectionKey> {
    let min_section_y = snapshot.min_y.div_euclid(SECTION_HEIGHT);
    let section_count = snapshot.height / SECTION_HEIGHT;
    (0..section_count)
        .map(|offset| RenderSectionKey::new(snapshot.pos.x, min_section_y + offset, snapshot.pos.z))
        .collect()
}

fn snapshot_contains_render_section(snapshot: &ChunkSnapshot, key: RenderSectionKey) -> bool {
    snapshot.pos == render_section_chunk_pos(key)
        && key.section_y * SECTION_HEIGHT >= snapshot.min_y
        && key.section_y * SECTION_HEIGHT < snapshot.min_y + snapshot.height
}

fn render_section_chunk_pos(key: RenderSectionKey) -> ChunkPos {
    ChunkPos::new(key.chunk_x, key.chunk_z)
}

fn render_dirty_section_keys_for_block_update(
    pos: ChunkPos,
    section_y: i32,
    update: &SectionBlockUpdate,
) -> BTreeSet<RenderSectionKey> {
    let world_x = pos.x * CHUNK_WIDTH + update.local_x as i32;
    let world_y = section_y * SECTION_HEIGHT + update.local_y as i32;
    let world_z = pos.z * CHUNK_WIDTH + update.local_z as i32;
    let mut keys = BTreeSet::new();
    for z in world_z - 1..=world_z + 1 {
        for x in world_x - 1..=world_x + 1 {
            for y in world_y - 1..=world_y + 1 {
                keys.insert(RenderSectionKey::new(
                    render_section_coord_from_block(x),
                    y.div_euclid(SECTION_HEIGHT),
                    render_section_coord_from_block(z),
                ));
            }
        }
    }
    keys
}

fn render_section_coord_from_block(block_coord: i32) -> i32 {
    block_coord.div_euclid(CHUNK_WIDTH)
}

fn render_section_neighbor_readiness(
    client: &ClientRuntime,
    key: RenderSectionKey,
    camera_position: Vec3,
) -> RenderNeighborReadiness {
    if render_section_distance_sq(key, camera_position) <= RENDER_NEIGHBOR_READY_DISTANCE_SQ {
        return RenderNeighborReadiness::ReadyNearCamera;
    }
    if has_horizontal_neighbor_snapshots(client, ChunkPos::new(key.chunk_x, key.chunk_z)) {
        RenderNeighborReadiness::ReadyWithNeighbors
    } else {
        RenderNeighborReadiness::DeferredMissingNeighbors
    }
}

fn render_section_distance_sq(key: RenderSectionKey, camera_position: Vec3) -> f32 {
    let center = render_section_center(key);
    center.distance_squared(camera_position)
}

fn sort_chunk_positions_by_distance(
    positions: impl IntoIterator<Item = ChunkPos>,
    camera_position: Vec3,
) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by(|left, right| {
        render_chunk_distance_sq(*left, camera_position)
            .total_cmp(&render_chunk_distance_sq(*right, camera_position))
            .then_with(|| left.x.cmp(&right.x))
            .then_with(|| left.z.cmp(&right.z))
    });
    positions
}

fn sort_dirty_section_chunks_by_distance(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    camera_position: Vec3,
) -> Vec<ChunkPos> {
    let mut positions = sections_by_chunk.keys().copied().collect::<Vec<_>>();
    positions.sort_by(|left, right| {
        dirty_section_chunk_distance_sq(sections_by_chunk, *left, camera_position)
            .total_cmp(&dirty_section_chunk_distance_sq(
                sections_by_chunk,
                *right,
                camera_position,
            ))
            .then_with(|| left.x.cmp(&right.x))
            .then_with(|| left.z.cmp(&right.z))
    });
    positions
}

fn dirty_section_chunk_distance_sq(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    pos: ChunkPos,
    camera_position: Vec3,
) -> f32 {
    sections_by_chunk
        .get(&pos)
        .and_then(|keys| {
            keys.iter()
                .map(|key| render_section_distance_sq(*key, camera_position))
                .min_by(f32::total_cmp)
        })
        .unwrap_or_else(|| render_chunk_distance_sq(pos, camera_position))
}

fn render_chunk_distance_sq(pos: ChunkPos, camera_position: Vec3) -> f32 {
    let center = Vec3::new(
        render_chunk_world_origin(pos.x) as f32 + CHUNK_WIDTH as f32 * 0.5,
        camera_position.y,
        render_chunk_world_origin(pos.z) as f32 + CHUNK_WIDTH as f32 * 0.5,
    );
    center.distance_squared(camera_position)
}

fn render_section_center(key: RenderSectionKey) -> Vec3 {
    Vec3::new(
        render_chunk_world_origin(key.chunk_x) as f32 + CHUNK_WIDTH as f32 * 0.5,
        (key.section_y * SECTION_HEIGHT) as f32 + SECTION_HEIGHT as f32 * 0.5,
        render_chunk_world_origin(key.chunk_z) as f32 + CHUNK_WIDTH as f32 * 0.5,
    )
}

fn has_horizontal_neighbor_snapshots(client: &ClientRuntime, pos: ChunkPos) -> bool {
    [
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
    .into_iter()
    .all(|neighbor| client.chunk_snapshot(neighbor).is_some())
}

fn render_chunk_world_origin(chunk_coord: i32) -> i32 {
    chunk_coord * CHUNK_WIDTH
}

fn render_dirty_chunk_neighborhood(pos: ChunkPos) -> [ChunkPos; 5] {
    [
        pos,
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RenderStreamStats {
    section_count: usize,
    drawn_section_count: usize,
    face_count: u32,
    drawn_face_count: u32,
    index_count: u32,
    drawn_index_count: u32,
    last_rebuilt_section_count: usize,
    last_removed_section_count: usize,
    last_rebuilt_vertex_count: u32,
    last_rebuilt_face_count: u32,
    last_rebuilt_index_count: u32,
    last_neighbor_ready_section_count: usize,
    last_near_exception_section_count: usize,
    last_deferred_section_count: usize,
    last_submitted_compile_section_count: usize,
    last_completed_compile_section_count: usize,
    last_stale_compile_section_count: usize,
    last_pending_compile_jobs: usize,
    last_visibility_graph_build_count: usize,
    last_visibility_graph_total_ms: f64,
    last_visibility_graph_worst_ms: f64,
    last_uploaded_section_count: usize,
    last_upload_removed_section_count: usize,
    last_uploaded_vertex_count: u32,
    last_uploaded_face_count: u32,
    last_uploaded_index_count: u32,
    last_remesh_ms: f64,
    last_upload_ms: f64,
    last_frame_ms: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum FramePacingMode {
    #[default]
    Vsync,
    Capped,
    Uncapped,
}

impl FramePacingMode {
    fn label(self) -> &'static str {
        match self {
            Self::Vsync => "VSync",
            Self::Capped => "Max FPS",
            Self::Uncapped => "Uncapped",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Vsync => Self::Capped,
            Self::Capped => Self::Uncapped,
            Self::Uncapped => Self::Vsync,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FramePacing {
    mode: FramePacingMode,
    fps_cap: u32,
    monitor_name: Option<String>,
    monitor_refresh_hz: Option<f32>,
    active_present_mode_label: &'static str,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
            monitor_name: None,
            monitor_refresh_hz: None,
            active_present_mode_label: "fifo",
        }
    }
}

impl FramePacing {
    fn present_mode_preference(&self) -> SurfacePresentModePreference {
        match self.mode {
            FramePacingMode::Vsync => SurfacePresentModePreference::Vsync,
            FramePacingMode::Capped | FramePacingMode::Uncapped => {
                SurfacePresentModePreference::NoVsync
            }
        }
    }

    fn target_frame_duration(&self) -> Option<Duration> {
        if self.mode != FramePacingMode::Capped {
            return None;
        }
        Some(Duration::from_secs_f64(1.0 / self.fps_cap.max(1) as f64))
    }

    fn target_frame_ms(&self) -> Option<f64> {
        match self.mode {
            FramePacingMode::Vsync => self
                .monitor_refresh_hz
                .filter(|refresh_hz| *refresh_hz > 1.0)
                .map(|refresh_hz| 1000.0 / refresh_hz as f64),
            FramePacingMode::Capped => Some(1000.0 / self.fps_cap.max(1) as f64),
            FramePacingMode::Uncapped => None,
        }
    }

    fn update_monitor(&mut self, window: &Window) {
        if let Some(monitor) = window.current_monitor() {
            self.monitor_name = monitor.name();
            self.monitor_refresh_hz = monitor
                .refresh_rate_millihertz()
                .map(|millihertz| millihertz as f32 / 1000.0);
        } else {
            self.monitor_name = None;
            self.monitor_refresh_hz = None;
        }
    }

    fn apply_to_surface(&mut self, surface: &mut NativeSurfaceContext) {
        let active = surface.set_present_mode_preference(self.present_mode_preference());
        self.active_present_mode_label = surface_present_mode_label(active);
    }

    fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
    }

    fn cycle_fps_cap(&mut self) {
        let current = FPS_CAPS
            .iter()
            .position(|cap| *cap == self.fps_cap)
            .unwrap_or_else(|| {
                FPS_CAPS
                    .iter()
                    .position(|cap| *cap >= self.fps_cap)
                    .unwrap_or(FPS_CAPS.len() - 1)
            });
        self.fps_cap = FPS_CAPS[(current + 1) % FPS_CAPS.len()];
    }

    fn ui_state(&self) -> FramePacingUiState {
        FramePacingUiState {
            mode: self.mode,
            fps_cap: self.fps_cap,
        }
    }

    fn debug_stats(&self) -> FramePacingDebugStats {
        FramePacingDebugStats {
            mode: self.mode,
            fps_cap: self.fps_cap,
            monitor_refresh_hz: self.monitor_refresh_hz,
            target_frame_ms: self.target_frame_ms(),
            active_present_mode_label: self.active_present_mode_label,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FramePacingUiState {
    mode: FramePacingMode,
    fps_cap: u32,
}

impl Default for FramePacingUiState {
    fn default() -> Self {
        Self {
            mode: FramePacingMode::Vsync,
            fps_cap: DEFAULT_FPS_CAP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FramePacingDebugStats {
    mode: FramePacingMode,
    fps_cap: u32,
    monitor_refresh_hz: Option<f32>,
    target_frame_ms: Option<f64>,
    active_present_mode_label: &'static str,
}

impl Default for FramePacingDebugStats {
    fn default() -> Self {
        FramePacing::default().debug_stats()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FrameTimingStats {
    frame_count: u64,
    over_budget_count: u64,
    over_2x_budget_count: u64,
    over_4x_budget_count: u64,
    last_frame_ms: f64,
    worst_frame_ms: f64,
    budget_ms: Option<f64>,
    last_runtime_poll_ms: f64,
    last_remesh_ms: f64,
    last_upload_ms: f64,
    last_render_ms: f64,
    last_surface_acquire_ms: f64,
    last_surface_encode_ms: f64,
    last_surface_submit_ms: f64,
    last_surface_present_ms: f64,
}

impl FrameTimingStats {
    fn begin_frame(&mut self, frame_ms: f64, budget_ms: Option<f64>) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.last_frame_ms = frame_ms;
        self.worst_frame_ms = self.worst_frame_ms.max(frame_ms);
        self.budget_ms = budget_ms;
        self.last_runtime_poll_ms = 0.0;
        self.last_remesh_ms = 0.0;
        self.last_upload_ms = 0.0;
        self.last_render_ms = 0.0;
        self.last_surface_acquire_ms = 0.0;
        self.last_surface_encode_ms = 0.0;
        self.last_surface_submit_ms = 0.0;
        self.last_surface_present_ms = 0.0;

        if let Some(budget_ms) = budget_ms.filter(|budget_ms| *budget_ms > 0.0) {
            if frame_ms > budget_ms {
                self.over_budget_count = self.over_budget_count.saturating_add(1);
            }
            if frame_ms > budget_ms * 2.0 {
                self.over_2x_budget_count = self.over_2x_budget_count.saturating_add(1);
            }
            if frame_ms > budget_ms * 4.0 {
                self.over_4x_budget_count = self.over_4x_budget_count.saturating_add(1);
            }
        }
    }

    fn record_runtime_poll(&mut self, ms: f64) {
        self.last_runtime_poll_ms = ms;
    }

    fn record_remesh_upload(&mut self, remesh_ms: f64, upload_ms: f64) {
        self.last_remesh_ms = remesh_ms;
        self.last_upload_ms = upload_ms;
    }

    fn record_surface_frame(
        &mut self,
        render_ms: f64,
        acquire_ms: f64,
        encode_ms: f64,
        submit_ms: f64,
        present_ms: f64,
    ) {
        self.last_render_ms = render_ms;
        self.last_surface_acquire_ms = acquire_ms;
        self.last_surface_encode_ms = encode_ms;
        self.last_surface_submit_ms = submit_ms;
        self.last_surface_present_ms = present_ms;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FullFrameRenderSummary {
    section_count: usize,
    drawn_section_count: usize,
    index_count: u32,
    drawn_index_count: u32,
    gui_command_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DebugPaneStats {
    position: Vec3,
    speed: f32,
    runtime: WindowRuntimeStats,
    render: RenderStreamStats,
    frame: FrameTimingStats,
    pacing: FramePacingDebugStats,
    section_occlusion: bool,
    force_fullbright: bool,
}

impl DebugPaneStats {
    fn lines(self) -> Vec<String> {
        let occlusion = if self.section_occlusion { "ON" } else { "OFF" };
        let lighting = if self.force_fullbright {
            "FULL"
        } else {
            "LIGHT"
        };
        let budget = self
            .pacing
            .target_frame_ms
            .map(|ms| format!("{ms:.1}MS"))
            .unwrap_or_else(|| "UNCAPPED".to_owned());
        let refresh = self
            .pacing
            .monitor_refresh_hz
            .map(|hz| format!("{hz:.1}HZ"))
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let pacing_target = match self.pacing.mode {
            FramePacingMode::Capped => format!("{}FPS", self.pacing.fps_cap),
            FramePacingMode::Vsync | FramePacingMode::Uncapped => refresh,
        };
        vec![
            "DEBUG".to_string(),
            format!(
                "POS {:.1} {:.1} {:.1}",
                self.position.x, self.position.y, self.position.z
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                self.runtime.interest_center.x, self.runtime.interest_center.z, self.speed
            ),
            format!("OCC {}  {}", occlusion, lighting),
            format!(
                "TICK {} SIM {}",
                self.runtime.last_tick, self.runtime.last_simulation_tick
            ),
            format!(
                "CHUNKS L{} V{} P{}",
                self.runtime.loaded_chunks,
                self.runtime.client_visible_chunks,
                self.runtime.pending_jobs
            ),
            format!("STREAM PUB{}", self.runtime.pending_publications),
            format!("MESH Q{}", self.runtime.pending_render_chunks),
            format!(
                "TICKING B{}:{} E{}:{}",
                self.runtime.block_ticking_chunks,
                self.runtime.last_simulation_block_tick_chunks,
                self.runtime.entity_ticking_chunks,
                self.runtime.last_simulation_entity_tick_chunks
            ),
            format!(
                "FLUID {}/{}/{}/{}",
                self.runtime.last_simulation_fluid_ticks_executed,
                self.runtime.last_simulation_deferred_fluid_ticks,
                self.runtime.last_simulation_fluid_mutated_blocks,
                self.runtime.scheduled_fluid_ticks
            ),
            format!(
                "DRAW S {}/{} F {}/{}",
                self.render.drawn_section_count,
                self.render.section_count,
                self.render.drawn_face_count,
                self.render.face_count
            ),
            format!(
                "MESH R{} U{} D{} SQ{} CQ{} X{} F {:.1}MS",
                self.render.last_rebuilt_section_count,
                self.render.last_uploaded_section_count,
                self.render.last_deferred_section_count,
                self.render.last_submitted_compile_section_count,
                self.render.last_completed_compile_section_count,
                self.render.last_stale_compile_section_count,
                self.render.last_frame_ms
            ),
            format!("BUDGET {} FRAME {:.1}MS", budget, self.frame.last_frame_ms),
            format!(
                "OVER {}/{}/{} WORST {:.1}",
                self.frame.over_budget_count,
                self.frame.over_2x_budget_count,
                self.frame.over_4x_budget_count,
                self.frame.worst_frame_ms
            ),
            format!(
                "STAGE POLL {:.1} MESH {:.1} UP {:.1}",
                self.frame.last_runtime_poll_ms,
                self.frame.last_remesh_ms,
                self.frame.last_upload_ms
            ),
            format!(
                "GPU ACQ {:.1} ENC {:.1} SUB {:.1} PRS {:.1}",
                self.frame.last_surface_acquire_ms,
                self.frame.last_surface_encode_ms,
                self.frame.last_surface_submit_ms,
                self.frame.last_surface_present_ms
            ),
            format!(
                "PACE {} {} {}",
                self.pacing.mode.label(),
                pacing_target,
                self.pacing.active_present_mode_label.to_ascii_uppercase()
            ),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeScreen {
    Title,
    Pause,
    Options { parent: OptionsParent },
}

impl HeadlessScreenshotUi {
    fn native_screen(self) -> Option<NativeScreen> {
        match self {
            Self::None => None,
            Self::Title => Some(NativeScreen::Title),
            Self::Pause => Some(NativeScreen::Pause),
            Self::OptionsTitle => Some(NativeScreen::Options {
                parent: OptionsParent::Title,
            }),
            Self::OptionsPause => Some(NativeScreen::Options {
                parent: OptionsParent::Pause,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OptionsParent {
    Title,
    Pause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeUiAction {
    StartWorld,
    Resume,
    OpenOptions(OptionsParent),
    BackToTitle,
    BackToPause,
    ToggleSectionOcclusion,
    ToggleFullbright,
    CycleFramePacing,
    CycleFpsCap,
    SetChunkRadius(i32),
    Quit,
}

struct NativeUi {
    screen: Option<NativeScreen>,
    pointer: Option<Point>,
    pressed: Option<WidgetId>,
    font: Font,
    chunk_radius: i32,
    scale: GuiScale,
}

const ID_TITLE_START: WidgetId = WidgetId(1);
const ID_TITLE_OPTIONS: WidgetId = WidgetId(2);
const ID_TITLE_QUIT: WidgetId = WidgetId(3);
const ID_PAUSE_RESUME: WidgetId = WidgetId(4);
const ID_PAUSE_OPTIONS: WidgetId = WidgetId(5);
const ID_PAUSE_TITLE: WidgetId = WidgetId(6);
const ID_OPTIONS_OCCLUSION: WidgetId = WidgetId(7);
const ID_OPTIONS_FULLBRIGHT: WidgetId = WidgetId(8);
const ID_OPTIONS_RADIUS: WidgetId = WidgetId(9);
const ID_OPTIONS_BACK: WidgetId = WidgetId(10);
const ID_OPTIONS_FRAME_PACING: WidgetId = WidgetId(11);
const ID_OPTIONS_FPS_CAP: WidgetId = WidgetId(12);

impl NativeUi {
    fn new(chunk_radius: i32) -> Self {
        Self {
            screen: Some(NativeScreen::Title),
            pointer: None,
            pressed: None,
            font: Font::default(),
            chunk_radius,
            scale: GuiScale::from_pixels(1280, 900),
        }
    }

    fn new_ingame(chunk_radius: i32) -> Self {
        let mut ui = Self::new(chunk_radius);
        ui.set_screen(None);
        ui
    }

    fn set_scale(&mut self, scale: GuiScale) {
        self.scale = scale;
        self.pointer = self.pointer.map(|point| Point {
            x: point.x.clamp(0.0, scale.width),
            y: point.y.clamp(0.0, scale.height),
        });
    }

    fn is_active(&self) -> bool {
        self.screen.is_some()
    }

    fn covers_world(&self) -> bool {
        self.screen == Some(NativeScreen::Title)
    }

    fn open_pause(&mut self) {
        self.screen = Some(NativeScreen::Pause);
        self.pressed = None;
    }

    fn close(&mut self) {
        self.screen = None;
        self.pressed = None;
    }

    fn set_screen(&mut self, screen: Option<NativeScreen>) {
        self.screen = screen;
        self.pressed = None;
    }

    fn clear_input(&mut self) {
        self.pointer = None;
        self.pressed = None;
    }

    fn pointer_move(&mut self, point: Point) -> (bool, Option<NativeUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let action = if self.pressed == Some(ID_OPTIONS_RADIUS) {
            Some(self.chunk_radius_action_at(point))
        } else {
            None
        };
        (true, action)
    }

    fn pointer_down(&mut self, point: Point) -> bool {
        if !self.is_active() {
            return false;
        }
        self.pointer = Some(point);
        self.pressed = self.widget_at(point);
        true
    }

    fn pointer_up(&mut self, point: Point) -> (bool, Option<NativeUiAction>) {
        if !self.is_active() {
            return (false, None);
        }
        self.pointer = Some(point);
        let pressed = self.pressed.take();
        let released = self.widget_at(point);
        let action = match (pressed, released) {
            (Some(ID_OPTIONS_RADIUS), Some(ID_OPTIONS_RADIUS)) => {
                Some(self.chunk_radius_action_at(point))
            }
            (Some(id), Some(released)) if id == released => self.action_for(id),
            _ => None,
        };
        (true, action)
    }

    fn key_pressed(&mut self, key: KeyCode) -> (bool, Option<NativeUiAction>) {
        let Some(screen) = self.screen else {
            return (false, None);
        };
        match (screen, key) {
            (NativeScreen::Pause, KeyCode::Escape) => (true, Some(NativeUiAction::Resume)),
            (NativeScreen::Options { parent }, KeyCode::Escape) => match parent {
                OptionsParent::Title => (true, Some(NativeUiAction::BackToTitle)),
                OptionsParent::Pause => (true, Some(NativeUiAction::BackToPause)),
            },
            (NativeScreen::Title, KeyCode::Escape) => (true, None),
            _ => (false, None),
        }
    }

    fn apply_action(&mut self, action: NativeUiAction) {
        match action {
            NativeUiAction::StartWorld | NativeUiAction::Resume => self.close(),
            NativeUiAction::OpenOptions(parent) => {
                self.screen = Some(NativeScreen::Options { parent });
                self.pressed = None;
            }
            NativeUiAction::BackToTitle => {
                self.screen = Some(NativeScreen::Title);
                self.pressed = None;
            }
            NativeUiAction::BackToPause => {
                self.screen = Some(NativeScreen::Pause);
                self.pressed = None;
            }
            NativeUiAction::SetChunkRadius(radius) => {
                self.chunk_radius = radius.clamp(0, MAX_CHUNK_RADIUS);
            }
            NativeUiAction::ToggleSectionOcclusion
            | NativeUiAction::ToggleFullbright
            | NativeUiAction::CycleFramePacing
            | NativeUiAction::CycleFpsCap
            | NativeUiAction::Quit => {}
        }
    }

    fn render_draw_list(
        &self,
        render_options: TexturedSectionRenderOptions,
        frame_pacing: FramePacingUiState,
    ) -> GuiDrawList {
        let mut draw = GuiDrawList::new();
        match self.screen {
            Some(NativeScreen::Title) => self.render_title(&mut draw),
            Some(NativeScreen::Pause) => self.render_pause(&mut draw),
            Some(NativeScreen::Options { parent }) => {
                self.render_options_screen(&mut draw, render_options, frame_pacing, parent)
            }
            None => {}
        }
        draw
    }

    fn render_debug_pane(&self, draw: &mut GuiDrawList, stats: &DebugPaneStats) {
        let line_height = self.font.line_height();
        let lines = stats.lines();
        let panel_width = 236.0_f32.min(self.scale.width - 8.0).max(120.0);
        let panel_height = 8.0 + line_height * lines.len() as f32;
        let panel = Rect::new(
            4.0,
            4.0,
            panel_width,
            panel_height.min((self.scale.height - 8.0).max(0.0)),
        );
        draw.fill(panel, Color::rgba(6, 9, 10, 185));
        draw.outline(panel, Color::rgba(110, 140, 136, 230));
        draw.push_clip(panel.inset(4.0));
        let text = Color::rgba(220, 238, 220, 255);
        let muted = Color::rgba(165, 186, 176, 255);
        let mut y = panel.y + 5.0;
        for (index, line) in lines.iter().enumerate() {
            self.font.draw_shadow(
                draw,
                line,
                panel.x + 6.0,
                y,
                if index == 0 { text } else { muted },
            );
            y += line_height;
        }
        draw.pop_clip();
    }

    fn widget_at(&self, point: Point) -> Option<WidgetId> {
        match self.screen? {
            NativeScreen::Title => title_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            NativeScreen::Pause => pause_buttons(self.scale)
                .into_iter()
                .find(|button| button.contains(point))
                .map(|button| button.id),
            NativeScreen::Options { .. } => {
                let rects = option_widgets(self.scale);
                if rects.occlusion.contains(point) {
                    Some(ID_OPTIONS_OCCLUSION)
                } else if rects.fullbright.contains(point) {
                    Some(ID_OPTIONS_FULLBRIGHT)
                } else if rects.frame_pacing.contains(point) {
                    Some(ID_OPTIONS_FRAME_PACING)
                } else if rects.fps_cap.contains(point) {
                    Some(ID_OPTIONS_FPS_CAP)
                } else if rects.radius.contains(point) {
                    Some(ID_OPTIONS_RADIUS)
                } else if rects.back.contains(point) {
                    Some(ID_OPTIONS_BACK)
                } else {
                    None
                }
            }
        }
    }

    fn action_for(&self, id: WidgetId) -> Option<NativeUiAction> {
        match id {
            ID_TITLE_START => Some(NativeUiAction::StartWorld),
            ID_TITLE_OPTIONS => Some(NativeUiAction::OpenOptions(OptionsParent::Title)),
            ID_TITLE_QUIT => Some(NativeUiAction::Quit),
            ID_PAUSE_RESUME => Some(NativeUiAction::Resume),
            ID_PAUSE_OPTIONS => Some(NativeUiAction::OpenOptions(OptionsParent::Pause)),
            ID_PAUSE_TITLE => Some(NativeUiAction::BackToTitle),
            ID_OPTIONS_OCCLUSION => Some(NativeUiAction::ToggleSectionOcclusion),
            ID_OPTIONS_FULLBRIGHT => Some(NativeUiAction::ToggleFullbright),
            ID_OPTIONS_FRAME_PACING => Some(NativeUiAction::CycleFramePacing),
            ID_OPTIONS_FPS_CAP => Some(NativeUiAction::CycleFpsCap),
            ID_OPTIONS_BACK => match self.screen {
                Some(NativeScreen::Options {
                    parent: OptionsParent::Title,
                }) => Some(NativeUiAction::BackToTitle),
                Some(NativeScreen::Options {
                    parent: OptionsParent::Pause,
                }) => Some(NativeUiAction::BackToPause),
                _ => None,
            },
            _ => None,
        }
    }

    fn chunk_radius_action_at(&self, point: Point) -> NativeUiAction {
        let slider = Slider::new(
            ID_OPTIONS_RADIUS,
            option_widgets(self.scale).radius,
            "",
            chunk_radius_slider_value(self.chunk_radius),
        );
        NativeUiAction::SetChunkRadius(chunk_radius_from_slider_value(
            slider.value_from_point(point),
        ))
    }

    fn interaction(&self) -> Interaction {
        Interaction {
            pointer: self.pointer,
            pressed: self.pressed,
            focused: None,
        }
    }

    fn render_title(&self, draw: &mut GuiDrawList) {
        draw.fill_gradient(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(24, 44, 51, 255),
            Color::rgba(7, 10, 12, 255),
        );
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 55),
        );
        self.font.draw_centered(
            draw,
            "MCLONE",
            self.scale.width * 0.5,
            34.0,
            Color::rgba(245, 252, 234, 255),
        );
        self.font.draw_centered(
            draw,
            "NATIVE RUST CLIENT",
            self.scale.width * 0.5,
            48.0,
            Color::rgba(185, 212, 198, 255),
        );
        for button in title_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
        self.font.draw_shadow(
            draw,
            "MINECRAFT 1.17.1 TARGET",
            4.0,
            self.scale.height - 12.0,
            Color::rgba(160, 176, 170, 255),
        );
    }

    fn render_pause(&self, draw: &mut GuiDrawList) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 135),
        );
        self.font.draw_centered(
            draw,
            "PAUSED",
            self.scale.width * 0.5,
            self.scale.height * 0.25,
            Color::rgba(245, 252, 234, 255),
        );
        for button in pause_buttons(self.scale) {
            button.render(draw, &self.font, self.interaction());
        }
    }

    fn render_options_screen(
        &self,
        draw: &mut GuiDrawList,
        render_options: TexturedSectionRenderOptions,
        frame_pacing: FramePacingUiState,
        parent: OptionsParent,
    ) {
        draw.fill(
            Rect::new(0.0, 0.0, self.scale.width, self.scale.height),
            Color::rgba(0, 0, 0, 150),
        );
        let panel = centered_panel(self.scale, 242.0, 190.0);
        draw.fill_gradient(
            panel,
            Color::rgba(33, 45, 47, 245),
            Color::rgba(15, 20, 22, 245),
        );
        draw.outline(panel, Color::rgba(130, 166, 154, 255));
        self.font.draw_centered(
            draw,
            "OPTIONS",
            panel.center_x(),
            panel.y + 12.0,
            Color::rgba(245, 252, 234, 255),
        );
        let widgets = option_widgets(self.scale);
        Checkbox::new(
            ID_OPTIONS_OCCLUSION,
            widgets.occlusion,
            "Section Occlusion",
            render_options.section_occlusion_culling,
        )
        .render(draw, &self.font, self.interaction());
        Checkbox::new(
            ID_OPTIONS_FULLBRIGHT,
            widgets.fullbright,
            "Force Fullbright",
            render_options.force_fullbright,
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FRAME_PACING,
            widgets.frame_pacing,
            "Frame Pacing",
            frame_pacing.mode.label(),
        )
        .render(draw, &self.font, self.interaction());
        CycleButton::new(
            ID_OPTIONS_FPS_CAP,
            widgets.fps_cap,
            "FPS Cap",
            frame_pacing.fps_cap.to_string(),
        )
        .render(draw, &self.font, self.interaction());
        Slider::new(
            ID_OPTIONS_RADIUS,
            widgets.radius,
            chunk_radius_label(self.chunk_radius),
            chunk_radius_slider_value(self.chunk_radius),
        )
        .render(draw, &self.font, self.interaction());
        Button::new(
            ID_OPTIONS_BACK,
            widgets.back,
            match parent {
                OptionsParent::Title => "Back",
                OptionsParent::Pause => "Done",
            },
        )
        .render(draw, &self.font, self.interaction());
    }
}

#[derive(Clone, Copy, Debug)]
struct OptionWidgetRects {
    occlusion: Rect,
    fullbright: Rect,
    frame_pacing: Rect,
    fps_cap: Rect,
    radius: Rect,
    back: Rect,
}

fn title_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 22.0;
    [
        Button::new(
            ID_TITLE_START,
            menu_button_rect(scale, y),
            "Start Local World",
        ),
        Button::new(
            ID_TITLE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        ),
        Button::new(ID_TITLE_QUIT, menu_button_rect(scale, y + 48.0), "Quit"),
    ]
}

fn pause_buttons(scale: GuiScale) -> [Button; 3] {
    let y = scale.height * 0.5 - 22.0;
    [
        Button::new(ID_PAUSE_RESUME, menu_button_rect(scale, y), "Back To Game"),
        Button::new(
            ID_PAUSE_OPTIONS,
            menu_button_rect(scale, y + 24.0),
            "Options",
        ),
        Button::new(
            ID_PAUSE_TITLE,
            menu_button_rect(scale, y + 48.0),
            "Quit To Title",
        ),
    ]
}

fn option_widgets(scale: GuiScale) -> OptionWidgetRects {
    let panel = centered_panel(scale, 242.0, 190.0);
    OptionWidgetRects {
        occlusion: Rect::new(panel.x + 26.0, panel.y + 38.0, 190.0, 18.0),
        fullbright: Rect::new(panel.x + 26.0, panel.y + 60.0, 190.0, 18.0),
        frame_pacing: Rect::new(panel.x + 25.0, panel.y + 84.0, 192.0, 20.0),
        fps_cap: Rect::new(panel.x + 25.0, panel.y + 108.0, 192.0, 20.0),
        radius: Rect::new(panel.x + 25.0, panel.y + 132.0, 192.0, 20.0),
        back: Rect::new(panel.center_x() - 55.0, panel.y + 160.0, 110.0, 20.0),
    }
}

fn chunk_radius_slider_value(radius: i32) -> f32 {
    if MAX_CHUNK_RADIUS <= MIN_UI_CHUNK_RADIUS {
        0.0
    } else {
        (radius.clamp(MIN_UI_CHUNK_RADIUS, MAX_CHUNK_RADIUS) - MIN_UI_CHUNK_RADIUS) as f32
            / (MAX_CHUNK_RADIUS - MIN_UI_CHUNK_RADIUS) as f32
    }
}

fn chunk_radius_from_slider_value(value: f32) -> i32 {
    MIN_UI_CHUNK_RADIUS
        + (value.clamp(0.0, 1.0) * (MAX_CHUNK_RADIUS - MIN_UI_CHUNK_RADIUS) as f32).round() as i32
}

fn chunk_radius_label(radius: i32) -> String {
    let radius = radius.clamp(0, MAX_CHUNK_RADIUS);
    let suffix = if radius == 1 { "chunk" } else { "chunks" };
    format!("Chunk Radius: {radius} {suffix}")
}

fn centered_panel(scale: GuiScale, width: f32, height: f32) -> Rect {
    Rect::new(
        (scale.width - width).max(0.0) * 0.5,
        (scale.height - height).max(0.0) * 0.5,
        width.min(scale.width),
        height.min(scale.height),
    )
}

fn menu_button_rect(scale: GuiScale, y: f32) -> Rect {
    Rect::new(scale.width * 0.5 - 90.0, y, 180.0, 20.0)
}

fn render_static_title_ui(width: u32, height: u32) -> GuiDrawList {
    let scale = GuiScale::from_pixels(width, height);
    let mut ui = NativeUi::new(DEFAULT_CHUNK_RADIUS);
    ui.set_scale(scale);
    ui.render_draw_list(
        TexturedSectionRenderOptions::default(),
        FramePacingUiState::default(),
    )
}

fn render_full_frame(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    draw: &mut TexturedSectionDrawResources,
    gui: &mut GuiRenderer,
    camera: ChunkCamera,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacingUiState,
    ui: &NativeUi,
    debug_stats: Option<DebugPaneStats>,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary> {
    let ui_active = ui.is_active();
    let ui_covers_world = ui.covers_world();
    let debug_stats = (!ui_active).then_some(debug_stats).flatten();
    let gui_active = ui_active || debug_stats.is_some();

    if !ui_covers_world {
        let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            mclone_render::default_clear_color(),
        )?;
        let frame_stats = draw.render_with_options(
            frame.queue,
            frame.encoder,
            render_target,
            render_view,
            render_options,
        )?;
        render_stats.drawn_section_count = frame_stats.drawn_section_count;
        render_stats.drawn_face_count = frame_stats.drawn_face_count();
        render_stats.drawn_index_count = frame_stats.drawn_index_count;
    } else {
        render_stats.drawn_section_count = 0;
        render_stats.drawn_face_count = 0;
        render_stats.drawn_index_count = 0;
    }

    let mut ui_draw = ui.render_draw_list(render_options, frame_pacing);
    if let Some(mut stats) = debug_stats {
        stats.render = *render_stats;
        ui.render_debug_pane(&mut ui_draw, &stats);
    }

    let gui_command_count = ui_draw.commands().len();
    if gui_active {
        gui.render(
            frame.device,
            frame.queue,
            frame.encoder,
            frame.target,
            [ui.scale.width, ui.scale.height],
            &ui_draw,
            if ui_covers_world {
                GuiRenderOptions::clear(mclone_render::default_clear_color())
            } else {
                GuiRenderOptions::overlay()
            },
        )?;
    }

    Ok(FullFrameRenderSummary {
        section_count: draw.section_count(),
        drawn_section_count: render_stats.drawn_section_count,
        index_count: draw.index_count(),
        drawn_index_count: render_stats.drawn_index_count,
        gui_command_count,
    })
}

struct ChunkApp {
    runtime: WindowSceneRuntime,
    spectator: SpectatorCamera,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacing,
    ui: NativeUi,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    depth: Option<ChunkDepthTarget>,
    draw: Option<TexturedSectionDrawResources>,
    gui: Option<GuiRenderer>,
    pressed_keys: std::collections::HashSet<KeyCode>,
    mouse_locked: bool,
    mouse_lock_requested: bool,
    last_cursor: Option<(f64, f64)>,
    debug_visible: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    render_stats: RenderStreamStats,
    frame_timing: FrameTimingStats,
}

impl ChunkApp {
    fn new(
        runtime: WindowSceneRuntime,
        spectator: SpectatorCamera,
        render_options: TexturedSectionRenderOptions,
        chunk_radius: i32,
    ) -> Self {
        Self {
            runtime,
            spectator,
            render_options,
            frame_pacing: FramePacing::default(),
            ui: NativeUi::new_ingame(chunk_radius),
            window: None,
            surface: None,
            depth: None,
            draw: None,
            gui: None,
            pressed_keys: std::collections::HashSet::new(),
            mouse_locked: false,
            mouse_lock_requested: false,
            last_cursor: None,
            debug_visible: false,
            last_frame: Instant::now(),
            next_redraw_at: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
        }
    }

    fn schedule_next_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        if self.frame_pacing.mode != FramePacingMode::Capped {
            self.next_redraw_at = None;
        }

        match redraw_schedule(self.frame_pacing.mode, Instant::now(), self.next_redraw_at) {
            RedrawSchedule::RequestNow => {
                event_loop.set_control_flow(ControlFlow::Poll);
                window.request_redraw();
            }
            RedrawSchedule::WaitUntil(deadline) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        }
    }

    fn finish_redraw(&mut self, event_loop: &ActiveEventLoop, frame_start: Instant) {
        if let Some(frame_duration) = self.frame_pacing.target_frame_duration() {
            let next_redraw_at = next_capped_redraw_deadline(
                frame_start,
                Instant::now(),
                self.next_redraw_at,
                frame_duration,
            );
            self.next_redraw_at = Some(next_redraw_at);
        } else {
            self.next_redraw_at = None;
        }
        self.schedule_next_redraw(event_loop);
    }

    fn update_camera_from_keys(&mut self, now: Instant) -> Result<()> {
        let frame_dt = now.duration_since(self.last_frame);
        let movement_dt = frame_dt.as_secs_f32().min(0.05);
        self.last_frame = now;
        self.render_stats.last_frame_ms = (frame_dt.as_secs_f64() * 1000.0) as f32;
        self.frame_timing.begin_frame(
            frame_dt.as_secs_f64() * 1000.0,
            self.frame_pacing.target_frame_ms(),
        );

        if self.ui.is_active() {
            return Ok(());
        }

        let right = key_axis(&self.pressed_keys, KeyCode::KeyD, KeyCode::KeyA);
        let up = vertical_axis(&self.pressed_keys);
        let forward = key_axis(&self.pressed_keys, KeyCode::KeyW, KeyCode::KeyS);
        let boosted = self.pressed_keys.contains(&KeyCode::ShiftLeft)
            || self.pressed_keys.contains(&KeyCode::ShiftRight);
        if self
            .spectator
            .move_local(right, up, forward, boosted, movement_dt)
        {
            self.update_interest_from_spectator()?;
        }
        Ok(())
    }

    fn gui_scale(&self) -> Option<GuiScale> {
        self.surface.as_ref().map(|surface| {
            GuiScale::from_pixels(surface.config.width.max(1), surface.config.height.max(1))
        })
    }

    fn gui_point(&self, x: f64, y: f64) -> Option<Point> {
        self.gui_scale().map(|scale| scale.client_to_gui(x, y))
    }

    fn apply_ui_action(
        &mut self,
        action: NativeUiAction,
        event_loop: &ActiveEventLoop,
        from_pointer_click: bool,
    ) {
        let should_arm_mouse_lock = from_pointer_click
            && matches!(action, NativeUiAction::StartWorld | NativeUiAction::Resume);
        let preserve_pointer_state = matches!(action, NativeUiAction::SetChunkRadius(_));
        match action {
            NativeUiAction::ToggleSectionOcclusion => {
                self.render_options.section_occlusion_culling =
                    !self.render_options.section_occlusion_culling;
                log::info!(
                    "section occlusion culling {}",
                    if self.render_options.section_occlusion_culling {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            NativeUiAction::ToggleFullbright => {
                self.render_options.force_fullbright = !self.render_options.force_fullbright;
                log::info!(
                    "fullbright {}",
                    if self.render_options.force_fullbright {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            NativeUiAction::CycleFramePacing => {
                self.frame_pacing.cycle_mode();
                self.next_redraw_at = None;
                if let Some(surface) = &mut self.surface {
                    self.frame_pacing.apply_to_surface(surface);
                }
                log::info!(
                    "frame pacing set to {} at cap {} fps",
                    self.frame_pacing.mode.label(),
                    self.frame_pacing.fps_cap
                );
            }
            NativeUiAction::CycleFpsCap => {
                self.frame_pacing.cycle_fps_cap();
                self.next_redraw_at = None;
                log::info!("fps cap set to {}", self.frame_pacing.fps_cap);
            }
            NativeUiAction::SetChunkRadius(radius) => {
                let radius = radius.clamp(0, MAX_CHUNK_RADIUS);
                let radius_chunks =
                    u32::try_from(radius).expect("clamped chunk radius must fit u32");
                match self.runtime.set_radius_chunks(radius_chunks) {
                    Ok(true) => {
                        log::info!("chunk radius set to {radius}");
                    }
                    Ok(false) => {}
                    Err(err) => {
                        log::error!("failed to set chunk radius to {radius}: {err:#}");
                        return;
                    }
                }
            }
            NativeUiAction::Quit => {
                event_loop.exit();
                return;
            }
            NativeUiAction::StartWorld
            | NativeUiAction::Resume
            | NativeUiAction::OpenOptions(_)
            | NativeUiAction::BackToTitle
            | NativeUiAction::BackToPause => {}
        }
        self.ui.apply_action(action);
        if should_arm_mouse_lock {
            self.mouse_lock_requested = true;
        }
        self.pressed_keys.clear();
        if !preserve_pointer_state {
            self.last_cursor = None;
        }
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn sync_mouse_lock(&mut self) {
        self.set_mouse_lock(self.mouse_lock_requested && !self.ui.is_active());
    }

    fn set_mouse_lock(&mut self, should_lock: bool) {
        let Some(window) = &self.window else {
            self.mouse_locked = false;
            return;
        };
        if should_lock == self.mouse_locked {
            return;
        }

        if should_lock {
            let grab_result =
                window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|locked_err| {
                        log::warn!(
                            "cursor lock unavailable ({locked_err}); trying confined cursor grab"
                        );
                        window.set_cursor_grab(CursorGrabMode::Confined)
                    });
            match grab_result {
                Ok(()) => {
                    window.set_cursor_visible(false);
                    self.mouse_locked = true;
                    self.last_cursor = None;
                }
                Err(err) => {
                    log::warn!("failed to grab cursor for mouse look: {err}");
                    window.set_cursor_visible(true);
                    self.mouse_locked = false;
                }
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                log::warn!("failed to release cursor grab: {err}");
            }
            window.set_cursor_visible(true);
            self.mouse_locked = false;
            self.last_cursor = None;
        }
    }

    fn update_interest_from_spectator(&mut self) -> Result<()> {
        let center = self.spectator.chunk_pos();
        if self.runtime.set_interest_center(center)? {
            log::info!(
                "chunk interest moved to ({}, {}) at spectator position ({:.1}, {:.1}, {:.1})",
                center.x,
                center.z,
                self.spectator.position.x,
                self.spectator.position.y,
                self.spectator.position.z
            );
        }
        Ok(())
    }

    fn poll_runtime_and_upload(&mut self) -> Result<()> {
        let poll_start = Instant::now();
        let changed = self.runtime.poll()?;
        self.frame_timing
            .record_runtime_poll(elapsed_ms(poll_start.elapsed()));
        if !changed
            && !self
                .runtime
                .has_pending_render_work(self.spectator.position)
        {
            return Ok(());
        }
        self.upload_runtime_sections()?;
        Ok(())
    }

    fn upload_runtime_sections(&mut self) -> Result<()> {
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let remesh_start = Instant::now();
        let section_update = self.runtime.sync_render_sections(self.spectator.position)?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let upload_report = draw
            .apply_section_updates(
                &surface.device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("failed to upload streamed chunk section updates")?;
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(&section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(remesh_ms, upload_ms);
        log::info!(
            "streamed chunks loaded={} sections={} faces={} indices={} rebuilt={} visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} uploaded={} uploaded_vertices={} uploaded_faces={} uploaded_indices={} removed={} remesh_ms={:.3} upload_ms={:.3}",
            self.runtime.client.loaded_chunk_count(),
            section_count,
            face_count,
            index_count,
            section_update.rebuilt_section_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            upload_report.uploaded_section_count,
            upload_report.uploaded_vertex_count,
            upload_report.uploaded_face_count(),
            upload_report.uploaded_index_count,
            upload_report.removed_section_count,
            remesh_ms,
            upload_ms
        );
        Ok(())
    }

    fn record_section_update_stats(
        &mut self,
        section_update: &RenderSectionCacheUpdate,
        upload_report: TexturedSectionUploadReport,
    ) {
        record_render_section_update_stats(&mut self.render_stats, section_update, upload_report);
    }

    fn effective_render_options(&self) -> TexturedSectionRenderOptions {
        effective_render_options_for_camera(
            self.render_options,
            self.runtime
                .camera_inside_occluding_block(self.spectator.position),
        )
    }

    fn debug_pane_stats(&self, render_options: TexturedSectionRenderOptions) -> DebugPaneStats {
        DebugPaneStats {
            position: self.spectator.position,
            speed: self.spectator.speed,
            runtime: self.runtime.stats(),
            render: self.render_stats,
            frame: self.frame_timing,
            pacing: self.frame_pacing.debug_stats(),
            section_occlusion: render_options.section_occlusion_culling,
            force_fullbright: render_options.force_fullbright,
        }
    }
}

fn effective_render_options_for_camera(
    mut render_options: TexturedSectionRenderOptions,
    camera_inside_occluding_block: bool,
) -> TexturedSectionRenderOptions {
    if camera_inside_occluding_block {
        render_options.section_occlusion_culling = false;
    }
    render_options
}

fn record_render_section_update_stats(
    render_stats: &mut RenderStreamStats,
    section_update: &RenderSectionCacheUpdate,
    upload_report: TexturedSectionUploadReport,
) {
    render_stats.last_rebuilt_section_count = section_update.rebuilt_section_count();
    render_stats.last_removed_section_count = section_update.removed_section_count();
    render_stats.last_rebuilt_vertex_count = section_update.rebuilt_vertex_count;
    render_stats.last_rebuilt_face_count = section_update.rebuilt_face_count();
    render_stats.last_rebuilt_index_count = section_update.rebuilt_index_count;
    render_stats.last_neighbor_ready_section_count = section_update.neighbor_ready_section_count;
    render_stats.last_near_exception_section_count = section_update.near_exception_section_count;
    render_stats.last_deferred_section_count = section_update.deferred_section_count;
    render_stats.last_submitted_compile_section_count =
        section_update.submitted_compile_section_count;
    render_stats.last_completed_compile_section_count =
        section_update.completed_compile_section_count;
    render_stats.last_stale_compile_section_count = section_update.stale_compile_section_count;
    render_stats.last_pending_compile_jobs = section_update.pending_compile_jobs;
    render_stats.last_visibility_graph_build_count =
        section_update.visibility_graph_stats.build_count;
    render_stats.last_visibility_graph_total_ms = section_update.visibility_graph_stats.total_ms;
    render_stats.last_visibility_graph_worst_ms = section_update.visibility_graph_stats.worst_ms;
    render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
    render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
    render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
    render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
    render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
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
        let mut surface = match NativeSurfaceContext::new(window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
        self.frame_pacing.update_monitor(&window);
        self.frame_pacing.apply_to_surface(&mut surface);
        let remesh_start = Instant::now();
        let section_update = match self
            .runtime
            .sync_all_render_sections(self.spectator.position)
        {
            Ok(update) => update,
            Err(err) => {
                log::error!("failed to build initial chunk sections: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let sections = self.runtime.cached_sections();
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let depth =
            ChunkDepthTarget::new(&surface.device, surface.config.width, surface.config.height);
        let draw = match TexturedSectionDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            &sections,
            self.runtime.mesh_assets.atlas.as_upload(),
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let mut draw = draw;
        draw.set_traversal_ready_sections(
            &self
                .runtime
                .traversal_ready_render_section_keys(self.spectator.position),
        );
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let gui = GuiRenderer::new(&surface.device, surface.config.format);
        self.ui.set_scale(GuiScale::from_pixels(
            surface.config.width,
            surface.config.height,
        ));
        self.render_stats.section_count = draw.section_count();
        self.render_stats.index_count = draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(
            &section_update,
            TexturedSectionUploadReport {
                uploaded_section_count: section_update.rebuilt_section_count(),
                removed_section_count: section_update.removed_section_count(),
                uploaded_vertex_count: section_update.rebuilt_vertex_count,
                uploaded_index_count: section_update.rebuilt_index_count,
            },
        );
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        log::info!(
            "uploaded {} initial render sections with {} vertices / {} faces / {} indices visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} remesh_ms={:.3} upload_ms={:.3}",
            draw.section_count(),
            section_update.rebuilt_vertex_count,
            self.render_stats.face_count,
            draw.index_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            remesh_ms,
            upload_ms
        );
        self.depth = Some(depth);
        self.draw = Some(draw);
        self.gui = Some(gui);
        self.surface = Some(surface);
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.frame_timing = FrameTimingStats::default();
        self.next_redraw_at = None;
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.resize(size);
                }
                if let (Some(surface), Some(depth)) = (&self.surface, &mut self.depth) {
                    depth.resize(&surface.device, surface.config.width, surface.config.height);
                    self.ui.set_scale(GuiScale::from_pixels(
                        surface.config.width,
                        surface.config.height,
                    ));
                }
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if let Some(scale) = self.gui_scale() {
                        self.ui.set_scale(scale);
                    }
                    if key_code == KeyCode::Backquote
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.debug_visible = !self.debug_visible;
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if event.state == ElementState::Pressed && self.ui.is_active() {
                        let (handled, action) = self.ui.key_pressed(key_code);
                        if let Some(action) = action {
                            self.apply_ui_action(action, event_loop, false);
                        } else if handled {
                            self.schedule_next_redraw(event_loop);
                        }
                        return;
                    }
                    if key_code == KeyCode::Escape
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.mouse_lock_requested = false;
                        self.ui.open_pause();
                        self.pressed_keys.clear();
                        self.last_cursor = None;
                        self.sync_mouse_lock();
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == KeyCode::KeyO
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.render_options.section_occlusion_culling =
                            !self.render_options.section_occlusion_culling;
                        log::info!(
                            "section occlusion culling {}",
                            if self.render_options.section_occlusion_culling {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == KeyCode::KeyL
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.render_options.force_fullbright =
                            !self.render_options.force_fullbright;
                        log::info!(
                            "fullbright {}",
                            if self.render_options.force_fullbright {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    match event.state {
                        ElementState::Pressed => {
                            self.pressed_keys.insert(key_code);
                        }
                        ElementState::Released => {
                            self.pressed_keys.remove(&key_code);
                        }
                    }
                    self.schedule_next_redraw(event_loop);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.ui.is_active() {
                    if button == MouseButton::Left {
                        if let Some((x, y)) = self.last_cursor
                            && let Some(point) = self.gui_point(x, y)
                        {
                            match state {
                                ElementState::Pressed => {
                                    self.ui.pointer_down(point);
                                }
                                ElementState::Released => {
                                    let (_handled, action) = self.ui.pointer_up(point);
                                    if let Some(action) = action {
                                        self.apply_ui_action(action, event_loop, true);
                                        return;
                                    }
                                }
                            }
                        }
                        self.schedule_next_redraw(event_loop);
                    }
                    return;
                }
                if state == ElementState::Pressed {
                    self.mouse_lock_requested = true;
                    self.last_cursor = None;
                    self.sync_mouse_lock();
                    self.schedule_next_redraw(event_loop);
                } else if button == MouseButton::Left || button == MouseButton::Right {
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let cursor = (position.x, position.y);
                if self.ui.is_active() {
                    self.last_cursor = Some(cursor);
                    if let Some(point) = self.gui_point(cursor.0, cursor.1) {
                        let (_handled, action) = self.ui.pointer_move(point);
                        if let Some(action) = action {
                            self.apply_ui_action(action, event_loop, true);
                            return;
                        }
                    }
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                if self.mouse_lock_requested && !self.mouse_locked {
                    if let Some(previous) = self.last_cursor {
                        let dx = (cursor.0 - previous.0) as f32;
                        let dy = (cursor.1 - previous.1) as f32;
                        self.spectator.look(
                            -dx * SPECTATOR_MOUSE_SENSITIVITY,
                            -dy * SPECTATOR_MOUSE_SENSITIVITY,
                        );
                        self.schedule_next_redraw(event_loop);
                    }
                }
                self.last_cursor = Some(cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.ui.is_active() {
                    return;
                }
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                self.spectator.adjust_speed(amount);
                log::info!("spectator speed {:.1} blocks/s", self.spectator.speed);
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.pressed_keys.clear();
                self.last_cursor = None;
                self.ui.clear_input();
                self.set_mouse_lock(false);
            }
            WindowEvent::Focused(true) => {
                self.sync_mouse_lock();
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                if let Some(window) = &self.window {
                    self.frame_pacing.update_monitor(window);
                }
                if let Err(err) = self.update_camera_from_keys(frame_start) {
                    log::error!("failed to update spectator camera: {err:#}");
                    event_loop.exit();
                    return;
                }
                if let Err(err) = self.poll_runtime_and_upload() {
                    log::error!("failed to poll chunk runtime: {err:#}");
                    event_loop.exit();
                    return;
                }
                let Some(gui_scale) = self.gui_scale() else {
                    return;
                };
                self.ui.set_scale(gui_scale);
                let camera = self.spectator.camera(self.runtime.radius_chunks);
                let render_options = self.effective_render_options();
                let frame_pacing = self.frame_pacing.ui_state();
                let debug_stats = self
                    .debug_visible
                    .then(|| self.debug_pane_stats(render_options));
                let traversal_ready_sections = self
                    .runtime
                    .traversal_ready_render_section_keys(self.spectator.position);
                let mut render_stats = self.render_stats;
                let render_start = Instant::now();
                let result = {
                    let (Some(surface), Some(depth), Some(draw), Some(gui)) = (
                        &mut self.surface,
                        &self.depth,
                        &mut self.draw,
                        &mut self.gui,
                    ) else {
                        return;
                    };
                    draw.set_traversal_ready_sections(&traversal_ready_sections);
                    self.frame_pacing.apply_to_surface(surface);
                    surface.render_with_report(|frame| {
                        render_full_frame(
                            frame,
                            depth,
                            draw,
                            gui,
                            camera,
                            render_options,
                            frame_pacing,
                            &self.ui,
                            debug_stats,
                            &mut render_stats,
                        )?;
                        Ok(())
                    })
                };
                match result {
                    Ok(report) => {
                        self.frame_timing.record_surface_frame(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                self.render_stats = render_stats;
                                self.finish_redraw(event_loop, frame_start);
                            }
                            SurfaceFrameStatus::Reconfigured => {
                                self.schedule_next_redraw(event_loop);
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("render failed: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.ui.is_active() || !self.mouse_locked {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event {
            let dx = delta.0 as f32;
            let dy = delta.1 as f32;
            self.spectator.look(
                -dx * SPECTATOR_MOUSE_SENSITIVITY,
                -dy * SPECTATOR_MOUSE_SENSITIVITY,
            );
            self.schedule_next_redraw(event_loop);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule_next_redraw(event_loop);
    }
}

fn key_axis(
    keys: &std::collections::HashSet<KeyCode>,
    positive: KeyCode,
    negative: KeyCode,
) -> f32 {
    let positive = keys.contains(&positive) as i32;
    let negative = keys.contains(&negative) as i32;
    (positive - negative) as f32
}

fn vertical_axis(keys: &std::collections::HashSet<KeyCode>) -> f32 {
    key_axis(keys, KeyCode::Space, KeyCode::KeyX)
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RedrawSchedule {
    RequestNow,
    WaitUntil(Instant),
}

fn redraw_schedule(
    mode: FramePacingMode,
    now: Instant,
    next_redraw_at: Option<Instant>,
) -> RedrawSchedule {
    if mode == FramePacingMode::Capped
        && let Some(deadline) = next_redraw_at
        && now < deadline
    {
        return RedrawSchedule::WaitUntil(deadline);
    }
    RedrawSchedule::RequestNow
}

fn next_capped_redraw_deadline(
    frame_start: Instant,
    finish: Instant,
    previous_deadline: Option<Instant>,
    frame_duration: Duration,
) -> Instant {
    let mut next = previous_deadline.unwrap_or(frame_start) + frame_duration;
    while next <= finish {
        next += frame_duration;
    }
    next
}

fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1000.0
}

fn print_benchmark_metadata(name: &str, indent: &str, trailing_comma: bool) {
    println!("{indent}\"benchmark\": \"{}\",", json_escape(name));
    println!(
        "{indent}\"recorded_unix_seconds\": {},",
        current_unix_seconds()
    );
    println!(
        "{indent}\"git_commit\": \"{}\",",
        json_escape(&git_short_commit())
    );
    println!("{indent}\"git_dirty\": {},", git_dirty());
    let suffix = if trailing_comma { "," } else { "" };
    println!(
        "{indent}\"debug_assertions\": {}{suffix}",
        cfg!(debug_assertions)
    );
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn git_short_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true)
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{BlockStateId, ChunkRevision, ChunkStatus};

    #[test]
    fn snapshot_block_state_lookup_reads_loaded_sections_and_omitted_air() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        block_state_ids[mclone_core::chunk_section_index(1, 15, 15)] = BlockStateId(42);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(1, -1),
            ChunkStatus::Full,
            ChunkRevision(1),
            -16,
            32,
            &block_state_ids,
        );

        assert_eq!(
            snapshot_block_state_at_world(&snapshot, 17, -1, -1),
            Some(BlockStateId(42))
        );
        assert_eq!(
            snapshot_block_state_at_world(&snapshot, 17, 0, -1),
            Some(AIR_BLOCK_STATE_ID)
        );
        assert_eq!(snapshot_block_state_at_world(&snapshot, 0, -1, -1), None);
        assert_eq!(snapshot_block_state_at_world(&snapshot, 17, 16, -1), None);
    }

    #[test]
    fn effective_render_options_disable_section_occlusion_inside_occluding_block() {
        let enabled = TexturedSectionRenderOptions {
            section_occlusion_culling: true,
            ..TexturedSectionRenderOptions::default()
        };
        let disabled = TexturedSectionRenderOptions {
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };

        assert!(effective_render_options_for_camera(enabled, false).section_occlusion_culling);
        assert!(!effective_render_options_for_camera(enabled, true).section_occlusion_culling);
        assert!(!effective_render_options_for_camera(disabled, true).section_occlusion_culling);
    }

    #[test]
    fn render_section_readiness_uses_near_exception_and_horizontal_neighbors() {
        let target = ChunkPos::new(2, -3);
        let key = RenderSectionKey::new(target.x, 0, target.z);
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(target)));

        assert_eq!(
            render_section_neighbor_readiness(&client, key, render_section_center(key)),
            RenderNeighborReadiness::ReadyNearCamera
        );

        let far_camera = render_section_center(key) + Vec3::new(128.0, 0.0, 0.0);
        assert_eq!(
            render_section_neighbor_readiness(&client, key, far_camera),
            RenderNeighborReadiness::DeferredMissingNeighbors
        );

        for neighbor in [
            ChunkPos::new(target.x - 1, target.z),
            ChunkPos::new(target.x + 1, target.z),
            ChunkPos::new(target.x, target.z - 1),
            ChunkPos::new(target.x, target.z + 1),
        ] {
            client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(neighbor)));
        }

        assert_eq!(
            render_section_neighbor_readiness(&client, key, far_camera),
            RenderNeighborReadiness::ReadyWithNeighbors
        );
    }

    #[test]
    fn render_section_keys_cover_snapshot_height() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(-1, 4),
            ChunkStatus::Full,
            ChunkRevision(1),
            -16,
            32,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2],
        );

        assert_eq!(
            render_section_keys_for_snapshot(&snapshot),
            vec![
                RenderSectionKey::new(-1, -1, 4),
                RenderSectionKey::new(-1, 0, 4)
            ]
        );
    }

    #[test]
    fn block_delta_dirty_sections_stay_local_for_interior_blocks() {
        let keys = render_dirty_section_keys_for_block_update(
            ChunkPos::new(2, -3),
            5,
            &SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: BlockStateId(42),
            },
        );

        assert_eq!(keys, BTreeSet::from([RenderSectionKey::new(2, 5, -3)]));
    }

    #[test]
    fn block_delta_dirty_sections_cross_section_boundaries() {
        let keys = render_dirty_section_keys_for_block_update(
            ChunkPos::new(0, 0),
            0,
            &SectionBlockUpdate {
                local_x: 0,
                local_y: 0,
                local_z: 15,
                block_state: BlockStateId(42),
            },
        );

        assert_eq!(
            keys,
            BTreeSet::from([
                RenderSectionKey::new(-1, -1, 0),
                RenderSectionKey::new(-1, -1, 1),
                RenderSectionKey::new(-1, 0, 0),
                RenderSectionKey::new(-1, 0, 1),
                RenderSectionKey::new(0, -1, 0),
                RenderSectionKey::new(0, -1, 1),
                RenderSectionKey::new(0, 0, 0),
                RenderSectionKey::new(0, 0, 1),
            ])
        );
    }

    fn empty_test_snapshot(pos: ChunkPos) -> ChunkSnapshot {
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
    }

    #[test]
    fn vertical_axis_uses_space_for_up_and_x_for_down() {
        let mut keys = std::collections::HashSet::new();

        keys.insert(KeyCode::Space);
        assert_eq!(vertical_axis(&keys), 1.0);

        keys.insert(KeyCode::KeyX);
        assert_eq!(vertical_axis(&keys), 0.0);

        keys.remove(&KeyCode::Space);
        assert_eq!(vertical_axis(&keys), -1.0);
    }

    #[test]
    fn capped_redraw_deadline_skips_missed_frames() {
        let frame_duration = Duration::from_millis(8);
        let start = Instant::now();
        let previous_deadline = start + frame_duration;
        let finish = start + Duration::from_millis(27);

        let next =
            next_capped_redraw_deadline(start, finish, Some(previous_deadline), frame_duration);

        assert!(next > finish);
        assert_eq!(next.duration_since(start), frame_duration * 4);
    }

    #[test]
    fn capped_redraw_schedule_waits_until_due() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(16);

        assert_eq!(
            redraw_schedule(FramePacingMode::Capped, now, Some(deadline)),
            RedrawSchedule::WaitUntil(deadline)
        );
        assert_eq!(
            redraw_schedule(FramePacingMode::Capped, deadline, Some(deadline)),
            RedrawSchedule::RequestNow
        );
        assert_eq!(
            redraw_schedule(FramePacingMode::Capped, now, None),
            RedrawSchedule::RequestNow
        );
        assert_eq!(
            redraw_schedule(FramePacingMode::Vsync, now, Some(deadline)),
            RedrawSchedule::RequestNow
        );
    }

    #[test]
    fn frame_timing_counts_budget_overruns() {
        let mut stats = FrameTimingStats::default();

        stats.begin_frame(9.0, Some(8.0));
        stats.begin_frame(17.0, Some(8.0));
        stats.begin_frame(33.0, Some(8.0));

        assert_eq!(stats.frame_count, 3);
        assert_eq!(stats.over_budget_count, 3);
        assert_eq!(stats.over_2x_budget_count, 2);
        assert_eq!(stats.over_4x_budget_count, 1);
        assert_eq!(stats.worst_frame_ms, 33.0);
    }

    #[test]
    fn frame_pacing_cycles_mode_and_fps_cap() {
        let mut pacing = FramePacing::default();

        pacing.cycle_mode();
        assert_eq!(pacing.mode, FramePacingMode::Capped);
        pacing.cycle_fps_cap();
        assert_eq!(pacing.fps_cap, 144);
    }

    #[test]
    fn debug_pane_formats_and_draws_runtime_stats() {
        let stats = DebugPaneStats {
            position: Vec3::new(1.25, 64.0, -2.5),
            speed: 32.0,
            runtime: WindowRuntimeStats {
                interest_center: ChunkPos::new(3, -4),
                loaded_chunks: 9,
                pending_jobs: 1,
                pending_publications: 2,
                pending_render_chunks: 3,
                pending_render_compile_jobs: 1,
                inflight_render_sections: 4,
                client_visible_chunks: 8,
                active_ticket_chunks: 9,
                pending_unload_chunks: 0,
                block_ticking_chunks: 4,
                entity_ticking_chunks: 2,
                last_tick: 12,
                last_simulation_tick: 11,
                last_tick_unloads_processed: 0,
                last_simulation_block_tick_chunks: 4,
                last_simulation_entity_tick_chunks: 2,
                last_simulation_scheduler_tick_ms: 0.1,
                last_simulation_block_tick_ms: 0.2,
                last_simulation_fluid_tick_ms: 0.3,
                last_simulation_entity_tick_ms: 0.4,
                last_simulation_fluid_ticks_executed: 5,
                last_simulation_deferred_fluid_ticks: 6,
                last_simulation_fluid_mutated_blocks: 7,
                scheduled_fluid_ticks: 8,
            },
            render: RenderStreamStats {
                section_count: 16,
                drawn_section_count: 10,
                face_count: 200,
                drawn_face_count: 120,
                last_rebuilt_section_count: 2,
                last_uploaded_section_count: 2,
                last_frame_ms: 16.7,
                ..RenderStreamStats::default()
            },
            frame: FrameTimingStats {
                frame_count: 12,
                over_budget_count: 3,
                over_2x_budget_count: 1,
                over_4x_budget_count: 0,
                last_frame_ms: 16.7,
                worst_frame_ms: 33.4,
                budget_ms: Some(8.3),
                last_runtime_poll_ms: 5.0,
                last_remesh_ms: 2.0,
                last_upload_ms: 1.0,
                last_surface_acquire_ms: 0.2,
                last_surface_encode_ms: 1.4,
                last_surface_submit_ms: 0.1,
                last_surface_present_ms: 0.0,
                ..FrameTimingStats::default()
            },
            pacing: FramePacingDebugStats {
                mode: FramePacingMode::Vsync,
                fps_cap: 120,
                monitor_refresh_hz: Some(120.0),
                target_frame_ms: Some(8.3),
                active_present_mode_label: "fifo",
            },
            section_occlusion: true,
            force_fullbright: false,
        };

        let lines = stats.lines();
        assert_eq!(lines[0], "DEBUG");
        assert_eq!(lines[1], "POS 1.2 64.0 -2.5");
        assert_eq!(lines[2], "CHUNK 3 -4 SPEED 32.0");
        assert_eq!(lines[3], "OCC ON  LIGHT");
        assert!(lines.iter().any(|line| line == "BUDGET 8.3MS FRAME 16.7MS"));
        assert!(lines.iter().any(|line| line == "OVER 3/1/0 WORST 33.4"));

        let mut ui = NativeUi::new(1);
        ui.set_scale(GuiScale::from_pixels(960, 540));
        let mut draw = GuiDrawList::new();
        ui.render_debug_pane(&mut draw, &stats);
        assert!(!draw.commands().is_empty());
    }

    #[test]
    fn options_radius_slider_sets_chunk_radius() {
        let mut ui = NativeUi::new(1);
        ui.set_screen(Some(NativeScreen::Options {
            parent: OptionsParent::Pause,
        }));
        ui.set_scale(GuiScale::from_pixels(960, 540));

        let radius = option_widgets(ui.scale).radius;
        let point = Point {
            x: radius.right() - 0.1,
            y: radius.y + radius.height * 0.5,
        };
        assert!(ui.pointer_down(point));
        let (_handled, action) = ui.pointer_up(point);

        let action = action.expect("radius slider release should produce an action");
        assert_eq!(action, NativeUiAction::SetChunkRadius(MAX_CHUNK_RADIUS));
        ui.apply_action(action);
        assert_eq!(ui.chunk_radius, MAX_CHUNK_RADIUS);

        let point = Point {
            x: radius.x,
            y: radius.y + radius.height * 0.5,
        };
        assert!(ui.pointer_down(point));
        let (_handled, action) = ui.pointer_up(point);

        let action = action.expect("radius slider release should produce an action");
        assert_eq!(action, NativeUiAction::SetChunkRadius(MIN_UI_CHUNK_RADIUS));
        ui.apply_action(action);
        assert_eq!(ui.chunk_radius, MIN_UI_CHUNK_RADIUS);
    }

    #[test]
    fn window_runtime_streams_chunks_when_spectator_crosses_boundary() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let initial_center = ChunkPos::new(0, 0);
        let initial_stats = runtime.stats();
        assert_eq!(initial_stats.interest_center, initial_center);
        assert_eq!(initial_stats.loaded_chunks, 1);
        assert_eq!(initial_stats.pending_jobs, 0);
        assert!(runtime.client.chunk_snapshot(initial_center).is_some());

        let mut spectator = SpectatorCamera::spawn_for_scene(&scene);
        let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
        assert_eq!(initial_submit.rebuilt_section_count(), 0);
        assert!(initial_submit.submitted_compile_section_count > 0);
        assert!(runtime.stats().pending_render_compile_jobs > 0);

        let initial_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(initial_update.rebuilt_section_count() > 0);
        assert_eq!(initial_update.removed_section_count(), 0);
        let initial_sections = runtime.cached_sections();
        assert!(!initial_sections.is_empty());
        assert!(section_index_count(&initial_sections) > 0);
        assert!(
            initial_sections
                .iter()
                .all(|section| section.key.chunk_x == 0 && section.key.chunk_z == 0)
        );

        spectator.position.x = 16.25;
        let next_center = spectator.chunk_pos();
        assert_eq!(next_center, ChunkPos::new(1, 0));
        assert!(runtime.set_interest_center(next_center).unwrap());
        assert_eq!(runtime.stats().interest_center, next_center);
        assert!(runtime.client.chunk_snapshot(initial_center).is_none());

        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let moved_stats = runtime.stats();
        assert_eq!(moved_stats.interest_center, next_center);
        assert_eq!(moved_stats.loaded_chunks, 1);
        assert_eq!(moved_stats.pending_jobs, 0);
        assert!(runtime.client.chunk_snapshot(initial_center).is_none());
        assert!(runtime.client.chunk_snapshot(next_center).is_some());

        let moved_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(moved_update.rebuilt_section_count() > 0);
        assert!(moved_update.removed_section_count() > 0);
        let moved_sections = runtime.cached_sections();
        assert!(!moved_sections.is_empty());
        assert!(section_index_count(&moved_sections) > 0);
        assert!(
            moved_sections
                .iter()
                .all(|section| section.key.chunk_x == 1 && section.key.chunk_z == 0)
        );
    }

    #[test]
    fn window_runtime_defers_far_boundary_sections_without_neighbors() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();

        assert!(update.rebuilt_section_count() > 0);
        assert!(update.near_exception_section_count > 0);
        assert!(update.deferred_section_count > 0);
        assert!(runtime.pending_render_chunk_count() > 0);
        assert!(!runtime.has_pending_render_work(spectator.position));
        assert!(runtime.cached_sections().iter().all(|section| {
            render_section_distance_sq(section.key, spectator.position)
                <= RENDER_NEIGHBOR_READY_DISTANCE_SQ
        }));

        let deferred_key = *runtime
            .dirty_render_sections
            .iter()
            .next()
            .expect("deferred dirty section should be retained");
        let ready_position = render_section_center(deferred_key);
        assert!(runtime.has_pending_render_work(ready_position));
        let ready_update = runtime.sync_all_render_sections(ready_position).unwrap();
        assert!(ready_update.rebuilt_section_count() > 0);
        assert!(!runtime.dirty_render_sections.contains(&deferred_key));
    }

    #[test]
    fn window_runtime_marks_section_block_updates_without_chunk_dirtying() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = WindowSceneRuntime::new(&SceneOptions::default()).unwrap();
        runtime.dirty_render_chunks.clear();
        runtime.dirty_render_sections.clear();

        runtime.apply_server_updates(vec![ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 7,
            updates: vec![SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: AIR_BLOCK_STATE_ID,
            }],
        }]);

        assert!(runtime.dirty_render_chunks.is_empty());
        assert_eq!(
            runtime.dirty_render_sections,
            BTreeSet::from([RenderSectionKey::new(0, 7, 0)])
        );
    }

    #[test]
    fn render_compile_scheduling_orders_dirty_chunks_by_camera_distance() {
        let camera = Vec3::new(8.0, 88.0, 8.0);
        let sorted = sort_chunk_positions_by_distance(
            [
                ChunkPos::new(4, 0),
                ChunkPos::new(0, 0),
                ChunkPos::new(-2, 0),
            ],
            camera,
        );

        assert_eq!(
            sorted,
            vec![
                ChunkPos::new(0, 0),
                ChunkPos::new(-2, 0),
                ChunkPos::new(4, 0)
            ]
        );
    }

    #[test]
    fn render_compile_revisions_stale_only_changed_sections() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
        assert!(initial_submit.submitted_compile_section_count > 1);
        let changed_key = RenderSectionKey::new(0, 5, 0);
        assert!(runtime.inflight_render_sections.contains(&changed_key));

        runtime.apply_server_updates(vec![ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 5,
            updates: vec![SectionBlockUpdate {
                local_x: 8,
                local_y: 8,
                local_z: 8,
                block_state: AIR_BLOCK_STATE_ID,
            }],
        }]);

        let completed = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();

        assert_eq!(completed.stale_compile_section_count, 1);
        assert!(
            completed.completed_compile_section_count
                >= initial_submit.submitted_compile_section_count
        );
        assert!(!runtime.has_pending_render_work(spectator.position));
    }

    #[test]
    fn window_runtime_updates_chunk_radius_live() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.radius_chunks, 0);
        assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

        assert!(runtime.set_radius_chunks(1).unwrap());
        assert_eq!(runtime.radius_chunks, 1);
        assert_eq!(runtime.client.chunk_interest().unwrap().radius_chunks, 1);
        poll_window_runtime_until_idle(&mut runtime).unwrap();
        assert_eq!(runtime.stats().loaded_chunks, square_count(1).unwrap());

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let grown_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(grown_update.rebuilt_section_count() > 0);
        assert_eq!(grown_update.removed_section_count(), 0);
        assert!(runtime.cached_sections().iter().any(|section| {
            section.key.chunk_x != scene.chunk_x || section.key.chunk_z != scene.chunk_z
        }));

        assert!(runtime.set_radius_chunks(0).unwrap());
        poll_window_runtime_until_idle(&mut runtime).unwrap();
        assert_eq!(runtime.radius_chunks, 0);
        assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

        let shrunk_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(shrunk_update.removed_section_count() > 0);
        assert!(runtime.cached_sections().iter().all(|section| {
            section.key.chunk_x == scene.chunk_x && section.key.chunk_z == scene.chunk_z
        }));
    }

    #[test]
    fn window_runtime_mesh_queue_processes_ready_work_by_chunk_budget() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            chunk_radius: 1,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.stats().loaded_chunks, 9);
        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let first_update = runtime.sync_render_sections(spectator.position).unwrap();
        assert_eq!(first_update.rebuilt_section_count(), 0);
        assert!(first_update.submitted_compile_section_count > 0);
        assert!(runtime.stats().pending_render_compile_jobs > 0);
        assert!(runtime.pending_render_chunk_count() > 0);

        let first_sections = runtime.cached_sections();
        assert!(first_sections.is_empty());

        let remaining_update = runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        assert!(remaining_update.rebuilt_section_count() > 0);
        assert!(!runtime.has_pending_render_work(spectator.position));
        assert!(!runtime.cached_sections().is_empty());
    }

    #[test]
    fn cli_defaults_to_window() {
        assert_eq!(
            Cli::parse([]).unwrap(),
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
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
    fn cli_parses_headless_ui_dimensions() {
        let cli = Cli::parse([
            "--headless-ui".to_owned(),
            "/tmp/mclone-ui.png".to_owned(),
            "--width".to_owned(),
            "960".to_owned(),
            "--height".to_owned(),
            "540".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessUi {
                path: PathBuf::from("/tmp/mclone-ui.png"),
                width: 960,
                height: 540,
            }
        );
    }

    #[test]
    fn cli_parses_full_frame_screenshot_options() {
        let cli = Cli::parse([
            "--screenshot".to_owned(),
            "/tmp/mclone-frame.png".to_owned(),
            "--width".to_owned(),
            "960".to_owned(),
            "--height".to_owned(),
            "540".to_owned(),
            "--screenshot-ui".to_owned(),
            "pause".to_owned(),
            "--screenshot-debug-pane".to_owned(),
            "true".to_owned(),
            "--force-fullbright".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessScreenshot {
                options: HeadlessScreenshotOptions {
                    path: PathBuf::from("/tmp/mclone-frame.png"),
                    width: 960,
                    height: 540,
                    scene: SceneOptions::default(),
                    render_options: TexturedSectionRenderOptions {
                        force_fullbright: true,
                        ..TexturedSectionRenderOptions::default()
                    },
                    ui: HeadlessScreenshotUi::Pause,
                    debug_pane: true,
                },
            }
        );
    }

    #[test]
    fn parse_screenshot_ui_accepts_named_screens() {
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("none".to_owned())).unwrap(),
            HeadlessScreenshotUi::None
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("title".to_owned())).unwrap(),
            HeadlessScreenshotUi::Title
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("options-title".to_owned())).unwrap(),
            HeadlessScreenshotUi::OptionsTitle
        );
        assert_eq!(
            parse_screenshot_ui_arg("--screenshot-ui", Some("options".to_owned())).unwrap(),
            HeadlessScreenshotUi::OptionsPause
        );
        assert!(parse_screenshot_ui_arg("--screenshot-ui", Some("bad".to_owned())).is_err());
    }

    #[test]
    fn native_ui_has_title_and_ingame_start_modes() {
        let title_ui = NativeUi::new(1);
        assert!(title_ui.is_active());
        assert!(title_ui.covers_world());

        let ingame_ui = NativeUi::new_ingame(1);
        assert!(!ingame_ui.is_active());
        assert!(!ingame_ui.covers_world());
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
                    chunk_radius: DEFAULT_CHUNK_RADIUS,
                    remote_addr: None,
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_chunk_radius() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--chunk-radius".to_owned(),
            "16".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    chunk_radius: 16,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_section_occlusion_toggle() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--section-occlusion".to_owned(),
            "false".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    ..TexturedSectionRenderOptions::default()
                },
            }
        );

        let cli = Cli::parse([
            "--disable-section-occlusion".to_owned(),
            "--enable-section-occlusion".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_fullbright_toggle() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--fullbright".to_owned(),
            "true".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions {
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
            }
        );

        let cli = Cli::parse([
            "--force-fullbright".to_owned(),
            "--disable-fullbright".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Window {
                scene: SceneOptions::default(),
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_movement_perf_options() {
        let cli = Cli::parse([
            "--movement-perf".to_owned(),
            "--movement-steps".to_owned(),
            "6".to_owned(),
            "--path-radius".to_owned(),
            "3".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "800".to_owned(),
            "--height".to_owned(),
            "600".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::MovementPerf {
                options: MovementPerfOptions {
                    scene: SceneOptions {
                        chunk_radius: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 800,
                    height: 600,
                    steps: 6,
                    path_radius_chunks: 3,
                },
            }
        );
    }

    #[test]
    fn cli_parses_timedemo_options() {
        let cli = Cli::parse([
            "--timedemo".to_owned(),
            "--timedemo-frames".to_owned(),
            "48".to_owned(),
            "--path-radius".to_owned(),
            "5".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "1024".to_owned(),
            "--height".to_owned(),
            "768".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::Timedemo {
                options: TimedemoOptions {
                    scene: SceneOptions {
                        chunk_radius: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    frames: 48,
                    path_radius_chunks: 5,
                },
            }
        );
    }

    #[test]
    fn cli_parses_frame_budget_probe_options() {
        let cli = Cli::parse([
            "--frame-budget-probe".to_owned(),
            "--frame-budget-frames".to_owned(),
            "36".to_owned(),
            "--target-hz".to_owned(),
            "120".to_owned(),
            "--path-radius".to_owned(),
            "5".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "1024".to_owned(),
            "--height".to_owned(),
            "768".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    scene: SceneOptions {
                        chunk_radius: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    mode: FrameBudgetProbeMode::StressOrbit,
                    frames: 36,
                    path_radius_chunks: 5,
                    target_hz: 120.0,
                    movement_speed: SPECTATOR_BASE_SPEED,
                },
            }
        );
    }

    #[test]
    fn cli_target_hz_selects_frame_budget_probe() {
        let cli = Cli::parse(["--target-hz".to_owned(), "90".to_owned()]).unwrap();

        assert_eq!(
            cli,
            Cli::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    target_hz: 90.0,
                    ..FrameBudgetProbeOptions::default()
                },
            }
        );
    }

    #[test]
    fn cli_parses_movement_frame_probe_options() {
        let cli = Cli::parse([
            "--target-hz".to_owned(),
            "120".to_owned(),
            "--movement-frame-probe".to_owned(),
            "--frame-budget-frames".to_owned(),
            "36".to_owned(),
            "--movement-frame-speed".to_owned(),
            "48".to_owned(),
            "--path-radius".to_owned(),
            "5".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
            "--width".to_owned(),
            "1024".to_owned(),
            "--height".to_owned(),
            "768".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::FrameBudgetProbe {
                options: FrameBudgetProbeOptions {
                    scene: SceneOptions {
                        chunk_radius: 2,
                        ..SceneOptions::default()
                    },
                    render_options: TexturedSectionRenderOptions::default(),
                    width: 1024,
                    height: 768,
                    mode: FrameBudgetProbeMode::MovementWalk,
                    frames: 36,
                    path_radius_chunks: 5,
                    target_hz: 120.0,
                    movement_speed: 48.0,
                },
            }
        );
    }

    #[test]
    fn cli_parses_headless_chunk_scenarios() {
        let cli = Cli::parse([
            "--headless-chunk-scenarios".to_owned(),
            "/tmp/mclone-camera".to_owned(),
            "--width".to_owned(),
            "320".to_owned(),
            "--height".to_owned(),
            "180".to_owned(),
            "--chunk-radius".to_owned(),
            "2".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunkScenarios {
                directory: PathBuf::from("/tmp/mclone-camera"),
                width: 320,
                height: 180,
                scene: SceneOptions {
                    chunk_radius: 2,
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn cli_parses_remote_addr() {
        let cli = Cli::parse([
            "--headless-chunk".to_owned(),
            "/tmp/mclone-chunk.png".to_owned(),
            "--remote-addr".to_owned(),
            "127.0.0.1:25565".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli::HeadlessChunk {
                path: PathBuf::from("/tmp/mclone-chunk.png"),
                width: 640,
                height: 480,
                scene: SceneOptions {
                    remote_addr: Some("127.0.0.1:25565".to_owned()),
                    ..SceneOptions::default()
                },
                render_options: TexturedSectionRenderOptions::default(),
            }
        );
    }

    #[test]
    fn scene_chunk_positions_cover_square_radius() {
        let positions = SceneOptions {
            seed: 0,
            chunk_x: -2,
            chunk_z: 3,
            chunk_radius: 1,
            remote_addr: None,
        }
        .chunk_positions()
        .collect::<Vec<_>>();

        assert_eq!(positions.len(), 9);
        assert!(positions.contains(&(-3, 2)));
        assert!(positions.contains(&(-2, 3)));
        assert!(positions.contains(&(-1, 4)));
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_integrated_server() {
        let scene = SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        };
        let client = build_scene_client_runtime(&scene).unwrap();

        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn scene_client_runtime_loads_center_chunk_from_remote_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let command = mclone_net::read_client_command_frame(&mut stream).unwrap();
            let mut server = IntegratedServer::new(DEFAULT_SEED);
            let mut updates = server.try_handle_command(command).unwrap();
            updates.extend(poll_integrated_server_until_idle(&mut server).unwrap());
            mclone_net::write_server_update_batch(&mut stream, &updates).unwrap();
        });
        let scene = SceneOptions {
            chunk_radius: 0,
            remote_addr: Some(addr.to_string()),
            ..SceneOptions::default()
        };

        let client = build_scene_client_runtime(&scene).unwrap();
        server.join().unwrap();

        assert_eq!(client.host(), ClientHost::RemoteDedicated);
        assert_eq!(client.loaded_chunk_count(), 1);
        assert!(
            client
                .chunk_snapshot(ChunkPos::new(scene.chunk_x, scene.chunk_z))
                .is_some()
        );
    }

    #[test]
    fn build_scene_textured_sections_uses_client_runtime_snapshots() {
        if !extracted_asset_root().exists() {
            return;
        }
        let scene_mesh = build_scene_textured_sections(&SceneOptions {
            chunk_radius: 0,
            ..SceneOptions::default()
        })
        .unwrap();

        assert!(scene_mesh.section_count() > 0);
        assert!(scene_mesh.index_count() > 0);
        assert!(scene_mesh.atlas.width > 0);
        assert!(scene_mesh.atlas.height > 0);
    }

    #[test]
    fn snapshot_mesh_block_state_ids_rehydrates_omitted_air_sections() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        block_state_ids[CHUNK_SECTION_VOLUME] = BlockStateId(1);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            32,
            &block_state_ids,
        );

        let mesh_blocks = snapshot_mesh_block_state_ids(&snapshot).unwrap();

        assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
        assert_eq!(mesh_blocks.blocks[0], AIR_BLOCK_STATE_ID);
        assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], BlockStateId(1));
    }

    fn poll_window_runtime_until_idle(runtime: &mut WindowSceneRuntime) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            runtime.poll()?;
            if runtime.pending_job_count() == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for window runtime worldgen jobs");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn section_index_count(sections: &[TexturedRenderSectionMesh]) -> u32 {
        sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}
