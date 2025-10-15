use std::f64::consts::PI;

use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::ray::Ray;
use crate::object_base::{Intersect, IntersectionResult, EPSILON};
use crate::transform::Transform;

// Local space: unit sphere, centre (0,0,0), radius 1.
pub struct Sphere {
    pub transform: Transform,
    pub material: Material,
}

impl Sphere {
    pub fn new(transform: Transform, material: Material) -> Self {
        Sphere { transform, material }
    }

    /// Local-space distance t along unit direction `v` from `p` to the nearest hit.
    fn local_hit(p: &Vector3<f64>, v: &Vector3<f64>) -> Option<f64> {
        // |p + tv|^2 = 1, with |v| = 1 so a = 1.
        let b = 2.0 * p.dot(v);
        let c = p.dot(p) - 1.0;
        let discriminant = b * b - 4.0 * c;

        if discriminant < 0.0 {
            return None; // No intersection
        }

        // Closest intersection in front of the ray origin.
        let t1 = (-b - discriminant.sqrt()) / 2.0;
        let t2 = (-b + discriminant.sqrt()) / 2.0;
        if t1 > EPSILON { Some(t1) } else if t2 > EPSILON { Some(t2) } else { None }
    }
}

impl Intersect for Sphere {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        let (p, v) = self.transform.ray_to_local(ray);
        let t = Self::local_hit(&p, &v)?;

        // On a unit sphere at the origin, position == normal.
        let local_point = p + v * t;

        // Spherical coordinates scaled to [0, 1] and [-1, 1], as in the C++ ObjSphere.
        let (x, y, z) = (local_point.x, local_point.y, local_point.z);
        let uv = Vector2::new((x * x + y * y).sqrt().atan2(z) / PI, y.atan2(x) / PI);

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&local_point),
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
        self.transform.bounds_to_world(&Aabb::new(Vector3::repeat(-1.0), Vector3::repeat(1.0)))
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }
}
