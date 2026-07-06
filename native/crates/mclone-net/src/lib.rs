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
    NativeClientIoDiagnostics, NativeClientIoSession, NativeClientSession, NativeServerUpdateBatch,
    NativeServerUpdateEnvelope, NativeTransportError, NativeTransportResult,
    complete_client_handshake, complete_client_handshake_with_version, complete_server_handshake,
    read_client_command_frame, read_client_command_frames, read_server_update_batch,
    request_server_updates, try_read_client_command_frame, write_client_command_frame,
    write_server_update_batch,
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
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

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

    #[derive(Clone, Debug, PartialEq)]
    pub struct NativeServerUpdateEnvelope {
        pub update: ServerUpdate,
        pub encoded_len: usize,
        pub response_sequence: u64,
        pub producer_read_ms: f64,
        pub producer_decode_ms: f64,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct NativeServerUpdateBatch {
        pub updates: Vec<NativeServerUpdateEnvelope>,
        pub response_sequence: u64,
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
        pub inbound_response_batches: usize,
        pub inbound_update_depth: usize,
        pub inbound_update_bytes: usize,
        pub response_sequence: u64,
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
        inbound_response_batches: AtomicUsize,
        inbound_update_depth: AtomicUsize,
        inbound_update_bytes: AtomicUsize,
        response_sequence: AtomicU64,
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
                inbound_response_batches: AtomicUsize::new(0),
                inbound_update_depth: AtomicUsize::new(0),
                inbound_update_bytes: AtomicUsize::new(0),
                response_sequence: AtomicU64::new(0),
                total_read_us: AtomicU64::new(0),
                max_read_us: AtomicU64::new(0),
                total_decode_us: AtomicU64::new(0),
                max_decode_us: AtomicU64::new(0),
                disconnected: AtomicBool::new(false),
                last_error: Mutex::new(None),
            }
        }

        fn snapshot(&self) -> NativeClientIoDiagnostics {
            NativeClientIoDiagnostics {
                outbound_command_depth: self.outbound_command_depth.load(Ordering::Acquire),
                inbound_response_batches: self.inbound_response_batches.load(Ordering::Acquire),
                inbound_update_depth: self.inbound_update_depth.load(Ordering::Acquire),
                inbound_update_bytes: self.inbound_update_bytes.load(Ordering::Acquire),
                response_sequence: self.response_sequence.load(Ordering::Acquire),
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

        fn record_batch_queued(&self, batch: &NativeServerUpdateBatch) {
            self.inbound_response_batches.fetch_add(1, Ordering::AcqRel);
            self.inbound_update_depth
                .fetch_add(batch.update_count(), Ordering::AcqRel);
            self.inbound_update_bytes
                .fetch_add(batch.encoded_bytes(), Ordering::AcqRel);
            self.response_sequence
                .store(batch.response_sequence, Ordering::Release);
            let read_us = duration_us(batch.producer_read_ms);
            let decode_us = duration_us(batch.producer_decode_ms);
            self.total_read_us.fetch_add(read_us, Ordering::AcqRel);
            self.total_decode_us.fetch_add(decode_us, Ordering::AcqRel);
            atomic_max(&self.max_read_us, read_us);
            atomic_max(&self.max_decode_us, decode_us);
        }

        fn record_batch_drained(&self, batch: &NativeServerUpdateBatch) {
            self.inbound_response_batches.fetch_sub(1, Ordering::AcqRel);
            self.inbound_update_depth
                .fetch_sub(batch.update_count(), Ordering::AcqRel);
            self.inbound_update_bytes
                .fetch_sub(batch.encoded_bytes(), Ordering::AcqRel);
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
        command_tx: Option<mpsc::Sender<NativeClientIoCommand>>,
        update_rx: mpsc::Receiver<NativeServerUpdateBatch>,
        diagnostics: Arc<NativeClientIoSharedDiagnostics>,
        shutdown_stream: TcpStream,
        join_handle: Option<JoinHandle<()>>,
    }

    #[derive(Debug)]
    struct NativeClientIoCommand {
        command: ClientCommand,
        write_result_tx: mpsc::Sender<Result<(), String>>,
    }

    impl NativeClientIoSession {
        pub fn connect(addr: impl ToSocketAddrs) -> NativeTransportResult<Self> {
            let mut stream = TcpStream::connect(addr)?;
            complete_client_handshake(&mut stream)?;
            let shutdown_stream = stream.try_clone()?;
            let (command_tx, command_rx) = mpsc::channel();
            let (update_tx, update_rx) = mpsc::channel();
            let diagnostics = Arc::new(NativeClientIoSharedDiagnostics::new());
            let worker_diagnostics = Arc::clone(&diagnostics);
            let join_handle = thread::Builder::new()
                .name("mclone-native-client-io".to_owned())
                .spawn(move || {
                    run_native_client_io_actor(stream, command_rx, update_tx, worker_diagnostics);
                })?;

            Ok(Self {
                command_tx: Some(command_tx),
                update_rx,
                diagnostics,
                shutdown_stream,
                join_handle: Some(join_handle),
            })
        }

        pub fn send_command_only(&mut self, command: ClientCommand) -> NativeTransportResult<()> {
            let (write_result_tx, write_result_rx) = mpsc::channel();
            self.diagnostics
                .outbound_command_depth
                .fetch_add(1, Ordering::AcqRel);
            let Some(command_tx) = &self.command_tx else {
                self.diagnostics
                    .outbound_command_depth
                    .fetch_sub(1, Ordering::AcqRel);
                return Err(self.diagnostics.actor_stopped_error());
            };
            if command_tx
                .send(NativeClientIoCommand {
                    command,
                    write_result_tx,
                })
                .is_err()
            {
                self.diagnostics
                    .outbound_command_depth
                    .fetch_sub(1, Ordering::AcqRel);
                return Err(self.diagnostics.actor_stopped_error());
            }
            match write_result_rx.recv() {
                Ok(Ok(())) => Ok(()),
                Ok(Err(message)) => Err(NativeTransportError::ActorStopped { message }),
                Err(_) => Err(self.diagnostics.actor_stopped_error()),
            }
        }

        pub fn drain_update_batch(&mut self) -> NativeTransportResult<NativeServerUpdateBatch> {
            match self.update_rx.recv() {
                Ok(batch) => {
                    self.diagnostics.record_batch_drained(&batch);
                    Ok(batch)
                }
                Err(_) => Err(self.diagnostics.actor_stopped_error()),
            }
        }

        pub fn try_drain_update_batch(
            &mut self,
        ) -> NativeTransportResult<Option<NativeServerUpdateBatch>> {
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

        pub fn drain_command_updates(&mut self) -> NativeTransportResult<Vec<ServerUpdate>> {
            self.drain_update_batch()
                .map(NativeServerUpdateBatch::into_updates)
        }

        pub fn try_drain_command_updates(
            &mut self,
        ) -> NativeTransportResult<Option<Vec<ServerUpdate>>> {
            self.try_drain_update_batch()
                .map(|batch| batch.map(NativeServerUpdateBatch::into_updates))
        }

        pub fn send_command(
            &mut self,
            command: ClientCommand,
        ) -> NativeTransportResult<Vec<ServerUpdate>> {
            self.send_command_only(command)?;
            self.drain_command_updates()
        }

        pub fn diagnostics(&self) -> NativeClientIoDiagnostics {
            self.diagnostics.snapshot()
        }
    }

    impl Drop for NativeClientIoSession {
        fn drop(&mut self) {
            self.command_tx.take();
            let _ = self.shutdown_stream.shutdown(Shutdown::Both);
            if let Some(join_handle) = self.join_handle.take() {
                let _ = join_handle.join();
            }
        }
    }

    fn run_native_client_io_actor(
        mut stream: TcpStream,
        command_rx: mpsc::Receiver<NativeClientIoCommand>,
        update_tx: mpsc::Sender<NativeServerUpdateBatch>,
        diagnostics: Arc<NativeClientIoSharedDiagnostics>,
    ) {
        let mut response_sequence = 0_u64;
        while let Ok(command_request) = command_rx.recv() {
            diagnostics
                .outbound_command_depth
                .fetch_sub(1, Ordering::AcqRel);
            if let Err(error) = write_client_command_frame(&mut stream, &command_request.command)
                .and_then(|()| stream.flush().map_err(NativeTransportError::from))
            {
                let message = error.to_string();
                let _ = command_request.write_result_tx.send(Err(message.clone()));
                diagnostics.mark_disconnected(message);
                return;
            }
            let _ = command_request.write_result_tx.send(Ok(()));

            response_sequence = response_sequence.saturating_add(1);
            let batch = match read_server_update_batch_instrumented(&mut stream, response_sequence)
            {
                Ok(batch) => batch,
                Err(error) => {
                    diagnostics.mark_disconnected(error.to_string());
                    return;
                }
            };
            diagnostics.record_batch_queued(&batch);
            if update_tx.send(batch).is_err() {
                diagnostics.mark_disconnected("runtime update receiver closed");
                return;
            }
        }
        diagnostics.mark_disconnected("runtime command sender closed");
    }

    fn read_server_update_batch_instrumented(
        reader: &mut impl Read,
        response_sequence: u64,
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
                response_sequence,
                producer_read_ms: update_read_ms,
                producer_decode_ms: update_decode_ms,
            });
        }
        Ok(NativeServerUpdateBatch {
            updates,
            response_sequence,
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
    fn native_tcp_client_io_actor_send_does_not_wait_for_delayed_response() {
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

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        let send_start = std::time::Instant::now();
        session.send_command_only(command).unwrap();
        assert!(
            send_start.elapsed() < std::time::Duration::from_millis(100),
            "actor-backed send waited for a delayed server response"
        );
        assert_eq!(session.try_drain_update_batch().unwrap(), None);

        let batch = session.drain_update_batch().unwrap();
        assert_eq!(batch.into_updates(), expected_updates);
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
            ServerUpdate::TimeUpdate { day_time: 10 },
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(4, -3),
            },
        ];
        let second_updates = vec![ServerUpdate::TimeUpdate { day_time: 20 }];
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
        });

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        session.send_command_only(first_command).unwrap();
        session.send_command_only(second_command).unwrap();

        let first_batch = session.drain_update_batch().unwrap();
        assert_eq!(first_batch.response_sequence, 1);
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
        assert_eq!(second_batch.response_sequence, 2);
        assert_eq!(second_batch.into_updates(), second_updates);

        let diagnostics = session.diagnostics();
        assert_eq!(diagnostics.outbound_command_depth, 0);
        assert_eq!(diagnostics.inbound_response_batches, 0);
        assert_eq!(diagnostics.inbound_update_depth, 0);
        assert_eq!(diagnostics.inbound_update_bytes, 0);
        assert_eq!(diagnostics.response_sequence, 2);
        assert!(!diagnostics.disconnected);
        assert_eq!(diagnostics.last_error, None);

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
