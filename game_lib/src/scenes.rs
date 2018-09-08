use *;

const PONGI_RADIUS: f32 = 7.5;
const PONGI_BASE_SPEED: f32 = 15.0 * UNIT_SIZE;

const WALL_THICKNESS: f32 = 0.5 * UNIT_SIZE;
const PADDLE_SIZE: f32 = 3.0 * UNIT_SIZE;

const FIELD_BOUNDS: Rect = Rect {
    left: -10.0 * UNIT_SIZE,
    right: 10.0 * UNIT_SIZE,
    top: -6.0 * UNIT_SIZE,
    bottom: 6.0 * UNIT_SIZE,
};

#[derive(Default)]
pub struct Globals {
    pub mouse_pos_world: WorldPoint,
    pub mouse_pos_canvas: CanvasPoint,

    pub mouse_delta_world: WorldVec,
    pub mouse_delta_canvas: CanvasVec,

    pub cam: Camera,
    pub error_happened: Option<String>,
}

pub trait Scene {
    fn reinitialize(&mut self, system_commands: &mut Vec<SystemCommand>);
    fn update_and_draw(&mut self, input: &GameInput, globals: &mut Globals, dc: &mut DrawContext);
}

//==================================================================================================
// DebugScene
//==================================================================================================
//
#[derive(Default)]
pub struct DebugScene;

impl Scene for DebugScene {
    fn reinitialize(&mut self, _system_commands: &mut Vec<SystemCommand>) {}
    fn update_and_draw(&mut self, input: &GameInput, globals: &mut Globals, dc: &mut DrawContext) {
        // Draw cursor
        let mut cursor_color = Color::new(0.0, 0.0, 0.0, 1.0);
        if input.mouse_button_left.is_pressed {
            cursor_color.x = 1.0;
        }
        if input.mouse_button_middle.is_pressed {
            cursor_color.y = 1.0;
        }
        if input.mouse_button_right.is_pressed {
            cursor_color.z = 1.0;
        }
        dc.debug_draw_cursor(
            globals.mouse_pos_canvas,
            -0.3,
            COLOR_WHITE,
            DrawSpace::Canvas,
        );
        dc.draw_rect_filled(
            Rect::from_point_dimension(globals.mouse_pos_world.pixel_snapped(), Vec2::ones()),
            -0.2,
            cursor_color,
            DrawSpace::World,
        );

        // Frametimes etc.
        let delta = pretty_format_duration_ms(f64::from(input.time_delta));
        let draw = pretty_format_duration_ms(f64::from(input.time_draw));
        let update = pretty_format_duration_ms(f64::from(input.time_update));
        dc.debug_draw_text(
            &format!("delta: {}\ndraw: {}\nupdate: {}\n", delta, draw, update),
            draw::COLOR_WHITE,
        );
        dc.debug_draw_text(
            &format!(
                "mouse_screen: {}x{}",
                input.mouse_pos_screen.x, input.mouse_pos_screen.y
            ),
            draw::COLOR_WHITE,
        );
        dc.debug_draw_text(
            &format!(
                "mouse_delta_screen: {}x{}",
                input.mouse_delta_screen.x, input.mouse_delta_screen.y
            ),
            draw::COLOR_WHITE,
        );
        dc.debug_draw_text(
            &format!(
                "mouse_world: {}x{}",
                globals.mouse_pos_world.x, globals.mouse_pos_world.y
            ),
            draw::COLOR_WHITE,
        );
        dc.debug_draw_text(
            &format!(
                "mouse_canvas: {}x{}\n",
                globals.mouse_pos_canvas.x, globals.mouse_pos_canvas.y
            ),
            draw::COLOR_WHITE,
        );

        if input.fast_time != 0 {
            if input.fast_time > 0 {
                dc.debug_draw_text(
                    &format!("Time speedup {}x", input.fast_time + 1),
                    draw::COLOR_GREEN,
                );
            } else if input.fast_time < 0 {
                dc.debug_draw_text(
                    &format!("Time slowdown {}x", i32::abs(input.fast_time) + 1),
                    draw::COLOR_YELLOW,
                );
            } else {
            };
        }
        if input.game_paused {
            dc.debug_draw_text("The game is paused", draw::COLOR_CYAN);
        }
        // Debug crash message
        if globals.error_happened.is_some() {
            dc.debug_draw_text(
                &format!(
                    "The game has crashed: {}",
                    globals.error_happened.clone().unwrap()
                ),
                draw::COLOR_RED,
            );
        }
    }
}

//==================================================================================================
// GameplayScene
//==================================================================================================
//
#[derive(Default)]
pub struct GameplayScene {
    is_multiplayer: bool,

    paddle_left_pos: f32,
    paddle_left_vel: f32,

    paddle_right_pos: f32,
    paddle_right_vel: f32,

    pongi_pos: WorldPoint,
    pongi_vel: Vec2,

    time_till_next_beat: f32,
}

impl Scene for GameplayScene {
    fn reinitialize(&mut self, system_commands: &mut Vec<SystemCommand>) {
        //gc.pongi_pos = Point::new(0.0, -3.0 * UNIT_SIZE);
        //gc.pongi_vel = Vec2::new(0.0, -5.0 * UNIT_SIZE);

        let angle: f32 = 40.0;
        self.pongi_pos = Point::new(8.0, -4.0) * UNIT_SIZE;
        self.pongi_vel = Vec2::from_angle(angle.to_radians()) * PONGI_BASE_SPEED;

        // gc.pongi_pos = Point::new(-151.48575, -88.0);
        // gc.pongi_vel = Vec2::new(-4644.807, 6393.034);

        system_commands.push(SystemCommand::EnableRelativeMouseMovementCapture(true));
    }

    fn update_and_draw(&mut self, input: &GameInput, globals: &mut Globals, dc: &mut DrawContext) {
        let delta_time = if input.game_paused || globals.error_happened.is_some() {
            0.0
        } else {
            let time_factor = if input.fast_time == 0 {
                1.0
            } else if input.fast_time > 0 {
                (input.fast_time + 1) as f32
            } else {
                1.0 / (((i32::abs(input.fast_time)) + 1) as f32)
            };
            input.time_delta * time_factor
        };

        // ---------------------------------------------------------------------------------------------
        // Playfield
        //

        let screen_rect = Rect::from_dimension(input.screen_dim);
        let canvas_rect = Rect::from_width_height(CANVAS_WIDTH, CANVAS_HEIGHT);

        // Draw grid
        let grid_light = Color::new(0.9, 0.7, 0.2, 1.0);
        for x in -30..30 {
            for diagonal in -20..20 {
                let pos = Point::new((x + diagonal) as f32, diagonal as f32) * UNIT_SIZE;
                if x % 2 == 0 {
                    dc.draw_rect_filled(
                        Rect::from_point_dimension(pos, Vec2::ones() * UNIT_SIZE),
                        -1.0,
                        grid_light,
                        DrawSpace::World,
                    );
                } else {
                    let r = (x + 30) as f32 / 60.0;
                    let g = (diagonal + 20) as f32 / 40.0;
                    let b = (r + g) / 2.0;
                    dc.draw_rect_filled(
                        Rect::from_point_dimension(pos, Vec2::ones() * UNIT_SIZE),
                        -1.0,
                        Color::new(r, g, b, 1.0),
                        DrawSpace::World,
                    );
                }
            }
        }

        // Draw playing field
        let field_depth = -0.4;

        let field_border_left = Rect {
            left: FIELD_BOUNDS.left - WALL_THICKNESS,
            right: FIELD_BOUNDS.left,
            top: FIELD_BOUNDS.top,
            bottom: FIELD_BOUNDS.bottom,
        };
        let field_border_right = Rect {
            left: FIELD_BOUNDS.right,
            right: FIELD_BOUNDS.right + WALL_THICKNESS,
            top: FIELD_BOUNDS.top,
            bottom: FIELD_BOUNDS.bottom,
        };
        let field_border_top = Rect {
            left: FIELD_BOUNDS.left - WALL_THICKNESS,
            right: FIELD_BOUNDS.right + WALL_THICKNESS,
            top: FIELD_BOUNDS.top - WALL_THICKNESS,
            bottom: FIELD_BOUNDS.top,
        };
        let field_border_bottom = Rect {
            left: FIELD_BOUNDS.left - WALL_THICKNESS,
            right: FIELD_BOUNDS.right + WALL_THICKNESS,
            top: FIELD_BOUNDS.bottom,
            bottom: FIELD_BOUNDS.bottom + WALL_THICKNESS,
        };
        // let field_border_center =
        //     Rect::unit_rect_centered().scaled_from_center(Vec2::ones() * UNIT_SIZE);

        for (&field_border, &color) in [
            field_border_left,
            field_border_right,
            field_border_top,
            field_border_bottom,
            //field_border_center,
        ].iter()
            .zip(
                [
                    draw::COLOR_RED,
                    draw::COLOR_GREEN,
                    draw::COLOR_BLUE,
                    draw::COLOR_BLACK,
                    //draw::COLOR_YELLOW,
                ].iter(),
            ) {
            dc.draw_rect_filled(field_border, field_depth, color, DrawSpace::World);
        }

        // Update beat
        const BPM: f32 = 100.0;
        const BEAT_LENGTH: f32 = 60.0 / BPM;

        let mut time_till_next_beat = self.time_till_next_beat;
        time_till_next_beat -= delta_time;
        while time_till_next_beat < 0.0 {
            time_till_next_beat += BEAT_LENGTH;
        }
        let beat_value = beat_visualizer_value(time_till_next_beat, BEAT_LENGTH);

        // Update pongi
        let pongi_pos = self.pongi_pos;
        let pongi_vel = self.pongi_vel;

        let mut collision_mesh = CollisionMesh::new("play_field");
        collision_mesh.add_rect("left_wall", field_border_left);
        collision_mesh.add_rect("right_wall", field_border_right);
        collision_mesh.add_rect("top_wall", field_border_top);
        collision_mesh.add_rect("bottom_wall", field_border_bottom);
        //collision_mesh.add_rect("center_wall", field_border_center);

        let mut error_happened = None;
        let (new_pongi_pos, new_pongi_vel) = move_sphere_with_full_elastic_collision(
            &collision_mesh,
            pongi_pos,
            pongi_vel,
            PONGI_RADIUS,
            delta_time,
        ).unwrap_or_else(|error| {
            error_happened = Some(error);
            (pongi_pos, pongi_vel)
        });

        // Write back to game_context
        globals.error_happened = error_happened;
        if globals.error_happened.is_none() {
            self.pongi_vel = new_pongi_vel;
            self.pongi_pos = new_pongi_pos;
            self.time_till_next_beat = time_till_next_beat;
            self.paddle_left_pos = clamp(
                self.paddle_left_pos
                    + input.mouse_delta_screen.y / screen_rect.height() * canvas_rect.height(),
                FIELD_BOUNDS.top,
                FIELD_BOUNDS.bottom - PADDLE_SIZE,
            );
        }

        // Debug draw sphere sweeping
        collision_mesh
            .shapes
            .iter()
            .map(|rect| RectSphereSum::new(rect, PONGI_RADIUS))
            .for_each(|sum| dc.draw_lines(&sum.to_lines(), 0.0, COLOR_YELLOW, DrawSpace::World));

        //if let Some(collision) = collision_mesh.sweepcast_sphere(look_ahead_raycast, PONGI_RADIUS) {
        //    println!(
        //        "Intersection with '{}' on shape '{:?}' on segment '{:?}':\n {:?}",
        //        collision_mesh.name, collision.shape, collision.segment, collision.intersection
        //    );
        //}

        // Draw beat visualizer
        let beat_box_pos = Vec2::new(canvas_rect.right - 2.0 * UNIT_SIZE, 1.5 * UNIT_SIZE - 1.0);
        let beat_box_size = UNIT_SIZE * (0.5 + beat_value);
        dc.draw_rect_filled(
            Rect::from_point_dimension(beat_box_pos, Vec2::ones() * beat_box_size).centered(),
            0.0,
            draw::COLOR_MAGENTA,
            DrawSpace::Canvas,
        );

        // Draw pongi
        dc.debug_draw_text(&dformat!(self.pongi_vel), draw::COLOR_WHITE);
        dc.debug_draw_text(&dformat!(self.pongi_pos), draw::COLOR_WHITE);
        dc.draw_arrow(
            self.pongi_pos.pixel_snapped(),
            self.pongi_vel.normalized(),
            0.3 * self.pongi_vel.magnitude(),
            -0.1,
            draw::COLOR_GREEN,
            DrawSpace::World,
        );

        dc.debug_draw_circle_textured(
            self.pongi_pos.pixel_snapped(),
            -0.3,
            Color::new(1.0 - beat_value, 1.0 - beat_value, 1.0, 1.0),
            DrawSpace::World,
        );

        // Draw paddles
        dc.draw_rect_filled(
            Rect::from_point(
                WorldPoint::new(FIELD_BOUNDS.left - WALL_THICKNESS, self.paddle_left_pos),
                WALL_THICKNESS,
                PADDLE_SIZE,
            ),
            -0.2,
            COLOR_WHITE,
            DrawSpace::World,
        );
        dc.draw_rect_filled(
            Rect::from_point(
                WorldPoint::new(FIELD_BOUNDS.right, self.paddle_right_pos),
                WALL_THICKNESS,
                PADDLE_SIZE,
            ),
            -0.2,
            COLOR_WHITE,
            DrawSpace::World,
        );
    }
}

fn move_sphere_with_full_elastic_collision(
    collision_mesh: &CollisionMesh,
    mut pos: WorldPoint,
    mut vel: Vec2,
    sphere_radius: f32,
    delta_time: f32,
) -> Result<(WorldPoint, Vec2), String> {
    let speed = vel.magnitude();
    let mut dir = vel.normalized();
    let mut travel_distance = speed * delta_time;

    let mut travel_raycast =
        Line::new(pos, pos + (travel_distance + COLLISION_SAFETY_MARGIN) * dir);

    let mut debug_num_loops = 0;
    while let Some(collision) = collision_mesh.sweepcast_sphere(travel_raycast, sphere_radius) {
        // Determine a point that is right before the actual collision point
        let distance_till_hit = (collision.intersection.point - pos).magnitude();
        let safe_collision_point_distance = distance_till_hit - COLLISION_SAFETY_MARGIN;
        if travel_distance < safe_collision_point_distance {
            // We won't hit anything yet in this frame
            break;
        }

        // Move ourselves to the position right before the actual collision point
        pos += safe_collision_point_distance * dir;
        dir = vel
            .normalized()
            .reflected_on_normal(collision.intersection.normal);
        vel = dir * speed;
        travel_distance -= safe_collision_point_distance;

        travel_raycast = Line::new(pos, pos + (travel_distance + COLLISION_SAFETY_MARGIN) * dir);

        debug_num_loops += 1;
        if debug_num_loops == 3 {
            return Err(format!(
                "Collision loop took {} iterations",
                debug_num_loops
            ));
        }
    }
    pos += travel_distance * dir;

    Ok((pos, vel))
}

fn beat_visualizer_value(time_till_next_beat: f32, beat_length: f32) -> f32 {
    let ratio = time_till_next_beat / beat_length;
    let increasing = (1.0 - ratio).powi(10);
    let decreasing = ratio.powi(3);

    increasing + decreasing
}

//==================================================================================================
// MenuScene
//==================================================================================================
//

enum MenuItem {
    StartSinglePlayer,
    StartTwoPlayers,
    Quit,
}

impl MenuItem {
    pub fn as_str(&self) -> &'static str {
        match self {
            MenuItem::StartSinglePlayer => "Play with computer",
            MenuItem::StartTwoPlayers => "Play with human",
            MenuItem::Quit => "Quit game",
        }
    }
}

impl Default for MenuItem {
    fn default() -> Self {
        MenuItem::StartSinglePlayer
    }
}

#[derive(Default)]
pub struct MenuScene {
    menu_items: Vec<MenuItem>,
    highlighted_menu_item_index: usize,
}

impl Scene for MenuScene {
    fn reinitialize(&mut self, system_commands: &mut Vec<SystemCommand>) {
        system_commands.push(SystemCommand::EnableRelativeMouseMovementCapture(false));

        self.menu_items = vec![
            MenuItem::StartSinglePlayer,
            MenuItem::StartTwoPlayers,
            MenuItem::Quit,
        ];

        self.highlighted_menu_item_index = 0;
    }

    fn update_and_draw(&mut self, input: &GameInput, globals: &mut Globals, dc: &mut DrawContext) {
        let button_margin = 2.0;
        let button_padding = 2.0;
        let canvas_rect = Rect::from_width_height(CANVAS_WIDTH, CANVAS_HEIGHT);

        dc.draw_rect_filled(
            canvas_rect,
            0.0,
            Color::new(0.4, 0.4, 0.4, 0.3),
            DrawSpace::Canvas,
        );

        let button_rects: Vec<_> = self
            .menu_items
            .iter()
            .map(|item| {
                let text_rect = Rect::from_dimension(dc.get_text_dimensions(item.as_str()));
                text_rect.extended_uniformly_by(button_padding)
            })
            .collect();

        let menu_height = button_rects
            .iter()
            .map(|rect| rect.height() + button_margin)
            .sum::<f32>() + button_margin;
        let menu_width = button_rects
            .iter()
            .map(|rect| rect.width() + 2.0 * button_margin)
            .max_by(|a, b| compare_floats(*a, *b))
            .unwrap_or_else(|| panic!("Menu has no buttons"));

        let menu_rect = Rect::from_width_height(menu_width, menu_height)
            .centered_in_rect(canvas_rect)
            .with_pixel_snapped_position();
        dc.draw_rect_filled(menu_rect, 0.0, COLOR_WHITE, DrawSpace::Canvas);

        let mut vertical_offset = button_margin;
        for (index, item) in self.menu_items.iter().enumerate() {
            // Draw button rect
            let button_rect = button_rects[index]
                .translated_to_pos(menu_rect.pos())
                .translated_by(Vec2::unit_y() * vertical_offset)
                .centered_horizontally_in_rect(menu_rect)
                .with_pixel_snapped_position();
            vertical_offset += button_rect.height() + button_margin;

            if globals.mouse_pos_canvas.intersects_rect(button_rect) {
                self.highlighted_menu_item_index = index;
            }

            dc.draw_rect_filled(
                button_rect,
                0.0,
                if index == self.highlighted_menu_item_index {
                    COLOR_MAGENTA
                } else {
                    COLOR_BLUE
                },
                DrawSpace::Canvas,
            );

            // Draw text
            let text_rect = Rect::from_dimension(dc.get_text_dimensions(item.as_str()))
                .centered_in_rect(button_rect);
            dc.draw_text(
                text_rect.pos(),
                item.as_str(),
                0.0,
                COLOR_WHITE,
                DrawSpace::Canvas,
            );
        }
    }
}