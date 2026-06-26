use anyhow::{Context, Result};
use mclone_app_runtime::host_mode::RemoteDedicatedServerSession;
use mclone_net::NativeClientSession;
use mclone_protocol::{ClientCommand, ServerUpdate};

#[derive(Debug)]
pub(crate) struct RemoteServerSession {
    addr: String,
    session: NativeClientSession,
}

impl RemoteServerSession {
    pub(crate) fn connect(addr: impl Into<String>) -> Result<Self> {
        let addr = addr.into();
        let session = NativeClientSession::connect(addr.as_str())
            .with_context(|| format!("failed to connect to remote server {addr}"))?;
        Ok(Self { addr, session })
    }

    pub(crate) fn reconnect(&mut self) -> Result<()> {
        self.session = NativeClientSession::connect(self.addr.as_str())
            .with_context(|| format!("failed to reconnect to remote server {}", self.addr))?;
        Ok(())
    }

    pub(crate) fn send_command(&mut self, command: ClientCommand) -> Result<Vec<ServerUpdate>> {
        self.session.send_command(&command).with_context(|| {
            format!(
                "failed to exchange command with remote server {}",
                self.addr
            )
        })
    }
}

impl RemoteDedicatedServerSession for RemoteServerSession {
    fn send_command(&mut self, command: ClientCommand) -> Result<Vec<ServerUpdate>> {
        RemoteServerSession::send_command(self, command)
    }

    fn reconnect(&mut self) -> Result<()> {
        RemoteServerSession::reconnect(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_protocol::{ChunkView, ServerUpdate};

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
            let mut session = RemoteServerSession::connect(addr.to_string()).unwrap();
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
            let mut session = RemoteServerSession::connect(addr.to_string()).unwrap();
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
