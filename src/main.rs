mod app;
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

use std::time::Instant;

use nalgebra::Vector3;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

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

    app::run(false)
}

/// `--solar ...`: the solar system's off-screen options, or the window starting on it.
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

    app::run(true)
}



