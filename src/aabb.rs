use nalgebra::Vector3;
use crate::ray::Ray;

// Extra margin on every side, so flat shapes (a plane has zero thickness) still have a hittable box.
const PADDING: f64 = 1e-6;

/// Axis-aligned bounding box, used to skip expensive intersection tests for rays that miss it.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vector3<f64>,
    pub max: Vector3<f64>,
}

impl Aabb {
    pub fn new(min: Vector3<f64>, max: Vector3<f64>) -> Self {
        Aabb { min, max }
    }

    /// Contains nothing; the identity for `union`.
    pub fn empty() -> Self {
        Aabb {
            min: Vector3::repeat(f64::INFINITY),
            max: Vector3::repeat(f64::NEG_INFINITY),
        }
    }

    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.inf(&other.min),
            max: self.max.sup(&other.max),
        }
    }

    /// The 8 corner points.
    pub fn corners(&self) -> [Vector3<f64>; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vector3::new(a.x, a.y, a.z), Vector3::new(b.x, a.y, a.z),
            Vector3::new(a.x, b.y, a.z), Vector3::new(b.x, b.y, a.z),
            Vector3::new(a.x, a.y, b.z), Vector3::new(b.x, a.y, b.z),
            Vector3::new(a.x, b.y, b.z), Vector3::new(b.x, b.y, b.z),
        ]
    }

    /// Smallest box containing all `points`.
    pub fn from_points(points: impl IntoIterator<Item = Vector3<f64>>) -> Self {
        points.into_iter().fold(Aabb::empty(), |bounds, p| Aabb {
            min: bounds.min.inf(&p),
            max: bounds.max.sup(&p),
        })
    }

    /// Does the ray (starting at point1, travelling along lab) pass through the box?
    pub fn hit(&self, ray: &Ray) -> bool {
        self.entry_distance(ray).is_some()
    }

    /// If the ray passes through the box, how far along it the box starts (0.0 if the ray
    /// starts inside), in units of `ray.lab`'s length; pass a unit-length `lab` for distances.
    pub fn entry_distance(&self, ray: &Ray) -> Option<f64> {
        self.entry_distance_inv(&ray.point1, &inverse_direction(&ray.lab))
    }

    /// `entry_distance` for a ray given as origin and `inverse_direction(lab)`. This is the hot
    /// path: callers testing many boxes against one ray compute the inverse once, so each box
    /// costs multiplies and comparisons instead of six divisions.
    #[inline]
    pub fn entry_distance_inv(&self, origin: &Vector3<f64>, inv_dir: &Vector3<f64>) -> Option<f64> {
        // Slab method, as in Cuboid::test_intersection. Starting t_near at 0 ignores the part
        // of the ray behind its origin. Plain comparisons are used instead of f64::min/max,
        // which are slower because of their NaN handling.
        //
        // A direction component of 0 gives an infinite inverse, so that axis's t values are
        // ±infinity: the slab then either covers the whole ray or none of it, as it should.
        // (If the origin lies exactly on a padded face, 0 * infinity is NaN; every comparison
        // with NaN is false, so that axis is ignored: at worst an object is tested needlessly,
        // never skipped wrongly.)
        let mut t_near = 0.0_f64;
        let mut t_far = f64::INFINITY;
        for i in 0..3 {
            let t1 = (self.min[i] - PADDING - origin[i]) * inv_dir[i];
            let t2 = (self.max[i] + PADDING - origin[i]) * inv_dir[i];
            let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
            if lo > t_near {
                t_near = lo;
            }
            if hi < t_far {
                t_far = hi;
            }
        }
        if t_near <= t_far { Some(t_near) } else { None }
    }
}

/// Component-wise 1 / direction, for `Aabb::entry_distance_inv`.
pub fn inverse_direction(dir: &Vector3<f64>) -> Vector3<f64> {
    dir.map(|d| 1.0 / d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_box() -> Aabb {
        Aabb::new(Vector3::repeat(-1.0), Vector3::repeat(1.0))
    }

    fn ray(origin: [f64; 3], dir: [f64; 3]) -> Ray {
        let o = Vector3::from(origin);
        Ray::new(o, o + Vector3::from(dir))
    }

    #[test]
    fn entry_distance_cases() {
        let b = unit_box();
        // Straight at the box from 5 away: enters at distance 4 (minus padding).
        let d = b.entry_distance(&ray([0.0, -5.0, 0.0], [0.0, 1.0, 0.0])).unwrap();
        assert!((d - 4.0).abs() < 1e-5);
        // Starting inside: 0.
        assert_eq!(b.entry_distance(&ray([0.0, 0.0, 0.0], [0.3, 0.4, 0.5])), Some(0.0));
        // Box behind the ray.
        assert_eq!(b.entry_distance(&ray([0.0, 5.0, 0.0], [0.0, 1.0, 0.0])), None);
        // Parallel to an axis (zero direction components) and outside the slab: miss.
        assert_eq!(b.entry_distance(&ray([0.0, -5.0, 2.0], [0.0, 1.0, 0.0])), None);
        // Parallel to an axis and inside the slab: hit.
        assert!(b.entry_distance(&ray([0.5, -5.0, 0.5], [0.0, 1.0, 0.0])).is_some());
        // Diagonal miss past a corner.
        assert_eq!(b.entry_distance(&ray([3.0, -5.0, 0.0], [0.0, 1.0, 0.0])), None);
        // Flat box (a plane's bounds), hit edge-on from above.
        let flat = Aabb::new(Vector3::new(-1.0, -1.0, 0.0), Vector3::new(1.0, 1.0, 0.0));
        assert!(flat.entry_distance(&ray([0.2, 0.3, 5.0], [0.0, 0.0, -1.0])).is_some());
    }
}
