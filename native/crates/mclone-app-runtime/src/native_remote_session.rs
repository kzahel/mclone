use anyhow::{Context, Result};
use mclone_net::{NativeClientIoSession, NativeServerUpdateBatch};
use mclone_protocol::{ClientCommand, ClientEphemeralMessage, ClientIdentity};

use crate::host_mode::{
    RemoteDedicatedServerSession, RemoteServerUpdate, RemoteServerUpdateBatch,
    RemoteUpdateQueueMetrics, SingleViewHostOptions,
};
use crate::native_service_assembly::NativeSessionServices;
use crate::render_asset_data::TexturedMeshAssets;
use crate::session::RemoteSessionEndpoint;

pub fn connect_native_remote_session_runtime(
    endpoint: RemoteSessionEndpoint,
    options: SingleViewHostOptions,
    mesh_assets: TexturedMeshAssets,
    host_label: &'static str,
) -> Result<NativeSessionServices<NativeRemoteServerSession>> {
    connect_native_remote_session_runtime_with_identity(
        endpoint,
        options,
        mesh_assets,
        host_label,
        ClientIdentity::test_default(),
    )
}

pub fn connect_native_remote_session_runtime_with_identity(
    endpoint: RemoteSessionEndpoint,
    options: SingleViewHostOptions,
    mesh_assets: TexturedMeshAssets,
    host_label: &'static str,
    identity: ClientIdentity,
) -> Result<NativeSessionServices<NativeRemoteServerSession>> {
    let session = NativeRemoteServerSession::connect_with_identity(
        endpoint.address.as_str(),
        host_label,
        identity,
    )?;
    NativeSessionServices::remote_dedicated_with_mesh_assets(
        endpoint,
        options,
        session,
        mesh_assets,
    )
}

/// Native TCP-backed remote session shared by desktop and XR hosts.
///
/// `host_label` is diagnostic context only (for example `"desktop"` or
/// `"Android XR"`); transport and update-batch semantics stay identical.
#[derive(Debug)]
pub struct NativeRemoteServerSession {
    addr: String,
    host_label: &'static str,
    identity: ClientIdentity,
    session: NativeClientIoSession,
}

impl NativeRemoteServerSession {
    pub fn connect(addr: impl Into<String>, host_label: &'static str) -> Result<Self> {
        Self::connect_with_identity(addr, host_label, ClientIdentity::test_default())
    }

    pub fn connect_with_identity(
        addr: impl Into<String>,
        host_label: &'static str,
        identity: ClientIdentity,
    ) -> Result<Self> {
        let addr = addr.into();
        let session = NativeClientIoSession::connect_with_identity(addr.as_str(), &identity)
            .with_context(|| format!("failed to connect to {host_label} remote server {addr}"))?;
        Ok(Self {
            addr,
            host_label,
            identity,
            session,
        })
    }

    pub fn reconnect(&mut self) -> Result<()> {
        self.session =
            NativeClientIoSession::connect_with_identity(self.addr.as_str(), &self.identity)
                .with_context(|| {
                    format!(
                        "failed to reconnect to {} remote server {}",
                        self.host_label, self.addr
                    )
                })?;
        Ok(())
    }

    pub fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
        self.session.send_command_only(command).with_context(|| {
            format!(
                "failed to send command to {} remote server {}",
                self.host_label, self.addr
            )
        })
    }

    pub fn send_ephemeral(&mut self, message: ClientEphemeralMessage) -> Result<()> {
        self.session.send_ephemeral(message).with_context(|| {
            format!(
                "failed to send ephemeral message to {} remote server {}",
                self.host_label, self.addr
            )
        })
    }

    pub fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>> {
        self.session
            .try_drain_update_batch()
            .map(|batch| batch.map(remote_batch_from_native))
            .with_context(|| {
                format!(
                    "failed to poll updates from {} remote server {}",
                    self.host_label, self.addr
                )
            })
    }

    pub fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics {
        let diagnostics = self.session.diagnostics();
        RemoteUpdateQueueMetrics {
            frame_depth: diagnostics.inbound_update_batches,
            update_depth: diagnostics.inbound_update_depth,
            update_bytes: diagnostics.inbound_update_bytes,
        }
    }
}

impl RemoteDedicatedServerSession for NativeRemoteServerSession {
    fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
        NativeRemoteServerSession::send_command_only(self, command)
    }

    fn send_ephemeral(&mut self, message: ClientEphemeralMessage) -> Result<()> {
        NativeRemoteServerSession::send_ephemeral(self, message)
    }

    fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>> {
        NativeRemoteServerSession::try_drain_update_batch(self)
    }

    fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics {
        NativeRemoteServerSession::pending_update_metrics(self)
    }

    fn reconnect(&mut self) -> Result<()> {
        NativeRemoteServerSession::reconnect(self)
    }
}

fn remote_batch_from_native(batch: NativeServerUpdateBatch) -> RemoteServerUpdateBatch {
    let queued_age = batch.queued_age();
    RemoteServerUpdateBatch {
        inbound_frame_sequence: Some(batch.inbound_frame_sequence),
        producer_read_ms: batch.producer_read_ms,
        producer_decode_ms: batch.producer_decode_ms,
        updates: batch
            .updates
            .into_iter()
            .map(|update| RemoteServerUpdate {
                update: update.update,
                encoded_len: Some(update.encoded_len),
                queued_age,
                inbound_frame_sequence: Some(update.inbound_frame_sequence),
                producer_read_ms: update.producer_read_ms,
                producer_decode_ms: update.producer_decode_ms,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_protocol::{ChunkView, ClientDisconnectReason, ServerUpdate};

    fn time_update(day_time: u64) -> ServerUpdate {
        ServerUpdate::TimeUpdate {
            game_time: day_time.saturating_add(100),
            day_time,
            daylight_cycle_running: true,
            calendar_policy: Default::default(),
        }
    }

    #[test]
    fn remote_server_session_reuses_one_native_tcp_connection() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let first_command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let second_command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let first_server_command = first_command.clone();
        let second_server_command = second_command.clone();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                first_server_command
            );
            mclone_net::write_server_update_batch(&mut stream, &[time_update(10)]).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                second_server_command
            );
            mclone_net::write_server_update_batch(&mut stream, &[time_update(20)]).unwrap();
            assert_eq!(
                mclone_net::try_read_client_command_frame(&mut stream).unwrap(),
                Some(ClientCommand::Disconnect(ClientDisconnectReason::Quit))
            );
        });

        {
            let mut session = NativeRemoteServerSession::connect(addr.to_string(), "test").unwrap();
            session.send_command_only(first_command).unwrap();
            assert_eq!(
                wait_for_update_batch(&mut session).into_updates(),
                vec![time_update(10)]
            );
            session.send_command_only(second_command).unwrap();
            assert_eq!(
                wait_for_update_batch(&mut session).into_updates(),
                vec![time_update(20)]
            );
        }
        server.join().unwrap();
    }

    #[test]
    fn remote_server_session_reconnects_after_dropped_connection() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let first_command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let second_command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let first_server_command = first_command.clone();
        let second_server_command = second_command.clone();

        let server = std::thread::spawn(move || {
            let (mut first_stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut first_stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut first_stream).unwrap(),
                first_server_command
            );
            drop(first_stream);

            let (mut second_stream, _) = listener.accept().unwrap();
            mclone_net::complete_server_handshake(&mut second_stream).unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut second_stream).unwrap(),
                second_server_command
            );
            mclone_net::write_server_update_batch(&mut second_stream, &[time_update(30)]).unwrap();
        });

        {
            let mut session = NativeRemoteServerSession::connect(addr.to_string(), "test").unwrap();
            session.send_command_only(first_command).unwrap();
            wait_for_disconnect(&mut session);
            session.reconnect().unwrap();
            session.send_command_only(second_command).unwrap();
            assert_eq!(
                wait_for_update_batch(&mut session).into_updates(),
                vec![time_update(30)]
            );
        }
        server.join().unwrap();
    }

    fn wait_for_update_batch(session: &mut NativeRemoteServerSession) -> RemoteServerUpdateBatch {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(batch) = session.try_drain_update_batch().unwrap() {
                return batch;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for update batch"
            );
            std::thread::yield_now();
        }
    }

    fn wait_for_disconnect(session: &mut NativeRemoteServerSession) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if session.try_drain_update_batch().is_err() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for disconnect"
            );
            std::thread::yield_now();
        }
    }
}
