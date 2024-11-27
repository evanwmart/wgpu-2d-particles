// Standard library imports
use std::iter; // Provides utility methods for iterator operations

use wgpu::util::DeviceExt;
use rand::Rng;

// Imports from the winit crate for windowing and event handling
use winit::{
    event::*, // Handles various types of events such as keyboard and mouse input
    event_loop::EventLoop, // Main event loop to handle window events
    keyboard::{ KeyCode, PhysicalKey }, // For handling keyboard events by key code
    window::{ Window, WindowBuilder }, // Used to create and manage windows
};

// Import for WebAssembly (wasm32) target, if applicable
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Particle {
    position: [f32; 2],
    velocity: [f32; 2],
}

// The main state struct which holds all resources needed for rendering
struct State<'a> {
    surface: wgpu::Surface<'a>, // Surface that represents the part of the window where rendering occurs
    device: wgpu::Device, // Represents the GPU and handles resource management
    queue: wgpu::Queue, // Handles the submission of commands to the GPU
    config: wgpu::SurfaceConfiguration, // Configuration for the surface, including display format and resolution
    size: winit::dpi::PhysicalSize<u32>, // Window size in physical pixels
    window: &'a Window, // Reference to the window instance for rendering
    render_pipeline: wgpu::RenderPipeline, // The pipeline object that contains rendering configurations
    particles: Vec<Particle>, //
    particle_buffer: wgpu::Buffer,
}

// Implementation of the State struct
impl<'a> State<'a> {
    // Asynchronous method to initialize a new State instance
    async fn new(window: &'a Window) -> State<'a> {
        let size = window.inner_size(); // Get the initial window size

        // Create an instance for interfacing with the GPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY, // Use primary backend on native platforms
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL, // Use OpenGL backend for WebAssembly
            ..Default::default()
        });

        // Create a surface for rendering in the window
        let surface = instance.create_surface(window).unwrap();

        // Request a GPU adapter that meets the preferred criteria
        let adapter = instance
            .request_adapter(
                &(wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance, // Prefer high-performance GPU
                    compatible_surface: Some(&surface), // Ensure adapter is compatible with the surface
                    force_fallback_adapter: true, // Allow fallback if no compatible adapter found
                })
            ).await
            .or_else(|| {
                // Fallback to manually enumerating adapters if preferred adapter is not available
                instance
                    .enumerate_adapters(wgpu::Backends::all())
                    .into_iter()
                    .find(|adapter| adapter.is_surface_supported(&surface))
            })
            .expect("Failed to find a compatible GPU adapter");

        // Request a logical device and a command queue from the adapter
        let (device, queue) = adapter
            .request_device(
                &(wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults() // WebGL 2 defaults for wasm
                    } else {
                        wgpu::Limits::default() // Default limits for native
                    },
                    memory_hints: Default::default(),
                }),
                None
            ).await
            .unwrap();

        // Get the supported formats and modes for the surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats
            .iter()
            .copied()
            .find(|f| f.is_srgb()) // Prefer sRGB format for better color accuracy
            .unwrap_or(surface_caps.formats[0]);

        // Configure the surface with specified usage and format
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // Usage for render output
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        // Load the WGSL shader code from an external file
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Set up the render pipeline layout with an empty layout as no resources are bound
        let render_pipeline_layout = device.create_pipeline_layout(
            &(wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            })
        );

        // Create the render pipeline, specifying shaders, topology, and blend options
        let render_pipeline = device.create_render_pipeline(
            &(wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Particle>() as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    offset: 0,
                                    shader_location: 0,
                                    format: wgpu::VertexFormat::Float32x2, // position
                                },
                            ],
                        },
                    ],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: config.format,
                            // Enable alpha blending
                            blend: Some(wgpu::BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::SrcAlpha,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
        );        

        // Configure the surface with device and configuration
        surface.configure(&device, &config);

        let mut rng = rand::thread_rng();
        let particles = (0..1000)
            .map(|_| Particle {
                position: [
                    rng.gen_range(-1.0..1.0), // Random x position
                    rng.gen_range(-1.0..1.0), // Random y position
                ],
                velocity: [
                    rng.gen_range(-0.00001..0.00019), // Random x velocity
                    rng.gen_range(-0.0008..-0.0003), // Random x velocity
                ],
            })
            .collect::<Vec<_>>();

        let particle_buffer = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Particle Buffer"),
                contents: bytemuck::cast_slice(&particles),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            })
        );

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            render_pipeline,
            particles,
            particle_buffer,
        }
    }

    // Accessor for the window reference
    fn window(&self) -> &Window {
        &self.window
    }

    // Resize handler to update surface configuration if the window size changes
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    // Handles input events, returning false as no input handling is done in this example
    #[allow(unused_variables)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    // Update function (empty in this example as no animations or transformations are applied)
    fn update(&mut self) {
        for particle in &mut self.particles {
            particle.position[0] += particle.velocity[0];
            particle.position[1] += particle.velocity[1];
        
            // Wrap around horizontally
            if particle.position[0] > 1.1 {
                particle.position[0] = -1.1;
            } else if particle.position[0] < -1.1 {
                particle.position[0] = 1.1;
            }
        
            // Wrap around vertically
            if particle.position[1] > 1.1 {
                particle.position[1] = -1.1;
            } else if particle.position[1] < -1.1 {
                particle.position[1] = 1.1;
            }
        
        }

        // Update the particle buffer on the GPU
        self.queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&self.particles));
    }

    // Render function that performs the drawing operations
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?; // Get the next texture for rendering
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default()); // Create a view for the texture

        let mut encoder = self.device.create_command_encoder(
            &(wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
        );

        // Start the render pass
        let mut render_pass = encoder.begin_render_pass(
            &(wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.05,
                                g: 0.06,
                                b: 0.09,
                                a: 1.0, // Background color for clearing the screen
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            })
        );

        render_pass.set_pipeline(&self.render_pipeline); // Set the render pipeline
        render_pass.set_vertex_buffer(0, self.particle_buffer.slice(..));

        render_pass.draw(0..6, 0..self.particles.len() as u32);

        drop(render_pass); // End the render pass

        self.queue.submit(iter::once(encoder.finish())); // Submit the command buffer for execution
        output.present(); // Present the rendered image to the window

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook)); // Set a panic hook for better error messages in wasm
            console_log::init_with_level(log::Level::Info).expect("Couldn't initialize logger"); // Initialize logging for wasm
        } else {
            env_logger::init();
        } // Initialize logging for native platforms
    }

    let event_loop = EventLoop::new().unwrap(); // Create the event loop to handle window events
    let window = WindowBuilder::new().build(&event_loop).unwrap(); // Build the main application window

    #[cfg(target_arch = "wasm32")]
    {
        use winit::dpi::PhysicalSize;
        use winit::platform::web::WindowExtWebSys;

        // Append the window canvas to the web document when running as WebAssembly
        web_sys
            ::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas()?);
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");

        let _ = window.request_inner_size(PhysicalSize::new(450, 400)); // Set initial size for WebAssembly window
    }

    let mut state = State::new(&window).await; // Initialize the rendering state

    event_loop
        .run(move |event, control_flow| {

            state.window().request_redraw();

            match event {
                Event::WindowEvent { ref event, window_id } if window_id == state.window().id() => {
                    if !state.input(event) {
                        // Handle window events such as closing and resizing
                        match event {

                            WindowEvent::CloseRequested
                            | WindowEvent::KeyboardInput {
                                  event: KeyEvent {
                                      state: ElementState::Pressed,
                                      physical_key: PhysicalKey::Code(KeyCode::Escape),
                                      ..
                                  },
                                  ..
                              } => control_flow.exit(), // Exit on escape key or close request
                            
                            WindowEvent::Resized(physical_size) => {
                                state.resize(*physical_size); // Handle window resize
                            }
                            WindowEvent::RedrawRequested => {

                                state.update(); // Update application state
                                
                                match state.render() {
                                    Ok(_) => {}
                                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) =>
                                        state.resize(state.size),
                                    Err(wgpu::SurfaceError::OutOfMemory) => control_flow.exit(), // Exit on out of memory error
                                    Err(wgpu::SurfaceError::Timeout) =>
                                        log::warn!("Surface timeout"), // Log timeout warnings
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        })
        .unwrap();
}
