#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
mod native_udp;

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use mclone_protocol::{
    ClientCommand, ClientIdentity, PROTOCOL_VERSION, PlayerProfileId, ProtocolCodecError,
    ServerUpdate, SessionCapabilities, decode_client_command, decode_server_update,
    encode_client_command, encode_server_update, validate_client_identity,
};

#[cfg(not(target_arch = "wasm32"))]
pub use native_tcp::{
    AcceptedNativeServerHandshake, NATIVE_CLIENT_COMMAND_QUEUE_CAPACITY,
    NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY, NativeClientIoDiagnostics, NativeClientIoSession,
    NativeServerUpdateBatch, NativeServerUpdateEnvelope, NativeTransportError,
    NativeTransportResult, complete_client_handshake, complete_client_handshake_with_identity,
    complete_client_handshake_with_identity_and_capabilities,
    complete_client_handshake_with_identity_capabilities_and_udp_offer,
    complete_client_handshake_with_version, complete_server_handshake,
    complete_server_handshake_with_capabilities,
    complete_server_handshake_with_capabilities_and_udp_offer, read_client_command_frame,
    read_client_command_frames, read_server_update_batch, try_read_client_command_frame,
    write_client_command_frame, write_server_update_batch,
};
#[cfg(not(target_arch = "wasm32"))]
pub use native_udp::{
    MAX_NATIVE_UDP_PACKET_BYTES, NATIVE_UDP_ATTACHMENT_TIMEOUT, NativeUdpAttachmentToken,
    NativeUdpAttachmentTokenGenerator, NativeUdpClient, NativeUdpDiagnostics, NativeUdpError,
    NativeUdpOffer, NativeUdpReceivedServerMessage, NativeUdpResult, NativeUdpServer,
    NativeUdpServerEvent, NativeUdpServerHandle,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientHandshake {
    pub protocol_version: u32,
    pub capabilities: SessionCapabilities,
    pub identity: ClientIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedClientHandshake {
    pub identity: ClientIdentity,
    pub capabilities: SessionCapabilities,
}

pub fn encode_websocket_client_handshake(
    protocol_version: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_client_handshake_with_identity(
        protocol_version,
        &ClientIdentity::test_default(),
    )
}

pub fn encode_websocket_client_handshake_with_identity(
    protocol_version: u32,
    identity: &ClientIdentity,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_client_handshake_with_identity_and_capabilities(
        protocol_version,
        identity,
        SessionCapabilities::DEVELOPMENT_DEFAULT,
    )
}

pub fn encode_websocket_client_handshake_with_identity_and_capabilities(
    protocol_version: u32,
    identity: &ClientIdentity,
    capabilities: SessionCapabilities,
) -> WebSocketTransportResult<Vec<u8>> {
    validate_client_identity(identity)?;
    let name = identity.display_name.as_bytes();
    let mut payload = Vec::with_capacity(WEBSOCKET_HANDSHAKE_MAGIC.len() + 29 + name.len());
    payload.extend_from_slice(WEBSOCKET_HANDSHAKE_MAGIC);
    payload.extend_from_slice(&protocol_version.to_le_bytes());
    payload.extend_from_slice(&capabilities.bits().to_le_bytes());
    payload.extend_from_slice(&identity.profile_id.bytes());
    payload.push(name.len() as u8);
    payload.extend_from_slice(name);
    checked_websocket_message_len("client handshake", payload.len())?;
    Ok(payload)
}

pub fn encode_current_websocket_client_handshake() -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_client_handshake(PROTOCOL_VERSION)
}

pub fn decode_websocket_client_handshake(
    payload: &[u8],
) -> WebSocketTransportResult<ClientHandshake> {
    checked_websocket_message_len("client handshake", payload.len())?;
    let fixed_len = WEBSOCKET_HANDSHAKE_MAGIC.len() + 29;
    if payload.len() < fixed_len {
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
    let protocol_version = u32::from_le_bytes(
        payload[version_offset..version_offset + 4]
            .try_into()
            .expect("websocket handshake version length checked"),
    );
    let capabilities_offset = version_offset + 4;
    let capabilities = SessionCapabilities::from_bits_retain(u64::from_le_bytes(
        payload[capabilities_offset..capabilities_offset + 8]
            .try_into()
            .expect("websocket handshake capabilities length checked"),
    ));
    let profile_offset = capabilities_offset + 8;
    let profile_id = PlayerProfileId::new(
        payload[profile_offset..profile_offset + 16]
            .try_into()
            .expect("websocket handshake profile UUID length checked"),
    );
    let name_len_offset = profile_offset + 16;
    let name_len = usize::from(payload[name_len_offset]);
    if payload.len() != fixed_len + name_len {
        return Err(WebSocketTransportError::InvalidHandshake(
            "client handshake display name length did not match payload",
        ));
    }
    let display_name = std::str::from_utf8(&payload[name_len_offset + 1..])
        .map_err(|_| {
            WebSocketTransportError::InvalidHandshake("client handshake display name was not UTF-8")
        })?
        .to_owned();
    let identity = ClientIdentity::new(profile_id, display_name)?;
    Ok(ClientHandshake {
        protocol_version,
        capabilities,
        identity,
    })
}

pub fn encode_websocket_server_handshake_accept(
    protocol_version: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_server_handshake_accept_with_capabilities(
        protocol_version,
        SessionCapabilities::DEVELOPMENT_DEFAULT,
    )
}

pub fn encode_websocket_server_handshake_accept_with_capabilities(
    protocol_version: u32,
    capabilities: SessionCapabilities,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_server_handshake(
        WEBSOCKET_HANDSHAKE_ACCEPT,
        protocol_version,
        protocol_version,
        capabilities,
    )
}

pub fn encode_websocket_server_handshake_reject(
    expected: u32,
    received: u32,
) -> WebSocketTransportResult<Vec<u8>> {
    encode_websocket_server_handshake(
        WEBSOCKET_HANDSHAKE_REJECT,
        expected,
        received,
        SessionCapabilities::NONE,
    )
}

pub fn decode_websocket_server_handshake(
    payload: &[u8],
    client_version: u32,
) -> WebSocketTransportResult<SessionCapabilities> {
    checked_websocket_message_len("server handshake", payload.len())?;
    if payload.len() != WEBSOCKET_HANDSHAKE_MAGIC.len() + 17 {
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
    let capabilities_offset = received_offset + 4;
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
    let capabilities = SessionCapabilities::from_bits_retain(u64::from_le_bytes(
        payload[capabilities_offset..capabilities_offset + 8]
            .try_into()
            .expect("server handshake capabilities length checked"),
    ))
    .known();

    match status {
        WEBSOCKET_HANDSHAKE_ACCEPT => {
            if expected != client_version || received != client_version {
                return Err(WebSocketTransportError::ProtocolVersionMismatch {
                    expected,
                    received: client_version,
                });
            }
            Ok(capabilities)
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
    capabilities: SessionCapabilities,
) -> WebSocketTransportResult<Vec<u8>> {
    let mut payload = Vec::with_capacity(WEBSOCKET_HANDSHAKE_MAGIC.len() + 17);
    payload.extend_from_slice(WEBSOCKET_HANDSHAKE_MAGIC);
    payload.push(status);
    payload.extend_from_slice(&expected.to_le_bytes());
    payload.extend_from_slice(&received.to_le_bytes());
    payload.extend_from_slice(&capabilities.bits().to_le_bytes());
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
    use std::collections::VecDeque;
    use std::error::Error;
    use std::fmt;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream, ToSocketAddrs};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use crate::{NativeUdpClient, NativeUdpDiagnostics, NativeUdpOffer};
    use mclone_protocol::{
        ClientCommand, ClientEphemeralMessage, ClientIdentity, DisconnectReason, PROTOCOL_VERSION,
        PlayerProfileId, ProtocolCodecError, ServerUpdate, SessionCapabilities,
        decode_client_command, decode_server_update, encode_client_command, encode_server_update,
        validate_client_identity,
    };

    pub type NativeTransportResult<T> = Result<T, NativeTransportError>;

    const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
    pub const NATIVE_CLIENT_COMMAND_QUEUE_CAPACITY: usize = 256;
    pub const NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY: usize = 256;
    const NATIVE_CLIENT_DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(4);
    const HANDSHAKE_MAGIC: &[u8] = b"MCLONE_NATIVE_TCP";
    const SERVER_HANDSHAKE_ACCEPT: u8 = 1;
    const SERVER_HANDSHAKE_REJECT: u8 = 2;
    const SERVER_HANDSHAKE_BASE_BYTES: usize = HANDSHAKE_MAGIC.len() + 17;
    const SERVER_HANDSHAKE_UDP_OFFER_BYTES: usize = 19;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct AcceptedNativeServerHandshake {
        pub capabilities: SessionCapabilities,
        pub udp_offer: Option<NativeUdpOffer>,
    }

    #[derive(Debug)]
    pub enum NativeTransportError {
        Io(std::io::Error),
        Protocol(ProtocolCodecError),
        InvalidHandshake(&'static str),
        ProtocolVersionMismatch { expected: u32, received: u32 },
        FrameTooLarge { field: &'static str, len: usize },
        QueueFull { lane: &'static str, capacity: usize },
        ActorStopped { message: String },
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
                Self::QueueFull { lane, capacity } => {
                    write!(
                        f,
                        "native transport {lane} queue reached capacity {capacity}"
                    )
                }
                Self::ActorStopped { message } => {
                    write!(f, "native client I/O actor stopped: {message}")
                }
            }
        }
    }

    impl Error for NativeTransportError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                Self::Io(err) => Some(err),
                Self::Protocol(err) => Some(err),
                Self::InvalidHandshake(_)
                | Self::ProtocolVersionMismatch { .. }
                | Self::FrameTooLarge { .. }
                | Self::QueueFull { .. }
                | Self::ActorStopped { .. } => None,
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

    #[derive(Clone, Debug, PartialEq)]
    pub struct NativeServerUpdateEnvelope {
        pub update: ServerUpdate,
        pub encoded_len: usize,
        /// Monotonic sequence of the inbound publication frame carrying this
        /// update.
        pub inbound_frame_sequence: u64,
        pub producer_read_ms: f64,
        pub producer_decode_ms: f64,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct NativeServerUpdateBatch {
        pub updates: Vec<NativeServerUpdateEnvelope>,
        /// Monotonic inbound publication-frame sequence.
        pub inbound_frame_sequence: u64,
        pub producer_read_ms: f64,
        pub producer_decode_ms: f64,
        queued_at: Instant,
    }

    impl NativeServerUpdateBatch {
        pub fn queued_age(&self) -> Duration {
            self.queued_at.elapsed()
        }

        pub fn update_count(&self) -> usize {
            self.updates.len()
        }

        pub fn encoded_bytes(&self) -> usize {
            self.updates.iter().map(|update| update.encoded_len).sum()
        }

        pub fn into_updates(self) -> Vec<ServerUpdate> {
            self.updates
                .into_iter()
                .map(|envelope| envelope.update)
                .collect()
        }
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct NativeClientIoDiagnostics {
        pub outbound_command_depth: usize,
        pub inbound_update_batches: usize,
        pub inbound_update_depth: usize,
        pub inbound_update_bytes: usize,
        pub oldest_inbound_update_age_ms: f64,
        pub inbound_frame_sequence: u64,
        pub inbound_frames_received: u64,
        pub inbound_updates_received: u64,
        pub inbound_update_bytes_received: u64,
        pub inbound_updates_drained: u64,
        pub inbound_update_bytes_drained: u64,
        pub inbound_overflow_disconnects: u64,
        pub total_read_ms: f64,
        pub max_read_ms: f64,
        pub total_decode_ms: f64,
        pub max_decode_ms: f64,
        pub disconnected: bool,
        pub last_error: Option<String>,
    }

    #[derive(Debug)]
    struct NativeClientIoSharedDiagnostics {
        outbound_command_depth: AtomicUsize,
        inbound_update_batches: AtomicUsize,
        inbound_update_depth: AtomicUsize,
        inbound_update_bytes: AtomicUsize,
        inbound_queued_at: Mutex<VecDeque<Instant>>,
        inbound_frame_sequence: AtomicU64,
        inbound_frames_received: AtomicU64,
        inbound_updates_received: AtomicU64,
        inbound_update_bytes_received: AtomicU64,
        inbound_updates_drained: AtomicU64,
        inbound_update_bytes_drained: AtomicU64,
        inbound_overflow_disconnects: AtomicU64,
        total_read_us: AtomicU64,
        max_read_us: AtomicU64,
        total_decode_us: AtomicU64,
        max_decode_us: AtomicU64,
        disconnected: AtomicBool,
        last_error: Mutex<Option<String>>,
    }

    impl NativeClientIoSharedDiagnostics {
        fn new() -> Self {
            Self {
                outbound_command_depth: AtomicUsize::new(0),
                inbound_update_batches: AtomicUsize::new(0),
                inbound_update_depth: AtomicUsize::new(0),
                inbound_update_bytes: AtomicUsize::new(0),
                inbound_queued_at: Mutex::new(VecDeque::new()),
                inbound_frame_sequence: AtomicU64::new(0),
                inbound_frames_received: AtomicU64::new(0),
                inbound_updates_received: AtomicU64::new(0),
                inbound_update_bytes_received: AtomicU64::new(0),
                inbound_updates_drained: AtomicU64::new(0),
                inbound_update_bytes_drained: AtomicU64::new(0),
                inbound_overflow_disconnects: AtomicU64::new(0),
                total_read_us: AtomicU64::new(0),
                max_read_us: AtomicU64::new(0),
                total_decode_us: AtomicU64::new(0),
                max_decode_us: AtomicU64::new(0),
                disconnected: AtomicBool::new(false),
                last_error: Mutex::new(None),
            }
        }

        fn snapshot(&self) -> NativeClientIoDiagnostics {
            let oldest_inbound_update_age_ms = self
                .inbound_queued_at
                .lock()
                .ok()
                .and_then(|queued| queued.front().copied())
                .map_or(0.0, |queued_at| elapsed_ms(queued_at.elapsed()));
            NativeClientIoDiagnostics {
                outbound_command_depth: self.outbound_command_depth.load(Ordering::Acquire),
                inbound_update_batches: self.inbound_update_batches.load(Ordering::Acquire),
                inbound_update_depth: self.inbound_update_depth.load(Ordering::Acquire),
                inbound_update_bytes: self.inbound_update_bytes.load(Ordering::Acquire),
                oldest_inbound_update_age_ms,
                inbound_frame_sequence: self.inbound_frame_sequence.load(Ordering::Acquire),
                inbound_frames_received: self.inbound_frames_received.load(Ordering::Acquire),
                inbound_updates_received: self.inbound_updates_received.load(Ordering::Acquire),
                inbound_update_bytes_received: self
                    .inbound_update_bytes_received
                    .load(Ordering::Acquire),
                inbound_updates_drained: self.inbound_updates_drained.load(Ordering::Acquire),
                inbound_update_bytes_drained: self
                    .inbound_update_bytes_drained
                    .load(Ordering::Acquire),
                inbound_overflow_disconnects: self
                    .inbound_overflow_disconnects
                    .load(Ordering::Acquire),
                total_read_ms: us_to_ms(self.total_read_us.load(Ordering::Acquire)),
                max_read_ms: us_to_ms(self.max_read_us.load(Ordering::Acquire)),
                total_decode_ms: us_to_ms(self.total_decode_us.load(Ordering::Acquire)),
                max_decode_ms: us_to_ms(self.max_decode_us.load(Ordering::Acquire)),
                disconnected: self.disconnected.load(Ordering::Acquire),
                last_error: self
                    .last_error
                    .lock()
                    .ok()
                    .and_then(|last_error| last_error.clone()),
            }
        }

        fn record_batch_queued(
            &self,
            update_count: usize,
            encoded_bytes: usize,
            inbound_frame_sequence: u64,
            producer_read_ms: f64,
            producer_decode_ms: f64,
        ) {
            self.inbound_update_batches.fetch_add(1, Ordering::AcqRel);
            self.inbound_update_depth
                .fetch_add(update_count, Ordering::AcqRel);
            self.inbound_update_bytes
                .fetch_add(encoded_bytes, Ordering::AcqRel);
            if let Ok(mut queued) = self.inbound_queued_at.lock() {
                queued.push_back(Instant::now());
            }
            self.inbound_frame_sequence
                .store(inbound_frame_sequence, Ordering::Release);
            self.inbound_frames_received.fetch_add(1, Ordering::AcqRel);
            self.inbound_updates_received.fetch_add(
                u64::try_from(update_count).unwrap_or(u64::MAX),
                Ordering::AcqRel,
            );
            self.inbound_update_bytes_received.fetch_add(
                u64::try_from(encoded_bytes).unwrap_or(u64::MAX),
                Ordering::AcqRel,
            );
            let read_us = duration_us(producer_read_ms);
            let decode_us = duration_us(producer_decode_ms);
            self.total_read_us.fetch_add(read_us, Ordering::AcqRel);
            self.total_decode_us.fetch_add(decode_us, Ordering::AcqRel);
            atomic_max(&self.max_read_us, read_us);
            atomic_max(&self.max_decode_us, decode_us);
        }

        fn record_batch_drained(&self, batch: &NativeServerUpdateBatch) {
            self.inbound_update_batches.fetch_sub(1, Ordering::AcqRel);
            self.inbound_update_depth
                .fetch_sub(batch.update_count(), Ordering::AcqRel);
            self.inbound_update_bytes
                .fetch_sub(batch.encoded_bytes(), Ordering::AcqRel);
            if let Ok(mut queued) = self.inbound_queued_at.lock() {
                queued.pop_front();
            }
            self.inbound_updates_drained.fetch_add(
                u64::try_from(batch.update_count()).unwrap_or(u64::MAX),
                Ordering::AcqRel,
            );
            self.inbound_update_bytes_drained.fetch_add(
                u64::try_from(batch.encoded_bytes()).unwrap_or(u64::MAX),
                Ordering::AcqRel,
            );
        }

        fn record_batch_rejected(&self, batch: &NativeServerUpdateBatch, overflow: bool) {
            self.inbound_update_batches.fetch_sub(1, Ordering::AcqRel);
            self.inbound_update_depth
                .fetch_sub(batch.update_count(), Ordering::AcqRel);
            self.inbound_update_bytes
                .fetch_sub(batch.encoded_bytes(), Ordering::AcqRel);
            if let Ok(mut queued) = self.inbound_queued_at.lock() {
                queued.pop_back();
            }
            if overflow {
                self.inbound_overflow_disconnects
                    .fetch_add(1, Ordering::AcqRel);
            }
        }

        fn mark_disconnected(&self, message: impl Into<String>) {
            let message = message.into();
            self.disconnected.store(true, Ordering::Release);
            if let Ok(mut last_error) = self.last_error.lock() {
                *last_error = Some(message);
            }
        }

        fn actor_stopped_error(&self) -> NativeTransportError {
            let message = self
                .last_error
                .lock()
                .ok()
                .and_then(|last_error| last_error.clone())
                .unwrap_or_else(|| "native client I/O actor channel closed".to_owned());
            NativeTransportError::ActorStopped { message }
        }
    }

    #[derive(Debug)]
    pub struct NativeClientIoSession {
        command_tx: Option<mpsc::SyncSender<NativeClientIoCommand>>,
        update_rx: mpsc::Receiver<NativeServerUpdateBatch>,
        udp_client: Option<NativeUdpClient>,
        next_udp_frame_sequence: u64,
        last_reliable_ephemeral_send: Option<Instant>,
        diagnostics: Arc<NativeClientIoSharedDiagnostics>,
        shutdown_stream: TcpStream,
        writer_join_handle: Option<JoinHandle<()>>,
        reader_join_handle: Option<JoinHandle<()>>,
        negotiated_capabilities: SessionCapabilities,
    }

    #[derive(Debug)]
    struct NativeClientIoCommand {
        command: ClientCommand,
        shutdown_after_send: bool,
    }

    impl NativeClientIoSession {
        pub fn connect(addr: impl ToSocketAddrs) -> NativeTransportResult<Self> {
            Self::connect_with_identity(addr, &ClientIdentity::test_default())
        }

        pub fn connect_with_identity(
            addr: impl ToSocketAddrs,
            identity: &ClientIdentity,
        ) -> NativeTransportResult<Self> {
            let mut stream = TcpStream::connect(addr)?;
            let accepted = complete_client_handshake_with_identity_capabilities_and_udp_offer(
                &mut stream,
                identity,
                SessionCapabilities::DEVELOPMENT_DEFAULT,
            )?;
            let negotiated_capabilities = accepted.capabilities;
            let udp_client = accepted.udp_offer.and_then(|offer| {
                let mut server_addr = stream.peer_addr().ok()?;
                server_addr.set_port(offer.port);
                NativeUdpClient::connect(server_addr, offer.token).ok()
            });
            let shutdown_stream = stream.try_clone()?;
            let reader_stream = stream.try_clone()?;
            let (command_tx, command_rx) = mpsc::sync_channel(NATIVE_CLIENT_COMMAND_QUEUE_CAPACITY);
            let reader_command_tx = command_tx.clone();
            let (update_tx, update_rx) =
                mpsc::sync_channel(NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY);
            let diagnostics = Arc::new(NativeClientIoSharedDiagnostics::new());
            let writer_diagnostics = Arc::clone(&diagnostics);
            let writer_join_handle = thread::Builder::new()
                .name("mclone-native-client-writer".to_owned())
                .spawn(move || {
                    run_native_client_writer(stream, command_rx, writer_diagnostics);
                })?;
            let reader_diagnostics = Arc::clone(&diagnostics);
            let reader_join_handle = match thread::Builder::new()
                .name("mclone-native-client-reader".to_owned())
                .spawn(move || {
                    run_native_client_reader(
                        reader_stream,
                        update_tx,
                        reader_command_tx,
                        reader_diagnostics,
                    );
                }) {
                Ok(join_handle) => join_handle,
                Err(error) => {
                    drop(command_tx);
                    let _ = shutdown_stream.shutdown(Shutdown::Both);
                    let _ = writer_join_handle.join();
                    return Err(error.into());
                }
            };

            Ok(Self {
                command_tx: Some(command_tx),
                update_rx,
                udp_client,
                next_udp_frame_sequence: 0,
                last_reliable_ephemeral_send: None,
                diagnostics,
                shutdown_stream,
                writer_join_handle: Some(writer_join_handle),
                reader_join_handle: Some(reader_join_handle),
                negotiated_capabilities,
            })
        }

        pub const fn negotiated_capabilities(&self) -> SessionCapabilities {
            self.negotiated_capabilities
        }

        pub fn send_command_only(&mut self, command: ClientCommand) -> NativeTransportResult<()> {
            self.diagnostics
                .outbound_command_depth
                .fetch_add(1, Ordering::AcqRel);
            let Some(command_tx) = &self.command_tx else {
                self.diagnostics
                    .outbound_command_depth
                    .fetch_sub(1, Ordering::AcqRel);
                return Err(self.diagnostics.actor_stopped_error());
            };
            match command_tx.try_send(NativeClientIoCommand {
                command,
                shutdown_after_send: false,
            }) {
                Ok(()) => Ok(()),
                Err(mpsc::TrySendError::Full(_)) => {
                    self.diagnostics
                        .outbound_command_depth
                        .fetch_sub(1, Ordering::AcqRel);
                    Err(NativeTransportError::QueueFull {
                        lane: "outbound command",
                        capacity: NATIVE_CLIENT_COMMAND_QUEUE_CAPACITY,
                    })
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.diagnostics
                        .outbound_command_depth
                        .fetch_sub(1, Ordering::AcqRel);
                    Err(self.diagnostics.actor_stopped_error())
                }
            }
        }

        pub fn send_ephemeral(
            &mut self,
            message: ClientEphemeralMessage,
        ) -> NativeTransportResult<()> {
            if self
                .udp_client
                .as_ref()
                .is_some_and(|udp| udp.send_latest(message))
            {
                return Ok(());
            }
            let discontinuity = match message {
                ClientEphemeralMessage::BodyPose(sample) => sample.discontinuity,
            };
            if !discontinuity
                && self
                    .last_reliable_ephemeral_send
                    .is_some_and(|sent| sent.elapsed() < Duration::from_millis(50))
            {
                return Ok(());
            }
            self.send_command_only(ClientCommand::EphemeralFallback(message))?;
            self.last_reliable_ephemeral_send = Some(Instant::now());
            Ok(())
        }

        pub fn drain_update_batch(&mut self) -> NativeTransportResult<NativeServerUpdateBatch> {
            loop {
                if let Some(batch) = self.take_udp_update_batch() {
                    return Ok(batch);
                }
                match self
                    .update_rx
                    .recv_timeout(NATIVE_CLIENT_DRAIN_POLL_INTERVAL)
                {
                    Ok(batch) => {
                        self.diagnostics.record_batch_drained(&batch);
                        return Ok(batch);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(self.diagnostics.actor_stopped_error());
                    }
                }
            }
        }

        pub fn try_drain_update_batch(
            &mut self,
        ) -> NativeTransportResult<Option<NativeServerUpdateBatch>> {
            if let Some(batch) = self.take_udp_update_batch() {
                return Ok(Some(batch));
            }
            match self.update_rx.try_recv() {
                Ok(batch) => {
                    self.diagnostics.record_batch_drained(&batch);
                    Ok(Some(batch))
                }
                Err(mpsc::TryRecvError::Empty) => Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err(self.diagnostics.actor_stopped_error())
                }
            }
        }

        pub fn diagnostics(&self) -> NativeClientIoDiagnostics {
            let mut diagnostics = self.diagnostics.snapshot();
            if let Some(udp) = &self.udp_client {
                let udp = udp.diagnostics();
                diagnostics.inbound_update_batches = diagnostics
                    .inbound_update_batches
                    .saturating_add(udp.pending_inbound);
                diagnostics.inbound_update_depth = diagnostics
                    .inbound_update_depth
                    .saturating_add(udp.pending_inbound);
            }
            diagnostics
        }

        pub fn native_udp_diagnostics(&self) -> Option<NativeUdpDiagnostics> {
            self.udp_client.as_ref().map(NativeUdpClient::diagnostics)
        }

        fn take_udp_update_batch(&mut self) -> Option<NativeServerUpdateBatch> {
            let received = self.udp_client.as_ref()?.take_received()?;
            self.next_udp_frame_sequence = self
                .next_udp_frame_sequence
                .max(
                    self.diagnostics
                        .inbound_frame_sequence
                        .load(Ordering::Acquire),
                )
                .saturating_add(1);
            let update = ServerUpdate::EphemeralFallback(received.message);
            let encoded_len = encode_server_update(&update)
                .map(|encoded| encoded.len())
                .unwrap_or(received.encoded_len);
            let batch = NativeServerUpdateBatch {
                updates: vec![NativeServerUpdateEnvelope {
                    update,
                    encoded_len,
                    inbound_frame_sequence: self.next_udp_frame_sequence,
                    producer_read_ms: 0.0,
                    producer_decode_ms: 0.0,
                }],
                inbound_frame_sequence: self.next_udp_frame_sequence,
                producer_read_ms: 0.0,
                producer_decode_ms: 0.0,
                queued_at: received.received_at,
            };
            self.diagnostics.record_batch_queued(
                batch.update_count(),
                batch.encoded_bytes(),
                batch.inbound_frame_sequence,
                0.0,
                0.0,
            );
            self.diagnostics.record_batch_drained(&batch);
            Some(batch)
        }
    }

    impl Drop for NativeClientIoSession {
        fn drop(&mut self) {
            if let Some(command_tx) = self.command_tx.take() {
                self.diagnostics
                    .outbound_command_depth
                    .fetch_add(1, Ordering::AcqRel);
                if command_tx
                    .try_send(NativeClientIoCommand {
                        command: ClientCommand::Disconnect(
                            mclone_protocol::ClientDisconnectReason::Quit,
                        ),
                        shutdown_after_send: true,
                    })
                    .is_err()
                {
                    self.diagnostics
                        .outbound_command_depth
                        .fetch_sub(1, Ordering::AcqRel);
                }
            }
            if let Some(join_handle) = self.writer_join_handle.take() {
                let _ = join_handle.join();
            }
            let _ = self.shutdown_stream.shutdown(Shutdown::Both);
            if let Some(join_handle) = self.reader_join_handle.take() {
                let _ = join_handle.join();
            }
        }
    }

    fn run_native_client_writer(
        mut stream: TcpStream,
        command_rx: mpsc::Receiver<NativeClientIoCommand>,
        diagnostics: Arc<NativeClientIoSharedDiagnostics>,
    ) {
        while let Ok(command_request) = command_rx.recv() {
            diagnostics
                .outbound_command_depth
                .fetch_sub(1, Ordering::AcqRel);
            if let Err(error) = write_client_command_frame(&mut stream, &command_request.command)
                .and_then(|()| stream.flush().map_err(NativeTransportError::from))
            {
                let message = error.to_string();
                diagnostics.mark_disconnected(message);
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
            if command_request.shutdown_after_send {
                return;
            }
        }
    }

    fn run_native_client_reader(
        mut stream: TcpStream,
        update_tx: mpsc::SyncSender<NativeServerUpdateBatch>,
        command_tx: mpsc::SyncSender<NativeClientIoCommand>,
        diagnostics: Arc<NativeClientIoSharedDiagnostics>,
    ) {
        let mut inbound_frame_sequence = 0_u64;
        loop {
            inbound_frame_sequence = inbound_frame_sequence.saturating_add(1);
            let batch =
                match read_server_update_batch_instrumented(&mut stream, inbound_frame_sequence) {
                    Ok(batch) => batch,
                    Err(error) => {
                        let message = error.to_string();
                        let update = ServerUpdate::Disconnect(match &error {
                            NativeTransportError::Io(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::UnexpectedEof
                                        | std::io::ErrorKind::ConnectionAborted
                                        | std::io::ErrorKind::ConnectionReset
                                        | std::io::ErrorKind::BrokenPipe
                                ) =>
                            {
                                DisconnectReason::end_of_stream(message.clone())
                            }
                            _ => DisconnectReason::transport_error(message.clone()),
                        });
                        let encoded_len = encode_server_update(&update)
                            .map(|encoded| encoded.len())
                            .unwrap_or_default();
                        let batch = NativeServerUpdateBatch {
                            updates: vec![NativeServerUpdateEnvelope {
                                update,
                                encoded_len,
                                inbound_frame_sequence,
                                producer_read_ms: 0.0,
                                producer_decode_ms: 0.0,
                            }],
                            inbound_frame_sequence,
                            producer_read_ms: 0.0,
                            producer_decode_ms: 0.0,
                            queued_at: Instant::now(),
                        };
                        diagnostics.record_batch_queued(
                            batch.update_count(),
                            batch.encoded_bytes(),
                            inbound_frame_sequence,
                            0.0,
                            0.0,
                        );
                        if let Err(error) = update_tx.try_send(batch) {
                            match error {
                                mpsc::TrySendError::Full(batch) => {
                                    diagnostics.record_batch_rejected(&batch, true);
                                }
                                mpsc::TrySendError::Disconnected(batch) => {
                                    diagnostics.record_batch_rejected(&batch, false);
                                }
                            }
                        }
                        diagnostics.mark_disconnected(message);
                        let _ = stream.shutdown(Shutdown::Both);
                        return;
                    }
                };
            for update in &batch.updates {
                let ServerUpdate::KeepAlive { id } = &update.update else {
                    continue;
                };
                diagnostics
                    .outbound_command_depth
                    .fetch_add(1, Ordering::AcqRel);
                if command_tx
                    .try_send(NativeClientIoCommand {
                        command: ClientCommand::KeepAlive { id: *id },
                        shutdown_after_send: false,
                    })
                    .is_err()
                {
                    diagnostics
                        .outbound_command_depth
                        .fetch_sub(1, Ordering::AcqRel);
                    diagnostics.mark_disconnected(
                        "outbound command queue could not accept a keepalive response",
                    );
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
            }
            let update_count = batch.update_count();
            let encoded_bytes = batch.encoded_bytes();
            let producer_read_ms = batch.producer_read_ms;
            let producer_decode_ms = batch.producer_decode_ms;
            diagnostics.record_batch_queued(
                update_count,
                encoded_bytes,
                inbound_frame_sequence,
                producer_read_ms,
                producer_decode_ms,
            );
            match update_tx.try_send(batch) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(batch)) => {
                    diagnostics.record_batch_rejected(&batch, true);
                    diagnostics.mark_disconnected(format!(
                        "inbound update batch queue reached capacity {NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY}"
                    ));
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
                Err(mpsc::TrySendError::Disconnected(batch)) => {
                    diagnostics.record_batch_rejected(&batch, false);
                    diagnostics.mark_disconnected("runtime update receiver closed");
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
            }
        }
    }

    fn read_server_update_batch_instrumented(
        reader: &mut impl Read,
        inbound_frame_sequence: u64,
    ) -> NativeTransportResult<NativeServerUpdateBatch> {
        let mut producer_read_ms = 0.0;
        let mut producer_decode_ms = 0.0;
        let read_start = Instant::now();
        let update_count = read_u32(reader)? as usize;
        producer_read_ms += elapsed_ms(read_start.elapsed());
        let mut updates = Vec::with_capacity(update_count);
        for _ in 0..update_count {
            let read_start = Instant::now();
            let payload = read_frame(reader, "server update")?;
            let update_read_ms = elapsed_ms(read_start.elapsed());
            producer_read_ms += update_read_ms;
            let encoded_len = payload.len();
            let decode_start = Instant::now();
            let update = decode_server_update(&payload)?;
            let update_decode_ms = elapsed_ms(decode_start.elapsed());
            producer_decode_ms += update_decode_ms;
            updates.push(NativeServerUpdateEnvelope {
                update,
                encoded_len,
                inbound_frame_sequence,
                producer_read_ms: update_read_ms,
                producer_decode_ms: update_decode_ms,
            });
        }
        Ok(NativeServerUpdateBatch {
            updates,
            inbound_frame_sequence,
            producer_read_ms,
            producer_decode_ms,
            queued_at: Instant::now(),
        })
    }

    fn elapsed_ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1_000.0
    }

    fn duration_us(duration_ms: f64) -> u64 {
        (duration_ms * 1_000.0).max(0.0).round() as u64
    }

    fn us_to_ms(duration_us: u64) -> f64 {
        duration_us as f64 / 1_000.0
    }

    fn atomic_max(value: &AtomicU64, candidate: u64) {
        let mut current = value.load(Ordering::Acquire);
        while candidate > current {
            match value.compare_exchange(current, candidate, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    pub fn complete_client_handshake<T: Read + Write>(stream: &mut T) -> NativeTransportResult<()> {
        complete_client_handshake_with_version(stream, PROTOCOL_VERSION)
    }

    pub fn complete_client_handshake_with_identity<T: Read + Write>(
        stream: &mut T,
        identity: &ClientIdentity,
    ) -> NativeTransportResult<()> {
        complete_client_handshake_with_identity_and_capabilities(
            stream,
            identity,
            SessionCapabilities::DEVELOPMENT_DEFAULT,
        )
        .map(|_| ())
    }

    pub fn complete_client_handshake_with_identity_and_capabilities<T: Read + Write>(
        stream: &mut T,
        identity: &ClientIdentity,
        capabilities: SessionCapabilities,
    ) -> NativeTransportResult<SessionCapabilities> {
        complete_client_handshake_with_identity_capabilities_and_udp_offer(
            stream,
            identity,
            capabilities,
        )
        .map(|accepted| accepted.capabilities)
    }

    pub fn complete_client_handshake_with_identity_capabilities_and_udp_offer<T: Read + Write>(
        stream: &mut T,
        identity: &ClientIdentity,
        capabilities: SessionCapabilities,
    ) -> NativeTransportResult<AcceptedNativeServerHandshake> {
        write_client_handshake(stream, PROTOCOL_VERSION, identity, capabilities)?;
        stream.flush()?;
        read_server_handshake(stream, PROTOCOL_VERSION)
    }

    pub fn complete_client_handshake_with_version<T: Read + Write>(
        stream: &mut T,
        protocol_version: u32,
    ) -> NativeTransportResult<()> {
        write_client_handshake(
            stream,
            protocol_version,
            &ClientIdentity::test_default(),
            SessionCapabilities::DEVELOPMENT_DEFAULT,
        )?;
        stream.flush()?;
        read_server_handshake(stream, protocol_version).map(|_| ())
    }

    pub fn complete_server_handshake<T: Read + Write>(
        stream: &mut T,
    ) -> NativeTransportResult<ClientIdentity> {
        complete_server_handshake_with_capabilities(
            stream,
            SessionCapabilities::DEVELOPMENT_DEFAULT,
        )
        .map(|accepted| accepted.identity)
    }

    pub fn complete_server_handshake_with_capabilities<T: Read + Write>(
        stream: &mut T,
        server_capabilities: SessionCapabilities,
    ) -> NativeTransportResult<super::AcceptedClientHandshake> {
        complete_server_handshake_with_capabilities_and_udp_offer(stream, server_capabilities, None)
    }

    pub fn complete_server_handshake_with_capabilities_and_udp_offer<T: Read + Write>(
        stream: &mut T,
        server_capabilities: SessionCapabilities,
        udp_offer: Option<NativeUdpOffer>,
    ) -> NativeTransportResult<super::AcceptedClientHandshake> {
        let received = read_client_handshake(stream)?;
        if received.protocol_version != PROTOCOL_VERSION {
            write_server_handshake_reject(stream, PROTOCOL_VERSION, received.protocol_version)?;
            return Err(NativeTransportError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received: received.protocol_version,
            });
        }
        let capabilities = received
            .capabilities
            .intersection(server_capabilities)
            .known();
        let udp_offer =
            udp_offer.filter(|_| capabilities.contains(SessionCapabilities::EPHEMERAL_BODY_POSE));
        write_server_handshake_accept(stream, PROTOCOL_VERSION, capabilities, udp_offer)?;
        Ok(super::AcceptedClientHandshake {
            identity: received.identity,
            capabilities,
        })
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

    fn write_client_handshake(
        writer: &mut impl Write,
        protocol_version: u32,
        identity: &ClientIdentity,
        capabilities: SessionCapabilities,
    ) -> NativeTransportResult<()> {
        validate_client_identity(identity)?;
        let name = identity.display_name.as_bytes();
        let mut payload = Vec::with_capacity(HANDSHAKE_MAGIC.len() + 29 + name.len());
        payload.extend_from_slice(HANDSHAKE_MAGIC);
        payload.extend_from_slice(&protocol_version.to_le_bytes());
        payload.extend_from_slice(&capabilities.bits().to_le_bytes());
        payload.extend_from_slice(&identity.profile_id.bytes());
        payload.push(name.len() as u8);
        payload.extend_from_slice(name);
        write_frame(writer, "client handshake", &payload)
    }

    fn read_client_handshake(
        reader: &mut impl Read,
    ) -> NativeTransportResult<super::ClientHandshake> {
        let payload = read_frame(reader, "client handshake")?;
        let fixed_len = HANDSHAKE_MAGIC.len() + 29;
        if payload.len() < fixed_len {
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
        let protocol_version = u32::from_le_bytes(
            payload[version_offset..version_offset + 4]
                .try_into()
                .expect("handshake version length checked"),
        );
        let capabilities_offset = version_offset + 4;
        let capabilities = SessionCapabilities::from_bits_retain(u64::from_le_bytes(
            payload[capabilities_offset..capabilities_offset + 8]
                .try_into()
                .expect("handshake capabilities length checked"),
        ));
        let profile_offset = capabilities_offset + 8;
        let profile_id = PlayerProfileId::new(
            payload[profile_offset..profile_offset + 16]
                .try_into()
                .expect("handshake profile UUID length checked"),
        );
        let name_len_offset = profile_offset + 16;
        let name_len = usize::from(payload[name_len_offset]);
        if payload.len() != fixed_len + name_len {
            return Err(NativeTransportError::InvalidHandshake(
                "client handshake display name length did not match payload",
            ));
        }
        let display_name = std::str::from_utf8(&payload[name_len_offset + 1..])
            .map_err(|_| {
                NativeTransportError::InvalidHandshake(
                    "client handshake display name was not UTF-8",
                )
            })?
            .to_owned();
        let identity = ClientIdentity::new(profile_id, display_name)?;
        Ok(super::ClientHandshake {
            protocol_version,
            capabilities,
            identity,
        })
    }

    fn write_server_handshake_accept(
        writer: &mut impl Write,
        protocol_version: u32,
        capabilities: SessionCapabilities,
        udp_offer: Option<NativeUdpOffer>,
    ) -> NativeTransportResult<()> {
        write_server_handshake(
            writer,
            SERVER_HANDSHAKE_ACCEPT,
            protocol_version,
            protocol_version,
            capabilities,
            udp_offer,
        )
    }

    fn write_server_handshake_reject(
        writer: &mut impl Write,
        expected: u32,
        received: u32,
    ) -> NativeTransportResult<()> {
        write_server_handshake(
            writer,
            SERVER_HANDSHAKE_REJECT,
            expected,
            received,
            SessionCapabilities::NONE,
            None,
        )
    }

    fn write_server_handshake(
        writer: &mut impl Write,
        status: u8,
        expected: u32,
        received: u32,
        capabilities: SessionCapabilities,
        udp_offer: Option<NativeUdpOffer>,
    ) -> NativeTransportResult<()> {
        let mut payload = Vec::with_capacity(
            SERVER_HANDSHAKE_BASE_BYTES + udp_offer.map_or(0, |_| SERVER_HANDSHAKE_UDP_OFFER_BYTES),
        );
        payload.extend_from_slice(HANDSHAKE_MAGIC);
        payload.push(status);
        payload.extend_from_slice(&expected.to_le_bytes());
        payload.extend_from_slice(&received.to_le_bytes());
        payload.extend_from_slice(&capabilities.bits().to_le_bytes());
        if let Some(offer) = udp_offer {
            payload.push(1);
            payload.extend_from_slice(&offer.port.to_le_bytes());
            payload.extend_from_slice(&offer.token.bytes());
        }
        write_frame(writer, "server handshake", &payload)?;
        writer.flush()?;
        Ok(())
    }

    fn read_server_handshake(
        reader: &mut impl Read,
        client_version: u32,
    ) -> NativeTransportResult<AcceptedNativeServerHandshake> {
        let payload = read_frame(reader, "server handshake")?;
        if payload.len() != SERVER_HANDSHAKE_BASE_BYTES
            && payload.len() != SERVER_HANDSHAKE_BASE_BYTES + SERVER_HANDSHAKE_UDP_OFFER_BYTES
        {
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
        let capabilities_offset = received_offset + 4;
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
        let capabilities = SessionCapabilities::from_bits_retain(u64::from_le_bytes(
            payload[capabilities_offset..capabilities_offset + 8]
                .try_into()
                .expect("server handshake capabilities length checked"),
        ))
        .known();
        let udp_offer = if payload.len() == SERVER_HANDSHAKE_BASE_BYTES {
            None
        } else {
            let offer_offset = SERVER_HANDSHAKE_BASE_BYTES;
            if payload[offer_offset] != 1 {
                return Err(NativeTransportError::InvalidHandshake(
                    "server handshake had invalid UDP offer marker",
                ));
            }
            let port = u16::from_le_bytes(
                payload[offer_offset + 1..offer_offset + 3]
                    .try_into()
                    .expect("server handshake UDP port length checked"),
            );
            if port == 0 {
                return Err(NativeTransportError::InvalidHandshake(
                    "server handshake UDP offer used port zero",
                ));
            }
            let token = crate::NativeUdpAttachmentToken::from_bytes(
                payload[offer_offset + 3..offer_offset + 19]
                    .try_into()
                    .expect("server handshake UDP token length checked"),
            );
            Some(NativeUdpOffer { port, token })
        };

        match status {
            SERVER_HANDSHAKE_ACCEPT => {
                if expected != client_version || received != client_version {
                    return Err(NativeTransportError::ProtocolVersionMismatch {
                        expected,
                        received: client_version,
                    });
                }
                Ok(AcceptedNativeServerHandshake {
                    capabilities,
                    udp_offer,
                })
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
    use mclone_protocol::{
        ChunkView, ClientDisconnectReason, DimensionKey, DisconnectReason, DisconnectReasonCode,
        PROTOCOL_VERSION,
    };
    #[cfg(not(target_arch = "wasm32"))]
    use std::io::Write;

    fn time_update(day_time: u64) -> ServerUpdate {
        ServerUpdate::TimeUpdate {
            game_time: day_time.saturating_add(100),
            day_time,
            daylight_cycle_running: true,
            calendar_policy: Default::default(),
        }
    }

    fn dimension_change() -> ServerUpdate {
        ServerUpdate::DimensionChange {
            dimension: DimensionKey::parse("mclone:moon").unwrap(),
            biome_zoom_seed: 54_321,
            topology: mclone_core::HorizontalTopology::UNBOUNDED,
            keep_player_state: true,
        }
    }

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
        let decoded = decode_websocket_client_handshake(&client).unwrap();
        assert_eq!(decoded.protocol_version, PROTOCOL_VERSION);
        assert_eq!(
            decoded.capabilities,
            SessionCapabilities::DEVELOPMENT_DEFAULT
        );
        assert_eq!(decoded.identity, ClientIdentity::test_default());

        let server = encode_websocket_server_handshake_accept(PROTOCOL_VERSION).unwrap();
        assert_eq!(
            decode_websocket_server_handshake(&server, PROTOCOL_VERSION).unwrap(),
            SessionCapabilities::DEVELOPMENT_DEFAULT
        );
    }

    #[test]
    fn websocket_handshake_carries_negotiated_capabilities() {
        let client = encode_websocket_client_handshake_with_identity_and_capabilities(
            PROTOCOL_VERSION,
            &ClientIdentity::test_default(),
            SessionCapabilities::from_bits_retain(
                SessionCapabilities::DEBUG_ACTIONS.bits() | (1 << 63),
            ),
        )
        .unwrap();
        let decoded = decode_websocket_client_handshake(&client).unwrap();
        assert!(
            decoded
                .capabilities
                .contains(SessionCapabilities::DEBUG_ACTIONS)
        );

        let server = encode_websocket_server_handshake_accept_with_capabilities(
            PROTOCOL_VERSION,
            decoded.capabilities.intersection(SessionCapabilities::NONE),
        )
        .unwrap();
        assert_eq!(
            decode_websocket_server_handshake(&server, PROTOCOL_VERSION).unwrap(),
            SessionCapabilities::NONE
        );
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
            dimension_change(),
            time_update(99),
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
            dimension_change(),
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
    fn native_tcp_loopback_exchanges_independent_frames() {
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

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        session.send_command_only(command).unwrap();
        let updates = session.drain_update_batch().unwrap().into_updates();
        server.join().unwrap();

        assert_eq!(updates, expected_updates);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_io_session_reuses_one_connection() {
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
        let first_updates = vec![time_update(1)];
        let second_updates = vec![time_update(2)];
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
            assert_eq!(
                try_read_client_command_frame(&mut stream).unwrap(),
                Some(ClientCommand::Disconnect(ClientDisconnectReason::Quit))
            );
        });

        {
            let mut session = NativeClientIoSession::connect(addr).unwrap();
            session.send_command_only(first_command).unwrap();
            assert_eq!(
                session.drain_update_batch().unwrap().into_updates(),
                first_updates
            );
            session.send_command_only(second_command).unwrap();
            assert_eq!(
                session.drain_update_batch().unwrap().into_updates(),
                second_updates
            );
        }
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_io_actor_send_does_not_wait_for_delayed_publication() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let server_command = command.clone();
        let expected_updates = vec![time_update(1234)];
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

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        let send_start = std::time::Instant::now();
        session.send_command_only(command).unwrap();
        assert!(
            send_start.elapsed() < std::time::Duration::from_millis(100),
            "actor-backed send waited for a delayed server publication"
        );
        assert_eq!(session.try_drain_update_batch().unwrap(), None);

        let batch = session.drain_update_batch().unwrap();
        assert_eq!(batch.into_updates(), expected_updates);
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_writer_progresses_while_update_reader_is_held() {
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
        let first_updates = vec![time_update(10)];
        let second_updates = vec![time_update(20)];
        let first_server_updates = first_updates.clone();
        let second_server_updates = second_updates.clone();
        let (first_command_read_tx, first_command_read_rx) = std::sync::mpsc::channel();
        let (second_command_read_tx, second_command_read_rx) = std::sync::mpsc::channel();
        let (release_first_publication_tx, release_first_publication_rx) =
            std::sync::mpsc::channel();
        let (release_server_tx, release_server_rx) = std::sync::mpsc::channel();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                first_server_command
            );
            first_command_read_tx.send(()).unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                second_server_command
            );
            second_command_read_tx.send(()).unwrap();
            release_first_publication_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            write_server_update_batch(&mut stream, &first_server_updates).unwrap();
            write_server_update_batch(&mut stream, &second_server_updates).unwrap();
            release_server_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        session.send_command_only(first_command).unwrap();
        first_command_read_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();

        let send_start = std::time::Instant::now();
        session.send_command_only(second_command).unwrap();
        assert!(
            send_start.elapsed() < std::time::Duration::from_millis(100),
            "actor-backed send waited for the prior held publication"
        );
        second_command_read_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("dedicated writer should deliver a second command while reads are held");
        assert_eq!(session.try_drain_update_batch().unwrap(), None);
        assert_eq!(session.diagnostics().outbound_command_depth, 0);

        release_first_publication_tx.send(()).unwrap();
        let first_batch = session.drain_update_batch().unwrap();
        assert_eq!(first_batch.inbound_frame_sequence, 1);
        assert_eq!(first_batch.into_updates(), first_updates);
        let second_batch = session.drain_update_batch().unwrap();
        assert_eq!(second_batch.inbound_frame_sequence, 2);
        assert_eq!(second_batch.into_updates(), second_updates);

        let diagnostics = session.diagnostics();
        assert_eq!(diagnostics.outbound_command_depth, 0);
        assert_eq!(diagnostics.inbound_update_batches, 0);
        assert_eq!(diagnostics.inbound_frame_sequence, 2);
        assert!(!diagnostics.disconnected);
        release_server_tx.send(()).unwrap();
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_reader_accepts_unsolicited_update_frame() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let expected_updates = vec![time_update(77)];
        let server_updates = expected_updates.clone();
        let (release_server_tx, release_server_rx) = std::sync::mpsc::channel();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            write_server_update_batch(&mut stream, &server_updates).unwrap();
            release_server_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        let batch = session.drain_update_batch().unwrap();
        assert_eq!(batch.inbound_frame_sequence, 1);
        assert_eq!(batch.into_updates(), expected_updates);
        assert_eq!(session.diagnostics().outbound_command_depth, 0);
        assert!(!session.diagnostics().disconnected);

        release_server_tx.send(()).unwrap();
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_io_actor_preserves_order_and_reports_diagnostics() {
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
        let first_updates = vec![
            time_update(10),
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(4, -3),
            },
        ];
        let second_updates = vec![time_update(20)];
        let first_server_updates = first_updates.clone();
        let second_server_updates = second_updates.clone();
        let (release_server_tx, release_server_rx) = std::sync::mpsc::channel();

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
            release_server_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        session.send_command_only(first_command).unwrap();
        session.send_command_only(second_command).unwrap();

        let first_batch = session.drain_update_batch().unwrap();
        assert_eq!(first_batch.inbound_frame_sequence, 1);
        assert_eq!(first_batch.updates.len(), first_updates.len());
        assert_eq!(first_batch.updates[0].update, first_updates[0]);
        assert_eq!(first_batch.updates[1].update, first_updates[1]);
        assert_eq!(
            first_batch.updates[0].encoded_len,
            mclone_protocol::encode_server_update(&first_updates[0])
                .unwrap()
                .len()
        );
        assert!(first_batch.producer_read_ms >= 0.0);
        assert!(first_batch.producer_decode_ms >= 0.0);

        let second_batch = session.drain_update_batch().unwrap();
        assert_eq!(second_batch.inbound_frame_sequence, 2);
        assert_eq!(second_batch.into_updates(), second_updates);

        let diagnostics = session.diagnostics();
        assert_eq!(diagnostics.outbound_command_depth, 0);
        assert_eq!(diagnostics.inbound_update_batches, 0);
        assert_eq!(diagnostics.inbound_update_depth, 0);
        assert_eq!(diagnostics.inbound_update_bytes, 0);
        assert_eq!(diagnostics.inbound_frame_sequence, 2);
        assert_eq!(diagnostics.inbound_frames_received, 2);
        assert_eq!(diagnostics.inbound_updates_received, 3);
        assert_eq!(diagnostics.inbound_updates_drained, 3);
        assert_eq!(
            diagnostics.inbound_update_bytes_received,
            diagnostics.inbound_update_bytes_drained
        );
        assert_eq!(diagnostics.inbound_overflow_disconnects, 0);
        assert_eq!(diagnostics.oldest_inbound_update_age_ms, 0.0);
        assert!(!diagnostics.disconnected);
        assert_eq!(diagnostics.last_error, None);

        release_server_tx.send(()).unwrap();
        server.join().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_disconnects_at_bounded_inbound_capacity() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            for sequence in 0..=NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY {
                if write_server_update_batch(&mut stream, &[time_update(sequence as u64)]).is_err()
                {
                    break;
                }
            }
        });

        let session = NativeClientIoSession::connect(addr).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let diagnostics = loop {
            let diagnostics = session.diagnostics();
            if diagnostics.disconnected {
                break diagnostics;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        assert_eq!(
            diagnostics.inbound_update_batches,
            NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY
        );
        assert_eq!(
            diagnostics.inbound_update_depth,
            NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY
        );
        assert_eq!(
            diagnostics.inbound_frames_received,
            (NATIVE_CLIENT_UPDATE_BATCH_QUEUE_CAPACITY + 1) as u64
        );
        assert_eq!(diagnostics.inbound_overflow_disconnects, 1);
        assert!(diagnostics.inbound_update_bytes > 0);
        assert!(diagnostics.oldest_inbound_update_age_ms >= 0.0);
        assert!(
            diagnostics
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("queue reached capacity 256"))
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_handshake_negotiates_capability_intersection() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake_with_capabilities(&mut stream, SessionCapabilities::NONE)
                .unwrap()
        });

        let mut stream = std::net::TcpStream::connect(addr).unwrap();
        let negotiated = complete_client_handshake_with_identity_and_capabilities(
            &mut stream,
            &ClientIdentity::test_default(),
            SessionCapabilities::DEVELOPMENT_DEFAULT,
        )
        .unwrap();
        let accepted = server.join().unwrap();

        assert_eq!(negotiated, SessionCapabilities::NONE);
        assert_eq!(accepted.capabilities, SessionCapabilities::NONE);
        assert_eq!(accepted.identity, ClientIdentity::test_default());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_echoes_keepalive_without_runtime_polling() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            write_server_update_batch(&mut stream, &[ServerUpdate::KeepAlive { id: 42 }]).unwrap();
            stream.flush().unwrap();
            try_read_client_command_frame(&mut stream).unwrap()
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        assert_eq!(
            session.drain_update_batch().unwrap().into_updates(),
            vec![ServerUpdate::KeepAlive { id: 42 }]
        );
        assert_eq!(
            server.join().unwrap(),
            Some(ClientCommand::KeepAlive { id: 42 })
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_reports_unexpected_eof_as_typed_disconnect() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        server.join().unwrap();
        let updates = session.drain_update_batch().unwrap().into_updates();
        let [ServerUpdate::Disconnect(reason)] = updates.as_slice() else {
            panic!("unexpected EOF should enqueue one typed disconnect");
        };
        assert_eq!(reason.code, DisconnectReasonCode::EndOfStream);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_drop_sends_ordered_quit_command() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            try_read_client_command_frame(&mut stream).unwrap()
        });

        let session = NativeClientIoSession::connect(addr).unwrap();
        drop(session);

        assert_eq!(
            server.join().unwrap(),
            Some(ClientCommand::Disconnect(ClientDisconnectReason::Quit))
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_tcp_client_preserves_explicit_server_disconnect() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reason = DisconnectReason::new(DisconnectReasonCode::Kicked, "test kick");
        let expected = reason.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            write_server_update_batch(&mut stream, &[ServerUpdate::Disconnect(reason)]).unwrap();
            stream.flush().unwrap();
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        let updates = session.drain_update_batch().unwrap().into_updates();
        server.join().unwrap();

        assert_eq!(updates, vec![ServerUpdate::Disconnect(expected)]);
    }
}
