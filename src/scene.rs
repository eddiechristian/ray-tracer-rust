use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI};

use crate::sphere::Sphere;
use crate::plane::Plane;
use crate::cuboid::Cuboid;
use crate::cylinder::Cylinder;
use crate::cone::Cone;
use crate::composite::Composite;
use crate::camera::Camera;
use crate::object_base::{closest_hit, Intersect, IntersectionResult};
use crate::material::Material;
use crate::texture::Checker;
use crate::light_base::Illuminate;
use crate::point_light::PointLight;
use crate::ambient_light::AmbientLight;
use crate::ray::{Color, Ray};
use crate::transform::Transform;
use nalgebra::{Point3, Vector2, Vector3};

pub struct Scene {
    pub camera: Camera,
    pub objects: Vec<Box<dyn Intersect>>,
    pub lights: Vec<Box<dyn Illuminate>>,
    pub x_size: u32,
    pub y_size: u32,

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

        Scene {
            camera: Camera::default(),
            objects: vec![
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

                // Composites: one standing on the floor, one floating and tilted as a whole.
                Box::new(spray_can(
                    Transform::new(Vector3::new(-3.0, 0.0, -1.0), Vector3::repeat(0.5)),
                    Color::new(0.9, 0.1, 0.1),
                )),
                Box::new(spray_can(tilted_can, Color::new(0.1, 0.3, 0.9))),
            ],
            lights: vec![
                Box::new(PointLight::new(Point3::new(5.0, -10.0, 5.0), 1.0)),
                Box::new(AmbientLight::new(0.15)),
            ],
            x_size: 1280,
            y_size: 720,
        }
    }

    /// The closest hit along `ray` across all objects in the scene.
    pub fn closest_hit(&self, ray: &Ray) -> Option<IntersectionResult<'_>> {
        closest_hit(&self.objects, ray)
    }

    /// Color seen along `ray`. Materials call this recursively for reflections and
    /// refraction, with `depth` counting the bounces so far.
    pub fn trace(&self, ray: &Ray, depth: u32) -> Color {
        match self.closest_hit(ray) {
            Some(hit) => hit.material.compute_color(self, ray, &hit, depth),
            None => Color::default(), // Background is black.
        }
    }

    pub fn render(&self,  buffer: &mut [u8])
    {

        let x_fact: f64 = 2.0 / self.x_size as f64;
        let y_fact: f64 = 2.0 / self.y_size as f64;

        for y in 0..self.y_size {
              for x in 0..self.x_size {
                    let norm_x: f64 = x as f64 * x_fact - 1.0;
                    // Buffer row 0 is the top of the window, but +v on the projection screen is up.
                    let norm_y: f64 = 1.0 - y as f64 * y_fact;
                    let ray = self.camera.generate_ray(norm_x, norm_y);
                    let pixel_index = (((y * self.x_size) + x) * 4) as usize;

                    let color = self.trace(&ray, 0);

                    // Colors are 0.0..=1.0; the `as u8` cast saturates anything out of range.
                    buffer[pixel_index] = (color.r * 255.0) as u8; // R
                    buffer[pixel_index + 1] = (color.g * 255.0) as u8; // G
                    buffer[pixel_index + 2] = (color.b * 255.0) as u8; // B
                    buffer[pixel_index + 3] = 255; // A
              }
        }

    }
}

/// A spray can built from three shapes, as in the C++ qbRayTrace2 composite example.
/// Local space: base at the origin, standing 2.7 tall along +z with radius 0.4.
fn spray_can(transform: Transform, body_color: Color) -> Composite {
    let body = Transform::new(Vector3::new(0.0, 0.0, 1.0), Vector3::new(0.4, 0.4, 1.0));

    // Cone flipped to point up, with its base sitting on top of the body.
    let mut shoulder = Transform::new(Vector3::new(0.0, 0.0, 2.5), Vector3::new(0.4, 0.4, 0.5));
    shoulder.set_rotation_euler(PI, 0.0, 0.0);

    let nozzle = Transform::new(Vector3::new(0.0, 0.0, 2.55), Vector3::new(0.12, 0.12, 0.15));

    Composite::new(transform)
        .with_shape(Cylinder::new(body, Material::flat(body_color)
            .with_reflectivity(0.15)
            .with_specular(1.0, 32.0)))
        .with_shape(Cone::new(shoulder, Material::flat(Color::new(0.7, 0.7, 0.7))
            .with_reflectivity(0.6)
            .with_specular(1.0, 64.0)))
        .with_shape(Cylinder::new(nozzle, Material::flat(Color::new(0.95, 0.95, 0.95))
            .with_specular(0.5, 16.0)))
}
