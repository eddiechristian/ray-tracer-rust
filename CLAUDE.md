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
cargo run --release                       # open the 1280x720 window (SCENE button / Tab switches shapes ↔ solar)
cargo run --release -- --solar            # same window, starting on the solar system
cargo run --release -- --bench [frames]   # time N off-screen renders (default 10), no window
cargo run --release -- --solar --bench [frames]  # same for the solar-system scene
cargo run --release -- --solar --bench-shuttle [frames]  # ...from the shuttle close-up (ray-marching cost)
cargo run --release -- --solar --save     # render one solar frame to frame_solar.png
cargo run --release -- --solar --closeup  # close-up of the shuttle to frame_shuttle.png
cargo clippy                              # lint
cargo test                                # unit tests; `cargo test <name>` runs the ones matching <name>
```

`.cargo/config.toml` sets `target-cpu=native`, so binaries may not run on other CPUs. `plan.md` holds the performance plan and the measured results so far. Use `--bench` to check for regressions after changing hot code. `perf` isn't installed here, so profile with valgrind callgrind: the exact commands are in `performanceImprovements.md` under "How to profile".

Running the binary needs a display: `winit` opens a window and `pixels` (wgpu) needs a GPU or software adapter.

## Architecture

- `main.rs`: command-line handling.
  - **The window:** with no flags, or with just `--solar`, it calls `app::run` (`--solar` starts on the solar system).
  - **Off-screen options:** `--save`, `--bench`, `--diagnose`, and `--solar --save/--bench/--bench-shuttle/--closeup` render without a window. `run_benchmark` takes a render closure, so both scenes share it.
  - **Window size:** defines `WIDTH` / `HEIGHT`.
- `app.rs`: the interactive window. It sets up the `winit` 0.29 event loop and the `pixels` framebuffer, and shows one of two scenes, switched with the **SCENE** button or **Tab**.
  - **Shared:** the ORBIT button, the SCENE button and the FPS counter in the title.
  - **Per scene:** each scene's state and controls live in `ShapesMode` / `SolarMode`, with the same small set of methods (`click`, `update`, `render`, `draw_ui`, `title`; the solar mode adds keys and mouse).
  - **Both scenes are built at start-up.** Only the one on screen is updated and rendered; the other keeps its state. Switching releases held keys.
  - **Each frame (`Event::AboutToWait`):** update the active scene from elapsed time, then `render(frame)`, which re-traces the whole image. This is intended: the camera and objects move, so there is no caching.
  - **`dt` is capped at `MAX_DT` (0.1 s),** so a stall doesn't make things jump.
  - **`winit` is pinned to 0.29** with the `rwh_05` feature so it works with `pixels` 0.13. Don't bump either crate on its own.
  - **Shapes scene:** the camera circles its look-at point (`orbit_position`, one revolution per `ORBIT_PERIOD_SECS`) while ORBIT is on, and `scene.animate(time)` moves animated objects. There's an AXES button.
  - **Solar scene:** the player flies the shuttle (see `ship.rs` / `chase_camera.rs` / `controls.rs`).
    - **One camera rig:** a single `ChaseCamera` (`system_camera()`: it starts at `SYSTEM_VIEW` and zooms from 0.35 to 20 units).
    - **VIEW button:** switches what the rig circles: `SYSTEM_VIEW.center` or the ship (`shuttle_view`). The rig keeps its zoom and angles, so switching just re-centres. It starts on the system.
    - **C** toggles free/chase without moving the camera (`ChaseCamera::set_mode` re-expresses the angles in the new frame). In system view it also switches to the ship, since chase follows the ship's heading. VIEW back to system returns to free.
    - **Camera controls** (drag, arrows, wheel, PgUp/PgDn) steer the rig.
    - **Zoom toward the mouse (system view):** the rig circles `SolarMode::pivot`, not a fixed point. The wheel picks the point under the cursor (`SolarScene::pick`, or in empty space the point at the pivot's depth) and shrinks the pivot and camera toward it by the zoom factor, so that point stays put on screen. Around the ship, the wheel zooms toward the ship. **H** resets to the starting system view. The zoom range is per view: `SYSTEM_ZOOM` (0.05–20) and `SHUTTLE_ZOOM` (0.35–20), set by `set_shuttle_view`.
    - **ORBIT button:** swings the rig automatically, adding to manual control.
    - **DOWN / UP buttons (either side of the TIME label) / -/= keys:** set the solar clock's speed (`TIME_SPEEDS`: 1x, 10x, 25x, 50x, 100x, 400x; it starts at 1x). A table at the top right (`speed_table`, drawn with `ui::draw_table`) shows how long a day, month, year and Moon orbit take at each speed, with the current one highlighted. The clock is `SolarMode::sim_time`, which advances by `dt` × speed and drives the planets and the date. The ship and camera always run in real time. The clock pauses while the shapes scene is shown.
    - **Each frame:** `ship.update(controls.ship_input())` and `rig.update(controls.camera_input())`, then `solar.animate(sim_time)`, then `solar.set_shuttle_pose`.
    - **Window focus:** losing it releases all held keys.
    - **Help:** a line of key help is drawn along the bottom.
  - **Testing the window:** X11 key events sent with `XSendEvent` reach winit when its window has focus, but mouse input goes through XInput2, so synthetic mouse events don't (hence Tab for SCENE).
- `ui.rs`: overlays drawn into the framebuffer after rendering, using `embedded-graphics` (`FONT_8X13`): the ORBIT toggle `Button`, `draw_label` (the solar scene's date) and `draw_table` (the time-speed table).
- `scene_solar.rs`: `SolarScene`, the solar-system scene (plans: `plan_image_textures_solar_system.md`, `plan_ray_marching_shuttle.md`).
  - **Contents:** the Sun, Earth, Moon and Mars as textured spheres from `textures/`, plus the shuttle. A `PointLight` sits at the Sun's centre.
  - **The Sun is kept in its own `sun` list**, not in `objects`. Otherwise every shadow ray would hit it, because the light is inside it. Its material is `with_emissive()`. `trace_inner` takes the nearer of the two lists' hits; `scene_objects()` (used for shadows) returns only `objects`.
  - **Units:** 1 AU = 2 units (`AU`). Planet sizes are inflated about 2600×, but their ratios are real. The Sun (radius 0.5) is shrunk on purpose, and the Moon's distance (0.4) isn't to scale.
  - **Time:** `time` is screen-seconds since J2000 (1 Jan 2000, 12:00). One Earth year is `EARTH_YEAR` = 1800 s (`DAYS_PER_YEAR` = 365.256 days), and all periods are sidereal. Tests check the Moon against real eclipses (2000–2025) to within 2°, so keep its period exact (`MOON_PERIOD`). The window's TIME button scales this clock. The camera's orbit (`ORBIT_PERIOD_SECS` = 30 s in `app.rs`) is separate.
  - **Orbits:** `Orbit` holds classical elements at J2000: semi-major axis, eccentricity, inclination, node, perihelion, mean longitude, plus optional node/perihelion drift. `position(time)` solves Kepler's equation. The Moon's node regresses (18.6 y) and its perigee advances (8.85 y).
  - **Spin:** Earth (23.44°) and Mars (25.19°) have axial tilts, with the north pole leaning toward +y. +x is the vernal equinox. The Moon is tidally locked: its texture's longitude 0 faces Earth.
  - **The date:** `date_label(time)` gives the calendar date shown on screen.
  - **The shuttle:** `SHUTTLE_POS` / `shuttle_orientation()` place it; `--closeup` uses both.
  - **Camera:** `CameraView` (centre, start, horiz size) defines `SYSTEM_VIEW`, which `SolarScene::new`, `--save` and `--bench` use. `system_camera()` is the window's rig (it starts there; the window moves its pivot when zooming to the mouse). `shuttle_camera()` is a close-up rig around the shuttle, used by `--bench-shuttle`. `ChaseCamera::from_offset` builds a rig from a starting camera offset.
  - **Speed:** `--bench-shuttle` times the shuttle rig's starting close-up. It's about 16× slower than the system view, because ray-marched pixels are expensive.
  - **Moving the shuttle:** `set_shuttle_pose(position, orientation)` moves it and refreshes the bounds.
- `scene.rs`: `Scene::animate(time)` is where per-frame object movement goes. Today it rocks the red spray can (`Wobble`) about the rim of its base, using the transform setters and then `objects.refresh_bounds()`. `--bench` never calls it, so benchmarks use the still scene. `Scene` owns the camera, a `Vec<Box<dyn Intersect>>` of objects, and a `Vec<Box<dyn Illuminate>>` of lights. `render` traces rows in parallel with `rayon` (`par_chunks_exact_mut`), which is why `Intersect`, `Illuminate` and `Texture` require `Send + Sync`. It maps each pixel to normalized screen coords in [-1, 1] (y flipped, because buffer row 0 is the top), generates a camera ray, and calls `trace`. `trace` finds the `closest_hit` and asks that object's material for its colour. Materials call `scene.trace` again for reflection and refraction, so rendering is recursive. Each ray carries a `weight` (its share of the final pixel), and secondary rays below `MIN_CONTRIBUTION` (1/255) or beyond `MAX_DEPTH` are not traced. Colours are 0.0..=1.0, and the cast to `u8` saturates anything outside that range.
- `camera.rs`: pinhole camera that mirrors the C++ `qbRT::Camera`. The fields are private: setters (`set_position`, `set_look_at`, `set_up`, `set_length`, `set_horiz_size`, `set_aspect_ratio`) recompute the projection-screen U/V/centre (`update_geometry`, the C++ `UpdateCameraGeometry`). The default position is `(0, -10, 0)` looking at the origin, with **Z as up**. The view is narrow: about x in [-2.5, 2.5] and z in [-1.4, 1.4] at the origin. `generate_ray` builds a ray from the camera position through a point on the projection screen.
- `object_base.rs`: the `Intersect` trait.
  - `test_intersection(&Ray) -> Option<IntersectionResult<'_>>` returns the world hit point, a unit outward normal, the surface `uv`, and a `&Material` borrowed from the shape that was hit (for a composite, the sub-shape).
  - `hit_distance(&Ray, max_dist)` is the cheap version. It returns only the distance (no uv, normal or world point) and is used to pick the closest object and for shadow rays. Shapes implement both methods on top of one private `local_hit`, so they always agree. The distance is converted with `Transform::distance_to_world`.
  - `bounds()` returns an `Aabb` in the parent's space.
  - `ObjectList` (used by both `Scene` and `Composite`) caches each object's bounds. It skips objects whose box is missed or starts beyond the nearest hit found so far, and calls `test_intersection` only on the winner. `occluded()` stops at the first blocker.
  - **Moving objects:** use `objects.get_mut(i)` (with `push` returning the index), then `.transform_mut()` setters, or `.children_mut()` for a composite's sub-shapes. Then call `ObjectList::refresh_bounds()` once before rendering, because the cached boxes are otherwise stale. Debug builds panic if you forget.
  - The file also defines `EPSILON`.
- `aabb.rs`: axis-aligned bounding box. `union` merges boxes. The hot path is `entry_distance_inv(origin, inverse_direction(dir))`: callers compute the inverse direction once per ray, and the test uses multiplies and plain comparisons, not divisions or `f64::min`/`max`. That change made the whole renderer 1.7x faster, so keep it that way. `entry_distance` / `hit` are convenience wrappers. `span` returns both entry and exit, for ray marching. `Transform::bounds_to_world` re-fits a local box by transforming its 8 corners.
- `composite.rs`: `Composite` (the C++ `CompositeBase`) has a `Transform` plus sub-shapes positioned in its local space. `add_shape`/`with_shape` grow `local_bounds`. `test_intersection` and `hit_distance` move the ray into local space and return `None` if the bounding box is missed; otherwise they search the sub-shapes and transform the result back to world space. Composites can nest. This is the Rust stand-in for the C++ `ObjectBase` virtual class.
- `transform.rs`: `Transform` is an `Isometry3` (quaternion rotation + translation) plus a per-axis `scale`. It caches the forward, inverse and normal matrices for speed. The fields are private: change a transform only through its setters (`set_translation`, `set_rotation`, `set_rotation_euler`, `set_scale`), which rebuild the matrices. Every shape is defined in local space and uses `ray_to_local`, `point_to_world` and `normal_to_world` (which applies the inverse transpose).
- Shapes (`sphere.rs`, `plane.rs`, `cuboid.rs`, `cylinder.rs`, `cone.rs`) each have a `transform` and a `material`, and each computes `uv` (C++ mappings; the cuboid uses the two in-face coordinates). Their local-space conventions follow the C++ series: the sphere has radius 1; the plane is z = 0 with |x|, |y| <= 1 and is two-sided; the cuboid spans [-1, 1]^3; the cylinder has radius 1 with z in [-1, 1] and caps; the cone has its apex at the origin and a radius-1 base cap at z = 1. The box is called `Cuboid` because `box` is reserved in Rust.
- `sdf.rs`: signed distance functions ported from Inigo Quilez (https://iquilezles.org/articles/distfunctions/).
  - **Shapes:** 3D primitives, plus 2D `triangle`/`trapezoid` for `extrude`.
  - **Operators:** union/subtract/intersect and their smooth versions, `round`, `mirror_y`, and `union_id`, which keeps the nearer shape's material index.
  - **Axis:** round shapes use z as their axis (the GLSL originals use y).
  - **Exactness:** `ellipsoid`, `tri_prism` and the smooth operators are bounds, not exact distances.
- `ray_march.rs`: `RayMarched` (the C++ `RM::RayMarchBase`) is a shape defined by an `Sdf` closure that returns `(distance, material index)`. That index lets one shape have several materials.
  - **Marching** runs only inside the local bounding box (`Aabb::span`). A point within `HIT_EPSILON` = 1e-4 counts as a hit, with at most 256 steps. The normal comes from four SDF samples (iq's tetrahedron).
  - **Bounds:** the box must contain the whole surface. Give it a margin: `shuttle.rs` tests this.
  - **Shadow and reflection rays** start inside the box and on the surface. They first step clear of the surface, but only when the ray starts inside the box (`entry == 0`). Without that check, a surface touching its box gets hit from outside at a point inside it, and then shadows itself.
  - **Keep the transform's scale uniform:** per-axis scale distorts SDF distances.
  - **Step scale:** use `with_step_scale(<1)` when the SDF can overestimate (smooth blends).
- `shuttle.rs`: the Space Shuttle orbiter as one `RayMarched` shape.
  - **Model space:** nose +x, z up, 2 units long, with real proportions.
  - **Parts:** fuselage, nose and flight deck; double-delta wings mirrored in y; fin; OMS pods; body flap; three hollow engine bells.
  - **Materials:** the `WHITE`/`BLACK`/`ENGINE`/`WINDOW` indices, chosen by position in `hull_material`.
  - **Tests:** the shape fits its bounds, proportions are right, and the SDF's slope outside the hull stays under 1/0.8.
- `ship.rs`: `Ship { position, orientation, velocity }`, flown arcade-style by a `ShipInput` (pitch/yaw/roll/thrust, each -1..=1).
  - **Ship space** matches the shuttle model: nose +x, left wing +y, up +z. +pitch = nose up, +yaw = nose left, +roll = right wing down.
  - **Movement:** turn rates are applied about the ship's own axes, thrust pushes along the nose, and exponential drag gives a top speed of `THRUST_ACCEL / DRAG`.
  - **Frame-rate independent:** `update` splits `dt` into steps of at most `MAX_STEP` (1/120 s), so the path is the same at any frame rate (tested).
- `chase_camera.rs`: `ChaseCamera`, the camera rig around the ship, steered independently of it by a `CameraInput` (orbit_x/orbit_y/zoom), plus `rotate`/`zoom` for mouse drags and the wheel.
  - **Angles:** azimuth 0 is behind the ship (−x), and elevation is clamped to ±89°.
  - **Modes:** `Free` measures the angles from the scene's axes, so the ship turns while the view stays put. `Chase` measures them from the ship's axes, with the ship's up as the camera's up. Switch with `set_mode` / `toggle_mode(target_orientation)`, which keep the camera where it is.
- `controls.rs`: keyboard/mouse → `ShipInput` / `CameraInput`, the only device-specific code. A game controller (e.g. the `gilrs` crate) should fill the same inputs.
  - **Ship:** W/S pitch (W = nose down), A/D yaw, Q/E roll, Shift/Ctrl thrust/brake.
  - **Camera:** right-drag or the arrow keys orbit; the wheel or PgUp/PgDn zoom; C toggles free/chase.
  - **R** resets the ship, **H** resets the camera to the system view. The C, R and H key presses are handled in `app.rs`.
- `light_base.rs` / `point_light.rs` / `ambient_light.rs`: the `Illuminate` trait. `PointLight` follows the C++ Ep4 formula: intensity × (1 − angle/90°) between the normal and the light direction, and `None` when the surface faces away or is in shadow. The shadow test casts a ray from the hit point toward the light, starting `SHADOW_BIAS` off the surface, against every object; that is why `compute_illumination` takes the scene's object slice. `AmbientLight` returns a constant intensity for every surface. `IlluminationResult.direction` (toward the light, or `None` for ambient) is used for specular highlights.
- `material.rs`: a single `Material` covers both C++ `SimpleMaterial` and `SimpleRefractive`. It holds a texture plus `reflectivity`, `specular`/`shininess` (Phong), and `translucency`/`ior`, configured with builder methods (`with_reflectivity` etc.). `compute_color` works as follows: diffuse is texture × sum of light intensities; that is mixed with the reflection by reflectivity, then with the refraction by translucency; then a white specular term is added. Refraction relies on normals pointing outward to tell whether a ray is entering or leaving. Secondary rays are offset by `SURFACE_BIAS`. An optional `normal_map` perturbs the normal before lighting. `with_emissive()` returns the texture colour unlit (the Sun). `with_headlight(strength)` adds diffuse light from the viewer's direction (strength × cos of the angle between the normal and the view ray), with no shadow rays. It keeps the shuttle visible on its night side (`shuttle::HEADLIGHT` = 0.35).
- `texture.rs`: the `Texture` trait (`color(uv)`), with `Flat`, `Checker` and `ImageTexture` (loaded with the `image` crate, sampled bilinearly from the sphere's uv). Textures can apply a 2D uv transform (`uv_transform`: translation × rotation × scale, as in the C++).
- `normal_map.rs`: the `NormalMap` trait, with `ImageNormalMap` (RGB normal-map images) and `BumpMap` (grayscale height maps). Both build a tangent frame from the normal alone, as in the qbRayTrace2 `PerturbNormal`.
- `ray.rs`: `Ray { point1, point2, lab }`, where `lab = point2 - point1`, following the C++ naming. It also defines `Color { r, g, b }`, with `+`, `* f64` and component-wise `* Color`.
- Math is `nalgebra::Vector3<f64>` / `Point3<f64>` directly, with no wrapper type. `nalgebra` replaces the C++ `qbLinAlg` library. `.normalize()` on a zero vector returns NaN.

Window size (1280x720) is hard-coded separately in `main.rs` (`WIDTH`/`HEIGHT`), in `Scene::new`, in `SolarScene::new`, and in the camera's `DEFAULT_ASPECT_RATIO`. Keep all four in sync if you change it.
