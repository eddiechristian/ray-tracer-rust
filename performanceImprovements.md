# Performance improvements

What we changed to speed up the renderer, and what each change measured. The plan and the steps not done yet are in `plan.md`.

All timings come from `cargo run --release -- --bench 10`: the mean of 10 off-screen renders at 1280x720 of the current scene (16 objects, 1 point light, 1 ambient light). The machine is an Intel i7-6700 (4 cores / 8 threads) with AVX2.

## Summary

| Step | Release frame time | FPS | Speed-up | Image |
|---|---|---|---|---|
| Baseline | 1,200 ms | 0.83 | | |
| 0. Benchmark | | | | |
| 1. Optimized dev builds | (debug build: ≥ 55 s → 1.96 s) | | ~28x for `cargo run` | identical |
| 2. Parallel rows with `rayon` | 233 ms | 4.3 | 5.2x | identical |
| 3. `target-cpu=native` | 227 ms | 4.4 | 1.03x | identical |
| 4–6. Bounding-box culling, fast shadow test, deferred hit details | 138.5 ms | 7.2 | 1.6x | identical |
| 7. Fewer secondary rays | 116 ms | 8.6 | 1.2x | 1.6% of pixels changed by ≤ 2/255 |
| 8. Precomputed transform matrices | ~108 ms | 9.2 | ~1.07x | 74 pixels flipped on checker-square boundaries |
| 9. Faster bounding-box test (found by profiling) | 62 ms | 16.1 | 1.74x | identical |
| **Total** | **1,200 → 62 ms** | **0.83 → 16.1** | **19.4x** | |

After each step, the image was compared byte for byte against a reference frame rendered before the changes.

## Where the time was going (baseline)

| What | Per frame | Notes |
|---|---|---|
| Camera rays | 921,600 | one per pixel |
| Reflection/refraction rays | 915,146 | ~1 per pixel: the floor is reflective and covers much of the image |
| Shadow rays | 760,858 | one per lit hit, per point light |
| Ray-vs-shape tests | 31.5 million | ~34 per pixel |
| Pixels that hit nothing | 477,052 (52%) | each still ran the full test against all 12 top-level objects |

The renderer was single-threaded, and every candidate hit computed full details (world point, normal, UV, material) even though only the closest one was used.

## Step 0: benchmark

**Change:** `main.rs` gained a `--bench [frames]` mode that renders off-screen, with no window. It does one untimed warm-up frame, then prints the mean, min, median and max frame time.

```bash
cargo run --release -- --bench        # 10 frames
cargo run --release -- --bench 50
```

**Result:** this produced all the numbers in this document. Re-run it after changing hot code to catch regressions.

## Step 1: optimized dev builds

**Change:** added to `Cargo.toml`:

```toml
[profile.dev]
opt-level = 2
```

**Result:** a debug-build frame went from at least 55 s to 1.96 s. Debug info and debug assertions are kept, which is why it's still slower than release (1.2 s at the time). The first build after this took ~72 s, because dependencies were recompiled with optimization. Rebuilds after an edit still take under 1 s.

## Step 2: render rows in parallel with `rayon`

**Change:**
- Added the `rayon` dependency.
- `Scene::render` now uses `buffer.par_chunks_exact_mut(row_bytes).enumerate()`, so each task traces one row of pixels.
- `Intersect`, `Illuminate` and `Texture` now require `Send + Sync`, so the scene can be shared between threads. New shapes, lights and textures must be thread-safe, which is automatic unless they hold something like `Rc` or `RefCell`.

**Result:** 1,200 ms → 233 ms, 5.2x. With 4 physical cores (8 threads via hyper-threading), this is close to the ceiling. Output was byte-identical.

## Step 3: build tuning

**Changes tried:**

| Change | Result | Kept? |
|---|---|---|
| `lto = "fat"`, `codegen-units = 1` in `[profile.release]` | 233 → 235 ms, no gain. Every release rebuild took 47 s instead of 2.5 s. | No |
| `.cargo/config.toml` with `rustflags = ["-C", "target-cpu=native"]` | 233 → 227 ms, ~3%. Rebuilds still ~2.5 s. Output byte-identical. | Yes |

**Note:** with `target-cpu=native`, binaries may not run on other or older CPUs. Delete `.cargo/config.toml` to build portable binaries.

## Steps 4–6: bounding-box culling, fast shadow test, deferred hit details

These three were done together, because they share one change: a cheap "how far away is the hit?" test.

**Changes:**
- **New `Intersect::hit_distance(&Ray, max_dist) -> Option<f64>`:** returns only the distance to the nearest hit. It skips the UV maths (`atan2`), the normal and the world-space point.
  - Each shape computes both `hit_distance` and the full `test_intersection` from one private `local_hit` function, so the two always agree.
  - `Transform::distance_to_world` converts a local-space distance to world units. Only the scale matters, because rotation and translation preserve length.
  - Composites forward `hit_distance` to their sub-shapes and convert the result back.
- **New `ObjectList` in `object_base.rs`:** used by `Scene` (and its lights) and by `Composite`.
  - **Step 4, bounding boxes:** it caches each object's world-space bounding box once. Before, `bounds()` re-fitted 8 transformed corners on every call. For each ray, it skips an object whose box is missed, or whose box starts beyond the closest hit found so far. `Aabb::entry_distance` supplies the distance to the box.
  - **Step 6, details:** `closest_hit` picks the winner with `hit_distance`, then calls `test_intersection` on that one object only.
  - **Step 5, shadows:** `occluded(ray, max_dist)` answers yes or no for shadow rays and stops at the first blocker. `PointLight` uses it instead of computing full hits for every object.
- **Signature change:** `Illuminate::compute_illumination` now takes `&ObjectList` instead of `&[Box<dyn Intersect>]`.

**Result:** 227 ms → 138.5 ms, 1.6x. Output byte-identical.

## Step 7: cast fewer secondary rays

**Changes:**
- `Scene::trace` and `Material::compute_color` take a `weight`: the share of the final pixel the ray carries. Camera rays start at 1.0.
- Reflection rays carry `weight × reflectivity × (1 − translucency)`, and refraction rays carry `weight × translucency`.
- A secondary ray whose weight would be below `MIN_CONTRIBUTION` (1/255, one step of an 8-bit color channel) isn't traced. For example, a 25%-reflective floor stops after 4 bounces, because 0.25⁴ < 1/255.
- `MAX_DEPTH` was left at 5. The weight cutoff already ends opaque reflection chains after 3–4 bounces, and glass still needs room to enter and leave an object.

**Result:** 138.5 ms → 116 ms, 1.2x. 14,993 of 921,600 pixels (1.6%) changed, almost all by 1/255 and at most by 2/255, which is not visible.

## Step 8: precomputed transform matrices (moving objects supported)

**Changes:**
- **`Transform` precomputes its matrices:** the quaternion `Isometry3` and the scale are still the source of truth, but `Transform` also caches the combined matrices:
  - `forward` = R S, for local → world points.
  - `inverse` = S⁻¹ R⁻¹, plus an inverse translation, for world → local rays.
  - `normal` = R S⁻¹, the inverse transpose, for normals.

  Every ray-to-local-space conversion is now one 3x3 matrix multiply, instead of a quaternion rotation plus a component-wise divide.
- **Fields are private, with setters:** `set_translation`, `set_rotation`, `set_rotation_euler` and `set_scale` rebuild the matrices, so they can't go stale when an object moves. Matching getters exist: `translation()`, `rotation()`, `scale()`.
- **Moving objects after they're added:**
  - `Intersect` gained `transform()` / `transform_mut()`, `refresh_bounds()`, and `children_mut()` (composites return their sub-shapes).
  - `ObjectList` gained `get` / `get_mut` / `len`, and `push` returns the object's index.
- **Refreshing the cached bounding boxes (from step 4):** after moving objects, call `ObjectList::refresh_bounds()` before the next render. It recurses into composites. In debug builds, `get_mut` marks the boxes as stale and rendering without a refresh panics with a clear message. Release builds don't check.
- **Tests:**
  - The matrices match the quaternion maths after every setter.
  - A moved object (and a moved sub-shape inside a composite) is hit at its new position after a refresh.
  - A missing refresh panics in debug builds.

**Per-frame workflow for animation:**

```rust
let obj = scene.objects.get_mut(index).unwrap();
obj.transform_mut().set_translation(new_position);   // matrices update automatically
scene.objects.refresh_bounds();                      // once, after all moves
scene.render(frame);
```

Moving the camera needs none of this, because the matrices belong to the objects.

**Result:** 116 ms → ~108 ms (107–111 ms over runs), ~1.07x. 74 pixels changed: 71 of them lie exactly on the boundary between two floor checker squares, where a last-digit rounding difference picks the other square's color. Visually identical.

## Step 9: faster bounding-box test (found by profiling)

**How it was found:** `perf` isn't installed on this machine, so the renderer was profiled with valgrind's callgrind (see "How to profile" below). The step 4 bounding-box test, which exists to save work, had become the single biggest cost:

| Function | Share of all instructions in a frame |
|---|---|
| `Aabb::entry_distance` (the box test) | **61%** |
| ↳ of which `f64::min` / `f64::max` | 27% |
| All shape intersection maths combined | under 10% |

Every ray tests up to 16 boxes, and each test did 6 divisions, so about 96 divisions per ray. Division is one of the slowest CPU instructions. `f64::min`/`max` are also slower than a plain comparison, because of their NaN handling.

**Changes:**
- **Divide once per ray:** new `aabb::inverse_direction(dir)` computes `1 / direction` once per ray. The new `Aabb::entry_distance_inv(origin, inv_dir)` then uses only multiplies and plain `<` comparisons. `ObjectList::nearest` and `ObjectList::occluded` compute the inverse once and share it across all their box tests.
- **Simpler slab test:** `t_near` starts at 0, which also removes the separate "is the box behind the ray?" check.
- **Zero direction components:** these become infinite inverses, which the slab test handles naturally. The one edge case (an origin exactly on a padded face) can only cause an extra shape test, never a wrongly skipped object.
- **`entry_distance(ray)` kept** as a convenience wrapper. The composite's own box check still uses it.
- **New unit test** for the box test: hit, starting inside, box behind the ray, axis-parallel rays inside and outside a slab, a corner miss, and a flat box.

**Result:** 108 ms → 62 ms, 1.74x. Output byte-identical. Instructions per frame went from 7.04 billion to 4.50 billion.

**Profile afterwards (where the remaining time is):**

| What | Share of instructions |
|---|---|
| Box tests (now inlined into `ObjectList::nearest` and the shadow test) | ~42%: cheap per test, but every ray tests all 16 boxes |
| Shadow rays (`PointLight::compute_illumination`, total) | ~24% |
| `atan2` (texture UVs) + `acos` (point-light angle) | ~5% |
| Shape intersection maths | ~10% |

## How to profile

```bash
# Release build with debug symbols, in a separate folder so the normal build isn't disturbed
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release --target-dir /tmp/rt-prof
# One frame on one thread, counting the instructions executed in every function
RAYON_NUM_THREADS=1 valgrind --tool=callgrind --callgrind-out-file=/tmp/rt.callgrind \
    /tmp/rt-prof/release/ray-tracer-rust --bench 1
callgrind_annotate --inclusive=no  /tmp/rt.callgrind | head -40   # work done in each function itself
callgrind_annotate --inclusive=yes /tmp/rt.callgrind | head -60   # work including everything it calls
```

Callgrind runs about 50x slower than normal (~1 minute for one frame) and counts instructions, not time. A division and a multiply each count as one instruction, but the division is much slower, so read the numbers as "where the work is", not exact time.

## Not done yet

See `plan.md`:
Ideas suggested by the profile after step 9, not yet measured:
- **Fewer box tests per ray:** group objects into composites, or build a small BVH (a tree of bounding boxes), so a ray skips whole groups. For example, the floating row and the floor row could each be one group.
- **Shadow rays:** test the object that blocked the previous shadow ray first. Neighbouring pixels usually have the same blocker.
- **The floor plane:** almost every ray hits its box, so for it the box test is pure overhead.
- **Maths library calls:** compute UVs (`atan2`) only when the material's texture needs them, and avoid `acos` in the point-light intensity.
