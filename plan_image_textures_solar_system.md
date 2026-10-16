# Image Textures, Bump/Normal Maps, and Solar System Scene

## Goal
Add image-based texturing and normal/bump mapping to the renderer, then use them to build a solar system scene with photorealistic Earth, Moon, and Mars.

## Current state
`texture.rs` defines a `Texture` trait with only `color(uv)`. Two concrete types exist: `Flat` and `Checker`. No image loading, no normal perturbation. `Material::compute_color` uses `hit.local_normal` directly for all lighting. `IntersectionResult` carries `uv: Vector2<f64>` (already computed per-shape) but no tangent vectors.

## Phase 1 — Image texture

### Dependency
Add `image = "0.25"` to `Cargo.toml`. The crate decodes PNG, JPG, EXR, and more; `image::RgbaImage` is the decoded pixel buffer.

### `ImageTexture` in `texture.rs`
Store pixels as `Arc<image::RgbaImage>` (required for `Send + Sync`). Implement `Texture::color(uv)` with bilinear interpolation:
- Map the shape's UV (which lives in `[-1, 1]` for most shapes) to `[0, width) × [0, height)`.
- Sample the four nearest pixels and interpolate.
- Return as `Color { r, g, b }`.

Builder:
```rust
ImageTexture::load("earth_diffuse.jpg") -> Result<Self, _>
```

### UV conventions
Sphere UV from `sphere.rs` (confirmed by C++ reference): `u = atan2(y, x) / π ∈ [-1, 1]`, `v = z ∈ [-1, 1]`. The image sampler maps this linearly to pixel coordinates:
```
px = (u + 1) / 2 × width
py = (1 - (v + 1) / 2) × height   ← y flipped so north pole is up
```

## Phase 2 — Normal / bump maps

### Key insight from qbRayTrace2
The C++ `PerturbNormal` constructs a tangent-space frame purely from the surface normal — no stored tangent vectors needed. This avoids any changes to `IntersectionResult`:
```
up  = (0, 0, -1)                       # fallback: (1, 0, 0) when normal.z ≈ ±1
pV  = normalize(cross(up, normal))     # tangent
pU  = normalize(cross(normal, pV))     # bitangent
perturbed = normalize(normal + pU*dx + pV*dy + normal*dz)
```

### New `NormalMap` trait in `src/normal_map.rs`
```rust
pub trait NormalMap: Send + Sync {
    fn perturb(&self, normal: &Vector3<f64>, uv: &Vector2<f64>) -> Vector3<f64>;
}
```

Two concrete types:
- **`ImageNormalMap`** (for standard RGB normal-map images): reads each pixel, maps `R → (r-128)/128`, `G → (g-128)/128`, `B → b/255`, passes `{dx, dy, dz}` to `PerturbNormal`. Matches the C++ `Normal::Image`.
- **`BumpMap`** (for grayscale height maps): uses symmetric finite differences over the texture's grayscale value to get `(du, dv)`, passes `{du, dv, 0} × amplitude` to `PerturbNormal`. Equivalent to the C++ `TextureDiff`.

Both support the same UV transform as `Checker` (translate/rotate/scale).

### `Material` changes
Add optional field:
```rust
pub normal_map: Option<Box<dyn NormalMap>>,
```
Add builder:
```rust
.with_normal_map(impl NormalMap + 'static)
```
In `compute_color`, before lighting: if `normal_map` is set, replace `normal` with `normal_map.perturb(&hit.local_normal, &hit.uv)`.

## Phase 3 — Solar system scene

### Scale
Real distances are ~23,000× the planet radii, so a uniform scale makes planets invisible. Use the standard visualization approach: accurate orbital distances but planets inflated ~200×.

```
1 unit = 1 AU
Sun    : origin, radius 0.05  (real: 0.0046 AU)
Earth  : 1.00 AU orbit, radius 0.012
Moon   : 0.003 AU from Earth, radius 0.003
Mars   : 1.52 AU orbit, radius 0.006
```

### New `scene_solar.rs` (separate from the existing scene)
Create as an alternative Scene constructor so both scenes can coexist and be selected at startup (e.g. `--solar` flag). Contains:
- Sun sphere + `PointLight` at origin (single light source; dark sides of planets are naturally unlit)
- Earth, Moon, Mars spheres with `ImageTexture` diffuse and `ImageNormalMap` or `BumpMap`
- Orbital animation in `animate(t)` using Keplerian periods (time-scaled so 1 Earth year ≈ 30 screen-seconds):
  ```
  Earth angle = t * TAU / 30.0
  Mars  angle = t * TAU / (30.0 * 1.88)    # 1.88× longer period
  Moon  angle = t * TAU / (30.0 / 12.37)   # relative to Earth
  ```
- Earth self-rotation (24 h ≈ 30 s / 365 ≈ 0.082 s per sim-day)
- Camera orbits above the ecliptic plane (Z up, orbits in XY)

## Where to get textures

### Recommended: Solar System Scope — solarsystemscope.com/textures
Free under CC Attribution 4.0. Matched sets of diffuse + normal map for every major body. Download the 2K PNG versions (manageable file size, sufficient for this renderer):
- `earth_daymap.jpg` + `earth_normal_map.jpg`
- `moon_map.jpg` + `moon_normal_map.jpg` (or `moon_bump.jpg` for height-map style)
- `mars_8k_color.jpg` + `mars_8k_normal.jpg`

### Also useful
- **NASA Visible Earth** (visibleearth.nasa.gov): public domain Blue Marble composites for Earth, very high-res.
- **Planet Pixel Emporium** (planetpixelemporium.com): smaller, classic free textures with bump maps for all major planets — well-suited to this renderer's scale.

## Implementation order
1. `image` crate + `ImageTexture` + test on one sphere.
2. `BumpMap` (simpler than normal maps, grayscale input).
3. `ImageNormalMap`.
4. Solar system scene with textures applied.
5. Earth self-rotation + orbital animation.
