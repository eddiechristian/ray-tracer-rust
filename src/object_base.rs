use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::ray::Ray;

// Minimum ray parameter t accepted as a hit, to avoid self-intersection from rounding error.
pub const EPSILON: f64 = 1e-9;

#[derive(Clone, Copy)]
pub struct IntersectionResult<'a> {
    pub intersection: Vector3<f64>,
    pub local_normal: Vector3<f64>,
    /// Surface coordinates for texture lookup; the mapping is defined by each shape.
    pub uv: Vector2<f64>,
    /// Material of the shape that was hit. For a composite, this is the sub-shape's material.
    pub material: &'a Material,
}

// Send + Sync so the scene can be shared by the parallel render threads.
pub trait Intersect: Send + Sync {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>>;
    /// Bounding box in the parent's coordinate space (world space for top-level objects).
    fn bounds(&self) -> Aabb;
}

/// The hit closest to the ray's origin among `objects`.
pub fn closest_hit<'a>(objects: &'a [Box<dyn Intersect>], ray: &Ray) -> Option<IntersectionResult<'a>> {
    objects
        .iter()
        .filter_map(|object| object.test_intersection(ray))
        .min_by(|a, b| {
            let dist_a = (a.intersection - ray.point1).norm_squared();
            let dist_b = (b.intersection - ray.point1).norm_squared();
            dist_a.total_cmp(&dist_b)
        })
}
