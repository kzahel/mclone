use std::collections::{HashMap, HashSet, hash_map::RandomState};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mclone_protocol::{
    ClientEphemeralMessage, ProtocolCodecError, ServerEphemeralMessage,
    decode_client_ephemeral_message, decode_server_ephemeral_message,
    encode_client_ephemeral_message, encode_server_ephemeral_message,
};

const NATIVE_UDP_MAGIC: &[u8; 8] = b"MCLUDP01";
const NATIVE_UDP_ATTACH: u8 = 1;
const NATIVE_UDP_ATTACH_ACK: u8 = 2;
const NATIVE_UDP_CLIENT_MESSAGE: u8 = 3;
const NATIVE_UDP_SERVER_MESSAGE: u8 = 4;
const NATIVE_UDP_HEADER_BYTES: usize = NATIVE_UDP_MAGIC.len() + 1 + 16;
const NATIVE_UDP_POLL_INTERVAL: Duration = Duration::from_millis(4);
const NATIVE_UDP_ATTACH_RETRY: Duration = Duration::from_millis(250);
pub const NATIVE_UDP_ATTACHMENT_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_NATIVE_UDP_PACKET_BYTES: usize = 1_200;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeUdpAttachmentToken([u8; 16]);

impl NativeUdpAttachmentToken {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeUdpOffer {
    pub port: u16,
    pub token: NativeUdpAttachmentToken,
}

#[derive(Debug)]
pub struct NativeUdpAttachmentTokenGenerator {
    first: RandomState,
    second: RandomState,
    sequence: u64,
}

impl Default for NativeUdpAttachmentTokenGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeUdpAttachmentTokenGenerator {
    pub fn new() -> Self {
        Self {
            first: RandomState::new(),
            second: RandomState::new(),
            sequence: 1,
        }
    }

    pub fn generate(
        &mut self,
        session_key: u64,
        peer_addr: SocketAddr,
    ) -> NativeUdpAttachmentToken {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1).max(1);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let first = hash_token_part(&self.first, &(session_key, peer_addr, sequence, timestamp));
        let second = hash_token_part(
            &self.second,
            &(timestamp.rotate_left(47), sequence, peer_addr, session_key),
        );
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&first.to_le_bytes());
        bytes[8..].copy_from_slice(&second.to_le_bytes());
        NativeUdpAttachmentToken(bytes)
    }
}

fn hash_token_part(state: &RandomState, value: &impl Hash) -> u64 {
    let mut hasher = state.build_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug)]
pub enum NativeUdpError {
    Io(io::Error),
    Protocol(ProtocolCodecError),
    InvalidPacket(&'static str),
    PacketTooLarge(usize),
    ActorStopped,
}

impl std::fmt::Display for NativeUdpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "native UDP I/O failed: {error}"),
            Self::Protocol(error) => write!(formatter, "native UDP payload failed: {error}"),
            Self::InvalidPacket(message) => {
                write!(formatter, "invalid native UDP packet: {message}")
            }
            Self::PacketTooLarge(len) => write!(
                formatter,
                "native UDP packet length {len} exceeds {MAX_NATIVE_UDP_PACKET_BYTES} bytes"
            ),
            Self::ActorStopped => write!(formatter, "native UDP socket actor stopped"),
        }
    }
}

impl std::error::Error for NativeUdpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::InvalidPacket(_) | Self::PacketTooLarge(_) | Self::ActorStopped => None,
        }
    }
}

impl From<io::Error> for NativeUdpError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ProtocolCodecError> for NativeUdpError {
    fn from(value: ProtocolCodecError) -> Self {
        Self::Protocol(value)
    }
}

pub type NativeUdpResult<T> = Result<T, NativeUdpError>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeUdpDiagnostics {
    pub attached: bool,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub malformed_packets: u64,
    pub superseded_outbound: u64,
    pub superseded_inbound: u64,
    pub pending_inbound: usize,
}

#[derive(Debug, Default)]
struct NativeUdpSharedDiagnostics {
    attached: AtomicBool,
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    malformed_packets: AtomicU64,
    superseded_outbound: AtomicU64,
    superseded_inbound: AtomicU64,
    pending_inbound: AtomicUsize,
}

impl NativeUdpSharedDiagnostics {
    fn snapshot(&self) -> NativeUdpDiagnostics {
        NativeUdpDiagnostics {
            attached: self.attached.load(Ordering::Acquire),
            packets_sent: self.packets_sent.load(Ordering::Acquire),
            packets_received: self.packets_received.load(Ordering::Acquire),
            malformed_packets: self.malformed_packets.load(Ordering::Acquire),
            superseded_outbound: self.superseded_outbound.load(Ordering::Acquire),
            superseded_inbound: self.superseded_inbound.load(Ordering::Acquire),
            pending_inbound: self.pending_inbound.load(Ordering::Acquire),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeUdpReceivedServerMessage {
    pub message: ServerEphemeralMessage,
    pub received_at: Instant,
    pub encoded_len: usize,
}

#[derive(Debug)]
pub struct NativeUdpClient {
    running: Arc<AtomicBool>,
    diagnostics: Arc<NativeUdpSharedDiagnostics>,
    pending_outbound: Arc<Mutex<Option<ClientEphemeralMessage>>>,
    pending_inbound: Arc<Mutex<Option<NativeUdpReceivedServerMessage>>>,
    socket: UdpSocket,
    join_handle: Option<JoinHandle<()>>,
}

impl NativeUdpClient {
    pub fn connect(
        server_addr: SocketAddr,
        token: NativeUdpAttachmentToken,
    ) -> NativeUdpResult<Self> {
        Self::connect_with_timeout(server_addr, token, NATIVE_UDP_ATTACHMENT_TIMEOUT)
    }

    fn connect_with_timeout(
        server_addr: SocketAddr,
        token: NativeUdpAttachmentToken,
        attachment_timeout: Duration,
    ) -> NativeUdpResult<Self> {
        let bind_addr = if server_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(server_addr)?;
        socket.set_read_timeout(Some(NATIVE_UDP_POLL_INTERVAL))?;
        let actor_socket = socket.try_clone()?;
        let running = Arc::new(AtomicBool::new(true));
        let diagnostics = Arc::new(NativeUdpSharedDiagnostics::default());
        let pending_outbound = Arc::new(Mutex::new(None));
        let pending_inbound = Arc::new(Mutex::new(None));
        let actor_running = Arc::clone(&running);
        let actor_diagnostics = Arc::clone(&diagnostics);
        let actor_outbound = Arc::clone(&pending_outbound);
        let actor_inbound = Arc::clone(&pending_inbound);
        let join_handle = thread::Builder::new()
            .name("mclone-native-udp-client".to_owned())
            .spawn(move || {
                run_native_udp_client(
                    actor_socket,
                    token,
                    actor_running,
                    actor_diagnostics,
                    actor_outbound,
                    actor_inbound,
                    attachment_timeout,
                );
            })?;
        Ok(Self {
            running,
            diagnostics,
            pending_outbound,
            pending_inbound,
            socket,
            join_handle: Some(join_handle),
        })
    }

    pub fn is_attached(&self) -> bool {
        self.diagnostics.attached.load(Ordering::Acquire)
    }

    pub fn send_latest(&self, message: ClientEphemeralMessage) -> bool {
        if !self.is_attached() || !self.running.load(Ordering::Acquire) {
            return false;
        }
        let Ok(mut pending) = self.pending_outbound.lock() else {
            return false;
        };
        if pending.replace(message).is_some() {
            self.diagnostics
                .superseded_outbound
                .fetch_add(1, Ordering::AcqRel);
        }
        true
    }

    pub fn take_received(&self) -> Option<NativeUdpReceivedServerMessage> {
        let message = self.pending_inbound.lock().ok()?.take();
        if message.is_some() {
            self.diagnostics.pending_inbound.store(0, Ordering::Release);
        }
        message
    }

    pub fn diagnostics(&self) -> NativeUdpDiagnostics {
        self.diagnostics.snapshot()
    }
}

impl Drop for NativeUdpClient {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let _ = self.socket.send(&[]);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

fn run_native_udp_client(
    socket: UdpSocket,
    token: NativeUdpAttachmentToken,
    running: Arc<AtomicBool>,
    diagnostics: Arc<NativeUdpSharedDiagnostics>,
    pending_outbound: Arc<Mutex<Option<ClientEphemeralMessage>>>,
    pending_inbound: Arc<Mutex<Option<NativeUdpReceivedServerMessage>>>,
    attachment_timeout: Duration,
) {
    let attach_packet =
        encode_packet(NATIVE_UDP_ATTACH, token, &[]).expect("attach packet is bounded");
    let mut last_attach = Instant::now()
        .checked_sub(NATIVE_UDP_ATTACH_RETRY)
        .unwrap_or_else(Instant::now);
    let mut last_server_seen = None;
    let mut buffer = [0_u8; MAX_NATIVE_UDP_PACKET_BYTES];
    while running.load(Ordering::Acquire) {
        if !diagnostics.attached.load(Ordering::Acquire)
            && last_attach.elapsed() >= NATIVE_UDP_ATTACH_RETRY
        {
            if socket.send(&attach_packet).is_ok() {
                diagnostics.packets_sent.fetch_add(1, Ordering::AcqRel);
            }
            last_attach = Instant::now();
        }
        if diagnostics.attached.load(Ordering::Acquire) {
            if last_server_seen.is_some_and(|seen: Instant| seen.elapsed() >= attachment_timeout) {
                diagnostics.attached.store(false, Ordering::Release);
                last_attach = Instant::now();
                continue;
            }
            let message = pending_outbound
                .lock()
                .ok()
                .and_then(|mut pending| pending.take());
            if let Some(message) = message {
                match encode_client_packet(token, message)
                    .and_then(|packet| socket.send(&packet).map_err(NativeUdpError::from))
                {
                    Ok(_) => {
                        diagnostics.packets_sent.fetch_add(1, Ordering::AcqRel);
                    }
                    Err(_) => {
                        diagnostics.attached.store(false, Ordering::Release);
                    }
                }
            }
        }
        match socket.recv(&mut buffer) {
            Ok(0) => {}
            Ok(len) => {
                diagnostics.packets_received.fetch_add(1, Ordering::AcqRel);
                match decode_packet(&buffer[..len]) {
                    Ok(NativeUdpPacket::AttachAck(received)) if received == token => {
                        diagnostics.attached.store(true, Ordering::Release);
                        last_server_seen = Some(Instant::now());
                    }
                    Ok(NativeUdpPacket::ServerMessage(received, message)) if received == token => {
                        last_server_seen = Some(Instant::now());
                        let received = NativeUdpReceivedServerMessage {
                            message,
                            received_at: Instant::now(),
                            encoded_len: len.saturating_sub(NATIVE_UDP_HEADER_BYTES),
                        };
                        if let Ok(mut pending) = pending_inbound.lock() {
                            if pending.replace(received).is_some() {
                                diagnostics
                                    .superseded_inbound
                                    .fetch_add(1, Ordering::AcqRel);
                            }
                            diagnostics.pending_inbound.store(1, Ordering::Release);
                        }
                    }
                    Ok(_) | Err(_) => {
                        diagnostics.malformed_packets.fetch_add(1, Ordering::AcqRel);
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                diagnostics.attached.store(false, Ordering::Release);
            }
        }
    }
    diagnostics.attached.store(false, Ordering::Release);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeUdpServerEvent {
    pub session_key: u64,
    pub peer_addr: SocketAddr,
    pub message: ClientEphemeralMessage,
}

#[derive(Debug)]
enum NativeUdpServerControl {
    Register {
        session_key: u64,
        token: NativeUdpAttachmentToken,
    },
    Unregister {
        session_key: u64,
    },
}

#[derive(Clone, Debug)]
pub struct NativeUdpServerHandle {
    control_tx: Sender<NativeUdpServerControl>,
    pending_outbound: Arc<Mutex<HashMap<u64, ServerEphemeralMessage>>>,
    attached: Arc<Mutex<HashSet<u64>>>,
    diagnostics: Arc<NativeUdpSharedDiagnostics>,
}

impl NativeUdpServerHandle {
    pub fn register(
        &self,
        session_key: u64,
        token: NativeUdpAttachmentToken,
    ) -> NativeUdpResult<()> {
        self.control_tx
            .send(NativeUdpServerControl::Register { session_key, token })
            .map_err(|_| NativeUdpError::ActorStopped)
    }

    pub fn unregister(&self, session_key: u64) {
        let _ = self
            .control_tx
            .send(NativeUdpServerControl::Unregister { session_key });
    }

    pub fn is_attached(&self, session_key: u64) -> bool {
        self.attached
            .lock()
            .is_ok_and(|attached| attached.contains(&session_key))
    }

    pub fn send_latest(&self, session_key: u64, message: ServerEphemeralMessage) -> bool {
        if !self.is_attached(session_key) {
            return false;
        }
        let Ok(mut pending) = self.pending_outbound.lock() else {
            return false;
        };
        if pending.insert(session_key, message).is_some() {
            self.diagnostics
                .superseded_outbound
                .fetch_add(1, Ordering::AcqRel);
        }
        true
    }

    pub fn diagnostics(&self) -> NativeUdpDiagnostics {
        self.diagnostics.snapshot()
    }
}

#[derive(Debug)]
pub struct NativeUdpServer {
    local_addr: SocketAddr,
    handle: NativeUdpServerHandle,
    pending_inbound: Arc<Mutex<HashMap<u64, NativeUdpServerEvent>>>,
    running: Arc<AtomicBool>,
    socket: UdpSocket,
    join_handle: Option<JoinHandle<()>>,
}

impl NativeUdpServer {
    pub fn bind(addr: SocketAddr) -> NativeUdpResult<Self> {
        Self::bind_with_timeout(addr, NATIVE_UDP_ATTACHMENT_TIMEOUT)
    }

    fn bind_with_timeout(addr: SocketAddr, attachment_timeout: Duration) -> NativeUdpResult<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_read_timeout(Some(NATIVE_UDP_POLL_INTERVAL))?;
        let local_addr = socket.local_addr()?;
        let actor_socket = socket.try_clone()?;
        let (control_tx, control_rx) = mpsc::channel();
        let pending_outbound = Arc::new(Mutex::new(HashMap::new()));
        let pending_inbound = Arc::new(Mutex::new(HashMap::new()));
        let attached = Arc::new(Mutex::new(HashSet::new()));
        let diagnostics = Arc::new(NativeUdpSharedDiagnostics::default());
        let running = Arc::new(AtomicBool::new(true));
        let actor_outbound = Arc::clone(&pending_outbound);
        let actor_inbound = Arc::clone(&pending_inbound);
        let actor_attached = Arc::clone(&attached);
        let actor_diagnostics = Arc::clone(&diagnostics);
        let actor_running = Arc::clone(&running);
        let join_handle = thread::Builder::new()
            .name("mclone-native-udp-server".to_owned())
            .spawn(move || {
                run_native_udp_server(
                    actor_socket,
                    control_rx,
                    actor_outbound,
                    actor_inbound,
                    actor_attached,
                    actor_diagnostics,
                    actor_running,
                    attachment_timeout,
                );
            })?;
        let handle = NativeUdpServerHandle {
            control_tx,
            pending_outbound,
            attached,
            diagnostics,
        };
        Ok(Self {
            local_addr,
            handle,
            pending_inbound,
            running,
            socket,
            join_handle: Some(join_handle),
        })
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn handle(&self) -> NativeUdpServerHandle {
        self.handle.clone()
    }

    pub fn try_recv(&self) -> NativeUdpResult<Option<NativeUdpServerEvent>> {
        let mut pending = self
            .pending_inbound
            .lock()
            .map_err(|_| NativeUdpError::ActorStopped)?;
        let event = pending
            .keys()
            .next()
            .copied()
            .and_then(|session_key| pending.remove(&session_key));
        self.handle
            .diagnostics
            .pending_inbound
            .store(pending.len(), Ordering::Release);
        Ok(event)
    }
}

impl Drop for NativeUdpServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let _ = self.socket.send_to(&[], self.local_addr);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ServerAttachment {
    session_key: u64,
    peer_addr: Option<SocketAddr>,
    last_seen: Instant,
}

fn run_native_udp_server(
    socket: UdpSocket,
    control_rx: Receiver<NativeUdpServerControl>,
    pending_outbound: Arc<Mutex<HashMap<u64, ServerEphemeralMessage>>>,
    pending_inbound: Arc<Mutex<HashMap<u64, NativeUdpServerEvent>>>,
    attached_shared: Arc<Mutex<HashSet<u64>>>,
    diagnostics: Arc<NativeUdpSharedDiagnostics>,
    running: Arc<AtomicBool>,
    attachment_timeout: Duration,
) {
    let mut by_token = HashMap::<NativeUdpAttachmentToken, ServerAttachment>::new();
    let mut token_by_session = HashMap::<u64, NativeUdpAttachmentToken>::new();
    let mut buffer = [0_u8; MAX_NATIVE_UDP_PACKET_BYTES];
    while running.load(Ordering::Acquire) {
        while let Ok(control) = control_rx.try_recv() {
            match control {
                NativeUdpServerControl::Register { session_key, token } => {
                    if let Some(previous) = token_by_session.insert(session_key, token) {
                        by_token.remove(&previous);
                    }
                    by_token.insert(
                        token,
                        ServerAttachment {
                            session_key,
                            peer_addr: None,
                            last_seen: Instant::now(),
                        },
                    );
                }
                NativeUdpServerControl::Unregister { session_key } => {
                    if let Some(token) = token_by_session.remove(&session_key) {
                        by_token.remove(&token);
                    }
                    if let Ok(mut attached) = attached_shared.lock() {
                        attached.remove(&session_key);
                    }
                    if let Ok(mut pending) = pending_outbound.lock() {
                        pending.remove(&session_key);
                    }
                }
            }
        }

        let outbound = pending_outbound
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default();
        for (session_key, message) in outbound {
            let Some(token) = token_by_session.get(&session_key).copied() else {
                continue;
            };
            let Some(peer_addr) = by_token.get(&token).and_then(|state| state.peer_addr) else {
                continue;
            };
            match encode_server_packet(token, message).and_then(|packet| {
                socket
                    .send_to(&packet, peer_addr)
                    .map_err(NativeUdpError::from)
            }) {
                Ok(_) => {
                    diagnostics.packets_sent.fetch_add(1, Ordering::AcqRel);
                }
                Err(_) => {
                    detach_session(
                        session_key,
                        &mut by_token,
                        &token_by_session,
                        &attached_shared,
                    );
                }
            }
        }

        match socket.recv_from(&mut buffer) {
            Ok((0, _)) => {}
            Ok((len, peer_addr)) => {
                diagnostics.packets_received.fetch_add(1, Ordering::AcqRel);
                match decode_packet(&buffer[..len]) {
                    Ok(NativeUdpPacket::Attach(token)) => {
                        let Some(state) = by_token.get_mut(&token) else {
                            diagnostics.malformed_packets.fetch_add(1, Ordering::AcqRel);
                            continue;
                        };
                        state.peer_addr = Some(peer_addr);
                        state.last_seen = Instant::now();
                        if let Ok(mut attached) = attached_shared.lock() {
                            attached.insert(state.session_key);
                        }
                        let packet = encode_packet(NATIVE_UDP_ATTACH_ACK, token, &[])
                            .expect("attach acknowledgement is bounded");
                        if socket.send_to(&packet, peer_addr).is_ok() {
                            diagnostics.packets_sent.fetch_add(1, Ordering::AcqRel);
                        }
                    }
                    Ok(NativeUdpPacket::ClientMessage(token, message)) => {
                        let Some(state) = by_token.get_mut(&token) else {
                            diagnostics.malformed_packets.fetch_add(1, Ordering::AcqRel);
                            continue;
                        };
                        if state.peer_addr != Some(peer_addr) {
                            diagnostics.malformed_packets.fetch_add(1, Ordering::AcqRel);
                            continue;
                        }
                        state.last_seen = Instant::now();
                        let acknowledgement = encode_packet(NATIVE_UDP_ATTACH_ACK, token, &[])
                            .expect("attachment acknowledgement is bounded");
                        if socket.send_to(&acknowledgement, peer_addr).is_ok() {
                            diagnostics.packets_sent.fetch_add(1, Ordering::AcqRel);
                        }
                        if let Ok(mut pending) = pending_inbound.lock() {
                            if pending
                                .insert(
                                    state.session_key,
                                    NativeUdpServerEvent {
                                        session_key: state.session_key,
                                        peer_addr,
                                        message,
                                    },
                                )
                                .is_some()
                            {
                                diagnostics
                                    .superseded_inbound
                                    .fetch_add(1, Ordering::AcqRel);
                            }
                            diagnostics
                                .pending_inbound
                                .store(pending.len(), Ordering::Release);
                        } else {
                            diagnostics
                                .superseded_inbound
                                .fetch_add(1, Ordering::AcqRel);
                        }
                    }
                    Ok(_) | Err(_) => {
                        diagnostics.malformed_packets.fetch_add(1, Ordering::AcqRel);
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {}
        }

        let expired = by_token
            .iter()
            .filter_map(|(token, state)| {
                (state.peer_addr.is_some() && state.last_seen.elapsed() >= attachment_timeout)
                    .then_some((*token, state.session_key))
            })
            .collect::<Vec<_>>();
        for (token, session_key) in expired {
            if let Some(state) = by_token.get_mut(&token) {
                state.peer_addr = None;
            }
            if let Ok(mut attached) = attached_shared.lock() {
                attached.remove(&session_key);
            }
        }
        diagnostics.attached.store(
            attached_shared
                .lock()
                .is_ok_and(|attached| !attached.is_empty()),
            Ordering::Release,
        );
    }
}

fn detach_session(
    session_key: u64,
    by_token: &mut HashMap<NativeUdpAttachmentToken, ServerAttachment>,
    token_by_session: &HashMap<u64, NativeUdpAttachmentToken>,
    attached_shared: &Mutex<HashSet<u64>>,
) {
    if let Some(token) = token_by_session.get(&session_key)
        && let Some(state) = by_token.get_mut(token)
    {
        state.peer_addr = None;
    }
    if let Ok(mut attached) = attached_shared.lock() {
        attached.remove(&session_key);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NativeUdpPacket {
    Attach(NativeUdpAttachmentToken),
    AttachAck(NativeUdpAttachmentToken),
    ClientMessage(NativeUdpAttachmentToken, ClientEphemeralMessage),
    ServerMessage(NativeUdpAttachmentToken, ServerEphemeralMessage),
}

fn encode_client_packet(
    token: NativeUdpAttachmentToken,
    message: ClientEphemeralMessage,
) -> NativeUdpResult<Vec<u8>> {
    encode_packet(
        NATIVE_UDP_CLIENT_MESSAGE,
        token,
        &encode_client_ephemeral_message(message)?,
    )
}

fn encode_server_packet(
    token: NativeUdpAttachmentToken,
    message: ServerEphemeralMessage,
) -> NativeUdpResult<Vec<u8>> {
    encode_packet(
        NATIVE_UDP_SERVER_MESSAGE,
        token,
        &encode_server_ephemeral_message(message)?,
    )
}

fn encode_packet(
    tag: u8,
    token: NativeUdpAttachmentToken,
    payload: &[u8],
) -> NativeUdpResult<Vec<u8>> {
    let len = NATIVE_UDP_HEADER_BYTES.saturating_add(payload.len());
    if len > MAX_NATIVE_UDP_PACKET_BYTES {
        return Err(NativeUdpError::PacketTooLarge(len));
    }
    let mut packet = Vec::with_capacity(len);
    packet.extend_from_slice(NATIVE_UDP_MAGIC);
    packet.push(tag);
    packet.extend_from_slice(&token.bytes());
    packet.extend_from_slice(payload);
    Ok(packet)
}

fn decode_packet(bytes: &[u8]) -> NativeUdpResult<NativeUdpPacket> {
    if bytes.len() > MAX_NATIVE_UDP_PACKET_BYTES {
        return Err(NativeUdpError::PacketTooLarge(bytes.len()));
    }
    if bytes.len() < NATIVE_UDP_HEADER_BYTES {
        return Err(NativeUdpError::InvalidPacket(
            "packet was shorter than its header",
        ));
    }
    if !bytes.starts_with(NATIVE_UDP_MAGIC) {
        return Err(NativeUdpError::InvalidPacket("packet magic did not match"));
    }
    let tag = bytes[NATIVE_UDP_MAGIC.len()];
    let token_offset = NATIVE_UDP_MAGIC.len() + 1;
    let token = NativeUdpAttachmentToken::from_bytes(
        bytes[token_offset..token_offset + 16]
            .try_into()
            .expect("native UDP token length checked"),
    );
    let payload = &bytes[NATIVE_UDP_HEADER_BYTES..];
    match tag {
        NATIVE_UDP_ATTACH if payload.is_empty() => Ok(NativeUdpPacket::Attach(token)),
        NATIVE_UDP_ATTACH_ACK if payload.is_empty() => Ok(NativeUdpPacket::AttachAck(token)),
        NATIVE_UDP_CLIENT_MESSAGE => Ok(NativeUdpPacket::ClientMessage(
            token,
            decode_client_ephemeral_message(payload)?,
        )),
        NATIVE_UDP_SERVER_MESSAGE => Ok(NativeUdpPacket::ServerMessage(
            token,
            decode_server_ephemeral_message(payload)?,
        )),
        NATIVE_UDP_ATTACH | NATIVE_UDP_ATTACH_ACK => Err(NativeUdpError::InvalidPacket(
            "attach packet carried a payload",
        )),
        _ => Err(NativeUdpError::InvalidPacket("packet tag was unknown")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Vec3d;
    use mclone_protocol::{PlayerBodyPoseSample, RemotePlayerBodyPoseSample, RemotePlayerId};

    fn client_message(sequence: u32) -> ClientEphemeralMessage {
        ClientEphemeralMessage::BodyPose(PlayerBodyPoseSample::new(
            1,
            sequence,
            sequence * 10,
            Vec3d::new(f64::from(sequence), 64.0, -2.0),
            45.0,
            -10.0,
            true,
        ))
    }

    fn server_message(sequence: u32) -> ServerEphemeralMessage {
        ServerEphemeralMessage::RemoteBodyPose(RemotePlayerBodyPoseSample {
            id: RemotePlayerId(7),
            pose: match client_message(sequence) {
                ClientEphemeralMessage::BodyPose(pose) => pose,
            },
        })
    }

    #[test]
    fn packets_round_trip_and_remain_below_the_datagram_budget() {
        let token = NativeUdpAttachmentToken::from_bytes([7; 16]);
        let client = encode_client_packet(token, client_message(3)).unwrap();
        let server = encode_server_packet(token, server_message(4)).unwrap();
        assert!(client.len() < MAX_NATIVE_UDP_PACKET_BYTES);
        assert!(server.len() < MAX_NATIVE_UDP_PACKET_BYTES);
        assert_eq!(
            decode_packet(&client).unwrap(),
            NativeUdpPacket::ClientMessage(token, client_message(3))
        );
        assert_eq!(
            decode_packet(&server).unwrap(),
            NativeUdpPacket::ServerMessage(token, server_message(4))
        );
    }

    #[test]
    fn packet_decoder_rejects_wrong_magic_tokenless_and_trailing_attach_data() {
        let token = NativeUdpAttachmentToken::from_bytes([3; 16]);
        let mut wrong_magic = encode_packet(NATIVE_UDP_ATTACH, token, &[]).unwrap();
        wrong_magic[0] ^= 0xff;
        assert!(decode_packet(&wrong_magic).is_err());
        assert!(decode_packet(b"short").is_err());
        let trailing = encode_packet(NATIVE_UDP_ATTACH, token, &[1]).unwrap();
        assert!(decode_packet(&trailing).is_err());
    }

    #[test]
    fn loopback_actor_attaches_and_relays_both_directions() {
        let server = NativeUdpServer::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let handle = server.handle();
        let token = NativeUdpAttachmentToken::from_bytes([9; 16]);
        handle.register(12, token).unwrap();
        let client = NativeUdpClient::connect(server.local_addr(), token).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !client.is_attached() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(client.is_attached());
        assert!(handle.is_attached(12));

        assert!(client.send_latest(client_message(5)));
        let received = loop {
            if let Some(event) = server.try_recv().unwrap() {
                break event;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for client datagram"
            );
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(received.session_key, 12);
        assert_eq!(received.message, client_message(5));

        assert!(handle.send_latest(12, server_message(6)));
        let received = loop {
            if let Some(received) = client.take_received() {
                break received;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for server datagram"
            );
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(received.message, server_message(6));
    }

    #[test]
    fn wrong_token_cannot_attach_or_inject() {
        let server = NativeUdpServer::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let handle = server.handle();
        handle
            .register(1, NativeUdpAttachmentToken::from_bytes([1; 16]))
            .unwrap();
        let wrong = NativeUdpClient::connect(
            server.local_addr(),
            NativeUdpAttachmentToken::from_bytes([2; 16]),
        )
        .unwrap();
        thread::sleep(Duration::from_millis(300));
        assert!(!wrong.is_attached());
        assert!(!handle.is_attached(1));
        assert!(server.try_recv().unwrap().is_none());
    }

    #[test]
    fn latest_wins_queues_are_bounded_to_one_message_per_session() {
        let server = NativeUdpServer::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let handle = server.handle();
        let token = NativeUdpAttachmentToken::from_bytes([4; 16]);
        handle.register(2, token).unwrap();
        let client = NativeUdpClient::connect(server.local_addr(), token).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !client.is_attached() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        for sequence in 1..=1_000 {
            assert!(client.send_latest(client_message(sequence)));
            assert!(handle.send_latest(2, server_message(sequence)));
        }
        assert!(client.diagnostics().superseded_outbound > 0);
        assert!(handle.diagnostics().superseded_outbound > 0);
    }

    #[test]
    fn attachment_timeout_detaches_both_ends_and_permits_reattachment() {
        let timeout = Duration::from_millis(60);
        let server =
            NativeUdpServer::bind_with_timeout("127.0.0.1:0".parse().unwrap(), timeout).unwrap();
        let handle = server.handle();
        let token = NativeUdpAttachmentToken::from_bytes([5; 16]);
        handle.register(3, token).unwrap();
        let client =
            NativeUdpClient::connect_with_timeout(server.local_addr(), token, timeout).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !client.is_attached() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(client.is_attached());
        assert!(handle.is_attached(3));

        thread::sleep(timeout + Duration::from_millis(25));
        let detach_deadline = Instant::now() + Duration::from_secs(1);
        while (client.is_attached() || handle.is_attached(3)) && Instant::now() < detach_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!client.is_attached());
        assert!(!handle.is_attached(3));

        let reattach_deadline = Instant::now() + Duration::from_secs(1);
        while !client.is_attached() && Instant::now() < reattach_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(client.is_attached());
        assert!(handle.is_attached(3));
    }
}
