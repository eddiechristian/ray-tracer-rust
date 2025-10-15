# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Git
Dont commit files to git the user will do that.

## What this is

A Rust port of the QuantitativeBytes "qbRayTrace" C++/SDL2 YouTube series, done episode by episode. Commits are named by chapter (e.g. "chapter 3 working"). The C++ source being ported lives in `qbRayTrace/` (`Ep1Code` … `Ep10Code`, one standalone snapshot per episode). Look at the matching `EpNCode/qbRayTrace/*.cpp` when implementing the next step. Ported C++ snippets are sometimes left as comments next to their Rust versions (see `src/camera.rs`).

`qbRayTrace/` is committed as a gitlink (submodule commit) with no `.gitmodules` file. `qbRayTrace2/` is the follow-on series (a separate git checkout, untracked here). It adds bounding boxes, composite shapes, normal maps, noise textures and ray marching. Treat both as read-only reference material, not part of the build. Note that the C++ scenes use z pointing down, while this port uses z up.

`qbRayTrace2/` is committed as a gitlink (submodule commit) with no `.gitmodules` file. Treat it as read-only reference material, This is the sequel to qbRayTrace, i.e. continuation.

## Commands

```bash
cargo build                               # dev profile is optimized (opt-level 2); unoptimized is ~45x slower
cargo run --release                       # open the 1280x720 window and render
cargo run --release -- --bench [frames]   # time N off-screen renders (default 10), no window
cargo clippy                              # lint
cargo test                                # no tests exist yet; use `cargo test <name>` to run one test
```

`.cargo/config.toml` sets `target-cpu=native`, so binaries may not run on other CPUs. `plan.md` holds the performance plan and the measured results so far. Use `--bench` to check for regressions after changing hot code. `perf` isn't installed here, so profile with valgrind callgrind: the exact commands are in `performanceImprovements.md` under "How to profile".

Running the binary needs a display: `winit` opens a window and `pixels` (wgpu) needs a GPU or software adapter.

## Architecture

- `main.rs`: sets up the `winit` 0.29 event loop and the `pixels` framebuffer. On every `Event::AboutToWait` it moves the camera along an orbit around its look-at point (`orbit_position`; the angle comes from elapsed time, one revolution per `ORBIT_PERIOD_SECS`), calls `scene.animate(time)` to move animated objects, then calls `scene.render(frame)`, which re-traces the whole image each iteration. This is intended: the camera and objects will move, so there is no caching. `winit` is pinned to 0.29 with the `rwh_05` feature so it works with `pixels` 0.13. Don't bump either crate on its own.
- `scene.rs`: `Scene::animate(time)` is where per-frame object movement goes. Today it rocks the red spray can (`Wobble`) about the rim of its base, using the transform setters and then `objects.refresh_bounds()`. `--bench` never calls it, so benchmarks use the still scene. `Scene` owns the camera, a `Vec<Box<dyn Intersect>>` of objects, and a `Vec<Box<dyn Illuminate>>` of lights. `render` traces rows in parallel with `rayon` (`par_chunks_exact_mut`), which is why `Intersect`, `Illuminate` and `Texture` require `Send + Sync`. It maps each pixel to normalized screen coords in [-1, 1] (y flipped, because buffer row 0 is the top), generates a camera ray, and calls `trace`. `trace` finds the `closest_hit` and asks that object's material for its colour. Materials call `scene.trace` again for reflection and refraction, so rendering is recursive. Each ray carries a `weight` (its share of the final pixel), and secondary rays below `MIN_CONTRIBUTION` (1/255) or beyond `MAX_DEPTH` are not traced. Colours are 0.0..=1.0, and the cast to `u8` saturates anything outside that range.
- `camera.rs`: pinhole camera that mirrors the C++ `qbRT::Camera`. The fields are private: setters (`set_position`, `set_look_at`, `set_up`, `set_length`, `set_horiz_size`, `set_aspect_ratio`) recompute the projection-screen U/V/centre (`update_geometry`, the C++ `UpdateCameraGeometry`). The default position is `(0, -10, 0)` looking at the origin, with **Z as up**. The view is narrow: about x in [-2.5, 2.5] and z in [-1.4, 1.4] at the origin. `generate_ray` builds a ray from the camera position through a point on the projection screen.
- `object_base.rs`: the `Intersect` trait.
  - `test_intersection(&Ray) -> Option<IntersectionResult<'_>>` returns the world hit point, a unit outward normal, the surface `uv`, and a `&Material` borrowed from the shape that was hit (for a composite, the sub-shape).
  - `hit_distance(&Ray, max_dist)` is the cheap version. It returns only the distance (no uv, normal or world point) and is used to pick the closest object and for shadow rays. Shapes implement both methods on top of one private `local_hit`, so they always agree. The distance is converted with `Transform::distance_to_world`.
  - `bounds()` returns an `Aabb` in the parent's space.
  - `ObjectList` (used by both `Scene` and `Composite`) caches each object's bounds. It skips objects whose box is missed or starts beyond the nearest hit found so far, and calls `test_intersection` only on the winner. `occluded()` stops at the first blocker.
  - **Moving objects:** use `objects.get_mut(i)` (with `push` returning the index), then `.transform_mut()` setters, or `.children_mut()` for a composite's sub-shapes. Then call `ObjectList::refresh_bounds()` once before rendering, because the cached boxes are otherwise stale. Debug builds panic if you forget.
  - The file also defines `EPSILON`.
- `aabb.rs`: axis-aligned bounding box. `union` merges boxes. The hot path is `entry_distance_inv(origin, inverse_direction(dir))`: callers compute the inverse direction once per ray, and the test uses multiplies and plain comparisons, not divisions or `f64::min`/`max`. That change made the whole renderer 1.7x faster, so keep it that way. `entry_distance` / `hit` are convenience wrappers. `Transform::bounds_to_world` re-fits a local box by transforming its 8 corners.
- `composite.rs`: `Composite` (the C++ `CompositeBase`) has a `Transform` plus sub-shapes positioned in its local space. `add_shape`/`with_shape` grow `local_bounds`. `test_intersection` and `hit_distance` move the ray into local space and return `None` if the bounding box is missed; otherwise they search the sub-shapes and transform the result back to world space. Composites can nest. This is the Rust stand-in for the C++ `ObjectBase` virtual class.
- `transform.rs`: `Transform` is an `Isometry3` (quaternion rotation + translation) plus a per-axis `scale`. It caches the forward, inverse and normal matrices for speed. The fields are private: change a transform only through its setters (`set_translation`, `set_rotation`, `set_rotation_euler`, `set_scale`), which rebuild the matrices. Every shape is defined in local space and uses `ray_to_local`, `point_to_world` and `normal_to_world` (which applies the inverse transpose).
- Shapes (`sphere.rs`, `plane.rs`, `cuboid.rs`, `cylinder.rs`, `cone.rs`) each have a `transform` and a `material`, and each computes `uv` (C++ mappings; the cuboid uses the two in-face coordinates). Their local-space conventions follow the C++ series: the sphere has radius 1; the plane is z = 0 with |x|, |y| <= 1 and is two-sided; the cuboid spans [-1, 1]^3; the cylinder has radius 1 with z in [-1, 1] and caps; the cone has its apex at the origin and a radius-1 base cap at z = 1. The box is called `Cuboid` because `box` is reserved in Rust.
- `light_base.rs` / `point_light.rs` / `ambient_light.rs`: the `Illuminate` trait. `PointLight` follows the C++ Ep4 formula: intensity × (1 − angle/90°) between the normal and the light direction, and `None` when the surface faces away or is in shadow. The shadow test casts a ray from the hit point toward the light, starting `SHADOW_BIAS` off the surface, against every object; that is why `compute_illumination` takes the scene's object slice. `AmbientLight` returns a constant intensity for every surface. `IlluminationResult.direction` (toward the light, or `None` for ambient) is used for specular highlights.
- `material.rs`: a single `Material` covers both C++ `SimpleMaterial` and `SimpleRefractive`. It holds a texture plus `reflectivity`, `specular`/`shininess` (Phong), and `translucency`/`ior`, configured with builder methods (`with_reflectivity` etc.). `compute_color` works as follows: diffuse is texture × sum of light intensities; that is mixed with the reflection by reflectivity, then with the refraction by translucency; then a white specular term is added. Refraction relies on normals pointing outward to tell whether a ray is entering or leaving. Secondary rays are offset by `SURFACE_BIAS`.
- `texture.rs`: the `Texture` trait (`color(uv)`), with `Flat` and `Checker`. Textures can apply a 2D uv transform (`uv_transform`: translation × rotation × scale, as in the C++). There is no image texture yet.
- `ray.rs`: `Ray { point1, point2, lab }`, where `lab = point2 - point1`, following the C++ naming. It also defines `Color { r, g, b }`, with `+`, `* f64` and component-wise `* Color`.
- Math is `nalgebra::Vector3<f64>` / `Point3<f64>` directly, with no wrapper type. `nalgebra` replaces the C++ `qbLinAlg` library. `.normalize()` on a zero vector returns NaN.

Window size (1280x720) is hard-coded separately in `main.rs`, in `Scene::new`, and in the camera's `DEFAULT_ASPECT_RATIO`. Keep all three in sync if you change it.
