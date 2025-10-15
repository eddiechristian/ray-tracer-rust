use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::ray::Ray;
use crate::object_base::{Intersect, IntersectionResult, EPSILON};
use crate::transform::Transform;

// Local space: the z = 0 plane, bounded to -1 <= x, y <= 1. Two-sided.
pub struct Plane {
    pub transform: Transform,
    pub material: Material,
}

impl Plane {
    pub fn new(transform: Transform, material: Material) -> Self {
        Plane { transform, material }
    }

    /// Local-space distance t along unit direction `v` from `p` to the hit.
    fn local_hit(p: &Vector3<f64>, v: &Vector3<f64>) -> Option<f64> {
        if v.z.abs() < EPSILON {
            return None; // Ray parallel to the plane
        }

        let t = -p.z / v.z;
        if t <= EPSILON {
            return None; // Plane is behind the ray origin
        }

        let local_point = p + v * t;
        if local_point.x.abs() > 1.0 || local_point.y.abs() > 1.0 {
            return None; // Outside the bounded region
        }
        Some(t)
    }
}

impl Intersect for Plane {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        let (p, v) = self.transform.ray_to_local(ray);
        let t = Self::local_hit(&p, &v)?;
        let local_point = p + v * t;

        // Normal faces back toward the incoming ray, so both sides can be lit.
        let local_normal = Vector3::new(0.0, 0.0, -v.z.signum());
        let uv = Vector2::new(local_point.x, local_point.y);

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&local_normal),
            uv,
            material: &self.material,
        })
    }

    fn hit_distance(&self, ray: &Ray, max_dist: f64) -> Option<f64> {
        let (p, v) = self.transform.ray_to_local(ray);
        let dist = self.transform.distance_to_world(&v, Self::local_hit(&p, &v)?);
        (dist < max_dist).then_some(dist)
    }

    fn bounds(&self) -> Aabb {
        self.transform.bounds_to_world(&Aabb::new(Vector3::new(-1.0, -1.0, 0.0), Vector3::new(1.0, 1.0, 0.0)))
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }
}
