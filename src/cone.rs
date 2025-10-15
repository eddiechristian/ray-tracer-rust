use std::f64::consts::PI;

use nalgebra::{Vector2, Vector3};
use crate::material::Material;
use crate::ray::Ray;
use crate::object_base::{Intersect, IntersectionResult, EPSILON};
use crate::transform::Transform;

// Local space: apex at the origin, opening along +z to a radius-1 base cap at z = 1.
pub struct Cone {
    pub transform: Transform,
    pub material: Material,
}

impl Cone {
    pub fn new(transform: Transform, material: Material) -> Self {
        Cone { transform, material }
    }
}

impl Intersect for Cone {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult> {
        let (p, v) = self.transform.ray_to_local(ray);

        // Closest valid hit so far: (t, local normal, hit a cap rather than the side).
        let mut best: Option<(f64, Vector3<f64>, bool)> = None;
        let mut consider = |t: f64, normal: Vector3<f64>, is_cap: bool| {
            if t > EPSILON && best.map_or(true, |(bt, _, _)| t < bt) {
                best = Some((t, normal, is_cap));
            }
        };

        // Side: x^2 + y^2 = z^2, for 0 <= z <= 1.
        let a = v.x * v.x + v.y * v.y - v.z * v.z;
        let b = 2.0 * (p.x * v.x + p.y * v.y - p.z * v.z);
        let c = p.x * p.x + p.y * p.y - p.z * p.z;
        let side_ts = if a.abs() < EPSILON {
            // Ray parallel to the cone's surface: equation is linear, one root.
            if b.abs() < EPSILON { [None, None] } else { [Some(-c / b), None] }
        } else {
            let discriminant = b * b - 4.0 * a * c;
            if discriminant < 0.0 {
                [None, None]
            } else {
                [Some((-b - discriminant.sqrt()) / (2.0 * a)), Some((-b + discriminant.sqrt()) / (2.0 * a))]
            }
        };
        for t in side_ts.into_iter().flatten() {
            let point = p + v * t;
            if (0.0..=1.0).contains(&point.z) {
                // Gradient of x^2 + y^2 - z^2.
                consider(t, Vector3::new(point.x, point.y, -point.z), false);
            }
        }

        // Base cap: z = 1, inside the unit circle.
        if v.z.abs() > EPSILON {
            let t = (1.0 - p.z) / v.z;
            let point = p + v * t;
            if point.x * point.x + point.y * point.y <= 1.0 {
                consider(t, Vector3::new(0.0, 0.0, 1.0), true);
            }
        }

        let (t, local_normal, is_cap) = best?;
        let local_point = p + v * t;

        // Side: angle around the axis and height; caps: flat x, y. As in the C++.
        let uv = if is_cap {
            Vector2::new(local_point.x, local_point.y)
        } else {
            Vector2::new(local_point.y.atan2(local_point.x) / PI, local_point.z * 2.0 + 1.0)
        };

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&local_normal),
            uv,
        })
    }

    fn material(&self) -> &Material {
        &self.material
    }
}
