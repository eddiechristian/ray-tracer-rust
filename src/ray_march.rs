use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::object_base::{Intersect, IntersectionResult};
use crate::ray::Ray;
use crate::transform::Transform;

// Ray-marched shape, as in the C++ qbRT::RM::RayMarchBase, defined by a signed distance
// function (see `sdf.rs`) in local space. Differences from the C++:
// - the march is clipped to the local bounding box (the C++ runs until the distance tops 1e6);
// - the hit tolerance is 1e-4, not 1e-12, which in practice means a ray can run out of steps;
// - the normal takes 4 SDF samples (iq's tetrahedron), not 6;
// - the SDF also returns a material index, so one shape can have several materials.

/// The shape's SDF: local point → (signed distance, index into `materials`).
pub type Sdf = Box<dyn Fn(&Vector3<f64>) -> (f64, usize) + Send + Sync>;

/// A point closer to the surface than this (in local units) counts as on it.
const HIT_EPSILON: f64 = 1e-4;
/// Rays starting on the surface (shadow and reflection rays) step until they're this far off
/// it before hits count; otherwise they'd hit the surface they started on.
const ESCAPE_DISTANCE: f64 = 2.0 * HIT_EPSILON;
/// Give up after this many steps (grazing rays creep along the surface).
const MAX_STEPS: usize = 256;
/// Offset for the gradient samples that give the normal.
const NORMAL_DELTA: f64 = 1e-5;

pub struct RayMarched {
    pub transform: Transform,
    pub materials: Vec<Material>,
    /// Must contain the whole surface; marching happens only inside it.
    local_bounds: Aabb,
    sdf: Sdf,
    /// Fraction of the SDF distance to step. 1.0 for exact SDFs; lower (e.g. 0.8) when the SDF
    /// can overestimate (smooth blends, bends), so the march doesn't step through the surface.
    step_scale: f64,
}

impl RayMarched {
    /// Keep `transform`'s scale uniform: scaling axes differently distorts SDF distances.
    pub fn new(transform: Transform, local_bounds: Aabb, materials: Vec<Material>, sdf: Sdf) -> Self {
        debug_assert!(!materials.is_empty(), "a RayMarched shape needs at least one material");
        RayMarched { transform, materials, local_bounds, sdf, step_scale: 1.0 }
    }

    pub fn with_step_scale(mut self, step_scale: f64) -> Self {
        self.step_scale = step_scale;
        self
    }

    /// Local-space distance along unit direction `v` from `p` to the first hit closer than
    /// `max_t`, and the material index there.
    fn local_hit(&self, p: &Vector3<f64>, v: &Vector3<f64>, max_t: f64) -> Option<(f64, usize)> {
        let (entry, exit) = self.local_bounds.span(p, v)?;
        let exit = exit.min(max_t);
        let mut t = entry;
        let (mut d, mut id) = (self.sdf)(&(p + v * t));
        let mut steps = 0;

        // Starting on the surface (shadow and reflection rays, which start inside the box): move
        // off it first. Going into the shape instead means the ray is blocked right away (e.g. a
        // shadow ray from a point facing away from the light). Rays from outside the box skip
        // this: where the surface touches the box, they'd otherwise step into it at entry.
        if entry == 0.0 && d < ESCAPE_DISTANCE {
            while d < ESCAPE_DISTANCE {
                if d < 0.0 && t > entry {
                    return (t <= exit).then_some((t, id));
                }
                t += d.abs().max(HIT_EPSILON);
                steps += 1;
                if t > exit || steps >= MAX_STEPS {
                    return None;
                }
                (d, id) = (self.sdf)(&(p + v * t));
            }
        }

        // Sphere tracing: nothing is closer than d, so it's safe to step that far.
        while steps < MAX_STEPS {
            if d < HIT_EPSILON {
                return Some((t, id));
            }
            t += d * self.step_scale;
            if t > exit {
                return None;
            }
            (d, id) = (self.sdf)(&(p + v * t));
            steps += 1;
        }
        None
    }

    /// Unit SDF gradient at `p`, from 4 samples at the corners of a tetrahedron (iq's method).
    fn local_normal(&self, p: &Vector3<f64>) -> Vector3<f64> {
        const CORNERS: [Vector3<f64>; 4] = [
            Vector3::new(1.0, -1.0, -1.0),
            Vector3::new(-1.0, -1.0, 1.0),
            Vector3::new(-1.0, 1.0, -1.0),
            Vector3::new(1.0, 1.0, 1.0),
        ];
        CORNERS
            .iter()
            .map(|k| k * (self.sdf)(&(p + k * NORMAL_DELTA)).0)
            .sum::<Vector3<f64>>()
            .normalize()
    }
}

impl Intersect for RayMarched {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        let (p, v) = self.transform.ray_to_local(ray);
        let (t, id) = self.local_hit(&p, &v, f64::INFINITY)?;
        let local_point = p + v * t;
        let normal = self.local_normal(&local_point);

        // Like the cuboid's faces: the two local coordinates across the dominant normal axis.
        let n = normal.abs();
        let uv = if n.x >= n.y && n.x >= n.z {
            Vector2::new(local_point.y, local_point.z)
        } else if n.y >= n.z {
            Vector2::new(local_point.x, local_point.z)
        } else {
            Vector2::new(local_point.x, local_point.y)
        };

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&local_point),
            local_normal: self.transform.normal_to_world(&normal),
            uv,
            material: &self.materials[id],
        })
    }

    fn hit_distance(&self, ray: &Ray, max_dist: f64) -> Option<f64> {
        let (p, v) = self.transform.ray_to_local(ray);
        // World distance per local unit along v, to stop the march at max_dist.
        let world_per_local = self.transform.distance_to_world(&v, 1.0);
        let (t, _) = self.local_hit(&p, &v, max_dist / world_per_local)?;
        let dist = t * world_per_local;
        (dist < max_dist).then_some(dist)
    }

    fn bounds(&self) -> Aabb {
        self.transform.bounds_to_world(&self.local_bounds)
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ray::Color;
    use crate::sdf;
    use crate::sphere::Sphere;
    use nalgebra::UnitQuaternion;

    fn transform() -> Transform {
        let mut t = Transform::new(Vector3::new(1.0, 2.0, -0.5), Vector3::repeat(1.7));
        t.set_rotation(UnitQuaternion::from_euler_angles(0.3, -0.7, 1.1));
        t
    }

    fn marched_sphere() -> RayMarched {
        RayMarched::new(
            transform(),
            Aabb::new(Vector3::repeat(-1.0), Vector3::repeat(1.0)),
            vec![Material::flat(Color::new(1.0, 1.0, 1.0))],
            Box::new(|p| (sdf::sphere(p, 1.0), 0)),
        )
    }

    fn ray(from: Vector3<f64>, toward: Vector3<f64>) -> Ray {
        Ray::new(from, toward)
    }

    #[test]
    fn marched_sphere_matches_analytic_sphere() {
        let marched = marched_sphere();
        let exact = Sphere::new(transform(), Material::flat(Color::new(1.0, 1.0, 1.0)));
        let centre = transform().translation();

        // Rays from all around, aimed at and near the centre (one grazing, one missing).
        let mut hits = 0;
        for (i, offset) in [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.5, -0.3, 0.2),
            Vector3::new(-1.0, 0.8, 0.4),
            Vector3::new(0.0, 1.69, 0.0),
            Vector3::new(3.0, 0.0, 0.0),
        ].iter().enumerate() {
            for from in [Vector3::new(8.0, 0.0, 0.0), Vector3::new(-2.0, -7.0, 3.0), Vector3::new(1.0, 5.0, -6.0)] {
                let r = ray(centre + from, centre + offset);
                let (a, b) = (marched.test_intersection(&r), exact.test_intersection(&r));
                assert_eq!(a.is_some(), b.is_some(), "ray {i} from {from:?}");
                if let (Some(a), Some(b)) = (a, b) {
                    hits += 1;
                    assert!((a.intersection - b.intersection).norm() < 1e-3, "point, ray {i}");
                    assert!((a.local_normal - b.local_normal).norm() < 1e-3, "normal, ray {i}");
                    let da = marched.hit_distance(&r, f64::INFINITY).unwrap();
                    let db = exact.hit_distance(&r, f64::INFINITY).unwrap();
                    assert!((da - db).abs() < 1e-3, "distance, ray {i}: {da} vs {db}");
                }
            }
        }
        assert!(hits >= 9, "only {hits} hits");
    }

    #[test]
    fn ray_leaving_the_surface_is_not_blocked_by_it() {
        let marched = marched_sphere();
        let centre = transform().translation();
        let hit = marched.test_intersection(&ray(centre + Vector3::new(9.0, 1.0, 0.5), centre)).unwrap();
        let start = hit.intersection + hit.local_normal * 1e-6; // as PointLight's SHADOW_BIAS

        // Straight out, and nearly along the surface (a grazing shadow ray).
        let tangent = hit.local_normal.cross(&Vector3::z()).normalize();
        for dir in [hit.local_normal, (tangent + hit.local_normal * 0.05).normalize()] {
            assert_eq!(marched.hit_distance(&ray(start, start + dir), f64::INFINITY), None, "{dir:?}");
        }
        // Back into the sphere: blocked by its far side (or immediately).
        assert!(marched.hit_distance(&ray(start, start - hit.local_normal), f64::INFINITY).is_some());
    }

    #[test]
    fn surface_touching_its_bounds_is_hit_from_outside_and_lit() {
        // A box that exactly fills its bounding box: rays enter the bounds on the surface.
        let mut t = Transform::new(Vector3::new(2.0, -0.3, 0.3), Vector3::repeat(0.09));
        t.set_rotation(UnitQuaternion::from_euler_angles(0.4, 0.3, 0.6));
        let shape = RayMarched::new(
            t,
            Aabb::new(Vector3::repeat(-1.0), Vector3::repeat(1.0)),
            vec![Material::flat(Color::new(1.0, 1.0, 1.0))],
            Box::new(|p| (sdf::round_box(p, &Vector3::repeat(1.0), 0.25), 0)),
        );
        for from in [Vector3::new(0.0, -5.5, 3.5), Vector3::new(-4.0, 0.0, 0.0), Vector3::new(2.0, 3.0, -2.0)] {
            let hit = shape.test_intersection(&ray(from, shape.transform.translation())).unwrap();
            // The hit is on or just outside the surface, never inside it.
            let (local, _) = shape.transform.ray_to_local(&ray(hit.intersection, hit.intersection + Vector3::x()));
            let d = (shape.sdf)(&local).0;
            assert!((0.0..HIT_EPSILON).contains(&d), "from {from:?}: d = {d}");
            // So a shadow ray back toward where the ray came from isn't blocked by the shape.
            let start = hit.intersection + hit.local_normal * 1e-6;
            assert_eq!(shape.hit_distance(&ray(start, from), (from - start).norm()), None, "from {from:?}");
        }
    }

    #[test]
    fn misses_and_max_dist() {
        let marched = marched_sphere();
        let centre = transform().translation();
        // Pointing away from the shape.
        assert!(marched.test_intersection(&ray(centre + Vector3::new(5.0, 0.0, 0.0), centre + Vector3::new(9.0, 0.0, 0.0))).is_none());
        // Passing beside it (sphere radius 1.7): misses the box or reaches its far side.
        assert!(marched.test_intersection(&ray(centre + Vector3::new(5.0, 2.5, 0.0), centre + Vector3::new(0.0, 2.5, 0.0))).is_none());
        // A hit 3.3 away doesn't count with max_dist 3.
        let r = ray(centre + Vector3::new(5.0, 0.0, 0.0), centre);
        assert!((marched.hit_distance(&r, 10.0).unwrap() - 3.3).abs() < 1e-3);
        assert_eq!(marched.hit_distance(&r, 3.0), None);
    }

    #[test]
    fn material_index_comes_from_the_sdf() {
        // Two spheres side by side: material 0 on the left (-x), 1 on the right.
        let shape = RayMarched::new(
            Transform::new(Vector3::zeros(), Vector3::repeat(1.0)),
            Aabb::new(Vector3::new(-2.0, -1.0, -1.0), Vector3::new(2.0, 1.0, 1.0)),
            vec![Material::flat(Color::new(1.0, 0.0, 0.0)), Material::flat(Color::new(0.0, 0.0, 1.0))],
            Box::new(|p| sdf::union_id(
                (sdf::sphere(&(p - Vector3::new(-1.0, 0.0, 0.0)), 1.0), 0),
                (sdf::sphere(&(p - Vector3::new(1.0, 0.0, 0.0)), 1.0), 1),
            )),
        );
        let left = shape.test_intersection(&ray(Vector3::new(-1.0, -5.0, 0.0), Vector3::new(-1.0, 0.0, 0.0))).unwrap();
        let right = shape.test_intersection(&ray(Vector3::new(1.0, -5.0, 0.0), Vector3::new(1.0, 0.0, 0.0))).unwrap();
        assert!(std::ptr::eq(left.material, &shape.materials[0]));
        assert!(std::ptr::eq(right.material, &shape.materials[1]));
    }
}

