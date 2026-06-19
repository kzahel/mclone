mod session;

use std::net::TcpListener;

use anyhow::{Context, Result, bail};
use mclone_protocol::PROTOCOL_VERSION;
use mclone_server::IntegratedServer;

use crate::session::handle_connection;

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
         The server accepts a persistent native TCP command stream per connection. --serve-once is intended for loopback smokes and exits after the first connection closes."
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
