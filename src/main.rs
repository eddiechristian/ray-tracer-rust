mod camera;
mod ray;
mod object_base;
mod sphere;
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
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
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
fn run_benchmark(frames: u32) {
    let scene = scene::Scene::new();
    let mut buffer = vec![0u8; (WIDTH * HEIGHT * 4) as usize];

    // One untimed warm-up frame (page faults, thread pool start-up, caches).
    scene.render(&mut buffer);

    let mut times_ms: Vec<f64> = (0..frames)
        .map(|_| {
            let start = Instant::now();
            scene.render(&mut buffer);
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--bench") {
        let frames = match args.get(2) {
            Some(n) => n.parse().map_err(|_| format!("--bench expects a frame count, got '{n}'"))?,
            None => 10,
        };
        if frames == 0 {
            return Err("--bench frame count must be at least 1".into());
        }
        run_benchmark(frames);
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
            Event::AboutToWait => {
                // 0. Move the camera along its orbit and animate the objects. Based on elapsed
                //    time, so the speed doesn't depend on the frame rate.
                let time = start_time.elapsed().as_secs_f64();
                let angle = time / ORBIT_PERIOD_SECS * TAU;
                scene.camera.set_position(orbit_position(orbit_center, orbit_start, angle));
                scene.animate(time);

                // 1. Get a mutable slice of your entire screen buffer
                let frame: &mut [u8] = pixels.frame_mut();

                // 2. Ray trace the scene into the buffer
                scene.render(frame);

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
                    window.set_title(&format!("Ray Tracer Framebuffer - {:.1} FPS ({:.1} ms/frame)", fps, 1000.0 / fps));
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}
