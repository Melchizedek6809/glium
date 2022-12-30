#[macro_use]
extern crate glium;

use std::borrow::Borrow;
use std::num::NonZeroU32;
use glium::backend::glutin::WindowedContext;
use lazy_static::__Deref;
use winit::event_loop::EventLoopBuilder;
use winit::window::WindowBuilder;
use glium::{glutin, Surface};
use glium::index::PrimitiveType;
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::config::{ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder};
use glutin::surface::{SurfaceAttributesBuilder, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasRawWindowHandle;

fn main() {
    let event_loop = EventLoopBuilder::new().build();
    let window_builder = WindowBuilder::new();
    let template = ConfigTemplateBuilder::new();
    let display_builder = DisplayBuilder::new().with_window_builder(Some(window_builder));

    let (window, gl_config) = display_builder
        .build(&event_loop, template, |configs| {
            // Find the config with the maximum number of samples, so our triangle will
            // be smooth.
            configs
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() > accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        })
        .unwrap();

    let raw_window_handle = window.as_ref().map(|window| window.raw_window_handle());
    let display = gl_config.display();
    let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(raw_window_handle);

    let mut surface = None;
    let mut glium_context = None;
    let mut vertex_buffer = None;
    let mut index_buffer = None;
    let mut program = None;

    // the main loop
    event_loop.run(move |event, _window_target, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;
        match event {
            winit::event::Event::Resumed => {
                let not_current_gl_context = Some(unsafe {
                    display.create_context(&gl_config, &context_attributes).unwrap_or_else(|_| {
                        display
                            .create_context(&gl_config, &fallback_context_attributes)
                            .expect("failed to create context")
                    })
                });

                let (width, height): (u32, u32) = window.as_ref().unwrap().inner_size().into();
                let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
                    raw_window_handle.unwrap(),
                    NonZeroU32::new(width).unwrap(),
                    NonZeroU32::new(height).unwrap(),
                );

                surface = Some(unsafe { gl_config.display().create_window_surface(&gl_config, &attrs).unwrap() });
                let current_context = not_current_gl_context.unwrap().make_current(surface.as_ref().unwrap()).unwrap();
                glium_context = Some(glium::Display::from_current_context(current_context).unwrap());

                vertex_buffer = Some({
                    #[derive(Copy, Clone)]
                    struct Vertex {
                        position: [f32; 2],
                        color: [f32; 3],
                    }

                    implement_vertex!(Vertex, position, color);

                    glium::VertexBuffer::new(glium_context.as_ref().unwrap(),
                        &[
                            Vertex { position: [-0.5, -0.5], color: [0.0, 1.0, 0.0] },
                            Vertex { position: [ 0.0,  0.5], color: [0.0, 0.0, 1.0] },
                            Vertex { position: [ 0.5, -0.5], color: [1.0, 0.0, 0.0] },
                        ]
                    ).unwrap()
                });

                // building the index buffer
                index_buffer = Some(glium::IndexBuffer::new(glium_context.as_ref().unwrap(), PrimitiveType::TrianglesList,
                                                           &[0u16, 1, 2]).unwrap());

                // compiling shaders and linking them together
                program = Some(program!(glium_context.as_ref().unwrap(),
                    100 => {
                        vertex: "
                            #version 100

                            uniform lowp mat4 matrix;

                            attribute lowp vec2 position;
                            attribute lowp vec3 color;

                            varying lowp vec3 vColor;

                            void main() {
                                gl_Position = vec4(position, 0.0, 1.0) * matrix;
                                vColor = color;
                            }
                        ",

                        fragment: "
                            #version 100
                            varying lowp vec3 vColor;

                            void main() {
                                gl_FragColor = vec4(vColor, 1.0);
                            }
                        ",
                    },
                ).unwrap());
            },
            winit::event::Event::Suspended => {
                surface = None;
                glium_context = None;
                vertex_buffer = None;
                index_buffer = None;
                program = None;
            },
            winit::event::Event::RedrawRequested(_) => {
                // building the uniforms
                let uniforms = uniform! {
                    matrix: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0f32]
                    ]
                };

                // drawing a frame
                let mut target = glium_context.as_ref().unwrap().draw();
                target.clear_color(0.0, 0.0, 0.0, 0.0);
                target.draw(vertex_buffer.as_ref().unwrap(), index_buffer.as_ref().unwrap(), program.as_ref().unwrap(), &uniforms, &Default::default()).unwrap();
                target.finish().unwrap();

                let wc = glium_context.as_ref().unwrap().gl_window();
                if let WindowedContext::PossiblyCurrent { context, .. } = wc.borrow().deref().deref() {
                    surface.as_ref().unwrap().swap_buffers(context).unwrap();
                }
            }
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => *control_flow = winit::event_loop::ControlFlow::Exit,
                winit::event::WindowEvent::Resized(size) => {
                    glium_context.as_ref().unwrap().gl_window().set_framebuffer_dimensions(size.into());
                }, // request_redraw
                _ => (),
            },
            _ => (),
        };
    });
}
