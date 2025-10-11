#![allow(unused)]

use xkbcommon::xkb;

use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::Proxy;
use wayland_client::QueueHandle;

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
    let connection = wayland_client::Connection::connect_to_env().unwrap();
    let mut event_queue: wayland_client::EventQueue<State> = connection.new_event_queue();

    let display = connection.display();
    display.get_registry(&event_queue.handle(), ());

    let mut state = State::default();
    state.active = true;

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }
}

#[derive(Default)]
struct State {
    active: bool,

    compositor: Option<WlCompositor>,
    surface: Option<WlSurface>,
    seat: Option<WlSeat>,

    shm: Option<WlShm>,
    pool: Option<WlShmPool>,
    buffer: Option<WlBuffer>,
    mmap: Option<memmap2::MmapMut>,

    layer_shell: Option<ZwlrLayerShellV1>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,

    keyboard: Option<WlKeyboard>,
    xkb_state: Option<xkb::State>,
    pointer: Option<WlPointer>,
}

impl State {
    fn compositor(&self) -> &WlCompositor {
        self.compositor.as_ref().unwrap()
    }

    fn surface(&self) -> &WlSurface {
        self.surface.as_ref().unwrap()
    }

    fn shm(&self) -> &WlShm {
        self.shm.as_ref().unwrap()
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
                "wl_shm" => {
                    let shm = registry.bind::<WlShm, _, _>(name, 2, qhandle, *data);
                    state.shm = Some(shm);
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
                use std::os::fd::AsFd;

                use memmap2::MmapMut;
                use nix::sys::memfd;

                let surface = state.surface();
                let shm = state.shm();

                let mem_fd =
                    memfd::memfd_create("chameleos", memfd::MFdFlags::MFD_CLOEXEC).unwrap();
                let mem_file = std::fs::File::from(mem_fd);
                mem_file.set_len((width * height * 4) as u64);
                let mmap = unsafe { MmapMut::map_mut(&mem_file).unwrap() };

                let pool = shm.create_pool(
                    mem_file.as_fd(),
                    (width * height * 4) as i32,
                    qhandle,
                    *data,
                );
                let buffer = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    (width * 4) as i32,
                    wl_shm::Format::Argb8888,
                    qhandle,
                    *data,
                );

                surface.attach(Some(&buffer), 0, 0);
                surface.commit();

                state.pool = Some(pool);
                state.buffer = Some(buffer);
                state.mmap = Some(mmap);
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

impl Dispatch<WlShm, ()> for State {
    fn event(
        state: &mut Self,
        shm: &WlShm,
        event: <WlShm as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        println!("WlShm: {:?}", event);
    }
}

impl Dispatch<WlShmPool, ()> for State {
    fn event(
        state: &mut Self,
        pool: &WlShmPool,
        event: <WlShmPool as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        println!("WlShmPool: {:?}", event);
    }
}

impl Dispatch<WlBuffer, ()> for State {
    fn event(
        state: &mut Self,
        buffer: &WlBuffer,
        event: <WlBuffer as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        println!("WlBuffer: {:?}", event);
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
    }
}
