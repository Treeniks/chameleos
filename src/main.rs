#![allow(unused)]

mod shader;
use shader::*;

use wgpu::util::DeviceExt;
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

const EPSILON: f32 = 5.0;
const SAMPLE_COUNT: u32 = 4;

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

        wpgu: None,

        lines: Vec::new(),
    };

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }

    // TODO maybe should do some better cleanup?
}

struct State {
    active: bool,
    width: usize,
    height: usize,

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

    wpgu: Option<Wgpu>,

    lines: Vec<Vec<(f32, f32)>>,
}

struct Wgpu {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,

    multisampled_texture: wgpu::Texture,
    multisampled_texture_view: wgpu::TextureView,

    render_pipeline: wgpu::RenderPipeline,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,

    screen_buffer: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
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

    fn tessellate(&self) -> lyon::tessellation::VertexBuffers<Vertex, u16> {
        // TODO cache previous lines

        use lyon::math::point;
        use lyon::path::Path;
        use lyon::tessellation::BuffersBuilder;
        use lyon::tessellation::StrokeOptions;
        use lyon::tessellation::StrokeTessellator;
        use lyon::tessellation::StrokeVertex;
        use lyon::tessellation::VertexBuffers;

        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let mut builder = Path::builder();
        for line in self.lines.iter() {
            if line.len() < 2 {
                continue;
            }

            builder.begin(point(line[0].0, line[0].1));
            for &(x, y) in line.iter().skip(1) {
                builder.line_to(point(x, y));
            }
            builder.end(false);
        }
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

        geometry
    }

    fn render(&self) {
        let wgpu = self.wgpu();
        let mut geometry = self.tessellate();

        for _ in 0..(geometry.indices.len() as u64 % wgpu::COPY_BUFFER_ALIGNMENT) {
            geometry.indices.push(0);
        }

        wgpu.queue.write_buffer(
            &wgpu.vertex_buffer,
            0,
            bytemuck::cast_slice(&geometry.vertices),
        );
        wgpu.queue.write_buffer(
            &wgpu.index_buffer,
            0,
            bytemuck::cast_slice(&geometry.indices),
        );

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

        let swapchain_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = wgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &wgpu.multisampled_texture_view,
                depth_slice: None,
                resolve_target: Some(&swapchain_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&wgpu.render_pipeline);
        render_pass.set_bind_group(0, &wgpu.screen_bind_group, &[]);
        render_pass.set_vertex_buffer(0, wgpu.vertex_buffer.slice(..));
        render_pass.set_index_buffer(wgpu.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..geometry.indices.len() as u32, 0, 0..1);

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

                let multisampled_texture = wgpu_device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: width as u32,
                        height: height as u32,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: SAMPLE_COUNT,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu_config.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let multisampled_texture_view =
                    multisampled_texture.create_view(&wgpu::TextureViewDescriptor::default());

                // =====

                let screen = Screen {
                    size: [width as f32, height as f32],
                };
                let screen_buffer =
                    wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::bytes_of(&screen),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                let screen_bind_group_layout =
                    wgpu_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    });
                let screen_bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &screen_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: screen_buffer.as_entire_binding(),
                    }],
                });

                let shader = wgpu_device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
                let render_pipeline_layout =
                    wgpu_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[&screen_bind_group_layout],
                        push_constant_ranges: &[],
                    });
                let render_pipeline =
                    wgpu_device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&render_pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[Vertex::desc()],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: wgpu_config.format,
                                // TODO blending might need to be different
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleList,
                            strip_index_format: None,
                            front_face: wgpu::FrontFace::Ccw,
                            // NOTE no culling because lyon may not honor it
                            cull_mode: None,
                            unclipped_depth: false,
                            polygon_mode: wgpu::PolygonMode::Fill,
                            conservative: false,
                        },
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState {
                            count: SAMPLE_COUNT,
                            mask: !0,
                            alpha_to_coverage_enabled: false,
                        },
                        multiview: None,
                        cache: None,
                    });

                let vertex_buffer = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    // TODO needs to be increased if we run out
                    size: 0x1000000,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                let index_buffer = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    // TODO same as above
                    size: 0x1000000,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                let wgpu = Wgpu {
                    surface: wgpu_surface,
                    surface_config: wgpu_config,
                    device: wgpu_device,
                    queue: wgpu_queue,

                    multisampled_texture,
                    multisampled_texture_view,

                    render_pipeline,

                    vertex_buffer,
                    index_buffer,

                    screen_buffer,
                    screen_bind_group,
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
                if state.mouse_button_held {
                    let line = state.lines.last_mut().unwrap();
                    let new_x = surface_x as f32;
                    let new_y = state.height as f32 - surface_y as f32;
                    match line.last() {
                        Some((x, y)) => {
                            if f32::abs(x - new_x) + f32::abs(y - new_y) > EPSILON {
                                line.push((new_x, new_y));
                            }
                        }
                        None => {
                            line.push((new_x, new_y));
                        }
                    }

                    state.render();
                    // TODO maybe use surface callbacks for redrawing?
                    // state.surface().frame(qhandle, ());
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
                            }
                            wl_pointer::ButtonState::Pressed => {
                                state.mouse_button_held = true;
                                state.lines.push(Vec::new());
                            }
                            _ => {}
                        },
                        wayland_client::WEnum::Unknown(_) => {}
                    }
                }
            }
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
