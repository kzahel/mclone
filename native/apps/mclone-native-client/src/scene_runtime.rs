use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::{
    AIR_BLOCK_STATE_ID, ChunkPos, ChunkSnapshot, SECTION_HEIGHT, block_to_chunk_coord,
    block_to_section_coord, chunk_block_coord, chunk_middle_block_coord, local_block_coord,
    local_section_block_coord,
};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
use mclone_net::LocalTransport;
use mclone_protocol::{
    ChunkView, ClientCommand, PlayerPositionUpdate, SectionBlockUpdate, ServerUpdate,
};
use mclone_server::IntegratedServer;

use crate::MIN_RENDER_DISTANCE;
use crate::actor_assets::{ActorTextureAssets, load_actor_texture_assets};
use crate::cli::SceneOptions;
use crate::frame_pacing::{elapsed_ms, micros_to_ms};
use crate::remote_session::RemoteServerSession;
use crate::render_cache::{
    RenderSectionCompileWorker, SceneTexturedSections, TexturedMeshAssets,
    load_textured_mesh_assets,
};
use mclone_render_session::{
    RenderSectionCacheUpdate, RenderSectionCompiler, RenderSectionNeighborReadiness,
    RenderSectionReadyPlan, RenderSectionSession, build_client_textured_sections,
    render_dirty_chunk_neighborhood, render_section_chunk_pos, render_section_keys_for_snapshot,
    snapshot_contains_render_section,
};

const DEFAULT_RENDER_CHUNK_MESH_BUDGET: usize = 1;
const RENDER_NEIGHBOR_READY_DISTANCE_SQ: f32 = 24.0 * 24.0;

pub(crate) fn square_count(radius: i32) -> Result<usize> {
    if radius < 0 {
        bail!("radius must be non-negative");
    }
    let side = usize::try_from(radius)
        .context("radius exceeds usize")?
        .saturating_mul(2)
        .saturating_add(1);
    Ok(side * side)
}

pub(crate) fn chunk_tracking_radius_for_render_distance(render_distance: u32) -> u32 {
    // Java 1.17.1 exposes render distance with a minimum of 2, then the
    // integrated server path arrives at ChunkMap's clamped minimum of 3.
    if render_distance < MIN_RENDER_DISTANCE as u32 {
        return render_distance;
    }
    render_distance.max(3)
}

fn scene_render_distance(scene: &SceneOptions) -> Result<u32> {
    u32::try_from(scene.render_distance).context("render distance must be non-negative")
}

fn chunk_view(center: ChunkPos, render_distance: u32) -> ChunkView {
    ChunkView {
        center,
        render_distance,
        chunk_tracking_radius: chunk_tracking_radius_for_render_distance(render_distance),
    }
}

pub(crate) fn build_scene_textured_sections(scene: &SceneOptions) -> Result<SceneTexturedSections> {
    let client = build_scene_client_runtime(scene)?;
    let mesh_assets = load_textured_mesh_assets()?;
    let build = build_client_textured_sections(&client, &mesh_assets.catalog)?;
    if build.sections.is_empty() {
        bail!(
            "generated chunk area seed={} center=({}, {}) render_distance={} produced no textured render sections",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.render_distance
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
    let render_distance = scene_render_distance(scene)?;

    let command = client.set_chunk_view(chunk_view(
        ChunkPos::new(scene.chunk_x, scene.chunk_z),
        render_distance,
    ));

    if let Some(remote_addr) = &scene.remote_addr {
        let mut session = RemoteServerSession::connect(remote_addr.as_str())?;
        let updates = session.send_command(command)?;
        client.apply_updates(updates);
    } else {
        let mut server = IntegratedServer::new(scene.seed);
        server.set_lighting_enabled(scene.lighting_enabled);
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
pub(crate) struct WindowSceneRuntime {
    pub(crate) client: ClientRuntime,
    server: Option<IntegratedServer>,
    transport: LocalTransport,
    remote_session: Option<RemoteServerSession>,
    pub(crate) render_distance: u32,
    pub(crate) chunk_tracking_radius: u32,
    pub(crate) interest_center: ChunkPos,
    pub(crate) mesh_assets: TexturedMeshAssets,
    pub(crate) actor_textures: ActorTextureAssets,
    render_session: RenderSectionSession,
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
pub(crate) struct RuntimePollDiagnostics {
    pub(crate) flush_commands_ms: f64,
    pub(crate) server_tick_ms: f64,
    pub(crate) server_reported_total_ms: f64,
    pub(crate) scheduler_tick_ms: f64,
    pub(crate) scheduler_report_ms: f64,
    pub(crate) scheduler_purge_stale_tickets_ms: f64,
    pub(crate) scheduler_reconcile_holders_ms: f64,
    pub(crate) scheduler_publish_completed_ms: f64,
    pub(crate) scheduler_pending_unload_ms: f64,
    pub(crate) scheduler_apply_events_ms: f64,
    pub(crate) block_tick_ms: f64,
    pub(crate) fluid_tick_ms: f64,
    pub(crate) fluid_event_apply_ms: f64,
    pub(crate) fluid_due_scan_ms: f64,
    pub(crate) fluid_remove_due_ms: f64,
    pub(crate) fluid_tick_fluid_ms: f64,
    pub(crate) fluid_set_block_ms: f64,
    pub(crate) entity_tick_ms: f64,
    pub(crate) apply_updates_ms: f64,
    pub(crate) dirty_mark_ms: f64,
    pub(crate) client_apply_updates_ms: f64,
    pub(crate) scheduler_events: usize,
    pub(crate) updates: usize,
    pub(crate) snapshot_updates: usize,
    pub(crate) section_block_updates: usize,
    pub(crate) unload_updates: usize,
    pub(crate) pending_unloads_processed: usize,
    pub(crate) fluid_due_ticks: usize,
    pub(crate) fluid_executed_ticks: usize,
    pub(crate) fluid_deferred_ticks: usize,
    pub(crate) fluid_mutated_blocks: usize,
    pub(crate) fluid_snapshot_events: usize,
    pub(crate) fluid_event_count: usize,
    pub(crate) scheduled_fluid_ticks: usize,
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
pub(crate) struct WindowRuntimeStats {
    pub(crate) interest_center: ChunkPos,
    pub(crate) render_distance: u32,
    pub(crate) chunk_tracking_radius: u32,
    pub(crate) loaded_chunks: usize,
    pub(crate) pending_jobs: usize,
    pub(crate) pending_publications: usize,
    pub(crate) pending_render_chunks: usize,
    pub(crate) pending_render_compile_jobs: usize,
    pub(crate) inflight_render_sections: usize,
    pub(crate) client_visible_chunks: usize,
    pub(crate) active_ticket_chunks: usize,
    pub(crate) tracked_players: usize,
    pub(crate) player_visible_chunks: usize,
    pub(crate) aggregate_player_ticket_chunks: usize,
    pub(crate) player_outbound_queue_depth: usize,
    pub(crate) max_player_visible_chunks: usize,
    pub(crate) max_player_outbound_queue_depth: usize,
    pub(crate) pending_unload_chunks: usize,
    pub(crate) block_ticking_chunks: usize,
    pub(crate) entity_ticking_chunks: usize,
    pub(crate) last_tick: u64,
    pub(crate) last_simulation_tick: u64,
    pub(crate) last_tick_unloads_processed: usize,
    pub(crate) last_simulation_block_tick_chunks: usize,
    pub(crate) last_simulation_entity_tick_chunks: usize,
    pub(crate) last_simulation_scheduler_tick_ms: f64,
    pub(crate) last_simulation_block_tick_ms: f64,
    pub(crate) last_simulation_fluid_tick_ms: f64,
    pub(crate) last_simulation_entity_tick_ms: f64,
    pub(crate) last_simulation_fluid_ticks_executed: usize,
    pub(crate) last_simulation_deferred_fluid_ticks: usize,
    pub(crate) last_simulation_fluid_mutated_blocks: usize,
    pub(crate) scheduled_fluid_ticks: usize,
}

impl WindowSceneRuntime {
    pub(crate) fn new(scene: &SceneOptions) -> Result<Self> {
        let client = if scene.remote_addr.is_some() {
            ClientRuntime::new(ClientHost::RemoteDedicated)
        } else {
            ClientRuntime::local_integrated()
        };
        let render_distance = scene_render_distance(scene)?;
        let chunk_tracking_radius = chunk_tracking_radius_for_render_distance(render_distance);
        let mesh_assets = load_textured_mesh_assets()?;
        let actor_textures = load_actor_texture_assets()?;
        let render_compile_worker = RenderSectionCompileWorker::new(mesh_assets.catalog.clone())?;
        let remote_session = scene
            .remote_addr
            .as_deref()
            .map(RemoteServerSession::connect)
            .transpose()?;
        let server = if remote_session.is_none() {
            let mut server = IntegratedServer::new(scene.seed);
            server.set_lighting_enabled(scene.lighting_enabled);
            Some(server)
        } else {
            None
        };
        let mut runtime = Self {
            client,
            server,
            transport: LocalTransport::new(),
            remote_session,
            render_distance,
            chunk_tracking_radius,
            interest_center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
            mesh_assets,
            actor_textures,
            render_session: RenderSectionSession::default(),
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
        runtime.set_chunk_view(
            runtime.interest_center,
            runtime.render_distance,
            runtime.chunk_tracking_radius,
        )?;
        // Debug day/night controls. `--freeze-time` stops the integrated server
        // from advancing the clock; `--day-time` forces a starting tick (and seeds
        // the client so the first frame is correct before the first server tick).
        if scene.freeze_time {
            if let Some(server) = runtime.server.as_mut() {
                server.set_day_time_frozen(true);
            }
        }
        if let Some(day_time) = scene.day_time_override {
            if let Some(server) = runtime.server.as_mut() {
                server.set_day_time(day_time);
            }
            runtime.force_day_time(day_time);
        }
        Ok(runtime)
    }

    pub(crate) fn set_interest_center(&mut self, center: ChunkPos) -> Result<bool> {
        self.set_chunk_view(center, self.render_distance, self.chunk_tracking_radius)
    }

    pub(crate) fn set_render_distance(&mut self, render_distance: u32) -> Result<bool> {
        self.set_chunk_view(
            self.interest_center,
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
    }

    fn set_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<bool> {
        if self.client.chunk_view().is_some_and(|view| {
            view.center == center
                && view.render_distance == render_distance
                && view.chunk_tracking_radius == chunk_tracking_radius
        }) {
            self.interest_center = center;
            self.render_distance = render_distance;
            self.chunk_tracking_radius = chunk_tracking_radius;
            return Ok(false);
        }

        self.interest_center = center;
        self.render_distance = render_distance;
        self.chunk_tracking_radius = chunk_tracking_radius;
        let command = self.client.set_chunk_view(ChunkView {
            center,
            render_distance,
            chunk_tracking_radius,
        });

        self.dispatch_client_command(command)
    }

    pub(crate) fn send_gameplay_command(&mut self, command: ClientCommand) -> Result<bool> {
        self.dispatch_client_command(command)
    }

    pub(crate) fn drain_player_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.client.drain_player_position_updates().collect()
    }

    fn dispatch_client_command(&mut self, command: ClientCommand) -> Result<bool> {
        if self.remote_session.is_some() {
            return self.dispatch_remote_client_command(command);
        }

        self.transport.send_client_command(command);
        self.flush_local_commands()
    }

    fn dispatch_remote_client_command(&mut self, command: ClientCommand) -> Result<bool> {
        let remote_session = self
            .remote_session
            .as_mut()
            .expect("remote dispatch requires a remote session");
        match remote_session.send_command(command) {
            Ok(updates) => {
                let changed = !updates.is_empty();
                self.apply_server_updates(updates);
                Ok(changed)
            }
            Err(err) => self.reconnect_remote_session_and_resync(err),
        }
    }

    fn reconnect_remote_session_and_resync(&mut self, err: anyhow::Error) -> Result<bool> {
        let Some(view) = self.client.chunk_view().cloned() else {
            bail!("remote server command failed before a chunk view was established: {err:#}");
        };

        log::warn!("remote server command failed; reconnecting and resyncing chunk view: {err:#}");
        self.clear_remote_replica_for_resync();
        let remote_session = self
            .remote_session
            .as_mut()
            .expect("remote resync requires a remote session");
        remote_session.reconnect()?;
        let updates = remote_session
            .send_command(ClientCommand::SetChunkView(view))
            .context("failed to resync chunk view after reconnect")?;
        self.apply_server_updates(updates);
        Ok(true)
    }

    fn clear_remote_replica_for_resync(&mut self) {
        let stale_chunks = self.client.loaded_chunk_positions().collect::<Vec<_>>();
        self.client.clear_server_replica();
        for pos in stale_chunks {
            self.mark_render_chunk_dirty(pos);
        }
    }

    pub(crate) fn poll(&mut self) -> Result<bool> {
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
        // Time updates arrive every tick and animate the sky, but they do not
        // dirty any chunk geometry; the renderer reads time-of-day each frame, so
        // they must not by themselves force a section re-upload.
        let changed = updates.iter().any(|update| {
            !matches!(
                update,
                ServerUpdate::TimeUpdate { .. }
                    | ServerUpdate::RemotePlayerAdd(_)
                    | ServerUpdate::RemotePlayerUpdate(_)
                    | ServerUpdate::RemotePlayerRemove { .. }
                    | ServerUpdate::EntitySnapshot(_)
                    | ServerUpdate::EntityUpdate(_)
                    | ServerUpdate::EntityRemove { .. }
            )
        });
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
                ServerUpdate::TimeUpdate { .. } => {}
                ServerUpdate::PlayerPosition(_) => {}
                ServerUpdate::RemotePlayerAdd(_)
                | ServerUpdate::RemotePlayerUpdate(_)
                | ServerUpdate::RemotePlayerRemove { .. }
                | ServerUpdate::EntitySnapshot(_)
                | ServerUpdate::EntityUpdate(_)
                | ServerUpdate::EntityRemove { .. } => {}
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
            let known_keys = self.known_render_section_keys_for_chunk(dirty_pos);
            self.render_session.mark_chunk_dirty(dirty_pos, known_keys);
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
                self.render_session.mark_section_dirty(key);
            }
        }
    }

    fn known_render_section_keys_for_chunk(&self, pos: ChunkPos) -> BTreeSet<RenderSectionKey> {
        let mut keys = BTreeSet::new();
        if let Some(snapshot) = self.client.chunk_snapshot(pos) {
            keys.extend(render_section_keys_for_snapshot(snapshot));
        }
        keys.extend(
            self.render_session
                .section_keys()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.render_session
                .dirty()
                .dirty_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.render_session
                .dirty()
                .inflight_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys
    }

    fn drain_completed_render_compile_jobs(&mut self) -> Result<RenderSectionCacheUpdate> {
        let mut update = RenderSectionCacheUpdate::default();
        for completed in self.render_compile_worker.try_recv_completed()? {
            let finish = self.render_session.finish_compile_result(completed, 0)?;
            if !finish.stale_sections.is_empty() {
                update.stale_compile_section_count += finish.stale_sections.len();
                self.requeue_stale_render_sections(finish.stale_sections);
            }
            let Some(build_report) = finish.build_report else {
                continue;
            };
            let completed_update = self.render_session.apply_finished_compile_report(
                &finish.accepted_sections,
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
            if snapshot_contains || self.render_session.contains_section(key) {
                self.render_session.dirty_mut().dirty_sections.insert(key);
            }
        }
    }

    fn submit_render_compile_plan(&mut self, ready_plan: &RenderSectionReadyPlan) -> Result<usize> {
        let snapshots = self.client.chunk_snapshots().cloned().collect();
        let Some(request) = self
            .render_session
            .build_ready_plan_compile_request(ready_plan, snapshots)
        else {
            return Ok(0);
        };
        let submitted_count = request.target_sections.len();
        self.render_compile_worker.submit(request)?;
        self.render_session
            .accept_ready_plan_compile_submission(ready_plan);
        Ok(submitted_count)
    }

    pub(crate) fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_with_budget(camera_position, DEFAULT_RENDER_CHUNK_MESH_BUDGET)
    }

    pub(crate) fn sync_all_render_sections(
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

        if self.render_session.cache_is_empty()
            && self.client.loaded_chunk_count() > 0
            && self.render_session.dirty_is_empty()
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

        if self.render_session.dirty_work_is_empty() {
            report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
            return Ok(report);
        }

        let dirty_work = self.render_session.classify_dirty_work(
            |pos| self.client.chunk_snapshot(pos).is_some(),
            |pos| self.render_session.contains_chunk(pos),
            |key| {
                let pos = render_section_chunk_pos(key);
                self.client
                    .chunk_snapshot(pos)
                    .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            },
            |key| self.render_session.contains_section(key),
        );
        let sorted_loaded_dirty_chunks = sort_chunk_positions_by_distance(
            dirty_work.loaded_dirty_chunks.iter().copied(),
            camera_position,
        );
        let sorted_dirty_section_chunks = sort_dirty_section_chunks_by_distance(
            &dirty_work.loaded_dirty_sections_by_chunk,
            camera_position,
        );
        let sync_plan = self.render_session.prepare_sync_plan(
            dirty_work,
            sorted_loaded_dirty_chunks,
            sorted_dirty_section_chunks,
            chunk_budget,
            |pos| {
                self.client
                    .chunk_snapshot(pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            },
            |key| render_section_neighbor_readiness(&self.client, key, camera_position),
        );

        if sync_plan.has_removals() {
            let removal_update = self.render_session.apply_removals(
                &sync_plan.dirty_work.removal_dirty_chunks,
                &sync_plan.dirty_work.removal_dirty_sections,
            );
            report.merge(removal_update);
        }

        if chunk_budget == 0 || self.render_compile_worker.pending_job_count() > 0 {
            report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
            return Ok(report);
        }

        if sync_plan.ready_plan.ready_section_keys.is_empty() {
            self.render_session.apply_ready_plan(&sync_plan.ready_plan);
            report.merge(sync_plan.ready_update(0));
            report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
            return Ok(report);
        }

        let submitted_compile_section_count =
            self.submit_render_compile_plan(&sync_plan.ready_plan)?;
        report.merge(sync_plan.ready_update(submitted_compile_section_count));
        report.pending_compile_jobs = self.render_compile_worker.pending_job_count();
        Ok(report)
    }

    pub(crate) fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.render_session.sections()
    }

    /// Sky clear color for the current day-time, driving the day/night gradient.
    pub(crate) fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.client.time_of_day())
    }

    /// Celestial phase in `[0, 1)` for the current day-time (noon = 0).
    pub(crate) fn time_of_day(&self) -> f32 {
        self.client.time_of_day()
    }

    /// Celestial rig rotation in radians for the current day-time.
    pub(crate) fn sun_angle(&self) -> f32 {
        self.client.sun_angle()
    }

    /// Force the client day/night clock to a specific `dayTime`. Debug-only hook
    /// for captures; the next server tick will overwrite it with authoritative
    /// time.
    pub(crate) fn force_day_time(&mut self, day_time: u64) {
        self.client
            .apply_update(ServerUpdate::TimeUpdate { day_time });
    }

    pub(crate) fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.render_session
            .section_keys()
            .filter(|key| {
                self.render_section_within_render_distance(*key)
                    && render_section_neighbor_readiness(&self.client, *key, camera_position)
                        .is_ready()
            })
            .collect()
    }

    fn render_section_within_render_distance(&self, key: RenderSectionKey) -> bool {
        let distance = i32::try_from(self.render_distance).unwrap_or(i32::MAX);
        (key.chunk_x - self.interest_center.x)
            .abs()
            .max((key.chunk_z - self.interest_center.z).abs())
            <= distance
    }

    pub(crate) fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.render_compile_worker.pending_job_count() > 0
            || self.has_ready_pending_render_work(camera_position)
    }

    pub(crate) fn pending_render_chunk_count(&self) -> usize {
        let mut pending = self
            .render_session
            .dirty()
            .dirty_chunks
            .iter()
            .filter(|pos| {
                self.client.chunk_snapshot(**pos).is_some()
                    || self.render_session.contains_chunk(**pos)
            })
            .copied()
            .collect::<BTreeSet<_>>();
        pending.extend(
            self.render_session
                .dirty()
                .dirty_sections
                .iter()
                .copied()
                .filter(|key| {
                    let pos = render_section_chunk_pos(*key);
                    self.client.chunk_snapshot(pos).is_some()
                        || self.render_session.contains_section(*key)
                })
                .map(render_section_chunk_pos),
        );
        pending.extend(
            self.render_session
                .dirty()
                .inflight_sections
                .iter()
                .copied()
                .map(render_section_chunk_pos),
        );
        pending.len()
    }

    fn has_ready_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.render_session
            .dirty()
            .dirty_chunks
            .iter()
            .copied()
            .any(|pos| {
                if self.render_session.contains_chunk(pos)
                    && self.client.chunk_snapshot(pos).is_none()
                {
                    return true;
                }
                let Some(snapshot) = self.client.chunk_snapshot(pos) else {
                    return false;
                };
                render_section_keys_for_snapshot(snapshot)
                    .into_iter()
                    .any(|key| {
                        !self.render_session.dirty().inflight_sections.contains(&key)
                            && render_section_neighbor_readiness(&self.client, key, camera_position)
                                .is_ready()
                    })
            })
            || self
                .render_session
                .dirty()
                .dirty_sections
                .iter()
                .copied()
                .any(|key| {
                    if self.render_session.dirty().inflight_sections.contains(&key) {
                        return false;
                    }
                    if self.render_session.contains_section(key)
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
                        && render_section_neighbor_readiness(&self.client, key, camera_position)
                            .is_ready()
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

    pub(crate) fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.last_poll_diagnostics
    }

    pub(crate) fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(state_id) = self.block_state_at_position(position) else {
            return false;
        };
        self.mesh_assets.catalog.occludes(state_id)
    }

    pub(crate) fn highest_non_air_block_y_at_world(
        &self,
        world_x: i32,
        world_z: i32,
    ) -> Option<i32> {
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        let snapshot = self.client.chunk_snapshot(pos)?;
        (snapshot.min_y..snapshot.min_y + snapshot.height)
            .rev()
            .find(|world_y| {
                snapshot_block_state_at_world(snapshot, world_x, *world_y, world_z)
                    .is_some_and(|state_id| state_id != AIR_BLOCK_STATE_ID)
            })
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
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        self.client
            .chunk_snapshot(pos)
            .and_then(|snapshot| snapshot_block_state_at_world(snapshot, world_x, world_y, world_z))
    }

    pub(crate) fn stats(&self) -> WindowRuntimeStats {
        let scheduler_metrics = self
            .server
            .as_ref()
            .map(|server| server.scheduler().metrics());
        let chunk_tracking = self
            .server
            .as_ref()
            .map(|server| server.chunk_tracking_diagnostics());
        WindowRuntimeStats {
            interest_center: self.interest_center,
            render_distance: self.render_distance,
            chunk_tracking_radius: self.chunk_tracking_radius,
            loaded_chunks: self.client.loaded_chunk_count(),
            pending_jobs: self.pending_job_count(),
            pending_publications: self.pending_publication_count(),
            pending_render_chunks: self.pending_render_chunk_count(),
            pending_render_compile_jobs: self.render_compile_worker.pending_job_count(),
            inflight_render_sections: self.render_session.dirty().inflight_sections.len(),
            client_visible_chunks: scheduler_metrics
                .map_or(self.client.loaded_chunk_count(), |metrics| {
                    metrics.client_visible_chunks
                }),
            active_ticket_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.active_ticket_chunks),
            tracked_players: chunk_tracking
                .as_ref()
                .map_or(self.client.remote_player_count(), |diagnostics| {
                    diagnostics.player_count
                }),
            player_visible_chunks: chunk_tracking
                .as_ref()
                .map_or(self.client.loaded_chunk_count(), |diagnostics| {
                    diagnostics.total_player_visible_chunks
                }),
            aggregate_player_ticket_chunks: chunk_tracking
                .as_ref()
                .map_or(0, |diagnostics| diagnostics.aggregate_player_ticket_chunks),
            player_outbound_queue_depth: chunk_tracking
                .as_ref()
                .map_or(0, |diagnostics| diagnostics.total_outbound_queue_depth),
            max_player_visible_chunks: chunk_tracking
                .as_ref()
                .map_or(self.client.loaded_chunk_count(), |diagnostics| {
                    diagnostics.max_player_visible_chunks
                }),
            max_player_outbound_queue_depth: chunk_tracking
                .as_ref()
                .map_or(0, |diagnostics| diagnostics.max_outbound_queue_depth),
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

pub(crate) fn poll_window_runtime_until_idle(
    runtime: &mut WindowSceneRuntime,
) -> Result<(usize, f64)> {
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
        let min_x = self.chunk_x - self.render_distance;
        let max_x = self.chunk_x + self.render_distance;
        let min_z = self.chunk_z - self.render_distance;
        let max_z = self.chunk_z + self.render_distance;
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
    if snapshot.pos != ChunkPos::from_block_coords(world_x, world_z) {
        return None;
    }

    let local_y = world_y - snapshot.min_y;
    if !(0..snapshot.height).contains(&local_y) {
        return None;
    }

    let section_y = block_to_section_coord(world_y);
    let local_x = local_block_coord(world_x);
    let local_z = local_block_coord(world_z);
    let local_section_y = local_section_block_coord(world_y);
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

fn render_dirty_section_keys_for_block_update(
    pos: ChunkPos,
    section_y: i32,
    update: &SectionBlockUpdate,
) -> BTreeSet<RenderSectionKey> {
    let world_x = chunk_block_coord(pos.x, update.local_x as i32);
    let world_y = section_y * SECTION_HEIGHT + update.local_y as i32;
    let world_z = chunk_block_coord(pos.z, update.local_z as i32);
    let mut keys = BTreeSet::new();
    for z in world_z - 1..=world_z + 1 {
        for x in world_x - 1..=world_x + 1 {
            for y in world_y - 1..=world_y + 1 {
                keys.insert(RenderSectionKey::new(
                    block_to_chunk_coord(x),
                    block_to_section_coord(y),
                    block_to_chunk_coord(z),
                ));
            }
        }
    }
    keys
}

fn render_section_neighbor_readiness(
    client: &ClientRuntime,
    key: RenderSectionKey,
    camera_position: Vec3,
) -> RenderSectionNeighborReadiness {
    // Native currently keeps an exact client snapshot radius, unlike Java's
    // overfetched client chunk cache. Keep this gate horizontal so flying above
    // a loaded column does not make the boundary ring disappear.
    if render_section_horizontal_distance_sq(key, camera_position)
        <= RENDER_NEIGHBOR_READY_DISTANCE_SQ
    {
        return RenderSectionNeighborReadiness::ReadyNearCamera;
    }
    if has_horizontal_neighbor_snapshots(client, ChunkPos::new(key.chunk_x, key.chunk_z)) {
        RenderSectionNeighborReadiness::ReadyWithNeighbors
    } else {
        RenderSectionNeighborReadiness::DeferredMissingNeighbors
    }
}

fn render_section_distance_sq(key: RenderSectionKey, camera_position: Vec3) -> f32 {
    let center = render_section_center(key);
    center.distance_squared(camera_position)
}

fn render_section_horizontal_distance_sq(key: RenderSectionKey, camera_position: Vec3) -> f32 {
    let center = render_section_center(key);
    let dx = center.x - camera_position.x;
    let dz = center.z - camera_position.z;
    dx * dx + dz * dz
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
        chunk_middle_block_coord(pos.x) as f32,
        camera_position.y,
        chunk_middle_block_coord(pos.z) as f32,
    );
    center.distance_squared(camera_position)
}

fn render_section_center(key: RenderSectionKey) -> Vec3 {
    Vec3::new(
        chunk_middle_block_coord(key.chunk_x) as f32,
        (key.section_y * SECTION_HEIGHT) as f32 + SECTION_HEIGHT as f32 * 0.5,
        chunk_middle_block_coord(key.chunk_z) as f32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_SEED;
    use crate::camera::SpectatorCamera;
    use crate::render_cache::extracted_asset_root;
    use mclone_core::{BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus};
    use mclone_render::chunk::{
        ChunkCamera, TexturedSectionRenderOptions,
        textured_section_visibility_stats_with_options_and_ready_sections,
    };

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
    fn render_section_readiness_uses_near_exception_and_horizontal_neighbors() {
        let target = ChunkPos::new(2, -3);
        let key = RenderSectionKey::new(target.x, 0, target.z);
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(target)));

        assert_eq!(
            render_section_neighbor_readiness(&client, key, render_section_center(key)),
            RenderSectionNeighborReadiness::ReadyNearCamera
        );

        let high_camera = render_section_center(key) + Vec3::new(0.0, 256.0, 0.0);
        assert_eq!(
            render_section_neighbor_readiness(&client, key, high_camera),
            RenderSectionNeighborReadiness::ReadyNearCamera
        );

        let diagonal_neighbor_key = RenderSectionKey::new(target.x + 1, 0, target.z + 1);
        assert_eq!(
            render_section_neighbor_readiness(&client, diagonal_neighbor_key, high_camera),
            RenderSectionNeighborReadiness::ReadyNearCamera
        );

        let far_camera = render_section_center(key) + Vec3::new(128.0, 0.0, 0.0);
        assert_eq!(
            render_section_neighbor_readiness(&client, key, far_camera),
            RenderSectionNeighborReadiness::DeferredMissingNeighbors
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
            RenderSectionNeighborReadiness::ReadyWithNeighbors
        );
    }

    #[test]
    fn render_distance_derives_java_shaped_tracking_radius() {
        assert_eq!(chunk_tracking_radius_for_render_distance(0), 0);
        assert_eq!(chunk_tracking_radius_for_render_distance(1), 1);
        assert_eq!(chunk_tracking_radius_for_render_distance(2), 3);
        assert_eq!(chunk_tracking_radius_for_render_distance(3), 3);
        assert_eq!(chunk_tracking_radius_for_render_distance(4), 4);
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

    #[test]
    fn window_runtime_streams_chunks_when_spectator_crosses_boundary() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
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
        assert!(
            runtime
                .highest_non_air_block_y_at_world(8, 8)
                .is_some_and(|y| (-64..320).contains(&y))
        );

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
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let far_camera = spectator.position + Vec3::new(128.0, 0.0, 0.0);
        let update = runtime.sync_all_render_sections(far_camera).unwrap();

        assert_eq!(update.rebuilt_section_count(), 0);
        assert_eq!(update.near_exception_section_count, 0);
        assert!(update.deferred_section_count > 0);
        assert!(runtime.pending_render_chunk_count() > 0);
        assert!(!runtime.has_pending_render_work(far_camera));
        assert!(runtime.cached_sections().is_empty());

        let deferred_key = *runtime
            .render_session
            .dirty()
            .dirty_sections
            .iter()
            .next()
            .expect("deferred dirty section should be retained");
        let ready_position = render_section_center(deferred_key);
        assert!(runtime.has_pending_render_work(ready_position));
        let ready_update = runtime.sync_all_render_sections(ready_position).unwrap();
        assert!(ready_update.rebuilt_section_count() > 0);
        assert!(
            !runtime
                .render_session
                .dirty()
                .dirty_sections
                .contains(&deferred_key)
        );
    }

    #[test]
    fn window_runtime_marks_section_block_updates_without_chunk_dirtying() {
        if !extracted_asset_root().exists() {
            return;
        }

        let mut runtime = WindowSceneRuntime::new(&SceneOptions::default()).unwrap();
        runtime.render_session.dirty_mut().dirty_chunks.clear();
        runtime.render_session.dirty_mut().dirty_sections.clear();

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

        assert!(runtime.render_session.dirty().dirty_chunks.is_empty());
        assert_eq!(
            runtime.render_session.dirty().dirty_sections,
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
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let initial_submit = runtime.sync_render_sections(spectator.position).unwrap();
        assert!(initial_submit.submitted_compile_section_count > 1);
        let changed_key = RenderSectionKey::new(0, 5, 0);
        assert!(
            runtime
                .render_session
                .dirty()
                .inflight_sections
                .contains(&changed_key)
        );

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
    fn window_runtime_updates_render_distance_live() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.render_distance, 0);
        assert_eq!(runtime.chunk_tracking_radius, 0);
        assert_eq!(runtime.stats().loaded_chunks, square_count(0).unwrap());

        assert!(runtime.set_render_distance(1).unwrap());
        assert_eq!(runtime.render_distance, 1);
        assert_eq!(runtime.chunk_tracking_radius, 1);
        assert_eq!(runtime.client.chunk_view().unwrap().render_distance, 1);
        assert_eq!(
            runtime.client.chunk_view().unwrap().chunk_tracking_radius,
            1
        );
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

        assert!(runtime.set_render_distance(0).unwrap());
        poll_window_runtime_until_idle(&mut runtime).unwrap();
        assert_eq!(runtime.render_distance, 0);
        assert_eq!(runtime.chunk_tracking_radius, 0);
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
            render_distance: 1,
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
    fn render_distance_two_tracks_extra_ring_without_drawing_it() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 2,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        assert_eq!(runtime.render_distance, 2);
        assert_eq!(runtime.chunk_tracking_radius, 3);
        assert_eq!(runtime.stats().loaded_chunks, square_count(3).unwrap());
        assert!(runtime.client.chunk_snapshots().any(|snapshot| {
            chunk_distance_from_scene_center(&scene, snapshot.pos.x, snapshot.pos.z) == 3
        }));

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        let ready_sections = runtime.traversal_ready_render_section_keys(spectator.position);
        assert!(!ready_sections.is_empty());
        assert!(ready_sections.iter().all(|key| {
            chunk_distance_from_scene_center(&scene, key.chunk_x, key.chunk_z) <= 2
        }));
    }

    #[test]
    fn high_altitude_radius_one_keeps_neighbor_chunks_draw_ready() {
        if !extracted_asset_root().exists() {
            return;
        }

        let scene = SceneOptions {
            render_distance: 1,
            ..SceneOptions::default()
        };
        let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
        poll_window_runtime_until_idle(&mut runtime).unwrap();

        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        runtime
            .sync_all_render_sections(spectator.position)
            .unwrap();
        let sections = runtime.cached_sections();
        assert!(sections.iter().any(|section| {
            section.key.chunk_x != scene.chunk_x || section.key.chunk_z != scene.chunk_z
        }));

        let high_position = spectator.position + Vec3::new(0.0, 256.0, 0.0);
        let ready_sections = runtime.traversal_ready_render_section_keys(high_position);
        assert!(
            ready_sections
                .iter()
                .any(|key| { key.chunk_x != scene.chunk_x || key.chunk_z != scene.chunk_z })
        );

        let center_drawable_sections = sections
            .iter()
            .filter(|section| {
                section.key.chunk_x == scene.chunk_x
                    && section.key.chunk_z == scene.chunk_z
                    && !section.is_empty()
            })
            .count();
        let camera = ChunkCamera {
            eye: high_position.to_array(),
            target: (high_position + Vec3::NEG_Y).to_array(),
            up: [0.0, 0.0, 1.0],
            fov_y_radians: 100.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0,
        };
        let stats = textured_section_visibility_stats_with_options_and_ready_sections(
            &sections,
            camera.render_view(960, 640),
            TexturedSectionRenderOptions {
                section_occlusion_culling: false,
                ..TexturedSectionRenderOptions::default()
            },
            Some(&ready_sections),
        );

        assert_eq!(stats.readiness_culled_section_count, 0);
        assert!(stats.drawn_section_count > center_drawable_sections);
    }

    #[test]
    fn scene_chunk_positions_cover_square_radius() {
        let positions = SceneOptions {
            seed: 0,
            chunk_x: -2,
            chunk_z: 3,
            render_distance: 1,
            remote_addr: None,
            day_time_override: None,
            freeze_time: false,
            lighting_enabled: true,
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
            render_distance: 0,
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
            mclone_net::complete_server_handshake(&mut stream).unwrap();
            let command = mclone_net::read_client_command_frame(&mut stream).unwrap();
            let mut server = IntegratedServer::new(DEFAULT_SEED);
            let mut updates = server.try_handle_command(command).unwrap();
            updates.extend(poll_integrated_server_until_idle(&mut server).unwrap());
            mclone_net::write_server_update_batch(&mut stream, &updates).unwrap();
        });
        let scene = SceneOptions {
            render_distance: 0,
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
    fn window_runtime_reuses_remote_session_for_interest_updates() {
        if !extracted_asset_root().exists() {
            return;
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut stream,
                &[ServerUpdate::TimeUpdate { day_time: 1 }],
            )
            .unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut stream,
                &[ServerUpdate::TimeUpdate { day_time: 2 }],
            )
            .unwrap();
            assert!(
                mclone_net::try_read_client_command_frame(&mut stream)
                    .unwrap()
                    .is_none()
            );
        });

        {
            let scene = SceneOptions {
                render_distance: 0,
                remote_addr: Some(addr.to_string()),
                ..SceneOptions::default()
            };
            let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
            assert!(runtime.set_interest_center(ChunkPos::new(1, 0)).unwrap());
        }
        server.join().unwrap();
    }

    #[test]
    fn window_runtime_reconnects_remote_session_and_resyncs_chunk_cache() {
        if !extracted_asset_root().exists() {
            return;
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let initial_center = ChunkPos::new(0, 0);
        let moved_center = ChunkPos::new(1, 0);
        let server = std::thread::spawn(move || {
            let (mut first_stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut first_stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: initial_center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut first_stream,
                &[ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                    initial_center,
                ))],
            )
            .unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: moved_center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            drop(first_stream);

            let (mut second_stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut second_stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut second_stream).unwrap(),
                ClientCommand::SetChunkView(ChunkView {
                    center: moved_center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                })
            );
            mclone_net::write_server_update_batch(
                &mut second_stream,
                &[ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                    moved_center,
                ))],
            )
            .unwrap();
        });

        {
            let scene = SceneOptions {
                render_distance: 0,
                remote_addr: Some(addr.to_string()),
                ..SceneOptions::default()
            };
            let mut runtime = WindowSceneRuntime::new(&scene).unwrap();
            assert!(runtime.client.chunk_snapshot(initial_center).is_some());
            runtime.render_session.dirty_mut().dirty_chunks.clear();
            runtime.render_session.dirty_mut().dirty_sections.clear();

            assert!(runtime.set_interest_center(moved_center).unwrap());

            assert_eq!(runtime.client.loaded_chunk_count(), 1);
            assert!(runtime.client.chunk_snapshot(initial_center).is_none());
            assert!(runtime.client.chunk_snapshot(moved_center).is_some());
            assert!(
                runtime
                    .render_session
                    .dirty()
                    .dirty_chunks
                    .contains(&initial_center)
            );
            assert!(
                runtime
                    .render_session
                    .dirty()
                    .dirty_chunks
                    .contains(&moved_center)
            );
        }
        server.join().unwrap();
    }

    #[test]
    fn build_scene_textured_sections_uses_client_runtime_snapshots() {
        if !extracted_asset_root().exists() {
            return;
        }
        let scene_mesh = build_scene_textured_sections(&SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        })
        .unwrap();

        assert!(scene_mesh.section_count() > 0);
        assert!(scene_mesh.index_count() > 0);
        assert!(scene_mesh.atlas.width > 0);
        assert!(scene_mesh.atlas.height > 0);
    }

    fn chunk_distance_from_scene_center(scene: &SceneOptions, chunk_x: i32, chunk_z: i32) -> i32 {
        (chunk_x - scene.chunk_x)
            .abs()
            .max((chunk_z - scene.chunk_z).abs())
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
