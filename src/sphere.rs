use crate::ray::{Color, Ray};
use crate::object_base::{Intersect, IntersectionResult, EPSILON};
use crate::transform::Transform;

// Local space: unit sphere, centre (0,0,0), radius 1.
pub struct Sphere {
    pub transform: Transform,
    pub color: Color,
}

impl Sphere {
    pub fn new(transform: Transform, color: Color) -> Self {
        Sphere { transform, color }
    }
}

impl Intersect for Sphere {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult> {
        let (p, v) = self.transform.ray_to_local(ray);

        // |p + tv|^2 = 1, with |v| = 1 so a = 1.
        let b = 2.0 * p.dot(&v);
        let c = p.dot(&p) - 1.0;
        let discriminant = b * b - 4.0 * c;

        if discriminant < 0.0 {
            return None; // No intersection
        }

        // Closest intersection in front of the ray origin.
        let t1 = (-b - discriminant.sqrt()) / 2.0;
        let t2 = (-b + discriminant.sqrt()) / 2.0;
        let t = if t1 > EPSILON { t1 } else if t2 > EPSILON { t2 } else { return None };

        // On a unit sphere at the origin, position == normal.
        let local_point = p + v * t;

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&local_point),
            color: self.color,
        })
    }
}
