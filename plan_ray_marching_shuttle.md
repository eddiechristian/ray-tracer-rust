# Ray Marching and a Space Shuttle

## Goal
Add ray-marched shapes defined by signed distance functions (SDFs), using the catalogue at
https://iquilezles.org/articles/distfunctions/, and use them to model a space shuttle for the
solar-system scene.

## Reference
qbRayTrace2 has a C++ version in `qbRayTrace2/qbRayTrace/qbRayMarch/` (`raymarchbase.cpp`,
`sdfunc.cpp`, `sphere.cpp`, `torus.cpp`, `cube.cpp`). This port follows it, but fixes three
weaknesses:
- **Where the march runs.** The C++ marches from the ray origin and only gives up once the
  distance passes 1e6. Here the ray is clipped to the shape's bounding box: the march starts
  where the ray enters it and stops where it leaves.
- **Normals.** The C++ takes six SDF samples per normal. Here it's four (Quilez's
  "tetrahedron" method).
- **Hit tolerance.** The C++ uses ε = 1e-12, which in practice means a ray can run out of
  steps. Here ε is about 1e-4 in local units.

## How it works
An SDF returns the distance from a point to the surface (negative inside). Sphere tracing steps
along the ray by that distance until it falls below ε. The normal is the SDF's gradient,
estimated from a few nearby samples.

## Plan

### 1. `src/sdf.rs`: distance functions and operators
Plain functions on `Vector3<f64>`, ported from Quilez's page:
- **3D shapes:** sphere, box, rounded box, capsule, capped cylinder, capped cone, round cone,
  ellipsoid, triangular prism, torus.
- **2D shapes to extrude into 3D** (wing and fin outlines): triangle, trapezoid.
- **Operators:** union, subtraction, intersection, and their smooth versions; rounding;
  extrusion; mirroring (`p.y = |p.y|`).

Unit tests check these against exact distances.

### 2. `src/ray_march.rs`: `RayMarched` shape implementing `Intersect`
- **Fields:** `transform`, a local `Aabb`, `materials: Vec<Material>`, and an SDF closure
  `Fn(&Vector3<f64>) -> (f64, usize) + Send + Sync`.
  - The closure returns the distance and a **material index**, so one shape can have several
    materials. `IntersectionResult` borrows `&materials[id]`.
- **`local_hit` is shared** by `test_intersection` and `hit_distance`:
  1. Clip the ray to the local box (`Aabb::span`). Return `None` if the ray misses it, and stop
     no later than `max_dist` for shadow rays.
  2. Sphere-trace: step `d × step_scale` until `d < ε`, for at most `MAX_STEPS` steps or until
     the box exit. Use `step_scale = 1.0` for exact SDFs and about 0.8 for smooth blends.
  3. Normal: Quilez's four-sample "tetrahedron" gradient.
  4. uv: two local coordinates picked by the normal's largest component, as with the cuboid's
     faces.

### 3. Gotchas
- **Shadow acne.** Secondary rays start 1e-6 off the surface, which is inside ε, so the march
  would "hit" the surface it started on. If the march starts within the escape distance of a
  surface, first step until `d` grows past it; if `d` goes negative, the ray went inside, which
  counts as a hit.
- **Uniform scale only.** Scaling an SDF differently along each axis breaks its distances.
  Shape the model inside the SDF and give the `Transform` one uniform scale.
- **Thin parts** (wings) can be stepped through. Give them some thickness with `opRound` and keep
  ε well below their thickness.
- **Speed.** A ray costs about 30–100 SDF evaluations, each over about 15 shapes. The bounding
  box limits this to the pixels the shape covers. Check with `--bench` and callgrind.

### 4. Tests
- An SDF sphere matches the analytic `Sphere` (hit point, normal, `hit_distance`) under the same
  translated, rotated, uniformly scaled transform.
- A ray leaving an SDF surface isn't blocked by it; a ray going into it is.
- A ray that misses the box returns `None`, and `max_dist` cuts off far hits.

### 5. `src/shuttle.rs`: the shuttle (local space, nose along +x, z up)
| Part | SDF recipe | Material |
|---|---|---|
| Fuselage | rounded box stretched along x, smooth-unioned with a round-cone nose | white, black underneath (`p.z` threshold) |
| Delta wings | extruded 2D triangle, mirrored in y, rounded, smooth-unioned | white top, black underside |
| Tail fin | extruded 2D trapezoid, upright | white |
| Engines | 3 capped cones at the tail | dark grey |
| Cockpit windows | small boxes used only to choose the material | black, slightly reflective |
| Payload bay seam (optional) | thin subtracted groove | — |
| Launch stack (optional) | orange capsule tank, two white capsule boosters with cone tips | orange / white |

Build it one part at a time, checking each with `cargo run --release -- --solar --save`.

### 6. Scene integration
- Replace the saucer, or keep both, at `SHIP_POS` with a uniform scale of about 0.15.
- Optional flight path in `animate()`: a low Earth orbit (reusing `Orbit`) or an Earth→Mars
  transfer, with the nose pointing along the velocity.

## Implementation order
1. `sdf.rs` + tests.
2. `RayMarched` with an SDF sphere, matched against the analytic sphere, plus the shadow-acne test.
3. One SDF box in the solar scene: check how it looks and how fast it renders.
4. The shuttle, part by part.
5. Several materials, then the flight path.
