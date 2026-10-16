mod camera;
mod ray;
mod normal_map;
mod scene_solar;
mod ui;
mod object_base;
mod sphere;
mod sdf;
mod ray_march;
mod shuttle;
mod ship;
mod chase_camera;
mod controls;
mod plane;
mod cuboid;
mod cylinder;
mod cone;
mod transform;
mod aabb;
mod composite;
mod material;
mod texture;
mod scene;
mod point_light;
mod ambient_light;
mod light_base;

use std::f64::consts::TAU;
use std::time::Instant;

use nalgebra::Vector3;

use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

// The camera circles the scene's centre once every ORBIT_PERIOD_SECS, starting from the default
// camera position (same radius and height), always looking at the centre.
const ORBIT_PERIOD_SECS: f64 = 30.0;

/// Camera position at `angle` radians around `center`, given the starting position at angle 0.
/// Rotates about the vertical (z) axis, so the height stays the same.
fn orbit_position(center: Vector3<f64>, start: Vector3<f64>, angle: f64) -> Vector3<f64> {
    let offset = start - center;
    let (sin, cos) = angle.sin_cos();
    center + Vector3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z)
}

/// Renders `frames` frames off-screen and prints timing; no window is opened.
/// Usage: `cargo run --release -- --bench [frames]` (default 10).
/// Times `frames` renders of `render` (a scene's render method), after one warm-up frame.
fn run_benchmark(frames: u32, render: impl Fn(&mut [u8])) {
    let mut buffer = vec![0u8; (WIDTH * HEIGHT * 4) as usize];

    // One untimed warm-up frame (page faults, thread pool start-up, caches).
    render(&mut buffer);

    let mut times_ms: Vec<f64> = (0..frames)
        .map(|_| {
            let start = Instant::now();
            render(&mut buffer);
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times_ms.sort_by(f64::total_cmp);

    let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
    println!(
        "{} frames at {}x{}: mean {:.1} ms ({:.2} FPS), min {:.1} ms, median {:.1} ms, max {:.1} ms",
        frames, WIDTH, HEIGHT, mean, 1000.0 / mean,
        times_ms[0], times_ms[times_ms.len() / 2], times_ms[times_ms.len() - 1],
    );
}

/// Frame count for `--bench [frames]` (default 10).
fn bench_frames(arg: Option<&String>) -> Result<u32, Box<dyn std::error::Error>> {
    let frames = match arg {
        Some(n) => n.parse().map_err(|_| format!("--bench expects a frame count, got '{n}'"))?,
        None => 10,
    };
    if frames == 0 {
        return Err("--bench frame count must be at least 1".into());
    }
    Ok(frames)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(String::as_str) == Some("--solar") {
        return run_solar(&args);
    }

    if args.get(1).map(String::as_str) == Some("--diagnose") {
        let img = image::open("textures/2k_earth_normal_map.tif")
            .expect("open tif").into_rgba8();
        println!("Normal map size: {}x{}", img.width(), img.height());
        let (mut rmin, mut rmax, mut gmin, mut gmax) = (255u8, 0u8, 255u8, 0u8);
        let (mut rmin_xy, mut rmax_xy) = ((0u32,0u32),(0u32,0u32));
        for y in 0..img.height() {
            for x in 0..img.width() {
                let p = img.get_pixel(x, y);
                if p[0] < rmin { rmin = p[0]; rmin_xy = (x,y); }
                if p[0] > rmax { rmax = p[0]; rmax_xy = (x,y); }
                if p[1] < gmin { gmin = p[1]; }
                if p[1] > gmax { gmax = p[1]; }
            }
        }
        println!("R range: {rmin}..{rmax}  (min at {:?}, max at {:?})", rmin_xy, rmax_xy);
        println!("G range: {gmin}..{gmax}");
        return Ok(());
    }

    if args.get(1).map(String::as_str) == Some("--save") {
        let scene = scene::Scene::new();
        let mut buffer = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        scene.render(&mut buffer);
        let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, buffer).expect("buffer size");
        img.save("frame.png").expect("save frame.png");
        println!("Saved frame.png");
        return Ok(());
    }

    if args.get(1).map(String::as_str) == Some("--bench") {
        let frames = bench_frames(args.get(2))?;
        let scene = scene::Scene::new();
        run_benchmark(frames, |buffer| scene.render(buffer));
        return Ok(());
    }

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Ray Tracer Framebuffer")
        .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)?;

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };

    let mut scene = scene::Scene::new();
    let orbit_center = scene.camera.look_at();
    let orbit_start = scene.camera.position();
    let start_time = Instant::now();

    let mut orbit_enabled = true;
    let mut cursor_pos = (0.0f64, 0.0f64);
    let mut orbit_time = 0.0f64; // advances only while orbit is enabled
    let mut last_wall_time = 0.0f64;

    // Buttons — add more here as needed.
    let mut orbit_btn = ui::Button::new("ORBIT: ON", 10, 10, 110, 26);
    let mut axes_btn  = ui::Button::new("AXES: ON",  130, 10, 100, 26);

    // Render continuously instead of only when an event arrives, so FPS is meaningful.
    event_loop.set_control_flow(ControlFlow::Poll);

    // FPS counter, shown in the window title and refreshed once per second.
    let mut frame_count: u32 = 0;
    let mut fps_timer = Instant::now();

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                cursor_pos = (position.x, position.y);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                },
                ..
            } => {
                let (cx, cy) = cursor_pos;
                if orbit_btn.contains(cx, cy) {
                    orbit_enabled = !orbit_enabled;
                }
                if axes_btn.contains(cx, cy) {
                    scene.axes_visible = !scene.axes_visible;
                }
                // Add more button hit-tests here.
            }
            Event::AboutToWait => {
                // 0. Move the camera along its orbit and animate the objects. Based on elapsed
                //    time, so the speed doesn't depend on the frame rate.
                let time = start_time.elapsed().as_secs_f64();
                let dt = time - last_wall_time;
                last_wall_time = time;
                if orbit_enabled {
                    orbit_time += dt;
                    let angle = orbit_time / ORBIT_PERIOD_SECS * TAU;
                    scene.camera.set_position(orbit_position(orbit_center, orbit_start, angle));
                }
                scene.animate(time);

                // 1. Get a mutable slice of your entire screen buffer
                let frame: &mut [u8] = pixels.frame_mut();

                // 2. Ray trace the scene into the buffer
                scene.render(frame);

                // 2b. Draw UI buttons on top of the rendered image.
                orbit_btn.label = if orbit_enabled { "ORBIT: ON".into() } else { "ORBIT: OFF".into() };
                orbit_btn.active = orbit_enabled;
                orbit_btn.draw(frame, WIDTH, HEIGHT);
                axes_btn.label = if scene.axes_visible { "AXES: ON".into() } else { "AXES: OFF".into() };
                axes_btn.active = scene.axes_visible;
                axes_btn.draw(frame, WIDTH, HEIGHT);
                // Add more btn.draw() calls here.

                // 3. Render the frame to the window
                if let Err(err) = pixels.render() {
                    println!("Render error: {}", err);
                    elwt.exit();
                }

                // 4. Update the FPS display
                frame_count += 1;
                let elapsed = fps_timer.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    let fps = frame_count as f64 / elapsed;
                    let orbit_str = if orbit_enabled { "ON" } else { "OFF" };
                    window.set_title(&format!("Ray Tracer - {:.1} FPS | Orbit: {orbit_str} (click green/red square to toggle)", fps));
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}

/// Interactive solar-system scene, sharing the same orbit/UI logic as the main scene.
fn run_solar(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Off-screen timing: `--solar --bench [frames]`, at the same moment as `--save`;
    // `--solar --bench-shuttle [frames]` from the shuttle camera's starting view instead.
    if matches!(args.get(2).map(String::as_str), Some("--bench" | "--bench-shuttle")) {
        let frames = bench_frames(args.get(3))?;
        let mut solar = scene_solar::SolarScene::new();
        solar.animate(5.0);
        if args[2] == "--bench-shuttle" {
            solar.camera = scene_solar::shuttle_camera()
                .camera(scene_solar::SHUTTLE_POS, &scene_solar::shuttle_orientation());
        }
        run_benchmark(frames, |buffer| solar.render(buffer));
        return Ok(());
    }

    // Close-up of the shuttle: `--solar --closeup`, saved to frame_shuttle.png.
    if args.get(2).map(String::as_str) == Some("--closeup") {
        let mut solar = scene_solar::SolarScene::new();
        solar.animate(5.0);
        // Above, behind and to the left of the shuttle in its own frame, on the sunlit side.
        let target = scene_solar::SHUTTLE_POS;
        let frame = scene_solar::shuttle_orientation();
        solar.camera = camera::Camera::new(
            target + frame * Vector3::new(-0.45, -0.55, 0.55),
            target,
            frame * Vector3::z(),
        );
        solar.camera.set_horiz_size(0.32);
        let mut buffer = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        solar.render(&mut buffer);
        let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, buffer).expect("buffer size");
        img.save("frame_shuttle.png").expect("save");
        println!("Saved frame_shuttle.png");
        return Ok(());
    }

    // Offline frame save: `--solar --save`
    if args.get(2).map(String::as_str) == Some("--save") {
        let mut solar = scene_solar::SolarScene::new();
        solar.animate(5.0); // 5 s into the 1800 s Earth year
        let mut buffer = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        solar.render(&mut buffer);
        ui::draw_label(&mut buffer, WIDTH, HEIGHT, &scene_solar::date_label(5.0), 130, 10);
        let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, buffer).expect("buffer size");
        img.save("frame_solar.png").expect("save");
        println!("Saved frame_solar.png");
        return Ok(());
    }

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Solar System")
        .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)?;
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };

    // One camera rig, steered with the mouse / arrow keys independently of the ship. VIEW
    // switches what it circles: the system's centre point or the ship (`shuttle_view`),
    // keeping its zoom and direction. C toggles free / chase (chase implies the ship).
    // Starts on the whole system.
    let mut shuttle_view = false;

    let mut solar = scene_solar::SolarScene::new();
    let mut ship = ship::Ship::new(scene_solar::SHUTTLE_POS, scene_solar::shuttle_orientation());
    let mut rig = scene_solar::system_camera();
    let mut controls = controls::Controls::default();
    let start_time   = Instant::now();

    let mut orbit_enabled  = true;
    let mut cursor_pos     = (0.0f64, 0.0f64);
    let mut dragging       = false; // right mouse button held: orbit the camera
    let mut last_wall_time = 0.0f64;
    let mut orbit_btn = ui::Button::new("ORBIT: ON", 10, 10, 110, 26);
    let mut view_btn = ui::Button::new("", 130, 10, 130, 26);
    const HELP: &str = "W/S A/D Q/E steer  Shift/Ctrl thrust/brake  R reset  |  \
                        right-drag/arrows orbit  wheel/PgUp/PgDn zoom  C free/chase";

    let mut frame_count: u32 = 0;
    let mut fps_timer = Instant::now();
    event_loop.set_control_flow(ControlFlow::Poll);

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
            Event::WindowEvent { event: WindowEvent::Focused(false), .. } => {
                // Key releases while unfocused never arrive: don't keep flying on a stuck key.
                controls.release_all();
                dragging = false;
            }
            Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let pressed = event.state == ElementState::Pressed;
                    controls.key(code, pressed);
                    if pressed && !event.repeat {
                        match code {
                            // Chase follows the ship's heading, so from the system view it
                            // switches to the ship too. The camera doesn't jump either way.
                            KeyCode::KeyC => {
                                if shuttle_view {
                                    rig.toggle_mode(&ship.orientation);
                                } else {
                                    shuttle_view = true;
                                    rig.set_mode(chase_camera::Mode::Chase, &ship.orientation);
                                }
                            }
                            KeyCode::KeyR => {
                                ship = ship::Ship::new(scene_solar::SHUTTLE_POS, scene_solar::shuttle_orientation());
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                if dragging {
                    // Drag right: the camera swings right around the ship; drag up: it rises.
                    let (dx, dy) = (position.x - cursor_pos.0, position.y - cursor_pos.1);
                    rig.rotate(dx * controls::DRAG_RADIANS_PER_PIXEL, -dy * controls::DRAG_RADIANS_PER_PIXEL);
                }
                cursor_pos = (position.x, position.y);
            }
            Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } => {
                let notches = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                rig.zoom(controls::WHEEL_ZOOM.powf(notches));
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button: MouseButton::Right, .. }, ..
            } => {
                dragging = state == ElementState::Pressed;
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput {
                    state: ElementState::Pressed, button: MouseButton::Left, ..
                }, ..
            } => {
                let (cx, cy) = cursor_pos;
                if orbit_btn.contains(cx, cy) { orbit_enabled = !orbit_enabled; }
                if view_btn.contains(cx, cy) {
                    // Back to the system's centre point: chase only applies to the ship.
                    if shuttle_view {
                        rig.set_mode(chase_camera::Mode::Free, &ship.orientation);
                    }
                    shuttle_view = !shuttle_view;
                }
            }
            Event::AboutToWait => {
                let time = start_time.elapsed().as_secs_f64();
                // Capped so a stall (window drag, first frame) doesn't fling the ship.
                let dt   = (time - last_wall_time).min(0.1);
                last_wall_time = time;

                // Ship and camera each move from their own inputs; the camera inputs steer
                // whichever view is showing.
                ship.update(&controls.ship_input(), dt);
                rig.update(&controls.camera_input(), dt);
                if orbit_enabled {
                    rig.rotate(dt / ORBIT_PERIOD_SECS * TAU, 0.0);
                }
                solar.animate(time);
                solar.set_shuttle_pose(ship.position, ship.orientation);
                solar.camera = if shuttle_view {
                    rig.camera(ship.position, &ship.orientation)
                } else {
                    // Free mode around a fixed point: the ship's orientation doesn't matter.
                    rig.camera(scene_solar::SYSTEM_VIEW.center, &nalgebra::UnitQuaternion::identity())
                };

                let frame = pixels.frame_mut();
                solar.render(frame);

                orbit_btn.label  = if orbit_enabled { "ORBIT: ON".into() } else { "ORBIT: OFF".into() };
                orbit_btn.active = orbit_enabled;
                orbit_btn.draw(frame, WIDTH, HEIGHT);
                view_btn.label  = if shuttle_view { "VIEW: SHUTTLE".into() } else { "VIEW: SYSTEM".into() };
                view_btn.active = true;
                view_btn.draw(frame, WIDTH, HEIGHT);
                ui::draw_label(frame, WIDTH, HEIGHT, &scene_solar::date_label(time), 270, 10);
                ui::draw_label(frame, WIDTH, HEIGHT, HELP, 10, HEIGHT as i32 - 36);

                if let Err(err) = pixels.render() {
                    println!("Render error: {err}");
                    elwt.exit();
                }

                frame_count += 1;
                let elapsed = fps_timer.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    let fps = frame_count as f64 / elapsed;
                    let orb = if orbit_enabled { "ON" } else { "OFF" };
                    let v = if shuttle_view { "shuttle" } else { "system" };
                    let cam = match rig.mode { chase_camera::Mode::Free => "free", chase_camera::Mode::Chase => "chase" };
                    window.set_title(&format!(
                        "Solar System - {fps:.1} FPS | Orbit: {orb} | View: {v} | Camera: {cam} | Speed: {:.2}",
                        ship.velocity.norm(),
                    ));
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
            }
            _ => {}
        }
    })?;
    Ok(())
}



