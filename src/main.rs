/*
TODO(JaSc):
  - Pixel perfect renderer with generalized (pixel independent) coordinate system
    - Render to offscreen buffer and blit to main screen
    - Static world camera 
    - Transformation mouse <-> screen <-> world 
  - Basic sprite loading and Bitmap font rendering (no sprite atlas yet)
  - Game input + keyboard/mouse-support
  - Gamestate + logic + timing
  - Audio playback
  - Some nice glowing shader effects
  - BG music with PHAT BEATSIES

BACKLOG(JaSc):
  - The following are things to remember to extract out of an old C project
    x Debug macro to print a variable and it's name quickly
    - Be able to conveniently do debug printing on screen
    - Moving camera system
    - Atlas textures and sprite/quad/line-batching
    - Atlas and font packer
    - Texture array of atlases implementation
    - Drawing debug overlays (grids/camera-frustums/crosshairs/depthbuffer)
    - Gamepad input
    - Mouse zooming
    - Raycasting and collision detection
    - Fixed sized and flexible sized pixel perfect canvases (framebuffers)
*/

#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

#[macro_use]
extern crate log;
extern crate cgmath;
extern crate fern;
extern crate image;
extern crate rand;

use gfx::traits::Factory;
use gfx::traits::FactoryExt;
use gfx::Device;
use glutin::GlContext;

use cgmath::prelude::*;
use rand::prelude::*;

type ColorFormat = gfx::format::Rgba8;
type DepthFormat = gfx::format::DepthStencil;
type Point2 = cgmath::Point2<f32>;
type Vec2 = cgmath::Vector2<f32>;
type Color = cgmath::Vector4<f32>;
type Mat4 = cgmath::Matrix4<f32>;

gfx_defines! {
    vertex Vertex {
        pos: [f32; 4] = "a_Pos",
        uv: [f32; 2] = "a_Uv",
        color: [f32; 4] = "a_Color",
    }

    pipeline screen_pipe {
        vertex_buffer: gfx::VertexBuffer<Vertex> = (),
        transform: gfx::Global<[[f32; 4];4]> = "u_Transform",
        texture: gfx::TextureSampler<[f32; 4]> = "u_Sampler",
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
    }

    pipeline canvas_pipe {
        vertex_buffer: gfx::VertexBuffer<Vertex> = (),
        transform: gfx::Global<[[f32; 4];4]> = "u_Transform",
        texture: gfx::TextureSampler<[f32; 4]> = "u_Sampler",
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
        out_depth: gfx::DepthTarget<DepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn from_dimension(width: f32, height: f32) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    fn from_corners(bottom_left: Point2, top_right: Point2) -> Rect {
        Rect {
            x: bottom_left.x,
            y: bottom_left.y,
            width: top_right.x - bottom_left.x,
            height: top_right.y - bottom_left.y,
        }
    }

    fn to_pos(&self) -> Point2 {
        Point2::new(self.x, self.y)
    }

    fn to_dim(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }
}

/// A macro used for debugging which returns a string containing the name and value of a given
/// variable.
///
/// It uses the `stringify` macro internally and requires the input to be an identifier.
///
/// # Examples
///
/// ```
/// let name = 5;
/// assert_eq!(dformat!(name), "name = 5");
/// ```
macro_rules! dformat {
    ($x:ident) => {
        format!("{} = {:?}", stringify!($x), $x)
    };
}

/// A macro used for debugging which prints a string containing the name and value of a given
/// variable.
///
/// It uses the `dformat` macro internally and requires the input to be an identifier.
/// For more information see the `dformat` macro
///
/// # Examples
///
/// ```
/// let name = 5;
/// dprintln!(name);
/// // prints: "name = 5"
/// ```
macro_rules! dprintln {
    ($x:ident) => {
        println!("{}", dformat!($x));
    };
}

fn main() {
    // Initializing logger
    //
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}-{}: {}",
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Trace)
        .level_for("gfx_device_gl", log::LevelFilter::Warn)
        .level_for("winit", log::LevelFilter::Warn)
        .chain(std::io::stdout())
        .apply()
        .expect("Could not initialize logger");

    // ---------------------------------------------------------------------------------------------
    // Video subsystem initialization
    //

    // TODO(JaSc): Read MONITOR_ID and FULLSCREEN_MODE from config file
    const MONITOR_ID: usize = 0;
    const FULLSCREEN_MODE: bool = false;
    const CANVAS_DIMENSIONS: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 320.0,
        height: 160.0,
    };
    const GL_VERSION_MAJOR: u8 = 3;
    const GL_VERSION_MINOR: u8 = 2;

    //
    info!("Getting monitor and its properties");
    //
    let mut events_loop = glutin::EventsLoop::new();
    let monitor = events_loop
        .get_available_monitors()
        .nth(MONITOR_ID)
        .unwrap_or_else(|| {
            panic!("No monitor with id {} found", MONITOR_ID);
        });
    let monitor_logical_dimensions = monitor
        .get_dimensions()
        .to_logical(monitor.get_hidpi_factor());
    info!(
        "Found monitor {} with logical dimensions: {:?}",
        MONITOR_ID,
        (
            monitor_logical_dimensions.width,
            monitor_logical_dimensions.height
        )
    );

    //
    info!("Creating window and drawing context");
    //
    let fullscreen_monitor = match FULLSCREEN_MODE {
        true => Some(monitor),
        false => None,
    };
    let window_builder = glutin::WindowBuilder::new()
        .with_resizable(!FULLSCREEN_MODE)
        // TODO(JaSc): Allow cursor grabbing in windowed mode when 
        //             https://github.com/tomaka/winit/issues/574
        //             is fixed. Grabbing the cursor in windowed mode and ALT-TABBING in and out
        //             is currently broken.
        .with_fullscreen(fullscreen_monitor)
        .with_title("Pongi".to_string());
    let context = glutin::ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(
            glutin::Api::OpenGl,
            (GL_VERSION_MAJOR, GL_VERSION_MINOR),
        ))
        .with_vsync(true);
    let (window, mut device, mut factory, screen_rendertarget, mut screen_depth_rendertarget) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(window_builder, context, &events_loop);

    //
    info!("Creating command buffer and shaders");
    //
    let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();
    let vertex_shader = include_bytes!("shaders/basic.glslv").to_vec();
    let fragment_shader = include_bytes!("shaders/basic.glslf").to_vec();

    //
    info!("Creating dummy texture and sampler");
    //
    use gfx::texture::{FilterMethod, SamplerInfo, WrapMode};
    let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
    let dummy_texture = debug_load_texture(&mut factory);
    let texture_sampler = factory.create_sampler(sampler_info);

    //
    info!("Creating offscreen render target and pipeline");
    //
    let (_, canvas_shader_resource_view, canvas_render_target_view) = factory
        .create_render_target::<ColorFormat>(
            CANVAS_DIMENSIONS.width as u16,
            CANVAS_DIMENSIONS.height as u16,
        )
        .unwrap();
    let canvas_depth_render_target_view = factory
        .create_depth_stencil_view_only::<DepthFormat>(
            CANVAS_DIMENSIONS.width as u16,
            CANVAS_DIMENSIONS.height as u16,
        )
        .unwrap();
    let canvas_pipeline_state_object = factory
        .create_pipeline_simple(&vertex_shader, &fragment_shader, canvas_pipe::new())
        .expect("Failed to create a pipeline state object");
    let mut canvas_pipeline_data = canvas_pipe::Data {
        vertex_buffer: factory.create_vertex_buffer(&[]),
        texture: (dummy_texture, texture_sampler.clone()),
        transform: Mat4::identity().into(),
        out_color: canvas_render_target_view,
        out_depth: canvas_depth_render_target_view,
    };

    //
    info!("Creating screen pipeline");
    //
    let screen_pipeline_state_object = factory
        .create_pipeline_simple(&vertex_shader, &fragment_shader, screen_pipe::new())
        .expect("Failed to create a pipeline state object");
    let mut screen_pipeline_data = screen_pipe::Data {
        vertex_buffer: factory.create_vertex_buffer(&[]),
        texture: (canvas_shader_resource_view, texture_sampler),
        transform: Mat4::identity().into(),
        out_color: screen_rendertarget,
    };

    // ---------------------------------------------------------------------------------------------
    // Main loop
    //

    // State variables
    let mut running = true;
    let mut cursor_pos = Point2::new(0.0, 0.0);

    let mut screen_rect = Rect::from_dimension(0.0, 0.0);
    let mut window_entered_fullscreen = false;

    //
    info!("Entering main event loop");
    info!("------------------------");
    //
    while running {
        use glutin::{Event, KeyboardInput, WindowEvent};
        events_loop.poll_events(|event| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => running = false,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: glutin::ElementState::Pressed,
                                virtual_keycode: Some(key),
                                // modifiers,
                                ..
                            },
                        ..
                    } => {
                        use glutin::VirtualKeyCode::*;
                        match key {
                            Escape => running = false,
                            _ => (),
                        }
                    }
                    WindowEvent::Focused(has_focus) => {
                        info!("Window has focus: {}", has_focus);
                        if FULLSCREEN_MODE && window_entered_fullscreen {
                            // NOTE: We need to grab/ungrab mouse cursor when ALT-TABBING in and out
                            //       or the user cannot use their computer correctly in a
                            //       multi-monitor setup while running our app.
                            window.grab_cursor(has_focus).unwrap();
                        }
                    }
                    WindowEvent::Resized(new_dim) => {
                        info!("Window resized: {:?}", (new_dim.width, new_dim.height));
                        window.resize(new_dim.to_physical(window.get_hidpi_factor()));
                        gfx_window_glutin::update_views(
                            &window,
                            &mut screen_pipeline_data.out_color,
                            &mut screen_depth_rendertarget,
                        );
                        screen_rect =
                            Rect::from_dimension(new_dim.width as f32, new_dim.height as f32);

                        // Grab mouse cursor in window
                        // NOTE: Due to https://github.com/tomaka/winit/issues/574 we need to first
                        //       make sure that our resized window now spans the full screen before
                        //       we allow grabbing the mouse cursor.
                        // TODO(JaSc): Remove workaround when upstream is fixed
                        if FULLSCREEN_MODE && new_dim == monitor_logical_dimensions {
                            // Our window now has its final size, we can safely grab the cursor now
                            info!("Mouse cursor grabbed");
                            window_entered_fullscreen = true;
                            window.grab_cursor(true).unwrap();
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let cursor_x = position.x as f32 / screen_rect.width;
                        let cursor_y = position.y as f32 / screen_rect.height;
                        cursor_pos = Point2::new(cursor_x - 0.5, -1.0 * cursor_y + 0.5);
                    }
                    _ => (),
                }
            }
        });

        // Aspect ratio correction for view and cursor
        let aspect_ratio = screen_rect.width / screen_rect.height;
        let (width, height) = if aspect_ratio > 1.0 {
            (1.0 * aspect_ratio, 1.0)
        } else {
            (1.0, 1.0 / aspect_ratio)
        };
        let cursor_pos = if aspect_ratio > 1.0 {
            Point2::new(cursor_pos.x * aspect_ratio, cursor_pos.y)
        } else {
            Point2::new(cursor_pos.x, cursor_pos.y / aspect_ratio)
        };

        // Draw canvas
        // -----------------------------------------------------------------------------------------
        let projection_mat = cgmath::ortho(
            -0.5 * width,
            0.5 * width,
            -0.5 * height,
            0.5 * height,
            -1.0,
            1.0,
        );

        let quad_color = Color::new(1.0, 0.0, 0.0, 1.0);
        let cursor_color = Color::new(0.0, 0.0, 0.0, 1.0);

        // Add dummy quad for cursor
        let dummy_quad = Quad::new(Rect::from_dimension(1.0, 1.0), 0.0, quad_color);
        let cursor_quad = Quad::new(
            Rect::new(cursor_pos.x, cursor_pos.y, 0.02, 0.02),
            -0.5,
            cursor_color,
        );

        let (mut canvas_vertices, mut canvas_indices) = (vec![], vec![]);
        dummy_quad.append_vertices_indices_centered(0, &mut canvas_vertices, &mut canvas_indices);
        cursor_quad.append_vertices_indices_centered(1, &mut canvas_vertices, &mut canvas_indices);
        let (canvas_vertex_buffer, canvas_slice) =
            factory.create_vertex_buffer_with_slice(&canvas_vertices, &*canvas_indices);

        canvas_pipeline_data.transform = projection_mat.into();
        canvas_pipeline_data.vertex_buffer = canvas_vertex_buffer;

        let canvas_color = Color::new(0.7, 0.4, 0.2, 1.0);
        encoder.clear(&canvas_pipeline_data.out_color, canvas_color.into());
        encoder.clear_depth(&canvas_pipeline_data.out_depth, 1.0);
        encoder.draw(
            &canvas_slice,
            &canvas_pipeline_state_object,
            &canvas_pipeline_data,
        );

        // Draw canvas to screen
        // -----------------------------------------------------------------------------------------

        let screen_quad = Quad::new(screen_rect, 0.0, Color::new(1.0, 1.0, 1.0, 1.0));
        let (mut screen_vertices, mut screen_indices) = (vec![], vec![]);
        screen_quad.append_vertices_indices(0, &mut screen_vertices, &mut screen_indices);
        let (screen_vertex_buffer, screen_slice) =
            factory.create_vertex_buffer_with_slice(&screen_vertices, &*screen_indices);

        // NOTE: The projection matrix is upside-down for correct rendering of the canvas
        let screen_projection_mat =
            cgmath::ortho(0.0, screen_rect.width, screen_rect.height, 0.0, -1.0, 1.0);

        screen_pipeline_data.vertex_buffer = screen_vertex_buffer;
        screen_pipeline_data.transform = screen_projection_mat.into();

        let screen_color = Color::new(0.2, 0.4, 0.7, 1.0);
        encoder.clear(&screen_pipeline_data.out_color, screen_color.into());
        encoder.draw(
            &screen_slice,
            &screen_pipeline_state_object,
            &screen_pipeline_data,
        );

        encoder.flush(&mut device);
        window.swap_buffers().expect("Failed to swap framebuffers");
        device.cleanup();
    }
}

// struct Context<C, R, F>
// where
//     R: gfx::Resources,
//     C: gfx::CommandBuffer<R>,
//     F: gfx::Factory<R>,
// {
//     factory: F,
//     encoder: gfx::Encoder<R, C>,
//
//     screen_pipeline_data: screen_pipe::Data<R>,
//     screen_pipeline_state_object: gfx::PipelineState<R, screen_pipe::Meta>,
//     screen_rendertarget: gfx::handle::RenderTargetView<R, ColorFormat>,
//
//     canvas_pipeline_data: canvas_pipe::Data<R>,
//     canvas_pipeline_state_object: gfx::PipelineState<R, canvas_pipe::Meta>,
//     canvas_shader_resource_view: gfx::handle::ShaderResourceView<R, [f32; 4]>,
//     canvas_depth_rendertarget: gfx::handle::DepthStencilView<R, DepthFormat>,
// }

fn debug_load_texture<F, R>(factory: &mut F) -> gfx::handle::ShaderResourceView<R, [f32; 4]>
where
    F: gfx::Factory<R>,
    R: gfx::Resources,
{
    use gfx::format::Rgba8;
    let img = image::open("resources/dummy.png").unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let kind = gfx::texture::Kind::D2(width as u16, height as u16, gfx::texture::AaMode::Single);
    let (_, view) = factory
        .create_texture_immutable_u8::<Rgba8>(kind, gfx::texture::Mipmap::Provided, &[&img])
        .unwrap();
    view
}

#[derive(Debug, Clone, Copy)]
struct Quad {
    rect: Rect,
    depth: f32,
    color: Color,
}

impl Quad {
    fn new(rect: Rect, depth: f32, color: Color) -> Quad {
        Quad { rect, depth, color }
    }

    fn unit_quad(depth: f32, color: Color) -> Quad {
        Quad {
            rect: Rect::from_dimension(1.0, 1.0),
            depth,
            color,
        }
    }

    fn append_vertices_indices(
        &self,
        quad_index: u16,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
    ) {
        let (self_vertices, self_indices) = self.into_vertices_indices(quad_index);
        vertices.extend(&self_vertices);
        indices.extend(&self_indices);
    }

    fn append_vertices_indices_centered(
        &self,
        quad_index: u16,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
    ) {
        let (self_vertices, self_indices) = self.into_vertices_indices_centered(quad_index);
        vertices.extend(&self_vertices);
        indices.extend(&self_indices);
    }

    fn into_vertices_indices(&self, quad_index: u16) -> ([Vertex; 4], [u16; 6]) {
        let pos = self.rect.to_pos();
        let dim = self.rect.to_dim();
        let color = self.color.into();
        let depth = self.depth;

        // NOTE: UVs y-axis is intentionally flipped to prevent upside-down images
        let vertices: [Vertex; 4] = [
            Vertex {
                pos: [pos.x, pos.y, depth, 1.0],
                uv: [0.0, 1.0],
                color,
            },
            Vertex {
                pos: [pos.x + dim.x, pos.y, depth, 1.0],
                uv: [1.0, 1.0],
                color,
            },
            Vertex {
                pos: [pos.x + dim.x, pos.y + dim.y, depth, 1.0],
                uv: [1.0, 0.0],
                color,
            },
            Vertex {
                pos: [pos.x, pos.y + dim.y, depth, 1.0],
                uv: [0.0, 0.0],
                color,
            },
        ];

        let indices: [u16; 6] = [
            4 * quad_index,
            4 * quad_index + 1,
            4 * quad_index + 2,
            4 * quad_index + 2,
            4 * quad_index + 3,
            4 * quad_index,
        ];

        (vertices, indices)
    }

    fn into_vertices_indices_centered(&self, quad_index: u16) -> ([Vertex; 4], [u16; 6]) {
        let pos = self.rect.to_pos();
        let half_dim = 0.5 * self.rect.to_dim();
        let color = self.color.into();
        let depth = self.depth;

        // NOTE: UVs y-axis is intentionally flipped to prevent upside-down images
        let vertices: [Vertex; 4] = [
            Vertex {
                pos: [pos.x - half_dim.x, pos.y - half_dim.y, depth, 1.0],
                uv: [0.0, 1.0],
                color,
            },
            Vertex {
                pos: [pos.x + half_dim.x, pos.y - half_dim.y, depth, 1.0],
                uv: [1.0, 1.0],
                color,
            },
            Vertex {
                pos: [pos.x + half_dim.x, pos.y + half_dim.y, depth, 1.0],
                uv: [1.0, 0.0],
                color,
            },
            Vertex {
                pos: [pos.x - half_dim.x, pos.y + half_dim.y, depth, 1.0],
                uv: [0.0, 0.0],
                color,
            },
        ];

        let indices: [u16; 6] = [
            4 * quad_index,
            4 * quad_index + 1,
            4 * quad_index + 2,
            4 * quad_index + 2,
            4 * quad_index + 3,
            4 * quad_index,
        ];

        (vertices, indices)
    }
}
