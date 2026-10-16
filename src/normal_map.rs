use std::sync::Arc;
use image::{ImageBuffer, Rgba};
use nalgebra::{Vector2, Vector3};

type Rgba16Image = ImageBuffer<Rgba<u16>, Vec<u16>>;

// ---------------------------------------------------------------------------
// Core perturbation helper (port of C++ qbRayTrace2 NormalBase::PerturbNormal)
// ---------------------------------------------------------------------------

/// Perturbs `normal` by `(dx, dy, dz)` in the surface's tangent-space frame.
///
/// The frame is built purely from the surface normal (no stored tangent vectors):
///   pV = normalize(cross(up, normal))   — tangent
///   pU = normalize(cross(normal, pV))   — bitangent
/// A fallback `up` vector avoids degeneracy when normal ≈ ±Z.
fn perturb_normal(normal: &Vector3<f64>, dx: f64, dy: f64, dz: f64) -> Vector3<f64> {
    let up = if normal.z.abs() > 0.99 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        Vector3::new(0.0, 0.0, -1.0)
    };
    let pv = up.cross(normal).normalize();
    let pu = normal.cross(&pv).normalize();
    (normal + pu * dx + pv * dy + normal * dz).normalize()
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Perturbs the surface normal at a hit point to add detail without changing geometry.
/// Implemented by image-based normal maps and height-field bump maps.
pub trait NormalMap: Send + Sync {
    fn perturb(&self, normal: &Vector3<f64>, uv: &Vector2<f64>) -> Vector3<f64>;
}

// ---------------------------------------------------------------------------
// ImageNormalMap  — standard RGB tangent-space normal map
// ---------------------------------------------------------------------------

/// A normal map loaded from an RGB image (PNG, JPG, TIFF …).
///
/// Supports both 8-bit and 16-bit images. Standard encoding:
///   R → X perturbation: centred at mid-range   ∈ [−1, 1]
///   G → Y perturbation: centred at mid-range   ∈ [−1, 1]
///   B → Z component:    always positive         ∈ [ 0, 1]
///
/// Solar System Scope normal maps are 16-bit TIFF; `into_rgba8()` crushes
/// the subtle perturbations to all-128. This type decodes as 16-bit so the
/// full precision is preserved.
pub struct ImageNormalMap {
    pixels:    Arc<Rgba16Image>,
    width:     u32,
    height:    u32,
    /// Scales the XY perturbation strength. 1.0 = full map strength.
    pub amplitude: f64,
}

impl ImageNormalMap {
    pub fn load(path: &str) -> Result<Self, image::ImageError> {
        let img: Rgba16Image = image::open(path)?.into_rgba16();
        Ok(Self { width: img.width(), height: img.height(), pixels: Arc::new(img), amplitude: 1.0 })
    }

    pub fn with_amplitude(mut self, amp: f64) -> Self { self.amplitude = amp; self }

    fn sample(&self, px: f64, py: f64) -> (f64, f64, f64) {
        let xi = (px.floor() as i64).rem_euclid(self.width  as i64) as u32;
        let yi = (py.floor() as i64).clamp(0, self.height as i64 - 1) as u32;
        let p = self.pixels.get_pixel(xi, yi);
        // 16-bit: neutral = 32768, range 0..=65535
        let dx = (p[0] as f64 - 32768.0) / 32768.0 * self.amplitude;
        let dy = (p[1] as f64 - 32768.0) / 32768.0 * self.amplitude;
        let dz =  p[2] as f64 / 65535.0;
        (dx, dy, dz)
    }
}

impl NormalMap for ImageNormalMap {
    fn perturb(&self, normal: &Vector3<f64>, uv: &Vector2<f64>) -> Vector3<f64> {
        // UV convention matches ImageTexture (sphere):
        //   uv.x = colatitude/π ∈ [0,1]  → image row
        //   uv.y = longitude/π  ∈ [-1,1] → image col
        let px = (uv.y + 1.0) / 2.0 * self.width  as f64;
        let py =  uv.x              * self.height as f64;
        let (dx, dy, dz) = self.sample(px, py);
        perturb_normal(normal, dx, dy, dz)
    }
}

// ---------------------------------------------------------------------------
// BumpMap  — grayscale height field, normals derived by finite differences
// ---------------------------------------------------------------------------

/// A bump map loaded from a grayscale image.
/// Normals are perturbed using symmetric finite differences of the height field,
/// matching the C++ `TextureDiff` approach.
pub struct BumpMap {
    pixels:    Arc<image::GrayImage>,
    width:     u32,
    height:    u32,
    /// Scales the gradient → how steep bumps appear. Typical range 0.5–3.0.
    pub amplitude: f64,
}

impl BumpMap {
    pub fn load(path: &str) -> Result<Self, image::ImageError> {
        let img = image::open(path)?.into_luma8();
        Ok(Self { width: img.width(), height: img.height(), pixels: Arc::new(img), amplitude: 1.0 })
    }

    pub fn with_amplitude(mut self, amp: f64) -> Self { self.amplitude = amp; self }

    fn height_at(&self, uv_x: f64, uv_y: f64) -> f64 {
        let px = (uv_y + 1.0) / 2.0 * self.width  as f64;
        let py =  uv_x              * self.height as f64;
        let xi = (px.floor() as i64).rem_euclid(self.width  as i64) as u32;
        let yi = (py.floor() as i64).clamp(0, self.height as i64 - 1) as u32;
        self.pixels.get_pixel(xi, yi)[0] as f64 / 255.0
    }
}

impl NormalMap for BumpMap {
    fn perturb(&self, normal: &Vector3<f64>, uv: &Vector2<f64>) -> Vector3<f64> {
        // Symmetric finite differences in UV space (h = 0.001, as in the C++ TextureDiff).
        const H: f64 = 0.001;
        let du = (self.height_at(uv.x, uv.y + H) - self.height_at(uv.x, uv.y - H)) / (2.0 * H);
        let dv = (self.height_at(uv.x + H, uv.y) - self.height_at(uv.x - H, uv.y)) / (2.0 * H);
        perturb_normal(normal, du * self.amplitude, dv * self.amplitude, 0.0)
    }
}
