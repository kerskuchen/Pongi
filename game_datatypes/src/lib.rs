pub extern crate cgmath;
pub use cgmath::prelude::*;

pub use cgmath::ortho;

pub type Point = cgmath::Point2<f32>;
pub type Vec2 = cgmath::Vector2<f32>;
pub type Color = cgmath::Vector4<f32>;
pub type Mat4 = cgmath::Matrix4<f32>;

pub struct DrawCommand {
    pub projection: Mat4,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<VertexIndex>,
    pub texture: String,
}

pub type VertexIndex = u16;
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 4],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    f32::max(min, f32::min(val, max))
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_dimension(width: f32, height: f32) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    pub fn from_corners(bottom_left: Point, top_right: Point) -> Rect {
        Rect {
            x: bottom_left.x,
            y: bottom_left.y,
            width: top_right.x - bottom_left.x,
            height: top_right.y - bottom_left.y,
        }
    }

    pub fn unit_rect_centered() -> Rect {
        Rect {
            x: -0.5,
            y: -0.5,
            width: 1.0,
            height: 1.0,
        }
    }

    /// Returns the biggest proportionally stretched version of the rectangle that can fit
    /// into `target`.
    pub fn stretched_to_fit(self, target: Rect) -> Rect {
        let source_aspect_ratio = self.width / self.height;
        let target_aspect_ratio = target.width / target.height;

        let scale_factor = if source_aspect_ratio < target_aspect_ratio {
            // Target rect is 'wider' than ours -> height is our limit when stretching
            target.height / self.height
        } else {
            // Target rect is 'narrower' than ours -> width is our limit when stretching
            target.width / self.width
        };

        let stretched_width = self.width * scale_factor;
        let stretched_height = self.height * scale_factor;

        Rect {
            x: self.x,
            y: self.x,
            width: stretched_width,
            height: stretched_height,
        }
    }

    /// Returns a version of the rectangle that is centered in `target`.
    pub fn centered_in(self, target: Rect) -> Rect {
        let x_offset_centered = target.x + (target.width - self.width) / 2.0;
        let y_offset_centered = target.y + (target.height - self.height) / 2.0;

        Rect {
            x: x_offset_centered,
            y: y_offset_centered,
            width: self.width,
            height: self.height,
        }
    }

    pub fn to_pos(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn to_dim(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }
}

pub fn clamp_point_in_rect(point: Point, rect: Rect) -> Point {
    Point {
        x: clamp(point.x, rect.x, rect.x + rect.width),
        y: clamp(point.y, rect.y, rect.y + rect.height),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Quad {
    pub rect: Rect,
    pub depth: f32,
    pub color: Color,
}

impl Quad {
    pub fn new(rect: Rect, depth: f32, color: Color) -> Quad {
        Quad { rect, depth, color }
    }

    pub fn unit_quad(depth: f32, color: Color) -> Quad {
        Quad {
            rect: Rect::from_dimension(1.0, 1.0),
            depth,
            color,
        }
    }

    // TODO(JaSc): Create vertex/index-buffer struct and move the `append_..` methods into that
    pub fn append_vertices_indices(
        &self,
        quad_index: VertexIndex,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<VertexIndex>,
    ) {
        let (self_vertices, self_indices) = self.into_vertices_indices(quad_index);
        vertices.extend_from_slice(&self_vertices);
        indices.extend(&self_indices);
    }

    pub fn append_vertices_indices_centered(
        &self,
        quad_index: VertexIndex,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<VertexIndex>,
    ) {
        let (self_vertices, self_indices) = self.into_vertices_indices_centered(quad_index);
        vertices.extend_from_slice(&self_vertices);
        indices.extend(&self_indices);
    }

    pub fn into_vertices_indices(self, quad_index: VertexIndex) -> ([Vertex; 4], [VertexIndex; 6]) {
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

        let indices: [VertexIndex; 6] = [
            4 * quad_index,
            4 * quad_index + 1,
            4 * quad_index + 2,
            4 * quad_index + 2,
            4 * quad_index + 3,
            4 * quad_index,
        ];

        (vertices, indices)
    }

    pub fn into_vertices_indices_centered(
        self,
        quad_index: VertexIndex,
    ) -> ([Vertex; 4], [VertexIndex; 6]) {
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

        let indices: [VertexIndex; 6] = [
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

/// A macro for debugging which returns a string representation of an expression and its value
///
/// It uses the `stringify` macro internally and requires the input to be an expression.
///
/// # Examples
///
/// ```
/// let name = 5;
/// assert_eq!(dformat!(1 + 2), "1 + 2 = 3");
/// assert_eq!(dformat!(1 + name), "1 + name = 6");
/// assert_eq!(dformat!(name), "name = 5");
/// ```
#[allow(unused_macros)]
macro_rules! dformat {
    ($x:expr) => {
        format!("{} = {:?}", stringify!($x), $x)
    };
}

/// A macro used for debugging which prints a string containing the name and value of a given
/// variable.
///
/// It uses the `dformat` macro internally and requires the input to be an expression.
/// For more information see the `dformat` macro
///
/// # Examples
///
/// ```
/// dprintln!(1 + 2);
/// // prints: "1 + 2 = 3"
///
/// let name = 5;
/// dprintln!(name);
/// // prints: "name = 5"
///
/// dprintln!(1 + name);
/// // prints: "1 + name = 6"
/// ```
#[allow(unused_macros)]
macro_rules! dprintln {
    ($x:expr) => {
        println!("{}", dformat!($x));
    };
}
