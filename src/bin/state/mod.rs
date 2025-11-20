mod draw;
mod mouse;
mod tablet;

use wayland_client::delegate_dispatch;

use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::EventQueue;
use wayland_client::Proxy;
use wayland_client::QueueHandle;
use wayland_client::WEnum;

use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_display::WlDisplay;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;

use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::ZwpTabletPadV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_v2::ZwpTabletV2;

use log::Level;
use log::log;

use crate::render::Backend;
use crate::render::WgpuState;

macro_rules! delegate_log {
    ($proxy:ty) => {
        impl Dispatch<$proxy, ()> for State {
            fn event(
                _state: &mut Self,
                _proxy: &$proxy,
                event: <$proxy as Proxy>::Event,
                _data: &(),
                _conn: &Connection,
                _qhandle: &QueueHandle<Self>,
            ) {
                log!(
                    target: "chameleos::wayland",
                    Level::Info,
                    "{}: {:?}",
                    std::any::type_name::<$proxy>(),
                    event,
                );
            }
        }
    };
}

#[derive(Default)]
struct SetupWaylandState {
    force_backend: Option<Backend>,

    compositor: Option<WlCompositor>,
    surface: Option<WlSurface>,
    seat: Option<WlSeat>,

    layer_shell: Option<ZwlrLayerShellV1>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,

    cursor_shape_manager: Option<WpCursorShapeManagerV1>,
    tablet_manager: Option<ZwpTabletManagerV2>,
}

impl SetupWaylandState {
    fn new(force_backend: Option<Backend>) -> Self {
        Self {
            force_backend,
            ..Default::default()
        }
    }

    fn into_state(self, connection: Connection, display: WlDisplay) -> WaylandState {
        WaylandState {
            connection,
            display,
            compositor: self.compositor.unwrap(),
            surface: self.surface.unwrap(),
            seat: self.seat.unwrap(),
            layer_shell: self.layer_shell.unwrap(),
            layer_surface: self.layer_surface.unwrap(),
            cursor_shape_manager: self.cursor_shape_manager.unwrap(),
            tablet_manager: self.tablet_manager.unwrap(),
        }
    }
}

impl Dispatch<WlRegistry, QueueHandle<State>> for SetupWaylandState {
    fn event(
        setup_state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as Proxy>::Event,
        state_qhandle: &QueueHandle<State>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlRegistry: {:?}", event);

        use wayland_client::protocol::wl_registry::Event;
        match event {
            Event::Global {
                name,
                interface,
                version: _, // TODO should maybe use?
            } => match interface.as_str() {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<WlCompositor, _, _>(name, 5, state_qhandle, ());
                    let surface = compositor.create_surface(state_qhandle, ());

                    setup_state.compositor = Some(compositor);
                    setup_state.surface = Some(surface);
                }
                "wl_seat" => {
                    let seat = registry.bind::<WlSeat, _, _>(name, 9, state_qhandle, ());
                    setup_state.seat = Some(seat);
                }
                "zwlr_layer_shell_v1" => {
                    let wl_surface = setup_state.surface.as_ref().unwrap();

                    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
                    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
                    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::KeyboardInteractivity;

                    let layer_shell =
                        registry.bind::<ZwlrLayerShellV1, _, _>(name, 4, state_qhandle, ());
                    let layer_surface = layer_shell.get_layer_surface(
                        &wl_surface,
                        None, // TODO this sets the monitor we should spawn on
                        Layer::Overlay,
                        "chameleos".to_string(),
                        state_qhandle,
                        setup_state.force_backend,
                    );

                    layer_surface.set_anchor(Anchor::all());
                    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
                    layer_surface.set_exclusive_zone(-1);

                    setup_state.layer_shell = Some(layer_shell);
                    setup_state.layer_surface = Some(layer_surface);
                }
                "wp_cursor_shape_manager_v1" => {
                    let cursor_shape_manager =
                        registry.bind::<WpCursorShapeManagerV1, _, _>(name, 1, state_qhandle, ());
                    setup_state.cursor_shape_manager = Some(cursor_shape_manager);
                }
                "zwp_tablet_manager_v2" => {
                    let tablet_manager =
                        registry.bind::<ZwpTabletManagerV2, _, _>(name, 1, state_qhandle, ());
                    setup_state.tablet_manager = Some(tablet_manager);
                }
                _ => {}
            },
            Event::GlobalRemove { name: _ } => {}
            _ => {}
        }
    }
}

pub struct State {
    active: bool,

    wayland: WaylandState,
    draw: draw::DrawState,

    mouse: mouse::MouseState,
    tablet: tablet::TabletState,

    wgpu: Option<WgpuState>,
}

impl State {
    pub fn setup_wayland(cli: crate::Cli) -> (Self, EventQueue<Self>) {
        let connection = Connection::connect_to_env().unwrap();
        let mut setup_queue = connection.new_event_queue();
        let event_queue = connection.new_event_queue();

        let display = connection.display();
        let _registry = display.get_registry(&setup_queue.handle(), event_queue.handle());

        let mut tmp_wayland_state = SetupWaylandState::new(cli.force_backend);

        setup_queue.roundtrip(&mut tmp_wayland_state).unwrap();

        let wayland_state = tmp_wayland_state.into_state(connection, display);
        wayland_state.surface.frame(&event_queue.handle(), ());
        wayland_state.surface.commit();

        let state = Self {
            active: false,
            wayland: wayland_state,
            draw: draw::DrawState::new(cli.stroke_width, cli.stroke_color),
            mouse: mouse::MouseState::default(),
            tablet: tablet::TabletState::default(),
            wgpu: None,
        };

        (state, event_queue)
    }

    pub fn toggle_input(&mut self, qhandle: &QueueHandle<Self>) {
        if self.active {
            self.deactivate(qhandle);
        } else {
            self.activate();
        }
    }

    pub fn activate(&mut self) {
        // reset to full region
        log!(target: "chameleos::general", Level::Info, "activate");
        self.wayland.surface.set_input_region(None);
        self.wayland.surface.commit();

        self.active = true;
    }

    pub fn deactivate(&mut self, qhandle: &QueueHandle<Self>) {
        log!(target: "chameleos::general", Level::Info, "deactivate");
        let empty_region = self.wayland.compositor.create_region(qhandle, ());
        self.wayland.surface.set_input_region(Some(&empty_region));
        self.wayland.surface.commit();

        self.active = false;
    }

    pub fn undo(&mut self) {
        self.draw.undo();
    }

    pub fn clear(&mut self) {
        self.draw.clear();
    }

    pub fn set_stroke_width(&mut self, width: f32) {
        self.draw.set_stroke_width(width);
    }

    pub fn set_stroke_color(&mut self, color: csscolorparser::Color) {
        self.draw.set_stroke_color(color);
    }

    fn render(&mut self) {
        if let Some(ref wgpu) = self.wgpu {
            self.draw.render(wgpu);
        }
    }

    fn force_render(&mut self) {
        if let Some(ref wgpu) = self.wgpu {
            self.draw.force_render(wgpu);
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.wgpu.take();
    }
}

#[allow(unused)]
struct WaylandState {
    connection: Connection,
    display: WlDisplay,
    compositor: WlCompositor,
    surface: WlSurface,
    seat: WlSeat,

    layer_shell: ZwlrLayerShellV1,
    layer_surface: ZwlrLayerSurfaceV1,

    cursor_shape_manager: WpCursorShapeManagerV1,
    tablet_manager: ZwpTabletManagerV2,
}

delegate_log!(WlCompositor);
delegate_log!(WlSurface);
delegate_log!(WlRegion);

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "WlSeat: {:?}", event);

        use wayland_client::protocol::wl_seat::Capability;
        use wayland_client::protocol::wl_seat::Event;
        match event {
            Event::Capabilities { capabilities } => match capabilities {
                WEnum::Value(capabilities) => {
                    if capabilities.contains(Capability::Pointer) {
                        let pointer = seat.get_pointer(qhandle, ());
                        let device =
                            state
                                .wayland
                                .cursor_shape_manager
                                .get_pointer(&pointer, qhandle, ());
                        state.mouse.set_cursor_shape_device(device);
                    }
                }
                WEnum::Unknown(_) => {}
            },
            Event::Name { name: _ } => {
                let tablet_seat = state
                    .wayland
                    .tablet_manager
                    .get_tablet_seat(seat, qhandle, ());
                state.tablet.set_tablet_seat(tablet_seat);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        _callback: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Trace, "WlCallback: {:?}", event);

        use wayland_client::protocol::wl_callback::Event;
        match event {
            Event::Done { callback_data: _ } => {
                state.render();

                state.wayland.surface.frame(qhandle, ());
                state.wayland.surface.commit();
            }
            _ => {}
        }
    }
}

delegate_log!(ZwlrLayerShellV1);
impl Dispatch<ZwlrLayerSurfaceV1, Option<Backend>> for State {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as Proxy>::Event,
        force_backend: &Option<Backend>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "LayerSurface: {:?}", event);

        use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event;
        match event {
            Event::Configure {
                serial,
                width,
                height,
            } => {
                layer_surface.ack_configure(serial);

                if state.wgpu.is_none() {
                    state.draw.set_height(height);

                    let wgpu = WgpuState::new(
                        &state.wayland.display,
                        &state.wayland.surface,
                        width,
                        height,
                        *force_backend,
                    );

                    if wgpu.surface_config().alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied {
                        state.draw.set_pre_multiply_stroke_color(true);
                    }

                    state.wgpu = Some(wgpu);

                    // some compositors are unhappy if we don't force render here
                    state.force_render();
                }
            }
            Event::Closed => {}
            _ => {}
        }
    }
}

delegate_dispatch!(State: [WlPointer: ()] => mouse::MouseState);

delegate_log!(WpCursorShapeManagerV1);
delegate_log!(WpCursorShapeDeviceV1);

delegate_log!(ZwpTabletManagerV2);
delegate_dispatch!(State: [ZwpTabletSeatV2: ()] => tablet::TabletState);
delegate_log!(ZwpTabletV2);
delegate_dispatch!(State: [ZwpTabletToolV2: ()] => tablet::TabletState);
delegate_log!(ZwpTabletPadV2);
