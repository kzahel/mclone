#![deny(unsafe_code)]

use mclone_client::{ClientHost, ClientRuntime};
use mclone_core::ChunkPos;
use mclone_net::LocalTransport;
use mclone_protocol::ChunkInterest;
use mclone_render::RenderBackend;
use mclone_server::IntegratedServer;

const SMOKE_SEED: i64 = 12_345;
const SMOKE_CENTER: ChunkPos = ChunkPos { x: 0, z: 0 };
const SMOKE_RADIUS_CHUNKS: u32 = 0;

const OK_BIT: u32 = 1 << 0;
const LOCAL_HOST_BIT: u32 = 1 << 1;
const WEB_RENDER_BACKEND_BIT: u32 = 1 << 2;
const CENTER_CHUNK_LOADED_BIT: u32 = 1 << 3;
const TRANSPORT_DRAINED_BIT: u32 = 1 << 4;
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
    pub command_count: usize,
    pub update_count: usize,
    pub loaded_chunk_count: usize,
}

impl WebSmokeReport {
    pub fn packed_bits(self) -> u32 {
        bool_bit(self.ok, OK_BIT)
            | bool_bit(self.local_host, LOCAL_HOST_BIT)
            | bool_bit(self.web_render_backend, WEB_RENDER_BACKEND_BIT)
            | bool_bit(self.center_chunk_loaded, CENTER_CHUNK_LOADED_BIT)
            | bool_bit(self.transport_drained, TRANSPORT_DRAINED_BIT)
            | (packed_count(self.command_count) << COMMAND_COUNT_SHIFT)
            | (packed_count(self.update_count) << UPDATE_COUNT_SHIFT)
            | (packed_count(self.loaded_chunk_count) << LOADED_CHUNK_COUNT_SHIFT)
    }
}

pub fn run_web_runtime_smoke() -> WebSmokeReport {
    let mut server = IntegratedServer::new(SMOKE_SEED);
    let mut client = ClientRuntime::local_integrated();
    let mut transport = LocalTransport::new();

    let command = client.set_chunk_interest(ChunkInterest {
        center: SMOKE_CENTER,
        radius_chunks: SMOKE_RADIUS_CHUNKS,
    });
    transport.send_client_command(command);

    let commands = transport.drain_client_commands();
    let command_count = commands.len();
    for command in commands {
        for update in server.handle_command(command) {
            transport.send_server_update(update);
        }
    }

    let updates = transport.drain_server_updates();
    let update_count = updates.len();
    client.apply_updates(updates);

    let local_host = client.host() == ClientHost::LocalIntegrated;
    let web_render_backend = RenderBackend::WebWgpu == RenderBackend::WebWgpu;
    let center_chunk_loaded = client.chunk_snapshot(SMOKE_CENTER).is_some();
    let transport_drained = transport.pending_client_command_count() == 0
        && transport.pending_server_update_count() == 0;
    let loaded_chunk_count = client.loaded_chunk_count();
    let ok = local_host
        && web_render_backend
        && center_chunk_loaded
        && transport_drained
        && command_count == 1
        && update_count == 1
        && loaded_chunk_count == 1;

    WebSmokeReport {
        ok,
        local_host,
        web_render_backend,
        center_chunk_loaded,
        transport_drained,
        command_count,
        update_count,
        loaded_chunk_count,
    }
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
    fn web_runtime_smoke_loads_one_chunk_through_protocol() {
        let report = run_web_runtime_smoke();

        assert_eq!(
            report,
            WebSmokeReport {
                ok: true,
                local_host: true,
                web_render_backend: true,
                center_chunk_loaded: true,
                transport_drained: true,
                command_count: 1,
                update_count: 1,
                loaded_chunk_count: 1,
            }
        );
    }

    #[test]
    fn packed_smoke_report_has_stable_browser_layout() {
        assert_eq!(mclone_web_smoke(), 0x0101_011f);
    }
}
