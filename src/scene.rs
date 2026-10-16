use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI, TAU};

use crate::sphere::Sphere;
use crate::plane::Plane;
use crate::cuboid::Cuboid;
use crate::cylinder::Cylinder;
use crate::cone::Cone;
use crate::composite::Composite;
use crate::camera::Camera;
use crate::object_base::{Intersect, IntersectionResult, ObjectList};
use crate::material::Material;
use crate::texture::Checker;
use crate::light_base::Illuminate;
use crate::point_light::PointLight;
use crate::ambient_light::AmbientLight;
use crate::aabb::Aabb;
use crate::ray::{Color, Ray};
use crate::transform::Transform;
use nalgebra::{Point3, UnitQuaternion, Vector2, Vector3};
use rayon::prelude::*;

/// Minimal context that `Material::compute_color` needs for recursive tracing and lighting.
/// Implemented by both `Scene` and `SolarScene` so materials are scene-agnostic.
pub trait SceneContext: Sync {
    fn trace(&self, ray: &Ray, depth: u32, weight: f64) -> Color;
    fn scene_lights(&self) -> &[Box<dyn Illuminate>];
    fn scene_objects(&self) -> &ObjectList;
}

pub struct Scene {
    pub camera: Camera,
    pub objects: ObjectList,
    pub lights: Vec<Box<dyn Illuminate>>,
    pub x_size: u32,
    pub y_size: u32,
    pub wobble: Wobble,
    /// Axis-arrow frame that tracks the rim's floor-contact (pivot) point each frame.
    pub pivot_marker: usize,
    /// Whether the pivot axis frame is rendered.
    pub axes_visible: bool,
    /// Index of the orange checker cylinder that spins in place.
    pub spinning_cylinder: usize,
    /// Axis-arrow frame attached to the spinning cylinder.
    pub cylinder_axes: usize,
}

/// An object standing on the floor that rocks from side to side, tipping onto the edge of
/// its base (like a can nudged on a table) rather than sinking into the floor.
pub struct Wobble {
    /// Index of the object in `Scene::objects`. Its local origin must be the centre of its base.
    pub object: usize,
    /// Where the base centre sits when upright.
    pub rest_position: Vector3<f64>,
    /// Radius of the base, in world units: the rim it rocks on.
    pub base_radius: f64,
    /// Largest tilt, in radians.
    pub amplitude: f64,
    /// Rocks per second (one rock = over to one side, over to the other, and back).
    pub frequency: f64,
}

/// Radius of a spray can's body in its local space (see `spray_can`).
const SPRAY_CAN_RADIUS: f64 = 0.4;

/// A group of axis arrows whose visibility can be toggled at runtime.
/// Wraps a `Composite` and short-circuits both intersection methods when hidden.
struct AxesFrame {
    inner: Composite,
    visible: bool,
}

impl AxesFrame {
    fn new(transform: Transform) -> Self {
        Self { inner: Composite::new(transform), visible: true }
    }
    fn with_shape(mut self, shape: impl Intersect + 'static) -> Self {
        self.inner.add_shape(shape);
        self
    }
}

impl Intersect for AxesFrame {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        if self.visible { self.inner.test_intersection(ray) } else { None }
    }
    fn hit_distance(&self, ray: &Ray, max_dist: f64) -> Option<f64> {
        if self.visible { self.inner.hit_distance(ray, max_dist) } else { None }
    }
    fn bounds(&self) -> Aabb { if self.visible { self.inner.bounds() } else { Aabb::empty() } }
    fn transform(&self) -> &Transform                 { self.inner.transform() }
    fn transform_mut(&mut self) -> &mut Transform     { self.inner.transform_mut() }
    fn refresh_bounds(&mut self)                      { self.inner.refresh_bounds(); }
    fn children_mut(&mut self) -> Option<&mut ObjectList> { self.inner.children_mut() }
    fn set_visible(&mut self, v: bool)                { self.visible = v; }
}

impl Scene {

    pub fn new() -> Self {
        // The default camera sees roughly -3.5 <= x <= 3.5 and -2 <= z <= 2 at y = 0,
        // so the objects are small and sit in a row on a floor at z = -1.

        let floor = Transform::new(Vector3::new(0.0, 0.0, -1.0), Vector3::new(6.0, 6.0, 1.0));

        let sphere = Transform::new(Vector3::new(-1.8, 0.0, -0.55), Vector3::new(0.45, 0.45, 0.45));

        let mut cuboid = Transform::new(Vector3::new(-0.6, 0.0, -0.6), Vector3::new(0.4, 0.4, 0.4));
        cuboid.set_rotation_euler(0.0, 0.0, FRAC_PI_6); // turn about z to show two faces

        let cylinder = Transform::new(Vector3::new(0.6, 0.0, -0.55), Vector3::new(0.4, 0.4, 0.45));

        // Flip the cone (local apex at the origin, base at z = 1) so it stands on its base.
        let mut cone = Transform::new(Vector3::new(1.8, 0.0, -0.1), Vector3::new(0.45, 0.45, 0.9));
        cone.set_rotation_euler(PI, 0.0, 0.0);

        // Second set: rotated copies floating in a row above the first.
        let mut panel = Transform::new(Vector3::new(-1.9, 0.0, 0.75), Vector3::new(0.35, 0.35, 1.0));
        panel.set_rotation_euler(PI / 3.0, 0.0, FRAC_PI_6);

        // A rotated sphere looks unchanged, so squash it into an ellipsoid to show the rotation.
        let mut ellipsoid = Transform::new(Vector3::new(-0.95, 0.0, 0.75), Vector3::new(0.45, 0.2, 0.25));
        ellipsoid.set_rotation_euler(0.0, FRAC_PI_6, FRAC_PI_4);

        let mut tilted_cuboid = Transform::new(Vector3::new(0.0, 0.0, 0.75), Vector3::new(0.28, 0.28, 0.28));
        tilted_cuboid.set_rotation_euler(FRAC_PI_4, PI / 5.0, FRAC_PI_6);

        let mut tilted_cylinder = Transform::new(Vector3::new(0.95, 0.0, 0.75), Vector3::new(0.22, 0.22, 0.38));
        tilted_cylinder.set_rotation_euler(PI / 3.0, 0.0, FRAC_PI_6);

        // Lying on its side with the apex pointing left (local +z rotated toward +x), tipped forward.
        let mut tilted_cone = Transform::new(Vector3::new(1.55, 0.0, 0.75), Vector3::new(0.35, 0.35, 0.7));
        tilted_cone.set_rotation_euler(FRAC_PI_6, FRAC_PI_2, 0.0);

        // The can's origin is its base, so this leans the whole can left and toward the camera.
        let mut tilted_can = Transform::new(Vector3::new(3.1, 0.0, -0.4), Vector3::repeat(0.5));
        tilted_can.set_rotation_euler(0.3, -0.5, 0.0);

        let mut objects: Vec<Box<dyn Intersect>> = vec![
            // Reflective checkerboard floor, as in the C++ Ep10 scene.
            Box::new(Plane::new(floor, Material::new(
                Checker::new(Color::new(1.0, 1.0, 1.0), Color::new(0.2, 0.2, 0.2))
                    .with_transform(Vector2::zeros(), 0.0, Vector2::new(12.0, 12.0)),
            ).with_reflectivity(0.25))),
            // Glass: mostly refracted light (the floor seen through it, flipped), slight red tint.
            Box::new(Sphere::new(sphere, Material::flat(Color::new(1.0, 0.6, 0.6))
                .with_reflectivity(0.1)
                .with_specular(1.0, 64.0)
                .with_translucency(0.8, 1.5))),
            Box::new(Cuboid::new(cuboid, Material::new(
                Checker::new(Color::new(0.0, 1.0, 0.0), Color::new(0.0, 0.3, 0.0))
                    .with_transform(Vector2::zeros(), 0.0, Vector2::new(2.0, 2.0)),
            ))),
            Box::new(Cylinder::new(cylinder, Material::flat(Color::new(0.2, 0.4, 1.0))
                .with_reflectivity(0.3)
                .with_specular(1.0, 32.0))),
            Box::new(Cone::new(cone, Material::flat(Color::new(1.0, 0.8, 0.0))
                .with_specular(0.6, 16.0))),

            Box::new(Plane::new(panel, Material::new(
                Checker::new(Color::new(1.0, 1.0, 1.0), Color::new(0.0, 0.0, 0.0))
                    .with_transform(Vector2::zeros(), 0.0, Vector2::new(4.0, 4.0)),
            ))),
            Box::new(Sphere::new(ellipsoid, Material::flat(Color::new(1.0, 0.2, 0.8))
                .with_reflectivity(0.6)
                .with_specular(1.0, 32.0))),
            Box::new(Cuboid::new(tilted_cuboid, Material::flat(Color::new(0.0, 0.9, 0.9))
                .with_specular(0.5, 8.0))),
            // Checker on the side wraps around the axis: u is the angle, v the height.
            Box::new(Cylinder::new(tilted_cylinder, Material::new(
                Checker::new(Color::new(1.0, 0.5, 0.0), Color::new(0.3, 0.1, 0.0))
                    .with_transform(Vector2::zeros(), 0.0, Vector2::new(4.0, 2.0)),
            ))),
            Box::new(Cone::new(tilted_cone, Material::flat(Color::new(0.6, 0.3, 1.0))
                .with_specular(1.0, 32.0))),
        ];

        // Composites: one standing on the floor (it wobbles, see `animate`), one floating and
        // tilted as a whole.
        let can_position = Vector3::new(-3.0, 0.0, -1.0);
        let can_scale = 0.5;
        let wobble = Wobble {
            object: objects.len(),
            rest_position: can_position,
            base_radius: SPRAY_CAN_RADIUS * can_scale,
            amplitude: 15.0_f64.to_radians(),
            frequency: 0.2, // slowed for axis-arrow inspection; was 0.8
        };
        objects.push(Box::new(spray_can(
            Transform::new(can_position, Vector3::repeat(can_scale)),
            Color::new(0.9, 0.1, 0.1),
            0.0, // opaque
        )));
        objects.push(Box::new(spray_can(tilted_can, Color::new(0.1, 0.3, 0.9), 0.0)));

        // Axis-arrow frame for the spinning cylinder — same position, rotates with it.
        let cylinder_axes = objects.len();
        {
            let cyl_pos = Vector3::new(0.95, 0.0, 0.75);
            let mut x_t = Transform::new(Vector3::zeros(), Vector3::repeat(1.0));
            x_t.set_rotation_euler(0.0, FRAC_PI_2, 0.0);
            let mut y_t = Transform::new(Vector3::zeros(), Vector3::repeat(1.0));
            y_t.set_rotation_euler(-FRAC_PI_2, 0.0, 0.0);
            let z_t = Transform::new(Vector3::zeros(), Vector3::repeat(1.0));
            objects.push(Box::new(
                AxesFrame::new(Transform::new(cyl_pos, Vector3::repeat(0.5)))
                    .with_shape(arrow(x_t, Color::new(1.0, 0.1, 0.1)))
                    .with_shape(arrow(y_t, Color::new(0.1, 0.9, 0.1)))
                    .with_shape(arrow(z_t, Color::new(0.1, 0.4, 1.0))),
            ));
        }

        // Axis-arrow frame at the rim contact (pivot) point. Only the translation is updated
        // each frame; the rotation stays identity so the arrows always point along world axes.
        // Scale 0.5 matches the world size of the can's own axis arrows.
        // Raised 0.03 above the floor so X/Y arrow shafts aren't clipped by the floor plane.
        let pivot_marker = objects.len();
        {
            let mut x_t = Transform::new(Vector3::zeros(), Vector3::repeat(1.0));
            x_t.set_rotation_euler(0.0, FRAC_PI_2, 0.0);
            let mut y_t = Transform::new(Vector3::zeros(), Vector3::repeat(1.0));
            y_t.set_rotation_euler(-FRAC_PI_2, 0.0, 0.0);
            let z_t = Transform::new(Vector3::zeros(), Vector3::repeat(1.0));
            let pivot_pos = Vector3::new(can_position.x, can_position.y, can_position.z + 0.03);
            objects.push(Box::new(
                AxesFrame::new(Transform::new(pivot_pos, Vector3::repeat(0.5)))
                    .with_shape(arrow(x_t, Color::new(1.0, 0.1, 0.1)))
                    .with_shape(arrow(y_t, Color::new(0.1, 0.9, 0.1)))
                    .with_shape(arrow(z_t, Color::new(0.1, 0.4, 1.0))),
            ));
        }

        Scene {
            camera: Camera::default(),
            objects: ObjectList::from(objects),
            lights: vec![
                Box::new(PointLight::new(Point3::new(5.0, -10.0, 5.0), 1.0)),
                Box::new(AmbientLight::new(0.15)),
            ],
            x_size: 1280,
            y_size: 720,
            wobble,
            pivot_marker,
            axes_visible: true,
            // The tilted orange checker cylinder is the 9th object pushed into the vec (index 8).
            spinning_cylinder: 8,
            cylinder_axes,
        }
    }

    /// Moves the animated objects to where they are `time` seconds after the start, then
    /// refreshes the cached bounding boxes. Call once per frame, before `render`.
    pub fn animate(&mut self, time: f64) {
        let w = &self.wobble;
        let angle = w.amplitude * (TAU * w.frequency * time).sin();

        // Tilting about the y axis. A positive angle tips the top toward +x, so the can rests
        // on the +x edge of its base; a negative angle, the -x edge. Rotating the upright can
        // about that rim point keeps the rim on the floor.
        let rotation = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), angle);
        let pivot = w.rest_position + Vector3::new(w.base_radius * angle.signum(), 0.0, 0.0);
        let position = pivot + rotation * (w.rest_position - pivot);

        let transform = self.objects.get_mut(w.object).expect("wobble object index").transform_mut();
        transform.set_rotation(rotation);
        transform.set_translation(position);

        // Move the pivot axis frame and apply current visibility.
        {
            let pm_obj = self.objects.get_mut(self.pivot_marker)
                .expect("pivot marker index");
            pm_obj.set_visible(self.axes_visible);
            let pm = pm_obj.transform_mut();
            pm.set_rotation(rotation);
            pm.set_translation(Vector3::new(pivot.x, pivot.y, w.rest_position.z + 0.03));
        }

        // Spin the orange checker cylinder around its own axis (one revolution every 4 s).
        // Compose the fixed tilt with a time-varying Z rotation so the tilt is preserved.
        // spin_z: continuous longitudinal spin (checker pattern rotates around cylinder axis).
        // spin_x: outer world-X rotation applied after the tilt+spin, so the whole
        //         spinning cylinder tilts around X without disturbing the longitudinal spin.
        let tilt   = UnitQuaternion::from_euler_angles(PI / 3.0, 0.0, FRAC_PI_6);
        let spin_z = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), time * TAU / 4.0);
        let spin_x = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), time * TAU / 15.0);
        self.objects.get_mut(self.spinning_cylinder)
            .expect("spinning cylinder")
            .transform_mut()
            .set_rotation(spin_x * tilt * spin_z);

        // Cylinder axes: same rotation as the cylinder, visibility follows axes_visible.
        {
            let ca = self.objects.get_mut(self.cylinder_axes)
                .expect("cylinder axes");
            ca.set_visible(self.axes_visible);
            ca.transform_mut().set_rotation(spin_x * tilt * spin_z);
        }

        self.objects.refresh_bounds();
    }

    /// The closest hit along `ray` across all objects in the scene.
    pub fn closest_hit(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        self.objects.closest_hit(ray)
    }

    /// Color seen along `ray`. Materials call this recursively for reflections and
    /// refraction, with `depth` counting the bounces so far and `weight` the share of the
    /// final pixel this ray carries (1.0 for camera rays).
    pub fn trace(&self, ray: &Ray, depth: u32, weight: f64) -> Color {
        match self.closest_hit(ray) {
            Some(hit) => hit.material.compute_color(self, ray, &hit, depth, weight),
            None => Color::default(), // Background is black.
        }
    }
}

impl SceneContext for Scene {
    fn trace(&self, ray: &Ray, depth: u32, weight: f64) -> Color { self.trace(ray, depth, weight) }
    fn scene_lights(&self) -> &[Box<dyn Illuminate>] { &self.lights }
    fn scene_objects(&self) -> &ObjectList { &self.objects }
}

impl Scene {  // re-open for render

    pub fn render(&self,  buffer: &mut [u8])
    {

        let x_fact: f64 = 2.0 / self.x_size as f64;
        let y_fact: f64 = 2.0 / self.y_size as f64;

        // Each row is independent, so rows are traced in parallel across all CPU cores.
        let row_bytes = (self.x_size * 4) as usize;
        buffer
            .par_chunks_exact_mut(row_bytes)
            .enumerate()
            .for_each(|(y, row)| {
                // Buffer row 0 is the top of the window, but +v on the projection screen is up.
                let norm_y: f64 = 1.0 - y as f64 * y_fact;

                for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let norm_x: f64 = x as f64 * x_fact - 1.0;
                    let ray = self.camera.generate_ray(norm_x, norm_y);

                    let color = self.trace(&ray, 0, 1.0);

                    // Colors are 0.0..=1.0; the `as u8` cast saturates anything out of range.
                    pixel[0] = (color.r * 255.0) as u8; // R
                    pixel[1] = (color.g * 255.0) as u8; // G
                    pixel[2] = (color.b * 255.0) as u8; // B
                    pixel[3] = 255; // A
                }
            });
    }
}

/// A spray can built from three shapes, as in the C++ qbRayTrace2 composite example.
/// Local space: base at the origin, standing 2.7 tall along +z with radius 0.4.
/// `body_translucency` 0.0 = opaque, 0.85 = mostly see-through (IOR 1.1, minimal distortion).
fn spray_can(transform: Transform, body_color: Color, body_translucency: f64) -> Composite {
    let body = Transform::new(Vector3::new(0.0, 0.0, 1.0), Vector3::new(SPRAY_CAN_RADIUS, SPRAY_CAN_RADIUS, 1.0));

    // Cone flipped to point up, with its base sitting on top of the body.
    let mut shoulder = Transform::new(Vector3::new(0.0, 0.0, 2.5), Vector3::new(0.4, 0.4, 0.5));
    shoulder.set_rotation_euler(PI, 0.0, 0.0);

    let nozzle = Transform::new(Vector3::new(0.0, 0.0, 2.55), Vector3::new(0.12, 0.12, 0.15));

    let body_mat = Material::flat(body_color)
        .with_reflectivity(0.15)
        .with_specular(1.0, 32.0)
        .with_translucency(body_translucency, 1.1);

    Composite::new(transform)
        .with_shape(Cylinder::new(body, body_mat))
        .with_shape(Cone::new(shoulder, Material::flat(Color::new(0.7, 0.7, 0.7))
            .with_reflectivity(0.6)
            .with_specular(1.0, 64.0)))
        .with_shape(Cylinder::new(nozzle, Material::flat(Color::new(0.95, 0.95, 0.95))
            .with_specular(0.5, 16.0)))
}

/// A cylinder-shaft + cone-tip arrow along the composite's local +Z axis.
/// Shaft runs z ∈ [0, SHAFT_LEN]; tip apex is at z = SHAFT_LEN + TIP_LEN.
fn arrow(transform: Transform, color: Color) -> Composite {
    const SHAFT_LEN: f64 = 0.8;
    const SHAFT_R:   f64 = 0.06;
    const TIP_LEN:   f64 = 0.25;
    const TIP_R:     f64 = 0.16;

    // Cylinder spans z ∈ [-1, 1] in local space; shift its centre to SHAFT_LEN/2 so it
    // runs from 0 to SHAFT_LEN in the arrow's local space.
    let shaft_t = Transform::new(
        Vector3::new(0.0, 0.0, SHAFT_LEN / 2.0),
        Vector3::new(SHAFT_R, SHAFT_R, SHAFT_LEN / 2.0),
    );

    // Cone: apex at z = 0 (local), base at z = 1 (local). Rotating PI about X negates z,
    // so the base ends up at z = -TIP_LEN (local after scale) and the apex stays at z = 0.
    // Translating to z = SHAFT_LEN + TIP_LEN puts:
    //   apex  at z = SHAFT_LEN + TIP_LEN  (the arrow tip)
    //   base  at z = SHAFT_LEN            (flush with the shaft top)
    let mut tip_t = Transform::new(
        Vector3::new(0.0, 0.0, SHAFT_LEN + TIP_LEN),
        Vector3::new(TIP_R, TIP_R, TIP_LEN),
    );
    tip_t.set_rotation_euler(PI, 0.0, 0.0);

    Composite::new(transform)
        .with_shape(Cylinder::new(shaft_t, Material::flat(color).with_specular(0.8, 32.0)))
        .with_shape(Cone::new(tip_t, Material::flat(color).with_specular(0.8, 32.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wobbling can's base rim must touch the floor (z = -1) at all times, never sinking in.
    #[test]
    fn wobbling_can_rests_on_the_floor() {
        let mut scene = Scene::new();
        for step in 0..=40 {
            let time = step as f64 * 0.05;
            scene.animate(time);
            let transform = scene.objects.get(scene.wobble.object).unwrap().transform();

            // Points around the base rim, in the can's local space (base at z = 0).
            let lowest = (0..360)
                .map(|deg| {
                    let a = (deg as f64).to_radians();
                    let rim = Vector3::new(SPRAY_CAN_RADIUS * a.cos(), SPRAY_CAN_RADIUS * a.sin(), 0.0);
                    transform.point_to_world(&rim).z
                })
                .fold(f64::INFINITY, f64::min);
            assert!((lowest - -1.0).abs() < 1e-9, "t = {time}: lowest rim point at z = {lowest}");
        }
    }

    /// At time 0 the can is upright, exactly as built.
    #[test]
    fn animate_at_time_zero_leaves_the_scene_unchanged() {
        let mut scene = Scene::new();
        let before = scene.objects.get(scene.wobble.object).unwrap().transform().translation();
        scene.animate(0.0);
        let transform = scene.objects.get(scene.wobble.object).unwrap().transform();
        assert!((transform.translation() - before).norm() < 1e-12);
        assert!(transform.rotation().angle() < 1e-12);
    }
}
