use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use mclone_net::{
    NativeClientSession, decode_websocket_client_command, decode_websocket_client_handshake,
    encode_websocket_server_handshake_accept, encode_websocket_server_handshake_reject,
    encode_websocket_server_update_batch,
};
use mclone_protocol::PROTOCOL_VERSION;
use tungstenite::{Message, accept};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebSocketBridgeMode {
    Forever,
    ServeOnce,
}

pub(crate) fn run_websocket_bridge(
    listener: TcpListener,
    upstream_addr: SocketAddr,
    mode: WebSocketBridgeMode,
) -> Result<()> {
    match mode {
        WebSocketBridgeMode::ServeOnce => {
            let (stream, peer_addr) = listener
                .accept()
                .context("failed to accept dedicated websocket connection")?;
            handle_websocket_connection(stream, peer_addr, upstream_addr)
        }
        WebSocketBridgeMode::Forever => run_forever(listener, upstream_addr),
    }
}

fn run_forever(listener: TcpListener, upstream_addr: SocketAddr) -> Result<()> {
    listener
        .set_nonblocking(true)
        .context("failed to set dedicated websocket listener nonblocking")?;
    loop {
        match listener.accept() {
            Ok((stream, peer_addr)) => {
                thread::Builder::new()
                    .name("mclone-dedicated-ws".to_owned())
                    .spawn(move || {
                        if let Err(err) =
                            handle_websocket_connection(stream, peer_addr, upstream_addr)
                        {
                            log::warn!(
                                "dedicated websocket client {peer_addr} disconnected with error: {err:#}"
                            );
                        }
                    })
                    .context("failed to spawn dedicated websocket connection thread")?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(err) => {
                log::warn!("failed to accept dedicated websocket connection: {err}");
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn handle_websocket_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    upstream_addr: SocketAddr,
) -> Result<()> {
    let mut websocket = accept(stream)
        .with_context(|| format!("failed dedicated websocket handshake with {peer_addr}"))?;
    complete_websocket_protocol_handshake(&mut websocket)?;
    let mut upstream = NativeClientSession::connect(upstream_addr).with_context(|| {
        format!("failed to connect websocket client {peer_addr} to native upstream {upstream_addr}")
    })?;

    let mut command_count = 0usize;
    loop {
        let message = websocket
            .read()
            .with_context(|| format!("failed to read websocket client {peer_addr} message"))?;
        let payload = match message {
            Message::Binary(payload) => payload,
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Text(_) => bail!("dedicated websocket client {peer_addr} sent text data"),
            Message::Frame(_) => continue,
        };
        let command = decode_websocket_client_command(&payload)
            .context("failed to decode websocket client command")?;
        let updates = upstream
            .send_command(&command)
            .context("failed to exchange websocket command with native upstream")?;
        let response = encode_websocket_server_update_batch(&updates)
            .context("failed to encode websocket server updates")?;
        websocket
            .send(Message::Binary(response.into()))
            .with_context(|| format!("failed to send websocket server updates to {peer_addr}"))?;
        command_count += 1;
    }

    log::info!(
        "dedicated websocket client {peer_addr} disconnected after {command_count} commands"
    );
    Ok(())
}

fn complete_websocket_protocol_handshake(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
) -> Result<()> {
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
    if received != PROTOCOL_VERSION {
        let response = encode_websocket_server_handshake_reject(PROTOCOL_VERSION, received)
            .context("failed to encode websocket protocol rejection")?;
        let _ = websocket.send(Message::Binary(response.into()));
        bail!(
            "websocket protocol version mismatch: expected {}, received {}",
            PROTOCOL_VERSION,
            received
        );
    }

    let response = encode_websocket_server_handshake_accept(PROTOCOL_VERSION)
        .context("failed to encode websocket protocol acceptance")?;
    websocket
        .send(Message::Binary(response.into()))
        .context("failed to send websocket protocol acceptance")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_net::{
        complete_server_handshake, decode_websocket_server_update_batch,
        encode_current_websocket_client_handshake, encode_websocket_client_command,
        read_client_command_frame, write_server_update_batch,
    };
    use mclone_protocol::{ChunkView, ClientCommand, ServerUpdate};

    #[test]
    fn websocket_bridge_proxies_commands_to_native_upstream() {
        let upstream_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        let bridge_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let bridge_addr = bridge_listener.local_addr().unwrap();
        let command = ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(2, -3),
            render_distance: 1,
            chunk_tracking_radius: 1,
        });
        let expected_command = command.clone();
        let updates = vec![ServerUpdate::TimeUpdate { day_time: 123 }];
        let expected_updates = updates.clone();

        let upstream = thread::spawn(move || {
            let (mut stream, _) = upstream_listener.accept().unwrap();
            complete_server_handshake(&mut stream).unwrap();
            assert_eq!(
                read_client_command_frame(&mut stream).unwrap(),
                expected_command
            );
            write_server_update_batch(&mut stream, &updates).unwrap();
        });
        let bridge = thread::spawn(move || {
            run_websocket_bridge(
                bridge_listener,
                upstream_addr,
                WebSocketBridgeMode::ServeOnce,
            )
            .unwrap();
        });

        let (mut socket, _) = tungstenite::connect(format!("ws://{bridge_addr}")).unwrap();
        socket
            .send(Message::Binary(
                encode_current_websocket_client_handshake().unwrap().into(),
            ))
            .unwrap();
        let handshake = socket.read().unwrap().into_data();
        mclone_net::decode_websocket_server_handshake(&handshake, PROTOCOL_VERSION).unwrap();
        socket
            .send(Message::Binary(
                encode_websocket_client_command(&command).unwrap().into(),
            ))
            .unwrap();
        let response = socket.read().unwrap().into_data();
        assert_eq!(
            decode_websocket_server_update_batch(&response).unwrap(),
            expected_updates
        );
        socket.close(None).unwrap();

        bridge.join().unwrap();
        upstream.join().unwrap();
    }
}
