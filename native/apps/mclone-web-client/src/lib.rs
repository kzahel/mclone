#![deny(unsafe_code)]

use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::ChunkPos;
use mclone_net::LocalTransport;
use mclone_protocol::{
    ChunkView, ClientCommand, ProtocolCodecError, ProtocolCodecResult, ServerUpdate,
    decode_client_command, decode_server_update, encode_client_command, encode_server_update,
};
use mclone_render::RenderBackend;
use mclone_server::IntegratedServer;

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
    client: ClientRuntime,
    host: WebLoopbackHost,
    command_count: usize,
    update_count: usize,
    protocol_codec_roundtrip: bool,
    transport_drained: bool,
}

impl WebRuntime {
    pub fn local_integrated(seed: i64) -> Self {
        Self {
            client: ClientRuntime::local_integrated(),
            host: WebLoopbackHost::new(seed),
            command_count: 0,
            update_count: 0,
            protocol_codec_roundtrip: true,
            transport_drained: true,
        }
    }

    pub fn request_chunk_view(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
    ) -> ProtocolCodecResult<WebRuntimeStepReport> {
        let command = self.client.set_chunk_view(ChunkView {
            center,
            render_distance,
            chunk_tracking_radius,
        });
        let exchange = self.host.exchange(command)?;

        self.command_count += exchange.command_count;
        self.update_count += exchange.update_count;
        self.protocol_codec_roundtrip &= exchange.protocol_codec_roundtrip;
        self.transport_drained &= exchange.transport_drained;
        self.client.apply_updates(exchange.updates);

        Ok(WebRuntimeStepReport {
            command_count: exchange.command_count,
            update_count: exchange.update_count,
            loaded_chunk_count: self.client.loaded_chunk_count(),
            protocol_codec_roundtrip: exchange.protocol_codec_roundtrip,
            transport_drained: exchange.transport_drained,
        })
    }

    pub const fn client(&self) -> &ClientRuntime {
        &self.client
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
        && update_count == 3
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
                update_count: 3,
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
                update_count: 1,
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
    fn packed_smoke_report_has_stable_browser_layout() {
        assert_eq!(mclone_web_smoke(), 0x0103_02ff);
    }
}
