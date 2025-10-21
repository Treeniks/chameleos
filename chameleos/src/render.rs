use log::Level;
use log::log;

use crate::shader::*;

use lyon::tessellation::VertexBuffers;

use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use wayland_client::protocol::wl_surface::WlSurface;

use wgpu::util::DeviceExt;

const SAMPLE_COUNT: u32 = 4;

#[derive(Debug, Clone)]
pub struct Geometry {
    vertex_buffers: VertexBuffers<Vertex, u16>,
    og_index_buffer_length: usize,
}

impl Geometry {
    pub fn new(mut geometry: VertexBuffers<Vertex, u16>) -> Self {
        // write_buffer wants multiples of `wgpu::COPY_BUFFER_ALIGNMENT`
        // should be fine for vertices, but indices might not be
        // so we need to extend the index buffer a bit, but also remember the original length
        let og_index_buffer_length = geometry.indices.len();
        for _ in 0..(geometry.indices.len() as u64 % wgpu::COPY_BUFFER_ALIGNMENT) {
            geometry.indices.push(0);
        }

        Self {
            vertex_buffers: geometry,
            og_index_buffer_length,
        }
    }
}

pub struct Wgpu {
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

impl Wgpu {
    pub fn new(
        display: &WlDisplay,
        surface: &WlSurface,
        width: u32,
        height: u32,
        stroke_color: &csscolorparser::Color,
    ) -> Self {
        let wgpu_instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let raw_display_handle = raw_window_handle::RawDisplayHandle::Wayland(
            raw_window_handle::WaylandDisplayHandle::new(
                std::ptr::NonNull::new(display.id().as_ptr() as *mut _).unwrap(),
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

        let wgpu_adapter =
            pollster::block_on(wgpu_instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&wgpu_surface),
            }))
            .unwrap();

        let info = wgpu_adapter.get_info();
        log!(target: "chameleos::render", Level::Info, "{:?}", wgpu_adapter.get_info());
        println!("GPU: {}", info.name);
        println!("Device Type: {:?}", info.device_type);
        println!("Driver: {} {}", info.driver, info.driver_info);
        println!("Backend: {}", info.backend);

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

        let surface_caps = wgpu_surface.get_capabilities(&wgpu_adapter);
        log!(target: "chameleos::render", Level::Info, "{:?}", surface_caps);

        let format = surface_caps
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = surface_caps
            .present_modes
            .iter()
            .find(|present_mode| matches!(present_mode, wgpu::PresentMode::Mailbox))
            .copied()
            // docs say this one is guaranteed to work
            .unwrap_or(wgpu::PresentMode::Fifo);

        // only PreMultiplied for now
        let alpha_mode = surface_caps
            .alpha_modes
            .iter()
            .find(|alpha_mode| matches!(alpha_mode, wgpu::CompositeAlphaMode::PreMultiplied))
            .copied()
            // TODO This will often fall back to Opaque which *should* show nothing but a black
            // screen. Except for some reason sometimes it also just works with Opaque (looking at
            // you Intel iGPU). No idea why.
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        log!(target: "chameleos::render", Level::Info, "Format: {:?}", format);
        log!(target: "chameleos::render", Level::Info, "Present Mode: {:?}", present_mode);
        log!(target: "chameleos::render", Level::Info, "Alpha Mode: {:?}", alpha_mode);

        // https://docs.rs/wgpu/latest/wgpu/struct.SurfaceCapabilities.html
        let wgpu_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
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

        let uniform = Uniform {
            screen_size: [width as f32, height as f32],
        };
        let uniform_buffer = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let uniform_bind_group_layout =
            wgpu_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let shader = wgpu_device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let render_pipeline_layout =
            wgpu_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline = wgpu_device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::DESC],
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

        Self {
            surface: wgpu_surface,
            surface_config: wgpu_config,
            device: wgpu_device,
            queue: wgpu_queue,

            multisampled_texture,
            multisampled_texture_view,

            render_pipeline,

            vertex_buffer,
            index_buffer,

            screen_buffer: uniform_buffer,
            screen_bind_group: uniform_bind_group,
        }
    }

    pub fn render<'a>(&self, geometries: impl IntoIterator<Item = &'a Geometry>) {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                self.surface.get_current_texture().unwrap()
            }
            _ => {
                panic!();
            }
        };

        let swapchain_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.multisampled_texture_view,
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

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.screen_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        let mut v_offset = 0;
        let mut i_offset = 0;
        for geometry in geometries.into_iter() {
            let geom = &geometry.vertex_buffers;

            self.queue.write_buffer(
                &self.vertex_buffer,
                v_offset * std::mem::size_of::<Vertex>() as u64,
                bytemuck::cast_slice(&geom.vertices),
            );
            self.queue.write_buffer(
                &self.index_buffer,
                i_offset * std::mem::size_of::<u16>() as u64,
                bytemuck::cast_slice(&geom.indices),
            );
            render_pass.draw_indexed(
                i_offset as u32..i_offset as u32 + geometry.og_index_buffer_length as u32,
                v_offset as i32,
                0..1,
            );

            v_offset += geom.vertices.len() as u64;
            i_offset += geom.indices.len() as u64;
        }

        drop(render_pass);

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }
}
