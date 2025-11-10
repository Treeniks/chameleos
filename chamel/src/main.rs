use std::io::Write;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixStream;

use clap::Parser;

use chamel::Command;

#[derive(Parser)]
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
