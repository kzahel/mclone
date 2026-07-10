use anyhow::{Context, Result};
use mclone_net::{NativeClientIoSession, NativeServerUpdateBatch};
use mclone_protocol::{ClientCommand, ServerUpdate};

use crate::host_mode::{
    RemoteCommandUpdate, RemoteCommandUpdateBatch, RemoteDedicatedServerSession,
    SingleViewHostOptions,
};
use crate::native_session_runtime::NativeSessionRuntime;
use crate::render_asset_data::TexturedMeshAssets;
use crate::session::RemoteSessionEndpoint;

pub fn connect_native_remote_session_runtime(
    endpoint: RemoteSessionEndpoint,
    options: SingleViewHostOptions,
    mesh_assets: TexturedMeshAssets,
    host_label: &'static str,
) -> Result<NativeSessionRuntime<NativeRemoteServerSession>> {
    let session = NativeRemoteServerSession::connect(endpoint.address.as_str(), host_label)?;
    NativeSessionRuntime::remote_dedicated_with_mesh_assets(endpoint, options, session, mesh_assets)
}

/// Native TCP-backed remote session shared by desktop and XR hosts.
///
/// `host_label` is diagnostic context only (for example `"desktop"` or
/// `"Android XR"`); transport and update-batch semantics stay identical.
#[derive(Debug)]
pub struct NativeRemoteServerSession {
    addr: String,
    host_label: &'static str,
    session: NativeClientIoSession,
}

impl NativeRemoteServerSession {
    pub fn connect(addr: impl Into<String>, host_label: &'static str) -> Result<Self> {
        let addr = addr.into();
        let session = NativeClientIoSession::connect(addr.as_str())
            .with_context(|| format!("failed to connect to {host_label} remote server {addr}"))?;
        Ok(Self {
            addr,
            host_label,
            session,
        })
    }

    pub fn reconnect(&mut self) -> Result<()> {
        self.session = NativeClientIoSession::connect(self.addr.as_str()).with_context(|| {
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

    pub fn drain_command_updates(&mut self) -> Result<Vec<ServerUpdate>> {
        self.session.drain_command_updates().with_context(|| {
            format!(
                "failed to drain updates from {} remote server {}",
                self.host_label, self.addr
            )
        })
    }

    pub fn try_drain_command_updates(&mut self) -> Result<Option<Vec<ServerUpdate>>> {
        self.session.try_drain_command_updates().with_context(|| {
            format!(
                "failed to poll updates from {} remote server {}",
                self.host_label, self.addr
            )
        })
    }

    pub fn drain_command_update_batch(&mut self) -> Result<RemoteCommandUpdateBatch> {
        self.session
            .drain_update_batch()
            .map(remote_batch_from_native)
            .with_context(|| {
                format!(
                    "failed to drain updates from {} remote server {}",
                    self.host_label, self.addr
                )
            })
    }

    pub fn try_drain_command_update_batch(&mut self) -> Result<Option<RemoteCommandUpdateBatch>> {
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
}

impl RemoteDedicatedServerSession for NativeRemoteServerSession {
    fn send_command_only(&mut self, command: ClientCommand) -> Result<()> {
        NativeRemoteServerSession::send_command_only(self, command)
    }

    fn drain_command_updates(&mut self) -> Result<Vec<ServerUpdate>> {
        NativeRemoteServerSession::drain_command_updates(self)
    }

    fn try_drain_command_updates(&mut self) -> Result<Option<Vec<ServerUpdate>>> {
        NativeRemoteServerSession::try_drain_command_updates(self)
    }

    fn drain_command_update_batch(&mut self) -> Result<RemoteCommandUpdateBatch> {
        NativeRemoteServerSession::drain_command_update_batch(self)
    }

    fn try_drain_command_update_batch(&mut self) -> Result<Option<RemoteCommandUpdateBatch>> {
        NativeRemoteServerSession::try_drain_command_update_batch(self)
    }

    fn reconnect(&mut self) -> Result<()> {
        NativeRemoteServerSession::reconnect(self)
    }
}

fn remote_batch_from_native(batch: NativeServerUpdateBatch) -> RemoteCommandUpdateBatch {
    let queued_age = batch.queued_age();
    RemoteCommandUpdateBatch {
        response_sequence: Some(batch.response_sequence),
        producer_read_ms: batch.producer_read_ms,
        producer_decode_ms: batch.producer_decode_ms,
        updates: batch
            .updates
            .into_iter()
            .map(|update| RemoteCommandUpdate {
                update: update.update,
                encoded_len: Some(update.encoded_len),
                queued_age,
                response_sequence: Some(update.response_sequence),
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
    use mclone_protocol::ChunkView;

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
            mclone_net::write_server_update_batch(
                &mut stream,
                &[ServerUpdate::TimeUpdate { day_time: 10 }],
            )
            .unwrap();
            assert_eq!(
                mclone_net::read_client_command_frame(&mut stream).unwrap(),
                second_server_command
            );
            mclone_net::write_server_update_batch(
                &mut stream,
                &[ServerUpdate::TimeUpdate { day_time: 20 }],
            )
            .unwrap();
            assert!(
                mclone_net::try_read_client_command_frame(&mut stream)
                    .unwrap()
                    .is_none()
            );
        });

        {
            let mut session = NativeRemoteServerSession::connect(addr.to_string(), "test").unwrap();
            assert_eq!(
                session.send_command(first_command).unwrap(),
                vec![ServerUpdate::TimeUpdate { day_time: 10 }]
            );
            assert_eq!(
                session.send_command(second_command).unwrap(),
                vec![ServerUpdate::TimeUpdate { day_time: 20 }]
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
            mclone_net::write_server_update_batch(
                &mut second_stream,
                &[ServerUpdate::TimeUpdate { day_time: 30 }],
            )
            .unwrap();
        });

        {
            let mut session = NativeRemoteServerSession::connect(addr.to_string(), "test").unwrap();
            assert!(session.send_command(first_command).is_err());
            session.reconnect().unwrap();
            assert_eq!(
                session.send_command(second_command).unwrap(),
                vec![ServerUpdate::TimeUpdate { day_time: 30 }]
            );
        }
        server.join().unwrap();
    }
}
