mod camera;
mod ray;
mod object_base;
mod sphere;
mod plane;
mod cuboid;
mod cylinder;
mod cone;
mod transform;
mod material;
mod texture;
mod scene;
mod point_light;
mod ambient_light;
mod light_base;

use std::time::Instant;

use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let scene = scene::Scene::new();

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
