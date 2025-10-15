# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
- `scene.rs`: `Scene` owns the camera and objects. `render` maps each pixel to normalized screen coords in [-1, 1], asks the camera for a ray, tests intersections, and writes RGBA bytes straight into the `pixels` buffer (`(y * width + x) * 4`). Right now the shading is distance-based red, with constants hand-tuned for the default camera and a unit sphere (Episode 3 behavior).
- `camera.rs`: pinhole camera that mirrors the C++ `qbRT::Camera`. `Default` precomputes the alignment vector and the projection-screen U/V/centre. The default position is `(0, -10, 0)` looking at the origin, with **Z as up**. `generate_ray` builds a ray from the camera position through a point on the projection screen.
- `object_base.rs`: the `Intersect` trait (`test_intersection(&Ray) -> Option<IntersectionResult>`). This is the Rust stand-in for the C++ `ObjectBase` virtual class, and new shapes should implement it.
- `sphere.rs`: a unit sphere at the origin (a unit struct, with no transform yet).
- `ray.rs`: `Ray { point1, point2, lab }`, where `lab = point2 - point1`, following the C++ naming. It also defines a `Color` struct that is currently unused.
- `vec3.rs`: `pub type Vec3 = nalgebra::Vector3<f64>` plus the constants `DEFAULT_POS` / `VEC3_ZERO`. `nalgebra` replaces the C++ `qbLinAlg` library. Use the nalgebra methods (`a.dot(&b)`, `a.cross(&b)`, `.norm()`, `.normalize()`). Unlike the old hand-written version, `.normalize()` on a zero vector returns NaN, not zero.

Window size (1280x720) is hard-coded separately in `main.rs`, in `Scene::new`, and in the camera's `DEFAULT_ASPECT_RATIO`. Keep all three in sync if you change it.
