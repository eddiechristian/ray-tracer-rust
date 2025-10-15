use nalgebra::{Vector2, Vector3};
use crate::material::Material;
use crate::ray::Ray;

// Minimum ray parameter t accepted as a hit, to avoid self-intersection from rounding error.
pub const EPSILON: f64 = 1e-9;

#[derive(Clone, Copy, Debug, Default)] 
pub struct IntersectionResult {
    pub intersection: Vector3<f64>,
    pub local_normal: Vector3<f64>,
    /// Surface coordinates for texture lookup; the mapping is defined by each shape.
    pub uv: Vector2<f64>,
}

pub trait Intersect {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult>;
    fn material(&self) -> &Material;
}