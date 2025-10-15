use crate::aabb::Aabb;
use crate::object_base::{closest_hit, Intersect, IntersectionResult};
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
    pub shapes: Vec<Box<dyn Intersect>>,
    /// Box around all sub-shapes, in the composite's local space.
    pub local_bounds: Aabb,
    pub use_bounding_box: bool,
}

impl Composite {
    pub fn new(transform: Transform) -> Self {
        Composite {
            transform,
            shapes: Vec::new(),
            local_bounds: Aabb::empty(),
            use_bounding_box: true,
        }
    }

    /// Adds a sub-shape (in the composite's local coordinates) and grows the bounding box to fit.
    pub fn add_shape(&mut self, shape: impl Intersect + 'static) {
        self.local_bounds = self.local_bounds.union(&shape.bounds());
        self.shapes.push(Box::new(shape));
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
        let hit = closest_hit(&self.shapes, &local_ray)?;

        Some(IntersectionResult {
            intersection: self.transform.point_to_world(&hit.intersection),
            local_normal: self.transform.normal_to_world(&hit.local_normal),
            ..hit
        })
    }

    fn bounds(&self) -> Aabb {
        self.transform.bounds_to_world(&self.local_bounds)
    }
}
