#![deny(unsafe_code)]

use std::collections::VecDeque;

use mclone_app_runtime::client_connection::{
    ClientConnection, ClientConnectionDrainResult, ClientConnectionQueueMetrics,
    QueuedServerUpdate, pump_client_connection_updates_report,
};
#[cfg(target_arch = "wasm32")]
use mclone_app_runtime::host_mode::diagnostics_command_update_queues_drained;
use mclone_app_runtime::{
    RuntimeExchange, RuntimeStepReport, RuntimeUpdatePumpBudget, SingleViewRuntime,
};
use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::ChunkPos;
use mclone_net::LocalTransport;
use mclone_protocol::{
    ChunkView, ClientCommand, ProtocolCodecError, ProtocolCodecResult, ServerUpdate,
    decode_client_command, decode_server_update, encode_client_command, encode_server_update,
};
use mclone_render::RenderBackend;
use mclone_server::LocalRealmSession;
#[cfg(target_arch = "wasm32")]
use mclone_server::{IntegratedServerRunner, ServerRunnerDiagnostics, ServerRunnerKind};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

mod render_worker_coordinator;
pub mod web_scene_protocol;

#[cfg(target_arch = "wasm32")]
mod web_canvas;
#[cfg(target_arch = "wasm32")]
mod web_scene_host;
#[cfg(target_arch = "wasm32")]
pub use web_scene_host::{
    WebSceneHost, mclone_web_create_remote_scene_host_with_startup,
    mclone_web_create_worker_scene_host_with_startup,
};
#[cfg(target_arch = "wasm32")]
mod web_compile_timing;
#[cfg(target_arch = "wasm32")]
mod web_remote_session;
#[cfg(target_arch = "wasm32")]
mod web_render_worker;
#[cfg(target_arch = "wasm32")]
mod web_server_worker;

#[cfg(target_arch = "wasm32")]
use web_remote_session::WebSocketServerSession;
#[cfg(target_arch = "wasm32")]
pub use web_server_worker::{WebIntegratedServerRunner, WebIntegratedServerRunnerConfig};

const SMOKE_SEED: i64 = 12_345;
const SMOKE_INITIAL_CENTER: ChunkPos = ChunkPos { x: 0, z: 0 };
const SMOKE_MOVED_CENTER: ChunkPos = ChunkPos { x: 1, z: 0 };
const SMOKE_RADIUS_CHUNKS: u32 = 0;

const OK_BIT: u32 = 1 << 0;
const LOCAL_HOST_BIT: u32 = 1 << 1;
const WEB_RENDER_BACKEND_BIT: u32 = 1 << 2;
const CENTER_CHUNK_LOADED_BIT: u32 = 1 << 3;
const TRANSPORT_DRAINED_BIT: u32 = 1 << 4;
const MOVED_CHUNK_LOADED_BIT: u32 = 1 << 5;
const PREVIOUS_CHUNK_UNLOADED_BIT: u32 = 1 << 6;
const PROTOCOL_CODEC_ROUNDTRIP_BIT: u32 = 1 << 7;
const COMMAND_COUNT_SHIFT: u32 = 8;
const UPDATE_COUNT_SHIFT: u32 = 16;
const LOADED_CHUNK_COUNT_SHIFT: u32 = 24;

/// Build a Rust-owned dead-player blob for the browser persistence smoke.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mclone_web_dead_player_record_fixture(
    profile_id: js_sys::Uint8Array,
    display_name: String,
) -> Result<js_sys::Uint8Array, JsValue> {
    let bytes: [u8; 16] = profile_id
        .to_vec()
        .try_into()
        .map_err(|_| JsValue::from_str("player record fixture UUID must contain 16 bytes"))?;
    let key = mclone_server::PlayerRecordKey::Uuid(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    ));
    let mut record = mclone_server::PlayerRecord::new(
        key,
        1,
        display_name,
        mclone_core::Vec3d::new(0.5, 93.0, 0.5),
    );
    record.on_ground = true;
    record.health = 0.0;
    record.pending_death_cause = Some(mclone_protocol::PlayerDamageCause::Lava);
    record
        .statistics
        .increment(mclone_protocol::StatisticKey::deaths(), 1);
    let encoded = mclone_server::encode_player_record(&record)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(js_sys::Uint8Array::from(encoded.as_slice()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mclone_web_remote_handshake_frame(
    profile_id: js_sys::Uint8Array,
    display_name: String,
) -> Result<js_sys::Uint8Array, JsValue> {
    let profile_id: [u8; 16] = profile_id
        .to_vec()
        .try_into()
        .map_err(|_| JsValue::from_str("remote profile UUID must contain 16 bytes"))?;
    let identity = mclone_protocol::ClientIdentity::new(
        mclone_protocol::PlayerProfileId::new(profile_id),
        display_name,
    )
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let frame = mclone_net::encode_websocket_client_handshake_with_identity(
        mclone_protocol::PROTOCOL_VERSION,
        &identity,
    )
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(js_sys::Uint8Array::from(frame.as_slice()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mclone_web_validate_remote_handshake(frame: js_sys::Uint8Array) -> Result<(), JsValue> {
    mclone_net::decode_websocket_server_handshake(
        &frame.to_vec(),
        mclone_protocol::PROTOCOL_VERSION,
    )
    .map(|_| ())
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mclone_web_canonicalize_remote_command(
    frame: js_sys::Uint8Array,
) -> Result<js_sys::Uint8Array, JsValue> {
    let command = mclone_net::decode_websocket_client_command(&frame.to_vec())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let canonical = mclone_net::encode_websocket_client_command(&command)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(js_sys::Uint8Array::from(canonical.as_slice()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mclone_web_decode_remote_update_batch(
    frame: js_sys::Uint8Array,
) -> Result<js_sys::Array, JsValue> {
    let updates = mclone_net::decode_websocket_server_update_batch(&frame.to_vec())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let frames = js_sys::Array::new();
    for update in updates {
        let canonical =
            encode_server_update(&update).map_err(|error| JsValue::from_str(&error.to_string()))?;
        frames.push(&js_sys::Uint8Array::from(canonical.as_slice()));
    }
    Ok(frames)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mclone_web_remote_control_response(
    frame: js_sys::Uint8Array,
) -> Result<js_sys::Uint8Array, JsValue> {
    let update = decode_server_update(&frame.to_vec())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let ServerUpdate::KeepAlive { id } = update else {
        return Ok(js_sys::Uint8Array::new_with_length(0));
    };
    let response = mclone_net::encode_websocket_client_command(&ClientCommand::KeepAlive { id })
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(js_sys::Uint8Array::from(response.as_slice()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebSmokeReport {
    pub ok: bool,
    pub local_host: bool,
    pub web_render_backend: bool,
    pub center_chunk_loaded: bool,
    pub transport_drained: bool,
    pub moved_chunk_loaded: bool,
    pub previous_chunk_unloaded: bool,
    pub protocol_codec_roundtrip: bool,
    pub command_count: usize,
    pub update_count: usize,
    pub loaded_chunk_count: usize,
}

impl WebSmokeReport {
    pub const fn failed() -> Self {
        Self {
            ok: false,
            local_host: false,
            web_render_backend: false,
            center_chunk_loaded: false,
            transport_drained: false,
            moved_chunk_loaded: false,
            previous_chunk_unloaded: false,
            protocol_codec_roundtrip: false,
            command_count: 0,
            update_count: 0,
            loaded_chunk_count: 0,
        }
    }

    pub fn packed_bits(self) -> u32 {
        bool_bit(self.ok, OK_BIT)
            | bool_bit(self.local_host, LOCAL_HOST_BIT)
            | bool_bit(self.web_render_backend, WEB_RENDER_BACKEND_BIT)
            | bool_bit(self.center_chunk_loaded, CENTER_CHUNK_LOADED_BIT)
            | bool_bit(self.transport_drained, TRANSPORT_DRAINED_BIT)
            | bool_bit(self.moved_chunk_loaded, MOVED_CHUNK_LOADED_BIT)
            | bool_bit(self.previous_chunk_unloaded, PREVIOUS_CHUNK_UNLOADED_BIT)
            | bool_bit(self.protocol_codec_roundtrip, PROTOCOL_CODEC_ROUNDTRIP_BIT)
            | (packed_count(self.command_count) << COMMAND_COUNT_SHIFT)
            | (packed_count(self.update_count) << UPDATE_COUNT_SHIFT)
            | (packed_count(self.loaded_chunk_count) << LOADED_CHUNK_COUNT_SHIFT)
    }
}

#[derive(Debug)]
pub(crate) struct WebRuntime {
    core: SingleViewRuntime,
    host: WebRuntimeHost,
}

impl WebRuntime {
    pub fn local_integrated(seed: i64) -> Self {
        Self::local_integrated_at(seed, SMOKE_INITIAL_CENTER)
    }

    pub fn local_integrated_at(seed: i64, initial_center: ChunkPos) -> Self {
        let host = WebLoopbackHost::new(seed);
        let mut core = SingleViewRuntime::local_integrated_with_seed(seed, initial_center, 0, 0);
        core.force_day_time(host.day_time());
        Self {
            core,
            host: WebRuntimeHost::Inline(host),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn web_worker_integrated_at(
        config: WebIntegratedServerRunnerConfig,
        initial_center: ChunkPos,
    ) -> Result<Self, String> {
        let seed = config.seed;
        let runner = WebIntegratedServerRunner::new(config).await?;
        let diagnostics = runner.diagnostics();
        let mut core = SingleViewRuntime::local_integrated_with_seed(seed, initial_center, 0, 0);
        core.force_day_time(diagnostics.day_time);
        Ok(Self {
            core,
            host: WebRuntimeHost::Worker(runner),
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn websocket_remote(url: impl Into<String>) -> Result<Self, String> {
        Self::websocket_remote_at(url, SMOKE_INITIAL_CENTER).await
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn websocket_remote_at(
        url: impl Into<String>,
        initial_center: ChunkPos,
    ) -> Result<Self, String> {
        let session = WebSocketServerSession::connect(url).await?;
        Ok(Self {
            core: SingleViewRuntime::remote_dedicated(initial_center, 0, 0),
            host: WebRuntimeHost::RemoteWebSocket(session),
        })
    }

    pub fn request_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> ProtocolCodecResult<WebRuntimeStepReport> {
        let command = self.chunk_view_command(center, render_distance, chunk_tracking_radius);
        self.send_and_drain_immediately(command)
            .map_err(|_| ProtocolCodecError::InvalidData("web runtime command failed"))
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn request_chunk_view_async(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<WebRuntimeStepReport, String> {
        let command = self.chunk_view_command(center, render_distance, chunk_tracking_radius);
        self.send_and_drain_immediately_async(command).await
    }

    #[cfg(target_arch = "wasm32")]
    pub fn request_chunk_view_deferred(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<WebRuntimeStepReport, String> {
        let command = self.chunk_view_command(center, render_distance, chunk_tracking_radius);
        let exchange = self.host.enqueue_command(command)?;
        Ok(self.apply_exchange(exchange))
    }

    #[cfg(test)]
    fn send_gameplay_command(
        &mut self,
        command: ClientCommand,
    ) -> ProtocolCodecResult<WebRuntimeStepReport> {
        self.send_and_drain_immediately(command)
            .map_err(|_| ProtocolCodecError::InvalidData("web runtime command failed"))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn send_gameplay_command_deferred(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebRuntimeStepReport, String> {
        let exchange = self.host.enqueue_command(command)?;
        Ok(self.apply_exchange(exchange))
    }

    pub fn drain_pending_runner_updates_with_budget(
        &mut self,
        budget: RuntimeUpdatePumpBudget,
    ) -> Result<WebRuntimeStepReport, String> {
        let pump_report = self.drain_pending_runner_updates_report(budget)?;
        Ok(WebRuntimeStepReport {
            command_count: 0,
            update_count: pump_report.apply_report.updates,
            loaded_chunk_count: self.core.client().loaded_chunk_count(),
            protocol_codec_roundtrip: true,
            transport_drained: self.host.command_update_queues_drained(),
        })
    }

    pub(crate) fn drain_pending_runner_updates_report(
        &mut self,
        budget: RuntimeUpdatePumpBudget,
    ) -> Result<mclone_app_runtime::RuntimeUpdatePumpReport, String> {
        pump_client_connection_updates_report(&mut self.core, &mut self.host, budget)
            .map_err(|error| error.to_string())
    }

    fn chunk_view_command(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> ClientCommand {
        self.core
            .set_chunk_view_command(center, render_distance, chunk_tracking_radius)
            .unwrap_or(ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance,
                chunk_tracking_radius,
            }))
    }

    fn apply_exchange(&mut self, exchange: RuntimeExchange) -> WebRuntimeStepReport {
        self.core.apply_exchange(exchange)
    }

    fn send_and_drain_immediately(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebRuntimeStepReport, String> {
        let command_exchange = self.host.enqueue_command(command)?;
        let drain_report =
            self.drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())?;
        let command_report =
            self.apply_immediate_command_accounting(command_exchange, drain_report);
        Ok(combine_step_reports(command_report, drain_report))
    }

    #[cfg(target_arch = "wasm32")]
    async fn send_and_drain_immediately_async(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebRuntimeStepReport, String> {
        self.enqueue_and_drain_immediately_async(command).await
    }

    #[cfg(target_arch = "wasm32")]
    async fn enqueue_and_drain_immediately_async(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebRuntimeStepReport, String> {
        let command_exchange = self.host.enqueue_command_async(command).await?;
        let drain_report =
            self.drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())?;
        let command_report =
            self.apply_immediate_command_accounting(command_exchange, drain_report);
        Ok(combine_step_reports(command_report, drain_report))
    }

    fn apply_immediate_command_accounting(
        &mut self,
        command_exchange: RuntimeExchange,
        drain_report: WebRuntimeStepReport,
    ) -> WebRuntimeStepReport {
        self.apply_exchange(RuntimeExchange::new(
            Vec::new(),
            command_exchange.command_count,
            command_exchange.protocol_codec_roundtrip,
            drain_report.transport_drained,
        ))
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.core.client()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) const fn scene_core(&self) -> &SingleViewRuntime {
        &self.core
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn scene_core_mut(&mut self) -> &mut SingleViewRuntime {
        &mut self.core
    }

    pub const fn command_count(&self) -> usize {
        self.core.command_count()
    }

    pub const fn update_count(&self) -> usize {
        self.core.update_count()
    }

    pub const fn protocol_codec_roundtrip(&self) -> bool {
        self.core.protocol_codec_roundtrip()
    }

    pub const fn transport_drained(&self) -> bool {
        self.core.transport_drained()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn runner_kind(&self) -> ServerRunnerKind {
        self.host.runner_kind()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn runner_diagnostics(&self) -> ServerRunnerDiagnostics {
        self.host.runner_diagnostics()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn request_shutdown(&mut self) {
        self.host.request_shutdown();
    }

    #[cfg(target_arch = "wasm32")]
    pub fn flush_persistence(&mut self) -> Result<usize, String> {
        self.host.flush_persistence()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn promote_observer_to_player(&mut self) -> Result<(), String> {
        self.host.promote_observer_to_player()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn demote_player_to_observer(&mut self) -> Result<(), String> {
        self.host.demote_player_to_observer()
    }
}

pub(crate) type WebRuntimeStepReport = RuntimeStepReport;

#[derive(Debug)]
enum WebRuntimeHost {
    Inline(WebLoopbackHost),
    #[cfg(target_arch = "wasm32")]
    Worker(WebIntegratedServerRunner),
    #[cfg(target_arch = "wasm32")]
    RemoteWebSocket(WebSocketServerSession),
}

impl WebRuntimeHost {
    fn enqueue_command(&mut self, command: ClientCommand) -> Result<RuntimeExchange, String> {
        match self {
            Self::Inline(host) => host
                .queue_command(command)
                .map(command_deferred_exchange)
                .map_err(|error| error.to_string()),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.queue_command(command).map(command_deferred_exchange),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => {
                host.queue_command(command).map(command_deferred_exchange)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn enqueue_command_async(
        &mut self,
        command: ClientCommand,
    ) -> Result<RuntimeExchange, String> {
        match self {
            Self::Inline(host) => host
                .queue_command(command)
                .map(command_deferred_exchange)
                .map_err(|error| error.to_string()),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host
                .send_command_acknowledged(command)
                .await
                .map(command_deferred_exchange),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => {
                host.queue_command(command).map(command_deferred_exchange)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn runner_kind(&self) -> ServerRunnerKind {
        match self {
            Self::Inline(_) => ServerRunnerKind::InlineFallback,
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.kind(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(_) => ServerRunnerKind::RemoteWebSocket,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn runner_diagnostics(&self) -> ServerRunnerDiagnostics {
        match self {
            Self::Inline(host) => host.diagnostics(),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.diagnostics(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => host.diagnostics(),
        }
    }

    fn command_update_queues_drained(&self) -> bool {
        match self {
            Self::Inline(host) => host.command_update_queues_drained(),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => diagnostics_command_update_queues_drained(&host.diagnostics()),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => {
                diagnostics_command_update_queues_drained(&host.diagnostics())
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn request_shutdown(&mut self) {
        match self {
            Self::Inline(_) => {}
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.request_shutdown(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => host.request_shutdown(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn flush_persistence(&mut self) -> Result<usize, String> {
        match self {
            Self::Inline(_) | Self::RemoteWebSocket(_) => Ok(0),
            Self::Worker(host) => {
                IntegratedServerRunner::flush_persistence(host).map_err(|error| error.to_string())
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn promote_observer_to_player(&mut self) -> Result<(), String> {
        match self {
            Self::Worker(host) => IntegratedServerRunner::promote_observer_to_player(host)
                .map_err(|error| error.to_string()),
            Self::Inline(_) | Self::RemoteWebSocket(_) => {
                Err("web runtime does not own a promotable local observer".to_owned())
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn demote_player_to_observer(&mut self) -> Result<(), String> {
        match self {
            Self::Worker(host) => IntegratedServerRunner::demote_player_to_observer(host)
                .map_err(|error| error.to_string()),
            Self::Inline(_) | Self::RemoteWebSocket(_) => {
                Err("web runtime does not own a demotable local player".to_owned())
            }
        }
    }
}

impl ClientConnection for WebRuntimeHost {
    fn send_command_only(&mut self, command: ClientCommand) -> anyhow::Result<()> {
        self.enqueue_command(command)
            .map(|_| ())
            .map_err(anyhow::Error::msg)
    }

    fn try_drain_next_update(&mut self) -> anyhow::Result<ClientConnectionDrainResult> {
        match self {
            Self::Inline(host) => host.drain_next_update().map_err(anyhow::Error::msg),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.drain_next_queued_update().map_err(anyhow::Error::msg),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => {
                host.drain_next_queued_update().map_err(anyhow::Error::msg)
            }
        }
    }

    fn pending_update_metrics(&mut self) -> anyhow::Result<ClientConnectionQueueMetrics> {
        Ok(match self {
            Self::Inline(host) => host.queued_update_metrics(),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.queued_update_metrics(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => host.queued_update_metrics(),
        })
    }
}

fn command_deferred_exchange(protocol_codec_roundtrip: bool) -> RuntimeExchange {
    RuntimeExchange::new(Vec::new(), 1, protocol_codec_roundtrip, false)
}

fn combine_step_reports(
    command_report: WebRuntimeStepReport,
    drain_report: WebRuntimeStepReport,
) -> WebRuntimeStepReport {
    WebRuntimeStepReport {
        command_count: command_report
            .command_count
            .saturating_add(drain_report.command_count),
        update_count: command_report
            .update_count
            .saturating_add(drain_report.update_count),
        loaded_chunk_count: drain_report.loaded_chunk_count,
        protocol_codec_roundtrip: command_report.protocol_codec_roundtrip
            && drain_report.protocol_codec_roundtrip,
        transport_drained: drain_report.transport_drained,
    }
}

#[derive(Debug)]
struct WebLoopbackHost {
    server: LocalRealmSession,
    transport: LocalTransport,
    queued_updates: VecDeque<(ServerUpdate, usize)>,
    queued_update_bytes: usize,
}

impl WebLoopbackHost {
    fn new(seed: i64) -> Self {
        Self {
            server: LocalRealmSession::local_integrated(seed),
            transport: LocalTransport::new(),
            queued_updates: VecDeque::new(),
            queued_update_bytes: 0,
        }
    }

    fn day_time(&self) -> u64 {
        self.server.day_time()
    }

    #[cfg(target_arch = "wasm32")]
    fn seed(&self) -> i64 {
        self.server.seed()
    }

    #[cfg(target_arch = "wasm32")]
    fn diagnostics(&self) -> ServerRunnerDiagnostics {
        let mut diagnostics = ServerRunnerDiagnostics::initial(
            ServerRunnerKind::InlineFallback,
            self.seed(),
            self.day_time(),
        );
        diagnostics.command_queue_depth = self.transport.pending_client_command_count();
        diagnostics.update_queue_depth = self.queued_updates.len();
        diagnostics.update_queue_bytes = self.queued_update_bytes;
        diagnostics.pending_jobs = self.server.pending_job_count();
        diagnostics.pending_publications = self.server.pending_publication_count();
        diagnostics.pending_persistence_loads =
            self.server.scheduler().pending_persistence_load_count();
        diagnostics.pending_persistence_saves =
            self.server.scheduler().pending_persistence_save_count();
        diagnostics
    }

    fn queue_command(&mut self, command: ClientCommand) -> ProtocolCodecResult<bool> {
        let encoded_command = encode_client_command(&command)?;
        let decoded_command = decode_client_command(&encoded_command)?;
        let mut protocol_codec_roundtrip = decoded_command == command;
        self.transport.send_client_command(decoded_command);

        let commands = self.transport.drain_client_commands();
        for command in commands {
            for update in self.server.handle_command(command) {
                self.send_roundtripped_update(update, &mut protocol_codec_roundtrip)?;
            }
        }
        for _ in 0..60_000 {
            if self.server.pending_job_count() == 0
                && self.server.scheduler().pending_persistence_load_count() == 0
            {
                break;
            }
            self.poll_server_updates(&mut protocol_codec_roundtrip)?;
            if self.server.pending_job_count() > 0 && self.server.pending_publication_count() == 0 {
                wait_for_worker_tick();
            }
        }
        if self.server.pending_job_count() > 0
            || self.server.scheduler().pending_persistence_load_count() > 0
        {
            return Err(ProtocolCodecError::InvalidData(
                "timed out waiting for web loopback chunk load jobs",
            ));
        }
        self.poll_server_updates(&mut protocol_codec_roundtrip)?;

        Ok(protocol_codec_roundtrip)
    }

    fn drain_next_update(&mut self) -> Result<ClientConnectionDrainResult, String> {
        let Some((update, encoded_len)) = self.queued_updates.pop_front() else {
            return Ok(ClientConnectionDrainResult::default());
        };
        self.queued_update_bytes = self.queued_update_bytes.saturating_sub(encoded_len);
        let transport_drained = self.command_update_queues_drained();
        Ok(ClientConnectionDrainResult::with_update(
            QueuedServerUpdate::single(
                update,
                encoded_len,
                std::time::Duration::ZERO,
                transport_drained,
            ),
            self.queued_updates.len(),
            self.queued_update_bytes,
        ))
    }

    fn queued_update_metrics(&self) -> ClientConnectionQueueMetrics {
        ClientConnectionQueueMetrics::new(self.queued_updates.len(), self.queued_update_bytes)
    }

    fn command_update_queues_drained(&self) -> bool {
        self.transport.pending_client_command_count() == 0 && self.queued_updates.is_empty()
    }

    fn poll_server_updates(
        &mut self,
        protocol_codec_roundtrip: &mut bool,
    ) -> ProtocolCodecResult<()> {
        for update in self.server.poll() {
            self.send_roundtripped_update(update, protocol_codec_roundtrip)?;
        }
        Ok(())
    }

    fn send_roundtripped_update(
        &mut self,
        update: ServerUpdate,
        protocol_codec_roundtrip: &mut bool,
    ) -> ProtocolCodecResult<()> {
        let encoded_update = encode_server_update(&update)?;
        let encoded_len = encoded_update.len();
        let decoded_update = decode_server_update(&encoded_update)?;
        *protocol_codec_roundtrip &= decoded_update == update;
        self.queued_updates.push_back((decoded_update, encoded_len));
        self.queued_update_bytes = self.queued_update_bytes.saturating_add(encoded_len);
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_worker_tick() {
    std::thread::sleep(std::time::Duration::from_millis(1));
}

#[cfg(target_arch = "wasm32")]
fn wait_for_worker_tick() {
    std::hint::spin_loop();
}

pub fn try_run_web_runtime_smoke() -> ProtocolCodecResult<WebSmokeReport> {
    let mut runtime = WebRuntime::local_integrated(SMOKE_SEED);

    runtime.request_chunk_view(
        SMOKE_INITIAL_CENTER,
        SMOKE_RADIUS_CHUNKS,
        SMOKE_RADIUS_CHUNKS,
    )?;
    let center_chunk_loaded = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .is_some();

    runtime.request_chunk_view(SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_RADIUS_CHUNKS)?;
    let moved_chunk_loaded = runtime
        .client()
        .chunk_snapshot(SMOKE_MOVED_CENTER)
        .is_some();
    let previous_chunk_unloaded = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .is_none();

    let local_host = runtime.client().host() == ClientHost::LocalIntegrated;
    let web_render_backend = RenderBackend::WebWgpu == RenderBackend::WebWgpu;
    let transport_drained = runtime.transport_drained();
    let protocol_codec_roundtrip = runtime.protocol_codec_roundtrip();
    let command_count = runtime.command_count();
    let update_count = runtime.update_count();
    let loaded_chunk_count = runtime.client().loaded_chunk_count();
    let ok = local_host
        && web_render_backend
        && center_chunk_loaded
        && moved_chunk_loaded
        && previous_chunk_unloaded
        && transport_drained
        && protocol_codec_roundtrip
        && command_count == 2
        && update_count == 13
        && loaded_chunk_count == 1;

    Ok(WebSmokeReport {
        ok,
        local_host,
        web_render_backend,
        center_chunk_loaded,
        transport_drained,
        moved_chunk_loaded,
        previous_chunk_unloaded,
        protocol_codec_roundtrip,
        command_count,
        update_count,
        loaded_chunk_count,
    })
}

pub fn run_web_runtime_smoke() -> WebSmokeReport {
    try_run_web_runtime_smoke().unwrap_or_else(|_| WebSmokeReport::failed())
}

pub fn mclone_web_smoke() -> u32 {
    run_web_runtime_smoke().packed_bits()
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn mclone_web_runtime_smoke_report() -> u32 {
    mclone_web_smoke()
}

const SCENE_ADAPTER_CLOCK_BIT: u32 = 1 << 0;
const SCENE_ADAPTER_SUPERSESSION_BIT: u32 = 1 << 1;
const SCENE_ADAPTER_RECONNECT_BIT: u32 = 1 << 2;
const SCENE_ADAPTER_CATALOG_BIT: u32 = 1 << 3;
const SCENE_ADAPTER_TEARDOWN_BIT: u32 = 1 << 4;

pub fn web_scene_adapter_contract_bits() -> u32 {
    use mclone_app_runtime::client_catalog_policy::{
        ClientCatalogController, ClientCatalogRequest,
    };
    use mclone_app_runtime::platform_operation::PlatformOperationCompletion;
    use mclone_app_runtime::session::{ActiveSessionDescriptor, RemoteSessionEndpoint};
    use mclone_app_runtime::world_catalog::{
        WorldCatalogCapabilities, WorldCatalogRequest, WorldCatalogRequestId, WorldCatalogResponse,
    };
    use web_scene_protocol::{
        WebScenePlatformServices, WebSceneSessionCompletionDisposition, WebSceneSessionOperation,
        WebSceneSessionOperationResult, WebSceneSessionState,
    };

    let (mut services, _clock, mut catalog_operations) = WebScenePlatformServices::new();
    let observed = services.observe_frame_time_millis(12.5);
    let regressed = services.observe_frame_time_millis(4.0);
    let clock_ok = observed == regressed;

    let first = services
        .lifecycle_mut()
        .begin_start(WebSceneSessionOperation::ConnectRemote {
            url: "ws://old.invalid".to_owned(),
        });
    let second = services
        .lifecycle_mut()
        .begin_start(WebSceneSessionOperation::ConnectRemote {
            url: "ws://new.invalid".to_owned(),
        });
    let stale = services
        .lifecycle_mut()
        .complete(PlatformOperationCompletion {
            token: first.token,
            result: Ok(WebSceneSessionOperationResult::Started(
                ActiveSessionDescriptor::Remote {
                    endpoint: RemoteSessionEndpoint::new("ws://old.invalid"),
                },
            )),
        });
    let applied = services
        .lifecycle_mut()
        .complete(PlatformOperationCompletion {
            token: second.token,
            result: Ok(WebSceneSessionOperationResult::Started(
                ActiveSessionDescriptor::Remote {
                    endpoint: RemoteSessionEndpoint::new("ws://new.invalid"),
                },
            )),
        });
    let supersession_ok = stale == WebSceneSessionCompletionDisposition::Stale
        && applied == WebSceneSessionCompletionDisposition::Applied;

    let reconnect = services
        .lifecycle_mut()
        .begin_reconnect("ws://new.invalid")
        .expect("active remote adapter state can reconnect");
    let reconnect_started = matches!(
        services.lifecycle().state(),
        WebSceneSessionState::Reconnecting { attempt: 1, .. }
    );
    let reconnect_applied = services
        .lifecycle_mut()
        .complete(PlatformOperationCompletion {
            token: reconnect.token,
            result: Ok(WebSceneSessionOperationResult::Reconnected),
        });
    let reconnect_ok =
        reconnect_started && reconnect_applied == WebSceneSessionCompletionDisposition::Applied;

    let request = ClientCatalogRequest {
        id: WorldCatalogRequestId(1),
        request: WorldCatalogRequest::ListWorlds,
    };
    catalog_operations.submit(request, None);
    let submitted_catalog = services
        .take_catalog_operation()
        .expect("deferred catalog adapter receives typed request");
    services.complete_catalog_operation(PlatformOperationCompletion {
        token: submitted_catalog.token,
        result: Ok(WorldCatalogResponse::WorldList {
            capabilities: WorldCatalogCapabilities::persistent_local(),
            worlds: Vec::new(),
        }),
    });
    let mut catalog = ClientCatalogController::new();
    let _ = catalog_operations.poll(&mut catalog);
    let catalog_ok = catalog_operations.pending_len() == 0;

    let late = services
        .lifecycle_mut()
        .begin_start(WebSceneSessionOperation::ConnectRemote {
            url: "ws://late.invalid".to_owned(),
        });
    services.teardown();
    let teardown_ok = services
        .lifecycle_mut()
        .complete(PlatformOperationCompletion {
            token: late.token,
            result: Ok(WebSceneSessionOperationResult::Started(
                ActiveSessionDescriptor::Remote {
                    endpoint: RemoteSessionEndpoint::new("ws://late.invalid"),
                },
            )),
        })
        == WebSceneSessionCompletionDisposition::Stale;

    bool_bit(clock_ok, SCENE_ADAPTER_CLOCK_BIT)
        | bool_bit(supersession_ok, SCENE_ADAPTER_SUPERSESSION_BIT)
        | bool_bit(reconnect_ok, SCENE_ADAPTER_RECONNECT_BIT)
        | bool_bit(catalog_ok, SCENE_ADAPTER_CATALOG_BIT)
        | bool_bit(teardown_ok, SCENE_ADAPTER_TEARDOWN_BIT)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn mclone_web_scene_adapter_contract_report() -> u32 {
    web_scene_adapter_contract_bits()
}

const fn bool_bit(value: bool, bit: u32) -> u32 {
    if value { bit } else { 0 }
}

fn packed_count(value: usize) -> u32 {
    u32::from(u8::try_from(value).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_runtime_smoke_moves_interest_through_protocol_codec() {
        let report = try_run_web_runtime_smoke().unwrap();

        assert_eq!(
            report,
            WebSmokeReport {
                ok: true,
                local_host: true,
                web_render_backend: true,
                center_chunk_loaded: true,
                transport_drained: true,
                moved_chunk_loaded: true,
                previous_chunk_unloaded: true,
                protocol_codec_roundtrip: true,
                command_count: 2,
                update_count: 13,
                loaded_chunk_count: 1,
            }
        );
    }

    #[test]
    fn web_scene_adapter_contract_rejects_stale_work() {
        assert_eq!(web_scene_adapter_contract_bits(), 0x1f);
    }

    #[test]
    fn web_runtime_hydrates_client_replica_from_step_updates() {
        let mut runtime = WebRuntime::local_integrated(SMOKE_SEED);

        assert_eq!(
            runtime
                .request_chunk_view(
                    SMOKE_INITIAL_CENTER,
                    SMOKE_RADIUS_CHUNKS,
                    SMOKE_RADIUS_CHUNKS
                )
                .unwrap(),
            WebRuntimeStepReport {
                command_count: 1,
                update_count: 9,
                loaded_chunk_count: 1,
                protocol_codec_roundtrip: true,
                transport_drained: true,
            }
        );
        assert_eq!(
            runtime.client().player_vitals(),
            mclone_protocol::PlayerVitals::full_health()
        );
        assert!(!runtime.client().player_is_dead());
        assert!(
            runtime
                .client()
                .chunk_snapshot(SMOKE_INITIAL_CENTER)
                .is_some()
        );

        assert_eq!(
            runtime
                .request_chunk_view(SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_RADIUS_CHUNKS)
                .unwrap(),
            WebRuntimeStepReport {
                command_count: 1,
                update_count: 4,
                loaded_chunk_count: 1,
                protocol_codec_roundtrip: true,
                transport_drained: true,
            }
        );
        assert!(
            runtime
                .client()
                .chunk_snapshot(SMOKE_MOVED_CENTER)
                .is_some()
        );
        assert!(
            runtime
                .client()
                .chunk_snapshot(SMOKE_INITIAL_CENTER)
                .is_none()
        );
    }

    #[test]
    fn web_runtime_roundtrips_gameplay_movement_commands() {
        let mut runtime = WebRuntime::local_integrated(SMOKE_SEED);
        runtime
            .request_chunk_view(
                SMOKE_INITIAL_CENTER,
                SMOKE_RADIUS_CHUNKS,
                SMOKE_RADIUS_CHUNKS,
            )
            .unwrap();

        let report = runtime
            .send_gameplay_command(ClientCommand::move_player(
                mclone_protocol::MovePlayerCommand::PosRot {
                    position: mclone_core::Vec3d::new(8.0, 104.0, 8.0),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    on_ground: false,
                },
            ))
            .unwrap();

        assert_eq!(report.command_count, 1);
        assert!(report.protocol_codec_roundtrip);
        assert!(report.transport_drained);
        assert!(runtime.command_count() >= 2);
    }

    #[test]
    fn web_runtime_seeds_initial_day_time_from_integrated_server() {
        let runtime = WebRuntime::local_integrated(SMOKE_SEED);

        assert_eq!(runtime.client().day_time(), 1000);
    }

    #[test]
    fn packed_smoke_report_has_stable_browser_layout() {
        assert_eq!(mclone_web_smoke(), 0x010d_02ff);
    }
}
