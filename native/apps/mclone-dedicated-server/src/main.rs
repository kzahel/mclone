mod connection;
mod session;

use std::collections::BTreeMap;
use std::net::TcpListener;

use anyhow::{Context, Result, bail};
use mclone_protocol::PROTOCOL_VERSION;
use mclone_server::IntegratedServer;

use crate::connection::{DedicatedConnectionId, DedicatedNetwork, DedicatedNetworkEvent};
use crate::session::DedicatedSession;

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:25565";
const DEFAULT_SEED: i64 = 12345;

#[cfg(test)]
static DEDICATED_NETWORK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn main() -> Result<()> {
    env_logger::init();
    run_server(Cli::parse(std::env::args().skip(1))?)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    listen: String,
    seed: i64,
    serve_once: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            listen: DEFAULT_LISTEN_ADDR.to_owned(),
            seed: DEFAULT_SEED,
            serve_once: false,
        }
    }
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut cli = Self::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--listen" => {
                    cli.listen = args.next().context("--listen requires HOST:PORT")?;
                }
                "--seed" => {
                    cli.seed = parse_i64_arg("--seed", args.next())?;
                }
                "--serve-once" => {
                    cli.serve_once = true;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }

        Ok(cli)
    }
}

fn parse_i64_arg(flag: &str, value: Option<String>) -> Result<i64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i64>()
        .with_context(|| format!("{flag} requires a signed 64-bit integer, got `{value}`"))
}

fn print_help() {
    println!(
        "mclone-dedicated-server\n\n\
         Usage:\n\
           mclone-dedicated-server [--listen 127.0.0.1:25565] [--seed 12345] [--serve-once]\n\n\
         The server accepts persistent native TCP command streams from multiple clients. --serve-once is intended for loopback smokes and exits after the first connection closes."
    );
}

fn run_server(cli: Cli) -> Result<()> {
    let listener = TcpListener::bind(&cli.listen)
        .with_context(|| format!("failed to bind dedicated server to {}", cli.listen))?;
    let local_addr = listener
        .local_addr()
        .context("failed to read listen addr")?;
    println!(
        "mclone dedicated server listening on {local_addr} seed={} protocol {}",
        cli.seed, PROTOCOL_VERSION
    );

    let mode = if cli.serve_once {
        ServerRunMode::ServeOnce
    } else {
        ServerRunMode::Forever
    };
    run_server_loop(listener, cli.seed, mode)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServerRunMode {
    Forever,
    ServeOnce,
    #[cfg(test)]
    UntilDisconnects(usize),
}

fn run_server_loop(listener: TcpListener, seed: i64, mode: ServerRunMode) -> Result<()> {
    let network = DedicatedNetwork::start(listener)?;
    let mut server = IntegratedServer::new(seed);
    let mut sessions = BTreeMap::<DedicatedConnectionId, DedicatedSession>::new();
    let mut completed_connections = 0_usize;

    loop {
        match network.recv()? {
            DedicatedNetworkEvent::Connected { id, peer_addr } => {
                sessions.insert(id, DedicatedSession::default());
                log::info!("accepted dedicated client {id} from {peer_addr}");
            }
            DedicatedNetworkEvent::Command {
                id,
                peer_addr,
                command,
                response,
            } => {
                let Some(session) = sessions.get_mut(&id) else {
                    let message = format!("received command for disconnected client {id}");
                    let _ = response.send(Err(message.clone()));
                    log::warn!("{message}");
                    continue;
                };
                match session.handle_client_command(&mut server, command) {
                    Ok(updates) => {
                        let update_count = updates.len();
                        if response.send(Ok(updates)).is_err() {
                            sessions.remove(&id);
                            log::warn!(
                                "dedicated client {id} {peer_addr} disconnected before receiving {update_count} updates"
                            );
                        } else {
                            log::debug!(
                                "served dedicated client {id} {peer_addr} with {update_count} updates"
                            );
                        }
                    }
                    Err(err) => {
                        let message = format!("{err:#}");
                        let _ = response.send(Err(message.clone()));
                        sessions.remove(&id);
                        if mode == ServerRunMode::ServeOnce {
                            return Err(err)
                                .with_context(|| format!("failed to serve {id} {peer_addr}"));
                        }
                        log::warn!("failed to serve dedicated client {id} {peer_addr}: {message}");
                    }
                }
            }
            DedicatedNetworkEvent::Disconnected {
                id,
                peer_addr,
                command_count,
                reason,
            } => {
                sessions.remove(&id);
                if command_count > 0 {
                    completed_connections += 1;
                }
                if let Some(reason) = reason {
                    log::warn!(
                        "dedicated client {id} {peer_addr} disconnected after {command_count} commands: {reason}"
                    );
                } else {
                    log::info!(
                        "dedicated client {id} {peer_addr} disconnected after {command_count} commands"
                    );
                }
                match mode {
                    ServerRunMode::Forever => {}
                    ServerRunMode::ServeOnce => {
                        if command_count == 0 {
                            bail!("connection {id} {peer_addr} closed without client command");
                        }
                        return Ok(());
                    }
                    #[cfg(test)]
                    ServerRunMode::UntilDisconnects(target) => {
                        if completed_connections >= target {
                            return Ok(());
                        }
                    }
                }
            }
            DedicatedNetworkEvent::AcceptFailed { message } => {
                if mode == ServerRunMode::ServeOnce {
                    bail!(message);
                }
                log::warn!("{message}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::ChunkPos;
    use mclone_net::NativeClientSession;
    use mclone_protocol::{ChunkView, ClientCommand, ServerUpdate};

    #[test]
    fn cli_defaults_to_localhost_server() {
        assert_eq!(Cli::parse([]).unwrap(), Cli::default());
    }

    #[test]
    fn cli_parses_listen_seed_and_serve_once() {
        assert_eq!(
            Cli::parse([
                "--listen".to_owned(),
                "127.0.0.1:0".to_owned(),
                "--seed".to_owned(),
                "-7".to_owned(),
                "--serve-once".to_owned(),
            ])
            .unwrap(),
            Cli {
                listen: "127.0.0.1:0".to_owned(),
                seed: -7,
                serve_once: true,
            }
        );
    }

    #[test]
    fn server_loop_serves_multiple_persistent_clients() {
        let _guard = DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            run_server_loop(listener, DEFAULT_SEED, ServerRunMode::UntilDisconnects(2)).unwrap();
        });

        let client_a = std::thread::spawn(move || {
            let mut session = NativeClientSession::connect(addr).unwrap();
            session
                .send_command(&ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap()
        });
        let client_b = std::thread::spawn(move || {
            let mut session = NativeClientSession::connect(addr).unwrap();
            session
                .send_command(&ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap()
        });

        let updates_a = client_a.join().unwrap();
        let updates_b = client_b.join().unwrap();
        server.join().unwrap();

        assert!(
            updates_a
                .iter()
                .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        );
        assert!(
            updates_b
                .iter()
                .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        );
    }
}
