use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_net::{read_client_command_frames, write_server_update_batch};
use mclone_protocol::{ClientCommand, MovePlayerCommand, PROTOCOL_VERSION, ServerUpdate};
use mclone_server::IntegratedServer;

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:25565";
const DEFAULT_SEED: i64 = 12345;

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
         The server accepts one or more native TCP request frames per connection. --serve-once is intended for loopback smokes and exits after the first connection."
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

    let mut server = IntegratedServer::new(cli.seed);
    loop {
        let (stream, peer_addr) = listener
            .accept()
            .context("failed to accept dedicated server connection")?;
        match handle_connection(stream, &mut server) {
            Ok(update_count) => {
                log::info!("served {peer_addr} with {update_count} updates");
            }
            Err(err) if cli.serve_once => {
                return Err(err).with_context(|| format!("failed to serve {peer_addr}"));
            }
            Err(err) => {
                log::warn!("failed to serve {peer_addr}: {err:#}");
            }
        }

        if cli.serve_once {
            return Ok(());
        }
    }
}

fn handle_connection(mut stream: TcpStream, server: &mut IntegratedServer) -> Result<usize> {
    serve_connection(&mut stream, server)
}

fn serve_connection(
    stream: &mut (impl Read + Write),
    server: &mut IntegratedServer,
) -> Result<usize> {
    let commands = read_client_command_frames(stream).context("failed to read client commands")?;
    if commands.is_empty() {
        bail!("connection closed without client command");
    }

    let mut connection = DedicatedConnectionState::default();
    let mut updates = Vec::new();
    for command in commands {
        updates.extend(
            connection
                .handle_command(server, command)
                .context("failed to apply client command")?,
        );
    }
    updates.extend(
        connection
            .flush_movement(server)
            .context("failed to flush movement commands")?,
    );
    updates.extend(wait_for_server_jobs(server)?);
    server
        .save_dirty_chunks()
        .context("failed to save dirty chunks")?;
    let update_count = updates.len();
    write_server_update_batch(stream, &updates).context("failed to write server update batch")?;
    Ok(update_count)
}

#[derive(Debug, Default)]
struct DedicatedConnectionState {
    pending_movement: VecDeque<MovePlayerCommand>,
    received_move_packet_count: u32,
    known_move_packet_count: u32,
}

impl DedicatedConnectionState {
    fn handle_command(
        &mut self,
        server: &mut IntegratedServer,
        command: ClientCommand,
    ) -> Result<Vec<ServerUpdate>> {
        match command {
            ClientCommand::MovePlayer(command) => {
                self.stage_movement(command);
                Ok(Vec::new())
            }
            command => {
                let mut updates = self.flush_movement(server)?;
                updates.extend(server.try_handle_command(command)?);
                Ok(updates)
            }
        }
    }

    fn stage_movement(&mut self, command: MovePlayerCommand) {
        self.received_move_packet_count = self.received_move_packet_count.saturating_add(1);
        self.pending_movement.push_back(command);
    }

    fn flush_movement(&mut self, server: &mut IntegratedServer) -> Result<Vec<ServerUpdate>> {
        let mut updates = Vec::new();
        while let Some(command) = self.pending_movement.pop_front() {
            updates.extend(server.try_handle_command(ClientCommand::MovePlayer(command))?);
        }
        Ok(updates)
    }

    fn mark_tick_boundary(&mut self) {
        self.known_move_packet_count = self.received_move_packet_count;
    }

    fn buffered_move_packet_count(&self) -> usize {
        self.pending_movement.len()
    }

    const fn received_move_packet_count(&self) -> u32 {
        self.received_move_packet_count
    }

    const fn known_move_packet_count(&self) -> u32 {
        self.known_move_packet_count
    }
}

fn wait_for_server_jobs(server: &mut IntegratedServer) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut updates = Vec::new();

    loop {
        updates.extend(
            server
                .try_poll()
                .context("failed to poll dedicated server worldgen jobs")?,
        );
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for dedicated server worldgen jobs");
        }
        if server.pending_publication_count() == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{ChunkPos, Vec3d};
    use mclone_net::{read_server_update_batch, write_client_command_frame};
    use mclone_protocol::{ChunkView, ClientCommand, ServerUpdate, SetCarriedItemCommand};

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
    fn serve_connection_returns_chunk_snapshot_and_spawn_updates() {
        let mut request = Vec::new();
        write_client_command_frame(
            &mut request,
            &ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .unwrap();
        let request_len = request.len();
        let mut stream = std::io::Cursor::new(request);
        let mut server = IntegratedServer::new(DEFAULT_SEED);

        assert_eq!(serve_connection(&mut stream, &mut server).unwrap(), 2);

        let response = stream.into_inner().split_off(request_len);
        let updates = read_server_update_batch(&mut std::io::Cursor::new(response)).unwrap();
        assert_eq!(updates.len(), 2);
        assert!(
            updates
                .iter()
                .any(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
        );
        assert!(
            updates
                .iter()
                .any(|update| matches!(update, ServerUpdate::PlayerPosition(_)))
        );
    }

    #[test]
    fn dedicated_connection_buffers_movement_until_flush_or_ordered_command() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        let mut connection = DedicatedConnectionState::default();

        let updates = connection
            .handle_command(
                &mut server,
                ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                    position: Vec3d::new(1.0, 64.0, 1.0),
                    on_ground: true,
                }),
            )
            .unwrap();

        assert!(updates.is_empty());
        assert_eq!(connection.buffered_move_packet_count(), 1);
        assert_eq!(connection.received_move_packet_count(), 1);
        assert_eq!(connection.known_move_packet_count(), 0);

        connection.mark_tick_boundary();
        assert_eq!(connection.known_move_packet_count(), 1);

        let updates = connection
            .handle_command(
                &mut server,
                ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 1 }),
            )
            .unwrap();

        assert!(updates.is_empty());
        assert_eq!(connection.buffered_move_packet_count(), 0);
        assert_eq!(connection.received_move_packet_count(), 1);
        assert_eq!(connection.known_move_packet_count(), 1);
    }

    #[test]
    fn serve_connection_accepts_multiple_client_command_frames() {
        let mut request = Vec::new();
        write_client_command_frame(
            &mut request,
            &ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(1.0, 64.0, 1.0),
                on_ground: true,
            }),
        )
        .unwrap();
        write_client_command_frame(
            &mut request,
            &ClientCommand::MovePlayer(MovePlayerCommand::Rot {
                y_rot_degrees: 90.0,
                x_rot_degrees: 15.0,
                on_ground: true,
            }),
        )
        .unwrap();
        let request_len = request.len();
        let mut stream = std::io::Cursor::new(request);
        let mut server = IntegratedServer::new(DEFAULT_SEED);

        assert_eq!(serve_connection(&mut stream, &mut server).unwrap(), 0);

        let response = stream.into_inner().split_off(request_len);
        let updates = read_server_update_batch(&mut std::io::Cursor::new(response)).unwrap();
        assert!(updates.is_empty());
    }
}
