use std::io::Read;

use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixListener;

mod render;
mod state;

use log::Level;
use log::log;

use clap::Parser;

use chamel::Command;

const EPSILON: f32 = 5.0;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'w', long, default_value_t = 8.0)]
    stroke_width: f32,

    // NOTE: We *cannot* use default_value_t
    // because clap does a to_string roundtrip with that value.
    // (presumably because it shows the value in the help)
    /// Takes any CSS color parseable by the csscolorparser crate
    #[arg(short = 'c', long, default_value = "red")]
    stroke_color: csscolorparser::Color,

    #[arg(short = 'b', long)]
    force_backend: Option<render::Backend>,
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

    // setup socket for messages
    let socket_addr = SocketAddr::from_abstract_name("chameleos.sock").unwrap();
    let listener = match UnixListener::bind_addr(&socket_addr) {
        Ok(l) => l,
        Err(e) => match e.kind() {
            std::io::ErrorKind::AddrInUse => {
                panic!("Socket occuppied, maybe chameleos is already running?")
            }
            _ => panic!("{}", e),
        },
    };
    let mut listener_buffer: Vec<u8> = Vec::with_capacity(128);

    let (mut state, mut event_queue) = state::State::setup_wayland(cli);
    let qhandle = event_queue.handle();

    state.deactivate(&qhandle);

    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        for mut stream in listener.incoming().filter_map(|s| s.ok()) {
            stream.read_to_end(&mut listener_buffer).unwrap();

            log!(
                target: "chameleos::socket",
                Level::Info,
                "received message: {}",
                String::from_utf8_lossy(&listener_buffer)
            );

            match Command::deserialize(&listener_buffer) {
                Ok(command) => sender.send(command).unwrap(),
                Err(s) => eprintln!("{}", s),
            }
            listener_buffer.clear();
        }
    });

    let qhandle = event_queue.handle();
    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();

        if let Ok(command) = receiver.try_recv() {
            match command {
                Command::Toggle => state.toggle_input(&qhandle),
                Command::Undo => state.undo(),
                Command::Clear => state.clear(),
                Command::ClearAndDeactivate => {
                    state.clear();
                    state.deactivate(&event_queue.handle());
                }
                Command::StrokeWidth { width } => state.set_stroke_width(width),
                Command::StrokeColor { color } => state.set_stroke_color(color),
                Command::Exit => break,
            }
        }
    }

    println!("Exiting");

    // TODO maybe should do some better cleanup?
}
