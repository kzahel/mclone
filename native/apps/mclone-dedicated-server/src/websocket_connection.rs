use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use mclone_net::{
    decode_websocket_client_command, decode_websocket_client_handshake,
    encode_websocket_server_handshake_accept_with_capabilities,
    encode_websocket_server_handshake_reject, encode_websocket_server_update_batch,
};
use mclone_protocol::{PROTOCOL_VERSION, ServerUpdate, SessionCapabilities};
use tungstenite::{Error as WebSocketError, Message, accept};

use crate::connection::{
    DedicatedConnectionId, DedicatedNetworkEvent, DedicatedOutbound, DedicatedOutboundMessage,
    DedicatedOutboundReceiver,
};

pub(crate) fn websocket_accept_loop(
    listener: TcpListener,
    events: Sender<DedicatedNetworkEvent>,
    running: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
) {
    if let Err(err) = listener.set_nonblocking(true) {
        let _ = events.send(DedicatedNetworkEvent::AcceptFailed {
            message: format!("failed to set dedicated websocket listener nonblocking: {err}"),
        });
        return;
    }
    while running.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer_addr)) => {
                let id = DedicatedConnectionId::next(&next_id);
                let connection_events = events.clone();
                let thread_name = format!("mclone-dedicated-ws-conn-{id}");
                if let Err(err) = thread::Builder::new().name(thread_name).spawn(move || {
                    websocket_connection_loop(id, stream, peer_addr, connection_events);
                }) {
                    let _ = events.send(DedicatedNetworkEvent::AcceptFailed {
                        message: format!(
                            "failed to spawn websocket connection {id} for {peer_addr}: {err}"
                        ),
                    });
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(err) => {
                if events
                    .send(DedicatedNetworkEvent::AcceptFailed {
                        message: format!("failed to accept dedicated websocket connection: {err}"),
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

fn websocket_connection_loop(
    id: DedicatedConnectionId,
    stream: TcpStream,
    peer_addr: SocketAddr,
    events: Sender<DedicatedNetworkEvent>,
) {
    if let Err(err) = stream.set_nonblocking(false) {
        send_disconnected(
            &events,
            id,
            peer_addr,
            0,
            Some(format!(
                "failed to make websocket handshake blocking: {err}"
            )),
        );
        return;
    }
    let mut websocket = match accept(stream) {
        Ok(websocket) => websocket,
        Err(err) => {
            send_disconnected(
                &events,
                id,
                peer_addr,
                0,
                Some(format!("failed websocket transport handshake: {err}")),
            );
            return;
        }
    };
    let accepted = match complete_websocket_protocol_handshake(&mut websocket) {
        Ok(accepted) => accepted,
        Err(err) => {
            send_disconnected(
                &events,
                id,
                peer_addr,
                0,
                Some(format!("failed websocket protocol handshake: {err:#}")),
            );
            return;
        }
    };
    if let Err(err) = websocket.get_mut().set_nonblocking(true) {
        send_disconnected(
            &events,
            id,
            peer_addr,
            0,
            Some(format!(
                "failed to make websocket connection nonblocking: {err}"
            )),
        );
        return;
    }

    let (outbound, outbound_rx) = DedicatedOutbound::channel();
    if events
        .send(DedicatedNetworkEvent::Connected {
            id,
            peer_addr,
            identity: accepted.identity,
            capabilities: accepted.capabilities,
            outbound,
        })
        .is_err()
    {
        return;
    }

    let mut command_count = 0_usize;
    let mut reason = None;
    let mut outbound_state = WebSocketOutboundState::default();
    'connected: loop {
        if let Err(err) = flush_outbound(&mut websocket, &outbound_rx, &mut outbound_state) {
            reason = Some(format!("failed to write websocket server updates: {err:#}"));
            break;
        }
        loop {
            let message = match websocket.read() {
                Ok(message) => message,
                Err(WebSocketError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                    break 'connected;
                }
                Err(err) => {
                    reason = Some(format!("failed to read websocket client command: {err}"));
                    break 'connected;
                }
            };
            let payload = match message {
                Message::Binary(payload) => payload,
                Message::Close(_) => break 'connected,
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Text(_) => {
                    reason = Some("websocket client sent text data".to_owned());
                    break 'connected;
                }
            };
            let command = match decode_websocket_client_command(&payload) {
                Ok(command) => command,
                Err(err) => {
                    reason = Some(format!(
                        "failed to decode websocket client command: {err:#}"
                    ));
                    break 'connected;
                }
            };
            command_count = command_count.saturating_add(1);
            if events
                .send(DedicatedNetworkEvent::Command {
                    id,
                    peer_addr,
                    command,
                })
                .is_err()
            {
                break 'connected;
            }
        }
        thread::sleep(Duration::from_millis(1));
    }

    send_disconnected(&events, id, peer_addr, command_count, reason);
    let _ = websocket.close(None);
}

#[derive(Debug, Default)]
struct WebSocketOutboundState {
    flush_pending: bool,
    close_after_flush: Option<String>,
}

fn flush_outbound<S>(
    websocket: &mut tungstenite::WebSocket<S>,
    outbound: &DedicatedOutboundReceiver,
    state: &mut WebSocketOutboundState,
) -> Result<()>
where
    S: Read + Write,
{
    loop {
        if state.flush_pending {
            match websocket.flush() {
                Ok(()) => {
                    state.flush_pending = false;
                    if let Some(detail) = state.close_after_flush.take() {
                        bail!(detail);
                    }
                }
                Err(WebSocketError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(());
                }
                Err(error) => {
                    return Err(error).context("failed to flush websocket server update batch");
                }
            }
        }

        let (message, close_after_flush) = match outbound.try_recv() {
            Ok(DedicatedOutboundMessage::Updates { updates, .. }) => {
                let frame = encode_websocket_server_update_batch(&updates)
                    .context("failed to encode websocket server update batch")?;
                (Message::Binary(frame.into()), None)
            }
            Ok(DedicatedOutboundMessage::Close(reason)) => {
                let frame = encode_websocket_server_update_batch(&[ServerUpdate::Disconnect(
                    reason.clone(),
                )])
                .context("failed to encode websocket disconnect update")?;
                (Message::Binary(frame.into()), Some(reason.detail))
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => bail!("dedicated websocket publisher stopped"),
        };
        match websocket.send(message) {
            Ok(()) => {
                if let Some(detail) = close_after_flush {
                    bail!(detail);
                }
            }
            Err(WebSocketError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {
                // Tungstenite guarantees that a frame remains queued after a
                // stream write failure unless it returns WriteBufferFull. A
                // nonblocking socket reaching kernel backpressure therefore
                // needs a later flush, not a connection teardown.
                state.flush_pending = true;
                state.close_after_flush = close_after_flush;
                return Ok(());
            }
            Err(error) => {
                return Err(error).context("failed to send websocket server update batch");
            }
        }
    }
}

fn send_disconnected(
    events: &Sender<DedicatedNetworkEvent>,
    id: DedicatedConnectionId,
    peer_addr: SocketAddr,
    command_count: usize,
    reason: Option<String>,
) {
    let _ = events.send(DedicatedNetworkEvent::Disconnected {
        id,
        peer_addr,
        command_count,
        reason,
    });
}

fn complete_websocket_protocol_handshake(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
) -> Result<mclone_net::AcceptedClientHandshake> {
    let message = websocket
        .read()
        .context("failed to read websocket protocol handshake")?;
    let payload = match message {
        Message::Binary(payload) => payload,
        Message::Close(_) => bail!("websocket closed before protocol handshake"),
        _ => bail!("websocket protocol handshake must be a binary message"),
    };
    let received = decode_websocket_client_handshake(&payload)
        .context("failed to decode websocket protocol handshake")?;
    if received.protocol_version != PROTOCOL_VERSION {
        let response =
            encode_websocket_server_handshake_reject(PROTOCOL_VERSION, received.protocol_version)
                .context("failed to encode websocket protocol rejection")?;
        let _ = websocket.send(Message::Binary(response.into()));
        bail!(
            "websocket protocol version mismatch: expected {}, received {}",
            PROTOCOL_VERSION,
            received.protocol_version
        );
    }

    let capabilities = received
        .capabilities
        .intersection(SessionCapabilities::DEVELOPMENT_DEFAULT)
        .known();
    let response =
        encode_websocket_server_handshake_accept_with_capabilities(PROTOCOL_VERSION, capabilities)
            .context("failed to encode websocket protocol acceptance")?;
    websocket
        .send(Message::Binary(response.into()))
        .context("failed to send websocket protocol acceptance")?;
    Ok(mclone_net::AcceptedClientHandshake {
        identity: received.identity,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    use mclone_core::ChunkPos;
    use mclone_net::{
        NativeClientIoSession, decode_websocket_server_handshake,
        decode_websocket_server_update_batch, encode_current_websocket_client_handshake,
        encode_websocket_client_command,
    };
    use mclone_protocol::{ChunkView, ClientCommand, DimensionKey, ServerUpdate};
    use mclone_server::LocalRealmSession;

    fn time_update(day_time: u64) -> ServerUpdate {
        ServerUpdate::TimeUpdate {
            game_time: day_time.saturating_add(100),
            day_time,
            daylight_cycle_running: true,
        }
    }

    #[derive(Debug, Default)]
    struct WouldBlockOnceStream {
        blocked_write: bool,
        written: Vec<u8>,
    }

    impl Read for WouldBlockOnceStream {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::WouldBlock))
        }
    }

    impl Write for WouldBlockOnceStream {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if !self.blocked_write {
                self.blocked_write = true;
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            self.written.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn websocket_outbound_retries_buffered_frame_after_would_block() {
        let mut websocket = tungstenite::WebSocket::from_raw_socket(
            WouldBlockOnceStream::default(),
            tungstenite::protocol::Role::Server,
            None,
        );
        let (outbound, outbound_rx) = DedicatedOutbound::channel();
        outbound.publish(vec![time_update(123)]).unwrap();
        let mut state = WebSocketOutboundState::default();

        flush_outbound(&mut websocket, &outbound_rx, &mut state).unwrap();
        assert!(state.flush_pending);
        assert!(websocket.get_ref().written.is_empty());

        flush_outbound(&mut websocket, &outbound_rx, &mut state).unwrap();
        assert!(!state.flush_pending);
        assert!(!websocket.get_ref().written.is_empty());
        assert_eq!(outbound.queue_metrics().queued_frames, 0);
    }

    #[test]
    fn websocket_peer_receives_unsolicited_updates_and_sends_commands() {
        let native_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let websocket_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let websocket_addr = websocket_listener.local_addr().unwrap();
        let network = crate::connection::DedicatedNetwork::start_with_websocket(
            native_listener,
            Some(websocket_listener),
        )
        .unwrap();

        let mut socket = tungstenite::connect(format!("ws://{websocket_addr}"))
            .unwrap()
            .0;
        socket
            .send(Message::Binary(
                encode_current_websocket_client_handshake().unwrap().into(),
            ))
            .unwrap();
        let handshake = socket.read().unwrap().into_data();
        decode_websocket_server_handshake(&handshake, PROTOCOL_VERSION).unwrap();

        let outbound = match network.recv().unwrap() {
            DedicatedNetworkEvent::Connected { outbound, .. } => outbound,
            event => panic!("expected websocket connection, got {event:?}"),
        };
        let expected_updates = vec![
            ServerUpdate::DimensionChange {
                dimension: DimensionKey::parse("mclone:moon").unwrap(),
                biome_zoom_seed: 54_321,
                topology: mclone_core::HorizontalTopology::UNBOUNDED,
                keep_player_state: true,
            },
            time_update(123),
        ];
        outbound.publish(expected_updates.clone()).unwrap();
        let publication = socket.read().unwrap().into_data();
        assert_eq!(
            decode_websocket_server_update_batch(&publication).unwrap(),
            expected_updates
        );

        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(2, -3),
            render_distance: 1,
            chunk_tracking_radius: 1,
        });
        socket
            .send(Message::Binary(
                encode_websocket_client_command(&command).unwrap().into(),
            ))
            .unwrap();
        match network.recv().unwrap() {
            DedicatedNetworkEvent::Command {
                command: received, ..
            } => assert_eq!(received, command),
            event => panic!("expected websocket command, got {event:?}"),
        }
        socket.close(None).unwrap();
    }

    #[test]
    fn cylinder_local_tcp_and_websocket_adapters_preserve_one_logical_trace() {
        let mut definition = mclone_server::DimensionDefinition::overworld(
            12_345,
            mclone_server::WorldGenerationProfile::FlatGrassV1,
        );
        definition.topology = mclone_core::HorizontalTopology::cylinder_x(0, 32);
        let mut local = LocalRealmSession::local_integrated_with_dimension_definition(definition);
        let join_trace = local.try_drain_updates().unwrap();
        assert!(join_trace.iter().any(|update| matches!(
            update,
            ServerUpdate::WorldInfo { topology, .. }
                if *topology == mclone_core::HorizontalTopology::cylinder_x(0, 32)
        )));

        let native_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let native_addr = native_listener.local_addr().unwrap();
        let websocket_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let websocket_addr = websocket_listener.local_addr().unwrap();
        let network = crate::connection::DedicatedNetwork::start_with_websocket(
            native_listener,
            Some(websocket_listener),
        )
        .unwrap();

        let mut native = NativeClientIoSession::connect(native_addr).unwrap();
        let native_outbound = match network.recv().unwrap() {
            DedicatedNetworkEvent::Connected { outbound, .. } => outbound,
            event => panic!("expected native connection, got {event:?}"),
        };

        let mut websocket = tungstenite::connect(format!("ws://{websocket_addr}"))
            .unwrap()
            .0;
        websocket
            .send(Message::Binary(
                encode_current_websocket_client_handshake().unwrap().into(),
            ))
            .unwrap();
        let handshake = websocket.read().unwrap().into_data();
        decode_websocket_server_handshake(&handshake, PROTOCOL_VERSION).unwrap();
        let websocket_outbound = match network.recv().unwrap() {
            DedicatedNetworkEvent::Connected { outbound, .. } => outbound,
            event => panic!("expected websocket connection, got {event:?}"),
        };

        native_outbound.publish(join_trace.clone()).unwrap();
        websocket_outbound.publish(join_trace.clone()).unwrap();
        assert_eq!(
            native.drain_update_batch().unwrap().into_updates(),
            join_trace
        );
        assert_eq!(
            decode_websocket_server_update_batch(&websocket.read().unwrap().into_data()).unwrap(),
            join_trace
        );

        let commands = [
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(2, -3),
                render_distance: 1,
                chunk_tracking_radius: 1,
            }),
            ClientCommand::Respawn,
        ];
        for command in commands {
            assert!(local.try_handle_command(command.clone()).is_ok());

            native.send_command_only(command.clone()).unwrap();
            match network.recv().unwrap() {
                DedicatedNetworkEvent::Command {
                    command: received, ..
                } => assert_eq!(received, command),
                event => panic!("expected native command, got {event:?}"),
            }

            websocket
                .send(Message::Binary(
                    encode_websocket_client_command(&command).unwrap().into(),
                ))
                .unwrap();
            match network.recv().unwrap() {
                DedicatedNetworkEvent::Command {
                    command: received, ..
                } => assert_eq!(received, command),
                event => panic!("expected websocket command, got {event:?}"),
            }
        }
        websocket.close(None).unwrap();
    }
}
