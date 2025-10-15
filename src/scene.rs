use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI};

use crate::sphere::Sphere;
use crate::plane::Plane;
use crate::cuboid::Cuboid;
use crate::cylinder::Cylinder;
use crate::cone::Cone;
use crate::camera::Camera;
use crate::object_base::Intersect;
use crate::light_base::Illuminate;
use crate::point_light::PointLight;
use crate::ambient_light::AmbientLight;
use crate::ray::Color;
use crate::transform::Transform;
use nalgebra::{Point3, Vector3};

pub struct Scene {
    pub camera: Camera,
    pub objects: Vec<Box<dyn Intersect>>,
    pub lights: Vec<Box<dyn Illuminate>>,
    pub x_size: u32,
    pub y_size: u32,

}

impl Scene {

    pub fn new() -> Self {
        // The default camera sees roughly -2.5 <= x <= 2.5 and -1.4 <= z <= 1.4 at y = 0,
        // so the objects are small and sit in a row on a floor at z = -1.

        let floor = Transform::new(Vector3::new(0.0, 0.0, -1.0), Vector3::new(4.0, 4.0, 1.0));

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

        Scene {
            camera: Camera::default(),
            objects: vec![
                Box::new(Plane::new(floor, Color { r: 0.8, g: 0.8, b: 0.8 })),
                Box::new(Sphere::new(sphere, Color { r: 1.0, g: 0.0, b: 0.0 })),
                Box::new(Cuboid::new(cuboid, Color { r: 0.0, g: 1.0, b: 0.0 })),
                Box::new(Cylinder::new(cylinder, Color { r: 0.2, g: 0.4, b: 1.0 })),
                Box::new(Cone::new(cone, Color { r: 1.0, g: 0.8, b: 0.0 })),
                Box::new(Plane::new(panel, Color { r: 1.0, g: 1.0, b: 1.0 })),
                Box::new(Sphere::new(ellipsoid, Color { r: 1.0, g: 0.2, b: 0.8 })),
                Box::new(Cuboid::new(tilted_cuboid, Color { r: 0.0, g: 0.9, b: 0.9 })),
                Box::new(Cylinder::new(tilted_cylinder, Color { r: 1.0, g: 0.5, b: 0.0 })),
                Box::new(Cone::new(tilted_cone, Color { r: 0.6, g: 0.3, b: 1.0 })),
            ],
            lights: vec![
                Box::new(PointLight::new(Point3::new(5.0, -10.0, 5.0), 1.0)),
                Box::new(AmbientLight::new(0.15)),
            ],
            x_size: 1280,
            y_size: 720,
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

                    // Background is black.
                    let mut color = Color::default();

                    // The visible object is the closest one the ray hits.
                    let closest = self.objects
                        .iter()
                        .filter_map(|object| object.test_intersection(&ray))
                        .min_by(|a, b| {
                            let dist_a = (a.intersection - ray.point1).norm_squared();
                            let dist_b = (b.intersection - ray.point1).norm_squared();
                            dist_a.total_cmp(&dist_b)
                        });

                    if let Some(intersection) = closest {
                        // Light contributions add up; lights that don't reach this point return None.
                        let intensity: f64 = self.lights
                            .iter()
                            .filter_map(|light| light.compute_illumination(&intersection))
                            .map(|illumination| illumination.intensity)
                            .sum();

                        color = Color {
                            r: intersection.color.r * intensity,
                            g: intersection.color.g * intensity,
                            b: intersection.color.b * intensity,
                        };
                    }

                    // Colors are 0.0..=1.0; the `as u8` cast saturates anything out of range.
                    buffer[pixel_index] = (color.r * 255.0) as u8; // R
                    buffer[pixel_index + 1] = (color.g * 255.0) as u8; // G
                    buffer[pixel_index + 2] = (color.b * 255.0) as u8; // B
                    buffer[pixel_index + 3] = 255; // A
              }
        }

    }
}
