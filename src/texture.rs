use std::sync::Arc;
use nalgebra::{Matrix3, Point2, Vector2};
use crate::ray::Color;

// A texture maps a surface (u, v) coordinate, as computed by each shape, to a color.
pub trait Texture: Send + Sync {
    fn color(&self, uv: &Vector2<f64>) -> Color;
}

// 2D affine transform applied to (u, v) before lookup: translation * rotation * scale,
// matching the C++ TextureBase::SetTransform. Larger scale = smaller, more repeated pattern.
pub fn uv_transform(translation: Vector2<f64>, rotation: f64, scale: Vector2<f64>) -> Matrix3<f64> {
    Matrix3::new_translation(&translation) * Matrix3::new_rotation(rotation) * Matrix3::new_nonuniform_scaling(&scale)
}

/// A single solid color.
pub struct Flat {
    pub color: Color,
}

impl Flat {
    pub fn new(color: Color) -> Self {
        Flat { color }
    }
}

impl Texture for Flat {
    fn color(&self, _uv: &Vector2<f64>) -> Color {
        self.color
    }
}

/// Alternating squares of two colors, one square per unit of (transformed) u and v.
pub struct Checker {
    pub color1: Color,
    pub color2: Color,
    pub transform: Matrix3<f64>,
}

impl Checker {
    pub fn new(color1: Color, color2: Color) -> Self {
        Checker { color1, color2, transform: Matrix3::identity() }
    }

    pub fn with_transform(mut self, translation: Vector2<f64>, rotation: f64, scale: Vector2<f64>) -> Self {
        self.transform = uv_transform(translation, rotation, scale);
        self
    }
}

impl Texture for Checker {
    fn color(&self, uv: &Vector2<f64>) -> Color {
        let p = self.transform.transform_point(&Point2::from(*uv));
        let check = p.x.floor() as i64 + p.y.floor() as i64;
        if check.rem_euclid(2) == 0 { self.color1 } else { self.color2 }
    }
}

/// An image texture loaded from a file (PNG, JPG, TIFF, etc.).
/// UV mapping follows the sphere convention:
///   uv.x = colatitude / π ∈ [0, 1]   (0 = north pole, 1 = south pole)
///   uv.y = longitude  / π ∈ [-1, 1]  (wraps around the equator)
pub struct ImageTexture {
    pixels: Arc<image::RgbaImage>,
    width:  u32,
    height: u32,
}

impl ImageTexture {
    /// Loads an image from `path`. Supports PNG, JPG, TIFF, and most common formats.
    pub fn load(path: &str) -> Result<Self, image::ImageError> {
        let img = image::open(path)?.into_rgba8();
        Ok(Self {
            width:  img.width(),
            height: img.height(),
            pixels: Arc::new(img),
        })
    }

    /// Bilinear sample at continuous pixel coordinates (wraps horizontally, clamps vertically).
    fn sample(&self, px: f64, py: f64) -> Color {
        let x0 = px.floor() as i64;
        let y0 = py.floor() as i64;
        let tx = px - x0 as f64;
        let ty = py - y0 as f64;

        let w = self.width  as i64;
        let h = self.height as i64;

        let get = |xi: i64, yi: i64| {
            let xi = xi.rem_euclid(w) as u32;
            let yi = yi.clamp(0, h - 1) as u32;
            let p = self.pixels.get_pixel(xi, yi);
            (p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0)
        };

        let (r00, g00, b00) = get(x0,     y0);
        let (r10, g10, b10) = get(x0 + 1, y0);
        let (r01, g01, b01) = get(x0,     y0 + 1);
        let (r11, g11, b11) = get(x0 + 1, y0 + 1);

        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let bi   = |v00, v10, v01, v11| lerp(lerp(v00, v10, tx), lerp(v01, v11, tx), ty);

        Color::new(bi(r00, r10, r01, r11), bi(g00, g10, g01, g11), bi(b00, b10, b01, b11))
    }
}

impl Texture for ImageTexture {
    fn color(&self, uv: &Vector2<f64>) -> Color {
        // uv.x = colatitude/π ∈ [0,1]  → image row  (0 = top)
        // uv.y = longitude/π  ∈ [-1,1] → image col  (wraps)
        let px = (uv.y + 1.0) / 2.0 * self.width  as f64;
        let py =  uv.x              * self.height as f64;
        self.sample(px, py)
    }
}
