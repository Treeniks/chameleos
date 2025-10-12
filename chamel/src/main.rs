use std::io::Write;

use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::Stream;
use interprocess::local_socket::prelude::*;

use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Toggle,
    Exit,
}

fn main() {
    let cli = Cli::parse();

    let socket_name = "chameleos.sock".to_ns_name::<GenericNamespaced>().unwrap();
    let mut stream = Stream::connect(socket_name).unwrap();

    match cli.command {
        Command::Toggle => stream.write_all(b"toggle").unwrap(),
        Command::Exit => stream.write_all(b"exit").unwrap(),
    }
}
