use nalgebra::Vector3;
use crate::ray::{Color, Ray};
use crate::object_base::{Intersect, IntersectionResult, EPSILON};
use crate::transform::Transform;

// Local space: axis-aligned box from (-1,-1,-1) to (1,1,1).
pub struct Cuboid {
    pub transform: Transform,
    pub color: Color,
}

impl Cuboid {
    pub fn new(transform: Transform, color: Color) -> Self {
        Cuboid { transform, color }
    }
}

impl Intersect for Cuboid {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult> {
        let (p, v) = self.transform.ray_to_local(ray);

        // Slab method: intersect the ray's t-ranges inside each pair of parallel faces.
        let mut t_near = f64::NEG_INFINITY;
        let mut t_far = f64::INFINITY;
        for i in 0..3 {
            if v[i].abs() < EPSILON {
                if p[i].abs() > 1.0 {
                    return None; // Parallel to this slab and outside it
                }
            } else {
                let t1 = (-1.0 - p[i]) / v[i];
                let t2 = (1.0 - p[i]) / v[i];
                t_near = t_near.max(t1.min(t2));
                t_far = t_far.min(t1.max(t2));
            }
        }

        if t_near > t_far || t_far <= EPSILON {
            return None;
        }
        // Entry point, or exit point if the ray starts inside the box.
        let t = if t_near > EPSILON { t_near } else { t_far };
        let local_point = p + v * t;

        // The face hit is the axis where the point is furthest from the centre.
        let axis = local_point.iamax();
        let mut local_normal = Vector3::zeros();
        local_normal[axis] = local_point[axis].signum();

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&local_normal),
            color: self.color,
        })
    }
}
