use std::f64::consts::PI;

use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::ray::Ray;
use crate::object_base::{Intersect, IntersectionResult, EPSILON};
use crate::transform::Transform;

// Local space: radius 1 around the z axis, from z = -1 to z = 1, with end caps.
pub struct Cylinder {
    pub transform: Transform,
    pub material: Material,
}

impl Cylinder {
    pub fn new(transform: Transform, material: Material) -> Self {
        Cylinder { transform, material }
    }
}

impl Intersect for Cylinder {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        let (p, v) = self.transform.ray_to_local(ray);

        // Closest valid hit so far: (t, local normal, hit a cap rather than the side).
        let mut best: Option<(f64, Vector3<f64>, bool)> = None;
        let mut consider = |t: f64, normal: Vector3<f64>, is_cap: bool| {
            if t > EPSILON && best.map_or(true, |(bt, _, _)| t < bt) {
                best = Some((t, normal, is_cap));
            }
        };

        // Side: x^2 + y^2 = 1, for -1 <= z <= 1.
        let a = v.x * v.x + v.y * v.y;
        let b = 2.0 * (p.x * v.x + p.y * v.y);
        let c = p.x * p.x + p.y * p.y - 1.0;
        let discriminant = b * b - 4.0 * a * c;
        if a > EPSILON && discriminant >= 0.0 {
            for t in [(-b - discriminant.sqrt()) / (2.0 * a), (-b + discriminant.sqrt()) / (2.0 * a)] {
                let point = p + v * t;
                if point.z.abs() <= 1.0 {
                    consider(t, Vector3::new(point.x, point.y, 0.0), false);
                }
            }
        }

        // Caps: z = -1 and z = 1, inside the unit circle.
        if v.z.abs() > EPSILON {
            for cap_z in [-1.0, 1.0] {
                let t = (cap_z - p.z) / v.z;
                let point = p + v * t;
                if point.x * point.x + point.y * point.y <= 1.0 {
                    consider(t, Vector3::new(0.0, 0.0, cap_z), true);
                }
            }
        }

        let (t, local_normal, is_cap) = best?;
        let local_point = p + v * t;

        // Side: angle around the axis and height; caps: flat x, y. As in the C++.
        let uv = if is_cap {
            Vector2::new(local_point.x, local_point.y)
        } else {
            Vector2::new(local_point.y.atan2(local_point.x) / PI, local_point.z)
        };

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&local_normal),
            uv,
            material: &self.material,
        })
    }

    fn bounds(&self) -> Aabb {
        self.transform.bounds_to_world(&Aabb::new(Vector3::repeat(-1.0), Vector3::repeat(1.0)))
    }
}
