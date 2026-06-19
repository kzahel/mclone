use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use mclone_net::{try_read_client_command_frame, write_server_update_batch};
use mclone_protocol::{ClientCommand, ServerUpdate};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DedicatedConnectionId(u64);

impl fmt::Display for DedicatedConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

pub(crate) type CommandResponse = std::result::Result<Vec<ServerUpdate>, String>;

#[derive(Debug)]
pub(crate) enum DedicatedNetworkEvent {
    Connected {
        id: DedicatedConnectionId,
        peer_addr: SocketAddr,
    },
    Command {
        id: DedicatedConnectionId,
        peer_addr: SocketAddr,
        command: ClientCommand,
        response: SyncSender<CommandResponse>,
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
    accept_thread: Option<JoinHandle<()>>,
}

impl DedicatedNetwork {
    pub(crate) fn start(listener: TcpListener) -> Result<Self> {
        listener
            .set_nonblocking(true)
            .context("failed to set dedicated server listener nonblocking")?;
        let (events_tx, events) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let accept_running = Arc::clone(&running);
        let next_id = Arc::new(AtomicU64::new(1));
        let accept_next_id = Arc::clone(&next_id);
        let accept_thread = thread::Builder::new()
            .name("mclone-dedicated-accept".to_owned())
            .spawn(move || {
                accept_loop(listener, events_tx, accept_running, accept_next_id);
            })
            .context("failed to spawn dedicated server accept thread")?;

        Ok(Self {
            events,
            running,
            accept_thread: Some(accept_thread),
        })
    }

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
        if let Some(accept_thread) = self.accept_thread.take() {
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
                let id = DedicatedConnectionId(next_id.fetch_add(1, Ordering::Relaxed));
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
    if events
        .send(DedicatedNetworkEvent::Connected { id, peer_addr })
        .is_err()
    {
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
        let (response_tx, response_rx) = mpsc::sync_channel(0);
        if events
            .send(DedicatedNetworkEvent::Command {
                id,
                peer_addr,
                command,
                response: response_tx,
            })
            .is_err()
        {
            break;
        }

        match response_rx.recv() {
            Ok(Ok(updates)) => {
                if let Err(err) = write_server_update_batch(&mut stream, &updates) {
                    reason = Some(format!("failed to write server update batch: {err}"));
                    break;
                }
            }
            Ok(Err(err)) => {
                reason = Some(err);
                break;
            }
            Err(err) => {
                reason = Some(format!("server command response channel closed: {err}"));
                break;
            }
        }
    }

    let _ = events.send(DedicatedNetworkEvent::Disconnected {
        id,
        peer_addr,
        command_count,
        reason,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_net::NativeClientSession;
    use mclone_protocol::ChunkView;

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
        while command_events.len() < CLIENT_COUNT {
            if let DedicatedNetworkEvent::Command {
                id,
                command,
                response,
                ..
            } = network.recv().unwrap()
            {
                command_events.push((id, command));
                response
                    .send(Ok(vec![ServerUpdate::TimeUpdate {
                        day_time: command_events.len() as u64,
                    }]))
                    .unwrap()
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
}
