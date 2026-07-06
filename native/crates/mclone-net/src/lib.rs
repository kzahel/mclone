#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use mclone_protocol::{
    ClientCommand, PROTOCOL_VERSION, ProtocolCodecError, ServerUpdate, decode_client_command,
    decode_server_update, encode_client_command, encode_server_update,
};

#[cfg(not(target_arch = "wasm32"))]
pub use native_tcp::{
    NativeClientSession, NativeTransportError, NativeTransportResult, complete_client_handshake,
    complete_client_handshake_with_version, complete_server_handshake, read_client_command_frame,
    read_client_command_frames, read_server_update_batch, request_server_updates,
    try_read_client_command_frame, write_client_command_frame, write_server_update_batch,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportKind {
    Local,
    NativeSocket,
    WebSocket,
}

const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const WEBSOCKET_HANDSHAKE_MAGIC: &[u8] = b"MCLONE_WS";
const WEBSOCKET_HANDSHAKE_ACCEPT: u8 = 1;
const WEBSOCKET_HANDSHAKE_REJECT: u8 = 2;

#[derive(Debug)]
pub enum WebSocketTransportError {
    Protocol(ProtocolCodecError),
    InvalidHandshake(&'static str),
    ProtocolVersionMismatch { expected: u32, received: u32 },
    MessageTooLarge { field: &'static str, len: usize },
    UnexpectedEof(&'static str),
    TrailingBytes { field: &'static str, bytes: usize },
}

impl fmt::Display for WebSocketTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(err) => write!(f, "websocket transport protocol failed: {err}"),
            Self::InvalidHandshake(message) => {
                write!(f, "websocket transport handshake failed: {message}")
            }
            Self::ProtocolVersionMismatch { expected, received } => write!(
                f,
                "websocket transport protocol version mismatch: expected {expected}, received {received}"
            ),
            Self::MessageTooLarge { field, len } => write!(
                f,
                "{field} websocket message length {len} exceeds {MAX_WEBSOCKET_MESSAGE_BYTES} bytes"
            ),
            Self::UnexpectedEof(field) => {
                write!(f, "{field} websocket message ended before expected data")
            }
            Self::TrailingBytes { field, bytes } => {
                write!(f, "{field} websocket message had {bytes} trailing bytes")
            }
        }
    }
}

impl Error for WebSocketTransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(err) => Some(err),
            Self::InvalidHandshake(_)
            | Self::ProtocolVersionMismatch { .. }
            | Self::MessageTooLarge { .. }
            | Self::UnexpectedEof(_)
            | Self::TrailingBytes { .. } => None,
        }
    }
}

impl From<ProtocolCodecError> for WebSocketTransportError {
    fn from(value: ProtocolCodecError) -> Self {
        Self::Protocol(value)
    }
}

pub type WebSocketTransportResult<T> = Result<T, WebSocketTransportError>;

pub fn encode_websocket_client_handshake(
    protocol_version: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    let mut payload = Vec::with_capacity(WEBSOCKET_HANDSHAKE_MAGIC.len() + 4);
    payload.extend_from_slice(WEBSOCKET_HANDSHAKE_MAGIC);
    payload.extend_from_slice(&protocol_version.to_le_bytes());
    checked_websocket_message_len("client handshake", payload.len())?;
    Ok(payload)
}

pub fn encode_current_websocket_client_handshake() -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_client_handshake(PROTOCOL_VERSION)
}

pub fn decode_websocket_client_handshake(payload: &[u8]) -> WebSocketTransportResult<u32> {
    checked_websocket_message_len("client handshake", payload.len())?;
    if payload.len() != WEBSOCKET_HANDSHAKE_MAGIC.len() + 4 {
        return Err(WebSocketTransportError::InvalidHandshake(
            "client handshake had invalid length",
        ));
    }
    if !payload.starts_with(WEBSOCKET_HANDSHAKE_MAGIC) {
        return Err(WebSocketTransportError::InvalidHandshake(
            "client handshake had invalid magic",
        ));
    }
    let version_offset = WEBSOCKET_HANDSHAKE_MAGIC.len();
    Ok(u32::from_le_bytes(
        payload[version_offset..version_offset + 4]
            .try_into()
            .expect("websocket handshake version length checked"),
    ))
}

pub fn encode_websocket_server_handshake_accept(
    protocol_version: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_server_handshake(
        WEBSOCKET_HANDSHAKE_ACCEPT,
        protocol_version,
        protocol_version,
    )
}

pub fn encode_websocket_server_handshake_reject(
    expected: u32,
    received: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_server_handshake(WEBSOCKET_HANDSHAKE_REJECT, expected, received)
}

pub fn decode_websocket_server_handshake(
    payload: &[u8],
    client_version: u32,
) -> WebSocketTransportResult<()> {
    checked_websocket_message_len("server handshake", payload.len())?;
    if payload.len() != WEBSOCKET_HANDSHAKE_MAGIC.len() + 9 {
        return Err(WebSocketTransportError::InvalidHandshake(
            "server handshake had invalid length",
        ));
    }
    if !payload.starts_with(WEBSOCKET_HANDSHAKE_MAGIC) {
        return Err(WebSocketTransportError::InvalidHandshake(
            "server handshake had invalid magic",
        ));
    }

    let status_offset = WEBSOCKET_HANDSHAKE_MAGIC.len();
    let expected_offset = status_offset + 1;
    let received_offset = expected_offset + 4;
    let status = payload[status_offset];
    let expected = u32::from_le_bytes(
        payload[expected_offset..expected_offset + 4]
            .try_into()
            .expect("server handshake expected version length checked"),
    );
    let received = u32::from_le_bytes(
        payload[received_offset..received_offset + 4]
            .try_into()
            .expect("server handshake received version length checked"),
    );

    match status {
        WEBSOCKET_HANDSHAKE_ACCEPT => {
            if expected != client_version || received != client_version {
                return Err(WebSocketTransportError::ProtocolVersionMismatch {
                    expected,
                    received: client_version,
                });
            }
            Ok(())
        }
        WEBSOCKET_HANDSHAKE_REJECT => {
            Err(WebSocketTransportError::ProtocolVersionMismatch { expected, received })
        }
        _ => Err(WebSocketTransportError::InvalidHandshake(
            "server handshake had unknown status",
        )),
    }
}

pub fn encode_websocket_client_command(
    command: &ClientCommand,
) -> WebSocketTransportResult<Vec<u8>> {
    let payload = encode_client_command(command)?;
    checked_websocket_message_len("client command", payload.len())?;
    Ok(payload)
}

pub fn decode_websocket_client_command(payload: &[u8]) -> WebSocketTransportResult<ClientCommand> {
    checked_websocket_message_len("client command", payload.len())?;
    Ok(decode_client_command(payload)?)
}

pub fn encode_websocket_server_update_batch(
    updates: &[ServerUpdate],
) -> WebSocketTransportResult<Vec<u8>> {
    let mut payload = Vec::new();
    write_websocket_u32(
        &mut payload,
        checked_websocket_u32_len("server update batch", updates.len())?,
    );
    for update in updates {
        let frame = encode_server_update(update)?;
        checked_websocket_message_len("server update", frame.len())?;
        write_websocket_u32(
            &mut payload,
            checked_websocket_u32_len("server update", frame.len())?,
        );
        payload.extend_from_slice(&frame);
    }
    checked_websocket_message_len("server update batch", payload.len())?;
    Ok(payload)
}

pub fn decode_websocket_server_update_batch(
    payload: &[u8],
) -> WebSocketTransportResult<Vec<ServerUpdate>> {
    checked_websocket_message_len("server update batch", payload.len())?;
    let mut cursor = 0usize;
    let update_count = read_websocket_u32(payload, &mut cursor, "server update batch")? as usize;
    let mut updates = Vec::with_capacity(update_count);
    for _ in 0..update_count {
        let frame_len = read_websocket_u32(payload, &mut cursor, "server update")? as usize;
        if frame_len > MAX_WEBSOCKET_MESSAGE_BYTES {
            return Err(WebSocketTransportError::MessageTooLarge {
                field: "server update",
                len: frame_len,
            });
        }
        let end =
            cursor
                .checked_add(frame_len)
                .ok_or(WebSocketTransportError::MessageTooLarge {
                    field: "server update",
                    len: frame_len,
                })?;
        let Some(frame) = payload.get(cursor..end) else {
            return Err(WebSocketTransportError::UnexpectedEof("server update"));
        };
        updates.push(decode_server_update(frame)?);
        cursor = end;
    }
    if cursor != payload.len() {
        return Err(WebSocketTransportError::TrailingBytes {
            field: "server update batch",
            bytes: payload.len() - cursor,
        });
    }
    Ok(updates)
}

fn encode_websocket_server_handshake(
    status: u8,
    expected: u32,
    received: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    let mut payload = Vec::with_capacity(WEBSOCKET_HANDSHAKE_MAGIC.len() + 9);
    payload.extend_from_slice(WEBSOCKET_HANDSHAKE_MAGIC);
    payload.push(status);
    payload.extend_from_slice(&expected.to_le_bytes());
    payload.extend_from_slice(&received.to_le_bytes());
    checked_websocket_message_len("server handshake", payload.len())?;
    Ok(payload)
}

fn checked_websocket_u32_len(field: &'static str, len: usize) -> WebSocketTransportResult<u32> {
    checked_websocket_message_len(field, len)?;
    u32::try_from(len).map_err(|_| WebSocketTransportError::MessageTooLarge { field, len })
}

fn checked_websocket_message_len(field: &'static str, len: usize) -> WebSocketTransportResult<()> {
    if len > MAX_WEBSOCKET_MESSAGE_BYTES {
        return Err(WebSocketTransportError::MessageTooLarge { field, len });
    }
    Ok(())
}

fn write_websocket_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn read_websocket_u32(
    payload: &[u8],
    cursor: &mut usize,
    field: &'static str,
) -> WebSocketTransportResult<u32> {
    let end = cursor
        .checked_add(4)
        .ok_or(WebSocketTransportError::UnexpectedEof(field))?;
    let Some(bytes) = payload.get(*cursor..end) else {
        return Err(WebSocketTransportError::UnexpectedEof(field));
    };
    *cursor = end;
    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .expect("websocket u32 field length checked"),
    ))
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
        ClientCommand, PROTOCOL_VERSION, ProtocolCodecError, ServerUpdate, decode_client_command,
        decode_server_update, encode_client_command, encode_server_update,
    };

    pub type NativeTransportResult<T> = Result<T, NativeTransportError>;

    const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
    const HANDSHAKE_MAGIC: &[u8] = b"MCLONE_NATIVE_TCP";
    const SERVER_HANDSHAKE_ACCEPT: u8 = 1;
    const SERVER_HANDSHAKE_REJECT: u8 = 2;

    #[derive(Debug)]
    pub enum NativeTransportError {
        Io(std::io::Error),
        Protocol(ProtocolCodecError),
        InvalidHandshake(&'static str),
        ProtocolVersionMismatch { expected: u32, received: u32 },
        FrameTooLarge { field: &'static str, len: usize },
    }

    impl fmt::Display for NativeTransportError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Io(err) => write!(f, "native transport I/O failed: {err}"),
                Self::Protocol(err) => write!(f, "native transport protocol failed: {err}"),
                Self::InvalidHandshake(message) => {
                    write!(f, "native transport handshake failed: {message}")
                }
                Self::ProtocolVersionMismatch { expected, received } => write!(
                    f,
                    "native transport protocol version mismatch: expected {expected}, received {received}"
                ),
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
                Self::InvalidHandshake(_) | Self::ProtocolVersionMismatch { .. } => None,
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

    #[derive(Debug)]
    pub struct NativeClientSession {
        stream: TcpStream,
    }

    impl NativeClientSession {
        pub fn connect(addr: impl ToSocketAddrs) -> NativeTransportResult<Self> {
            let mut stream = TcpStream::connect(addr)?;
            complete_client_handshake(&mut stream)?;
            Ok(Self { stream })
        }

        pub fn send_command_only(&mut self, command: &ClientCommand) -> NativeTransportResult<()> {
            write_client_command_frame(&mut self.stream, command)?;
            self.stream.flush()?;
            Ok(())
        }

        pub fn drain_command_updates(&mut self) -> NativeTransportResult<Vec<ServerUpdate>> {
            read_server_update_batch(&mut self.stream)
        }

        pub fn try_drain_command_updates(
            &mut self,
        ) -> NativeTransportResult<Option<Vec<ServerUpdate>>> {
            if !self.server_update_batch_available()? {
                return Ok(None);
            }
            self.drain_command_updates().map(Some)
        }

        pub fn send_command(
            &mut self,
            command: &ClientCommand,
        ) -> NativeTransportResult<Vec<ServerUpdate>> {
            self.send_command_only(command)?;
            self.drain_command_updates()
        }

        fn server_update_batch_available(&self) -> NativeTransportResult<bool> {
            self.stream.set_nonblocking(true)?;
            let mut byte = [0u8; 1];
            let peek_result = loop {
                match self.stream.peek(&mut byte) {
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                    result => break result,
                }
            };
            self.stream.set_nonblocking(false)?;

            match peek_result {
                Ok(0) => Ok(true),
                Ok(_) => Ok(true),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
                Err(err) => Err(err.into()),
            }
        }
    }

    pub fn complete_client_handshake<T: Read + Write>(stream: &mut T) -> NativeTransportResult<()> {
        complete_client_handshake_with_version(stream, PROTOCOL_VERSION)
    }

    pub fn complete_client_handshake_with_version<T: Read + Write>(
        stream: &mut T,
        protocol_version: u32,
    ) -> NativeTransportResult<()> {
        write_client_handshake(stream, protocol_version)?;
        stream.flush()?;
        read_server_handshake(stream, protocol_version)
    }

    pub fn complete_server_handshake<T: Read + Write>(stream: &mut T) -> NativeTransportResult<()> {
        let received = read_client_handshake(stream)?;
        if received != PROTOCOL_VERSION {
            write_server_handshake_reject(stream, PROTOCOL_VERSION, received)?;
            return Err(NativeTransportError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received,
            });
        }
        write_server_handshake_accept(stream, PROTOCOL_VERSION)?;
        Ok(())
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
        let mut session = NativeClientSession::connect(addr)?;
        let updates = session.send_command(command)?;
        session.stream.shutdown(Shutdown::Write)?;
        Ok(updates)
    }

    fn write_client_handshake(
        writer: &mut impl Write,
        protocol_version: u32,
    ) -> NativeTransportResult<()> {
        let mut payload = Vec::with_capacity(HANDSHAKE_MAGIC.len() + 4);
        payload.extend_from_slice(HANDSHAKE_MAGIC);
        payload.extend_from_slice(&protocol_version.to_le_bytes());
        write_frame(writer, "client handshake", &payload)
    }

    fn read_client_handshake(reader: &mut impl Read) -> NativeTransportResult<u32> {
        let payload = read_frame(reader, "client handshake")?;
        if payload.len() != HANDSHAKE_MAGIC.len() + 4 {
            return Err(NativeTransportError::InvalidHandshake(
                "client handshake had invalid length",
            ));
        }
        if !payload.starts_with(HANDSHAKE_MAGIC) {
            return Err(NativeTransportError::InvalidHandshake(
                "client handshake had invalid magic",
            ));
        }

        let version_offset = HANDSHAKE_MAGIC.len();
        Ok(u32::from_le_bytes(
            payload[version_offset..version_offset + 4]
                .try_into()
                .expect("handshake version length checked"),
        ))
    }

    fn write_server_handshake_accept(
        writer: &mut impl Write,
        protocol_version: u32,
    ) -> NativeTransportResult<()> {
        write_server_handshake(
            writer,
            SERVER_HANDSHAKE_ACCEPT,
            protocol_version,
            protocol_version,
        )
    }

    fn write_server_handshake_reject(
        writer: &mut impl Write,
        expected: u32,
        received: u32,
    ) -> NativeTransportResult<()> {
        write_server_handshake(writer, SERVER_HANDSHAKE_REJECT, expected, received)
    }

    fn write_server_handshake(
        writer: &mut impl Write,
        status: u8,
        expected: u32,
        received: u32,
    ) -> NativeTransportResult<()> {
        let mut payload = Vec::with_capacity(HANDSHAKE_MAGIC.len() + 9);
        payload.extend_from_slice(HANDSHAKE_MAGIC);
        payload.push(status);
        payload.extend_from_slice(&expected.to_le_bytes());
        payload.extend_from_slice(&received.to_le_bytes());
        write_frame(writer, "server handshake", &payload)?;
        writer.flush()?;
        Ok(())
    }

    fn read_server_handshake(
        reader: &mut impl Read,
        client_version: u32,
    ) -> NativeTransportResult<()> {
        let payload = read_frame(reader, "server handshake")?;
        if payload.len() != HANDSHAKE_MAGIC.len() + 9 {
            return Err(NativeTransportError::InvalidHandshake(
                "server handshake had invalid length",
            ));
        }
        if !payload.starts_with(HANDSHAKE_MAGIC) {
            return Err(NativeTransportError::InvalidHandshake(
                "server handshake had invalid magic",
            ));
        }

        let status_offset = HANDSHAKE_MAGIC.len();
        let expected_offset = status_offset + 1;
        let received_offset = expected_offset + 4;
        let status = payload[status_offset];
        let expected = u32::from_le_bytes(
            payload[expected_offset..expected_offset + 4]
                .try_into()
                .expect("server handshake expected version length checked"),
        );
        let received = u32::from_le_bytes(
            payload[received_offset..received_offset + 4]
                .try_into()
                .expect("server handshake received version length checked"),
        );

        match status {
            SERVER_HANDSHAKE_ACCEPT => {
                if expected != client_version || received != client_version {
                    return Err(NativeTransportError::ProtocolVersionMismatch {
                        expected,
                        received: client_version,
                    });
                }
                Ok(())
            }
            SERVER_HANDSHAKE_REJECT => {
                Err(NativeTransportError::ProtocolVersionMismatch { expected, received })
            }
            _ => Err(NativeTransportError::InvalidHandshake(
                "server handshake had unknown status",
            )),
        }
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
    use mclone_protocol::{ChunkView, PROTOCOL_VERSION};

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

    #[test]
    fn websocket_handshake_accepts_current_protocol_version() {
        let client = encode_current_websocket_client_handshake().unwrap();
        assert_eq!(
            decode_websocket_client_handshake(&client).unwrap(),
            PROTOCOL_VERSION
        );

        let server = encode_websocket_server_handshake_accept(PROTOCOL_VERSION).unwrap();
        decode_websocket_server_handshake(&server, PROTOCOL_VERSION).unwrap();
    }

    #[test]
    fn websocket_handshake_reports_protocol_mismatch() {
        let mismatched = PROTOCOL_VERSION + 1;
        let server =
            encode_websocket_server_handshake_reject(PROTOCOL_VERSION, mismatched).unwrap();

        assert!(matches!(
            decode_websocket_server_handshake(&server, mismatched),
            Err(WebSocketTransportError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received
            }) if received == mismatched
        ));
    }

    #[test]
    fn websocket_command_and_update_batch_round_trip_protocol_payloads() {
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(-2, 4),
            render_distance: 2,
            chunk_tracking_radius: 2,
        });
        let command_payload = encode_websocket_client_command(&command).unwrap();
        assert_eq!(
            decode_websocket_client_command(&command_payload).unwrap(),
            command
        );

        let updates = vec![
            ServerUpdate::TimeUpdate { day_time: 99 },
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(5, -7),
            },
        ];
        let update_payload = encode_websocket_server_update_batch(&updates).unwrap();
        assert_eq!(
            decode_websocket_server_update_batch(&update_payload).unwrap(),
            updates
        );
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
            complete_server_handshake(&mut stream).unwrap();
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_session_reuses_one_connection() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let first_command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let second_command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, -1),
            render_distance: 1,
            chunk_tracking_radius: 1,
        });
        let first_server_command = first_command.clone();
        let second_server_command = second_command.clone();
        let first_updates = vec![ServerUpdate::TimeUpdate { day_time: 1 }];
        let second_updates = vec![ServerUpdate::TimeUpdate { day_time: 2 }];
        let first_server_updates = first_updates.clone();
        let second_server_updates = second_updates.clone();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                first_server_command
            );
            write_server_update_batch(&mut stream, &first_server_updates).unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                second_server_command
            );
            write_server_update_batch(&mut stream, &second_server_updates).unwrap();
            assert!(
                try_read_client_command_frame(&mut stream)
                    .unwrap()
                    .is_none()
            );
        });

        {
            let mut session = NativeClientSession::connect(addr).unwrap();
            assert_eq!(session.send_command(&first_command).unwrap(), first_updates);
            assert_eq!(
                session.send_command(&second_command).unwrap(),
                second_updates
            );
        }
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_try_drain_does_not_wait_for_delayed_response() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let server_command = command.clone();
        let expected_updates = vec![ServerUpdate::TimeUpdate { day_time: 1234 }];
        let server_updates = expected_updates.clone();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                server_command
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
            write_server_update_batch(&mut stream, &server_updates).unwrap();
        });

        let mut session = NativeClientSession::connect(addr).unwrap();
        session.send_command_only(&command).unwrap();

        let poll_start = std::time::Instant::now();
        assert_eq!(session.try_drain_command_updates().unwrap(), None);
        assert!(
            poll_start.elapsed() < std::time::Duration::from_millis(100),
            "try_drain_command_updates blocked on a delayed server response"
        );

        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            session.try_drain_command_updates().unwrap(),
            Some(expected_updates)
        );
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_handshake_rejects_protocol_version_mismatch() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mismatched_version = PROTOCOL_VERSION + 1;

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let err = complete_server_handshake(&mut stream).unwrap_err();
            assert!(matches!(
                err,
                NativeTransportError::ProtocolVersionMismatch {
                    expected: PROTOCOL_VERSION,
                    received
                } if received == mismatched_version
            ));
        });

        let mut stream = std::net::TcpStream::connect(addr).unwrap();
        let err =
            complete_client_handshake_with_version(&mut stream, mismatched_version).unwrap_err();
        server.join().unwrap();

        assert!(matches!(
            err,
            NativeTransportError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received
            } if received == mismatched_version
        ));
        assert!(err.to_string().contains("protocol version mismatch"));
    }
}
