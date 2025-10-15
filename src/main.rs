mod camera;
mod ray;
mod object_base;
mod sphere;
mod scene;

use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
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
            }
            _ => {}
        }
    })?;

    Ok(())
}
