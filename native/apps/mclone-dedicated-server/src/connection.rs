use std::fmt;
use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use mclone_net::{
    complete_server_handshake, try_read_client_command_frame, write_server_update_batch,
};
use mclone_protocol::{ClientCommand, ServerUpdate};

pub(crate) const DEDICATED_OUTBOUND_QUEUE_CAPACITY: usize = 64;
pub(crate) const DEDICATED_OUTBOUND_QUEUE_BYTE_CAPACITY: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DedicatedConnectionId(u64);

impl DedicatedConnectionId {
    pub(crate) fn next(counter: &AtomicU64) -> Self {
        Self(counter.fetch_add(1, Ordering::Relaxed))
    }
}

#[cfg(test)]
impl DedicatedConnectionId {
    pub(crate) const fn test_new(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for DedicatedConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug)]
pub(crate) enum DedicatedOutboundMessage {
    Updates {
        updates: Vec<ServerUpdate>,
        reserved_bytes: usize,
    },
    Close(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DedicatedOutboundLimits {
    frames: usize,
    bytes: usize,
}

impl Default for DedicatedOutboundLimits {
    fn default() -> Self {
        Self {
            frames: DEDICATED_OUTBOUND_QUEUE_CAPACITY,
            bytes: DEDICATED_OUTBOUND_QUEUE_BYTE_CAPACITY,
        }
    }
}

#[derive(Debug, Default)]
struct DedicatedOutboundPressure {
    queued_frames: AtomicUsize,
    queued_bytes: AtomicUsize,
    max_queued_frames: AtomicUsize,
    max_queued_bytes: AtomicUsize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DedicatedOutboundQueueMetrics {
    pub(crate) queued_frames: usize,
    pub(crate) queued_bytes: usize,
    pub(crate) max_queued_frames: usize,
    pub(crate) max_queued_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct DedicatedOutbound {
    frames: SyncSender<DedicatedOutboundMessage>,
    pressure: Arc<DedicatedOutboundPressure>,
    limits: DedicatedOutboundLimits,
}

#[derive(Debug)]
pub(crate) struct DedicatedOutboundReceiver {
    frames: Receiver<DedicatedOutboundMessage>,
    pressure: Arc<DedicatedOutboundPressure>,
}

impl DedicatedOutbound {
    pub(crate) fn channel() -> (Self, DedicatedOutboundReceiver) {
        Self::channel_with_limits(DedicatedOutboundLimits::default())
    }

    fn channel_with_limits(limits: DedicatedOutboundLimits) -> (Self, DedicatedOutboundReceiver) {
        let (frames, receiver) = mpsc::sync_channel(limits.frames);
        let pressure = Arc::new(DedicatedOutboundPressure::default());
        (
            Self {
                frames,
                pressure: Arc::clone(&pressure),
                limits,
            },
            DedicatedOutboundReceiver {
                frames: receiver,
                pressure,
            },
        )
    }

    pub(crate) fn publish(&self, updates: Vec<ServerUpdate>) -> Result<()> {
        let encoded_bytes = encoded_update_batch_len(&updates)?;
        self.reserve(encoded_bytes)?;
        self.try_send_reserved(
            DedicatedOutboundMessage::Updates {
                updates,
                reserved_bytes: encoded_bytes,
            },
            encoded_bytes,
        )
    }

    pub(crate) fn close(&self, reason: impl Into<String>) -> Result<()> {
        self.reserve(0)?;
        self.try_send_reserved(DedicatedOutboundMessage::Close(reason.into()), 0)
    }

    pub(crate) fn queue_metrics(&self) -> DedicatedOutboundQueueMetrics {
        DedicatedOutboundQueueMetrics {
            queued_frames: self.pressure.queued_frames.load(Ordering::Acquire),
            queued_bytes: self.pressure.queued_bytes.load(Ordering::Acquire),
            max_queued_frames: self.pressure.max_queued_frames.load(Ordering::Acquire),
            max_queued_bytes: self.pressure.max_queued_bytes.load(Ordering::Acquire),
        }
    }

    fn reserve(&self, encoded_bytes: usize) -> Result<()> {
        reserve_bounded(
            &self.pressure.queued_frames,
            1,
            self.limits.frames,
            "frames",
        )?;
        if let Err(error) = reserve_bounded(
            &self.pressure.queued_bytes,
            encoded_bytes,
            self.limits.bytes,
            "bytes",
        ) {
            self.pressure.queued_frames.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        atomic_max(
            &self.pressure.max_queued_frames,
            self.pressure.queued_frames.load(Ordering::Acquire),
        );
        atomic_max(
            &self.pressure.max_queued_bytes,
            self.pressure.queued_bytes.load(Ordering::Acquire),
        );
        Ok(())
    }

    fn try_send_reserved(
        &self,
        message: DedicatedOutboundMessage,
        reserved_bytes: usize,
    ) -> Result<()> {
        match self.frames.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.release(reserved_bytes);
                anyhow::bail!(
                    "dedicated outbound queue reached frame capacity {}",
                    self.limits.frames
                )
            }
            Err(TrySendError::Disconnected(_)) => {
                self.release(reserved_bytes);
                anyhow::bail!("dedicated outbound writer stopped")
            }
        }
    }

    fn release(&self, reserved_bytes: usize) {
        self.pressure.queued_frames.fetch_sub(1, Ordering::AcqRel);
        self.pressure
            .queued_bytes
            .fetch_sub(reserved_bytes, Ordering::AcqRel);
    }
}

impl DedicatedOutboundReceiver {
    fn recv(&self) -> std::result::Result<DedicatedOutboundMessage, mpsc::RecvError> {
        self.frames.recv().inspect(|message| self.release(message))
    }

    pub(crate) fn try_recv(
        &self,
    ) -> std::result::Result<DedicatedOutboundMessage, mpsc::TryRecvError> {
        self.frames
            .try_recv()
            .inspect(|message| self.release(message))
    }

    fn release(&self, message: &DedicatedOutboundMessage) {
        let reserved_bytes = match message {
            DedicatedOutboundMessage::Updates { reserved_bytes, .. } => *reserved_bytes,
            DedicatedOutboundMessage::Close(_) => 0,
        };
        self.pressure.queued_frames.fetch_sub(1, Ordering::AcqRel);
        self.pressure
            .queued_bytes
            .fetch_sub(reserved_bytes, Ordering::AcqRel);
    }
}

fn encoded_update_batch_len(updates: &[ServerUpdate]) -> Result<usize> {
    let mut bytes = 4_usize;
    for update in updates {
        let update_bytes = mclone_protocol::encode_server_update(update)
            .context("failed to measure dedicated publication update")?
            .len();
        bytes = bytes
            .checked_add(4)
            .and_then(|bytes| bytes.checked_add(update_bytes))
            .context("dedicated publication byte count overflowed")?;
    }
    Ok(bytes)
}

fn reserve_bounded(value: &AtomicUsize, delta: usize, limit: usize, unit: &str) -> Result<()> {
    let mut current = value.load(Ordering::Acquire);
    loop {
        let next = current
            .checked_add(delta)
            .context("dedicated outbound pressure counter overflowed")?;
        if next > limit {
            anyhow::bail!("dedicated outbound queue reached {unit} capacity {limit}");
        }
        match value.compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn atomic_max(value: &AtomicUsize, candidate: usize) {
    let mut current = value.load(Ordering::Acquire);
    while candidate > current {
        match value.compare_exchange(current, candidate, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

#[derive(Debug)]
pub(crate) enum DedicatedNetworkEvent {
    Connected {
        id: DedicatedConnectionId,
        peer_addr: SocketAddr,
        outbound: DedicatedOutbound,
    },
    Command {
        id: DedicatedConnectionId,
        peer_addr: SocketAddr,
        command: ClientCommand,
    },
    Disconnected {
        id: DedicatedConnectionId,
        peer_addr: SocketAddr,
        command_count: usize,
        reason: Option<String>,
    },
    AcceptFailed {
        message: String,
    },
}

#[derive(Debug)]
pub(crate) struct DedicatedNetwork {
    events: Receiver<DedicatedNetworkEvent>,
    running: Arc<AtomicBool>,
    accept_threads: Vec<JoinHandle<()>>,
}

impl DedicatedNetwork {
    pub(crate) fn start(listener: TcpListener) -> Result<Self> {
        Self::start_with_websocket(listener, None)
    }

    pub(crate) fn start_with_websocket(
        listener: TcpListener,
        websocket_listener: Option<TcpListener>,
    ) -> Result<Self> {
        listener
            .set_nonblocking(true)
            .context("failed to set dedicated server listener nonblocking")?;
        let (events_tx, events) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let accept_running = Arc::clone(&running);
        let next_id = Arc::new(AtomicU64::new(1));
        let accept_next_id = Arc::clone(&next_id);
        let tcp_events = events_tx.clone();
        let tcp_accept_thread = thread::Builder::new()
            .name("mclone-dedicated-accept".to_owned())
            .spawn(move || {
                accept_loop(listener, tcp_events, accept_running, accept_next_id);
            })
            .context("failed to spawn dedicated server accept thread")?;
        let mut accept_threads = vec![tcp_accept_thread];
        if let Some(websocket_listener) = websocket_listener {
            let websocket_events = events_tx.clone();
            let websocket_running = Arc::clone(&running);
            let websocket_next_id = Arc::clone(&next_id);
            let websocket_accept_thread = thread::Builder::new()
                .name("mclone-dedicated-ws-accept".to_owned())
                .spawn(move || {
                    crate::websocket_connection::websocket_accept_loop(
                        websocket_listener,
                        websocket_events,
                        websocket_running,
                        websocket_next_id,
                    );
                })
                .context("failed to spawn dedicated websocket accept thread")?;
            accept_threads.push(websocket_accept_thread);
        }

        Ok(Self {
            events,
            running,
            accept_threads,
        })
    }

    #[cfg(test)]
    pub(crate) fn recv(&self) -> Result<DedicatedNetworkEvent> {
        self.events
            .recv()
            .context("dedicated network event channel closed")
    }

    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<Option<DedicatedNetworkEvent>> {
        match self.events.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                anyhow::bail!("dedicated network event channel closed")
            }
        }
    }
}

impl Drop for DedicatedNetwork {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        for accept_thread in self.accept_threads.drain(..) {
            let _ = accept_thread.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    events: mpsc::Sender<DedicatedNetworkEvent>,
    running: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
) {
    while running.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer_addr)) => {
                let id = DedicatedConnectionId::next(&next_id);
                let connection_events = events.clone();
                let thread_name = format!("mclone-dedicated-conn-{}", id.0);
                if let Err(err) = thread::Builder::new().name(thread_name).spawn(move || {
                    connection_loop(id, stream, peer_addr, connection_events);
                }) {
                    let _ = events.send(DedicatedNetworkEvent::AcceptFailed {
                        message: format!("failed to spawn connection {id} for {peer_addr}: {err}"),
                    });
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(err) => {
                if events
                    .send(DedicatedNetworkEvent::AcceptFailed {
                        message: format!("failed to accept dedicated server connection: {err}"),
                    })
                    .is_err()
                {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn connection_loop(
    id: DedicatedConnectionId,
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    events: mpsc::Sender<DedicatedNetworkEvent>,
) {
    if let Err(err) = stream.set_nonblocking(false) {
        let _ = events.send(DedicatedNetworkEvent::Disconnected {
            id,
            peer_addr,
            command_count: 0,
            reason: Some(format!(
                "failed to set blocking mode for {id} {peer_addr}: {err}"
            )),
        });
        return;
    }
    if let Err(err) = stream.set_nodelay(true) {
        log::warn!("failed to set TCP_NODELAY for {id} {peer_addr}: {err}");
    }
    if let Err(err) = complete_server_handshake(&mut stream) {
        let _ = events.send(DedicatedNetworkEvent::Disconnected {
            id,
            peer_addr,
            command_count: 0,
            reason: Some(format!("failed dedicated protocol handshake: {err}")),
        });
        return;
    }
    let writer_stream = match stream.try_clone() {
        Ok(stream) => stream,
        Err(err) => {
            let _ = events.send(DedicatedNetworkEvent::Disconnected {
                id,
                peer_addr,
                command_count: 0,
                reason: Some(format!("failed to clone dedicated connection: {err}")),
            });
            return;
        }
    };
    let (outbound, outbound_rx) = DedicatedOutbound::channel();
    let writer_reason = Arc::new(Mutex::new(None));
    let writer_shared_reason = Arc::clone(&writer_reason);
    let writer_thread = match thread::Builder::new()
        .name(format!("mclone-dedicated-writer-{}", id.0))
        .spawn(move || {
            connection_writer_loop(writer_stream, outbound_rx, writer_shared_reason);
        }) {
        Ok(thread) => thread,
        Err(err) => {
            let _ = events.send(DedicatedNetworkEvent::Disconnected {
                id,
                peer_addr,
                command_count: 0,
                reason: Some(format!("failed to spawn dedicated writer: {err}")),
            });
            return;
        }
    };
    if events
        .send(DedicatedNetworkEvent::Connected {
            id,
            peer_addr,
            outbound,
        })
        .is_err()
    {
        let _ = writer_thread.join();
        return;
    }

    let mut command_count = 0;
    let mut reason = None;
    loop {
        let command = match try_read_client_command_frame(&mut stream) {
            Ok(Some(command)) => command,
            Ok(None) => break,
            Err(err) => {
                reason = Some(format!("failed to read client command: {err}"));
                break;
            }
        };

        command_count += 1;
        if events
            .send(DedicatedNetworkEvent::Command {
                id,
                peer_addr,
                command,
            })
            .is_err()
        {
            break;
        }
    }

    if let Ok(writer_reason) = writer_reason.lock()
        && writer_reason.is_some()
    {
        reason = writer_reason.clone();
    }

    let _ = events.send(DedicatedNetworkEvent::Disconnected {
        id,
        peer_addr,
        command_count,
        reason,
    });
    let _ = writer_thread.join();
}

fn connection_writer_loop(
    mut stream: TcpStream,
    outbound: DedicatedOutboundReceiver,
    reason: Arc<Mutex<Option<String>>>,
) {
    while let Ok(message) = outbound.recv() {
        match message {
            DedicatedOutboundMessage::Updates { updates, .. } => {
                if let Err(err) = write_server_update_batch(&mut stream, &updates) {
                    if let Ok(mut reason) = reason.lock() {
                        *reason = Some(format!("failed to write server update batch: {err}"));
                    }
                    break;
                }
            }
            DedicatedOutboundMessage::Close(message) => {
                if let Ok(mut reason) = reason.lock() {
                    *reason = Some(message);
                }
                break;
            }
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_net::{
        NativeClientIoSession, NativeClientSession, NativeTransportError,
        complete_client_handshake_with_version,
    };
    use mclone_protocol::{ChunkView, PROTOCOL_VERSION};

    #[test]
    fn dedicated_network_accepts_many_connections() {
        let _guard = crate::DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        const CLIENT_COUNT: usize = 8;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let network = DedicatedNetwork::start(listener).unwrap();

        let clients = (0..CLIENT_COUNT)
            .map(|index| {
                thread::spawn(move || {
                    let mut session = NativeClientSession::connect(addr).unwrap();
                    session
                        .send_command(&ClientCommand::SetChunkView(ChunkView {
                            center: ChunkPos::new(index as i32, 0),
                            render_distance: 0,
                            chunk_tracking_radius: 0,
                        }))
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();

        let mut command_events = Vec::new();
        let mut outbound = std::collections::BTreeMap::new();
        while command_events.len() < CLIENT_COUNT {
            match network.recv().unwrap() {
                DedicatedNetworkEvent::Connected {
                    id,
                    outbound: connection_outbound,
                    ..
                } => {
                    outbound.insert(id, connection_outbound);
                }
                DedicatedNetworkEvent::Command { id, command, .. } => {
                    command_events.push((id, command));
                    outbound[&id]
                        .publish(vec![ServerUpdate::TimeUpdate {
                            day_time: command_events.len() as u64,
                        }])
                        .unwrap();
                }
                DedicatedNetworkEvent::Disconnected { id, .. } => {
                    outbound.remove(&id);
                }
                event => panic!("unexpected dedicated network event: {event:?}"),
            }
        }

        for client in clients {
            assert_eq!(client.join().unwrap().len(), 1);
        }
        let ids = command_events
            .iter()
            .map(|(id, _)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), CLIENT_COUNT);
        assert!(
            command_events
                .iter()
                .all(|(_, command)| matches!(command, ClientCommand::SetChunkView(_)))
        );
    }

    #[test]
    fn dedicated_connection_reads_and_writes_independent_frames() {
        let _guard = crate::DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let network = DedicatedNetwork::start(listener).unwrap();
        let mut client = NativeClientIoSession::connect(addr).unwrap();

        let DedicatedNetworkEvent::Connected { id, outbound, .. } = network.recv().unwrap() else {
            panic!("expected connected event");
        };
        outbound
            .publish(vec![ServerUpdate::TimeUpdate { day_time: 1 }])
            .unwrap();
        assert_eq!(
            client.drain_update_batch().unwrap().into_updates(),
            vec![ServerUpdate::TimeUpdate { day_time: 1 }]
        );

        let first = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(0, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        let second = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        });
        client.send_command_only(first.clone()).unwrap();
        client.send_command_only(second.clone()).unwrap();

        let DedicatedNetworkEvent::Command {
            id: first_id,
            command: first_received,
            ..
        } = network.recv().unwrap()
        else {
            panic!("expected first command event");
        };
        let DedicatedNetworkEvent::Command {
            id: second_id,
            command: second_received,
            ..
        } = network.recv().unwrap()
        else {
            panic!("expected second command event");
        };
        assert_eq!(first_id, id);
        assert_eq!(second_id, id);
        assert_eq!(first_received, first);
        assert_eq!(second_received, second);

        outbound
            .publish(vec![ServerUpdate::TimeUpdate { day_time: 2 }])
            .unwrap();
        outbound
            .publish(vec![ServerUpdate::TimeUpdate { day_time: 3 }])
            .unwrap();
        assert_eq!(
            client.drain_update_batch().unwrap().inbound_frame_sequence,
            2
        );
        assert_eq!(
            client.drain_update_batch().unwrap().inbound_frame_sequence,
            3
        );
    }

    #[test]
    fn dedicated_outbound_pressure_is_bounded_and_nonblocking_per_peer() {
        let (slow, _held_slow_reader) =
            DedicatedOutbound::channel_with_limits(DedicatedOutboundLimits {
                frames: 3,
                bytes: 64,
            });
        let (fast, fast_reader) = DedicatedOutbound::channel_with_limits(DedicatedOutboundLimits {
            frames: 3,
            bytes: 64,
        });
        let update = vec![ServerUpdate::TimeUpdate { day_time: 1 }];
        let encoded_bytes = encoded_update_batch_len(&update).unwrap();

        let start = std::time::Instant::now();
        for _ in 0..3 {
            slow.publish(update.clone()).unwrap();
        }
        let error = slow.publish(update.clone()).unwrap_err();
        assert!(error.to_string().contains("frames capacity 3"));
        assert!(start.elapsed() < Duration::from_millis(100));
        assert_eq!(
            slow.queue_metrics(),
            DedicatedOutboundQueueMetrics {
                queued_frames: 3,
                queued_bytes: encoded_bytes * 3,
                max_queued_frames: 3,
                max_queued_bytes: encoded_bytes * 3,
            }
        );

        fast.publish(update.clone()).unwrap();
        assert!(matches!(
            fast_reader.try_recv().unwrap(),
            DedicatedOutboundMessage::Updates { updates, .. } if updates == update
        ));
        assert_eq!(fast.queue_metrics().queued_frames, 0);
        assert_eq!(fast.queue_metrics().queued_bytes, 0);
    }

    #[test]
    fn dedicated_outbound_enforces_encoded_byte_capacity() {
        let update = vec![ServerUpdate::TimeUpdate { day_time: 1 }];
        let encoded_bytes = encoded_update_batch_len(&update).unwrap();
        let (outbound, _held_reader) =
            DedicatedOutbound::channel_with_limits(DedicatedOutboundLimits {
                frames: 8,
                bytes: encoded_bytes,
            });

        outbound.publish(update.clone()).unwrap();
        let error = outbound.publish(update).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("bytes capacity {encoded_bytes}"))
        );
        let metrics = outbound.queue_metrics();
        assert_eq!(metrics.queued_frames, 1);
        assert_eq!(metrics.queued_bytes, encoded_bytes);
        assert_eq!(metrics.max_queued_bytes, encoded_bytes);
    }

    #[test]
    fn dedicated_network_rejects_protocol_mismatch_before_connecting_player() {
        let _guard = crate::DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let network = DedicatedNetwork::start(listener).unwrap();
        let mismatched_version = PROTOCOL_VERSION + 1;

        let mut stream = TcpStream::connect(addr).unwrap();
        let err =
            complete_client_handshake_with_version(&mut stream, mismatched_version).unwrap_err();
        assert!(matches!(
            err,
            NativeTransportError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received
            } if received == mismatched_version
        ));

        let event = network
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .expect("expected protocol mismatch disconnect event");
        let DedicatedNetworkEvent::Disconnected {
            command_count,
            reason,
            ..
        } = event
        else {
            panic!("expected protocol mismatch disconnect event, got {event:?}");
        };
        assert_eq!(command_count, 0);
        let reason = reason.expect("expected protocol mismatch reason");
        assert!(reason.contains("protocol version mismatch"));
        assert!(reason.contains(&format!("expected {PROTOCOL_VERSION}")));
        assert!(reason.contains(&format!("received {mismatched_version}")));
    }
}
