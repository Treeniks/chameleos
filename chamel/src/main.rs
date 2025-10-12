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
    Clear,
    ClearAndDeactivate,
    StrokeWidth { width: f32 },
    Exit,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let socket_name = "chameleos.sock".to_ns_name::<GenericNamespaced>().unwrap();
    let mut stream = Stream::connect(socket_name).unwrap();

    match cli.command {
        Command::Toggle => stream.write_all(b"toggle"),
        Command::Clear => stream.write_all(b"clear"),
        Command::ClearAndDeactivate => stream.write_all(b"clear_and_deactivate"),
        Command::StrokeWidth { width } => {
            let s = format!("stroke_width {}", width);
            stream.write_all(s.as_bytes())
        }
        Command::Exit => stream.write_all(b"exit"),
    }
}
