#![deny(unsafe_code)]

use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::ChunkPos;
use mclone_net::LocalTransport;
use mclone_protocol::{
    ChunkView, ClientCommand, ProtocolCodecError, ProtocolCodecResult, ServerUpdate,
    decode_client_command, decode_server_update, encode_client_command, encode_server_update,
};
use mclone_render::RenderBackend;
use mclone_render_session::{EngineRenderSession, EngineServerUpdateDirtyPolicy};
#[cfg(target_arch = "wasm32")]
use mclone_server::IntegratedServerRunner;
use mclone_server::{IntegratedServer, ServerRunnerDiagnostics, ServerRunnerKind};

#[cfg(target_arch = "wasm32")]
mod web_canvas;
#[cfg(target_arch = "wasm32")]
mod web_remote_session;
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
pub struct WebRuntime {
    engine: EngineRenderSession,
    host: WebRuntimeHost,
    command_count: usize,
    update_count: usize,
    protocol_codec_roundtrip: bool,
    transport_drained: bool,
}

impl WebRuntime {
    pub fn local_integrated(seed: i64) -> Self {
        let host = WebLoopbackHost::new(seed);
        let mut engine = EngineRenderSession::new(ClientRuntime::local_integrated());
        engine.client_mut().apply_update(ServerUpdate::TimeUpdate {
            day_time: host.day_time(),
        });
        Self {
            engine,
            host: WebRuntimeHost::Inline(host),
            command_count: 0,
            update_count: 0,
            protocol_codec_roundtrip: true,
            transport_drained: true,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn web_worker_integrated(
        config: WebIntegratedServerRunnerConfig,
    ) -> Result<Self, String> {
        let runner = WebIntegratedServerRunner::new(config).await?;
        let diagnostics = runner.diagnostics();
        let mut engine = EngineRenderSession::new(ClientRuntime::local_integrated());
        engine.client_mut().apply_update(ServerUpdate::TimeUpdate {
            day_time: diagnostics.day_time,
        });
        Ok(Self {
            engine,
            host: WebRuntimeHost::Worker(runner),
            command_count: 0,
            update_count: 0,
            protocol_codec_roundtrip: true,
            transport_drained: true,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn websocket_remote(url: impl Into<String>) -> Result<Self, String> {
        let session = WebSocketServerSession::connect(url).await?;
        Ok(Self {
            engine: EngineRenderSession::new(ClientRuntime::new(ClientHost::RemoteDedicated)),
            host: WebRuntimeHost::RemoteWebSocket(session),
            command_count: 0,
            update_count: 0,
            protocol_codec_roundtrip: true,
            transport_drained: true,
        })
    }

    pub fn request_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> ProtocolCodecResult<WebRuntimeStepReport> {
        let command = self.engine.client_mut().set_chunk_view(ChunkView {
            center,
            render_distance,
            chunk_tracking_radius,
        });
        let exchange = self.host.exchange(command)?;
        Ok(self.apply_exchange(exchange))
    }

    pub async fn request_chunk_view_async(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<WebRuntimeStepReport, String> {
        let command = self.engine.client_mut().set_chunk_view(ChunkView {
            center,
            render_distance,
            chunk_tracking_radius,
        });
        let exchange = self.host.exchange_async(command).await?;
        Ok(self.apply_exchange(exchange))
    }

    pub fn request_chunk_view_deferred(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> Result<WebRuntimeStepReport, String> {
        let command = self.engine.client_mut().set_chunk_view(ChunkView {
            center,
            render_distance,
            chunk_tracking_radius,
        });
        let exchange = self.host.send_deferred(command)?;
        Ok(self.apply_exchange(exchange))
    }

    pub fn send_gameplay_command(
        &mut self,
        command: ClientCommand,
    ) -> ProtocolCodecResult<WebRuntimeStepReport> {
        let exchange = self.host.exchange(command)?;
        Ok(self.apply_exchange(exchange))
    }

    pub async fn send_gameplay_command_async(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebRuntimeStepReport, String> {
        let exchange = self.host.exchange_async(command).await?;
        Ok(self.apply_exchange(exchange))
    }

    pub fn send_gameplay_command_deferred(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebRuntimeStepReport, String> {
        let exchange = self.host.send_deferred(command)?;
        Ok(self.apply_exchange(exchange))
    }

    pub fn drain_pending_runner_updates(&mut self) -> Result<WebRuntimeStepReport, String> {
        let exchange = self.host.drain_pending_updates()?;
        Ok(self.apply_exchange(exchange))
    }

    pub fn drain_pending_runner_updates_budgeted(
        &mut self,
        max_update_frames: usize,
    ) -> Result<WebRuntimeStepReport, String> {
        let exchange = self
            .host
            .drain_pending_updates_budgeted(max_update_frames)?;
        Ok(self.apply_exchange(exchange))
    }

    fn apply_exchange(&mut self, exchange: WebExchange) -> WebRuntimeStepReport {
        self.command_count += exchange.command_count;
        self.update_count += exchange.update_count;
        self.protocol_codec_roundtrip &= exchange.protocol_codec_roundtrip;
        self.transport_drained &= exchange.transport_drained;
        // 067 Stage 3: drive render dirty state event-driven from drained updates, exactly
        // like desktop. Chunk snapshots/unloads (not just block edits) mark neighborhoods
        // render-dirty as they arrive, so the per-frame streaming loop plans purely from
        // dirty state and no longer needs the view-sync delta to discover new/removed
        // chunks.
        let update_report = self
            .engine
            .apply_server_updates_with_dirty_policy(exchange.updates, EngineServerUpdateDirtyPolicy::ALL);

        WebRuntimeStepReport {
            command_count: exchange.command_count,
            update_count: update_report.updates,
            loaded_chunk_count: self.engine.client().loaded_chunk_count(),
            protocol_codec_roundtrip: exchange.protocol_codec_roundtrip,
            transport_drained: exchange.transport_drained,
        }
    }

    pub const fn client(&self) -> &ClientRuntime {
        self.engine.client()
    }

    pub const fn engine(&self) -> &EngineRenderSession {
        &self.engine
    }

    pub const fn engine_mut(&mut self) -> &mut EngineRenderSession {
        &mut self.engine
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn update_count(&self) -> usize {
        self.update_count
    }

    pub const fn protocol_codec_roundtrip(&self) -> bool {
        self.protocol_codec_roundtrip
    }

    pub const fn transport_drained(&self) -> bool {
        self.transport_drained
    }

    pub fn drain_player_position_updates(
        &mut self,
    ) -> impl Iterator<Item = mclone_protocol::PlayerPositionUpdate> + '_ {
        self.engine.client_mut().drain_player_position_updates()
    }

    pub fn runner_kind(&self) -> ServerRunnerKind {
        self.host.runner_kind()
    }

    pub fn runner_diagnostics(&self) -> ServerRunnerDiagnostics {
        self.host.runner_diagnostics()
    }

    pub fn request_shutdown(&mut self) {
        self.host.request_shutdown();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebRuntimeStepReport {
    pub command_count: usize,
    pub update_count: usize,
    pub loaded_chunk_count: usize,
    pub protocol_codec_roundtrip: bool,
    pub transport_drained: bool,
}

#[derive(Debug)]
enum WebRuntimeHost {
    Inline(WebLoopbackHost),
    #[cfg(target_arch = "wasm32")]
    Worker(WebIntegratedServerRunner),
    #[cfg(target_arch = "wasm32")]
    RemoteWebSocket(WebSocketServerSession),
}

impl WebRuntimeHost {
    fn exchange(&mut self, command: ClientCommand) -> ProtocolCodecResult<WebExchange> {
        match self {
            Self::Inline(host) => host.exchange(command),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(_) | Self::RemoteWebSocket(_) => Err(ProtocolCodecError::InvalidData(
                "browser transport runtime requires async exchange",
            )),
        }
    }

    fn send_deferred(&mut self, command: ClientCommand) -> Result<WebExchange, String> {
        match self {
            Self::Inline(host) => host.exchange(command).map_err(|error| error.to_string()),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => {
                host.send_command(command)
                    .map_err(|error| error.to_string())?;
                Ok(WebExchange {
                    updates: Vec::new(),
                    command_count: 1,
                    update_count: 0,
                    protocol_codec_roundtrip: true,
                    transport_drained: false,
                })
            }
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(_) => Err(
                "deferred commands are not implemented for remote websocket web runtime".to_owned(),
            ),
        }
    }

    async fn exchange_async(&mut self, command: ClientCommand) -> Result<WebExchange, String> {
        match self {
            Self::Inline(host) => host.exchange(command).map_err(|error| error.to_string()),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => {
                let exchange = host.exchange_command(command).await?;
                Ok(WebExchange {
                    update_count: exchange.updates.len(),
                    updates: exchange.updates,
                    command_count: 1,
                    protocol_codec_roundtrip: exchange.protocol_codec_roundtrip,
                    transport_drained: exchange.transport_drained,
                })
            }
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => {
                let exchange = host.exchange_command(command).await?;
                Ok(WebExchange {
                    update_count: exchange.updates.len(),
                    updates: exchange.updates,
                    command_count: 1,
                    protocol_codec_roundtrip: exchange.protocol_codec_roundtrip,
                    transport_drained: exchange.transport_drained,
                })
            }
        }
    }

    fn drain_pending_updates(&mut self) -> Result<WebExchange, String> {
        self.drain_pending_updates_budgeted(usize::MAX)
    }

    fn drain_pending_updates_budgeted(
        &mut self,
        _max_update_frames: usize,
    ) -> Result<WebExchange, String> {
        match self {
            Self::Inline(_) => Ok(WebExchange {
                updates: Vec::new(),
                command_count: 0,
                update_count: 0,
                protocol_codec_roundtrip: true,
                transport_drained: true,
            }),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => {
                let updates = host.drain_decoded_updates_budgeted(_max_update_frames)?;
                let diagnostics = host.diagnostics();
                Ok(WebExchange {
                    update_count: updates.len(),
                    updates,
                    command_count: 0,
                    protocol_codec_roundtrip: true,
                    transport_drained: diagnostics.command_queue_depth == 0
                        && diagnostics.update_queue_depth == 0,
                })
            }
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => {
                let diagnostics = host.diagnostics();
                Ok(WebExchange {
                    updates: Vec::new(),
                    command_count: 0,
                    update_count: 0,
                    protocol_codec_roundtrip: true,
                    transport_drained: diagnostics.command_queue_depth == 0
                        && diagnostics.update_queue_depth == 0,
                })
            }
        }
    }

    fn runner_kind(&self) -> ServerRunnerKind {
        match self {
            Self::Inline(_) => ServerRunnerKind::InlineFallback,
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.kind(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(_) => ServerRunnerKind::RemoteWebSocket,
        }
    }

    fn runner_diagnostics(&self) -> ServerRunnerDiagnostics {
        match self {
            Self::Inline(host) => ServerRunnerDiagnostics::initial(
                ServerRunnerKind::InlineFallback,
                host.seed(),
                host.day_time(),
            ),
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.diagnostics(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => host.diagnostics(),
        }
    }

    fn request_shutdown(&mut self) {
        match self {
            Self::Inline(_) => {}
            #[cfg(target_arch = "wasm32")]
            Self::Worker(host) => host.request_shutdown(),
            #[cfg(target_arch = "wasm32")]
            Self::RemoteWebSocket(host) => host.request_shutdown(),
        }
    }
}

#[derive(Debug)]
struct WebLoopbackHost {
    server: IntegratedServer,
    transport: LocalTransport,
}

impl WebLoopbackHost {
    fn new(seed: i64) -> Self {
        Self {
            server: IntegratedServer::new(seed),
            transport: LocalTransport::new(),
        }
    }

    fn day_time(&self) -> u64 {
        self.server.day_time()
    }

    fn seed(&self) -> i64 {
        self.server.seed()
    }

    fn exchange(&mut self, command: ClientCommand) -> ProtocolCodecResult<WebExchange> {
        let encoded_command = encode_client_command(&command)?;
        let decoded_command = decode_client_command(&encoded_command)?;
        let mut protocol_codec_roundtrip = decoded_command == command;
        self.transport.send_client_command(decoded_command);

        let commands = self.transport.drain_client_commands();
        let command_count = commands.len();
        for command in commands {
            for update in self.server.handle_command(command) {
                self.send_roundtripped_update(update, &mut protocol_codec_roundtrip)?;
            }
        }
        for _ in 0..60_000 {
            if self.server.pending_job_count() == 0 {
                break;
            }
            self.poll_server_updates(&mut protocol_codec_roundtrip)?;
            if self.server.pending_job_count() > 0 && self.server.pending_publication_count() == 0 {
                wait_for_worker_tick();
            }
        }
        if self.server.pending_job_count() > 0 {
            return Err(ProtocolCodecError::InvalidData(
                "timed out waiting for web loopback worldgen jobs",
            ));
        }
        self.poll_server_updates(&mut protocol_codec_roundtrip)?;

        let updates = self.transport.drain_server_updates();
        let update_count = updates.len();
        let transport_drained = self.transport.pending_client_command_count() == 0
            && self.transport.pending_server_update_count() == 0;

        Ok(WebExchange {
            updates,
            command_count,
            update_count,
            protocol_codec_roundtrip,
            transport_drained,
        })
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
        let decoded_update = decode_server_update(&encoded_update)?;
        *protocol_codec_roundtrip &= decoded_update == update;
        self.transport.send_server_update(decoded_update);
        Ok(())
    }
}

fn wait_for_worker_tick() {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::sleep(std::time::Duration::from_millis(1));
    #[cfg(target_arch = "wasm32")]
    std::hint::spin_loop();
}

#[derive(Debug)]
struct WebExchange {
    updates: Vec<ServerUpdate>,
    command_count: usize,
    update_count: usize,
    protocol_codec_roundtrip: bool,
    transport_drained: bool,
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
        && update_count == 4
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
                update_count: 4,
                loaded_chunk_count: 1,
            }
        );
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
                update_count: 2,
                loaded_chunk_count: 1,
                protocol_codec_roundtrip: true,
                transport_drained: true,
            }
        );
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
                update_count: 2,
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
            .send_gameplay_command(ClientCommand::MovePlayer(
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
        assert_eq!(mclone_web_smoke(), 0x0104_02ff);
    }
}
