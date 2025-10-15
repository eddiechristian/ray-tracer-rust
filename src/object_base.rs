use nalgebra::Vector3;
use crate::ray::Ray;
use crate::ray::Color;

// Minimum ray parameter t accepted as a hit, to avoid self-intersection from rounding error.
pub const EPSILON: f64 = 1e-9;

#[derive(Clone, Copy, Debug, Default)] 
pub struct IntersectionResult {
    pub intersection: Vector3<f64>,
    pub local_normal: Vector3<f64>,
    pub color: Color,
}

pub trait Intersect {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult>;
}