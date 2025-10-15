use crate::ray::Ray;
use nalgebra::{Isometry3, Point3, Translation3, UnitQuaternion, Vector3};

// Places an object defined in its local coordinate system into the world:
// world = isometry(scale ⊙ local), where `isometry` is a quaternion rotation + translation.
// Isometry3 cannot represent scaling, so the (per-axis) scale is stored separately.
pub struct Transform {
    pub isometry: Isometry3<f64>,
    pub scale: Vector3<f64>,
}

impl Transform {
    pub fn new(translation: Vector3<f64>, scale: Vector3<f64>) -> Self {
        Transform {
            isometry: Isometry3::from_parts(Translation3::from(translation), UnitQuaternion::identity()),
            scale,
        }
    }

    /// Sets the rotation from Euler angles in radians: roll about X, pitch about Y,
    /// yaw about Z (applied in that order).
    pub fn set_rotation_euler(&mut self, roll: f64, pitch: f64, yaw: f64) {
        self.isometry.rotation = UnitQuaternion::from_euler_angles(roll, pitch, yaw);
    }

    /// Transforms a world-space ray into local space, returning (origin, unit direction).
    pub fn ray_to_local(&self, ray: &Ray) -> (Vector3<f64>, Vector3<f64>) {
        let p = self.isometry.inverse_transform_point(&Point3::from(ray.point1)).coords.component_div(&self.scale);
        let v = self.isometry.inverse_transform_vector(&ray.lab).component_div(&self.scale).normalize();
        (p, v)
    }

    pub fn point_to_world(&self, local_point: &Vector3<f64>) -> Vector3<f64> {
        self.isometry.transform_point(&Point3::from(local_point.component_mul(&self.scale))).coords
    }

    /// Normals transform by the inverse transpose: divide by scale, then rotate.
    pub fn normal_to_world(&self, local_normal: &Vector3<f64>) -> Vector3<f64> {
        self.isometry.transform_vector(&local_normal.component_div(&self.scale)).normalize()
    }
}
