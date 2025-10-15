# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Git
Dont commit files to git the user will do that.

## What this is

A Rust port of the QuantitativeBytes "qbRayTrace" C++/SDL2 YouTube series, done episode by episode. Commits are named by chapter (e.g. "chapter 3 working"). The C++ source being ported lives in `qbRayTrace/` (`Ep1Code` … `Ep10Code`, one standalone snapshot per episode). Look at the matching `EpNCode/qbRayTrace/*.cpp` when implementing the next step. Ported C++ snippets are sometimes left as comments next to their Rust versions (see `src/camera.rs`).

`qbRayTrace/` is committed as a gitlink (submodule commit) with no `.gitmodules` file. Treat it as read-only reference material, not part of the build.

## Commands

```bash
cargo build            # build
cargo run --release    # open the 1280x720 window and render; use release, since the debug build re-renders slowly
cargo clippy           # lint
cargo test             # no tests exist yet; use `cargo test <name>` to run one test
```

Running the binary needs a display: `winit` opens a window and `pixels` (wgpu) needs a GPU or software adapter.

## Architecture

- `main.rs`: sets up the `winit` 0.29 event loop and the `pixels` framebuffer. On every `Event::AboutToWait` it calls `scene.render(frame)`, which re-traces the whole image each iteration (no caching). `winit` is pinned to 0.29 with the `rwh_05` feature so it works with `pixels` 0.13. Don't bump either crate on its own.
- `scene.rs`: `Scene` owns the camera, a `Vec<Box<dyn Intersect>>` of objects, and a `Vec<Box<dyn Illuminate>>` of lights. `render` maps each pixel to normalized screen coords in [-1, 1] (y flipped, because buffer row 0 is the top), asks the camera for a ray, and keeps the closest hit across all objects. The colour written is the object's colour times the sum of all light intensities, as RGBA bytes straight into the `pixels` buffer (`(y * width + x) * 4`). Colours are 0.0..=1.0.
- `camera.rs`: pinhole camera that mirrors the C++ `qbRT::Camera`. `Default` precomputes the alignment vector and the projection-screen U/V/centre. The default position is `(0, -10, 0)` looking at the origin, with **Z as up**. The view is narrow: about x in [-2.5, 2.5] and z in [-1.4, 1.4] at the origin. `generate_ray` builds a ray from the camera position through a point on the projection screen.
- `object_base.rs`: the `Intersect` trait (`test_intersection(&Ray) -> Option<IntersectionResult>`, which returns the world hit point, a unit normal and a colour) plus `EPSILON`. This is the Rust stand-in for the C++ `ObjectBase` virtual class.
- `transform.rs`: `Transform` is an `Isometry3` (quaternion rotation + translation) plus a per-axis `scale`, with Euler-angle rotation. Every shape is defined in local space and uses `ray_to_local`, `point_to_world` and `normal_to_world` (which applies the inverse transpose).
- Shapes (`sphere.rs`, `plane.rs`, `cuboid.rs`, `cylinder.rs`, `cone.rs`) each have a `transform` and a `color`. Their local-space conventions follow the C++ series: the sphere has radius 1; the plane is z = 0 with |x|, |y| <= 1 and is two-sided; the cuboid spans [-1, 1]^3; the cylinder has radius 1 with z in [-1, 1] and caps; the cone has its apex at the origin and a radius-1 base cap at z = 1. The box is called `Cuboid` because `box` is reserved in Rust.
- `light_base.rs` / `point_light.rs` / `ambient_light.rs`: the `Illuminate` trait. `PointLight` follows the C++ Ep4 formula: intensity × (1 − angle/90°) between the normal and the light direction, and `None` when the surface faces away. `AmbientLight` returns a constant intensity for every surface.
- `ray.rs`: `Ray { point1, point2, lab }`, where `lab = point2 - point1`, following the C++ naming. It also defines `Color { r, g, b }`.
- Math is `nalgebra::Vector3<f64>` / `Point3<f64>` directly, with no wrapper type. `nalgebra` replaces the C++ `qbLinAlg` library. `.normalize()` on a zero vector returns NaN.

Window size (1280x720) is hard-coded separately in `main.rs`, in `Scene::new`, and in the camera's `DEFAULT_ASPECT_RATIO`. Keep all three in sync if you change it.
