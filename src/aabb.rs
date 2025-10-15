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
        // Slab method, as in Cuboid::test_intersection.
        let mut t_near = f64::NEG_INFINITY;
        let mut t_far = f64::INFINITY;
        for i in 0..3 {
            let (lo, hi) = (self.min[i] - PADDING, self.max[i] + PADDING);
            if ray.lab[i] == 0.0 {
                if ray.point1[i] < lo || ray.point1[i] > hi {
                    return false;
                }
            } else {
                let t1 = (lo - ray.point1[i]) / ray.lab[i];
                let t2 = (hi - ray.point1[i]) / ray.lab[i];
                t_near = t_near.max(t1.min(t2));
                t_far = t_far.min(t1.max(t2));
            }
        }
        t_near <= t_far && t_far >= 0.0
    }
}
