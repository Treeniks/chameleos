use std::io::Read;

use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixListener;

mod render;
mod state;

use log::Level;
use log::log;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;

use clap::Parser;

use chameleos_core::Command;

const EPSILON: f32 = 5.0;

mod metadata {
    include!(concat!(env!("OUT_DIR"), "/metadata.rs"));
}

#[derive(Parser)]
#[command(
    version = metadata::VERSION,
    long_version = metadata::LONG_VERSION,
    about,
    long_about = None,
)]
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

    let (mut state, connection, event_queue) = state::State::setup_wayland(cli);
    let qhandle = event_queue.handle();

    state.deactivate(&qhandle);

    let (sender, receiver) = calloop::channel::channel();

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

    let mut event_loop: EventLoop<state::State> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let stop_handle = event_loop.get_signal();

    loop_handle
        .insert_source(receiver, move |event, (), state| match event {
            calloop::channel::Event::Msg(command) => match command {
                Command::Toggle => state.toggle_input(&qhandle),
                Command::Undo => state.undo(),
                Command::Clear => state.clear(),
                Command::ClearAndDeactivate => {
                    state.clear();
                    state.deactivate(&qhandle);
                }
                Command::StrokeWidth { width } => state.set_stroke_width(width),
                Command::StrokeColor { color } => state.set_stroke_color(color),
                Command::Exit => stop_handle.stop(),
            },
            calloop::channel::Event::Closed => {
                eprintln!("listener channel closed");
                stop_handle.stop();
            }
        })
        .unwrap();
    WaylandSource::new(connection, event_queue)
        .insert(loop_handle)
        .unwrap();

    event_loop.run(None, &mut state, |_| {}).unwrap();

    println!("Exiting");
}
