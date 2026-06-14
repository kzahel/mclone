#![forbid(unsafe_code)]

use std::collections::VecDeque;

use mclone_protocol::{ClientCommand, ServerUpdate};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportKind {
    Local,
    NativeSocket,
    WebSocket,
}

#[derive(Debug, Default)]
pub struct LocalTransport {
    client_to_server: VecDeque<ClientCommand>,
    server_to_client: VecDeque<ServerUpdate>,
}

impl LocalTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send_client_command(&mut self, command: ClientCommand) {
        self.client_to_server.push_back(command);
    }

    pub fn send_server_update(&mut self, update: ServerUpdate) {
        self.server_to_client.push_back(update);
    }

    pub fn drain_client_commands(&mut self) -> Vec<ClientCommand> {
        self.client_to_server.drain(..).collect()
    }

    pub fn drain_server_updates(&mut self) -> Vec<ServerUpdate> {
        self.server_to_client.drain(..).collect()
    }

    pub fn pending_client_command_count(&self) -> usize {
        self.client_to_server.len()
    }

    pub fn pending_server_update_count(&self) -> usize {
        self.server_to_client.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_protocol::ChunkInterest;

    #[test]
    fn distinguishes_local_native_and_web_transports() {
        assert_ne!(TransportKind::Local, TransportKind::NativeSocket);
        assert_ne!(TransportKind::NativeSocket, TransportKind::WebSocket);
    }

    #[test]
    fn local_transport_queues_and_drains_protocol_messages() {
        let mut transport = LocalTransport::new();
        transport.send_client_command(ClientCommand::SetChunkInterest(ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 1,
        }));
        transport.send_server_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(4, -2),
        });

        assert_eq!(transport.pending_client_command_count(), 1);
        assert_eq!(transport.pending_server_update_count(), 1);
        assert_eq!(transport.drain_client_commands().len(), 1);
        assert_eq!(transport.drain_server_updates().len(), 1);
        assert_eq!(transport.pending_client_command_count(), 0);
        assert_eq!(transport.pending_server_update_count(), 0);
    }
}
