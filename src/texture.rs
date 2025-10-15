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
