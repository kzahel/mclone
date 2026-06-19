#![forbid(unsafe_code)]

use std::collections::VecDeque;

use mclone_protocol::{ClientCommand, ServerUpdate};

#[cfg(not(target_arch = "wasm32"))]
pub use native_tcp::{
    NativeTransportError, NativeTransportResult, read_client_command_frame,
    read_client_command_frames, read_server_update_batch, request_server_updates,
    try_read_client_command_frame, write_client_command_frame, write_server_update_batch,
};

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

#[cfg(not(target_arch = "wasm32"))]
mod native_tcp {
    use std::error::Error;
    use std::fmt;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream, ToSocketAddrs};

    use mclone_protocol::{
        ClientCommand, ProtocolCodecError, ServerUpdate, decode_client_command,
        decode_server_update, encode_client_command, encode_server_update,
    };

    pub type NativeTransportResult<T> = Result<T, NativeTransportError>;

    const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

    #[derive(Debug)]
    pub enum NativeTransportError {
        Io(std::io::Error),
        Protocol(ProtocolCodecError),
        FrameTooLarge { field: &'static str, len: usize },
    }

    impl fmt::Display for NativeTransportError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Io(err) => write!(f, "native transport I/O failed: {err}"),
                Self::Protocol(err) => write!(f, "native transport protocol failed: {err}"),
                Self::FrameTooLarge { field, len } => {
                    write!(
                        f,
                        "{field} frame length {len} exceeds {MAX_FRAME_BYTES} bytes"
                    )
                }
            }
        }
    }

    impl Error for NativeTransportError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                Self::Io(err) => Some(err),
                Self::Protocol(err) => Some(err),
                Self::FrameTooLarge { .. } => None,
            }
        }
    }

    impl From<std::io::Error> for NativeTransportError {
        fn from(value: std::io::Error) -> Self {
            Self::Io(value)
        }
    }

    impl From<ProtocolCodecError> for NativeTransportError {
        fn from(value: ProtocolCodecError) -> Self {
            Self::Protocol(value)
        }
    }

    pub fn write_client_command_frame(
        writer: &mut impl Write,
        command: &ClientCommand,
    ) -> NativeTransportResult<()> {
        let payload = encode_client_command(command)?;
        write_frame(writer, "client command", &payload)
    }

    pub fn read_client_command_frame(
        reader: &mut impl Read,
    ) -> NativeTransportResult<ClientCommand> {
        let payload = read_frame(reader, "client command")?;
        Ok(decode_client_command(&payload)?)
    }

    pub fn try_read_client_command_frame(
        reader: &mut impl Read,
    ) -> NativeTransportResult<Option<ClientCommand>> {
        let Some(payload) = try_read_frame(reader, "client command")? else {
            return Ok(None);
        };
        Ok(Some(decode_client_command(&payload)?))
    }

    pub fn read_client_command_frames(
        reader: &mut impl Read,
    ) -> NativeTransportResult<Vec<ClientCommand>> {
        let mut commands = Vec::new();
        while let Some(command) = try_read_client_command_frame(reader)? {
            commands.push(command);
        }
        Ok(commands)
    }

    pub fn write_server_update_batch(
        writer: &mut impl Write,
        updates: &[ServerUpdate],
    ) -> NativeTransportResult<()> {
        write_u32(
            writer,
            checked_u32_len("server update batch", updates.len())?,
        )?;
        for update in updates {
            let payload = encode_server_update(update)?;
            write_frame(writer, "server update", &payload)?;
        }
        writer.flush()?;
        Ok(())
    }

    pub fn read_server_update_batch(
        reader: &mut impl Read,
    ) -> NativeTransportResult<Vec<ServerUpdate>> {
        let update_count = read_u32(reader)? as usize;
        let mut updates = Vec::with_capacity(update_count);
        for _ in 0..update_count {
            let payload = read_frame(reader, "server update")?;
            updates.push(decode_server_update(&payload)?);
        }
        Ok(updates)
    }

    pub fn request_server_updates(
        addr: impl ToSocketAddrs,
        command: &ClientCommand,
    ) -> NativeTransportResult<Vec<ServerUpdate>> {
        let mut stream = TcpStream::connect(addr)?;
        write_client_command_frame(&mut stream, command)?;
        stream.shutdown(Shutdown::Write)?;
        read_server_update_batch(&mut stream)
    }

    fn write_frame(
        writer: &mut impl Write,
        field: &'static str,
        payload: &[u8],
    ) -> NativeTransportResult<()> {
        write_u32(writer, checked_u32_len(field, payload.len())?)?;
        writer.write_all(payload)?;
        Ok(())
    }

    fn read_frame(reader: &mut impl Read, field: &'static str) -> NativeTransportResult<Vec<u8>> {
        let len = read_u32(reader)? as usize;
        if len > MAX_FRAME_BYTES {
            return Err(NativeTransportError::FrameTooLarge { field, len });
        }
        let mut payload = vec![0; len];
        reader.read_exact(&mut payload)?;
        Ok(payload)
    }

    fn try_read_frame(
        reader: &mut impl Read,
        field: &'static str,
    ) -> NativeTransportResult<Option<Vec<u8>>> {
        let Some(len) = try_read_u32(reader)? else {
            return Ok(None);
        };
        let len = len as usize;
        if len > MAX_FRAME_BYTES {
            return Err(NativeTransportError::FrameTooLarge { field, len });
        }
        let mut payload = vec![0; len];
        reader.read_exact(&mut payload)?;
        Ok(Some(payload))
    }

    fn checked_u32_len(field: &'static str, len: usize) -> NativeTransportResult<u32> {
        let len_u32 =
            u32::try_from(len).map_err(|_| NativeTransportError::FrameTooLarge { field, len })?;
        if len > MAX_FRAME_BYTES {
            return Err(NativeTransportError::FrameTooLarge { field, len });
        }
        Ok(len_u32)
    }

    fn write_u32(writer: &mut impl Write, value: u32) -> std::io::Result<()> {
        writer.write_all(&value.to_le_bytes())
    }

    fn read_u32(reader: &mut impl Read) -> std::io::Result<u32> {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn try_read_u32(reader: &mut impl Read) -> std::io::Result<Option<u32>> {
        let mut bytes = [0; 4];
        let mut read = 0;
        while read < bytes.len() {
            let n = reader.read(&mut bytes[read..])?;
            if n == 0 {
                if read == 0 {
                    return Ok(None);
                }
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
            }
            read += n;
        }
        Ok(Some(u32::from_le_bytes(bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_protocol::ChunkView;

    #[test]
    fn distinguishes_local_native_and_web_transports() {
        assert_ne!(TransportKind::Local, TransportKind::NativeSocket);
        assert_ne!(TransportKind::NativeSocket, TransportKind::WebSocket);
    }

    #[test]
    fn local_transport_queues_and_drains_protocol_messages() {
        let mut transport = LocalTransport::new();
        transport.send_client_command(ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 1,
            chunk_tracking_radius: 1,
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_command_frame_round_trips() {
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(-2, 4),
            render_distance: 2,
            chunk_tracking_radius: 2,
        });
        let mut bytes = Vec::new();

        write_client_command_frame(&mut bytes, &command).unwrap();

        assert_eq!(
            read_client_command_frame(&mut std::io::Cursor::new(bytes)).unwrap(),
            command
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_command_frames_read_until_clean_eof() {
        let commands = vec![
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(-2, 4),
                render_distance: 2,
                chunk_tracking_radius: 2,
            }),
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(5, -7),
                render_distance: 0,
                chunk_tracking_radius: 1,
            }),
        ];
        let mut bytes = Vec::new();
        for command in &commands {
            write_client_command_frame(&mut bytes, command).unwrap();
        }

        assert_eq!(
            read_client_command_frames(&mut std::io::Cursor::new(bytes)).unwrap(),
            commands
        );
        assert!(
            read_client_command_frames(&mut std::io::Cursor::new(Vec::new()))
                .unwrap()
                .is_empty()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_command_frame_optional_read_distinguishes_clean_eof() {
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(-2, 4),
            render_distance: 2,
            chunk_tracking_radius: 2,
        });
        let mut bytes = Vec::new();
        write_client_command_frame(&mut bytes, &command).unwrap();
        let mut cursor = std::io::Cursor::new(bytes);

        assert_eq!(
            try_read_client_command_frame(&mut cursor).unwrap(),
            Some(command)
        );
        assert_eq!(try_read_client_command_frame(&mut cursor).unwrap(), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_update_batch_round_trips() {
        let updates = vec![
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(1, 2),
            },
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(-3, 4),
            },
        ];
        let mut bytes = Vec::new();

        write_server_update_batch(&mut bytes, &updates).unwrap();

        assert_eq!(
            read_server_update_batch(&mut std::io::Cursor::new(bytes)).unwrap(),
            updates
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_loopback_requests_updates() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let server_command = command.clone();
        let expected_updates = vec![ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(9, -9),
        }];
        let server_updates = expected_updates.clone();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                server_command
            );
            write_server_update_batch(&mut stream, &server_updates).unwrap();
        });

        let updates = request_server_updates(addr, &command).unwrap();
        server.join().unwrap();

        assert_eq!(updates, expected_updates);
    }
}
