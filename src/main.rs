mod camera;
mod ray;
mod vec3;

use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

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

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),
            Event::AboutToWait => {
                // 1. Get a mutable slice of your entire screen buffer
                let frame = pixels.frame_mut();

                // 2. Sample pixel math (coloring coordinate x:100, y:134)
                let x = 100;
                let y = 134;
                let color = (223, 234, 56); // RGB

                // Formula to find the 1D index in an RGBA buffer
                let pixel_index = ((y * WIDTH as usize) + x) * 4;

                if pixel_index + 3 < frame.len() {
                    frame[pixel_index] = color.0; // R
                    frame[pixel_index + 1] = color.1; // G
                    frame[pixel_index + 2] = color.2; // B
                    frame[pixel_index + 3] = 255; // A
                }

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
