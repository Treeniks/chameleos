use std::io::Write;

use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::Stream;
use interprocess::local_socket::prelude::*;

use clap::Parser;

use chamel::Command;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let socket_name = "chameleos.sock".to_ns_name::<GenericNamespaced>()?;
    let mut stream = match Stream::connect(socket_name) {
        Ok(stream) => stream,
        Err(e) => match e.kind() {
            std::io::ErrorKind::ConnectionRefused => {
                eprintln!("Connection Refused, is chameleos running?");
                return Err(e);
            }
            _ => return Err(e),
        },
    };

    let s = cli.command.serialize();
    stream.write_all(&s)
}
