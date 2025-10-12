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
    Undo,
    Clear,
    ClearAndDeactivate,
    StrokeWidth { width: f32 },
    Exit,
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

    match cli.command {
        Command::Toggle => stream.write_all(b"toggle"),
        Command::Undo => stream.write_all(b"undo"),
        Command::Clear => stream.write_all(b"clear"),
        Command::ClearAndDeactivate => stream.write_all(b"clear_and_deactivate"),
        Command::StrokeWidth { width } => {
            let s = format!("stroke_width {}", width);
            stream.write_all(s.as_bytes())
        }
        Command::Exit => stream.write_all(b"exit"),
    }
}
