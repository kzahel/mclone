use std::fmt;
use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    Updates(Vec<ServerUpdate>),
    Close(String),
}

#[derive(Debug)]
pub(crate) struct DedicatedOutbound {
    frames: SyncSender<DedicatedOutboundMessage>,
}

impl DedicatedOutbound {
    pub(crate) fn channel() -> (Self, Receiver<DedicatedOutboundMessage>) {
        let (frames, receiver) = mpsc::sync_channel(DEDICATED_OUTBOUND_QUEUE_CAPACITY);
        (Self { frames }, receiver)
    }

    pub(crate) fn publish(&self, updates: Vec<ServerUpdate>) -> Result<()> {
        self.try_send(DedicatedOutboundMessage::Updates(updates))
    }

    pub(crate) fn close(&self, reason: impl Into<String>) -> Result<()> {
        self.try_send(DedicatedOutboundMessage::Close(reason.into()))
    }

    fn try_send(&self, message: DedicatedOutboundMessage) -> Result<()> {
        match self.frames.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => anyhow::bail!(
                "dedicated outbound queue reached capacity {DEDICATED_OUTBOUND_QUEUE_CAPACITY}"
            ),
            Err(TrySendError::Disconnected(_)) => {
                anyhow::bail!("dedicated outbound writer stopped")
            }
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
    outbound: Receiver<DedicatedOutboundMessage>,
    reason: Arc<Mutex<Option<String>>>,
) {
    while let Ok(message) = outbound.recv() {
        match message {
            DedicatedOutboundMessage::Updates(updates) => {
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
