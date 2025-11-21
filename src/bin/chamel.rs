use std::io::Write;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixStream;

use clap::Parser;

use chameleos::Command;

mod metadata {
    include!(concat!(env!("OUT_DIR"), "/metadata.rs"));
}

#[derive(Parser)]
#[command(
    name = "chamel",
    version = metadata::VERSION,
    long_version = metadata::LONG_VERSION,
    about = "Helper utility for sending commands to chameleos",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let socket_addr = SocketAddr::from_abstract_name("chameleos.sock")?;
    let mut stream = UnixStream::connect_addr(&socket_addr)?;

    let s = cli.command.serialize();
    stream.write_all(&s)
}
