#![allow(unused)]

use std::collections::HashMap;
use std::io::Read;

mod shader;
use shader::*;

mod render;
use render::*;

use log::Level;
use log::log;

use wayland_backend::client::ObjectId;
use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::Proxy;
use wayland_client::QueueHandle;

use wayland_client::protocol::wl_display;
use wayland_client::protocol::wl_display::WlDisplay;

use wayland_client::protocol::wl_compositor;
use wayland_client::protocol::wl_compositor::WlCompositor;

use wayland_client::protocol::wl_surface;
use wayland_client::protocol::wl_surface::WlSurface;

use wayland_client::protocol::wl_pointer;
use wayland_client::protocol::wl_pointer::WlPointer;

use wayland_client::protocol::wl_region;
use wayland_client::protocol::wl_region::WlRegion;

use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_registry::WlRegistry;

use wayland_client::protocol::wl_callback;
use wayland_client::protocol::wl_callback::WlCallback;

use wayland_client::protocol::wl_seat;
use wayland_client::protocol::wl_seat::WlSeat;

use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::ZwpTabletPadV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::*;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_v2::ZwpTabletV2;

use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape as CursorShape;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::prelude::*;

const EPSILON: f32 = 5.0;

use clap::Parser;
use clap::ValueEnum;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'w', long, default_value_t = 8.0)]
    stroke_width: f32,

    #[arg(short = 'c', long)]
    stroke_color: Option<csscolorparser::Color>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DebugMode {
    /// Skips WlCallback events
    Wayland,
    WaylandAll,
    Socket,
    Other,
}

fn main() {
    env_logger::init();

    let mut cli = Cli::parse();

    let stroke_color = cli
        .stroke_color
        .unwrap_or(csscolorparser::Color::from_rgba8(255, 0, 0, 255));

    // setup socket for messages
    let socket_name = "chameleos.sock".to_ns_name::<GenericNamespaced>().unwrap();
    let socket_opts = ListenerOptions::new()
        .name(socket_name)
        .nonblocking(interprocess::local_socket::ListenerNonblockingMode::Accept);
    let listener = match socket_opts.create_sync() {
        Ok(l) => l,
        Err(e) => match e.kind() {
            std::io::ErrorKind::AddrInUse => {
                panic!("Socket occuppied, maybe chameleos is already running?");
            }
            _ => {
                panic!("{}", e);
            }
        },
    };
    let mut listener_buffer: Vec<u8> = Vec::with_capacity(128);

    // setup wayland client
    let connection = wayland_client::Connection::connect_to_env().unwrap();
    let mut event_queue: wayland_client::EventQueue<State> = connection.new_event_queue();

    let display = connection.display();
    let _registry = display.get_registry(&event_queue.handle(), ());

    let mut state = State {
        active: false,
        width: 0,
        height: 0,

        eraser: false,

        mouse_x: None,
        mouse_y: None,
        mouse_button_held: false,

        display,
        compositor: None,
        surface: None,
        seat: None,

        layer_shell: None,
        layer_surface: None,

        tablet_manager: None,
        tablet_seat: None,
        // will usually only ever be one anyway
        tablet_cursor_shape_devices: HashMap::new(),

        pointer: None,
        cursor_shape_manager: None,
        cursor_shape_device: None,

        wpgu: None,

        stroke_width: cli.stroke_width,
        stroke_color: stroke_color,
        current_line: Vec::new(),
        tessellated_lines: Vec::new(),
        tessellated_lines_source: Vec::new(),
    };

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();

        // request new frame
        // needed so the application doesn't die when disabling interactivity
        state.surface().frame(&event_queue.handle(), ());
        state.surface().commit();

        if let Ok(mut stream) = listener.accept() {
            stream.read_to_end(&mut listener_buffer);

            log!(
                target: "chameleos::socket",
                Level::Info,
                "received message: {}",
                String::from_utf8_lossy(&listener_buffer)
            );

            let mut split = listener_buffer.split(|&c| c == b' ');

            match split.next() {
                Some(b"toggle") => state.toggle_input(&event_queue.handle()),
                Some(b"undo") => state.undo(),
                Some(b"clear") => state.clear(),
                Some(b"clear_and_deactivate") => {
                    state.clear();
                    state.deactivate(&event_queue.handle());
                }
                Some(b"stroke_width") => {
                    match split
                        .next()
                        .and_then(|width_text| String::from_utf8(width_text.to_vec()).ok())
                        .and_then(|width_text| width_text.parse::<f32>().ok())
                    {
                        Some(width) => state.stroke_width = width,
                        None => {
                            eprintln!("received stroke width message but couldn't parse a width")
                        }
                    }
                }
                Some(b"stroke_color") => {
                    match split
                        .next()
                        .and_then(|color_text| String::from_utf8(color_text.to_vec()).ok())
                        .and_then(|color_text| csscolorparser::parse(&color_text).ok())
                    {
                        Some(color) => state.stroke_color = color,
                        None => {
                            eprintln!("received stroke color message but couldn't parse a color")
                        }
                    }
                }
                Some(b"exit") => break,
                Some(message) => eprintln!("unknown message: {}", String::from_utf8_lossy(message)),
                None => eprintln!("received empty message"),
            }

            listener_buffer.clear();
        }
    }

    println!("Exiting");

    // TODO maybe should do some better cleanup?
}

struct State {
    active: bool,
    width: usize,
    height: usize,

    eraser: bool,

    // needs to be to know the mouse position on presses
    mouse_x: Option<f64>,
    mouse_y: Option<f64>,
    mouse_button_held: bool,

    display: WlDisplay,
    compositor: Option<WlCompositor>,
    surface: Option<WlSurface>,
    seat: Option<WlSeat>,

    layer_shell: Option<ZwlrLayerShellV1>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,

    tablet_manager: Option<ZwpTabletManagerV2>,
    tablet_seat: Option<ZwpTabletSeatV2>,
    tablet_cursor_shape_devices: HashMap<ObjectId, WpCursorShapeDeviceV1>,

    pointer: Option<WlPointer>,
    cursor_shape_manager: Option<WpCursorShapeManagerV1>,
    cursor_shape_device: Option<WpCursorShapeDeviceV1>,

    wpgu: Option<Wgpu>,

    stroke_width: f32,
    stroke_color: csscolorparser::Color,
    current_line: Vec<(f32, f32)>,
    tessellated_lines: Vec<Geometry>,
    tessellated_lines_source: Vec<lyon::path::Path>,
}

impl State {
    fn compositor(&self) -> &WlCompositor {
        self.compositor.as_ref().unwrap()
    }

    fn surface(&self) -> &WlSurface {
        self.surface.as_ref().unwrap()
    }

    fn layer_surface(&self) -> &ZwlrLayerSurfaceV1 {
        self.layer_surface.as_ref().unwrap()
    }

    fn seat(&self) -> &WlSeat {
        self.seat.as_ref().unwrap()
    }

    fn tablet_manager(&self) -> &ZwpTabletManagerV2 {
        self.tablet_manager.as_ref().unwrap()
    }

    fn cursor_shape_manager(&mut self) -> &WpCursorShapeManagerV1 {
        self.cursor_shape_manager.as_mut().unwrap()
    }

    fn cursor_shape_device(&mut self) -> &WpCursorShapeDeviceV1 {
        self.cursor_shape_device.as_mut().unwrap()
    }

    fn wgpu(&self) -> &Wgpu {
        self.wpgu.as_ref().unwrap()
    }

    fn undo(&mut self) {
        if self.current_line.is_empty() {
            self.tessellated_lines.pop();
            self.tessellated_lines_source.pop();
        } else {
            self.current_line.clear();
        }
    }

    fn clear(&mut self) {
        self.tessellated_lines.clear();
        self.tessellated_lines_source.clear();
        self.current_line.clear();
    }

    fn erase(&mut self) {
        if let Some(mouse_x) = self.mouse_x
            && let Some(mouse_y) = self.mouse_y
        {
            let x = mouse_x as f32;
            let y = self.height as f32 - mouse_y as f32;

            let p = lyon::math::point(x, y);

            let eraser_size = self.stroke_width * 10.0;

            let mut to_remove = None;

            for (i, line) in self.tessellated_lines_source.iter().enumerate() {
                // simple distance check from each point to our cursor
                // we could also use lyon::math::hit_test
                // but that has caused problems with short paths
                for event in line {
                    match event {
                        lyon::path::Event::Begin { at } => {
                            if (at - p).square_length() < eraser_size {
                                to_remove = Some(i);
                                break;
                            }
                        }
                        lyon::path::Event::Line { from, to } => {
                            if (to - p).square_length() < eraser_size {
                                to_remove = Some(i);
                                break;
                            }
                        }
                        lyon::path::Event::Quadratic { from, ctrl, to } => todo!(),
                        lyon::path::Event::Cubic {
                            from,
                            ctrl1,
                            ctrl2,
                            to,
                        } => todo!(),
                        lyon::path::Event::End { last, first, close } => {}
                    }
                }
            }

            if let Some(i) = to_remove {
                self.tessellated_lines.remove(i);
                self.tessellated_lines_source.remove(i);
            }
        }
    }

    fn add_point_to_line(&mut self) {
        if let Some(mouse_x) = self.mouse_x
            && let Some(mouse_y) = self.mouse_y
        {
            let new_x = mouse_x as f32;
            let new_y = self.height as f32 - mouse_y as f32;
            match self.current_line.last() {
                Some((x, y)) => {
                    if f32::abs(x - new_x) + f32::abs(y - new_y) > EPSILON {
                        self.current_line.push((new_x, new_y));
                    }
                }
                None => {
                    self.current_line.push((new_x, new_y));
                }
            }

            // lines shouldn't get *too* long or it'll cause performance issues
            // also lyon has an upper limit at some point
            if self.current_line.len() > 0x800 {
                let (line, path) = self.tessellate_current_line().unwrap();
                self.tessellated_lines.push(line);
                self.tessellated_lines_source.push(path);
                self.current_line.clear();
            }
        }
    }

    fn activate(&mut self, qhandle: &QueueHandle<Self>) {
        let compositor = self.compositor();
        let surface = self.surface();
        let layer_surface = self.layer_surface();

        // reset to full region
        log!(target: "chameleos::general", Level::Info, "activate");
        surface.set_input_region(None);
        surface.commit();

        self.active = true;
    }

    fn deactivate(&mut self, qhandle: &QueueHandle<Self>) {
        let compositor = self.compositor();
        let surface = self.surface();
        let layer_surface = self.layer_surface();

        log!(target: "chameleos::general", Level::Info, "deactivate");
        let empty_region = compositor.create_region(qhandle, ());
        surface.set_input_region(Some(&empty_region));
        surface.commit();

        self.active = false;
    }

    fn toggle_input(&mut self, qhandle: &QueueHandle<Self>) {
        if self.active {
            // make inactive
            self.deactivate(qhandle);
        } else {
            self.activate(qhandle);
        }
    }

    fn tessellate_current_line(&self) -> Option<(Geometry, lyon::path::Path)> {
        use lyon::math::point;
        use lyon::path::Path;
        use lyon::tessellation::BuffersBuilder;
        use lyon::tessellation::StrokeOptions;
        use lyon::tessellation::StrokeTessellator;
        use lyon::tessellation::StrokeVertex;
        use lyon::tessellation::VertexBuffers;

        let line = &self.current_line;

        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let mut builder = Path::builder();
        if line.len() < 1 {
            return None;
        }

        builder.begin(point(line[0].0, line[0].1));
        // small hack for drawing dots
        builder.line_to(point(line[0].0, line[0].1));
        for &(x, y) in line.iter().skip(1) {
            builder.line_to(point(x, y));
        }
        builder.end(false);
        let path = builder.build();

        let mut tessellator = StrokeTessellator::new();
        let stroke_options = StrokeOptions::default()
            .with_line_width(self.stroke_width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);

        tessellator
            .tessellate_path(
                &path,
                &stroke_options,
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                    Vertex::new(vertex, &self.stroke_color)
                }),
            )
            .unwrap();

        Some((Geometry::new(geometry), path))
    }

    fn render(&self) {
        if let Some((current_line_geometry, _)) = self.tessellate_current_line() {
            self.wgpu().render(
                self.tessellated_lines
                    .iter()
                    .chain(std::iter::once(&current_line_geometry)),
            );
        } else {
            self.wgpu().render(&self.tessellated_lines);
        }
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlRegistry: {:?}", event);

        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "wl_compositor" => {
                    let compositor = registry.bind::<WlCompositor, _, _>(name, 6, qhandle, *data);
                    let surface = compositor.create_surface(qhandle, *data);

                    state.compositor = Some(compositor);
                    state.surface = Some(surface);
                }
                "wl_seat" => {
                    let seat = registry.bind::<WlSeat, _, _>(name, 9, qhandle, *data);
                    state.seat = Some(seat);
                }
                "wp_cursor_shape_manager_v1" => {
                    let cursor_shape_manager =
                        registry.bind::<WpCursorShapeManagerV1, _, _>(name, 1, qhandle, *data);
                    state.cursor_shape_manager = Some(cursor_shape_manager);
                }
                "zwp_tablet_manager_v2" => {
                    let tablet_manager =
                        registry.bind::<ZwpTabletManagerV2, _, _>(name, 1, qhandle, *data);
                    state.tablet_manager = Some(tablet_manager);
                }
                "zwlr_layer_shell_v1" => {
                    let surface = state.surface();

                    let layer_shell = registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                        name, 4, qhandle, *data,
                    );
                    let layer_surface = layer_shell.get_layer_surface(
                        surface,
                        None, // TODO this sets the monitor we should spawn on
                        zwlr_layer_shell_v1::Layer::Overlay,
                        "chameleos".to_string(),
                        qhandle,
                        *data,
                    );

                    layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::all());
                    layer_surface.set_keyboard_interactivity(
                        zwlr_layer_surface_v1::KeyboardInteractivity::None,
                    );
                    layer_surface.set_exclusive_zone(-1);

                    state.layer_shell = Some(layer_shell);
                    state.layer_surface = Some(layer_surface);

                    state.deactivate(qhandle);
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } => todo!(),
            _ => todo!(),
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "LayerSurface: {:?}", event);

        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                state.layer_surface().ack_configure(serial);

                state.width = width as usize;
                state.height = height as usize;

                let surface = state.surface();

                if state.wpgu.is_none() {
                    state.wpgu = Some(Wgpu::new(
                        &state.display,
                        surface,
                        width,
                        height,
                        &state.stroke_color,
                    ));
                }

                // need to render or else Hyprland spams us with configure events
                state.render();
            }
            zwlr_layer_surface_v1::Event::Closed => {}
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for State {
    fn event(
        state: &mut Self,
        shell: &ZwlrLayerShellV1,
        event: <ZwlrLayerShellV1 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "LayerShell: {:?}", event);
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn event(
        state: &mut Self,
        compositor: &WlCompositor,
        event: <WlCompositor as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlCompositor: {:?}", event);
    }
}

impl Dispatch<WlSurface, ()> for State {
    fn event(
        state: &mut Self,
        surface: &WlSurface,
        event: <WlSurface as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlSurface: {:?}", event);
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlSeat: {:?}", event);

        match event {
            wl_seat::Event::Capabilities { capabilities } => match capabilities {
                wayland_client::WEnum::Value(capabilities) => {
                    if capabilities.contains(wl_seat::Capability::Pointer) {
                        let pointer = seat.get_pointer(qhandle, *data);

                        let device = state
                            .cursor_shape_manager()
                            .get_pointer(&pointer, qhandle, *data);

                        state.cursor_shape_device = Some(device);
                        state.pointer = Some(pointer);
                    }
                }
                wayland_client::WEnum::Unknown(_) => {}
            },
            wl_seat::Event::Name { name } => {
                state.tablet_seat =
                    Some(state.tablet_manager().get_tablet_seat(seat, qhandle, *data));
            }
            _ => {}
        }
    }
}

impl Dispatch<WlRegion, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &WlRegion,
        event: <WlRegion as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlRegion: {:?}", event);
    }
}

impl Dispatch<WpCursorShapeManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        manager: &WpCursorShapeManagerV1,
        event: <WpCursorShapeManagerV1 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WpCursorShapeManagerV1: {:?}", event);
    }
}

impl Dispatch<WpCursorShapeDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        device: &WpCursorShapeDeviceV1,
        event: <WpCursorShapeDeviceV1 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WpCursorShapeDeviceV1: {:?}", event);
    }
}

impl Dispatch<WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        callback: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Trace, "WlCallback: {:?}", event);

        state.render();
    }
}

impl Dispatch<ZwpTabletManagerV2, ()> for State {
    fn event(
        state: &mut Self,
        tablet_manager: &ZwpTabletManagerV2,
        event: <ZwpTabletManagerV2 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "ZwpTabletManagerV2: {:?}", event);
    }
}

impl Dispatch<ZwpTabletSeatV2, ()> for State {
    fn event(
        state: &mut Self,
        tablet_seat: &ZwpTabletSeatV2,
        event: <ZwpTabletSeatV2 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "ZwpTabletSeatV2: {:?}", event);

        match event {
            Event::TabletAdded { id } => {}
            Event::ToolAdded { id } => {
                let cursor_shape_device = state
                    .cursor_shape_manager()
                    .get_tablet_tool_v2(&id, qhandle, *data);
                state
                    .tablet_cursor_shape_devices
                    .insert(id.id(), cursor_shape_device);
            }
            Event::PadAdded { id } => {}
            _ => {}
        }
    }

    wayland_client::event_created_child!(State, ZwpTabletSeatV2, [
        EVT_PAD_ADDED_OPCODE => (ZwpTabletPadV2, ()),
        EVT_TABLET_ADDED_OPCODE => (ZwpTabletV2, ()),
        EVT_TOOL_ADDED_OPCODE => (ZwpTabletToolV2, ()),
    ]);
}

impl Dispatch<ZwpTabletV2, ()> for State {
    fn event(
        state: &mut Self,
        tablet: &ZwpTabletV2,
        event: <ZwpTabletV2 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "ZwpTabletV2: {:?}", event);
    }
}

impl Dispatch<ZwpTabletPadV2, ()> for State {
    fn event(
        state: &mut Self,
        pad: &ZwpTabletPadV2,
        event: <ZwpTabletPadV2 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "ZwpTabletPadV2: {:?}", event);
    }
}

impl Dispatch<ZwpTabletToolV2, ()> for State {
    fn event(
        state: &mut Self,
        tool: &ZwpTabletToolV2,
        event: <ZwpTabletToolV2 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Debug, "ZwpTabletToolV2: {:?}", event);

        match event {
            zwp_tablet_tool_v2::Event::Type { tool_type } => {}
            zwp_tablet_tool_v2::Event::HardwareSerial {
                hardware_serial_hi,
                hardware_serial_lo,
            } => {}
            zwp_tablet_tool_v2::Event::HardwareIdWacom {
                hardware_id_hi,
                hardware_id_lo,
            } => {}
            zwp_tablet_tool_v2::Event::Capability { capability } => {}
            zwp_tablet_tool_v2::Event::Done => {}
            zwp_tablet_tool_v2::Event::Removed => {}
            zwp_tablet_tool_v2::Event::ProximityIn {
                serial,
                tablet,
                surface,
            } => {
                state.tablet_cursor_shape_devices[&tool.id()]
                    .set_shape(serial, CursorShape::Crosshair);
            }
            zwp_tablet_tool_v2::Event::ProximityOut => {}
            zwp_tablet_tool_v2::Event::Down { serial } => {
                state.mouse_button_held = true;
                debug_assert!(state.current_line.is_empty());
                if state.eraser {
                    state.erase();
                } else {
                    state.add_point_to_line();
                }
            }
            zwp_tablet_tool_v2::Event::Up => {
                state.mouse_button_held = false;
                if let Some((tesselated_line, path)) = state.tessellate_current_line() {
                    state.tessellated_lines.push(tesselated_line);
                    state.tessellated_lines_source.push(path);
                }
                state.current_line.clear();
            }
            zwp_tablet_tool_v2::Event::Motion { x, y } => {
                state.mouse_x = Some(x);
                state.mouse_y = Some(y);

                if state.mouse_button_held {
                    if state.eraser {
                        state.erase();
                    } else {
                        state.add_point_to_line();
                    }
                }
            }
            zwp_tablet_tool_v2::Event::Pressure { pressure } => {}
            zwp_tablet_tool_v2::Event::Distance { distance } => {}
            zwp_tablet_tool_v2::Event::Tilt { tilt_x, tilt_y } => {}
            zwp_tablet_tool_v2::Event::Rotation { degrees } => {}
            zwp_tablet_tool_v2::Event::Slider { position } => {}
            zwp_tablet_tool_v2::Event::Wheel { degrees, clicks } => {}
            zwp_tablet_tool_v2::Event::Button {
                serial,
                button,
                state: button_state,
            } => {
                if button == 331 {
                    match button_state {
                        wayland_client::WEnum::Value(button_state) => match button_state {
                            zwp_tablet_tool_v2::ButtonState::Released => {
                                state.eraser = false;
                            }
                            zwp_tablet_tool_v2::ButtonState::Pressed => {
                                state.eraser = true;
                                if state.mouse_button_held {
                                    state.erase();
                                }
                            }
                            _ => {}
                        },
                        wayland_client::WEnum::Unknown(_) => {}
                    }
                }
            }
            zwp_tablet_tool_v2::Event::Frame { time } => {
                // TODO same as pointer, logic should be handled here instead
            }
            _ => todo!(),
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        pointer: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Debug, "WlPointer: {:?}", event);

        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                state.mouse_x = Some(surface_x);
                state.mouse_y = Some(surface_y);

                state
                    .cursor_shape_device()
                    .set_shape(serial, CursorShape::Crosshair);
            }
            wl_pointer::Event::Leave { serial, surface } => {
                state.mouse_x = None;
                state.mouse_y = None;
            }
            wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                state.mouse_x = Some(surface_x);
                state.mouse_y = Some(surface_y);

                if state.mouse_button_held {
                    state.add_point_to_line();
                }

                if state.eraser {
                    state.erase();
                }
            }
            wl_pointer::Event::Button {
                serial,
                time,
                button,
                state: button_state,
            } => {
                // left mouse button
                if button == 272 {
                    match button_state {
                        wayland_client::WEnum::Value(button_state) => match button_state {
                            wl_pointer::ButtonState::Released => {
                                state.mouse_button_held = false;
                                if let Some((tesselated_line, path)) =
                                    state.tessellate_current_line()
                                {
                                    state.tessellated_lines.push(tesselated_line);
                                    state.tessellated_lines_source.push(path);
                                }
                                state.current_line.clear();
                            }
                            wl_pointer::ButtonState::Pressed => {
                                state.mouse_button_held = true;
                                debug_assert!(state.current_line.is_empty());
                                state.add_point_to_line();
                            }
                            _ => {}
                        },
                        wayland_client::WEnum::Unknown(_) => {}
                    }
                }

                // right mouse button
                if button == 273 {
                    match button_state {
                        wayland_client::WEnum::Value(button_state) => match button_state {
                            wl_pointer::ButtonState::Released => {
                                state.eraser = false;
                            }
                            wl_pointer::ButtonState::Pressed => {
                                state.eraser = true;
                                state.erase();
                            }
                            _ => {}
                        },
                        wayland_client::WEnum::Unknown(_) => {}
                    }
                }
            }
            wl_pointer::Event::Axis { time, axis, value } => {}
            wl_pointer::Event::Frame => {
                // TODO we're supposed to do all the logic here actually
            }
            wl_pointer::Event::AxisSource { axis_source } => {}
            wl_pointer::Event::AxisStop { time, axis } => {}
            wl_pointer::Event::AxisDiscrete { axis, discrete } => {}
            wl_pointer::Event::AxisValue120 { axis, value120 } => {}
            wl_pointer::Event::AxisRelativeDirection { axis, direction } => {}
            _ => {}
        }
    }
}
