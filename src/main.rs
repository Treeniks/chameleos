#![allow(unused)]

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

use wayland_client::protocol::wl_seat;
use wayland_client::protocol::wl_seat::WlSeat;

use wayland_client::protocol::wl_shm;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;

use wayland_client::protocol::wl_buffer;
use wayland_client::protocol::wl_buffer::WlBuffer;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

fn main() {
    env_logger::init();

    let connection = wayland_client::Connection::connect_to_env().unwrap();
    let mut event_queue: wayland_client::EventQueue<State> = connection.new_event_queue();

    let display = connection.display();
    display.get_registry(&event_queue.handle(), ());

    let mut state = State {
        active: true,
        width: 0,
        height: 0,

        display,
        compositor: None,
        surface: None,
        seat: None,

        layer_shell: None,
        layer_surface: None,

        keyboard: None,
        xkb_state: None,
        pointer: None,

        wpgu: None,

        v: 0.0,
    };

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }
}

struct State {
    active: bool,
    width: usize,
    height: usize,

    display: WlDisplay,
    compositor: Option<WlCompositor>,
    surface: Option<WlSurface>,
    seat: Option<WlSeat>,

    layer_shell: Option<ZwlrLayerShellV1>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,

    keyboard: Option<WlKeyboard>,
    xkb_state: Option<xkb::State>,
    pointer: Option<WlPointer>,

    wpgu: Option<Wgpu>,

    v: f64,
}

struct Wgpu {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
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

    fn wgpu(&self) -> &Wgpu {
        self.wpgu.as_ref().unwrap()
    }

    fn toggle_input(&self, qhandle: &QueueHandle<Self>) {
        let compositor = self.compositor();
        let surface = self.surface();
        let layer_surface = self.layer_surface();

        if self.active {
            // make inactive
            let empty_region = compositor.create_region(qhandle, ());
            surface.set_input_region(Some(&empty_region));
            layer_surface
                .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
            surface.commit();
        }
        // TODO make active
    }

    fn render(&self) {
        let wgpu = self.wgpu();

        let r = self.v / self.width as f64;

        let output = match wgpu.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Outdated) => {
                wgpu.surface.configure(&wgpu.device, &wgpu.surface_config);
                wgpu.surface.get_current_texture().unwrap()
            }
            _ => {
                panic!();
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = wgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r,
                        g: 0.2,
                        b: 0.3,
                        a: 0.5,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        drop(render_pass);

        wgpu.queue.submit(Some(encoder.finish()));
        output.present();
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
        println!("WlRegistry: {:?}", event);
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
                    surface.commit();

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
        println!("LayerSurface: {:?}", event);
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                state.width = width as usize;
                state.height = height as usize;

                let surface = state.surface();
                let layer_surface = state.layer_surface();

                let wgpu_instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                });

                let raw_display_handle = raw_window_handle::RawDisplayHandle::Wayland(
                    raw_window_handle::WaylandDisplayHandle::new(
                        std::ptr::NonNull::new(state.display.id().as_ptr() as *mut _).unwrap(),
                    ),
                );
                let raw_window_handle = raw_window_handle::RawWindowHandle::Wayland(
                    raw_window_handle::WaylandWindowHandle::new(
                        std::ptr::NonNull::new(surface.id().as_ptr() as *mut _).unwrap(),
                    ),
                );

                let wgpu_surface = unsafe {
                    wgpu_instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: raw_display_handle,
                        raw_window_handle: raw_window_handle,
                    })
                }
                .unwrap();

                let wgpu_adapter = pollster::block_on(wgpu_instance.request_adapter(
                    &wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::default(),
                        force_fallback_adapter: false,
                        compatible_surface: Some(&wgpu_surface),
                    },
                ))
                .unwrap();

                println!("GPU selected: {}", wgpu_adapter.get_info().name);

                let (wgpu_device, wgpu_queue) =
                    pollster::block_on(wgpu_adapter.request_device(&wgpu::DeviceDescriptor {
                        label: None,
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                        experimental_features: wgpu::ExperimentalFeatures::disabled(),
                        memory_hints: wgpu::MemoryHints::default(),
                        trace: wgpu::Trace::Off,
                    }))
                    .unwrap();

                let wgpu_surface_caps = wgpu_surface.get_capabilities(&wgpu_adapter);
                let wgpu_surface_format = wgpu_surface_caps
                    .formats
                    .iter()
                    .find(|f| f.is_srgb())
                    .copied()
                    .unwrap_or(wgpu_surface_caps.formats[0]);
                // only PreMultiplied for now
                let wgpu_alpha_mode = wgpu_surface_caps
                    .alpha_modes
                    .iter()
                    .find(|a| matches!(a, wgpu::CompositeAlphaMode::PreMultiplied))
                    .copied()
                    .unwrap();

                // https://docs.rs/wgpu/latest/wgpu/struct.SurfaceCapabilities.html
                let wgpu_config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: wgpu_surface_format,
                    width,
                    height,
                    // TODO I think this currently defaults to Fifo (VSync)
                    present_mode: wgpu_surface_caps.present_modes[0],
                    desired_maximum_frame_latency: 2,
                    alpha_mode: wgpu_alpha_mode,
                    view_formats: vec![],
                };

                wgpu_surface.configure(&wgpu_device, &wgpu_config);

                let wgpu = Wgpu {
                    surface: wgpu_surface,
                    surface_config: wgpu_config,
                    device: wgpu_device,
                    queue: wgpu_queue,
                };

                state.wpgu = Some(wgpu);

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
        println!("LayerShell: {:?}", event);
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
        println!("WlCompositor: {:?}", event);
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
        println!("WlSurface: {:?}", event);
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
        println!("WlSeat: {:?}", event);
        match event {
            wl_seat::Event::Capabilities { capabilities } => match capabilities {
                wayland_client::WEnum::Value(capabilities) => {
                    if capabilities.contains(wl_seat::Capability::Keyboard) {
                        let keyboard = seat.get_keyboard(qhandle, *data);
                        state.keyboard = Some(keyboard);
                    }

                    if capabilities.contains(wl_seat::Capability::Pointer) {
                        let pointer = seat.get_pointer(qhandle, *data);
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
        println!("WlRegion: {:?}", event);
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
        println!("WlKeyboard: {:?}", event);
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
                let sym = state.xkb_state().key_get_one_sym(key_code);

                if sym == xkb::Keysym::x {
                    state.toggle_input(qhandle);
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
        proxy: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        println!("WlPointer: {:?}", event);
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {}
            wl_pointer::Event::Leave { serial, surface } => {}
            wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                state.v = surface_x;

                state.render();

                // TODO maybe use surface callbacks for redrawing?
                // state.surface().frame(qhandle, ());
            }
            wl_pointer::Event::Button {
                serial,
                time,
                button,
                state,
            } => {}
            wl_pointer::Event::Axis { time, axis, value } => {}
            wl_pointer::Event::Frame => {}
            wl_pointer::Event::AxisSource { axis_source } => {}
            wl_pointer::Event::AxisStop { time, axis } => {}
            wl_pointer::Event::AxisDiscrete { axis, discrete } => {}
            wl_pointer::Event::AxisValue120 { axis, value120 } => {}
            wl_pointer::Event::AxisRelativeDirection { axis, direction } => {}
            _ => {}
        }
    }
}
