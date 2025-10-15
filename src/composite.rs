use crate::aabb::Aabb;
use crate::object_base::{Intersect, IntersectionResult, ObjectList};
use crate::ray::Ray;
use crate::transform::Transform;

// A group of shapes treated as one object (the C++ CompositeBase). It has no surface of its
// own: sub-shapes are positioned in the composite's local space, and the composite's
// transform moves, rotates and scales them all together. Composites can be nested.
//
// Rays are first tested against a bounding box around all the sub-shapes, so a ray that
// misses the group costs one box test instead of one test per sub-shape.
pub struct Composite {
    pub transform: Transform,
    pub shapes: ObjectList,
    /// Box around all sub-shapes, in the composite's local space.
    pub local_bounds: Aabb,
    pub use_bounding_box: bool,
}

impl Composite {
    pub fn new(transform: Transform) -> Self {
        Composite {
            transform,
            shapes: ObjectList::new(),
            local_bounds: Aabb::empty(),
            use_bounding_box: true,
        }
    }

    /// Adds a sub-shape (in the composite's local coordinates) and grows the bounding box to fit.
    pub fn add_shape(&mut self, shape: impl Intersect + 'static) {
        self.shapes.push(Box::new(shape));
        self.local_bounds = self.shapes.bounds();
    }

    /// Builder form of `add_shape`.
    pub fn with_shape(mut self, shape: impl Intersect + 'static) -> Self {
        self.add_shape(shape);
        self
    }
}

impl Intersect for Composite {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        let local_ray = self.transform.ray_to_local_ray(ray);

        if self.use_bounding_box && !self.local_bounds.hit(&local_ray) {
            return None;
        }

        // Closest sub-shape hit, in local space. Distances along one ray keep their order
        // under the transform, so the closest local hit is also the closest world hit.
        let hit = self.shapes.closest_hit(&local_ray)?;

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&hit.intersection),
            local_normal: self.transform.normal_to_world(&hit.local_normal),
            ..hit
        })
    }

    fn hit_distance(&self, ray: &Ray, max_dist: f64) -> Option<f64> {
        let (p, v) = self.transform.ray_to_local(ray);
        let local_ray = Ray::new(p, p + v);

        if self.use_bounding_box && !self.local_bounds.hit(&local_ray) {
            return None;
        }

        // Search in local units, then convert the result back.
        let to_world = self.transform.distance_to_world(&v, 1.0);
        let local_dist = self.shapes.hit_distance(&local_ray, max_dist / to_world)?;
        Some(local_dist * to_world)
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

    /// Sub-shapes may have moved: refresh their boxes, then the box around all of them.
    fn refresh_bounds(&mut self) {
        self.shapes.refresh_bounds();
        self.local_bounds = self.shapes.bounds();
    }

    fn children_mut(&mut self) -> Option<&mut ObjectList> {
        Some(&mut self.shapes)
    }
}
