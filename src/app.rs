// The interactive window: one `winit` event loop and `pixels` framebuffer showing either the
// shapes scene (`scene.rs`) or the solar system (`scene_solar.rs`), switched with the SCENE
// button. Both scenes are built at start-up and keep their state while the other is shown; only
// the one on screen is updated and rendered. Each scene's own controls live in its `*Mode` type
// below; the ORBIT button, SCENE button and FPS counter are shared.

use std::f64::consts::TAU;
use std::time::Instant;

use nalgebra::{UnitQuaternion, Vector3};
use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::{chase_camera, controls, scene, scene_solar, ship, ui, HEIGHT, WIDTH};

// The camera circles its centre once every ORBIT_PERIOD_SECS while ORBIT is on.
const ORBIT_PERIOD_SECS: f64 = 30.0;
/// Longest frame time fed to the animation, so a stall (window drag, first frame, switching
/// scenes) doesn't make things jump.
const MAX_DT: f64 = 0.1;

/// Camera position at `angle` radians around `center`, given the starting position at angle 0.
/// Rotates about the vertical (z) axis, so the height stays the same.
fn orbit_position(center: Vector3<f64>, start: Vector3<f64>, angle: f64) -> Vector3<f64> {
    let offset = start - center;
    let (sin, cos) = angle.sin_cos();
    center + Vector3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z)
}

// ---------------------------------------------------------------------------
// Shapes scene
// ---------------------------------------------------------------------------

struct ShapesMode {
    scene: scene::Scene,
    orbit_center: Vector3<f64>,
    orbit_start: Vector3<f64>,
    orbit_time: f64, // advances only while orbit is enabled
    axes_btn: ui::Button,
}

impl ShapesMode {
    fn new() -> Self {
        let scene = scene::Scene::new();
        ShapesMode {
            orbit_center: scene.camera.look_at(),
            orbit_start: scene.camera.position(),
            scene,
            orbit_time: 0.0,
            axes_btn: ui::Button::new("AXES: ON", 270, 10, 100, 26),
        }
    }

    fn click(&mut self, cx: f64, cy: f64) {
        if self.axes_btn.contains(cx, cy) {
            self.scene.axes_visible = !self.scene.axes_visible;
        }
    }

    fn update(&mut self, time: f64, dt: f64, orbit: bool) {
        if orbit {
            self.orbit_time += dt;
            let angle = self.orbit_time / ORBIT_PERIOD_SECS * TAU;
            self.scene.camera.set_position(orbit_position(self.orbit_center, self.orbit_start, angle));
        }
        self.scene.animate(time);
    }

    fn render(&self, frame: &mut [u8]) {
        self.scene.render(frame);
    }

    fn draw_ui(&mut self, frame: &mut [u8]) {
        self.axes_btn.label = if self.scene.axes_visible { "AXES: ON".into() } else { "AXES: OFF".into() };
        self.axes_btn.active = self.scene.axes_visible;
        self.axes_btn.draw(frame, WIDTH, HEIGHT);
    }

    fn title(&self) -> String {
        "Shapes".into()
    }
}

// ---------------------------------------------------------------------------
// Solar-system scene
// ---------------------------------------------------------------------------

const SOLAR_HELP: &str = "W/S A/D Q/E steer  Shift/Ctrl thrust/brake  R reset  |  \
                          right-drag/arrows orbit  wheel/PgUp/PgDn zoom  C free/chase  Tab scene";

struct SolarMode {
    solar: scene_solar::SolarScene,
    ship: ship::Ship,
    /// One camera rig, steered independently of the ship. VIEW switches what it circles: the
    /// system's centre point or the ship (`shuttle_view`), keeping its zoom and direction.
    /// C toggles free / chase (chase implies the ship).
    rig: chase_camera::ChaseCamera,
    shuttle_view: bool,
    controls: controls::Controls,
    dragging: bool, // right mouse button held: orbit the camera
    view_btn: ui::Button,
}

impl SolarMode {
    fn new() -> Self {
        SolarMode {
            solar: scene_solar::SolarScene::new(),
            ship: ship::Ship::new(scene_solar::SHUTTLE_POS, scene_solar::shuttle_orientation()),
            rig: scene_solar::system_camera(),
            shuttle_view: false,
            controls: controls::Controls::default(),
            dragging: false,
            view_btn: ui::Button::new("", 270, 10, 130, 26),
        }
    }

    fn click(&mut self, cx: f64, cy: f64) {
        if self.view_btn.contains(cx, cy) {
            // Back to the system's centre point: chase only applies to the ship.
            if self.shuttle_view {
                self.rig.set_mode(chase_camera::Mode::Free, &self.ship.orientation);
            }
            self.shuttle_view = !self.shuttle_view;
        }
    }

    fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool) {
        self.controls.key(code, pressed);
        if !pressed || repeat {
            return;
        }
        match code {
            // Chase follows the ship's heading, so from the system view it switches to the
            // ship too. The camera doesn't jump either way.
            KeyCode::KeyC => {
                if self.shuttle_view {
                    self.rig.toggle_mode(&self.ship.orientation);
                } else {
                    self.shuttle_view = true;
                    self.rig.set_mode(chase_camera::Mode::Chase, &self.ship.orientation);
                }
            }
            KeyCode::KeyR => {
                self.ship = ship::Ship::new(scene_solar::SHUTTLE_POS, scene_solar::shuttle_orientation());
            }
            _ => {}
        }
    }

    /// Key releases while unfocused (or while the other scene is shown) never arrive here:
    /// don't keep flying on a stuck key.
    fn release_inputs(&mut self) {
        self.controls.release_all();
        self.dragging = false;
    }

    fn right_button(&mut self, pressed: bool) {
        self.dragging = pressed;
    }

    fn mouse_moved(&mut self, dx: f64, dy: f64) {
        if self.dragging {
            // Drag right: the camera swings right around its centre; drag up: it rises.
            self.rig.rotate(dx * controls::DRAG_RADIANS_PER_PIXEL, -dy * controls::DRAG_RADIANS_PER_PIXEL);
        }
    }

    fn wheel(&mut self, notches: f64) {
        self.rig.zoom(controls::WHEEL_ZOOM.powf(notches));
    }

    fn update(&mut self, time: f64, dt: f64, orbit: bool) {
        // Ship and camera each move from their own inputs.
        self.ship.update(&self.controls.ship_input(), dt);
        self.rig.update(&self.controls.camera_input(), dt);
        if orbit {
            self.rig.rotate(dt / ORBIT_PERIOD_SECS * TAU, 0.0);
        }
        self.solar.animate(time);
        self.solar.set_shuttle_pose(self.ship.position, self.ship.orientation);
        self.solar.camera = if self.shuttle_view {
            self.rig.camera(self.ship.position, &self.ship.orientation)
        } else {
            // Free mode around a fixed point: the ship's orientation doesn't matter.
            self.rig.camera(scene_solar::SYSTEM_VIEW.center, &UnitQuaternion::identity())
        };
    }

    fn render(&self, frame: &mut [u8]) {
        self.solar.render(frame);
    }

    fn draw_ui(&mut self, frame: &mut [u8], time: f64) {
        self.view_btn.label = if self.shuttle_view { "VIEW: SHUTTLE".into() } else { "VIEW: SYSTEM".into() };
        self.view_btn.active = true;
        self.view_btn.draw(frame, WIDTH, HEIGHT);
        ui::draw_label(frame, WIDTH, HEIGHT, &scene_solar::date_label(time), 410, 10);
        ui::draw_label(frame, WIDTH, HEIGHT, SOLAR_HELP, 10, HEIGHT as i32 - 36);
    }

    fn title(&self) -> String {
        let view = if self.shuttle_view { "shuttle" } else { "system" };
        let camera = match self.rig.mode {
            chase_camera::Mode::Free => "free",
            chase_camera::Mode::Chase => "chase",
        };
        format!("Solar System | View: {view} | Camera: {camera} | Speed: {:.2}", self.ship.velocity.norm())
    }
}

// ---------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------

/// Opens the window and runs until it's closed. `start_solar` picks the first scene shown.
pub fn run(start_solar: bool) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Ray Tracer")
        .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)?;
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };

    let mut shapes = ShapesMode::new();
    let mut solar = SolarMode::new();
    let mut show_solar = start_solar;

    let start_time = Instant::now();
    let mut last_wall_time = 0.0f64;
    let mut orbit_enabled = true;
    let mut cursor_pos = (0.0f64, 0.0f64);
    let mut orbit_btn = ui::Button::new("ORBIT: ON", 10, 10, 110, 26);
    let mut scene_btn = ui::Button::new("", 130, 10, 130, 26);

    // Render continuously instead of only when an event arrives, so FPS is meaningful.
    event_loop.set_control_flow(ControlFlow::Poll);
    // FPS counter, shown in the window title and refreshed once per second.
    let mut frame_count: u32 = 0;
    let mut fps_timer = Instant::now();

    event_loop.run(move |event, elwt| {
        let Event::WindowEvent { event, .. } = event else {
            if let Event::AboutToWait = event {
                let time = start_time.elapsed().as_secs_f64();
                let dt = (time - last_wall_time).min(MAX_DT);
                last_wall_time = time;

                let frame = pixels.frame_mut();
                if show_solar {
                    solar.update(time, dt, orbit_enabled);
                    solar.render(frame);
                    solar.draw_ui(frame, time);
                } else {
                    shapes.update(time, dt, orbit_enabled);
                    shapes.render(frame);
                    shapes.draw_ui(frame);
                }
                orbit_btn.label = if orbit_enabled { "ORBIT: ON".into() } else { "ORBIT: OFF".into() };
                orbit_btn.active = orbit_enabled;
                orbit_btn.draw(frame, WIDTH, HEIGHT);
                scene_btn.label = if show_solar { "SCENE: SOLAR".into() } else { "SCENE: SHAPES".into() };
                scene_btn.active = true;
                scene_btn.draw(frame, WIDTH, HEIGHT);

                if let Err(err) = pixels.render() {
                    println!("Render error: {err}");
                    elwt.exit();
                }

                frame_count += 1;
                let elapsed = fps_timer.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    let fps = frame_count as f64 / elapsed;
                    let orbit = if orbit_enabled { "ON" } else { "OFF" };
                    let scene = if show_solar { solar.title() } else { shapes.title() };
                    window.set_title(&format!("Ray Tracer - {scene} | {fps:.1} FPS | Orbit: {orbit}"));
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
            }
            return;
        };

        match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::Focused(false) => solar.release_inputs(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let pressed = event.state == ElementState::Pressed;
                if code == KeyCode::Tab {
                    // Same as the SCENE button.
                    if pressed && !event.repeat {
                        show_solar = !show_solar;
                        solar.release_inputs();
                    }
                } else if show_solar {
                    solar.key(code, pressed, event.repeat);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if show_solar {
                    solar.mouse_moved(position.x - cursor_pos.0, position.y - cursor_pos.1);
                }
                cursor_pos = (position.x, position.y);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let notches = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                if show_solar {
                    solar.wheel(notches);
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } => {
                if show_solar {
                    solar.right_button(state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                let (cx, cy) = cursor_pos;
                if orbit_btn.contains(cx, cy) {
                    orbit_enabled = !orbit_enabled;
                } else if scene_btn.contains(cx, cy) {
                    show_solar = !show_solar;
                    solar.release_inputs();
                } else if show_solar {
                    solar.click(cx, cy);
                } else {
                    shapes.click(cx, cy);
                }
            }
            _ => {}
        }
    })?;
    Ok(())
}
