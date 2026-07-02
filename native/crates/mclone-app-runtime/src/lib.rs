#![forbid(unsafe_code)]

pub mod far_lod;
pub mod frame_render;
pub mod host_mode;
#[cfg(not(target_arch = "wasm32"))]
pub mod local_single_view;
#[cfg(not(target_arch = "wasm32"))]
pub mod render_assets;
pub mod session;
pub mod startup_args;

use std::collections::BTreeSet;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

use anyhow::Result;
use glam::Vec3;
use mclone_client::{
    ClientHost, ClientRuntime,
    block_facts::{BlockFluidKind, block_fluid_height, block_fluid_kind},
};
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockStateId, ChunkPos, ChunkSnapshot, ChunkStatus, block_to_section_coord,
    local_block_coord, local_section_block_coord,
};
use mclone_mesh::{RenderSectionKey, TexturedMeshCatalog, TexturedRenderSectionMesh};
use mclone_protocol::{
    ChunkView, ClientCommand, HOTBAR_SLOT_COUNT_USIZE, PlayerAppearance, PlayerModelKind,
    PlayerPositionUpdate, ServerUpdate, SetPlayerAppearanceCommand,
};
use mclone_render_session::{
    EngineRenderSession, RenderSectionCacheUpdate, RenderSectionCompiler, RenderSectionRemovalMode,
    RenderSectionSession, build_client_textured_sections, render_section_chunk_pos,
    render_section_neighbor_readiness, sort_chunk_positions_by_distance,
    sort_dirty_section_chunks_by_distance,
};
use mclone_server::{
    ChunkLoadingProgressSnapshot, ChunkLoadingProgressStats, ServerRunnerDiagnostics,
    ServerRunnerKind,
};
use mclone_ui::{
    BlockPaletteEntry, BlockPaletteOverlay, EMPTY_BLOCK_PALETTE_ENTRIES, GamePlayerModel,
    GuiTextureUv, LoadingProgressCell, LoadingProgressCellStatus, LoadingProgressOverlay,
};

pub const DEFAULT_RENDER_CHUNK_MESH_BUDGET: usize = 1;
pub const DEFAULT_RENDER_SECTION_COMPILE_WORKERS: usize = 1;
pub const JAVA_MIN_TRACKING_RENDER_DISTANCE: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderSectionSyncTiming {
    pub completed_result_accept_ms: f64,
    pub dirty_seed_ms: f64,
    pub prepare_ms: f64,
    pub submit_ms: f64,
    pub submit_snapshot_ms: f64,
    pub submit_handoff_ms: f64,
}

impl RenderSectionSyncTiming {
    pub fn merge(&mut self, other: Self) {
        self.completed_result_accept_ms += other.completed_result_accept_ms;
        self.dirty_seed_ms += other.dirty_seed_ms;
        self.prepare_ms += other.prepare_ms;
        self.submit_ms += other.submit_ms;
        self.submit_snapshot_ms += other.submit_snapshot_ms;
        self.submit_handoff_ms += other.submit_handoff_ms;
    }
}

#[derive(Clone, Debug, Default)]
pub struct TimedRenderSectionCacheUpdate {
    pub cache_update: RenderSectionCacheUpdate,
    pub timing: RenderSectionSyncTiming,
}

impl TimedRenderSectionCacheUpdate {
    pub fn merge(&mut self, other: Self) {
        self.cache_update.merge(other.cache_update);
        self.timing.merge(other.timing);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn compile_snapshots_for_target_sections(
    client: &ClientRuntime,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Vec<ChunkSnapshot> {
    let mut positions = BTreeSet::new();
    for key in target_sections {
        positions.extend(mclone_render_session::render_dirty_chunk_neighborhood(
            render_section_chunk_pos(*key),
        ));
    }
    positions
        .into_iter()
        .filter_map(|pos| client.chunk_snapshot(pos).cloned())
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn average_duration(total: Duration, count: usize) -> Duration {
    if count == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(total.as_secs_f64() / count as f64)
    }
}

pub fn square_count(radius: i32) -> Result<usize> {
    anyhow::ensure!(radius >= 0, "radius must be non-negative");
    let side = usize::try_from(radius)?.saturating_mul(2).saturating_add(1);
    Ok(side * side)
}

pub fn chunk_tracking_radius_for_render_distance(render_distance: u32) -> u32 {
    // Java 1.17.1 exposes render distance with a minimum of 2, then the
    // integrated server path arrives at ChunkMap's clamped minimum of 3.
    if render_distance < JAVA_MIN_TRACKING_RENDER_DISTANCE {
        return render_distance;
    }
    render_distance.max(3)
}

pub fn chunk_view(center: ChunkPos, render_distance: u32, chunk_tracking_radius: u32) -> ChunkView {
    ChunkView {
        center,
        render_distance,
        chunk_tracking_radius,
    }
}

pub const fn player_model_kind_for_ui_model(model: GamePlayerModel) -> PlayerModelKind {
    match model {
        GamePlayerModel::Player => PlayerModelKind::Player,
        GamePlayerModel::UprightBear => PlayerModelKind::UprightBear,
    }
}

pub const fn player_appearance_for_ui_model(model: GamePlayerModel) -> PlayerAppearance {
    PlayerAppearance {
        model: player_model_kind_for_ui_model(model),
    }
}

pub const fn set_player_appearance_command_for_ui_model(model: GamePlayerModel) -> ClientCommand {
    ClientCommand::SetPlayerAppearance(SetPlayerAppearanceCommand {
        appearance: player_appearance_for_ui_model(model),
    })
}

const DEBUG_BLOCK_PALETTE: &[(BlockStateId, &str)] = &[
    (BlockStateId(1), "Stone"),
    (BlockStateId(10), "Granite"),
    (BlockStateId(11), "Diorite"),
    (BlockStateId(12), "Andesite"),
    (BlockStateId(52), "Tuff"),
    (BlockStateId(53), "Deepslate"),
    (BlockStateId(5), "Dirt"),
    (BlockStateId(13), "Coarse Dirt"),
    (BlockStateId(14), "Podzol"),
    (BlockStateId(15), "Mycelium"),
    (BlockStateId(4), "Grass Block"),
    (BlockStateId(6), "Sand"),
    (BlockStateId(38), "Red Sand"),
    (BlockStateId(7), "Gravel"),
    (BlockStateId(33), "Sandstone"),
    (BlockStateId(34), "Red Sandstone"),
    (BlockStateId(88), "Clay"),
    (BlockStateId(40), "Snow Block"),
    (BlockStateId(35), "Packed Ice"),
    (BlockStateId(39), "Ice"),
    (BlockStateId(36), "Obsidian"),
    (BlockStateId(37), "Magma Block"),
    (BlockStateId(16), "Terracotta"),
    (BlockStateId(17), "White Terracotta"),
    (BlockStateId(18), "Orange Terracotta"),
    (BlockStateId(19), "Magenta Terracotta"),
    (BlockStateId(20), "Light Blue Terracotta"),
    (BlockStateId(21), "Yellow Terracotta"),
    (BlockStateId(22), "Lime Terracotta"),
    (BlockStateId(23), "Pink Terracotta"),
    (BlockStateId(24), "Gray Terracotta"),
    (BlockStateId(25), "Light Gray Terracotta"),
    (BlockStateId(26), "Cyan Terracotta"),
    (BlockStateId(27), "Purple Terracotta"),
    (BlockStateId(28), "Blue Terracotta"),
    (BlockStateId(29), "Brown Terracotta"),
    (BlockStateId(30), "Green Terracotta"),
    (BlockStateId(31), "Red Terracotta"),
    (BlockStateId(32), "Black Terracotta"),
    (BlockStateId(91), "Bricks"),
    (BlockStateId(41), "Oak Log"),
    (BlockStateId(46), "Birch Log"),
    (BlockStateId(48), "Spruce Log"),
    (BlockStateId(42), "Oak Leaves"),
    (BlockStateId(47), "Birch Leaves"),
    (BlockStateId(49), "Spruce Leaves"),
    (BlockStateId(54), "Coal Ore"),
    (BlockStateId(56), "Copper Ore"),
    (BlockStateId(58), "Iron Ore"),
    (BlockStateId(60), "Gold Ore"),
    (BlockStateId(64), "Diamond Ore"),
    (BlockStateId(66), "Lapis Ore"),
    (BlockStateId(62), "Redstone Ore"),
    (BlockStateId(89), "Dripstone Block"),
    (BlockStateId(90), "Pointed Dripstone"),
    (BlockStateId(100), "Torch"),
    (BlockStateId(43), "Grass"),
    (BlockStateId(44), "Dandelion"),
    (BlockStateId(45), "Poppy"),
    (BlockStateId(50), "Fern"),
    (BlockStateId(51), "Dead Bush"),
];

pub fn debug_hotbar_icons(
    items: [Option<BlockStateId>; HOTBAR_SLOT_COUNT_USIZE],
    catalog: &TexturedMeshCatalog,
) -> [Option<GuiTextureUv>; HOTBAR_SLOT_COUNT_USIZE] {
    items.map(|state_id| {
        let uv = catalog.gui_icon_uv(state_id?)?;
        Some(GuiTextureUv::new(uv.u0, uv.v0, uv.u1, uv.v1))
    })
}

pub fn debug_block_palette_overlay(
    catalog: &TexturedMeshCatalog,
    selected_hotbar_slot: u8,
) -> BlockPaletteOverlay {
    let mut entries = EMPTY_BLOCK_PALETTE_ENTRIES;
    for (index, &(block_state, label)) in DEBUG_BLOCK_PALETTE.iter().take(entries.len()).enumerate()
    {
        let icon = catalog
            .gui_icon_uv(block_state)
            .map(|uv| GuiTextureUv::new(uv.u0, uv.v0, uv.u1, uv.v1));
        entries[index] = Some(BlockPaletteEntry::new(block_state.0, icon, label));
    }
    BlockPaletteOverlay::visible(selected_hotbar_slot, entries)
}

pub fn loading_progress_overlay_from_diagnostics(
    diagnostics: &ServerRunnerDiagnostics,
) -> Option<LoadingProgressOverlay> {
    diagnostics
        .loading_progress_snapshot
        .as_ref()
        .map(loading_progress_overlay_from_snapshot)
}

pub fn view_readiness_overlay_from_diagnostics(
    diagnostics: &ServerRunnerDiagnostics,
) -> Option<LoadingProgressOverlay> {
    diagnostics
        .view_readiness_snapshot
        .as_ref()
        .map(loading_progress_overlay_from_snapshot)
}

pub fn loading_progress_overlay_from_snapshot(
    snapshot: &ChunkLoadingProgressSnapshot,
) -> LoadingProgressOverlay {
    LoadingProgressOverlay::new(
        snapshot.stats.target_radius,
        snapshot.stats.target_ready_chunks,
        snapshot.stats.target_chunk_count,
        snapshot.stats.playable_chunk_ready,
        snapshot.cells.iter().map(|cell| {
            LoadingProgressCell::new(
                cell.relative_x,
                cell.relative_z,
                loading_progress_cell_status(cell.status, cell.target_ready),
            )
            .playable(cell.playable)
        }),
    )
}

fn loading_progress_cell_status(
    status: Option<ChunkStatus>,
    target_ready: bool,
) -> LoadingProgressCellStatus {
    if target_ready {
        return LoadingProgressCellStatus::TargetReady;
    }
    match status {
        None => LoadingProgressCellStatus::None,
        Some(ChunkStatus::Terrain) => LoadingProgressCellStatus::Terrain,
        Some(ChunkStatus::Surface) => LoadingProgressCellStatus::Surface,
        Some(ChunkStatus::Features) => LoadingProgressCellStatus::Features,
        Some(ChunkStatus::Light) => LoadingProgressCellStatus::Light,
        Some(ChunkStatus::Full) => LoadingProgressCellStatus::TargetReady,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExchange {
    pub updates: Vec<ServerUpdate>,
    pub command_count: usize,
    pub protocol_codec_roundtrip: bool,
    pub transport_drained: bool,
}

impl RuntimeExchange {
    pub fn new(
        updates: Vec<ServerUpdate>,
        command_count: usize,
        protocol_codec_roundtrip: bool,
        transport_drained: bool,
    ) -> Self {
        Self {
            updates,
            command_count,
            protocol_codec_roundtrip,
            transport_drained,
        }
    }

    pub fn command(updates: Vec<ServerUpdate>) -> Self {
        Self::new(updates, 1, true, true)
    }

    pub fn updates(updates: Vec<ServerUpdate>, transport_drained: bool) -> Self {
        Self::new(updates, 0, true, transport_drained)
    }

    pub const fn empty(transport_drained: bool) -> Self {
        Self {
            updates: Vec::new(),
            command_count: 0,
            protocol_codec_roundtrip: true,
            transport_drained,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStepReport {
    pub command_count: usize,
    pub update_count: usize,
    pub loaded_chunk_count: usize,
    pub protocol_codec_roundtrip: bool,
    pub transport_drained: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeUpdateApplyReport {
    pub changed: bool,
    pub total_ms: f64,
    pub dirty_mark_ms: f64,
    pub client_apply_updates_ms: f64,
    pub updates: usize,
    pub snapshot_updates: usize,
    pub section_block_updates: usize,
    pub unload_updates: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePollTiming {
    pub total_ms: f64,
    pub drain_updates_ms: f64,
    pub poll_diagnostics_ms: f64,
    pub diagnostics_refreshed: bool,
    pub diagnostics_cache_age_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeExchangeApplyReport {
    pub step: RuntimeStepReport,
    pub update_apply: RuntimeUpdateApplyReport,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePollDiagnostics {
    pub server_runner_kind: Option<ServerRunnerKind>,
    pub server_command_queue_depth: usize,
    pub server_update_queue_depth: usize,
    pub server_pending_jobs: usize,
    pub server_pending_publications: usize,
    pub flush_commands_ms: f64,
    pub poll_total_ms: f64,
    pub drain_updates_ms: f64,
    pub poll_diagnostics_ms: f64,
    pub diagnostics_refreshed: bool,
    pub diagnostics_cache_age_ms: f64,
    pub server_diagnostics_detail_refreshes: u64,
    pub server_diagnostics_detail_age_ms: f64,
    pub server_tick_ms: f64,
    pub server_reported_total_ms: f64,
    pub scheduler_tick_ms: f64,
    pub scheduler_report_ms: f64,
    pub scheduler_purge_stale_tickets_ms: f64,
    pub scheduler_reconcile_holders_ms: f64,
    pub scheduler_publish_completed_ms: f64,
    pub scheduler_pending_unload_ms: f64,
    pub scheduler_apply_events_ms: f64,
    pub block_tick_ms: f64,
    pub fluid_tick_ms: f64,
    pub fluid_event_apply_ms: f64,
    pub fluid_due_scan_ms: f64,
    pub fluid_remove_due_ms: f64,
    pub fluid_tick_fluid_ms: f64,
    pub fluid_set_block_ms: f64,
    pub entity_tick_ms: f64,
    pub apply_updates_ms: f64,
    pub dirty_mark_ms: f64,
    pub client_apply_updates_ms: f64,
    pub scheduler_pending_jobs: usize,
    pub scheduler_completed_jobs: usize,
    pub scheduler_dirty_chunks: usize,
    pub scheduler_loaded_snapshot_chunks: usize,
    pub scheduler_client_visible_chunks: usize,
    pub scheduler_active_ticket_chunks: usize,
    pub loading_progress: Option<ChunkLoadingProgressStats>,
    pub player_visible_chunks: usize,
    pub player_outbound_queue_depth: usize,
    pub scheduler_events: usize,
    pub updates: usize,
    pub snapshot_updates: usize,
    pub section_block_updates: usize,
    pub unload_updates: usize,
    pub pending_unloads_processed: usize,
    pub fluid_due_ticks: usize,
    pub fluid_executed_ticks: usize,
    pub fluid_deferred_ticks: usize,
    pub fluid_mutated_blocks: usize,
    pub fluid_snapshot_events: usize,
    pub fluid_event_count: usize,
    pub scheduled_fluid_ticks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleViewRuntimeStats {
    pub server_runner_kind: Option<ServerRunnerKind>,
    pub server_command_queue_depth: usize,
    pub server_update_queue_depth: usize,
    pub interest_center: ChunkPos,
    pub render_distance: u32,
    pub chunk_tracking_radius: u32,
    pub loaded_chunks: usize,
    pub pending_jobs: usize,
    pub pending_publications: usize,
    pub pending_render_chunks: usize,
    pub pending_render_compile_jobs: usize,
    pub inflight_render_sections: usize,
    pub client_visible_chunks: usize,
    pub active_ticket_chunks: usize,
    pub loading_progress: Option<ChunkLoadingProgressStats>,
    pub tracked_players: usize,
    pub player_visible_chunks: usize,
    pub aggregate_player_ticket_chunks: usize,
    pub player_outbound_queue_depth: usize,
    pub max_player_visible_chunks: usize,
    pub max_player_outbound_queue_depth: usize,
    pub pending_unload_chunks: usize,
    pub block_ticking_chunks: usize,
    pub entity_ticking_chunks: usize,
    pub last_tick: u64,
    pub last_simulation_tick: u64,
    pub last_tick_unloads_processed: usize,
    pub last_simulation_block_tick_chunks: usize,
    pub last_simulation_entity_tick_chunks: usize,
    pub last_simulation_scheduler_tick_ms: f64,
    pub last_simulation_block_tick_ms: f64,
    pub last_simulation_fluid_tick_ms: f64,
    pub last_simulation_entity_tick_ms: f64,
    pub last_simulation_fluid_ticks_executed: usize,
    pub last_simulation_deferred_fluid_ticks: usize,
    pub last_simulation_fluid_mutated_blocks: usize,
    pub scheduled_fluid_ticks: usize,
}

#[derive(Debug)]
pub struct SingleViewRuntime {
    engine: EngineRenderSession,
    render_distance: u32,
    chunk_tracking_radius: u32,
    interest_center: ChunkPos,
    command_count: usize,
    update_count: usize,
    snapshot_update_count: usize,
    section_block_update_count: usize,
    unload_update_count: usize,
    protocol_codec_roundtrip: bool,
    transport_drained: bool,
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

impl SingleViewRuntime {
    pub fn new(
        client: ClientRuntime,
        interest_center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Self {
        Self {
            engine: EngineRenderSession::new(client),
            render_distance,
            chunk_tracking_radius,
            interest_center,
            command_count: 0,
            update_count: 0,
            snapshot_update_count: 0,
            section_block_update_count: 0,
            unload_update_count: 0,
            protocol_codec_roundtrip: true,
            transport_drained: true,
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
        }
    }

    pub fn local_integrated(
        interest_center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Self {
        Self::new(
            ClientRuntime::local_integrated(),
            interest_center,
            render_distance,
            chunk_tracking_radius,
        )
    }

    pub fn remote_dedicated(
        interest_center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Self {
        Self::new(
            ClientRuntime::new(ClientHost::RemoteDedicated),
            interest_center,
            render_distance,
            chunk_tracking_radius,
        )
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.engine.client()
    }

    pub const fn client_mut(&mut self) -> &mut ClientRuntime {
        self.engine.client_mut()
    }

    pub const fn engine(&self) -> &EngineRenderSession {
        &self.engine
    }

    pub const fn engine_mut(&mut self) -> &mut EngineRenderSession {
        &mut self.engine
    }

    pub const fn render_session(&self) -> &RenderSectionSession {
        self.engine.render_session()
    }

    pub const fn render_session_mut(&mut self) -> &mut RenderSectionSession {
        self.engine.render_session_mut()
    }

    pub const fn render_distance(&self) -> u32 {
        self.render_distance
    }

    pub const fn chunk_tracking_radius(&self) -> u32 {
        self.chunk_tracking_radius
    }

    pub const fn interest_center(&self) -> ChunkPos {
        self.interest_center
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn update_count(&self) -> usize {
        self.update_count
    }

    pub const fn snapshot_update_count(&self) -> usize {
        self.snapshot_update_count
    }

    pub const fn section_block_update_count(&self) -> usize {
        self.section_block_update_count
    }

    pub const fn unload_update_count(&self) -> usize {
        self.unload_update_count
    }

    pub const fn protocol_codec_roundtrip(&self) -> bool {
        self.protocol_codec_roundtrip
    }

    pub const fn transport_drained(&self) -> bool {
        self.transport_drained
    }

    pub const fn last_poll_diagnostics(&self) -> RuntimePollDiagnostics {
        self.last_poll_diagnostics
    }

    pub fn set_interest_center_command(&mut self, center: ChunkPos) -> Option<ClientCommand> {
        self.set_chunk_view_command(center, self.render_distance, self.chunk_tracking_radius)
    }

    pub fn set_render_distance_command(&mut self, render_distance: u32) -> Option<ClientCommand> {
        self.set_chunk_view_command(
            self.interest_center,
            render_distance,
            chunk_tracking_radius_for_render_distance(render_distance),
        )
    }

    pub fn set_chunk_view_command(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Option<ClientCommand> {
        if self.client().chunk_view().is_some_and(|view| {
            view.center == center
                && view.render_distance == render_distance
                && view.chunk_tracking_radius == chunk_tracking_radius
        }) {
            self.interest_center = center;
            self.render_distance = render_distance;
            self.chunk_tracking_radius = chunk_tracking_radius;
            return None;
        }

        self.interest_center = center;
        self.render_distance = render_distance;
        self.chunk_tracking_radius = chunk_tracking_radius;
        Some(self.client_mut().set_chunk_view(chunk_view(
            center,
            render_distance,
            chunk_tracking_radius,
        )))
    }

    pub fn apply_exchange(&mut self, exchange: RuntimeExchange) -> RuntimeStepReport {
        self.apply_exchange_report(exchange).step
    }

    pub fn apply_exchange_report(
        &mut self,
        exchange: RuntimeExchange,
    ) -> RuntimeExchangeApplyReport {
        self.command_count += exchange.command_count;
        self.protocol_codec_roundtrip &= exchange.protocol_codec_roundtrip;
        self.transport_drained &= exchange.transport_drained;
        let update_apply = self.apply_server_updates_report(exchange.updates);
        self.update_count += update_apply.updates;
        self.snapshot_update_count += update_apply.snapshot_updates;
        self.section_block_update_count += update_apply.section_block_updates;
        self.unload_update_count += update_apply.unload_updates;
        let step = RuntimeStepReport {
            command_count: exchange.command_count,
            update_count: update_apply.updates,
            loaded_chunk_count: self.client().loaded_chunk_count(),
            protocol_codec_roundtrip: exchange.protocol_codec_roundtrip,
            transport_drained: exchange.transport_drained,
        };
        RuntimeExchangeApplyReport { step, update_apply }
    }

    pub fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> bool {
        self.apply_server_updates_report(updates).changed
    }

    pub fn apply_server_updates_report(
        &mut self,
        updates: Vec<ServerUpdate>,
    ) -> RuntimeUpdateApplyReport {
        let total_start = timing_start();
        let dirty_mark_start = timing_start();
        let update_report = self.engine.mark_server_update_render_dirty(&updates);
        let dirty_mark_ms = timing_elapsed_ms(dirty_mark_start);
        let client_apply_start = timing_start();
        self.client_mut().apply_updates(updates);
        let client_apply_updates_ms = timing_elapsed_ms(client_apply_start);
        RuntimeUpdateApplyReport {
            changed: update_report.changed,
            total_ms: timing_elapsed_ms(total_start),
            dirty_mark_ms,
            client_apply_updates_ms,
            updates: update_report.updates,
            snapshot_updates: update_report.snapshot_updates,
            section_block_updates: update_report.section_block_updates,
            unload_updates: update_report.unload_updates,
        }
    }

    pub fn clear_client_replica_and_mark_render_dirty(&mut self) -> Vec<ChunkPos> {
        self.engine.clear_client_replica_and_mark_render_dirty()
    }

    pub fn drain_player_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.client_mut().drain_player_position_updates().collect()
    }

    pub fn finish_poll_diagnostics(
        &mut self,
        timing: RuntimePollTiming,
        apply_report: RuntimeUpdateApplyReport,
        runner_diagnostics: Option<&ServerRunnerDiagnostics>,
    ) -> RuntimePollDiagnostics {
        let mut diagnostics = RuntimePollDiagnostics {
            flush_commands_ms: timing.total_ms,
            poll_total_ms: timing.total_ms,
            drain_updates_ms: timing.drain_updates_ms,
            poll_diagnostics_ms: timing.poll_diagnostics_ms,
            diagnostics_refreshed: timing.diagnostics_refreshed,
            diagnostics_cache_age_ms: timing.diagnostics_cache_age_ms,
            apply_updates_ms: apply_report.total_ms,
            dirty_mark_ms: apply_report.dirty_mark_ms,
            client_apply_updates_ms: apply_report.client_apply_updates_ms,
            updates: apply_report.updates,
            snapshot_updates: apply_report.snapshot_updates,
            section_block_updates: apply_report.section_block_updates,
            unload_updates: apply_report.unload_updates,
            ..RuntimePollDiagnostics::default()
        };
        if let Some(runner_diagnostics) = runner_diagnostics {
            self.apply_runner_diagnostics(runner_diagnostics, &mut diagnostics);
        }
        self.last_poll_diagnostics = diagnostics;
        diagnostics
    }

    pub fn sync_render_sections_with_budget<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        chunk_budget: usize,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        self.sync_render_sections_with_budget_and_completed_result_acceptance(
            compiler,
            camera_position,
            chunk_budget,
            None,
            snapshots_for_submit,
        )
    }

    pub fn sync_render_sections_with_budget_and_completed_result_acceptance<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        chunk_budget: usize,
        completed_result_accept_budget: Option<usize>,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        self.engine
            .sync_render_sections_with_budget_and_completed_result_acceptance(
                compiler,
                chunk_budget,
                |dirty_work| {
                    sort_chunk_positions_by_distance(
                        dirty_work.loaded_dirty_chunks.iter().copied(),
                        camera_position,
                    )
                },
                |dirty_work| {
                    sort_dirty_section_chunks_by_distance(
                        &dirty_work.loaded_dirty_sections_by_chunk,
                        camera_position,
                    )
                },
                |client, key| render_section_neighbor_readiness(client, key, camera_position),
                RenderSectionRemovalMode::ApplyImmediately,
                completed_result_accept_budget,
                snapshots_for_submit,
            )
    }

    pub fn sync_render_sections<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        self.sync_render_sections_with_budget(
            compiler,
            camera_position,
            DEFAULT_RENDER_CHUNK_MESH_BUDGET,
            snapshots_for_submit,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_with_budget_and_completed_result_acceptance_timed<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        chunk_budget: usize,
        completed_result_accept_budget: Option<usize>,
        snapshots_for_submit: Snapshots,
    ) -> Result<TimedRenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        let mut timing = RenderSectionSyncTiming::default();

        let accept_start = Instant::now();
        let completed_results = compiler.try_recv_completed()?;
        let pending_compile_jobs = compiler.pending_job_count();
        let mut cache_update = self
            .engine
            .drain_completed_compile_updates_with_acceptance_budget(
                completed_results,
                |_| 0,
                pending_compile_jobs,
                completed_result_accept_budget,
            )?;
        timing.completed_result_accept_ms = elapsed_ms(accept_start.elapsed());

        if cache_update.queued_completed_compile_result_count > 0 {
            cache_update.pending_compile_jobs = compiler.pending_job_count();
            return Ok(TimedRenderSectionCacheUpdate {
                cache_update,
                timing,
            });
        }

        let seed_start = Instant::now();
        self.engine.mark_loaded_chunks_dirty_when_cache_empty();
        timing.dirty_seed_ms = elapsed_ms(seed_start.elapsed());

        if self.engine.render_session().dirty_work_is_empty() {
            cache_update.pending_compile_jobs = compiler.pending_job_count();
            return Ok(TimedRenderSectionCacheUpdate {
                cache_update,
                timing,
            });
        }

        let prepare_start = Instant::now();
        let sync_update = self.engine.prepare_sync_update(
            |dirty_work| {
                sort_chunk_positions_by_distance(
                    dirty_work.loaded_dirty_chunks.iter().copied(),
                    camera_position,
                )
            },
            |dirty_work| {
                sort_dirty_section_chunks_by_distance(
                    &dirty_work.loaded_dirty_sections_by_chunk,
                    camera_position,
                )
            },
            chunk_budget,
            |client, key| render_section_neighbor_readiness(client, key, camera_position),
            RenderSectionRemovalMode::ApplyImmediately,
        );
        timing.prepare_ms = elapsed_ms(prepare_start.elapsed());
        cache_update.merge(sync_update.cache_update);

        if chunk_budget == 0 || !compiler.has_pending_job_capacity() {
            cache_update.pending_compile_jobs = compiler.pending_job_count();
            return Ok(TimedRenderSectionCacheUpdate {
                cache_update,
                timing,
            });
        }

        let submit_start = Instant::now();
        let sync_plan = sync_update.sync_plan;
        let snapshot_start = Instant::now();
        let snapshots = snapshots_for_submit(self.client(), compiler);
        timing.submit_snapshot_ms = elapsed_ms(snapshot_start.elapsed());
        let handoff_start = Instant::now();
        let submission_update = self.engine.submit_prepared_sync_plan(
            &sync_plan,
            snapshots,
            |_sync_plan, request| compiler.submit(request),
        )?;
        timing.submit_handoff_ms = elapsed_ms(handoff_start.elapsed());
        cache_update.merge(submission_update.cache_update);
        cache_update.pending_compile_jobs = compiler.pending_job_count();
        timing.submit_ms = elapsed_ms(submit_start.elapsed());

        Ok(TimedRenderSectionCacheUpdate {
            cache_update,
            timing,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots<C>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        chunk_budget: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
    {
        self.sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots_timed(
            compiler,
            camera_position,
            chunk_budget,
            completed_result_accept_budget,
        )
        .map(|timed| timed.cache_update)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots_timed<
        C,
    >(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        chunk_budget: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
    {
        let mut timing = RenderSectionSyncTiming::default();

        let accept_start = Instant::now();
        let completed_results = compiler.try_recv_completed()?;
        let pending_compile_jobs = compiler.pending_job_count();
        let mut cache_update = self
            .engine
            .drain_completed_compile_updates_with_acceptance_budget(
                completed_results,
                |_| 0,
                pending_compile_jobs,
                completed_result_accept_budget,
            )?;
        timing.completed_result_accept_ms = elapsed_ms(accept_start.elapsed());

        if cache_update.queued_completed_compile_result_count > 0 {
            cache_update.pending_compile_jobs = compiler.pending_job_count();
            return Ok(TimedRenderSectionCacheUpdate {
                cache_update,
                timing,
            });
        }

        let seed_start = Instant::now();
        self.engine.mark_loaded_chunks_dirty_when_cache_empty();
        timing.dirty_seed_ms = elapsed_ms(seed_start.elapsed());

        if self.engine.render_session().dirty_work_is_empty() {
            cache_update.pending_compile_jobs = compiler.pending_job_count();
            return Ok(TimedRenderSectionCacheUpdate {
                cache_update,
                timing,
            });
        }

        let prepare_start = Instant::now();
        let sync_update = self.engine.prepare_sync_update(
            |dirty_work| {
                sort_chunk_positions_by_distance(
                    dirty_work.loaded_dirty_chunks.iter().copied(),
                    camera_position,
                )
            },
            |dirty_work| {
                sort_dirty_section_chunks_by_distance(
                    &dirty_work.loaded_dirty_sections_by_chunk,
                    camera_position,
                )
            },
            chunk_budget,
            |client, key| render_section_neighbor_readiness(client, key, camera_position),
            RenderSectionRemovalMode::ApplyImmediately,
        );
        timing.prepare_ms = elapsed_ms(prepare_start.elapsed());
        cache_update.merge(sync_update.cache_update);

        if chunk_budget == 0 || !compiler.has_pending_job_capacity() {
            cache_update.pending_compile_jobs = compiler.pending_job_count();
            return Ok(TimedRenderSectionCacheUpdate {
                cache_update,
                timing,
            });
        }

        let submit_start = Instant::now();
        let sync_plan = sync_update.sync_plan;
        let snapshot_start = Instant::now();
        let snapshots = compile_snapshots_for_target_sections(
            self.client(),
            &sync_plan.ready_plan.ready_section_keys,
        );
        timing.submit_snapshot_ms = elapsed_ms(snapshot_start.elapsed());
        let handoff_start = Instant::now();
        let submission_update = self.engine.submit_prepared_sync_plan(
            &sync_plan,
            snapshots,
            |_sync_plan, request| compiler.submit(request),
        )?;
        timing.submit_handoff_ms = elapsed_ms(handoff_start.elapsed());
        cache_update.merge(submission_update.cache_update);
        cache_update.pending_compile_jobs = compiler.pending_job_count();
        timing.submit_ms = elapsed_ms(submit_start.elapsed());

        Ok(TimedRenderSectionCacheUpdate {
            cache_update,
            timing,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_until_deadline<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        deadline: Instant,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnMut(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        self.sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
            compiler,
            camera_position,
            deadline,
            None,
            snapshots_for_submit,
        )
        .map(|timed| timed.cache_update)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnMut(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        self.sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
            compiler,
            camera_position,
            deadline,
            completed_result_accept_budget,
            snapshots_for_submit,
        )
        .map(|timed| timed.cache_update)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed<
        C,
        Snapshots,
    >(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
        mut snapshots_for_submit: Snapshots,
    ) -> Result<TimedRenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnMut(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        let mut combined = RenderSectionCacheUpdate::default();
        let mut combined_timing = RenderSectionSyncTiming::default();
        let mut submitted_request_count = 0_usize;
        let mut admission_total = Duration::ZERO;
        let mut remaining_result_accept_budget = completed_result_accept_budget;

        loop {
            if submitted_request_count > 0 {
                let average_admission = average_duration(admission_total, submitted_request_count);
                if average_admission > Duration::ZERO
                    && deadline.saturating_duration_since(Instant::now()) < average_admission
                {
                    if compiler.has_pending_job_capacity()
                        && self.has_ready_pending_render_work(camera_position)
                    {
                        combined.deadline_skipped_compile_request_count += 1;
                    }
                    combined.pending_compile_jobs = compiler.pending_job_count();
                    return Ok(TimedRenderSectionCacheUpdate {
                        cache_update: combined,
                        timing: combined_timing,
                    });
                }
            }

            let admission_start = Instant::now();
            let timed_update = self
                .sync_render_sections_with_budget_and_completed_result_acceptance_timed(
                    compiler,
                    camera_position,
                    DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                    remaining_result_accept_budget,
                    |client, compiler| snapshots_for_submit(client, compiler),
                )?;
            let update = timed_update.cache_update;
            combined_timing.merge(timed_update.timing);
            if let Some(remaining) = remaining_result_accept_budget.as_mut() {
                *remaining = remaining.saturating_sub(update.accepted_compile_result_count);
            }
            let submitted = update.submitted_compile_section_count > 0;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || submitted
                || update.accepted_compile_result_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            if submitted {
                submitted_request_count += 1;
                admission_total += admission_start.elapsed();
            }
            combined.merge(update);

            if compiler.pending_job_count() == 0
                && !self.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(TimedRenderSectionCacheUpdate {
                    cache_update: combined,
                    timing: combined_timing,
                });
            }
            if !compiler.has_pending_job_capacity() || !progressed {
                combined.pending_compile_jobs = compiler.pending_job_count();
                return Ok(TimedRenderSectionCacheUpdate {
                    cache_update: combined,
                    timing: combined_timing,
                });
            }
            if submitted_request_count > 0 && Instant::now() >= deadline {
                if self.has_ready_pending_render_work(camera_position) {
                    combined.deadline_skipped_compile_request_count += 1;
                }
                combined.pending_compile_jobs = compiler.pending_job_count();
                return Ok(TimedRenderSectionCacheUpdate {
                    cache_update: combined,
                    timing: combined_timing,
                });
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots<
        C,
    >(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
    {
        self.sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots_timed(
            compiler,
            camera_position,
            deadline,
            completed_result_accept_budget,
        )
        .map(|timed| timed.cache_update)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_render_sections_until_deadline_with_completed_result_acceptance_targeted_snapshots_timed<
        C,
    >(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        deadline: Instant,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<TimedRenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
    {
        let mut combined = RenderSectionCacheUpdate::default();
        let mut combined_timing = RenderSectionSyncTiming::default();
        let mut submitted_request_count = 0_usize;
        let mut admission_total = Duration::ZERO;
        let mut remaining_result_accept_budget = completed_result_accept_budget;

        loop {
            if submitted_request_count > 0 {
                let average_admission = average_duration(admission_total, submitted_request_count);
                if average_admission > Duration::ZERO
                    && deadline.saturating_duration_since(Instant::now()) < average_admission
                {
                    if compiler.has_pending_job_capacity()
                        && self.has_ready_pending_render_work(camera_position)
                    {
                        combined.deadline_skipped_compile_request_count += 1;
                    }
                    combined.pending_compile_jobs = compiler.pending_job_count();
                    return Ok(TimedRenderSectionCacheUpdate {
                        cache_update: combined,
                        timing: combined_timing,
                    });
                }
            }

            let admission_start = Instant::now();
            let timed_update = self
                .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots_timed(
                    compiler,
                    camera_position,
                    DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                    remaining_result_accept_budget,
                )?;
            let update = timed_update.cache_update;
            combined_timing.merge(timed_update.timing);
            if let Some(remaining) = remaining_result_accept_budget.as_mut() {
                *remaining = remaining.saturating_sub(update.accepted_compile_result_count);
            }
            let submitted = update.submitted_compile_section_count > 0;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || submitted
                || update.accepted_compile_result_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            if submitted {
                submitted_request_count += 1;
                admission_total += admission_start.elapsed();
            }
            combined.merge(update);

            if compiler.pending_job_count() == 0
                && !self.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(TimedRenderSectionCacheUpdate {
                    cache_update: combined,
                    timing: combined_timing,
                });
            }
            if !compiler.has_pending_job_capacity() || !progressed {
                combined.pending_compile_jobs = compiler.pending_job_count();
                return Ok(TimedRenderSectionCacheUpdate {
                    cache_update: combined,
                    timing: combined_timing,
                });
            }
            if submitted_request_count > 0 && Instant::now() >= deadline {
                if self.has_ready_pending_render_work(camera_position) {
                    combined.deadline_skipped_compile_request_count += 1;
                }
                combined.pending_compile_jobs = compiler.pending_job_count();
                return Ok(TimedRenderSectionCacheUpdate {
                    cache_update: combined,
                    timing: combined_timing,
                });
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_all_render_sections<C, Snapshots>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
        mut snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        Snapshots: FnMut(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut combined = RenderSectionCacheUpdate::default();
        loop {
            let update = self.sync_render_sections_with_budget(
                compiler,
                camera_position,
                usize::MAX,
                |client, compiler| snapshots_for_submit(client, compiler),
            )?;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || update.submitted_compile_section_count > 0
                || update.accepted_compile_result_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            combined.merge(update);
            if compiler.pending_job_count() == 0
                && !self.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(combined);
            }
            if Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for render section compile queue");
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sync_all_render_sections_targeted_snapshots<C>(
        &mut self,
        compiler: &mut C,
        camera_position: Vec3,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
    {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut combined = RenderSectionCacheUpdate::default();
        loop {
            let update =
                self.sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                    compiler,
                    camera_position,
                    usize::MAX,
                    None,
                )?;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || update.submitted_compile_section_count > 0
                || update.accepted_compile_result_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            combined.merge(update);
            if compiler.pending_job_count() == 0
                && !self.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(combined);
            }
            if Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for render section compile queue");
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    pub fn cached_sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.engine.sections()
    }

    pub fn traversal_ready_render_section_keys(
        &self,
        camera_position: Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        let client = self.client();
        self.render_session()
            .section_keys()
            .filter(|key| {
                self.render_section_within_render_distance(*key)
                    && render_section_neighbor_readiness(client, *key, camera_position).is_ready()
            })
            .collect()
    }

    pub fn render_section_within_render_distance(&self, key: RenderSectionKey) -> bool {
        let distance = i32::try_from(self.render_distance).unwrap_or(i32::MAX);
        (key.chunk_x - self.interest_center.x)
            .abs()
            .max((key.chunk_z - self.interest_center.z).abs())
            <= distance
    }

    pub fn has_pending_render_work(
        &self,
        compiler_pending_job_count: usize,
        camera_position: Vec3,
    ) -> bool {
        self.engine
            .has_pending_render_work(compiler_pending_job_count, |client, key| {
                render_section_neighbor_readiness(client, key, camera_position)
            })
    }

    pub fn has_ready_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.engine.has_pending_render_work(0, |client, key| {
            render_section_neighbor_readiness(client, key, camera_position)
        })
    }

    pub fn pending_completed_compile_result_count(&self) -> usize {
        self.engine.pending_completed_compile_result_count()
    }

    pub fn pending_render_chunk_count(&self) -> usize {
        let mut pending = self
            .render_session()
            .dirty()
            .dirty_chunks
            .iter()
            .filter(|pos| {
                self.client().chunk_snapshot(**pos).is_some()
                    || self.render_session().contains_chunk(**pos)
            })
            .copied()
            .collect::<BTreeSet<_>>();
        pending.extend(
            self.render_session()
                .dirty()
                .dirty_sections
                .iter()
                .copied()
                .filter(|key| {
                    let pos = render_section_chunk_pos(*key);
                    self.client().chunk_snapshot(pos).is_some()
                        || self.render_session().contains_section(*key)
                })
                .map(render_section_chunk_pos),
        );
        pending.extend(
            self.render_session()
                .dirty()
                .inflight_sections
                .iter()
                .copied()
                .map(render_section_chunk_pos),
        );
        pending.len()
    }

    pub fn camera_inside_water(&self, position: Vec3) -> bool {
        let Some(state_id) = self.block_state_at_position(position) else {
            return false;
        };
        camera_position_inside_water_block(position.y, position.y.floor() as i32, state_id)
    }

    pub fn highest_non_air_block_y_at_world(&self, world_x: i32, world_z: i32) -> Option<i32> {
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        let snapshot = self.client().chunk_snapshot(pos)?;
        (snapshot.min_y..snapshot.min_y + snapshot.height)
            .rev()
            .find(|world_y| {
                snapshot_block_state_at_world(snapshot, world_x, *world_y, world_z)
                    .is_some_and(|state_id| state_id != AIR_BLOCK_STATE_ID)
            })
    }

    pub fn block_state_at_position(&self, position: Vec3) -> Option<BlockStateId> {
        if !position.is_finite() {
            return None;
        }
        self.block_state_at_world(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        )
    }

    pub fn block_state_at_world(
        &self,
        world_x: i32,
        world_y: i32,
        world_z: i32,
    ) -> Option<BlockStateId> {
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        self.client()
            .chunk_snapshot(pos)
            .and_then(|snapshot| snapshot_block_state_at_world(snapshot, world_x, world_y, world_z))
    }

    pub fn day_time(&self) -> u64 {
        self.client().day_time()
    }

    pub fn time_of_day(&self) -> f32 {
        self.client().time_of_day()
    }

    pub fn sun_angle(&self) -> f32 {
        self.client().sun_angle()
    }

    pub fn force_day_time(&mut self, day_time: u64) {
        self.client_mut()
            .apply_update(ServerUpdate::TimeUpdate { day_time });
    }

    pub fn stats(
        &self,
        runner_diagnostics: Option<&ServerRunnerDiagnostics>,
        pending_render_compile_jobs: usize,
    ) -> SingleViewRuntimeStats {
        let scheduler_metrics = runner_diagnostics.map(|diagnostics| diagnostics.scheduler_metrics);
        let chunk_tracking = runner_diagnostics.map(|diagnostics| &diagnostics.chunk_tracking);
        SingleViewRuntimeStats {
            server_runner_kind: runner_diagnostics.map(|diagnostics| diagnostics.kind),
            server_command_queue_depth: runner_diagnostics
                .map_or(0, |diagnostics| diagnostics.command_queue_depth),
            server_update_queue_depth: runner_diagnostics
                .map_or(0, |diagnostics| diagnostics.update_queue_depth),
            interest_center: self.interest_center,
            render_distance: self.render_distance,
            chunk_tracking_radius: self.chunk_tracking_radius,
            loaded_chunks: self.client().loaded_chunk_count(),
            pending_jobs: runner_diagnostics.map_or(0, |diagnostics| diagnostics.pending_jobs),
            pending_publications: runner_diagnostics
                .map_or(0, |diagnostics| diagnostics.pending_publications),
            pending_render_chunks: self.pending_render_chunk_count(),
            pending_render_compile_jobs,
            inflight_render_sections: self.render_session().dirty().inflight_sections.len(),
            client_visible_chunks: scheduler_metrics
                .map_or(self.client().loaded_chunk_count(), |metrics| {
                    metrics.client_visible_chunks
                }),
            active_ticket_chunks: scheduler_metrics
                .map_or(0, |metrics| metrics.active_ticket_chunks),
            loading_progress: runner_diagnostics
                .and_then(|diagnostics| diagnostics.loading_progress),
            tracked_players: chunk_tracking
                .map_or(self.client().remote_player_count(), |diagnostics| {
                    diagnostics.player_count
                }),
            player_visible_chunks: chunk_tracking
                .map_or(self.client().loaded_chunk_count(), |diagnostics| {
                    diagnostics.total_player_visible_chunks
                }),
            aggregate_player_ticket_chunks: chunk_tracking
                .map_or(0, |diagnostics| diagnostics.aggregate_player_ticket_chunks),
            player_outbound_queue_depth: chunk_tracking
                .map_or(0, |diagnostics| diagnostics.total_outbound_queue_depth),
            max_player_visible_chunks: chunk_tracking
                .map_or(self.client().loaded_chunk_count(), |diagnostics| {
                    diagnostics.max_player_visible_chunks
                }),
            max_player_outbound_queue_depth: chunk_tracking
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

    pub fn build_client_textured_sections(
        &self,
        catalog: &mclone_mesh::TexturedMeshCatalog,
    ) -> Result<mclone_mesh::TexturedRenderSectionBuildReport> {
        build_client_textured_sections(self.client(), catalog)
    }

    fn apply_runner_diagnostics(
        &mut self,
        runner_diagnostics: &ServerRunnerDiagnostics,
        diagnostics: &mut RuntimePollDiagnostics,
    ) {
        let tick = runner_diagnostics.last_tick;
        diagnostics.server_runner_kind = Some(runner_diagnostics.kind);
        diagnostics.server_command_queue_depth = runner_diagnostics.command_queue_depth;
        diagnostics.server_update_queue_depth = runner_diagnostics.update_queue_depth;
        diagnostics.server_pending_jobs = runner_diagnostics.pending_jobs;
        diagnostics.server_pending_publications = runner_diagnostics.pending_publications;
        diagnostics.server_diagnostics_detail_refreshes =
            runner_diagnostics.diagnostics_detail_refreshes;
        diagnostics.server_diagnostics_detail_age_ms = runner_diagnostics.diagnostics_detail_age_ms;
        diagnostics.server_tick_ms = micros_to_ms(tick.wall_us);
        diagnostics.server_reported_total_ms = micros_to_ms(tick.timing.total_us);
        diagnostics.scheduler_tick_ms = micros_to_ms(tick.timing.scheduler_tick_us);
        diagnostics.scheduler_report_ms = micros_to_ms(tick.timing.scheduler_report_us);
        diagnostics.scheduler_purge_stale_tickets_ms =
            micros_to_ms(tick.timing.scheduler_purge_stale_tickets_us);
        diagnostics.scheduler_reconcile_holders_ms =
            micros_to_ms(tick.timing.scheduler_reconcile_holders_us);
        diagnostics.scheduler_publish_completed_ms =
            micros_to_ms(tick.timing.scheduler_publish_completed_us);
        diagnostics.scheduler_pending_unload_ms =
            micros_to_ms(tick.timing.scheduler_pending_unload_us);
        diagnostics.scheduler_apply_events_ms = micros_to_ms(tick.timing.scheduler_apply_events_us);
        diagnostics.block_tick_ms = micros_to_ms(tick.timing.block_tick_us);
        diagnostics.fluid_tick_ms = micros_to_ms(tick.timing.fluid_tick_us);
        diagnostics.fluid_event_apply_ms = micros_to_ms(tick.timing.fluid_event_apply_us);
        diagnostics.fluid_due_scan_ms = micros_to_ms(tick.timing.fluid_due_scan_us);
        diagnostics.fluid_remove_due_ms = micros_to_ms(tick.timing.fluid_remove_due_us);
        diagnostics.fluid_tick_fluid_ms = micros_to_ms(tick.timing.fluid_tick_fluid_us);
        diagnostics.fluid_set_block_ms = micros_to_ms(tick.timing.fluid_set_block_us);
        diagnostics.entity_tick_ms = micros_to_ms(tick.timing.entity_tick_us);
        diagnostics.scheduler_pending_jobs = runner_diagnostics.scheduler_metrics.pending_jobs;
        diagnostics.scheduler_completed_jobs = runner_diagnostics.scheduler_metrics.completed_jobs;
        diagnostics.scheduler_dirty_chunks = runner_diagnostics.scheduler_metrics.dirty_chunks;
        diagnostics.scheduler_loaded_snapshot_chunks =
            runner_diagnostics.scheduler_metrics.loaded_snapshot_chunks;
        diagnostics.scheduler_client_visible_chunks =
            runner_diagnostics.scheduler_metrics.client_visible_chunks;
        diagnostics.scheduler_active_ticket_chunks =
            runner_diagnostics.scheduler_metrics.active_ticket_chunks;
        diagnostics.loading_progress = runner_diagnostics.loading_progress;
        diagnostics.player_visible_chunks = runner_diagnostics
            .chunk_tracking
            .total_player_visible_chunks;
        diagnostics.player_outbound_queue_depth =
            runner_diagnostics.chunk_tracking.total_outbound_queue_depth;
        diagnostics.scheduler_events = tick.scheduler_event_count;
        diagnostics.pending_unloads_processed = tick.pending_unloads_processed;
        diagnostics.fluid_due_ticks = tick.fluid_due_ticks;
        diagnostics.fluid_executed_ticks = tick.fluid_ticks_executed;
        diagnostics.fluid_deferred_ticks = tick.deferred_fluid_ticks;
        diagnostics.fluid_mutated_blocks = tick.fluid_mutated_blocks;
        diagnostics.fluid_snapshot_events = tick.fluid_snapshot_events;
        diagnostics.fluid_event_count = tick.fluid_event_count;
        diagnostics.scheduled_fluid_ticks = tick.scheduled_fluid_ticks;
        self.last_tick = tick.chunk_tick;
        self.last_simulation_tick = tick.simulation_tick;
        self.last_tick_unloads_processed = tick.pending_unloads_processed;
        self.last_simulation_block_tick_chunks = tick.block_tick_chunks;
        self.last_simulation_entity_tick_chunks = tick.entity_tick_chunks;
        self.last_simulation_scheduler_tick_ms = micros_to_ms(tick.timing.scheduler_tick_us);
        self.last_simulation_block_tick_ms = micros_to_ms(tick.timing.block_tick_us);
        self.last_simulation_fluid_tick_ms = micros_to_ms(tick.timing.fluid_tick_us);
        self.last_simulation_entity_tick_ms = micros_to_ms(tick.timing.entity_tick_us);
        self.last_simulation_fluid_ticks_executed = tick.fluid_ticks_executed;
        self.last_simulation_deferred_fluid_ticks = tick.deferred_fluid_ticks;
        self.last_simulation_fluid_mutated_blocks = tick.fluid_mutated_blocks;
        self.scheduled_fluid_ticks = tick.scheduled_fluid_ticks;
    }
}

pub fn snapshot_block_state_at_world(
    snapshot: &ChunkSnapshot,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<BlockStateId> {
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

pub fn camera_position_inside_water_block(
    position_y: f32,
    block_y: i32,
    state_id: BlockStateId,
) -> bool {
    if !position_y.is_finite() || block_fluid_kind(state_id) != BlockFluidKind::Water {
        return false;
    }
    let Some(fluid_height) = block_fluid_height(state_id) else {
        return false;
    };
    position_y < block_y as f32 + fluid_height
}

pub fn elapsed_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub fn micros_to_ms(micros: u128) -> f64 {
    micros as f64 / 1000.0
}

#[cfg(not(target_arch = "wasm32"))]
type RuntimeTimingSample = Instant;

#[cfg(target_arch = "wasm32")]
type RuntimeTimingSample = ();

#[cfg(not(target_arch = "wasm32"))]
fn timing_start() -> Option<RuntimeTimingSample> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn timing_start() -> Option<RuntimeTimingSample> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn timing_elapsed_ms(start: Option<RuntimeTimingSample>) -> f64 {
    start.map_or(0.0, |start| elapsed_ms(start.elapsed()))
}

#[cfg(target_arch = "wasm32")]
fn timing_elapsed_ms(_start: Option<RuntimeTimingSample>) -> f64 {
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus};
    use mclone_render_session::{RenderSectionCompileRequest, RenderSectionCompileResult};
    use mclone_server::{ChunkLoadingProgressCell, ChunkLoadingProgressSnapshot};

    #[derive(Default)]
    struct DeadlineTestCompiler {
        pending_jobs: usize,
        max_pending_jobs: usize,
        submitted_requests: Vec<RenderSectionCompileRequest>,
    }

    impl DeadlineTestCompiler {
        fn new(max_pending_jobs: usize) -> Self {
            Self {
                max_pending_jobs,
                ..Self::default()
            }
        }
    }

    impl RenderSectionCompiler for DeadlineTestCompiler {
        fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()> {
            self.pending_jobs += 1;
            self.submitted_requests.push(request);
            Ok(())
        }

        fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>> {
            Ok(Vec::new())
        }

        fn pending_job_count(&self) -> usize {
            self.pending_jobs
        }

        fn max_pending_job_count(&self) -> usize {
            self.max_pending_jobs
        }
    }

    fn empty_runtime_test_snapshot(pos: ChunkPos) -> ChunkSnapshot {
        let block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            16,
            &block_state_ids,
        )
    }

    #[test]
    fn chunk_tracking_radius_derives_java_shaped_minimum() {
        assert_eq!(chunk_tracking_radius_for_render_distance(0), 0);
        assert_eq!(chunk_tracking_radius_for_render_distance(1), 1);
        assert_eq!(chunk_tracking_radius_for_render_distance(2), 3);
        assert_eq!(chunk_tracking_radius_for_render_distance(3), 3);
        assert_eq!(chunk_tracking_radius_for_render_distance(4), 4);
    }

    #[test]
    fn debug_block_palette_exposes_torch() {
        assert!(
            DEBUG_BLOCK_PALETTE
                .iter()
                .any(|&(state, label)| state == BlockStateId(100) && label == "Torch")
        );
    }

    #[test]
    fn loading_progress_snapshot_maps_to_shared_overlay() {
        let snapshot = ChunkLoadingProgressSnapshot {
            stats: ChunkLoadingProgressStats {
                center: ChunkPos::new(4, -3),
                target_radius: 1,
                target_status: ChunkStatus::Light,
                target_chunk_count: 9,
                target_ready_chunks: 2,
                playable_chunk: ChunkPos::new(4, -3),
                playable_chunk_ready: true,
            },
            cells: vec![
                ChunkLoadingProgressCell {
                    relative_x: 0,
                    relative_z: 0,
                    status: Some(ChunkStatus::Light),
                    target_ready: true,
                    playable: true,
                },
                ChunkLoadingProgressCell {
                    relative_x: 1,
                    relative_z: 0,
                    status: Some(ChunkStatus::Features),
                    target_ready: false,
                    playable: false,
                },
                ChunkLoadingProgressCell {
                    relative_x: -1,
                    relative_z: 0,
                    status: None,
                    target_ready: false,
                    playable: false,
                },
            ],
        };

        let overlay = loading_progress_overlay_from_snapshot(&snapshot);

        assert_eq!(overlay.grid_side(), 3);
        assert_eq!(overlay.percent(), 22);
        assert!(overlay.playable_ready);
        assert_eq!(
            overlay.status_at(0, 0),
            LoadingProgressCellStatus::TargetReady
        );
        assert_eq!(overlay.status_at(1, 0), LoadingProgressCellStatus::Features);
        assert_eq!(overlay.status_at(-1, 0), LoadingProgressCellStatus::None);
        assert!(overlay.playable_cell().unwrap().playable);

        let mut diagnostics =
            ServerRunnerDiagnostics::initial(ServerRunnerKind::InlineFallback, 0, 0);
        diagnostics.view_readiness_snapshot = Some(snapshot);
        let diagnostics_overlay = view_readiness_overlay_from_diagnostics(&diagnostics)
            .expect("view readiness diagnostics should map to an overlay");
        assert_eq!(diagnostics_overlay.percent(), overlay.percent());
        assert_eq!(
            diagnostics_overlay.status_at(1, 0),
            LoadingProgressCellStatus::Features
        );
    }

    #[test]
    fn camera_water_detection_uses_fluid_height_boundary() {
        let water = mclone_client::block_facts::WATER_BLOCK_STATE_ID;
        let lava = mclone_client::block_facts::LAVA_BLOCK_STATE_ID;
        let air = AIR_BLOCK_STATE_ID;

        assert!(camera_position_inside_water_block(62.999, 62, water));
        assert!(!camera_position_inside_water_block(63.0, 62, water));
        assert!(!camera_position_inside_water_block(62.5, 62, lava));
        assert!(!camera_position_inside_water_block(62.5, 62, air));
        assert!(!camera_position_inside_water_block(f32::NAN, 62, water));
    }

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
    fn runtime_exchange_updates_counters_and_client_state() {
        let mut runtime = SingleViewRuntime::local_integrated(ChunkPos::new(0, 0), 0, 0);
        let report =
            runtime.apply_exchange(RuntimeExchange::command(vec![ServerUpdate::TimeUpdate {
                day_time: 6000,
            }]));

        assert_eq!(report.command_count, 1);
        assert_eq!(report.update_count, 1);
        assert_eq!(runtime.command_count(), 1);
        assert_eq!(runtime.update_count(), 1);
        assert_eq!(runtime.snapshot_update_count(), 0);
        assert_eq!(runtime.section_block_update_count(), 0);
        assert_eq!(runtime.unload_update_count(), 0);
        assert_eq!(runtime.day_time(), 6000);

        let block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            16,
            &block_state_ids,
        );
        let report = runtime.apply_exchange(RuntimeExchange::updates(
            vec![
                ServerUpdate::ChunkSnapshot(snapshot),
                ServerUpdate::SectionBlockUpdates {
                    pos: ChunkPos::new(0, 0),
                    section_y: 0,
                    updates: vec![mclone_protocol::SectionBlockUpdate {
                        local_x: 1,
                        local_y: 2,
                        local_z: 3,
                        block_state: BlockStateId(1),
                    }],
                },
                ServerUpdate::ChunkUnload {
                    pos: ChunkPos::new(0, 0),
                },
            ],
            true,
        ));

        assert_eq!(report.command_count, 0);
        assert_eq!(report.update_count, 3);
        assert_eq!(runtime.update_count(), 4);
        assert_eq!(runtime.snapshot_update_count(), 1);
        assert_eq!(runtime.section_block_update_count(), 1);
        assert_eq!(runtime.unload_update_count(), 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn render_section_deadline_loop_reports_skipped_ready_admission() {
        let chunks = [ChunkPos::new(0, 0), ChunkPos::new(1, 0)];
        let mut runtime = SingleViewRuntime::local_integrated(ChunkPos::new(0, 0), 2, 2);
        runtime.apply_server_updates(
            chunks
                .iter()
                .copied()
                .map(empty_runtime_test_snapshot)
                .map(ServerUpdate::ChunkSnapshot)
                .collect(),
        );
        let mut compiler = DeadlineTestCompiler::new(2);

        let update = runtime
            .sync_render_sections_until_deadline(
                &mut compiler,
                Vec3::new(8.0, 8.0, 8.0),
                Instant::now(),
                |client, _compiler| client.chunk_snapshots().cloned().collect(),
            )
            .expect("deadline render sync should succeed");

        assert_eq!(compiler.submitted_requests.len(), 1);
        assert_eq!(update.submitted_compile_section_count, 1);
        assert_eq!(update.deadline_skipped_compile_request_count, 1);
        assert_eq!(update.pending_compile_jobs, 1);
        assert!(
            runtime
                .has_pending_render_work(compiler.pending_job_count(), Vec3::new(8.0, 8.0, 8.0),)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn targeted_snapshot_submission_clones_only_target_chunk_neighborhood() {
        let target = ChunkPos::new(0, 0);
        let required_chunks = BTreeSet::from([
            target,
            ChunkPos::new(-1, 0),
            ChunkPos::new(1, 0),
            ChunkPos::new(0, -1),
            ChunkPos::new(0, 1),
        ]);
        let unrelated = ChunkPos::new(4, 4);
        let mut runtime = SingleViewRuntime::local_integrated(target, 5, 5);
        runtime.apply_server_updates(
            required_chunks
                .iter()
                .copied()
                .chain([unrelated])
                .map(empty_runtime_test_snapshot)
                .map(ServerUpdate::ChunkSnapshot)
                .collect(),
        );
        let mut compiler = DeadlineTestCompiler::new(1);

        let update = runtime
            .sync_render_sections_with_budget_and_completed_result_acceptance_targeted_snapshots(
                &mut compiler,
                Vec3::new(8.0, 8.0, 8.0),
                DEFAULT_RENDER_CHUNK_MESH_BUDGET,
                None,
            )
            .expect("targeted render sync should succeed");

        assert_eq!(update.submitted_compile_section_count, 1);
        assert_eq!(compiler.submitted_requests.len(), 1);
        let snapshot_chunks = compiler.submitted_requests[0]
            .snapshots
            .iter()
            .map(|snapshot| snapshot.pos)
            .collect::<BTreeSet<_>>();
        assert_eq!(snapshot_chunks, required_chunks);
        assert!(!snapshot_chunks.contains(&unrelated));
    }
}
