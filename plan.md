# Performance plan

Goal: get the renderer from under 1 FPS to interactive frame rates.

## Baseline (measured 2026-10-15, 8 cores, 1280x720, current scene)

| Build | Time per frame | FPS |
|---|---|---|
| `cargo run` (debug) | ≥ 55 s | ~0.02 |
| `cargo run --release` | 1.2 s | ~0.8 |

Work per frame:

| What | Count | Notes |
|---|---|---|
| Camera rays | 921,600 | one per pixel |
| Reflection/refraction rays | 915,146 | ~1 per pixel: the floor is reflective and covers much of the image |
| Shadow rays | 760,858 | one per lit hit, per point light |
| Ray-vs-shape tests | 31.5 million | ~34 per pixel |
| Pixels that hit nothing | 477,052 (52%) | each still runs the full test against all 12 top-level objects |

The renderer is single-threaded. It re-traces the whole image every frame. That's intended, since the camera and objects will move, so the trace itself has to get fast.

## Progress

| Step | Status | Release frame time | Notes |
|---|---|---|---|
| Baseline | | 1,200 ms (0.83 FPS) | |
| 0. Benchmark | done | | `cargo run --release -- --bench [frames]` |
| 1. Optimized dev builds | done | | Debug build: ≥ 55 s → 1.96 s per frame. Rebuilds after an edit stay under 1 s. |
| 2. rayon | done | 233 ms (4.3 FPS) | 5.2x. The CPU is an i7-6700 with 4 cores / 8 threads, so this is close to the ceiling. Output is byte-identical. |
| 3. Build tuning | partly done | 227 ms (4.4 FPS) | `target-cpu=native`: ~3%, output byte-identical. LTO + `codegen-units = 1` gave no measurable gain but made every release rebuild take 47 s, so it was not kept. |
| 4–6. Bounding-box culling, fast shadow test, deferred hit details | done | 138.5 ms (7.2 FPS) | 1.6x, output byte-identical. Done together through `ObjectList` and `Intersect::hit_distance`. |
| 7. Fewer secondary rays | done | 116 ms (8.6 FPS) | 1.2x. Rays contributing < 1/255 are not traced. 1.6% of pixels changed, by at most 2/255. `MAX_DEPTH` was left at 5, since the weight cutoff already ends opaque chains after 3–4 bounces. |
| 8. Precomputed transform matrices | done | ~108 ms (9.2 FPS) | ~1.07x. Setters keep the matrices in sync; `ObjectList::refresh_bounds` handles moved objects. 74 pixels changed, almost all exactly on checker-square boundaries (rounding). |

Overall: 1,200 ms → ~108 ms, ~11x.

## Steps, in recommended order

Each step lists the expected gain. Re-measure after each one (see step 0), because the gains multiply but they also change where the remaining time goes.

### 0. Add a repeatable benchmark

- Add a way to time N renders without opening a window: for example a `--bench` command-line flag in `main.rs`, or an `#[ignore]` test run with `cargo test --release -- --ignored --nocapture`.
- Optionally, keep the ray/shape-test counters behind a Cargo feature (e.g. `stats`), so the counts above can be regenerated.
- For finding hotspots, use `cargo install flamegraph`, then run `cargo flamegraph --release`.

**Gain:** none by itself, but every later step depends on it to check for regressions.

### 1. Always run optimized code

Debug builds are about 45x slower than release builds for this kind of number-crunching. Use one of these:
- `cargo run --release`, or
- the following in `Cargo.toml`, so a plain `cargo run` is fast too:

```toml
[profile.dev]
opt-level = 2
```

**Gain:** about 45x if you're currently running debug. **Effort:** trivial.

### 2. Render rows in parallel with `rayon`

Every pixel is independent, so this scales almost linearly with cores.
- Add `rayon` and use `buffer.par_chunks_mut(width * 4).enumerate()` in `Scene::render`, so each task traces one row.
- `Scene` must be `Sync`: change `Box<dyn Intersect>` to `Box<dyn Intersect + Send + Sync>`, and do the same for `Illuminate` and `Texture`. (Alternatively, add `Send + Sync` as supertraits.)

**Gain:** about 6–7x on 8 cores (1.2 s → about 180 ms). **Effort:** small.

### 3. Tune the release build

```toml
[profile.release]
lto = "fat"
codegen-units = 1
```

Also add `.cargo/config.toml` with `rustflags = ["-C", "target-cpu=native"]` under `[build]`. This lets the compiler use AVX for the f64 math, but makes the binary non-portable.

**Gain:** about 5–20%. **Effort:** trivial.

### 4. Skip objects whose bounding box the ray misses

`closest_hit` currently runs the full `test_intersection` on every object. That includes the inverse transform, the quadratic, and building the world-space result.
- Store each top-level object's world `Aabb` once, when the scene is built. (`bounds()` re-fits 8 transformed corners on every call, so don't call it per ray.)
- In `closest_hit`, test `aabb.hit(ray)` first.
- Better still, skip boxes that start farther away than the closest hit found so far. This needs the slab test to return `t_near`.

This mostly helps the 52% of pixels that hit nothing, and shadow rays.

**Gain:** about 1.5–2x on this scene, and much more with more objects. **Effort:** small–medium.

### 5. Fast path for shadow rays

The point light needs only "is anything in the way?", but it calls `test_intersection` for every object. That computes the hit point, normal, UV (`atan2`, which is slow) and the material, then compares the distance.
- Add `fn occluded(&self, ray: &Ray, max_dist: f64) -> bool` to `Intersect`. A default implementation can call `test_intersection`; shapes override it to stop at "is there a t in (ε, max_dist)?".
- Use the bounding-box cull from step 4 here too, and stop at the first blocker (`any` already does this).

**Gain:** about 1.2–1.5x (760k shadow rays × 12 objects). **Effort:** medium.

### 6. Compute hit details only for the closest hit

Every candidate hit currently computes `point_to_world`, `normal_to_world` (a normalize) and UV, but only the closest one is used. Split intersection into two parts:
1. A cheap `t` test that returns the distance.
2. A `hit_details(&ray, t)` call made once, for the winner.

For composites, the winner is the sub-shape plus the composite's transform.

**Gain:** about 1.2–1.4x. **Effort:** medium, because it touches every shape.

### 7. Cast fewer secondary rays

There are about 915k reflection/refraction rays per frame, roughly one per pixel.
- **Stop rays that barely contribute:** pass the ray's accumulated weight (the product of reflectivity/translucency along the path) into `trace`, and stop once it drops below about 1/255. A 25%-reflective floor hit two levels deep contributes < 7%, and three levels deep < 2%.
- **Tune `MAX_DEPTH`:** 5 is only needed for glass. 3 is enough for opaque reflections.

**Gain:** about 1.2–1.5x, depending on the scene. **Effort:** small.

### 8. Precompute transform matrices

`Transform::ray_to_local` applies a quaternion inverse and a component-wise divide on every test. Instead:
- Precompute the inverse as one `Matrix3` (rotation⁻¹ with 1/scale folded in) plus a translation.
- For normals, precompute the inverse-transpose matrix.

**Gain:** about 1.1–1.2x. **Effort:** small–medium.

### Later / only if needed

- **BVH (a tree of bounding boxes):** replaces the linear object list. Essential with hundreds of objects, but not worth it for 12.
- **f32 and SIMD:** large rewrite, small win compared with the steps above.
- **GPU (wgpu compute shader):** a different project.

## Expected result

| After step | Estimated release frame time |
|---|---|
| Baseline | 1.2 s |
| 2 (rayon) | ~180 ms |
| 3 (build tuning) | ~160 ms |
| 4–7 (culling, shadow fast path, deferred details, fewer rays) | ~50–80 ms (12–20 FPS) |

These estimates come from the counts above. Confirm each one with the step 0 benchmark.
