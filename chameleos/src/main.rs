#![allow(unused)]

use std::io::Read;

mod shader;
use shader::*;

mod render;
use render::*;

use xkbcommon::xkb;

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

use wayland_client::protocol::wl_keyboard;
use wayland_client::protocol::wl_keyboard::WlKeyboard;

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

#[derive(Parser)]
struct Cli {
    /// Turn on debug printing
    #[arg(long)]
    debug: bool,
}

macro_rules! dprintln {
    ($state:expr, $($arg:tt)*) => {
        if $state.debug {
            println!($($arg)*);
        }
    };
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

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
        debug: cli.debug,

        active: true,
        width: 0,
        height: 0,

        mouse_x: None,
        mouse_y: None,
        mouse_button_held: false,

        display,
        compositor: None,
        surface: None,
        seat: None,

        layer_shell: None,
        layer_surface: None,

        keyboard: None,
        xkb_state: None,
        pointer: None,
        cursor_shape_manager: None,
        cursor_shape_device: None,

        wpgu: None,

        current_line: Vec::new(),
        tessellated_lines: Vec::new(),
    };

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();

        // request new frame
        // needed so the application doesn't die when disabling interactivity
        state.surface().frame(&event_queue.handle(), ());
        state.surface().commit();

        if let Ok(mut stream) = listener.accept() {
            stream.read_to_end(&mut listener_buffer);
            dprintln!(
                state,
                "received message: {}",
                String::from_utf8_lossy(&listener_buffer)
            );

            match listener_buffer.as_slice() {
                b"toggle" => {
                    state.toggle_input(&event_queue.handle());
                }
                b"exit" => break,
                _ => {}
            }

            listener_buffer.clear();
        }
    }

    // TODO maybe should do some better cleanup?
}

struct State {
    debug: bool,

    active: bool,
    width: usize,
    height: usize,

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

    keyboard: Option<WlKeyboard>,
    xkb_state: Option<xkb::State>,
    pointer: Option<WlPointer>,
    cursor_shape_manager: Option<WpCursorShapeManagerV1>,
    cursor_shape_device: Option<WpCursorShapeDeviceV1>,

    wpgu: Option<Wgpu>,

    current_line: Vec<(f32, f32)>,
    tessellated_lines: Vec<Geometry>,
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

    fn xkb_state(&self) -> &xkb::State {
        self.xkb_state.as_ref().unwrap()
    }
    fn xkb_state_mut(&mut self) -> &mut xkb::State {
        self.xkb_state.as_mut().unwrap()
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

    fn clear(&mut self) {
        self.tessellated_lines.clear();
        self.render();
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
                self.tessellated_lines
                    .push(self.tessellate_current_line().unwrap());
                self.current_line.clear();
            }
        }
    }

    fn toggle_input(&mut self, qhandle: &QueueHandle<Self>) {
        let compositor = self.compositor();
        let surface = self.surface();
        let layer_surface = self.layer_surface();

        if self.active {
            // make inactive
            dprintln!(self, "deactivate");
            let empty_region = compositor.create_region(qhandle, ());
            surface.set_input_region(Some(&empty_region));
            layer_surface
                .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        } else {
            // reset to full region
            dprintln!(self, "activate");
            surface.set_input_region(None);
            layer_surface.set_keyboard_interactivity(
                zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
            );
        }

        surface.commit();
        self.active = !self.active;
    }

    fn tessellate_current_line(&self) -> Option<Geometry> {
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
            .with_line_width(16.0)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);

        tessellator
            .tessellate_path(
                &path,
                &stroke_options,
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| Vertex {
                    position: vertex.position().to_array(),
                }),
            )
            .unwrap();

        Some(Geometry::new(geometry))
    }

    fn render(&self) {
        if let Some(current_line_geometry) = self.tessellate_current_line() {
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
        dprintln!(state, "WlRegistry: {:?}", event);
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
                        zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
                    );
                    layer_surface.set_exclusive_zone(-1);
                    surface.set_input_region(None);

                    state.layer_shell = Some(layer_shell);
                    state.layer_surface = Some(layer_surface);
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
        dprintln!(state, "LayerSurface: {:?}", event);
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                state.width = width as usize;
                state.height = height as usize;

                let surface = state.surface();

                if state.wpgu.is_none() {
                    state.wpgu = Some(Wgpu::new(&state.display, surface, width, height));
                }

                state.layer_surface().ack_configure(serial);
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
        dprintln!(state, "LayerShell: {:?}", event);
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
        dprintln!(state, "WlCompositor: {:?}", event);
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
        dprintln!(state, "WlSurface: {:?}", event);
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
        dprintln!(state, "WlSeat: {:?}", event);
        match event {
            wl_seat::Event::Capabilities { capabilities } => match capabilities {
                wayland_client::WEnum::Value(capabilities) => {
                    if capabilities.contains(wl_seat::Capability::Keyboard) {
                        let keyboard = seat.get_keyboard(qhandle, *data);
                        state.keyboard = Some(keyboard);
                    }

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
        dprintln!(state, "WlRegion: {:?}", event);
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
        dprintln!(state, "WpCursorShapeManagerV1: {:?}", event);
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
        dprintln!(state, "WpCursorShapeDeviceV1: {:?}", event);
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
        dprintln!(state, "WlCallback: {:?}", event);
        state.render();
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        keyboard: &WlKeyboard,
        event: <WlKeyboard as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        dprintln!(state, "WlKeyboard: {:?}", event);
        match event {
            wl_keyboard::Event::Key {
                serial,
                time,
                key,
                state: key_state,
            } => {
                let xkb_state = state.xkb_state_mut();

                // TODO update xkb_state

                // +8 because of conversion from Wayland to X11 keycodes
                // because X11 reserves the first 8 keycodes
                let key_code = xkb::Keycode::new(key + 8);
                let sym = xkb_state.key_get_one_sym(key_code);

                if sym == xkb::Keysym::c {
                    state.clear();
                }
            }
            wl_keyboard::Event::Keymap { format, fd, size } => match format {
                wayland_client::WEnum::Value(format) => {
                    let keymap = unsafe {
                        xkb::Keymap::new_from_fd(
                            &xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
                            fd,
                            size as usize,
                            format as u32,
                            xkb::KEYMAP_COMPILE_NO_FLAGS,
                        )
                        .unwrap()
                        .unwrap()
                    };
                    state.xkb_state = Some(xkb::State::new(&keymap));
                }
                wayland_client::WEnum::Unknown(_) => {}
            },
            _ => {}
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
        dprintln!(state, "WlPointer: {:?}", event);
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
                                if let Some(tesselated_line) = state.tessellate_current_line() {
                                    state.tessellated_lines.push(tesselated_line);
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
