use crate::ray::Ray;
use crate::object_base::Intersect;
use crate::object_base::IntersectionResult;
use nalgebra::{Isometry3, Point3, Translation3, UnitQuaternion, Vector3};

// A unit sphere (centre (0,0,0), radius 1) in local space, placed in the world by
// `transform` (quaternion rotation + translation) and a uniform `scale`.
// Isometry3 cannot represent scaling, so scale is stored separately.
pub struct Sphere {
    pub transform: Isometry3<f64>,
    pub scale: f64,
}

impl Sphere {
    pub fn new(translation: Vector3<f64>, scale: f64) -> Self {
        Sphere {
            transform: Isometry3::from_parts(Translation3::from(translation), UnitQuaternion::identity()),
            scale,
        }
    }

    /// Sets the rotation from Euler angles in radians: roll about X, pitch about Y,
    /// yaw about Z (applied in that order).
    pub fn set_rotation_euler(&mut self, roll: f64, pitch: f64, yaw: f64) {
        self.transform.rotation = UnitQuaternion::from_euler_angles(roll, pitch, yaw);
    }
}

impl Intersect for Sphere {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult> {
        // Transform the ray into the sphere's local space, where it is a unit sphere at the origin.
        let p = self.transform.inverse_transform_point(&Point3::from(ray.point1)).coords / self.scale;
        let v = (self.transform.inverse_transform_vector(&ray.lab) / self.scale).normalize();

        let a = 1.0;
        let b = 2.0 * p.dot(&v);

        let c = p.dot(&p) - 1.0;

        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None; // No intersection
        }

        // Find the closest positive intersection point t
        let t1: f64 = (-b + discriminant.sqrt()) / (2.0 * a);
        let t2: f64 = (-b - discriminant.sqrt()) / (2.0 * a);

        if t1 < 0.0  && t2 < 0.0{
            return None; // Hit is behind the camera
        }

        let t_min = t1.min(t2);
        // Local intersection point; on a unit sphere at the origin, position == normal.
        let local_point = p + (v * t_min);

        // Back to world space. Uniform scale doesn't change normal direction, so only rotate it.
        let intersection = self.transform.transform_point(&Point3::from(local_point * self.scale)).coords;
        let local_normal = self.transform.transform_vector(&local_point);

        Some(IntersectionResult {
            intersection,
            local_normal,
        })
    }
}
