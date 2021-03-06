pub extern crate cgmath;
extern crate lodepng;
extern crate rgb;

extern crate hound;
extern crate lewton;
extern crate sample;
extern crate time_calc;

#[macro_use]
extern crate log;
extern crate fern;

#[macro_use]
extern crate serde_derive;
extern crate bincode;
extern crate ron;
extern crate serde;

#[macro_use]
pub mod utility;
mod audio;
pub mod collision;
pub mod draw;
pub mod gui;
pub mod math;
mod scenes;

pub type ResourcePath = String;

use crate::audio::*;
pub use crate::collision::*;
pub use crate::draw::*;
pub use crate::math::*;
use crate::scenes::*;
use std::collections::HashMap;

//==================================================================================================
// SystemCommand

pub enum SystemCommand {
    EnableRelativeMouseMovementCapture(bool),
    ShutdownGame,
}

//==================================================================================================
// External calls
//==================================================================================================
//

/// Forwards directly to [`game_lib::update_and_draw`]
#[no_mangle]
pub fn update_and_draw<'game_context>(
    input: &GameInput,
    game_context: &'game_context mut GameContext<'game_context>,
) {
    update_and_draw_internal(input, game_context);
}

/// Forwards directly to [`game_lib::process_audio`]
#[no_mangle]
pub fn process_audio<'game_context>(
    input: &GameInput,
    game_context: &'game_context mut GameContext<'game_context>,
    audio_output_buffer: &mut Vec<f32>,
) {
    process_audio_internal(input, game_context, audio_output_buffer);
}

//==================================================================================================
// GameContext
//==================================================================================================
//
const UNIT_SIZE: f32 = 16.0;
const CANVAS_WIDTH: f32 = 480.0;
const CANVAS_HEIGHT: f32 = 270.0;

const LOG_LEVEL_GENERAL: log::LevelFilter = log::LevelFilter::Trace;
const LOG_LEVEL_GAME_LIB: log::LevelFilter = log::LevelFilter::Trace;
const LOG_LEVEL_MATH: log::LevelFilter = log::LevelFilter::Trace;
const LOG_LEVEL_DRAW: log::LevelFilter = log::LevelFilter::Trace;

#[derive(Default)]
pub struct GameContext<'game_context> {
    is_initialized: bool,

    globals: Globals,
    gameplay_scene: GameplayScene,
    menu_scene: MenuScene,
    debug_scene: DebugScene,

    audio_context: AudioContext,

    drawcontext: DrawContext<'game_context>,
    system_commands: Vec<SystemCommand>,
}

impl<'game_context> GameContext<'game_context> {
    pub fn get_draw_commands(&mut self) -> Vec<DrawCommand> {
        std::mem::replace(&mut self.drawcontext.draw_commands, Vec::new())
    }

    pub fn get_system_commands(&mut self) -> Vec<SystemCommand> {
        std::mem::replace(&mut self.system_commands, Vec::new())
    }

    pub fn new(
        num_audio_channels: usize,
        audio_sample_rate_hz: usize,
    ) -> GameContext<'game_context> {
        GameContext {
            audio_context: AudioContext::new(num_audio_channels, audio_sample_rate_hz),
            ..Default::default()
        }
    }
}

//==================================================================================================
// GameInput
//==================================================================================================
//

type InputAction = String;

#[derive(Default)]
pub struct GameInput {
    pub time_since_startup: f64,
    pub time_delta: f32,
    pub time_update: f32,
    pub time_draw: f32,
    pub time_audio: f32,

    pub screen_dim: Vec2,

    pub current_audio_sample_index: usize,

    /// Regular buttons
    pub buttons: HashMap<InputAction, GameButton>,
    /// Buttons that toggle its state on key-press
    pub buttons_toggle: HashMap<InputAction, GameButton>,
    /// Buttons that go into pressed state on key-press but reset back into unpressed
    /// after the current frame is over
    pub buttons_oneshot: HashMap<InputAction, GameButton>,

    /// Mouse position is given in the following interval:
    /// [0 .. screen_width - 1] x [0 .. screen_height - 1]
    /// where (0,0) is the top left of the screen
    pub mouse_pos_screen: Point,
    pub mouse_delta_screen: Vec2,

    pub mouse_button_left: GameButton,
    pub mouse_button_middle: GameButton,
    pub mouse_button_right: GameButton,

    /// * `Positive`: Moving away from user
    /// * `Negative`: Moving towards user
    pub mouse_wheel_delta: i32,
}

impl GameInput {
    pub fn new() -> GameInput {
        Default::default()
    }

    pub fn register_input_action(&mut self, action: &str) {
        if action.ends_with("toggle") {
            self.buttons_toggle
                .insert(action.to_owned(), GameButton::new());
        } else if action.ends_with("oneshot") {
            self.buttons_oneshot
                .insert(action.to_owned(), GameButton::new());
        } else {
            self.buttons.insert(action.to_owned(), GameButton::new());
        }
    }

    pub fn had_press_event(&self, action: &str) -> bool {
        let button = self.get_button(action);
        button.is_pressed && button.num_state_transitions > 0
    }

    pub fn had_release_event(&self, action: &str) -> bool {
        let button = self.get_button(action);
        !button.is_pressed && button.num_state_transitions > 0
    }

    pub fn had_transition_event(&self, action: &str) -> bool {
        let button = self.get_button(action);
        button.num_state_transitions > 0
    }

    pub fn is_pressed(&self, action: &str) -> bool {
        self.get_button(action).is_pressed
    }

    fn get_button(&self, action: &str) -> GameButton {
        if let Some(button) = self.buttons.get(action) {
            button.clone()
        } else if let Some(button) = self.buttons_toggle.get(action) {
            button.clone()
        } else if let Some(button) = self.buttons_oneshot.get(action) {
            button.clone()
        } else {
            panic!("Button for input action '{}' does not exist", action);
        }
    }

    pub fn process_button_event(&mut self, action: &str, is_pressed: bool) {
        if let Some(button) = self.buttons.get_mut(action) {
            button.set_state(is_pressed);
            return;
        }
        if let Some(button) = self.buttons_toggle.get_mut(action) {
            // Toggles activate only on a press event
            if is_pressed {
                let previous_state = button.is_pressed;
                button.set_state(!previous_state);
            }
            return;
        }
        if let Some(button) = self.buttons_oneshot.get_mut(action) {
            // One-shots activate only on a press event
            if is_pressed {
                button.set_state(is_pressed);
            }
            return;
        }
        panic!("Button for input action '{}' does not exist", action);
    }

    pub fn prepare_for_next_frame(&mut self) {
        self.mouse_button_left.clear_transitions();
        self.mouse_button_middle.clear_transitions();
        self.mouse_button_right.clear_transitions();
        self.mouse_wheel_delta = 0;

        for (_, button) in self
            .buttons
            .iter_mut()
            .chain(self.buttons_toggle.iter_mut())
            .chain(self.buttons_oneshot.iter_mut())
        {
            button.clear_transitions();
        }

        for (_, button) in self.buttons_oneshot.iter_mut() {
            button.set_state(false);
            button.clear_transitions();
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct GameButton {
    pub num_state_transitions: u32,
    pub is_pressed: bool,
}

impl GameButton {
    pub fn new() -> GameButton {
        Default::default()
    }

    pub fn set_state(&mut self, is_pressed: bool) {
        if self.is_pressed != is_pressed {
            self.num_state_transitions += 1;
            self.is_pressed = is_pressed;
        }
    }

    pub fn clear_transitions(&mut self) {
        self.num_state_transitions = 0;
    }

    pub fn toggle(&mut self) {
        if self.is_pressed {
            self.set_state(false);
        } else {
            self.set_state(true);
        }
    }
}

//==================================================================================================
// Game
//==================================================================================================
//

// TODO(JaSc): Maybe we additionally want something like SystemCommands that tell the platform
//             layer to create framebuffers / go fullscreen / turn on vsync / upload textures
pub fn update_and_draw_internal<'game_context>(
    input: &GameInput,
    gc: &'game_context mut GameContext<'game_context>,
) {
    // ---------------------------------------------------------------------------------------------
    // Init / re-init
    //
    if !gc.is_initialized || input.had_press_event("debug_hotreload_code_oneshot") {
        // Initializing logger
        // NOTE: When hot reloading the game lib dll the logging must be reinitialized
        // TODO(JaSc): Do we actually need the logging?
        //
        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!("{}: {}", record.level(), message))
            })
            .level(LOG_LEVEL_GENERAL)
            .level_for("game_lib", LOG_LEVEL_GAME_LIB)
            .level_for("game_lib::math", LOG_LEVEL_MATH)
            .level_for("game_lib::draw", LOG_LEVEL_DRAW)
            .chain(std::io::stdout())
            .apply()
            .expect("Could not initialize logger");
    }

    if !gc.is_initialized || input.had_press_event("debug_reset_gamestate_oneshot") {
        gc.globals.cam = Camera::new(
            WorldPoint::zero(),
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
            DEFAULT_WORLD_ZNEAR,
            DEFAULT_WORLD_ZFAR,
        );
        gc.globals.error_happened = None;
        gc.gameplay_scene.reinitialize(&mut gc.system_commands);
        gc.debug_scene.reinitialize(&mut gc.system_commands);
        gc.menu_scene.reinitialize(&mut gc.system_commands);
    }

    if !gc.is_initialized
        || input.had_press_event("debug_hotreload_assets_oneshot")
        || input.had_transition_event("debug_highres_drawing_toggle")
    {
        let canvas_dim = if input.is_pressed("debug_highres_drawing_toggle") {
            (input.screen_dim.x as u16, input.screen_dim.y as u16)
        } else {
            (CANVAS_WIDTH as u16, CANVAS_HEIGHT as u16)
        };
        gc.drawcontext.reinitialize(canvas_dim.0, canvas_dim.1);
        gc.audio_context.reinitialize();
    }

    if !gc.is_initialized {
        gc.is_initialized = true;
    }

    // Additional debug input
    if input.had_press_event("debug_time_speedup") {
        gc.globals.debug_time_factor_increment += 1;
    }
    if input.had_press_event("debug_time_slowdown") {
        gc.globals.debug_time_factor_increment -= 1;
    }
    gc.globals.debug_game_paused = input.is_pressed("debug_pause_game_toggle");

    // ---------------------------------------------------------------------------------------------
    // Mouse input and camera
    //
    let screen_rect = Rect::from_dimension(input.screen_dim);
    let canvas_rect = Rect::from_width_height(CANVAS_WIDTH, CANVAS_HEIGHT);
    let canvas_blit_rect = canvas_blit_rect(screen_rect, canvas_rect);

    // Canvas mouse position
    // TODO(JaSc): new_mouse_pos_canvas and accumulations of new_mouse_delta_canvas will go
    //             out of sync due to rounding errors. Maybe only allow just one or the other
    //             when we get to implement event based input?
    let new_mouse_pos_canvas =
        screen_pos_to_canvas_pos(input.mouse_pos_screen, screen_rect, canvas_rect);
    let new_mouse_delta_canvas =
        screen_vec_to_canvas_vec(input.mouse_delta_screen, screen_rect, canvas_rect);

    // World mouse position
    let new_mouse_pos_world = gc
        .globals
        .cam
        .canvas_point_to_world_point(new_mouse_pos_canvas);
    let new_mouse_delta_world = gc
        .globals
        .cam
        .canvas_vec_to_world_vec(new_mouse_delta_canvas);

    // Camera movement
    if input.mouse_button_right.is_pressed {
        gc.globals.cam.pan(new_mouse_delta_canvas);

    }

    if input.mouse_button_middle.is_pressed {
        gc.globals.cam.zoom_to_world_point(new_mouse_pos_world, 1.0);
    }

    if input.mouse_wheel_delta > 0 {
        let new_zoom_level = f32::min(gc.globals.cam.zoom_level * 2.0, 8.0);
        gc.globals
            .cam
            .zoom_to_world_point(new_mouse_pos_world, new_zoom_level);
    } else if input.mouse_wheel_delta < 0 {
        let new_zoom_level = f32::max(gc.globals.cam.zoom_level / 2.0, 1.0 / 32.0);
        gc.globals
            .cam
            .zoom_to_world_point(new_mouse_pos_world, new_zoom_level);
    }

    gc.globals.mouse_pos_world = new_mouse_pos_world;
    gc.globals.mouse_pos_canvas = new_mouse_pos_canvas;

    gc.globals.mouse_delta_world = new_mouse_delta_world;
    gc.globals.mouse_delta_canvas = new_mouse_delta_canvas;

    // ---------------------------------------------------------------------------------------------
    // Update and draw scenes
    //
    let mut ac = &mut gc.audio_context;
    let mut dc = &mut gc.drawcontext;
    dc.start_drawing();
    {
        //_do_collision_tests(dc, new_mouse_pos_world);
        gc.gameplay_scene.update_and_draw(
            input,
            &mut gc.globals,
            &mut dc,
            &mut ac,
            &mut gc.system_commands,
        );

        gc.menu_scene.update_and_draw(
            input,
            &mut gc.globals,
            &mut dc,
            &mut ac,
            &mut gc.system_commands,
        );

        gc.debug_scene.update_and_draw(
            input,
            &mut gc.globals,
            &mut dc,
            &mut ac,
            &mut gc.system_commands,
        );
    }
    let transform = gc.globals.cam.proj_view_matrix();
    dc.finish_drawing(transform, canvas_rect, canvas_blit_rect);
}

//==================================================================================================
// Audio
//==================================================================================================
//
pub fn process_audio_internal<'game_context>(
    _input: &GameInput,
    gc: &'game_context mut GameContext<'game_context>,
    audio_output_buffer: &mut Vec<f32>,
) {
    let ac = &mut gc.audio_context;
    ac.fill_buffer(audio_output_buffer);
}

fn lalaa() {
    use time_calc::{
        Bars, Beats, Bpm, DivType, Division, Measure, Ms, Ppqn, SampleHz, Samples, Ticks, TimeSig,
    };

    // "Samples per second" is used to convert between samples and milliseconds.
    const SAMPLE_HZ: SampleHz = 44_100.0;
    // "Parts per quarter note" is used to calculate Ticks; a high resolution musical time measurement.
    const PPQN: Ppqn = 16;

    println!("time_calc demonstration!");

    // Out `Bars` type is a simplified version of a Measure.
    assert!(Bars(1).measure() == Measure(1, Division::Bar, DivType::Whole));
    // The same goes for out `Beats` type.
    assert!(Beats(1).measure() == Measure(1, Division::Beat, DivType::Whole));

    // We can use "parts per quarter note" to convert to ticks.
    println!("Parts per quarter note: {}", PPQN);
    println!("Duration of a beat in ticks: {}", Beats(1).ticks(PPQN));
    println!(
        "Duration of 38_400 ticks in beats: {}",
        Ticks(32).beats(PPQN)
    );

    // We can use "beats per minute" to convert from musical time to milliseconds.
    let bpm: Bpm = 120.0;
    println!(
        "Duration of a beat at 120 beats per minute: {} milliseconds.",
        Beats(1).ms(bpm)
    );

    // And we can use "samples per second" to convert our duration to samples.
    println!("Samples per second: {}", SAMPLE_HZ);
    println!(
        "Duration of a beat at 120bpm in samples: {}",
        Beats(1).samples(bpm, SAMPLE_HZ)
    );

    // We also need to know the "time signature" if we are to convert from "Bars".
    // This is because different time signatures can have a different duration in "Beats".
    let beats_per_bar = TimeSig { top: 4, bottom: 4 }.beats_per_bar();
    println!(
        "Duration of a bar in `Beats` with a 4/4 Time Signature: {}",
        beats_per_bar
    );

    let time_sig = TimeSig { top: 4, bottom: 4 };
    println!(
        "Duration of a bar at 120bpm, 44_100 sample_hz and 4/4 Time Sig in samples: {}",
        Bars(1).samples(bpm, time_sig, SAMPLE_HZ)
    );

    // We can also convert back the other way! Here's an example from Ms -> Beats.
    println!(
        "1 minute as a duration in beats: {}",
        Ms(60_000.0).beats(bpm)
    );

    // Here's an example from Samples -> Bars.
    println!(
        "176_400 samples as a duration in bars: {}",
        Samples(176_400).bars(bpm, time_sig, SAMPLE_HZ)
    );

    println!("Great Success!");
}
// =================================================================================================
// TODO(JaSc): Find a better place for the following functions
// =================================================================================================

/// Returns the `blit_rectangle` of for given canvas and screen rectangles.
/// The `blit-rectange` is the area of the screen where the content of the canvas is drawn onto.
/// It is as big as the canvas proportionally stretched and centered to fill the whole
/// screen.
///
/// It may or may not be smaller than the full screen size depending on the aspect
/// ratio of both the screen and the canvas. The `blit_rectange` is guaranteed to either have
/// the same width a as the screen (with letterboxing if needed) or the same height as the
/// screen (with columnboxing if needed) or completely fill the screen.
///
/// # Examples
/// ```
/// // +------+  +--------------+  +---------------+
/// // |canvas|  |   screen     |  |               | <- screen
/// // | 8x4  |  |    16x12     |  +---------------+
/// // +------+  |              |  |   blit-rect   |
/// //           |              |  |     16x10     |
/// //           |              |  |               |
/// //           |              |  |               |
/// //           |              |  |               |
/// //           |              |  |               |
/// //           |              |  +---------------+
/// //           |              |  |               |
/// //           +--------------+  +---------------+
/// //
/// // +------+  +----------------+  +-+-------------+-+
/// // |canvas|  |     screen     |  | |             | |
/// // | 8x4  |  |      18x8      |  | |             | |
/// // +------+  |                |  | |  blit-rect  | |
/// //           |                |  | |    16x8     | |
/// //           |                |  | |             | |
/// //           |                |  | |             | |
/// //           +----------------+  +-+-------------+-+
/// //                                                ^---- screen
/// //
/// // +------+  +----------------+  +-----------------+
/// // |canvas|  |     screen     |  |                 |
/// // | 8x4  |  |      16x8      |  |                 |
/// // +------+  |                |  |    blit-rect    |
/// //           |                |  |      16x8       |
/// //           |                |  |                 |
/// //           |                |  |                 |
/// //           +----------------+  +-----------------+
/// //                                                ^---- blit-rect == screen
/// ```
pub fn canvas_blit_rect(screen_rect: Rect, canvas_rect: Rect) -> Rect {
    canvas_rect
        .stretched_to_fit(screen_rect)
        .centered_in_rect(screen_rect)
}

// TODO(JaSc): Proofread and refactor this
/// Clamps a given `screen_point` to the area of the [`canvas_blit_rect`] and converts the result
/// into a canvas-position in the following interval:
/// `[0..canvas_rect.width-1]x[0..canvas_rect.height-1]`
/// where `(0,0)` is the top left of the canvas.
fn screen_pos_to_canvas_pos(screen_point: Point, screen_rect: Rect, canvas_rect: Rect) -> Point {
    // NOTE: Clamping the point needs to use integer arithmetic such that
    //          x != canvas.rect.width and y != canvas.rect.height
    //       holds. We therefore need to subtract one from the blit_rect's dimension and then
    //       add one again after clamping to achieve the desired effect.
    // TODO(JaSc): Maybe make this more self documenting via integer rectangles
    let mut blit_rect = canvas_blit_rect(screen_rect, canvas_rect);
    blit_rect.right -= 1.0;
    blit_rect.bottom -= 1.0;
    let clamped_point = screen_point.clamped_in_rect(blit_rect);
    blit_rect.right += 1.0;
    blit_rect.bottom += 1.0;

    (canvas_rect.dim() * ((clamped_point - blit_rect.pos()) / blit_rect.dim())).pixel_snapped()
}

fn screen_vec_to_canvas_vec(screen_vec: Vec2, screen_rect: Rect, canvas_rect: Rect) -> CanvasVec {
    (canvas_rect.dim() * (screen_vec / screen_rect.dim())).pixel_snapped()
}

fn pretty_format_duration_ms(duration: f64) -> String {
    format!("{:.3}ms", (duration * 1_000_000.0).round() / 1000.0)
}
