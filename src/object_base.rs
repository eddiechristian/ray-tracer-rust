use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::ray::Ray;
use crate::transform::Transform;

// Minimum ray parameter t accepted as a hit, to avoid self-intersection from rounding error.
pub const EPSILON: f64 = 1e-9;

#[derive(Clone, Copy)]
pub struct IntersectionResult<'a> {
    pub intersection: Vector3<f64>,
    pub local_normal: Vector3<f64>,
    /// Surface coordinates for texture lookup; the mapping is defined by each shape.
    pub uv: Vector2<f64>,
    /// Material of the shape that was hit. For a composite, this is the sub-shape's material.
    pub material: &'a Material,
}

// Send + Sync so the scene can be shared by the parallel render threads.
pub trait Intersect: Send + Sync {
    /// Full hit details: world point, normal, uv and material.
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>>;
    /// Distance from `ray.point1` to the nearest hit, if it is closer than `max_dist`.
    /// Cheaper than `test_intersection` (no uv, normal or world-space point), so it is used
    /// to pick the closest object and for shadow rays; must agree with `test_intersection`.
    fn hit_distance(&self, ray: &Ray, max_dist: f64) -> Option<f64>;
    /// Bounding box in the parent's coordinate space (world space for top-level objects).
    fn bounds(&self) -> Aabb;

    fn transform(&self) -> &Transform;
    /// For moving the object. Its transform's setters keep the transform itself consistent;
    /// afterwards call `refresh_bounds` on the list holding it, before the next render.
    fn transform_mut(&mut self) -> &mut Transform;

    /// Recompute any cached bounding boxes inside this object (composites cache their sub-shapes').
    fn refresh_bounds(&mut self) {}

    /// Sub-shapes, for objects that have them (composites), so they can be moved individually.
    fn children_mut(&mut self) -> Option<&mut ObjectList> {
        None
    }
}

/// A list of objects with each one's bounding box cached, so rays can skip objects whose
/// box they miss (or whose box starts beyond the closest hit found so far).
/// Used for the scene's objects and for a composite's sub-shapes.
///
/// The cached boxes go stale when objects move: after changing objects through `get_mut`,
/// call `refresh_bounds` before rendering. Debug builds assert this.
#[derive(Default)]
pub struct ObjectList {
    objects: Vec<Box<dyn Intersect>>,
    bounds: Vec<Aabb>,
    /// Set by `get_mut`, cleared by `refresh_bounds`.
    bounds_stale: bool,
}

impl ObjectList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an object and returns its index, for `get` / `get_mut` later.
    pub fn push(&mut self, object: Box<dyn Intersect>) -> usize {
        self.bounds.push(object.bounds());
        self.objects.push(object);
        self.objects.len() - 1
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn get(&self, index: usize) -> Option<&dyn Intersect> {
        self.objects.get(index).map(|object| object.as_ref())
    }

    /// For moving an object. Marks the cached bounds as stale until `refresh_bounds` is called.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut (dyn Intersect + 'static)> {
        self.bounds_stale = true;
        self.objects.get_mut(index).map(|object| object.as_mut())
    }

    /// Recompute every object's cached bounding box (and those inside composites).
    /// Call once per frame after moving objects, before rendering.
    pub fn refresh_bounds(&mut self) {
        for (object, bounds) in self.objects.iter_mut().zip(&mut self.bounds) {
            object.refresh_bounds();
            *bounds = object.bounds();
        }
        self.bounds_stale = false;
    }

    /// Box around every object in the list.
    pub fn bounds(&self) -> Aabb {
        self.bounds.iter().fold(Aabb::empty(), |all, b| all.union(b))
    }

    /// Index and distance of the nearest object hit closer than `max_dist`.
    fn nearest(&self, ray: &Ray, max_dist: f64) -> Option<(usize, f64)> {
        debug_assert!(!self.bounds_stale, "objects moved: call ObjectList::refresh_bounds() before rendering");
        // Unit-length direction, so box entry and hit distances are in the same units.
        let ray = Ray::new(ray.point1, ray.point1 + ray.lab.normalize());
        let mut nearest = None;
        let mut nearest_dist = max_dist;
        for (i, (object, bounds)) in self.objects.iter().zip(&self.bounds).enumerate() {
            match bounds.entry_distance(&ray) {
                Some(entry) if entry < nearest_dist => {}
                _ => continue, // Missed the box, or it starts beyond the nearest hit so far.
            }
            if let Some(dist) = object.hit_distance(&ray, nearest_dist) {
                nearest = Some(i);
                nearest_dist = dist;
            }
        }
        nearest.map(|i| (i, nearest_dist))
    }

    /// Full details of the closest hit. Only the winning object computes them.
    pub fn closest_hit(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        let (i, _) = self.nearest(ray, f64::INFINITY)?;
        self.objects[i].test_intersection(ray)
    }

    pub fn hit_distance(&self, ray: &Ray, max_dist: f64) -> Option<f64> {
        self.nearest(ray, max_dist).map(|(_, dist)| dist)
    }

    /// Is anything hit closer than `max_dist`? Stops at the first object found (shadow rays).
    pub fn occluded(&self, ray: &Ray, max_dist: f64) -> bool {
        debug_assert!(!self.bounds_stale, "objects moved: call ObjectList::refresh_bounds() before rendering");
        let ray = Ray::new(ray.point1, ray.point1 + ray.lab.normalize());
        self.objects.iter().zip(&self.bounds).any(|(object, bounds)| {
            bounds.entry_distance(&ray).is_some_and(|entry| entry < max_dist)
                && object.hit_distance(&ray, max_dist).is_some()
        })
    }
}

impl From<Vec<Box<dyn Intersect>>> for ObjectList {
    fn from(objects: Vec<Box<dyn Intersect>>) -> Self {
        let mut list = ObjectList::new();
        for object in objects {
            list.push(object);
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composite::Composite;
    use crate::material::Material;
    use crate::ray::Color;
    use crate::sphere::Sphere;
    use nalgebra::Vector3;

    fn sphere_at(x: f64) -> Box<dyn Intersect> {
        Box::new(Sphere::new(
            Transform::new(Vector3::new(x, 0.0, 0.0), Vector3::repeat(1.0)),
            Material::flat(Color::new(1.0, 0.0, 0.0)),
        ))
    }

    /// Ray from (x, -10, 0) toward +y.
    fn ray_at(x: f64) -> Ray {
        Ray::new(Vector3::new(x, -10.0, 0.0), Vector3::new(x, -9.0, 0.0))
    }

    #[test]
    fn moved_object_is_hit_at_its_new_position_after_refresh() {
        let mut list = ObjectList::new();
        let index = list.push(sphere_at(0.0));
        assert!(list.closest_hit(&ray_at(0.0)).is_some());

        list.get_mut(index).unwrap().transform_mut().set_translation(Vector3::new(5.0, 0.0, 0.0));
        list.refresh_bounds();

        assert!(list.closest_hit(&ray_at(0.0)).is_none(), "old position should be empty");
        let hit = list.closest_hit(&ray_at(5.0)).expect("new position should be hit");
        assert!((hit.intersection - Vector3::new(5.0, -1.0, 0.0)).norm() < 1e-9);
        assert!(list.occluded(&ray_at(5.0), 100.0));
    }

    #[test]
    fn moved_composite_child_is_hit_after_refresh() {
        let mut composite = Composite::new(Transform::new(Vector3::zeros(), Vector3::repeat(1.0)));
        composite.shapes.push(sphere_at(0.0));
        composite.local_bounds = composite.shapes.bounds();

        let mut list = ObjectList::new();
        let index = list.push(Box::new(composite));

        // Move the sub-shape, not the composite: both the composite's box and the list's box must grow.
        let children = list.get_mut(index).unwrap().children_mut().unwrap();
        children.get_mut(0).unwrap().transform_mut().set_translation(Vector3::new(-6.0, 0.0, 0.0));
        list.refresh_bounds();

        assert!(list.closest_hit(&ray_at(0.0)).is_none());
        assert!(list.closest_hit(&ray_at(-6.0)).is_some());
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "refresh_bounds")]
    fn rendering_after_moving_without_refresh_panics_in_debug_builds() {
        let mut list = ObjectList::new();
        let index = list.push(sphere_at(0.0));
        list.get_mut(index).unwrap().transform_mut().set_translation(Vector3::new(5.0, 0.0, 0.0));
        list.closest_hit(&ray_at(5.0));
    }
}
