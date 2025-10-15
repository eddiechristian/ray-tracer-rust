use crate::aabb::Aabb;
use crate::ray::Ray;
use nalgebra::{Isometry3, Matrix3, Translation3, UnitQuaternion, Vector3};

// Places an object defined in its local coordinate system into the world:
// world = isometry(scale ⊙ local), where `isometry` is a quaternion rotation + translation.
// Isometry3 cannot represent scaling, so the (per-axis) scale is stored separately.
//
// Rays are transformed millions of times per frame, so the combined rotation+scale matrices
// are precomputed. The fields are private and every setter rebuilds the matrices, so they can
// never go stale when an object moves.
pub struct Transform {
    isometry: Isometry3<f64>,
    scale: Vector3<f64>,

    // Precomputed from `isometry` and `scale` by `update_matrices`:
    /// world = forward * local + translation   (R S)
    forward: Matrix3<f64>,
    /// local = inverse * world + inverse_translation   (S⁻¹ R⁻¹)
    inverse: Matrix3<f64>,
    inverse_translation: Vector3<f64>,
    /// Normals use the inverse transpose of `forward`: (R S)⁻ᵀ = R S⁻¹.
    normal: Matrix3<f64>,
}

impl Transform {
    pub fn new(translation: Vector3<f64>, scale: Vector3<f64>) -> Self {
        let mut transform = Transform {
            isometry: Isometry3::from_parts(Translation3::from(translation), UnitQuaternion::identity()),
            scale,
            forward: Matrix3::identity(),
            inverse: Matrix3::identity(),
            inverse_translation: Vector3::zeros(),
            normal: Matrix3::identity(),
        };
        transform.update_matrices();
        transform
    }

    pub fn translation(&self) -> Vector3<f64> {
        self.isometry.translation.vector
    }

    pub fn rotation(&self) -> UnitQuaternion<f64> {
        self.isometry.rotation
    }

    pub fn scale(&self) -> Vector3<f64> {
        self.scale
    }

    pub fn set_translation(&mut self, translation: Vector3<f64>) {
        self.isometry.translation = Translation3::from(translation);
        self.update_matrices();
    }

    pub fn set_rotation(&mut self, rotation: UnitQuaternion<f64>) {
        self.isometry.rotation = rotation;
        self.update_matrices();
    }

    /// Sets the rotation from Euler angles in radians: roll about X, pitch about Y,
    /// yaw about Z (applied in that order).
    pub fn set_rotation_euler(&mut self, roll: f64, pitch: f64, yaw: f64) {
        self.set_rotation(UnitQuaternion::from_euler_angles(roll, pitch, yaw));
    }

    pub fn set_scale(&mut self, scale: Vector3<f64>) {
        self.scale = scale;
        self.update_matrices();
    }

    fn update_matrices(&mut self) {
        let rotation = self.isometry.rotation.to_rotation_matrix().into_inner();
        let scale = Matrix3::from_diagonal(&self.scale);
        let inverse_scale = Matrix3::from_diagonal(&self.scale.map(|s| 1.0 / s));

        self.forward = rotation * scale;
        self.inverse = inverse_scale * rotation.transpose(); // a rotation's inverse is its transpose
        self.inverse_translation = -(self.inverse * self.translation());
        self.normal = rotation * inverse_scale;
    }

    /// Transforms a world-space ray into local space, returning (origin, unit direction).
    pub fn ray_to_local(&self, ray: &Ray) -> (Vector3<f64>, Vector3<f64>) {
        let p = self.inverse * ray.point1 + self.inverse_translation;
        let v = (self.inverse * ray.lab).normalize();
        (p, v)
    }

    /// `ray_to_local` as a Ray, for passing on to objects nested inside this transform.
    pub fn ray_to_local_ray(&self, ray: &Ray) -> Ray {
        let (p, v) = self.ray_to_local(ray);
        Ray::new(p, p + v)
    }

    /// World-space box around a local-space box: transform its corners and re-fit.
    /// (Rotation makes this looser than the shape, but it is always a valid bound.)
    pub fn bounds_to_world(&self, local_bounds: &Aabb) -> Aabb {
        Aabb::from_points(local_bounds.corners().iter().map(|c| self.point_to_world(c)))
    }

    /// Converts a distance along the local unit direction `local_dir` (from `ray_to_local`)
    /// into the corresponding distance in the parent space. Rotation and translation don't
    /// change lengths, so only the scale matters.
    pub fn distance_to_world(&self, local_dir: &Vector3<f64>, local_dist: f64) -> f64 {
        local_dist * local_dir.component_mul(&self.scale).norm()
    }

    pub fn point_to_world(&self, local_point: &Vector3<f64>) -> Vector3<f64> {
        self.forward * local_point + self.translation()
    }

    /// Normals transform by the inverse transpose: divide by scale, then rotate.
    pub fn normal_to_world(&self, local_normal: &Vector3<f64>) -> Vector3<f64> {
        (self.normal * local_normal).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Point3;

    // The precomputed matrices must match transforming step by step with the quaternion.
    fn assert_matches_isometry(t: &Transform) {
        let iso = t.isometry;
        let local = Vector3::new(0.2, -0.9, 0.4);

        let expected_world = iso.transform_point(&Point3::from(local.component_mul(&t.scale))).coords;
        assert!((t.point_to_world(&local) - expected_world).norm() < 1e-12);

        let ray = Ray::new(expected_world, expected_world + Vector3::new(0.3, 0.1, -0.8));
        let (p, v) = t.ray_to_local(&ray);
        assert!((p - local).norm() < 1e-12);
        let expected_dir = iso.inverse_transform_vector(&ray.lab).component_div(&t.scale).normalize();
        assert!((v - expected_dir).norm() < 1e-12);

        let n = Vector3::new(0.0, 0.6, 0.8);
        let expected_normal = iso.transform_vector(&n.component_div(&t.scale)).normalize();
        assert!((t.normal_to_world(&n) - expected_normal).norm() < 1e-12);
    }

    #[test]
    fn matrices_match_isometry_and_scale() {
        let mut t = Transform::new(Vector3::new(1.0, -2.0, 3.0), Vector3::new(0.5, 2.0, 1.5));
        assert_matches_isometry(&t);

        t.set_rotation_euler(0.3, -0.7, 1.1);
        assert_matches_isometry(&t);

        // Every setter must keep the matrices in sync.
        t.set_translation(Vector3::new(-4.0, 0.0, 2.0));
        assert_matches_isometry(&t);
        t.set_scale(Vector3::new(1.0, 3.0, 0.25));
        assert_matches_isometry(&t);
    }
}
